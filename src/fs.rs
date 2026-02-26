//! File system utilities.

use anyhow::Result;
use std::fs;
use std::path::Path;

/// Helper function to copy a directory recursively, preserving symbolic links.
pub fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> Result<()> {
    let src = src.as_ref();
    let dst = dst.as_ref();

    // Guard against infinite recursion if dst is inside src.
    // We check this by canonicalizing src. If dst exists, we canonicalize it too.
    // If it doesn't exist, we check if it is logically inside src.
    let src_canon = fs::canonicalize(src)?;
    if dst.exists() {
        let dst_canon = fs::canonicalize(dst)?;
        if dst_canon.starts_with(&src_canon) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "Cannot copy directory into itself: {:?} is inside {:?}",
                    dst_canon, src_canon
                ),
            )
            .into());
        }
    } else {
        // If dst doesn't exist, check if its absolute path starts with src_canon
        let dst_abs = std::env::current_dir()?.join(dst);
        // This is a bit naive but covers common cases. Canonicalize the parent if possible.
        if dst_abs
            .parent()
            .and_then(|p| fs::canonicalize(p).ok())
            .filter(|p| p.starts_with(&src_canon))
            .is_some()
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "Cannot copy directory into itself: {:?} is inside {:?}",
                    dst_abs, src_canon
                ),
            )
            .into());
        }
    }

    if !dst.exists() {
        fs::create_dir_all(dst)?;
    }

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dst_path = dst.join(entry.file_name());

        if ty.is_dir() {
            copy_dir_all(entry.path(), &dst_path)?;
        } else if ty.is_symlink() {
            let target = fs::read_link(entry.path())?;
            #[cfg(unix)]
            std::os::unix::fs::symlink(target, &dst_path)?;
            #[cfg(windows)]
            {
                // On Windows, we need to know if the target is a directory to use symlink_dir.
                // fs::metadata() follows symlinks, so it gives the target's metadata.
                let is_dir = fs::metadata(entry.path())
                    .map(|m| m.is_dir())
                    .unwrap_or(false);
                if is_dir {
                    std::os::windows::fs::symlink_dir(target, &dst_path)?;
                } else {
                    std::os::windows::fs::symlink_file(target, &dst_path)?;
                }
            }
        } else {
            fs::copy(entry.path(), &dst_path)?;
        }
    }
    Ok(())
}
