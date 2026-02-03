//! 🍠 MASH core library.
//!
//! `mash-core` holds shared types, config, and reusable building blocks used by
//! higher-level crates (workflow orchestration, TUI, binaries).

#![allow(dead_code)] // Evolutionary refactor; some modules are temporarily unused.
#![allow(clippy::too_many_arguments)] // Installer config has many params.

pub mod boot_config;
pub mod cli;
pub mod config_states;
pub mod disk_ops;
pub mod dojo_catalogue;
pub mod download;
pub mod download_manager;
pub mod downloader;
pub mod errors;
pub mod flash;
pub mod install_report;
pub mod locale;
pub mod logging;
pub mod partitioning;
pub mod process_timeout;
pub mod progress;
pub mod stages;
pub mod state_manager;
pub mod system_config;

#[cfg(test)]
pub mod test_env;
