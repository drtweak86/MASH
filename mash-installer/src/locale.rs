//! Offline locale patching for the installed system
//!
//! Sets user-selected locale and keymap before first boot by modifying:
//! - /etc/locale.conf (LANG setting)
//! - /etc/vconsole.conf (console keymap)
//! - /etc/default/keyboard (X11 keyboard layout)

use crate::errors::Result;
use anyhow::bail; // Add anyhow for bail! macro
use std::fs;
use std::path::Path;

/// Locale configuration
#[derive(Debug, Clone)]
pub struct LocaleConfig {
    /// Language setting (e.g., "en_GB.UTF-8")
    pub lang: &'static str,
    /// Console keymap (e.g., "gb")
    pub keymap: &'static str,
    /// X11 keyboard layout (e.g., "gb")
    pub x11_layout: &'static str,
}

impl LocaleConfig {
    pub fn parse_from_str(s: &str) -> Result<Self> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 2 {
            bail!("Invalid locale format: expected 'lang:keymap', got '{}'", s);
        }
        let lang = parts[0];
        let keymap = parts[1];

        // Find matching locale in LOCALES
        LOCALES
            .iter()
            .find(|lc| lc.lang == lang && lc.keymap == keymap)
            .cloned() // Clone to get an owned LocaleConfig
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Unsupported locale: '{}'. Available: {:?}",
                    s,
                    LOCALES
                        .iter()
                        .map(|lc| format!("{}:{}", lc.lang, lc.keymap))
                        .collect::<Vec<_>>()
                )
            })
    }
}

/// Available locales
pub static LOCALES: &[LocaleConfig] = &[
    LocaleConfig {
        lang: "en_GB.UTF-8",
        keymap: "gb",
        x11_layout: "gb",
    },
    LocaleConfig {
        lang: "en_US.UTF-8",
        keymap: "us",
        x11_layout: "us",
    },
    LocaleConfig {
        lang: "de_DE.UTF-8",
        keymap: "de",
        x11_layout: "de",
    },
    LocaleConfig {
        lang: "fr_FR.UTF-8",
        keymap: "fr",
        x11_layout: "fr",
    },
    LocaleConfig {
        lang: "es_ES.UTF-8",
        keymap: "es",
        x11_layout: "es",
    },
];

/// Apply locale configuration to a mounted root filesystem
pub fn patch_locale(root_mount: &Path, locale: &LocaleConfig, dry_run: bool) -> Result<()> {
    log::info!(
        "Configuring locale: {} (keymap: {})",
        locale.lang,
        locale.keymap
    );

    // 1. Write /etc/locale.conf
    let locale_conf = root_mount.join("etc/locale.conf");
    let locale_content = format!("LANG={}\n", locale.lang);
    if dry_run {
        log::info!(
            "(dry-run) would write to {}: {}",
            locale_conf.display(),
            locale_content.trim()
        );
    } else {
        // Ensure parent directory exists
        if let Some(parent) = locale_conf.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&locale_conf, &locale_content)?;
        log::info!("Wrote {}", locale_conf.display());
    }

    // 2. Update /etc/vconsole.conf (preserve other settings if present)
    let vconsole_conf = root_mount.join("etc/vconsole.conf");
    let vconsole_content = update_vconsole(&vconsole_conf, locale)?;
    if dry_run {
        log::info!(
            "(dry-run) would write to {}: {}",
            vconsole_conf.display(),
            vconsole_content.trim()
        );
    } else {
        fs::write(&vconsole_conf, &vconsole_content)?;
        log::info!("Wrote {}", vconsole_conf.display());
    }

    // 3. Patch /etc/default/keyboard for X11
    let keyboard_conf = root_mount.join("etc/default/keyboard");
    let keyboard_content = update_keyboard(&keyboard_conf, locale)?;
    if dry_run {
        log::info!(
            "(dry-run) would write to {}: {}",
            keyboard_conf.display(),
            keyboard_content.trim()
        );
    } else {
        // Ensure parent directory exists
        if let Some(parent) = keyboard_conf.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&keyboard_conf, &keyboard_content)?;
        log::info!("Wrote {}", keyboard_conf.display());
    }

    Ok(())
}

/// Update vconsole.conf, preserving other settings
fn update_vconsole(path: &Path, locale: &LocaleConfig) -> Result<String> {
    let mut keymap_set = false;
    let mut lines: Vec<String> = Vec::new();

    // Read existing content if present
    if path.exists() {
        let content = fs::read_to_string(path)?;
        for line in content.lines() {
            if line.starts_with("KEYMAP=") {
                lines.push(format!("KEYMAP={}", locale.keymap));
                keymap_set = true;
            } else {
                lines.push(line.to_string());
            }
        }
    }

    // Add KEYMAP if not already present
    if !keymap_set {
        lines.push(format!("KEYMAP={}", locale.keymap));
    }

    Ok(lines.join("\n") + "\n")
}

/// Update /etc/default/keyboard for X11
fn update_keyboard(path: &Path, locale: &LocaleConfig) -> Result<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut xkblayout_set = false;
    let mut xkbmodel_set = false;

    // Read existing content if present
    if path.exists() {
        let content = fs::read_to_string(path)?;
        for line in content.lines() {
            if line.starts_with("XKBLAYOUT=") {
                lines.push(format!("XKBLAYOUT=\"{}\"", locale.x11_layout));
                xkblayout_set = true;
            } else if line.starts_with("XKBMODEL=") {
                lines.push(line.to_string());
                xkbmodel_set = true;
            } else {
                lines.push(line.to_string());
            }
        }
    }

    // Add required fields if not present
    if !xkbmodel_set {
        lines.insert(0, "XKBMODEL=\"pc105\"".to_string());
    }
    if !xkblayout_set {
        lines.push(format!("XKBLAYOUT=\"{}\"", locale.x11_layout));
    }

    // Add standard options if this is a new file
    if !path.exists() {
        lines.push("XKBVARIANT=\"\"".to_string());
        lines.push("XKBOPTIONS=\"\"".to_string());
        lines.push("BACKSPACE=\"guess\"".to_string());
    }

    Ok(lines.join("\n") + "\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_locales_defined() {
        assert!(LOCALES.len() >= 5);
        assert_eq!(LOCALES[0].lang, "en_GB.UTF-8");
        assert_eq!(LOCALES[0].keymap, "gb");
    }

    #[test]
    fn test_patch_locale_dry_run() {
        let dir = tempdir().unwrap();
        let locale = &LOCALES[0];
        patch_locale(dir.path(), locale, true).unwrap();
        // In dry-run, no files should be created
        assert!(!dir.path().join("etc/locale.conf").exists());
    }

    #[test]
    fn test_patch_locale_real() {
        let dir = tempdir().unwrap();
        let locale = &LOCALES[0];

        // Create etc directory
        fs::create_dir_all(dir.path().join("etc")).unwrap();

        patch_locale(dir.path(), locale, false).unwrap();

        // Check locale.conf
        let locale_content = fs::read_to_string(dir.path().join("etc/locale.conf")).unwrap();
        assert!(locale_content.contains("LANG=en_GB.UTF-8"));

        // Check vconsole.conf
        let vconsole_content = fs::read_to_string(dir.path().join("etc/vconsole.conf")).unwrap();
        assert!(vconsole_content.contains("KEYMAP=gb"));
    }

    #[test]
    fn test_update_vconsole_preserve() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("vconsole.conf");

        // Create existing file with extra settings
        fs::write(&path, "KEYMAP=us\nFONT=latarcyrheb-sun16\n").unwrap();

        let locale = LocaleConfig {
            lang: "en_GB.UTF-8",
            keymap: "gb",
            x11_layout: "gb",
        };

        let result = update_vconsole(&path, &locale).unwrap();

        assert!(result.contains("KEYMAP=gb"));
        assert!(result.contains("FONT=latarcyrheb-sun16"));
        // Should not have duplicate KEYMAP
        assert_eq!(result.matches("KEYMAP=").count(), 1);
    }
}
