use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};

use anyhow::{Context, Result};
use serde::Serialize;

use crate::paths;

pub(crate) fn write(path: impl AsRef<Path>, value: &impl Serialize) -> Result<()> {
    let path = path.as_ref();
    paths::ensure_file_write_path_safe(path)?;
    let parent = path.parent().context("output path has no parent")?;
    fs::create_dir_all(parent).with_context(|| format!("could not create {}", parent.display()))?;
    paths::ensure_file_write_path_safe(path)?;

    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    let name = path.file_name().context("output path has no file name")?;

    for attempt in 0..1000_u32 {
        let temporary = parent.join(format!(
            ".{}.{}.{}.tmp",
            name.to_string_lossy(),
            std::process::id(),
            attempt
        ));
        let mut file = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(error).with_context(|| {
                    format!(
                        "could not create atomic temporary file beside {}",
                        path.display()
                    )
                });
            }
        };

        let result = (|| -> Result<()> {
            file.write_all(&bytes)
                .with_context(|| format!("could not write {}", temporary.display()))?;
            file.flush()
                .with_context(|| format!("could not flush {}", temporary.display()))?;
            file.sync_all()
                .with_context(|| format!("could not sync {}", temporary.display()))?;
            drop(file);
            paths::ensure_file_write_path_safe(path)?;
            replace_file(&temporary, path)?;
            sync_parent(parent)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        return result;
    }

    anyhow::bail!(
        "could not allocate an atomic temporary file beside {}",
        path.display()
    )
}

#[cfg(not(windows))]
fn replace_file(temporary: &Path, path: &Path) -> Result<()> {
    fs::rename(temporary, path)
        .with_context(|| format!("could not atomically replace {}", path.display()))
}

#[cfg(windows)]
fn replace_file(temporary: &Path, path: &Path) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };

    let temporary = temporary
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let path = path
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let replaced = unsafe {
        MoveFileExW(
            temporary.as_ptr(),
            path.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if replaced == 0 {
        return Err(std::io::Error::last_os_error())
            .context("could not atomically replace JSON file");
    }
    Ok(())
}

#[cfg(unix)]
fn sync_parent(parent: &Path) -> Result<()> {
    fs::File::open(parent)
        .and_then(|directory| directory.sync_all())
        .with_context(|| format!("could not sync {}", parent.display()))
}

#[cfg(not(unix))]
fn sync_parent(_parent: &Path) -> Result<()> {
    // MoveFileExW with MOVEFILE_WRITE_THROUGH requests durable metadata replacement.
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::ser::{Error, Serializer};
    use tempfile::TempDir;

    struct SerializationFailure;

    impl Serialize for SerializationFailure {
        fn serialize<S>(&self, _serializer: S) -> std::result::Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            Err(S::Error::custom("intentional serialization failure"))
        }
    }

    #[test]
    fn failed_serialization_preserves_existing_content() {
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("config.json");
        fs::write(&path, b"old content").unwrap();

        assert!(write(&path, &SerializationFailure).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"old content");
    }

    #[cfg(unix)]
    #[test]
    fn temporary_symlink_is_not_followed() {
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("config.json");
        let outside = directory.path().join("outside");
        fs::write(&outside, b"outside content").unwrap();
        let temporary = directory
            .path()
            .join(format!(".config.json.{}.0.tmp", std::process::id()));
        std::os::unix::fs::symlink(&outside, &temporary).unwrap();

        write(&path, &serde_json::json!({"safe": true})).unwrap();

        assert_eq!(fs::read(&outside).unwrap(), b"outside content");
        assert!(
            fs::symlink_metadata(&temporary)
                .unwrap()
                .file_type()
                .is_symlink()
        );
    }

    #[cfg(unix)]
    #[test]
    fn dangling_destination_symlink_is_rejected() {
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("config.json");
        std::os::unix::fs::symlink(directory.path().join("missing"), &path).unwrap();

        assert!(write(&path, &serde_json::json!({"safe": true})).is_err());
        assert!(
            fs::symlink_metadata(&path)
                .unwrap()
                .file_type()
                .is_symlink()
        );
    }
}
