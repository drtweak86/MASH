use anyhow::{anyhow, Result};
use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::Path;

pub fn run(args: &[String]) -> Result<()> {
    let root = args
        .first()
        .map(String::as_str)
        .ok_or_else(|| anyhow!("need target root path"))?;
    let stage = args
        .get(1)
        .map(String::as_str)
        .ok_or_else(|| anyhow!("need staging dir path"))?;

    let root_path = Path::new(root);
    let stage_path = Path::new(stage);

    let systemd_root = root_path.join("etc/systemd/system");
    let mash_lib = root_path.join("usr/local/lib/mash/system");
    let wants = systemd_root.join("multi-user.target.wants");

    fs::create_dir_all(&systemd_root)?;
    fs::create_dir_all(&mash_lib)?;
    fs::create_dir_all(&wants)?;

    let service_src = stage_path.join("systemd/mash-early-ssh.service");
    let script_src = stage_path.join("systemd/early-ssh.sh");

    let service_dst = systemd_root.join("mash-early-ssh.service");
    let script_dst = mash_lib.join("early-ssh.sh");

    fs::copy(&service_src, &service_dst)?;
    fs::set_permissions(&service_dst, fs::Permissions::from_mode(0o644))?;

    fs::copy(&script_src, &script_dst)?;
    fs::set_permissions(&script_dst, fs::Permissions::from_mode(0o755))?;

    let link_path = wants.join("mash-early-ssh.service");
    if link_path.exists() {
        fs::remove_file(&link_path)?;
    }
    symlink("../mash-early-ssh.service", &link_path)?;

    Ok(())
}
