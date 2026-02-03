//! Parsing helpers for `/proc/meminfo`.

/// Extracts the most relevant memory-available value from `/proc/meminfo`.
///
/// Returns the value in KiB.
pub fn parse_mem_available_kb(content: &str) -> Option<u64> {
    // Preferred on modern kernels.
    for line in content.lines() {
        if let Some(value) = line.strip_prefix("MemAvailable:") {
            return value
                .split_whitespace()
                .next()
                .and_then(|num| num.parse().ok());
        }
    }
    // Fallback for minimal / older formats.
    for line in content.lines() {
        if let Some(value) = line.strip_prefix("MemTotal:") {
            return value
                .split_whitespace()
                .next()
                .and_then(|num| num.parse().ok());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_mem_available_prefers_available() {
        let data = "MemTotal: 16384000 kB\nMemAvailable: 8000000 kB\n";
        assert_eq!(parse_mem_available_kb(data), Some(8000000));
    }

    #[test]
    fn parse_mem_available_uses_total_when_available_missing() {
        let data = "MemTotal: 16384000 kB\n";
        assert_eq!(parse_mem_available_kb(data), Some(16384000));
    }
}
