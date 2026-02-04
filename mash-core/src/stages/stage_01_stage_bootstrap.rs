use anyhow::{Context, Result};
use mash_hal::{CopyOps, CopyOptions, LinuxHal, SystemOps};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

pub fn run(args: &[String]) -> Result<()> {
    let data_mount = args.first().map(String::as_str).unwrap_or("/mnt/data");
    let src_root = args
        .get(1)
        .map(String::as_str)
        .ok_or_else(|| anyhow::anyhow!("need path to mash_helpers root"))?;
    let dst = Path::new(data_mount).join("bootstrap");
    log::info!("[*] Staging bootstrap into {}", dst.display());
    fs::create_dir_all(&dst)?;

    let hal = LinuxHal::new();
    hal.copy_tree_native(
        Path::new(src_root),
        &dst,
        &CopyOptions::archive(),
        &mut |_p| true,
    )
    .context("copy failed")?;

    let mash_forge = dst.join("mash_forge.py");
    if mash_forge.exists() {
        fs::set_permissions(&mash_forge, fs::Permissions::from_mode(0o755))?;
    }

    let helpers_dir = dst.join("helpers");
    if helpers_dir.exists() {
        for entry in fs::read_dir(&helpers_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("sh") {
                let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o755));
            }
        }
    }

    let _ = hal.sync();
    log::info!(
        "[+] Staged. On Fedora first boot run: sudo /data/bootstrap/mash_forge.py firstboot ..."
    );
    Ok(())
}
