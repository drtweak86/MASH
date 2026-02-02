# Dojo Program Catalogue Content Proposal

This proposal lays out the first 8 categories for the Dojo Program Catalogue, each following the schema defined in Issue #50. Every program entry lists the catalogue fields our schema requires (id, label, description, default flag, Fedora `dnf` package names, and `reason_why`). Advanced alternatives are explicitly gated when they should only be surfaced to expert users.

## 1. Web Browser
### Default: Firefox ESR
- `id`: `firefox-esr`
- `label`: Firefox ESR
- `description`: "Reliable, privacy-respecting browser with upstream ESR cadence." 
- `default`: true
- `package_names`: `dnf: ["firefox"]`
- `reason_why`: "Ships with security fixes on Fedora ARM and keeps telemetry minimal, so it is safe for press-enter-and-go installations."

### Alternatives
1. **Chromium**
   - `id`: `chromium`
   - `label`: Chromium
   - `description`: "Chromium upstream without Google branding; updates frequently." 
   - `default`: false
   - `package_names`: `dnf: ["chromium"]`
   - `reason_why`: "Good Chrome familiarity for advanced users; still open-source and widely packaged."
2. **Brave**
   - `id`: `brave-browser`
   - `label`: Brave Browser
   - `description`: "Chromium fork with integrated ad/track blocking and rewards system." 
   - `default`: false
   - `package_names`: `dnf: ["brave-browser"]`
   - `reason_why`: "Blocks ads out of the box, but ships proprietary components so it is an opt-in alternative."
3. **Ungoogled Chromium** (expert mode)
   - `id`: `ungoogled-chromium`
   - `label`: Ungoogled Chromium
   - `description`: "Chromium rebuilt to strip Google services and telemetry." 
   - `default`: false
   - `package_names`: `dnf: ["ungoogled-chromium"]`
   - `reason_why`: "Maximum privacy if users understand manual updates; gated behind expert mode."

## 2. Terminal
### Default: GNOME Terminal
- `id`: `gnome-terminal`
- `label`: GNOME Terminal
- `description`: "Standard terminal emulator that integrates with Dojo’s default desktop." 
- `default`: true
- `package_names`: `dnf: ["gnome-terminal"]`
- `reason_why`: "Friendly UI, profiles, and works with the GTK session without extra configuration."

### Alternatives
1. **Kitty**
   - `id`: `kitty`
   - `label`: Kitty
   - `description`: "GPU-accelerated terminal with layout support that stays responsive." 
   - `default`: false
   - `package_names`: `dnf: ["kitty"]`
   - `reason_why`: "Great for power users who need tiling panes and modern rendering."
2. **Alacritty**
   - `id`: `alacritty`
   - `label`: Alacritty
   - `description`: "Minimal terminal emulator with GPU rendering and a config file." 
   - `default`: false
   - `package_names`: `dnf: ["alacritty"]`
   - `reason_why`: "Fast, keyboard-friendly, but requires editing `alacritty.yml`."
3. **Konsole**
   - `id`: `konsole`
   - `label`: Konsole
   - `description`: "KDE terminal with session management and tabs." 
   - `default`: false
   - `package_names`: `dnf: ["konsole"]`
   - `reason_why`: "Feature-rich for users coming from KDE, although it pulls KDE dependencies."

## 3. Text Editor
### Default: Nano
- `id`: `nano`
- `label`: GNU *nano*
- `description`: "Easy-to-learn terminal editor for quick file edits." 
- `default`: true
- `package_names`: `dnf: ["nano"]`
- `reason_why`: "Every distro ships nano and it avoids the modal learning curve."

### Alternatives
1. **Neovim**
   - `id`: `neovim`
   - `label`: Neovim
   - `description`: "Modern Vim fork with Lua config for power users." 
   - `default`: false
   - `package_names`: `dnf: ["neovim"]`
   - `reason_why`: "Responsive and extensible, but requires familiarity with Vim."
2. **Helix**
   - `id`: `helix`
   - `label`: Helix
   - `description`: "Modal editor with syntax-aware selection and native Rust implementation." 
   - `default`: false
   - `package_names`: `dnf: ["helix"]`
   - `reason_why`: "Developed in Rust, targets Vim users yet remains opinionated about keybindings."
3. **Micro**
   - `id`: `micro`
   - `label`: Micro
   - `description`: "Terminal editor with sane defaults and mouse support." 
   - `default`: false
   - `package_names`: `dnf: ["micro"]`
   - `reason_why`: "Tiny learning curve, but lacks advanced scripting features."

## 4. Office Suite
### Default: LibreOffice
- `id`: `libreoffice`
- `label`: LibreOffice
- `description`: "Full office suite with writer, calc, and impress components." 
- `default`: true
- `package_names`: `dnf: ["libreoffice", "libreoffice-fresh"]`
- `reason_why`: "Comprehensive and upstream-supported on Fedora, covering common needs."

### Alternatives
1. **Calligra Suite**
   - `id`: `calligra-suite`
   - `label`: Calligra Suite
   - `description`: "KDE-native alternative focused on documents and flowcharts." 
   - `default`: false
   - `package_names`: `dnf: ["calligra"]`
   - `reason_why`: "Lighter than LibreOffice but has a smaller feature set."
2. **OnlyOffice Desktop Editors**
   - `id`: `onlyoffice-desktopeditors`
   - `label`: OnlyOffice
   - `description`: "Cloud-friendly suite with strong Microsoft compatibility." 
   - `default`: false
   - `package_names`: `dnf: ["onlyoffice-desktopeditors"]`
   - `reason_why`: "Best for editing Microsoft formats but includes proprietary components."
3. **AbiWord + Gnumeric combo**
   - `id`: `abiword-gnumeric`
   - `label`: AbiWord & Gnumeric
   - `description`: "Lightweight word processor and spreadsheet pair." 
   - `default`: false
   - `package_names`: `dnf: ["abiword", "gnumeric"]`
   - `reason_why`: "Minimal footprint for offline quick edits, but manually stitched together."

## 5. Media Player
### Default: MPV
- `id`: `mpv`
- `label`: MPV
- `description`: "Command-line and GUI media player that handles most codecs." 
- `default`: true
- `package_names`: `dnf: ["mpv"]`
- `reason_why`: "Lightweight, scriptable, and stays up to date on Fedora."

### Alternatives
1. **VLC**
   - `id`: `vlc`
   - `label`: VLC Media Player
   - `description`: "GUI player with wide codec support and streaming tools." 
   - `default`: false
   - `package_names`: `dnf: ["vlc"]`
   - `reason_why`: "Industrial-grade player for unfamiliar formats but heavier."
2. **GNOME MPV**
   - `id`: `gnome-mpv`
   - `label`: GNOME MPV
   - `description`: "GTK frontend for MPV with simplified controls." 
   - `default`: false
   - `package_names`: `dnf: ["gnome-mpv"]`
   - `reason_why`: "Bridges MPV power with a friendly UI for GNOME users."
3. **Audacious**
   - `id`: `audacious`
   - `label`: Audacious
   - `description`: "Lightweight audio player with Winamp-style playlist management." 
   - `default`: false
   - `package_names`: `dnf: ["audacious"]`
   - `reason_why`: "Excellent for audio-only use-cases in minimal desktops."

## 6. Communication
### Default: Thunderbird
- `id`: `thunderbird`
- `label`: Thunderbird
- `description`: "Full-featured email client with calendaring." 
- `default`: true
- `package_names`: `dnf: ["thunderbird"]`
- `reason_why`: "Handles POP/IMAP securely and is the standard for Fedora Workstation."

### Alternatives
1. **Geary**
   - `id`: `geary`
   - `label`: Geary
   - `description`: "Simple, lightweight email client for GNOME." 
   - `default`: false
   - `package_names`: `dnf: ["geary"]`
   - `reason_why`: "Great for quick checks but lacks offline/IMAP account depth."
2. **Element**
   - `id`: `element-desktop`
   - `label`: Element
   - `description`: "Matrix-first chat client with end-to-end encryption." 
   - `default`: false
   - `package_names`: `dnf: ["element-desktop"]`
   - `reason_why`: "Modern with bridged rooms, but requires federation knowledge; expert-mode gated." 
3. **Signal Desktop**
   - `id`: `signal-desktop`
   - `label`: Signal Desktop
   - `description`: "Secure messenger client backed by the Signal protocol." 
   - `default`: false
   - `package_names`: `dnf: ["signal-desktop"]`
   - `reason_why`: "Best for encrypted SMS-like conversations but depends on the Signal ecosystem."

## 7. File Manager
### Default: Nautilus
- `id`: `nautilus`
- `label`: Files (Nautilus)
- `description`: "GNOME file manager with search and quick actions." 
- `default`: true
- `package_names`: `dnf: ["nautilus"]`
- `reason_why`: "Integrated with the default desktop and has intuitive gestures."

### Alternatives
1. **Thunar**
   - `id`: `thunar`
   - `label`: Thunar
   - `description`: "Lightweight XFCE file manager with simple navigation." 
   - `default`: false
   - `package_names`: `dnf: ["thunar"]`
   - `reason_why`: "Fast and straighforward but lacks heavy integration features."
2. **Dolphin**
   - `id`: `dolphin`
   - `label`: Dolphin
   - `description`: "KDE file manager with tabs, split view, and service menus." 
   - `default`: false
   - `package_names`: `dnf: ["dolphin"]`
   - `reason_why`: "Feature-rich but pulls KDE frameworks, so optional for GNOME users."
3. **Ranger** (expert mode)
   - `id`: `ranger`
   - `label`: Ranger
   - `description`: "TUI file navigator inspired by Vim." 
   - `default`: false
   - `package_names`: `dnf: ["ranger"]`
   - `reason_why`: "Keyboard-focused alternative for power users; gated due to text-only interface." 

## 8. System Monitoring & Maintenance
### Default: GNOME System Monitor
- `id`: `gnome-system-monitor`
- `label`: GNOME System Monitor
- `description`: "Graphical utility for processes, resources, and system load." 
- `default`: true
- `package_names`: `dnf: ["gnome-system-monitor"]`
- `reason_why`: "Friendly overview helps users understand resource usage on Fedora."

### Alternatives
1. **htop**
   - `id`: `htop`
   - `label`: htop
   - `description`: "Terminal-based process viewer with sortable columns." 
   - `default`: false
   - `package_names`: `dnf: ["htop"]`
   - `reason_why`: "Widely known by sysadmins and scriptable via command line."
2. **bpytop**
   - `id`: `bpytop`
   - `label`: bpytop
   - `description`: "Python-based resource monitor with beautiful ASCII graphs." 
   - `default`: false
   - `package_names`: `dnf: ["bpytop"]`
   - `reason_why`: "Stylish but heavier; sits between GUI and CLI tools."
3. **GNOME Disks**
   - `id`: `gnome-disk-utility`
   - `label`: GNOME Disks
   - `description`: "Utility for inspecting storage devices and SMART data." 
   - `default`: false
   - `package_names`: `dnf: ["gnome-disk-utility"]`
   - `reason_why`: "Essential for imaging or verifying storage health, especially on Raspberry Pi."
