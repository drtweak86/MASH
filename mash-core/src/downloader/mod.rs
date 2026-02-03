use anyhow::{Context, Result};
use log::info;
use once_cell::sync::Lazy;
use reqwest::blocking::Client;
use reqwest::StatusCode;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::thread::sleep;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum OsKind {
    Fedora,
    Ubuntu,
    #[serde(rename = "raspberry_pi_os")]
    RaspberryPiOS,
    Manjaro,
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

#[derive(Debug, Clone, Deserialize)]
pub struct HealthCheckSpec {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    Zip,
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

pub static DOWNLOAD_INDEX: Lazy<Result<DownloadIndex>> = Lazy::new(|| {
    let index = include_str!("../../../docs/os-download-links.toml");
    parse_index(index).context("failed to parse docs/os-download-links.toml (download index)")
});

pub fn download_index() -> Result<&'static DownloadIndex> {
    DOWNLOAD_INDEX
        .as_ref()
        .map_err(|err| anyhow::anyhow!("{:#}", err))
}

pub fn parse_index(toml_text: &str) -> Result<DownloadIndex> {
    toml::from_str(toml_text).context("failed to parse download index TOML")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageKey {
    pub os: OsKind,
    pub variant: String,
    pub arch: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetKey {
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct DownloadOptions {
    pub asset: Option<AssetKey>,
    pub image: Option<ImageKey>,
    pub mirror_override: Option<String>,
    pub checksum_override: Option<String>,
    pub checksum_url: Option<String>,
    pub max_retries: usize,
    pub timeout_secs: u64,
    pub download_dir: PathBuf,
    pub resume: bool,
}

impl Default for DownloadOptions {
    fn default() -> Self {
        Self {
            asset: None,
            image: None,
            mirror_override: None,
            checksum_override: None,
            checksum_url: None,
            max_retries: 3,
            timeout_secs: 120,
            download_dir: PathBuf::from("downloads/images"),
            resume: true,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DownloadProgress {
    pub downloaded: u64,
    pub total: Option<u64>,
    pub speed_bytes_per_sec: u64,
}

#[derive(Debug, Clone)]
pub struct DownloadArtifact {
    pub name: String,
    pub path: PathBuf,
    pub checksum: String,
    pub size: u64,
    pub resumed: bool,
}

impl DownloadArtifact {
    pub fn new(name: String, path: PathBuf, checksum: String, size: u64, resumed: bool) -> Self {
        Self {
            name,
            path,
            checksum,
            size,
            resumed,
        }
    }

    pub fn verify_checksum(&self) -> Result<()> {
        let mut file = File::open(&self.path)?;
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 8192];
        loop {
            let read = file.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }
        let computed = format!("{:x}", hasher.finalize());
        if computed != self.checksum {
            anyhow::bail!("checksum mismatch: {} != {}", computed, self.checksum);
        }
        Ok(())
    }
}

fn pick_source(opts: &DownloadOptions) -> Result<ImageSpec> {
    let index = download_index()?;
    pick_source_from_index(index, opts)
}

fn pick_source_from_index(index: &DownloadIndex, opts: &DownloadOptions) -> Result<ImageSpec> {
    if let Some(ref override_url) = opts.mirror_override {
        let checksum = opts
            .checksum_override
            .clone()
            .context("override checksum required for download override")?;
        let spec = ImageSpec {
            os: OsKind::Fedora,
            variant: "override".to_string(),
            arch: "aarch64".to_string(),
            file_name: "override.img.xz".to_string(),
            checksum_sha256: checksum,
            checksum_url: None,
            mirrors: vec![override_url.clone()],
        };
        return Ok(spec);
    }

    let key = opts.image.as_ref().context(
        "download selection required (set DownloadOptions.image or use mirror_override)",
    )?;

    index
        .images
        .iter()
        .find(|spec| spec.os == key.os && spec.variant == key.variant && spec.arch == key.arch)
        .cloned()
        .with_context(|| {
            format!(
                "no download spec found for os={:?} variant={} arch={}",
                key.os, key.variant, key.arch
            )
        })
}

fn pick_asset_from_index(index: &DownloadIndex, opts: &DownloadOptions) -> Result<AssetSpec> {
    let key = opts
        .asset
        .as_ref()
        .context("download selection required (set DownloadOptions.asset)")?;
    index
        .assets
        .iter()
        .find(|spec| spec.name == key.name)
        .cloned()
        .with_context(|| format!("no asset download spec found for name={}", key.name))
}

fn download_checksum(opts: &DownloadOptions, client: &Client, spec: &ImageSpec) -> Result<String> {
    if let Some(ref checksum) = opts.checksum_override {
        return Ok(checksum.clone());
    }

    let checksum_url = opts
        .checksum_url
        .clone()
        .or_else(|| spec.checksum_url.clone());

    if let Some(url) = checksum_url {
        let response = client
            .get(url)
            .send()
            .context("Failed to fetch checksum from URL")?
            .error_for_status()?;
        let checksum_text = response.text()?;
        if let Some(extracted) = extract_sha256_from_checksum_file(&checksum_text, &spec.file_name)
        {
            return Ok(extracted);
        }
        anyhow::bail!(
            "checksum URL did not contain an entry for {}",
            spec.file_name
        );
    }

    Ok(spec.checksum_sha256.clone())
}

fn create_http_client(timeout_secs: u64) -> Result<Client> {
    Ok(Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .user_agent("mash-installer")
        .build()?)
}

pub fn download(opts: &DownloadOptions) -> Result<DownloadArtifact> {
    let index = download_index()?;
    download_from_index_with_progress(index, opts, None)
}

fn download_from_index(index: &DownloadIndex, opts: &DownloadOptions) -> Result<DownloadArtifact> {
    download_from_index_with_progress(index, opts, None)
}

pub fn download_with_progress(
    opts: &DownloadOptions,
    progress: &mut dyn FnMut(DownloadProgress) -> bool,
) -> Result<DownloadArtifact> {
    let index = download_index()?;
    download_from_index_with_progress(index, opts, Some(progress))
}

fn download_from_index_with_progress(
    index: &DownloadIndex,
    opts: &DownloadOptions,
    mut progress: Option<&mut dyn FnMut(DownloadProgress) -> bool>,
) -> Result<DownloadArtifact> {
    // Images and assets are both downloaded as files; spec selection differs.
    let (file_name, mirrors, checksum, checksum_url) = if opts.asset.is_some() {
        let spec = pick_asset_from_index(index, opts)?;
        (
            spec.file_name,
            spec.mirrors,
            spec.checksum_sha256,
            spec.checksum_url,
        )
    } else {
        let spec = pick_source_from_index(index, opts)?;
        (
            spec.file_name,
            spec.mirrors,
            spec.checksum_sha256,
            spec.checksum_url,
        )
    };

    let client = create_http_client(opts.timeout_secs)?;
    let checksum = {
        if let Some(ref checksum) = opts.checksum_override {
            checksum.clone()
        } else if let Some(url) = opts.checksum_url.clone().or(checksum_url) {
            let response = client
                .get(url)
                .send()
                .context("Failed to fetch checksum from URL")?
                .error_for_status()?;
            let checksum_text = response.text()?;
            if let Some(extracted) = extract_sha256_from_checksum_file(&checksum_text, &file_name) {
                extracted
            } else {
                anyhow::bail!("checksum URL did not contain an entry for {}", file_name);
            }
        } else {
            checksum
        }
    };
    fs::create_dir_all(&opts.download_dir)?;
    let target = opts.download_dir.join(&file_name);
    let mut last_err = None;
    let mut resumed = false;

    for attempt in 1..=opts.max_retries.max(1) {
        info!("Download attempt {}/{}", attempt, opts.max_retries.max(1));

        let result = if opts.resume && target.exists() {
            match download_with_resume(&client, &mirrors, &target, attempt, &mut progress) {
                Ok(size) => {
                    resumed = true;
                    Ok(size)
                }
                Err(_) => {
                    let _ = fs::remove_file(&target);
                    download_full(&client, &mirrors, &target, attempt, &mut progress)
                }
            }
        } else {
            download_full(&client, &mirrors, &target, attempt, &mut progress)
        };

        match result {
            Ok(size) => {
                let artifact = DownloadArtifact::new(
                    file_name.clone(),
                    target.clone(),
                    checksum.clone(),
                    size,
                    resumed,
                );
                match artifact.verify_checksum() {
                    Ok(()) => return Ok(artifact),
                    Err(err) => {
                        // If checksum fails, remove the file so the next attempt cannot
                        // "resume" a known-bad artifact.
                        let _ = fs::remove_file(&target);
                        last_err = Some(err);
                        if attempt < opts.max_retries.max(1) {
                            sleep(Duration::from_secs(2));
                        }
                    }
                }
            }
            Err(err) => {
                last_err = Some(err);
                if attempt < opts.max_retries.max(1) {
                    sleep(Duration::from_secs(2));
                }
            }
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("download failed")))
}

fn extract_sha256_from_checksum_file(text: &str, file_name: &str) -> Option<String> {
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        if !line.contains(file_name) {
            continue;
        }

        // Fedora CHECKSUM format:
        //   SHA256 (filename) = <sha>
        if let Some(rest) = line.strip_prefix("SHA256 (") {
            if let Some((fname_part, sha_part)) = rest.split_once(") = ") {
                if fname_part.trim() == file_name {
                    let sha = sha_part.trim().to_ascii_lowercase();
                    if is_valid_sha256(&sha) {
                        return Some(sha);
                    }
                }
            }
        }

        // Ubuntu/RPi formats:
        //   <sha> *filename
        //   <sha>  filename
        let mut parts = line.split_whitespace();
        let sha = parts.next()?.trim().to_ascii_lowercase();
        if !is_valid_sha256(&sha) {
            continue;
        }

        for token in parts {
            let token = token.trim_start_matches('*');
            if token == file_name {
                return Some(sha);
            }
        }
    }
    None
}

fn is_valid_sha256(s: &str) -> bool {
    s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit())
}

fn download_full(
    client: &Client,
    mirrors: &[String],
    target: &Path,
    attempt: usize,
    progress: &mut Option<&mut dyn FnMut(DownloadProgress) -> bool>,
) -> Result<u64> {
    let mut last_err = None;
    for mirror in mirrors {
        let response = client
            .get(mirror)
            .header("X-MASH-ATTEMPT", attempt.to_string())
            .send();
        match response {
            Ok(mut response) => {
                if !response.status().is_success() {
                    last_err = Some(anyhow::anyhow!("{} returned {}", mirror, response.status()));
                    continue;
                }
                let total = response.content_length();
                let mut dest = File::create(target)?;
                let mut downloaded: u64 = 0;
                let start = Instant::now();
                let mut last_tick = Instant::now();
                let mut buf = [0u8; 8192];
                loop {
                    let read = response.read(&mut buf)?;
                    if read == 0 {
                        break;
                    }
                    dest.write_all(&buf[..read])?;
                    downloaded = downloaded.saturating_add(read as u64);
                    if let Some(cb) = progress.as_deref_mut() {
                        if last_tick.elapsed() >= Duration::from_millis(200) {
                            let elapsed = start.elapsed().as_secs_f64();
                            let speed = if elapsed > 0.0 {
                                (downloaded as f64 / elapsed) as u64
                            } else {
                                0
                            };
                            let keep_going = cb(DownloadProgress {
                                downloaded,
                                total,
                                speed_bytes_per_sec: speed,
                            });
                            if !keep_going {
                                anyhow::bail!("cancelled");
                            }
                            last_tick = Instant::now();
                        }
                    }
                }
                return Ok(dest.stream_position()?);
            }
            Err(err) => {
                last_err = Some(anyhow::anyhow!("{} request failed: {}", mirror, err));
            }
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("all mirrors failed")))
}

fn download_with_resume(
    client: &Client,
    mirrors: &[String],
    target: &Path,
    attempt: usize,
    progress: &mut Option<&mut dyn FnMut(DownloadProgress) -> bool>,
) -> Result<u64> {
    let current_len = target.metadata().map(|m| m.len()).unwrap_or(0);
    if current_len == 0 {
        return Err(anyhow::anyhow!("resume requested but no partial file"));
    }
    for mirror in mirrors {
        let response = client
            .get(mirror)
            .header("X-MASH-ATTEMPT", attempt.to_string())
            .header("Range", format!("bytes={}-", current_len))
            .send();
        match response {
            Ok(mut response) => {
                let status = response.status();
                if status == StatusCode::PARTIAL_CONTENT {
                    let mut dest = OpenOptions::new().append(true).open(target)?;
                    let total = response
                        .content_length()
                        .map(|t| t.saturating_add(current_len));
                    let mut downloaded = current_len;
                    let start = Instant::now();
                    let mut last_tick = Instant::now();
                    let mut buf = [0u8; 8192];
                    loop {
                        let read = response.read(&mut buf)?;
                        if read == 0 {
                            break;
                        }
                        dest.write_all(&buf[..read])?;
                        downloaded = downloaded.saturating_add(read as u64);
                        if let Some(cb) = progress.as_deref_mut() {
                            if last_tick.elapsed() >= Duration::from_millis(200) {
                                let elapsed = start.elapsed().as_secs_f64();
                                let speed = if elapsed > 0.0 {
                                    ((downloaded - current_len) as f64 / elapsed) as u64
                                } else {
                                    0
                                };
                                let keep_going = cb(DownloadProgress {
                                    downloaded,
                                    total,
                                    speed_bytes_per_sec: speed,
                                });
                                if !keep_going {
                                    anyhow::bail!("cancelled");
                                }
                                last_tick = Instant::now();
                            }
                        }
                    }
                    return Ok(dest.stream_position()?);
                } else if status.is_success() {
                    let mut dest = File::create(target)?;
                    let total = response.content_length();
                    let mut downloaded: u64 = 0;
                    let start = Instant::now();
                    let mut last_tick = Instant::now();
                    let mut buf = [0u8; 8192];
                    loop {
                        let read = response.read(&mut buf)?;
                        if read == 0 {
                            break;
                        }
                        dest.write_all(&buf[..read])?;
                        downloaded = downloaded.saturating_add(read as u64);
                        if let Some(cb) = progress.as_deref_mut() {
                            if last_tick.elapsed() >= Duration::from_millis(200) {
                                let elapsed = start.elapsed().as_secs_f64();
                                let speed = if elapsed > 0.0 {
                                    (downloaded as f64 / elapsed) as u64
                                } else {
                                    0
                                };
                                let keep_going = cb(DownloadProgress {
                                    downloaded,
                                    total,
                                    speed_bytes_per_sec: speed,
                                });
                                if !keep_going {
                                    anyhow::bail!("cancelled");
                                }
                                last_tick = Instant::now();
                            }
                        }
                    }
                    return Ok(dest.stream_position()?);
                }
            }
            Err(_) => continue,
        }
    }
    Err(anyhow::anyhow!("resume failed"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::Method::GET;
    use httpmock::MockServer;
    use sha2::Sha256;
    use std::fs::{self, write};
    use tempfile::tempdir;

    fn make_opts(server: &MockServer, checksum: &str) -> DownloadOptions {
        DownloadOptions {
            image: Some(ImageKey {
                os: OsKind::Fedora,
                variant: "override".to_string(),
                arch: "aarch64".to_string(),
            }),
            mirror_override: Some(server.url("/image")),
            checksum_override: Some(checksum.to_string()),
            max_retries: 2,
            timeout_secs: 5,
            download_dir: tempdir().unwrap().path().join("dl"),
            resume: true,
            ..Default::default()
        }
    }

    #[test]
    fn download_success() {
        let server = MockServer::start();
        let body = b"abcdef";
        let checksum = format!("{:x}", Sha256::digest(body));
        server.mock(|when, then| {
            when.method(GET)
                .path("/image")
                .header("x-mash-attempt", "1");
            then.status(200).body(body);
        });
        let opts = make_opts(&server, &checksum);
        let artifact = download(&opts).unwrap();
        assert_eq!(artifact.size, body.len() as u64);
        assert!(!artifact.resumed);
    }

    #[test]
    fn checksum_mismatch_fails() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/image");
            then.status(200).body(b"bad");
        });
        let opts = make_opts(&server, "deadbeef");
        assert!(download(&opts).is_err());
    }

    #[test]
    fn retry_then_success() {
        let server = MockServer::start();
        let body = b"retry";
        let checksum = format!("{:x}", Sha256::digest(body));
        server.mock(|when, then| {
            when.method(GET)
                .path("/image")
                .header("x-mash-attempt", "1");
            then.status(500);
        });
        server.mock(|when, then| {
            when.method(GET)
                .path("/image")
                .header("x-mash-attempt", "2");
            then.status(200).body(body);
        });
        let opts = make_opts(&server, &checksum);
        let artifact = download(&opts).unwrap();
        assert_eq!(artifact.size, body.len() as u64);
    }

    #[test]
    fn resume_appends() {
        let server = MockServer::start();
        let body = b"0123456789";
        let checksum = format!("{:x}", Sha256::digest(body));
        server.mock(|when, then| {
            when.method(GET)
                .path("/image")
                .header("range", "bytes=5-")
                .header("x-mash-attempt", "1");
            then.status(206).body(&body[5..]);
        });
        let opts = make_opts(&server, &checksum);
        fs::create_dir_all(&opts.download_dir).unwrap();
        let partial = opts.download_dir.join("override.img.xz");
        write(&partial, &body[..5]).unwrap();
        let artifact = download(&opts).unwrap();
        assert!(artifact.resumed);
        assert_eq!(artifact.size, body.len() as u64);
    }

    #[test]
    fn checksum_url_parses_fedora_format() {
        let checksum = "a615aa6ea7cba59215ec8e46b21df57494fb51b47b8f89c97b15ae31f8282696";
        let text = format!(
            "SHA256 (Fedora-KDE-Mobile-Disk-43-1.6.aarch64.raw.xz) = {}\n",
            checksum
        );
        let extracted = extract_sha256_from_checksum_file(
            &text,
            "Fedora-KDE-Mobile-Disk-43-1.6.aarch64.raw.xz",
        )
        .unwrap();
        assert_eq!(extracted, checksum);
    }

    #[test]
    fn checksum_url_parses_ubuntu_format() {
        let checksum = "9bb1799cee8965e6df0234c1c879dd35be1d87afe39b84951f278b6bd0433e56";
        let text = format!(
            "{} *ubuntu-24.04.3-preinstalled-server-arm64+raspi.img.xz\n",
            checksum
        );
        let extracted = extract_sha256_from_checksum_file(
            &text,
            "ubuntu-24.04.3-preinstalled-server-arm64+raspi.img.xz",
        )
        .unwrap();
        assert_eq!(extracted, checksum);
    }

    #[test]
    fn checksum_url_parses_rpi_format() {
        let checksum = "f7afb40e587746128538d84f217bf478a23af59484d4db77f2d06bf647f7c82e";
        let text = format!("{}  2025-12-04-raspios-trixie-arm64.img.xz\n", checksum);
        let extracted =
            extract_sha256_from_checksum_file(&text, "2025-12-04-raspios-trixie-arm64.img.xz")
                .unwrap();
        assert_eq!(extracted, checksum);
    }

    fn make_index_spec(
        os: OsKind,
        variant: &str,
        file_name: &str,
        checksum_sha256: &str,
        checksum_url: Option<String>,
        mirrors: Vec<String>,
    ) -> DownloadIndex {
        DownloadIndex {
            images: vec![ImageSpec {
                os,
                variant: variant.to_string(),
                arch: "aarch64".to_string(),
                file_name: file_name.to_string(),
                checksum_sha256: checksum_sha256.to_string(),
                checksum_url,
                mirrors,
            }],
            health_checks: Vec::new(),
            assets: Vec::new(),
        }
    }

    fn opts_for_key(dir: &Path, os: OsKind, variant: &str) -> DownloadOptions {
        DownloadOptions {
            image: Some(ImageKey {
                os,
                variant: variant.to_string(),
                arch: "aarch64".to_string(),
            }),
            download_dir: dir.to_path_buf(),
            max_retries: 1,
            timeout_secs: 5,
            resume: true,
            ..Default::default()
        }
    }

    // Required by issue #84: success + checksum failure + resume, per OS, CI-safe.

    #[test]
    fn fedora_downloader_success_checksum_url_and_resume() {
        let server = MockServer::start();
        let body = b"0123456789";
        let checksum = format!("{:x}", Sha256::digest(body));
        let file_name = "fedora.raw.xz";

        server.mock(|when, then| {
            when.method(GET)
                .path("/image")
                .header("range", "bytes=5-")
                .header("x-mash-attempt", "1");
            then.status(206).body(&body[5..]);
        });
        server.mock(|when, then| {
            when.method(GET).path("/CHECKSUM");
            then.status(200)
                .body(format!("SHA256 ({}) = {}\n", file_name, checksum));
        });

        let tmp = tempdir().unwrap();
        let downloads = tmp.path().join("dl");
        fs::create_dir_all(&downloads).unwrap();
        write(downloads.join(file_name), &body[..5]).unwrap();

        let index = make_index_spec(
            OsKind::Fedora,
            "kde_mobile_disk",
            file_name,
            "ignored",
            Some(server.url("/CHECKSUM")),
            vec![server.url("/image")],
        );
        let opts = opts_for_key(&downloads, OsKind::Fedora, "kde_mobile_disk");
        let artifact = download_from_index(&index, &opts).unwrap();
        assert!(artifact.resumed);
        assert_eq!(artifact.size, body.len() as u64);
    }

    #[test]
    fn ubuntu_downloader_checksum_failure() {
        let server = MockServer::start();
        let body = b"ubuntu";
        let checksum = format!("{:x}", Sha256::digest(body));
        let file_name = "ubuntu.img.xz";

        server.mock(|when, then| {
            when.method(GET)
                .path("/image")
                .header("x-mash-attempt", "1");
            then.status(200).body(body);
        });
        server.mock(|when, then| {
            when.method(GET).path("/SHA256SUMS");
            then.status(200)
                .body(format!("{} *{}\n", checksum, file_name));
        });

        let tmp = tempdir().unwrap();
        let downloads = tmp.path().join("dl");
        let index = make_index_spec(
            OsKind::Ubuntu,
            "server",
            file_name,
            "ignored",
            Some(server.url("/SHA256SUMS")),
            vec![server.url("/image")],
        );
        let mut opts = opts_for_key(&downloads, OsKind::Ubuntu, "server");
        opts.checksum_override = Some("deadbeef".to_string());
        assert!(download_from_index(&index, &opts).is_err());
    }

    #[test]
    fn ubuntu_downloader_success_and_resume() {
        let server = MockServer::start();
        let body = b"0123456789";
        let checksum = format!("{:x}", Sha256::digest(body));
        let file_name = "ubuntu.img.xz";

        server.mock(|when, then| {
            when.method(GET)
                .path("/image")
                .header("range", "bytes=3-")
                .header("x-mash-attempt", "1");
            then.status(206).body(&body[3..]);
        });
        server.mock(|when, then| {
            when.method(GET).path("/SHA256SUMS");
            then.status(200)
                .body(format!("{} *{}\n", checksum, file_name));
        });

        let tmp = tempdir().unwrap();
        let downloads = tmp.path().join("dl");
        fs::create_dir_all(&downloads).unwrap();
        write(downloads.join(file_name), &body[..3]).unwrap();

        let index = make_index_spec(
            OsKind::Ubuntu,
            "server",
            file_name,
            "ignored",
            Some(server.url("/SHA256SUMS")),
            vec![server.url("/image")],
        );
        let opts = opts_for_key(&downloads, OsKind::Ubuntu, "server");
        let artifact = download_from_index(&index, &opts).unwrap();
        assert!(artifact.resumed);
        assert_eq!(artifact.size, body.len() as u64);
    }

    #[test]
    fn rpi_os_downloader_success_and_checksum_failure() {
        let server = MockServer::start();
        let body = b"rpi_os";
        let checksum = format!("{:x}", Sha256::digest(body));
        let file_name = "rpi.img.xz";

        server.mock(|when, then| {
            when.method(GET)
                .path("/image")
                .header("x-mash-attempt", "1");
            then.status(200).body(body);
        });
        server.mock(|when, then| {
            when.method(GET).path("/sha256");
            then.status(200)
                .body(format!("{}  {}\n", checksum, file_name));
        });

        let tmp = tempdir().unwrap();
        let downloads = tmp.path().join("dl");
        let index = make_index_spec(
            OsKind::RaspberryPiOS,
            "arm64_latest",
            file_name,
            "ignored",
            Some(server.url("/sha256")),
            vec![server.url("/image")],
        );

        let opts = opts_for_key(&downloads, OsKind::RaspberryPiOS, "arm64_latest");
        let artifact = download_from_index(&index, &opts).unwrap();
        assert_eq!(artifact.size, body.len() as u64);

        let mut bad = opts_for_key(&downloads, OsKind::RaspberryPiOS, "arm64_latest");
        bad.checksum_override = Some("deadbeef".to_string());
        assert!(download_from_index(&index, &bad).is_err());
    }

    #[test]
    fn rpi_os_downloader_resume() {
        let server = MockServer::start();
        let body = b"0123456789";
        let checksum = format!("{:x}", Sha256::digest(body));
        let file_name = "rpi.img.xz";

        server.mock(|when, then| {
            when.method(GET)
                .path("/image")
                .header("range", "bytes=6-")
                .header("x-mash-attempt", "1");
            then.status(206).body(&body[6..]);
        });
        server.mock(|when, then| {
            when.method(GET).path("/sha256");
            then.status(200)
                .body(format!("{}  {}\n", checksum, file_name));
        });

        let tmp = tempdir().unwrap();
        let downloads = tmp.path().join("dl");
        fs::create_dir_all(&downloads).unwrap();
        write(downloads.join(file_name), &body[..6]).unwrap();

        let index = make_index_spec(
            OsKind::RaspberryPiOS,
            "arm64_latest",
            file_name,
            "ignored",
            Some(server.url("/sha256")),
            vec![server.url("/image")],
        );
        let opts = opts_for_key(&downloads, OsKind::RaspberryPiOS, "arm64_latest");
        let artifact = download_from_index(&index, &opts).unwrap();
        assert!(artifact.resumed);
        assert_eq!(artifact.size, body.len() as u64);
    }

    #[test]
    fn manjaro_downloader_success_checksum_failure_and_resume() {
        let server = MockServer::start();
        let body = b"0123456789";
        let checksum = format!("{:x}", Sha256::digest(body));
        let file_name = "manjaro.img.xz";

        server.mock(|when, then| {
            when.method(GET)
                .path("/image")
                .header("range", "bytes=5-")
                .header("x-mash-attempt", "1");
            then.status(206).body(&body[5..]);
        });

        let tmp = tempdir().unwrap();
        let downloads = tmp.path().join("dl");
        fs::create_dir_all(&downloads).unwrap();
        write(downloads.join(file_name), &body[..5]).unwrap();

        let index = make_index_spec(
            OsKind::Manjaro,
            "minimal_rpi4",
            file_name,
            &checksum,
            None,
            vec![server.url("/image")],
        );
        let opts = opts_for_key(&downloads, OsKind::Manjaro, "minimal_rpi4");
        let artifact = download_from_index(&index, &opts).unwrap();
        assert!(artifact.resumed);

        let mut bad = opts_for_key(&downloads, OsKind::Manjaro, "minimal_rpi4");
        bad.checksum_override = Some("deadbeef".to_string());
        assert!(download_from_index(&index, &bad).is_err());
    }

    // TOML validation tests for WORK ORDER #84

    #[test]
    fn toml_index_contains_ubuntu_entries() {
        let index = download_index().unwrap();
        let ubuntu_server = index
            .images
            .iter()
            .find(|img| img.os == OsKind::Ubuntu && img.variant == "server_24_04_3");
        assert!(
            ubuntu_server.is_some(),
            "Ubuntu Server 24.04.3 entry should exist in TOML"
        );
        let ubuntu_server = ubuntu_server.unwrap();
        assert_eq!(ubuntu_server.arch, "aarch64");
        assert_eq!(
            ubuntu_server.file_name,
            "ubuntu-24.04.3-preinstalled-server-arm64+raspi.img.xz"
        );
        assert_eq!(
            ubuntu_server.checksum_sha256,
            "9bb1799cee8965e6df0234c1c879dd35be1d87afe39b84951f278b6bd0433e56"
        );

        let ubuntu_desktop = index
            .images
            .iter()
            .find(|img| img.os == OsKind::Ubuntu && img.variant == "desktop_24_04_3");
        assert!(
            ubuntu_desktop.is_some(),
            "Ubuntu Desktop 24.04.3 entry should exist in TOML"
        );
        let ubuntu_desktop = ubuntu_desktop.unwrap();
        assert_eq!(
            ubuntu_desktop.checksum_sha256,
            "04a87330d2dfbe29c29f69d2113d92bbde44daa516054074ff4b96c7ee3c528b"
        );
    }

    #[test]
    fn toml_index_contains_raspberry_pi_os_entry() {
        let index = download_index().unwrap();
        let raspios = index
            .images
            .iter()
            .find(|img| img.os == OsKind::RaspberryPiOS && img.variant == "arm64_latest");
        assert!(
            raspios.is_some(),
            "Raspberry Pi OS entry should exist in TOML"
        );
        let raspios = raspios.unwrap();
        assert_eq!(raspios.arch, "aarch64");
        assert_eq!(raspios.file_name, "2025-12-04-raspios-trixie-arm64.img.xz");
        assert_eq!(
            raspios.checksum_sha256,
            "f7afb40e587746128538d84f217bf478a23af59484d4db77f2d06bf647f7c82e"
        );
    }

    #[test]
    fn toml_index_contains_manjaro_entries() {
        let index = download_index().unwrap();

        let manjaro_minimal = index
            .images
            .iter()
            .find(|img| img.os == OsKind::Manjaro && img.variant == "minimal_rpi4_20260126");
        assert!(
            manjaro_minimal.is_some(),
            "Manjaro Minimal 20260126 entry should exist in TOML"
        );
        let manjaro_minimal = manjaro_minimal.unwrap();
        assert_eq!(manjaro_minimal.arch, "aarch64");
        assert_eq!(
            manjaro_minimal.file_name,
            "Manjaro-ARM-minimal-rpi4-20260126.img.xz"
        );
        assert_eq!(
            manjaro_minimal.checksum_sha256,
            "a37bdc5b53e7b0e8ca0b0a3524aade58c58d3d07da226dfe79fcdf0388671ad5"
        );

        let manjaro_kde = index
            .images
            .iter()
            .find(|img| img.os == OsKind::Manjaro && img.variant == "kde_plasma_rpi4_20260126");
        assert!(
            manjaro_kde.is_some(),
            "Manjaro KDE Plasma 20260126 entry should exist in TOML"
        );
        let manjaro_kde = manjaro_kde.unwrap();
        assert_eq!(
            manjaro_kde.checksum_sha256,
            "5080615cd4beabea377f83ffd705300596848410d99a927bfc55bfddfd111412"
        );

        let manjaro_xfce = index
            .images
            .iter()
            .find(|img| img.os == OsKind::Manjaro && img.variant == "xfce_rpi4_20260126");
        assert!(
            manjaro_xfce.is_some(),
            "Manjaro XFCE 20260126 entry should exist in TOML"
        );
        let manjaro_xfce = manjaro_xfce.unwrap();
        assert_eq!(
            manjaro_xfce.checksum_sha256,
            "5e3c46025c825cff1bf9c6331b908e01488b6f934bbc749dfe3184e5f298ac48"
        );
    }

    #[test]
    fn toml_index_all_images_have_valid_checksums() {
        let index = download_index().unwrap();
        for img in &index.images {
            assert_eq!(
                img.checksum_sha256.len(),
                64,
                "Image {} should have valid SHA256 checksum (64 hex chars)",
                img.file_name
            );
            assert!(
                img.checksum_sha256.chars().all(|c| c.is_ascii_hexdigit()),
                "Image {} checksum should be hex digits",
                img.file_name
            );
        }
    }

    #[test]
    fn toml_index_all_images_have_mirrors() {
        let index = download_index().unwrap();
        for img in &index.images {
            assert!(
                !img.mirrors.is_empty(),
                "Image {} should have at least one mirror",
                img.file_name
            );
            for mirror in &img.mirrors {
                assert!(
                    mirror.starts_with("http://") || mirror.starts_with("https://"),
                    "Mirror for {} should be HTTP/HTTPS URL",
                    img.file_name
                );
            }
        }
    }
}
