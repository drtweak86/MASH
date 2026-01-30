# Legacy Scripts Reference 📜

This document describes the historical shell and Python scripts that preceded the current Rust implementation.

---

## Why Keep These?

The MASH installer evolved through several iterations before becoming a Rust application. These legacy scripts:

- **Document the project's evolution** — Understanding what came before helps understand design decisions
- **Preserve working implementations** — Some techniques may be useful as reference
- **Maintain institutional knowledge** — The "why" behind various approaches

They are **not used** by the current installer. They are preserved for historical reference only.

---

## Location

Legacy scripts are stored in:
```
legacy_scripts/
├── HISTORY/           # Detailed diffs and analysis
│   ├── ninja.py.md
│   ├── holy-loop-fedora.sh.md
│   ├── holy-loop-fedora-ninja.py.md
│   ├── holy-loop-fedora-mbr-dracut.sh.md
│   └── mash_bootstrap.py.md
└── MERGE_SUMMARY_v1.0.md
```

---

## Script Lineage

### `ninja.py`

**Purpose:** Early partition layout and flashing logic in Python.

- **Master version:** `ninja-mbr4-final.py`
- **Variants:** 7 iterations

Key features that carried forward:
- 4-partition layout (EFI, BOOT, ROOT, DATA)
- MBR vs GPT option
- Loop device handling

### `holy-loop-fedora.sh`

**Purpose:** Shell script for loop-mounting and rsync-based installation.

- **Master version:** `holy-loop-fedora-final.sh`
- **Variants:** 3 iterations

Key techniques:
- Loop-mount source image
- rsync with archive mode
- UEFI boot configuration

### `holy-loop-fedora-ninja.py`

**Purpose:** Python version combining loop-mount + ninja partitioning.

- **Master version:** `holy-loop-fedora-ninja-final.py`
- **Variants:** 6 iterations

Combined the best of both approaches.

### `holy-loop-fedora-mbr-dracut.sh`

**Purpose:** MBR-specific implementation with dracut integration.

- **Master version:** `holy-loop-fedora-mbr-dracut.sh`
- **Variants:** 3 iterations

Focused on:
- MBR partition table creation
- Dracut initramfs regeneration
- UEFI boot setup for MBR layouts

### `mash_bootstrap.py`

**Purpose:** Bootstrap/installer orchestration script.

- **Master version:** `mash_bootstrap_v2_2.py`
- **Variants:** 5 iterations

High-level orchestration that:
- Downloaded Fedora images
- Called partitioning scripts
- Configured first-boot

---

## What Changed in Rust

The Rust implementation (`mash-installer/`) replaced all of these with:

| Old | New |
|-----|-----|
| Shell/Python scripts | Single Rust binary |
| Text-based prompts | Ratatui TUI |
| Manual error handling | anyhow + context |
| Scattered logic | Modular architecture |
| No progress tracking | Real-time progress updates |

---

## Reading the History Files

Each `*.md` file in `HISTORY/` contains:

1. **Summary** — What the script did
2. **Variant list** — Different versions and their purposes
3. **Diffs** — Changes between versions
4. **Notes** — Why changes were made

These are useful when:
- Debugging obscure boot issues
- Understanding partition layout rationale
- Referencing shell commands for edge cases

---

## Note on the `legacy/` Directory

There's also a `legacy/` directory at the repository root containing:
- `legacy/scripts/` — Older script copies
- `legacy/README.md` — Basic overview

This predates the organized `legacy_scripts/HISTORY/` structure and may contain duplicates.

---

## See Also

- [ARCHITECTURE.md](ARCHITECTURE.md) — Current implementation design
- [DOJO.md](DOJO.md) — Development principles
