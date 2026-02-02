use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

type StageName = String;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InstallState {
    pub version: u32,
    pub dry_run: bool,
    pub current_stage: Option<StageName>,
    pub completed_stages: Vec<StageName>,
}

impl InstallState {
    pub fn new(dry_run: bool) -> Self {
        Self {
            version: 1,
            dry_run,
            current_stage: None,
            completed_stages: Vec::new(),
        }
    }

    pub fn is_completed(&self, stage: &str) -> bool {
        self.completed_stages.iter().any(|s| s == stage)
    }

    pub fn mark_completed(&mut self, stage: &str) {
        if !self.is_completed(stage) {
            self.completed_stages.push(stage.to_string());
        }
        self.current_stage = None;
    }

    pub fn set_current(&mut self, stage: &str) {
        self.current_stage = Some(stage.to_string());
    }
}

pub fn load_state(path: &Path) -> Result<Option<InstallState>> {
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read state file: {}", path.display()))?;
    let state = serde_json::from_str(&content).context("Failed to parse state file")?;
    Ok(Some(state))
}

pub fn save_state_atomic(path: &Path, state: &InstallState) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create state directory: {}", parent.display()))?;
    }

    let tmp_path = temp_path(path);
    let payload = serde_json::to_string_pretty(state).context("Failed to serialize state")?;

    let mut file = File::create(&tmp_path)
        .with_context(|| format!("Failed to create temp state file: {}", tmp_path.display()))?;
    file.write_all(payload.as_bytes())
        .context("Failed to write state")?;
    file.sync_all().context("Failed to flush state")?;

    fs::rename(&tmp_path, path).with_context(|| {
        format!(
            "Failed to atomically replace state file: {}",
            path.display()
        )
    })?;

    if let Some(parent) = path.parent() {
        let dir = File::open(parent)
            .with_context(|| format!("Failed to open state directory: {}", parent.display()))?;
        dir.sync_all().ok();
    }

    Ok(())
}

fn temp_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("state.json");
    let tmp_name = format!("{}.tmp", file_name);
    path.with_file_name(tmp_name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn save_and_load_state_roundtrip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("state.json");
        let mut state = InstallState::new(true);
        state.set_current("stage-1");
        state.mark_completed("stage-0");

        save_state_atomic(&path, &state).unwrap();
        let loaded = load_state(&path).unwrap().unwrap();
        assert_eq!(state, loaded);
    }
}
