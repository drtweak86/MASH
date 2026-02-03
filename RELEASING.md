# Releasing + Branch Policy

This repo uses a simple branching model designed to work with ABB (Always Be Backing Up).

## Permanent Branches

- `main`
  - Production/default branch.
  - Must always be green (format/clippy/tests) before pushing.
- `develop`
  - Integration branch for stacking work before landing on `main`.
  - Should be kept close to `main` (fast-forward/merge from `main` regularly).

## Transient Branches

- `issue/<number>-<short-name>` or `feature/<short-name>`
  - Working branches for a single change set.
  - Delete after the work is merged into `main` (or `develop`, if used).

## ABB (Backup) Branches

- `backup/<topic>-<YYYYMMDD-HHMMSS>` and `abb/<topic>`
  - Checkpoint branches created to preserve work in progress or risky refactors.
  - These branches are allowed to be messy; they are safety nets.

Retention guidance:
- Keep ABB branches for at least 30 days after merge.
- After that, either delete them or convert them to an annotated tag if they represent a useful historical snapshot.

## Recommended Workflow

1. Create a work branch from `main`:
   - `issue/<n>-...` for issue-scoped work.
2. Create an ABB checkpoint before risky changes:
   - `backup/<topic>-<timestamp>`
3. Keep `main` green:
   - `cargo fmt`
   - `cargo clippy -- -D warnings`
   - `cargo test`
4. Merge into `main` (or `develop` -> `main` if using `develop`).
5. Delete the transient work branch after merge.

