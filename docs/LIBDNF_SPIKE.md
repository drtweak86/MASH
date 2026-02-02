# libdnf Spike (Issue #30)

## Fedora Build Dependencies

To generate bindings with `bindgen` against `libdnf`, Fedora needs:

- `libdnf-devel`
- `clang`
- `clang-devel`
- `llvm-devel`
- `pkgconf-pkg-config`
- `gcc` (or `gcc-c++` if needed by LLVM tooling)

These provide the headers (`libdnf`) and `libclang` needed by `bindgen`.

## POC Crate

`crates/libdnf-sys` is a minimal sys crate that:

- Uses `pkg-config` to locate `libdnf` headers.
- Runs `bindgen` in `build.rs`.
- Emits bindings into `OUT_DIR` and exposes them via `include!`.

This crate intentionally does **not** link to `libdnf` yet; it only proves
that bindings can be generated and compiled in CI.

## Proposed API Surface (mash-core)

The stable API surface remains the existing trait in
`mash-core/src/system_config/packages.rs`:

```rust
pub trait PackageManager {
    fn install(&self, pkgs: &[String]) -> Result<()>;
    fn update(&self) -> Result<()>;
}
```

This keeps callers decoupled from the underlying implementation:

- Current: `DnfShell` (direct `dnf` invocation).
- Future: `LibDnfManager` behind the `libdnf` feature flag.

No integration is performed in this spike; the goal is to validate feasibility
and define the minimal trait boundary for a safe swap later.
