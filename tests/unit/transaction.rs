use anyhow::{Context, Result};
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use tempfile::tempdir;

use agentsync::skills::transaction::with_rollback;

#[test]
fn rollback_on_failure_deletes_dir() -> Result<()> {
    let temp_dir = tempdir()?;
    let skill_dir = temp_dir.path().join("test-skill");
    fs::create_dir(&skill_dir)?;
    let file_path = skill_dir.join("SKILL.md");
    let mut file = File::create(&file_path)?;
    writeln!(file, "Skill manifest")?;

    let op = || -> Result<()> {
        // Simulate partial install
        assert!(file_path.exists());
        // Simulate failure
        anyhow::bail!("Something failed during install");
    };

    let cleanup = || {
        // Should remove the skill dir if op failed
        if skill_dir.exists() {
            fs::remove_dir_all(&skill_dir).ok();
        }
    };

    let result = with_rollback(op, cleanup);

    assert!(result.is_err());
    assert!(
        !skill_dir.exists(),
        "Skill dir should be cleaned up on error"
    );
    Ok(())
}

#[test]
fn no_rollback_on_success() -> Result<()> {
    let temp_dir = tempdir()?;
    let skill_dir = temp_dir.path().join("test-skill");
    fs::create_dir(&skill_dir)?;
    let op = || Ok(());
    let cleanup = || {
        // Should not be called
        panic!("Cleanup called unexpectedly");
    };
    let result = with_rollback(op, cleanup);
    assert!(result.is_ok());
    assert!(skill_dir.exists());
    Ok(())
}
