# The Dojo 🥋

Development principles and code philosophy for MASH.

---

## Core Rules

These are non-negotiable. Every contributor must follow them.

### 1. `MASH_GIT` is the Single Source of Truth

- All code changes go through the main repository
- No separate forks with divergent features
- Backups go in `bak/` directory
- Legacy scripts preserved in `legacy_scripts/HISTORY/`

### 2. Destructive Actions Must Be Explicit

MASH erases disks. This is intentional and necessary. But:

- **Always require confirmation** before destructive operations
- **Never auto-confirm** with hidden defaults
- The `--yes-i-know` flag exists for automation, but must be explicitly provided
- Display clear warnings before proceeding

Example:
```
⚠️  WARNING: This will ERASE ALL DATA on /dev/sda
    Device: SanDisk Ultra 32GB

    Type 'yes' to confirm:
```

### 3. GPT / MBR User Choice is Mandatory

The partition scheme decision is **always** left to the user.

- Never automatically select MBR or GPT
- Both must always be available as options
- MBR is the recommended default, but not forced
- Document the trade-offs, let the user decide

**Why?** Raspberry Pi UEFI firmware behaves differently across versions, boot media, and hardware revisions. Only the user knows their specific setup.

### 4. Overwrites Must Create a `bak/` Mirror

When modifying or replacing files:

- Copy the original to `bak/` first
- Preserve directory structure in backups
- Never silently overwrite without backup

This applies to:
- Configuration files
- Documentation updates
- Any file modification during development

---

## Style & Tone

### Code Should Be Noisy, Clear, and Defensive

- Log important operations: `info!("📍 Mounting EFI partition...")`
- Fail loudly with context: `.context("Failed to mount /dev/sda1")?`
- Never swallow errors silently
- Prefer verbose errors over cryptic ones

Good:
```rust
fs::create_dir_all(&mount_point)
    .with_context(|| format!("Failed to create mount point: {}", mount_point.display()))?;
```

Bad:
```rust
fs::create_dir_all(&mount_point).ok();
```

### Clever is Fine — Confusing is Not

Write code that's easy to understand:

- Clever optimizations are welcome if documented
- Magic numbers need comments
- Complex logic needs explanation
- If a reviewer asks "what does this do?", it needs clarification

### Humor is Encouraged 😈

We use emojis in logs and messages. This is intentional:

- 🍠 MASH branding
- ✅ Success indicators
- ⚠️ Warnings
- 💾 Disk operations
- 🔧 Configuration

It makes the tool friendlier without sacrificing clarity.

### Surprises Are Not Welcome

The user should always know:

- What will happen before it happens
- What is happening while it happens
- What happened after it finishes

No hidden side effects. No undocumented behavior.

---

## Code Guidelines

### Error Handling

- Use `anyhow` for application errors
- Add context to every fallible operation
- Return early on errors (fail fast)
- Cleanup must run even on error

### Logging

- `debug!` for internal details
- `info!` for user-visible progress
- `warn!` for recoverable issues
- `error!` for fatal problems

### Testing

- Unit tests for pure logic (locale parsing, path handling)
- Integration testing requires physical hardware
- Always support `--dry-run` for safe testing
- Never automate destructive tests in CI

### Documentation

- Code comments explain "why", not "what"
- Public functions need doc comments
- User-facing features need documentation
- Keep docs updated when code changes

---

## Git Workflow

### Commit Messages

- Present tense: "Add feature" not "Added feature"
- Explain what and why, not how
- Reference issues when relevant

### Branching

- `main` is the production branch
- Feature branches for development
- Squash trivial fixups before merging

### Never Force Push to Main

Force pushing to `main` can destroy history. Don't do it.

---

## Pull Request Checklist

Before merging:

- [ ] `cargo fmt --check` passes
- [ ] `cargo clippy -- -D warnings` passes
- [ ] `cargo test` passes
- [ ] Documentation updated if needed
- [ ] No new warnings introduced
- [ ] Destructive operations have confirmation
- [ ] User choice preserved where applicable

---

## Philosophy

MASH exists to solve a specific problem: installing Fedora on Raspberry Pi with UEFI boot is tedious and error-prone. We automate the boring parts.

But we also respect that:

- Users know their hardware better than we do
- Decisions about their data should be theirs
- Transparency builds trust
- Simple tools that do one thing well are valuable

When in doubt, ask: "Would I trust this tool with my data?"

---

## Dojo Program Catalogue

The Training Ground entry point now surfaces the curated catalogue described in `docs/dojo/catalogue_content_proposal.md`. Each category highlights a trusted default and a set of alternatives so users can choose intent-driven software without guessing trade-offs. The `v1.3.1` release notes (`docs/v1.3.1-release-notes.md`) capture how this catalogue fits into the larger Dojo narrative.

## See Also

- [ARCHITECTURE.md](ARCHITECTURE.md) — Technical design
- [DEPLOYMENT.md](DEPLOYMENT.md) — Building and packaging
- [QUICKSTART.md](QUICKSTART.md) — User guide
