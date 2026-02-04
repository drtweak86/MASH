use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

static CANCEL_FLAG: OnceLock<Mutex<Option<Arc<AtomicBool>>>> = OnceLock::new();

pub fn set_cancel_flag(flag: Arc<AtomicBool>) {
    let lock = CANCEL_FLAG.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = lock.lock() {
        *guard = Some(flag);
    }
}

pub fn clear_cancel_flag() {
    if let Some(lock) = CANCEL_FLAG.get() {
        if let Ok(mut guard) = lock.lock() {
            *guard = None;
        }
    }
}

pub(super) fn cancel_requested() -> bool {
    CANCEL_FLAG
        .get()
        .and_then(|lock| lock.lock().ok())
        .and_then(|guard| guard.as_ref().map(|flag| flag.load(Ordering::Relaxed)))
        .unwrap_or(false)
}
