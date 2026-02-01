use anyhow::Result;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

pub fn run(args: &[String]) -> Result<()> {
    let _user = args.first().map(String::as_str).unwrap_or("DrTweak");
    println!("[*] Argon One V2 (best effort)");

    let _ = Command::new("dnf")
        .args([
            "install",
            "-y",
            "--skip-unavailable",
            "git",
            "gcc",
            "make",
            "dtc",
            "i2c-tools",
            "libi2c-devel",
        ])
        .status();

    let _ = Command::new("install")
        .args(["-d", "/opt/argononed"])
        .status();

    let git_dir = "/opt/argononed/.git";
    if fs::metadata(git_dir).is_err() {
        let _ = Command::new("rm").args(["-rf", "/opt/argononed"]).status();
        let _ = Command::new("git")
            .args([
                "clone",
                "https://gitlab.com/DarkElvenAngel/argononed.git",
                "/opt/argononed",
            ])
            .status();
    }

    let install_sh = "/opt/argononed/install.sh";
    let executable = fs::metadata(install_sh)
        .map(|meta| meta.permissions().mode() & 0o111 != 0)
        .unwrap_or(false);

    if executable {
        let _ = Command::new("bash")
            .args(["-lc", "cd /opt/argononed && ./install.sh"])
            .status();
    } else {
        println!("[!] Repo layout differs; review /opt/argononed contents.");
    }

    println!(
        "[+] If fan still dead: ensure dtparam=i2c_arm=on in /boot/efi/config.txt and reboot."
    );

    Ok(())
}
