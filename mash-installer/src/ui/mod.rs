//! Shared UI helpers for TUI and CLI flows.

use std::io::IsTerminal;

pub mod cancel;
pub mod confirm;
pub mod style;
pub mod validation;

pub fn ensure_interactive_terminal() -> anyhow::Result<()> {
    if std::io::stdout().is_terminal() {
        return Ok(());
    }

    anyhow::bail!(
        "No TTY detected. The TUI requires an interactive terminal.\n\
         Try running directly in a terminal (not piped or via script).\n\
         If using sudo, try: sudo -E mash"
    );
}
