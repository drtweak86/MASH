
/// Defines the sequence of steps in the Dojo UI
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallStepType {
    Welcome,
    DiskSelection,
    DiskConfirmation,
    BackupConfirmation, // New step for backup
    PartitionScheme,
    PartitionLayout,
    PartitionCustomize,
    DownloadSourceSelection,
    ImageSelection,
    VariantSelection,
    EfiImage,
    LocaleSelection,
    Options,
    FirstBootUser,
    PlanReview,
    Confirmation,
    ExecuteConfirmationGate,
    DisarmSafeMode,
    DownloadingFedora,
    DownloadingUefi,
    Flashing,
    Complete,
}

impl InstallStepType {
    pub fn all() -> &'static [InstallStepType] {
        &[
            InstallStepType::Welcome,
            InstallStepType::ImageSelection,
            InstallStepType::VariantSelection,
            InstallStepType::DownloadSourceSelection,
            InstallStepType::DiskSelection,
            InstallStepType::DiskConfirmation,
            InstallStepType::BackupConfirmation,
            InstallStepType::PartitionScheme,
            InstallStepType::PartitionLayout,
            InstallStepType::PartitionCustomize,
            InstallStepType::EfiImage,
            InstallStepType::LocaleSelection,
            InstallStepType::Options,
            InstallStepType::FirstBootUser,
            InstallStepType::PlanReview,
            InstallStepType::Confirmation,
            InstallStepType::ExecuteConfirmationGate,
            InstallStepType::DisarmSafeMode,
            InstallStepType::DownloadingFedora,
            InstallStepType::DownloadingUefi,
            InstallStepType::Flashing,
            InstallStepType::Complete,
        ]
    }

    pub fn title(&self) -> &'static str {
        match self {
            InstallStepType::Welcome => "Enter the Dojo",
            InstallStepType::DiskSelection => "Select Target Disk",
            InstallStepType::DiskConfirmation => "Confirm Disk Destruction",
            InstallStepType::BackupConfirmation => "Backup Confirmation", // Title for new step
            InstallStepType::PartitionScheme => "Partition Scheme",
            InstallStepType::PartitionLayout => "Partition Layout",
            InstallStepType::PartitionCustomize => "Customize Partitions",
            InstallStepType::DownloadSourceSelection => "Select Image Source",
            InstallStepType::ImageSelection => "Select OS",
            InstallStepType::VariantSelection => "Select Variant",
            InstallStepType::EfiImage => "EFI Image",
            InstallStepType::LocaleSelection => "Locale & Keymap",
            InstallStepType::Options => "Installation Options",
            InstallStepType::FirstBootUser => "First-Boot User",
            InstallStepType::PlanReview => "Execution Plan",
            InstallStepType::Confirmation => "Final Confirmation",
            InstallStepType::ExecuteConfirmationGate => "Final Execute Gate",
            InstallStepType::DisarmSafeMode => "Disarm Safe Mode",
            InstallStepType::DownloadingFedora => "Downloading Fedora Image",
            InstallStepType::DownloadingUefi => "Downloading EFI Image",
            InstallStepType::Flashing => "Installing...",
            InstallStepType::Complete => "Installation Complete!",
        }
    }

    // Helper to get the next step in the sequence
    // Flow: Welcome → Distro → Flavour → Download Source → Disk → Partition → EFI → Locale → Options → Review → Confirm
    pub fn next(&self) -> Option<InstallStepType> {
        match self {
            InstallStepType::Welcome => Some(InstallStepType::ImageSelection),
            InstallStepType::ImageSelection => Some(InstallStepType::VariantSelection),
            InstallStepType::VariantSelection => Some(InstallStepType::DownloadSourceSelection),
            InstallStepType::DownloadSourceSelection => Some(InstallStepType::DiskSelection),
            InstallStepType::DiskSelection => Some(InstallStepType::DiskConfirmation),
            InstallStepType::DiskConfirmation => Some(InstallStepType::PartitionScheme),
            InstallStepType::BackupConfirmation => Some(InstallStepType::PartitionScheme),
            InstallStepType::PartitionScheme => Some(InstallStepType::PartitionLayout),
            InstallStepType::PartitionLayout => Some(InstallStepType::PartitionCustomize),
            InstallStepType::PartitionCustomize => Some(InstallStepType::EfiImage),
            InstallStepType::EfiImage => Some(InstallStepType::LocaleSelection),
            InstallStepType::LocaleSelection => Some(InstallStepType::Options),
            InstallStepType::Options => Some(InstallStepType::PlanReview),
            InstallStepType::FirstBootUser => Some(InstallStepType::PlanReview),
            InstallStepType::PlanReview => Some(InstallStepType::Confirmation),
            InstallStepType::Confirmation => None,
            InstallStepType::ExecuteConfirmationGate => None,
            InstallStepType::DisarmSafeMode => None,
            InstallStepType::DownloadingFedora => None, // Execution steps do not have 'next' in this flow
            InstallStepType::DownloadingUefi => None, // Execution steps do not have 'next' in this flow
            InstallStepType::Flashing => Some(InstallStepType::Complete),
            InstallStepType::Complete => None,
        }
    }

    // Helper to get the previous step in the sequence
    pub fn prev(&self) -> Option<InstallStepType> {
        match self {
            InstallStepType::Welcome => None,
            InstallStepType::ImageSelection => Some(InstallStepType::Welcome),
            InstallStepType::VariantSelection => Some(InstallStepType::ImageSelection),
            InstallStepType::DownloadSourceSelection => Some(InstallStepType::VariantSelection),
            InstallStepType::DiskSelection => Some(InstallStepType::DownloadSourceSelection),
            InstallStepType::DiskConfirmation => Some(InstallStepType::DiskSelection),
            InstallStepType::BackupConfirmation => Some(InstallStepType::DiskConfirmation),
            InstallStepType::PartitionScheme => Some(InstallStepType::DiskConfirmation),
            InstallStepType::PartitionLayout => Some(InstallStepType::PartitionScheme),
            InstallStepType::PartitionCustomize => Some(InstallStepType::PartitionLayout),
            InstallStepType::EfiImage => Some(InstallStepType::PartitionCustomize),
            InstallStepType::LocaleSelection => Some(InstallStepType::EfiImage),
            InstallStepType::Options => Some(InstallStepType::LocaleSelection),
            InstallStepType::FirstBootUser => Some(InstallStepType::Options),
            InstallStepType::PlanReview => Some(InstallStepType::Options),
            InstallStepType::Confirmation => Some(InstallStepType::PlanReview),
            InstallStepType::ExecuteConfirmationGate => Some(InstallStepType::Confirmation),
            InstallStepType::DisarmSafeMode => Some(InstallStepType::Confirmation),
            _ => None, // Execution steps do not have 'prev' in this flow
        }
    }

    // Check if this step is part of the configuration phase
    pub fn is_config_step(&self) -> bool {
        matches!(
            self,
            InstallStepType::Welcome
                | InstallStepType::DiskSelection
                | InstallStepType::DiskConfirmation
                | InstallStepType::BackupConfirmation
                | InstallStepType::PartitionScheme
                | InstallStepType::PartitionLayout
                | InstallStepType::PartitionCustomize
                | InstallStepType::DownloadSourceSelection
                | InstallStepType::ImageSelection
                | InstallStepType::VariantSelection
                | InstallStepType::EfiImage
                | InstallStepType::LocaleSelection
                | InstallStepType::Options
                | InstallStepType::FirstBootUser
                | InstallStepType::PlanReview
                | InstallStepType::Confirmation
                | InstallStepType::ExecuteConfirmationGate
                | InstallStepType::DisarmSafeMode
        )
    }
}
