use once_cell::sync::Lazy;
use std::sync::{Mutex, MutexGuard};

/// Global lock to serialize tests that mutate process-wide environment variables (e.g. PATH).
static ENV_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

pub struct EnvLockGuard(#[allow(dead_code)] MutexGuard<'static, ()>);

pub fn lock() -> EnvLockGuard {
    let guard = match ENV_LOCK.lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };
    EnvLockGuard(guard)
}
