use anyhow::{anyhow, Result};

mod early_ssh;
mod install_dojo;

pub fn run_task(task: &str, args: &[String]) -> Result<()> {
    match task {
        "install_dojo" | "install-dojo" => install_dojo::run(args),
        "early_ssh" | "early-ssh" => early_ssh::run(args),
        _ => Err(anyhow!("unknown dojo task: {task}")),
    }
}
