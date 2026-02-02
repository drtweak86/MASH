use anyhow::Context;
use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::OwnedObjectPath;

const SYSTEMD_DESTINATION: &str = "org.freedesktop.systemd1";
const SYSTEMD_PATH: &str = "/org/freedesktop/systemd1";
const SYSTEMD_INTERFACE: &str = "org.freedesktop.systemd1.Manager";

pub fn start_unit(conn: &Connection, unit: &str) -> anyhow::Result<()> {
    let proxy = manager_proxy(conn)?;
    let _: OwnedObjectPath = proxy
        .call("StartUnit", &(unit, "replace"))
        .context("Failed to start systemd unit")?;
    Ok(())
}

pub fn stop_unit(conn: &Connection, unit: &str) -> anyhow::Result<()> {
    let proxy = manager_proxy(conn)?;
    let _: OwnedObjectPath = proxy
        .call("StopUnit", &(unit, "replace"))
        .context("Failed to stop systemd unit")?;
    Ok(())
}

pub fn enable_unit_files(conn: &Connection, units: &[&str]) -> anyhow::Result<()> {
    let proxy = manager_proxy(conn)?;
    let _: (bool, Vec<(String, String, String)>) = proxy
        .call("EnableUnitFiles", &(units, false, true))
        .context("Failed to enable systemd unit files")?;
    Ok(())
}

fn manager_proxy(conn: &Connection) -> anyhow::Result<Proxy<'_>> {
    Proxy::new(conn, SYSTEMD_DESTINATION, SYSTEMD_PATH, SYSTEMD_INTERFACE)
        .context("Failed to create systemd manager proxy")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use zbus::blocking::ConnectionBuilder;
    use zbus::interface;

    #[derive(Clone, Default)]
    struct CallLog {
        entries: Arc<Mutex<Vec<String>>>,
    }

    impl CallLog {
        fn push(&self, entry: String) {
            self.entries.lock().unwrap().push(entry);
        }

        fn entries(&self) -> Vec<String> {
            self.entries.lock().unwrap().clone()
        }
    }

    struct MockManager {
        log: CallLog,
    }

    #[interface(name = "org.freedesktop.systemd1.Manager")]
    impl MockManager {
        fn start_unit(&self, name: &str, mode: &str) -> OwnedObjectPath {
            self.log.push(format!("StartUnit:{}:{}", name, mode));
            OwnedObjectPath::try_from("/org/freedesktop/systemd1/job/1").unwrap()
        }

        fn stop_unit(&self, name: &str, mode: &str) -> OwnedObjectPath {
            self.log.push(format!("StopUnit:{}:{}", name, mode));
            OwnedObjectPath::try_from("/org/freedesktop/systemd1/job/2").unwrap()
        }

        fn enable_unit_files(
            &self,
            files: Vec<&str>,
            runtime: bool,
            force: bool,
        ) -> (bool, Vec<(String, String, String)>) {
            self.log
                .push(format!("EnableUnitFiles:{:?}:{}:{}", files, runtime, force));
            (true, Vec::new())
        }
    }

    fn setup_mock_systemd() -> Option<(Connection, CallLog)> {
        let log = CallLog::default();
        let manager = MockManager { log: log.clone() };
        let builder = ConnectionBuilder::session().ok()?;
        let connection = builder
            .name(SYSTEMD_DESTINATION)
            .ok()?
            .serve_at(SYSTEMD_PATH, manager)
            .ok()?
            .build()
            .ok()?;
        Some((connection, log))
    }

    #[test]
    fn start_unit_calls_systemd() {
        let Some((conn, log)) = setup_mock_systemd() else {
            return;
        };
        start_unit(&conn, "mash-dojo.service").unwrap();
        assert_eq!(
            log.entries(),
            vec!["StartUnit:mash-dojo.service:replace".to_string()]
        );
    }

    #[test]
    fn stop_unit_calls_systemd() {
        let Some((conn, log)) = setup_mock_systemd() else {
            return;
        };
        stop_unit(&conn, "mash-dojo.service").unwrap();
        assert_eq!(
            log.entries(),
            vec!["StopUnit:mash-dojo.service:replace".to_string()]
        );
    }

    #[test]
    fn enable_unit_files_calls_systemd() {
        let Some((conn, log)) = setup_mock_systemd() else {
            return;
        };
        enable_unit_files(&conn, &["mash-dojo.service"]).unwrap();
        assert_eq!(
            log.entries(),
            vec!["EnableUnitFiles:[\"mash-dojo.service\"]:false:true".to_string()]
        );
    }
}
