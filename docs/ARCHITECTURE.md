# Architecture 🧠

## High‑level flow

1. Download Fedora `.raw.xz`
2. Decompress to `.raw`
3. Loop‑mount image partitions
4. User selects partition scheme (**MBR or GPT**)
5. Target disk partitioned and formatted
6. Root filesystem copied via `rsync`
7. EFI partition merged
8. `BOOTAA64.EFI` enforced
9. First‑boot hooks staged

---

## Why MBR *and* GPT?

Raspberry Pi UEFI firmware behaves differently across versions,
firmware builds, and storage media.

- **MBR** is the safest default
- **GPT** is supported for modern setups
- MASH never removes the choice

The user decides.
The installer adapts.

---

## First boot behaviour

- Autologin as `mash`
- Locale: UK
- Ratatouille launches automatically
- Installer artefacts self‑destruct
