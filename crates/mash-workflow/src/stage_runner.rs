use anyhow::Result;
use mash_core::state_manager::StageName;

pub type StageFn<'a, S> = Box<dyn Fn(&mut S, bool) -> Result<()> + 'a>;

pub struct StageDefinition<'a, S> {
    pub name: StageName,
    pub run: StageFn<'a, S>,
}

pub trait WorkflowState {
    fn is_completed(&self, stage: &StageName) -> bool;
    fn set_current(&mut self, stage: &StageName);
    fn mark_completed(&mut self, stage: &StageName);
    fn ensure_armed(&self) -> Result<()>;
    fn arm_execute(&mut self);
}

pub trait StateStore<S>: Send + Sync {
    fn load(&self) -> Result<Option<S>>;
    fn save(&self, state: &S) -> Result<()>;
}

pub struct StageRunner<S, Store> {
    store: Store,
    dry_run: bool,
    persist: bool,
    init_state: Box<dyn Fn(bool) -> S + Send + Sync>,
    require_armed: bool,
}

impl<S, Store> StageRunner<S, Store>
where
    S: WorkflowState,
    Store: StateStore<S>,
{
    pub fn new(
        store: Store,
        dry_run: bool,
        init_state: impl Fn(bool) -> S + Send + Sync + 'static,
    ) -> Self {
        Self {
            store,
            dry_run,
            persist: true,
            init_state: Box::new(init_state),
            require_armed: false,
        }
    }

    pub fn new_with_persist(
        store: Store,
        dry_run: bool,
        persist: bool,
        init_state: impl Fn(bool) -> S + Send + Sync + 'static,
    ) -> Self {
        Self {
            store,
            dry_run,
            persist,
            init_state: Box::new(init_state),
            require_armed: false,
        }
    }

    pub fn with_require_armed(mut self, require: bool) -> Self {
        self.require_armed = require;
        self
    }

    pub fn run(&self, stages: &[StageDefinition<'_, S>]) -> Result<S> {
        let mut state = self
            .store
            .load()?
            .unwrap_or_else(|| (self.init_state)(self.dry_run));

        if self.require_armed {
            // first-run: arm state so subsequent resumes stay armed
            state.arm_execute();
            if self.persist {
                self.store.save(&state)?;
            }
        }

        for stage in stages {
            if self.require_armed {
                state.ensure_armed()?;
            }
            if state.is_completed(&stage.name) {
                continue;
            }
            state.set_current(&stage.name);
            if self.persist {
                self.store.save(&state)?;
            }

            (stage.run)(&mut state, self.dry_run)?;

            state.mark_completed(&stage.name);
            if self.persist {
                self.store.save(&state)?;
            }
        }

        Ok(state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mash_core::state_manager::StageName;
    use std::collections::BTreeSet;
    use std::sync::{Arc, Mutex};

    #[derive(Debug, Clone, Default)]
    struct TestState {
        current: Option<StageName>,
        completed: BTreeSet<StageName>,
        dry_run: bool,
    }

    impl TestState {
        fn new(dry_run: bool) -> Self {
            Self {
                current: None,
                completed: BTreeSet::new(),
                dry_run,
            }
        }
    }

    impl WorkflowState for TestState {
        fn is_completed(&self, stage: &StageName) -> bool {
            self.completed.contains(stage)
        }

        fn set_current(&mut self, stage: &StageName) {
            self.current = Some(stage.clone());
        }

        fn mark_completed(&mut self, stage: &StageName) {
            self.completed.insert(stage.clone());
        }

        fn ensure_armed(&self) -> Result<()> {
            Ok(())
        }

        fn arm_execute(&mut self) {}
    }

    #[derive(Clone, Default)]
    struct MemStore<S>(Arc<Mutex<Option<S>>>);

    impl<S: Clone + Send + 'static> StateStore<S> for MemStore<S> {
        fn load(&self) -> Result<Option<S>> {
            Ok(self.0.lock().unwrap().clone())
        }

        fn save(&self, state: &S) -> Result<()> {
            *self.0.lock().unwrap() = Some(state.clone());
            Ok(())
        }
    }

    #[test]
    fn runner_skips_completed_stages() {
        let store = MemStore::<TestState>::default();
        store
            .save(&TestState {
                current: None,
                completed: [StageName::Other("stage-1".to_string())]
                    .into_iter()
                    .collect(),
                dry_run: false,
            })
            .unwrap();

        let calls = Arc::new(Mutex::new(Vec::new()));
        let calls_stage_1 = Arc::clone(&calls);
        let calls_stage_2 = Arc::clone(&calls);

        let stages = vec![
            StageDefinition {
                name: StageName::Other("stage-1".into()),
                run: Box::new(move |_state, _dry_run| {
                    calls_stage_1.lock().unwrap().push("stage-1".to_string());
                    Ok(())
                }),
            },
            StageDefinition {
                name: StageName::Other("stage-2".into()),
                run: Box::new(move |_state, _dry_run| {
                    calls_stage_2.lock().unwrap().push("stage-2".to_string());
                    Ok(())
                }),
            },
        ];

        let runner = StageRunner::new(store.clone(), false, TestState::new);
        let final_state = runner.run(&stages).unwrap();

        assert_eq!(calls.lock().unwrap().as_slice(), &["stage-2"]);
        assert!(final_state.is_completed(&StageName::Other("stage-1".into())));
        assert!(final_state.is_completed(&StageName::Other("stage-2".into())));
        assert!(store
            .load()
            .unwrap()
            .unwrap()
            .is_completed(&StageName::Other("stage-2".into())));
    }

    #[test]
    fn runner_initializes_state_with_dry_run_flag() {
        let store = MemStore::<TestState>::default();
        let runner = StageRunner::new(store, true, TestState::new);
        let state = runner.run(&[]).unwrap();
        assert!(state.dry_run);
    }

    #[test]
    fn runner_can_disable_persistence() {
        let store = MemStore::<TestState>::default();
        let stages = vec![StageDefinition {
            name: StageName::Other("stage-1".into()),
            run: Box::new(|_state, _dry_run| Ok(())),
        }];

        let runner = StageRunner::new_with_persist(store.clone(), false, false, TestState::new);
        let state = runner.run(&stages).unwrap();
        assert!(state.is_completed(&StageName::Other("stage-1".into())));
        assert!(store.load().unwrap().is_none());
    }
}
