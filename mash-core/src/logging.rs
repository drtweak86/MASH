pub fn init() {
    use env_logger::Target;
    use std::fs;
    use std::io;

    // Prefer writing logs to a stable location for one-shot installs. If we cannot
    // create the file (permissions, readonly FS, etc.), fall back to stderr.
    let target = (|| -> io::Result<Target> {
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
