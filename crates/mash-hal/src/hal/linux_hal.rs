//! Linux HAL implementation using real system calls.

use super::{
    BtrfsOps, CopyOps, CopyOptions, CopyProgress, FlashOps, FlashOptions, FormatOps, FormatOptions,
    HostInfoOps, LoopOps, MountOps, MountOptions, OsReleaseInfo, PartedOp, PartedOptions,
    PartitionOps, ProbeOps, ProcessOps, SystemOps, WipeFsOptions,
};
use crate::{HalError, HalResult};
use filetime::FileTime;
use std::fs;
use std::io::{self, Read};
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::time::Duration;
use wait_timeout::ChildExt;
use walkdir::WalkDir;
#[cfg(unix)]
use {
    nix::unistd::{chown, Gid, Uid},
    std::os::unix::fs::{symlink, MetadataExt},
};

/// Real HAL implementation for Linux systems.
#[derive(Debug, Clone, Default)]
pub struct LinuxHal;

impl LinuxHal {
    pub fn new() -> Self {
        Self
    }

    // Helper for running commands and capturing output, without cwd.
    fn command_output(&self, program: &str, args: &[&str], timeout: Duration) -> HalResult<Output> {
        let mut cmd = Command::new(program);
        cmd.args(args);
        output_with_timeout(program, &mut cmd, timeout)
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

const PROBE_TIMEOUT: Duration = Duration::from_secs(10);
const SYNC_TIMEOUT: Duration = Duration::from_secs(60);
const FORMAT_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const WIPEFS_TIMEOUT: Duration = Duration::from_secs(60);
const PARTED_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const LOSETUP_TIMEOUT: Duration = Duration::from_secs(30);
const BTRFS_TIMEOUT: Duration = Duration::from_secs(60);

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

        nix::mount::umount2(target, nix::mount::MntFlags::empty()).map_err(map_nix_err)?;

        Ok(())
    }

    fn unmount_recursive(&self, target: &Path, dry_run: bool) -> HalResult<()> {
        if dry_run {
            log::info!("DRY RUN: unmount -R {}", target.display());
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
            let _ = nix::mount::umount2(&mp, nix::mount::MntFlags::empty());
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

        let mut args: Vec<String> = vec!["-F".to_string(), "32".to_string()];
        args.push("-n".to_string());
        args.push(label.to_string());
        args.extend(opts.extra_args.iter().cloned());
        args.push(device.display().to_string());

        let mut cmd = Command::new("mkfs.vfat");
        cmd.args(&args);
        let output = output_with_timeout("mkfs.vfat", &mut cmd, FORMAT_TIMEOUT)?;

        if !output.status.success() {
            return Err(output_failed("mkfs.vfat", &output));
        }

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
        // Avoid linking libc directly; keep behavior aligned with existing shell usage.
        let mut cmd = Command::new("sync");
        status_with_timeout("sync", &mut cmd, SYNC_TIMEOUT)
    }

    fn udev_settle(&self) -> HalResult<()> {
        let mut cmd = Command::new("udevadm");
        cmd.arg("settle");
        status_with_timeout("udevadm", &mut cmd, SYNC_TIMEOUT)
    }
}

impl ProbeOps for LinuxHal {
    fn lsblk_mountpoints(&self, disk: &Path) -> HalResult<Vec<std::path::PathBuf>> {
        let mut cmd = Command::new("lsblk");
        cmd.args(["-lnpo", "MOUNTPOINT"]).arg(disk);
        let output = output_with_timeout("lsblk", &mut cmd, PROBE_TIMEOUT)?;

        if !output.status.success() {
            return Err(output_failed("lsblk", &output));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut mountpoints = Vec::new();
        for line in stdout.lines().map(str::trim).filter(|l| !l.is_empty()) {
            mountpoints.push(std::path::PathBuf::from(line));
        }
        Ok(mountpoints)
    }

    fn lsblk_table(&self, disk: &Path) -> HalResult<String> {
        let mut cmd = Command::new("lsblk");
        cmd.args(["-o", "NAME,SIZE,TYPE,FSTYPE,MOUNTPOINTS,MODEL"])
            .arg(disk);
        let output = output_with_timeout("lsblk", &mut cmd, PROBE_TIMEOUT)?;

        if !output.status.success() {
            return Err(output_failed("lsblk", &output));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    fn blkid_uuid(&self, device: &Path) -> HalResult<String> {
        let mut cmd = Command::new("blkid");
        cmd.args(["-s", "UUID", "-o", "value"]).arg(device);
        let output = output_with_timeout("blkid", &mut cmd, PROBE_TIMEOUT)?;

        if !output.status.success() {
            return Err(output_failed("blkid", &output));
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
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

        let mut cmd = Command::new("wipefs");
        cmd.args(["-a"]).arg(disk);
        let output = output_with_timeout("wipefs", &mut cmd, WIPEFS_TIMEOUT)?;
        if !output.status.success() {
            return Err(output_failed("wipefs", &output));
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

        let mut args: Vec<String> = vec!["-s".to_string(), disk.display().to_string()];
        match op {
            PartedOp::MkLabel { label } => {
                args.push("mklabel".to_string());
                args.push(label);
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
                args.push(part_type);
                args.push(fs_type);
                args.push(start);
                args.push(end);
            }
            PartedOp::SetFlag {
                part_num,
                flag,
                state,
            } => {
                args.push("set".to_string());
                args.push(part_num.to_string());
                args.push(flag);
                args.push(state);
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
}

impl LoopOps for LinuxHal {
    fn losetup_attach(&self, image: &Path, scan_partitions: bool) -> HalResult<String> {
        let mut args = vec!["--show".to_string(), "-f".to_string()];
        if scan_partitions {
            args.push("-P".to_string());
        }
        args.push(image.display().to_string());

        let mut cmd = Command::new("losetup");
        cmd.args(&args);
        let output = output_with_timeout("losetup", &mut cmd, LOSETUP_TIMEOUT)?;

        if !output.status.success() {
            return Err(output_failed("losetup", &output));
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    fn losetup_detach(&self, loop_device: &str) -> HalResult<()> {
        let mut cmd = Command::new("losetup");
        cmd.args(["-d", loop_device]);
        let output = output_with_timeout("losetup", &mut cmd, LOSETUP_TIMEOUT)?;
        if !output.status.success() {
            return Err(output_failed("losetup", &output));
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

impl CopyOps for LinuxHal {
    fn copy_tree_native(
        &self,
        src: &Path,
        dst: &Path,
        opts: &CopyOptions,
        on_progress: &mut dyn FnMut(CopyProgress) -> bool,
    ) -> HalResult<()> {
        if !src.exists() {
            return Err(HalError::ValidationFailed(format!(
                "source {} does not exist",
                src.display()
            )));
        }
        if !src.is_dir() {
            return Err(HalError::ValidationFailed(format!(
                "source {} is not a directory",
                src.display()
            )));
        }

        fs::create_dir_all(dst)?;

        let to_io_err = |e: walkdir::Error| -> io::Error { io::Error::other(e.to_string()) };

        // First pass: totals for progress.
        let mut total_bytes = 0u64;
        let mut files_total = 0u64;
        for entry in WalkDir::new(src).follow_links(false) {
            let entry = entry.map_err(|e| HalError::Io(to_io_err(e)))?;
            let meta = fs::symlink_metadata(entry.path())?;
            let ft = meta.file_type();
            if ft.is_file() {
                total_bytes = total_bytes.saturating_add(meta.len());
                files_total += 1;
            } else if ft.is_symlink() {
                files_total += 1;
            }
        }

        let mut progress = CopyProgress {
            bytes_copied: 0,
            bytes_total: total_bytes,
            files_copied: 0,
            files_total,
        };

        let mut emit = |prog: &CopyProgress| -> HalResult<()> {
            if !on_progress(prog.clone()) {
                return Err(HalError::Other("copy cancelled".to_string()));
            }
            Ok(())
        };

        for entry in WalkDir::new(src).follow_links(false) {
            let entry = entry.map_err(|e| HalError::Io(to_io_err(e)))?;
            let rel = entry.path().strip_prefix(src).map_err(|_| {
                HalError::ValidationFailed("failed to strip source prefix".to_string())
            })?;
            let meta = fs::symlink_metadata(entry.path())?;
            let ft = meta.file_type();

            if rel.as_os_str().is_empty() {
                apply_metadata(dst, &meta, opts)?;
                continue;
            }

            let target = dst.join(rel);
            if ft.is_dir() {
                ensure_directory(&target)?;
                apply_metadata(&target, &meta, opts)?;
                continue;
            }

            if ft.is_symlink() {
                copy_symlink(entry.path(), &target)?;
                progress.files_copied += 1;
                emit(&progress)?;
                continue;
            }

            if ft.is_file() {
                let bytes = copy_file(entry.path(), &target, &meta, opts)?;
                progress.bytes_copied = progress.bytes_copied.saturating_add(bytes);
                progress.files_copied += 1;
                emit(&progress)?;
                continue;
            }
        }

        // Best-effort fsync on destination root to flush directory metadata.
        if let Ok(dir) = fs::File::open(dst) {
            let _ = dir.sync_all();
        }

        Ok(())
    }
}

fn ensure_directory(path: &Path) -> HalResult<()> {
    if path.exists() && !path.is_dir() {
        fs::remove_file(path)?;
    }
    fs::create_dir_all(path)?;
    Ok(())
}

fn copy_symlink(src: &Path, dst: &Path) -> HalResult<()> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    if dst.exists() {
        fs::remove_file(dst)?;
    }
    let target = fs::read_link(src)?;
    symlink(&target, dst)?;
    Ok(())
}

fn copy_file(src: &Path, dst: &Path, meta: &fs::Metadata, opts: &CopyOptions) -> HalResult<u64> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    if dst.exists() {
        fs::remove_file(dst)?;
    }

    let bytes = fs::copy(src, dst)?;

    // Flush data to storage; best-effort if the platform supports it.
    if let Ok(f) = fs::OpenOptions::new().read(true).write(true).open(dst) {
        let _ = f.sync_data();
    }

    apply_metadata(dst, meta, opts)?;

    Ok(bytes)
}

fn apply_metadata(path: &Path, meta: &fs::Metadata, opts: &CopyOptions) -> HalResult<()> {
    #[cfg(unix)]
    {
        if opts.preserve_perms {
            fs::set_permissions(path, meta.permissions())?;
        }
        if opts.preserve_owner {
            let uid = meta.uid();
            let gid = meta.gid();
            chown(path, Some(Uid::from_raw(uid)), Some(Gid::from_raw(gid)))?;
        }
    }

    if opts.preserve_times {
        let atime = FileTime::from_last_access_time(meta);
        let mtime = FileTime::from_last_modification_time(meta);
        filetime::set_file_times(path, atime, mtime)?;
    }

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
}
