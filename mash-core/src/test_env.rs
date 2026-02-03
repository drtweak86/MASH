#[cfg(test)]
use once_cell::sync::Lazy;
#[cfg(test)]
use std::sync::{Mutex, MutexGuard};

/// Global lock to serialize tests that mutate process-wide environment variables (e.g. PATH).
#[cfg(test)]
static ENV_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

#[cfg(test)]
pub struct EnvLockGuard(MutexGuard<'static, ()>);

#[cfg(test)]
pub fn lock() -> EnvLockGuard {
    EnvLockGuard(ENV_LOCK.lock().expect("ENV_LOCK poisoned"))
}

