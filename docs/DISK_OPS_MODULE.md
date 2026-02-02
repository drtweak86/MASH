# Disk Operations Module (mash_installer::disk_ops)

This document describes the Rust-native disk operations scaffolding used by the installer for safe, deterministic planning and dry-run execution.

---

## Overview

`mash_installer::disk_ops` provides a minimal, Rust-native interface for:
- probing disks
- planning partition layouts
- formatting partitions
- mounting partitions
- verifying disk operations

These functions are **dry-run safe** by design. When `dry_run = true`, they return simulated data and log what would happen. When `dry_run = false`, the implementations are currently unimplemented and will return an error (or panic) until the real disk paths are fully ported.

---

## API Summary

### `probe_disks(dry_run: bool) -> Result<Vec<DiskInfo>>`
- **Purpose:** Discover candidate disks for installation.
- **dry_run = true:** Returns simulated disks and logs a DRY RUN message.
- **dry_run = false:** Not implemented.
- **Errors:** Returns `Result` with error if real probing is invoked before implementation.

### `plan_partitioning(disk: &DiskInfo, dry_run: bool) -> Result<PartitionPlan>`
- **Purpose:** Build a `PartitionPlan` for the target disk.
- **dry_run = true:** Returns a deterministic plan with EFI/BOOT/ROOT/DATA entries.
- **dry_run = false:** Not implemented.
- **Errors:** Returns `Result` with error if real planning is invoked before implementation.

### `format_partitions(plan: &PartitionPlan, dry_run: bool) -> Result<()>`
- **Purpose:** Format the planned partitions.
- **dry_run = true:** Logs formatting actions per partition and returns `Ok(())`.
- **dry_run = false:** Not implemented.
- **Errors:** Returns `Result` with error if real formatting is invoked before implementation.

### `mount_partitions(plan: &PartitionPlan, dry_run: bool) -> Result<()>`
- **Purpose:** Mount the planned partitions to their mount points.
- **dry_run = true:** Logs mount operations and returns `Ok(())`.
- **dry_run = false:** Not implemented.
- **Errors:** Returns `Result` with error if real mounts are invoked before implementation.

### `verify_disk_operations(plan: &PartitionPlan, dry_run: bool) -> Result<()>`
- **Purpose:** Perform post-operation verification steps.
- **dry_run = true:** Logs verification and returns `Ok(())`.
- **dry_run = false:** Not implemented.
- **Errors:** Returns `Result` with error if real verification is invoked before implementation.

---

## Dry-Run Semantics

When `dry_run = true`, each function emits clear log messages (e.g., "DRY RUN: ...") and returns deterministic results without touching the system. This mode is used to validate configuration and execution flow before enabling real disk operations.

When `dry_run = false`, real disk access is intentionally blocked until the Rust-native implementation is complete. This guard rail prevents accidental data loss while the module is being ported.

---

## Safety Guarantees

- No disk mutations occur in dry-run mode.
- Deterministic output for integration tests.
- Explicit errors for unimplemented real disk operations.

---

## Related Documentation

- `docs/INSTALL_STAGES_MODULE.md` for post-flash stage execution.
- `docs/ARCHITECTURE.md` for overall installer flow.
