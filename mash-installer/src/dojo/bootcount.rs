use anyhow::Result;
use std::fs;
use std::io::Write;
use std::process::Command;

pub fn run(_args: &[String]) -> Result<()> {
    let state_dir = "/var/lib/mash";
    fs::create_dir_all(state_dir)?;
    let count_file = format!("{state_dir}/bootcount");

    let log_dir = "/data/mash-logs";
    fs::create_dir_all(log_dir)?;
    let log_path = format!("{log_dir}/bootcount.log");

    let mut count: u64 = fs::read_to_string(&count_file)
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(0);
    count += 1;
    fs::write(&count_file, count.to_string())?;

    let mut log = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;
    let timestamp = Command::new("date")
        .args(["-Is"])
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .unwrap_or_else(|| "unknown".to_string());
    writeln!(log, "=== {} bootcount={} ===", timestamp.trim(), count)?;

    Ok(())
}
