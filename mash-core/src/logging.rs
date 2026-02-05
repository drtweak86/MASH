use std::path::PathBuf;

/// Initialize logging, optionally directing output to a specific file.
/// Falls back to /var/log/mash/dojo.log, and finally stderr if file logging fails.
pub fn init_with(log_file: Option<PathBuf>) {
    use env_logger::Target;
    use std::fs;
    use std::io;

    let target = (|| -> io::Result<Target> {
        if let Some(path) = log_file {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            let file = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)?;
            return Ok(Target::Pipe(Box::new(file)));
        }

        // Default path used historically; keep as first fallback.
        fs::create_dir_all("/var/log/mash")?;
        let file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/var/log/mash/dojo.log")?;
        Ok(Target::Pipe(Box::new(file)))
    })()
    .unwrap_or(Target::Stderr);

    env_logger::Builder::from_default_env()
        .target(target)
        .filter_level(log::LevelFilter::Info)
        .init();
}

/// Backwards-compatible initializer (defaults to built-in path).
pub fn init() {
    init_with(None);
}
