use mash_installer::stages::stage_21_zsh_starship::setup_zsh_starship;
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

fn write_executable(path: &std::path::Path, content: &str) {
    fs::write(path, content).expect("write script");
    let mut perms = fs::metadata(path).expect("metadata").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms).expect("set perms");
}

fn setup_stub_env(temp_dir: &TempDir, home_dir: &std::path::Path) -> EnvGuard {
    let bin_dir = temp_dir.path().join("bin");
    fs::create_dir_all(&bin_dir).expect("create bin");

    let getent = bin_dir.join("getent");
    let getent_script = format!(
        "#!/bin/sh\nif [ \"$1\" = \"passwd\" ]; then\n  echo 'DrTweak:x:1000:1000::{}:/bin/sh'\nfi\n",
        home_dir.display()
    );
    write_executable(&getent, &getent_script);

    let dnf = bin_dir.join("dnf");
    write_executable(&dnf, "#!/bin/sh\nexit 0\n");

    let chsh = bin_dir.join("chsh");
    write_executable(&chsh, "#!/bin/sh\nexit 0\n");

    let sh = bin_dir.join("sh");
    let sh_script = format!(
        "#!/bin/sh\nif [ \"$1\" = \"-c\" ]; then\n  echo \"$2\" > '{}'\n  exit 0\nfi\nexit 0\n",
        temp_dir.path().join("install.log").display()
    );
    write_executable(&sh, &sh_script);

    let path_value = format!("{}", bin_dir.display());
    EnvGuard::set("PATH", std::ffi::OsStr::new(&path_value))
}

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[test]
fn setup_zsh_starship_skips_install_when_starship_present() {
    let _lock = env_lock().lock().expect("env lock");
    let temp_dir = TempDir::new().expect("temp dir");
    let home_dir = temp_dir.path().join("home");
    let _guard = setup_stub_env(&temp_dir, &home_dir);

    let starship = temp_dir.path().join("bin").join("starship");
    write_executable(&starship, "#!/bin/sh\nexit 0\n");

    setup_zsh_starship("DrTweak").expect("setup zsh starship");

    let install_log = temp_dir.path().join("install.log");
    assert!(!install_log.exists(), "starship installer should not run");
}

#[test]
fn setup_zsh_starship_runs_install_when_starship_missing() {
    let _lock = env_lock().lock().expect("env lock");
    let temp_dir = TempDir::new().expect("temp dir");
    let home_dir = temp_dir.path().join("home");
    let _guard = setup_stub_env(&temp_dir, &home_dir);

    setup_zsh_starship("DrTweak").expect("setup zsh starship");

    let install_log = temp_dir.path().join("install.log");
    assert!(install_log.exists(), "starship installer should run");

    let zshrc = home_dir.join(".zshrc");
    let contents = fs::read_to_string(&zshrc).expect("read zshrc");
    assert!(contents.contains("starship init zsh"));
}
