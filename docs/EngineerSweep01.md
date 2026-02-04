# Senior Engineer Refactor & Optimization Report (Release Mode)

## 1. Architectural Consolidation
- Downloader paths: download logic lives in `mash-core/src/downloader/mod.rs`, with wrapper duplication in `mash-core/src/download_manager.rs` and `crates/mash-workflow/src/installer/pipeline/stages.rs::run_download_stage`; artifact types differ (`downloader::DownloadArtifact` vs `state_manager::DownloadArtifact`), risking drift in resume and checksum recording; make `mash-core::downloader` the single canonical owner and expose a state-recording adapter inside `mash-core::state_manager`.
- Disk/partition handling: formatting and mount helpers exist in `mash-core/src/disk_ops` (hard-wired LinuxHal), workflow disk stage uses its own loop in `installer/pipeline/stages.rs`, and Fedora flash pipeline reimplements parted/mkfs/mount inside `mash-core/src/flash/runner.rs`; consolidate under HAL-parameterized helpers in `mash-core::disk_ops` and reuse from both the flash pipeline and workflow stages.
- Flashing logic: two execution engines (monolithic `mash-core::flash::runner` and staged `mash-workflow::installer::pipeline`) perform overlapping partitioning, flashing, and reporting with different safety semantics; designate `mash-workflow::installer::pipeline` as the single executor and make `mash-core::flash` a thin adapter that builds a plan and delegates.
- State persistence: workflow uses `state_manager::InstallState` while flash pipeline keeps ad-hoc progress/report structs; download artifacts are duplicated; promote `InstallState` as authoritative for all runs and feed progress, artifacts, and stages through it.
- Safety/arming logic: typed tokens live in `mash-core/src/config_states.rs` but TUI keeps separate `destructive_armed/yes_i_know` flags and workflow pipeline relies on booleans; enforce `config_states` tokens at every entry point, with TUI collecting inputs and passing tokens through.
- UI flow coordination: TUI orchestrates download + flash via `download_manager` and `flash` directly while pipeline has parallel download/format/resume flows; route TUI actions through `mash-workflow` to avoid duplicate logic.

## 2. Modularization Opportunities
- `mash-core/src/flash/runner.rs` mixes validation, partition planning, mounting, rsync, EFI staging, progress; split into `plan`, `execute` (HAL-driven ops), `artifacts/reporting`, and `safety`.
- `mash-core/src/downloader/mod.rs` bundles index parsing, HTTP/retry, and checksum parsing; split into `index`, `client`, `verify`.
- `crates/mash-tui/src/dojo/dojo_app/app.rs` is monolithic; split into `state` (App struct), `actions` (side effects like download/flash/rescan), and `navigation` (step transitions).
- `crates/mash-tui/src/dojo/dojo_ui/content.rs` mixes static copy and dynamic rendering; separate static text blocks from render functions to reduce churn.
- `mash-core/src/system_config` could separate pure rendering (unit content) from imperative systemd application for clearer testability.

## 3. HAL Boundary Audit
- `mash-core/src/disk_ops/{format,mounts}.rs` instantiate `LinuxHal` internally, blocking FakeHal use and timeouts control; accept injected `FormatOps/MountOps` and lift construction to callers.
- `mash-core/src/flash/runner.rs` mostly uses `InstallerHal` but mixes direct filesystem ops (tmpdirs, chmod, cleanup) with device actions; ensure all device mutations (losetup, rsync, wipefs, parted, mkfs) stay behind HAL traits for FakeHal coverage.
- FakeHal lacks coverage for partition creation and resume-unit install paths exercised by flash runner; add trait methods/mocks so tests can run fully without devices.
- Direct `fs::remove_dir_all` cleanup in flash paths can delete host directories during tests; guard via HAL or scoped paths.

## 4. State & Resume Model Review
- Dual state models: `InstallState` (workflow) vs flash-local context/reporting; unify on `InstallState` and persist stage transitions for both pipelines.
- Stage names are stringly typed in `InstallState.completed_stages` and `StageDefinition.name`; convert to enums with stable snake_case serialization to prevent silent resume mismatch.
- Download artifact structs are duplicated; fold into one struct that records checksum, size, resumed flag, and path, shared by downloader and state manager.
- `partial_ok_resume` flag is written but not consumed; define the resume policy and enforce it when resuming downloads or flashing.
- Confirmation/arming tokens are not persisted; require re-arming on resume and persist the armed state alongside `InstallState`.
- Crash mid-partitioning leaves no persisted “current phase”; persist pre-destructive phase markers and enforce cleanup/idempotency before resuming.

## 5. Performance & Responsiveness
- Disk scanning: TUI rescans via full sysfs walk each refresh; debounce or cache results when watch mode polls frequently (must-fix only if polling).
- Flash runner re-queries lsblk and rebuilds parted options multiple times; cache geometry and HAL lookups per run (nice-to-have).
- Download path re-hashes whole files after resume; retain prior partial hash or use ranged verification to avoid duplicate hashing (nice-to-have).
- TUI progress redraw fires on every tick; throttle to percent or phase deltas to reduce terminal churn (nice-to-have).

## 6. Code Quality & Safety Hygiene
- Error masking: `disk_ops/format.rs` collapses HAL errors to “Command failed”; return structured errors with stderr/exit codes for diagnostics.
- Unwraps in flash runner (parted output parsing, `fstab_path.parent().unwrap()`, string parsing of speeds) can panic on malformed images; replace with typed errors.
- Device path inference: `FlashContext::partition_path` uses substring heuristics; centralize in HAL for nvme/mmc/loop/mapper correctness.
- Hardcoded image identifiers: workflow download stage and manager hardcode Fedora `kde_mobile_disk`/`aarch64`; move to shared constants to avoid drift.
- Logging vs stdout: ensure library code uses log macros (audit downloader/flash helpers) to keep UI clean and testable.

## 7. Risk Ranking
- High: Dual installer paths with divergent safety/state; stringly-typed stages; HAL bypass in disk_ops; non-persisted arming on resume.
- Medium: Duplicated downloader/artifact models; generic error masking in formatting; partition-path heuristics.
- Low: Monolithic modules hurting readability; progress/hash throttling; lsblk/parted re-query overhead.
