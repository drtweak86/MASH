use mash_installer::ui::validation;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use tempfile::tempdir;

#[test]
fn disk_path_requires_dev_prefix() {
    let result = validation::validate_disk_path("sda");
    assert!(result.is_err());
}

#[test]
fn disk_path_accepts_existing_dev_path() {
    let result = validation::validate_disk_path("/dev/null");
    assert!(result.is_ok());
}

#[test]
fn image_path_requires_existing_file() {
    let temp = tempdir().expect("tempdir");
    let image_path = temp.path().join("image.raw");
    let mut file = File::create(&image_path).expect("create file");
    writeln!(file, "test").expect("write");

    let result = validation::validate_image_path(&image_path);
    assert!(result.is_ok());
}

#[test]
fn image_path_rejects_missing() {
    let temp = tempdir().expect("tempdir");
    let missing = temp.path().join("missing.raw");
    let result = validation::validate_image_path(&missing);
    assert!(result.is_err());
}

#[test]
fn uefi_dir_requires_rpi_efi() {
    let temp = tempdir().expect("tempdir");
    let result = validation::validate_uefi_dir(temp.path());
    assert!(result.is_err());
}

#[test]
fn uefi_dir_accepts_valid_layout() {
    let temp = tempdir().expect("tempdir");
    let rpi_efi = temp.path().join("RPI_EFI.fd");
    File::create(&rpi_efi).expect("create RPI_EFI.fd");

    let result = validation::validate_uefi_dir(temp.path());
    assert!(result.is_ok());
}

#[test]
fn uefi_dir_rejects_empty_path() {
    let empty = PathBuf::new();
    let result = validation::validate_uefi_dir(&empty);
    assert!(result.is_err());
}
