use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
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
}

#[derive(Debug, Subcommand)]
enum ReleaseCommand {
    /// Update the version in the root Cargo.toml
    Bump { version: String },

    /// Create and push a git tag
    Tag { version: String },
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
