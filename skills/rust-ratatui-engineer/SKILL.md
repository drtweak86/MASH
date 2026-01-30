---
name: rust-ratatui-engineer
description: "Disciplined Rust engineering for ratatui-based TUI installer flows with CI-safe, minimal changes. Use when working on mash-installer TUI code, step-based wizards, event-driven input handling, or when fixing Rust/clippy/test issues without unrelated refactors."
---

# Rust Ratatui Engineer

## Overview

Apply minimal, CI-safe Rust changes for ratatui TUI installer flows. Prioritize correctness, ownership clarity, and stable state-machine behavior.

## Scope

- Work only under `mash-installer/src/**`.
- Pay extra attention to `mash-installer/src/tui/new_app.rs`, `mash-installer/src/tui/new_ui.rs`, `mash-installer/src/tui/data_sources.rs`, `mash-installer/src/tui/progress.rs`.
- Touch GitHub workflows only if CI is failing or tooling is missing.

## Core Rules

- Keep CI green: `cargo fmt`, `cargo clippy -- -D warnings`, tests.
- Fix errors minimally and locally; never refactor unrelated code.
- Do not introduce async or threading unless explicitly requested.
- Do not invent features, abstractions, or UX changes.
- Do not change legacy or gated code.
- If unsure, stop and report risk instead of guessing.

## Engineering Principles

- Treat Rust + ratatui as first-class: idiomatic Rust, ownership-aware, clippy-clean.
- Respect TUI state machines, step-based wizards, and event-driven input handling.
- Prefer explicit state structs over implicit globals.

## Workflow

- Read the smallest relevant scope to understand the current state transitions.
- Identify the minimal fix; keep changes localized.
- Ensure state transitions remain explicit and deterministic.
- Validate CI expectations (fmt, clippy, tests) and avoid risky edits.
- If a safe fix is unclear, pause and surface the risk.

## Output Style

- Keep explanations concise.
- Provide bullet-point summaries.
- When behavior changes, include clear "What / Why / Risk".
