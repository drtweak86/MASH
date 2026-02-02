use anyhow::anyhow;
use mash_installer::ui::confirm;
use std::sync::atomic::{AtomicBool, Ordering};

#[test]
fn destructive_requires_confirmation() {
    let ran = AtomicBool::new(false);
    let result = confirm::confirm_and_run_with(
        "Confirm destructive action?",
        |_prompt| Ok(false),
        || {
            ran.store(true, Ordering::SeqCst);
            Ok(())
        },
    )
    .expect("confirmation result");

    assert!(!result);
    assert!(!ran.load(Ordering::SeqCst));
}

#[test]
fn destructive_runs_on_yes() {
    let ran = AtomicBool::new(false);
    let result = confirm::confirm_and_run_with(
        "Confirm destructive action?",
        |_prompt| Ok(true),
        || {
            ran.store(true, Ordering::SeqCst);
            Ok(())
        },
    )
    .expect("confirmation result");

    assert!(result);
    assert!(ran.load(Ordering::SeqCst));
}

#[test]
fn destructive_aborts_on_prompt_error() {
    let ran = AtomicBool::new(false);
    let result = confirm::confirm_and_run_with(
        "Confirm destructive action?",
        |_prompt| Err(anyhow!("prompt cancelled")),
        || {
            ran.store(true, Ordering::SeqCst);
            Ok(())
        },
    );

    assert!(result.is_err());
    assert!(!ran.load(Ordering::SeqCst));
}
