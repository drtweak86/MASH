use anyhow::{anyhow, Result};

pub mod stage_00_write_config_txt;
pub mod stage_01_stage_bootstrap;
pub mod stage_02_early_ssh;
pub mod stage_02_internet_wait;
pub mod stage_03_fail2ban_lite;
pub mod stage_03_stage_starship_toml;
pub mod stage_05_fonts_essential;
pub mod stage_10_locale_uk;
pub mod stage_11_snapper_init;
pub mod stage_12_firewall_sane;
pub mod stage_13_packages_core;
pub mod stage_14_packages_dev;
pub mod stage_15_packages_desktop;
pub mod stage_16_mount_data;
pub mod stage_17_brave_browser;

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
        "05_fonts_essential" | "05-fonts-essential" => stage_05_fonts_essential::run(args),
        "10_locale_uk" | "10-locale-uk" => stage_10_locale_uk::run(args),
        "11_snapper_init" | "11-snapper-init" => stage_11_snapper_init::run(args),
        "12_firewall_sane" | "12-firewall-sane" => stage_12_firewall_sane::run(args),
        "13_packages_core" | "13-packages-core" => stage_13_packages_core::run(args),
        "14_packages_dev" | "14-packages-dev" => stage_14_packages_dev::run(args),
        "15_packages_desktop" | "15-packages-desktop" => stage_15_packages_desktop::run(args),
        "16_mount_data" | "16-mount-data" => stage_16_mount_data::run(args),
        "17_brave_browser" | "17-brave-browser" => stage_17_brave_browser::run(args),
        _ => Err(anyhow!("unknown stage: {stage}")),
    }
}
