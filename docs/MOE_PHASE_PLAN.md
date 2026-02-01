# MOE_PHASE_PLAN.md

This document summarizes the comprehensive multi-issue execution sequence for the "One-Shot Install Hardening and Rust-First Modernization" Epic, as planned by Moe operating under the `advisor-project-sanity` skill. This plan is designed to minimize risk for the MASH installation process, ensuring determinism, safety, and a robust user experience, with a strong emphasis on a "Rust-first" approach for critical operations.

All Work Orders (WO) are designed for Larry (Codex) to execute deterministically, adhering to strict ABB (Always Be Backing Up) protocols and CI-only validation gates.

---

## EPIC: [One-Shot Install Hardening and Rust-First Modernization](https://github.com/drtweak86/MASH/issues/17) (#17)

This Epic outlines the overarching strategy to transform the MASH installer into a bulletproof, Rust-native solution, especially for scenarios involving external HDD wipes. It details the scope, critical stop conditions, and the six deterministic phases of development.

### Key Risks & Mitigations:

*   **External HDD Wipe:** Emphasized a "Rust-first" approach for all disk operations to ensure maximum safety and error handling, moving away from potentially fragile shell scripts. Dry-run capabilities are prioritized.
*   **User Error:** TUI UX polish includes guard rails, explicit confirmations for destructive actions, and graceful cancellation to prevent user mistakes.
*   **Code Quality & Stability:** Strict CI gates (`cargo fmt`, `cargo clippy -D warnings`, `cargo test --all-targets`) after *every sub-step* enforce code quality and immediate feedback on regressions.
*   **Ambiguity & Scope Creep:** Explicit "stop conditions" at every level prevent Larry from proceeding without clear instructions or if the task deviates from the plan.

### Deterministic Phases (Executed via Child Issues):

*   **Phase 0: Freeze, Audit, Remove Junk, Confirm CI Green, Tag Baseline**
    *   *Executed by Moe (this planning phase)*, ensuring a clean and stable starting point.
*   **Phase 1: Rust-Only Disk Operations** (Implemented by Child Issue #18)
    *   Focus: Implementing `disk_ops` module with dry-run capabilities for probing, partitioning, formatting, mounting, and verification.
*   **Phase 2: Port Remaining Install Scripts/Helpers into Rust Stages/Tasks** (Implemented by Child Issue #19)
    *   Focus: Migrating essential non-disk-touching logic from `helpers/` shell scripts to Rust-native stages.
*   **Phase 3: Repository Deduplication and Cleanup** (Implemented by Child Issue #20)
    *   Focus: Removing superseded `helpers/` scripts and consolidating trivial duplicate utilities.
*   **Phase 4: Full TUI UX Polish with Emojis, Guard Rails, and Cancellation Correctness** (Implemented by Child Issue #21)
    *   Focus: Enhancing the `mash-installer` TUI with clear feedback, input validation, progress indicators, and graceful cancellation.
*   **Phase 5: Documentation Overhaul and Cleanup** (Implemented by Child Issue #22)
    *   Focus: Aligning documentation (`README.md`, `docs/`) with the new Rust-first process and repository state.
*   **Phase 6: Release and "Final Install Rehearsal" Checklist**
    *   *Overarching post-completion phase*, including comprehensive dry-runs and real installation tests.

---

## Child Issues Work Order Summary:

Each child issue has been updated with detailed Work Orders, explicit file scope boundaries, Git hygiene reminders (SSH-only operations, proper `.gitignore` usage), and precise CI gates after each sub-step.

### 1. [Phase 1: Implement Rust-Native Disk Operations (Probe, Partition, Format, Mount, Verify)](https://github.com/drtweak86/MASH/issues/18) (#18)

*   **Goal:** Re-implement all disk-related functionalities in Rust, starting with robust dry-run capabilities.
*   **Key Tasks:** Structure `disk_ops` module, carefully add minimal dependencies, implement `probe_disks`, `plan_partitioning`, `format_partitions`, `mount_partitions`, and `verify_disk_operations` all with `dry_run` functionality, and integrate into `main.rs` with integration tests.
*   **Critical Stop Condition:** Any perceived risk of unintended data loss or system corruption.

### 2. [Phase 2: Port `helpers/` Scripts to Rust Stages/Tasks](https://github.com/drtweak86/MASH/issues/19) (#19)

*   **Goal:** Port essential non-disk-touching installation logic from `helpers/` shell scripts to structured Rust-native stages.
*   **Key Tasks:** Set up `stages` module, port `00_write_config_txt.sh`, package installation scripts, `21_zsh_starship.sh`, and other critical `helpers/` scripts to Rust functions with comprehensive testing, then integrate into `mash-installer`'s main flow.
*   **Critical Stop Condition:** Inability to perfectly replicate shell script functionality in Rust.

### 3. [Phase 3: Repository Deduplication and Cleanup](https://github.com/drtweak86/MASH/issues/20) (#20)

*   **Goal:** Streamline the repository by removing superseded `helpers/` scripts and consolidating trivial duplicate utilities.
*   **Key Tasks:** Confirm Phase 2 completion, identify and delete `helpers/` scripts fully replaced by Rust code (verifying no active references remain), and (optionally/trivially) consolidate small duplicated Rust utility functions.
*   **Critical Stop Condition:** If a deleted script is still an active dependency, or if the Rust replacement is not fully functional.

### 4. [Phase 4: TUI UX Polish (Emojis, Guard Rails, Cancellation)](https://github.com/drtweak86/MASH/issues/21) (#21)

*   **Goal:** Enhance the `mash-installer` TUI for a polished, user-friendly, and safe experience.
*   **Key Tasks:** Add TUI dependencies, implement consistent emoji usage, add critical action confirmation prompts, implement robust input validation (guard rails), integrate progress indicators, and implement graceful cancellation handling with integration tests.
*   **Critical Stop Condition:** Any regression in user interaction, lack of clarity in prompts, or unexpected behavior during cancellation.

### 5. [Phase 5: Documentation Overhaul and Cleanup](https://github.com/drtweak86/MASH/issues/22) (#22)

*   **Goal:** Align documentation (`README.md`, `docs/`) with the new Rust-first installation process and repository state.
*   **Key Tasks:** Remove "codex-test" references from `README.md`, update `README.md` for the Rust-first approach, update/remove `helpers/` script documentation, document new Rust installation stages and disk operations, and perform an overall consistency review with link validation for all `docs/` files.
*   **Critical Stop Condition:** Any remaining outdated or inaccurate information, particularly regarding disk operations or shell script usage.

---

This comprehensive plan ensures a phased, deterministic approach to achieve the "One-Shot Install Hardening and Rust-First Modernization" goal, with clear guidance and safety measures for Larry at every step.
