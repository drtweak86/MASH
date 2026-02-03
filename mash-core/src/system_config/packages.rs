use anyhow::{anyhow, Result};
use std::env;
use std::process::Command;

pub trait PackageManager {
    fn install(&self, pkgs: &[String]) -> Result<()>;
    fn update(&self) -> Result<()>;
}

#[derive(Debug, Clone)]
pub struct DnfShell {
    pub dry_run: bool,
}

impl DnfShell {
    pub fn new(dry_run: bool) -> Self {
        Self { dry_run }
    }
}

impl PackageManager for DnfShell {
    fn install(&self, pkgs: &[String]) -> Result<()> {
        if skip_dnf_commands() {
            log::info!("Skipping dnf install (test stub)");
            return Ok(());
        }
        if pkgs.is_empty() {
            return Ok(());
        }
        if self.dry_run {
            log::info!("DRY RUN: dnf install -y {}", pkgs.join(" "));
            return Ok(());
        }
        let spec = install_command_spec(pkgs);
        run_command(&spec)
    }

    fn update(&self) -> Result<()> {
        if skip_dnf_commands() {
            log::info!("Skipping dnf update (test stub)");
            return Ok(());
        }
        if self.dry_run {
            log::info!("DRY RUN: dnf update -y");
            return Ok(());
        }
        let spec = update_command_spec();
        run_command(&spec)
    }
}

pub fn default_package_manager(dry_run: bool) -> Box<dyn PackageManager> {
    #[cfg(feature = "libdnf")]
    {
        if use_libdnf_backend() {
            return Box::new(LibDnfPackageManager::new(dry_run));
        }
    }
    Box::new(DnfShell::new(dry_run))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec {
    pub program: String,
    pub args: Vec<String>,
}

pub fn install_command_spec(pkgs: &[String]) -> CommandSpec {
    let mut args = vec!["install".to_string(), "-y".to_string()];
    args.extend(pkgs.iter().cloned());
    CommandSpec {
        program: "dnf".to_string(),
        args,
    }
}

pub fn update_command_spec() -> CommandSpec {
    CommandSpec {
        program: "dnf".to_string(),
        args: vec!["update".to_string(), "-y".to_string()],
    }
}

fn run_command(spec: &CommandSpec) -> Result<()> {
    let status = Command::new(&spec.program).args(&spec.args).status()?;
    if !status.success() {
        return Err(anyhow!("Command failed: {}", spec.program));
    }
    Ok(())
}

fn skip_dnf_commands() -> bool {
    env::var_os("MASH_TEST_SKIP_DNF").is_some()
}

#[cfg(feature = "libdnf")]
pub trait LibDnfBackend: Send + Sync + std::fmt::Debug {
    fn install(&self, pkgs: &[String]) -> Result<()>;
    fn update(&self) -> Result<()>;
}

#[cfg(feature = "libdnf")]
#[derive(Debug, Default)]
pub struct LibDnfSysBackend;

#[cfg(feature = "libdnf")]
impl LibDnfBackend for LibDnfSysBackend {
    fn install(&self, pkgs: &[String]) -> Result<()> {
        libdnf_sys::install(pkgs).map_err(|err| anyhow!(err))
    }

    fn update(&self) -> Result<()> {
        libdnf_sys::update().map_err(|err| anyhow!(err))
    }
}

#[cfg(feature = "libdnf")]
#[derive(Debug)]
pub struct LibDnfPackageManager {
    backend: Box<dyn LibDnfBackend>,
    dry_run: bool,
}

#[cfg(feature = "libdnf")]
impl LibDnfPackageManager {
    pub fn new(dry_run: bool) -> Self {
        Self {
            backend: Box::new(LibDnfSysBackend),
            dry_run,
        }
    }

    #[cfg(test)]
    fn with_backend(dry_run: bool, backend: Box<dyn LibDnfBackend>) -> Self {
        Self { backend, dry_run }
    }
}

#[cfg(feature = "libdnf")]
impl PackageManager for LibDnfPackageManager {
    fn install(&self, pkgs: &[String]) -> Result<()> {
        if skip_dnf_commands() {
            log::info!("Skipping libdnf install (test stub)");
            return Ok(());
        }
        if pkgs.is_empty() {
            return Ok(());
        }
        if self.dry_run {
            log::info!("DRY RUN: libdnf install {:?}", pkgs);
            return Ok(());
        }
        self.backend.install(pkgs)
    }

    fn update(&self) -> Result<()> {
        if skip_dnf_commands() {
            log::info!("Skipping libdnf update (test stub)");
            return Ok(());
        }
        if self.dry_run {
            log::info!("DRY RUN: libdnf update");
            return Ok(());
        }
        self.backend.update()
    }
}

#[cfg(feature = "libdnf")]
fn use_libdnf_backend() -> bool {
    matches!(
        env::var("MASH_USE_LIBDNF").as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE") | Ok("yes") | Ok("YES")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_command_includes_packages() {
        let pkgs = vec!["vim".to_string(), "git".to_string()];
        let spec = install_command_spec(&pkgs);
        assert_eq!(spec.program, "dnf");
        assert_eq!(spec.args, vec!["install", "-y", "vim", "git"]);
    }

    #[test]
    fn update_command_is_expected() {
        let spec = update_command_spec();
        assert_eq!(spec.program, "dnf");
        assert_eq!(spec.args, vec!["update", "-y"]);
    }

    #[test]
    fn dry_run_does_not_execute() {
        let mgr = DnfShell::new(true);
        mgr.install(&["vim".to_string()]).unwrap();
        mgr.update().unwrap();
    }
}

#[cfg(all(test, feature = "libdnf"))]
mod libdnf_tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    #[derive(Debug, Default)]
    struct MockState {
        install_calls: Mutex<Vec<Vec<String>>>,
        update_calls: AtomicUsize,
        fail_install: bool,
        fail_update: bool,
    }

    #[derive(Debug, Clone, Default)]
    struct MockBackend {
        state: Arc<MockState>,
    }

    impl LibDnfBackend for MockBackend {
        fn install(&self, pkgs: &[String]) -> Result<()> {
            if self.state.fail_install {
                return Err(anyhow!("install failed"));
            }
            self.state.install_calls.lock().unwrap().push(pkgs.to_vec());
            Ok(())
        }

        fn update(&self) -> Result<()> {
            if self.state.fail_update {
                return Err(anyhow!("update failed"));
            }
            self.state.update_calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[test]
    fn libdnf_install_delegates_to_backend() {
        let backend = MockBackend::default();
        let state = backend.state.clone();
        let mgr = LibDnfPackageManager::with_backend(false, Box::new(backend));
        mgr.install(&["vim".to_string(), "git".to_string()])
            .unwrap();
        let calls = state.install_calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0], vec!["vim".to_string(), "git".to_string()]);
    }

    #[test]
    fn libdnf_update_delegates_to_backend() {
        let backend = MockBackend::default();
        let state = backend.state.clone();
        let mgr = LibDnfPackageManager::with_backend(false, Box::new(backend));
        mgr.update().unwrap();
        assert_eq!(state.update_calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn libdnf_dry_run_skips_backend() {
        let backend = MockBackend::default();
        let state = backend.state.clone();
        let mgr = LibDnfPackageManager::with_backend(true, Box::new(backend));
        mgr.install(&["vim".to_string()]).unwrap();
        mgr.update().unwrap();
        assert!(state.install_calls.lock().unwrap().is_empty());
        assert_eq!(state.update_calls.load(Ordering::SeqCst), 0);
    }
}
