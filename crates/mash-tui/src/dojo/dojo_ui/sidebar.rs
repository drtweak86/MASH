use super::super::dojo_app::{App, InstallStepType};

pub(super) fn build_step_sidebar(app: &App) -> String {
    let all_steps = [
        (InstallStepType::Welcome, "Welcome"),
        (InstallStepType::ImageSelection, "Select OS"),
        (InstallStepType::VariantSelection, "Select Flavour"),
        (InstallStepType::DownloadSourceSelection, "Image Source"),
        (InstallStepType::DiskSelection, "Select Disk"),
        (InstallStepType::DiskConfirmation, "Confirm Disk"),
        (InstallStepType::PartitionScheme, "Partition Scheme"),
        (InstallStepType::PartitionLayout, "Partition Layout"),
        (InstallStepType::EfiImage, "EFI Setup"),
        (InstallStepType::LocaleSelection, "Locale/Keymap"),
        (InstallStepType::Options, "Options"),
        (InstallStepType::PlanReview, "Review Plan"),
        (InstallStepType::Confirmation, "Confirm"),
        (InstallStepType::ExecuteConfirmationGate, "Execute Gate"),
        (InstallStepType::Flashing, "Installing..."),
        (InstallStepType::Complete, "Complete!"),
    ];

    let mut lines = Vec::new();
    for (step, label) in &all_steps {
        let marker = if *step == app.current_step_type {
            "▶"
        } else if is_step_before(*step, app.current_step_type) {
            "✓"
        } else {
            " "
        };
        lines.push(format!("{} {}", marker, label));
    }
    lines.join("\n")
}

/// Check if step_a comes before step_b in the flow
pub(super) fn is_step_before(step_a: InstallStepType, step_b: InstallStepType) -> bool {
    let order = [
        InstallStepType::Welcome,
        InstallStepType::ImageSelection,
        InstallStepType::VariantSelection,
        InstallStepType::DownloadSourceSelection,
        InstallStepType::DiskSelection,
        InstallStepType::DiskConfirmation,
        InstallStepType::PartitionScheme,
        InstallStepType::PartitionLayout,
        InstallStepType::PartitionCustomize,
        InstallStepType::EfiImage,
        InstallStepType::LocaleSelection,
        InstallStepType::Options,
        InstallStepType::PlanReview,
        InstallStepType::Confirmation,
        InstallStepType::ExecuteConfirmationGate,
        InstallStepType::DisarmSafeMode,
        InstallStepType::Flashing,
        InstallStepType::Complete,
    ];

    let pos_a = order.iter().position(|&s| s == step_a);
    let pos_b = order.iter().position(|&s| s == step_b);

    match (pos_a, pos_b) {
        (Some(a), Some(b)) => a < b,
        _ => false,
    }
}
