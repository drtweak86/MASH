# Releasing MASH

This project uses Rust tooling for release automation. The authoritative workflow is the `mash-release` tool under `tools/mash-release/`.

## Requirements

- A clean working tree (unless `--allow-dirty` is explicitly used).
- Cargo.toml version values must be strict SemVer (`X.Y.Z`).
- Tags must be `vX.Y.Z` with no extra text.

## Standard Release Flow

1. Run a dry run to review the plan:
   ```bash
   cargo run --manifest-path tools/mash-release/Cargo.toml -- --dry-run
   ```
2. Perform the release (example bump):
   ```bash
   cargo run --manifest-path tools/mash-release/Cargo.toml -- --bump patch --yes
   ```

The tool will update `mash-installer/Cargo.toml`, optionally update the README title line if it contains `vX.Y.Z`, run formatting and tests, then commit, tag, and push (unless `--no-tag` or `--no-push` are provided).

## Notes

- The legacy bash script remains in `tools/release/release.sh` for reference only.
- Do not manually insert non-SemVer version strings into `mash-installer/Cargo.toml`.
