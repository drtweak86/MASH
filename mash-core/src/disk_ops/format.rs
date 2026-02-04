use anyhow::Result;
use mash_hal::{FormatOps, FormatOptions};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec {
    pub program: String,
    pub args: Vec<String>,
}

pub fn format_ext4(hal: &dyn FormatOps, device: &Path, opts: &FormatOptions) -> Result<()> {
    hal.format_ext4(device, opts).map_err(anyhow::Error::new)
}

pub fn format_btrfs(hal: &dyn FormatOps, device: &Path, opts: &FormatOptions) -> Result<()> {
    hal.format_btrfs(device, opts).map_err(anyhow::Error::new)
}

pub fn ext4_command_spec(device: &Path, opts: &FormatOptions) -> CommandSpec {
    let mut args = opts.extra_args.clone();
    args.push(device.display().to_string());
    CommandSpec {
        program: "mkfs.ext4".to_string(),
        args,
    }
}

pub fn btrfs_command_spec(device: &Path, opts: &FormatOptions) -> CommandSpec {
    let mut args = opts.extra_args.clone();
    args.push(device.display().to_string());
    CommandSpec {
        program: "mkfs.btrfs".to_string(),
        args,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ext4_command_is_constructed() {
        let opts = FormatOptions::new(true, true);
        let spec = ext4_command_spec(Path::new("/dev/sda1"), &opts);
        assert_eq!(spec.program, "mkfs.ext4");
        assert_eq!(spec.args, vec!["/dev/sda1".to_string()]);
    }

    #[test]
    fn btrfs_command_is_constructed() {
        let opts = FormatOptions::new(true, true);
        let spec = btrfs_command_spec(Path::new("/dev/sda2"), &opts);
        assert_eq!(spec.program, "mkfs.btrfs");
        assert_eq!(spec.args, vec!["/dev/sda2".to_string()]);
    }

    #[test]
    fn format_requires_confirmation() {
        let opts = FormatOptions::new(false, false);
        let hal = mash_hal::FakeHal::default();
        let err = format_ext4(&hal, Path::new("/dev/sda1"), &opts).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("SafetyLock") || msg.to_ascii_lowercase().contains("safety"),
            "unexpected error message: {msg}"
        );
    }
}
