use anyhow::Result;
use std::process::Command;

pub fn run(_args: &[String]) -> Result<()> {
    println!("[*] Packages: desktop/media bits (safe subset)");

    let _ = Command::new("dnf")
        .args([
            "install",
            "-y",
            "--skip-unavailable",
            "--setopt=install_weak_deps=True",
            "pipewire",
            "pipewire-pulseaudio",
            "alsa-utils",
            "pavucontrol",
            "gstreamer1-plugins-ugly",
            "gstreamer1-plugins-bad-free-extras",
        ])
        .status();

    Ok(())
}
