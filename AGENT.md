# Agent Contract – Larry

## Project
MASH Installer – Rust TUI Wizard

## Current Phase
Phase B1 – Stub-backed UI state (NO real disk logic)

## Active Rules
- Do NOT perform real disk scanning
- Do NOT read from flash.rs
- Do NOT add system dependencies
- Stub/mock all data sources
- UI completeness > data correctness
- CI must remain green

## Allowed
- Fake disks (/dev/sda, /dev/nvme0n1)
- Fake image lists and versions
- Fake locale/keymap lists
- Fake UEFI directory paths
- Derived confirmation summary from state

## STOP Conditions
Stop immediately if:
1. CI fails
2. A step cannot be reasonably stubbed
3. Real disk scanning would be required
4. A new GitHub issue is needed

## Success Criteria
- All wizard steps render selectable options
- Forward/back navigation works
- Confirmation screen shows coherent summary
- No blank screens
- cargo fmt, clippy, test pass

## Next Planned Phase
Phase B2 – Replace stubs with real plumbing (future issue)
