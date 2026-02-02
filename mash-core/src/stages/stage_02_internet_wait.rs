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

    let systemd_dir = root_path.join("etc/systemd/system");
    let lib_dir = root_path.join("usr/local/lib/mash/system");
    let wants_dir = systemd_dir.join("multi-user.target.wants");

    fs::create_dir_all(&systemd_dir)?;
    fs::create_dir_all(&lib_dir)?;
    fs::create_dir_all(&wants_dir)?;

    let service_src = stage_path.join("systemd/mash-internet-wait.service");
    let service_dst = systemd_dir.join("mash-internet-wait.service");
    fs::copy(&service_src, &service_dst)?;
    fs::set_permissions(&service_dst, fs::Permissions::from_mode(0o644))?;

    let script_src = stage_path.join("systemd/internet-wait.sh");
    let script_dst = lib_dir.join("internet-wait.sh");
    fs::copy(&script_src, &script_dst)?;
    fs::set_permissions(&script_dst, fs::Permissions::from_mode(0o755))?;

    let link = wants_dir.join("mash-internet-wait.service");
    if link.exists() {
        fs::remove_file(&link)?;
    }
    symlink("../mash-internet-wait.service", &link)?;

    Ok(())
}
