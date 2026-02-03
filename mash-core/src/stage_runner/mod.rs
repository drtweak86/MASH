use crate::state_manager::{load_state, save_state_atomic, InstallState};
use anyhow::Result;
use mash_workflow::stage_runner as wf;
use std::path::PathBuf;

pub type StageFn<'a> = wf::StageFn<'a, InstallState>;
pub type StageDefinition<'a> = wf::StageDefinition<'a, InstallState>;

#[derive(Clone)]
struct InstallStateFileStore {
    state_path: PathBuf,
}

impl wf::StateStore<InstallState> for InstallStateFileStore {
    fn load(&self) -> Result<Option<InstallState>> {
        load_state(&self.state_path)
    }

    fn save(&self, state: &InstallState) -> Result<()> {
        save_state_atomic(&self.state_path, state)
    }
}

impl wf::WorkflowState for InstallState {
    fn is_completed(&self, stage: &str) -> bool {
        self.is_completed(stage)
    }

    fn set_current(&mut self, stage: &str) {
        self.set_current(stage);
    }

    fn mark_completed(&mut self, stage: &str) {
        self.mark_completed(stage);
    }
}

pub struct StageRunner {
    inner: wf::StageRunner<InstallState, InstallStateFileStore>,
}

impl StageRunner {
    pub fn new(state_path: PathBuf, dry_run: bool) -> Self {
        let store = InstallStateFileStore { state_path };
        Self {
            inner: wf::StageRunner::new(store, dry_run, InstallState::new),
        }
    }

    pub fn new_with_persist(state_path: PathBuf, dry_run: bool, persist: bool) -> Self {
        let store = InstallStateFileStore { state_path };
        Self {
            inner: wf::StageRunner::new_with_persist(store, dry_run, persist, InstallState::new),
        }
    }

    pub fn run(&self, stages: &[StageDefinition<'_>]) -> Result<InstallState> {
        self.inner.run(stages)
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
        let calls_stage_1 = Arc::clone(&calls);
        let calls_stage_2 = Arc::clone(&calls);

        let stages = vec![
            StageDefinition {
                name: "stage-1",
                run: Box::new(move |_state, _dry_run| {
                    calls_stage_1.lock().unwrap().push("stage-1".to_string());
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
