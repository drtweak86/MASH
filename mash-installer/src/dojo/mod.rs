use anyhow::{anyhow, Result};

mod install_dojo;

pub fn run_task(task: &str, args: &[String]) -> Result<()> {
    match task {
        "install_dojo" | "install-dojo" => install_dojo::run(args),
        _ => Err(anyhow!("unknown dojo task: {task}")),
    }
}
