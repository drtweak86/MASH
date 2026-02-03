// Stub backend for environments without libdnf5 headers/libraries.
//
// WO-020 Phase 1 gating runs with --all-features, which enables mash-core's optional
// libdnf backend. In CI/dev where libdnf5 isn't installed, we build this stub to keep
// the workspace compiling. The real backend is compiled when pkg-config finds libdnf5.

extern "C" {
int mash_libdnf5_update() { return 1; }
int mash_libdnf5_install(const char* const*, unsigned long) { return 1; }
}

