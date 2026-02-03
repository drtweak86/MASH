use libc::c_char;
use std::ffi::CString;

#[link(name = "mash_libdnf5_bridge", kind = "static")]
extern "C" {
    fn mash_libdnf5_update() -> i32;
    fn mash_libdnf5_install(pkgs: *const *const c_char, len: usize) -> i32;
}

fn ensure_success(code: i32, context: &str) -> Result<(), String> {
    if code == 0 {
        Ok(())
    } else {
        Err(format!("libdnf5 backend error during {context}"))
    }
}

pub fn update() -> Result<(), String> {
    let code = unsafe { mash_libdnf5_update() };
    ensure_success(code, "update")
}

pub fn install(pkgs: &[String]) -> Result<(), String> {
    let mut c_strings = Vec::with_capacity(pkgs.len());
    for pkg in pkgs {
        c_strings.push(
            CString::new(pkg.as_str()).map_err(|_| {
                format!("libdnf5 backend received package with interior NUL: {pkg}")
            })?,
        );
    }
    let mut pointers: Vec<*const c_char> = c_strings.iter().map(|c| c.as_ptr()).collect();
    let code = unsafe { mash_libdnf5_install(pointers.as_mut_ptr(), pointers.len()) };
    ensure_success(code, "install")
}
