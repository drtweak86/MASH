//! Core logic for MASH, ported from scripts/mash-full-loop.py.

use anyhow::{anyhow, Result};
use clap::{Parser, ValueEnum};
use regex::Regex;
use std::path::{Path, PathBuf};
use std::process::Command;

// ---- Helpers (mash-full-loop.py:55-214) ----

/// Corresponds to `sh` helper (mash-full-loop.py:55-72).
pub fn sh(cmd: &str) -> Result<String> {
    let output = Command::new("sh").arg("-c").arg(cmd).output()?;
    if !output.status.success() {
        return Err(anyhow!(
            "command failed (status {}): {}",
            output.status,
            cmd
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Corresponds to `need` helper (mash-full-loop.py:75-78).
pub fn need(binname: &str) -> Result<()> {
    let output = Command::new("which").arg(binname).output()?;
    if !output.status.success() {
        return Err(anyhow!("Missing required command: {}", binname));
    }
    Ok(())
}

/// Corresponds to `die` helper (mash-full-loop.py:80-82).
pub fn die(msg: &str, code: i32) -> ! {
    eprintln!("\n[FATAL] {}\n", msg);
    std::process::exit(code);
}

/// Corresponds to `banner` helper (mash-full-loop.py:85-89).
pub fn banner(title: &str) -> Result<()> {
    let line = "─".repeat(title.chars().count() + 4);
    println!("\n╭{}╮", line);
    println!("│  {}  │", title);
    println!("╰{}╯", line);
    Ok(())
}

/// Corresponds to `mkdirp` helper (mash-full-loop.py:92-93).
pub fn mkdirp(_path: &Path) -> Result<()> {
    todo!("Implement mkdir -p, corresponds to mash-full-loop.py:92-93");
}

/// Corresponds to `umount` helper (mash-full-loop.py:96-97).
pub fn umount(_path: &Path) -> Result<()> {
    todo!("Implement recursive unmount, corresponds to mash-full-loop.py:96-97");
}

/// Corresponds to `udev_settle` helper (mash-full-loop.py:100-101).
pub fn udev_settle() -> Result<()> {
    todo!("Implement udev settle, corresponds to mash-full-loop.py:100-101");
}

/// Corresponds to `lsblk_tree` helper (mash-full-loop.py:104-105).
pub fn lsblk_tree(_disk: &str) -> Result<()> {
    let cmd = format!("lsblk -o NAME,SIZE,TYPE,FSTYPE,MOUNTPOINTS,MODEL {}", _disk);
    let _ = sh(&cmd)?;
    Ok(())
}

/// Corresponds to `blkid_uuid` helper (mash-full-loop.py:108-109).
pub fn blkid_uuid(_dev: &str) -> Result<String> {
    let cmd = format!("blkid -s UUID -o value {}", _dev);
    let output = sh(&cmd)?;
    Ok(output)
}

/// Corresponds to `parse_size_to_mib` helper (mash-full-loop.py:112-119).
pub fn parse_size_to_mib(size: &str) -> Result<u64> {
    let re = Regex::new(r"^(\d+)\s*(MiB|GiB)$")?;
    let caps = re
        .captures(size.trim())
        .ok_or_else(|| anyhow!("Size must be like 1024MiB or 2GiB, got: {}", size))?;
    let value: u64 = caps[1].parse()?;
    let unit = caps[2].to_lowercase();
    Ok(if unit == "mib" { value } else { value * 1024 })
}

/// Corresponds to `rsync_progress` helper (mash-full-loop.py:122-153).
pub fn rsync_progress(_src: &Path, _dst: &Path, _desc: &str) -> Result<()> {
    todo!("Implement rsync with progress, corresponds to mash-full-loop.py:122-153");
}

/// Corresponds to `rsync_vfat_safe` helper (mash-full-loop.py:156-162).
pub fn rsync_vfat_safe(_src: &Path, _dst: &Path, _desc: &str) -> Result<()> {
    todo!("Implement vfat-safe rsync, corresponds to mash-full-loop.py:156-162");
}

/// Corresponds to `write_file` helper (mash-full-loop.py:165-166).
pub fn write_file(_path: &Path, _content: &str) -> Result<()> {
    todo!("Implement write file, corresponds to mash-full-loop.py:165-166");
}

/// Corresponds to `patch_bls_entries` helper (mash-full-loop.py:169-213).
pub fn patch_bls_entries(_boot_entries_dir: &Path, _root_uuid: &str) -> Result<()> {
    todo!("Implement BLS patching, corresponds to mash-full-loop.py:169-213");
}

// ---- Main flow (mash-full-loop.py:216-545) ----

#[derive(Debug, Clone, ValueEnum)]
pub enum Scheme {
    Gpt,
    Mbr,
}

#[derive(Debug, Parser)]
pub struct Args {
    /// Fedora *.raw image
    pub image: PathBuf,

    /// Target disk (MASH)
    #[arg(long, default_value = "/dev/sda")]
    pub disk: String,

    /// PFTF UEFI dir (must contain RPI_EFI.fd)
    #[arg(long, default_value = "./rpi4uefi")]
    pub uefi_dir: PathBuf,

    /// Partition scheme (gpt recommended)
    #[arg(long, value_enum, default_value_t = Scheme::Gpt)]
    pub scheme: Scheme,

    /// Create /data partition (GPT recommended)
    #[arg(long)]
    pub make_data: bool,

    /// EFI size (default 1024MiB)
    #[arg(long, default_value = "1024MiB")]
    pub efi_size: String,

    /// /boot size (default 2048MiB)
    #[arg(long, default_value = "2048MiB")]
    pub boot_size: String,

    /// MBR-only: end of ROOT partition (default 1800GiB)
    #[arg(long, default_value = "1800GiB")]
    pub mbr_root_end: String,

    /// Skip dracut (not recommended)
    #[arg(long)]
    pub no_dracut: bool,
}

/// Corresponds to argument parsing and validation (mash-full-loop.py:220-245).
pub fn parse_args_and_validate() -> Result<Args> {
    let args = Args::parse();

    if unsafe { libc::geteuid() } != 0 {
        die("Run as root: sudo ./holy-loop-fedora-ninja-final.py ...", 1);
    }

    if !args.image.exists() {
        return Err(anyhow!("Image not found: {}", args.image.display()));
    }

    if !Path::new(&args.disk).exists() {
        return Err(anyhow!("Disk not found: {}", args.disk));
    }

    let uefi_firmware = args.uefi_dir.join("RPI_EFI.fd");
    if !uefi_firmware.exists() {
        return Err(anyhow!("Missing {}/RPI_EFI.fd", args.uefi_dir.display()));
    }

    Ok(args)
}

#[derive(Debug, Clone, Copy)]
pub struct BtrfsSubvolsInfo {
    pub has_root: bool,
    pub has_home: bool,
    pub has_var: bool,
}

/// Corresponds to btrfs subvolume detection (mash-full-loop.py:363-367).
pub fn read_btrfs_subvols(path: &Path) -> Result<BtrfsSubvolsInfo> {
    let output = sh(&format!("btrfs subvolume list {}", path.display()))?;
    let root_re = Regex::new(r"\bpath\s+root$")?;
    let home_re = Regex::new(r"\bpath\s+home$")?;
    let var_re = Regex::new(r"\bpath\s+var$")?;

    let mut info = BtrfsSubvolsInfo {
        has_root: false,
        has_home: false,
        has_var: false,
    };

    for line in output.lines() {
        if root_re.is_match(line) {
            info.has_root = true;
        }
        if home_re.is_match(line) {
            info.has_home = true;
        }
        if var_re.is_match(line) {
            info.has_var = true;
        }
    }

    Ok(info)
}

/// Corresponds to cleanup routine (mash-full-loop.py:252-283).
pub fn cleanup_mounts_and_loopdev() -> Result<()> {
    todo!("Implement cleanup routine, corresponds to mash-full-loop.py:252-283");
}

/// Corresponds to safety check banner + pause (mash-full-loop.py:285-289).
pub fn safety_check_pause() -> Result<()> {
    todo!("Implement safety check pause, corresponds to mash-full-loop.py:285-289");
}

/// Corresponds to unmounting target disk (mash-full-loop.py:291-297).
pub fn unmount_target_disk() -> Result<()> {
    todo!("Implement unmount target disk, corresponds to mash-full-loop.py:291-297");
}

/// Corresponds to wiping signatures (mash-full-loop.py:299-302).
pub fn wipe_signatures() -> Result<()> {
    todo!("Implement wipefs, corresponds to mash-full-loop.py:299-302");
}

/// Corresponds to partitioning logic (mash-full-loop.py:303-337).
pub fn partition_disk() -> Result<()> {
    todo!("Implement partitioning, corresponds to mash-full-loop.py:303-337");
}

/// Corresponds to filesystem formatting (mash-full-loop.py:343-351).
pub fn format_filesystems() -> Result<()> {
    todo!("Implement formatting, corresponds to mash-full-loop.py:343-351");
}

/// Corresponds to loop-mounting the image (mash-full-loop.py:354-361).
pub fn loop_mount_image() -> Result<()> {
    todo!("Implement loop mount, corresponds to mash-full-loop.py:354-361");
}

/// Corresponds to mounting image partitions (mash-full-loop.py:363-387).
pub fn mount_image_partitions() -> Result<()> {
    todo!("Implement image mounts, corresponds to mash-full-loop.py:363-387");
}

/// Corresponds to mounting destination partitions (mash-full-loop.py:389-399).
pub fn mount_destination_partitions() -> Result<()> {
    todo!("Implement destination mounts, corresponds to mash-full-loop.py:389-399");
}

/// Corresponds to creating destination btrfs subvolumes (mash-full-loop.py:401-407).
pub fn create_destination_subvols() -> Result<()> {
    todo!("Implement subvolume creation, corresponds to mash-full-loop.py:401-407");
}

/// Corresponds to mounting destination subvols (mash-full-loop.py:409-414).
pub fn mount_destination_subvols() -> Result<()> {
    todo!("Implement destination subvol mounts, corresponds to mash-full-loop.py:409-414");
}

/// Corresponds to copying root subvolumes (mash-full-loop.py:416-421).
pub fn copy_root_subvols() -> Result<()> {
    todo!("Implement root subvol rsync, corresponds to mash-full-loop.py:416-421");
}

/// Corresponds to copying /boot partition (mash-full-loop.py:423-424).
pub fn copy_boot_partition() -> Result<()> {
    todo!("Implement boot rsync, corresponds to mash-full-loop.py:423-424");
}

/// Corresponds to binding /boot and /boot/efi inside target root (mash-full-loop.py:426-434).
pub fn bind_mount_boot_inside_root() -> Result<()> {
    todo!("Implement boot bind mounts, corresponds to mash-full-loop.py:426-434");
}

/// Corresponds to installing Fedora EFI loaders (mash-full-loop.py:436-439).
pub fn install_fedora_efi_loaders() -> Result<()> {
    todo!("Implement Fedora EFI copy, corresponds to mash-full-loop.py:436-439");
}

/// Corresponds to installing PFTF UEFI firmware (mash-full-loop.py:441-444).
pub fn install_pftf_uefi() -> Result<()> {
    todo!("Implement PFTF UEFI copy, corresponds to mash-full-loop.py:441-444");
}

/// Corresponds to writing config.txt (mash-full-loop.py:446-463).
pub fn write_uefi_config_txt() -> Result<()> {
    todo!("Implement config.txt write, corresponds to mash-full-loop.py:446-463");
}

/// Corresponds to writing UUID-based /etc/fstab (mash-full-loop.py:464-485).
pub fn write_fstab() -> Result<()> {
    todo!("Implement fstab write, corresponds to mash-full-loop.py:464-485");
}

/// Corresponds to patching BLS entries (mash-full-loop.py:486-489).
pub fn patch_bls() -> Result<()> {
    todo!("Implement BLS patch call, corresponds to mash-full-loop.py:486-489");
}

/// Corresponds to dracut chroot flow (mash-full-loop.py:491-519).
pub fn dracut_in_chroot() -> Result<()> {
    todo!("Implement dracut in chroot, corresponds to mash-full-loop.py:491-519");
}

/// Corresponds to final sanity checks (mash-full-loop.py:520-533).
pub fn final_sanity_checks() -> Result<()> {
    todo!("Implement final sanity checks, corresponds to mash-full-loop.py:520-533");
}

/// Corresponds to done banner and next steps output (mash-full-loop.py:534-541).
pub fn print_completion_summary() -> Result<()> {
    todo!("Implement completion summary, corresponds to mash-full-loop.py:534-541");
}
