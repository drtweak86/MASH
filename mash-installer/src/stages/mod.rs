use anyhow::{anyhow, Result};

pub mod stage_00_write_config_txt;
pub mod stage_01_stage_bootstrap;
pub mod stage_02_early_ssh;
pub mod stage_02_internet_wait;
pub mod stage_03_fail2ban_lite;
pub mod stage_03_stage_starship_toml;

pub fn run_stage(stage: &str, args: &[String]) -> Result<()> {
    match stage {
        "00" | "00_write_config_txt" => stage_00_write_config_txt::run(args),
        "01" | "01_stage_bootstrap" => stage_01_stage_bootstrap::run(args),
        "02_early_ssh" | "02-early-ssh" => stage_02_early_ssh::run(args),
        "02_internet_wait" | "02-internet-wait" => stage_02_internet_wait::run(args),
        "03_fail2ban_lite" | "03-fail2ban-lite" => stage_03_fail2ban_lite::run(args),
        "03_stage_starship_toml" | "03-stage-starship-toml" => {
            stage_03_stage_starship_toml::run(args)
        }
        _ => Err(anyhow!("unknown stage: {stage}")),
    }
}
