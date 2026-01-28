use anyhow::{anyhow, Context, Result};
use std::fs::{self, File};
use std::io::copy;
use std::path::{Path, PathBuf};
use log::{info, debug};
use serde::Deserialize;
use zip::ZipArchive;
use std::process::Command;

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

pub fn download_uefi_firmware(destination_dir: &Path) -> Result<()> {
    info!("Starting UEFI firmware download...");
    fs::create_dir_all(destination_dir)
        .with_context(|| format!("Failed to create destination directory: {}", destination_dir.display()))?;

    // Step 1: Get latest release info from GitHub API
    let github_api_url = "https://api.github.com/repos/pftf-rpm-sbsa/RPi4-UEFI-Firmware/releases/latest";
    info!("Fetching latest UEFI firmware release info from: {}", github_api_url);

    let client = reqwest::blocking::Client::new();
    let response: GithubRelease = client
        .get(github_api_url)
        .header(reqwest::header::USER_AGENT, "mash-installer") // GitHub API requires User-Agent
        .send()?
        .json()
        .context("Failed to parse GitHub API response for UEFI firmware releases")?;

    // Step 2: Find the zip asset (e.g., RPi4-UEFI-Firmware-vX.Y.zip)
    let asset = response.assets.iter().find(|a| {
        a.name.starts_with("RPi4-UEFI-Firmware-") && a.name.ends_with(".zip")
    }).ok_or_else(|| anyhow!("Could not find RPi4-UEFI-Firmware-vX.Y.zip asset in latest release"))?;

    let download_url = &asset.browser_download_url;
    info!("Found UEFI firmware zip: {} at {}", asset.name, download_url);

    // Step 3: Download the zip file to a temporary location
    let temp_zip_path = destination_dir.join("uefi_firmware.zip");
    info!("Downloading UEFI firmware to {}", temp_zip_path.display());
    let mut response_stream = client.get(download_url).send()?;
    let mut temp_zip_file = File::create(&temp_zip_path)?;
    copy(&mut response_stream, &mut temp_zip_file)?;
    info!("Download complete.");

    // Step 4: Unzip the file to the destination_dir
    info!("Unzipping firmware to {}", destination_dir.display());
    let file = File::open(&temp_zip_path)?;
    let mut archive = ZipArchive::new(file)?;

    for i in 0..archive.len() {
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
            copy(&mut file, &mut outfile)?;
        }
    }
    fs::remove_file(&temp_zip_path)?; // Clean up the downloaded zip
    info!("UEFI firmware download and extraction complete to {}", destination_dir.display());
    Ok(())
}

pub fn download_fedora_image(
    destination_dir: &Path,
    version: &str,
    edition: &str,
) -> Result<PathBuf> {
    info!("Starting Fedora image download...");
    fs::create_dir_all(destination_dir)
        .with_context(|| format!("Failed to create destination directory: {}", destination_dir.display()))?;

    let arch = "aarch64";
    let filename = format!("Fedora-{}-Desktop-{}.{}.raw.xz", edition, version, arch);
    let base_url = "https://download.fedoraproject.org/pub/fedora/linux/releases";
    let download_url = format!("{}/{}/{}/{}/images/{}", base_url, version, edition, arch, filename);

    let dest_path = destination_dir.join(&filename);
    info!("Downloading Fedora image from: {}", download_url);
    info!("Saving to: {}", dest_path.display());

    let client = reqwest::blocking::Client::new();
    let mut response_stream = client
        .get(&download_url)
        .send()?
        .error_for_status() // Automatically handle HTTP errors
        .context(format!("Failed to download Fedora image from {}", download_url))?;

    let mut dest_file = File::create(&dest_path)?;
    copy(&mut response_stream, &mut dest_file)?;

    info!("Fedora image download complete to {}", dest_path.display());

    // Decompress .raw.xz to .raw (required for losetup)
    let raw_path = destination_dir.join(format!("Fedora-{}-Desktop-{}.{}.raw", edition, version, arch));
    if raw_path.exists() {
        debug!("Raw image already exists, skipping decompression: {}", raw_path.display());
        return Ok(raw_path);
    }

    info!("Decompressing image (unxz)...");
    let status = Command::new("unxz")
        .args(["-T0", "-f", dest_path.to_str().unwrap()])
        .status()
        .context("Failed to run unxz (install xz/unxz)")?;
    if !status.success() {
        return Err(anyhow!("unxz failed with status: {}", status));
    }

    info!("Decompressed Fedora image to {}", raw_path.display());
    Ok(raw_path)
}