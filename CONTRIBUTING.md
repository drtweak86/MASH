# Contributing

## Development Gates

Before opening a PR, run:

```bash
cargo fmt -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
```

## Maelstrom (Test Isolation)

CI requires Maelstrom to pass.

Install (one-time):

```bash
cargo install cargo-maelstrom
```

Run from the repo root:

```bash
cargo maelstrom --all-features
```

Notes:
- Maelstrom relies on Linux user namespaces and `clone3`. If you are running inside a restricted container, you may need a less restrictive seccomp profile or to run on the host.

