use anyhow::Context;
use std::fs;
use std::path::{Path, PathBuf};

/// Mount points for the installation.
pub(super) struct MountPoints {
    // Source (image) mounts
    pub(super) src_efi: PathBuf,
    pub(super) src_boot: PathBuf,
    pub(super) src_root_top: PathBuf,
    pub(super) src_root_subvol: PathBuf,
    pub(super) src_home_subvol: PathBuf,
    pub(super) src_var_subvol: PathBuf,
    // Destination (target) mounts
    pub(super) dst_efi: PathBuf,
    pub(super) dst_boot: PathBuf,
    pub(super) dst_data: PathBuf,
    pub(super) dst_root_top: PathBuf,
    pub(super) dst_root_subvol: PathBuf,
    pub(super) dst_home_subvol: PathBuf,
    pub(super) dst_var_subvol: PathBuf,
}

impl MountPoints {
    pub(super) fn new(work_dir: &Path) -> Self {
        let src = work_dir.join("src");
        let dst = work_dir.join("dst");

        Self {
            src_efi: src.join("efi"),
            src_boot: src.join("boot"),
            src_root_top: src.join("root_top"),
            // Keep these paths stable; they match the original flash pipeline layout.
            src_root_subvol: src.join("root_sub_root"),
            src_home_subvol: src.join("root_sub_home"),
            src_var_subvol: src.join("root_sub_var"),
            dst_efi: dst.join("efi"),
            dst_boot: dst.join("boot"),
            dst_data: dst.join("data"),
            dst_root_top: dst.join("root_top"),
            dst_root_subvol: dst.join("root_sub_root"),
            dst_home_subvol: dst.join("root_sub_home"),
            dst_var_subvol: dst.join("root_sub_var"),
        }
    }

    pub(super) fn ensure_dirs(&self) -> anyhow::Result<()> {
        for dir in [
            &self.src_efi,
            &self.src_boot,
            &self.src_root_top,
            &self.src_root_subvol,
            &self.src_home_subvol,
            &self.src_var_subvol,
            &self.dst_efi,
            &self.dst_boot,
            &self.dst_data,
            &self.dst_root_top,
            &self.dst_root_subvol,
            &self.dst_home_subvol,
            &self.dst_var_subvol,
        ] {
            fs::create_dir_all(dir)
                .with_context(|| format!("Failed to create mount point: {}", dir.display()))?;
        }
        Ok(())
    }

    pub(super) fn create_all(&self) -> anyhow::Result<()> {
        self.ensure_dirs()
    }
}
