/// Ported from dojo_bundle/usr_local_lib_mash/dojo/mount_data.sh
use anyhow::Result;
use std::fs;
use std::io::Write;
use std::process::Command;

pub fn run(_args: &[String]) -> Result<()> {
    banner("Storage: ensure LABEL=DATA mounted at /data");

    let _ = Command::new("sudo").args(["mkdir", "-p", "/data"]).status();

    let fstab = fs::read_to_string("/etc/fstab").unwrap_or_default();
    let has_data = fstab
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .any(|line| line.split_whitespace().nth(1) == Some("/data"));

    if !has_data {
        println!("Adding fstab entry for /data");
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/etc/fstab")?;
        writeln!(file, "LABEL=DATA  /data  ext4  defaults,noatime  0  2")?;
    } else {
        println!("fstab already has /data entry.");
    }

    let _ = Command::new("sudo").args(["mount", "-a"]).status();

    let user = Command::new("id")
        .args(["-un"])
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .unwrap_or_default();
    let group = Command::new("id")
        .args(["-gn"])
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .unwrap_or_default();
    let owner = format!("{}:{}", user.trim(), group.trim());
    let _ = Command::new("sudo")
        .args(["chown", &owner, "/data"])
        .status();

    println!("Done. df -h /data:");
    let _ = Command::new("df").args(["-h", "/data"]).status();

    Ok(())
}

fn banner(msg: &str) {
    println!("==============================================================================");
    println!("{msg}");
    println!("==============================================================================");
}
