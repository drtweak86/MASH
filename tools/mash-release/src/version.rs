use semver::Version;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BumpKind {
    Major,
    Minor,
    Patch,
}

pub fn parse_strict_version(input: &str) -> Result<Version, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("invalid SemVer: empty string".to_string());
    }
    if trimmed.contains("..") {
        return Err("invalid SemVer: empty segment".to_string());
    }
    let normalized = trimmed.strip_prefix('v').unwrap_or(trimmed);
    let parsed = Version::parse(normalized).map_err(|err| format!("invalid SemVer: {}", err))?;
    if !parsed.pre.is_empty() || !parsed.build.is_empty() {
        return Err("invalid SemVer: pre-release/build not allowed".to_string());
    }
    Ok(parsed)
}

pub fn bump_version(version: &Version, kind: BumpKind) -> Version {
    let mut next = version.clone();
    match kind {
        BumpKind::Major => {
            next.major = next.major.saturating_add(1);
            next.minor = 0;
            next.patch = 0;
        }
        BumpKind::Minor => {
            next.minor = next.minor.saturating_add(1);
            next.patch = 0;
        }
        BumpKind::Patch => {
            next.patch = next.patch.saturating_add(1);
        }
    }
    next
}

pub fn format_tag(version: &Version) -> String {
    format!("v{}.{}.{}", version.major, version.minor, version.patch)
}

impl std::fmt::Display for BumpKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BumpKind::Major => write!(f, "major"),
            BumpKind::Minor => write!(f, "minor"),
            BumpKind::Patch => write!(f, "patch"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_strict_version_accepts_plain() {
        let parsed = parse_strict_version("1.2.3").unwrap();
        assert_eq!(parsed.major, 1);
        assert_eq!(parsed.minor, 2);
        assert_eq!(parsed.patch, 3);
    }

    #[test]
    fn parse_strict_version_accepts_v_prefix() {
        let parsed = parse_strict_version("v2.0.4").unwrap();
        assert_eq!(parsed.major, 2);
        assert_eq!(parsed.minor, 0);
        assert_eq!(parsed.patch, 4);
    }

    #[test]
    fn parse_strict_version_rejects_double_dots() {
        let err = parse_strict_version("1.2..1").unwrap_err();
        assert!(err.contains("empty segment"));
    }

    #[test]
    fn parse_strict_version_rejects_incomplete() {
        let err = parse_strict_version("1.2").unwrap_err();
        assert!(err.contains("invalid SemVer"));
    }

    #[test]
    fn parse_strict_version_rejects_prerelease() {
        let err = parse_strict_version("1.2.3-alpha").unwrap_err();
        assert!(err.contains("pre-release"));
    }

    #[test]
    fn parse_strict_version_rejects_bad_prerelease() {
        let err = parse_strict_version("1.2.3-safety-codex..20260130.1514").unwrap_err();
        assert!(err.contains("empty segment"));
    }

    #[test]
    fn parse_strict_version_rejects_build_metadata() {
        let err = parse_strict_version("1.2.3+build.1").unwrap_err();
        assert!(err.contains("pre-release"));
    }

    #[test]
    fn bump_version_patch() {
        let parsed = parse_strict_version("1.2.3").unwrap();
        let bumped = bump_version(&parsed, BumpKind::Patch);
        assert_eq!(bumped.to_string(), "1.2.4");
    }

    #[test]
    fn bump_version_minor() {
        let parsed = parse_strict_version("1.2.3").unwrap();
        let bumped = bump_version(&parsed, BumpKind::Minor);
        assert_eq!(bumped.to_string(), "1.3.0");
    }

    #[test]
    fn bump_version_major() {
        let parsed = parse_strict_version("1.2.3").unwrap();
        let bumped = bump_version(&parsed, BumpKind::Major);
        assert_eq!(bumped.to_string(), "2.0.0");
    }

    #[test]
    fn format_tag_uses_v_prefix() {
        let parsed = parse_strict_version("2.3.4").unwrap();
        assert_eq!(format_tag(&parsed), "v2.3.4");
    }
}
