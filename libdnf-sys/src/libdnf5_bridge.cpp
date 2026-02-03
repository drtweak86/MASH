#include <libdnf5/base/base.hpp>

extern "C" int mash_libdnf5_update() {
    try {
        libdnf5::Base base;
        (void)base;
        return 0;
    } catch (...) {
        return 1;
    }
}

extern "C" int mash_libdnf5_install(const char ** /*pkgs*/, size_t /*len*/) {
    try {
        libdnf5::Base base;
        (void)base;
        return 0;
    } catch (...) {
        return 1;
    }
}
