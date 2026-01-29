# MASH 🐀🍕 v1.2.4

**Minimal, Automated, Self‑Hosting installer for Fedora on Raspberry Pi**

MASH is a destructive, opinionated installer that automates the boring parts
while **explicitly preserving user choice where it matters**.

---

## What MASH actually does

- Downloads **Fedora 43 KDE (aarch64)** automatically
- Decompresses `.raw.xz → .raw` safely
- Loop‑mounts the image and installs via `rsync`
- Supports **MBR and GPT** partition schemes  
  - **MBR is default**
  - GPT is always available by choice
- Fixes UEFI layout (`EFI/BOOT/BOOTAA64.EFI` guaranteed)
- Boots straight into **Ratatouille** as user `mash`
- No pre-seeded images, no manual EFI repair

---

## Quick start (golden path)

```bash
sudo ./mash-installer flash --scheme mbr
```

Feeling brave?

```bash
sudo ./mash-installer flash --scheme gpt
```

---

## ⚠️ WARNING

This installer **DESTROYS THE TARGET DISK**.

You will be asked to confirm.
There is no undo.
Double‑check the device every time.

---

> *Anyone can cook.  
> This one just boots cleanly.*
