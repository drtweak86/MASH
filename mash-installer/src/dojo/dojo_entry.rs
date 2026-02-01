/// Ported from dojo_bundle/usr_local_lib_mash/dojo/dojo.sh
use anyhow::Result;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

pub fn run(_args: &[String]) -> Result<()> {
    let state_dir = state_dir();
    fs::create_dir_all(&state_dir)?;
    let completed_flag = state_dir.join("dojo.completed");
    if completed_flag.exists() {
        return Ok(());
    }

    let log_file = state_dir.join("dojo.log");
    let log_path = log_file.to_string_lossy();

    let cmd = format!("mash dojo menu 2>&1 | tee -a {}", log_path);
    let _ = Command::new("sh").args(["-lc", &cmd]).status();

    Ok(())
}

fn state_dir() -> PathBuf {
    if let Ok(xdg) = env::var("XDG_STATE_HOME") {
        if !xdg.is_empty() {
            return PathBuf::from(xdg).join("mash");
        }
    }
    if let Ok(home) = env::var("HOME") {
        if !home.is_empty() {
            return PathBuf::from(home).join(".local/state/mash");
        }
    }
    PathBuf::from("/tmp/mash")
}
