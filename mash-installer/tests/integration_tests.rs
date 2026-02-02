use std::process::Command;

#[test]
fn dry_run_disk_ops_sequence_logs_expected_messages() {
    let output = Command::new(env!("CARGO_BIN_EXE_mash"))
        .arg("--dry-run")
        .output()
        .expect("failed to run mash binary");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);

    assert!(combined.contains("DRY RUN: Executing disk operations sequence."));
    assert!(combined.contains("DRY RUN: Simulating disk probing."));
    assert!(combined.contains("DRY RUN: Proposing partition scheme for /dev/sda."));
    assert!(combined.contains("DRY RUN: Formatting partition with Fat32 filesystem."));
    assert!(combined.contains("DRY RUN: Mounting Fat32 to \"/boot/efi\"."));
    assert!(combined.contains("DRY RUN: Verifying disk operations for \"/dev/sda\"."));
}
