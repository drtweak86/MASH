//! Progress tracking for the installation process

use std::time::{Duration, Instant};

/// Installation phases
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Partition,
    Format,
    CopyRoot,
    CopyBoot,
    CopyEfi,
    UefiConfig,
    LocaleConfig,
    Fstab,
    StageDojo,
    Cleanup,
}

impl Phase {
    /// Get the jovial emoji-enhanced phase name
    pub fn name(&self) -> &'static str {
        match self {
            Phase::Partition => "🔪 Slicing up the disk",
            Phase::Format => "✨ Making it sparkly clean",
            Phase::CopyRoot => "📦 Moving the big stuff",
            Phase::CopyBoot => "🥾 Setting up the boots",
            Phase::CopyEfi => "🎩 Adding the fancy hat",
            Phase::UefiConfig => "🔧 Tweaking the UEFI magic",
            Phase::LocaleConfig => "🗣️ Teaching it your language",
            Phase::Fstab => "📋 Writing the mount map",
            Phase::StageDojo => "🥋 Preparing the Dojo",
            Phase::Cleanup => "🧹 Tidying up",
        }
    }

    pub fn number(&self) -> usize {
        match self {
            Phase::Partition => 1,
            Phase::Format => 2,
            Phase::CopyRoot => 3,
            Phase::CopyBoot => 4,
            Phase::CopyEfi => 5,
            Phase::UefiConfig => 6,
            Phase::LocaleConfig => 7,
            Phase::Fstab => 8,
            Phase::StageDojo => 9,
            Phase::Cleanup => 10,
        }
    }

    pub fn total() -> usize {
        10
    }

    pub fn all() -> &'static [Phase] {
        &[
            Phase::Partition,
            Phase::Format,
            Phase::CopyRoot,
            Phase::CopyBoot,
            Phase::CopyEfi,
            Phase::UefiConfig,
            Phase::LocaleConfig,
            Phase::Fstab,
            Phase::StageDojo,
            Phase::Cleanup,
        ]
    }

    /// Get the spinner frames for this phase type
    pub fn spinners(&self) -> &'static [&'static str] {
        match self {
            // Disk operations
            Phase::Partition | Phase::Format => &["💿", "📀", "💾", "🖴"],
            // Copy operations
            Phase::CopyRoot | Phase::CopyBoot | Phase::CopyEfi => &["📦", "📤", "📥", "🗃️"],
            // Config operations
            Phase::UefiConfig | Phase::LocaleConfig | Phase::Fstab => &["🔧", "⚙️", "🛠️", "🔩"],
            // Dojo stage
            Phase::StageDojo => &["🥋", "🥷", "⚔️", "🏯"],
            // Cleanup
            Phase::Cleanup => &["🧹", "🧼", "✨", "🪄"],
        }
    }

    /// Get the current spinner frame for this phase
    pub fn spinner_frame(&self, tick: u64) -> &'static str {
        let spinners = self.spinners();
        let index = (tick / 3) as usize % spinners.len(); // Change every 3 ticks (~300ms)
        spinners[index]
    }
}

/// Progress update message
#[derive(Debug, Clone)]
pub enum ProgressUpdate {
    /// Started a new phase
    PhaseStarted(Phase),
    /// Completed a phase
    PhaseCompleted(Phase),
    /// Rsync progress update (percent, bytes_per_sec, files_done, files_total)
    RsyncProgress {
        percent: f64,
        speed_mbps: f64,
        files_done: u64,
        files_total: u64,
    },
    /// Disk I/O rate update
    DiskIo { mbps: f64 },
    /// Status message
    Status(String),
    /// Installation completed successfully
    Complete,
    /// Installation failed with error
    Error(String),
}

/// State of installation progress
#[derive(Debug, Clone)]
pub struct ProgressState {
    /// Current phase
    pub current_phase: Option<Phase>,
    /// Completed phases
    pub completed_phases: Vec<Phase>,
    /// Overall progress percentage (0-100)
    pub overall_percent: f64,
    /// Current phase progress percentage (0-100)
    pub phase_percent: f64,
    /// Current rsync speed in MB/s
    pub rsync_speed: f64,
    /// Peak rsync speed in MB/s
    pub peak_speed: f64,
    /// Running average speed in MB/s
    pub average_speed: f64,
    /// Speed samples for averaging
    speed_samples: Vec<f64>,
    /// Disk I/O speed in MB/s
    pub disk_io_speed: f64,
    /// Bytes transferred
    pub bytes_done: u64,
    /// Total bytes to transfer
    pub bytes_total: u64,
    /// Files copied / total
    pub files_done: u64,
    pub files_total: u64,
    /// Current file being processed
    pub current_file: Option<String>,
    /// Status message
    pub status: String,
    /// Start time
    pub start_time: Option<Instant>,
    /// Phase start time
    pub phase_start_time: Option<Instant>,
    /// Is installation complete?
    pub is_complete: bool,
    /// Error message if failed
    pub error: Option<String>,
}

impl Default for ProgressState {
    fn default() -> Self {
        Self {
            current_phase: None,
            completed_phases: Vec::new(),
            overall_percent: 0.0,
            phase_percent: 0.0,
            rsync_speed: 0.0,
            peak_speed: 0.0,
            average_speed: 0.0,
            speed_samples: Vec::new(),
            disk_io_speed: 0.0,
            bytes_done: 0,
            bytes_total: 0,
            files_done: 0,
            files_total: 0,
            current_file: None,
            status: "🚀 Starting installation...".to_string(),
            start_time: None,
            phase_start_time: None,
            is_complete: false,
            error: None,
        }
    }
}

impl ProgressState {
    /// Apply a progress update
    pub fn apply_update(&mut self, update: ProgressUpdate) {
        match update {
            ProgressUpdate::PhaseStarted(phase) => {
                self.current_phase = Some(phase);
                self.phase_percent = 0.0;
                self.status = format!("{}...", phase.name());
                self.phase_start_time = Some(Instant::now());
                if self.start_time.is_none() {
                    self.start_time = Some(Instant::now());
                }
                // Update overall progress based on phase
                self.update_overall_progress();
            }
            ProgressUpdate::PhaseCompleted(phase) => {
                if !self.completed_phases.contains(&phase) {
                    self.completed_phases.push(phase);
                }
                self.phase_percent = 100.0;
                self.update_overall_progress();
            }
            ProgressUpdate::RsyncProgress {
                percent,
                speed_mbps,
                files_done,
                files_total,
            } => {
                self.phase_percent = percent;
                self.rsync_speed = speed_mbps;
                self.files_done = files_done;
                self.files_total = files_total;

                // Track peak speed
                if speed_mbps > self.peak_speed {
                    self.peak_speed = speed_mbps;
                }

                // Update running average
                self.speed_samples.push(speed_mbps);
                if self.speed_samples.len() > 100 {
                    self.speed_samples.remove(0);
                }
                self.average_speed =
                    self.speed_samples.iter().sum::<f64>() / self.speed_samples.len() as f64;

                self.update_overall_progress();
            }
            ProgressUpdate::DiskIo { mbps } => {
                self.disk_io_speed = mbps;
            }
            ProgressUpdate::Status(msg) => {
                self.status = msg;
            }
            ProgressUpdate::Complete => {
                self.is_complete = true;
                self.overall_percent = 100.0;
                self.status = "🎉 Installation complete!".to_string();
            }
            ProgressUpdate::Error(msg) => {
                self.is_complete = true;
                self.error = Some(msg.clone());
                self.status = format!("❌ Error: {}", msg);
            }
        }
    }

    fn update_overall_progress(&mut self) {
        let completed = self.completed_phases.len() as f64;
        let current_contribution = if self.current_phase.is_some() {
            self.phase_percent / 100.0
        } else {
            0.0
        };
        let total = Phase::total() as f64;
        self.overall_percent = ((completed + current_contribution) / total) * 100.0;
    }

    /// Get estimated time remaining
    pub fn eta(&self) -> Option<Duration> {
        let start = self.start_time?;
        let elapsed = start.elapsed();

        if self.overall_percent <= 0.0 {
            return None;
        }

        let total_estimated = elapsed.as_secs_f64() / (self.overall_percent / 100.0);
        let remaining = total_estimated - elapsed.as_secs_f64();

        if remaining > 0.0 {
            Some(Duration::from_secs_f64(remaining))
        } else {
            None
        }
    }

    /// Get elapsed time
    pub fn elapsed(&self) -> Option<Duration> {
        self.start_time.map(|t| t.elapsed())
    }

    /// Get phase elapsed time
    pub fn phase_elapsed(&self) -> Option<Duration> {
        self.phase_start_time.map(|t| t.elapsed())
    }

    /// Format duration as string
    pub fn format_duration(d: Duration) -> String {
        let secs = d.as_secs();
        if secs < 60 {
            format!("{}s", secs)
        } else if secs < 3600 {
            format!("{}m {}s", secs / 60, secs % 60)
        } else {
            format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
        }
    }

    /// Format ETA as string
    pub fn eta_string(&self) -> String {
        match self.eta() {
            Some(d) => Self::format_duration(d),
            None => "⏳ calculating...".to_string(),
        }
    }

    /// Format elapsed time as string
    pub fn elapsed_string(&self) -> String {
        match self.elapsed() {
            Some(d) => Self::format_duration(d),
            None => "0s".to_string(),
        }
    }

    /// Format phase elapsed time as string
    pub fn phase_elapsed_string(&self) -> String {
        match self.phase_elapsed() {
            Some(d) => Self::format_duration(d),
            None => "0s".to_string(),
        }
    }

    /// Get phase status symbol
    pub fn phase_symbol(&self, phase: Phase) -> &'static str {
        if self.completed_phases.contains(&phase) {
            "✅" // Completed
        } else if self.current_phase == Some(phase) {
            "⏳" // In progress
        } else {
            "⏸️" // Pending
        }
    }

    /// Format bytes as human-readable string
    pub fn format_bytes(bytes: u64) -> String {
        const KB: u64 = 1024;
        const MB: u64 = 1024 * KB;
        const GB: u64 = 1024 * MB;

        if bytes >= GB {
            format!("{:.1} GB", bytes as f64 / GB as f64)
        } else if bytes >= MB {
            format!("{:.1} MB", bytes as f64 / MB as f64)
        } else if bytes >= KB {
            format!("{:.1} KB", bytes as f64 / KB as f64)
        } else {
            format!("{} B", bytes)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_progress_updates() {
        let mut state = ProgressState::default();

        state.apply_update(ProgressUpdate::PhaseStarted(Phase::Partition));
        assert_eq!(state.current_phase, Some(Phase::Partition));

        state.apply_update(ProgressUpdate::PhaseCompleted(Phase::Partition));
        assert!(state.completed_phases.contains(&Phase::Partition));

        state.apply_update(ProgressUpdate::Complete);
        assert!(state.is_complete);
        assert_eq!(state.overall_percent, 100.0);
    }

    #[test]
    fn test_phase_spinners() {
        let disk_phase = Phase::Partition;
        let copy_phase = Phase::CopyRoot;
        let config_phase = Phase::UefiConfig;

        // Each phase type should have its own spinners
        assert_eq!(disk_phase.spinners()[0], "💿");
        assert_eq!(copy_phase.spinners()[0], "📦");
        assert_eq!(config_phase.spinners()[0], "🔧");
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(ProgressState::format_bytes(512), "512 B");
        assert_eq!(ProgressState::format_bytes(1536), "1.5 KB");
        assert_eq!(ProgressState::format_bytes(1_500_000), "1.4 MB");
        assert_eq!(ProgressState::format_bytes(2_500_000_000), "2.3 GB");
    }
}
