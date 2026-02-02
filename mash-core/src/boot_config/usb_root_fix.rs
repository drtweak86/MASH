use anyhow::{anyhow, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

pub fn apply_usb_root_fix(root: &Path, mountinfo: &str, by_uuid_dir: &Path) -> Result<()> {
    let root_device = root_device_from_mountinfo(mountinfo)?;
    let root_uuid = uuid_for_device(by_uuid_dir, &root_device)?;

    let cmdline_path = root.join("etc/kernel/cmdline");
    patch_kernel_cmdline(&cmdline_path, &root_uuid)?;

    let bls_dir = root.join("boot/loader/entries");
    patch_bls_entries(&bls_dir, &root_uuid)?;

    validate_kernel_cmdline(&cmdline_path, &root_uuid)?;
    validate_bls_entries(&bls_dir, &root_uuid)?;

    Ok(())
}

pub fn root_device_from_mountinfo(mountinfo: &str) -> Result<String> {
    for line in mountinfo.lines() {
        let mut parts = line.split(" - ");
        let pre = parts.next().unwrap_or("");
        let post = parts.next().unwrap_or("");

        let pre_fields: Vec<&str> = pre.split_whitespace().collect();
        if pre_fields.len() < 5 {
            continue;
        }
        let mount_point = pre_fields[4];
        if mount_point != "/" {
            continue;
        }

        let post_fields: Vec<&str> = post.split_whitespace().collect();
        if post_fields.len() < 2 {
            continue;
        }
        let source = post_fields[1];
        return Ok(source.to_string());
    }
    Err(anyhow!("Failed to locate root mount in mountinfo"))
}

pub fn uuid_for_device(by_uuid_dir: &Path, device: &str) -> Result<String> {
    let device_path = fs::canonicalize(device)
        .with_context(|| format!("Failed to resolve device path: {}", device))?;

    for entry in fs::read_dir(by_uuid_dir)
        .with_context(|| format!("Failed to read {}", by_uuid_dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if !path.is_symlink() {
            continue;
        }
        let target = fs::read_link(&path)
            .with_context(|| format!("Failed to read link {}", path.display()))?;
        let resolved = if target.is_absolute() {
            target
        } else {
            path.parent().unwrap_or(by_uuid_dir).join(target)
        };
        if fs::canonicalize(&resolved).ok() == Some(device_path.clone()) {
            let uuid = path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| anyhow!("Invalid UUID entry name"))?;
            return Ok(uuid.to_string());
        }
    }

    Err(anyhow!(
        "No UUID entry in {} for device {}",
        by_uuid_dir.display(),
        device
    ))
}

pub fn patch_kernel_cmdline(path: &Path, root_uuid: &str) -> Result<()> {
    if !path.exists() {
        return Err(anyhow!("Kernel cmdline not found: {}", path.display()));
    }
    let content =
        fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?;
    let updated = patch_options_line(&content, root_uuid)?;
    if updated != content {
        backup_file(path)?;
        fs::write(path, updated).with_context(|| format!("Failed to write {}", path.display()))?;
    }
    Ok(())
}

pub fn patch_bls_entries(entries_dir: &Path, root_uuid: &str) -> Result<()> {
    if !entries_dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(entries_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().map(|e| e == "conf").unwrap_or(false) {
            let content = fs::read_to_string(&path)?;
            let updated = patch_bls_content(&content, root_uuid)?;
            if updated != content {
                backup_file(&path)?;
                fs::write(&path, updated)?;
            }
        }
    }
    Ok(())
}

fn patch_options_line(content: &str, root_uuid: &str) -> Result<String> {
    let mut line = content.trim().to_string();
    if line.is_empty() {
        return Err(anyhow!("Kernel cmdline is empty"));
    }
    line = replace_root_arg(&line, root_uuid);
    line = ensure_rootflags(line);
    Ok(format!("{}\n", line))
}

fn patch_bls_content(content: &str, root_uuid: &str) -> Result<String> {
    let mut out = Vec::new();
    for line in content.lines() {
        if let Some(stripped) = line.strip_prefix("options ") {
            let mut opts = stripped.trim().to_string();
            opts = replace_root_arg(&opts, root_uuid);
            opts = ensure_rootflags(opts);
            out.push(format!("options {}", opts));
        } else {
            out.push(line.to_string());
        }
    }
    Ok(out.join("\n") + "\n")
}

fn replace_root_arg(options: &str, root_uuid: &str) -> String {
    let mut parts: Vec<String> = options.split_whitespace().map(|s| s.to_string()).collect();
    let mut replaced = false;
    for part in &mut parts {
        if part.starts_with("root=") {
            *part = format!("root=UUID={}", root_uuid);
            replaced = true;
            break;
        }
    }
    if !replaced {
        parts.insert(0, format!("root=UUID={}", root_uuid));
    }
    parts.join(" ")
}

fn ensure_rootflags(options: String) -> String {
    let mut parts: Vec<String> = options.split_whitespace().map(|s| s.to_string()).collect();
    let mut found = false;
    for part in &mut parts {
        if part.starts_with("rootflags=") {
            *part = "rootflags=subvol=root".to_string();
            found = true;
            break;
        }
    }
    if !found {
        parts.push("rootflags=subvol=root".to_string());
    }
    parts.join(" ")
}

fn backup_file(path: &Path) -> Result<PathBuf> {
    let backup = path.with_file_name(format!(
        "{}.bak",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("file")
    ));
    if !backup.exists() {
        fs::copy(path, &backup).with_context(|| {
            format!(
                "Failed to backup {} to {}",
                path.display(),
                backup.display()
            )
        })?;
    }
    Ok(backup)
}

fn validate_kernel_cmdline(path: &Path, root_uuid: &str) -> Result<()> {
    let content = fs::read_to_string(path)?;
    validate_options_line(&content, root_uuid)
}

fn validate_bls_entries(entries_dir: &Path, root_uuid: &str) -> Result<()> {
    if !entries_dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(entries_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().map(|e| e == "conf").unwrap_or(false) {
            let content = fs::read_to_string(&path)?;
            for line in content.lines() {
                if let Some(stripped) = line.strip_prefix("options ") {
                    validate_options_line(stripped, root_uuid)?;
                }
            }
        }
    }
    Ok(())
}

fn validate_options_line(options: &str, root_uuid: &str) -> Result<()> {
    let root_token = format!("root=UUID={}", root_uuid);
    if !options.split_whitespace().any(|part| part == root_token) {
        return Err(anyhow!("Missing root UUID option"));
    }
    if !options
        .split_whitespace()
        .any(|part| part == "rootflags=subvol=root")
    {
        return Err(anyhow!("Missing rootflags=subvol=root"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    use tempfile::tempdir;

    #[test]
    fn root_device_is_parsed_from_mountinfo() {
        let mountinfo = "36 28 0:31 / / rw,relatime - ext4 /dev/sda3 rw,data=ordered";
        let device = root_device_from_mountinfo(mountinfo).unwrap();
        assert_eq!(device, "/dev/sda3");
    }

    #[test]
    fn uuid_is_resolved_from_by_uuid() {
        let dir = tempdir().unwrap();
        let dev_dir = dir.path().join("dev");
        fs::create_dir_all(&dev_dir).unwrap();
        let device = dev_dir.join("sda3");
        fs::write(&device, "").unwrap();

        let by_uuid = dir.path().join("by-uuid");
        fs::create_dir_all(&by_uuid).unwrap();
        symlink(&device, by_uuid.join("ABC-123")).unwrap();

        let uuid = uuid_for_device(&by_uuid, device.to_str().unwrap()).unwrap();
        assert_eq!(uuid, "ABC-123");
    }

    #[test]
    fn cmdline_and_bls_are_patched_with_backups() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let etc_kernel = root.join("etc/kernel");
        let boot_entries = root.join("boot/loader/entries");
        fs::create_dir_all(&etc_kernel).unwrap();
        fs::create_dir_all(&boot_entries).unwrap();

        let cmdline = etc_kernel.join("cmdline");
        fs::write(&cmdline, "root=/dev/sda3 quiet").unwrap();

        let bls = boot_entries.join("test.conf");
        fs::write(&bls, "options root=/dev/sda3 quiet\n").unwrap();

        let dev_dir = root.join("dev");
        fs::create_dir_all(&dev_dir).unwrap();
        let device = dev_dir.join("sda3");
        fs::write(&device, "").unwrap();
        let mountinfo = format!(
            "36 28 0:31 / / rw,relatime - ext4 {} rw,data=ordered",
            device.display()
        );

        let by_uuid = root.join("by-uuid");
        fs::create_dir_all(&by_uuid).unwrap();
        symlink(&device, by_uuid.join("XYZ-999")).unwrap();

        apply_usb_root_fix(root, &mountinfo, &by_uuid).unwrap();

        let updated_cmdline = fs::read_to_string(&cmdline).unwrap();
        assert!(updated_cmdline.contains("root=UUID=XYZ-999"));
        assert!(updated_cmdline.contains("rootflags=subvol=root"));
        assert!(cmdline.with_file_name("cmdline.bak").exists());

        let updated_bls = fs::read_to_string(&bls).unwrap();
        assert!(updated_bls.contains("root=UUID=XYZ-999"));
        assert!(updated_bls.contains("rootflags=subvol=root"));
        assert!(bls.with_file_name("test.conf.bak").exists());
    }
}
