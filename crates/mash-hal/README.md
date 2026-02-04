# mash-hal

`mash-hal` is the single boundary for "world-touching" code in MASH.

Rules (enforced by convention + clippy/CI):
- Do **not** call `std::process::Command` outside `crates/mash-hal`.
- Do **not** read Linux host state from `/proc`, `/sys`, or `/etc` outside `crates/mash-hal`.
- Route system interactions through `mash_hal` traits (e.g. `InstallerHal`, `ProcessOps`, `HostInfoOps`,
  `MountOps`, `PartitionOps`, `FlashOps`), and use `FakeHal` in tests where possible.

This keeps the workflow/TUI layers deterministic and testable, and makes it explicit where privileges and
side-effects live.

