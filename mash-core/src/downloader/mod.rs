use anyhow::{Context, Result};
use log::info;
use once_cell::sync::Lazy;
use reqwest::blocking::Client;
use reqwest::StatusCode;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek};
use std::path::{Path, PathBuf};
use std::thread::sleep;
use std::time::Duration;

#[derive(Debug, Clone, Deserialize)]
pub struct ImageSpec {
    pub version: String,
    pub edition: String,
    pub arch: String,
    pub checksum_sha256: String,
    pub mirrors: Vec<String>,
}

pub static DOWNLOAD_INDEX: Lazy<Vec<ImageSpec>> = Lazy::new(|| {
    let index = include_str!("../../../docs/os-download-links.toml");
    toml::from_str(index).unwrap_or_default()
});

#[derive(Debug, Clone)]
pub struct DownloadOptions {
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
    if let Some(ref override_url) = opts.mirror_override {
        let checksum = opts
            .checksum_override
            .clone()
            .context("override checksum required for download override")?;
        let spec = ImageSpec {
            version: "override".to_string(),
            edition: "override".to_string(),
            arch: "aarch64".to_string(),
            checksum_sha256: checksum,
            mirrors: vec![override_url.clone()],
        };
        return Ok(spec);
    }
    DOWNLOAD_INDEX
        .last()
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("no download specs found"))
}

fn download_checksum(opts: &DownloadOptions, client: &Client, spec: &ImageSpec) -> Result<String> {
    if let Some(ref checksum) = opts.checksum_override {
        return Ok(checksum.clone());
    }
    if let Some(ref url) = opts.checksum_url {
        let response = client
            .get(url)
            .send()
            .context("Failed to fetch checksum from URL")?
            .error_for_status()?;
        let checksum_text = response.text()?;
        return Ok(checksum_text.trim().to_string());
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
    let spec = pick_source(opts)?;
    let client = create_http_client(opts.timeout_secs)?;
    let checksum = download_checksum(opts, &client, &spec)?;
    fs::create_dir_all(&opts.download_dir)?;
    let filename = format!(
        "Fedora-{}-{}-{}.raw.xz",
        spec.edition, spec.version, spec.arch
    );
    let target = opts.download_dir.join(&filename);
    let mut last_err = None;
    let mut resumed = false;
    for attempt in 1..=opts.max_retries.max(1) {
        info!("Download attempt {}/{}", attempt, opts.max_retries.max(1));
        let result = if opts.resume && target.exists() {
            match download_with_resume(&client, &spec.mirrors, &target, attempt) {
                Ok(size) => {
                    resumed = true;
                    Ok(size)
                }
                Err(_) => {
                    let _ = fs::remove_file(&target);
                    download_full(&client, &spec.mirrors, &target, attempt)
                }
            }
        } else {
            download_full(&client, &spec.mirrors, &target, attempt)
        };
        match result {
            Ok(_) => {
                let metadata = target.metadata().context("missing download artifact")?;
                let artifact = DownloadArtifact::new(
                    filename.clone(),
                    target.clone(),
                    checksum.clone(),
                    metadata.len(),
                    resumed,
                );
                artifact.verify_checksum()?;
                return Ok(artifact);
            }
            Err(err) => {
                last_err = Some(err);
                if attempt < opts.max_retries.max(1) {
                    let backoff = Duration::from_secs(1 << attempt.min(5));
                    sleep(backoff);
                }
            }
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("download failed")))
}

fn download_full(
    client: &Client,
    mirrors: &[String],
    target: &Path,
    attempt: usize,
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
                let mut dest = File::create(target)?;
                io::copy(&mut response, &mut dest)?;
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
                    io::copy(&mut response, &mut dest)?;
                    return Ok(dest.stream_position()?);
                } else if status.is_success() {
                    let mut dest = File::create(target)?;
                    io::copy(&mut response, &mut dest)?;
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
        let partial = opts
            .download_dir
            .join("Fedora-override-override-aarch64.raw.xz");
        write(&partial, &body[..5]).unwrap();
        let artifact = download(&opts).unwrap();
        assert!(artifact.resumed);
        assert_eq!(artifact.size, body.len() as u64);
    }
}
