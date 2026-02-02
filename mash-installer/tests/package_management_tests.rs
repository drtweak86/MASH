use mash_installer::stages::package_management::install_packages;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::sync::{Mutex, OnceLock};
use tempfile::TempDir;

const DNF_BIN_ENV: &str = "MASH_DNF_BIN";

struct EnvGuard {
    key: &'static str,
    previous: Option<std::ffi::OsString>,
}

impl EnvGuard {
    fn set(key: &'static str, value: &std::path::Path) -> Self {
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

fn write_executable(path: &std::path::Path, content: &str) {
    fs::write(path, content).expect("write script");
    let mut perms = fs::metadata(path).expect("metadata").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms).expect("set perms");
}

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[test]
fn install_packages_passes_expected_args() {
    let _lock = env_lock().lock().expect("env lock");
    let temp_dir = TempDir::new().expect("temp dir");
    let log_path = temp_dir.path().join("args.log");
    let script_path = temp_dir.path().join("dnf-mock");

    let script = format!(
        "#!/usr/bin/env bash\nprintf '%s\\n' \"$@\" > '{}'\n",
        log_path.display()
    );
    write_executable(&script_path, &script);

    let _guard = EnvGuard::set(DNF_BIN_ENV, &script_path);

    install_packages(&["git", "curl"]).expect("install packages");

    let args = fs::read_to_string(&log_path).expect("read args log");
    let args: Vec<&str> = args.lines().collect();

    assert_eq!(
        args,
        vec![
            "install",
            "-y",
            "--skip-unavailable",
            "--setopt=install_weak_deps=True",
            "git",
            "curl"
        ]
    );
}

#[test]
fn install_packages_ignores_nonzero_status() {
    let _lock = env_lock().lock().expect("env lock");
    let temp_dir = TempDir::new().expect("temp dir");
    let script_path = temp_dir.path().join("dnf-fail");

    let script = "#!/usr/bin/env bash\nexit 1\n";
    write_executable(&script_path, script);

    let _guard = EnvGuard::set(DNF_BIN_ENV, &script_path);

    install_packages(&["git"]).expect("install packages should ignore nonzero");
}
