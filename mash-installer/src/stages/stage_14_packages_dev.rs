use anyhow::Result;

use crate::stages::package_management;

pub fn run(_args: &[String]) -> Result<()> {
    println!("[*] Packages: dev/build");
    package_management::install_packages(&[
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
    ])?;
    Ok(())
}
