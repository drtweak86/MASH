# 📄 `REFACTORING_SWEEP_REPORT.md`

Scope: read-only analysis of `mash-core`, `crates/mash-hal`, `crates/mash-workflow`, `crates/mash-tui`, `bin/larry_cli`, and workspace glue (excluding `/legacy` + `/legacy_scripts`).

## 1. Executive Summary (high-level wins)

- **Biggest entropy sources are monolith files**: `crates/mash-tui/src/dojo/dojo_app.rs` (~2652 LOC), `mash-core/src/flash.rs` (~1808 LOC), `crates/mash-workflow/src/installer/pipeline.rs` (~1289 LOC), `crates/mash-tui/src/dojo/dojo_ui.rs` (~1155 LOC), `mash-core/src/downloader/mod.rs` (~1136 LOC). These concentrate multiple responsibilities, making behavior harder to reason about, test, and keep deterministic.
- **HAL boundary is still leaky**: several “system ops” occur outside `mash_hal` (direct `std::process::Command`, direct `/proc` reads, formatting helpers outside `mash_hal`), which weakens test isolation and makes “all destructive ops through HAL” harder to enforce consistently.
- **Safety invariants are partially centralized but still duplicated**: typed confirmation strings exist centrally (`mash-core/src/config_states.rs:9-10`), yet the TUI also carries hard-coded strings and UX copy, inviting drift.
- **Cancellation/reporting/state are implemented but split across layers**: `mash-core` owns atomic state/report writing, while `mash-workflow` owns a parallel “StageRunner + Store” abstraction. Consolidating ownership would reduce duplication and improve resumability guarantees.
- **Low-risk performance wins exist in disk scanning + UI render plumbing**: avoid repeated sysfs/procfs reads and reduce allocation churn in hot UI formatting paths; prefer cached snapshots + preformatted labels.

## 2. High-Impact Refactors (worth doing soon)

### 2.1 Make `mash_hal` the *only* home for system commands + privileged ops

**Why:** The project is aiming for deterministic, resumable installs with strong safety constraints. Leaky boundaries make it too easy to accidentally bypass safety/timeouts and too hard to unit-test workflows without hitting the host system.

**Concrete targets (current leaks):**
- Formatting commands exist outside HAL: `mash-core/src/disk_ops/format.rs:1-86` uses `std::process::Command` to run `mkfs.*` and a second timeout layer (`mash-core/src/process_timeout.rs:11-75`).
- Non-HAL system commands: `mash-core/src/system_config/packages.rs`, `mash-core/src/stages/stage_01_stage_bootstrap.rs`, `mash-core/src/stages/stage_03_fail2ban_lite.rs`, etc. (see `rg "std::process::Command"` hits).
- Workflow reads `/proc` and uses `std::fs` for system-affecting paths: `crates/mash-workflow/src/installer/pipeline.rs:346` reads mountinfo directly; `crates/mash-workflow/src/installer/pipeline.rs:370` creates mount dirs directly.

**Recommended refactor shape:**
- Delete or internalize `mash-core/src/process_timeout.rs` and route all command execution through `crates/mash-hal/src/hal/linux_hal.rs` (already has structured timeouts + typed errors: `crates/mash-hal/src/hal/linux_hal.rs:27-96`, `crates/mash-hal/src/error.rs:5-31`).
- Move the non-HAL formatting path to call `mash_hal::FormatOps` exclusively, or remove `mash-core/src/disk_ops/format.rs` entirely after callers are migrated.
- Add narrow HAL traits for the remaining “system config” actions the workflow needs (e.g., read mountinfo, systemd unit writes/enables, package manager actions) so workflows can be tested via `FakeHal`/mocks without `Command` calls.

### 2.2 Consolidate “format/mount/rsync/disk identity” types so there’s one canonical model

**Why:** Duplicate “options structs” and parallel type definitions cause subtle divergence and prevent simple trait-based composition across crates.

**Evidence: duplicate `FormatOptions` types**
- `mash-core/src/disk_ops/format.rs:7-11`
- `crates/mash-hal/src/hal/format_ops.rs:27-35`

**Other split ownership worth collapsing:**
- Disk inventory and identity logic spans TUI + HAL:
  - HAL scan: `crates/mash-hal/src/sysfs/block.rs:38-45`
  - TUI scan + identity build: `crates/mash-tui/src/dojo/data_sources.rs:178-221` (plus `fs::read_link` in transport detection: `crates/mash-tui/src/dojo/data_sources.rs:224-250`)

**Recommended refactor shape:**
- Promote a single “disk inventory” model into `mash-hal` (e.g., `BlockDeviceInfo` + identity methods). TUI should treat it as immutable data and avoid sysfs link reads directly.
- Keep UI-only display helpers in TUI, but remove any disk classification logic that depends on sysfs parsing from the UI layer.

### 2.3 Merge state persistence responsibilities (reduce two stage-runner systems into one)

**Why:** Resumability/determinism depends on a single source of truth for:
1) what’s completed, 2) what’s current, and 3) what artifacts exist.

**Evidence: two parallel systems**
- `mash-core` owns state file format + atomic save/load: `mash-core/src/state_manager/mod.rs:117-156`
- `mash-workflow` owns an abstract stage runner + store: `crates/mash-workflow/src/stage_runner.rs:10-85`

**Risk today:** it’s easy for workflow orchestration to drift from persisted state semantics (or to partially persist in one layer and not the other).

**Recommended refactor shape:**
- Make `mash-workflow` depend on a single “state store” implementation from `mash-core` (or move the state store to `mash-workflow`, but not both).
- Encode “stage id” as an enum (not `String`) at the workflow boundary, and serialize it in a stable representation (reduces drift and typo-class bugs).
- Add explicit versioned migrations for `InstallState.version` (currently `version: 1` with no migration logic: `mash-core/src/state_manager/mod.rs:31-49`).

### 2.4 Break up `mash-core/src/flash.rs` by responsibility, without changing behavior

**Why:** This file is doing multiple concerns (mount layout, partition ops, fs ops, rsync parsing/progress, cancellation global state, staging templates, report/status messaging). That prevents focused testing and encourages cross-cutting changes.

**Concrete seams already visible:**
- Global cancellation mechanism: `mash-core/src/flash.rs:1196-1219` (static `OnceLock` + `Mutex<Option<Arc<AtomicBool>>>>`)
- Filesystem writes + permissions: e.g. `mash-core/src/flash.rs:1582-1586`, `mash-core/src/flash.rs:1604-1609`

**Recommended split (module-level):**
- `flash/cancel.rs`: cancellation token wiring (eliminate global where feasible)
- `flash/mounts.rs`: mount planning + mount/unmount
- `flash/rsync.rs`: rsync invocation + progress parse
- `flash/staging.rs`: dojo/firstboot staging templates
- `flash/fstab.rs`: fstab generation + uuid lookup

Goal is not “abstraction purity” but smaller, testable units with explicit inputs/outputs.

## 3. Medium / Nice-to-Have Refactors

- **Centralize typed confirmation strings and UI copy**: constants exist in one place (`mash-core/src/config_states.rs:9-10`), but TUI still hardcodes or mirrors strings in multiple flows (e.g., tests and UI render). Move “display text” to a single UI module that references core constants to prevent drift.
- **Remove `println!` from TUI crate entirely**: `crates/mash-tui/src/dojo/mod.rs:72-79` uses `println!` (even if debug-only). Make debug dumps write to a file or log sink so TUI never writes to stdout (stdout breaks terminal UI invariants).
- **Normalize “validation” location**: partition size parsing/validation is embedded in TUI (`crates/mash-tui/src/dojo/dojo_app.rs:1926-2038`). Consider moving invariant checks into `mash-core` config validation so CLI and TUI share the same safety logic.
- **Make `OsDistro/Variant` availability consistent**: variant label mapping in TUI (`crates/mash-tui/src/dojo/dojo_app.rs:1900-1919`) hardcodes versioned strings. Prefer generating these from a single “catalogue” source (or `docs/os-download-links.toml`) to avoid UI drift.

## 4. Performance Improvements

Low-risk wins that should not change external behavior:

- **Disk scanning caching**: disk scan reads sysfs + resolves links (`crates/mash-tui/src/dojo/data_sources.rs:178-260`). It is currently invoked on explicit rescan and on destructive transitions (good), but the scan itself can be made cheaper by:
  - caching the last scan snapshot + timestamp, and
  - moving transport detection into `mash-hal` so the UI does not do per-disk `read_link` work.
- **Reduce allocation churn in TUI draw**: `crates/mash-tui/src/dojo/dojo_ui.rs` is large (~1155 LOC). Many render-path helpers likely allocate `String`s each frame (common ratatui pitfall). Consider precomputing stable labels (disk identity, distro/variant strings) on state changes rather than in `draw`.
- **Avoid repeated cloning in flash pipeline**: `mash-core/src/flash.rs` frequently constructs formatted strings and clones config fields when emitting status/progress. Push toward `&str`/`Cow<'_>` where it is obviously safe and reduces churn; keep lifetime complexity low.
- **Checksum verification is streaming (good) but could fail late**: downloader verifies by streaming file (`mash-core/src/downloader/mod.rs:155-171`), which is correct. Add cheap early checks (e.g., “downloaded bytes > 0” and “size matches Content-Length when provided”) to fail faster on 0-byte downloads and obvious truncations.

## 5. API / Crate Boundary Cleanup

- **Choose one error philosophy at the crate boundary**:
  - `mash-hal` uses typed `HalError` (`crates/mash-hal/src/error.rs:5-46`)
  - `mash-core` exposes `MashError` but returns `anyhow::Result` (`mash-core/src/errors.rs:4-36`)
  - Many call sites use `anyhow::Error::new` mappings (e.g., `mash-core/src/flash.rs:1189-1192`, `mash-core/src/flash.rs:1590-1594`)

  Recommended: keep typed errors inside library crates and convert to `anyhow` only at binary boundary (`bin/larry_cli`). This improves test assertions and reduces “stringly failure” surfaces.

- **Stop duplicating timeout logic**: both `mash-core/src/process_timeout.rs` and `crates/mash-hal/src/hal/linux_hal.rs` implement “spawn with timeout + drain output”. Keep one (prefer HAL) to enforce consistent behavior.

- **Disk model should be HAL-owned**: the UI should not interpret sysfs path tokens or decide transport types via path parsing (`crates/mash-tui/src/dojo/data_sources.rs:224-250`). Move to HAL so it can be mocked and tested centrally.

- **Avoid workflow reading `/proc` and writing target dirs directly**: provide HAL helpers for mount dir creation and for reading `/proc/self/mountinfo` through a single path, to avoid portability issues and to enable isolation.

## 6. Testing Improvements

- **Introduce `FakeHal` as the default workflow test harness**:
  - Unit test “safety invariants” (boot disk exclusion, SAFE/ARMED gating, one-shot restrictions) without touching the host.
  - Unit test “resume determinism” by replaying persisted state and ensuring identical planned actions.

- **Add invariant tests around state versioning**:
  - `InstallState.version` exists (`mash-core/src/state_manager/mod.rs:31-49`) but there is no migration layer. Add a test that loads a “v0 fixture” and ensures defaults are applied and semantics preserved.

- **Harden downloader tests for truncation/0-byte files**:
  - Add a test case where server returns 200 with zero bytes; ensure it errors early (or at least does not proceed).
  - Add a test for “checksum mismatch” already exists implicitly via retry logic; make it explicit and deterministic.

- **Eliminate `println!`-dependent tests or debug paths**:
  - Debug helpers should write to a buffer/log sink so tests can assert behavior without polluting stdout (important for Maelstrom/isolation).

## 7. Explicit Non-Goals (things NOT worth touching)

- No sweeping renames or “style harmonization” across crates.
- No macro-heavy refactors in TUI rendering (ratatui is already complex; keep changes mechanical).
- No switching async runtimes or rewriting downloader to async unless there is a measured bottleneck.
- No “perfect abstraction layering” crusade; only changes that reduce bugs, improve determinism, or improve testability.

## 8. Proposed Follow-Up Work Orders (WO list, no code)

- **WO-XXX: HAL Boundary Seal Pass**
  - Migrate all remaining `std::process::Command` call sites to `mash_hal` traits.
  - Remove/retire `mash-core/src/process_timeout.rs` in favor of HAL timeouts.
  - Target files: `mash-core/src/disk_ops/format.rs`, `mash-core/src/system_config/packages.rs`, `mash-core/src/stages/*`, `mash-core/src/install_report.rs`.

- **WO-XXX: Canonical Disk Inventory + Identity**
  - Move transport detection + identity composition into `mash-hal`.
  - Expose a stable, UI-friendly disk label struct.
  - Target files: `crates/mash-hal/src/sysfs/block.rs`, `crates/mash-tui/src/dojo/data_sources.rs`.

- **WO-XXX: State + StageRunner Unification**
  - Pick one stage/state runner abstraction; remove the duplicate.
  - Introduce enum stage IDs + versioned state migrations.
  - Target files: `mash-core/src/state_manager/mod.rs`, `crates/mash-workflow/src/stage_runner.rs`.

- **WO-XXX: Flash Pipeline Decomposition**
  - Split `mash-core/src/flash.rs` into 4-6 focused modules without behavior changes.
  - Add unit tests for parse/plan logic (rsync progress parsing, fstab generation).

- **WO-XXX: UI Hot-Path Allocation Reduction**
  - Precompute stable labels on state transitions; keep `draw()` mostly formatting-free.
  - Target files: `crates/mash-tui/src/dojo/dojo_ui.rs`, `crates/mash-tui/src/dojo/dojo_app.rs`.

