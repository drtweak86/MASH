/// Ported from dojo_bundle/usr_local_lib_mash/dojo/bootstrap.sh
use anyhow::{anyhow, Result};
use std::env;
use std::fs;
use std::io::{self, Read};
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

pub fn run(args: &[String]) -> Result<()> {
    match args.first().map(String::as_str) {
        Some("--run") => run_bootstrap(),
        Some("--preview-starship") => preview_starship(),
        None | Some("--help") | Some("-h") => Ok(()),
        Some(other) => Err(anyhow!("Unknown arg: {other}")),
    }
}

fn run_bootstrap() -> Result<()> {
    println!("== MASH bootstrap 🔥 ==");
    println!(
        "(placeholder) This should call /data/mash-staging/install_dojo.sh or mash_forge actions."
    );
    let install = "/data/mash-staging/install_dojo.sh";
    if is_executable(install) {
        let _ = Command::new("sudo")
            .args([install, "/data/mash-staging"])
            .status();
    } else {
        println!("⚠️ /data/mash-staging/install_dojo.sh not found.");
    }
    Ok(())
}

fn preview_starship() -> Result<()> {
    println!("== Starship preview ⭐ ==");
    let has_starship = Command::new("sh")
        .args(["-lc", "command -v starship"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !has_starship {
        println!("Installing starship...");
        let _ = Command::new("sudo")
            .args(["dnf", "-y", "install", "starship"])
            .status();
    }

    let assets = "/usr/local/lib/mash/dojo/assets/starship.toml";
    if fs::metadata(assets).is_ok() {
        env::set_var("STARSHIP_CONFIG", assets);
    }

    let has_starship = Command::new("sh")
        .args(["-lc", "command -v starship"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if has_starship {
        println!("\nPrompt preview:");
        println!("----------------------------------------");
        let output = Command::new("starship")
            .args(["prompt"])
            .output()
            .ok()
            .and_then(|out| String::from_utf8(out.stdout).ok())
            .unwrap_or_default();
        print!("{output}");
        println!("\n----------------------------------------");
        if let Ok(cfg) = env::var("STARSHIP_CONFIG") {
            println!("(Set STARSHIP_CONFIG={cfg})");
        }
    } else {
        println!("❌ starship not available.");
    }

    println!("Press Enter to return… ");
    let _ = io::stdin().read_to_end(&mut Vec::new());
    Ok(())
}

fn is_executable(path: &str) -> bool {
    fs::metadata(path)
        .map(|meta| meta.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}
