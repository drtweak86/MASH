use anyhow::Result;
use std::process::Command;

pub fn run(_args: &[String]) -> Result<()> {
    println!("[*] Packages: dev/build");

    let _ = Command::new("dnf")
        .args([
            "install",
            "-y",
            "--skip-unavailable",
            "--setopt=install_weak_deps=True",
            "gcc",
            "gcc-c++",
            "make",
            "cmake",
            "ninja-build",
            "ccache",
            "pkgconf-pkg-config",
            "python3-devel",
            "python3-pip",
            "patchelf",
            "git",
        ])
        .status();

    Ok(())
}
