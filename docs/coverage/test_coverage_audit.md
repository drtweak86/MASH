# Test Coverage Audit (Feb 2026)

## Scope
This audit inspects the existing test suite for lower-coverage areas, documents tooling failures, and lists the follow-up issues that implement the missing paths.

## Tooling attempt
- Ran `cargo install cargo-tarpaulin` to generate coverage reports, but the installer fails in this environment because `openssl-sys` cannot locate OpenSSL headers (`OPENSSL_DIR` / `PKG_CONFIG_PATH` are unset). Without system-SSL we cannot build the coverage toolchain (the same issue would occur when running tarpaulin on a clean CI image unless its base image installs `libssl-dev`).
- Because of that, there is no machine-generated coverage report to attach, so the rest of this audit is a manual, best-effort summary based on repo structure and current tests.

## Manual findings
| Module / concern | Current coverage state | Notes / gap | Next action |
| --- | --- | --- | --- |
| `mash-installer/src/download.rs` | New tests already cover downloads, progress, firmware decoding (`issue #46`). | The new `reqwest`+mock HTTP server suite now exercises the critical download helpers. | None — this area is settled.
| `mash-installer/src/tui/new_app.rs` + `tui/new_ui.rs` | The UI has input-state tests, but most progress and cancellation transitions still rely on manual smoke checks. | Additional coverage could focus on progress updates, cancellation flag propagation, and final-state transitions that are hard to simulate headless. | Document future TUI test tasks (e.g., harnessing the `progress` module).
| Installer pipeline (`flash.rs`, `mash-core` stage runner) | No coverage yet because the new pipeline lives in `mash-core`. | Issue #47 is tracking stage runner coverage once that crate merges. Until then, the pipe remains untested.
| CLI/resume (`mash-core/src/cli.rs`, `installer::pipeline`, `system_config::resume`) | Not tested (code is not in this repo yet). | Issue #48 will cover parsing/resume logic once the sources exist.
| `preflight.rs`, `locale.rs`, and other helpers | Partial tests exist; spot checks show a few validation helpers lacking tests. | Potential future tasks could add fixtures to `preflight` and `locale` to ensure the path-splitting logic does not regress. | Consider adding another coverage issue after #47/#48 close (for `preflight` and `locale`).

## Child issues and follow-up work
- #46 (`test: cover download helpers`) now exercises the download paths.\
- #47 (`Test: stage pipeline coverage`) will cover the new stage runner once `mash-core` is merged (currently blocked).\
- #48 (`Test: CLI & resume coverage`) is planned to exercise CLI parsing and resume state, again pending the `mash-core` modules.

## CI / tooling recommendations
1. Add `libssl-dev` (or the equivalent package for the runner OS) as part of the coverage workflow so that `cargo-tarpaulin` can install cleanly.\
2. Once tarpaulin succeeds, capture its XML or JSON output and commit it under `coverage/` for reference.\
3. Keep the existing `cargo test`/`clippy` gates but consider adding a scheduled coverage job that runs nightly and uploads the report to Github Actions artifacts.
4. Document the coverage job in `README` (or a `docs/coverage` index) so contributors know how to regenerate the report locally.

## Summary
Because tarpaulin cannot be installed without OpenSSL, this audit relies on manual review plus the three existing child issues (#46-#48). The plan is to unblock the stage runner and CLI coverage tasks after the `mash-core` code lands, then revisit the remaining helpers (`preflight`, `locale`, etc.) based on the coverage data that tarpaulin will produce once the environment supports it.
