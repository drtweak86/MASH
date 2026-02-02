use anyhow::{anyhow, Context, Result};
use log::{debug, info};
use serde::Deserialize;
use std::env;
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

#[derive(Debug, Clone, Copy)]
pub struct DownloadProgress {
    pub downloaded: u64,
    pub total: Option<u64>,
    pub speed_bytes_per_sec: u64,
}

fn github_release_api_url() -> String {
    env::var("MASH_GITHUB_API_URL")
        .unwrap_or_else(|_| "https://api.github.com/repos/pftf/RPi4/releases/latest".into())
}

fn fedora_download_url_patterns(version: &str, arch: &str, filename: &str) -> Vec<String> {
    if let Ok(overrides) = env::var("MASH_FEDORA_DOWNLOAD_URLS") {
        overrides
            .split(';')
            .filter(|entry| !entry.is_empty())
            .map(|entry| entry.to_string())
            .collect()
    } else {
        vec![
            format!(
                "https://download.fedoraproject.org/pub/fedora/linux/releases/{}/Spins/{}/images/{}",
                version, arch, filename
            ),
            format!(
                "https://mirrors.fedoraproject.org/mirrorlist?repo=fedora-{}&arch={}",
                version, arch
            ),
        ]
    }
}

fn unxz_command(path: &Path) -> Command {
    let executable = env::var("MASH_UNXZ_COMMAND").unwrap_or_else(|_| "unxz".into());
    let mut cmd = Command::new(executable);
    cmd.args(["-T0", "-fkv", path.to_str().unwrap_or_default()]);
    cmd
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
    eprintln!("\n🔧 Downloading UEFI Firmware for Raspberry Pi 4...");

    fs::create_dir_all(destination_dir).with_context(|| {
        format!(
            "Failed to create destination directory: {}",
            destination_dir.display()
        )
    })?;

    // Step 1: Get latest release info from GitHub API
    let github_api_url = github_release_api_url();
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

    let github_api_url = github_release_api_url();
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
    let filename = format!("Fedora-{}-{}-{}.raw.xz", edition, version, arch);

    // Try multiple URL patterns (Fedora changes these occasionally)
    let url_patterns = fedora_download_url_patterns(version, arch, &filename);

    let dest_path = destination_dir.join(&filename);

    // Check if already downloaded
    if dest_path.exists() {
        eprintln!("   ℹ️  Compressed image already exists, skipping download");
        info!("Fedora image already downloaded: {}", dest_path.display());
    } else {
        info!("Downloading Fedora image: {}", filename);
        eprintln!("   File: {}", filename);
        eprintln!("   ⚠️  This is a large download (~2-3 GB). Please be patient.\n");

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
                            eprintln!("   ⚠️  URL failed, trying next mirror...");
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
        eprintln!("\n   ℹ️  Raw image already exists, skipping decompression");
        debug!(
            "Raw image already exists, skipping decompression: {}",
            raw_path.display()
        );
        return Ok(raw_path);
    }

    eprintln!("\n📦 Decompressing image (this may take a few minutes)...");
    info!("Decompressing image (unxz)...");

    let status = unxz_command(&dest_path)
        .status()
        .context("Failed to run unxz (install xz-utils if missing)")?;

    if !status.success() {
        return Err(anyhow!("unxz failed with status: {}", status));
    }

    eprintln!("   ✅ Decompression complete");
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
    let mut child = unxz_command(&dest_path)
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashMap;
    use std::env;
    use std::io::{Cursor, ErrorKind, Read, Write};
    use std::net::TcpListener;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    };
    use std::thread;
    use std::time::Duration;
    use tempfile::tempdir;
    use zip::write::{FileOptions, ZipWriter};
    use zip::CompressionMethod;

    type RouteList = Vec<(String, Vec<u8>)>;

    struct EnvGuard {
        key: &'static str,
        previous: Option<String>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let previous = env::var(key).ok();
            env::set_var(key, value);
            EnvGuard { key, previous }
        }

        fn remove(key: &'static str) -> Self {
            let previous = env::var(key).ok();
            env::remove_var(key);
            EnvGuard { key, previous }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            if let Some(value) = self.previous.take() {
                env::set_var(self.key, value);
            } else {
                env::remove_var(self.key);
            }
        }
    }

    fn build_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut buffer = Vec::new();
        {
            let mut writer = ZipWriter::new(Cursor::new(&mut buffer));
            let options = FileOptions::default().compression_method(CompressionMethod::Stored);
            for (name, contents) in entries {
                writer.start_file(*name, options).unwrap();
                writer.write_all(contents).unwrap();
            }
            writer.finish().unwrap();
        }
        buffer
    }

    struct SimpleMockServer {
        address: std::net::SocketAddr,
        hits: Arc<Mutex<HashMap<String, usize>>>,
        shutdown: Arc<AtomicBool>,
        handle: Option<thread::JoinHandle<()>>,
        routes: Arc<Mutex<RouteList>>,
    }

    impl SimpleMockServer {
        fn start(routes: Vec<(&str, Vec<u8>)>) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let address = listener.local_addr().unwrap();
            let routes = Arc::new(Mutex::new(
                routes
                    .into_iter()
                    .map(|(p, b)| (p.to_string(), b))
                    .collect::<RouteList>(),
            ));
            let hits = Arc::new(Mutex::new(HashMap::new()));
            let shutdown = Arc::new(AtomicBool::new(false));
            let cloned_routes = routes.clone();
            let cloned_hits = hits.clone();
            let cloned_shutdown = shutdown.clone();
            let handle = thread::spawn(move || {
                while !cloned_shutdown.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((mut stream, _)) => {
                            let mut buffer = [0u8; 2048];
                            let bytes = stream.read(&mut buffer).unwrap_or(0);
                            if bytes == 0 {
                                continue;
                            }
                            let request = String::from_utf8_lossy(&buffer[..bytes]);
                            let path = request
                                .lines()
                                .next()
                                .and_then(|line| line.split_whitespace().nth(1))
                                .unwrap_or("/");
                            let (body, found): (Vec<u8>, bool) = {
                                let routes = cloned_routes.lock().unwrap();
                                if let Some((_, body)) =
                                    routes.iter().find(|(route_path, _)| route_path == path)
                                {
                                    (body.clone(), true)
                                } else {
                                    (b"Not Found".to_vec(), false)
                                }
                            };
                            cloned_hits
                                .lock()
                                .unwrap()
                                .entry(path.to_string())
                                .and_modify(|c| *c += 1)
                                .or_insert(1);
                            let status_line = if found {
                                "HTTP/1.1 200 OK\r\n"
                            } else {
                                "HTTP/1.1 404 Not Found\r\n"
                            };
                            let response =
                                format!("{}content-length: {}\r\n\r\n", status_line, body.len());
                            stream.write_all(response.as_bytes()).unwrap();
                            stream.write_all(&body).unwrap();
                            stream.flush().unwrap();
                        }
                        Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(5));
                        }
                        Err(_) => break,
                    }
                }
            });
            SimpleMockServer {
                address,
                hits,
                shutdown,
                handle: Some(handle),
                routes,
            }
        }

        fn url(&self, path: &str) -> String {
            format!("http://{}{}", self.address, path)
        }

        fn hits(&self, path: &str) -> usize {
            *self.hits.lock().unwrap().get(path).unwrap_or(&0)
        }

        fn add_route(&self, path: &str, body: Vec<u8>) {
            self.routes.lock().unwrap().push((path.to_string(), body));
        }
    }

    impl Drop for SimpleMockServer {
        fn drop(&mut self) {
            self.shutdown.store(true, Ordering::Relaxed);
            if let Some(handle) = self.handle.take() {
                let _ = handle.join();
            }
        }
    }

    #[test]
    fn download_with_progress_streams_data() {
        let server = SimpleMockServer::start(vec![("/file", b"payload".to_vec())]);
        let client = reqwest::blocking::Client::builder().build().unwrap();
        let mut dest = tempfile::tempfile().unwrap();
        let url = server.url("/file");
        let downloaded = download_with_progress(&client, &url, &mut dest, "test").unwrap();
        assert_eq!(downloaded, 7);
        assert_eq!(server.hits("/file"), 1);
    }

    #[test]
    fn download_with_progress_cb_cancelled() {
        let server = SimpleMockServer::start(vec![("/stream", b"chunk".to_vec())]);
        let client = reqwest::blocking::Client::builder().build().unwrap();
        let mut dest = tempfile::tempfile().unwrap();
        let url = server.url("/stream");
        let mut first = true;
        let err = download_with_progress_cb(
            &client,
            &url,
            &mut dest,
            &mut |_: DownloadProgress| {
                if first {
                    first = false;
                    false
                } else {
                    true
                }
            },
            None,
        )
        .expect_err("expected cancellation");
        assert!(err.to_string().contains("cancelled"));
        assert_eq!(server.hits("/stream"), 1);
    }

    #[test]
    fn download_uefi_firmware_extracts_asset() {
        let zip_bytes = build_zip(&[("boot/README.txt", b"mash")]);
        let server = SimpleMockServer::start(vec![("/firmware.zip", zip_bytes.clone())]);
        let firmware_url = server.url("/firmware.zip");
        let payload = json!({
            "assets": [{
                "name": "RPi4_UEFI_Firmware_v1.0.zip",
                "browser_download_url": firmware_url
            }]
        })
        .to_string();
        server.add_route("/latest", payload.into_bytes());
        let api_url = server.url("/latest");
        let _guard = EnvGuard::set("MASH_GITHUB_API_URL", &api_url);
        let dir = tempdir().unwrap();
        download_uefi_firmware(dir.path()).unwrap();
        assert!(dir.path().join("boot/README.txt").exists());
        assert!(server.hits("/latest") >= 1);
    }

    #[test]
    fn download_fedora_image_skips_if_artifacts_exist() {
        let dir = tempdir().unwrap();
        let version = "43";
        let edition = "KDE";
        let arch = "aarch64";
        let dest_name = format!("Fedora-{}-{}-{}.raw.xz", edition, version, arch);
        let raw_name = format!("Fedora-{}-{}-{}.raw", edition, version, arch);
        let dest_path = dir.path().join(&dest_name);
        let raw_path = dir.path().join(&raw_name);
        File::create(&dest_path).unwrap();
        File::create(&raw_path).unwrap();
        let result = download_fedora_image(dir.path(), version, edition).unwrap();
        assert_eq!(result, raw_path);
    }

    #[test]
    fn download_fedora_image_remote_download_and_unxz_failure() {
        let server = SimpleMockServer::start(vec![("/fedora.raw.xz", b"data".to_vec())]);
        let _env = EnvGuard::set("MASH_FEDORA_DOWNLOAD_URLS", &server.url("/fedora.raw.xz"));
        let _unxz = EnvGuard::set("MASH_UNXZ_COMMAND", "/bin/false");
        let dir = tempdir().unwrap();
        let version = "43";
        let edition = "KDE";
        let err = download_fedora_image(dir.path(), version, edition).unwrap_err();
        assert!(err.to_string().contains("unxz failed"));
        assert!(server.hits("/fedora.raw.xz") >= 1);
    }
}
