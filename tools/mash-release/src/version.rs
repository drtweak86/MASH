use semver::Version;

pub fn parse_semver_tag(input: &str) -> Result<Version, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("invalid SemVer: empty string".to_string());
    }
    let normalized = trimmed.strip_prefix('v').unwrap_or(trimmed);
    if normalized.contains("..") {
        return Err("invalid SemVer: empty segment".to_string());
    }
    Version::parse(normalized).map_err(|err| format!("invalid SemVer: {}", err))
}

pub fn bump_patch(version: &Version) -> Version {
    let mut next = version.clone();
    next.patch = next.patch.saturating_add(1);
    next
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_semver_tag_accepts_plain() {
        let parsed = parse_semver_tag("1.2.3").unwrap();
        assert_eq!(parsed.major, 1);
        assert_eq!(parsed.minor, 2);
        assert_eq!(parsed.patch, 3);
    }

    #[test]
    fn parse_semver_tag_accepts_v_prefix() {
        let parsed = parse_semver_tag("v2.0.4").unwrap();
        assert_eq!(parsed.major, 2);
        assert_eq!(parsed.minor, 0);
        assert_eq!(parsed.patch, 4);
    }

    #[test]
    fn parse_semver_tag_rejects_double_dots() {
        let err = parse_semver_tag("1.2..1").unwrap_err();
        assert!(err.contains("empty segment"));
    }

    #[test]
    fn parse_semver_tag_rejects_incomplete() {
        let err = parse_semver_tag("1.2").unwrap_err();
        assert!(err.contains("invalid SemVer"));
    }
}
