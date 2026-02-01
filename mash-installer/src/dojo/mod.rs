use anyhow::{anyhow, Result};

mod argon_one;
mod audio;
mod bootcount;
mod bootstrap;
mod borg;
mod browser;
mod dojo_entry;
mod early_ssh;
mod firewall;
mod graphics;
mod install_dojo;
mod internet_wait;
mod menu;
mod mount_data;
mod rclone;

pub fn run_task(task: &str, args: &[String]) -> Result<()> {
    match task {
        "install_dojo" | "install-dojo" => install_dojo::run(args),
        "early_ssh" | "early-ssh" => early_ssh::run(args),
        "internet_wait" | "internet-wait" => internet_wait::run(args),
        "argon_one" | "argon-one" => argon_one::run(args),
        "audio" => audio::run(args),
        "bootcount" => bootcount::run(args),
        "bootstrap" => bootstrap::run(args),
        "borg" => borg::run(args),
        "browser" => browser::run(args),
        "dojo" => dojo_entry::run(args),
        "firewall" => firewall::run(args),
        "graphics" => graphics::run(args),
        "menu" => menu::run(args),
        "mount_data" | "mount-data" => mount_data::run(args),
        "rclone" => rclone::run(args),
        _ => Err(anyhow!("unknown dojo task: {task}")),
    }
}
