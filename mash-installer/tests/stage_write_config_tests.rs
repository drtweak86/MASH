use mash_installer::stages::stage_00_write_config_txt::write_config_txt;
use tempfile::TempDir;

const EXPECTED_CONFIG: &str = "arm_64bit=1\n\
enable_uart=1\n\
enable_gic=1\n\
armstub=RPI_EFI.fd\n\
disable_commandline_tags=2\n\
\n\
[pi4]\n\
dtoverlay=upstream-pi4\n\
\n\
[all]\n\
# Add overlays here if needed\n";

#[test]
fn write_config_txt_writes_expected_contents() {
    let temp_dir = TempDir::new().expect("temp dir");
    let config_path = temp_dir.path().join("config.txt");

    write_config_txt(&config_path).expect("write config.txt");

    let written = std::fs::read_to_string(&config_path).expect("read config.txt");
    assert_eq!(written, EXPECTED_CONFIG);
}
