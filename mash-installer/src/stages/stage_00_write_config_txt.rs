use anyhow::Result;
use std::error::Error;
use std::fs;
use std::path::Path;
use std::process::Command;

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
    write_config_txt(&cfg_path).map_err(|err| anyhow::anyhow!(err.to_string()))?;
    Ok(())
}

pub fn write_config_txt(config_path: &Path) -> Result<(), Box<dyn Error>> {
    fs::write(config_path, CONFIG_TXT)?;
    let _ = Command::new("sync").status();
    Ok(())
}
