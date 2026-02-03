use std::env;

fn main() {
    println!("cargo:rerun-if-changed=src/libdnf5_probe.cpp");

    let lib = pkg_config::Config::new()
        .atleast_version("5")
        .probe("libdnf5")
        .expect("libdnf5 not found via pkg-config (install libdnf5-devel)");

    let mut build = cc::Build::new();
    build.cpp(true);
    build.flag_if_supported("-std=c++17");
    build.file("src/libdnf5_probe.cpp");
    for path in lib.include_paths {
        build.include(path);
    }

    if let Ok(cxx) = env::var("CXX") {
        build.compiler(cxx);
    }

    build.compile("libdnf5_probe");

    println!("cargo:rustc-link-lib=stdc++");
}
