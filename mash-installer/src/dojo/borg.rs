/// Ported from dojo_bundle/usr_local_lib_mash/dojo/borg.sh
use anyhow::Result;
use std::process::Command;

pub fn run(_args: &[String]) -> Result<()> {
    banner("Borg: install + quick-start");

    let _ = Command::new("sudo")
        .args(["dnf", "install", "-y", "borgbackup"])
        .status();

    println!(
        "\nNext steps (example):\n  mkdir -p /data/borgrepo\n  borg init --encryption=repokey-blake2 /data/borgrepo\n\nBackup example:\n  borg create --stats /data/borgrepo::\"{{hostname}}-{{now}}\" /home\n\nWhen you're ready, we can bake your exact policy (excludes, compression, timers).\n"
    );

    Ok(())
}

fn banner(msg: &str) {
    println!("==============================================================================");
    println!("{msg}");
    println!("==============================================================================");
}
