# MASH TUI Roadmap

This document summarizes the phased TUI evolution and the next recommended steps.

---

## Phase B1 - Stub-backed UI state

Goal:
- Get the entire Dojo UI rendering end-to-end without any real disk logic.

Why:
- Enable rapid UI iteration while keeping the system safe and CI green.

How:
- Added `tui/dojo_app.rs` + `tui/dojo_ui.rs` with in-memory stub options.
- Ensured forward/back navigation and a coherent confirmation summary.

Result:
- Every step renders at least one selectable option.
- No real disk scanning, downloads, or flashing.

---

## Phase B2 - TUI flow completion (stub-safe)

Goal:
- Make all steps reachable and replace placeholder copy with real stub data.

Why:
- Avoid dead ends before introducing real data sources.

How:
- Rewired step transitions to include `PartitionCustomize`.
- Added input handling for download steps using stub state.
- Replaced placeholder content with selectable lists.

Result:
- Full flow from Welcome → Complete is navigable using only stub data.

---

## Phase B3 - Read-only data plumbing (feature-flagged)

Goal:
- Start using real data sources without enabling side effects.

Why:
- Validate UI against real-world data while remaining non-destructive.

How:
- Added `tui/data_sources.rs` with read-only collectors.
- Feature flags:
  - `MASH_TUI_REAL_DATA=1` enables all read-only sources.
  - `MASH_TUI_REAL_DISKS=1` enables `/sys/block` disk scan.
  - `MASH_TUI_REAL_IMAGES=1` enables local image metadata scan + remote metadata list.
  - `MASH_TUI_REAL_LOCALES=1` enables locale/keymap lists from system files.
  - `MASH_TUI_IMAGE_DIRS=/path1:/path2` overrides local image scan paths.
- Confirmation summary now reflects real selections when enabled.

Result:
- Default remains stubbed and safe.
- Real lists can be tested without downloads or disk writes.

---

## Recommended Next Phase: Phase C - Real plumbing with destructive gates

Goal:
- Connect real operations (download/flash) behind explicit, opt-in switches.

Why:
- Keep safety defaults while enabling end-to-end testing when intended.

Suggested approach:
- Introduce `--enable-destructive` (CLI) or explicit "arming" flow (TUI).
- Keep read-only scanning always allowed, but hard-gate any writes.
- Reuse existing progress channels for live telemetry.

Success criteria:
- Real downloads and flashes are possible only when explicitly armed.
- Default path remains safe and stubbed.
