# Dojo Catalogue TUI Interaction Design

## Overview
The coveted "press enter and go" experience extends to the Dojo Program Catalogue screen. Users move through categories, view the curated defaults/alternatives, and make **exactly one selection per category**. Expert-only options remain hidden until the traveller explicitly unlocks them.

---

## Screen layout mockup
```
┌─────────────────────────────────────────────────────────────┐
│ CATEGORY: Web Browser         [Expert mode: disabled]       │
│ ← Previous   ↑/↓ Options   Enter = select   → Next          │
├─────────────────────────────────────────────────────────────┤
│ ● Firefox ESR                       (default)              │
│   Reason: upstream ESR, stable on Fedora ARM                │
│    [ENTER] select this, [TAB] inspect package list          │
│ ○ Chromium                          (alternative)          │
│   Reason: familiar Chrome base; more frequent updates        │
│   [ENTER] select this; toggles default off                  │
│ ○ Ungoogled Chromium (expert only) (locked)                │
│   Reason: privacy-first but manual updates required.        │
│   [E] unlock expert mode to reveal gating instructions       │
└─────────────────────────────────────────────────────────────┘
```

- The header shows the current category, expert mode status, and navigation hints.
- Options list uses circle indicators (`●` for default, `○` for non-default). Selected item adds a checkmark and a brighter color.
- Each entry exposes its `reason_why` text directly below the label for quick comparison.

---

## Navigation rules
- **Left/Right arrows** (or `Tab/Shift+Tab`) cycle categories; each category preloads its programs to keep the UI responsive.
- **Up/Down** arrows move between program entries within the current category.
- **Enter** selects the highlighted option and automatically deselects any other choice in that category (enforcing "exactly one selection").
- **Space** or **double Enter** can also toggle the active cursor and selection at the same time for keyboard accessibility.

---

## Default presentation and overrides
- Defaults are marked with `●` plus the `(default)` note and are visually highlighted (bold + green/text accent).
- When the user selects an alternative, the default indicator stays for reference but its highlight dims, making it obvious the canonical suggestion has been overridden.
- A summary bar at the bottom reads `Current choice: Firefox ESR (default)` or `Current choice: Chromium (custom)` depending on whether the default is active.
- The first category automatically focuses its default entry so novice users can hit Enter immediately.
- A `Reset to defaults` shortcut (e.g., `Ctrl+D`) reverts all categories to their recommended defaults.

---

## Enforcing the "one choice" rule
- The UI keeps a per-category state that stores the selected option ID. Selecting option `X` in category `Y` writes to this slot and clears all other option checkmarks in `Y` before drawing.
- Selecting the same option twice toggles it off and back on (no multi-selection), preventing race conditions when keyboard input arrives rapidly.
- A warning message appears if the user attempts to proceed (e.g., `F12: Continue`) while any category still has no selection. The warning lists empty categories.

---

## Expert/advanced option gating
- Expert mode remains **off** by default. The header hint shows `Expert mode: disabled` and a footer line explains `Press E to show gated options.`
- Pressing `E` toggles expert mode, revealing entries flagged as `requires_expert_mode` from the schema. Locked entries display a `[Locked]` badge until `Expert mode` is enabled.
- While expert mode is active, the header becomes amber, and a sticky note below the category warns `Advanced options may require manual updates.`
- Turning expert mode off hides the gated alternatives again, but already-selected advanced choices remain selected; turning expert mode off simply greys them out but preserves the selection state.
- Expert mode is remembered for the session but resets to off on the next run (to keep defaults safe).

---

## Visual cues
| Cue | Meaning | Implementation notes |
| --- | --- | --- |
| `●` + `(default)` | Default recommendation | Default entries show bold text and green accent. |
| `✔` appended to a label | Current selection | Always visible for whichever option is active, even if it is the default. |
| Bright highlight + reason text | Focused item | The currently hovered option uses a brighter background and keeps the reason visible. |
| Dimmed label + `[Locked]` | Expert-only alternative, expert mode off | Cannot be selected yet; pressing Enter prompts the expert mode hint. |
| `Expert mode: enabled` header | Expert toggle active | Header turns amber and the body scrolls to show advanced items. |

---

## Summary
The catalogue TUI guides users through categories with persistent defaults, enforces one choice per category, and hides expert selections until explicitly unlocked. The mix of textual hints, reason copy, and icons keeps the experience friendly while still surfacing advanced workflows when requested.
