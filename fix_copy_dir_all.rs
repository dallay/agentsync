/// Helper function to copy a directory recursively
pub fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> Result<()> {
    fs::create_dir_all(&dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dst_path = dst.as_ref().join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(entry.path(), dst_path)?;
        } else if ty.is_symlink() {
            let target = fs::read_link(entry.path())?;
            #[cfg(unix)]
            std::os::unix::fs::symlink(target, dst_path)?;
            #[cfg(windows)]
            {
                if entry.path().is_dir() {
                    std::os::windows::fs::symlink_dir(target, dst_path)?;
                } else {
                    std::os::windows::fs::symlink_file(target, dst_path)?;
                }
            }
        } else {
            fs::copy(entry.path(), dst_path)?;
        }
    }
    Ok(())
}
