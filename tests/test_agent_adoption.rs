//! Integration tests for agent adoption flow
//!
//! Simulates the real-world scenario from GitHub #256:
//! A user has an existing repo with agent files (Claude, Gemini, Codex, etc.)
//! and adopts AgentSync. These tests verify the full init → config → apply
//! pipeline works correctly, producing proper symlinks from .agents/ sources.

use agentsync::config::Config;
use agentsync::linker::{Linker, SyncOptions};
use anyhow::Result;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

/// Helper: create a file with parent directories
fn create_file(base: &Path, relative: &str, content: &str) {
    let path = base.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(&path, content).unwrap();
}

/// Helper: assert a symlink exists and points to expected target substring
fn assert_symlink_points_to(base: &Path, link_path: &str, target_contains: &str) {
    let full = base.join(link_path);
    assert!(full.exists(), "Expected symlink at {link_path} to exist");
    assert!(
        full.symlink_metadata().unwrap().file_type().is_symlink(),
        "Expected {link_path} to be a symlink"
    );
    let target = fs::read_link(&full).unwrap();
    let target_str = target.to_string_lossy();
    assert!(
        target_str.contains(target_contains),
        "Expected symlink {link_path} to point to something containing '{target_contains}', got '{target_str}'"
    );
}

/// A target definition: (target_name, source, destination, sync_type)
type TargetDef<'a> = (&'a str, &'a str, &'a str, &'a str);

/// Helper: write a config with skills and commands targets for a given agent
fn write_adoption_config(agents_dir: &Path, agents: &[(&str, Vec<TargetDef<'_>>)]) {
    let mut toml = String::from("source_dir = \".\"\n\n[gitignore]\nenabled = false\n\n");

    for (agent_name, targets) in agents {
        toml.push_str(&format!("[agents.{agent_name}]\nenabled = true\n\n"));
        for (target_name, source, destination, sync_type) in targets {
            toml.push_str(&format!(
                "[agents.{agent_name}.targets.{target_name}]\n\
                 source = \"{source}\"\n\
                 destination = \"{destination}\"\n\
                 type = \"{sync_type}\"\n\n"
            ));
        }
    }

    fs::write(agents_dir.join("agentsync.toml"), toml).unwrap();
}

// ---------------------------------------------------------------------------
// Test: Existing Claude repo with CLAUDE.md + .claude/skills/ → adopt → apply
// ---------------------------------------------------------------------------
#[test]
fn test_adoption_claude_with_skills_and_instructions() -> Result<()> {
    let tmp = TempDir::new()?;
    let root = tmp.path();

    // 1. Simulate existing Claude repo
    create_file(root, "CLAUDE.md", "# Claude Instructions\nBe helpful.");
    create_file(
        root,
        ".claude/skills/debugging/SKILL.md",
        "---\nname: debugging\nversion: 1.0.0\n---\n# Debugging skill",
    );
    create_file(
        root,
        ".claude/skills/testing/SKILL.md",
        "---\nname: testing\nversion: 1.0.0\n---\n# Testing skill",
    );
    create_file(
        root,
        ".claude/commands/review.md",
        "# Review command\nReview the code.",
    );

    // 2. Run init (creates .agents/ structure)
    agentsync::init::init(root, false)?;

    // 3. Simulate wizard migration: copy skills and commands into .agents/
    let agents_skills = root.join(".agents/skills");
    let agents_commands = root.join(".agents/commands");
    fs::create_dir_all(&agents_commands)?;

    // Copy skills
    for skill in &["debugging", "testing"] {
        let src = root.join(format!(".claude/skills/{skill}"));
        let dst = agents_skills.join(skill);
        fs::create_dir_all(&dst)?;
        for entry in fs::read_dir(&src)? {
            let entry = entry?;
            fs::copy(entry.path(), dst.join(entry.file_name()))?;
        }
    }

    // Copy commands
    fs::copy(
        root.join(".claude/commands/review.md"),
        agents_commands.join("review.md"),
    )?;

    // Copy CLAUDE.md content into AGENTS.md
    let claude_content = fs::read_to_string(root.join("CLAUDE.md"))?;
    fs::write(root.join(".agents/AGENTS.md"), &claude_content)?;

    // 4. Write config with skills + commands + instructions targets
    write_adoption_config(
        &root.join(".agents"),
        &[(
            "claude",
            vec![
                ("instructions", "AGENTS.md", "CLAUDE.md", "symlink"),
                ("skills", "skills", ".claude/skills", "symlink"),
                (
                    "commands",
                    "commands",
                    ".claude/commands",
                    "symlink-contents",
                ),
            ],
        )],
    );

    // 5. Remove original .claude/ to prove symlinks recreate them
    fs::remove_dir_all(root.join(".claude"))?;
    fs::remove_file(root.join("CLAUDE.md")).ok(); // may not exist as symlink yet

    // 6. Apply
    let config_path = root.join(".agents/agentsync.toml");
    let config = Config::load(&config_path)?;
    let linker = Linker::new(config, config_path);
    let result = linker.sync(&SyncOptions::default())?;

    // 7. Verify
    assert!(
        result.errors == 0,
        "Expected no errors, got {}",
        result.errors
    );
    assert!(result.created > 0, "Expected symlinks to be created");

    // CLAUDE.md symlink
    assert_symlink_points_to(root, "CLAUDE.md", "AGENTS.md");

    // Skills: directory symlink pointing to skills source
    assert_symlink_points_to(root, ".claude/skills", "skills");
    assert!(root.join(".claude/skills/debugging/SKILL.md").exists());
    assert!(root.join(".claude/skills/testing/SKILL.md").exists());

    // Commands symlink
    assert_symlink_points_to(root, ".claude/commands/review.md", "review.md");

    Ok(())
}

// ---------------------------------------------------------------------------
// Test: Existing Gemini repo with GEMINI.md + skills + commands → adopt → apply
// ---------------------------------------------------------------------------
#[test]
fn test_adoption_gemini_with_skills_and_commands() -> Result<()> {
    let tmp = TempDir::new()?;
    let root = tmp.path();

    // 1. Simulate existing Gemini repo
    create_file(root, "GEMINI.md", "# Gemini Instructions");
    create_file(
        root,
        ".gemini/skills/code-review/SKILL.md",
        "---\nname: code-review\nversion: 1.0.0\n---\n# Code review skill",
    );
    create_file(root, ".gemini/commands/analyze.md", "# Analyze command");

    // 2. Init
    agentsync::init::init(root, false)?;

    // 3. Simulate wizard migration
    let agents_skills = root.join(".agents/skills");
    let agents_commands = root.join(".agents/commands");
    fs::create_dir_all(&agents_commands)?;

    let src_skill = root.join(".gemini/skills/code-review");
    let dst_skill = agents_skills.join("code-review");
    fs::create_dir_all(&dst_skill)?;
    fs::copy(src_skill.join("SKILL.md"), dst_skill.join("SKILL.md"))?;

    fs::copy(
        root.join(".gemini/commands/analyze.md"),
        agents_commands.join("analyze.md"),
    )?;

    let gemini_content = fs::read_to_string(root.join("GEMINI.md"))?;
    fs::write(root.join(".agents/AGENTS.md"), &gemini_content)?;

    // 4. Config
    write_adoption_config(
        &root.join(".agents"),
        &[(
            "gemini",
            vec![
                ("instructions", "AGENTS.md", "GEMINI.md", "symlink"),
                ("skills", "skills", ".gemini/skills", "symlink"),
                (
                    "commands",
                    "commands",
                    ".gemini/commands",
                    "symlink-contents",
                ),
            ],
        )],
    );

    // 5. Remove originals
    fs::remove_dir_all(root.join(".gemini"))?;
    fs::remove_file(root.join("GEMINI.md")).ok();

    // 6. Apply
    let config_path = root.join(".agents/agentsync.toml");
    let config = Config::load(&config_path)?;
    let linker = Linker::new(config, config_path);
    let result = linker.sync(&SyncOptions::default())?;

    // 7. Verify
    assert!(result.errors == 0);
    assert_symlink_points_to(root, "GEMINI.md", "AGENTS.md");
    assert_symlink_points_to(root, ".gemini/skills", "skills");
    assert!(root.join(".gemini/skills/code-review/SKILL.md").exists());
    assert_symlink_points_to(root, ".gemini/commands/analyze.md", "analyze.md");

    Ok(())
}

// ---------------------------------------------------------------------------
// Test: Existing Codex repo with .codex/skills/ → adopt → apply
// ---------------------------------------------------------------------------
#[test]
fn test_adoption_codex_with_skills() -> Result<()> {
    let tmp = TempDir::new()?;
    let root = tmp.path();

    // 1. Simulate existing Codex repo
    create_file(root, "AGENTS.md", "# Codex Instructions");
    create_file(
        root,
        ".codex/skills/linting/SKILL.md",
        "---\nname: linting\nversion: 1.0.0\n---\n# Linting skill",
    );

    // 2. Init
    agentsync::init::init(root, false)?;

    // 3. Simulate wizard migration
    let agents_skills = root.join(".agents/skills");
    let dst_skill = agents_skills.join("linting");
    fs::create_dir_all(&dst_skill)?;
    fs::copy(
        root.join(".codex/skills/linting/SKILL.md"),
        dst_skill.join("SKILL.md"),
    )?;

    fs::write(root.join(".agents/AGENTS.md"), "# Codex Instructions")?;

    // 4. Config
    write_adoption_config(
        &root.join(".agents"),
        &[(
            "codex",
            vec![("skills", "skills", ".codex/skills", "symlink")],
        )],
    );

    // 5. Remove originals
    fs::remove_dir_all(root.join(".codex"))?;

    // 6. Apply
    let config_path = root.join(".agents/agentsync.toml");
    let config = Config::load(&config_path)?;
    let linker = Linker::new(config, config_path);
    let result = linker.sync(&SyncOptions::default())?;

    // 7. Verify
    assert!(result.errors == 0);
    assert_symlink_points_to(root, ".codex/skills", "skills");
    assert!(root.join(".codex/skills/linting/SKILL.md").exists());

    Ok(())
}

// ---------------------------------------------------------------------------
// Test: Multi-agent repo (Claude + Gemini + Codex) → adopt all → apply
// ---------------------------------------------------------------------------
#[test]
fn test_adoption_multi_agent_claude_gemini_codex() -> Result<()> {
    let tmp = TempDir::new()?;
    let root = tmp.path();

    // 1. Simulate multi-agent repo
    create_file(root, "CLAUDE.md", "# Claude");
    create_file(
        root,
        ".claude/skills/debugging/SKILL.md",
        "---\nname: debugging\nversion: 1.0.0\n---\n# Debug",
    );
    create_file(root, ".claude/commands/fix.md", "# Fix command");
    create_file(root, "GEMINI.md", "# Gemini");
    create_file(
        root,
        ".gemini/skills/review/SKILL.md",
        "---\nname: review\nversion: 1.0.0\n---\n# Review",
    );
    create_file(
        root,
        ".codex/skills/format/SKILL.md",
        "---\nname: format\nversion: 1.0.0\n---\n# Format",
    );

    // 2. Init
    agentsync::init::init(root, false)?;

    // 3. Simulate wizard migration — merge all skills into .agents/skills/
    let agents_skills = root.join(".agents/skills");
    let agents_commands = root.join(".agents/commands");
    fs::create_dir_all(&agents_commands)?;

    for (agent_dir, skill_name) in &[
        (".claude/skills/debugging", "debugging"),
        (".gemini/skills/review", "review"),
        (".codex/skills/format", "format"),
    ] {
        let dst = agents_skills.join(skill_name);
        fs::create_dir_all(&dst)?;
        fs::copy(root.join(agent_dir).join("SKILL.md"), dst.join("SKILL.md"))?;
    }

    fs::copy(
        root.join(".claude/commands/fix.md"),
        agents_commands.join("fix.md"),
    )?;

    // Merge instructions
    let merged = format!(
        "{}\n\n{}\n",
        fs::read_to_string(root.join("CLAUDE.md"))?,
        fs::read_to_string(root.join("GEMINI.md"))?,
    );
    fs::write(root.join(".agents/AGENTS.md"), &merged)?;

    // 4. Config for all three agents
    write_adoption_config(
        &root.join(".agents"),
        &[
            (
                "claude",
                vec![
                    ("instructions", "AGENTS.md", "CLAUDE.md", "symlink"),
                    ("skills", "skills", ".claude/skills", "symlink"),
                    (
                        "commands",
                        "commands",
                        ".claude/commands",
                        "symlink-contents",
                    ),
                ],
            ),
            (
                "gemini",
                vec![
                    ("instructions", "AGENTS.md", "GEMINI.md", "symlink"),
                    ("skills", "skills", ".gemini/skills", "symlink"),
                ],
            ),
            (
                "codex",
                vec![("skills", "skills", ".codex/skills", "symlink")],
            ),
            (
                "opencode",
                vec![("skills", "skills", ".opencode/skills", "symlink")],
            ),
            (
                "copilot",
                vec![("skills", "skills", ".github/skills", "symlink")],
            ),
        ],
    );

    // 5. Remove originals
    fs::remove_dir_all(root.join(".claude"))?;
    fs::remove_dir_all(root.join(".gemini"))?;
    fs::remove_dir_all(root.join(".codex"))?;
    fs::remove_file(root.join("CLAUDE.md")).ok();
    fs::remove_file(root.join("GEMINI.md")).ok();

    // 6. Apply
    let config_path = root.join(".agents/agentsync.toml");
    let config = Config::load(&config_path)?;
    let linker = Linker::new(config, config_path);
    let result = linker.sync(&SyncOptions::default())?;

    // 7. Verify — all agents get their symlinks
    assert!(
        result.errors == 0,
        "Expected no errors, got {}",
        result.errors
    );

    // Claude
    assert_symlink_points_to(root, "CLAUDE.md", "AGENTS.md");
    assert_symlink_points_to(root, ".claude/skills", "skills");
    assert!(root.join(".claude/skills/debugging").exists());
    assert!(root.join(".claude/skills/review").exists());
    assert!(root.join(".claude/skills/format").exists());
    assert_symlink_points_to(root, ".claude/commands/fix.md", "fix.md");

    // Gemini
    assert_symlink_points_to(root, "GEMINI.md", "AGENTS.md");
    assert_symlink_points_to(root, ".gemini/skills", "skills");
    assert!(root.join(".gemini/skills/debugging").exists());
    assert!(root.join(".gemini/skills/review").exists());
    assert!(root.join(".gemini/skills/format").exists());

    // Codex
    assert_symlink_points_to(root, ".codex/skills", "skills");
    assert!(root.join(".codex/skills/debugging").exists());
    assert!(root.join(".codex/skills/review").exists());
    assert!(root.join(".codex/skills/format").exists());

    // OpenCode
    assert_symlink_points_to(root, ".opencode/skills", "skills");
    assert!(root.join(".opencode/skills/debugging").exists());

    // Copilot
    assert_symlink_points_to(root, ".github/skills", "skills");
    assert!(root.join(".github/skills/debugging").exists());

    Ok(())
}

// ---------------------------------------------------------------------------
// Test: Adoption with dry-run shows actions without creating symlinks
// ---------------------------------------------------------------------------
#[test]
fn test_adoption_dry_run_no_side_effects() -> Result<()> {
    let tmp = TempDir::new()?;
    let root = tmp.path();

    // 1. Init and set up skills
    agentsync::init::init(root, false)?;
    create_file(
        root,
        ".agents/skills/my-skill/SKILL.md",
        "---\nname: my-skill\n---\n# Skill",
    );

    // 2. Config
    write_adoption_config(
        &root.join(".agents"),
        &[(
            "claude",
            vec![("skills", "skills", ".claude/skills", "symlink")],
        )],
    );

    // 3. Apply with dry-run
    let config_path = root.join(".agents/agentsync.toml");
    let config = Config::load(&config_path)?;
    let linker = Linker::new(config, config_path);
    let result = linker.sync(&SyncOptions {
        dry_run: true,
        ..Default::default()
    })?;

    // 4. Verify — no symlinks created
    assert!(result.errors == 0);
    assert!(
        !root.join(".claude/skills").exists(),
        "Dry-run should not create symlinks"
    );

    Ok(())
}
