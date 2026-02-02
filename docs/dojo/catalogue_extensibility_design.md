# Dojo Catalogue Extensibility Design

## Purpose
We expect the core Dojo catalogue to remain curated and safe, but we also want to let advanced users or community contributors extend the set of categories later. This design outlines how to discover, validate, and surface external catalogues without compromising security or the "press enter and go" defaults.

## Discovering external catalogues
1. **Registry file:** Maintain a `docs/dojo/catalogue_registry.toml` or user-writable `~/.config/mash/catalogue_sources.toml` listing trusted catalogue sources (URL, checksum, signature, priority).
2. **Local drop-in directory:** Support a `dojo_catalogues.d/` folder where users or automation can drop TOML files. The installer reads every file, validates it, and merges it with the official catalogue.
3. **Discovery workflow:** On startup, the installer loads the registry, fetches each remote source over HTTPS, caches it with a versioned filename, and only enables it if checksum/signature validation succeeds.

## Validating external schema
- External catalogues must comply with the schema defined in Issue #50. The loader runs a `serde` deserialization pass and rejects files that miss required fields (e.g., `id`, `label`, `package_names`).
- Additional heuristics verify that each program entry includes `reason_why` and toggles `requires_expert_mode` when the choice is not curated.
- Validation errors are surfaced via a log entry and a TUI banner: "External catalogue <name> is invalid and was skipped." The official catalogue remains unaffected.

## Managing conflicts with official catalogues
1. **Priority metadata:** Each catalogue declares a `priority` value. Official entries use priority `100`, while external catalogues default to `10`. When two catalogues claim the same category ID or program ID, the one with higher priority is used.
2. **Slot-level warnings:** For conflicts with official defaults (e.g., both show a `Web Browser` default), the UI explicitly labels the external alternative as `external` and marks it with a warning badge. The user must explicitly accept the external choice (e.g., by toggling expert mode), preventing accidental overrides of curated defaults.
3. **Conflict resolution policy:** External catalogues can declare `replace_official = true` for specific categories, but the loader only honors this if the user has enabled the catalogue in settings; otherwise, the entry is treated as an alternative.

## UX for enabling/disabling external catalogues
- Within the TUI settings (or a `Dojo → Catalogues` menu), users can toggle each discovered catalogue. Enabled catalogues appear alongside official ones; disabled ones are kept in the registry but not merged.
- Enabling a catalogue triggers a preview dialog showing the new categories/programs and their `reason_why`, plus a list of any conflicts with official entries.
- External catalogues default to disabled to preserve safety. An explicit action (e.g., `Enable external catalogue: [x]`) is required, and the UI records the user’s choice in `~/.config/mash/catalogue_state.json`.

## Maintaining curation and safety
- Even when external sources are enabled, the official catalogue remains the baseline. External programs must opt into expert mode (`requires_expert_mode = true`) so that only advanced users see them unless they explicitly toggle the gate.
- Safety metadata (e.g., `conflicts_with`, `requires_clean_state`) is still enforced for external entries. The loader rejects any external entry that tries to bypass these fields.
- External catalogues are rate-limited: reloading is manual (`F5: refresh catalogues`) or on-demand when a new file appears, preventing automatic (and potentially malicious) updates during an install.

## Schema validation mechanisms
- The loader maintains a JSON Schema/TOML spec derived from the official `serde` structs. Each external file is run through `toml::from_str` and optionally sanitized via a schema validator (e.g., `jsonschema`-like).
- We keep `catalogue_schema_digest` checksums in the canonical repo so the installer can detect when new schema fields appear and warn upstream contributors to regenerate their catalogues.

## Security implications & mitigations
- **Remote fetch authenticity:** Remote catalogues must be fetched via HTTPS and signed (e.g., a detached Ed25519 signature stored alongside the file). The loader verifies the signature before merging.
- **Execution risk:** External catalogues influence package installation; we limit them to `package_names` declarations and do not allow arbitrary scripts. Any attempt to specify hooks or shell commands is rejected during validation.
- **User awareness:** TUI warnings explicitly mention that the catalogue is external and may not be vetted by Team MASH. Expert-mode gating keeps novices inside the curated experience.
- **Audit trail:** The installer records which catalogues were enabled/disabled in `catalogue_state.json`, so support can identify which third-party content affected an install.

## Conclusion
Extensibility lives in a layered approach: discovery + validation + explicit user opt-in. The official catalogue remains untouched, schema checks enforce structure, conflict metadata prevents unsafe combinations, and expert gating plus signature checks mitigate the security surface for community-driven contributions.
