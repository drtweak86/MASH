# WORK ORDER — WO-020: Grand Refactor (Crate Extraction)

## Goal:
Extract reusable crates from mash-installer/mash-core so the codebase becomes modular, testable, and fast to compile.

## Target crates (additive):
- crates/mash-hal (hardware + OS boundary; all world-touching code)
- crates/mash-tui (ratatui reusable widgets/components; no business logic)
- crates/mash-workflow (stage engine + step traits + orchestration; deterministic)

## Keep existing crates:
- crates/mash-core (types/state/errors/config)
- mash-installer (thin glue + app wiring)

## Phase plan (must include):

### Phase 0 — Audit & carve lines
- Identify “world-touching” modules (fs/proc/sysfs/shell) to move into mash-hal.
- Identify UI widgets/helpers to move into mash-tui.
- Identify stage runner + pipeline logic to move into mash-workflow.
Deliverable: mapping table (old path → new crate/path) + risk list.

### Phase 1 — Scaffolding (Larry)
- Create crates/mash-hal crates/mash-tui crates/mash-workflow with minimal lib.rs.
- Update workspace Cargo.toml members.
- Add empty public APIs and compile-only wiring (no behavior changes).
Acceptance: cargo fmt/clippy/test pass.

### Phase 2 — Parallel extraction (Claude + Larry, non-colliding)
Claude owns:
- Move reusable ratatui widget code into mash-tui.
- Provide tests for widget helpers where feasible (snapshot-like or logic-level tests).
Larry owns:
- Move OS/proc/sysfs parsing helpers into mash-hal.
- Move stage/pipeline runner logic into mash-workflow.
Rule: Claude does not touch pipeline/state_manager; Larry does not touch widget rendering files.

### Phase 3 — Glue thinning (Larry)
- mash-installer imports mash-tui + mash-workflow and becomes orchestration-only.
- mash-core remains shared types; mash-hal used by workflow for all world ops.
Acceptance: mash-installer behavior unchanged; all gates green.

## Testing requirements:
- Add unit tests in mash-hal for parsing/proc/sysfs/mount logic with fixtures.
- Add unit tests in mash-workflow for deterministic stage ordering + resume behavior using mock HAL.
- Keep CI deterministic (no real disk/network).

## Deliverables:
- docs/work-orders/WO-020-grand-refactor.md
- Parent GitHub issue “WO-020: Grand Refactor — Crate Extraction”
- Child issues split for parallelism:
  1) Scaffold new crates + workspace wiring (Larry)
  2) Extract mash-hal (Larry)
  3) Extract mash-workflow (Larry)
  4) Extract mash-tui (Claude)
  5) Thin mash-installer glue layer (Larry)
  6) Test harness + fixtures upgrades (Claude assists if needed)

## Constraints:
- This must be evolutionary: DO NOT rename existing crates or move mash-core/mash-installer in the first pass.
- Additive extraction only: create new crates and migrate modules gradually while keeping main buildable after each PR.
- No placeholder architecture diagrams; output must be an actionable WORK_ORDER.md with phases, file scopes, and merge order.
- Enable parallel work: Claude and Larry must be able to work without touching the same files.

## Completion Criteria:
- All OSes installable end-to-end (from previous WO, but contextually relevant)
- No “coming soon” labels anywhere (from previous WO, but contextually relevant)
- cargo fmt -- --check
- cargo clippy --all-targets --all-features -- -D warnings
- cargo test --all-targets

## Completion Message:
COMPLETE – Grand Refactor (Workspace Extraction) successfully completed. Codebase is now modular with mash-hal, mash-tui, and mash-workflow crates. All tests green, parallel work enabled.
