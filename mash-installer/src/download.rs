use anyhow::{anyhow, Context, Result};
use log::{debug, info};
use serde::Deserialize;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::Sender;
use std::time::Instant;
use zip::ZipArchive;

use crate::tui::DownloadUpdate;

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

/// Download with progress indication to terminal
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
    let start_time = Instant::now();
    let mut last_print = Instant::now();

    // Print header
    if let Some(total) = total_size {
        eprintln!("\n📥 {} ({}):", description, format_bytes(total));
    } else {
        eprintln!("\n📥 {}:", description);
    }

    loop {
        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }

        dest_file.write_all(&buffer[..bytes_read])?;
        downloaded += bytes_read as u64;

        // Update progress every 250ms to avoid flooding terminal
        if last_print.elapsed().as_millis() >= 250 {
            let elapsed = start_time.elapsed().as_secs_f64();
            let speed = if elapsed > 0.0 {
                downloaded as f64 / elapsed
            } else {
                0.0
            };

            if let Some(total) = total_size {
                let percent = (downloaded as f64 / total as f64) * 100.0;
                let eta = if speed > 0.0 {
                    let remaining = total - downloaded;
                    remaining as f64 / speed
                } else {
                    0.0
                };

                // Progress bar
                let bar_width = 30;
                let filled = (percent / 100.0 * bar_width as f64) as usize;
                let empty = bar_width - filled;
                let bar: String = "█".repeat(filled) + &"░".repeat(empty);

                eprint!(
                    "\r   [{}] {:>5.1}% | {} / {} | {}/s | ETA: {}s   ",
                    bar,
                    percent,
                    format_bytes(downloaded),
                    format_bytes(total),
                    format_bytes(speed as u64),
                    eta as u64
                );
            } else {
                eprint!(
                    "\r   Downloaded: {} | {}/s   ",
                    format_bytes(downloaded),
                    format_bytes(speed as u64)
                );
            }
            io::stderr().flush().ok();
            last_print = Instant::now();
        }
    }

    // Final line
    let elapsed = start_time.elapsed().as_secs_f64();
    let avg_speed = if elapsed > 0.0 {
        downloaded as f64 / elapsed
    } else {
        0.0
    };
    eprintln!(
        "\r   ✅ Complete: {} downloaded in {:.1}s ({}/s avg)                    ",
        format_bytes(downloaded),
        elapsed,
        format_bytes(avg_speed as u64)
    );

    Ok(downloaded)
}

pub fn download_uefi_firmware(destination_dir: &Path) -> Result<()> {
    info!("Starting UEFI firmware download...");
    eprintln!("\n🔧 Downloading UEFI Firmware for Raspberry Pi 4...");

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
    eprintln!("   Checking GitHub for latest release...");

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
    eprintln!("   Found: {}", asset.name);

    // Step 3: Download the zip file to a temporary location
    let temp_zip_path = destination_dir.join("uefi_firmware.zip");
    info!("Downloading UEFI firmware to {}", temp_zip_path.display());

    let mut temp_zip_file = File::create(&temp_zip_path)?;
    download_with_progress(&client, download_url, &mut temp_zip_file, "UEFI Firmware")?;

    // Step 4: Unzip the file to the destination_dir
    eprintln!("\n📦 Extracting UEFI firmware...");
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
    eprintln!("   ✅ Extracted {} files", total_files);

    info!(
        "UEFI firmware download and extraction complete to {}",
        destination_dir.display()
    );
    Ok(())
}

pub fn download_fedora_image(
    destination_dir: &Path,
    version: &str,
    edition: &str,
) -> Result<PathBuf> {
    info!("Starting Fedora image download...");
    eprintln!(
        "\n🐧 Downloading Fedora {} {} for Raspberry Pi 4...",
        version, edition
    );

    fs::create_dir_all(destination_dir).with_context(|| {
        format!(
            "Failed to create destination directory: {}",
            destination_dir.display()
        )
    })?;

    let arch = "aarch64";
    // Fedora ARM spin filename format: Fedora-{Edition}-Disk-{version}-{patch}.{arch}.raw.xz
    let (spin_name, category) = match edition {
        "KDE" => ("KDE-Mobile-Disk", "Spins"),
        "Xfce" => ("Xfce-Disk", "Spins"),
        "LXQt" => ("LXQt-Disk", "Spins"),
        "Minimal" => ("Minimal", "Spins"),
        "Server" => ("Server-Host-Generic", "Server"),
        _ => ("Minimal", "Spins"),
    };

    let client = reqwest::blocking::Client::builder()
        .user_agent("mash-installer")
        .timeout(std::time::Duration::from_secs(3600))
        .build()?;

    // Try common patch versions
    let patch_versions = ["1.6", "1.5", "1.4", "1.3", "1.2", "1.1"];
    let mut found_url = None;
    let mut found_filename = String::new();

    eprintln!(
        "   🔍 Searching for Fedora {} {} image...",
        version, edition
    );

    for patch in &patch_versions {
        let filename = format!("Fedora-{}-{}-{}.{}.raw.xz", spin_name, version, patch, arch);
        let url = format!(
            "https://download.fedoraproject.org/pub/fedora/linux/releases/{}/{}/{}/images/{}",
            version, category, arch, filename
        );

        if let Ok(resp) = client.head(&url).send() {
            if resp.status().is_success() || resp.status().is_redirection() {
                found_url = Some(url);
                found_filename = filename;
                eprintln!("   ✅ Found: {}", found_filename);
                break;
            }
        }
    }

    let (url, filename) = match found_url {
        Some(u) => (u, found_filename),
        None => {
            return Err(anyhow!(
                "Could not find Fedora {} {} image for aarch64. \
                 The image may not be available for this version/edition.",
                version,
                edition
            ));
        }
    };

    let dest_path = destination_dir.join(&filename);

    // Check if already downloaded
    if dest_path.exists() {
        eprintln!("   ℹ️  Compressed image already exists, skipping download");
        info!("Fedora image already downloaded: {}", dest_path.display());
    } else {
        info!("Downloading Fedora image: {}", filename);
        eprintln!("   File: {}", filename);
        eprintln!("   ⚠️  This is a large download (~2-3 GB). Please be patient.\n");

        let mut dest_file = File::create(&dest_path)?;
        download_with_progress(&client, &url, &mut dest_file, "Fedora Image")?;
    }

    info!("Fedora image download complete to {}", dest_path.display());

    // Decompress .raw.xz to .raw (required for losetup)
    let raw_filename = filename.trim_end_matches(".xz");
    let raw_path = destination_dir.join(raw_filename);

    if raw_path.exists() {
        eprintln!("\n   ℹ️  Raw image already exists, skipping decompression");
        debug!(
            "Raw image already exists, skipping decompression: {}",
            raw_path.display()
        );
        return Ok(raw_path);
    }

    eprintln!("\n📦 Decompressing image (this may take a few minutes)...");
    info!("Decompressing image (unxz)...");

    let status = Command::new("unxz")
        .args(["-T0", "-fkv", dest_path.to_str().unwrap()])
        .status()
        .context("Failed to run unxz (install xz-utils if missing)")?;

    if !status.success() {
        return Err(anyhow!("unxz failed with status: {}", status));
    }

    eprintln!("   ✅ Decompression complete");
    info!("Decompressed Fedora image to {}", raw_path.display());
    Ok(raw_path)
}

// ============================================================================
// TUI-integrated download functions with channel-based progress
// ============================================================================

/// Download with progress sent to a channel (for TUI)
fn download_with_channel(
    client: &reqwest::blocking::Client,
    url: &str,
    dest_file: &mut File,
    description: &str,
    tx: &Sender<DownloadUpdate>,
) -> Result<u64> {
    let response = client
        .get(url)
        .send()?
        .error_for_status()
        .context(format!("Failed to download from {}", url))?;

    let total_size = response.content_length();

    // Send start notification
    let _ = tx.send(DownloadUpdate::Started {
        description: description.to_string(),
        total_bytes: total_size,
    });

    let mut reader = response;
    let mut downloaded: u64 = 0;
    let mut buffer = [0u8; 8192];
    let start_time = Instant::now();
    let mut last_update = Instant::now();

    loop {
        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }

        dest_file.write_all(&buffer[..bytes_read])?;
        downloaded += bytes_read as u64;

        // Send progress updates every 100ms
        if last_update.elapsed().as_millis() >= 100 {
            let elapsed = start_time.elapsed().as_secs_f64();
            let speed = if elapsed > 0.0 {
                (downloaded as f64 / elapsed) as u64
            } else {
                0
            };

            let eta = if let Some(total) = total_size {
                if speed > 0 {
                    (total - downloaded) / speed
                } else {
                    0
                }
            } else {
                0
            };

            let _ = tx.send(DownloadUpdate::Progress {
                current_bytes: downloaded,
                speed,
                eta,
            });

            last_update = Instant::now();
        }
    }

    Ok(downloaded)
}

/// Download UEFI firmware with TUI progress updates
pub fn download_uefi_firmware_with_progress(
    destination_dir: &Path,
    tx: Sender<DownloadUpdate>,
) -> Result<PathBuf> {
    info!("Starting UEFI firmware download (TUI mode)...");

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
        .ok_or_else(|| {
            anyhow!("Could not find RPi4_UEFI_Firmware_vX.Y.zip asset in latest release")
        })?;

    let download_url = &asset.browser_download_url;
    let temp_zip_path = destination_dir.join("uefi_firmware.zip");

    let mut temp_zip_file = File::create(&temp_zip_path)?;
    download_with_channel(
        &client,
        download_url,
        &mut temp_zip_file,
        &format!("UEFI Firmware ({})", asset.name),
        &tx,
    )?;

    // Send extracting status
    let _ = tx.send(DownloadUpdate::Extracting);

    let file = File::open(&temp_zip_path)?;
    let mut archive = ZipArchive::new(file)?;

    for i in 0..archive.len() {
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

    let _ = tx.send(DownloadUpdate::Complete);
    info!(
        "UEFI firmware download complete to {}",
        destination_dir.display()
    );

    Ok(destination_dir.to_path_buf())
}

/// Download Fedora image with TUI progress updates
pub fn download_fedora_image_with_progress(
    destination_dir: &Path,
    version: &str,
    edition: &str,
    tx: Sender<DownloadUpdate>,
) -> Result<PathBuf> {
    info!("Starting Fedora image download (TUI mode)...");

    fs::create_dir_all(destination_dir).with_context(|| {
        format!(
            "Failed to create destination directory: {}",
            destination_dir.display()
        )
    })?;

    let arch = "aarch64";
    // Fedora ARM spin filename format: Fedora-{Edition}-Disk-{version}-1.6.{arch}.raw.xz
    // Edition mapping for ARM spins
    let (spin_name, category) = match edition {
        "KDE" => ("KDE-Mobile-Disk", "Spins"),
        "Xfce" => ("Xfce-Disk", "Spins"),
        "LXQt" => ("LXQt-Disk", "Spins"),
        "Minimal" => ("Minimal", "Spins"),
        "Server" => ("Server-Host-Generic", "Server"),
        _ => ("Minimal", "Spins"), // Fallback to Minimal
    };

    // Try common patch versions (1.6, 1.5, 1.4, etc.)
    let patch_versions = ["1.6", "1.5", "1.4", "1.3", "1.2", "1.1"];
    let mut found_url = None;
    let mut found_filename = String::new();

    let client = reqwest::blocking::Client::builder()
        .user_agent("mash-installer")
        .timeout(std::time::Duration::from_secs(3600))
        .build()?;

    for patch in &patch_versions {
        let filename = format!("Fedora-{}-{}-{}.{}.raw.xz", spin_name, version, patch, arch);
        let url = format!(
            "https://download.fedoraproject.org/pub/fedora/linux/releases/{}/{}/{}/images/{}",
            version, category, arch, filename
        );

        // Check if URL exists with HEAD request
        if let Ok(resp) = client.head(&url).send() {
            if resp.status().is_success() || resp.status().is_redirection() {
                found_url = Some(url);
                found_filename = filename;
                break;
            }
        }
    }

    let (url, filename) = match found_url {
        Some(u) => (u, found_filename),
        None => {
            let _ = tx.send(DownloadUpdate::Error(format!(
                "Could not find Fedora {} {} image for aarch64",
                version, edition
            )));
            return Err(anyhow!(
                "Could not find Fedora {} {} image for aarch64",
                version,
                edition
            ));
        }
    };

    let dest_path = destination_dir.join(&filename);

    if !dest_path.exists() {
        let mut dest_file = File::create(&dest_path)?;
        download_with_channel(
            &client,
            &url,
            &mut dest_file,
            &format!("Fedora {} {} (aarch64)", version, edition),
            &tx,
        )?;
    }

    // Decompress - the raw file has the same base name without .xz
    let raw_filename = filename.trim_end_matches(".xz");
    let raw_path = destination_dir.join(raw_filename);

    if !raw_path.exists() {
        let _ = tx.send(DownloadUpdate::Extracting);

        let status = Command::new("unxz")
            .args(["-T0", "-fkv", dest_path.to_str().unwrap()])
            .status()
            .context("Failed to run unxz")?;

        if !status.success() {
            let _ = tx.send(DownloadUpdate::Error(
                "unxz decompression failed".to_string(),
            ));
            return Err(anyhow!("unxz failed"));
        }
    }

    let _ = tx.send(DownloadUpdate::Complete);
    info!("Fedora image download complete to {}", raw_path.display());

    Ok(raw_path)
}
