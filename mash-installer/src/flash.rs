use anyhow::{Context, Result, bail};
use log::{info, warn, error};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::fs;

use crate::errors::MashError;

// Partition sizes (matching ninja-mbr4-v2.py defaults)
const EFI_SIZE_MB: &str = "1024MiB";      // 1GB for RPi4 UEFI
const BOOT_SIZE_MB: &str = "2048MiB";     // 2GB for kernels
const ROOT_END_GB: &str = "1800GiB";      // 1.8TB for system
// DATA uses remaining space

pub struct FlashConfig {
    pub image: PathBuf,
    pub disk: String,
    pub uefi_dir: PathBuf,
    pub dry_run: bool,
    pub auto_unmount: bool,
    pub yes_i_know: bool,
    pub skip_dojo: bool,
}

pub fn run(
    image: &Path,
    disk: &str,
    uefi_dir: &Path,
    dry_run: bool,
    auto_unmount: bool,
    yes_i_know: bool,
) -> Result<()> {
    let config = FlashConfig {
        image: image.to_path_buf(),
        disk: normalize_disk(disk),
        uefi_dir: uefi_dir.to_path_buf(),
        dry_run,
        auto_unmount,
        yes_i_know,
        skip_dojo: false,
    };

    run_flash(&config)
}

fn run_flash(config: &FlashConfig) -> Result<()> {
    info!("🎮 MASH Full-Loop Installer: Fedora KDE + UEFI Boot for RPi4");
    info!("Target disk: {}", config.disk);
    info!("Image: {}", config.image.display());
    info!("UEFI dir: {}", config.uefi_dir.display());

    // Verify image exists
    if !config.image.exists() {
        bail!("Image file not found: {}", config.image.display());
    }

    // Verify UEFI directory and required files
    verify_uefi_dir(&config.uefi_dir)?;

    // Show disk info
    show_lsblk(&config.disk)?;

    // Safety check
    if !config.yes_i_know && !config.dry_run {
        return Err(MashError::MissingYesIKnow.into());
    }

    // Unmount if requested
    if config.auto_unmount {
        if config.dry_run {
            info!("(dry-run) would unmount anything mounted on {}", config.disk);
        } else {
            if !config.yes_i_know && !confirm(&format!("Unmount anything on {}?", config.disk))? {
                return Err(MashError::Aborted.into());
            }
            unmount_all(&config.disk)?;
        }
    }

    if config.dry_run {
        info!("(dry-run) Would perform:");
        info!("  1. Wipe disk and create MBR partition table");
        info!("  2. Create 4 partitions: EFI (1GB), BOOT (2GB), ROOT (1.8TB, btrfs), DATA (remaining)");
        info!("  3. Format partitions");
        info!("  4. Loop mount image and rsync to ROOT");
        info!("  5. Configure UEFI boot (dracut, grub)");
        info!("  6. Stage Dojo to DATA partition");
        info!("  7. Install offline boot units");
        return Ok(());
    }

    // Real installation pipeline
    info!("🔥 Starting full installation pipeline...");
    
    // Step 1: Partition the disk (MBR with 4 partitions)
    partition_disk_mbr(&config)?;
    
    // Step 2: Format partitions
    format_partitions(&config)?;
    
    // Step 3: Mount image and install system
    install_system_from_loop(&config)?;
    
    // Step 4: Configure UEFI boot
    configure_uefi_boot(&config)?;
    
    // Step 5: Stage Dojo bundle to DATA partition
    if !config.skip_dojo {
        stage_dojo_to_data(&config)?;
    }
    
    // Step 6: Install offline boot units and locale
    install_offline_boot_units(&config)?;
    offline_locale_patch(&config)?;
    
    // Step 7: Cleanup
    final_cleanup(&config)?;
    
    info!("✅ Installation complete!");
    info!("📝 Next steps:");
    info!("   1. Safely remove the SD card/USB");
    info!("   2. Insert into Raspberry Pi 4");
    info!("   3. Power on - UEFI should boot Fedora");
    info!("   4. After first boot, Dojo will appear automatically");
    info!("   5. Or manually run: /data/mash-staging/install_dojo.sh");
    
    Ok(())
}

fn normalize_disk(d: &str) -> String {
    if d.starts_with("/dev/") { 
        d.to_string() 
    } else { 
        format!("/dev/{}", d) 
    }
}

fn verify_uefi_dir(uefi_dir: &Path) -> Result<()> {
    if !uefi_dir.exists() || !uefi_dir.is_dir() {
        bail!("UEFI directory not found: {}", uefi_dir.display());
    }
    
    let required_files = ["RPI_EFI.fd", "start4.elf", "fixup4.dat"];
    let mut missing = Vec::new();
    
    for file in &required_files {
        if !uefi_dir.join(file).exists() {
            missing.push(*file);
        }
    }
    
    if !missing.is_empty() {
        bail!("UEFI directory missing required files: {}", missing.join(", "));
    }
    
    Ok(())
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

fn unmount_all(disk: &str) -> Result<()> {
    info!("🧹 Unmounting all partitions on {}", disk);
    
    let output = Command::new("bash")
        .arg("-c")
        .arg(format!("lsblk -rno NAME,MOUNTPOINTS {} | awk '$2 != \"\" {{print $2}}' | sort -r", disk))
        .output()
        .context("Failed to scan mounts")?;
    
    let mounts: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if mounts.is_empty() {
        info!("✅ No mounted partitions");
        return Ok(());
    }

    for mount in mounts {
        info!("Unmounting: {}", mount);
        let status = Command::new("sudo")
            .args(["umount", &mount])
            .status()
            .context("Failed to unmount")?;
        
        if !status.success() {
            warn!("Failed to unmount {}, trying force...", mount);
            Command::new("sudo")
                .args(["umount", "-l", &mount])
                .status()
                .context("Failed to force unmount")?;
        }
    }
    
    info!("✅ All partitions unmounted");
    Ok(())
}

fn partition_disk_mbr(config: &FlashConfig) -> Result<()> {
    info!("🔧 Creating MBR partition table (4 partitions for MASH)");
    
    // Wipe existing partition table
    info!("Wiping existing data...");
    run_cmd(&["sudo", "wipefs", "-a", &config.disk])?;
    
    // Create MBR partition table
    info!("Creating MBR partition table...");
    run_cmd(&["sudo", "parted", "-s", &config.disk, "mklabel", "msdos"])?;
    
    // Partition layout:
    // p1: EFI  - 1MB to 1GB+1MB        (FAT32, boot flag)
    // p2: BOOT - 1GB+1MB to 3GB+1MB    (ext4)
    // p3: ROOT - 3GB+1MB to 1800GB     (btrfs, LABEL=FEDORA)
    // p4: DATA - 1800GB to 100%        (ext4, LABEL=DATA)
    
    info!("Creating partitions:");
    info!("  EFI:  1MiB - {}      (FAT32, boot)", EFI_SIZE_MB);
    info!("  BOOT: {}  - {}       (ext4)", EFI_SIZE_MB, BOOT_SIZE_MB);
    info!("  ROOT: {}  - {}       (btrfs, LABEL=FEDORA)", BOOT_SIZE_MB, ROOT_END_GB);
    info!("  DATA: {}  - 100%     (ext4, LABEL=DATA)", ROOT_END_GB);
    
    // P1: EFI
    run_cmd(&["sudo", "parted", "-s", &config.disk, "mkpart", "primary", "fat32",
               "1MiB", EFI_SIZE_MB])?;
    
    // P2: BOOT  
    run_cmd(&["sudo", "parted", "-s", &config.disk, "mkpart", "primary", "ext4",
               EFI_SIZE_MB, &format!("{}+{}", EFI_SIZE_MB, BOOT_SIZE_MB)])?;
    
    // P3: ROOT (btrfs)
    let boot_end = format!("{}+{}", EFI_SIZE_MB, BOOT_SIZE_MB);
    run_cmd(&["sudo", "parted", "-s", &config.disk, "mkpart", "primary", "btrfs",
               &boot_end, ROOT_END_GB])?;
    
    // P4: DATA
    run_cmd(&["sudo", "parted", "-s", &config.disk, "mkpart", "primary", "ext4",
               ROOT_END_GB, "100%"])?;
    
    // Set boot flag on EFI partition
    run_cmd(&["sudo", "parted", "-s", &config.disk, "set", "1", "boot", "on"])?;
    
    // Wait for kernel to recognize partitions
    run_cmd(&["sudo", "partprobe", &config.disk])?;
    std::thread::sleep(std::time::Duration::from_secs(3));
    
    info!("✅ Partitioning complete");
    show_lsblk(&config.disk)?;
    
    Ok(())
}

fn format_partitions(config: &FlashConfig) -> Result<()> {
    info!("💾 Formatting partitions");
    
    let p1 = format!("{}1", config.disk);
    let p2 = format!("{}2", config.disk);
    let p3 = format!("{}3", config.disk);
    let p4 = format!("{}4", config.disk);
    
    // Format EFI partition as FAT32
    info!("Formatting EFI partition (FAT32)...");
    run_cmd(&["sudo", "mkfs.vfat", "-F", "32", "-n", "EFI", &p1])?;
    
    // Format BOOT partition as ext4
    info!("Formatting BOOT partition (ext4)...");
    run_cmd(&["sudo", "mkfs.ext4", "-F", "-L", "BOOT", &p2])?;
    
    // Format ROOT partition as btrfs with subvolumes
    info!("Formatting ROOT partition (btrfs with subvolumes)...");
    run_cmd(&["sudo", "mkfs.btrfs", "-f", "-L", "FEDORA", &p3])?;
    
    // Mount and create btrfs subvolumes
    let mnt_btrfs = PathBuf::from("/tmp/mash_btrfs");
    fs::create_dir_all(&mnt_btrfs)?;
    run_cmd(&["sudo", "mount", &p3, mnt_btrfs.to_str().unwrap()])?;
    
    info!("Creating btrfs subvolumes...");
    run_cmd(&["sudo", "btrfs", "subvolume", "create", &format!("{}/root", mnt_btrfs.display())])?;
    run_cmd(&["sudo", "btrfs", "subvolume", "create", &format!("{}/home", mnt_btrfs.display())])?;
    
    run_cmd(&["sudo", "umount", mnt_btrfs.to_str().unwrap()])?;
    
    // Format DATA partition as ext4
    info!("Formatting DATA partition (ext4)...");
    run_cmd(&["sudo", "mkfs.ext4", "-F", "-L", "DATA", &p4])?;
    
    info!("✅ Formatting complete");
    Ok(())
}

fn install_system_from_loop(config: &FlashConfig) -> Result<()> {
    info!("📦 Installing system from loop-mounted image");
    
    let p3 = format!("{}3", config.disk);
    
    // Create mount points
    let loop_mount = PathBuf::from("/tmp/mash_loop");
    let root_mount = PathBuf::from("/tmp/mash_root");
    
    fs::create_dir_all(&loop_mount)?;
    fs::create_dir_all(&root_mount)?;
    
    // Setup loop device for image
    info!("Setting up loop device for image...");
    let output = Command::new("sudo")
        .args(["losetup", "-f", "--show", "-P", config.image.to_str().unwrap()])
        .output()
        .context("Failed to setup loop device")?;
    
    let loop_dev = String::from_utf8_lossy(&output.stdout).trim().to_string();
    info!("Loop device: {}", loop_dev);
    
    // Wait for partition nodes to appear
    std::thread::sleep(std::time::Duration::from_secs(2));
    
    // Find the root partition in the loop image (typically p3 for Fedora)
    let image_root_part = format!("{}p3", loop_dev);
    
    // Mount image root (read-only)
    info!("Mounting image root partition (read-only)...");
    run_cmd(&["sudo", "mount", "-o", "ro", &image_root_part, loop_mount.to_str().unwrap()])?;
    
    // Mount target root (btrfs subvol=root)
    info!("Mounting target root partition (btrfs subvol=root)...");
    run_cmd(&["sudo", "mount", "-o", "subvol=root", &p3, root_mount.to_str().unwrap()])?;
    
    // Copy system with rsync
    info!("Copying system (this will take several minutes)...");
    info!("Excluding: /dev, /proc, /sys, /tmp, /run, /mnt, /media");
    
    let status = Command::new("sudo")
        .args([
            "rsync", "-aAXHv", "--info=progress2",
            "--exclude=/dev/*",
            "--exclude=/proc/*",
            "--exclude=/sys/*",
            "--exclude=/tmp/*",
            "--exclude=/run/*",
            "--exclude=/mnt/*",
            "--exclude=/media/*",
            "--exclude=/lost+found",
            &format!("{}/", loop_mount.display()),
            &format!("{}/", root_mount.display()),
        ])
        .status()
        .context("Failed to rsync system")?;
    
    if !status.success() {
        warn!("rsync reported warnings (may be non-fatal)");
    }
    
    // Cleanup loop mounts
    info!("Cleaning up loop mounts...");
    run_cmd(&["sudo", "umount", loop_mount.to_str().unwrap()]).ok();
    run_cmd(&["sudo", "losetup", "-d", &loop_dev]).ok();
    
    // Keep root mounted for next steps
    info!("✅ System installation complete");
    Ok(())
}

fn configure_uefi_boot(config: &FlashConfig) -> Result<()> {
    info!("⚙️  Configuring UEFI boot");
    
    let p1 = format!("{}1", config.disk);
    let p2 = format!("{}2", config.disk);
    let p3 = format!("{}3", config.disk);
    
    let root_mount = PathBuf::from("/tmp/mash_root");
    let boot_mount = root_mount.join("boot");
    let efi_mount = boot_mount.join("efi");
    
    // Create mount points
    fs::create_dir_all(&boot_mount)?;
    fs::create_dir_all(&efi_mount)?;
    
    // Mount BOOT and EFI
    run_cmd(&["sudo", "mount", &p2, boot_mount.to_str().unwrap()])?;
    run_cmd(&["sudo", "mount", &p1, efi_mount.to_str().unwrap()])?;
    
    // Copy UEFI firmware files
    info!("Copying UEFI firmware files...");
    run_cmd(&["sudo", "rsync", "-av", 
               &format!("{}/", config.uefi_dir.display()),
               efi_mount.to_str().unwrap()])?;
    
    // Get UUIDs
    let root_uuid = get_uuid(&p3)?;
    let boot_uuid = get_uuid(&p2)?;
    let efi_uuid = get_uuid(&p1)?;
    let data_uuid = get_uuid(&format!("{}4", config.disk))?;
    
    info!("Root UUID: {}", root_uuid);
    
    // Generate fstab
    info!("Generating /etc/fstab...");
    let fstab_content = format!(
        "# /etc/fstab - MASH auto-generated\n\
         UUID={}  /           btrfs   subvol=root,defaults,noatime,compress=zstd  0 0\n\
         UUID={}  /home       btrfs   subvol=home,defaults,noatime,compress=zstd  0 0\n\
         UUID={}  /boot       ext4    defaults,noatime                             0 2\n\
         UUID={}  /boot/efi   vfat    defaults,umask=0077                          0 2\n\
         UUID={}  /data       ext4    defaults,noatime                             0 2\n",
        root_uuid, root_uuid, boot_uuid, efi_uuid, data_uuid
    );
    
    let fstab_path = root_mount.join("etc/fstab");
    fs::write(&fstab_path, fstab_content)
        .context("Failed to write fstab")?;
    
    // Mount necessary pseudo-filesystems for chroot
    info!("Preparing chroot environment...");
    run_cmd(&["sudo", "mount", "--bind", "/dev", &format!("{}/dev", root_mount.display())])?;
    run_cmd(&["sudo", "mount", "--bind", "/proc", &format!("{}/proc", root_mount.display())])?;
    run_cmd(&["sudo", "mount", "--bind", "/sys", &format!("{}/sys", root_mount.display())])?;
    
    // Run dracut to generate initramfs
    info!("Running dracut to generate initramfs...");
    let dracut_cmd = format!(
        "chroot {} /bin/bash -c 'dracut --force --kver $(ls /lib/modules | head -n1)'",
        root_mount.display()
    );
    run_cmd(&["sudo", "bash", "-c", &dracut_cmd])?;
    
    // Generate GRUB config
    info!("Generating GRUB configuration...");
    let grub_cmd = format!(
        "chroot {} /bin/bash -c 'grub2-mkconfig -o /boot/grub2/grub.cfg'",
        root_mount.display()
    );
    run_cmd(&["sudo", "bash", "-c", &grub_cmd])?;
    
    // Install GRUB for ARM64-EFI
    info!("Installing GRUB for ARM64-EFI...");
    let grub_install_cmd = format!(
        "chroot {} /bin/bash -c 'grub2-install --target=arm64-efi --efi-directory=/boot/efi --bootloader-id=fedora --no-nvram'",
        root_mount.display()
    );
    run_cmd(&["sudo", "bash", "-c", &grub_install_cmd])?;
    
    info!("✅ UEFI boot configuration complete");
    Ok(())
}

fn stage_dojo_to_data(config: &FlashConfig) -> Result<()> {
    info!("🥋 Staging Dojo bundle to DATA partition");
    
    let data_mount = PathBuf::from("/tmp/mash_data");
    fs::create_dir_all(&data_mount)?;
    
    // Mount DATA partition by label
    let p4 = format!("{}4", config.disk);
    run_cmd(&["sudo", "mount", &p4, data_mount.to_str().unwrap()])?;
    
    // Create staging directory
    let staging_dir = data_mount.join("mash-staging");
    let logs_dir = data_mount.join("mash-logs");
    
    fs::create_dir_all(&staging_dir)?;
    fs::create_dir_all(&logs_dir)?;
    
    // Look for dojo_bundle in various locations
    let possible_dojo_paths = [
        PathBuf::from("/tmp/dojo_bundle"),
        PathBuf::from("./dojo_bundle"),
        PathBuf::from("../dojo_bundle"),
    ];
    
    let mut dojo_src: Option<PathBuf> = None;
    for path in &possible_dojo_paths {
        if path.exists() && path.is_dir() {
            dojo_src = Some(path.clone());
            break;
        }
    }
    
    if let Some(src) = dojo_src {
        info!("Found dojo_bundle at: {}", src.display());
        run_cmd(&["sudo", "rsync", "-av", 
                   &format!("{}/", src.display()),
                   staging_dir.to_str().unwrap()])?;
    } else {
        warn!("dojo_bundle not found - Dojo will need to be installed manually");
        info!("Expected locations:");
        for path in &possible_dojo_paths {
            info!("  - {}", path.display());
        }
    }
    
    // Unmount DATA
    run_cmd(&["sudo", "umount", data_mount.to_str().unwrap()])?;
    
    info!("✅ Dojo staged to /data/mash-staging");
    Ok(())
}

fn install_offline_boot_units(config: &FlashConfig) -> Result<()> {
    info!("🔧 Installing offline boot units");
    
    let root_mount = PathBuf::from("/tmp/mash_root");
    let systemd_dir = root_mount.join("etc/systemd/system");
    let wants_dir = systemd_dir.join("multi-user.target.wants");
    
    fs::create_dir_all(&wants_dir)?;
    
    // Enable NetworkManager, SDDM, Bluetooth
    let services = vec!["NetworkManager", "sddm", "bluetooth"];
    for service in services {
        let service_name = format!("{}.service", service);
        let service_file = root_mount.join("usr/lib/systemd/system").join(&service_name);
        
        if service_file.exists() {
            let link = wants_dir.join(&service_name);
            if link.exists() || link.is_symlink() {
                fs::remove_file(&link).ok();
            }
            std::os::unix::fs::symlink(
                format!("/usr/lib/systemd/system/{}", service_name),
                link
            ).ok();
            info!("Enabled: {}", service);
        }
    }
    
    info!("✅ Boot units installed");
    Ok(())
}

fn offline_locale_patch(config: &FlashConfig) -> Result<()> {
    info!("🌍 Patching locale settings (en_GB.UTF-8, gb keymap)");
    
    let root_mount = PathBuf::from("/tmp/mash_root");
    let etc_dir = root_mount.join("etc");
    
    // locale.conf
    let locale_conf = etc_dir.join("locale.conf");
    fs::write(&locale_conf, "LANG=en_GB.UTF-8\n")?;
    
    // vconsole.conf
    let vconsole_conf = etc_dir.join("vconsole.conf");
    fs::write(&vconsole_conf, "KEYMAP=gb\n")?;
    
    run_cmd(&["sync"])?;
    info!("✅ Locale patched");
    Ok(())
}

fn final_cleanup(config: &FlashConfig) -> Result<()> {
    info!("🧹 Final cleanup");
    
    let root_mount = PathBuf::from("/tmp/mash_root");
    
    // Unmount pseudo-filesystems
    run_cmd(&["sudo", "umount", &format!("{}/sys", root_mount.display())]).ok();
    run_cmd(&["sudo", "umount", &format!("{}/proc", root_mount.display())]).ok();
    run_cmd(&["sudo", "umount", &format!("{}/dev", root_mount.display())]).ok();
    
    // Unmount everything recursively
    run_cmd(&["sudo", "umount", "-R", root_mount.to_str().unwrap()])?;
    
    // Sync to disk
    run_cmd(&["sync"])?;
    
    info!("✅ Cleanup complete");
    Ok(())
}

fn get_uuid(partition: &str) -> Result<String> {
    let output = Command::new("sudo")
        .args(["blkid", "-s", "UUID", "-o", "value", partition])
        .output()
        .context("Failed to get UUID")?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn run_cmd(args: &[&str]) -> Result<()> {
    let status = Command::new(args[0])
        .args(&args[1..])
        .status()
        .with_context(|| format!("Failed to run: {}", args.join(" ")))?;
    
    if !status.success() {
        bail!("Command failed: {}", args.join(" "));
    }
    
    Ok(())
}

fn confirm(prompt: &str) -> Result<bool> {
    use std::io::{self, Write};
    print!("{} [y/N]: ", prompt);
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().eq_ignore_ascii_case("y") || input.trim().eq_ignore_ascii_case("yes"))
}
