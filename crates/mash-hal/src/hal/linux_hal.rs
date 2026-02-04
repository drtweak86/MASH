//! Linux HAL implementation using real system calls.

use super::{
    BtrfsOps, FlashOps, FlashOptions, FormatOps, FormatOptions, HostInfoOps, LoopOps, MountOps,
    MountOptions, OsReleaseInfo, PartedOp, PartedOptions, PartitionOps, ProbeOps, ProcessOps,
    RsyncOps, RsyncOptions, SystemOps, WipeFsOptions,
};
use crate::{HalError, HalResult};
use fatfs::{format_volume, FatType, FormatVolumeOptions};
use gpt::{disk::LogicalBlockSize, partition_types, GptConfig};
use mbrman::{MBRPartitionEntry, CHS};
use std::fs;
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};
use wait_timeout::ChildExt;
#[cfg(unix)]
use {
    std::ffi::CString, std::os::unix::ffi::OsStrExt, std::os::unix::fs::FileTypeExt,
    std::os::unix::io::AsRawFd,
};

/// Real HAL implementation for Linux systems.
#[derive(Debug, Clone, Default)]
pub struct LinuxHal;

impl LinuxHal {
    pub fn new() -> Self {
        Self
    }
}

impl HostInfoOps for LinuxHal {
    fn hostname(&self) -> HalResult<Option<String>> {
        let s = fs::read_to_string("/etc/hostname")?;
        let trimmed = s.trim().to_string();
        Ok((!trimmed.is_empty()).then_some(trimmed))
    }

    fn kernel_release(&self) -> HalResult<Option<String>> {
        let out = self.command_output("uname", &["-r"], Duration::from_secs(2))?;
        let s = String::from_utf8(out.stdout)?.trim().to_string();
        Ok((!s.is_empty()).then_some(s))
    }

    fn os_release(&self) -> HalResult<OsReleaseInfo> {
        let content = fs::read_to_string("/etc/os-release").unwrap_or_default();
        let mut id = None;
        let mut version_id = None;
        for line in content.lines() {
            if let Some(v) = line.strip_prefix("ID=") {
                id = Some(v.trim().trim_matches('"').to_string());
            }
            if let Some(v) = line.strip_prefix("VERSION_ID=") {
                version_id = Some(v.trim().trim_matches('"').to_string());
            }
        }
        Ok(OsReleaseInfo { id, version_id })
    }

    fn proc_cmdline(&self) -> HalResult<String> {
        Ok(fs::read_to_string("/proc/cmdline").unwrap_or_default())
    }

    fn proc_cpuinfo(&self) -> HalResult<String> {
        Ok(fs::read_to_string("/proc/cpuinfo").unwrap_or_default())
    }

    fn proc_meminfo(&self) -> HalResult<String> {
        Ok(fs::read_to_string("/proc/meminfo").unwrap_or_default())
    }

    fn proc_mounts(&self) -> HalResult<String> {
        Ok(fs::read_to_string("/proc/self/mounts").unwrap_or_default())
    }

    fn proc_mountinfo(&self) -> HalResult<String> {
        Ok(fs::read_to_string("/proc/self/mountinfo").unwrap_or_default())
    }
}

impl ProcessOps for LinuxHal {
    fn command_output_with_cwd(
        &self,
        program: &str,
        args: &[&str],
        cwd: Option<&Path>,
        timeout: Duration,
    ) -> HalResult<Output> {
        let mut cmd = Command::new(program);
        cmd.args(args);
        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }
        output_with_timeout(program, &mut cmd, timeout)
    }

    fn command_status_with_cwd(
        &self,
        program: &str,
        args: &[&str],
        cwd: Option<&Path>,
        timeout: Duration,
    ) -> HalResult<()> {
        let mut cmd = Command::new(program);
        cmd.args(args);
        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }
        status_with_timeout(program, &mut cmd, timeout)
    }
}

const FORMAT_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const PARTED_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const BTRFS_TIMEOUT: Duration = Duration::from_secs(60);
const RSYNC_IDLE_TIMEOUT: Duration = Duration::from_secs(120);
const RSYNC_MAX_TIMEOUT: Duration = Duration::from_secs(6 * 60 * 60);

// Loop ioctl constants (from linux/loop.h)
const LOOP_CTL_GET_FREE: libc::c_ulong = 0x4C82;
const LOOP_SET_FD: libc::c_ulong = 0x4C00;
const LOOP_CLR_FD: libc::c_ulong = 0x4C01;
const LOOP_SET_STATUS64: libc::c_ulong = 0x4C04;
const LO_NAME_SIZE: usize = 64;
const LO_KEY_SIZE: usize = 32;
const LO_FLAGS_AUTOCLEAR: u32 = 1 << 2;
const LO_FLAGS_PARTSCAN: u32 = 1 << 3;

// Block ioctl constants
const BLKRRPART: libc::c_ulong = 0x125f;

#[repr(C)]
#[derive(Clone, Copy)]
struct LoopInfo64 {
    lo_device: u64,
    lo_inode: u64,
    lo_rdevice: u64,
    lo_offset: u64,
    lo_sizelimit: u64,
    lo_number: u32,
    lo_encrypt_type: u32,
    lo_encrypt_key_size: u32,
    lo_flags: u32,
    lo_file_name: [u8; LO_NAME_SIZE],
    lo_crypt_name: [u8; LO_NAME_SIZE],
    lo_encrypt_key: [u8; LO_KEY_SIZE],
    lo_init: [u64; 2],
}

impl Default for LoopInfo64 {
    fn default() -> Self {
        LoopInfo64 {
            lo_device: 0,
            lo_inode: 0,
            lo_rdevice: 0,
            lo_offset: 0,
            lo_sizelimit: 0,
            lo_number: 0,
            lo_encrypt_type: 0,
            lo_encrypt_key_size: 0,
            lo_flags: 0,
            lo_file_name: [0; LO_NAME_SIZE],
            lo_crypt_name: [0; LO_NAME_SIZE],
            lo_encrypt_key: [0; LO_KEY_SIZE],
            lo_init: [0; 2],
        }
    }
}

fn map_command_err(program: &str, err: std::io::Error) -> HalError {
    if err.kind() == std::io::ErrorKind::NotFound {
        return HalError::CommandNotFound(program.to_string());
    }
    HalError::Io(err)
}

fn output_failed(program: &str, output: &Output) -> HalError {
    HalError::CommandFailed {
        program: program.to_string(),
        code: output.status.code(),
        stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
    }
}

fn output_with_timeout(program: &str, cmd: &mut Command, timeout: Duration) -> HalResult<Output> {
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| map_command_err(program, e))?;

    let mut stdout = child.stdout.take();
    let mut stderr = child.stderr.take();

    // Drain pipes concurrently to avoid deadlocks on large output.
    let stdout_handle = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut out) = stdout.take() {
            let _ = out.read_to_end(&mut buf);
        }
        buf
    });
    let stderr_handle = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut err) = stderr.take() {
            let _ = err.read_to_end(&mut buf);
        }
        buf
    });

    let status = match child.wait_timeout(timeout).map_err(HalError::Io)? {
        Some(status) => status,
        None => {
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdout_handle.join();
            let _ = stderr_handle.join();
            return Err(HalError::CommandTimeout {
                program: program.to_string(),
                timeout_secs: timeout.as_secs(),
            });
        }
    };

    let stdout = stdout_handle.join().unwrap_or_default();
    let stderr = stderr_handle.join().unwrap_or_default();
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

fn status_with_timeout(program: &str, cmd: &mut Command, timeout: Duration) -> HalResult<()> {
    let output = output_with_timeout(program, cmd, timeout)?;
    if !output.status.success() {
        return Err(output_failed(program, &output));
    }
    Ok(())
}

fn map_nix_err(err: nix::errno::Errno) -> HalError {
    use nix::errno::Errno;
    match err {
        Errno::EBUSY => HalError::DiskBusy,
        Errno::EACCES | Errno::EPERM => HalError::PermissionDenied,
        other => HalError::Nix(other),
    }
}

impl MountOps for LinuxHal {
    fn mount_device(
        &self,
        device: &Path,
        target: &Path,
        fstype: Option<&str>,
        options: MountOptions,
        dry_run: bool,
    ) -> HalResult<()> {
        if dry_run {
            log::info!(
                "DRY RUN: mount {} -> {}",
                device.display(),
                target.display()
            );
            return Ok(());
        }

        if self.is_mounted(target)? {
            log::info!(
                "mount skipped: {} already mounted at {}",
                device.display(),
                target.display()
            );
            return Ok(());
        }

        // Use nix::mount for the actual mounting
        let flags = nix::mount::MsFlags::empty();
        let data = options.options.as_deref();

        nix::mount::mount(Some(device), target, fstype, flags, data).map_err(map_nix_err)?;

        Ok(())
    }

    fn unmount(&self, target: &Path, dry_run: bool) -> HalResult<()> {
        if dry_run {
            log::info!("DRY RUN: unmount {}", target.display());
            return Ok(());
        }

        if !self.is_mounted(target)? {
            return Ok(());
        }

        match nix::mount::umount2(target, nix::mount::MntFlags::empty()) {
            Ok(_) => Ok(()),
            Err(nix::errno::Errno::EINVAL) => Ok(()),
            Err(err) => Err(map_nix_err(err)),
        }?;

        Ok(())
    }

    fn unmount_recursive(&self, target: &Path, dry_run: bool) -> HalResult<()> {
        if dry_run {
            log::info!("DRY RUN: unmount -R {}", target.display());
            return Ok(());
        }

        if !self.is_mounted(target)? {
            return Ok(());
        }

        // Read current mount table and unmount deepest-first for anything under `target`.
        let content = fs::read_to_string("/proc/self/mountinfo")?;
        let entries = crate::procfs::mountinfo::parse_mountinfo(&content);

        let mut under: Vec<std::path::PathBuf> = entries
            .iter()
            .map(|e| e.mount_point.clone())
            .filter(|mp| mp == target || mp.starts_with(target))
            .collect();

        // Unmount deepest paths first.
        under.sort_by_key(|p| std::cmp::Reverse(p.components().count()));

        for mp in under {
            // Ignore errors for already-unmounted paths; bubble up anything else.
            if let Err(err) = nix::mount::umount2(&mp, nix::mount::MntFlags::empty()) {
                if err != nix::errno::Errno::EINVAL {
                    return Err(map_nix_err(err));
                }
            }
        }

        Ok(())
    }

    fn is_mounted(&self, path: &Path) -> HalResult<bool> {
        let content = fs::read_to_string("/proc/self/mountinfo")?;
        let entries = crate::procfs::mountinfo::parse_mountinfo(&content);
        Ok(crate::procfs::mountinfo::is_mounted_from_info(
            path, &entries,
        ))
    }
}

impl FormatOps for LinuxHal {
    fn format_ext4(&self, device: &Path, opts: &FormatOptions) -> HalResult<()> {
        if opts.dry_run {
            log::info!("DRY RUN: mkfs.ext4 {}", device.display());
            return Ok(());
        }

        if !opts.confirmed {
            return Err(HalError::SafetyLock);
        }

        let mut args = opts.extra_args.clone();
        args.push(device.display().to_string());

        let mut cmd = Command::new("mkfs.ext4");
        cmd.args(&args);
        let output = output_with_timeout("mkfs.ext4", &mut cmd, FORMAT_TIMEOUT)?;

        if !output.status.success() {
            return Err(output_failed("mkfs.ext4", &output));
        }

        Ok(())
    }

    fn format_btrfs(&self, device: &Path, opts: &FormatOptions) -> HalResult<()> {
        if opts.dry_run {
            log::info!("DRY RUN: mkfs.btrfs {}", device.display());
            return Ok(());
        }

        if !opts.confirmed {
            return Err(HalError::SafetyLock);
        }

        let mut args = opts.extra_args.clone();
        args.push(device.display().to_string());

        let mut cmd = Command::new("mkfs.btrfs");
        cmd.args(&args);
        let output = output_with_timeout("mkfs.btrfs", &mut cmd, FORMAT_TIMEOUT)?;

        if !output.status.success() {
            return Err(output_failed("mkfs.btrfs", &output));
        }

        Ok(())
    }

    fn format_vfat(&self, device: &Path, label: &str, opts: &FormatOptions) -> HalResult<()> {
        if opts.dry_run {
            log::info!("DRY RUN: mkfs.vfat {} ({})", device.display(), label);
            return Ok(());
        }

        if !opts.confirmed {
            return Err(HalError::SafetyLock);
        }

        if !opts.extra_args.is_empty() {
            log::warn!("format_vfat ignoring extra_args; native formatter handles defaults");
        }

        let mut file = fs::OpenOptions::new().read(true).write(true).open(device)?;

        let vol_label = normalize_vfat_label(label);
        let opts = FormatVolumeOptions::new()
            .fat_type(FatType::Fat32)
            .volume_label(vol_label);
        format_volume(&mut file, opts)
            .map_err(|e| HalError::Other(format!("fatfs format failed: {}", e)))?;

        // Best-effort flush for block devices or files.
        let _ = file.sync_all();

        Ok(())
    }
}

impl FlashOps for LinuxHal {
    fn flash_raw_image(
        &self,
        image_path: &Path,
        target_disk: &Path,
        opts: &FlashOptions,
    ) -> HalResult<()> {
        if opts.dry_run {
            log::info!(
                "DRY RUN: flash {} -> {}",
                image_path.display(),
                target_disk.display()
            );
            return Ok(());
        }

        if !opts.confirmed {
            return Err(HalError::SafetyLock);
        }

        log::info!(
            "💾 Flashing image {} -> {}",
            image_path.display(),
            target_disk.display()
        );

        let input = fs::File::open(image_path)?;

        let mut reader: Box<dyn Read> = if image_path.extension().is_some_and(|e| e == "xz") {
            Box::new(xz2::read::XzDecoder::new(input))
        } else {
            Box::new(input)
        };

        let mut out = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(target_disk)?;

        // For regular files (CI tests), truncate; for block devices, this may fail and is fine.
        let _ = out.set_len(0);

        io::copy(&mut reader, &mut out)?;

        // Best-effort flush (block devices may ignore).
        out.sync_all().ok();

        Ok(())
    }
}

impl SystemOps for LinuxHal {
    fn sync(&self) -> HalResult<()> {
        // Direct syscall to avoid external command dependency.
        unsafe {
            libc::sync();
        }
        Ok(())
    }

    fn udev_settle(&self) -> HalResult<()> {
        // Best-effort wait: small sleep to allow udev to settle without external command.
        std::thread::sleep(Duration::from_millis(150));
        Ok(())
    }
}

impl ProbeOps for LinuxHal {
    fn lsblk_mountpoints(&self, disk: &Path) -> HalResult<Vec<std::path::PathBuf>> {
        let content = fs::read_to_string("/proc/self/mountinfo")?;
        let mounts = crate::procfs::mountinfo::mounted_under_device(&content, disk)
            .into_iter()
            .map(std::path::PathBuf::from)
            .collect();
        Ok(mounts)
    }

    fn lsblk_table(&self, disk: &Path) -> HalResult<String> {
        let devices = crate::sysfs::block::scan_block_devices()
            .map_err(|e| HalError::Other(e.to_string()))?;
        let name = disk
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| HalError::Parse(format!("invalid disk path {}", disk.display())))?;
        let info = devices
            .into_iter()
            .find(|d| d.name == name)
            .ok_or_else(|| HalError::Other(format!("disk {} not found", disk.display())))?;

        let mounts = self.lsblk_mountpoints(disk)?;
        let mounts_str = if mounts.is_empty() {
            "-".to_string()
        } else {
            mounts
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        };
        let model = info
            .model
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or("unknown");
        let serial = info
            .serial
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or("n/a");
        let size_gib = (info.size_bytes as f64) / (1024.0 * 1024.0 * 1024.0);

        Ok(format!(
            "NAME\tSIZE(GB)\tTYPE\tMODEL\tSERIAL\tMOUNTS\n{}\t{:.2}\tdisk\t{}\t{}\t{}",
            info.name, size_gib, model, serial, mounts_str
        ))
    }

    fn blkid_uuid(&self, device: &Path) -> HalResult<String> {
        uuid_for_device_in(Path::new("/dev/disk/by-uuid"), device)
            .ok_or_else(|| HalError::Other(format!("UUID not found for {}", device.display())))
    }
}

impl PartitionOps for LinuxHal {
    fn wipefs_all(&self, disk: &Path, opts: &WipeFsOptions) -> HalResult<()> {
        if opts.dry_run {
            log::info!("DRY RUN: wipefs -a {}", disk.display());
            return Ok(());
        }
        if !opts.confirmed {
            return Err(HalError::SafetyLock);
        }

        let mut file = fs::OpenOptions::new().read(true).write(true).open(disk)?;

        zero_region(&mut file, 0, 1 << 20)?; // first 1 MiB

        if let Ok(len) = file.metadata().map(|m| m.len()) {
            if len > (1 << 20) {
                let tail_start = len.saturating_sub(1 << 20);
                zero_region(&mut file, tail_start, 1 << 20)?;
            }
        }

        // Best-effort partition table re-read for block devices.
        if file
            .metadata()
            .ok()
            .map(|m| m.file_type().is_block_device())
            .unwrap_or(false)
        {
            unsafe { libc::ioctl(file.as_raw_fd(), BLKRRPART) };
        }

        Ok(())
    }

    fn parted(&self, disk: &Path, op: PartedOp, opts: &PartedOptions) -> HalResult<String> {
        if opts.dry_run {
            log::info!("DRY RUN: parted -s {} {:?}", disk.display(), op);
            return Ok(String::new());
        }
        if !opts.confirmed {
            return Err(HalError::SafetyLock);
        }

        match self.parted_native(disk, &op) {
            Ok(()) => Ok(String::new()),
            Err(err) => {
                log::warn!(
                    "native partition op failed, falling back to parted: {}",
                    err
                );
                match self.parted_fallback(disk, &op) {
                    Ok(s) => Ok(s),
                    Err(_) => Err(err),
                }
            }
        }
    }
}

impl LinuxHal {
    fn parted_native(&self, disk: &Path, op: &PartedOp) -> HalResult<()> {
        match op {
            PartedOp::MkLabel { label } => self.create_label(disk, label),
            PartedOp::MkPart {
                part_type,
                fs_type,
                start,
                end,
            } => self.create_partition(disk, part_type, fs_type, start, end),
            PartedOp::SetFlag { .. } | PartedOp::Print => Err(HalError::Other(
                "native path does not handle this op".into(),
            )),
        }
    }

    fn parted_fallback(&self, disk: &Path, op: &PartedOp) -> HalResult<String> {
        let mut args: Vec<String> = vec!["-s".to_string(), disk.display().to_string()];
        match op {
            PartedOp::MkLabel { label } => {
                args.push("mklabel".to_string());
                args.push(label.clone());
            }
            PartedOp::MkPart {
                part_type,
                fs_type,
                start,
                end,
            } => {
                args.push("-a".to_string());
                args.push("optimal".to_string());
                args.push("mkpart".to_string());
                args.push(part_type.clone());
                args.push(fs_type.clone());
                args.push(start.clone());
                args.push(end.clone());
            }
            PartedOp::SetFlag {
                part_num,
                flag,
                state,
            } => {
                args.push("set".to_string());
                args.push(part_num.to_string());
                args.push(flag.clone());
                args.push(state.clone());
            }
            PartedOp::Print => {
                args.push("print".to_string());
            }
        }

        let mut cmd = Command::new("parted");
        cmd.args(&args);
        let output = output_with_timeout("parted", &mut cmd, PARTED_TIMEOUT)?;
        if !output.status.success() {
            return Err(output_failed("parted", &output));
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    fn create_label(&self, disk: &Path, label: &str) -> HalResult<()> {
        match label {
            l if l.eq_ignore_ascii_case("gpt") => {
                let mut file = fs::OpenOptions::new().read(true).write(true).open(disk)?;
                let cfg = GptConfig::new()
                    .writable(true)
                    .initialized(false)
                    .logical_block_size(LogicalBlockSize::Lb512);
                let gpt = cfg.create_from_device(Box::new(&mut file), None)?;
                gpt.write()?;
                Ok(())
            }
            l if l.eq_ignore_ascii_case("msdos") || l.eq_ignore_ascii_case("mbr") => {
                let mut file = fs::OpenOptions::new().read(true).write(true).open(disk)?;
                let mut mbr = mbrman::MBR::new_from(&mut file, 512, [0u8; 4]).map_err(
                    |e: mbrman::Error| HalError::Other(format!("mbr init failed: {}", e)),
                )?;
                mbr.write_into(&mut file)
                    .map_err(|e: mbrman::Error| HalError::Other(e.to_string()))?;
                Ok(())
            }
            other => Err(HalError::Other(format!("unsupported label {}", other))),
        }
    }

    fn create_partition(
        &self,
        disk: &Path,
        part_type: &str,
        fs_type: &str,
        start: &str,
        end: &str,
    ) -> HalResult<()> {
        let disk_size = fs::metadata(disk)?.len();
        let (start_bytes, end_bytes) = parse_range_mib(start, end, disk_size)
            .ok_or_else(|| HalError::Parse("invalid partition range".into()))?;
        let start_lba = start_bytes / 512;
        let end_lba = end_bytes / 512;
        if end_lba <= start_lba {
            return Err(HalError::ValidationFailed("partition length zero".into()));
        }
        let size_lba = end_lba - start_lba;
        // Try GPT first (existing table).
        if let Ok(mut gpt) = {
            let file = fs::OpenOptions::new().read(true).write(true).open(disk)?;
            GptConfig::new()
                .writable(true)
                .logical_block_size(LogicalBlockSize::Lb512)
                .initialized(true)
                .open_from_device(Box::new(file))
        } {
            let ptype = match fs_type.to_ascii_lowercase().as_str() {
                "fat32" | "efi" => partition_types::EFI,
                _ => partition_types::LINUX_FS,
            };
            gpt.add_partition(part_type, size_lba, ptype, 0u64, Some(2048u64))?;
            gpt.write()?;
            return Ok(());
        }
        // Try existing MBR.
        if let Ok(mut mbr) = {
            let mut file = fs::OpenOptions::new().read(true).write(true).open(disk)?;
            mbrman::MBR::read_from(&mut file, 512)
        } {
            let mut file = fs::OpenOptions::new().read(true).write(true).open(disk)?;
            let entry = MBRPartitionEntry {
                boot: 0,
                first_chs: CHS::empty(),
                sys: 0x83,
                last_chs: CHS::empty(),
                starting_lba: start_lba as u32,
                sectors: size_lba as u32,
            };
            mbr[1] = entry;
            mbr.write_into(&mut file)
                .map_err(|e: mbrman::Error| HalError::Other(e.to_string()))?;
            return Ok(());
        }
        // Fresh GPT creation.
        let mut gpt = {
            let file = fs::OpenOptions::new().read(true).write(true).open(disk)?;
            GptConfig::new()
                .writable(true)
                .initialized(false)
                .logical_block_size(LogicalBlockSize::Lb512)
                .create_from_device(Box::new(file), None)?
        };
        let ptype = match fs_type.to_ascii_lowercase().as_str() {
            "fat32" | "efi" => partition_types::EFI,
            _ => partition_types::LINUX_FS,
        };
        gpt.add_partition(part_type, size_lba, ptype, 0u64, Some(2048u64))?;
        gpt.write()?;
        Ok(())
    }
}

fn parse_range_mib(start: &str, end: &str, disk_size: u64) -> Option<(u64, u64)> {
    let sb = parse_mib_value(start, disk_size)?;
    let eb = if end.trim_end_matches('%') == "100" || end == "100%" {
        disk_size
    } else {
        parse_mib_value(end, disk_size)?
    };
    Some((align_1m(sb), align_1m(eb)))
}

fn parse_mib_value(s: &str, disk_size: u64) -> Option<u64> {
    let t = s.trim().to_ascii_lowercase();
    if t.ends_with("mib") {
        let v: f64 = t.trim_end_matches("mib").trim().parse().ok()?;
        return Some((v * 1024.0 * 1024.0) as u64);
    }
    if t.ends_with('%') {
        let pct: f64 = t.trim_end_matches('%').trim().parse().ok()?;
        return Some(((pct / 100.0) * disk_size as f64) as u64);
    }
    None
}

fn align_1m(bytes: u64) -> u64 {
    const ONE_MIB: u64 = 1024 * 1024;
    bytes.div_ceil(ONE_MIB) * ONE_MIB
}

fn normalize_vfat_label(label: &str) -> [u8; 11] {
    let mut out = [b' '; 11];
    for (i, c) in label
        .trim()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == ' ')
        .map(|c| c.to_ascii_uppercase())
        .take(11)
        .enumerate()
    {
        out[i] = c as u8;
    }
    out
}

impl LoopOps for LinuxHal {
    fn losetup_attach(&self, image: &Path, scan_partitions: bool) -> HalResult<String> {
        // Get a free loop device.
        let ctl_path = CString::new("/dev/loop-control").unwrap();
        let ctl_fd = unsafe { libc::open(ctl_path.as_ptr(), libc::O_RDONLY) };
        if ctl_fd < 0 {
            return Err(HalError::Io(io::Error::last_os_error()));
        }
        let loop_num = unsafe { libc::ioctl(ctl_fd, LOOP_CTL_GET_FREE, 0) };
        unsafe { libc::close(ctl_fd) };
        if loop_num < 0 {
            return Err(HalError::Io(io::Error::last_os_error()));
        }

        let loop_path = format!("/dev/loop{}", loop_num);
        let loop_file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&loop_path)?;
        let img = fs::OpenOptions::new().read(true).open(image)?;

        // Associate file descriptor.
        let res = unsafe { libc::ioctl(loop_file.as_raw_fd(), LOOP_SET_FD, img.as_raw_fd()) };
        if res != 0 {
            return Err(HalError::Io(io::Error::last_os_error()));
        }

        // Configure flags.
        let mut info = LoopInfo64 {
            lo_flags: LO_FLAGS_AUTOCLEAR
                | if scan_partitions {
                    LO_FLAGS_PARTSCAN
                } else {
                    0
                },
            ..LoopInfo64::default()
        };
        if let Ok(cstr) = CString::new(image.as_os_str().as_bytes()) {
            let bytes = cstr.as_bytes();
            let len = bytes.len().min(LO_NAME_SIZE - 1);
            info.lo_file_name[..len].copy_from_slice(&bytes[..len]);
        }
        let res = unsafe {
            libc::ioctl(
                loop_file.as_raw_fd(),
                LOOP_SET_STATUS64,
                &info as *const LoopInfo64,
            )
        };
        if res != 0 {
            return Err(HalError::Io(io::Error::last_os_error()));
        }

        // Trigger partition re-read if requested.
        if scan_partitions {
            let _ = unsafe { libc::ioctl(loop_file.as_raw_fd(), BLKRRPART, 0) };
        }

        Ok(loop_path)
    }

    fn losetup_detach(&self, loop_device: &str) -> HalResult<()> {
        let fd = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(loop_device)?;
        let res = unsafe { libc::ioctl(fd.as_raw_fd(), LOOP_CLR_FD, 0) };
        if res != 0 {
            return Err(HalError::Io(io::Error::last_os_error()));
        }
        Ok(())
    }
}

impl BtrfsOps for LinuxHal {
    fn btrfs_subvolume_list(&self, mount_point: &Path) -> HalResult<String> {
        let mut cmd = Command::new("btrfs");
        cmd.args(["subvolume", "list"]).arg(mount_point);
        let output = output_with_timeout("btrfs", &mut cmd, BTRFS_TIMEOUT)?;
        if !output.status.success() {
            return Err(output_failed("btrfs", &output));
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    fn btrfs_subvolume_create(&self, path: &Path) -> HalResult<()> {
        let mut cmd = Command::new("btrfs");
        cmd.args(["subvolume", "create"]).arg(path);
        let output = output_with_timeout("btrfs", &mut cmd, BTRFS_TIMEOUT)?;
        if !output.status.success() {
            return Err(output_failed("btrfs", &output));
        }
        Ok(())
    }
}

impl RsyncOps for LinuxHal {
    fn rsync_stream_stdout(
        &self,
        src: &Path,
        dst: &Path,
        opts: &RsyncOptions,
        on_stdout_line: &mut dyn FnMut(&str) -> bool,
    ) -> HalResult<()> {
        let mut args: Vec<String> = Vec::new();

        if opts.archive && opts.extra_args.is_empty() {
            // Default to the existing full-loop settings.
            args.push("-aHAX".to_string());
        }

        if opts.numeric_ids {
            args.push("--numeric-ids".to_string());
        }

        if let Some(info) = &opts.info {
            args.push(format!("--info={}", info));
        }

        args.extend(opts.extra_args.iter().cloned());

        // Ensure trailing slash on src to copy contents.
        let src_str = format!("{}/", src.display());
        args.push(src_str);
        args.push(dst.display().to_string());

        let mut child = Command::new("rsync")
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| map_command_err("rsync", e))?;

        // Drain stderr in the background to avoid deadlocks if rsync is chatty.
        let mut stderr_handle = child.stderr.take().map(|stderr| {
            std::thread::spawn(move || {
                let mut s = String::new();
                let mut reader = BufReader::new(stderr);
                let _ = reader.read_to_string(&mut s);
                s
            })
        });

        let (tx, rx) = mpsc::channel::<io::Result<String>>();
        if let Some(stdout) = child.stdout.take() {
            std::thread::spawn(move || {
                let reader = BufReader::new(stdout);
                for line in reader.lines() {
                    if tx.send(line).is_err() {
                        break;
                    }
                }
            });
        }

        let start = Instant::now();
        let mut last_output = Instant::now();
        let watch_idle = opts.info.is_some();
        loop {
            // Hard upper bound.
            if start.elapsed() > RSYNC_MAX_TIMEOUT {
                let _ = child.kill();
                let _ = child.wait();
                if let Some(h) = stderr_handle.take() {
                    let _ = h.join();
                }
                return Err(HalError::CommandTimeout {
                    program: "rsync".to_string(),
                    timeout_secs: RSYNC_MAX_TIMEOUT.as_secs(),
                });
            }

            // Idle detection (no stdout lines observed) only when the caller asked for progress
            // output; otherwise rsync may be intentionally quiet.
            if watch_idle && last_output.elapsed() > RSYNC_IDLE_TIMEOUT {
                let _ = child.kill();
                let _ = child.wait();
                if let Some(h) = stderr_handle.take() {
                    let _ = h.join();
                }
                return Err(HalError::CommandTimeout {
                    program: "rsync".to_string(),
                    timeout_secs: RSYNC_IDLE_TIMEOUT.as_secs(),
                });
            }

            match rx.recv_timeout(Duration::from_millis(250)) {
                Ok(Ok(line)) => {
                    last_output = Instant::now();
                    if !on_stdout_line(&line) {
                        let _ = child.kill();
                        let _ = child.wait();
                        if let Some(h) = stderr_handle.take() {
                            let _ = h.join();
                        }
                        return Err(HalError::Other("rsync cancelled".to_string()));
                    }
                }
                Ok(Err(err)) => return Err(HalError::Io(err)),
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    // Poll for process exit while waiting for output.
                    if let Some(status) = child.try_wait()? {
                        if !status.success() {
                            let stderr_s = stderr_handle
                                .take()
                                .and_then(|h| h.join().ok())
                                .unwrap_or_default();
                            return Err(HalError::CommandFailed {
                                program: "rsync".to_string(),
                                code: status.code(),
                                stderr: stderr_s.trim().to_string(),
                            });
                        }
                        // Success.
                        let _ = stderr_handle.take().and_then(|h| h.join().ok());
                        return Ok(());
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }

        let status = match child
            .wait_timeout(Duration::from_secs(5))
            .map_err(HalError::Io)?
        {
            Some(status) => status,
            None => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(HalError::CommandTimeout {
                    program: "rsync".to_string(),
                    timeout_secs: 5,
                });
            }
        };
        if !status.success() {
            let stderr_s = stderr_handle
                .take()
                .and_then(|h| h.join().ok())
                .unwrap_or_default();
            return Err(HalError::CommandFailed {
                program: "rsync".to_string(),
                code: status.code(),
                stderr: stderr_s.trim().to_string(),
            });
        }
        Ok(())
    }
}

fn uuid_for_device_in(base: &Path, device: &Path) -> Option<String> {
    let target = device.to_path_buf();
    let entries = fs::read_dir(base).ok()?;
    for entry in entries.flatten() {
        let link_path = entry.path();
        let target_path = fs::read_link(&link_path).ok().map(|p| {
            if p.is_absolute() {
                p
            } else {
                link_path.parent().unwrap_or(base).join(p)
            }
        })?;
        if target_path == target {
            if let Some(name) = link_path.file_name().and_then(|s| s.to_str()) {
                return Some(name.to_string());
            }
        }
    }
    None
}

fn zero_region(file: &mut fs::File, offset: u64, length: u64) -> HalResult<()> {
    file.seek(SeekFrom::Start(offset))?;
    let chunk = [0u8; 4096];
    let mut remaining = length;
    while remaining > 0 {
        let write_len = remaining.min(chunk.len() as u64);
        file.write_all(&chunk[..write_len as usize])?;
        remaining -= write_len;
    }
    file.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn format_ext4_requires_confirmation() {
        let hal = LinuxHal::new();
        let opts = FormatOptions::new(false, false);
        let err = hal.format_ext4(Path::new("/dev/null"), &opts).unwrap_err();
        assert!(matches!(err, crate::HalError::SafetyLock));
    }

    #[test]
    fn format_btrfs_requires_confirmation() {
        let hal = LinuxHal::new();
        let opts = FormatOptions::new(false, false);
        let err = hal.format_btrfs(Path::new("/dev/null"), &opts).unwrap_err();
        assert!(matches!(err, crate::HalError::SafetyLock));
    }

    #[test]
    fn format_vfat_requires_confirmation() {
        let hal = LinuxHal::new();
        let opts = FormatOptions::new(false, false);
        let err = hal
            .format_vfat(Path::new("/dev/null"), "EFI", &opts)
            .unwrap_err();
        assert!(matches!(err, crate::HalError::SafetyLock));
    }

    #[test]
    fn format_vfat_creates_mountable_fs() {
        let dir = tempdir().unwrap();
        let img = dir.path().join("fat.img");
        let f = std::fs::File::create(&img).unwrap();
        f.set_len(64 * 1024 * 1024).unwrap(); // 64 MiB for FAT32

        let hal = LinuxHal::new();
        let opts = FormatOptions::new(false, true);
        hal.format_vfat(&img, "EFI", &opts).unwrap();

        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&img)
            .unwrap();
        let fs = fatfs::FileSystem::new(file, fatfs::FsOptions::new()).unwrap();
        let label = String::from_utf8(fs.volume_label_as_bytes().to_vec()).unwrap();
        assert_eq!(label.trim(), "EFI");
    }

    #[test]
    fn flash_raw_image_writes_to_file() {
        let dir = tempdir().unwrap();
        let image = dir.path().join("test.img");
        let target = dir.path().join("target.img");

        std::fs::write(&image, b"test content").unwrap();

        let hal = LinuxHal::new();
        let opts = super::FlashOptions::new(false, true);
        hal.flash_raw_image(&image, &target, &opts).unwrap();

        let result = std::fs::read(&target).unwrap();
        assert_eq!(result, b"test content");
    }

    #[test]
    fn flash_xz_image_decompresses() {
        let dir = tempdir().unwrap();
        let image = dir.path().join("test.img.xz");
        let target = dir.path().join("target.img");

        // Create a simple xz compressed file
        use std::io::Write;
        let mut encoder = xz2::write::XzEncoder::new(Vec::new(), 6);
        encoder.write_all(b"compressed data").unwrap();
        let compressed = encoder.finish().unwrap();
        std::fs::write(&image, compressed).unwrap();

        let hal = LinuxHal::new();
        let opts = super::FlashOptions::new(false, true);
        hal.flash_raw_image(&image, &target, &opts).unwrap();

        let result = std::fs::read(&target).unwrap();
        assert_eq!(result, b"compressed data");
    }

    #[test]
    fn uuid_lookup_finds_match() {
        use std::os::unix::fs::symlink;

        let dir = tempdir().unwrap();
        let dev = dir.path().join("sda1");
        std::fs::write(&dev, b"fake").unwrap();
        let by_uuid = dir.path().join("by-uuid");
        std::fs::create_dir(&by_uuid).unwrap();
        let link = by_uuid.join("UUID-TEST");
        symlink(&dev, &link).unwrap();

        let found = uuid_for_device_in(&by_uuid, &dev);
        assert_eq!(found.as_deref(), Some("UUID-TEST"));
    }

    #[test]
    fn zero_region_overwrites_bytes() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("file.bin");
        std::fs::write(&path, vec![0xAAu8; 8192]).unwrap();

        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        zero_region(&mut file, 0, 4096).unwrap();

        let data = std::fs::read(&path).unwrap();
        assert!(data[..4096].iter().all(|b| *b == 0));
        assert!(data[4096..].iter().all(|b| *b == 0xAA));
    }
}
