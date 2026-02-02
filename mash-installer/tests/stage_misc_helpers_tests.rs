use mash_installer::stages::stage_10_locale_uk;
use mash_installer::stages::stage_11_snapper_init;
use mash_installer::stages::stage_12_firewall_sane;
use mash_installer::stages::stage_17_brave_browser;
use mash_installer::stages::stage_17_brave_default;
use mash_installer::stages::stage_20_argon_one;
use mash_installer::stages::stage_22_kde_screensaver_nuke;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
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
fn stage_10_locale_uk_runs_commands() {
    let _lock = env_lock().lock().expect("env lock");
    let temp_dir = TempDir::new().expect("temp dir");
    let bin_dir = temp_dir.path().join("bin");
    fs::create_dir_all(&bin_dir).expect("create bin");
    let log_path = temp_dir.path().join("locale.log");

    stub_command(&bin_dir, "dnf", &log_path);
    stub_command(&bin_dir, "localectl", &log_path);

    let _guard = setup_path_env(&bin_dir);

    stage_10_locale_uk::run(&[]).expect("run locale stage");

    let log = fs::read_to_string(&log_path).expect("read log");
    assert!(log.contains("dnf install -y langpacks-en_GB"));
    assert!(log.contains("localectl set-locale LANG=en_GB.UTF-8"));
    assert!(log.contains("localectl set-x11-keymap gb"));
}

#[test]
fn stage_11_snapper_init_runs_commands() {
    let _lock = env_lock().lock().expect("env lock");
    let temp_dir = TempDir::new().expect("temp dir");
    let bin_dir = temp_dir.path().join("bin");
    fs::create_dir_all(&bin_dir).expect("create bin");
    let log_path = temp_dir.path().join("snapper.log");

    stub_command(&bin_dir, "dnf", &log_path);
    stub_command(&bin_dir, "snapper", &log_path);
    stub_command(&bin_dir, "chmod", &log_path);
    stub_command(&bin_dir, "chown", &log_path);

    let _guard = setup_path_env(&bin_dir);

    stage_11_snapper_init::run(&[]).expect("run snapper stage");

    let log = fs::read_to_string(&log_path).expect("read log");
    assert!(log.contains("dnf install -y snapper"));
    assert!(log.contains("snapper -c root create-config /"));
    assert!(log.contains("chmod a+rx /.snapshots"));
    assert!(log.contains("chown :DrTweak /.snapshots"));
}

#[test]
fn stage_12_firewall_sane_installs_files() {
    let temp_dir = TempDir::new().expect("temp dir");
    let root_dir = temp_dir.path().join("root");
    let stage_dir = temp_dir.path().join("stage");

    let systemd_dir = stage_dir.join("systemd");
    fs::create_dir_all(&systemd_dir).expect("create systemd");

    let service_src = systemd_dir.join("mash-early-ssh.service");
    let script_src = systemd_dir.join("early-ssh.sh");
    fs::write(&service_src, "service-data").expect("write service");
    fs::write(&script_src, "script-data").expect("write script");

    let args = vec![
        root_dir.to_string_lossy().to_string(),
        stage_dir.to_string_lossy().to_string(),
    ];
    stage_12_firewall_sane::run(&args).expect("run firewall stage");

    let service_dst = root_dir.join("etc/systemd/system/mash-early-ssh.service");
    let script_dst = root_dir.join("usr/local/lib/mash/system/early-ssh.sh");
    let link_dst =
        root_dir.join("etc/systemd/system/multi-user.target.wants/mash-early-ssh.service");

    assert_eq!(
        fs::read_to_string(&service_dst).expect("read service"),
        "service-data"
    );
    assert_eq!(
        fs::read_to_string(&script_dst).expect("read script"),
        "script-data"
    );

    let service_mode = fs::metadata(&service_dst)
        .expect("service meta")
        .permissions()
        .mode()
        & 0o777;
    let script_mode = fs::metadata(&script_dst)
        .expect("script meta")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(service_mode, 0o644);
    assert_eq!(script_mode, 0o755);

    let target = fs::read_link(&link_dst).expect("read symlink");
    assert_eq!(target, PathBuf::from("../mash-early-ssh.service"));
}

#[test]
fn stage_17_brave_browser_creates_repo_and_mimeapps() {
    let _lock = env_lock().lock().expect("env lock");
    let temp_dir = TempDir::new().expect("temp dir");
    let bin_dir = temp_dir.path().join("bin");
    fs::create_dir_all(&bin_dir).expect("create bin");
    let log_path = temp_dir.path().join("brave.log");

    let repo_file = temp_dir.path().join("brave-browser.repo");
    let home_dir = temp_dir.path().join("home");

    stub_command(&bin_dir, "dnf", &log_path);

    let rpm_script = "#!/bin/sh\nif [ \"$2\" = \"brave-browser\" ]; then\n  exit 0\nfi\nexit 1\n";
    write_executable(&bin_dir.join("rpm"), rpm_script);

    let id_script = "#!/bin/sh\nexit 0\n";
    write_executable(&bin_dir.join("id"), id_script);

    let getent_script = format!(
        "#!/bin/sh\necho 'drtweak:x:1000:1000::{}:/bin/sh'\n",
        home_dir.display()
    );
    write_executable(&bin_dir.join("getent"), &getent_script);

    stub_command(&bin_dir, "sudo", &log_path);
    stub_command(&bin_dir, "chown", &log_path);

    let _guard = setup_path_env(&bin_dir);
    let _log_env = EnvGuard::set("MASH_BRAVE_LOG_DIR", temp_dir.path().as_os_str());
    let _repo_env = EnvGuard::set("MASH_BRAVE_REPO_FILE", repo_file.as_os_str());

    stage_17_brave_browser::run(&["drtweak".to_string()]).expect("run brave browser stage");

    assert!(repo_file.exists());

    let mimeapps = home_dir.join(".config/mimeapps.list");
    let contents = fs::read_to_string(&mimeapps).expect("read mimeapps");
    assert!(contents.contains("[Default Applications]"));
    assert!(contents.contains("x-scheme-handler/http=brave-browser.desktop"));
    assert!(contents.contains("x-scheme-handler/https=brave-browser.desktop"));
    assert!(contents.contains("text/html=brave-browser.desktop"));
}

#[test]
fn stage_17_brave_default_skips_without_internet() {
    let _lock = env_lock().lock().expect("env lock");
    let temp_dir = TempDir::new().expect("temp dir");
    let bin_dir = temp_dir.path().join("bin");
    fs::create_dir_all(&bin_dir).expect("create bin");

    let curl_script = "#!/bin/sh\nexit 1\n";
    write_executable(&bin_dir.join("curl"), curl_script);
    stub_command(
        &bin_dir,
        "date",
        temp_dir.path().join("brave.log").as_path(),
    );
    stub_command(
        &bin_dir,
        "sudo",
        temp_dir.path().join("brave.log").as_path(),
    );
    stub_command(&bin_dir, "id", temp_dir.path().join("brave.log").as_path());

    let _guard = setup_path_env(&bin_dir);
    let _log_env = EnvGuard::set("MASH_BRAVE_DEFAULT_LOG_DIR", temp_dir.path().as_os_str());
    let _repo_env = EnvGuard::set(
        "MASH_BRAVE_DEFAULT_REPO_FILE",
        temp_dir.path().join("brave-browser.repo").as_os_str(),
    );

    stage_17_brave_default::run(&["drtweak".to_string()]).expect("run brave default stage");

    let log_path = temp_dir.path().join("brave.log");
    let log = fs::read_to_string(&log_path).expect("read log");
    assert!(log.contains("No internet detected; skipping Brave install for now."));
}

#[test]
fn stage_20_argon_one_runs_install_when_present() {
    let _lock = env_lock().lock().expect("env lock");
    let temp_dir = TempDir::new().expect("temp dir");
    let bin_dir = temp_dir.path().join("bin");
    fs::create_dir_all(&bin_dir).expect("create bin");
    let log_path = temp_dir.path().join("argon.log");

    stub_command(&bin_dir, "dnf", &log_path);
    stub_command(&bin_dir, "git", &log_path);
    stub_command(&bin_dir, "bash", &log_path);

    let argon_root = temp_dir.path().join("argon");
    fs::create_dir_all(argon_root.join(".git")).expect("create git dir");
    let install_script = argon_root.join("install.sh");
    fs::write(&install_script, "#!/bin/sh\nexit 0\n").expect("write install");
    let mut perms = fs::metadata(&install_script).expect("meta").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&install_script, perms).expect("set perms");

    let _guard = setup_path_env(&bin_dir);
    let _argon_env = EnvGuard::set("MASH_ARGON_ROOT", argon_root.as_os_str());

    stage_20_argon_one::run(&[]).expect("run argon stage");

    let log = fs::read_to_string(&log_path).expect("read log");
    assert!(
        log.contains("dnf install -y --skip-unavailable git gcc make dtc i2c-tools libi2c-devel")
    );
    assert!(log.contains("bash -lc"));
}

#[test]
fn stage_22_kde_screensaver_nuke_runs_commands() {
    let _lock = env_lock().lock().expect("env lock");
    let temp_dir = TempDir::new().expect("temp dir");
    let bin_dir = temp_dir.path().join("bin");
    fs::create_dir_all(&bin_dir).expect("create bin");
    let log_path = temp_dir.path().join("kde.log");

    stub_command(&bin_dir, "sudo", &log_path);

    let _guard = setup_path_env(&bin_dir);

    stage_22_kde_screensaver_nuke::run(&[]).expect("run kde stage");

    let log = fs::read_to_string(&log_path).expect("read log");
    assert!(log.contains("sudo -u DrTweak sh -c kwriteconfig5 --file kscreenlockerrc --group Daemon --key Autolock false"));
    assert!(log.contains("sudo -u DrTweak sh -c kwriteconfig5 --file powerdevilrc --group AC --group SuspendSession --key suspendType 0"));
    assert!(log.contains("sudo -u DrTweak sh -c xset s off"));
    assert!(log.contains("sudo -u DrTweak sh -c xset -dpms"));
}
