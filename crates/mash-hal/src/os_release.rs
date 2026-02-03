//! Parsing helpers for `/etc/os-release`.

use anyhow::Result;

fn parse_os_id_fallback(content: &str) -> Option<String> {
    content.lines().find_map(|line| {
        if let Some(value) = line.strip_prefix("ID=") {
            return Some(value.trim().trim_matches('"').to_lowercase());
        }
        if let Some(value) = line.strip_prefix("NAME=") {
            return Some(value.trim().trim_matches('"').to_lowercase());
        }
        None
    })
}

/// Parses `os-release` content and returns `(id, version_id)`.
///
/// `id` is lowercased. `version_id` is parsed as a number when available.
pub fn parse_os_release(content: &str) -> Result<(String, Option<u32>)> {
    let mut id: Option<String> = None;
    let mut version_id: Option<u32> = None;

    for line in content.lines() {
        if let Some(value) = line.strip_prefix("ID=") {
            id = Some(value.trim().trim_matches('"').to_lowercase());
        } else if let Some(value) = line.strip_prefix("VERSION_ID=") {
            let raw = value.trim().trim_matches('"');
            version_id = raw.parse::<u32>().ok();
        }
    }

    let id = id
        .or_else(|| parse_os_id_fallback(content))
        .unwrap_or_else(|| "unknown".to_string());
    Ok((id, version_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_os_id_handles_name() {
        let release = "NAME=\"Fedora Linux\"\nID=fedora\n";
        assert_eq!(
            parse_os_id_fallback(release),
            Some("fedora linux".to_string())
        );
    }

    #[test]
    fn parse_os_release_extracts_version_id() {
        let release = "NAME=\"Fedora Linux\"\nID=fedora\nVERSION_ID=\"43\"\n";
        let (id, version) = parse_os_release(release).unwrap();
        assert_eq!(id, "fedora");
        assert_eq!(version, Some(43));
    }
}
