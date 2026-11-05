use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use directories::ProjectDirs;

pub fn project_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

pub fn blur_home() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("BLUR_HOME") {
        return absolute(PathBuf::from(path));
    }
    ProjectDirs::from("com", "kpihx-labs", "blur")
        .map(|dirs| dirs.data_local_dir().to_path_buf())
        .context("could not determine a local data directory; set BLUR_HOME")
}

pub fn profiles_root() -> Result<PathBuf> {
    Ok(blur_home()?.join("profiles"))
}

pub fn profile_root(name: &str) -> Result<PathBuf> {
    validate_profile_name(name)?;
    Ok(profiles_root()?.join(name))
}

pub fn ensure_path_components_safe(path: &Path) -> Result<()> {
    let absolute = absolute(path.to_path_buf())?;
    let mut current = PathBuf::new();
    for component in absolute.components() {
        current.push(component);
        if current.exists() {
            ensure_not_symlink(&current)?;
        }
    }
    Ok(())
}

pub fn ensure_file_write_path_safe(path: &Path) -> Result<()> {
    let parent = path
        .parent()
        .context("path has no parent directory for safe write")?;
    ensure_path_components_safe(parent)?;
    if path.exists() {
        ensure_not_symlink(path)?;
    }
    Ok(())
}

pub fn validate_profile_name(name: &str) -> Result<()> {
    anyhow::ensure!(
        !name.is_empty()
            && name.len() <= 32
            && name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')),
        "profile names must be 1-32 ASCII letters, digits, '-' or '_'"
    );
    anyhow::ensure!(name != "." && name != "..", "invalid profile name");
    Ok(())
}

fn absolute(path: PathBuf) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path)
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}

fn ensure_not_symlink(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("could not inspect {}", path.display()))?;
    anyhow::ensure!(
        !metadata.file_type().is_symlink(),
        "refusing symlink/reparse path component: {}",
        path.display()
    );
    Ok(())
}
