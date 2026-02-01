# Rustification Phase 2 — Script Conversion Plan (Issue 12)

### **1. Goal**
This Work Order begins Phase 2 of the MASH Rustification project. The primary goals are to standardize the workspace configuration and to create a concrete, executable plan for porting the core Python installation script (`scripts/mash-full-loop.py`) to a new Rust crate.

### **2. Scope**

#### **Inclusions**
-   **WO-1:** Standardizing metadata across all workspace `Cargo.toml` files.
-   **WO-2:** Creating a new, empty library crate (`tools/mash-core`) to house the ported Python logic.
-   **WO-3:** Scaffolding the new crate with placeholder functions that map directly to the logic in `scripts/mash-full-loop.py`.

#### **Exclusions (Non-negotiable)**
-   **NO** implementation of the actual porting logic. All new functions in WO-3 must be placeholders using `todo!()`.
-   **NO** changes to any files under `legacy/` or `legacy_scripts/`.
-   **NO** refactoring of existing Rust code in `mash-installer/`.

### **3. Stop Conditions**
-   If any CI step fails, **STOP**, revert the changes for that WO, and report the failure.
-   If the purpose of a function or section in `mash-full-loop.py` is ambiguous, **STOP** and ask for clarification before creating the placeholder function.

---

### **Work Order 1: Workspace Standardization & Hygiene**
**Goal:** Apply consistent metadata across the workspace `Cargo.toml` files.

#### **Step 1.1: ABB & Branching**
- **1.1.1:** Create a backup branch: `git checkout -b backup/WO-1-metadata-YYYYMMDD`
- **1.1.2:** Create the working branch: `git checkout -b issue/12-wo-1-metadata`

#### **Step 1.2: Standardize Workspace**
- **1.2.1:** In the root `Cargo.toml`, add a `[workspace.package]` table with canonical metadata.
  ```toml
  [workspace.package]
  version = "1.2.14"
  edition = "2021"
  authors = ["MASH Team"]
  license = "MIT"
  repository = "https://github.com/drtweak86/MASH"
  homepage = "https://github.com/drtweak86/MASH"
  rust-version = "1.70"
  ```
- **1.2.2:** In `mash-installer/Cargo.toml`, `tools/mash-release/Cargo.toml`, and `tools/mash-tools/Cargo.toml`, update the `[package]` sections to inherit from the workspace.
  ```toml
  # Example for a member crate
  [package]
  name = "mash-tools"
  version.workspace = true
  edition.workspace = true
  # ... and so on for other inherited fields
  ```

#### **Step 1.3: Validation & Commit**
- **1.3.1:** Run CI gates: `cargo fmt -- --check`, `cargo clippy -- -D warnings`, `cargo test --workspace`.
- **1.3.2:** Commit with message: `chore(cargo): standardize workspace metadata`.

---

### **Work Order 2: Create `mash-core` Crate**
**Goal:** Create the new crate that will contain the ported logic.

#### **Step 2.1: ABB & Branching**
- **2.1.1:** Create a backup branch and a working branch (`issue/12-wo-2-create-crate`).

#### **Step 2.2: Create Crate**
- **2.2.1:** Create a new library crate.
  ```bash
  cargo new --lib tools/mash-core
  ```
- **2.2.2:** Add the new crate to the `[workspace.members]` list in the root `Cargo.toml`.
- **2.2.3:** Add a `README.md` to `tools/mash-core/` explaining its purpose.

#### **Step 2.3: Validation & Commit**
- **2.3.1:** Run `cargo check --workspace` to ensure the new member is recognized.
- **2.3.2:** Commit with message: `feat(project): create mash-core crate for rustification`.

---

### **Work Order 3: Scaffold Python Logic in `mash-core`**
**Goal:** Create a 1-to-1 mapping of functions from `scripts/mash-full-loop.py` to placeholder functions in `tools/mash-core/src/lib.rs`.

#### **Step 3.1: ABB & Branching**
- **3.1.1:** Create a backup branch and a working branch (`issue/12-wo-3-scaffold-logic`).

#### **Step 3.2: Create Placeholder Functions**
- **3.2.1:** For each major logical block in `mash-full-loop.py` (e.g., `sh`, `banner`, `partition`, `format`, `mount`, `rsync`, `dracut`), create a corresponding public function in `tools/mash-core/src/lib.rs`.
- **3.2.2:** Each function must have a `todo!()` macro in its body and a comment linking it to the line(s) in the Python script.

    **Example for `tools/mash-core/src/lib.rs`:**
    ```rust
    //! Core logic for MASH, ported from scripts/mash-full-loop.py.

    use std::path::Path;

    /// Corresponds to the `sh` helper in the Python script.
    pub fn sh(cmd: &str) -> anyhow::Result<()> {
        todo!("Implement shell command execution, corresponds to mash-full-loop.py:29");
    }

    /// Corresponds to the partitioning logic.
    pub fn partition_disk(disk: &Path) -> anyhow::Result<()> {
        todo!("Implement partitioning logic from mash-full-loop.py:107-137");
    }

    /// Corresponds to the formatting logic.
    pub fn format_partitions(disk: &Path) -> anyhow::Result<()> {
        todo!("Implement formatting logic from mash-full-loop.py:141-145");
    }

    // ... continue for all other logical blocks ...
    ```

#### **Step 3.3: Validation & Commit**
- **3.3.1:** Run `cargo clippy -- -D warnings`. It should pass but note the `todo!()` macros.
- **3.3.2:** Commit with message: `feat(mash-core): scaffold logic from python script`.
