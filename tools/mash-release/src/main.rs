use clap::{Parser, ValueEnum};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

mod version;

use version::{bump_version, format_tag, parse_strict_version, BumpKind};

#[derive(Debug, Parser)]
#[command(name = "mash-release")]
#[command(about = "Release helper for the MASH installer")]
struct Cli {
    /// Which SemVer component to bump
    #[arg(long, value_enum, default_value = "patch")]
    bump: BumpArg,

    /// Explicit version to set (must be strict SemVer)
    #[arg(long, conflicts_with = "bump")]
    set: Option<String>,

    /// Show planned changes without modifying files or running git commands
    #[arg(long)]
    dry_run: bool,

    /// Skip creating a git tag
    #[arg(long)]
    no_tag: bool,

    /// Do not push to origin
    #[arg(long)]
    no_push: bool,

    /// Use a specific commit message
    #[arg(long)]
    message: Option<String>,

    /// Skip interactive prompts and accept defaults
    #[arg(short = 'y', long)]
    yes: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum BumpArg {
    Major,
    Minor,
    Patch,
}

impl From<BumpArg> for BumpKind {
    fn from(value: BumpArg) -> Self {
        match value {
            BumpArg::Major => BumpKind::Major,
            BumpArg::Minor => BumpKind::Minor,
            BumpArg::Patch => BumpKind::Patch,
        }
    }
}

fn main() -> Result<(), String> {
    let cli = Cli::parse();
    let repo_root = repo_root()?;
    let cargo_path = repo_root.join("mash-installer").join("Cargo.toml");
    let readme_path = repo_root.join("README.md");

    let current_version = read_cargo_version(&cargo_path)?;
    let next_version = if let Some(set_version) = cli.set.as_deref() {
        parse_strict_version(set_version)?
    } else {
        bump_version(&current_version, cli.bump.into())
    };
    let tag = format_tag(&next_version);

    if cli.dry_run {
        print_plan(&current_version, &next_version, &tag, &cargo_path, &readme_path)?;
        return Ok(());
    }

    let mut changed_files = Vec::new();
    if update_cargo_version(&cargo_path, &next_version)? {
        changed_files.push(path_from_root(&repo_root, &cargo_path));
    }
    if update_readme_title(&readme_path, &tag)? {
        changed_files.push(path_from_root(&repo_root, &readme_path));
    }

    run_checks(&repo_root)?;

    let commit_message = resolve_commit_message(&tag, cli.message, cli.yes)?;
    confirm_or_exit(&tag, &commit_message, cli.yes)?;

    if changed_files.is_empty() {
        return Err("no files changed; nothing to commit".to_string());
    }
    git_add(&repo_root, &changed_files)?;
    git_commit(&repo_root, &commit_message)?;
    if !cli.no_tag {
        git_tag(&repo_root, &tag)?;
    }
    if !cli.no_push {
        git_push(&repo_root, !cli.no_tag)?;
    }

    Ok(())
}

fn repo_root() -> Result<PathBuf, String> {
    let mut current = std::env::current_dir()
        .map_err(|err| format!("failed to read current dir: {}", err))?;
    loop {
        if current.join(".git").exists() {
            return Ok(current);
        }
        if !current.pop() {
            return Err("failed to locate repo root".to_string());
        }
    }
}

fn read_cargo_version(path: &Path) -> Result<semver::Version, String> {
    let content = fs::read_to_string(path)
        .map_err(|err| format!("failed to read {}: {}", path.display(), err))?;
    let mut in_package = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_package = trimmed == "[package]";
            continue;
        }
        if in_package && trimmed.starts_with("version") {
            let version = trimmed
                .split('=')
                .nth(1)
                .map(|value| value.trim().trim_matches('"'))
                .ok_or_else(|| "invalid version line".to_string())?;
            return parse_strict_version(version);
        }
    }
    Err("version not found in mash-installer/Cargo.toml".to_string())
}

fn update_cargo_version(path: &Path, version: &semver::Version) -> Result<bool, String> {
    let content = fs::read_to_string(path)
        .map_err(|err| format!("failed to read {}: {}", path.display(), err))?;
    let mut updated = Vec::new();
    let mut in_package = false;
    let mut changed = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_package = trimmed == "[package]";
            updated.push(line.to_string());
            continue;
        }
        if in_package && trimmed.starts_with("version") {
            let new_line = format!("version = \"{}\"", version);
            if trimmed != new_line {
                changed = true;
            }
            updated.push(new_line);
        } else {
            updated.push(line.to_string());
        }
    }
    if changed {
        fs::write(path, updated.join("\n") + "\n")
            .map_err(|err| format!("failed to write {}: {}", path.display(), err))?;
    }
    Ok(changed)
}

fn update_readme_title(path: &Path, tag: &str) -> Result<bool, String> {
    let content = fs::read_to_string(path)
        .map_err(|err| format!("failed to read {}: {}", path.display(), err))?;
    let mut lines: Vec<String> = content.lines().map(|line| line.to_string()).collect();
    if lines.is_empty() {
        return Ok(false);
    }
    let first = lines[0].clone();
    if let Some(updated) = replace_version_in_title(&first, tag) {
        if updated != first {
            lines[0] = updated;
            fs::write(path, lines.join("\n") + "\n")
                .map_err(|err| format!("failed to write {}: {}", path.display(), err))?;
            return Ok(true);
        }
    }
    Ok(false)
}

fn replace_version_in_title(line: &str, tag: &str) -> Option<String> {
    if !line.contains("MASH") {
        return None;
    }
    let mut chars = line.char_indices();
    while let Some((idx, ch)) = chars.next() {
        if ch != 'v' {
            continue;
        }
        let candidate = &line[idx + 1..];
        let mut end = 0;
        for (offset, c) in candidate.char_indices() {
            if c.is_ascii_digit() || c == '.' {
                end = offset + c.len_utf8();
            } else {
                break;
            }
        }
        if end == 0 {
            continue;
        }
        let version_str = &candidate[..end];
        if parse_strict_version(version_str).is_ok() {
            let mut updated = String::new();
            updated.push_str(&line[..idx]);
            updated.push_str(tag);
            updated.push_str(&candidate[end..]);
            return Some(updated);
        }
    }
    None
}

fn run_checks(repo_root: &Path) -> Result<(), String> {
    let mash_dir = repo_root.join("mash-installer");
    let mut fmt_cmd = Command::new("cargo");
    fmt_cmd
        .arg("fmt")
        .arg("--all")
        .arg("--")
        .arg("--check")
        .current_dir(&mash_dir);
    run_command(fmt_cmd, "cargo fmt --all -- --check")?;
    let mut test_cmd = Command::new("cargo");
    test_cmd.arg("test").current_dir(&mash_dir);
    run_command(test_cmd, "cargo test")?;
    Ok(())
}

fn run_command(mut cmd: Command, label: &str) -> Result<(), String> {
    let status = cmd
        .status()
        .map_err(|err| format!("failed to run {}: {}", label, err))?;
    if !status.success() {
        return Err(format!("{} failed", label));
    }
    Ok(())
}

fn resolve_commit_message(tag: &str, message: Option<String>, yes: bool) -> Result<String, String> {
    if let Some(message) = message {
        return Ok(message);
    }
    let default_msg = format!("chore: bump version to {}", tag);
    if yes {
        return Ok(default_msg);
    }
    println!("════════════════════════════════════════");
    println!("Enter your commit message (or press Enter for default):");
    println!("Default: '{}'", default_msg);
    println!("════════════════════════════════════════");
    let input = read_line()?;
    if input.trim().is_empty() {
        Ok(default_msg)
    } else {
        Ok(input.trim().to_string())
    }
}

fn confirm_or_exit(tag: &str, message: &str, yes: bool) -> Result<(), String> {
    if yes {
        return Ok(());
    }
    println!();
    println!("📝 Commit message: {}", message);
    println!("🏷️  Tag: {}", tag);
    print!("Proceed with commit, tag, and push? [y/N] ");
    io::stdout()
        .flush()
        .map_err(|err| format!("failed to flush stdout: {}", err))?;
    let input = read_line()?;
    if matches!(input.trim(), "y" | "Y") {
        Ok(())
    } else {
        Err("aborted".to_string())
    }
}

fn read_line() -> Result<String, String> {
    let mut buffer = String::new();
    io::stdin()
        .read_line(&mut buffer)
        .map_err(|err| format!("failed to read input: {}", err))?;
    Ok(buffer)
}

fn path_from_root(root: &Path, path: &Path) -> PathBuf {
    path.strip_prefix(root).unwrap_or(path).to_path_buf()
}

fn git_add(repo_root: &Path, files: &[PathBuf]) -> Result<(), String> {
    let mut cmd = Command::new("git");
    cmd.arg("add");
    for file in files {
        cmd.arg(file);
    }
    cmd.current_dir(repo_root);
    run_command(cmd, "git add")
}

fn git_commit(repo_root: &Path, message: &str) -> Result<(), String> {
    let mut cmd = Command::new("git");
    cmd.args(["commit", "-m", message]).current_dir(repo_root);
    run_command(cmd, "git commit")
}

fn git_tag(repo_root: &Path, tag: &str) -> Result<(), String> {
    let mut cmd = Command::new("git");
    cmd.args(["tag", "-a", tag, "-m", &format!("Release {}", tag)])
        .current_dir(repo_root);
    run_command(cmd, "git tag")
}

fn git_push(repo_root: &Path, with_tags: bool) -> Result<(), String> {
    let mut cmd = Command::new("git");
    cmd.arg("push").arg("origin").arg("main");
    if with_tags {
        cmd.arg("--tags");
    }
    cmd.current_dir(repo_root);
    run_command(cmd, "git push")
}

fn print_plan(
    current: &semver::Version,
    next: &semver::Version,
    tag: &str,
    cargo_path: &Path,
    readme_path: &Path,
) -> Result<(), String> {
    println!("Current version: {}", current);
    println!("Next version: {}", next);
    println!("Tag: {}", tag);
    println!("Would update: {}", cargo_path.display());
    println!("Would update (if present): {}", readme_path.display());
    println!("Would run: cargo fmt --all -- --check (in mash-installer)");
    println!("Would run: cargo test (in mash-installer)");
    println!("Would git add/commit/tag/push unless disabled");
    Ok(())
}
