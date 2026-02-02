//! Confirmation helpers for destructive operations.

use anyhow::{Context, Result};
use dialoguer::Confirm;

pub fn confirm_destructive_action(prompt: &str) -> Result<bool> {
    Confirm::new()
        .with_prompt(prompt)
        .default(false)
        .interact()
        .context("Failed to read confirmation input")
}

pub fn confirm_and_run_with<C, A>(prompt: &str, confirm: C, action: A) -> Result<bool>
where
    C: FnOnce(&str) -> Result<bool>,
    A: FnOnce() -> Result<()>,
{
    if confirm(prompt)? {
        action()?;
        Ok(true)
    } else {
        Ok(false)
    }
}

pub fn confirm_and_run<A>(prompt: &str, action: A) -> Result<bool>
where
    A: FnOnce() -> Result<()>,
{
    confirm_and_run_with(prompt, confirm_destructive_action, action)
}
