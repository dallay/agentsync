# Proposal: Init User Template

## Intent

Users who manage multiple repos need consistent agent configs. Today, `agentsync init` always writes a hardcoded 7-agent default, forcing manual edits every time. This change lets users define their preferred config once and reuse it via `--template` flag or XDG auto-discovery.

GitHub issue: #478

## Scope

### In Scope
- `--template <path>` CLI flag on `agentsync init`
- XDG auto-discovery fallback (`$XDG_CONFIG_HOME/agentsync/config.toml` → `~/.config/agentsync/config.toml`)
- Precedence: `--template` > XDG user config > hardcoded `DEFAULT_CONFIG`
- Template validation before writing (parse as TOML, reject invalid)
- Provenance messaging in output (show which source was used)
- Template support in both `init` and `init --wizard` paths
- Unit + integration tests for precedence, validation, error cases
- Documentation updates (cli.mdx, getting-started.mdx, configuration.mdx)

### Out of Scope
- Partial/sparse config merge (template is full file replacement)
- `--default-agents` flag (covered by template mechanism)
- New crate dependencies (use `$HOME` + `$XDG_CONFIG_HOME` env vars directly)
- Windows XDG support (Windows users use `--template` explicitly)
- Template generation/scaffolding commands

## Capabilities

### New Capabilities
- `init-user-template`: User-level config template resolution for `agentsync init` — flag, XDG fallback, validation, and provenance output

### Modified Capabilities
- `config-schema`: No schema changes — templates must conform to existing schema. Validation reuses `Config::load` parsing.

## Approach

Add `resolve_config_template(template: Option<&Path>) -> Result<String>` in `init.rs` as the single resolution point. Thread the resolved string into `init()` and `build_default_config_with_skills_modes()` by changing their signatures to accept a `base_config: &str` parameter. XDG lookup uses `std::env::var` for `XDG_CONFIG_HOME` and `HOME` — no new dependencies.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `src/main.rs:314-340` | Modified | Add `template: Option<PathBuf>` to `Init` command struct |
| `src/init.rs:197-261` | Modified | `init()` accepts config content param instead of using `DEFAULT_CONFIG` |
| `src/init.rs:1075-1107` | Modified | `build_default_config_with_skills_modes()` accepts `base_config: &str` |
| `src/init.rs:1583+` | Modified | `init_wizard()` threads template through 3 fallback calls + direct build call |
| `src/init.rs` (new fn) | New | `resolve_config_template()` + `user_config_path()` |
| `tests/` | New | Unit tests for resolution, integration tests for `--template` and XDG |
| `website/docs/.../cli.mdx` | Modified | Document `--template` flag and XDG behavior |
| `website/docs/.../getting-started.mdx` | Modified | Mention template option |
| `website/docs/.../configuration.mdx` | Modified | Add user template section |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Wizard line-patching misses agents not in template | Low | Correct behavior — only patches what exists. Document this. |
| User provides syntactically valid but semantically broken template | Low | Validate with `Config::load` (full deserialization), not just TOML parse |
| XDG path doesn't exist on CI/containers | Low | Graceful fallback to `DEFAULT_CONFIG` — no error if XDG file absent |

## Rollback Plan

Revert the `--template` field from `Init` struct and restore original `init()`/`build_default_config_with_skills_modes()` signatures. No data migration needed — templates are read-only inputs, nothing is persisted beyond the generated `agentsync.toml` (which is always overwritable via `init --force`).

## Dependencies

- None. No new crates. Uses existing `Config::load` for validation.

## Success Criteria

- [ ] `agentsync init --template my.toml` writes config from template with provenance message
- [ ] `agentsync init` without flag picks up `~/.config/agentsync/config.toml` when present
- [ ] `agentsync init` without flag or XDG file behaves identically to current behavior
- [ ] `agentsync init --wizard --template my.toml` uses template as wizard base
- [ ] Invalid template produces clear error with file path and parse error
- [ ] All existing E2E tests pass unchanged
- [ ] New unit + integration tests cover precedence chain and error cases
