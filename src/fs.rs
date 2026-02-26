//! File system utilities.

use anyhow::Result;
use std::fs;
use std::path::Path;

/// Helper function to copy a directory recursively, preserving symbolic links.
pub fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> Result<()> {
    let src = src.as_ref();
    let dst = dst.as_ref();

    // Guard against dst being inside src to avoid infinite recursion
    let src_canon = fs::canonicalize(src)?;

    // For dst, if it doesn't exist, canonicalize its parent directory
    let dst_canon = if dst.exists() {
        fs::canonicalize(dst)?
    } else if let Some(parent) = dst.parent() {
        // Use parent if dst doesn't exist yet
        let parent_canon = fs::canonicalize(parent)?;
        let dst_file_name = dst.file_name().map(Path::new).unwrap_or(Path::new(""));
        parent_canon.join(dst_file_name)
    } else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "destination path is invalid",
        )
        .into());
    };

    // Check if dst is a subdirectory of src
    if dst_canon.starts_with(&src_canon) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "destination cannot be inside source directory",
        )
        .into());
    }

    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dst_path = dst.join(entry.file_name());

        if ty.is_dir() {
            copy_dir_all(entry.path(), &dst_path)?;
        } else if ty.is_symlink() {
            let symlink_path = entry.path();
            let target = fs::read_link(&symlink_path)?;
            #[cfg(unix)]
            std::os::unix::fs::symlink(target, &dst_path)?;
            #[cfg(windows)]
            {
                // On Windows, we need to know if the target is a directory to use symlink_dir
                // Resolve the target path relative to the symlink's parent directory
                let symlink_parent = symlink_path.parent().unwrap_or(Path::new(""));
                let resolved_target = if target.is_absolute() {
                    target.clone()
                } else {
                    symlink_parent.join(&target)
                };
                let target_is_dir = fs::metadata(&resolved_target)
                    .map(|m| m.is_dir())
                    .unwrap_or(false);
                if target_is_dir {
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
