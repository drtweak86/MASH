use crate::system_config::services;
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use zbus::blocking::Connection;

const UNIT_NAME: &str = "mash-core-resume.service";

pub fn resume_unit_name() -> &'static str {
    UNIT_NAME
}

pub fn render_resume_unit(exec_path: &Path, state_path: &Path) -> String {
    format!(
        "[Unit]\nDescription=MASH Core Resume\nAfter=network-online.target\n\n[Service]\nType=oneshot\nExecStart={} --resume --state {}\n\n[Install]\nWantedBy=multi-user.target\n",
        exec_path.display(),
        state_path.display()
    )
}

pub fn install_resume_unit(root: &Path, content: &str) -> Result<PathBuf> {
    let unit_dir = root.join("etc/systemd/system");
    fs::create_dir_all(&unit_dir)
        .with_context(|| format!("Failed to create {}", unit_dir.display()))?;
    let unit_path = unit_dir.join(UNIT_NAME);
    backup_if_exists(&unit_path)?;
    fs::write(&unit_path, content)
        .with_context(|| format!("Failed to write {}", unit_path.display()))?;
    Ok(unit_path)
}

pub fn enable_resume_unit(conn: &Connection) -> Result<()> {
    services::enable_unit_files(conn, &[UNIT_NAME])
}

pub fn request_reboot(dry_run: bool) -> Result<()> {
    if dry_run {
        log::info!("DRY RUN: reboot requested.");
        return Ok(());
    }
    log::info!("Reboot requested (no-op scaffold).");
    Ok(())
}

fn backup_if_exists(path: &Path) -> Result<()> {
    if path.exists() {
        let backup = path.with_file_name(format!(
            "{}.bak",
            path.file_name().and_then(|n| n.to_str()).unwrap_or("unit")
        ));
        fs::copy(path, &backup).with_context(|| {
            format!(
                "Failed to backup {} to {}",
                path.display(),
                backup.display()
            )
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use zbus::blocking::ConnectionBuilder;
    use zbus::interface;
    use zbus::zvariant::OwnedObjectPath;

    struct MockManager;

    #[interface(name = "org.freedesktop.systemd1.Manager")]
    impl MockManager {
        fn enable_unit_files(
            &self,
            _files: Vec<&str>,
            _runtime: bool,
            _force: bool,
        ) -> (bool, Vec<(String, String, String)>) {
            (true, Vec::new())
        }

        fn start_unit(&self, _name: &str, _mode: &str) -> OwnedObjectPath {
            OwnedObjectPath::try_from("/org/freedesktop/systemd1/job/1").unwrap()
        }

        fn stop_unit(&self, _name: &str, _mode: &str) -> OwnedObjectPath {
            OwnedObjectPath::try_from("/org/freedesktop/systemd1/job/2").unwrap()
        }
    }

    fn mock_connection() -> Option<Connection> {
        let builder = ConnectionBuilder::session().ok()?;
        let connection = builder
            .name("org.freedesktop.systemd1")
            .ok()?
            .serve_at("/org/freedesktop/systemd1", MockManager)
            .ok()?
            .build()
            .ok()?;
        Some(connection)
    }

    #[test]
    fn render_unit_contains_exec_and_state() {
        let content = render_resume_unit(
            Path::new("/usr/bin/mash"),
            Path::new("/var/lib/mash/state.json"),
        );
        assert!(
            content.contains("ExecStart=/usr/bin/mash --resume --state /var/lib/mash/state.json")
        );
    }

    #[test]
    fn install_resume_unit_writes_file_and_backup() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let content = "unit-content";
        let path = install_resume_unit(root, content).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), content);

        let updated = "updated";
        install_resume_unit(root, updated).unwrap();
        let backup = path.with_file_name("mash-core-resume.service.bak");
        assert_eq!(fs::read_to_string(&backup).unwrap(), content);
    }

    #[test]
    fn enable_resume_unit_uses_zbus() {
        let Some(conn) = mock_connection() else {
            return;
        };
        enable_resume_unit(&conn).unwrap();
    }

    #[test]
    fn request_reboot_is_noop_in_tests() {
        request_reboot(true).unwrap();
    }
}
