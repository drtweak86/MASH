use super::stages::run_download_stage;
use super::stages::{run_boot_stage, run_disk_stage};
use super::*;
use crate::install_runner::{StageDefinition, StageRunner};
use mash_core::state_manager::StageName;

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
        name: StageName::DownloadAssets,
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
) -> InstallConfig {
    InstallConfig {
        dry_run,
        execute,
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
    assert!(saved.completed_stages.contains(&StageName::DownloadAssets));
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
        artifact_path.clone(),
        checksum.clone(),
        6,
        false,
    ));
    preset.mark_checksum_verified(&checksum);
    preset.mark_completed(&StageName::DownloadAssets);
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
        name: StageName::DownloadAssets,
        run: Box::new(move |state, dry_run| run_download_stage(state, &download_cfg, dry_run)),
    };
    let runner = StageRunner::new(state_path.clone(), false);
    let saved = runner.run(&[stage]).unwrap();
    let persisted = state_manager::load_state(&state_path).unwrap().unwrap();
    assert_eq!(saved, persisted);
    assert_eq!(saved.download_artifacts.len(), 1);
    assert!(saved.completed_stages.contains(&StageName::DownloadAssets));
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
        name: StageName::DownloadAssets,
        run: Box::new(move |state, dry_run| run_download_stage(state, &download_cfg, dry_run)),
    };
    let runner = StageRunner::new(state_path.clone(), false);
    assert!(runner.run(&[stage]).is_err());
    let saved = state_manager::load_state(&state_path).unwrap().unwrap();
    assert!(!saved.completed_stages.contains(&StageName::DownloadAssets));
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

    let token = mash_core::config_states::ExecuteArmToken::try_new(true, true, true).unwrap();
    let validated = mash_core::config_states::UnvalidatedConfig::new(cfg)
        .validate()
        .unwrap();
    let armed = validated.arm_execute(token).unwrap();
    run_pipeline_execute(armed).unwrap();
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

    let token = mash_core::config_states::ExecuteArmToken::try_new(true, true, true).unwrap();
    let validated = mash_core::config_states::UnvalidatedConfig::new(cfg)
        .validate()
        .unwrap();
    let armed = validated.arm_execute(token).unwrap();
    run_pipeline_execute(armed).unwrap();
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
    );
    let disk_cfg = DiskStageConfig::from_install_config(&cfg);
    let hal = mash_hal::LinuxHal::new();
    let stage = StageDefinition {
        name: StageName::FormatPlan,
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
    );
    let disk_cfg = DiskStageConfig::from_install_config(&cfg);
    let hal = mash_hal::LinuxHal::new();
    let stage = StageDefinition {
        name: StageName::FormatPlan,
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
        name: StageName::KernelFixCheck,
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
        name: StageName::KernelFixCheck,
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

    let token = mash_core::config_states::ExecuteArmToken::try_new(true, true, true).unwrap();
    let validated = mash_core::config_states::UnvalidatedConfig::new(cfg)
        .validate()
        .unwrap();
    let armed = validated.arm_execute(token).unwrap();
    run_pipeline_execute(armed).unwrap();

    let unit_path = tmp
        .path()
        .join("etc/systemd/system")
        .join("mash-core-resume.service");
    assert!(unit_path.exists());
    let content = fs::read_to_string(&unit_path).unwrap();
    assert!(content.contains("--resume --state"));
}
