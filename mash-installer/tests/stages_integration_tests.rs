use mash_installer::stages;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::sync::{Mutex, OnceLock};
use tempfile::TempDir;

struct EnvGuard {
    key: &'static str,
    previous: Option<std::ffi::OsString>,
}

impl EnvGuard {
    fn set(key: &'static str, value: &std::ffi::OsStr) -> Self {
        let previous = std::env::var_os(key);
        std::env::set_var(key, value);
        Self { key, previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        if let Some(value) = self.previous.take() {
            std::env::set_var(self.key, value);
        } else {
            std::env::remove_var(self.key);
        }
    }
}

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn write_executable(path: &std::path::Path, content: &str) {
    fs::write(path, content).expect("write script");
    let mut perms = fs::metadata(path).expect("metadata").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms).expect("set perms");
}

fn stub_command(bin_dir: &std::path::Path, name: &str, log_path: &std::path::Path) {
    let script = format!(
        "#!/bin/sh\necho '{name} '$@ >> '{}'\nexit 0\n",
        log_path.display()
    );
    write_executable(&bin_dir.join(name), &script);
}

fn setup_path_env(bin_dir: &std::path::Path) -> EnvGuard {
    let path_value = bin_dir.display().to_string();
    EnvGuard::set("PATH", std::ffi::OsStr::new(&path_value))
}

#[test]
fn stage_runner_executes_sequence() {
    let _lock = env_lock().lock().expect("env lock");
    let temp_dir = TempDir::new().expect("temp dir");
    let bin_dir = temp_dir.path().join("bin");
    fs::create_dir_all(&bin_dir).expect("create bin");
    let log_path = temp_dir.path().join("stage.log");

    stub_command(&bin_dir, "dnf", &log_path);
    stub_command(&bin_dir, "localectl", &log_path);
    stub_command(&bin_dir, "snapper", &log_path);
    stub_command(&bin_dir, "chmod", &log_path);
    stub_command(&bin_dir, "chown", &log_path);

    let _guard = setup_path_env(&bin_dir);

    stages::run_stage("10_locale_uk", &[]).expect("run locale stage");
    stages::run_stage("11_snapper_init", &[]).expect("run snapper stage");

    let stage_dir = temp_dir.path().join("stage");
    let root_dir = temp_dir.path().join("root");
    let systemd_dir = stage_dir.join("systemd");
    fs::create_dir_all(&systemd_dir).expect("create systemd");
    fs::write(systemd_dir.join("mash-early-ssh.service"), "svc").expect("write service");
    fs::write(systemd_dir.join("early-ssh.sh"), "script").expect("write script");

    stages::run_stage(
        "12_firewall_sane",
        &[
            root_dir.to_string_lossy().to_string(),
            stage_dir.to_string_lossy().to_string(),
        ],
    )
    .expect("run firewall stage");

    let log = fs::read_to_string(&log_path).expect("read log");
    assert!(log.contains("dnf install -y langpacks-en_GB"));
    assert!(log.contains("snapper -c root create-config /"));

    assert!(root_dir
        .join("etc/systemd/system/mash-early-ssh.service")
        .exists());
    assert!(root_dir
        .join("usr/local/lib/mash/system/early-ssh.sh")
        .exists());
}
