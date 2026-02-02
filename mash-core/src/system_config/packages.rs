use anyhow::{anyhow, Result};
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
        if self.dry_run {
            log::info!("DRY RUN: dnf update -y");
            return Ok(());
        }
        let spec = update_command_spec();
        run_command(&spec)
    }
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

#[cfg(feature = "libdnf")]
pub trait LibDnfBackend {
    fn install(&self, pkgs: &[String]) -> Result<()>;
    fn update(&self) -> Result<()>;
}

#[cfg(feature = "libdnf")]
pub struct LibDnfPackageManager<B: LibDnfBackend> {
    backend: B,
}

#[cfg(feature = "libdnf")]
impl<B: LibDnfBackend> LibDnfPackageManager<B> {
    pub fn new(backend: B) -> Self {
        Self { backend }
    }
}

#[cfg(feature = "libdnf")]
impl<B: LibDnfBackend> PackageManager for LibDnfPackageManager<B> {
    fn install(&self, pkgs: &[String]) -> Result<()> {
        if pkgs.is_empty() {
            return Ok(());
        }
        self.backend.install(pkgs)
    }

    fn update(&self) -> Result<()> {
        self.backend.update()
    }
}

#[cfg(feature = "libdnf")]
#[derive(Debug, Default)]
pub struct RealLibDnfBackend;

#[cfg(feature = "libdnf")]
impl RealLibDnfBackend {
    pub fn new() -> Self {
        let _ = libdnf_sys::bindings_marker();
        Self
    }
}

#[cfg(feature = "libdnf")]
impl LibDnfBackend for RealLibDnfBackend {
    fn install(&self, _pkgs: &[String]) -> Result<()> {
        Err(anyhow!("libdnf backend not yet wired"))
    }

    fn update(&self) -> Result<()> {
        Err(anyhow!("libdnf backend not yet wired"))
    }
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
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct MockBackend {
        installs: Arc<Mutex<Vec<Vec<String>>>>,
        updates: Arc<Mutex<u32>>,
    }

    impl LibDnfBackend for MockBackend {
        fn install(&self, pkgs: &[String]) -> Result<()> {
            self.installs.lock().unwrap().push(pkgs.to_vec());
            Ok(())
        }

        fn update(&self) -> Result<()> {
            let mut count = self.updates.lock().unwrap();
            *count += 1;
            Ok(())
        }
    }

    #[test]
    fn libdnf_manager_calls_backend() {
        let backend = MockBackend::default();
        let manager = LibDnfPackageManager::new(backend.clone());

        manager
            .install(&["vim".to_string(), "git".to_string()])
            .unwrap();
        manager.update().unwrap();

        let installs = backend.installs.lock().unwrap();
        assert_eq!(installs.len(), 1);
        assert_eq!(installs[0], vec!["vim".to_string(), "git".to_string()]);
        assert_eq!(*backend.updates.lock().unwrap(), 1);
    }

    #[test]
    fn libdnf_manager_skips_empty_install() {
        let backend = MockBackend::default();
        let manager = LibDnfPackageManager::new(backend.clone());

        manager.install(&[]).unwrap();

        let installs = backend.installs.lock().unwrap();
        assert!(installs.is_empty());
    }
}
