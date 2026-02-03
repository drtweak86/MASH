//! Flash module - Full installation pipeline for MASH
//!
//! Handles partitioning, formatting, copying, and configuration of
//! Fedora KDE for Raspberry Pi 4 with UEFI boot.
//!
//! Based on holy-loop-fedora-uefi-gpt.sh - GPT with 4 partitions:
//!   p1: EFI (vfat) - esp flag
//!   p2: BOOT (ext4)
//!   p3: ROOT (btrfs with subvols: root, home, var)
//!   p4: DATA (ext4) - for mash-staging

use anyhow::{bail, Context, Result};
use log::info;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, OnceLock};

use crate::cli::PartitionScheme;
use crate::config_states::{
    ArmedConfig, ExecuteArmToken, HasRunMode, UnvalidatedConfig, ValidateConfig, ValidatedConfig,
};
use crate::install_report::{DiskIdentityReport, InstallReportWriter, RunMode, SelectionReport};
use crate::locale::LocaleConfig;
use crate::progress::{Phase, ProgressUpdate};
use mash_hal::{
    FlashOptions, FormatOptions, InstallerHal, LinuxHal, MountOptions, PartedOp, PartedOptions,
    RsyncOptions, WipeFsOptions,
};

/// Flash a full-disk image to a target block device.
///
/// - Supports raw images and `.xz`-compressed images.
/// - Uses streaming decompression for `.xz` so it does not require an intermediate uncompressed file.
///
/// This is used for non-Fedora OS profiles (Ubuntu, Raspberry Pi OS, Manjaro) which ship as full
/// disk images with their own partition tables.
pub fn flash_raw_image_to_disk(
    hal: &dyn mash_hal::FlashOps,
    image_path: &Path,
    target_disk: &Path,
    opts: &FlashOptions,
) -> Result<()> {
    info!(
        "💾 Flashing image {} -> {}",
        image_path.display(),
        target_disk.display()
    );

    hal.flash_raw_image(image_path, target_disk, opts)
        .map_err(anyhow::Error::new)
        .context("Failed to flash raw image to target disk")
}

/// Mount points for the installation
struct MountPoints {
    // Source (image) mounts
    src_efi: PathBuf,
    src_boot: PathBuf,
    src_root_top: PathBuf,
    src_root_subvol: PathBuf,
    src_home_subvol: PathBuf,
    src_var_subvol: PathBuf,
    // Destination (target) mounts
    dst_efi: PathBuf,
    dst_boot: PathBuf,
    dst_data: PathBuf,
    dst_root_top: PathBuf,
    dst_root_subvol: PathBuf,
    dst_home_subvol: PathBuf,
    dst_var_subvol: PathBuf,
}

impl MountPoints {
    fn new(work_dir: &Path) -> Self {
        let src = work_dir.join("src");
        let dst = work_dir.join("dst");
        Self {
            src_efi: src.join("efi"),
            src_boot: src.join("boot"),
            src_root_top: src.join("root_top"),
            src_root_subvol: src.join("root_sub_root"),
            src_home_subvol: src.join("root_sub_home"),
            src_var_subvol: src.join("root_sub_var"),
            dst_efi: dst.join("efi"),
            dst_boot: dst.join("boot"),
            dst_data: dst.join("data"),
            dst_root_top: dst.join("root_top"),
            dst_root_subvol: dst.join("root_sub_root"),
            dst_home_subvol: dst.join("root_sub_home"),
            dst_var_subvol: dst.join("root_sub_var"),
        }
    }

    fn create_all(&self) -> Result<()> {
        for dir in [
            &self.src_efi,
            &self.src_boot,
            &self.src_root_top,
            &self.src_root_subvol,
            &self.src_home_subvol,
            &self.src_var_subvol,
            &self.dst_efi,
            &self.dst_boot,
            &self.dst_data,
            &self.dst_root_top,
            &self.dst_root_subvol,
            &self.dst_home_subvol,
            &self.dst_var_subvol,
        ] {
            fs::create_dir_all(dir)
                .with_context(|| format!("Failed to create mount point: {}", dir.display()))?;
        }
        Ok(())
    }
}

/// Detected btrfs subvolumes in source image
struct BtrfsSubvols {
    has_root: bool,
    has_home: bool,
    has_var: bool,
}

/// Installation context with all configuration
pub struct FlashContext {
    pub hal: Arc<dyn InstallerHal>,
    pub image: PathBuf,
    pub disk: String,
    pub scheme: PartitionScheme,
    pub uefi_dir: PathBuf,
    pub dry_run: bool,
    pub auto_unmount: bool,
    pub locale: Option<LocaleConfig>,
    pub early_ssh: bool,
    pub progress_tx: Option<Sender<ProgressUpdate>>,
    pub work_dir: PathBuf,
    pub loop_device: Option<String>,
    pub efi_size: String,
    pub boot_size: String,
    pub root_end: String,
    pub report: Option<InstallReportWriter>,
}

impl FlashContext {
    fn send_progress(&self, update: ProgressUpdate) {
        if let Some(ref report) = self.report {
            report.record_progress_update(&update);
        }
        if let Some(ref tx) = self.progress_tx {
            let _ = tx.send(update);
        }
    }

    fn check_cancel(&self) -> Result<()> {
        if cancel_requested() {
            self.status("🧹 Cleaning up...");
            bail!("Cancelled");
        }
        Ok(())
    }

    fn start_phase(&self, phase: Phase) {
        info!("📍 Starting phase: {}", phase.name());
        self.send_progress(ProgressUpdate::PhaseStarted(phase));
    }

    fn complete_phase(&self, phase: Phase) {
        info!("✅ Completed phase: {}", phase.name());
        self.send_progress(ProgressUpdate::PhaseCompleted(phase));
    }

    fn status(&self, msg: &str) {
        info!("{}", msg);
        self.send_progress(ProgressUpdate::Status(msg.to_string()));
    }

    /// Get partition device path (handles nvme/mmcblk naming)
    fn partition_path(&self, num: u32) -> String {
        if self.disk.contains("nvme") || self.disk.contains("mmcblk") {
            format!("{}p{}", self.disk, num)
        } else {
            format!("{}{}", self.disk, num)
        }
    }
}

/// Core flashing configuration.
///
/// This is the validated, non-UI-specific input required to execute a flash.
/// (UI-only selections like "download Fedora" belong in the TUI layer.)
#[derive(Debug, Clone)]
pub struct FlashConfig {
    /// Human label for the selected OS (for reporting / completion messaging).
    pub os_distro: Option<String>,
    /// Human label for the selected flavour/variant (for reporting / completion messaging).
    pub os_flavour: Option<String>,
    /// Selected target disk identity (best-effort; may be missing on some hardware).
    pub disk_identity: Option<DiskIdentityReport>,
    /// Where EFI came from (e.g. "download" or "local") for reporting.
    pub efi_source: Option<String>,
    pub image: PathBuf,
    pub disk: String,
    pub scheme: PartitionScheme,
    pub uefi_dir: PathBuf,
    pub dry_run: bool,
    pub auto_unmount: bool,
    pub locale: Option<LocaleConfig>,
    pub early_ssh: bool,
    pub progress_tx: Option<Sender<ProgressUpdate>>,
    pub efi_size: String,
    pub boot_size: String,
    pub root_end: String,
}

impl FlashConfig {
    pub fn validate(&self) -> Result<()> {
        if !self.image.exists() {
            bail!("Image file not found: {}", self.image.display());
        }
        if !self.uefi_dir.exists() {
            bail!("UEFI directory not found: {}", self.uefi_dir.display());
        }

        // Check for required UEFI file. Allow either:
        // - a directory containing RPI_EFI.fd (bundle), or
        // - a direct path to an EFI image file (will be staged into a temp dir at runtime)
        if self.uefi_dir.is_dir() {
            let rpi_efi = self.uefi_dir.join("RPI_EFI.fd");
            if !rpi_efi.exists() {
                bail!("Missing required UEFI file: {}", rpi_efi.display());
            }
        } else if self.uefi_dir.is_file() {
            // File path is accepted here; it will be normalized in `run_with_progress`.
        } else {
            bail!(
                "UEFI path is neither a file nor directory: {}",
                self.uefi_dir.display()
            );
        }

        let disk = normalize_disk(&self.disk);
        if !Path::new(&disk).exists() {
            bail!("Disk device not found: {}", disk);
        }

        Ok(())
    }
}

impl ValidateConfig for FlashConfig {
    fn validate_cfg(&self) -> Result<()> {
        self.validate()
    }
}

impl HasRunMode for FlashConfig {
    fn is_dry_run(&self) -> bool {
        self.dry_run
    }
}

/// Simple run function for CLI compatibility
pub fn run(
    image: &Path,
    disk: &str,
    scheme: PartitionScheme,
    uefi_dir: &Path,
    dry_run: bool,
    auto_unmount: bool,
    yes_i_know: bool,
    locale: Option<LocaleConfig>,
    early_ssh: bool,
    efi_size: &str,
    boot_size: &str,
    root_end: &str,
) -> Result<()> {
    // Create a temporary FlashConfig for CLI run
    let cli_flash_config = FlashConfig {
        os_distro: Some("Fedora".to_string()),
        os_flavour: None,
        disk_identity: None,
        efi_source: None,
        image: image.to_path_buf(),
        disk: disk.to_string(),
        scheme,
        uefi_dir: uefi_dir.to_path_buf(),
        dry_run,
        auto_unmount,
        locale,
        early_ssh,
        progress_tx: None, // No progress reporting for simple CLI run
        efi_size: efi_size.to_string(),
        boot_size: boot_size.to_string(),
        root_end: root_end.to_string(),
    };

    run_with_progress(
        &cli_flash_config,
        yes_i_know, // yes_i_know is still a separate parameter for safety
    )
}

/// Full run function with progress reporting
pub fn run_with_progress(
    config: &FlashConfig,
    yes_i_know: bool, // Still required separately for explicit confirmation
) -> Result<()> {
    run_with_progress_with_confirmation(config, yes_i_know, false)
}

/// Full run function with progress reporting + explicit typed confirmation (WO-036).
pub fn run_with_progress_with_confirmation(
    config: &FlashConfig,
    yes_i_know: bool,
    typed_confirmation: bool,
) -> Result<()> {
    let hal: Arc<dyn InstallerHal> = Arc::new(LinuxHal::new());
    run_with_progress_with_confirmation_with_hal(config, yes_i_know, typed_confirmation, hal)
}

/// Full run function with progress reporting + explicit typed confirmation (WO-036),
/// using the provided HAL implementation.
pub fn run_with_progress_with_confirmation_with_hal(
    config: &FlashConfig,
    yes_i_know: bool,
    typed_confirmation: bool,
    hal: Arc<dyn InstallerHal>,
) -> Result<()> {
    let validated = UnvalidatedConfig::new(config.clone()).validate()?;

    if validated.0.dry_run {
        return run_full_loop_dry_run(validated, hal);
    }

    // CLI compatibility: safe-mode disarm is a TUI concept; we treat `yes_i_know` as the caller's
    // explicit arming signal for non-TUI contexts.
    let token = ExecuteArmToken::try_new(yes_i_know, yes_i_know, typed_confirmation)?;
    let armed = validated.arm_execute(token)?;
    run_full_loop_execute(armed, hal)
}

fn run_full_loop_dry_run(
    validated: ValidatedConfig<FlashConfig>,
    hal: Arc<dyn InstallerHal>,
) -> Result<()> {
    validated.require_dry_run()?;
    run_full_loop_from_config(validated.0, hal, RunMode::DryRun, false, false, None)
}

fn run_full_loop_execute(
    armed: ArmedConfig<FlashConfig>,
    hal: Arc<dyn InstallerHal>,
) -> Result<()> {
    let executing = armed.into_executing();
    run_full_loop_from_config(
        executing.cfg,
        hal,
        RunMode::Execute,
        true,
        true,
        Some(executing.token),
    )
}

/// Execute a validated flash config in dry-run mode. This is used by the TUI to enforce the
/// validation -> (optional) arming type-state pipeline at the call site.
pub fn run_dry_run_with_hal(
    validated: ValidatedConfig<FlashConfig>,
    hal: Arc<dyn InstallerHal>,
) -> Result<()> {
    run_full_loop_dry_run(validated, hal)
}

/// Execute an explicitly-armed flash config in execute mode. This is used by the TUI to enforce
/// the validation -> arming type-state pipeline at the call site.
pub fn run_execute_with_hal(
    armed: ArmedConfig<FlashConfig>,
    hal: Arc<dyn InstallerHal>,
) -> Result<()> {
    run_full_loop_execute(armed, hal)
}

fn run_full_loop_from_config(
    config: FlashConfig,
    hal: Arc<dyn InstallerHal>,
    mode: RunMode,
    report_yes_i_know: bool,
    report_typed_confirmation: bool,
    token: Option<ExecuteArmToken>,
) -> Result<()> {
    info!("🍠 MASH Full-Loop Installer: Fedora KDE + UEFI Boot for RPi4");
    info!("📋 GPT layout with 4 partitions (EFI, BOOT, ROOT/btrfs, DATA)");

    let disk = normalize_disk(&config.disk);
    info!("💾 Target disk: {}", disk);
    info!("📀 Image: {}", config.image.display());
    info!("🔧 UEFI dir: {}", config.uefi_dir.display());
    info!("📏 EFI size: {}", config.efi_size);
    info!("📏 BOOT size: {}", config.boot_size);
    info!("📏 ROOT end: {}", config.root_end);

    show_lsblk(&*hal, &disk)?;

    // Persistent install report artifact (always).
    let selection = SelectionReport {
        distro: config
            .os_distro
            .clone()
            .unwrap_or_else(|| "Fedora".to_string()),
        flavour: config.os_flavour.clone(),
        target_disk: disk.clone(),
        disk_identity: config.disk_identity.clone(),
        partition_scheme: Some(format!("{}", config.scheme)),
        efi_size: Some(config.efi_size.clone()),
        boot_size: Some(config.boot_size.clone()),
        root_end: Some(config.root_end.clone()),
        efi_source: config.efi_source.clone(),
        efi_path: Some(config.uefi_dir.display().to_string()),
    };
    let report = InstallReportWriter::new(
        mode,
        report_yes_i_know,
        report_typed_confirmation,
        selection,
    )
    .ok();

    // Create work directory
    let work_dir = PathBuf::from("/tmp/mash-install");
    if work_dir.exists() {
        fs::remove_dir_all(&work_dir)?;
    }
    fs::create_dir_all(&work_dir)?;

    // Normalize UEFI input into a directory suitable for VFAT-safe rsync.
    // If a file is provided, stage it into a temp dir as RPI_EFI.fd.
    let effective_uefi_dir = if config.uefi_dir.is_file() {
        let staged = work_dir.join("uefi");
        fs::create_dir_all(&staged)?;
        fs::copy(&config.uefi_dir, staged.join("RPI_EFI.fd")).with_context(|| {
            format!(
                "Failed to stage EFI image {} into {}",
                config.uefi_dir.display(),
                staged.display()
            )
        })?;
        staged
    } else {
        config.uefi_dir.clone()
    };

    let mut ctx = FlashContext {
        hal,
        image: config.image.clone(),
        disk: disk.clone(),
        scheme: config.scheme,
        uefi_dir: effective_uefi_dir,
        dry_run: config.dry_run,
        auto_unmount: config.auto_unmount,
        locale: config.locale.clone(),
        early_ssh: config.early_ssh,
        progress_tx: config.progress_tx.clone(),
        work_dir: work_dir.clone(),
        loop_device: None,
        efi_size: config.efi_size.clone(),
        boot_size: config.boot_size.clone(),
        root_end: config.root_end.clone(),
        report,
    };

    // If the image is an .xz file, decompress it
    if ctx.image.extension().is_some_and(|ext| ext == "xz") {
        let decompressed_image = decompress_xz_image(&ctx, &ctx.image)?;
        ctx.image = decompressed_image;
    }

    let result = match token {
        Some(t) => run_installation_execute(&mut ctx, t),
        None => run_installation_dry_run(&mut ctx),
    };
    cleanup(&ctx);
    let _ = fs::remove_dir_all(&work_dir);
    result
}

/// Main installation sequence
fn run_installation_dry_run(ctx: &mut FlashContext) -> Result<()> {
    let mounts = MountPoints::new(&ctx.work_dir);

    info!("🧪 DRY-RUN MODE - No changes will be made");
    ctx.check_cancel()?;
    simulate_installation(ctx)?;
    drop(mounts);
    Ok(())
}

fn run_installation_execute(ctx: &mut FlashContext, token: ExecuteArmToken) -> Result<()> {
    let mounts = MountPoints::new(&ctx.work_dir);

    prepare_mount_points(ctx, &mounts)?;

    execute_partition_phase(ctx, &token)?;
    execute_format_phase(ctx, &token)?;

    setup_image_loop_phase(ctx)?;
    let subvols = mount_source_phase(ctx, &mounts)?;
    mount_dest_phase(ctx, &mounts)?;
    create_dest_subvols_phase(ctx, &mounts, &subvols)?;

    execute_copy_root_phase(ctx, &mounts, &subvols)?;
    execute_copy_boot_phase(ctx, &mounts)?;
    execute_copy_efi_phase(ctx, &mounts)?;

    execute_uefi_config_phase(ctx, &mounts, &subvols)?;
    execute_locale_phase(ctx, &mounts.dst_root_subvol)?;
    execute_fstab_phase(ctx, &mounts.dst_root_subvol, &subvols)?;
    execute_stage_dojo_phase(ctx, &mounts.dst_data, &mounts.dst_root_subvol)?;
    execute_cleanup_phase(ctx)?;

    ctx.send_progress(ProgressUpdate::Complete);
    info!("🎉 Installation complete!");
    Ok(())
}

fn prepare_mount_points(ctx: &FlashContext, mounts: &MountPoints) -> Result<()> {
    ctx.check_cancel()?;
    mounts.create_all()?;
    Ok(())
}

fn execute_partition_phase(ctx: &FlashContext, token: &ExecuteArmToken) -> Result<()> {
    ctx.check_cancel()?;
    ctx.start_phase(Phase::Partition);
    let _ = token;
    unmount_disk_partitions(&*ctx.hal, &ctx.disk, ctx.auto_unmount, ctx.dry_run)?;
    match ctx.scheme {
        PartitionScheme::Mbr => partition_disk_mbr(ctx, token)?,
        PartitionScheme::Gpt => partition_disk_gpt(ctx, token)?,
    };
    ctx.complete_phase(Phase::Partition);
    Ok(())
}

fn execute_format_phase(ctx: &FlashContext, token: &ExecuteArmToken) -> Result<()> {
    ctx.check_cancel()?;
    ctx.start_phase(Phase::Format);
    format_partitions(ctx, token)?;
    ctx.complete_phase(Phase::Format);
    Ok(())
}

fn setup_image_loop_phase(ctx: &mut FlashContext) -> Result<()> {
    ctx.check_cancel()?;
    ctx.status("🔄 Setting up image loop device...");
    setup_image_loop(ctx)?;
    Ok(())
}

fn mount_source_phase(ctx: &FlashContext, mounts: &MountPoints) -> Result<BtrfsSubvols> {
    ctx.check_cancel()?;
    ctx.status("📂 Mounting image partitions...");
    mount_source_partitions(ctx, mounts)
}

fn mount_dest_phase(ctx: &FlashContext, mounts: &MountPoints) -> Result<()> {
    ctx.check_cancel()?;
    ctx.status("📂 Mounting target partitions...");
    mount_dest_partitions(ctx, mounts)
}

fn create_dest_subvols_phase(
    ctx: &FlashContext,
    mounts: &MountPoints,
    subvols: &BtrfsSubvols,
) -> Result<()> {
    ctx.check_cancel()?;
    ctx.status("🌳 Creating btrfs subvolumes...");
    create_dest_subvols(ctx, mounts, subvols)
}

fn execute_copy_root_phase(
    ctx: &FlashContext,
    mounts: &MountPoints,
    subvols: &BtrfsSubvols,
) -> Result<()> {
    ctx.check_cancel()?;
    ctx.start_phase(Phase::CopyRoot);
    rsync_with_progress(
        ctx,
        &mounts.src_root_subvol,
        &mounts.dst_root_subvol,
        "root subvol",
    )?;
    if subvols.has_home {
        rsync_with_progress(
            ctx,
            &mounts.src_home_subvol,
            &mounts.dst_home_subvol,
            "home subvol",
        )?;
    }
    if subvols.has_var {
        rsync_with_progress(
            ctx,
            &mounts.src_var_subvol,
            &mounts.dst_var_subvol,
            "var subvol",
        )?;
    }
    ctx.complete_phase(Phase::CopyRoot);
    Ok(())
}

fn execute_copy_boot_phase(ctx: &FlashContext, mounts: &MountPoints) -> Result<()> {
    ctx.check_cancel()?;
    ctx.start_phase(Phase::CopyBoot);
    rsync_with_progress(ctx, &mounts.src_boot, &mounts.dst_boot, "boot")?;
    ctx.complete_phase(Phase::CopyBoot);
    Ok(())
}

fn execute_copy_efi_phase(ctx: &FlashContext, mounts: &MountPoints) -> Result<()> {
    ctx.check_cancel()?;
    ctx.start_phase(Phase::CopyEfi);
    // Copy Fedora EFI tree (safe for vfat)
    rsync_vfat_safe(
        ctx,
        &mounts.src_efi.join("EFI"),
        &mounts.dst_efi.join("EFI"),
    )?;
    // Copy UEFI firmware (LAST - overwrites any conflicts)
    rsync_vfat_safe(ctx, &ctx.uefi_dir, &mounts.dst_efi)?;
    // Write config.txt
    write_config_txt(&mounts.dst_efi)?;
    ctx.complete_phase(Phase::CopyEfi);
    Ok(())
}

fn execute_uefi_config_phase(
    ctx: &FlashContext,
    mounts: &MountPoints,
    subvols: &BtrfsSubvols,
) -> Result<()> {
    ctx.check_cancel()?;
    ctx.start_phase(Phase::UefiConfig);
    configure_boot(ctx, mounts, subvols)?;
    ctx.complete_phase(Phase::UefiConfig);
    Ok(())
}

fn execute_locale_phase(ctx: &FlashContext, target_root: &Path) -> Result<()> {
    ctx.check_cancel()?;
    ctx.start_phase(Phase::LocaleConfig);
    configure_locale(ctx, target_root)?;
    if ctx.early_ssh {
        enable_early_ssh(target_root)?;
    }
    enable_first_boot_setup(ctx, target_root)?;
    disable_autologin(ctx, target_root)?;
    ctx.complete_phase(Phase::LocaleConfig);
    Ok(())
}

fn execute_fstab_phase(
    ctx: &FlashContext,
    target_root: &Path,
    subvols: &BtrfsSubvols,
) -> Result<()> {
    ctx.check_cancel()?;
    ctx.start_phase(Phase::Fstab);
    generate_fstab(ctx, target_root, subvols)?;
    ctx.complete_phase(Phase::Fstab);
    Ok(())
}

fn execute_stage_dojo_phase(
    ctx: &FlashContext,
    data_mount: &Path,
    target_root: &Path,
) -> Result<()> {
    ctx.check_cancel()?;
    ctx.start_phase(Phase::StageDojo);
    stage_dojo(ctx, data_mount, target_root)?;
    ctx.complete_phase(Phase::StageDojo);
    Ok(())
}

fn execute_cleanup_phase(ctx: &FlashContext) -> Result<()> {
    ctx.check_cancel()?;
    ctx.start_phase(Phase::Cleanup);
    ctx.status("💾 Syncing filesystems...");
    let _ = ctx.hal.sync();
    ctx.complete_phase(Phase::Cleanup);
    Ok(())
}

fn simulate_installation(ctx: &FlashContext) -> Result<()> {
    // DRY-RUN must never look destructive. We avoid disk-op wording and mark phases as skipped.
    for phase in Phase::all() {
        ctx.check_cancel()?;
        ctx.send_progress(ProgressUpdate::PhaseSkipped(*phase));
        ctx.status(&format!(
            "DRY-RUN: would run phase {}/{}",
            phase.number(),
            Phase::total()
        ));
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    ctx.send_progress(ProgressUpdate::Complete);
    Ok(())
}

// ============================================================================
// Partition and Format Functions (GPT with parted)
// ============================================================================

fn unmount_disk_partitions(
    hal: &dyn InstallerHal,
    disk: &str,
    auto_unmount: bool,
    dry_run: bool,
) -> Result<()> {
    info!("🔍 Checking for mounted partitions on {}...", disk);
    let mountpoints = hal.lsblk_mountpoints(Path::new(disk))?;
    for mp in mountpoints {
        if auto_unmount {
            info!("🔌 Unmounting {}", mp.display());
            let _ = hal.unmount_recursive(&mp, dry_run);
        } else {
            bail!(
                "Partition mounted at {}. Use --auto-unmount or unmount manually.",
                mp.display()
            );
        }
    }
    Ok(())
}

/// Partition disk with MBR (msdos) using parted
fn partition_disk_mbr(ctx: &FlashContext, _token: &ExecuteArmToken) -> Result<()> {
    ctx.status("🔪 Creating MBR (msdos) partition table with parted...");

    // Wipe existing
    ctx.hal
        .wipefs_all(Path::new(&ctx.disk), &WipeFsOptions::new(ctx.dry_run, true))?;
    let _ = ctx.hal.udev_settle();

    let efi_start = "4MiB";
    let efi_end = ctx.efi_size.clone();
    let boot_end = format!(
        "{}MiB",
        parse_size_to_mib(&ctx.efi_size)? + parse_size_to_mib(&ctx.boot_size)?
    );

    // Create msdos partition table
    ctx.hal.parted(
        Path::new(&ctx.disk),
        PartedOp::MkLabel {
            label: "msdos".to_string(),
        },
        &PartedOptions::new(ctx.dry_run, true),
    )?;

    // p1: EFI (fat32) — mark bootable for broad Pi UEFI compatibility
    ctx.hal.parted(
        Path::new(&ctx.disk),
        PartedOp::MkPart {
            part_type: "primary".to_string(),
            fs_type: "fat32".to_string(),
            start: efi_start.to_string(),
            end: efi_end.clone(),
        },
        &PartedOptions::new(ctx.dry_run, true),
    )?;
    // On msdos, "esp" isn't always supported; boot flag is the reliable choice.
    let _ = ctx.hal.parted(
        Path::new(&ctx.disk),
        PartedOp::SetFlag {
            part_num: 1,
            flag: "boot".to_string(),
            state: "on".to_string(),
        },
        &PartedOptions::new(ctx.dry_run, true),
    );

    // p2: BOOT (ext4)
    ctx.hal.parted(
        Path::new(&ctx.disk),
        PartedOp::MkPart {
            part_type: "primary".to_string(),
            fs_type: "ext4".to_string(),
            start: efi_end.clone(),
            end: boot_end.clone(),
        },
        &PartedOptions::new(ctx.dry_run, true),
    )?;

    // p3: ROOT (btrfs) — keep filesystem consistent with pipeline; only the table differs.
    ctx.hal.parted(
        Path::new(&ctx.disk),
        PartedOp::MkPart {
            part_type: "primary".to_string(),
            fs_type: "btrfs".to_string(),
            start: boot_end.clone(),
            end: ctx.root_end.clone(),
        },
        &PartedOptions::new(ctx.dry_run, true),
    )?;

    // p4: DATA (btrfs)
    ctx.hal.parted(
        Path::new(&ctx.disk),
        PartedOp::MkPart {
            part_type: "primary".to_string(),
            fs_type: "btrfs".to_string(),
            start: ctx.root_end.clone(),
            end: "100%".to_string(),
        },
        &PartedOptions::new(ctx.dry_run, true),
    )?;

    let _ = ctx.hal.parted(
        Path::new(&ctx.disk),
        PartedOp::Print,
        &PartedOptions::new(ctx.dry_run, true),
    )?;
    let _ = ctx.hal.udev_settle();

    info!("📋 MBR partition table created (4 partitions)");
    Ok(())
}

fn partition_disk_gpt(ctx: &FlashContext, _token: &ExecuteArmToken) -> Result<()> {
    ctx.status("🔪 Creating GPT partition table with parted...");

    // Wipe existing
    ctx.hal
        .wipefs_all(Path::new(&ctx.disk), &WipeFsOptions::new(ctx.dry_run, true))?;
    let _ = ctx.hal.udev_settle();

    // Calculate partition boundaries
    let efi_start = "4MiB";
    let efi_end = ctx.efi_size.clone(); // Use value from context
    let boot_end = format!(
        "{}MiB",
        parse_size_to_mib(&ctx.efi_size)? + parse_size_to_mib(&ctx.boot_size)?
    ); // Calculate based on ctx values

    // Create GPT partition table
    ctx.hal.parted(
        Path::new(&ctx.disk),
        PartedOp::MkLabel {
            label: "gpt".to_string(),
        },
        &PartedOptions::new(ctx.dry_run, true),
    )?;

    // p1: EFI (fat32) with esp flag
    ctx.hal.parted(
        Path::new(&ctx.disk),
        PartedOp::MkPart {
            part_type: "primary".to_string(),
            fs_type: "fat32".to_string(),
            start: efi_start.to_string(),
            end: efi_end.clone(),
        },
        &PartedOptions::new(ctx.dry_run, true),
    )?;
    ctx.hal.parted(
        Path::new(&ctx.disk),
        PartedOp::SetFlag {
            part_num: 1,
            flag: "esp".to_string(),
            state: "on".to_string(),
        },
        &PartedOptions::new(ctx.dry_run, true),
    )?;

    // p2: BOOT (ext4)
    ctx.hal.parted(
        Path::new(&ctx.disk),
        PartedOp::MkPart {
            part_type: "primary".to_string(),
            fs_type: "ext4".to_string(),
            start: efi_end.clone(),
            end: boot_end.clone(),
        },
        &PartedOptions::new(ctx.dry_run, true),
    )?;

    // p3: ROOT (btrfs) - from boot_end to ROOT_END
    ctx.hal.parted(
        Path::new(&ctx.disk),
        PartedOp::MkPart {
            part_type: "primary".to_string(),
            fs_type: "btrfs".to_string(),
            start: boot_end.clone(),
            end: ctx.root_end.clone(),
        },
        &PartedOptions::new(ctx.dry_run, true),
    )?;

    // p4: DATA (btrfs) - from ROOT_END to 100%
    ctx.hal.parted(
        Path::new(&ctx.disk),
        PartedOp::MkPart {
            part_type: "primary".to_string(),
            fs_type: "btrfs".to_string(),
            start: ctx.root_end.clone(),
            end: "100%".to_string(),
        },
        &PartedOptions::new(ctx.dry_run, true),
    )?;

    // Show result
    let _ = ctx.hal.parted(
        Path::new(&ctx.disk),
        PartedOp::Print,
        &PartedOptions::new(ctx.dry_run, true),
    )?;
    let _ = ctx.hal.udev_settle();

    info!("📋 GPT partition table created (4 partitions)");
    Ok(())
}

fn format_partitions(ctx: &FlashContext, _token: &ExecuteArmToken) -> Result<()> {
    let p1 = ctx.partition_path(1);
    let p2 = ctx.partition_path(2);
    let p3 = ctx.partition_path(3);
    let p4 = ctx.partition_path(4);

    ctx.status("✨ Formatting EFI partition (FAT32)...");
    ctx.hal.format_vfat(
        Path::new(&p1),
        "EFI",
        &FormatOptions::new(ctx.dry_run, true),
    )?;

    ctx.status("✨ Formatting BOOT partition (ext4)...");
    ctx.hal.format_ext4(
        Path::new(&p2),
        &FormatOptions::new(ctx.dry_run, true).with_args(vec![
            "-F".to_string(),
            "-L".to_string(),
            "BOOT".to_string(),
        ]),
    )?;

    ctx.status("✨ Formatting ROOT partition (btrfs)...");
    ctx.hal.format_btrfs(
        Path::new(&p3),
        &FormatOptions::new(ctx.dry_run, true).with_args(vec![
            "-f".to_string(),
            "-L".to_string(),
            "FEDORA".to_string(),
        ]),
    )?;

    ctx.status("✨ Formatting DATA partition (btrfs)...");
    ctx.hal.format_btrfs(
        Path::new(&p4),
        &FormatOptions::new(ctx.dry_run, true).with_args(vec![
            "-f".to_string(),
            "-L".to_string(),
            "DATA".to_string(),
        ]),
    )?;

    let _ = ctx.hal.udev_settle();
    Ok(())
}

// ============================================================================
// Loop Device and Mount Functions
// ============================================================================

fn setup_image_loop(ctx: &mut FlashContext) -> Result<()> {
    let loop_dev = ctx
        .hal
        .losetup_attach(&ctx.image, true)
        .map_err(anyhow::Error::new)?;
    info!("🔄 Image mounted at loop device: {}", loop_dev);
    ctx.loop_device = Some(loop_dev);

    // Wait for kernel to surface loop partitions (best-effort).
    let _ = ctx.hal.udev_settle();
    Ok(())
}

fn mount_source_partitions(ctx: &FlashContext, mounts: &MountPoints) -> Result<BtrfsSubvols> {
    let loop_dev = ctx
        .loop_device
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Loop device not set"))?;

    let img_efi = format!("{}p1", loop_dev);
    let img_boot = format!("{}p2", loop_dev);
    let img_root = format!("{}p3", loop_dev);

    // Mount EFI and boot
    ctx.hal.mount_device(
        Path::new(&img_efi),
        &mounts.src_efi,
        None,
        MountOptions::new(),
        ctx.dry_run,
    )?;
    ctx.hal.mount_device(
        Path::new(&img_boot),
        &mounts.src_boot,
        None,
        MountOptions::new(),
        ctx.dry_run,
    )?;

    // Mount btrfs root (top-level first to detect subvols)
    ctx.hal.mount_device(
        Path::new(&img_root),
        &mounts.src_root_top,
        Some("btrfs"),
        MountOptions::new(),
        ctx.dry_run,
    )?;

    // Detect subvolumes
    let subvol_list = ctx.hal.btrfs_subvolume_list(&mounts.src_root_top)?;

    let has_root = subvol_list.contains(" path root");
    let has_home = subvol_list.contains(" path home");
    let has_var = subvol_list.contains(" path var");

    if !has_root {
        bail!("Image does not contain btrfs subvol 'root' (unexpected for Fedora RAW)");
    }

    info!(
        "🌳 Detected subvols: root={}, home={}, var={}",
        has_root, has_home, has_var
    );

    // Mount subvolumes
    ctx.hal.mount_device(
        Path::new(&img_root),
        &mounts.src_root_subvol,
        Some("btrfs"),
        MountOptions::with_options("subvol=root"),
        ctx.dry_run,
    )?;
    if has_home {
        ctx.hal.mount_device(
            Path::new(&img_root),
            &mounts.src_home_subvol,
            Some("btrfs"),
            MountOptions::with_options("subvol=home"),
            ctx.dry_run,
        )?;
    }
    if has_var {
        ctx.hal.mount_device(
            Path::new(&img_root),
            &mounts.src_var_subvol,
            Some("btrfs"),
            MountOptions::with_options("subvol=var"),
            ctx.dry_run,
        )?;
    }

    Ok(BtrfsSubvols {
        has_root,
        has_home,
        has_var,
    })
}

fn mount_dest_partitions(ctx: &FlashContext, mounts: &MountPoints) -> Result<()> {
    let p1 = ctx.partition_path(1);
    let p2 = ctx.partition_path(2);
    let p3 = ctx.partition_path(3);
    let p4 = ctx.partition_path(4);

    ctx.hal.mount_device(
        Path::new(&p1),
        &mounts.dst_efi,
        None,
        MountOptions::new(),
        ctx.dry_run,
    )?;
    ctx.hal.mount_device(
        Path::new(&p2),
        &mounts.dst_boot,
        None,
        MountOptions::new(),
        ctx.dry_run,
    )?;
    ctx.hal.mount_device(
        Path::new(&p4),
        &mounts.dst_data,
        None,
        MountOptions::new(),
        ctx.dry_run,
    )?;
    ctx.hal.mount_device(
        Path::new(&p3),
        &mounts.dst_root_top,
        Some("btrfs"),
        MountOptions::new(),
        ctx.dry_run,
    )?;

    Ok(())
}

fn create_dest_subvols(
    ctx: &FlashContext,
    mounts: &MountPoints,
    subvols: &BtrfsSubvols,
) -> Result<()> {
    let p3 = ctx.partition_path(3);

    // Create subvolumes
    ctx.hal
        .btrfs_subvolume_create(&mounts.dst_root_top.join("root"))?;
    if subvols.has_home {
        ctx.hal
            .btrfs_subvolume_create(&mounts.dst_root_top.join("home"))?;
    }
    if subvols.has_var {
        ctx.hal
            .btrfs_subvolume_create(&mounts.dst_root_top.join("var"))?;
    }

    // Mount subvolumes for copying
    ctx.hal.mount_device(
        Path::new(&p3),
        &mounts.dst_root_subvol,
        Some("btrfs"),
        MountOptions::with_options("subvol=root"),
        ctx.dry_run,
    )?;
    if subvols.has_home {
        ctx.hal.mount_device(
            Path::new(&p3),
            &mounts.dst_home_subvol,
            Some("btrfs"),
            MountOptions::with_options("subvol=home"),
            ctx.dry_run,
        )?;
    }
    if subvols.has_var {
        ctx.hal.mount_device(
            Path::new(&p3),
            &mounts.dst_var_subvol,
            Some("btrfs"),
            MountOptions::with_options("subvol=var"),
            ctx.dry_run,
        )?;
    }

    Ok(())
}

// ============================================================================
// Copy Functions
// ============================================================================

fn rsync_with_progress(ctx: &FlashContext, src: &Path, dst: &Path, label: &str) -> Result<()> {
    ctx.status(&format!("📦 Copying {}...", label));

    let mut on_line = |line: &str| -> bool {
        if cancel_requested() {
            return false;
        }
        if let Some(progress) = parse_rsync_progress(line) {
            ctx.send_progress(ProgressUpdate::RsyncProgress {
                percent: progress.percent,
                speed_mbps: progress.speed_mbps,
                files_done: progress.files_done,
                files_total: progress.files_total,
            });
        }
        true
    };

    ctx.hal
        .rsync_stream_stdout(src, dst, &RsyncOptions::progress2_archive(), &mut on_line)
        .map_err(anyhow::Error::new)
        .with_context(|| format!("rsync failed for {}", label))?;
    Ok(())
}

static CANCEL_FLAG: OnceLock<Mutex<Option<Arc<AtomicBool>>>> = OnceLock::new();

pub fn set_cancel_flag(flag: Arc<AtomicBool>) {
    let lock = CANCEL_FLAG.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = lock.lock() {
        *guard = Some(flag);
    }
}

pub fn clear_cancel_flag() {
    if let Some(lock) = CANCEL_FLAG.get() {
        if let Ok(mut guard) = lock.lock() {
            *guard = None;
        }
    }
}

fn cancel_requested() -> bool {
    CANCEL_FLAG
        .get()
        .and_then(|lock| lock.lock().ok())
        .and_then(|guard| guard.as_ref().map(|flag| flag.load(Ordering::Relaxed)))
        .unwrap_or(false)
}

/// VFAT-safe rsync (no ownership/permissions)
fn rsync_vfat_safe(ctx: &FlashContext, src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    let mut on_line = |_line: &str| -> bool { true };
    ctx.hal
        .rsync_stream_stdout(src, dst, &RsyncOptions::vfat_safe(), &mut on_line)
        .map_err(anyhow::Error::new)
}

struct RsyncProgress {
    percent: f64,
    speed_mbps: f64,
    files_done: u64,
    files_total: u64,
}

fn parse_rsync_progress(line: &str) -> Option<RsyncProgress> {
    let percent_idx = line.find('%')?;
    let percent_start = line[..percent_idx]
        .rfind(char::is_whitespace)
        .map(|i| i + 1)
        .unwrap_or(0);
    let percent: f64 = line[percent_start..percent_idx].trim().parse().ok()?;

    let speed_mbps = if let Some(speed_end) = line.find("/s") {
        let speed_part = &line[..speed_end];
        let speed_start = speed_part.rfind(char::is_whitespace).unwrap_or(0);
        let speed_str = speed_part[speed_start..].trim();
        let (value, mult) = if speed_str.ends_with("GB") {
            (speed_str.trim_end_matches("GB"), 1024.0)
        } else if speed_str.ends_with("MB") {
            (speed_str.trim_end_matches("MB"), 1.0)
        } else if speed_str.ends_with("kB") {
            (speed_str.trim_end_matches("kB"), 0.001)
        } else {
            (speed_str, 0.000001)
        };
        value.parse::<f64>().unwrap_or(0.0) * mult
    } else {
        0.0
    };

    let (files_done, files_total) = if let Some(xfr_start) = line.find("xfr#") {
        let xfr_end = line[xfr_start..].find(',').map(|i| i + xfr_start)?;
        let done: u64 = line[xfr_start + 4..xfr_end].parse().ok()?;
        if let Some(chk_start) = line.find("to-chk=") {
            let chk_part = &line[chk_start + 7..];
            let slash = chk_part.find('/')?;
            let paren = chk_part.find(')')?;
            let total: u64 = chk_part[slash + 1..paren].parse().ok()?;
            (done, total)
        } else {
            (done, done)
        }
    } else {
        (0, 0)
    };

    Some(RsyncProgress {
        percent,
        speed_mbps,
        files_done,
        files_total,
    })
}

// ============================================================================
// Configuration Functions
// ============================================================================

fn write_config_txt(efi_mount: &Path) -> Result<()> {
    let config = r#"# Pi4 UEFI (PFTF) boot config for Fedora on USB (GPT, 4-part)
arm_64bit=1
enable_uart=1
enable_gic=1
armstub=RPI_EFI.fd
disable_commandline_tags=2

[pi4]
dtoverlay=upstream-pi4

[all]
"#;
    fs::write(efi_mount.join("config.txt"), config)?;
    Ok(())
}

fn configure_boot(ctx: &FlashContext, mounts: &MountPoints, _subvols: &BtrfsSubvols) -> Result<()> {
    let boot_uuid = get_partition_uuid(ctx, Path::new(&ctx.partition_path(2)))?;
    let root_uuid = get_partition_uuid(ctx, Path::new(&ctx.partition_path(3)))?;

    // Write GRUB stub on EFI -> points to /boot UUID
    ctx.status("📝 Writing GRUB stub...");
    let grub_dir = mounts.dst_efi.join("EFI/fedora");
    fs::create_dir_all(&grub_dir)?;
    let grub_stub = format!(
        "search --no-floppy --fs-uuid --set=dev {}\nset prefix=($dev)/grub2\nconfigfile $prefix/grub.cfg\n",
        boot_uuid
    );
    fs::write(grub_dir.join("grub.cfg"), grub_stub)?;

    // Patch BLS entries
    ctx.status("🩹 Patching BLS boot entries...");
    patch_bls_entries(&mounts.dst_boot.join("loader/entries"), &root_uuid)?;

    Ok(())
}

fn patch_bls_entries(entries_dir: &Path, root_uuid: &str) -> Result<()> {
    if !entries_dir.exists() {
        info!("⚠️ No BLS entries found at {}", entries_dir.display());
        return Ok(());
    }

    let expected_options = format!(
        "options root=UUID={} rootflags=subvol=root rw rhgb quiet",
        root_uuid
    );

    for entry in fs::read_dir(entries_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().map(|e| e == "conf").unwrap_or(false) {
            let content = fs::read_to_string(&path)?;
            let new_content: String = content
                .lines()
                .map(|line| {
                    if line.starts_with("options ") {
                        expected_options.as_str()
                    } else {
                        line
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
                + "\n";
            fs::write(&path, new_content)?;
            info!("✅ Patched {}", path.display());
        }
    }
    Ok(())
}

fn configure_locale(ctx: &FlashContext, target_root: &Path) -> Result<()> {
    if let Some(ref locale) = ctx.locale {
        ctx.status(&format!(
            "🗣️ Configuring locale: {} (keymap: {})",
            locale.lang, locale.keymap
        ));
        crate::locale::patch_locale(target_root, locale, false)?;
    } else {
        ctx.status("🗣️ Using default locale settings");
    }
    Ok(())
}

fn enable_first_boot_setup(ctx: &FlashContext, target_root: &Path) -> Result<()> {
    ctx.status("🧑‍💻 Enabling first-boot user setup...");
    let sysconfig = target_root.join("etc/sysconfig/initial-setup");
    if sysconfig.exists() {
        let content = fs::read_to_string(&sysconfig)?;
        let mut out = Vec::new();
        let mut found = false;
        for line in content.lines() {
            if line.trim_start().starts_with("RUN_FIRSTBOOT=") {
                out.push("RUN_FIRSTBOOT=YES".to_string());
                found = true;
            } else {
                out.push(line.to_string());
            }
        }
        if !found {
            out.push("RUN_FIRSTBOOT=YES".to_string());
        }
        fs::write(&sysconfig, out.join("\n") + "\n")?;
    }

    enable_service(
        target_root,
        "initial-setup.service",
        "multi-user.target.wants",
    )?;
    enable_service(
        target_root,
        "initial-setup-graphical.service",
        "graphical.target.wants",
    )?;
    Ok(())
}

fn enable_service(target_root: &Path, service: &str, target_dir: &str) -> Result<()> {
    let unit = target_root.join("usr/lib/systemd/system").join(service);
    if !unit.exists() {
        return Ok(());
    }
    let wants_dir = target_root.join("etc/systemd/system").join(target_dir);
    fs::create_dir_all(&wants_dir)?;
    let link = wants_dir.join(service);
    if !link.exists() {
        std::os::unix::fs::symlink(format!("/usr/lib/systemd/system/{}", service), &link)?;
    }
    Ok(())
}

fn disable_autologin(ctx: &FlashContext, target_root: &Path) -> Result<()> {
    ctx.status("🛑 Disabling autologin...");
    disable_gdm_autologin(target_root)?;
    disable_sddm_autologin(target_root)?;
    disable_lightdm_autologin(target_root)?;
    Ok(())
}

fn disable_gdm_autologin(target_root: &Path) -> Result<()> {
    let path = target_root.join("etc/gdm/custom.conf");
    if !path.exists() {
        return Ok(());
    }
    let content = fs::read_to_string(&path)?;
    let mut out = Vec::new();
    for line in content.lines() {
        if line.trim_start().starts_with("AutomaticLoginEnable") {
            out.push("AutomaticLoginEnable=false".to_string());
        } else if line.trim_start().starts_with("AutomaticLogin=") {
            out.push(format!("#{}", line));
        } else {
            out.push(line.to_string());
        }
    }
    fs::write(path, out.join("\n") + "\n")?;
    Ok(())
}

fn disable_sddm_autologin(target_root: &Path) -> Result<()> {
    let mut files = Vec::new();
    let main = target_root.join("etc/sddm.conf");
    if main.exists() {
        files.push(main);
    }
    let dir = target_root.join("etc/sddm.conf.d");
    if dir.exists() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            if entry
                .path()
                .extension()
                .map(|e| e == "conf")
                .unwrap_or(false)
            {
                files.push(entry.path());
            }
        }
    }
    for path in files {
        let content = fs::read_to_string(&path)?;
        let mut out = Vec::new();
        let mut in_autologin = false;
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                in_autologin = trimmed.eq_ignore_ascii_case("[Autologin]");
                out.push(line.to_string());
                continue;
            }
            if in_autologin
                && (trimmed.starts_with("User=")
                    || trimmed.starts_with("Session=")
                    || trimmed.starts_with("Relogin="))
            {
                continue;
            }
            out.push(line.to_string());
        }
        fs::write(&path, out.join("\n") + "\n")?;
    }
    Ok(())
}

fn disable_lightdm_autologin(target_root: &Path) -> Result<()> {
    let mut files = Vec::new();
    let main = target_root.join("etc/lightdm/lightdm.conf");
    if main.exists() {
        files.push(main);
    }
    let dir = target_root.join("etc/lightdm/lightdm.conf.d");
    if dir.exists() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            if entry
                .path()
                .extension()
                .map(|e| e == "conf")
                .unwrap_or(false)
            {
                files.push(entry.path());
            }
        }
    }
    for path in files {
        let content = fs::read_to_string(&path)?;
        let mut out = Vec::new();
        for line in content.lines() {
            if line.trim_start().starts_with("autologin-user=") {
                out.push(format!("#{}", line));
            } else {
                out.push(line.to_string());
            }
        }
        fs::write(&path, out.join("\n") + "\n")?;
    }
    Ok(())
}

fn enable_early_ssh(target_root: &Path) -> Result<()> {
    info!("🔐 Enabling early SSH access...");
    let systemd_dir = target_root.join("etc/systemd/system/multi-user.target.wants");
    fs::create_dir_all(&systemd_dir)?;
    let sshd_link = systemd_dir.join("sshd.service");
    if !sshd_link.exists() {
        std::os::unix::fs::symlink("/usr/lib/systemd/system/sshd.service", &sshd_link)?;
    }
    Ok(())
}

fn generate_fstab(ctx: &FlashContext, target_root: &Path, subvols: &BtrfsSubvols) -> Result<()> {
    ctx.status("📋 Generating /etc/fstab...");

    let efi_uuid = get_partition_uuid(ctx, Path::new(&ctx.partition_path(1)))?;
    let boot_uuid = get_partition_uuid(ctx, Path::new(&ctx.partition_path(2)))?;
    let root_uuid = get_partition_uuid(ctx, Path::new(&ctx.partition_path(3)))?;
    let data_uuid = get_partition_uuid(ctx, Path::new(&ctx.partition_path(4)))?;

    let mut fstab = String::from("# /etc/fstab - Generated by MASH Installer\n");
    fstab.push_str(&format!(
        "UUID={}  /         btrfs  subvol=root,compress=zstd:1,defaults,noatime  0 0\n",
        root_uuid
    ));
    if subvols.has_home {
        fstab.push_str(&format!(
            "UUID={}  /home     btrfs  subvol=home,compress=zstd:1,defaults,noatime  0 0\n",
            root_uuid
        ));
    }
    if subvols.has_var {
        fstab.push_str(&format!(
            "UUID={}  /var      btrfs  subvol=var,compress=zstd:1,defaults,noatime   0 0\n",
            root_uuid
        ));
    }
    fstab.push_str(&format!(
        "UUID={}  /boot     ext4   defaults,noatime  0 2\n",
        boot_uuid
    ));
    fstab.push_str(&format!(
        "UUID={}   /boot/efi vfat   umask=0077,shortname=winnt  0 2\n",
        efi_uuid
    ));
    fstab.push_str(&format!(
        "UUID={}  /data     btrfs  defaults,noatime  0 0\n",
        data_uuid
    ));

    let fstab_path = target_root.join("etc/fstab");
    fs::create_dir_all(fstab_path.parent().unwrap())?;
    fs::write(&fstab_path, fstab)?;
    info!("📋 Written {}", fstab_path.display());
    Ok(())
}

fn get_partition_uuid(ctx: &FlashContext, device: &Path) -> Result<String> {
    ctx.hal
        .blkid_uuid(device)
        .map_err(anyhow::Error::new)
        .with_context(|| format!("Failed to get UUID for {}", device.display()))
}

fn stage_dojo(ctx: &FlashContext, data_mount: &Path, target_root: &Path) -> Result<()> {
    ctx.status("🥋 Staging Dojo installation files to DATA partition...");

    let staging_dir = data_mount.join("mash-staging");
    let logs_dir = data_mount.join("mash-logs");
    fs::create_dir_all(&staging_dir)?;
    fs::create_dir_all(&logs_dir)?;

    // Create install_dojo.sh
    let dojo_script = staging_dir.join("install_dojo.sh");
    let script_content = include_str!("dojo_install_template.sh");
    fs::write(&dojo_script, script_content.replace("{{PLACEHOLDER}}", ""))?;
    fs::set_permissions(&dojo_script, fs::Permissions::from_mode(0o755))?;

    stage_firstboot_dojo(ctx, target_root)?;

    info!("🥋 Dojo staging complete at {}", staging_dir.display());
    Ok(())
}

fn stage_firstboot_dojo(ctx: &FlashContext, target_root: &Path) -> Result<()> {
    ctx.status("🥋 Staging MASH Dojo to /usr/local/bin and /etc/systemd/system...");
    if ctx.dry_run {
        ctx.status("(dry-run) Would stage mash-dojo binary and service");
        return Ok(());
    }

    let dojo_bin = resolve_mash_dojo_binary()?;
    let bin_dir = target_root.join("usr/local/bin");
    fs::create_dir_all(&bin_dir)?;
    let target_bin = bin_dir.join("mash-dojo");
    fs::copy(&dojo_bin, &target_bin)?;
    fs::set_permissions(&target_bin, fs::Permissions::from_mode(0o755))?;

    let service_content = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../mash-installer/firstboot/dojo/mash-dojo.service"
    ));
    let service_dir = target_root.join("etc/systemd/system");
    fs::create_dir_all(&service_dir)?;
    let service_path = service_dir.join("mash-dojo.service");
    fs::write(&service_path, service_content)?;

    let wants_dir = service_dir.join("multi-user.target.wants");
    fs::create_dir_all(&wants_dir)?;
    let link = wants_dir.join("mash-dojo.service");
    if !link.exists() {
        std::os::unix::fs::symlink("/etc/systemd/system/mash-dojo.service", &link)?;
    }

    Ok(())
}

fn resolve_mash_dojo_binary() -> Result<PathBuf> {
    let exe = std::env::current_exe().context("Failed to locate running mash binary")?;
    let candidate = exe.with_file_name("mash-dojo");
    if candidate.exists() {
        return Ok(candidate);
    }
    bail!("mash-dojo binary not found next to {}", exe.display());
}

fn decompress_xz_image(ctx: &FlashContext, xz_image_path: &Path) -> Result<PathBuf> {
    ctx.status(&format!(
        "Decompressing XZ image: {}...",
        xz_image_path.display()
    ));

    let raw_image_path = xz_image_path.with_extension(""); // Remove .xz extension

    // Check if the raw image already exists
    if raw_image_path.exists() {
        ctx.status(&format!(
            "Raw image already exists: {}",
            raw_image_path.display()
        ));
        return Ok(raw_image_path);
    }

    let input = std::fs::File::open(xz_image_path)
        .with_context(|| format!("Failed to open xz image: {}", xz_image_path.display()))?;
    let mut decoder = xz2::read::XzDecoder::new(input);
    let mut output_file = std::fs::File::create(&raw_image_path).with_context(|| {
        format!(
            "Failed to create raw image file: {}",
            raw_image_path.display()
        )
    })?;

    std::io::copy(&mut decoder, &mut output_file).context("Failed to decompress xz image")?;

    ctx.status(&format!(
        "Decompression complete: {} -> {}",
        xz_image_path.display(),
        raw_image_path.display()
    ));
    Ok(raw_image_path)
}

// ============================================================================
// Helper Functions
// ============================================================================

// Helper to parse size strings like "1024MiB" or "2GiB" into MiB
fn parse_size_to_mib(s: &str) -> Result<u64> {
    let s_lower = s.to_ascii_lowercase();
    if s_lower.ends_with("mib") {
        s_lower
            .trim_end_matches("mib")
            .parse::<u64>()
            .map_err(|e| anyhow::anyhow!("Invalid MiB format: {} ({})", s, e))
    } else if s_lower.ends_with("gib") {
        s_lower
            .trim_end_matches("gib")
            .parse::<u64>()
            .map_err(|e| anyhow::anyhow!("Invalid GiB format: {} ({})", s, e))
            .and_then(|g| {
                g.checked_mul(1024)
                    .ok_or_else(|| anyhow::anyhow!("Size overflow for GiB: {}", s))
            })
    } else {
        bail!("Size must be like 1024MiB or 2GiB, got: {}", s)
    }
}

// ============================================================================
// Cleanup and Helper Functions
// ============================================================================

fn cleanup(ctx: &FlashContext) {
    info!("🧹 Cleaning up...");

    // Unmount everything (best effort, reverse order)
    let base = &ctx.work_dir;
    let mount_points = [
        base.join("dst/root_sub_var"),
        base.join("dst/root_sub_home"),
        base.join("dst/root_sub_root"),
        base.join("dst/root_top"),
        base.join("dst/data"),
        base.join("dst/boot"),
        base.join("dst/efi"),
        base.join("src/root_sub_var"),
        base.join("src/root_sub_home"),
        base.join("src/root_sub_root"),
        base.join("src/root_top"),
        base.join("src/boot"),
        base.join("src/efi"),
    ];

    for mp in &mount_points {
        if mp.exists() {
            let _ = ctx.hal.unmount_recursive(mp, false);
        }
    }

    if let Some(ref loop_dev) = ctx.loop_device {
        let _ = ctx.hal.losetup_detach(loop_dev);
    }

    let _ = ctx.hal.udev_settle();
}

fn normalize_disk(d: &str) -> String {
    if d.starts_with("/dev/") {
        d.to_string()
    } else {
        format!("/dev/{}", d)
    }
}

fn show_lsblk(hal: &dyn InstallerHal, disk: &str) -> Result<()> {
    info!("🧾 Current disk layout for {}", disk);
    let table = hal.lsblk_table(Path::new(disk))?;
    info!("\n{}", table);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mash_hal::FakeHal;

    #[test]
    fn test_normalize_disk() {
        assert_eq!(normalize_disk("sda"), "/dev/sda");
        assert_eq!(normalize_disk("/dev/sda"), "/dev/sda");
    }

    #[test]
    fn test_partition_path() {
        let ctx = FlashContext {
            hal: Arc::new(FakeHal::new()),
            image: PathBuf::new(),
            disk: "/dev/sda".to_string(),
            scheme: PartitionScheme::Mbr,
            uefi_dir: PathBuf::new(),
            dry_run: false,
            auto_unmount: false,
            locale: None,
            early_ssh: false,
            progress_tx: None,
            work_dir: PathBuf::new(),
            loop_device: None,
            efi_size: "512M".to_string(),
            boot_size: "1G".to_string(),
            root_end: "100%".to_string(),
            report: None,
        };
        assert_eq!(ctx.partition_path(1), "/dev/sda1");
        assert_eq!(ctx.partition_path(4), "/dev/sda4");
    }
}
