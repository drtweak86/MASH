use anyhow::{anyhow, Result};

pub mod stage_00_write_config_txt;

pub fn run_stage(stage: &str, args: &[String]) -> Result<()> {
    match stage {
        "00" | "00_write_config_txt" => stage_00_write_config_txt::run(args),
        _ => Err(anyhow!("unknown stage: {stage}")),
    }
}
