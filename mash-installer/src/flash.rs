//! Flash module - Core installation logic
//!
//! Phase 1B/1C: Flash + stage Dojo.
//! - Uses losetup+mount+rsync instead of dd so we can apply UEFI tweaks + stage Dojo.
//! - Strong safety rails: refuses to touch the root disk; confirmation gate unless --yes-i-know.
//! - `--dry-run` prints intended actions but does not modify disks.

use crate::cli::Cli;
use crate::errors::{MashError, Result};
use crate::locale::LocaleConfig;
use crate::tui::progress::{Phase, ProgressUpdate};
use indicatif::{ProgressBar, ProgressStyle};
use std::ffi::OsStr;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::Sender;
use std::thread;
use std::time::{Duration, Instant};

/// Run flash with CLI (original interface, no progress channel)
pub fn run(
    cli: &Cli,
    image: &PathBuf,
    disk: &str,
    uefi_dir: &PathBuf,
    dry_run: bool,
    auto_unmount: bool,
    yes_i_know: bool,
    watch: bool,
) -> Result<()> {
    run_with_progress(
        cli,
        image,
        disk,
        uefi_dir,
        dry_run,
        auto_unmount,
        yes_i_know,
        watch,
        None,   // No locale config (use default)
        true,   // Early SSH enabled by default
        None,   // No progress channel
    )
}

/// Run flash with full options including progress channel for TUI
pub fn run_with_progress(
    cli: &Cli,
    image: &PathBuf,
    disk: &str,
    uefi_dir: &PathBuf,
    dry_run: bool,
    auto_unmount: bool,
    yes_i_know: bool,
    watch: bool,
    locale: Option<LocaleConfig>,
    early_ssh: bool,
    progress_tx: Option<Sender<ProgressUpdate>>,
) -> Result<()> {
    let send_progress = |update: ProgressUpdate| {
        if let Some(ref tx) = progress_tx {
            let _ = tx.send(update);
        }
    };

    require_root()?;
    validate_inputs(image, disk, uefi_dir)?;

    log::info!("🥋 Phase 1B/1C: Flash");
    log::info!("Image:   {}", image.display());
    log::info!("Disk:    {}", disk);
    log::info!("UEFI:    {}", uefi_dir.display());
    if dry_run {
        log::info!("Mode:    dry-run (no changes will be made)");
    }

    // Safety: refuse root disk (where / is mounted)
    refuse_root_disk(disk)?;

    // Show disk layout for confidence
    print_disk_layout(disk)?;

    // Confirmation gate
    if !yes_i_know {
        confirm_or_bail()?;
    }

    // Auto-unmount anything from this disk
    if auto_unmount {
        unmount_all(disk, dry_run)?;
    }

    // === Phase 1: Partition ===
    send_progress(ProgressUpdate::PhaseStarted(Phase::Partition));
    send_progress(ProgressUpdate::Status("Wiping disk signatures...".into()));

    // Wipe signatures (helps when reusing disks)
    run_cmd(dry_run, "wipefs", &["-a", disk])?;

    // Partition (MBR) + mkfs
    partition_mbr_4(disk, dry_run)?;
    send_progress(ProgressUpdate::PhaseCompleted(Phase::Partition));

    // === Phase 2: Format ===
    send_progress(ProgressUpdate::PhaseStarted(Phase::Format));
    mkfs_all(disk, dry_run)?;
    send_progress(ProgressUpdate::PhaseCompleted(Phase::Format));

    // Mount destinations
    let mnt = TempMounts::new()?;
    if !dry_run {
        mnt.mount_all(disk)?;
    }

    // Loop-mount source image and mount its partitions
    let loopdev = setup_loop(image, dry_run)?;
    let src = if !dry_run {
        mount_source(&loopdev)?
    } else {
        SourceMounts::dry()
    };

    // Prepare BTRFS subvols on dest root
    if !dry_run {
        prepare_btrfs(&mnt.dst_root)?;
        // remount @ and @home
        remount_btrfs_subvols(disk, &mnt)?;
    }

    // === Phase 3: Copy Root ===
    send_progress(ProgressUpdate::PhaseStarted(Phase::CopyRoot));
    if !dry_run {
        rsync_copy_with_progress(&src.src_root, &mnt.dst_root, watch, disk, &progress_tx)?;
    } else {
        log::info!("(dry-run) would rsync root from image onto target");
    }
    send_progress(ProgressUpdate::PhaseCompleted(Phase::CopyRoot));

    // === Phase 4: Copy Boot ===
    send_progress(ProgressUpdate::PhaseStarted(Phase::CopyBoot));
    if !dry_run {
        rsync_copy_with_progress(&src.src_boot, &mnt.dst_boot, watch, disk, &progress_tx)?;
    } else {
        log::info!("(dry-run) would rsync boot from image onto target");
    }
    send_progress(ProgressUpdate::PhaseCompleted(Phase::CopyBoot));

    // === Phase 5: Copy EFI ===
    send_progress(ProgressUpdate::PhaseStarted(Phase::CopyEfi));
    if !dry_run {
        rsync_copy_with_progress(&src.src_efi, &mnt.dst_efi, watch, disk, &progress_tx)?;
    } else {
        log::info!("(dry-run) would rsync efi from image onto target");
    }
    send_progress(ProgressUpdate::PhaseCompleted(Phase::CopyEfi));

    // === Phase 6: UEFI Config ===
    send_progress(ProgressUpdate::PhaseStarted(Phase::UefiConfig));
    if !dry_run {
        apply_uefi_overlay(uefi_dir, &mnt.dst_efi)?;
        ensure_bootaa64(&mnt.dst_efi)?;
    } else {
        log::info!(
            "(dry-run) would apply UEFI overlay from {}",
            uefi_dir.display()
        );
    }
    send_progress(ProgressUpdate::PhaseCompleted(Phase::UefiConfig));

    // === Phase 7: Locale Config ===
    send_progress(ProgressUpdate::PhaseStarted(Phase::LocaleConfig));
    let locale_config = locale.unwrap_or_else(|| crate::locale::LOCALES[0].clone());
    crate::locale::patch_locale(&mnt.dst_root, &locale_config, dry_run)?;
    send_progress(ProgressUpdate::PhaseCompleted(Phase::LocaleConfig));

    // === Phase 8: Fstab ===
    send_progress(ProgressUpdate::PhaseStarted(Phase::Fstab));
    generate_fstab(disk, &mnt.dst_root, dry_run)?;
    send_progress(ProgressUpdate::PhaseCompleted(Phase::Fstab));

    // === Phase 9: Stage Dojo ===
    send_progress(ProgressUpdate::PhaseStarted(Phase::StageDojo));
    if !dry_run {
        stage_dojo(cli, &mnt.dst_data, &mnt.dst_root, early_ssh)?;
    } else {
        log::info!("(dry-run) would stage dojo_bundle + helpers to DATA partition");
        if early_ssh {
            log::info!("(dry-run) would enable early SSH services");
        }
    }
    send_progress(ProgressUpdate::PhaseCompleted(Phase::StageDojo));

    // === Phase 10: Cleanup ===
    send_progress(ProgressUpdate::PhaseStarted(Phase::Cleanup));
    drop(src);
    if !dry_run {
        detach_loop(&loopdev)?;
    }
    if !dry_run {
        mnt.sync_and_unmount()?;
    }
    send_progress(ProgressUpdate::PhaseCompleted(Phase::Cleanup));

    send_progress(ProgressUpdate::Complete);

    log::info!("✅ Flash complete.");
    log::info!("Next: plug the disk into the Pi UEFI boot chain and boot Fedora.");
    log::info!("Dojo staged at: /data/mash-staging (DATA partition).");
    Ok(())
}

fn require_root() -> Result<()> {
    if unsafe { libc::geteuid() } != 0 {
        anyhow::bail!("Must run as root (sudo).");
    }
    Ok(())
}

fn validate_inputs(image: &Path, disk: &str, uefi_dir: &Path) -> Result<()> {
    if !image.exists() {
        anyhow::bail!("Image not found: {}", image.display());
    }
    if !uefi_dir.exists() || !uefi_dir.is_dir() {
        anyhow::bail!("UEFI dir not found: {}", uefi_dir.display());
    }
    if !disk.starts_with("/dev/") {
        return Err(MashError::InvalidDisk(disk.to_string()).into());
    }
    // Basic guard: refuse partitions (/dev/sda1) — we want whole disk.
    if disk.chars().last().unwrap_or('x').is_ascii_digit() && !disk.contains("nvme") {
        anyhow::bail!(
            "Disk must be a whole disk (e.g. /dev/sda), not a partition: {}",
            disk
        );
    }
    Ok(())
}

fn refuse_root_disk(target_disk: &str) -> Result<()> {
    // Find block device backing /
    let out = cmd_output("findmnt", &["-n", "-o", "SOURCE", "/"])?;
    let src = out.trim().to_string();
    if src.starts_with("/dev/") {
        // Resolve to a base disk: /dev/mmcblk0p2 -> /dev/mmcblk0, /dev/sda2 -> /dev/sda
        let base = base_disk_from_dev(&src);
        let tgt_base = base_disk_from_dev(target_disk);
        if base == tgt_base {
            return Err(MashError::RefuseRootDisk(target_disk.to_string()).into());
        }
    }
    Ok(())
}

fn base_disk_from_dev(dev: &str) -> String {
    // Handles: /dev/sda2 -> /dev/sda, /dev/mmcblk0p2 -> /dev/mmcblk0, /dev/nvme0n1p2 -> /dev/nvme0n1
    let mut s = dev.to_string();
    if s.contains("nvme") {
        // strip trailing p\d+
        s = regex_strip(&s, r"p\d+$");
        return s;
    }
    // strip trailing digits
    s = regex_strip(&s, r"\d+$");
    // special mmcblk0p2 -> mmcblk0p
    s = regex_strip(&s, r"p$");
    s
}

fn regex_strip(s: &str, pat: &str) -> String {
    let re = regex::Regex::new(pat).unwrap();
    re.replace(s, "").to_string()
}

fn print_disk_layout(disk: &str) -> Result<()> {
    log::info!("Current disk layout:");
    let out = cmd_output(
        "lsblk",
        &["-o", "NAME,SIZE,FSTYPE,LABEL,MOUNTPOINTS", disk],
    )?;
    eprintln!("{}", out); // stderr so it doesn't pollute URL outputs if embedded
    Ok(())
}

fn confirm_or_bail() -> Result<()> {
    eprintln!();
    eprintln!("⚠️  DANGER ZONE ⚠️");
    eprintln!("This will ERASE the target disk.");
    eprintln!("Type EXACTLY: YES I KNOW");
    eprint!("> ");
    io::stdout().flush()?;
    let mut s = String::new();
    io::stdin().read_line(&mut s)?;
    if s.trim() != "YES I KNOW" {
        anyhow::bail!("Aborted.");
    }
    Ok(())
}

fn unmount_all(disk: &str, dry_run: bool) -> Result<()> {
    // List mount targets for any /dev/<disk>* sources
    let base = Path::new(disk)
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or("");
    let pattern = format!("/dev/{}*", base);
    let out = cmd_output("findmnt", &["-rn", "-S", &pattern, "-o", "TARGET"])?;
    for line in out.lines().filter(|l| !l.trim().is_empty()) {
        run_cmd(dry_run, "umount", &["-fl", line])?;
    }
    Ok(())
}

fn partition_mbr_4(disk: &str, dry_run: bool) -> Result<()> {
    // Phase 1B – MBR (msdos) 4-partition layout, tuned for 4TB-class drives.
    let dev = disk.trim();
    let disk_base = dev.rsplit('/').next().unwrap_or(dev);

    // disk size bytes
    let size_bytes: u64 = {
        let out = cmd_output("blockdev", &["--getsize64", dev])?;
        out.trim().parse().unwrap_or(0)
    };

    // logical block size (bytes)
    let log_sec: u64 = {
        let p = format!("/sys/block/{}/queue/logical_block_size", disk_base);
        std::fs::read_to_string(&p)
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(512)
    };

    let mbr_limit_bytes: u64 = (u64::from(u32::MAX) + 1) * 512; // ~2TiB at 512B sectors
    if log_sec == 512 && size_bytes > mbr_limit_bytes {
        log::warn!("⚠️  Disk is >2TiB and reports 512B logical sectors.");
        log::warn!("⚠️  Classic MBR may not address space beyond ~2TiB on this disk.");
        log::warn!("⚠️  You said this layout is battle-tested on your kit, so continuing…");
    } else {
        log::info!("Disk logical sector size: {} bytes", log_sec);
    }

    // Desired sizes (decimal TB) -> MiB for parted
    let root_bytes: u64 = 1_800_000_000_000;
    let data_bytes: u64 = 1_900_000_000_000;
    let bytes_per_mib: u64 = 1024 * 1024;
    let root_mib: u64 = root_bytes / bytes_per_mib;
    let data_mib: u64 = data_bytes / bytes_per_mib;

    // Partition plan (MiB)
    let efi_start: u64 = 1;
    let efi_end: u64 = 512; // 511MiB EFI
    let boot_end: u64 = 1536; // +1024MiB BOOT
    let root_start: u64 = boot_end;
    let root_end: u64 = root_start + root_mib;
    let data_start: u64 = root_end;
    let disk_mib: u64 = size_bytes / bytes_per_mib;

    // Safety padding at end
    let end_pad: u64 = 4;
    if disk_mib <= data_start + 32 {
        anyhow::bail!(
            "Disk too small for requested layout (disk ≈ {} MiB). Need > {} MiB.",
            disk_mib,
            data_start + 32
        );
    }
    // If requested data size overruns disk, clamp to 100%
    let mut data_end: u64 = data_start + data_mib;
    if data_end + end_pad > disk_mib {
        data_end = disk_mib.saturating_sub(end_pad);
        log::warn!(
            "⚠️  Data partition clamped to end of disk (requested {} MiB, available {}).",
            data_mib,
            data_end.saturating_sub(data_start)
        );
    }

    log::info!(
        "MBR layout (MiB): EFI {}-{}, BOOT {}-{}, ROOT {}-{}, DATA {}-{}",
        efi_start,
        efi_end,
        efi_end,
        boot_end,
        root_start,
        root_end,
        data_start,
        data_end
    );

    // Create msdos table
    run_cmd(dry_run, "parted", &["-s", dev, "mklabel", "msdos"])?;

    // parted wants explicit units
    let u = |m: u64| format!("{}MiB", m);

    // p1 EFI (fat32, boot flag)
    run_cmd(
        dry_run,
        "parted",
        &[
            "-s",
            dev,
            "mkpart",
            "primary",
            "fat32",
            &u(efi_start),
            &u(efi_end),
        ],
    )?;
    run_cmd(dry_run, "parted", &["-s", dev, "set", "1", "boot", "on"])?;

    // p2 BOOT (ext4)
    run_cmd(
        dry_run,
        "parted",
        &[
            "-s",
            dev,
            "mkpart",
            "primary",
            "ext4",
            &u(efi_end),
            &u(boot_end),
        ],
    )?;

    // p3 ROOT (btrfs)
    run_cmd(
        dry_run,
        "parted",
        &[
            "-s",
            dev,
            "mkpart",
            "primary",
            "btrfs",
            &u(root_start),
            &u(root_end),
        ],
    )?;

    // p4 DATA (ext4)
    run_cmd(
        dry_run,
        "parted",
        &[
            "-s",
            dev,
            "mkpart",
            "primary",
            "ext4",
            &u(data_start),
            &u(data_end),
        ],
    )?;

    // Ensure kernel sees new table
    run_cmd(dry_run, "partprobe", &[dev])?;
    Ok(())
}

fn mkfs_all(disk: &str, dry_run: bool) -> Result<()> {
    let p1 = part_path(disk, 1);
    let p2 = part_path(disk, 2);
    let p3 = part_path(disk, 3);
    let p4 = part_path(disk, 4);

    run_cmd(dry_run, "mkfs.vfat", &["-F", "32", "-n", "EFI", &p1])?;
    run_cmd(dry_run, "mkfs.ext4", &["-F", "-L", "BOOT", &p2])?;
    run_cmd(dry_run, "mkfs.btrfs", &["-f", "-L", "ROOT", &p3])?;
    run_cmd(dry_run, "mkfs.ext4", &["-F", "-L", "DATA", &p4])?;
    Ok(())
}

fn part_path(disk: &str, n: u8) -> String {
    if disk.contains("nvme") || disk.contains("mmcblk") {
        format!("{}p{}", disk, n)
    } else {
        format!("{}{}", disk, n)
    }
}

fn setup_loop(image: &Path, dry_run: bool) -> Result<String> {
    if dry_run {
        return Ok("/dev/loopDRY".to_string());
    }
    let out = cmd_output(
        "losetup",
        &["--find", "--show", "-P", image.to_str().unwrap()],
    )?;
    Ok(out.trim().to_string())
}

fn detach_loop(loopdev: &str) -> Result<()> {
    run_cmd(false, "losetup", &["-d", loopdev])?;
    Ok(())
}

struct TempMounts {
    _root: tempfile::TempDir,
    dst_root: PathBuf,
    dst_boot: PathBuf,
    dst_efi: PathBuf,
    dst_data: PathBuf,
}

impl TempMounts {
    fn new() -> Result<Self> {
        let root = tempfile::tempdir()?;
        let dst_root = root.path().join("dst-root");
        let dst_boot = root.path().join("dst-boot");
        let dst_efi = root.path().join("dst-efi");
        let dst_data = root.path().join("dst-data");
        fs::create_dir_all(&dst_root)?;
        fs::create_dir_all(&dst_boot)?;
        fs::create_dir_all(&dst_efi)?;
        fs::create_dir_all(&dst_data)?;
        Ok(Self {
            _root: root,
            dst_root,
            dst_boot,
            dst_efi,
            dst_data,
        })
    }

    fn mount_all(&self, disk: &str) -> Result<()> {
        // Initially mount btrfs root partition to create subvols.
        let p3 = part_path(disk, 3);
        run_cmd(false, "mount", &[&p3, self.dst_root.to_str().unwrap()])?;

        let p2 = part_path(disk, 2);
        run_cmd(false, "mount", &[&p2, self.dst_boot.to_str().unwrap()])?;

        let p1 = part_path(disk, 1);
        run_cmd(false, "mount", &[&p1, self.dst_efi.to_str().unwrap()])?;

        let p4 = part_path(disk, 4);
        run_cmd(false, "mount", &[&p4, self.dst_data.to_str().unwrap()])?;
        Ok(())
    }

    fn sync_and_unmount(&self) -> Result<()> {
        run_cmd(false, "sync", &[]).ok();
        // unmount in reverse order; ignore failures
        for p in [
            &self.dst_data,
            &self.dst_efi,
            &self.dst_boot,
            &self.dst_root,
        ] {
            run_cmd(false, "umount", &["-fl", p.to_str().unwrap()]).ok();
        }
        run_cmd(false, "sync", &[]).ok();
        Ok(())
    }
}

struct SourceMounts {
    src_root: PathBuf,
    src_boot: PathBuf,
    src_efi: PathBuf,
    _src_dir: Option<tempfile::TempDir>,
}

impl SourceMounts {
    fn dry() -> Self {
        Self {
            src_root: PathBuf::from("/mnt/src-root"),
            src_boot: PathBuf::from("/mnt/src-boot"),
            src_efi: PathBuf::from("/mnt/src-efi"),
            _src_dir: None,
        }
    }
}

fn mount_source(loopdev: &str) -> Result<SourceMounts> {
    let dir = tempfile::tempdir()?;
    let src_root = dir.path().join("src-root");
    let src_boot = dir.path().join("src-boot");
    let src_efi = dir.path().join("src-efi");
    fs::create_dir_all(&src_root)?;
    fs::create_dir_all(&src_boot)?;
    fs::create_dir_all(&src_efi)?;

    // Identify partitions from loop device:
    // Prefer: vfat -> EFI, ext4 -> BOOT, btrfs/xfs -> ROOT (Fedora raw is commonly btrfs)
    let out = cmd_output("lsblk", &["-rn", "-o", "NAME,FSTYPE,LABEL", loopdev])?;
    let mut efi_part = None;
    let mut boot_part = None;
    let mut root_part = None;

    for line in out.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 {
            continue;
        }
        let name = parts[0];
        let fstype = parts[1];
        let dev = format!("/dev/{}", name);
        if fstype == "vfat" && efi_part.is_none() {
            efi_part = Some(dev);
        } else if (fstype == "ext4" || fstype == "xfs") && boot_part.is_none() {
            // Fedora /boot is often ext4
            boot_part = Some(dev);
        } else if (fstype == "btrfs" || fstype == "xfs" || fstype == "ext4") && root_part.is_none() {
            // We'll refine: avoid choosing boot again
            if Some(&dev) != boot_part.as_ref() {
                root_part = Some(dev);
            }
        }
    }

    let efi = efi_part.ok_or_else(|| anyhow::anyhow!("Could not find EFI partition in image"))?;
    let boot =
        boot_part.ok_or_else(|| anyhow::anyhow!("Could not find BOOT partition in image"))?;
    let root =
        root_part.ok_or_else(|| anyhow::anyhow!("Could not find ROOT partition in image"))?;

    run_cmd(false, "mount", &[&root, src_root.to_str().unwrap()])?;
    run_cmd(false, "mount", &[&boot, src_boot.to_str().unwrap()])?;
    run_cmd(false, "mount", &[&efi, src_efi.to_str().unwrap()])?;

    Ok(SourceMounts {
        src_root,
        src_boot,
        src_efi,
        _src_dir: Some(dir),
    })
}

fn prepare_btrfs(dst_root_mount: &Path) -> Result<()> {
    // Create subvols @ and @home, then copy rootfs into @
    // We mounted the raw btrfs volume at dst_root_mount.
    run_cmd(
        false,
        "btrfs",
        &[
            "subvolume",
            "create",
            &dst_root_mount.join("@").to_string_lossy(),
        ],
    )?;
    run_cmd(
        false,
        "btrfs",
        &[
            "subvolume",
            "create",
            &dst_root_mount.join("@home").to_string_lossy(),
        ],
    )?;
    Ok(())
}

fn remount_btrfs_subvols(disk: &str, mnt: &TempMounts) -> Result<()> {
    // Unmount current root mount and remount @ subvol at dst_root, @home at dst_root/home
    run_cmd(false, "umount", &["-fl", mnt.dst_root.to_str().unwrap()]).ok();
    fs::create_dir_all(mnt.dst_root.join("home"))?;
    let p3 = part_path(disk, 3);
    run_cmd(
        false,
        "mount",
        &[
            "-o",
            "subvol=@,compress=zstd:1",
            &p3,
            mnt.dst_root.to_str().unwrap(),
        ],
    )?;
    run_cmd(
        false,
        "mount",
        &[
            "-o",
            "subvol=@home,compress=zstd:1",
            &p3,
            mnt.dst_root.join("home").to_str().unwrap(),
        ],
    )?;
    // mount boot and efi inside root
    fs::create_dir_all(mnt.dst_root.join("boot"))?;
    fs::create_dir_all(mnt.dst_root.join("boot/efi"))?;
    run_cmd(
        false,
        "mount",
        &[
            &part_path(disk, 2),
            mnt.dst_root.join("boot").to_str().unwrap(),
        ],
    )?;
    run_cmd(
        false,
        "mount",
        &[
            &part_path(disk, 1),
            mnt.dst_root.join("boot/efi").to_str().unwrap(),
        ],
    )?;
    Ok(())
}

/// rsync with progress reporting to channel
fn rsync_copy_with_progress(
    src: &Path,
    dst: &Path,
    watch: bool,
    disk: &str,
    progress_tx: &Option<Sender<ProgressUpdate>>,
) -> Result<()> {
    log::info!("📦 rsync: {} -> {}", src.display(), dst.display());
    let mut cmd = Command::new("rsync");
    cmd.arg("-aHAX")
        .arg("--numeric-ids")
        .arg("--info=progress2")
        .arg(format!("{}/", src.display()))
        .arg(format!("{}/", dst.display()))
        .stderr(Stdio::piped())
        .stdout(Stdio::null());

    let mut child = cmd
        .spawn()
        .map_err(|e| anyhow::anyhow!("Failed to spawn rsync: {e}"))?;

    let pb = ProgressBar::new_spinner();
    pb.set_style(ProgressStyle::with_template("{spinner} {msg}").unwrap());
    pb.enable_steady_tick(Duration::from_millis(120));
    pb.set_message("rsync running…");

    let stats_stop = if watch {
        Some(spawn_diskstats_watch(disk, pb.clone(), progress_tx.clone()))
    } else {
        None
    };

    // Read rsync stderr and update message with last progress line
    if let Some(mut stderr) = child.stderr.take() {
        let mut buf = [0u8; 4096];
        loop {
            let n = stderr.read(&mut buf).unwrap_or(0);
            if n == 0 {
                break;
            }
            let chunk = String::from_utf8_lossy(&buf[..n]);
            for line in chunk.lines() {
                // rsync progress2 lines often include bytes + percent
                if line.contains('%') || line.contains("to-chk=") {
                    pb.set_message(line.to_string());

                    // Parse progress for TUI
                    if let Some(ref tx) = progress_tx {
                        if let Some(update) = parse_rsync_progress(line) {
                            let _ = tx.send(update);
                        }
                    }
                }
            }
        }
    }

    let status = child.wait()?;
    pb.finish_and_clear();

    if let Some(h) = stats_stop {
        h.stop();
    }

    if !status.success() {
        return Err(MashError::CommandFailed {
            cmd: "rsync".into(),
            stderr: format!("exit={:?}", status.code()),
        }
        .into());
    }
    Ok(())
}

/// Parse rsync --info=progress2 output
fn parse_rsync_progress(line: &str) -> Option<ProgressUpdate> {
    // Format: "  1,234,567  12%   45.67MB/s    0:01:23 (xfr#123, to-chk=456/789)"
    let percent = line
        .split_whitespace()
        .find(|s| s.ends_with('%'))
        .and_then(|s| s.trim_end_matches('%').parse::<f64>().ok())?;

    let speed = line
        .split_whitespace()
        .find(|s| s.ends_with("MB/s") || s.ends_with("GB/s") || s.ends_with("kB/s"))
        .and_then(|s| {
            let multiplier = if s.ends_with("GB/s") {
                1024.0
            } else if s.ends_with("kB/s") {
                1.0 / 1024.0
            } else {
                1.0
            };
            s.trim_end_matches("MB/s")
                .trim_end_matches("GB/s")
                .trim_end_matches("kB/s")
                .parse::<f64>()
                .ok()
                .map(|v| v * multiplier)
        })
        .unwrap_or(0.0);

    // Parse to-chk for file counts
    let (files_done, files_total) = if let Some(start) = line.find("to-chk=") {
        let rest = &line[start + 7..];
        if let Some(end) = rest.find(')') {
            let nums: Vec<&str> = rest[..end].split('/').collect();
            if nums.len() == 2 {
                let remaining: u64 = nums[0].parse().unwrap_or(0);
                let total: u64 = nums[1].parse().unwrap_or(0);
                (total.saturating_sub(remaining), total)
            } else {
                (0, 0)
            }
        } else {
            (0, 0)
        }
    } else {
        (0, 0)
    };

    Some(ProgressUpdate::RsyncProgress {
        percent,
        speed_mbps: speed,
        files_done,
        files_total,
    })
}

struct WatchHandle {
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    join: Option<std::thread::JoinHandle<()>>,
}

impl WatchHandle {
    fn stop(mut self) {
        self.stop
            .store(true, std::sync::atomic::Ordering::Relaxed);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

fn spawn_diskstats_watch(
    disk: &str,
    pb: ProgressBar,
    progress_tx: Option<Sender<ProgressUpdate>>,
) -> WatchHandle {
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop2 = stop.clone();
    let disk_name = Path::new(disk)
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or("")
        .to_string();

    let join = thread::spawn(move || {
        let mut last = read_diskstats_sectors(&disk_name).unwrap_or(0);
        let mut last_t = Instant::now();
        while !stop2.load(std::sync::atomic::Ordering::Relaxed) {
            thread::sleep(Duration::from_millis(250));
            if let Some(now) = read_diskstats_sectors(&disk_name) {
                let dt = last_t.elapsed().as_secs_f64();
                if dt > 0.0 {
                    let delta = now.saturating_sub(last);
                    let mbps = (delta as f64 * 512.0) / (1024.0 * 1024.0) / dt;
                    pb.set_message(format!("disk write: {:.1} MB/s", mbps));
                    if let Some(ref tx) = progress_tx {
                        let _ = tx.send(ProgressUpdate::DiskIo { mbps });
                    }
                }
                last = now;
                last_t = Instant::now();
            }
        }
    });
    WatchHandle {
        stop,
        join: Some(join),
    }
}

fn read_diskstats_sectors(dev: &str) -> Option<u64> {
    let content = fs::read_to_string("/proc/diskstats").ok()?;
    for line in content.lines() {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() < 10 {
            continue;
        }
        if cols[2] == dev {
            return cols[9].parse::<u64>().ok();
        }
    }
    None
}

fn apply_uefi_overlay(uefi_dir: &Path, dst_efi: &Path) -> Result<()> {
    // Copy everything in uefi_dir onto EFI partition root (merge).
    // Use rsync to preserve structure.
    run_cmd(
        false,
        "rsync",
        &[
            "-a",
            &format!("{}/", uefi_dir.display()),
            dst_efi.to_str().unwrap(),
        ],
    )?;
    Ok(())
}

fn ensure_bootaa64(dst_efi: &Path) -> Result<()> {
    // Ensure EFI/BOOT/BOOTAA64.EFI exists (fallback: copy from EFI/fedora/grubaa64.efi if present)
    let boot = dst_efi.join("EFI/BOOT");
    fs::create_dir_all(&boot)?;
    let bootaa64 = boot.join("BOOTAA64.EFI");
    if bootaa64.exists() {
        return Ok(());
    }

    let fed_grub = dst_efi.join("EFI/fedora/grubaa64.efi");
    if fed_grub.exists() {
        fs::copy(fed_grub, bootaa64)?;
        return Ok(());
    }
    // If still missing, don't fail hard – user might have different loader.
    log::warn!("⚠️ Could not ensure BOOTAA64.EFI (no EFI/fedora/grubaa64.efi found).");
    Ok(())
}

/// Generate /etc/fstab with UUIDs
fn generate_fstab(disk: &str, root_mount: &Path, dry_run: bool) -> Result<()> {
    log::info!("Generating fstab with UUIDs...");

    if dry_run {
        log::info!("(dry-run) would generate fstab");
        return Ok(());
    }

    let p1 = part_path(disk, 1);
    let p2 = part_path(disk, 2);
    let p3 = part_path(disk, 3);
    let p4 = part_path(disk, 4);

    let efi_uuid = get_uuid(&p1)?;
    let boot_uuid = get_uuid(&p2)?;
    let root_uuid = get_uuid(&p3)?;
    let data_uuid = get_uuid(&p4)?;

    let fstab = format!(
        "# /etc/fstab - Generated by MASH Installer
# <file system>  <mount point>  <type>  <options>  <dump>  <pass>

# Root filesystem (btrfs @ subvol)
UUID={root_uuid}  /       btrfs  subvol=@,compress=zstd:1,noatime  0 0

# Home (btrfs @home subvol)
UUID={root_uuid}  /home   btrfs  subvol=@home,compress=zstd:1,noatime  0 0

# Boot partition (ext4)
UUID={boot_uuid}  /boot   ext4   defaults  0 2

# EFI System Partition (vfat)
UUID={efi_uuid}   /boot/efi  vfat  umask=0077  0 1

# Data partition (ext4)
UUID={data_uuid}  /data   ext4   defaults,nofail  0 2
"
    );

    let fstab_path = root_mount.join("etc/fstab");
    fs::write(&fstab_path, &fstab)?;
    log::info!("Wrote {}", fstab_path.display());

    Ok(())
}

/// Get UUID for a partition using blkid
fn get_uuid(partition: &str) -> Result<String> {
    let output = cmd_output("blkid", &["-s", "UUID", "-o", "value", partition])?;
    let uuid = output.trim().to_string();
    if uuid.is_empty() {
        anyhow::bail!("Could not get UUID for {}", partition);
    }
    Ok(uuid)
}

fn stage_dojo(cli: &Cli, dst_data: &Path, dst_root: &Path, early_ssh: bool) -> Result<()> {
    // Stage into DATA partition: /data/mash-staging
    let stage = dst_data.join("mash-staging");
    fs::create_dir_all(&stage)?;

    let dojo_bundle = cli.mash_root.join("dojo_bundle");
    let helpers = cli.mash_root.join("helpers");

    if dojo_bundle.exists() {
        run_cmd(
            false,
            "rsync",
            &[
                "-a",
                &format!("{}/", dojo_bundle.display()),
                &format!("{}/", stage.display()),
            ],
        )?;
    } else {
        log::warn!(
            "dojo_bundle not found at {} (skipping)",
            dojo_bundle.display()
        );
    }
    if helpers.exists() {
        let hdst = stage.join("helpers");
        fs::create_dir_all(&hdst)?;
        run_cmd(
            false,
            "rsync",
            &[
                "-a",
                &format!("{}/", helpers.display()),
                &format!("{}/", hdst.display()),
            ],
        )?;
    } else {
        log::warn!("helpers not found at {} (skipping)", helpers.display());
    }

    // Drop a README marker
    fs::write(
        stage.join("README_FIRST_BOOT.txt"),
        "🥋 MASH Dojo is staged.\n\n\
         After first boot into Fedora, run:\n  \
         sudo /data/mash-staging/install_dojo.sh\n\n\
         Or run individual helpers in /data/mash-staging/helpers/\n",
    )?;

    // Create DOJO_READY marker
    fs::write(
        stage.join("DOJO_READY"),
        "🥋 MASH Dojo is ready!\n\n\
         Run: sudo /data/mash-staging/install_dojo.sh\n",
    )?;

    // === Early SSH Setup ===
    if early_ssh {
        log::info!("Enabling early SSH services...");
        setup_early_ssh(dst_root, &dojo_bundle)?;
    }

    Ok(())
}

/// Set up early SSH services on the target system
fn setup_early_ssh(dst_root: &Path, dojo_bundle: &Path) -> Result<()> {
    // Create necessary directories
    let systemd_dir = dst_root.join("etc/systemd/system");
    let mash_lib_dir = dst_root.join("usr/local/lib/mash/system");
    let wants_dir = systemd_dir.join("multi-user.target.wants");

    fs::create_dir_all(&systemd_dir)?;
    fs::create_dir_all(&mash_lib_dir)?;
    fs::create_dir_all(&wants_dir)?;

    // Copy service files from dojo_bundle
    let services = [
        ("systemd/mash-early-ssh.service", "mash-early-ssh.service"),
        (
            "systemd/mash-internet-wait.service",
            "mash-internet-wait.service",
        ),
    ];

    for (src_rel, dst_name) in &services {
        let src_path = dojo_bundle.join(src_rel);
        let dst_path = systemd_dir.join(dst_name);

        if src_path.exists() {
            fs::copy(&src_path, &dst_path)?;
            log::info!("Staged {}", dst_path.display());

            // Create symlink to enable the service
            let link_path = wants_dir.join(dst_name);
            let target = format!("../{}", dst_name);
            if !link_path.exists() {
                std::os::unix::fs::symlink(&target, &link_path)?;
                log::info!("Enabled {} at boot", dst_name);
            }
        } else {
            log::warn!("Service file not found: {}", src_path.display());
        }
    }

    // Copy the scripts
    let scripts = [
        ("systemd/early-ssh.sh", "early-ssh.sh"),
        ("systemd/internet-wait.sh", "internet-wait.sh"),
    ];

    for (src_rel, dst_name) in &scripts {
        let src_path = dojo_bundle.join(src_rel);
        let dst_path = mash_lib_dir.join(dst_name);

        if src_path.exists() {
            fs::copy(&src_path, &dst_path)?;
            // Make executable
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = fs::metadata(&dst_path)?.permissions();
                perms.set_mode(0o755);
                fs::set_permissions(&dst_path, perms)?;
            }
            log::info!("Staged script {}", dst_path.display());
        } else {
            log::warn!("Script not found: {}", src_path.display());
        }
    }

    Ok(())
}

fn run_cmd(dry_run: bool, cmd: &str, args: &[&str]) -> Result<()> {
    if dry_run {
        log::info!("(dry-run) {} {}", cmd, args.join(" "));
        return Ok(());
    }
    let status = Command::new(cmd).args(args).status()?;
    if !status.success() {
        return Err(MashError::CommandFailed {
            cmd: format!("{} {}", cmd, args.join(" ")),
            stderr: format!("exit={:?}", status.code()),
        }
        .into());
    }
    Ok(())
}

fn cmd_output(cmd: &str, args: &[&str]) -> Result<String> {
    let out = Command::new(cmd).args(args).output()?;
    if !out.status.success() {
        return Err(MashError::CommandFailed {
            cmd: format!("{} {}", cmd, args.join(" ")),
            stderr: String::from_utf8_lossy(&out.stderr).to_string(),
        }
        .into());
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}
