mod config;
mod plan;
mod run;
mod stages;

pub use config::{
    BootStageConfig, DiskStageConfig, DownloadStageConfig, InstallConfig, MountSpec,
    MountStageConfig, PackageStageConfig, ResumeStageConfig,
};
pub use plan::{build_plan, InstallPlan, StagePlan};
pub use run::{run_pipeline, run_pipeline_execute};

#[cfg(test)]
mod tests;
