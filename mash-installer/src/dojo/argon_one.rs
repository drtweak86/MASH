/// Ported from dojo_bundle/usr_local_lib_mash/dojo/argon_one.sh
use anyhow::Result;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn run(_args: &[String]) -> Result<()> {
    banner("Argon One V2: install (Fedora) + enable I2C");

    let esp = Path::new("/boot/efi");
    let esp_mounted = Command::new("mountpoint")
        .args(["-q", "/boot/efi"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if esp_mounted {
        let config = esp.join("config.txt");
        if config.exists() {
            let ts = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let backup = esp.join(format!("config.txt.bak.{ts}"));
            let _ = Command::new("cp")
                .args([
                    "-an",
                    config.to_string_lossy().as_ref(),
                    backup.to_string_lossy().as_ref(),
                ])
                .status();
        }

        let contents = fs::read_to_string(&config).unwrap_or_default();
        if !contents.contains("dtparam=i2c_arm=on") {
            println!("Enabling I2C in /boot/efi/config.txt");
            let mut file = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&config)?;
            writeln!(file, "\n# MASH: Argon One V2\n[all]\ndtparam=i2c_arm=on")?;
        } else {
            println!("I2C already enabled in /boot/efi/config.txt");
        }
    } else {
        println!("⚠️  /boot/efi not mounted. Mount it then rerun this step.");
    }

    let _ = Command::new("sudo")
        .args([
            "dnf",
            "install",
            "-y",
            "--setopt=install_weak_deps=True",
            "gcc",
            "make",
            "git",
            "i2c-tools",
            "libi2c-devel",
        ])
        .status();

    let work = "/usr/local/src";
    let _ = Command::new("sudo").args(["mkdir", "-p", work]).status();

    let repo = format!("{work}/argononed");
    if !Path::new(&repo).exists() {
        let _ = Command::new("sudo")
            .args([
                "git",
                "clone",
                "https://gitlab.com/DarkElvenAngel/argononed.git",
                &repo,
            ])
            .status();
    } else {
        let _ = Command::new("sudo")
            .args(["git", "-C", &repo, "pull"])
            .status();
    }

    let argononed_sh = format!("{repo}/argononed.sh");
    let makefile = format!("{repo}/Makefile");
    if Path::new(&argononed_sh).exists() {
        println!("Running argononed.sh (may prompt once).");
        let _ = Command::new("sudo").args(["bash", &argononed_sh]).status();
    } else if Path::new(&makefile).exists() {
        let _ = Command::new("sudo").args(["make", "-C", &repo]).status();
        let _ = Command::new("sudo")
            .args(["make", "-C", &repo, "install"])
            .status();
    } else {
        println!(
            "argononed repo layout unexpected; open /usr/local/src/argononed and install manually."
        );
    }

    println!("\nIf the fan doesn't kick in after install: reboot once (I2C overlay change).\n");
    Ok(())
}

fn banner(msg: &str) {
    println!("==============================================================================");
    println!("{msg}");
    println!("==============================================================================");
}
