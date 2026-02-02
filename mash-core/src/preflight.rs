use anyhow::{anyhow, bail, Context, Result};
use std::collections::HashMap;
use std::fs;
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

const MIN_RAM_MIB: u64 = 2048;
const MIN_DISK_GIB: u64 = 8;

#[derive(Debug, Clone)]
pub struct PreflightConfig {
    pub min_ram_mib: u64,
    pub min_disk_gib: u64,
    pub require_network: bool,
    pub disk: Option<PathBuf>,
    pub required_binaries: Vec<String>,
    pub os_release_path: PathBuf,
    pub meminfo_path: PathBuf,
    pub mountinfo_path: PathBuf,
    pub sys_block_dir: PathBuf,
    pub path_env: String,
    pub require_dev_prefix: bool,
}

impl Default for PreflightConfig {
    fn default() -> Self {
        let path_env = std::env::var("PATH").unwrap_or_default();
        Self {
            min_ram_mib: MIN_RAM_MIB,
            min_disk_gib: MIN_DISK_GIB,
            require_network: false,
            disk: None,
            required_binaries: vec![
                "dnf".to_string(),
                "mkfs.ext4".to_string(),
                "mkfs.btrfs".to_string(),
            ],
            os_release_path: PathBuf::from("/etc/os-release"),
            meminfo_path: PathBuf::from("/proc/meminfo"),
            mountinfo_path: PathBuf::from("/proc/self/mountinfo"),
            sys_block_dir: PathBuf::from("/sys/class/block"),
            path_env,
            require_dev_prefix: true,
        }
    }
}

pub struct PreflightChecks {
    pub network_check: Box<dyn Fn() -> Result<()> + Send + Sync>,
}

impl PreflightChecks {
    pub fn with_network_check(check: Box<dyn Fn() -> Result<()> + Send + Sync>) -> Self {
        Self {
            network_check: check,
        }
    }
}

impl Default for PreflightChecks {
    fn default() -> Self {
        Self {
            network_check: Box::new(check_network),
        }
    }
}

pub fn run(_dry_run: bool) -> Result<()> {
    let cfg = PreflightConfig::default();
    let checks = PreflightChecks::with_network_check(Box::new(check_network));
    run_with(&cfg, &checks)
}

pub fn run_with(cfg: &PreflightConfig, checks: &PreflightChecks) -> Result<()> {
    log::info!("🧪 Preflight checks");

    check_os_release(cfg)?;
    check_memory(cfg)?;
    check_binaries(cfg)?;

    if let Some(disk) = cfg.disk.as_ref() {
        check_disk(cfg, disk)?;
    }

    if cfg.require_network {
        (checks.network_check)()
            .map_err(|err| anyhow!("Network connectivity required: {}", err))?;
    }

    log::info!("✅ Preflight complete");
    Ok(())
}

fn check_os_release(cfg: &PreflightConfig) -> Result<()> {
    let contents = fs::read_to_string(&cfg.os_release_path)
        .with_context(|| format!("Unable to read {}", cfg.os_release_path.display()))?;
    let data = parse_os_release(&contents);
    let id = data
        .get("ID")
        .ok_or_else(|| anyhow!("OS release file missing ID"))?;
    let version = data
        .get("VERSION_ID")
        .ok_or_else(|| anyhow!("OS release file missing VERSION_ID"))?;

    if id != "fedora" {
        bail!("Unsupported OS: expected Fedora, found {}", id);
    }
    if version.parse::<u32>().is_err() {
        bail!("Unsupported Fedora VERSION_ID: {}", version);
    }
    Ok(())
}

fn check_memory(cfg: &PreflightConfig) -> Result<()> {
    let contents = fs::read_to_string(&cfg.meminfo_path)
        .with_context(|| format!("Unable to read {}", cfg.meminfo_path.display()))?;
    let mem_total_kib = parse_meminfo_kib(&contents)?;
    let required_kib = cfg.min_ram_mib.saturating_mul(1024);
    if mem_total_kib < required_kib {
        bail!(
            "Insufficient RAM: {} MiB available, {} MiB required",
            mem_total_kib / 1024,
            cfg.min_ram_mib
        );
    }
    Ok(())
}

fn check_binaries(cfg: &PreflightConfig) -> Result<()> {
    let mut missing = Vec::new();
    for bin in &cfg.required_binaries {
        if find_executable_in_path(bin, &cfg.path_env).is_none() {
            missing.push(bin.clone());
        }
    }
    if !missing.is_empty() {
        bail!("Missing required binaries on PATH: {}", missing.join(", "));
    }
    Ok(())
}

fn check_disk(cfg: &PreflightConfig, disk: &Path) -> Result<()> {
    if cfg.require_dev_prefix && !disk.starts_with("/dev/") {
        bail!("Disk path must be under /dev: {}", disk.display());
    }
    if !disk.exists() {
        bail!("Disk path does not exist: {}", disk.display());
    }

    let is_block = is_block_device_path(disk, &cfg.sys_block_dir)?;
    if !is_block {
        bail!("Disk path is not a block device: {}", disk.display());
    }

    let size_bytes = device_size_bytes(disk, &cfg.sys_block_dir)?;
    let size_gib = size_bytes as f64 / 1024.0 / 1024.0 / 1024.0;
    if size_gib < cfg.min_disk_gib as f64 {
        bail!(
            "Disk too small: {:.2} GiB available, {} GiB required",
            size_gib,
            cfg.min_disk_gib
        );
    }

    if is_mounted(disk, &cfg.mountinfo_path)? {
        bail!("Disk appears mounted: {}", disk.display());
    }

    Ok(())
}

fn parse_os_release(contents: &str) -> HashMap<String, String> {
    let mut values = HashMap::new();
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.splitn(2, '=');
        if let (Some(key), Some(value)) = (parts.next(), parts.next()) {
            let mut cleaned = value.trim().to_string();
            if cleaned.starts_with('"') && cleaned.ends_with('"') && cleaned.len() >= 2 {
                cleaned = cleaned[1..cleaned.len() - 1].to_string();
            }
            values.insert(key.to_string(), cleaned);
        }
    }
    values
}

fn parse_meminfo_kib(contents: &str) -> Result<u64> {
    for line in contents.lines() {
        if let Some(rest) = line.strip_prefix("MemTotal:") {
            let value = rest.split_whitespace().next();
            if let Some(num) = value {
                let parsed = num
                    .parse::<u64>()
                    .context("Unable to parse MemTotal value")?;
                return Ok(parsed);
            }
        }
    }
    bail!("Unable to read MemTotal from meminfo")
}

fn find_executable_in_path(binary: &str, path_env: &str) -> Option<PathBuf> {
    for dir in path_env.split(':').filter(|dir| !dir.is_empty()) {
        let candidate = Path::new(dir).join(binary);
        if let Ok(metadata) = fs::metadata(&candidate) {
            if metadata.is_file() && metadata.permissions().mode() & 0o111 != 0 {
                return Some(candidate);
            }
        }
    }
    None
}

fn check_network() -> Result<()> {
    let addr = SocketAddr::from((Ipv4Addr::new(1, 1, 1, 1), 53));
    TcpStream::connect_timeout(&addr, Duration::from_secs(2))
        .map(|_| ())
        .context("Unable to reach network")
}

fn is_block_device_path(path: &Path, sys_block_dir: &Path) -> Result<bool> {
    let device_name = resolve_device_name(path)?;
    let size_path = sys_block_dir.join(&device_name).join("size");
    Ok(size_path.exists())
}

fn device_size_bytes(path: &Path, sys_block_dir: &Path) -> Result<u64> {
    let device_name = resolve_device_name(path)?;
    let size_path = sys_block_dir.join(&device_name).join("size");
    let sectors_str = fs::read_to_string(&size_path)
        .with_context(|| format!("Unable to read {}", size_path.display()))?;
    let sectors = sectors_str
        .trim()
        .parse::<u64>()
        .context("Unable to parse disk size sectors")?;
    Ok(sectors.saturating_mul(512))
}

fn resolve_device_name(path: &Path) -> Result<String> {
    let resolved = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let name = resolved
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| anyhow!("Unable to resolve disk device name"))?;
    Ok(name.to_string())
}

fn is_mounted(disk: &Path, mountinfo_path: &Path) -> Result<bool> {
    let contents = fs::read_to_string(mountinfo_path)
        .with_context(|| format!("Unable to read {}", mountinfo_path.display()))?;
    let disk_str = disk.to_string_lossy();
    let disk_str = disk_str.as_ref();

    for line in contents.lines() {
        if let Some(source) = parse_mount_source(line) {
            if source == disk_str || source.starts_with(disk_str) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn parse_mount_source(line: &str) -> Option<String> {
    let mut parts = line.split(" - ");
    let _pre = parts.next()?;
    let post = parts.next()?;
    let mut post_parts = post.split_whitespace();
    let _fstype = post_parts.next()?;
    let source = post_parts.next()?;
    Some(source.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn write_file(path: &Path, contents: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    fn create_exec(path: &Path) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "#!/bin/true").unwrap();
        let mut perms = fs::metadata(path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).unwrap();
    }

    fn base_config(tmp: &Path) -> PreflightConfig {
        let os_release = tmp.join("etc/os-release");
        write_file(&os_release, "ID=fedora\nVERSION_ID=40\n");

        let meminfo = tmp.join("proc/meminfo");
        write_file(&meminfo, "MemTotal:       4194304 kB\n");

        let mountinfo = tmp.join("proc/self/mountinfo");
        write_file(&mountinfo, "29 0 8:1 / / rw,relatime - ext4 /dev/root rw\n");

        let sys_block = tmp.join("sys/class/block");
        let size_path = sys_block.join("sda/size");
        write_file(&size_path, "20971520\n");

        let bin_dir = tmp.join("bin");
        create_exec(&bin_dir.join("dnf"));
        create_exec(&bin_dir.join("mkfs.ext4"));
        create_exec(&bin_dir.join("mkfs.btrfs"));

        let dev_path = tmp.join("dev/sda");
        write_file(&dev_path, "");

        PreflightConfig {
            min_ram_mib: 2048,
            min_disk_gib: 8,
            require_network: false,
            disk: Some(dev_path),
            required_binaries: vec![
                "dnf".to_string(),
                "mkfs.ext4".to_string(),
                "mkfs.btrfs".to_string(),
            ],
            os_release_path: os_release,
            meminfo_path: meminfo,
            mountinfo_path: mountinfo,
            sys_block_dir: sys_block,
            path_env: bin_dir.to_string_lossy().to_string(),
            require_dev_prefix: false,
        }
    }

    #[test]
    fn fails_on_missing_binary() {
        let tmp = tempdir().unwrap();
        let mut cfg = base_config(tmp.path());
        cfg.required_binaries.push("missing".to_string());

        let checks = PreflightChecks::with_network_check(Box::new(|| Ok(())));
        let err = run_with(&cfg, &checks).unwrap_err();
        assert!(err.to_string().contains("Missing required binaries"));
    }

    #[test]
    fn fails_on_low_memory() {
        let tmp = tempdir().unwrap();
        let cfg = {
            let cfg = base_config(tmp.path());
            write_file(&cfg.meminfo_path, "MemTotal:       1024 kB\n");
            cfg
        };

        let checks = PreflightChecks::with_network_check(Box::new(|| Ok(())));
        let err = run_with(&cfg, &checks).unwrap_err();
        assert!(err.to_string().contains("Insufficient RAM"));
    }

    #[test]
    fn fails_on_wrong_os() {
        let tmp = tempdir().unwrap();
        let cfg = {
            let cfg = base_config(tmp.path());
            write_file(&cfg.os_release_path, "ID=ubuntu\nVERSION_ID=22\n");
            cfg
        };

        let checks = PreflightChecks::with_network_check(Box::new(|| Ok(())));
        let err = run_with(&cfg, &checks).unwrap_err();
        assert!(err.to_string().contains("Unsupported OS"));
    }

    #[test]
    fn fails_on_mounted_disk() {
        let tmp = tempdir().unwrap();
        let cfg = {
            let cfg = base_config(tmp.path());
            let source = format!(
                "29 0 8:1 / / rw,relatime - ext4 {}/dev/sda1 rw\n",
                tmp.path().display()
            );
            write_file(&cfg.mountinfo_path, &source);
            cfg
        };

        let checks = PreflightChecks::with_network_check(Box::new(|| Ok(())));
        let err = run_with(&cfg, &checks).unwrap_err();
        assert!(err.to_string().contains("Disk appears mounted"));
    }

    #[test]
    fn passes_with_valid_inputs() {
        let tmp = tempdir().unwrap();
        let cfg = base_config(tmp.path());

        let checks = PreflightChecks::with_network_check(Box::new(|| Ok(())));
        run_with(&cfg, &checks).unwrap();
    }
}
