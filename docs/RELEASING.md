# Releasing MASH

This project uses Rust tooling for release automation. The authoritative workflow is the `mash-tools` release CLI under `tools/mash-tools/`.

## Requirements

- A clean working tree (unless `--allow-dirty` is explicitly used).
- Cargo.toml version values must be strict SemVer (`X.Y.Z`).
- Tags must be `vX.Y.Z` with no extra text.

## Standard Release Flow

1. Bump the version in `mash-installer/Cargo.toml`:
   ```bash
   cargo run --package mash-tools -- release bump X.Y.Z
   ```
2. Create and push the tag:
   ```bash
   cargo run --package mash-tools -- release tag X.Y.Z
   ```

The tool updates `mash-installer/Cargo.toml`, runs workspace checks, and enforces clean working tree unless `--allow-dirty` is supplied. Tags are always `vX.Y.Z`.

## Notes

- Do not manually insert non-SemVer version strings into `mash-installer/Cargo.toml`.

## Branch management policy

- `main` is the single permanent canonical branch that matches the released state.
- Temporary branches should follow `issue/<N>-<short-slug>` and be deleted once the work merges.
- Snapshot or emergency backups use `backup/...` or `abb/...` prefixes; they must be pruned when no longer needed to keep the branch list manageable.
- Avoid creating long-lived feature branches with feedback loops. If someone needs to pause work, create a short-lived issue or rescope.
- Local agent or tooling directories (e.g., `.git/info/exclude`, `.claude`, `.docker`) should not be committed; they are already covered by `.gitignore`.
