# ADR 001: Dojo Program Catalogue Schema

## Status
Accepted — 2026-02-02

## Context
The Dojo Program Catalogue drives the curated software choices that users encounter in the TUI. The catalogue must express categories, defaults, and safe alternatives while describing package names for every supported distribution. We need a Rust-friendly representation that can be loaded with `serde` and stays close to the existing TOML-heavy configuration style in this repository.

## Decision
- Format: continue using **TOML** because the repo already uses it for configuration, it deserializes cleanly with `serde`, and it supports nested table arrays that align with category/program grouping.
- Schema shape:
  - `[[category]]` anchors each intent group (e.g., "Web Browser"). Fields per category include `id`, `label`, `description`, and a `programs` table array.
  - Each `[[category.programs]]` entry represents a curated choice and exposes:
    - `id`, `label`, `description` (user-facing metadata).
    - `default` (boolean, only one per category should be true).
    - `package_names` (map of distribution keys to `string[]`, e.g., `dnf`, `apt`, `pacman`).
    - `reason_why` (concise explanation of the trade-off).
    - Optional gating metadata (`requires_expert_mode`, `gated_message`).
- The schema can be loaded into Rust via the structs below, enabling multi-distro lookups and gating logic.

## Minimal Rust prototype
```rust
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct DojoCatalogue {
    pub categories: Vec<Category>,
}

#[derive(Debug, Deserialize)]
pub struct Category {
    pub id: String,
    pub label: String,
    pub description: String,
    pub programs: Vec<Program>,
}

#[derive(Debug, Deserialize)]
pub struct Program {
    pub id: String,
    pub label: String,
    pub description: String,
    pub default: bool,
    pub package_names: HashMap<String, Vec<String>>,
    pub reason_why: String,
    pub gating: Option<Gating>,
}

#[derive(Debug, Deserialize)]
pub struct Gating {
    pub requires_expert_mode: bool,
    pub gated_message: Option<String>,
}
```

## Consequences
- Documentation and tooling can continue to favor TOML while Rust code deserializes directly into strongly typed structs.
- Multi-distro support is explicit via the `package_names` map, enabling installers to pick matching package managers at runtime.
- The gating metadata lets the UI hide advanced choices unless `requires_expert_mode` is satisfied.
- The example file at `docs/architecture/dojo_catalogue_schema_example.toml` illustrates the full schema and can be treated as input for future catalogue readers.
