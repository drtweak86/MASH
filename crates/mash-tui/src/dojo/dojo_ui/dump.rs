use super::super::dojo_app::App;
use super::content::{
    build_dojo_lines, expected_actions, phase_line, progress_detail, status_message,
};

pub fn dump_step(app: &App) -> String {
    let progress_state = app.progress_state_snapshot();
    let dojo_lines = build_dojo_lines(app);
    let header = "MASH Installer";
    let dojo_hint = dojo_lines
        .first()
        .cloned()
        .unwrap_or_else(|| "🧭 Step: (unknown)".to_string());
    let body_lines = if dojo_lines.len() > 1 {
        dojo_lines[1..].join("\n")
    } else {
        "(no body content)".to_string()
    };
    let percent = progress_state.overall_percent.round().clamp(0.0, 100.0) as u16;
    let phase_line = phase_line(&progress_state);
    let eta_line = format!("⏱️ ETA: {}", progress_state.eta_string());
    let phase_percent = progress_state.phase_percent.round().clamp(0.0, 100.0) as u16;
    let overall_line = format!("📈 Overall: {}% | Phase: {}%", percent, phase_percent);
    let telemetry = progress_detail(&progress_state, &phase_line, &overall_line, &eta_line);
    let status = status_message(app, &progress_state);
    let actions = expected_actions(app.current_step_type);

    format!(
        "STEP: {}\n\n- Header: {}\n- Dojo hint line: {}\n- Body contents:\n{}\n- Footer/progress/telemetry/status blocks:\nProgress: {}%\nTelemetry: {}\nStatus: {}\n- Expected user actions (keys): {}\n",
        app.current_step_type.title(),
        header,
        dojo_hint,
        body_lines,
        percent,
        telemetry,
        status,
        actions
    )
}
