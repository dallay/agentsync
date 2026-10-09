use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use tempfile::TempDir;

fn run_doctor(project_root: &Path, path: Option<&Path>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_agentsync"));
    command.current_dir(project_root).arg("doctor");
    if let Some(path) = path {
        command.env("PATH", path);
    }
    command
        .output()
        .unwrap_or_else(|error| panic!("failed to run agentsync doctor: {error}"))
}

fn write_config(project_root: &Path, content: &str) {
    let config_path = project_root.join(".agents/agentsync.toml");
    fs::create_dir_all(config_path.parent().unwrap()).unwrap();
    fs::write(config_path, content).unwrap();
}

#[test]
fn doctor_reports_missing_config_and_returns_success() {
    let temp = TempDir::new().unwrap();

    let output = run_doctor(temp.path(), None);

    assert!(
        output.status.success(),
        "doctor should report missing config without failing the command: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("Could not find config"),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn doctor_reports_invalid_config_and_returns_success() {
    let temp = TempDir::new().unwrap();
    write_config(temp.path(), "invalid = [");

    let output = run_doctor(temp.path(), None);

    assert!(
        output.status.success(),
        "doctor should report invalid config without failing the command: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("Failed to parse config"),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn doctor_accepts_healthy_cross_platform_project() {
    let temp = TempDir::new().unwrap();
    write_config(
        temp.path(),
        r#"
        [gitignore]
        enabled = false

        [agents.claude]
        enabled = true

        [agents.claude.targets.instructions]
        source = "AGENTS.md"
        destination = ".claude/CLAUDE.md"
        type = "symlink"
        "#,
    );
    fs::write(temp.path().join(".agents/AGENTS.md"), "# instructions\n").unwrap();

    let output = run_doctor(temp.path(), None);

    assert!(
        output.status.success(),
        "doctor should succeed for a healthy project: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("All target sources exist"), "{stdout}");
    assert!(stdout.contains("No issues found"), "{stdout}");
}

#[test]
fn doctor_reports_missing_sources_configuration_warnings_and_conflicts() {
    let temp = TempDir::new().unwrap();
    write_config(
        temp.path(),
        r#"
        [gitignore]
        enabled = true

        [agents.inactive]
        enabled = false

        [agents.inactive.targets.ignored]
        source = "ignored.md"
        destination = "ignored"
        type = "symlink"

        [agents.claude]
        enabled = true

        [agents.claude.targets.regular]
        source = "missing.md"
        destination = ".shared"
        type = "symlink"

        [[agents.claude.targets.regular.mappings]]
        source = "unused-mapping.md"
        destination = "unused"

        [agents.claude.targets.modules]
        source = "placeholder"
        destination = "placeholder"
        type = "module-map"

        [[agents.claude.targets.modules.mappings]]
        source = "missing-mapping.md"
        destination = "src"

        [agents.claude.targets.nested]
        source = "missing-root"
        destination = ".claude/{file_name}"
        type = "nested-glob"
        pattern = "**/AGENTS.md"

        [agents.copilot]
        enabled = true

        [agents.copilot.targets.duplicate]
        source = "AGENTS.md"
        destination = ".shared"
        type = "symlink"

        [agents.copilot.targets.child]
        source = "AGENTS.md"
        destination = ".shared/child"
        type = "symlink"
        "#,
    );
    fs::write(temp.path().join(".agents/AGENTS.md"), "# instructions\n").unwrap();

    let output = run_doctor(temp.path(), None);

    assert!(
        output.status.success(),
        "doctor diagnostics should not fail the command: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("mappings is only used by module-map targets"),
        "{stdout}"
    );
    assert!(
        stdout.contains("Missing source for agent claude (target regular)"),
        "{stdout}"
    );
    assert!(
        stdout.contains("Missing mapping source for agent claude (target modules"),
        "{stdout}"
    );
    assert!(stdout.contains("Overlapping destinations"), "{stdout}");
    assert!(stdout.contains("Duplicate destination"), "{stdout}");
    assert!(stdout.contains(".gitignore file not found"), "{stdout}");
    assert!(stdout.contains("Found 7 issues"), "{stdout}");
}

#[test]
fn doctor_audits_mcp_commands_gitignore_and_unmanaged_skills() {
    let temp = TempDir::new().unwrap();
    let tool_path = temp.path().join("doctor-tool");
    let missing_tool_path = temp.path().join("missing-tool");
    let non_executable_path = temp.path().join("non-executable-tool");
    fs::write(&tool_path, "fixture executable").unwrap();
    fs::write(&non_executable_path, "not executable on Unix").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&tool_path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    let config = format!(
        r#"
        [gitignore]
        enabled = true

        [mcp]
        enabled = true

        [mcp_servers.present]
        command = {:?}

        [mcp_servers.missing]
        command = {:?}

        [mcp_servers.non_executable]
        command = {:?}

        [mcp_servers.unconfigured]
        args = ["fixture"]

        [mcp_servers.disabled]
        disabled = true
        command = {:?}
        "#,
        tool_path.to_string_lossy(),
        missing_tool_path.to_string_lossy(),
        non_executable_path.to_string_lossy(),
        missing_tool_path.to_string_lossy()
    );
    write_config(temp.path(), &config);
    let unmanaged_skill = temp.path().join(".claude/skills/local-skill/SKILL.md");
    fs::create_dir_all(unmanaged_skill.parent().unwrap()).unwrap();
    fs::write(&unmanaged_skill, "# Local skill\n").unwrap();

    let output = run_doctor(temp.path(), None);

    assert!(
        output.status.success(),
        "doctor diagnostics should not fail the command: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("MCP server present command executable"),
        "{stdout}"
    );
    assert!(
        stdout.contains("MCP server missing command not found in PATH"),
        "{stdout}"
    );
    assert!(
        stdout.contains("MCP server unconfigured has no command configured"),
        "{stdout}"
    );
    assert!(!stdout.contains("MCP server disabled"), "{stdout}");
    #[cfg(unix)]
    assert!(
        stdout.contains("MCP server non_executable command not found in PATH"),
        "{stdout}"
    );
    #[cfg(windows)]
    assert!(
        stdout.contains("MCP server non_executable command executable"),
        "{stdout}"
    );
    assert!(stdout.contains(".gitignore file not found"), "{stdout}");
    assert!(
        stdout.contains(".claude/skills/ has content but is not managed"),
        "{stdout}"
    );
    #[cfg(unix)]
    assert!(stdout.contains("Found 4 issues"), "{stdout}");
    #[cfg(windows)]
    assert!(stdout.contains("Found 3 issues"), "{stdout}");
}

#[test]
fn doctor_finds_platform_executable_extensions_in_isolated_path() {
    let temp = TempDir::new().unwrap();
    let bin_dir = temp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();

    let command_names = ["doctor-exe", "doctor-cmd", "doctor-bat", "doctor-com"];
    for command_name in &command_names {
        #[cfg(windows)]
        let file_name = format!(
            "{command_name}.{}",
            match *command_name {
                "doctor-exe" => "exe",
                "doctor-cmd" => "cmd",
                "doctor-bat" => "bat",
                "doctor-com" => "com",
                _ => unreachable!("fixture names are enumerated above"),
            }
        );
        #[cfg(unix)]
        let file_name = command_name.to_string();

        let executable_path = bin_dir.join(file_name);
        fs::write(&executable_path, "fixture executable").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&executable_path, fs::Permissions::from_mode(0o755)).unwrap();
        }
    }

    let mut config = String::from("[gitignore]\nenabled = false\n[mcp]\nenabled = true\n");
    for command_name in command_names {
        config.push_str(&format!(
            "\n[mcp_servers.{command_name}]\ncommand = {command_name:?}\n"
        ));
    }
    write_config(temp.path(), &config);

    let output = run_doctor(temp.path(), Some(&bin_dir));

    assert!(
        output.status.success(),
        "doctor should find each fixture executable: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    for command_name in command_names {
        assert!(
            stdout.contains(&format!("MCP server {command_name} command executable")),
            "missing executable diagnostic for {command_name}: {stdout}"
        );
    }
    assert!(stdout.contains("No issues found"), "{stdout}");
}

#[test]
fn doctor_reports_missing_and_extra_managed_gitignore_entries() {
    let temp = TempDir::new().unwrap();
    write_config(
        temp.path(),
        r#"
        [agents.claude]
        enabled = true

        [agents.claude.targets.instructions]
        source = "AGENTS.md"
        destination = "AGENTS.md"
        type = "symlink"
        "#,
    );
    fs::write(temp.path().join(".agents/AGENTS.md"), "# instructions\n").unwrap();
    fs::write(
        temp.path().join(".gitignore"),
        "# START AI Agent Symlinks\nmanual-entry\n# END AI Agent Symlinks\n",
    )
    .unwrap();

    let output = run_doctor(temp.path(), None);

    assert!(
        output.status.success(),
        "doctor diagnostics should not fail the command: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(".gitignore missing"), "{stdout}");
    assert!(
        stdout.contains(".gitignore has 1 extra entries"),
        "{stdout}"
    );
}

#[test]
fn doctor_reports_partial_managed_gitignore_markers_even_when_disabled() {
    let temp = TempDir::new().unwrap();
    write_config(temp.path(), "[gitignore]\nenabled = false\n");
    fs::write(
        temp.path().join(".gitignore"),
        "# START AI Agent Symlinks\nmanaged-entry\n",
    )
    .unwrap();

    let output = run_doctor(temp.path(), None);

    assert!(output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stdout).contains(".gitignore managed section missing"),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
}
