use anyhow::{anyhow, Context, Result};
use libc::statvfs;
use log::info;
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
            min_disk_gb: MIN_DISK_GB,
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

    check_ram(cfg.min_ram_mb)?;
    check_disk_space(&cfg.disk_space_path, cfg.min_disk_gb)?;
    if let Some(target_disk) = &cfg.target_disk {
        check_target_disk(target_disk, cfg.min_target_disk_gb)?;
    }
    check_os_release()?;
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

fn check_ram(min_mb: u64) -> Result<()> {
    let content = fs::read_to_string("/proc/meminfo")
        .context("failed to read /proc/meminfo for RAM check")?;
    let available_kb = parse_mem_available(&content)
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

fn check_target_disk(path: &Path, min_target_disk_gb: u64) -> Result<()> {
    let metadata = fs::metadata(path)
        .with_context(|| format!("target disk {} not accessible", path.display()))?;
    if !metadata.file_type().is_block_device() {
        anyhow::bail!(
            "Target disk {} is not a block device; please provide the correct device path",
            path.display()
        );
    }

    // Reject partitions when a full-disk device is expected (e.g. /dev/sda not /dev/sda1).
    let name = device_basename(path)?;
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
    let mountinfo = fs::read_to_string("/proc/self/mountinfo")
        .context("failed to read /proc/self/mountinfo")?;
    let mounted = mounted_under_device(&mountinfo, path);
    if !mounted.is_empty() {
        anyhow::bail!(
            "Target disk {} has mounted filesystems: {}. Unmount them before continuing.",
            path.display(),
            mounted.join(", ")
        );
    }

    // Avoid clobbering the host system disk (rootfs).
    if let Some(root_source) = root_mount_source(&mountinfo) {
        if root_source.starts_with(&path.to_string_lossy().to_string()) {
            anyhow::bail!(
                "Target disk {} appears to be the current root device ({}); refusing to continue.",
                path.display(),
                root_source
            );
        }
    }

    // Minimum target device capacity check (read-only, via sysfs).
    let size_bytes = block_device_size_bytes(&sys_path)
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

fn check_os_release() -> Result<()> {
    let content = fs::read_to_string("/etc/os-release")
        .context("failed to read /etc/os-release for Fedora verification")?;
    let (id, version) = parse_os_release(&content)?;
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

fn parse_mem_available(content: &str) -> Option<u64> {
    for line in content.lines() {
        if let Some(value) = line.strip_prefix("MemAvailable:") {
            return value
                .split_whitespace()
                .next()
                .and_then(|num| num.parse().ok());
        }
    }
    for line in content.lines() {
        if let Some(value) = line.strip_prefix("MemTotal:") {
            return value
                .split_whitespace()
                .next()
                .and_then(|num| num.parse().ok());
        }
    }
    None
}

fn parse_os_id(content: &str) -> Option<String> {
    content.lines().find_map(|line| {
        if let Some(value) = line.strip_prefix("ID=") {
            return Some(value.trim().trim_matches('"').to_lowercase());
        }
        if let Some(value) = line.strip_prefix("NAME=") {
            return Some(value.trim().trim_matches('"').to_lowercase());
        }
        None
    })
}

fn parse_os_release(content: &str) -> Result<(String, Option<u32>)> {
    let mut id: Option<String> = None;
    let mut version_id: Option<u32> = None;

    for line in content.lines() {
        if let Some(value) = line.strip_prefix("ID=") {
            id = Some(value.trim().trim_matches('"').to_lowercase());
        } else if let Some(value) = line.strip_prefix("VERSION_ID=") {
            let raw = value.trim().trim_matches('"');
            version_id = raw.parse::<u32>().ok();
        }
    }

    let id = id
        .or_else(|| parse_os_id(content))
        .unwrap_or_else(|| "unknown".to_string());
    Ok((id, version_id))
}

fn device_basename(path: &Path) -> Result<String> {
    let name = path
        .file_name()
        .ok_or_else(|| anyhow!("invalid device path {}", path.display()))?
        .to_string_lossy()
        .to_string();
    Ok(name)
}

fn block_device_size_bytes(sys_path: &Path) -> Result<u64> {
    // /sys/class/block/<dev>/size is in 512-byte sectors.
    let sectors_str = fs::read_to_string(sys_path.join("size"))?;
    let sectors: u64 = sectors_str.trim().parse()?;
    Ok(sectors.saturating_mul(512))
}

fn mounted_under_device(mountinfo: &str, dev_path: &Path) -> Vec<String> {
    let prefix = dev_path.to_string_lossy().to_string();
    let mut mounts = Vec::new();

    for line in mountinfo.lines() {
        // mountinfo format:
        //   <pre fields...> <mount point> <...> - <fstype> <source> <superopts>
        let (pre, post) = match line.split_once(" - ") {
            Some(v) => v,
            None => continue,
        };
        let mut pre_fields = pre.split_whitespace();
        let _mount_id = pre_fields.next();
        let _parent_id = pre_fields.next();
        let _major_minor = pre_fields.next();
        let _root = pre_fields.next();
        let mount_point = match pre_fields.next() {
            Some(v) => unescape_mount_path(v),
            None => continue,
        };
        let mut post_fields = post.split_whitespace();
        let _fstype = post_fields.next();
        let source = match post_fields.next() {
            Some(v) => v,
            None => continue,
        };
        if source.starts_with(&prefix) {
            mounts.push(mount_point);
        }
    }

    mounts.sort();
    mounts.dedup();
    mounts
}

fn root_mount_source(mountinfo: &str) -> Option<String> {
    for line in mountinfo.lines() {
        let (pre, post) = line.split_once(" - ")?;
        let pre_fields: Vec<&str> = pre.split_whitespace().collect();
        if pre_fields.len() < 5 {
            continue;
        }
        let mount_point = unescape_mount_path(pre_fields[4]);
        if mount_point != "/" {
            continue;
        }
        let mut post_fields = post.split_whitespace();
        let _fstype = post_fields.next()?;
        let source = post_fields.next()?.to_string();
        return Some(source);
    }
    None
}

fn unescape_mount_path(raw: &str) -> String {
    raw.replace("\\040", " ")
        .replace("\\011", "\t")
        .replace("\\012", "\n")
        .replace("\\134", "\\")
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
    fn parse_mem_available_prefers_available() {
        let data = "MemTotal: 16384000 kB\nMemAvailable: 8000000 kB\n";
        assert_eq!(parse_mem_available(data), Some(8000000));
    }

    #[test]
    fn parse_mem_available_uses_total_when_available_missing() {
        let data = "MemTotal: 16384000 kB\n";
        assert_eq!(parse_mem_available(data), Some(16384000));
    }

    #[test]
    fn parse_os_id_handles_name() {
        let release = "NAME=\"Fedora Linux\"\nID=fedora\n";
        assert_eq!(parse_os_id(release), Some("fedora linux".to_string()));
    }

    #[test]
    fn parse_os_release_extracts_version_id() {
        let release = "NAME=\"Fedora Linux\"\nID=fedora\nVERSION_ID=\"43\"\n";
        let (id, version) = parse_os_release(release).unwrap();
        assert_eq!(id, "fedora");
        assert_eq!(version, Some(43));
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
    fn mounted_under_device_finds_matching_sources() {
        let mi = "36 28 0:31 / / rw,relatime - ext4 /dev/sda3 rw\n\
                  37 28 0:32 / /mnt/boot rw,relatime - ext4 /dev/sda1 rw\n\
                  38 28 0:33 / /mnt/other rw,relatime - ext4 /dev/sdb1 rw\n";
        let mounts = mounted_under_device(mi, Path::new("/dev/sda"));
        assert_eq!(mounts, vec!["/".to_string(), "/mnt/boot".to_string()]);
    }

    #[test]
    fn root_mount_source_extracts_device() {
        let mi = "36 28 0:31 / / rw,relatime - ext4 /dev/sda3 rw\n";
        assert_eq!(root_mount_source(mi), Some("/dev/sda3".to_string()));
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
        let err = check_target_disk(&p, 1).unwrap_err();
        assert!(err.to_string().contains("not a block device"));
    }
}
