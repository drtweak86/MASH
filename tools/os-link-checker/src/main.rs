use anyhow::{anyhow, Context, Result};
use reqwest::blocking::Client;
use reqwest::{redirect::Policy, StatusCode};
use serde::Deserialize;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Deserialize)]
struct LinksFile {
    links: Vec<OsLink>,
}

#[derive(Debug, Deserialize)]
struct OsLink {
    name: String,
    url: String,
    description: Option<String>,
}

fn main() -> Result<()> {
    let config_path = config_path_env()?;
    let contents = fs::read_to_string(&config_path)
        .with_context(|| format!("failed to read {}", config_path.display()))?;
    let links_file: LinksFile = toml::from_str(&contents)
        .with_context(|| format!("failed to parse {}", config_path.display()))?;

    if links_file.links.is_empty() {
        return Err(anyhow!("no links defined in {}", config_path.display()));
    }

    let client = build_client()?;
    let mut failures = Vec::new();

    for link in &links_file.links {
        match check_link(&client, link) {
            Ok(status) => print_success(link, status),
            Err(err) => {
                failures.push(link.name.clone());
                eprintln!("[FAIL] {} ({}) — {}", link.name, link.url, err);
            }
        }
    }

    if failures.is_empty() {
        println!(
            "All {} OS download links are reachable.",
            links_file.links.len()
        );
        Ok(())
    } else {
        Err(anyhow!(
            "{} download links failed ({}).",
            failures.len(),
            failures.join(", ")
        ))
    }
}

fn config_path_env() -> Result<PathBuf> {
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--config" {
            if let Some(value) = args.next() {
                return Ok(PathBuf::from(value));
            }
            break;
        }
    }
    Ok(PathBuf::from("docs/os-download-links.toml"))
}

fn build_client() -> Result<Client> {
    Client::builder()
        .user_agent("mash-os-link-checker/1.0")
        .timeout(Duration::from_secs(20))
        .redirect(Policy::limited(10))
        .build()
        .context("failed to build HTTP client")
}

fn check_link(client: &Client, link: &OsLink) -> Result<StatusCode> {
    let mut response = client
        .head(&link.url)
        .send()
        .with_context(|| format!("failed to reach {}", link.url))?;

    if response.status() == StatusCode::METHOD_NOT_ALLOWED {
        response = client
            .get(&link.url)
            .send()
            .with_context(|| format!("failed to reach {} via GET", link.url))?;
    }

    if response.status().is_success() {
        Ok(response.status())
    } else {
        Err(anyhow!("HTTP {}", response.status()))
    }
}

fn print_success(link: &OsLink, status: StatusCode) {
    if let Some(desc) = &link.description {
        println!("[OK] {} — {} ({})", link.name, desc, status);
    } else {
        println!("[OK] {} — {}", link.name, status);
    }
}
