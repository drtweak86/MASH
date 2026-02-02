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

#[derive(Clone, Debug)]
pub struct PreflightConfig {
    pub min_ram_mb: u64,
    pub min_disk_gb: u64,
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
    pub fn for_install(disk: Option<PathBuf>) -> Self {
        let mut cfg = Self::default();
        if let Some(target) = disk {
            cfg.target_disk = Some(target);
        }
        cfg
    }
}

pub fn run(cfg: &PreflightConfig) -> Result<()> {
    info!("🧪 Preflight checks");

    check_ram(cfg.min_ram_mb)?;
    check_disk_space(&cfg.disk_space_path, cfg.min_disk_gb)?;
    if let Some(target_disk) = &cfg.target_disk {
        check_target_disk(target_disk)?;
    }
    check_os_release()?;
    if let Some((_, _)) = &cfg.network_endpoint {
        check_network(cfg)?;
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

fn check_target_disk(path: &Path) -> Result<()> {
    let metadata = fs::metadata(path)
        .with_context(|| format!("target disk {} not accessible", path.display()))?;
    if !metadata.file_type().is_block_device() {
        anyhow::bail!(
            "Target disk {} is not a block device; please provide the correct device path",
            path.display()
        );
    }
    Ok(())
}

fn check_os_release() -> Result<()> {
    let content = fs::read_to_string("/etc/os-release")
        .context("failed to read /etc/os-release for Fedora verification")?;
    let id = parse_os_id(&content).unwrap_or_default();
    if !id.contains("fedora") {
        anyhow::bail!("Preflight requires Fedora, found {}", id);
    }
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
        if find_in_paths(bin, &entries).is_some() {
            continue;
        }
        anyhow::bail!("Required binary '{}' not found in PATH", bin);
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
    use tempfile::tempdir;

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
    fn find_in_paths_ignores_missing_binary() {
        let paths = env::split_paths(&env::var_os("PATH").unwrap_or_default())
            .take(1)
            .collect::<Vec<_>>();
        assert!(find_in_paths("unlikely-binary-123", &paths).is_none());
    }

    #[test]
    fn available_bytes_positive() {
        let dir = tempdir().unwrap();
        assert!(available_bytes(dir.path()).unwrap() > 0);
    }
}
