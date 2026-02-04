use serde::Deserialize;

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum OsKind {
    Fedora,
    Ubuntu,
    #[serde(rename = "raspberry_pi_os")]
    RaspberryPiOS,
    Manjaro,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    Zip,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HealthCheckSpec {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssetSpec {
    pub name: String,
    pub kind: AssetKind,
    pub file_name: String,
    pub checksum_sha256: String,
    #[serde(default)]
    pub checksum_url: Option<String>,
    pub mirrors: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ImageSpec {
    pub os: OsKind,
    pub variant: String,
    pub arch: String,
    pub file_name: String,
    pub checksum_sha256: String,
    #[serde(default)]
    pub checksum_url: Option<String>,
    pub mirrors: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DownloadIndex {
    #[serde(default)]
    pub images: Vec<ImageSpec>,

    // Present for the scheduled health-check action (issue #45).
    #[serde(default)]
    pub health_checks: Vec<HealthCheckSpec>,

    #[serde(default)]
    pub assets: Vec<AssetSpec>,
}
