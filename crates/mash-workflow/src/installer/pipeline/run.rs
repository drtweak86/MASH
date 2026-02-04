use super::config::InstallConfig;
use super::plan::{build_plan, InstallPlan};
use super::stage_defs::build_stage_definitions;
use crate::install_runner::StageRunner;
use anyhow::Result;
use mash_core::config_states::{ArmedConfig, ValidatedConfig};
use std::sync::Arc;

pub fn run_pipeline(cfg: &InstallConfig) -> Result<InstallPlan> {
    let validated = mash_core::config_states::UnvalidatedConfig::new(cfg.clone()).validate()?;
    run_pipeline_validated(validated)
}

pub fn run_pipeline_execute(cfg: ArmedConfig<InstallConfig>) -> Result<InstallPlan> {
    if !cfg.cfg.execute || cfg.cfg.dry_run {
        anyhow::bail!("run_pipeline_execute requires execute=true and dry_run=false");
    }
    run_pipeline_impl(cfg.cfg)
}

fn run_pipeline_validated(cfg: ValidatedConfig<InstallConfig>) -> Result<InstallPlan> {
    if !cfg.0.execute {
        return Ok(build_plan(&cfg.0));
    }

    // For execute-mode, callers must arm to perform destructive operations. We only allow a
    // simulated run here (execute=true, dry_run=true).
    if !cfg.0.dry_run {
        anyhow::bail!("execute-mode requires an ArmedConfig; use run_pipeline_execute");
    }

    run_pipeline_impl(cfg.0)
}

fn run_pipeline_impl(cfg: InstallConfig) -> Result<InstallPlan> {
    let plan = build_plan(&cfg);
    if !cfg.execute {
        return Ok(plan);
    }

    let hal: Arc<mash_hal::LinuxHal> = Arc::new(mash_hal::LinuxHal::new());
    let validated = mash_core::config_states::UnvalidatedConfig::new(cfg.clone()).validate()?;
    let stage_defs = build_stage_definitions(&plan, &validated, Arc::clone(&hal));

    let require_armed = cfg.execute && !cfg.dry_run;
    let runner =
        StageRunner::new(cfg.state_path.clone(), cfg.dry_run).with_require_armed(require_armed);
    runner.run(&stage_defs)?;

    Ok(plan)
}
