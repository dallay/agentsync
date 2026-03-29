use crate::config::{SyncType, TargetConfig};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillsLayoutMatch {
    DirectorySymlinkToExpectedSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillsModeMismatch {
    pub agent_name: String,
    pub target_name: String,
    pub destination: PathBuf,
    pub expected_source: PathBuf,
    pub configured_mode: SyncType,
    pub detected_layout: SkillsLayoutMatch,
}

impl SkillsModeMismatch {
    fn destination_display(&self) -> String {
        self.destination.display().to_string()
    }

    fn expected_source_display(&self) -> String {
        self.expected_source.display().to_string()
    }

    pub fn wizard_warning(&self) -> String {
        format!(
            "Skills target {} ({}) is configured as \"symlink-contents\", but {} is already a directory symlink to {}. Applying this config can replace that directory link with per-item links.",
            self.agent_name,
            self.target_name,
            self.destination_display(),
            self.expected_source_display()
        )
    }

    pub fn doctor_warning(&self) -> String {
        format!(
            "Skills mode mismatch for agent {} (target {}): configured as \"symlink-contents\", but {} is already a directory symlink to {}. Applying can cause avoidable churn; switch this target to \"symlink\" or rerun `agentsync init --wizard`.",
            self.agent_name,
            self.target_name,
            self.destination_display(),
            self.expected_source_display()
        )
    }

    pub fn status_hint(&self) -> String {
        format!(
            "Hint: configured as \"symlink-contents\", but this destination is already the expected directory symlink to {}. `agentsync apply` may churn unless you change it to \"symlink\".",
            self.expected_source_display()
        )
    }
}

pub fn is_skills_target(target_name: &str, target: &TargetConfig) -> bool {
    target_name == "skills" && target.source == "skills"
}

pub fn detect_skills_layout_match(
    project_root: &Path,
    expected_source: &Path,
    target_name: &str,
    target: &TargetConfig,
) -> Option<SkillsLayoutMatch> {
    if !is_skills_target(target_name, target) {
        return None;
    }

    let destination = project_root.join(&target.destination);
    let metadata = fs::symlink_metadata(&destination).ok()?;
    if !metadata.file_type().is_symlink() {
        return None;
    }

    let link_target = fs::read_link(&destination).ok()?;
    let resolved_link = resolve_link_target(&destination, &link_target);
    let expected_canon = fs::canonicalize(expected_source).ok()?;
    let link_canon = fs::canonicalize(resolved_link).ok()?;

    if expected_canon == link_canon && expected_canon.is_dir() {
        Some(SkillsLayoutMatch::DirectorySymlinkToExpectedSource)
    } else {
        None
    }
}

pub fn detect_skills_mode_mismatch(
    project_root: &Path,
    expected_source: &Path,
    agent_name: &str,
    target_name: &str,
    target: &TargetConfig,
) -> Option<SkillsModeMismatch> {
    if target.sync_type != SyncType::SymlinkContents {
        return None;
    }

    let detected_layout =
        detect_skills_layout_match(project_root, expected_source, target_name, target)?;

    Some(SkillsModeMismatch {
        agent_name: agent_name.to_string(),
        target_name: target_name.to_string(),
        destination: project_root.join(&target.destination),
        expected_source: expected_source.to_path_buf(),
        configured_mode: target.sync_type,
        detected_layout,
    })
}

fn resolve_link_target(destination: &Path, link_target: &Path) -> PathBuf {
    if link_target.is_absolute() {
        link_target.to_path_buf()
    } else {
        destination
            .parent()
            .unwrap_or(parent_fallback(destination))
            .join(link_target)
    }
}

fn parent_fallback(path: &Path) -> &Path {
    path.parent().unwrap_or_else(|| Path::new("."))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    #[cfg(unix)]
    fn detects_directory_symlink_to_expected_source() {
        use std::os::unix::fs as unix_fs;

        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path();
        let expected_source = project_root.join(".agents/skills");
        fs::create_dir_all(&expected_source).unwrap();

        let destination = project_root.join(".claude");
        fs::create_dir_all(&destination).unwrap();
        unix_fs::symlink("../.agents/skills", destination.join("skills")).unwrap();

        let target = TargetConfig {
            source: "skills".to_string(),
            destination: ".claude/skills".to_string(),
            sync_type: SyncType::SymlinkContents,
            pattern: None,
            exclude: Vec::new(),
            mappings: Vec::new(),
        };

        let layout = detect_skills_layout_match(project_root, &expected_source, "skills", &target);
        assert_eq!(
            layout,
            Some(SkillsLayoutMatch::DirectorySymlinkToExpectedSource)
        );

        let mismatch = detect_skills_mode_mismatch(
            project_root,
            &expected_source,
            "claude",
            "skills",
            &target,
        )
        .unwrap();
        assert!(mismatch.doctor_warning().contains("symlink-contents"));
        assert!(mismatch.doctor_warning().contains("directory symlink"));
    }

    #[test]
    #[cfg(unix)]
    fn ignores_unrelated_layouts() {
        use std::os::unix::fs as unix_fs;

        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path();
        let expected_source = project_root.join(".agents/skills");
        let other_source = project_root.join("other-skills");
        fs::create_dir_all(&expected_source).unwrap();
        fs::create_dir_all(&other_source).unwrap();

        let destination = project_root.join(".claude");
        fs::create_dir_all(&destination).unwrap();
        unix_fs::symlink("../other-skills", destination.join("skills")).unwrap();

        let target = TargetConfig {
            source: "skills".to_string(),
            destination: ".claude/skills".to_string(),
            sync_type: SyncType::SymlinkContents,
            pattern: None,
            exclude: Vec::new(),
            mappings: Vec::new(),
        };

        assert!(
            detect_skills_layout_match(project_root, &expected_source, "skills", &target).is_none()
        );
        assert!(
            detect_skills_mode_mismatch(
                project_root,
                &expected_source,
                "claude",
                "skills",
                &target,
            )
            .is_none()
        );
    }

    #[test]
    fn regular_directory_destination_is_not_a_layout_match() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path();
        let expected_source = project_root.join(".agents/skills");
        let destination = project_root.join(".claude/skills");
        fs::create_dir_all(&expected_source).unwrap();
        fs::create_dir_all(&destination).unwrap();

        let target = TargetConfig {
            source: "skills".to_string(),
            destination: ".claude/skills".to_string(),
            sync_type: SyncType::SymlinkContents,
            pattern: None,
            exclude: Vec::new(),
            mappings: Vec::new(),
        };

        assert!(
            detect_skills_layout_match(project_root, &expected_source, "skills", &target).is_none()
        );
    }

    #[test]
    #[cfg(unix)]
    fn symlink_destination_with_symlink_mode_has_layout_match_but_no_mismatch() {
        use std::os::unix::fs as unix_fs;

        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path();
        let expected_source = project_root.join(".agents/skills");
        fs::create_dir_all(&expected_source).unwrap();

        let destination = project_root.join(".claude");
        fs::create_dir_all(&destination).unwrap();
        unix_fs::symlink("../.agents/skills", destination.join("skills")).unwrap();

        let target = TargetConfig {
            source: "skills".to_string(),
            destination: ".claude/skills".to_string(),
            sync_type: SyncType::Symlink,
            pattern: None,
            exclude: Vec::new(),
            mappings: Vec::new(),
        };

        assert_eq!(
            detect_skills_layout_match(project_root, &expected_source, "skills", &target),
            Some(SkillsLayoutMatch::DirectorySymlinkToExpectedSource)
        );
        assert!(
            detect_skills_mode_mismatch(
                project_root,
                &expected_source,
                "claude",
                "skills",
                &target
            )
            .is_none()
        );
    }

    #[test]
    fn non_skills_targets_are_ignored_by_both_helpers() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path();
        let expected_source = project_root.join(".agents/commands");
        fs::create_dir_all(&expected_source).unwrap();

        let target = TargetConfig {
            source: "commands".to_string(),
            destination: ".claude/commands".to_string(),
            sync_type: SyncType::SymlinkContents,
            pattern: None,
            exclude: Vec::new(),
            mappings: Vec::new(),
        };

        assert!(
            detect_skills_layout_match(project_root, &expected_source, "skills", &target).is_none()
        );
        assert!(
            detect_skills_mode_mismatch(
                project_root,
                &expected_source,
                "claude",
                "skills",
                &target
            )
            .is_none()
        );
    }
}
