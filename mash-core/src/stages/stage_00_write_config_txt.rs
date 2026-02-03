use anyhow::Result;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

const CONFIG_TXT: &str = r#"arm_64bit=1
enable_uart=1
enable_gic=1
armstub=RPI_EFI.fd
disable_commandline_tags=2

[pi4]
dtoverlay=upstream-pi4

[all]
# Add overlays here if needed
"#;

pub fn run(args: &[String]) -> Result<()> {
    let efi_mount = args.first().map(String::as_str).unwrap_or("/boot/efi");
    let cfg_path = Path::new(efi_mount).join("config.txt");
    println!(
        "[*] Writing safe Pi4 UEFI config.txt -> {}",
        cfg_path.display()
    );
    fs::write(&cfg_path, CONFIG_TXT)?;
    let mut cmd = Command::new("sync");
    let _ = crate::process_timeout::status_with_timeout("sync", &mut cmd, Duration::from_secs(60));
    Ok(())
}
