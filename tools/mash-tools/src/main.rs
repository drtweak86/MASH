use anyhow::{Context, Result, anyhow};
use clap::{Parser, Subcommand};
use reqwest::blocking::Client;
use reqwest::header::RANGE;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use toml_edit::{DocumentMut, Item};

#[derive(Debug, Parser)]
#[command(name = "mash-tools")]
#[command(about = "Rust tools for MASH maintenance tasks")]
struct Cli {
    /// Allow dirty git working tree
    #[arg(long)]
    allow_dirty: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    #[command(subcommand)]
    Release(ReleaseCommand),
    #[command(subcommand)]
    Links(LinksCommand),
}

#[derive(Debug, Subcommand)]
enum ReleaseCommand {
    /// Update the version in the root Cargo.toml
    Bump { version: String },

    /// Create and push a git tag
    Tag { version: String },
}

#[derive(Debug, Subcommand)]
enum LinksCommand {
    /// Perform HTTP health checks against documented OS download links.
    Check {
        /// Path to a TOML file containing [[health_checks]] entries.
        #[arg(long)]
        file: PathBuf,

        /// Per-request timeout in seconds.
        #[arg(long, default_value_t = 10)]
        timeout_secs: u64,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let repo_root = repo_root()?;
    ensure_clean_worktree(&repo_root, cli.allow_dirty)?;

    match cli.command {
        Commands::Release(cmd) => match cmd {
            ReleaseCommand::Bump { version } => bump_version(&repo_root, &version)?,
            ReleaseCommand::Tag { version } => tag_release(&repo_root, &version)?,
        },
        Commands::Links(cmd) => match cmd {
            LinksCommand::Check { file, timeout_secs } => {
                check_links(&repo_root, &file, timeout_secs)?
            }
        },
    }

    Ok(())
}

fn repo_root() -> Result<PathBuf> {
    let mut current = std::env::current_dir().context("failed to read current dir")?;
    loop {
        if current.join(".git").exists() {
            return Ok(current);
        }
        if !current.pop() {
            return Err(anyhow!("failed to locate repo root"));
        }
    }
}

fn ensure_clean_worktree(repo_root: &Path, allow_dirty: bool) -> Result<()> {
    if allow_dirty {
        return Ok(());
    }
    let output = Command::new("git")
        .arg("status")
        .arg("--porcelain")
        .current_dir(repo_root)
        .output()
        .context("failed to run git status")?;
    if !output.status.success() {
        return Err(anyhow!("git status failed"));
    }
    if !output.stdout.is_empty() {
        return Err(anyhow!(
            "working tree is dirty (use --allow-dirty to override)"
        ));
    }
    Ok(())
}

fn bump_version(repo_root: &Path, version: &str) -> Result<()> {
    let version = normalize_version(version)?;
    let cargo_path = repo_root.join("Cargo.toml");
    let content = fs::read_to_string(&cargo_path)
        .with_context(|| format!("failed to read {}", cargo_path.display()))?;
    let mut doc = content
        .parse::<DocumentMut>()
        .with_context(|| format!("failed to parse {}", cargo_path.display()))?;
    let workspace = doc
        .get_mut("workspace")
        .and_then(Item::as_table_mut)
        .ok_or_else(|| anyhow!("missing [workspace] table in Cargo.toml"))?;
    let workspace_package = workspace
        .get_mut("package")
        .and_then(Item::as_table_mut)
        .ok_or_else(|| anyhow!("missing [workspace.package] table in Cargo.toml"))?;
    workspace_package["version"] = toml_edit::value(version);
    fs::write(&cargo_path, doc.to_string())
        .with_context(|| format!("failed to write {}", cargo_path.display()))?;
    run_checks(repo_root)?;
    Ok(())
}

fn tag_release(repo_root: &Path, version: &str) -> Result<()> {
    let version = normalize_version(version)?;
    let tag = format!("v{version}");
    run_git(repo_root, ["tag", &tag])?;
    run_git(repo_root, ["push", "origin", &tag])?;
    Ok(())
}

fn normalize_version(raw: &str) -> Result<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("version cannot be empty"));
    }
    if trimmed.starts_with('v') {
        return Err(anyhow!("version must not include a leading 'v'"));
    }
    let parts: Vec<&str> = trimmed.split('.').collect();
    if parts.len() != 3 {
        return Err(anyhow!("version must be in X.Y.Z format"));
    }
    for part in parts {
        if part.is_empty() || !part.chars().all(|ch| ch.is_ascii_digit()) {
            return Err(anyhow!("version must be numeric X.Y.Z without prefixes"));
        }
    }
    Ok(trimmed.to_string())
}

fn run_checks(repo_root: &Path) -> Result<()> {
    run_cargo(repo_root, ["fmt"])?;
    run_cargo(repo_root, ["clippy", "--workspace", "--", "-D", "warnings"])?;
    run_cargo(repo_root, ["test", "--workspace"])?;
    Ok(())
}

fn run_cargo<I, S>(repo_root: &Path, args: I) -> Result<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let status = Command::new("cargo")
        .args(args)
        .current_dir(repo_root)
        .status()
        .context("failed to run cargo command")?;
    if !status.success() {
        return Err(anyhow!("cargo command failed"));
    }
    Ok(())
}

fn run_git<I, S>(repo_root: &Path, args: I) -> Result<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let status = Command::new("git")
        .args(args)
        .current_dir(repo_root)
        .status()
        .context("failed to run git command")?;
    if !status.success() {
        return Err(anyhow!("git command failed"));
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
struct LinkHealthFile {
    #[serde(default)]
    health_checks: Vec<LinkHealthCheck>,
}

#[derive(Debug, Deserialize)]
struct LinkHealthCheck {
    name: String,
    url: String,
}

fn check_links(repo_root: &Path, rel_path: &Path, timeout_secs: u64) -> Result<()> {
    let path = if rel_path.is_absolute() {
        rel_path.to_path_buf()
    } else {
        repo_root.join(rel_path)
    };
    let content =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let parsed: LinkHealthFile =
        toml::from_str(&content).with_context(|| format!("failed to parse {}", path.display()))?;
    if parsed.health_checks.is_empty() {
        return Err(anyhow!(
            "no [[health_checks]] entries found in {}",
            path.display()
        ));
    }

    let client = Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .user_agent("mash-tools/os-link-health")
        .build()
        .context("failed to build HTTP client")?;

    let mut failures = Vec::new();
    for check in parsed.health_checks {
        let res = check_one_link(&client, &check.url);
        match res {
            Ok(status) => {
                println!("OK  {} -> {}", check.name, status);
            }
            Err(err) => {
                println!("BAD {} -> {} ({})", check.name, check.url, err);
                failures.push((check.name, check.url, err.to_string()));
            }
        }
    }

    if failures.is_empty() {
        Ok(())
    } else {
        let mut msg = String::from("one or more OS download links failed:\n");
        for (name, url, err) in failures {
            msg.push_str(&format!("- {name}: {url} ({err})\n"));
        }
        Err(anyhow!(msg))
    }
}

fn check_one_link(client: &Client, url: &str) -> Result<String> {
    // Prefer HEAD to avoid large downloads. Some sites reject HEAD, so fall back to a tiny GET.
    let head = client.head(url).send();
    match head {
        Ok(resp) => {
            if resp.status().is_success() {
                return Ok(resp.status().to_string());
            }
            // Fall through to GET attempt for non-success statuses; some CDNs block HEAD.
        }
        Err(_) => {
            // Fall through to GET attempt.
        }
    }

    let resp = client
        .get(url)
        .header(RANGE, "bytes=0-0")
        .send()
        .context("GET fallback failed")?;
    if resp.status().is_success() {
        Ok(resp.status().to_string())
    } else {
        Err(anyhow!("HTTP {}", resp.status()))
    }
}
