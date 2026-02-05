use anyhow::{anyhow, Context, Result};
use libc::statvfs;
use log::info;
use mash_hal::{os_release, procfs, sysfs, HostInfoOps, LinuxHal};
use std::env;
use std::ffi::CString;
use std::fs;
use std::net::{TcpStream, ToSocketAddrs};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::FileTypeExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

const MIN_RAM_MB: u64 = 4096;
const MIN_DISK_GB: u64 = 16;
const NETWORK_TIMEOUT_SECS: u64 = 3;
const SYS_BLOCK: &str = "/sys/class/block";

#[cfg(test)]
const TEST_MIN_DISK_ENV: &str = "MASH_TEST_MIN_DISK_GB";

fn min_disk_requirement() -> u64 {
    #[cfg(test)]
    {
        if let Ok(value) = std::env::var(TEST_MIN_DISK_ENV) {
            if let Ok(parsed) = value.parse::<u64>() {
                return parsed;
            }
        }
    }
    MIN_DISK_GB
}

#[derive(Clone, Debug)]
pub struct PreflightConfig {
    pub min_ram_mb: u64,
    pub min_disk_gb: u64,
    pub min_target_disk_gb: u64,
    pub disk_space_path: PathBuf,
    pub target_disk: Option<PathBuf>,
    pub network_endpoint: Option<(String, u16)>,
    pub required_binaries: Vec<String>,
}

impl Default for PreflightConfig {
    fn default() -> Self {
        Self {
            min_ram_mb: MIN_RAM_MB,
            min_disk_gb: min_disk_requirement(),
            min_target_disk_gb: MIN_DISK_GB,
            disk_space_path: PathBuf::from("/"),
            target_disk: None,
            network_endpoint: Some(("kde.fedoraproject.org".to_string(), 443)),
            required_binaries: vec![
                "dnf".to_string(),
                "mkfs.ext4".to_string(),
                "mkfs.btrfs".to_string(),
                "mount".to_string(),
                "rsync".to_string(),
                "systemctl".to_string(),
            ],
        }
    }
}

impl PreflightConfig {
    pub fn for_install(
        disk: Option<PathBuf>,
        requires_network: bool,
        required_binaries: Vec<String>,
    ) -> Self {
        let mut cfg = Self::default();
        if let Some(target) = disk {
            cfg.target_disk = Some(target);
        }
        if !requires_network {
            cfg.network_endpoint = None;
        }
        // The pipeline computes the exact set of binaries required for the selected stages.
        // An empty list explicitly means "no binary requirements".
        cfg.required_binaries = required_binaries;
        cfg
    }
}

pub fn run(cfg: &PreflightConfig) -> Result<()> {
    info!("🧪 Preflight checks");

    let hal = LinuxHal::new();

    check_ram(&hal, cfg.min_ram_mb)?;
    check_disk_space(&cfg.disk_space_path, cfg.min_disk_gb)?;
    if let Some(target_disk) = &cfg.target_disk {
        check_target_disk(&hal, target_disk, cfg.min_target_disk_gb)?;
    }
    check_os_release(&hal)?;
    if let Some((_, _)) = &cfg.network_endpoint {
        if std::env::var_os("MASH_TEST_SKIP_NETWORK_CHECK").is_some() {
            info!("Skipping network check (MASH_TEST_SKIP_NETWORK_CHECK)");
        } else {
            check_network(cfg)?;
        }
    }
    check_binaries(&cfg.required_binaries)?;

    info!("✅ Preflight complete");
    Ok(())
}

fn check_ram(hal: &dyn HostInfoOps, min_mb: u64) -> Result<()> {
    let content = hal
        .proc_meminfo()
        .context("failed to read /proc/meminfo for RAM check")?;
    let available_kb = procfs::meminfo::parse_mem_available_kb(&content)
        .ok_or_else(|| anyhow!("failed to determine available RAM"))?;
    let available_mb = available_kb / 1024;
    if available_mb < min_mb {
        anyhow::bail!(
            "Insufficient RAM: {} MiB available ({} MiB required)",
            available_mb,
            min_mb
        );
    }
    Ok(())
}

fn check_disk_space(path: &Path, min_gb: u64) -> Result<()> {
    let available = available_bytes(path)?;
    let available_gb = available as f64 / (1024.0 * 1024.0 * 1024.0);
    if available_gb < min_gb as f64 {
        anyhow::bail!(
            "Insufficient disk space at {}: {:.1} GiB available ({} GiB required)",
            path.display(),
            available_gb,
            min_gb
        );
    }
    Ok(())
}

fn check_target_disk(hal: &dyn HostInfoOps, path: &Path, min_target_disk_gb: u64) -> Result<()> {
    let metadata = fs::metadata(path)
        .with_context(|| format!("target disk {} not accessible", path.display()))?;
    if !metadata.file_type().is_block_device() {
        anyhow::bail!(
            "Target disk {} is not a block device; please provide the correct device path",
            path.display()
        );
    }

    // Reject partitions when a full-disk device is expected (e.g. /dev/sda not /dev/sda1).
    let name = sysfs::block::device_basename(path)?;
    let sys_path = Path::new(SYS_BLOCK).join(&name);
    if !sys_path.exists() {
        anyhow::bail!(
            "Target disk {} is not recognized in {}; is this a valid /dev block device name?",
            path.display(),
            SYS_BLOCK
        );
    }
    if sys_path.join("partition").exists() {
        anyhow::bail!(
            "Target disk {} appears to be a partition; please pass the whole disk (e.g. /dev/sda, not /dev/sda1)",
            path.display()
        );
    }

    // Fail fast if anything from this disk is currently mounted.
    let mountinfo = hal
        .proc_mountinfo()
        .context("failed to read /proc/self/mountinfo")?;
    let mounted = procfs::mountinfo::mounted_under_device(&mountinfo, path);
    if !mounted.is_empty() {
        anyhow::bail!(
            "Target disk {} has mounted filesystems: {}. Unmount them before continuing.",
            path.display(),
            mounted.join(", ")
        );
    }

    // Avoid clobbering the host system disk (rootfs / boot media).
    // This can only be overridden explicitly in developer mode.
    if std::env::var_os("MASH_DEVELOPER_MODE").is_none() {
        if let Some(root_source) = procfs::mountinfo::root_mount_source(&mountinfo) {
            if let (Some(root_disk), Some(target_disk)) = (
                base_block_device(&root_source),
                base_block_device(&path.to_string_lossy()),
            ) {
                if root_disk == target_disk {
                    anyhow::bail!(
                        "Target disk {} is the current root/boot media ({}); refusing to continue.",
                        path.display(),
                        root_disk
                    );
                }
            }
        }
    }

    // Minimum target device capacity check (read-only, via sysfs).
    let size_bytes = sysfs::block::block_device_size_bytes(&sys_path)
        .with_context(|| format!("failed to read size for {}", path.display()))?;
    let size_gib = size_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
    if size_gib < min_target_disk_gb as f64 {
        anyhow::bail!(
            "Target disk {} is too small: {:.1} GiB ({} GiB required)",
            path.display(),
            size_gib,
            min_target_disk_gb
        );
    }
    Ok(())
}

fn base_block_device(device: &str) -> Option<String> {
    if !device.starts_with("/dev/") {
        return None;
    }
    let name = device.trim_start_matches("/dev/");
    let base = if name.starts_with("nvme") || name.starts_with("mmcblk") || name.starts_with("loop")
    {
        if let Some(idx) = name.rfind('p') {
            let suffix = &name[idx + 1..];
            if !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit()) {
                name[..idx].to_string()
            } else {
                name.to_string()
            }
        } else {
            name.to_string()
        }
    } else {
        let trimmed = name.trim_end_matches(|c: char| c.is_ascii_digit());
        if trimmed.is_empty() {
            name.to_string()
        } else {
            trimmed.to_string()
        }
    };
    Some(format!("/dev/{}", base))
}

fn check_os_release(hal: &dyn HostInfoOps) -> Result<()> {
    // CI/test runs often happen on non-Fedora hosts; allow tests to point preflight at a fixture.
    // In production this defaults to `/etc/os-release`.
    let os_release_path = env::var_os("MASH_OS_RELEASE_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/etc/os-release"));

    let (id, version) = if os_release_path == Path::new("/etc/os-release") {
        let info = hal.os_release()?;
        (
            info.id.unwrap_or_default(),
            info.version_id.and_then(|v| v.parse::<u32>().ok()),
        )
    } else {
        let content = fs::read_to_string(&os_release_path).with_context(|| {
            format!(
                "failed to read {} for Fedora verification",
                os_release_path.display()
            )
        })?;
        os_release::parse_os_release(&content)?
    };

    if id != "fedora" {
        anyhow::bail!("Preflight requires Fedora, found {}", id);
    }
    let version = version.ok_or_else(|| anyhow!("Unable to determine Fedora VERSION_ID"))?;
    info!("Fedora VERSION_ID={}", version);
    Ok(())
}

fn check_network(cfg: &PreflightConfig) -> Result<()> {
    if let Some((host, port)) = &cfg.network_endpoint {
        let addr_str = format!("{host}:{port}");
        let addrs = addr_str
            .to_socket_addrs()
            .with_context(|| format!("failed to resolve {}", addr_str))?;
        let timeout = Duration::from_secs(NETWORK_TIMEOUT_SECS);
        for addr in addrs {
            if TcpStream::connect_timeout(&addr, timeout).is_ok() {
                return Ok(());
            }
        }
        anyhow::bail!("Network check to {} timed out", addr_str);
    }
    Ok(())
}

fn check_binaries(bins: &[String]) -> Result<()> {
    let path = env::var_os("PATH").unwrap_or_default();
    let entries = env::split_paths(&path).collect::<Vec<_>>();
    for bin in bins {
        let Some(found) = find_in_paths(bin, &entries) else {
            anyhow::bail!("Required binary '{}' not found in PATH", bin);
        };
        ensure_executable(&found).with_context(|| {
            format!(
                "Required binary '{}' was found at {} but is not executable",
                bin,
                found.display()
            )
        })?;
    }
    Ok(())
}

fn ensure_executable(path: &Path) -> Result<()> {
    let md = fs::metadata(path).with_context(|| format!("failed to stat {}", path.display()))?;
    if !md.is_file() {
        anyhow::bail!("{} is not a regular file", path.display());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = md.permissions().mode();
        if mode & 0o111 == 0 {
            anyhow::bail!("{} is not executable", path.display());
        }
    }
    Ok(())
}

fn find_in_paths(binary: &str, paths: &[PathBuf]) -> Option<PathBuf> {
    for dir in paths {
        let candidate = dir.join(binary);
        if candidate.exists() {
            return Some(candidate);
        }
        #[cfg(unix)]
        {
            let alt = dir.join(format!("{binary}.exe"));
            if alt.exists() {
                return Some(alt);
            }
        }
    }
    None
}

#[allow(clippy::unnecessary_cast)]
fn available_bytes(path: &Path) -> Result<u64> {
    let c_path = CString::new(path.as_os_str().as_bytes())
        .map_err(|_| anyhow!("invalid path for disk space check"))?;
    let mut stat: statvfs = unsafe { std::mem::zeroed() };
    let result = unsafe { statvfs(c_path.as_ptr(), &mut stat) };
    if result != 0 {
        anyhow::bail!("failed to stat filesystem {}", path.display());
    }
    Ok(stat.f_bavail as u64 * stat.f_frsize as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs::File;
    use tempfile::tempdir;

    struct EnvVarGuard<'a> {
        key: &'a str,
        original: Option<std::ffi::OsString>,
    }

    impl<'a> EnvVarGuard<'a> {
        fn new(key: &'a str, value: &std::ffi::OsStr) -> Self {
            let original = env::var_os(key);
            env::set_var(key, value);
            Self { key, original }
        }
    }

    impl Drop for EnvVarGuard<'_> {
        fn drop(&mut self) {
            if let Some(ref original) = self.original {
                env::set_var(self.key, original);
            } else {
                env::remove_var(self.key);
            }
        }
    }

    #[test]
    fn find_in_paths_ignores_missing_binary() {
        let paths = env::split_paths(&env::var_os("PATH").unwrap_or_default())
            .take(1)
            .collect::<Vec<_>>();
        assert!(find_in_paths("unlikely-binary-123", &paths).is_none());
    }

    #[test]
    fn check_binaries_fails_when_missing() {
        let _lock = crate::test_env::lock();
        let tmp = tempdir().unwrap();
        let _guard = EnvVarGuard::new("PATH", tmp.path().as_os_str());
        let err = check_binaries(&["dnf".to_string()]).unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn check_binaries_requires_executable_bit() {
        let _lock = crate::test_env::lock();
        let tmp = tempdir().unwrap();
        let bin_dir = tmp.path().join("bin");
        fs::create_dir_all(&bin_dir).unwrap();
        let bin = bin_dir.join("dnf");
        let _ = File::create(&bin).unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&bin).unwrap().permissions();
            perms.set_mode(0o644);
            fs::set_permissions(&bin, perms).unwrap();
        }

        let _guard = EnvVarGuard::new("PATH", bin_dir.as_os_str());
        let err = check_binaries(&["dnf".to_string()]).unwrap_err();
        assert!(err.to_string().contains("not executable"));
    }

    #[test]
    fn available_bytes_positive() {
        let dir = tempdir().unwrap();
        assert!(available_bytes(dir.path()).unwrap() > 0);
    }

    #[test]
    fn check_target_disk_rejects_non_block() {
        let tmp = tempdir().unwrap();
        let p = tmp.path().join("not-a-block");
        fs::write(&p, "x").unwrap();
        let hal = mash_hal::FakeHal::new();
        let err = check_target_disk(&hal, &p, 1).unwrap_err();
        assert!(err.to_string().contains("not a block device"));
    }
}
