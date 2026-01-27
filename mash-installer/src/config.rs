use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    pub mash_root: PathBuf,
}

impl Config {
    pub fn new(mash_root: PathBuf) -> Self {
        Self { mash_root }
    }
}
