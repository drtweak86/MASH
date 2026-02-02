use anyhow::{anyhow, Result};
use std::fs;
use std::path::Path;

pub fn copy_starship_toml(stage: &Path, src: &Path) -> Result<()> {
    let assets_dir = stage.join("assets");
    fs::create_dir_all(&assets_dir)?;
    let dest = assets_dir.join("starship.toml");
    fs::copy(src, dest)?;
    Ok(())
}

pub fn run(args: &[String]) -> Result<()> {
    let stage = args
        .first()
        .map(String::as_str)
        .ok_or_else(|| anyhow!("need staging dir path"))?;
    let src = args
        .get(1)
        .map(String::as_str)
        .ok_or_else(|| anyhow!("need starship.toml path"))?;

    copy_starship_toml(Path::new(stage), Path::new(src))
}
