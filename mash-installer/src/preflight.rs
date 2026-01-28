use std::process::Command;

pub fn run(dry_run: bool) -> anyhow::Result<()> {
    println!("🧪 Phase 1A – Preflight");
    for tool in ["rsync","pv","parted","losetup"] {
        let ok = Command::new("which").arg(tool).status()?.success();
        if ok { println!("✅ {}", tool); }
        else { anyhow::bail!("❌ missing {}", tool); }
    }
    if dry_run { println!("(dry-run) no changes made"); }
    Ok(())
}
