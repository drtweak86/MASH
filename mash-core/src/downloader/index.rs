pub use super::types::DownloadIndex;
use anyhow::Context;
use once_cell::sync::Lazy;

pub static DOWNLOAD_INDEX: Lazy<anyhow::Result<DownloadIndex>> = Lazy::new(|| {
    let index = include_str!("../../../docs/os-download-links.toml");
    parse_index(index).context("failed to parse docs/os-download-links.toml (download index)")
});

pub fn download_index() -> anyhow::Result<&'static DownloadIndex> {
    DOWNLOAD_INDEX
        .as_ref()
        .map_err(|err| anyhow::anyhow!("{:#}", err))
}

pub fn parse_index(toml_text: &str) -> anyhow::Result<DownloadIndex> {
    toml::from_str(toml_text).context("failed to parse download index TOML")
}
