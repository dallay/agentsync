---
name: agentsync
description: >
  Use AgentSync correctly and consistently to inspect repository agent setup, manage installable skills,
  and apply agent configuration changes. Trigger: when an agent needs to use the agentsync CLI, inspect
  AI agent symlinks, suggest/install/update skills, or initialize/sync AgentSync-managed configuration.
license: Apache-2.0
metadata:
  author: yuniel-acosta
  version: "1.0.0"
---

# AgentSync

Reusable operating guidance for AI agents working with the `agentsync` CLI in this repository and in
projects that use AgentSync-managed `.agents/` configuration.

## When to Use

- The task requires invoking `agentsync` directly.
- You need to inspect agent configuration status or diagnose sync problems.
- You need to recommend, install, update, or uninstall skills using `agentsync skill ...`.
- You need to initialize or apply AgentSync configuration in a project.

## Critical Patterns

### 1. Discover capabilities before acting

Do not assume subcommands or flags. Verify with CLI help first when behavior is uncertain.

Recommended discovery order:

1. `agentsync --help`
2. `agentsync <command> --help`
3. Prefer machine-readable output when available (`--json`)

### 2. Prefer machine-readable output for agent workflows

When an agent needs to reason about results programmatically, prefer JSON-capable commands:

- `agentsync status --json`
- `agentsync skill suggest --json`
- `agentsync skill install <skill-id> --json`
- `agentsync skill update <skill-id> --json`
- `agentsync skill uninstall <skill-id> --json`

Use human-readable output only when the user explicitly wants prose/terminal-style rendering.

### 3. Scope commands to the right project root

For `skill`, `status`, and `doctor`, pass `--project-root <path>` when operating outside the current
directory or when ambiguity exists.

For `init`, `apply`, and `clean`, use the path/config flags those commands actually support.

### 4. Use the correct command by intent

| Intent                                     | Command                                                           |
|--------------------------------------------|-------------------------------------------------------------------|
| Inspect managed symlink state              | `agentsync status [--json]`                                       |
| Diagnose setup problems                    | `agentsync doctor [--project-root <path>]`                        |
| Initialize config in a repo                | `agentsync init [-p <path>]`                                      |
| Sync configuration into target agent files | `agentsync apply [-p <path>] [-c <config>] [--dry-run]`           |
| Remove AgentSync-created symlinks          | `agentsync clean [-p <path>] [-c <config>] [--dry-run]`           |
| Recommend skills from repo context         | `agentsync skill suggest [--json]`                                |
| Install one known skill                    | `agentsync skill install <skill-id> [--source <source>] [--json]` |
| Update one installed skill                 | `agentsync skill update <skill-id> [--source <source>] [--json]`  |
| Remove one installed skill                 | `agentsync skill uninstall <skill-id> [--json]`                   |

### 5. Important limitation: do not rely on `skill list`

`agentsync skill list` appears in CLI help, but in the current implementation it is not implemented
and
returns an error. For installed-skill workflows, use the skill registry or other verified project
state
instead of depending on `skill list`.

### 6. Recommendation/install flow for agents

When the goal is “find good skills for this repo”, use this pattern:

1. Run `agentsync skill suggest --json`
2. Inspect `detections`, `recommendations`, and `summary`
3. If the user wants installation:
    - Interactive terminal available: `agentsync skill suggest --install`
    - Non-interactive automation: `agentsync skill suggest --install --all --json`

Do not invent recommendation ids. Use the returned `skill_id` values exactly.

### 7. Handle structured errors by code when available

Common JSON error codes from skill flows include:

- `suggest_error`
- `install_error`
- `update_error`
- `interactive_tty_required`
- `invalid_suggestion_selection`

When JSON is enabled, inspect `code` and `remediation` before retrying.

## Interaction Contract

When using AgentSync on behalf of a user, follow this sequence:

1. **Verify context**: confirm the project root and whether the repo already contains `.agents/`
2. **Discover**: inspect help or current state if command behavior is uncertain
3. **Preview when risky**: use `--dry-run` for `apply`/`clean` when the user wants safety first
4. **Execute the narrowest command** that solves the task
5. **Report concretely**:
    - command run
    - project root used
    - key result fields or summary
    - next action if follow-up is needed

## Commands

```bash
# Discover CLI surface
agentsync --help
agentsync skill --help

# Inspect repo state
agentsync status --json --project-root /path/to/repo
agentsync doctor --project-root /path/to/repo

# Initialize/configure a repo
agentsync init -p /path/to/repo
agentsync apply -p /path/to/repo --dry-run
agentsync apply -p /path/to/repo

# Skill recommendation flow
agentsync skill suggest --json --project-root /path/to/repo
agentsync skill suggest --install --all --json --project-root /path/to/repo

# Direct skill management
agentsync skill install dallay/agents-skills/nothing-design --json --project-root /path/to/repo
agentsync skill update nothing-design --json --project-root /path/to/repo
agentsync skill uninstall nothing-design --json --project-root /path/to/repo
```

## Decision Rules

- Need repository sync state? Use `status`, not `apply`.
- Need recommendations? Use `skill suggest`, not `skill install` blindly.
- Need deterministic automation? Prefer `--json` and avoid interactive flows.
- Need to remove links safely? Start with `clean --dry-run`.
- Need to apply config changes after editing `.agents/` content? Run `agentsync apply`.

## Anti-Patterns

- Do not assume `skill list` works.
- Do not parse human-readable output if `--json` exists.
- Do not install guessed skill ids; discover them first when possible.
- Do not run broad cleanup/apply operations without checking the target path.
- Do not assume current working directory is the intended project root.

## Resources

- Local project guidance: `.agents/AGENTS.md`
- Skill command implementation: `src/commands/skill.rs`
