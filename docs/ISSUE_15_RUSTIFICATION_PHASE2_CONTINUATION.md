# Epic: MASH Installer Rustification & Automation Phase 2

### **1. Goal**
This Epic outlines a series of high-impact, deterministic upgrades to improve the MASH project's reliability, safety, and automation. Each Work Order (WO) is designed to be a small, focused, and verifiable step towards a more robust system, building on the foundation laid in previous issues.

### **2. Scope & Definition of Done**

#### **Inclusions**
-   Implementing `read-only` logic within `tools/mash-core`.
-   Planning for `write` logic implementation in `tools/mash-core`.
-   Creating agent skill files from `AGENT.md`.
-   Removing the root `install.sh` wrapper script.
-   Integrating Docker builds into CI/CD for artifact generation.

#### **Exclusions (Non-negotiable)**
-   **NO** implementation of disk-modifying (write) logic in `tools/mash-core` during WO-1.
-   **NO** refactoring of existing Rust code outside the specific scope of each WO.
-   **NO** changes to any files under `legacy/` or `legacy_scripts/`.
-   **NO** changes to the `helpers/*.sh` or `dojo_bundle/**/*.sh` scripts in this Epic.

#### **Definition of Done**
-   [ ] All Work Orders (WO-1 through WO-5) are completed and committed individually.
-   [ ] All CI checks (`fmt`, `clippy`, `test`) pass for each commit.
-   [ ] The final "Megillah" comment is posted on this issue.
-   [ ] All backup branches are deleted after the main branch is confirmed stable.

### **3. Stop Conditions**
-   If any CI step fails and the cause is not immediately obvious, **STOP**, revert the changes for that WO, and report the failure in a comment.
-   If any step introduces unexpected side effects or requires deviation from read-only principles in WO-1, **STOP**, revert, and report.

---

### **Work Order 1: Complete `mash-core` Read-Only Logic Implementation**
**Goal:** Implement remaining placeholder functions in `tools/mash-core/src/lib.rs` that perform purely read-only operations, gathering information and validating inputs. This continues work from Issue #14.

#### **Step 1.1: ABB & Branching**
-   **1.1.1:** Create a backup branch: `git checkout -b backup/WO-1-readonly-core-YYYYMMDD`
-   **1.1.2:** Create the working branch: `git checkout -b issue/15-wo-1-readonly-core`

#### **Step 1.2: Implement Read-Only Functions**
-   **1.2.1:** Implement `cleanup_mounts_and_loopdev()`:
    -   This function should be primarily structural; actual unmounting logic will be implemented in a later write phase. For this WO, ensure it checks current mounts and identifies what *would* be unmounted, but perform no actual unmount operations.
    -   Corresponds to `mash-full-loop.py:252-283`.
-   **1.2.2:** Implement `safety_check_pause()`:
    -   Use `lsblk_tree()` to display disk information.
    -   Implement the pause/countdown.
    -   Corresponds to `mash-full-loop.py:285-289`.
-   **1.2.3:** Implement `unmount_target_disk()`:
    -   This function should *identify* currently mounted partitions on the target disk via `lsblk` but *not* execute actual `umount` commands.
    -   Corresponds to `mash-full-loop.py:291-297`.
-   **1.2.4:** Implement `read_btrfs_subvols(path: &Path) -> anyhow::Result<BtrfsSubvolsInfo>`:
    -   Execute `btrfs subvolume list` via the `sh()` helper.
    -   Parse output to determine if `root`, `home`, and `var` subvolumes exist on the source image.
    -   Return a struct containing this information.
    -   Corresponds to `mash-full-loop.py:363-367`.

#### **Step 1.3: Validation & Commit**
-   **1.3.1:** Run CI gates: `cargo fmt -- --check`, `cargo clippy -- -D warnings`, `cargo test --workspace`.
-   **1.3.2:** Commit with message: `feat(mash-core): complete read-only logic for core operations`.

---

### **Work Order 2: Plan `mash-core` Write Logic Implementation**
**Goal:** Create placeholder functions for all state-changing operations, outlining inputs, outputs, and potential Rust crates for implementation.

#### **Step 2.1: ABB & Branching**
-   **2.1.1:** Create branches: `backup/WO-2-write-plan-YYYYMMDD` and `issue/15-wo-2-write-plan`.

#### **Step 2.2: Add Write Placeholder Functions**
-   **2.2.1:** Create new `pub fn` placeholder functions in `tools/mash-core/src/lib.rs` for each major write operation. Each function must contain `todo!()` and detailed comments.
    -   `wipe_signatures()` (from `mash-full-loop.py:299-302`).
    -   `partition_disk_gpt_mbr()` (from `mash-full-loop.py:303-337`).
    -   `format_filesystems()` (from `mash-full-loop.py:343-351`).
    -   `loop_mount_image()` (from `mash-full-loop.py:354-361`).
    -   `mount_image_partitions()` (from `mash-full-loop.py:363-387`).
    -   `mount_destination_partitions()` (from `mash-full-loop.py:389-399`).
    -   `create_destination_subvols()` (from `mash-full-loop.py:401-407`).
    -   `mount_destination_subvols()` (from `mash-full-loop.py:409-414`).
    -   `copy_root_subvols()` (from `mash-full-loop.py:416-421`).
    -   `copy_boot_partition()` (from `mash-full-loop.py:423-424`).
    -   `bind_mount_boot_inside_root()` (from `mash-full-loop.py:426-434`).
    -   `install_fedora_efi_loaders()` (from `mash-full-loop.py:436-439`).
    -   `install_pftf_uefi()` (from `mash-full-loop.py:441-444`).
    -   `write_uefi_config_txt()` (from `mash-full-loop.py:446-463`).
    -   `write_fstab()` (from `mash-full-loop.py:464-485`).
    -   `patch_bls()` (from `mash-full-loop.py:486-489`).
    -   `dracut_in_chroot()` (from `mash-full-loop.py:491-519`).
    -   `final_sanity_checks()` (from `mash-full-loop.py:520-533`).
    -   `print_completion_summary()` (from `mash-full-loop.py:534-541`).
-   **2.2.2:** For each placeholder, research and note potential Rust crates/methods for implementation (e.g., `nix` for low-level OS calls, `parted` crate if available, `std::fs` for file writing).

#### **Step 2.3: Validation & Commit**
-   **2.3.1:** Run `cargo clippy -- -D warnings`. It should pass but note the `todo!()` macros.
-   **2.3.2:** Commit with message: `docs(mash-core): scaffold write logic for core operations`.

---

### **Work Order 3: Integrate `AGENT.md` as Agent Skill Files**
**Goal:** Make the defined agent personas in `AGENT.md` operational by creating dedicated `SKILL.md` files for each.

#### **Step 3.1: ABB & Branching**
-   **3.1.1:** Create branches: `backup/WO-3-agent-skills-YYYYMMDD` and `issue/15-wo-3-agent-skills`.

#### **Step 3.2: Create Skill Files**
-   **3.2.1:** Create directory `.github/skills/larry-implementer/`.
-   **3.2.2:** Create `.github/skills/larry-implementer/SKILL.md`. Populate it with the "Larry" role definition from `AGENT.md`, formatted as a skill file.
-   **3.2.3:** Create directory `.github/skills/moe-advisory/`.
-   **3.2.4:** Create `.github/skills/moe-advisory/SKILL.md`. Populate it with the "Moe" role definition from `AGENT.md`, formatted as a skill file (similar to `advisor-project-sanity` but potentially distinct).
-   **3.2.5:** Create directory `.github/skills/curly-project-manager/`.
-   **3.2.6:** Create `.github/skills/curly-project-manager/SKILL.md`. Populate it with the "Curly" role definition from `AGENT.md`, formatted as a skill file.

#### **Step 3.3: Validation & Commit**
-   **3.3.1:** Run CI gates.
-   **3.3.2:** Commit with message: `feat(agent): create skill files for Larry, Moe, and Curly`.

---

### **Work Order 4: Remove Root `install.sh` Wrapper Script**
**Goal:** Simplify the project structure by removing the trivial `install.sh` wrapper, making the Rust binary the direct entry point.

#### **Step 4.1: ABB & Branching**
-   **4.1.1:** Create branches: `backup/WO-4-remove-install-sh-YYYYMMDD` and `issue/15-wo-4-remove-install-sh`.

#### **Step 4.2: Remove Wrapper**
-   **4.2.1:** **Remove** the `install.sh` file from the root directory.
-   **4.2.2:** Update `README.md` to instruct users to run the compiled `mash` binary directly (e.g., `target/release/mash`) or `cargo run --`.

#### **Step 4.3: Validation & Commit**
-   **4.3.1:** Run CI gates.
-   **4.3.2:** Commit with message: `chore(build): remove install.sh wrapper script`.

---

### **Work Order 5: Docker Build Automation for CI/CD**
**Goal:** Integrate Docker builds into the CI workflow to ensure a reproducible build environment and automatically produce container images.

#### **Step 5.1: ABB & Branching**
-   **5.1.1:** Create branches: `backup/WO-5-docker-ci-YYYYMMDD` and `issue/15-wo-5-docker-ci`.

#### **Step 5.2: Update CI Workflow**
-   **5.2.1:** Modify `.github/workflows/rust.yml` (or create a new `docker.yml`) to include steps for building the Docker image created in Issue #12. This should ideally push to a container registry.
-   **5.2.2:** Ensure the Docker build uses caching mechanisms efficiently (e.g., `cargo-chef` stages).

#### **Step 5.3: Validation & Commit**
-   **5.3.1:** Run CI gates. Confirm Docker image builds successfully in CI.
-   **5.3.2:** Commit with message: `ci(docker): integrate Docker build into CI/CD`.

---

### **Risks & Rollback Notes**
-   **High:** Changes to `Cargo.toml` and CI workflows can easily break the build. The `ABB` rule and `CI-only validation` are critical.
-   **Medium:** Incorrect path handling or command execution in Rust replacements could lead to subtle bugs. Gradual implementation is key.
-   **Rollback:** Always revert to the backup branch created before each Work Order if an issue cannot be immediately resolved.

### **Larry Mission Statement**
"Larry: read Issue #<n> and execute each Work Order (WO-1 through WO-5) end-to-end, following ABB/CI rules, and strictly adhering to the read-only principle for WO-1."

### **Megillah Template**
When this Epic is complete, post the following summary:
```md
## Epic: MASH Installer Rustification & Automation Phase 2 COMPLETE

### Summary
This Epic significantly advanced the Rustification and automation of the MASH installer. Key achievements include completing the read-only port of core logic, planning for write operations, formalizing agent personas, streamlining the installer entry point, and integrating Docker builds into CI/CD.

### Work Orders Completed:
-   [x] **WO-1: Complete `mash-core` Read-Only Logic Implementation**
-   [x] **WO-2: Plan `mash-core` Write Logic Implementation**
-   [x] **WO-3: Integrate `AGENT.md` as Agent Skill Files**
-   [x] **WO-4: Remove Root `install.sh` Wrapper Script**
-   [x] **WO-5: Docker Build Automation for CI/CD**

All CI checks are green, and the project is now significantly more robust and ready for the next phase of development.
```
