Here is a step-by-step plan to rewrite the scripts in Rust.

### 1. Inventory Table

| Path | Language | Purpose | Inputs | Outputs | External Dependencies | Risk Level | Recommendation |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `install.sh` | bash | Installs MASH by downloading, verifying, and running the installer. | - | Files in `/usr/local/bin`, `/etc/mash`, etc. | `uname`, `curl`, `sha256sum`, `tar`, `sudo`, `systemctl` | High | REWRITE IN RUST |
| `quick-fix.sh`| bash | Applies a temporary patch to the installer. | - | Modifies `install.sh`. | `sed` | High | DELETE |
| `Makefile` | make | Defines build, test, and release tasks. | - | Binary artifacts, Docker images. | `cargo`, `docker`, `git`, `gh` | Low | KEEP |
| `scripts/bump-version.sh` | bash | Bumps the project version in `Cargo.toml`. | Version string (e.g., `v1.2.3`) | Modifies `Cargo.toml`. | `sed` | Medium | REWRITE IN RUST |
| `scripts/mash-full-loop.py` | python | Placeholder for a full system setup loop. | - | - | - | Low | DELETE |
| `scripts/tag-release.sh` | bash | Creates and pushes a git tag for a new release. | Version string (e.g., `v1.2.3`) | Git tag in the remote repository. | `git`, `gh` | Medium | REWRITE IN RUST |
| `.github/workflows/build.yml` | yaml | CI workflow to build and test the Rust code. | - | Test results, build artifacts. | `cargo`, `actions/checkout` | Low | KEEP |
| `.github/workflows/rust.yml` | yaml | Standard Rust CI workflow. | - | Test results. | `cargo` | Low | KEEP |
| `helpers/*.sh` | bash | A collection of 20+ scripts to configure a Linux system. | Various | System state modifications (packages, configs, etc.). | `apt`, `systemctl`, `sed`, `tee`, etc. | High | MERGE |

### 2. Fit vs. Unfit Decision Notes

*   **`install.sh` (Unfit):** This script is complex, with multiple steps (download, checksum, unpack, install). A failure in any step can leave the system in an inconsistent state. It uses many external commands, making it fragile. Rewriting in Rust would make it a single, robust binary with better error handling and transactional steps.
*   **`quick-fix.sh` (Unfit):** This script modifies another script (`install.sh`) using `sed`. This is extremely dangerous and proves the installer is hard to maintain. This script should be deleted, and the logic it contains should be incorporated into the new Rust installer if still needed.
*   **`Makefile` (Fit):** `make` is the right tool for defining build-related tasks. It's a standard developer tool, and the defined workflows are simple. There is no benefit to rewriting this in Rust.
*   **`scripts/bump-version.sh` (Unfit):** Uses `sed` to modify `Cargo.toml`. This is risky. A Rust utility could use a proper TOML parser (`toml_edit`) to safely and reliably update the version.
*   **`scripts/mash-full-loop.py` (Unfit):** This appears to be an empty or placeholder script. It has no value and should be deleted.
*   **`scripts/tag-release.sh` (Unfit):** This script automates git operations. While simple, combining it with `bump-version.sh` into a single "release" tool written in Rust would be more robust and provide a better CLI experience.
*   **`.github/workflows/*.yml` (Fit):** These are GitHub Actions workflow configurations, not scripts. They are written in YAML and are executed by the GitHub Actions runner. They are fit for their purpose.
*   **`helpers/*.sh` (Unfit):** This is a large collection of tightly-coupled shell scripts that perform system administration. They are individually complex and have many external dependencies. Their ordering is critical and not enforced. Merging them into a single, idempotent Rust application with subcommands would be a massive improvement in reliability and maintainability.

### 3. Rustification Plan (Phased)

#### Phase 0: Backups and Safety Rails

1.  **Backup:** Before any changes, create a new branch `feature/rust-migration-audit`. All work will be done on this branch.
2.  **Rollback Plan:** To roll back, simply discard the changes on this branch or revert the commits. No production code will be touched until a PR is approved.
3.  **Safety:** Create a new Rust crate `tools/mash-tools` to house all the new code. This isolates the new code from the existing `mash-installer`.

#### Phase 1: Easiest Wins

*   **Scripts Included:** `scripts/bump-version.sh`, `scripts/tag-release.sh`
*   **Crate Layout:**
    ```
    tools/mash-tools/
    ├── Cargo.toml
    └── src/
        ├── main.rs
        ├── cli.rs
        └── release.rs
    ```
*   **CLI Design:**
    ```
    mash-tools release bump <VERSION>
    mash-tools release tag <VERSION>
    ```
*   **Error Handling:** `anyhow` for simple error propagation in the CLI.
*   **Logging:** `env_logger` for basic log messages.
*   **Behavior:** The tool will read `Cargo.toml` from the workspace root, update the version using the `toml_edit` crate, and execute `git` commands using `std::process::Command`.

#### Phase 2: The Installer

*   **Scripts Included:** `install.sh`
*   **Crate Layout:** Create a new crate `mash-installer-v2`.
    ```
    mash-installer-v2/
    ├── Cargo.toml
    └── src/
        ├── main.rs
        ├── download.rs
        ├── verify.rs
        └── install.rs
    ```
*   **CLI Design:**
    ```
    mash-installer-v2
    ```
    (No arguments needed, it should be self-contained).
*   **Error Handling:** `thiserror` to define specific error types for download failures, checksum mismatches, and installation errors. `anyhow` can be used in `main.rs`.
*   **Logging:** `tracing` with `tracing-subscriber` to provide structured logs.
*   **Behavior:** The new installer will perform the same steps as `install.sh` but within a single binary. It will use the proposed shared utilities for downloading and verification.

#### Phase 3: The System Setup monolith

*   **Scripts Included:** All scripts in `helpers/`.
*   **Crate Layout:** Add new modules to the `mash-tools` crate.
    ```
    tools/mash-tools/
    └── src/
        ├── setup/
        │   ├── packages.rs
        │   ├── firewall.rs
        │   └── ...
        ├── main.rs
        ├── cli.rs
        └── ...
    ```
*   **CLI Design:**
    ```
    mash-tools setup packages --core
    mash-tools setup packages --dev
    mash-tools setup firewall
    mash-tools setup fonts
    ...
    ```
*   **Error Handling:** `thiserror` for domain-specific errors (e.g., `PackageInstallFailed`), wrapped with `anyhow`.
*   **Logging:** `tracing` to log each step of the setup process.
*   **Behavior:** Each subcommand will encapsulate the logic from one of the shell scripts. The goal is to make each step idempotent.

### 4. Shared Rust Utilities Proposal

A new crate, `mash-common`, should be created to house shared utilities.

*   **Command Execution:** A helper function `run_command(cmd: &str, args: &[&str]) -> anyhow::Result<()>` that wraps `std::process::Command`, executes a command, and returns a detailed error if it fails.
*   **Downloads & Verification:** A function `download_and_verify(url: &str, checksum: &str) -> anyhow::Result<Vec<u8>>` that uses `reqwest` to download a file and `sha2` to verify its checksum.
*   **File Editing:** Use `toml_edit` for TOML files and potentially `serde_yaml` for YAML files if needed in the future.
*   **Progress Reporting:** The `indicatif` crate can be used to show progress bars for downloads and long-running operations. It is compatible with `ratatui`.

### 5. "Do Not Rustify" List

*   **`Makefile`:** It's a build system tool, not a script. It's doing its job correctly.
*   **`.github/workflows/*.yml`:** These are CI configuration files, not scripts.

---

### Top 5 Highest-Risk Scripts

1.  **`install.sh`**: High complexity, performs privileged operations, and has many external dependencies.
2.  **`helpers/11_snapper_init.sh`**: Deals with filesystem snapshots, which is inherently risky.
3.  **`helpers/12_firewall_sane.sh`**: Modifies firewall rules, which can lock a user out of their system.
4.  **`helpers/16_mount_data.sh`**: Modifies `/etc/fstab`, which can render a system unbootable.
5.  **`quick-fix.sh`**: Modifies executable code on the fly. Extremely dangerous.

### Top 5 Fastest Wins

1.  **`scripts/mash-full-loop.py`**: Easiest win is to delete it.
2.  **`scripts/bump-version.sh`**: Simple, self-contained, and a perfect candidate for a small Rust utility.
3.  **`scripts/tag-release.sh`**: Also very simple and can be combined with the version bumper.
4.  **`quick-fix.sh`**: Deleting this is a quick win for security and maintainability.
5.  **`helpers/03_stage_starship_toml.sh`**: A very simple script that just copies a config file. Easy to rewrite.

### "Definition of Done" Checklist for a Rewrite PR

-   [ ] The new Rust code is in its own crate or module.
-   [ ] The old script has been deleted.
-   [ ] The new Rust implementation passes all CI checks.
-   [ ] The new implementation has 100% behavioral parity with the old script.
-   [ ] A `README.md` file is included in the new crate explaining its purpose and usage.
-   [ ] CLI arguments and flags (if any) are documented.
-   [ ] The PR has been reviewed and approved by at least one other engineer.
