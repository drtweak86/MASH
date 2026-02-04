use anyhow::Result;
use mash_core::state_manager::{load_state, save_state_atomic, InstallState, StageName};
use std::path::PathBuf;

use crate::stage_runner as wf;

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
    fn is_completed(&self, stage: &StageName) -> bool {
        self.is_completed(stage)
    }

    fn set_current(&mut self, stage: &StageName) {
        self.set_current(stage);
    }

    fn mark_completed(&mut self, stage: &StageName) {
        self.mark_completed(stage);
    }

    fn ensure_armed(&self) -> anyhow::Result<()> {
        self.ensure_armed()
    }

    fn arm_execute(&mut self) {
        self.arm_execute()
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

    pub fn with_require_armed(mut self, require: bool) -> Self {
        self.inner = self.inner.with_require_armed(require);
        self
    }

    pub fn run(&self, stages: &[StageDefinition<'_>]) -> Result<InstallState> {
        self.inner.run(stages)
    }
}
