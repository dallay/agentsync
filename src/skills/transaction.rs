use anyhow::Result;

/// Runs the operation. If it fails, runs cleanup/rollback logic.
/// Ensures failed installs are reverted (idempotency).
pub fn with_rollback<F, C>(f: F, cleanup: C) -> Result<()>
where
    F: FnOnce() -> Result<()>,
    C: FnOnce(),
{
    match f() {
        Ok(_) => Ok(()),
        Err(e) => {
            cleanup();
            Err(e)
        }
    }
}
