# LARRYENGINEERREPORT.md

## 1. Correctness Risks

- **Disconnected / changing disks not handled:** Disk inventory is captured once at startup (`data_sources::scan_disks()`), then treated as stable; there is no rescan or "device disappeared" UX before destructive stages. See `crates/mash-tui/src/dojo/dojo_app.rs:449-456`. A drive can vanish between selection and flash/format, yielding late failures and potentially confusing state.
- **"Identity required" can brick selection on real hardware:** `DiskIdentity::new()` returns `None` unless *both* vendor+model exist, with no fallback (`crates/mash-tui/src/dojo/data_sources.rs:61-87`). UI then hard-fails the entry (`crates/mash-tui/src/dojo/dojo_ui.rs:922-929`). Many removable devices expose blank/odd sysfs strings; this can leave users with "IDENTITY FAILED" and no valid target.
- **0-byte / corrupted cached downloads can be accepted silently (Fedora/UEFI "old downloader" path):**
  - Fedora: "if file exists, skip download" with no size/hash validation (`mash-core/src/download.rs:349-354`) and same behavior in progress variant (`mash-core/src/download.rs:450-455`).
  - UEFI: downloads a ZIP with no checksum verification at all (`mash-core/src/download.rs:195-227`).
- **Download index parse failure is silently ignored:** `DOWNLOAD_INDEX` uses `toml::from_str(...).unwrap_or(empty)` which can mask a broken `docs/os-download-links.toml` and produce "no images" behavior with no crash/signal (`mash-core/src/downloader/mod.rs:52-58`).
- **Mount stage masks HAL errors:** `is_mounted()` errors are discarded with `unwrap_or(false)`, so a mountinfo parse/permission failure becomes "not mounted" and the code proceeds to mount anyway (`crates/mash-workflow/src/installer/pipeline.rs:353-366`).
- **FlashConfig validation is too weak for disk correctness:** Only checks device path exists, not that it is a block device or safe target (`mash-core/src/flash.rs:243-248`).

## 2. Security & Privacy

- **Input validation gaps (destructive sizing):** Partition size parsing only accepts integer M/G units and does not bound-check against target disk capacity or enforce sane maxima; a typo can generate a nonsensical plan that passes validation (`crates/mash-tui/src/dojo/dojo_app.rs:1743-1820`).
- **Unsafe "work dir" handling in privileged contexts:** Installer uses a fixed `/tmp/mash-install` and deletes it recursively if present (`mash-core/src/flash.rs:314-319`). This is a common hardening hotspot (TOCTOU / attacker-controlled contents) in root-running installers.
- **No obvious secrets dumped via stdout in TUI path:** `println!/eprintln!` appear absent in active code paths for TUI execution (good). Note: `dump_all_steps()` still prints to stdout (`crates/mash-tui/src/dojo/mod.rs:72-79`), so running that in a production environment can leak configuration details into logs/CI output.

## 3. Performance & Scalability

- **Render-loop allocations are heavy but acceptable; no blocking I/O spotted in draw():** `dojo_ui::draw` builds many `String`s each frame (expected for ratatui), but disk/procfs scanning is not in the render loop (scan happens in `App::new`) (`crates/mash-tui/src/dojo/dojo_app.rs:449-456`).
- **Checksum verification is streaming (good):** Large image hashing uses an 8KiB buffer and does not load the file into RAM (`mash-core/src/downloader/mod.rs:118-134`).
- **However, two parallel download implementations exist:** TUI's Fedora path uses `mash-core/src/download.rs` (no checksum for Fedora/UEFI, weak caching logic), while workflow uses `mash-core/src/downloader/mod.rs` (checksum+resume). This duplication increases the chance that a "fast path" skips integrity checks, and makes perf regressions likely.

## 4. Reliability & Ops

- **Network requests without timeouts (UEFI GitHub API path):** `reqwest::blocking::Client::builder().build()` is used without `.timeout(...)` for GitHub release queries in both UEFI flows (`mash-core/src/download.rs:165-173` and `mash-core/src/download.rs:249-258`). These can hang indefinitely on bad networks/DNS/proxy stalls.
- **External commands can hang indefinitely:** Several critical operations are `Command::new(...).status()/spawn()` without enforced timeouts (e.g., `unxz`, `rsync`, `losetup`, `mount`, `btrfs` in `mash-core/src/flash.rs`, plus `unxz` in `mash-core/src/download.rs:497-519`). Cancellation is partial (kills `unxz` in one path), but many commands remain non-interruptible.
- **Resume/idempotency is uneven:** The workflow pipeline persists stage completion via `StageRunner`, but the Fedora "existing installer behavior" bypasses that and runs `flash::run_with_progress` directly (`crates/mash-tui/src/dojo/mod.rs:203-207`). A crash mid-flash in the Fedora path has different recovery semantics than the staged path.
- **Observability is file-based but not structured:** Logging defaults to `/var/log/mash/dojo.log` when writable (`mash-core/src/logging.rs:6-16`), but messages are mostly free-form text; correlation IDs / structured fields are absent.

## 5. Maintainability

- **Abstraction leaks (HAL bypass):**
  - Workflow correctly routes format/mount through `mash_hal` traits in places (`crates/mash-workflow/src/installer/pipeline.rs:278-369`), but still uses direct `std::fs::create_dir_all` in the mount stage (`crates/mash-workflow/src/installer/pipeline.rs:357-359`).
  - Core flashing/partitioning is still command-driven (`std::process::Command`) in `mash-core/src/flash.rs` and does not route through `mash_hal` (large surface area; see e.g. `mash-core/src/flash.rs:314+`, plus numerous `Command::new(...)` hits).
- **Error typing is inconsistent:** `mash-hal` defines `HalError` (`crates/mash-hal/src/error.rs:1-16`) but the traits return `anyhow::Result<()>` (`crates/mash-hal/src/hal/flash_ops.rs:17-33`, `crates/mash-hal/src/hal/format_ops.rs:7-36`, `crates/mash-hal/src/hal/mount_ops.rs:6-44`). Typed errors exist but are not enforced at API boundaries; downstream code ends up string-matching or losing specificity.
- **TUI/business logic coupling is still high:** TUI directly orchestrates downloads and chooses between two installer backends (`crates/mash-tui/src/dojo/mod.rs:203-207`), which makes it hard to guarantee invariants (resume, safety, integrity) across all flows.
- **Dead / placeholder UI branches remain:** "DownloadingFedora/DownloadingUefi" steps still present with explicit "stub/simulate" messaging (`crates/mash-tui/src/dojo/dojo_ui.rs:741-774`). Even if unused, this is maintenance debt and a footgun for future wiring.
- **OS exposure does not reflect wiring maturity:** `OsDistro::is_available()` always returns true (`crates/mash-tui/src/dojo/flash_config.rs:63-65`), meaning the UI can advertise OS paths that may not be fully end-to-end deterministic.

## 6. Backward Compatibility & Migration

- **State versioning is present but not enforced/migrated:** `InstallState` has `version: 1` (`mash-core/src/state_manager/mod.rs:30-67`), but `load_state` does not check version or migrate; future schema changes can strand users with non-loadable state files (`mash-core/src/state_manager/mod.rs:117-125`).
- **Partial forwards-compat is accidental:** Some newer fields are `#[serde(default)]` (`mash-core/src/state_manager/mod.rs:41-49`), but any future non-defaulted additions will break old state unless a migration layer is introduced.

## 7. Recommended Improvements

High-impact refactors:

1. **Unify download implementation + integrity rules (single source of truth):** Replace the TUI's `mash-core/src/download.rs` usage with the indexed/checksum/resume downloader (`mash-core/src/downloader/mod.rs`), and expose a progress callback there. Primary touchpoints: `crates/mash-tui/src/dojo/mod.rs`, `mash-core/src/download_manager.rs`, `mash-core/src/download.rs`, `mash-core/src/downloader/mod.rs`.
2. **Move destructive disk operations behind `mash_hal` (real boundary):** Consolidate partitioning/mount/losetup/rsync/udev settle operations currently embedded in `mash-core/src/flash.rs` into HAL traits (or a single "WorldOps" facade) and make both TUI + workflow go through the same executor.
3. **Make typed errors real at the trait boundary:** Change `mash_hal` traits to return `Result<_, HalError>` (or a typed enum with transparent wrappers) so callers can reliably branch on `SafetyLock`, `DiskBusy`, etc. Files: `crates/mash-hal/src/hal/*.rs`, `crates/mash-workflow/src/**`, `crates/mash-tui/src/**`.

One safety pattern change:

- **Adopt a TypeState pipeline for "ValidatedConfig -> ArmedConfig -> Executing":** Encode SAFE/ARMED + validated partition/disk selection at the type level so you cannot call flash/format/partition without passing through the explicit disarm flow and preflight. Start at `crates/mash-tui/src/dojo/flash_config.rs` + `crates/mash-workflow/src/installer/*` and enforce that only `ArmedConfig` can be converted into "destructive stage definitions."

