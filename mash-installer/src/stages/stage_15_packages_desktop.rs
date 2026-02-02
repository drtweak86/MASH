use anyhow::Result;

use crate::stages::package_management;

pub fn run(_args: &[String]) -> Result<()> {
    println!("[*] Packages: desktop/media bits (safe subset)");
    package_management::install_packages(&[
        "pipewire",
        "pipewire-pulseaudio",
        "alsa-utils",
        "pavucontrol",
        "gstreamer1-plugins-ugly",
        "gstreamer1-plugins-bad-free-extras",
    ])?;
    Ok(())
}
