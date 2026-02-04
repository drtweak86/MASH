use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

pub use crate::downloader::DownloadArtifact;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum StageName {
    Preflight,
    DownloadAssets,
    MountPlan,
    FormatPlan,
    PackagePlan,
    KernelFixCheck,
    ResumeUnit,
    Other(String),
}

impl StageName {
    pub fn as_str(&self) -> &str {
        match self {
            StageName::Preflight => "Preflight",
            StageName::DownloadAssets => "Download assets",
            StageName::MountPlan => "Mount plan",
            StageName::FormatPlan => "Format plan",
            StageName::PackagePlan => "Package plan",
            StageName::KernelFixCheck => "Kernel fix check",
            StageName::ResumeUnit => "Resume unit",
            StageName::Other(s) => s.as_str(),
        }
    }
}

impl std::fmt::Display for StageName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl From<&str> for StageName {
    fn from(s: &str) -> Self {
        match s {
            "Preflight" => StageName::Preflight,
            "Download assets" => StageName::DownloadAssets,
            "Mount plan" => StageName::MountPlan,
            "Format plan" => StageName::FormatPlan,
            "Package plan" => StageName::PackagePlan,
            "Kernel fix check" => StageName::KernelFixCheck,
            "Resume unit" => StageName::ResumeUnit,
            other => StageName::Other(other.to_string()),
        }
    }
}

impl From<String> for StageName {
    fn from(value: String) -> Self {
        StageName::from(value.as_str())
    }
}

impl Serialize for StageName {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for StageName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(StageName::from(s.as_str()))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InstallState {
    pub version: u32,
    pub dry_run: bool,
    pub current_stage: Option<StageName>,
    pub completed_stages: Vec<StageName>,
    pub armed_execute: bool,
    pub typed_confirmation: bool,
    pub download_artifacts: Vec<DownloadArtifact>,
    pub verified_checksums: Vec<String>,
    pub formatted_devices: Vec<String>,
    pub partial_ok_resume: bool,
    pub boot_stage_completed: bool,
    #[serde(default)]
    pub selected_os: Option<String>,
    #[serde(default)]
    pub selected_variant: Option<String>,
    #[serde(default)]
    pub flashed_devices: Vec<String>,
    #[serde(default)]
    pub post_boot_partition_expansion_required: bool,
}

impl InstallState {
    pub fn new(dry_run: bool) -> Self {
        Self {
            version: 1,
            dry_run,
            current_stage: None,
            completed_stages: Vec::new(),
            armed_execute: false,
            typed_confirmation: false,
            download_artifacts: Vec::new(),
            verified_checksums: Vec::new(),
            formatted_devices: Vec::new(),
            partial_ok_resume: false,
            boot_stage_completed: false,
            selected_os: None,
            selected_variant: None,
            flashed_devices: Vec::new(),
            post_boot_partition_expansion_required: false,
        }
    }

    pub fn is_completed(&self, stage: &StageName) -> bool {
        self.completed_stages.iter().any(|s| s == stage)
    }

    pub fn mark_completed(&mut self, stage: &StageName) {
        if !self.is_completed(stage) {
            self.completed_stages.push(stage.clone());
        }
        self.current_stage = None;
    }

    pub fn set_current(&mut self, stage: &StageName) {
        self.current_stage = Some(stage.clone());
    }

    pub fn record_download(&mut self, artifact: DownloadArtifact) {
        self.download_artifacts.push(artifact);
    }

    pub fn record_formatted_device(&mut self, device: &Path) {
        let name = device.display().to_string();
        if !self.formatted_devices.iter().any(|entry| entry == &name) {
            self.formatted_devices.push(name);
        }
    }

    pub fn mark_boot_completed(&mut self) {
        self.boot_stage_completed = true;
    }

    pub fn mark_checksum_verified(&mut self, checksum: &str) {
        if !self
            .verified_checksums
            .iter()
            .any(|entry| entry == checksum)
        {
            self.verified_checksums.push(checksum.to_string());
        }
    }

    pub fn set_partial_resume(&mut self, partial: bool) {
        if partial {
            self.partial_ok_resume = true;
        }
    }

    pub fn arm_execute(&mut self) {
        self.armed_execute = true;
        self.typed_confirmation = true;
    }

    pub fn ensure_armed(&self) -> Result<()> {
        if self.dry_run {
            return Ok(());
        }
        if !self.armed_execute {
            anyhow::bail!("execute path requires armed confirmation (resume not armed)");
        }
        Ok(())
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
        state.set_current(&StageName::Other("stage-1".into()));
        state.mark_completed(&StageName::Other("stage-0".into()));
        state.record_download(DownloadArtifact::new(
            "disk.img".to_string(),
            dir.path().join("disk.img"),
            "abc".to_string(),
            1024,
            false,
        ));
        state.mark_checksum_verified("abc");
        state.set_partial_resume(true);

        save_state_atomic(&path, &state).unwrap();
        let loaded = load_state(&path).unwrap().unwrap();
        assert_eq!(state, loaded);
    }
}
