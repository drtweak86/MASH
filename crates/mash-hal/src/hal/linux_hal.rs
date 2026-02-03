//! Linux HAL implementation using real system calls.

use super::{FlashOps, FlashOptions, FormatOps, FormatOptions, MountOps, MountOptions};
use anyhow::{anyhow, Context, Result};
use std::fs;
use std::io::{self, Read};
use std::path::Path;
use std::process::Command;

/// Real HAL implementation for Linux systems.
#[derive(Debug, Clone, Default)]
pub struct LinuxHal;

impl LinuxHal {
    pub fn new() -> Self {
        Self
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
    ) -> Result<()> {
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

        nix::mount::mount(Some(device), target, fstype, flags, data)
            .context("Failed to mount device")?;

        Ok(())
    }

    fn unmount(&self, target: &Path, dry_run: bool) -> Result<()> {
        if dry_run {
            log::info!("DRY RUN: unmount {}", target.display());
            return Ok(());
        }

        nix::mount::umount2(target, nix::mount::MntFlags::empty()).context("Failed to unmount")?;

        Ok(())
    }

    fn is_mounted(&self, path: &Path) -> Result<bool> {
        let content = fs::read_to_string("/proc/self/mountinfo")
            .context("Failed to read /proc/self/mountinfo")?;
        let entries = crate::procfs::mountinfo::parse_mountinfo(&content);
        Ok(crate::procfs::mountinfo::is_mounted_from_info(
            path, &entries,
        ))
    }
}

impl FormatOps for LinuxHal {
    fn format_ext4(&self, device: &Path, opts: &FormatOptions) -> Result<()> {
        if opts.dry_run {
            log::info!("DRY RUN: mkfs.ext4 {}", device.display());
            return Ok(());
        }

        if !opts.confirmed {
            return Err(anyhow::Error::new(crate::HalError::SafetyLock));
        }

        let mut args = opts.extra_args.clone();
        args.push(device.display().to_string());

        let status = Command::new("mkfs.ext4")
            .args(&args)
            .status()
            .context("Failed to execute mkfs.ext4")?;

        if !status.success() {
            return Err(anyhow!("mkfs.ext4 failed"));
        }

        Ok(())
    }

    fn format_btrfs(&self, device: &Path, opts: &FormatOptions) -> Result<()> {
        if opts.dry_run {
            log::info!("DRY RUN: mkfs.btrfs {}", device.display());
            return Ok(());
        }

        if !opts.confirmed {
            return Err(anyhow::Error::new(crate::HalError::SafetyLock));
        }

        let mut args = opts.extra_args.clone();
        args.push(device.display().to_string());

        let status = Command::new("mkfs.btrfs")
            .args(&args)
            .status()
            .context("Failed to execute mkfs.btrfs")?;

        if !status.success() {
            return Err(anyhow!("mkfs.btrfs failed"));
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
    ) -> Result<()> {
        if opts.dry_run {
            log::info!(
                "DRY RUN: flash {} -> {}",
                image_path.display(),
                target_disk.display()
            );
            return Ok(());
        }

        if !opts.confirmed {
            return Err(anyhow::Error::new(crate::HalError::SafetyLock));
        }

        log::info!(
            "💾 Flashing image {} -> {}",
            image_path.display(),
            target_disk.display()
        );

        let input = fs::File::open(image_path)
            .with_context(|| format!("Failed to open image: {}", image_path.display()))?;

        let mut reader: Box<dyn Read> = if image_path.extension().is_some_and(|e| e == "xz") {
            Box::new(xz2::read::XzDecoder::new(input))
        } else {
            Box::new(input)
        };

        let mut out = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(target_disk)
            .with_context(|| format!("Failed to open target disk: {}", target_disk.display()))?;

        // For regular files (CI tests), truncate; for block devices, this may fail and is fine.
        let _ = out.set_len(0);

        io::copy(&mut reader, &mut out).context("Failed to write image to target disk")?;

        // Best-effort flush (block devices may ignore).
        out.sync_all().ok();

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
        assert!(err.downcast_ref::<crate::HalError>().is_some());
    }

    #[test]
    fn format_btrfs_requires_confirmation() {
        let hal = LinuxHal::new();
        let opts = FormatOptions::new(false, false);
        let err = hal.format_btrfs(Path::new("/dev/null"), &opts).unwrap_err();
        assert!(err.downcast_ref::<crate::HalError>().is_some());
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
