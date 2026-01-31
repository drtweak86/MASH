use anyhow::{anyhow, Result};

pub mod stage_00_write_config_txt;
pub mod stage_01_stage_bootstrap;

pub fn run_stage(stage: &str, args: &[String]) -> Result<()> {
    match stage {
        "00" | "00_write_config_txt" => stage_00_write_config_txt::run(args),
        "01" | "01_stage_bootstrap" => stage_01_stage_bootstrap::run(args),
        _ => Err(anyhow!("unknown stage: {stage}")),
    }
}
