use crate::install_runner::{StageDefinition, StageRunner};
use crate::preflight;
use anyhow::{Context, Result};
use mash_core::downloader;
use mash_core::state_manager::{self, DownloadArtifact};
use mash_core::{boot_config, system_config};
use std::env;
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone)]
pub struct DownloadStageConfig {
    pub enabled: bool,
    pub mirror_override: Option<String>,
    pub checksum_override: Option<String>,
    pub checksum_url: Option<String>,
    pub timeout_secs: u64,
    pub retries: usize,
    pub download_dir: PathBuf,
}

impl DownloadStageConfig {
    fn from_install_config(cfg: &InstallConfig) -> Self {
        Self {
            enabled: cfg.download_image,
            mirror_override: cfg.download_mirror.clone(),
            checksum_override: cfg.download_checksum.clone(),
            checksum_url: cfg.download_checksum_url.clone(),
            timeout_secs: cfg.download_timeout_secs,
            retries: cfg.download_retries,
            download_dir: cfg.download_dir.clone(),
        }
    }
}

#[derive(Clone)]
pub struct DiskStageConfig {
    pub format_ext4: Vec<PathBuf>,
    pub format_btrfs: Vec<PathBuf>,
    pub confirmed: bool,
}

impl DiskStageConfig {
    fn from_install_config(cfg: &InstallConfig) -> Self {
        Self {
            format_ext4: cfg.format_ext4.iter().map(PathBuf::from).collect(),
            format_btrfs: cfg.format_btrfs.iter().map(PathBuf::from).collect(),
            confirmed: cfg.confirmed,
        }
    }
}

#[derive(Clone)]
pub struct BootStageConfig {
    pub enabled: bool,
    pub root: Option<PathBuf>,
    pub mountinfo: Option<PathBuf>,
    pub by_uuid: Option<PathBuf>,
}

impl BootStageConfig {
    fn from_install_config(cfg: &InstallConfig) -> Self {
        Self {
            enabled: cfg.kernel_fix,
            root: cfg.kernel_fix_root.clone(),
            mountinfo: cfg.mountinfo_path.clone(),
            by_uuid: cfg.by_uuid_path.clone(),
        }
    }
}

#[derive(Clone)]
pub struct MountStageConfig {
    pub mounts: Vec<MountSpec>,
}

impl MountStageConfig {
    fn from_install_config(cfg: &InstallConfig) -> Self {
        Self {
            mounts: cfg.mounts.clone(),
        }
    }
}

#[derive(Clone)]
pub struct PackageStageConfig {
    pub packages: Vec<String>,
}

impl PackageStageConfig {
    fn from_install_config(cfg: &InstallConfig) -> Self {
        Self {
            packages: cfg.packages.clone(),
        }
    }
}

#[derive(Clone)]
pub struct ResumeStageConfig {
    pub mash_root: PathBuf,
    pub state_path: PathBuf,
}

impl ResumeStageConfig {
    fn from_install_config(cfg: &InstallConfig) -> Self {
        Self {
            mash_root: cfg.mash_root.clone(),
            state_path: cfg.state_path.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct InstallConfig {
    pub dry_run: bool,
    pub execute: bool,
    pub confirmed: bool,
    pub state_path: PathBuf,
    pub disk: Option<String>,
    pub mounts: Vec<MountSpec>,
    pub format_ext4: Vec<String>,
    pub format_btrfs: Vec<String>,
    pub packages: Vec<String>,
    pub kernel_fix: bool,
    pub kernel_fix_root: Option<PathBuf>,
    pub mountinfo_path: Option<PathBuf>,
    pub by_uuid_path: Option<PathBuf>,
    pub reboot_count: u32,
    pub mash_root: PathBuf,
    pub download_image: bool,
    pub download_uefi: bool,
    pub image_version: String,
    pub image_edition: String,
    pub download_mirror: Option<String>,
    pub download_checksum: Option<String>,
    pub download_checksum_url: Option<String>,
    pub download_timeout_secs: u64,
    pub download_retries: usize,
    pub download_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct MountSpec {
    pub device: String,
    pub target: String,
    pub fstype: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StagePlan {
    pub name: &'static str,
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct InstallPlan {
    pub stages: Vec<StagePlan>,
    pub reboot_count: u32,
}

impl InstallPlan {
    pub fn summary_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        lines.push("Execution plan:".to_string());
        for (idx, stage) in self.stages.iter().enumerate() {
            lines.push(format!(
                "{:02}. {} — {}",
                idx + 1,
                stage.name,
                stage.description
            ));
        }
        lines.push(format!("Reboots: {}", self.reboot_count));
        lines
    }
}

pub fn build_plan(cfg: &InstallConfig) -> InstallPlan {
    let disk = cfg.disk.as_deref().unwrap_or("<unspecified>").to_string();
    let mount_count = cfg.mounts.len();
    let format_count = cfg.format_ext4.len() + cfg.format_btrfs.len();
    let package_count = cfg.packages.len();

    let download_desc = format!(
        "Download Fedora {} {} + optional UEFI to {}",
        cfg.image_version,
        cfg.image_edition,
        cfg.mash_root.join("downloads").display()
    );
    let stages = vec![
        StagePlan {
            name: "Preflight",
            description: "Read-only checks".to_string(),
        },
        StagePlan {
            name: "Download assets",
            description: download_desc,
        },
        StagePlan {
            name: "Disk probe",
            description: format!("Target: {}", disk),
        },
        StagePlan {
            name: "Format plan",
            description: format!("{} format operations", format_count),
        },
        StagePlan {
            name: "Mount plan",
            description: format!("{} mount operations", mount_count),
        },
        StagePlan {
            name: "Package plan",
            description: format!("{} packages", package_count),
        },
        StagePlan {
            name: "Kernel fix check",
            description: if cfg.kernel_fix {
                "USB-root alignment enabled (paths required)".to_string()
            } else {
                "Skipped".to_string()
            },
        },
        StagePlan {
            name: "Resume unit",
            description: "Install resume unit + request reboot (if required)".to_string(),
        },
    ];

    InstallPlan {
        stages,
        reboot_count: cfg.reboot_count,
    }
}

fn run_download_stage(
    state: &mut state_manager::InstallState,
    cfg: &DownloadStageConfig,
    dry_run: bool,
) -> Result<()> {
    if !cfg.enabled {
        log::info!("Download stage skipped; download_image disabled");
        return Ok(());
    }
    if dry_run {
        log::info!("DRY RUN: download stage would fetch Fedora assets");
        return Ok(());
    }

    let opts = downloader::DownloadOptions {
        asset: None,
        image: Some(downloader::ImageKey {
            os: downloader::OsKind::Fedora,
            // This pipeline is currently Fedora-oriented; default to the canonical Fedora entry.
            variant: "kde_mobile_disk".to_string(),
            arch: "aarch64".to_string(),
        }),
        mirror_override: cfg.mirror_override.clone(),
        checksum_override: cfg.checksum_override.clone(),
        checksum_url: cfg.checksum_url.clone(),
        max_retries: cfg.retries,
        timeout_secs: cfg.timeout_secs,
        download_dir: cfg.download_dir.clone(),
        resume: true,
    };
    let artifact = downloader::download(&opts)?;
    state.record_download(DownloadArtifact::new(
        artifact.name.clone(),
        &artifact.path,
        artifact.size,
        artifact.checksum.clone(),
        artifact.resumed,
    ));
    state.mark_checksum_verified(&artifact.checksum);
    state.set_partial_resume(artifact.resumed);
    Ok(())
}

fn run_disk_stage<H: mash_hal::FormatOps>(
    hal: &H,
    state: &mut state_manager::InstallState,
    cfg: &DiskStageConfig,
    dry_run: bool,
) -> Result<()> {
    if cfg.format_ext4.is_empty() && cfg.format_btrfs.is_empty() {
        log::info!("Disk stage skipped; no format targets configured");
        return Ok(());
    }
    let format_opts = mash_hal::FormatOptions::new(dry_run, cfg.confirmed);
    for device in &cfg.format_ext4 {
        hal.format_ext4(device, &format_opts)?;
        if !dry_run {
            state.record_formatted_device(device);
        }
    }
    for device in &cfg.format_btrfs {
        hal.format_btrfs(device, &format_opts)?;
        if !dry_run {
            state.record_formatted_device(device);
        }
    }
    Ok(())
}

fn run_boot_stage(
    state: &mut state_manager::InstallState,
    cfg: &BootStageConfig,
    dry_run: bool,
) -> Result<()> {
    if !cfg.enabled {
        log::info!("Boot stage skipped; kernel fix disabled");
        return Ok(());
    }
    let root = cfg
        .root
        .as_ref()
        .context("kernel_fix_root is required for boot stage")?;
    let mountinfo_path = cfg
        .mountinfo
        .as_ref()
        .context("mountinfo_path is required for boot stage")?;
    let by_uuid_path = cfg
        .by_uuid
        .as_ref()
        .context("by_uuid_path is required for boot stage")?;
    if dry_run {
        log::info!(
            "DRY RUN: kernel fix would patch {} using {} and {}",
            root.display(),
            mountinfo_path.display(),
            by_uuid_path.display()
        );
        return Ok(());
    }
    let mountinfo_content = std::fs::read_to_string(mountinfo_path)?;
    boot_config::usb_root_fix::apply_usb_root_fix(root, &mountinfo_content, by_uuid_path)?;
    state.mark_boot_completed();
    Ok(())
}

fn run_mount_stage<H: mash_hal::MountOps>(
    hal: &H,
    _state: &mut state_manager::InstallState,
    cfg: &MountStageConfig,
    dry_run: bool,
) -> Result<()> {
    if cfg.mounts.is_empty() {
        log::info!("Mount stage skipped; no mounts configured");
        return Ok(());
    }

    for spec in &cfg.mounts {
        let target = PathBuf::from(&spec.target);
        if hal.is_mounted(&target).unwrap_or(false) {
            log::info!("Mount already present: {}", target.display());
            continue;
        }
        if !dry_run {
            std::fs::create_dir_all(&target)?;
        }
        hal.mount_device(
            PathBuf::from(&spec.device).as_path(),
            &target,
            spec.fstype.as_deref(),
            mash_hal::MountOptions::new(),
            dry_run,
        )?;
    }
    Ok(())
}

fn run_package_stage(
    _state: &mut state_manager::InstallState,
    cfg: &PackageStageConfig,
    dry_run: bool,
) -> Result<()> {
    if cfg.packages.is_empty() {
        log::info!("Package stage skipped; no packages configured");
        return Ok(());
    }
    let pkg_mgr = system_config::packages::default_package_manager(dry_run);
    pkg_mgr.update()?;
    pkg_mgr.install(&cfg.packages)?;
    Ok(())
}

fn run_resume_stage(
    state: &mut state_manager::InstallState,
    cfg: &ResumeStageConfig,
    dry_run: bool,
) -> Result<()> {
    if !state.boot_stage_completed {
        log::info!("Resume stage skipped; boot stage not completed");
        return Ok(());
    }

    if dry_run {
        log::info!("DRY RUN: would install resume unit + request reboot");
        return Ok(());
    }

    let exec_path = env::current_exe().context("Failed to determine current executable path")?;
    let unit_content = system_config::resume::render_resume_unit(&exec_path, &cfg.state_path);
    system_config::resume::install_resume_unit(&cfg.mash_root, &unit_content)?;
    if let Some(conn) = system_config::resume::connect_systemd() {
        system_config::resume::enable_resume_unit(&conn)?;
    } else {
        log::warn!("No systemd connection available; skipping resume unit enable");
    }
    system_config::resume::request_reboot(dry_run)?;

    Ok(())
}

pub fn run_pipeline(cfg: &InstallConfig) -> Result<InstallPlan> {
    let plan = build_plan(cfg);
    if cfg.execute && !cfg.confirmed {
        anyhow::bail!("Execution requires explicit confirmation");
    }
    if !cfg.execute {
        return Ok(plan);
    }
    let requires_network = !cfg.packages.is_empty() || cfg.download_image || cfg.download_uefi;
    let required_binaries = {
        let mut bins = Vec::new();
        if !cfg.packages.is_empty() {
            bins.push("dnf".to_string());
        }
        if !cfg.format_ext4.is_empty() {
            bins.push("mkfs.ext4".to_string());
        }
        if !cfg.format_btrfs.is_empty() {
            bins.push("mkfs.btrfs".to_string());
        }
        bins
    };
    // Create HAL for system operations
    let hal = Arc::new(mash_hal::LinuxHal::new());

    let preflight_cfg = Arc::new(preflight::PreflightConfig::for_install(
        cfg.disk.as_ref().map(PathBuf::from),
        requires_network,
        required_binaries,
    ));
    let mount_cfg = Arc::new(MountStageConfig::from_install_config(cfg));
    let package_cfg = Arc::new(PackageStageConfig::from_install_config(cfg));
    let resume_cfg = Arc::new(ResumeStageConfig::from_install_config(cfg));
    let stage_defs = plan
        .stages
        .iter()
        .map(|stage| match stage.name {
            "Preflight" => {
                let cfg = Arc::clone(&preflight_cfg);
                StageDefinition {
                    name: stage.name,
                    run: Box::new(move |_state, _dry_run| preflight::run(&cfg)),
                }
            }
            "Download assets" => {
                let download_cfg = DownloadStageConfig::from_install_config(cfg);
                StageDefinition {
                    name: stage.name,
                    run: Box::new(move |state, dry_run| {
                        run_download_stage(state, &download_cfg, dry_run)
                    }),
                }
            }
            "Mount plan" => {
                let cfg = Arc::clone(&mount_cfg);
                let hal = Arc::clone(&hal);
                StageDefinition {
                    name: stage.name,
                    run: Box::new(move |state, dry_run| {
                        run_mount_stage(hal.as_ref(), state, &cfg, dry_run)
                    }),
                }
            }
            "Format plan" => {
                let disk_cfg = DiskStageConfig::from_install_config(cfg);
                let hal = Arc::clone(&hal);
                StageDefinition {
                    name: stage.name,
                    run: Box::new(move |state, dry_run| {
                        run_disk_stage(hal.as_ref(), state, &disk_cfg, dry_run)
                    }),
                }
            }
            "Package plan" => {
                let cfg = Arc::clone(&package_cfg);
                StageDefinition {
                    name: stage.name,
                    run: Box::new(move |state, dry_run| run_package_stage(state, &cfg, dry_run)),
                }
            }
            "Kernel fix check" => {
                let boot_cfg = BootStageConfig::from_install_config(cfg);
                StageDefinition {
                    name: stage.name,
                    run: Box::new(move |state, dry_run| run_boot_stage(state, &boot_cfg, dry_run)),
                }
            }
            "Resume unit" => {
                let cfg = Arc::clone(&resume_cfg);
                StageDefinition {
                    name: stage.name,
                    run: Box::new(move |state, dry_run| run_resume_stage(state, &cfg, dry_run)),
                }
            }
            name => {
                let description = stage.description.clone();
                StageDefinition {
                    name,
                    run: Box::new(move |_state, dry_run| {
                        if dry_run {
                            log::info!("DRY RUN: {} — {}", name, description);
                        } else {
                            log::info!("Stage: {} — {}", name, description);
                        }
                        Ok(())
                    }),
                }
            }
        })
        .collect::<Vec<_>>();

    let runner = StageRunner::new(cfg.state_path.clone(), cfg.dry_run);
    runner.run(&stage_defs)?;

    Ok(plan)
}

impl fmt::Display for InstallPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for line in self.summary_lines() {
            writeln!(f, "{}", line)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::Method::GET;
    use httpmock::MockServer;
    use mash_core::state_manager::{self, DownloadArtifact};
    use sha2::{Digest, Sha256};
    use std::env;
    use std::ffi::OsString;
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::symlink;
    use std::path::{Path, PathBuf};
    use tempfile::{tempdir, TempDir};

    fn run_download_stage_with_runner(
        cfg: &InstallConfig,
        state_path: &Path,
    ) -> state_manager::InstallState {
        let stage_cfg = DownloadStageConfig::from_install_config(cfg);
        let stage_def = StageDefinition {
            name: "Download assets",
            run: Box::new(move |state, dry_run| run_download_stage(state, &stage_cfg, dry_run)),
        };
        StageRunner::new(state_path.to_path_buf(), false)
            .run(&[stage_def])
            .unwrap()
    }

    struct PathGuard(Option<OsString>);

    impl PathGuard {
        fn new(extra: &Path) -> Self {
            let original = env::var_os("PATH");
            let mut paths = Vec::new();
            paths.push(extra.to_path_buf());
            if let Some(ref orig) = original {
                paths.extend(env::split_paths(orig));
            }
            let joined = env::join_paths(paths).unwrap();
            env::set_var("PATH", &joined);
            Self(original)
        }
    }

    impl Drop for PathGuard {
        fn drop(&mut self) {
            if let Some(ref original) = self.0 {
                env::set_var("PATH", original);
            } else {
                env::remove_var("PATH");
            }
        }
    }

    struct EnvVarGuard {
        key: String,
        original: Option<OsString>,
    }

    impl EnvVarGuard {
        fn new(key: impl Into<String>, value: impl AsRef<std::ffi::OsStr>) -> Self {
            let key = key.into();
            let original = env::var_os(&key);
            env::set_var(&key, value);
            Self { key, original }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            if let Some(ref original) = self.original {
                env::set_var(&self.key, original);
            } else {
                env::remove_var(&self.key);
            }
        }
    }

    struct PreflightEnv {
        _env_lock: crate::test_env::EnvLockGuard,
        _path_guard: PathGuard,
        _skip_network: EnvVarGuard,
        _os_release: EnvVarGuard,
        bin_dir: PathBuf,
    }

    fn prepare_preflight_env(tmp: &TempDir) -> PreflightEnv {
        let env_lock = crate::test_env::lock();
        let bin_dir = tmp.path().join("bin");
        fs::create_dir_all(&bin_dir).unwrap();
        for binary in &[
            "dnf",
            "mkfs.ext4",
            "mkfs.btrfs",
            "mount",
            "rsync",
            "systemctl",
        ] {
            let path = bin_dir.join(binary);
            fs::write(&path, "#!/bin/sh\n").unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = fs::metadata(&path).unwrap().permissions();
                perms.set_mode(0o755);
                fs::set_permissions(&path, perms).unwrap();
            }
        }

        // Ensure preflight's Fedora-only check is deterministic across CI hosts.
        let os_release_path = tmp.path().join("os-release");
        fs::write(
            &os_release_path,
            "NAME=\"Fedora Linux\"\nID=fedora\nVERSION_ID=\"43\"\n",
        )
        .unwrap();
        PreflightEnv {
            _env_lock: env_lock,
            _path_guard: PathGuard::new(&bin_dir),
            _skip_network: EnvVarGuard::new("MASH_TEST_SKIP_NETWORK_CHECK", "1"),
            _os_release: EnvVarGuard::new("MASH_OS_RELEASE_PATH", &os_release_path),
            bin_dir,
        }
    }

    #[test]
    fn plan_includes_expected_stages() {
        let cfg = InstallConfig {
            dry_run: true,
            execute: false,
            confirmed: false,
            state_path: PathBuf::from("/tmp/state.json"),
            disk: None,
            mounts: Vec::new(),
            format_ext4: Vec::new(),
            format_btrfs: Vec::new(),
            packages: Vec::new(),
            kernel_fix: false,
            kernel_fix_root: None,
            mountinfo_path: None,
            by_uuid_path: None,
            reboot_count: 1,
            mash_root: PathBuf::from("/"),
            download_image: false,
            download_uefi: false,
            image_version: "43".to_string(),
            image_edition: "KDE".to_string(),
            download_mirror: None,
            download_checksum: None,
            download_checksum_url: None,
            download_timeout_secs: 120,
            download_retries: 3,
            download_dir: PathBuf::from("downloads/images"),
        };
        let plan = build_plan(&cfg);
        assert_eq!(plan.stages.len(), 8);
        assert_eq!(plan.stages[0].name, "Preflight");
        assert_eq!(plan.stages[1].name, "Download assets");
        assert_eq!(plan.stages[6].name, "Kernel fix check");
        assert_eq!(plan.stages[7].name, "Resume unit");
    }

    fn make_download_config_internal(
        state_path: PathBuf,
        mash_root: PathBuf,
        download_dir: PathBuf,
        mirror: String,
        checksum: String,
        execute: bool,
        dry_run: bool,
    ) -> InstallConfig {
        InstallConfig {
            dry_run,
            execute,
            confirmed: true,
            state_path,
            disk: None,
            mounts: Vec::new(),
            format_ext4: Vec::new(),
            format_btrfs: Vec::new(),
            packages: Vec::new(),
            kernel_fix: false,
            kernel_fix_root: None,
            mountinfo_path: None,
            by_uuid_path: None,
            reboot_count: 1,
            mash_root,
            download_image: true,
            download_uefi: false,
            image_version: "43".to_string(),
            image_edition: "KDE".to_string(),
            download_mirror: Some(mirror),
            download_checksum: Some(checksum),
            download_checksum_url: None,
            download_timeout_secs: 5,
            download_retries: 2,
            download_dir,
        }
    }

    fn make_download_config(
        state_path: PathBuf,
        mash_root: PathBuf,
        download_dir: PathBuf,
        mirror: String,
        checksum: String,
    ) -> InstallConfig {
        make_download_config_internal(
            state_path,
            mash_root,
            download_dir,
            mirror,
            checksum,
            true,
            false,
        )
    }

    fn make_boot_config(
        state_path: PathBuf,
        mash_root: PathBuf,
        root: PathBuf,
        mountinfo: PathBuf,
        by_uuid: PathBuf,
    ) -> InstallConfig {
        InstallConfig {
            dry_run: false,
            execute: true,
            confirmed: true,
            state_path,
            disk: None,
            mounts: Vec::new(),
            format_ext4: Vec::new(),
            format_btrfs: Vec::new(),
            packages: Vec::new(),
            kernel_fix: true,
            kernel_fix_root: Some(root),
            mountinfo_path: Some(mountinfo),
            by_uuid_path: Some(by_uuid),
            reboot_count: 1,
            mash_root,
            download_image: false,
            download_uefi: false,
            image_version: "43".to_string(),
            image_edition: "KDE".to_string(),
            download_mirror: None,
            download_checksum: None,
            download_checksum_url: None,
            download_timeout_secs: 120,
            download_retries: 3,
            download_dir: PathBuf::from("downloads/images"),
        }
    }

    fn setup_boot_environment(tmp: &TempDir) -> (PathBuf, PathBuf, PathBuf, String) {
        let root = tmp.path().join("rootfs");
        fs::create_dir_all(root.join("etc/kernel")).unwrap();
        fs::write(root.join("etc/kernel/cmdline"), "root=/dev/mock0 quiet\n").unwrap();
        let bls_dir = root.join("boot/loader/entries");
        fs::create_dir_all(&bls_dir).unwrap();
        fs::write(
            bls_dir.join("entry.conf"),
            "title Fedora\noptions root=/dev/mock0 quiet\n",
        )
        .unwrap();

        let mountinfo = tmp.path().join("mountinfo");
        let device = tmp.path().join("dev/mock0");
        fs::create_dir_all(device.parent().unwrap()).unwrap();
        fs::write(&device, "").unwrap();
        let mount_line = format!(
            "1 2 0:41 / / rw,relatime - ext4 {} rw,errors=remount-ro\n",
            device.display()
        );
        fs::write(&mountinfo, mount_line).unwrap();

        let by_uuid = tmp.path().join("by-uuid");
        fs::create_dir_all(&by_uuid).unwrap();
        let uuid = "1234-5678";
        #[cfg(unix)]
        symlink(&device, by_uuid.join(uuid)).unwrap();

        (root, mountinfo, by_uuid, uuid.to_string())
    }

    fn make_disk_config(
        state_path: PathBuf,
        mash_root: PathBuf,
        format_ext4: Vec<String>,
        format_btrfs: Vec<String>,
        execute: bool,
        dry_run: bool,
        confirmed: bool,
    ) -> InstallConfig {
        InstallConfig {
            dry_run,
            execute,
            confirmed,
            state_path,
            disk: None,
            mounts: Vec::new(),
            format_ext4,
            format_btrfs,
            packages: Vec::new(),
            kernel_fix: false,
            kernel_fix_root: None,
            mountinfo_path: None,
            by_uuid_path: None,
            reboot_count: 1,
            mash_root,
            download_image: false,
            download_uefi: false,
            image_version: "43".to_string(),
            image_edition: "KDE".to_string(),
            download_mirror: None,
            download_checksum: None,
            download_checksum_url: None,
            download_timeout_secs: 120,
            download_retries: 3,
            download_dir: PathBuf::from("downloads/images"),
        }
    }

    fn write_mkfs_script(bin_dir: &Path, program: &str, log_path: &Path) {
        let script = format!(
            "#!/bin/sh\nprintf \"{prog} %s\\n\" \"$@\" >> \"{log}\"\n",
            prog = program,
            log = log_path.display()
        );
        let path = bin_dir.join(program);
        fs::write(&path, script).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&path).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&path, perms).unwrap();
        }
    }

    #[test]
    fn download_stage_records_artifact() {
        let server = MockServer::start();
        let body = b"artifact";
        let checksum = format!("{:x}", Sha256::digest(body));
        server.mock(|when, then| {
            when.method(GET).path("/image");
            then.status(200).body(body);
        });

        let tmp = tempdir().unwrap();
        let state_path = tmp.path().join("state.json");
        let downloads = tmp.path().join("downloads").join("images");
        let cfg = make_download_config(
            state_path.clone(),
            tmp.path().to_path_buf(),
            downloads.clone(),
            server.url("/image"),
            checksum.clone(),
        );

        let saved = run_download_stage_with_runner(&cfg, &state_path);
        let persisted = state_manager::load_state(&state_path).unwrap().unwrap();
        assert_eq!(saved, persisted);
        assert!(saved
            .completed_stages
            .contains(&"Download assets".to_string()));
        assert_eq!(saved.download_artifacts.len(), 1);
        assert_eq!(saved.verified_checksums, vec![checksum]);
        assert_eq!(saved.download_artifacts[0].size, body.len() as u64);
    }

    #[test]
    fn download_stage_skips_when_completed() {
        let tmp = tempdir().unwrap();
        let state_path = tmp.path().join("state.json");
        let downloads = tmp.path().join("downloads").join("images");
        fs::create_dir_all(&downloads).unwrap();
        let artifact_path = downloads.join("Fedora-43-KDE-aarch64.raw.xz");
        fs::write(&artifact_path, b"cached").unwrap();
        let checksum = format!("{:x}", Sha256::digest(b"cached"));

        let mut preset = state_manager::InstallState::new(false);
        preset.record_download(DownloadArtifact::new(
            "Fedora-43-KDE-aarch64.raw.xz".to_string(),
            &artifact_path,
            6,
            checksum.clone(),
            false,
        ));
        preset.mark_checksum_verified(&checksum);
        preset.mark_completed("Download assets");
        state_manager::save_state_atomic(&state_path, &preset).unwrap();

        let cfg = make_download_config(
            state_path.clone(),
            tmp.path().to_path_buf(),
            downloads.clone(),
            "http://127.0.0.1/unused".to_string(),
            checksum,
        );

        let download_cfg = DownloadStageConfig::from_install_config(&cfg);
        let stage = StageDefinition {
            name: "Download assets",
            run: Box::new(move |state, dry_run| run_download_stage(state, &download_cfg, dry_run)),
        };
        let runner = StageRunner::new(state_path.clone(), false);
        let saved = runner.run(&[stage]).unwrap();
        let persisted = state_manager::load_state(&state_path).unwrap().unwrap();
        assert_eq!(saved, persisted);
        assert_eq!(saved.download_artifacts.len(), 1);
        assert!(saved
            .completed_stages
            .contains(&"Download assets".to_string()));
    }

    #[test]
    fn download_stage_checksum_failure() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/image");
            then.status(200).body(b"bad");
        });

        let tmp = tempdir().unwrap();
        let state_path = tmp.path().join("state.json");
        let downloads = tmp.path().join("downloads").join("images");
        let cfg = make_download_config(
            state_path.clone(),
            tmp.path().to_path_buf(),
            downloads.clone(),
            server.url("/image"),
            "deadbeef".to_string(),
        );

        let download_cfg = DownloadStageConfig::from_install_config(&cfg);
        let stage = StageDefinition {
            name: "Download assets",
            run: Box::new(move |state, dry_run| run_download_stage(state, &download_cfg, dry_run)),
        };
        let runner = StageRunner::new(state_path.clone(), false);
        assert!(runner.run(&[stage]).is_err());
        let saved = state_manager::load_state(&state_path).unwrap().unwrap();
        assert!(!saved
            .completed_stages
            .contains(&"Download assets".to_string()));
        assert!(saved.download_artifacts.is_empty());
    }

    #[test]
    fn pipeline_dry_run_preflight_and_download() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET)
                .path("/image")
                .header("x-mash-attempt", "1");
            then.status(200).body(b"ok");
        });

        let tmp = tempdir().unwrap();
        let _guards = prepare_preflight_env(&tmp);
        let state_path = tmp.path().join("state.json");
        let downloads = tmp.path().join("downloads").join("images");
        let checksum = format!("{:x}", Sha256::digest(b"ok"));
        let cfg = make_download_config_internal(
            state_path.clone(),
            tmp.path().to_path_buf(),
            downloads.clone(),
            server.url("/image"),
            checksum,
            true,
            true,
        );

        let plan = run_pipeline(&cfg).unwrap();
        assert_eq!(plan.stages[1].name, "Download assets");
        let artifact_path = downloads.join("override.img.xz");
        assert!(!artifact_path.exists());
    }

    #[test]
    fn pipeline_execute_plan_records_download_artifact() {
        let server = MockServer::start();
        let body = b"execute";
        let checksum = format!("{:x}", Sha256::digest(body));
        server.mock(|when, then| {
            when.method(GET)
                .path("/image")
                .header("x-mash-attempt", "1");
            then.status(200).body(body);
        });

        let tmp = tempdir().unwrap();
        let _guards = prepare_preflight_env(&tmp);
        let state_path = tmp.path().join("state.json");
        let downloads = tmp.path().join("downloads").join("images");
        let cfg = make_download_config_internal(
            state_path.clone(),
            tmp.path().to_path_buf(),
            downloads.clone(),
            server.url("/image"),
            checksum.clone(),
            true,
            false,
        );

        run_pipeline(&cfg).unwrap();
        let artifact_path = downloads.join("override.img.xz");
        let metadata = artifact_path.metadata().unwrap();
        assert_eq!(metadata.len(), body.len() as u64);
    }

    #[test]
    fn pipeline_resumes_partial_download_state() {
        let server = MockServer::start();
        let body = b"0123456789";
        let checksum = format!("{:x}", Sha256::digest(body));
        server.mock(|when, then| {
            when.method(GET)
                .path("/image")
                .header("range", "bytes=5-")
                .header("x-mash-attempt", "1");
            then.status(206).body(&body[5..]);
        });

        let tmp = tempdir().unwrap();
        let _guards = prepare_preflight_env(&tmp);
        let state_path = tmp.path().join("state.json");
        let downloads = tmp.path().join("downloads").join("images");
        fs::create_dir_all(&downloads).unwrap();
        let partial = downloads.join("override.img.xz");
        fs::write(&partial, &body[..5]).unwrap();

        let cfg = make_download_config_internal(
            state_path.clone(),
            tmp.path().to_path_buf(),
            downloads.clone(),
            server.url("/image"),
            checksum,
            true,
            false,
        );

        run_pipeline(&cfg).unwrap();
        let artifact_path = downloads.join("override.img.xz");
        assert_eq!(fs::read(&artifact_path).unwrap(), body);
    }

    #[test]
    fn disk_stage_formats_commands_when_confirmed() {
        let tmp = tempdir().unwrap();
        let env = prepare_preflight_env(&tmp);
        let log_path = tmp.path().join("mkfs.log");
        write_mkfs_script(&env.bin_dir, "mkfs.ext4", &log_path);

        let state_path = tmp.path().join("state.json");
        let cfg = make_disk_config(
            state_path.clone(),
            tmp.path().to_path_buf(),
            vec!["/dev/mock1".to_string()],
            Vec::new(),
            true,
            false,
            true,
        );
        let disk_cfg = DiskStageConfig::from_install_config(&cfg);
        let hal = mash_hal::LinuxHal::new();
        let stage = StageDefinition {
            name: "Format plan",
            run: Box::new(move |state, dry_run| run_disk_stage(&hal, state, &disk_cfg, dry_run)),
        };

        let state = StageRunner::new(state_path.clone(), false)
            .run(&[stage])
            .unwrap();
        assert_eq!(state.formatted_devices, vec!["/dev/mock1".to_string()]);
        assert_eq!(
            fs::read_to_string(&log_path).unwrap(),
            "mkfs.ext4 /dev/mock1\n"
        );
    }

    #[test]
    fn disk_stage_dry_run_skips_formatting() {
        let tmp = tempdir().unwrap();
        let env = prepare_preflight_env(&tmp);
        let log_path = tmp.path().join("mkfs.log");
        write_mkfs_script(&env.bin_dir, "mkfs.ext4", &log_path);

        let state_path = tmp.path().join("state.json");
        let cfg = make_disk_config(
            state_path.clone(),
            tmp.path().to_path_buf(),
            vec!["/dev/mock1".to_string()],
            Vec::new(),
            false,
            true,
            false,
        );
        let disk_cfg = DiskStageConfig::from_install_config(&cfg);
        let hal = mash_hal::LinuxHal::new();
        let stage = StageDefinition {
            name: "Format plan",
            run: Box::new(move |state, dry_run| run_disk_stage(&hal, state, &disk_cfg, dry_run)),
        };

        let state = StageRunner::new(state_path.clone(), true)
            .run(&[stage])
            .unwrap();
        assert!(state.formatted_devices.is_empty());
        assert!(!log_path.exists());
    }

    #[test]
    fn boot_stage_applies_kernel_fix() {
        let tmp = tempdir().unwrap();
        let state_path = tmp.path().join("state.json");
        let (root, mountinfo, by_uuid, _) = setup_boot_environment(&tmp);
        let cfg = make_boot_config(
            state_path.clone(),
            tmp.path().to_path_buf(),
            root.clone(),
            mountinfo.clone(),
            by_uuid.clone(),
        );
        let boot_cfg = BootStageConfig::from_install_config(&cfg);
        let stage = StageDefinition {
            name: "Kernel fix check",
            run: Box::new(move |state, dry_run| run_boot_stage(state, &boot_cfg, dry_run)),
        };

        let state = StageRunner::new(state_path.clone(), false)
            .run(&[stage])
            .unwrap();
        assert!(state.boot_stage_completed);
        let cmdline = fs::read_to_string(root.join("etc/kernel/cmdline")).unwrap();
        assert!(cmdline.contains("root=UUID="));
        assert!(cmdline.contains("rootflags=subvol=root"));
        let bls_content = fs::read_to_string(root.join("boot/loader/entries/entry.conf")).unwrap();
        assert!(bls_content.contains("root=UUID="));
        assert!(bls_content.contains("rootflags=subvol=root"));
        assert!(root.join("etc/kernel/cmdline.bak").exists());
        assert!(root.join("boot/loader/entries/entry.conf.bak").exists());
    }

    #[test]
    fn boot_stage_dry_run_leaves_files_untouched() {
        let tmp = tempdir().unwrap();
        let state_path = tmp.path().join("state.json");
        let (root, mountinfo, by_uuid, _) = setup_boot_environment(&tmp);
        let initial_cmdline = fs::read_to_string(root.join("etc/kernel/cmdline")).unwrap();
        let initial_bls = fs::read_to_string(root.join("boot/loader/entries/entry.conf")).unwrap();
        let cfg = make_boot_config(
            state_path.clone(),
            tmp.path().to_path_buf(),
            root.clone(),
            mountinfo.clone(),
            by_uuid.clone(),
        );
        let boot_cfg = BootStageConfig::from_install_config(&cfg);
        let stage = StageDefinition {
            name: "Kernel fix check",
            run: Box::new(move |state, dry_run| run_boot_stage(state, &boot_cfg, dry_run)),
        };

        let state = StageRunner::new(state_path.clone(), true)
            .run(&[stage])
            .unwrap();
        assert!(!state.boot_stage_completed);
        assert_eq!(
            fs::read_to_string(root.join("etc/kernel/cmdline")).unwrap(),
            initial_cmdline
        );
        assert_eq!(
            fs::read_to_string(root.join("boot/loader/entries/entry.conf")).unwrap(),
            initial_bls
        );
    }

    #[test]
    fn pipeline_installs_resume_unit_and_requests_reboot() {
        let tmp = tempdir().unwrap();
        let _guards = prepare_preflight_env(&tmp);
        let _skip_dnf = EnvVarGuard::new("MASH_TEST_SKIP_DNF", "1");
        let state_path = tmp.path().join("state.json");
        let (root, mountinfo, by_uuid, _) = setup_boot_environment(&tmp);
        let cfg = make_boot_config(
            state_path.clone(),
            tmp.path().to_path_buf(),
            root.clone(),
            mountinfo.clone(),
            by_uuid.clone(),
        );

        run_pipeline(&cfg).unwrap();

        let unit_path = tmp
            .path()
            .join("etc/systemd/system")
            .join("mash-core-resume.service");
        assert!(unit_path.exists());
        let content = fs::read_to_string(&unit_path).unwrap();
        assert!(content.contains("--resume --state"));
    }
}
