use anyhow::{anyhow, Result};

mod argon_one;
mod audio;
mod bootcount;
mod bootstrap;
mod borg;
mod browser;
mod early_ssh;
mod install_dojo;
mod internet_wait;

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
        _ => Err(anyhow!("unknown dojo task: {task}")),
    }
}
