use crate::state_manager::{load_state, save_state_atomic, InstallState};
use anyhow::Result;
use std::path::PathBuf;

pub type StageFn<'a> = Box<dyn Fn(&mut InstallState, bool) -> Result<()> + 'a>;

pub struct StageDefinition<'a> {
    pub name: &'a str,
    pub run: StageFn<'a>,
}

pub struct StageRunner {
    state_path: PathBuf,
    dry_run: bool,
    persist: bool,
}

impl StageRunner {
    pub fn new(state_path: PathBuf, dry_run: bool) -> Self {
        Self {
            state_path,
            dry_run,
            persist: true,
        }
    }

    pub fn new_with_persist(state_path: PathBuf, dry_run: bool, persist: bool) -> Self {
        Self {
            state_path,
            dry_run,
            persist,
        }
    }

    pub fn run(&self, stages: &[StageDefinition<'_>]) -> Result<InstallState> {
        let mut state =
            load_state(&self.state_path)?.unwrap_or_else(|| InstallState::new(self.dry_run));

        for stage in stages {
            if state.is_completed(stage.name) {
                continue;
            }
            state.set_current(stage.name);
            if self.persist {
                save_state_atomic(&self.state_path, &state)?;
            }

            (stage.run)(&mut state, self.dry_run)?;

            state.mark_completed(stage.name);
            if self.persist {
                save_state_atomic(&self.state_path, &state)?;
            }
        }

        Ok(state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use tempfile::tempdir;

    #[test]
    fn runner_skips_completed_stages() {
        let dir = tempdir().unwrap();
        let state_path = dir.path().join("state.json");
        let mut state = InstallState::new(false);
        state.mark_completed("stage-1");
        save_state_atomic(&state_path, &state).unwrap();

        let calls = Arc::new(Mutex::new(Vec::new()));
        let calls_stage_1 = calls.clone();
        let calls_stage_2 = calls.clone();

        let stages = vec![
            StageDefinition {
                name: "stage-1",
                run: Box::new(move |_state, _dry_run| {
                    calls_stage_1.lock().unwrap().push("stage-1".to_string());
                    // TODO(larry): Re-enable when DownloadRecord is implemented
                    // state.record_download(crate::state_manager::DownloadRecord {
                    //     name: "dummy1".to_string(),
                    //     path: "/tmp/dummy1".to_string(),
                    //     size: 1,
                    //     checksum: "c1".to_string(),
                    //     resumed: false,
                    // });
                    Ok(())
                }),
            },
            StageDefinition {
                name: "stage-2",
                run: Box::new(move |_state, _dry_run| {
                    calls_stage_2.lock().unwrap().push("stage-2".to_string());
                    Ok(())
                }),
            },
        ];

        let runner = StageRunner::new(state_path, false);
        let final_state = runner.run(&stages).unwrap();

        assert_eq!(calls.lock().unwrap().as_slice(), &["stage-2"]);
        assert!(final_state.is_completed("stage-1"));
        assert!(final_state.is_completed("stage-2"));
    }
}
