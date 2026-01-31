use anyhow::Result;
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::LazyLock;

static TEMPLATE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\{\{([^}]+)\}\}").unwrap());

/// Resolves all variables for substitution
pub fn resolve_variables(
    project_root: &Path,
    custom_vars: &HashMap<String, String>,
) -> HashMap<String, String> {
    let mut vars = HashMap::new();

    // 1. Project Name from package.json
    if let Ok(project_name) = get_project_name(project_root) {
        vars.insert("project_name".to_string(), project_name);
    }

    // 2. Git Branch
    if let Ok(git_branch) = get_git_branch(project_root) {
        vars.insert("git_branch".to_string(), git_branch);
    }

    // 3. Custom Variables
    for (key, value) in custom_vars {
        vars.insert(key.clone(), value.clone());
    }

    vars
}

/// Substitutes placeholders in the content with variable values
pub fn substitute(content: &str, vars: &HashMap<String, String>) -> String {
    TEMPLATE_RE
        .replace_all(content, |caps: &regex::Captures| {
            let key = caps.get(1).unwrap().as_str().trim();
            vars.get(key)
                .cloned()
                .unwrap_or_else(|| caps.get(0).unwrap().as_str().to_string())
        })
        .to_string()
}

fn get_project_name(project_root: &Path) -> Result<String> {
    let package_json_path = project_root.join("package.json");
    let content = fs::read_to_string(package_json_path)?;
    let v: serde_json::Value = serde_json::from_str(&content)?;

    if let Some(name) = v.get("name").and_then(|n| n.as_str()) {
        Ok(name.to_string())
    } else {
        anyhow::bail!("No name field in package.json")
    }
}

fn get_git_branch(project_root: &Path) -> Result<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(project_root)
        .output()?;

    if output.status.success() {
        let branch = String::from_utf8(output.stdout)?.trim().to_string();
        Ok(branch)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stderr = stderr.trim();
        anyhow::bail!("Failed to get git branch: {stderr}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_substitute() {
        let mut vars = HashMap::new();
        vars.insert("project_name".to_string(), "my-project".to_string());
        vars.insert("git_branch".to_string(), "main".to_string());

        let content = "Project: {{project_name}}, Branch: {{git_branch}}, Unknown: {{unknown}}";
        let result = substitute(content, &vars);

        assert_eq!(
            result,
            "Project: my-project, Branch: main, Unknown: {{unknown}}"
        );
    }

    #[test]
    fn test_get_project_name() {
        let temp_dir = TempDir::new().unwrap();
        let package_json = r#"{ "name": "test-package" }"#;
        fs::write(temp_dir.path().join("package.json"), package_json).unwrap();

        let name = get_project_name(temp_dir.path()).unwrap();
        assert_eq!(name, "test-package");
    }

    #[test]
    fn test_get_git_branch() {
        let temp_dir = TempDir::new().unwrap();

        // Initialize a git repo
        Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(temp_dir.path())
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.email", "you@example.com"])
            .current_dir(temp_dir.path())
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.name", "Your Name"])
            .current_dir(temp_dir.path())
            .output()
            .unwrap();

        // Need a commit to have a branch in some git versions, but usually rev-parse works
        fs::write(temp_dir.path().join("README"), "test").unwrap();
        Command::new("git")
            .arg("add")
            .arg(".")
            .current_dir(temp_dir.path())
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "initial"])
            .current_dir(temp_dir.path())
            .output()
            .unwrap();

        let branch = get_git_branch(temp_dir.path()).unwrap();
        // The default branch name can be master or main
        assert_eq!(branch, "main");
    }
}
