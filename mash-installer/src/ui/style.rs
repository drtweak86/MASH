//! Centralized emoji + message helpers for consistent UX copy.

pub mod emoji {
    pub const ACTION: &str = "🧩";
    pub const CLEANUP: &str = "🧹";
    pub const DOWNLOAD: &str = "⬇️";
    pub const ERROR: &str = "❌";
    pub const CANCEL: &str = "🛑";
    pub const DISK: &str = "💾";
    pub const PARTY: &str = "🎉";
    pub const SEARCH: &str = "🔍";
    pub const SUCCESS: &str = "✅";
}

pub fn with(emoji: &str, message: &str) -> String {
    format!("{} {}", emoji, message)
}
