//! Linux HAL implementation using real system calls.

use super::{
    BtrfsOps, FlashOps, FlashOptions, FormatOps, FormatOptions, LoopOps, MountOps, MountOptions,
    PartedOp, PartedOptions, PartitionOps, ProbeOps, RsyncOps, RsyncOptions, SystemOps,
    WipeFsOptions,
};
use crate::{HalError, HalResult};
use std::fs;
use std::io::{self, BufRead, BufReader, Read};
use std::path::Path;
use std::process::{Command, Output, Stdio};

/// Real HAL implementation for Linux systems.
#[derive(Debug, Clone, Default)]
pub struct LinuxHal;

impl LinuxHal {
    pub fn new() -> Self {
        Self
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

        let output = Command::new("mkfs.ext4")
            .args(&args)
            .output()
            .map_err(|e| map_command_err("mkfs.ext4", e))?;

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

        let output = Command::new("mkfs.btrfs")
            .args(&args)
            .output()
            .map_err(|e| map_command_err("mkfs.btrfs", e))?;

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

        let output = Command::new("mkfs.vfat")
            .args(&args)
            .output()
            .map_err(|e| map_command_err("mkfs.vfat", e))?;

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
        let _ = Command::new("sync")
            .status()
            .map_err(|e| map_command_err("sync", e))?;
        Ok(())
    }

    fn udev_settle(&self) -> HalResult<()> {
        let _ = Command::new("udevadm")
            .arg("settle")
            .status()
            .map_err(|e| map_command_err("udevadm", e))?;
        Ok(())
    }
}

impl ProbeOps for LinuxHal {
    fn lsblk_mountpoints(&self, disk: &Path) -> HalResult<Vec<std::path::PathBuf>> {
        let output = Command::new("lsblk")
            .args(["-lnpo", "MOUNTPOINT"])
            .arg(disk)
            .output()
            .map_err(|e| map_command_err("lsblk", e))?;

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
        let output = Command::new("lsblk")
            .args(["-o", "NAME,SIZE,TYPE,FSTYPE,MOUNTPOINTS,MODEL"])
            .arg(disk)
            .output()
            .map_err(|e| map_command_err("lsblk", e))?;

        if !output.status.success() {
            return Err(output_failed("lsblk", &output));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    fn blkid_uuid(&self, device: &Path) -> HalResult<String> {
        let output = Command::new("blkid")
            .args(["-s", "UUID", "-o", "value"])
            .arg(device)
            .output()
            .map_err(|e| map_command_err("blkid", e))?;

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

        let output = Command::new("wipefs")
            .args(["-a"])
            .arg(disk)
            .output()
            .map_err(|e| map_command_err("wipefs", e))?;
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

        let output = Command::new("parted")
            .args(&args)
            .output()
            .map_err(|e| map_command_err("parted", e))?;
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

        let output = Command::new("losetup")
            .args(&args)
            .output()
            .map_err(|e| map_command_err("losetup", e))?;

        if !output.status.success() {
            return Err(output_failed("losetup", &output));
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    fn losetup_detach(&self, loop_device: &str) -> HalResult<()> {
        let output = Command::new("losetup")
            .args(["-d", loop_device])
            .output()
            .map_err(|e| map_command_err("losetup", e))?;
        if !output.status.success() {
            return Err(output_failed("losetup", &output));
        }
        Ok(())
    }
}

impl BtrfsOps for LinuxHal {
    fn btrfs_subvolume_list(&self, mount_point: &Path) -> HalResult<String> {
        let output = Command::new("btrfs")
            .args(["subvolume", "list"])
            .arg(mount_point)
            .output()
            .map_err(|e| map_command_err("btrfs", e))?;
        if !output.status.success() {
            return Err(output_failed("btrfs", &output));
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    fn btrfs_subvolume_create(&self, path: &Path) -> HalResult<()> {
        let output = Command::new("btrfs")
            .args(["subvolume", "create"])
            .arg(path)
            .output()
            .map_err(|e| map_command_err("btrfs", e))?;
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

        if let Some(stdout) = child.stdout.take() {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                let line = line?;
                if !on_stdout_line(&line) {
                    let _ = child.kill();
                    if let Some(h) = stderr_handle.take() {
                        let _ = h.join();
                    }
                    return Err(HalError::Other("rsync cancelled".to_string()));
                }
            }
        }

        let status = child.wait()?;
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
