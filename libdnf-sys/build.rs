use std::env;

fn main() {
    println!("cargo:rerun-if-changed=src/libdnf5_bridge.cpp");
    println!("cargo:rerun-if-changed=src/libdnf5_bridge_stub.cpp");
    println!("cargo:rerun-if-env-changed=LIBDNF5_NO_PKG_CONFIG");

    let skip_pkg_config = env::var_os("LIBDNF5_NO_PKG_CONFIG").is_some();
    let lib = if skip_pkg_config {
        None
    } else {
        match pkg_config::Config::new()
            .atleast_version("5")
            .probe("libdnf5")
        {
            Ok(lib) => Some(lib),
            Err(err) => {
                // For CI and dev environments without libdnf5 installed we still want the
                // workspace to build (WO-020 Phase 1 gating uses --all-features).
                println!(
                    "cargo:warning=libdnf5 not found via pkg-config; building stub backend ({err})"
                );
                None
            }
        }
    };

    let mut build = cc::Build::new();
    build.cpp(true);
    build.flag_if_supported("-std=c++17");
    if let Some(lib) = lib {
        build.file("src/libdnf5_bridge.cpp");
        for path in lib.include_paths {
            build.include(path);
        }
    } else {
        build.file("src/libdnf5_bridge_stub.cpp");
    }

    if let Ok(cxx) = env::var("CXX") {
        build.compiler(cxx);
    }

    build.compile("mash_libdnf5_bridge");

    println!("cargo:rustc-link-lib=stdc++");
}
