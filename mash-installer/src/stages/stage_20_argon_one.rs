use anyhow::Result;
use std::env;
use std::error::Error;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

const ARGON_ROOT_ENV: &str = "MASH_ARGON_ROOT";
const ARGON_REPO: &str = "https://gitlab.com/DarkElvenAngel/argononed.git";

pub fn run(args: &[String]) -> Result<()> {
    let _user = args.first().map(String::as_str).unwrap_or("DrTweak");
    println!("[*] Argon One V2 (best effort)");
    run_argon_one().map_err(|err| anyhow::anyhow!(err.to_string()))?;
    Ok(())
}

fn run_argon_one() -> Result<(), Box<dyn Error>> {
    let root = env::var(ARGON_ROOT_ENV).unwrap_or_else(|_| "/opt/argononed".to_string());
    let root_path = Path::new(&root);

    let mut dnf = Command::new("dnf");
    dnf.args([
        "install",
        "-y",
        "--skip-unavailable",
        "git",
        "gcc",
        "make",
        "dtc",
        "i2c-tools",
        "libi2c-devel",
    ]);
    run_command_ignore_failure(&mut dnf)?;

    fs::create_dir_all(root_path)?;

    let git_dir = root_path.join(".git");
    if !git_dir.exists() {
        if root_path.exists() {
            let _ = fs::remove_dir_all(root_path);
        }
        let mut git = Command::new("git");
        git.args(["clone", ARGON_REPO, root_path.to_string_lossy().as_ref()]);
        run_command_ignore_failure(&mut git)?;
    }

    let install_script = root_path.join("install.sh");
    if install_script.exists() {
        let is_executable = fs::metadata(&install_script)
            .map(|meta| meta.permissions().mode() & 0o111 != 0)
            .unwrap_or(false);
        if is_executable {
            let mut bash = Command::new("bash");
            bash.args([
                "-lc",
                &format!("cd {} && ./install.sh", root_path.display()),
            ]);
            run_command_ignore_failure(&mut bash)?;
        } else {
            println!(
                "[!] Repo layout differs; review {} contents.",
                root_path.display()
            );
        }
    } else {
        println!(
            "[!] Repo layout differs; review {} contents.",
            root_path.display()
        );
    }

    println!(
        "[+] If fan still dead: ensure dtparam=i2c_arm=on in /boot/efi/config.txt and reboot."
    );

    Ok(())
}

fn run_command_ignore_failure(cmd: &mut Command) -> Result<(), Box<dyn Error>> {
    let status = cmd.status()?;
    if !status.success() {
        log::warn!("command exited with status {status}");
    }
    Ok(())
}
