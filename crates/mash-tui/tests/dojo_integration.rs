use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use mash_tui::dojo::dojo_app::{App, InputResult, InstallStepType};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    }
}

#[test]
fn end_to_end_flow_reaches_start_flash() {
    let mut app = App::new_with_flags(true); // dry-run to avoid destructive side effects

    app.handle_input(key(KeyCode::Enter));
    assert_eq!(app.current_step_type, InstallStepType::ImageSelection);

    app.handle_input(key(KeyCode::Enter));
    assert_eq!(app.current_step_type, InstallStepType::VariantSelection);

    app.handle_input(key(KeyCode::Enter));
    assert_eq!(
        app.current_step_type,
        InstallStepType::DownloadSourceSelection
    );

    app.handle_input(key(KeyCode::Enter));
    assert_eq!(app.current_step_type, InstallStepType::DiskSelection);

    app.handle_input(key(KeyCode::Char(' ')));
    app.handle_input(key(KeyCode::Enter));
    assert_eq!(app.current_step_type, InstallStepType::DiskConfirmation);

    for ch in "DESTROY".chars() {
        app.handle_input(key(KeyCode::Char(ch)));
    }
    app.handle_input(key(KeyCode::Enter));
    assert_eq!(app.current_step_type, InstallStepType::BackupConfirmation);

    app.backup_choice_index = 1;
    app.backup_confirmed = true;
    app.handle_input(key(KeyCode::Enter));
    assert_eq!(app.current_step_type, InstallStepType::PartitionScheme);

    app.handle_input(key(KeyCode::Enter));
    assert_eq!(app.current_step_type, InstallStepType::PartitionLayout);

    app.layout_selected = 0;
    app.handle_input(key(KeyCode::Enter));
    assert_eq!(app.current_step_type, InstallStepType::EfiImage);

    app.handle_input(key(KeyCode::Enter));
    assert_eq!(app.current_step_type, InstallStepType::LocaleSelection);

    app.handle_input(key(KeyCode::Enter));
    app.handle_input(key(KeyCode::Enter));
    app.handle_input(key(KeyCode::Enter));
    assert_eq!(app.current_step_type, InstallStepType::Confirmation);

    let result = app.handle_input(key(KeyCode::Enter));
    assert!(matches!(result, InputResult::StartFlash(_)));
    assert_eq!(app.current_step_type, InstallStepType::Flashing);
}
