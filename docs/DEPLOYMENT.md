# MASH Deployment Guide 🚀

How to build, package, and distribute MASH.

---

## 📦 What Ships

A MASH release includes:

- `mash` — The CLI/TUI binary (statically linked where possible)
- Source tarball (for building from source)

**Not included:**
- `.git/` history
- Pre-downloaded Fedora images
- Cached downloads
- Test artifacts

This keeps releases small, auditable, and reproducible.

---

## 🔧 Build Prerequisites

### Rust Toolchain

Install Rust 1.70 or later:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### System Dependencies

**Debian/Ubuntu:**
```bash
sudo apt install build-essential pkg-config libssl-dev
```

**Fedora:**
```bash
sudo dnf install gcc pkg-config openssl-devel
```

**Arch:**
```bash
sudo pacman -S base-devel openssl
```

### Runtime Dependencies

The built binary requires these tools at runtime:
- `parted`
- `rsync`
- `xz`
- `mkfs.vfat` (dosfstools)
- `mkfs.ext4` (e2fsprogs)
- `mkfs.btrfs` (btrfs-progs)
- `losetup` (util-linux)

## 🔗 OS image verification

Fedora Workstation remains the primary image for MASH; grab it from https://getfedora.org/en/workstation/download/. Optional OS downloads are documented in `docs/OS_IMAGE_LINKS.md`, and `.github/workflows/os-download-links.yml` pings those URLs daily via HTTP HEAD to keep them accurate. Update the `docs/os-download-links.toml` list and rerun the workflow if any link changes.

---

## 🏗️ Building

### Release Build

```bash
make build-cli
```

Output: `mash-installer/target/release/mash`

### Debug Build

```bash
make dev-cli
```

Faster compilation, includes debug symbols.

### Direct Cargo Build

```bash
cd mash-installer
cargo build --release
```

---

## ✅ Testing

### Run All Tests

```bash
make test
```

### Run Specific Test

```bash
cd mash-installer
cargo test test_locale_parsing
```

### Linting

```bash
make lint
```

Runs `clippy` with `-D warnings` — fails on any warning.

### Formatting

```bash
make format
```

Runs `cargo fmt` to format code.

---

## 📋 Pre-Release Checklist

Before releasing a new version:

1. **Update version number:**
   ```bash
   make bump-patch   # or bump-minor, bump-major
   ```

2. **Run full test suite:**
   ```bash
   make test
   make lint
   ```

3. **Build release binary:**
   ```bash
   make build-cli
   ```

4. **Test on real hardware:**
   - Run with `--dry-run` first
   - Test on actual SD card (use sacrificial media)
   - Verify boot on Raspberry Pi 4

5. **Create distribution tarball:**
   ```bash
   make dist
   ```

---

## 📦 Creating a Release Tarball

```bash
make dist
```

This creates `dist/mash-installer-X.Y.Z.tar.gz` containing:
- `mash` binary
- `README.md`
- `LICENSE`

### Manual Tarball

```bash
VERSION=$(grep '^version' mash-installer/Cargo.toml | cut -d'"' -f2)
mkdir -p dist/mash-$VERSION
cp mash-installer/target/release/mash dist/mash-$VERSION/
cp README.md LICENSE dist/mash-$VERSION/
cd dist && tar -czf mash-$VERSION.tar.gz mash-$VERSION
```

---

## 🔢 Version Bumping

MASH uses semantic versioning (X.Y.Z):

```bash
make bump-major   # Breaking changes (1.0.0 → 2.0.0)
make bump-minor   # New features (1.0.0 → 1.1.0)
make bump-patch   # Bug fixes (1.0.0 → 1.0.1)
```

This runs `scripts/bump-version.sh` which:
- Updates `Cargo.toml`
- Updates `README.md` version badge
- Creates a git commit

---

## 🖥️ Cross-Compilation

### For aarch64 (Raspberry Pi)

```bash
# Install target
rustup target add aarch64-unknown-linux-gnu

# Install cross-compiler
sudo apt install gcc-aarch64-linux-gnu

# Build
cd mash-installer
cargo build --release --target aarch64-unknown-linux-gnu
```

Output: `target/aarch64-unknown-linux-gnu/release/mash`

### Using Cross (Docker-based)

```bash
# Install cross
cargo install cross

# Build
cd mash-installer
cross build --release --target aarch64-unknown-linux-gnu
```

---

## 📁 Installation

### Local Install

```bash
make install-cli
```

Installs to `/usr/local/bin/mash`.

### Custom Prefix

```bash
make PREFIX=/opt/mash install-cli
```

### Package Building (DESTDIR)

For creating distribution packages:

```bash
make DESTDIR=/tmp/pkg PREFIX=/usr install-cli
```

Creates `/tmp/pkg/usr/bin/mash`.

---

## ⚠️ Testing Guidelines

### Always Use Sacrificial Media

- Never test on your system drive
- Use cheap SD cards for testing
- Expect complete data loss on test media

### Dry Run First

Always verify with `--dry-run` before real writes:

```bash
sudo mash flash --disk /dev/sda --scheme mbr --dry-run ...
```

### MBR vs GPT Testing

- **MBR** is the safer default — test this first
- **GPT** may have firmware-specific issues
- Document any GPT quirks you discover

### Multi-Pi Testing

Different Pi 4 revisions may behave differently. Test on:
- Pi 4 1GB (early revision)
- Pi 4 4GB/8GB (later revisions)
- Different UEFI firmware versions

---

## 🔄 CI/CD Notes

### Minimum CI Steps

```yaml
- cargo fmt --check
- cargo clippy -- -D warnings
- cargo test
- cargo build --release
```

### Automated Testing Limitations

Full installation testing requires physical hardware and is too dangerous to automate. CI should cover:
- Unit tests
- Linting
- Build verification
- Dry-run simulation (if feasible)

---

## See Also

- [ARCHITECTURE.md](ARCHITECTURE.md) — Technical design
- [DOJO.md](DOJO.md) — Development principles
- [QUICKSTART.md](QUICKSTART.md) — User guide
