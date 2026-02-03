use anyhow::{anyhow, Context, Result};
use log::{debug, info};
use serde::Deserialize;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use zip::ZipArchive;

// GitHub API types for deserialization
#[derive(Debug, Deserialize)]
struct GithubRelease {
    assets: Vec<GithubReleaseAsset>,
}

#[derive(Debug, Deserialize)]
struct GithubReleaseAsset {
    name: String,
    browser_download_url: String,
}

/// Format bytes into human readable string (e.g., "1.5 GB")
fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Download helper used by CLI/TUI flows.
///
/// IMPORTANT: Do not print to stdout/stderr here. The TUI depends on a clean terminal.
fn download_with_progress(
    client: &reqwest::blocking::Client,
    url: &str,
    dest_file: &mut File,
    description: &str,
) -> Result<u64> {
    let response = client
        .get(url)
        .send()?
        .error_for_status()
        .context(format!("Failed to download from {}", url))?;

    let total_size = response.content_length();
    let mut reader = response;
    let mut downloaded: u64 = 0;
    let mut buffer = [0u8; 8192];
    if let Some(total) = total_size {
        info!(
            "📥 Downloading {} ({}): {}",
            description,
            format_bytes(total),
            url
        );
    } else {
        info!("📥 Downloading {}: {}", description, url);
    }

    loop {
        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }

        dest_file.write_all(&buffer[..bytes_read])?;
        downloaded += bytes_read as u64;
    }

    debug!("✅ Download complete: {}", format_bytes(downloaded));

    Ok(downloaded)
}

#[derive(Debug, Clone, Copy)]
pub struct DownloadProgress {
    pub downloaded: u64,
    pub total: Option<u64>,
    pub speed_bytes_per_sec: u64,
}

fn download_with_progress_cb(
    client: &reqwest::blocking::Client,
    url: &str,
    dest_file: &mut File,
    progress: &mut dyn FnMut(DownloadProgress) -> bool,
    cancel_flag: Option<&AtomicBool>,
) -> Result<u64> {
    let response = client
        .get(url)
        .send()?
        .error_for_status()
        .context(format!("Failed to download from {}", url))?;

    let total_size = response.content_length();
    let mut reader = response;
    let mut downloaded: u64 = 0;
    let mut buffer = [0u8; 8192];
    let start_time = Instant::now();

    loop {
        if cancel_flag
            .map(|flag| flag.load(std::sync::atomic::Ordering::Relaxed))
            .unwrap_or(false)
        {
            return Err(anyhow!("cancelled"));
        }
        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }

        dest_file.write_all(&buffer[..bytes_read])?;
        downloaded += bytes_read as u64;

        let elapsed = start_time.elapsed().as_secs_f64();
        let speed = if elapsed > 0.0 {
            downloaded as f64 / elapsed
        } else {
            0.0
        };
        let keep_going = progress(DownloadProgress {
            downloaded,
            total: total_size,
            speed_bytes_per_sec: speed as u64,
        });
        if !keep_going {
            return Err(anyhow!("cancelled"));
        }
    }

    Ok(downloaded)
}

pub fn download_uefi_firmware(destination_dir: &Path) -> Result<()> {
    info!("Starting UEFI firmware download...");
    info!("🔧 Downloading UEFI Firmware for Raspberry Pi 4");

    fs::create_dir_all(destination_dir).with_context(|| {
        format!(
            "Failed to create destination directory: {}",
            destination_dir.display()
        )
    })?;

    // Step 1: Get latest release info from GitHub API
    let github_api_url = "https://api.github.com/repos/pftf/RPi4/releases/latest";
    info!(
        "Fetching latest UEFI firmware release info from: {}",
        github_api_url
    );
    info!("Checking GitHub for latest release...");

    let client = reqwest::blocking::Client::builder()
        .user_agent("mash-installer")
        .build()?;

    let response: GithubRelease = client
        .get(github_api_url)
        .send()?
        .json()
        .context("Failed to parse GitHub API response for UEFI firmware releases")?;

    // Step 2: Find the zip asset (e.g., RPi4_UEFI_Firmware_vX.Y.zip)
    let asset = response
        .assets
        .iter()
        .find(|a| a.name.starts_with("RPi4_UEFI_Firmware_") && a.name.ends_with(".zip"))
        .ok_or_else(|| {
            anyhow!("Could not find RPi4_UEFI_Firmware_vX.Y.zip asset in latest release")
        })?;

    let download_url = &asset.browser_download_url;
    info!(
        "Found UEFI firmware zip: {} at {}",
        asset.name, download_url
    );
    info!("Found asset: {}", asset.name);

    // Step 3: Download the zip file to a temporary location
    let temp_zip_path = destination_dir.join("uefi_firmware.zip");
    info!("Downloading UEFI firmware to {}", temp_zip_path.display());

    let mut temp_zip_file = File::create(&temp_zip_path)?;
    download_with_progress(&client, download_url, &mut temp_zip_file, "UEFI Firmware")?;

    // Step 4: Unzip the file to the destination_dir
    info!("Unzipping firmware to {}", destination_dir.display());
    let file = File::open(&temp_zip_path)?;
    let mut archive = ZipArchive::new(file)?;

    let total_files = archive.len();
    for i in 0..total_files {
        let mut file = archive.by_index(i)?;
        let outpath = match file.enclosed_name() {
            Some(path) => destination_dir.join(path),
            None => continue,
        };

        if (*file.name()).ends_with('/') {
            debug!("Creating directory {}", outpath.display());
            fs::create_dir_all(&outpath)?;
        } else {
            debug!("Extracting file {}", outpath.display());
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    fs::create_dir_all(p)?;
                }
            }
            let mut outfile = File::create(&outpath)?;
            io::copy(&mut file, &mut outfile)?;
        }
    }
    fs::remove_file(&temp_zip_path)?; // Clean up the downloaded zip
    info!("✅ Extracted {} files", total_files);

    info!(
        "UEFI firmware download and extraction complete to {}",
        destination_dir.display()
    );
    Ok(())
}

pub fn download_uefi_firmware_with_progress(
    destination_dir: &Path,
    progress: &mut dyn FnMut(DownloadProgress) -> bool,
    stage: &mut dyn FnMut(&str),
    cancel_flag: Option<&AtomicBool>,
) -> Result<PathBuf> {
    info!("Starting UEFI firmware download...");
    fs::create_dir_all(destination_dir).with_context(|| {
        format!(
            "Failed to create destination directory: {}",
            destination_dir.display()
        )
    })?;

    let github_api_url = "https://api.github.com/repos/pftf/RPi4/releases/latest";
    let client = reqwest::blocking::Client::builder()
        .user_agent("mash-installer")
        .build()?;

    let response: GithubRelease = client
        .get(github_api_url)
        .send()?
        .json()
        .context("Failed to parse GitHub API response for UEFI firmware releases")?;

    let asset = response
        .assets
        .iter()
        .find(|a| a.name.starts_with("RPi4_UEFI_Firmware_") && a.name.ends_with(".zip"))
        .ok_or_else(|| anyhow!("Could not find RPi4_UEFI_Firmware_vX.Y.zip asset"))?;

    let download_url = &asset.browser_download_url;
    let temp_zip_path = destination_dir.join("uefi_firmware.zip");

    let mut temp_zip_file = File::create(&temp_zip_path)?;
    download_with_progress_cb(
        &client,
        download_url,
        &mut temp_zip_file,
        progress,
        cancel_flag,
    )?;

    stage("Extracting UEFI bundle...");
    let file = File::open(&temp_zip_path)?;
    let mut archive = ZipArchive::new(file)?;

    let total_files = archive.len();
    for i in 0..total_files {
        if cancel_flag
            .map(|flag| flag.load(std::sync::atomic::Ordering::Relaxed))
            .unwrap_or(false)
        {
            return Err(anyhow!("cancelled"));
        }
        let mut file = archive.by_index(i)?;
        let outpath = match file.enclosed_name() {
            Some(path) => destination_dir.join(path),
            None => continue,
        };

        if (*file.name()).ends_with('/') {
            fs::create_dir_all(&outpath)?;
        } else {
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    fs::create_dir_all(p)?;
                }
            }
            let mut outfile = File::create(&outpath)?;
            io::copy(&mut file, &mut outfile)?;
        }
    }
    fs::remove_file(&temp_zip_path)?;
    Ok(destination_dir.to_path_buf())
}

pub fn download_fedora_image(
    destination_dir: &Path,
    version: &str,
    edition: &str,
) -> Result<PathBuf> {
    info!("Starting Fedora image download...");
    info!(
        "🐧 Downloading Fedora {} {} for Raspberry Pi 4",
        version, edition
    );

    fs::create_dir_all(destination_dir).with_context(|| {
        format!(
            "Failed to create destination directory: {}",
            destination_dir.display()
        )
    })?;

    let arch = "aarch64";
    let filename = format!("Fedora-{}-{}-{}.raw.xz", edition, version, arch);

    // Try multiple URL patterns (Fedora changes these occasionally)
    let url_patterns = [
        // Standard releases path
        format!(
            "https://download.fedoraproject.org/pub/fedora/linux/releases/{}/Spins/{}/images/{}",
            version, arch, filename
        ),
        // Alternative: direct mirror
        format!(
            "https://mirrors.fedoraproject.org/mirrorlist?repo=fedora-{}&arch={}",
            version, arch
        ),
    ];

    let dest_path = destination_dir.join(&filename);

    // Check if already downloaded
    if dest_path.exists() {
        info!("Fedora image already downloaded: {}", dest_path.display());
    } else {
        info!("Downloading Fedora image: {}", filename);
        info!("File: {}", filename);
        info!("Large download (~2-3 GB) - please be patient.");

        let client = reqwest::blocking::Client::builder()
            .user_agent("mash-installer")
            .timeout(std::time::Duration::from_secs(3600)) // 1 hour timeout for large file
            .build()?;

        let mut last_error = None;
        let mut success = false;

        for url in &url_patterns {
            info!("Trying URL: {}", url);

            match File::create(&dest_path) {
                Ok(mut dest_file) => {
                    match download_with_progress(&client, url, &mut dest_file, "Fedora Image") {
                        Ok(_) => {
                            success = true;
                            break;
                        }
                        Err(e) => {
                            info!("URL failed, trying next mirror...");
                            last_error = Some(e);
                            let _ = fs::remove_file(&dest_path); // Clean up partial download
                        }
                    }
                }
                Err(e) => {
                    last_error = Some(e.into());
                }
            }
        }

        if !success {
            return Err(last_error.unwrap_or_else(|| anyhow!("All download URLs failed")));
        }
    }

    info!("Fedora image download complete to {}", dest_path.display());

    // Decompress .raw.xz to .raw (required for losetup)
    let raw_path = destination_dir.join(format!("Fedora-{}-{}-{}.raw", edition, version, arch));

    if raw_path.exists() {
        debug!(
            "Raw image already exists, skipping decompression: {}",
            raw_path.display()
        );
        return Ok(raw_path);
    }

    info!("Decompressing image (unxz)...");

    let status = Command::new("unxz")
        .args(["-T0", "-fkv", dest_path.to_str().unwrap()])
        .status()
        .context("Failed to run unxz (install xz-utils if missing)")?;

    if !status.success() {
        return Err(anyhow!("unxz failed with status: {}", status));
    }

    info!("Decompressed Fedora image to {}", raw_path.display());
    Ok(raw_path)
}

pub fn download_fedora_image_with_progress(
    destination_dir: &Path,
    version: &str,
    edition: &str,
    progress: &mut dyn FnMut(DownloadProgress) -> bool,
    stage: &mut dyn FnMut(&str),
    cancel_flag: Option<&AtomicBool>,
) -> Result<PathBuf> {
    fs::create_dir_all(destination_dir).with_context(|| {
        format!(
            "Failed to create destination directory: {}",
            destination_dir.display()
        )
    })?;

    let arch = "aarch64";
    let filename = format!("Fedora-{}-{}-{}.raw.xz", edition, version, arch);

    let url_patterns = [
        format!(
            "https://download.fedoraproject.org/pub/fedora/linux/releases/{}/Spins/{}/images/{}",
            version, arch, filename
        ),
        format!(
            "https://mirrors.fedoraproject.org/mirrorlist?repo=fedora-{}&arch={}",
            version, arch
        ),
    ];

    let dest_path = destination_dir.join(&filename);

    if !dest_path.exists() {
        let client = reqwest::blocking::Client::builder()
            .user_agent("mash-installer")
            .timeout(std::time::Duration::from_secs(3600))
            .build()?;

        let mut last_error = None;
        let mut success = false;

        for url in &url_patterns {
            match File::create(&dest_path) {
                Ok(mut dest_file) => {
                    match download_with_progress_cb(
                        &client,
                        url,
                        &mut dest_file,
                        progress,
                        cancel_flag,
                    ) {
                        Ok(_) => {
                            success = true;
                            break;
                        }
                        Err(e) => {
                            last_error = Some(e);
                            let _ = fs::remove_file(&dest_path);
                        }
                    }
                }
                Err(e) => {
                    last_error = Some(e.into());
                }
            }
        }

        if !success {
            return Err(last_error.unwrap_or_else(|| anyhow!("All download URLs failed")));
        }
    }

    let raw_path = destination_dir.join(format!("Fedora-{}-{}-{}.raw", edition, version, arch));
    if raw_path.exists() {
        return Ok(raw_path);
    }

    stage("Extracting Fedora image...");
    let mut child = Command::new("unxz")
        .args(["-T0", "-fkv", dest_path.to_str().unwrap()])
        .spawn()
        .context("Failed to run unxz (install xz-utils if missing)")?;

    loop {
        if cancel_flag
            .map(|flag| flag.load(std::sync::atomic::Ordering::Relaxed))
            .unwrap_or(false)
        {
            let _ = child.kill();
            return Err(anyhow!("cancelled"));
        }
        if let Some(status) = child.try_wait()? {
            if !status.success() {
                return Err(anyhow!("unxz failed with status: {}", status));
            }
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    Ok(raw_path)
}
