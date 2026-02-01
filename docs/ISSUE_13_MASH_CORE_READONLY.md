# Rustification Phase 3 — `mash-core` Read-Only Logic (Issue 13)

### **1. Goal**
This Work Order begins the implementation phase for the `mash-core` crate. The goal is to replace `todo!()` macros with *read-only* Rust logic, focusing on argument parsing, validation, and information gathering, without making any changes to the system state.

### **2. Scope**

#### **Inclusions**
-   Implementing placeholder functions in `tools/mash-core/src/lib.rs` with actual Rust code.
-   Adding necessary Rust dependencies to `tools/mash-core/Cargo.toml`.

#### **Exclusions (Non-negotiable)**
-   **NO** implementation of any function that performs disk writes (e.g., partitioning, formatting, `rsync`, `write_file`).
-   **NO** changes to any files under `legacy/` or `legacy_scripts/`.
-   **NO** refactoring of existing Rust code outside `tools/mash-core/`.

### **3. Stop Conditions**
-   If any CI step fails, **STOP**, revert the changes for that WO, and report the failure.
-   If the implementation of a function requires writing to disk or modifying system state to complete its read-only task, **STOP** and report this finding.

---

### **Work Order 1: Implement Core Read-Only Helpers**
**Goal:** Replace `todo!()` macros for fundamental, purely read-only helper functions.

#### **Step 1.1: ABB & Branching**
-   **1.1.1:** Create a backup branch: `git checkout -b backup/WO-1-helpers-YYYYMMDD`
-   **1.1.2:** Create the working branch: `git checkout -b issue/13-wo-1-helpers`

#### **Step 1.2: Implement Helper Functions**
-   **1.2.1:** Implement `banner(title: &str) -> anyhow::Result<()>`:
    -   Use `println!` for output.
    -   Corresponds to `mash-full-loop.py:85-89`.
-   **1.2.2:** Implement `die(msg: &str, code: i32)`:
    -   Use `eprintln!` for the fatal message and `std::process::exit(code)`.
    -   Corresponds to `mash-full-loop.py:80-82`.
-   **1.2.3:** Implement `sh(cmd: &str) -> anyhow::Result<String>` (adjust return type to capture output):
    -   Use `std::process::Command` to execute external commands.
    -   Add `anyhow` as a dependency to `tools/mash-core/Cargo.toml`.
    -   Ensure `capture=True` functionality is handled for commands like `blkid`.
    -   Corresponds to `mash-full-loop.py:55-72`.
-   **1.2.4:** Implement `need(binname: &str) -> anyhow::Result<()>`:
    -   Use `std::process::Command::new("which").arg(binname).output()`.
    -   Corresponds to `mash-full-loop.py:75-78`.
-   **1.2.5:** Implement `parse_size_to_mib(size: &str) -> anyhow::Result<u64>`:
    -   Use `regex` crate for parsing, or manual string splitting and `parse::<u64>()`.
    -   Add `regex` as a dependency to `tools/mash-core/Cargo.toml`.
    -   Corresponds to `mash-full-loop.py:112-119`.

#### **Step 1.3: Validation & Commit**
-   **1.3.1:** Run CI gates: `cargo fmt -- --check`, `cargo clippy -- -D warnings`, `cargo test --workspace`.
-   **1.3.2:** Commit with message: `feat(mash-core): implement core read-only helpers`.

---

### **Work Order 2: Implement Argument Parsing & Initial Validation**
**Goal:** Port argument parsing and initial existence checks without side effects.

#### **Step 2.1: ABB & Branching**
-   **2.1.1:** Create branches: `backup/WO-2-args-validate-YYYYMMDD` and `issue/13-wo-2-args-validate`.

#### **Step 2.2: Implement Parsing & Validation**
-   **2.2.1:** Implement `parse_args_and_validate()`:
    -   Use the `clap` crate for argument parsing.
    -   Add `clap = { version = "4.0", features = ["derive"] }` as a dependency to `tools/mash-core/Cargo.toml`.
    -   Define a `struct Args` for CLI arguments (image, disk, uefi_dir, scheme, make_data, etc.).
    -   Implement `if os.geteuid() != 0: die(...)` check.
    -   Implement the `image.exists()`, `Path(disk).exists()`, `uefi_dir / "RPI_EFI.fd").exists()` checks using `std::path::Path::exists()`.
    -   Corresponds to `mash-full-loop.py:220-245`.

#### **Step 2.3: Validation & Commit**
-   **2.3.1:** Run CI gates: `cargo fmt -- --check`, `cargo clippy -- -D warnings`, `cargo test --workspace`.
-   **2.3.2:** Commit with message: `feat(mash-core): implement arg parsing and initial validation`.

---

### **Work Order 3: Implement Read-Only Disk Analysis**
**Goal:** Port logic to display disk information and perform btrfs subvolume checks.

#### **Step 3.1: ABB & Branching**
-   **3.1.1:** Create branches: `backup/WO-3-disk-analyze-YYYYMMDD` and `issue/13-wo-3-disk-analyze`.

#### **Step 3.2: Implement Disk Analysis**
-   **3.2.1:** Implement `lsblk_tree(disk: &str) -> anyhow::Result<()>`:
    -   Use the `sh` helper implemented in WO-1.
    -   Corresponds to `mash-full-loop.py:104-105` and `mash-full-loop.py:285-289`.
-   **3.2.2:** Implement `blkid_uuid(dev: &str) -> anyhow::Result<String>`:
    -   Use the `sh` helper implemented in WO-1.
    -   Corresponds to `mash-full-loop.py:108-109`.
-   **3.2.3:** Implement a function `read_btrfs_subvols(path: &Path) -> anyhow::Result<BtrfsSubvolsInfo>`:
    -   This function should execute `btrfs subvolume list` via the `sh` helper.
    -   Parse the output to determine if `root`, `home`, and `var` subvolumes exist, returning a struct or tuple with this information.
    -   Corresponds to `mash-full-loop.py:363-367`.

#### **Step 3.3: Validation & Commit**
-   **3.3.1:** Run CI gates: `cargo fmt -- --check`, `cargo clippy -- -D warnings`, `cargo test --workspace`.
-   **3.3.2:** Commit with message: `feat(mash-core): implement read-only disk analysis`.

---

### **Megillah Template**
When the entire issue is complete, post the following summary:
```md
## Phase 3 - `mash-core` Read-Only Logic: COMPLETE

### Action Taken
Implemented core read-only helper functions, argument parsing and validation, and initial disk analysis functions within the `mash-core` crate.

### Work Orders Completed:
-   [x] **WO-1: Implement Core Read-Only Helpers**
-   [x] **WO-2: Implement Argument Parsing & Initial Validation**
-   [x] **WO-3: Implement Read-Only Disk Analysis**

All CI checks are green, and the `mash-core` crate now contains foundational read-only logic for the flashing process.
```
