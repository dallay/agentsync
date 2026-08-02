# Init User Template Specification

## Purpose

User-level config template resolution for `agentsync init` — CLI flag, XDG auto-discovery fallback, TOML validation, and provenance output.

## Requirements

### Requirement: REQ-01 — Template flag

The system MUST accept a `--template <path>` flag on `agentsync init` that loads the specified file as the config template content.

#### Scenario: init with --template flag and valid file

- GIVEN a valid TOML file at `/tmp/my-config.toml` parseable as `Config`
- WHEN the user runs `agentsync init --template /tmp/my-config.toml`
- THEN `.agents/agentsync.toml` is written with the template file's content
- AND the output includes the template source path

#### Scenario: init with --template flag and missing file

- GIVEN no file exists at `/tmp/missing.toml`
- WHEN the user runs `agentsync init --template /tmp/missing.toml`
- THEN the command exits with a non-zero code
- AND the error message includes the path `/tmp/missing.toml`

#### Scenario: init with --template flag and invalid TOML

- GIVEN a file at `/tmp/bad.toml` containing invalid TOML or TOML not parseable as `Config`
- WHEN the user runs `agentsync init --template /tmp/bad.toml`
- THEN the command exits with a non-zero code
- AND the error message includes the file path and the parse error

### Requirement: REQ-02 — XDG discovery

When no `--template` flag is provided, the system MUST check `$XDG_CONFIG_HOME/agentsync/config.toml`, then `~/.config/agentsync/config.toml`. The first existing file is used as the template.

#### Scenario: init without --template, XDG config exists

- GIVEN `$XDG_CONFIG_HOME` is unset
- AND a valid config file exists at `~/.config/agentsync/config.toml`
- WHEN the user runs `agentsync init`
- THEN `.agents/agentsync.toml` is written with the XDG file's content
- AND the output indicates the XDG source

#### Scenario: XDG_CONFIG_HOME env var set to custom path

- GIVEN `$XDG_CONFIG_HOME` is set to `/tmp/custom-xdg`
- AND a valid config file exists at `/tmp/custom-xdg/agentsync/config.toml`
- WHEN the user runs `agentsync init`
- THEN `.agents/agentsync.toml` uses that file as template

#### Scenario: HOME not set (graceful fallback)

- GIVEN `$XDG_CONFIG_HOME` is unset and `$HOME` is unset
- AND no `--template` flag is provided
- WHEN the user runs `agentsync init`
- THEN `.agents/agentsync.toml` is written with `DEFAULT_CONFIG`

### Requirement: REQ-03 — Precedence order

The system MUST resolve templates in this order: `--template` > XDG discovery > `DEFAULT_CONFIG`.

#### Scenario: --template overrides XDG config

- GIVEN a valid XDG config at `~/.config/agentsync/config.toml` with 3 agents
- AND a valid template file at `/tmp/two-agents.toml` with 2 agents
- WHEN the user runs `agentsync init --template /tmp/two-agents.toml`
- THEN `.agents/agentsync.toml` contains the 2-agent config from `--template`

#### Scenario: init without --template, no XDG config

- GIVEN no XDG config file exists and no `--template` flag
- WHEN the user runs `agentsync init`
- THEN `.agents/agentsync.toml` is written with `DEFAULT_CONFIG` (7 agents)

### Requirement: REQ-04 — Full file replacement

Templates MUST be used as full file replacement. The system MUST NOT merge template content with `DEFAULT_CONFIG`.

### Requirement: REQ-05 — Template validation

Template files MUST be valid TOML parseable as `Config` via the existing deserialization. The system MUST error before writing if validation fails.

### Requirement: REQ-06 — Wizard compatibility

`--template` MUST work with `init --wizard`. The template becomes the base config that the wizard's skills-mode patching operates on.

#### Scenario: init --wizard with --template

- GIVEN a valid template at `/tmp/custom.toml`
- WHEN the user runs `agentsync init --wizard --template /tmp/custom.toml`
- THEN the wizard uses the template as its base config for skills-mode patching

#### Scenario: init --wizard without --template, XDG exists

- GIVEN a valid XDG config at `~/.config/agentsync/config.toml`
- WHEN the user runs `agentsync init --wizard`
- THEN the wizard uses the XDG config as its base

### Requirement: REQ-07 — Provenance output

When a template is used (via flag or XDG), the output message MUST indicate which source was used.

### Requirement: REQ-08 — Missing template error

If `--template` points to a nonexistent path, the system MUST exit with a clear error including the path. It MUST NOT fall back to XDG or `DEFAULT_CONFIG`.

### Requirement: REQ-09 — No new dependencies

This change MUST NOT add new crate dependencies. XDG resolution uses `std::env::var` only.

### Requirement: REQ-10 — Backward compatibility

Without `--template` flag and without an XDG config file, behavior MUST be identical to the current implementation.

## Acceptance Criteria

- All 10 requirements have passing tests (unit + integration)
- All existing E2E tests (`01-init-blank.sh`, `02-init-adoption.sh`) pass unchanged
- `cargo clippy` and `cargo fmt` pass
- `Cargo.toml` has no new dependencies
- CLI help text documents `--template` flag

## Out of Scope

- Partial/sparse config merge with `DEFAULT_CONFIG`
- `--default-agents` flag
- `dirs` crate or any new dependency for XDG resolution
- Windows-specific XDG path discovery
- Template generation or scaffolding commands
