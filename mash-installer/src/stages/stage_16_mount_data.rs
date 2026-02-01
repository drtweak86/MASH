use anyhow::Result;
use std::fs;
use std::io::Write;
use std::process::Command;

pub fn run(args: &[String]) -> Result<()> {
    let user = args.first().map(String::as_str).unwrap_or("DrTweak");
    println!("[*] Ensuring DATA partition mounted at /data");

    fs::create_dir_all("/data")?;

    let fstab = fs::read_to_string("/etc/fstab").unwrap_or_default();
    let has_data = fstab.lines().any(|line| {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            return false;
        }
        let mut parts = trimmed.split_whitespace();
        let _src = parts.next();
        let mount = parts.next();
        mount == Some("/data")
    });

    if !has_data {
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/etc/fstab")?;
        writeln!(file, "LABEL=DATA  /data  ext4  defaults,noatime  0  2")?;
    }

    let _ = Command::new("mount").args(["-a"]).status();
    let _ = Command::new("chown")
        .args([&format!("{user}:{user}"), "/data"])
        .status();

    Ok(())
}
