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
use log::{debug, info};
use std::fs;
use std::io::{BufRead, BufReader};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, OnceLock};

use crate::cli::PartitionScheme;
use crate::errors::MashError;
use crate::locale::LocaleConfig;
use crate::tui::progress::{Phase, ProgressUpdate};

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
}

impl FlashContext {
    fn send_progress(&self, update: ProgressUpdate) {
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
    pub(crate) fn validate(&self) -> Result<()> {
        if !self.image.exists() {
            bail!("Image file not found: {}", self.image.display());
        }
        if !self.uefi_dir.exists() {
            bail!("UEFI directory not found: {}", self.uefi_dir.display());
        }

        // Check for required UEFI files
        let rpi_efi = self.uefi_dir.join("RPI_EFI.fd");
        if !rpi_efi.exists() {
            bail!("Missing required UEFI file: {}", rpi_efi.display());
        }

        let disk = normalize_disk(&self.disk);
        if !Path::new(&disk).exists() {
            bail!("Disk device not found: {}", disk);
        }

        Ok(())
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
    info!("🍠 MASH Full-Loop Installer: Fedora KDE + UEFI Boot for RPi4");
    info!("📋 GPT layout with 4 partitions (EFI, BOOT, ROOT/btrfs, DATA)");

    config.validate()?;

    let disk = normalize_disk(&config.disk);
    info!("💾 Target disk: {}", disk);
    info!("📀 Image: {}", config.image.display());
    info!("🔧 UEFI dir: {}", config.uefi_dir.display());
    info!("📏 EFI size: {}", config.efi_size);
    info!("📏 BOOT size: {}", config.boot_size);
    info!("📏 ROOT end: {}", config.root_end);

    // Safety check
    if !yes_i_know && !config.dry_run {
        return Err(MashError::MissingYesIKnow.into());
    }

    show_lsblk(&disk)?;

    // Create work directory
    let work_dir = PathBuf::from("/tmp/mash-install");
    if work_dir.exists() {
        fs::remove_dir_all(&work_dir)?;
    }
    fs::create_dir_all(&work_dir)?;

    let mut ctx = FlashContext {
        image: config.image.clone(),
        disk: disk.clone(),
        scheme: config.scheme,
        uefi_dir: config.uefi_dir.clone(),
        dry_run: config.dry_run,
        auto_unmount: config.auto_unmount,
        locale: config.locale.clone(),
        early_ssh: config.early_ssh,
        progress_tx: config.progress_tx.clone(), // Use the progress_tx from config
        work_dir: work_dir.clone(),
        loop_device: None,
        efi_size: config.efi_size.clone(),
        boot_size: config.boot_size.clone(),
        root_end: config.root_end.clone(),
    };

    // If the image is an .xz file, decompress it
    if ctx.image.extension().is_some_and(|ext| ext == "xz") {
        let decompressed_image = decompress_xz_image(&ctx, &ctx.image)?;
        ctx.image = decompressed_image;
    }

    let result = run_installation(&mut ctx);
    cleanup(&ctx);
    let _ = fs::remove_dir_all(&work_dir);
    result
}

/// Main installation sequence
fn run_installation(ctx: &mut FlashContext) -> Result<()> {
    let mounts = MountPoints::new(&ctx.work_dir);

    if ctx.dry_run {
        info!("🧪 DRY-RUN MODE - No changes will be made");
        ctx.check_cancel()?;
        simulate_installation(ctx)?;
        return Ok(());
    }

    prepare_mount_points(ctx, &mounts)?;

    execute_partition_phase(ctx)?;
    execute_format_phase(ctx)?;

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

fn execute_partition_phase(ctx: &FlashContext) -> Result<()> {
    ctx.check_cancel()?;
    ctx.start_phase(Phase::Partition);
    unmount_disk_partitions(&ctx.disk, ctx.auto_unmount)?;
    match ctx.scheme {
        PartitionScheme::Mbr => partition_disk_mbr(ctx)?,
        PartitionScheme::Gpt => partition_disk_gpt(ctx)?,
    };
    ctx.complete_phase(Phase::Partition);
    Ok(())
}

fn execute_format_phase(ctx: &FlashContext) -> Result<()> {
    ctx.check_cancel()?;
    ctx.start_phase(Phase::Format);
    format_partitions(ctx)?;
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
    rsync_vfat_safe(&mounts.src_efi.join("EFI"), &mounts.dst_efi.join("EFI"))?;
    // Copy UEFI firmware (LAST - overwrites any conflicts)
    rsync_vfat_safe(&ctx.uefi_dir, &mounts.dst_efi)?;
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
    let _ = Command::new("sync").status();
    ctx.complete_phase(Phase::Cleanup);
    Ok(())
}

fn simulate_installation(ctx: &FlashContext) -> Result<()> {
    for phase in Phase::all() {
        ctx.check_cancel()?;
        if matches!(phase, Phase::DownloadImage | Phase::DownloadUefi) {
            ctx.send_progress(ProgressUpdate::PhaseSkipped(*phase));
            ctx.status(&format!("(dry-run) Skipping {}", phase.name()));
            continue;
        }
        ctx.start_phase(*phase);
        ctx.status(&format!("(dry-run) Would execute: {}", phase.name()));
        std::thread::sleep(std::time::Duration::from_millis(300));
        ctx.complete_phase(*phase);
    }
    ctx.send_progress(ProgressUpdate::Complete);
    Ok(())
}

// ============================================================================
// Partition and Format Functions (GPT with parted)
// ============================================================================

fn unmount_disk_partitions(disk: &str, auto_unmount: bool) -> Result<()> {
    info!("🔍 Checking for mounted partitions on {}...", disk);
    let output = Command::new("lsblk")
        .args(["-lnpo", "MOUNTPOINT", disk])
        .output()
        .context("Failed to run lsblk")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    for mp in stdout.lines().filter(|l| !l.is_empty()) {
        if auto_unmount {
            info!("🔌 Unmounting {}", mp);
            let _ = Command::new("umount").args(["-R", mp]).status();
        } else {
            bail!(
                "Partition mounted at {}. Use --auto-unmount or unmount manually.",
                mp
            );
        }
    }
    Ok(())
}

/// Partition disk with MBR (msdos) using parted
fn partition_disk_mbr(ctx: &FlashContext) -> Result<()> {
    ctx.status("🔪 Creating MBR (msdos) partition table with parted...");

    // Wipe existing
    run_command("wipefs", &["-a", &ctx.disk])?;
    udev_settle();

    let efi_start = "4MiB";
    let efi_end = ctx.efi_size.clone();
    let boot_end = format!(
        "{}MiB",
        parse_size_to_mib(&ctx.efi_size)? + parse_size_to_mib(&ctx.boot_size)?
    );

    // Create msdos partition table
    run_command("parted", &["-s", &ctx.disk, "mklabel", "msdos"])?;

    // p1: EFI (fat32) — mark bootable for broad Pi UEFI compatibility
    run_command(
        "parted",
        &[
            "-s", "-a", "optimal", &ctx.disk, "mkpart", "primary", "fat32", efi_start, &efi_end,
        ],
    )?;
    // On msdos, "esp" isn't always supported; boot flag is the reliable choice.
    let _ = run_command("parted", &["-s", &ctx.disk, "set", "1", "boot", "on"]);

    // p2: BOOT (ext4)
    run_command(
        "parted",
        &[
            "-s", "-a", "optimal", &ctx.disk, "mkpart", "primary", "ext4", &efi_end, &boot_end,
        ],
    )?;

    // p3: ROOT (btrfs) — keep filesystem consistent with pipeline; only the table differs.
    run_command(
        "parted",
        &[
            "-s",
            "-a",
            "optimal",
            &ctx.disk,
            "mkpart",
            "primary",
            "btrfs",
            &boot_end,
            &ctx.root_end,
        ],
    )?;

    // p4: DATA (btrfs)
    run_command(
        "parted",
        &[
            "-s",
            "-a",
            "optimal",
            &ctx.disk,
            "mkpart",
            "primary",
            "btrfs",
            &ctx.root_end,
            "100%",
        ],
    )?;

    run_command("parted", &["-s", &ctx.disk, "print"])?;
    udev_settle();

    info!("📋 MBR partition table created (4 partitions)");
    Ok(())
}

fn partition_disk_gpt(ctx: &FlashContext) -> Result<()> {
    ctx.status("🔪 Creating GPT partition table with parted...");

    // Wipe existing
    run_command("wipefs", &["-a", &ctx.disk])?;
    udev_settle();

    // Calculate partition boundaries
    let efi_start = "4MiB";
    let efi_end = ctx.efi_size.clone(); // Use value from context
    let boot_end = format!(
        "{}MiB",
        parse_size_to_mib(&ctx.efi_size)? + parse_size_to_mib(&ctx.boot_size)?
    ); // Calculate based on ctx values

    // Create GPT partition table
    run_command("parted", &["-s", &ctx.disk, "mklabel", "gpt"])?;

    // p1: EFI (fat32) with esp flag
    run_command(
        "parted",
        &[
            "-s", "-a", "optimal", &ctx.disk, "mkpart", "primary", "fat32", efi_start, &efi_end,
        ],
    )?;
    run_command("parted", &["-s", &ctx.disk, "set", "1", "esp", "on"])?;

    // p2: BOOT (ext4)
    run_command(
        "parted",
        &[
            "-s", "-a", "optimal", &ctx.disk, "mkpart", "primary", "ext4", &efi_end, &boot_end,
        ],
    )?;

    // p3: ROOT (btrfs) - from boot_end to ROOT_END
    run_command(
        "parted",
        &[
            "-s",
            "-a",
            "optimal",
            &ctx.disk,
            "mkpart",
            "primary",
            "btrfs",
            &boot_end,
            &ctx.root_end,
        ],
    )?; // Use value from context

    // p4: DATA (btrfs) - from ROOT_END to 100%
    run_command(
        "parted",
        &[
            "-s",
            "-a",
            "optimal",
            &ctx.disk,
            "mkpart",
            "primary",
            "btrfs",
            &ctx.root_end,
            "100%",
        ],
    )?; // Use value from context

    // Show result
    run_command("parted", &["-s", &ctx.disk, "print"])?;
    udev_settle();

    info!("📋 GPT partition table created (4 partitions)");
    Ok(())
}

fn format_partitions(ctx: &FlashContext) -> Result<()> {
    let p1 = ctx.partition_path(1);
    let p2 = ctx.partition_path(2);
    let p3 = ctx.partition_path(3);
    let p4 = ctx.partition_path(4);

    ctx.status("✨ Formatting EFI partition (FAT32)...");
    run_command("mkfs.vfat", &["-F", "32", "-n", "EFI", &p1])?;

    ctx.status("✨ Formatting BOOT partition (ext4)...");
    run_command("mkfs.ext4", &["-F", "-L", "BOOT", &p2])?;

    ctx.status("✨ Formatting ROOT partition (btrfs)...");
    run_command("mkfs.btrfs", &["-f", "-L", "FEDORA", &p3])?;

    ctx.status("✨ Formatting DATA partition (btrfs)...");
    run_command("mkfs.btrfs", &["-f", "-L", "DATA", &p4])?;

    udev_settle();
    Ok(())
}

// ============================================================================
// Loop Device and Mount Functions
// ============================================================================

fn setup_image_loop(ctx: &mut FlashContext) -> Result<()> {
    let output = Command::new("losetup")
        .args(["--show", "-Pf", ctx.image.to_str().unwrap()])
        .output()
        .context("Failed to setup loop device")?;

    if !output.status.success() {
        bail!(
            "losetup failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let loop_dev = String::from_utf8_lossy(&output.stdout).trim().to_string();
    info!("🔄 Image mounted at loop device: {}", loop_dev);
    ctx.loop_device = Some(loop_dev);

    std::thread::sleep(std::time::Duration::from_secs(1));
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
    run_command("mount", &[&img_efi, mounts.src_efi.to_str().unwrap()])?;
    run_command("mount", &[&img_boot, mounts.src_boot.to_str().unwrap()])?;

    // Mount btrfs root (top-level first to detect subvols)
    run_command(
        "mount",
        &[
            "-t",
            "btrfs",
            &img_root,
            mounts.src_root_top.to_str().unwrap(),
        ],
    )?;

    // Detect subvolumes
    let subvol_output = Command::new("btrfs")
        .args(["subvolume", "list", mounts.src_root_top.to_str().unwrap()])
        .output()?;
    let subvol_list = String::from_utf8_lossy(&subvol_output.stdout);

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
    run_command(
        "mount",
        &[
            "-t",
            "btrfs",
            "-o",
            "subvol=root",
            &img_root,
            mounts.src_root_subvol.to_str().unwrap(),
        ],
    )?;
    if has_home {
        run_command(
            "mount",
            &[
                "-t",
                "btrfs",
                "-o",
                "subvol=home",
                &img_root,
                mounts.src_home_subvol.to_str().unwrap(),
            ],
        )?;
    }
    if has_var {
        run_command(
            "mount",
            &[
                "-t",
                "btrfs",
                "-o",
                "subvol=var",
                &img_root,
                mounts.src_var_subvol.to_str().unwrap(),
            ],
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

    run_command("mount", &[&p1, mounts.dst_efi.to_str().unwrap()])?;
    run_command("mount", &[&p2, mounts.dst_boot.to_str().unwrap()])?;
    run_command("mount", &[&p4, mounts.dst_data.to_str().unwrap()])?;
    run_command(
        "mount",
        &["-t", "btrfs", &p3, mounts.dst_root_top.to_str().unwrap()],
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
    run_command(
        "btrfs",
        &[
            "subvolume",
            "create",
            mounts.dst_root_top.join("root").to_str().unwrap(),
        ],
    )?;
    if subvols.has_home {
        run_command(
            "btrfs",
            &[
                "subvolume",
                "create",
                mounts.dst_root_top.join("home").to_str().unwrap(),
            ],
        )?;
    }
    if subvols.has_var {
        run_command(
            "btrfs",
            &[
                "subvolume",
                "create",
                mounts.dst_root_top.join("var").to_str().unwrap(),
            ],
        )?;
    }

    // Mount subvolumes for copying
    run_command(
        "mount",
        &[
            "-t",
            "btrfs",
            "-o",
            "subvol=root",
            &p3,
            mounts.dst_root_subvol.to_str().unwrap(),
        ],
    )?;
    if subvols.has_home {
        run_command(
            "mount",
            &[
                "-t",
                "btrfs",
                "-o",
                "subvol=home",
                &p3,
                mounts.dst_home_subvol.to_str().unwrap(),
            ],
        )?;
    }
    if subvols.has_var {
        run_command(
            "mount",
            &[
                "-t",
                "btrfs",
                "-o",
                "subvol=var",
                &p3,
                mounts.dst_var_subvol.to_str().unwrap(),
            ],
        )?;
    }

    Ok(())
}

// ============================================================================
// Copy Functions
// ============================================================================

fn rsync_with_progress(ctx: &FlashContext, src: &Path, dst: &Path, label: &str) -> Result<()> {
    ctx.status(&format!("📦 Copying {}...", label));

    let src_str = format!("{}/", src.to_str().unwrap());
    let dst_str = dst.to_str().unwrap();

    let mut child = Command::new("rsync")
        .args([
            "-aHAX",
            "--numeric-ids",
            "--info=progress2",
            &src_str,
            dst_str,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to spawn rsync")?;

    if let Some(stdout) = child.stdout.take() {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            if cancel_requested() {
                let _ = child.kill();
                bail!("Cancelled");
            }
            if let Some(progress) = parse_rsync_progress(&line) {
                ctx.send_progress(ProgressUpdate::RsyncProgress {
                    percent: progress.percent,
                    speed_mbps: progress.speed_mbps,
                    files_done: progress.files_done,
                    files_total: progress.files_total,
                });
            }
        }
    }

    let status = child.wait()?;
    if !status.success() {
        bail!("rsync failed for {}", label);
    }
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
fn rsync_vfat_safe(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    run_command(
        "rsync",
        &[
            "-rltD",
            "--no-owner",
            "--no-group",
            "--no-perms",
            &format!("{}/", src.to_str().unwrap()),
            dst.to_str().unwrap(),
        ],
    )
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
    let boot_uuid = get_partition_uuid(&ctx.partition_path(2))?;
    let root_uuid = get_partition_uuid(&ctx.partition_path(3))?;

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

    let efi_uuid = get_partition_uuid(&ctx.partition_path(1))?;
    let boot_uuid = get_partition_uuid(&ctx.partition_path(2))?;
    let root_uuid = get_partition_uuid(&ctx.partition_path(3))?;
    let data_uuid = get_partition_uuid(&ctx.partition_path(4))?;

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

fn get_partition_uuid(device: &str) -> Result<String> {
    let output = Command::new("blkid")
        .args(["-s", "UUID", "-o", "value", device])
        .output()
        .context("Failed to get partition UUID")?;
    if !output.status.success() {
        bail!("blkid failed for {}", device);
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
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
    run_command("chmod", &["+x", dojo_script.to_str().unwrap()])?;

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

    let mut cmd = Command::new("xz");
    cmd.args(["-dc", xz_image_path.to_str().unwrap()]);

    let mut child = cmd
        .stdout(Stdio::piped())
        .spawn()
        .context("Failed to spawn xz process")?;

    let mut output_file = std::fs::File::create(&raw_image_path).with_context(|| {
        format!(
            "Failed to create raw image file: {}",
            raw_image_path.display()
        )
    })?;

    let mut stdout = child
        .stdout
        .take()
        .context("Failed to get stdout from xz process")?;
    std::io::copy(&mut stdout, &mut output_file).context("Failed to copy decompressed data")?;

    let status = child.wait().context("Failed to wait for xz process")?;

    if !status.success() {
        bail!(
            "xz decompression failed with exit code: {:?}",
            status.code()
        );
    }

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
            let _ = Command::new("umount")
                .args(["-R", mp.to_str().unwrap()])
                .status();
        }
    }

    if let Some(ref loop_dev) = ctx.loop_device {
        let _ = Command::new("losetup").args(["-d", loop_dev]).status();
    }

    udev_settle();
}

fn normalize_disk(d: &str) -> String {
    if d.starts_with("/dev/") {
        d.to_string()
    } else {
        format!("/dev/{}", d)
    }
}

fn show_lsblk(disk: &str) -> Result<()> {
    info!("🧾 Current disk layout for {}", disk);
    let output = Command::new("lsblk")
        .args(["-o", "NAME,SIZE,TYPE,FSTYPE,MOUNTPOINTS,MODEL", disk])
        .output()
        .context("Failed to run lsblk")?;
    info!("\n{}", String::from_utf8_lossy(&output.stdout));
    Ok(())
}

fn run_command(cmd: &str, args: &[&str]) -> Result<()> {
    debug!("Running: {} {}", cmd, args.join(" "));
    let status = Command::new(cmd)
        .args(args)
        .status()
        .with_context(|| format!("Failed to execute {}", cmd))?;
    if !status.success() {
        bail!("{} failed with exit code: {:?}", cmd, status.code());
    }
    Ok(())
}

fn udev_settle() {
    let _ = Command::new("udevadm").arg("settle").status();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_disk() {
        assert_eq!(normalize_disk("sda"), "/dev/sda");
        assert_eq!(normalize_disk("/dev/sda"), "/dev/sda");
    }

    #[test]
    fn test_partition_path() {
        let ctx = FlashContext {
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
        };
        assert_eq!(ctx.partition_path(1), "/dev/sda1");
        assert_eq!(ctx.partition_path(4), "/dev/sda4");
    }
}
