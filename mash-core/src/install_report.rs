//! Persistent install report artifact.
//!
//! WO-036: Always write a resumable, append-only-ish install report to disk.
//! Default path: `/mash/install-report.json` (override via `MASH_INSTALL_REPORT_PATH` for tests).

use crate::progress::{Phase, ProgressUpdate};
use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_REPORT_PATH: &str = "/mash/install-report.json";

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn hostname() -> Option<String> {
    std::fs::read_to_string("/etc/hostname")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn kernel_release() -> Option<String> {
    // Best effort: avoid adding dependencies; `uname -r` is stable on Linux.
    let mut cmd = std::process::Command::new("uname");
    cmd.arg("-r");
    crate::process_timeout::output_with_timeout(
        "uname",
        &mut cmd,
        std::time::Duration::from_secs(2),
    )
    .ok()
    .and_then(|out| String::from_utf8(out.stdout).ok())
    .map(|s| s.trim().to_string())
    .filter(|s| !s.is_empty())
}

fn parse_cpu_model(cpuinfo: &str) -> Option<String> {
    for line in cpuinfo.lines() {
        let lower = line.to_ascii_lowercase();
        if let Some(v) = line.split_once(':').map(|(_, v)| v.trim()) {
            if (lower.starts_with("model name")
                || lower.starts_with("hardware")
                || lower.starts_with("processor"))
                && !v.is_empty()
            {
                return Some(v.to_string());
            }
        }
    }
    None
}

fn parse_mem_total_kb(meminfo: &str) -> Option<u64> {
    for line in meminfo.lines() {
        if let Some(rest) = line.strip_prefix("MemTotal:") {
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if let Some(num) = parts.first() {
                return num.parse::<u64>().ok();
            }
        }
    }
    None
}

fn os_release_id_version() -> (Option<String>, Option<String>) {
    let content = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
    let mut id = None;
    let mut version = None;
    for line in content.lines() {
        if let Some(v) = line.strip_prefix("ID=") {
            id = Some(v.trim().trim_matches('"').to_string());
        }
        if let Some(v) = line.strip_prefix("VERSION_ID=") {
            version = Some(v.trim().trim_matches('"').to_string());
        }
    }
    (id, version)
}

pub fn report_path() -> PathBuf {
    std::env::var_os("MASH_INSTALL_REPORT_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_REPORT_PATH))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareInfo {
    pub collected_at_unix_ms: u64,
    pub hostname: Option<String>,
    pub kernel_release: Option<String>,
    pub arch: String,
    pub cpu_model: Option<String>,
    pub mem_total_kb: Option<u64>,
    pub os_release_id: Option<String>,
    pub os_release_version_id: Option<String>,
}

impl HardwareInfo {
    pub fn collect() -> Self {
        let cpuinfo = std::fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
        let meminfo = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
        let (os_id, os_ver) = os_release_id_version();
        Self {
            collected_at_unix_ms: now_unix_ms(),
            hostname: hostname(),
            kernel_release: kernel_release(),
            arch: std::env::consts::ARCH.to_string(),
            cpu_model: parse_cpu_model(&cpuinfo),
            mem_total_kb: parse_mem_total_kb(&meminfo),
            os_release_id: os_id,
            os_release_version_id: os_ver,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunMode {
    DryRun,
    Execute,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StageStatus {
    Started,
    Completed,
    Skipped,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageResult {
    pub name: String,
    pub started_at_unix_ms: Option<u64>,
    pub ended_at_unix_ms: Option<u64>,
    pub status: StageStatus,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskIdentityReport {
    pub vendor: Option<String>,
    pub model: Option<String>,
    pub transport: Option<String>,
    pub size_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionReport {
    pub distro: String,
    pub flavour: Option<String>,
    pub target_disk: String,
    pub disk_identity: Option<DiskIdentityReport>,
    pub partition_scheme: Option<String>,
    pub efi_size: Option<String>,
    pub boot_size: Option<String>,
    pub root_end: Option<String>,
    pub efi_source: Option<String>,
    pub efi_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallReport {
    pub report_version: u32,
    pub started_at_unix_ms: u64,
    pub ended_at_unix_ms: Option<u64>,
    pub mode: RunMode,
    pub destructive_armed: bool,
    pub typed_confirmation: bool,
    pub hardware: HardwareInfo,
    pub selection: SelectionReport,

    /// Phase-oriented results (Fedora flash path).
    #[serde(default)]
    pub phases: BTreeMap<String, StageResult>,
    /// Generic stage results (workflow/orchestration stages).
    #[serde(default)]
    pub stages: Vec<StageResult>,

    #[serde(default)]
    pub last_status: Option<String>,
    #[serde(default)]
    pub errors: Vec<String>,
}

impl InstallReport {
    fn new(
        mode: RunMode,
        destructive_armed: bool,
        typed_confirmation: bool,
        selection: SelectionReport,
    ) -> Self {
        Self {
            report_version: 1,
            started_at_unix_ms: now_unix_ms(),
            ended_at_unix_ms: None,
            mode,
            destructive_armed,
            typed_confirmation,
            hardware: HardwareInfo::collect(),
            selection,
            phases: BTreeMap::new(),
            stages: Vec::new(),
            last_status: None,
            errors: Vec::new(),
        }
    }
}

#[derive(Clone)]
pub struct InstallReportWriter {
    path: PathBuf,
    inner: Arc<Mutex<InstallReport>>,
}

impl InstallReportWriter {
    pub fn new(
        mode: RunMode,
        destructive_armed: bool,
        typed_confirmation: bool,
        selection: SelectionReport,
    ) -> anyhow::Result<Self> {
        let path = report_path();
        let report = InstallReport::new(mode, destructive_armed, typed_confirmation, selection);
        let writer = Self {
            path,
            inner: Arc::new(Mutex::new(report)),
        };
        writer.persist().ok(); // best-effort at creation
        Ok(writer)
    }

    pub fn record_progress_update(&self, update: &ProgressUpdate) {
        if let Ok(mut report) = self.inner.lock() {
            match update {
                ProgressUpdate::PhaseStarted(phase) => {
                    let key = phase_key(*phase);
                    let entry = report.phases.entry(key).or_insert_with(|| StageResult {
                        name: phase.name().to_string(),
                        started_at_unix_ms: Some(now_unix_ms()),
                        ended_at_unix_ms: None,
                        status: StageStatus::Started,
                        error: None,
                    });
                    entry.started_at_unix_ms.get_or_insert(now_unix_ms());
                    entry.status = StageStatus::Started;
                }
                ProgressUpdate::PhaseCompleted(phase) => {
                    let key = phase_key(*phase);
                    let entry = report.phases.entry(key).or_insert_with(|| StageResult {
                        name: phase.name().to_string(),
                        started_at_unix_ms: None,
                        ended_at_unix_ms: None,
                        status: StageStatus::Completed,
                        error: None,
                    });
                    entry.ended_at_unix_ms = Some(now_unix_ms());
                    entry.status = StageStatus::Completed;
                }
                ProgressUpdate::PhaseSkipped(phase) => {
                    let key = phase_key(*phase);
                    let entry = report.phases.entry(key).or_insert_with(|| StageResult {
                        name: phase.name().to_string(),
                        started_at_unix_ms: None,
                        ended_at_unix_ms: None,
                        status: StageStatus::Skipped,
                        error: None,
                    });
                    if entry.started_at_unix_ms.is_none() {
                        entry.started_at_unix_ms = Some(now_unix_ms());
                    }
                    entry.ended_at_unix_ms = Some(now_unix_ms());
                    entry.status = StageStatus::Skipped;
                }
                ProgressUpdate::Status(msg) => {
                    report.last_status = Some(msg.clone());
                }
                ProgressUpdate::Error(msg) => {
                    report.errors.push(msg.clone());
                    report.ended_at_unix_ms = Some(now_unix_ms());
                }
                ProgressUpdate::Complete => {
                    report.ended_at_unix_ms = Some(now_unix_ms());
                }
                ProgressUpdate::RsyncProgress { .. } | ProgressUpdate::DiskIo { .. } => {}
            }
        }
        let _ = self.persist();
    }

    pub fn stage_started(&self, name: &str) {
        if let Ok(mut report) = self.inner.lock() {
            report.stages.push(StageResult {
                name: name.to_string(),
                started_at_unix_ms: Some(now_unix_ms()),
                ended_at_unix_ms: None,
                status: StageStatus::Started,
                error: None,
            });
        }
        let _ = self.persist();
    }

    pub fn stage_completed(&self, name: &str) {
        if let Ok(mut report) = self.inner.lock() {
            if let Some(last) = report
                .stages
                .iter_mut()
                .rev()
                .find(|s| s.name == name && matches!(s.status, StageStatus::Started))
            {
                last.status = StageStatus::Completed;
                last.ended_at_unix_ms = Some(now_unix_ms());
            } else {
                report.stages.push(StageResult {
                    name: name.to_string(),
                    started_at_unix_ms: None,
                    ended_at_unix_ms: Some(now_unix_ms()),
                    status: StageStatus::Completed,
                    error: None,
                });
            }
        }
        let _ = self.persist();
    }

    pub fn stage_error(&self, name: &str, error: &str) {
        if let Ok(mut report) = self.inner.lock() {
            report.errors.push(error.to_string());
            report.stages.push(StageResult {
                name: name.to_string(),
                started_at_unix_ms: Some(now_unix_ms()),
                ended_at_unix_ms: Some(now_unix_ms()),
                status: StageStatus::Error,
                error: Some(error.to_string()),
            });
            report.ended_at_unix_ms = Some(now_unix_ms());
        }
        let _ = self.persist();
    }

    pub fn persist(&self) -> anyhow::Result<()> {
        let report = self
            .inner
            .lock()
            .map_err(|_| anyhow::anyhow!("install report mutex poisoned"))?
            .clone();
        write_json_atomic(&self.path, &report).context("failed to persist install report")
    }
}

fn phase_key(phase: Phase) -> String {
    format!(
        "{:02}_{}",
        phase.number(),
        format!("{:?}", phase).to_ascii_lowercase()
    )
}

fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create install report directory: {}",
                parent.display()
            )
        })?;
    }
    let tmp = path.with_extension("json.tmp");
    let payload = serde_json::to_string_pretty(value).context("failed to serialize report")?;
    fs::write(&tmp, payload).with_context(|| format!("failed to write {}", tmp.display()))?;
    fs::rename(&tmp, path).with_context(|| {
        format!(
            "failed to atomically replace install report: {}",
            path.display()
        )
    })?;
    if let Some(parent) = path.parent() {
        let dir = fs::File::open(parent).with_context(|| {
            format!(
                "failed to open install report directory for sync: {}",
                parent.display()
            )
        })?;
        let _ = dir.sync_all();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn report_writes_atomic_json() {
        let dir = tempdir().unwrap();
        let report_path = dir.path().join("install-report.json");
        std::env::set_var("MASH_INSTALL_REPORT_PATH", &report_path);
        let selection = SelectionReport {
            distro: "Fedora".to_string(),
            flavour: Some("KDE".to_string()),
            target_disk: "/dev/sdz".to_string(),
            disk_identity: None,
            partition_scheme: Some("MBR".to_string()),
            efi_size: Some("1024MiB".to_string()),
            boot_size: Some("2048MiB".to_string()),
            root_end: Some("1800GiB".to_string()),
            efi_source: Some("local".to_string()),
            efi_path: Some("/tmp/uefi".to_string()),
        };
        let writer = InstallReportWriter::new(RunMode::DryRun, false, false, selection).unwrap();
        writer.record_progress_update(&ProgressUpdate::PhaseSkipped(Phase::Partition));
        writer.record_progress_update(&ProgressUpdate::Complete);

        let content = std::fs::read_to_string(report_path).unwrap();
        let parsed: InstallReport = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed.report_version, 1);
        assert!(parsed.started_at_unix_ms > 0);
        assert!(parsed.ended_at_unix_ms.is_some());
        assert!(parsed.phases.contains_key("03_partition"));
    }
}
