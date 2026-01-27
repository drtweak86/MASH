pub fn init() {
    // Respect RUST_LOG if set, otherwise default to info
    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", "info");
    }
    env_logger::init();
}
