# Dojo Catalogue Extensibility (Other Distros)

This document explains how the Dojo Program Catalogue (`InstallSpec`) is designed to grow beyond Fedora, and what work is required to safely support additional distributions (e.g., Debian/Ubuntu, Arch).

## Current State (Fedora-First)

- The catalogue schema is defined in `mash-core/src/dojo_catalogue.rs` and documented with a TOML reference in `docs/dojo-catalogue.schema.toml`.
- Entries declare:
  - `supported_distros` (what distros the entry is intended to support)
  - `install_method` (how it would be installed)
  - `packages` (Fedora RPM package names live under `packages.fedora`)
  - `risk_level`, `default_tier`, plus dependency/conflict metadata.
- The current Dojo first-boot UI (`mash-dojo`) only *shows* programs that support Fedora (`supported_distros` contains `fedora`). This is the Fedora-first scope guard in practice.

## Design Goal

Keep a single catalogue file that can describe options across multiple distros, while ensuring:

- The UI only offers options that are valid for the detected distro.
- The provisioning backend only attempts methods it actually implements.
- Safety/UX rules (e.g., Spicy confirmation) remain consistent across distros.

## Extension Plan

### 1) Expand `packages` to be multi-distro (schema)

Today the schema supports:

- `packages.fedora = ["..."]`

To support other distros, extend `PackageSpec` and the TOML schema to include (examples):

- `packages.debian = ["..."]` (Debian/Ubuntu `apt`)
- `packages.ubuntu = ["..."]` (optional split if needed)
- `packages.arch = ["..."]` (Arch `pacman`)

If/when the per-distro surface grows, consider replacing the fixed struct with a map form (e.g., `packages = { fedora = [...], debian = [...] }`) to avoid repeated schema churn.

### 2) Detect distro at runtime

Add a small “distro detector” that:

- reads `/etc/os-release`
- maps it into a `SupportedDistro` (or a richer `Distro` type)

This output becomes the single source of truth used for:

- catalogue filtering (`supported_distros`)
- choosing an install backend

### 3) Implement per-distro install backends

Introduce backends behind a shared interface (pseudo):

- Fedora: `dnf`
- Debian/Ubuntu: `apt-get` / `apt`
- Arch: `pacman`

Rules:

- If the current distro is not supported by the selected `InstallSpec`, the UI must hide it and the backend must refuse it (defense in depth).
- If `install_method` is not implemented on that distro, the UI must hide it (or mark as “not yet supported”) until implemented.

### 4) Keep conflict/require rules distro-agnostic

The conflict/require graph should remain consistent regardless of distro:

- `requires`, `conflicts_with`, `alternatives` are expressed in terms of `InstallSpec.id` (intent-level IDs), not package names.
- Validation in `mash-core/src/dojo_catalogue.rs` should continue to enforce:
  - references exist
  - conflicts are symmetric
  - no self-references / duplicates

### 5) TUI behaviour when new distros are added

Once distro detection exists, the TUI should filter by the detected distro instead of hard-coding Fedora. The selection view rules remain:

- Default view: CoreDefault + Champion + Alternative (up to 5, after filtering)
- Expanded view: Top 5 (after filtering)
- `risk_level = spicy` requires explicit confirmation with implications

### 6) Versioning and migrations

- Bump `schema_version` when the schema meaningfully changes.
- Keep older schema versions readable if practical, or provide a conversion tool.
- Add unit tests for:
  - filtering behaviour per distro
  - backend selection + refusal when unsupported
  - “Spicy” confirmation logic

