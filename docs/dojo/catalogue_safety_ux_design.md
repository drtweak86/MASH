# Dojo Catalogue Safety & UX Design

## Objectives
Safety is paramount for Dojo’s curated catalogue. This document describes how the UI and backend work together to avoid conflicting selections, explain each choice, and let users intentionally skip categories.

## 1. Preventing mutual-exclusion conflicts
1. **Metadata-driven conflicts:** Each program entry may expose `conflicts_with: ["id1", "id2"]` referencing other catalogue entries (e.g., `firewall-cmd` conflicts with `ufw`). When the selection engine evaluates a candidate, it scans the other categories for their current selections and rejects the change if there is an overlap. The UI reports a modal warning such as "Cannot install `firewall-cmd` while `ufw` is selected; choose one." The warning lists the conflicting selections and suggests the user revert or skip one of them.
2. **Category-level exclusivity:** Some categories (like desktop environments) are inherently exclusive. The schema allows `category.exclusive = true`, and the TUI enforces single selection while also disabling alternatives that would leave the system in an unsupported mixed state.
3. **Conflict hints:** When a conflicting entry is highlighted, the reason area includes `Conflicts with: <list>` so the user can see the clash before selecting.
4. **Rollback action:** If a conflict is detected late (e.g., during a pipeline dry-run), the pipeline writes a structured error and the UI scrolls to the offending category, flips it into focused mode, and displays the blocking message.

## 2. Showing `reason_why` explanations
- The catalogue screen dedicates a compact "reason" pane below the option list. When the cursor moves to a program, its `reason_why` text populates the pane so users know why it was curated (e.g., "Stable on Fedora ARM with minimal telemetry").
- Defaults show a short version of the reason plus a "Why default?" indicator, while alternatives show deeper tradeoffs, such as maintenance burden or feature differences.
- Pressing `I` while an option is focused toggles a tertiary view that surfaces the full textual `reason_why` plus any guidance about required knowledge or caveats.
- Tooltips in the TUI (e.g., `Press ? for help`) remind users that `reason_why` is available for every decision.

## 3. Skipping categories
- A special choice `Skip this category` appears at the end of each category when `category.allow_skip = true` is set in the schema. Its entry is not `default`, but selecting it counts as the "one selection" for that category.
- Skipping intentionally leaves the system unchanged for that intent (e.g., a user may skip `Media Player` to avoid installing bloat). The summary bar will show `Media Player: skipped (no packages installed).`
- The TUI warns if skipping might result in missing functionality (e.g., skipping `Web Browser` will show `You won’t have a browser unless you install one later.`) but still lets the user proceed.
- Skipped categories remain highlighted so the user can easily return to them. There is also a `Reset skips` hotkey (Ctrl+S) to revert all skipped categories to their defaults.

## 4. Handling pre-existing system software conflicts
1. **Inventory check:** When Dojo starts, a lightweight inventory gathers installed packages via `dnf repoquery --installed` or `rpm -qa`. The catalogue maps program IDs to Fedora packages, so the installer can detect pre-existing installations of conflicting programs.
2. **Conflict messaging:** If the user selects a program that is already present in the system but tagged as `conflicts_with`, the UI shows `Conflict with existing package: <package>`. The user can choose to (a) uninstall the conflicting package (invoked by the stage runner), (b) skip the new installation, or (c) toggle an override flag (expert action).
3. **Forced resolution for critical conflicts:** Some programs (e.g., multiple desktop environments) require the installer to remove the old package before proceeding. Those entries set `requires_clean_state = true`, the TUI prohibits selection until the user uninstalls or reboots, and the pipeline aborts with a clear error message if the conflict still exists during `run_pipeline`.
4. **Dry-run safety:** In dry-run mode, the conflict detection runs early and surfaces a dedicated warning panel with `Potential conflict with installed <package> — nothing will be changed unless you run with --execute.`

## 5. Summary
Safety rests on declarative conflict metadata, rich `reason_why` copy, a visible skip option, and early warnings for existing system software. The TUI highlights conflicts, the pipeline double-checks them before mutating the system, and expert overrides remain gated to protect less experienced users.
