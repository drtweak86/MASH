# libdnf5 POC

This crate is a build-time probe to confirm we can locate and link against
libdnf5 using pkg-config and the C++ headers.

## Build requirements

- pkg-config (or pkgconf)
- libdnf5-devel (Fedora package name)
- a C++ compiler (g++)

## Notes

- The build script uses pkg-config to locate `libdnf5` and compiles a tiny
  C++ translation unit that includes `<libdnf5/base/base.hpp>`.
- The Rust binary calls into the compiled probe so that the link step
  actually pulls in libdnf5.
