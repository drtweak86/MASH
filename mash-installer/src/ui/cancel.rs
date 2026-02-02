//! Ctrl+C cancellation handling.

use std::sync::OnceLock;

static HANDLER_SET: OnceLock<()> = OnceLock::new();

pub fn install_ctrlc_handler<F>(on_cancel: F) -> anyhow::Result<()>
where
    F: Fn() + Send + Sync + 'static,
{
    if HANDLER_SET.get().is_some() {
        return Ok(());
    }

    ctrlc::set_handler(move || {
        on_cancel();
        log::info!("Cancellation requested (Ctrl+C).");
    })?;

    let _ = HANDLER_SET.set(());
    Ok(())
}
