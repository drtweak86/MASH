//! Small, reusable UI helpers used by multiple screens.

/// Checkbox state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckboxState {
    Unchecked,
    Checked,
}

impl CheckboxState {
    pub fn toggle(&mut self) {
        *self = match self {
            CheckboxState::Unchecked => CheckboxState::Checked,
            CheckboxState::Checked => CheckboxState::Unchecked,
        };
    }

    pub fn is_checked(&self) -> bool {
        matches!(self, CheckboxState::Checked)
    }

    pub fn symbol(&self) -> &'static str {
        match self {
            CheckboxState::Unchecked => "[ ]",
            CheckboxState::Checked => "[x]",
        }
    }
}

impl From<bool> for CheckboxState {
    fn from(b: bool) -> Self {
        if b {
            CheckboxState::Checked
        } else {
            CheckboxState::Unchecked
        }
    }
}

/// Get base disk from partition path.
///
/// Examples:
/// - `/dev/sda1` -> `/dev/sda`
/// - `/dev/nvme0n1p2` -> `/dev/nvme0n1`
/// - `/dev/mmcblk0p2` -> `/dev/mmcblk0`
pub fn base_disk(dev: &str) -> String {
    let mut s = dev.to_string();
    // Handle nvme: /dev/nvme0n1p2 -> /dev/nvme0n1.
    if s.contains("nvme") {
        if let Some(pos) = s.rfind('p') {
            if s[pos + 1..].chars().all(|c| c.is_ascii_digit()) {
                s.truncate(pos);
            }
        }
        return s;
    }
    // Handle mmcblk: /dev/mmcblk0p2 -> /dev/mmcblk0.
    if s.contains("mmcblk") {
        if let Some(pos) = s.rfind('p') {
            if s[pos + 1..].chars().all(|c| c.is_ascii_digit()) {
                s.truncate(pos);
            }
        }
        return s;
    }
    // Handle standard: /dev/sda1 -> /dev/sda.
    while s
        .chars()
        .last()
        .map(|c| c.is_ascii_digit())
        .unwrap_or(false)
    {
        s.pop();
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_disk() {
        assert_eq!(base_disk("/dev/sda1"), "/dev/sda");
        assert_eq!(base_disk("/dev/sda"), "/dev/sda");
        assert_eq!(base_disk("/dev/nvme0n1p2"), "/dev/nvme0n1");
        assert_eq!(base_disk("/dev/nvme0n1"), "/dev/nvme0n1");
        assert_eq!(base_disk("/dev/mmcblk0p2"), "/dev/mmcblk0");
    }

    #[test]
    fn test_checkbox() {
        let mut cb = CheckboxState::Unchecked;
        assert!(!cb.is_checked());
        assert_eq!(cb.symbol(), "[ ]");

        cb.toggle();
        assert!(cb.is_checked());
        assert_eq!(cb.symbol(), "[x]");
    }
}
