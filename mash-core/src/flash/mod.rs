//! Flash module - Full installation pipeline for MASH

mod approvals;
mod cancel;
mod config;
mod mounts;
mod runner;
mod staging;

pub use approvals::{
    apply_approvals, generate_plan_summary, ApprovalRequest, ApprovalState, PartitionApprovalMode,
};
pub use cancel::{clear_cancel_flag, set_cancel_flag};
pub use config::{FlashConfig, FlashContext};
pub use runner::{
    flash_raw_image_to_disk, run, run_dry_run_with_hal, run_execute_with_hal, run_with_progress,
    run_with_progress_with_confirmation, run_with_progress_with_confirmation_with_hal,
    stable_disk_id, stable_disk_id_for,
};
pub use staging::staging_root;
