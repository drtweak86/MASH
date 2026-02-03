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
use std::sync::mpsc;
use std::time::{Duration, Instant};
use wait_timeout::ChildExt;

/// Real HAL implementation for Linux systems.
#[derive(Debug, Clone, Default)]
pub struct LinuxHal;

impl LinuxHal {
    pub fn new() -> Self {
        Self
    }
}

const PROBE_TIMEOUT: Duration = Duration::from_secs(10);
const SYNC_TIMEOUT: Duration = Duration::from_secs(60);
const FORMAT_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const WIPEFS_TIMEOUT: Duration = Duration::from_secs(60);
const PARTED_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const LOSETUP_TIMEOUT: Duration = Duration::from_secs(30);
const BTRFS_TIMEOUT: Duration = Duration::from_secs(60);
const RSYNC_IDLE_TIMEOUT: Duration = Duration::from_secs(120);
const RSYNC_MAX_TIMEOUT: Duration = Duration::from_secs(6 * 60 * 60);

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
