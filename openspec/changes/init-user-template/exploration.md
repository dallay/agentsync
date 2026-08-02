# Exploration: init --template and XDG user config

## Current State

`agentsync init` writes a hardcoded `DEFAULT_CONFIG` constant (src/init.rs:15, ~170 lines of TOML) containing all 7 agents enabled. Two code paths use it:

1. **`init()`** (init.rs:197) — plain init, writes `DEFAULT_CONFIG` directly at line 244: `fs::write(&config_path, DEFAULT_CONFIG)`
2. **`init_wizard()`** (init.rs:1583) — interactive wizard. Falls back to `init()` if no files found (line 1667) or no files selected (lines 1686, 1695). When it proceeds, it calls `build_default_config_with_skills_modes()` (line 1738) which iterates `DEFAULT_CONFIG.lines()` and patches `type =` values per agent. The wizard also appends a layout block via `upsert_agent_config_layout_block()`.
3. **`init_wizard_experimental_tui()`** — just shows a TUI intro then delegates to `init_wizard()`.

The `Init` command struct (main.rs:314) has 4 fields: `path`, `force`, `wizard`, `experimental_tui`. The handler (main.rs:398-426) branches on `wizard` flag.

## Affected Areas

- `src/main.rs:314-340` — Add `--template` arg to `Init` variant, pass it through handler (lines 398-419)
- `src/init.rs:197-261` — `init()` must accept optional template path, read+validate it, use instead of `DEFAULT_CONFIG`
- `src/init.rs:1075-1107` — `build_default_config_with_skills_modes()` takes `DEFAULT_CONFIG` as implicit base. Must accept a `&str` base parameter instead
- `src/init.rs:1583+` — `init_wizard()` calls `init()` as fallback (3 call sites: 1667, 1686, 1695) and `build_default_config_with_skills_modes()` at 1738. All need template threading
- `src/config.rs:290-298` — `Config::load()` already validates TOML → use it to validate user templates before writing
- `website/docs/src/content/docs/reference/cli.mdx:14-44` — Document `--template` flag and XDG fallback
- `Cargo.toml` — Possibly add `dirs` crate for XDG resolution

## Approaches

### 1. Minimal internal refactor — new `resolve_config_template()` function

- Add a `fn resolve_config_template(template: Option<&Path>) -> Result<String>` in init.rs that:
  1. If `--template` given: read file, validate with `Config::load` (parse as TOML), return content
  2. Else check `$XDG_CONFIG_HOME/agentsync/config.toml` then `~/.config/agentsync/config.toml`
  3. Else return `DEFAULT_CONFIG.to_string()`
- Thread resolved template string into `init()` and `build_default_config_with_skills_modes()`
- Pros: Single resolution point, clear precedence, minimal API change
- Cons: None significant
- Effort: **Low-Medium**

### 2. Config struct method approach

- Put resolution logic on `Config` or a new `TemplateResolver` struct
- Pros: More testable in isolation
- Cons: Over-engineering for what's essentially a string lookup
- Effort: **Medium**

## Recommendation

**Approach 1**. The change is localized. Key implementation:

1. Add `template: Option<PathBuf>` to `Init` in main.rs
2. Create `resolve_config_template(template: Option<&Path>) -> Result<String>` in init.rs
3. Change `init(project_root, force)` signature to `init(project_root, force, config_content: &str)`
4. Change `build_default_config_with_skills_modes(modes)` to accept `base_config: &str` instead of reading `DEFAULT_CONFIG`
5. Thread through all 3 wizard fallback calls to `init()` and the direct `build_default_config_with_skills_modes()` call

### XDG Resolution — no `dirs` crate needed

`std::env::home_dir()` is deprecated since Rust 1.29 but still works. However, this project already uses `std::env::var("HOME")` patterns implicitly. The cleanest approach:

```rust
fn user_config_path() -> Option<PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        let p = PathBuf::from(xdg).join("agentsync/config.toml");
        if p.exists() { return Some(p); }
    }
    if let Ok(home) = std::env::var("HOME") {
        let p = PathBuf::from(home).join(".config/agentsync/config.toml");
        if p.exists() { return Some(p); }
    }
    None
}
```

No new dependency needed. `$HOME` is reliable on macOS/Linux. Windows users would use `--template` explicitly.

### Validation strategy

Before writing the template content to disk, parse it with `toml::from_str::<Config>()` (same as `Config::load`). If it fails, error with: `"Invalid template: {path}: {toml_error}"`. This prevents writing broken configs.

### Output messaging

When using a template, change the init message:
- Default: `"  ✔ Created: .agents/agentsync.toml"`
- Template: `"  ✔ Created: .agents/agentsync.toml (from template: {path})"`
- XDG: `"  ✔ Created: .agents/agentsync.toml (from user config: ~/.config/agentsync/config.toml)"`

## Risks

- **Wizard + template interaction**: The wizard's `build_default_config_with_skills_modes()` iterates lines looking for `[agents.X]` sections and `type = ` lines. If a user template has different agent names or structure, the skills mode patching will silently miss agents not in the template. This is actually **correct behavior** — it only patches what exists.
- **E2E test `01-init-blank.sh`**: Asserts specific agents exist in output (`[agents.claude]`, `[agents.gemini]`, `[agents.opencode]`). Won't break since default path is unchanged, but we need NEW tests for template path.
- **Backward compatibility**: Fully preserved — no flag = same behavior as today.

## Test Strategy

### Existing tests
- **E2E**: `tests/e2e/scenarios/01-init-blank.sh` (blank repo init), `02-init-adoption.sh` (wizard migration)
- **Unit**: `Config::load` tests in config.rs (lines 604-710) — file not found, invalid TOML, find_config precedence
- **No unit tests** for `init()`, `init_wizard()`, or `build_default_config_with_skills_modes()`

### New tests needed
1. **Unit: `resolve_config_template`** — test precedence: explicit path > XDG > default
2. **Unit: `resolve_config_template` with invalid TOML** — expect error
3. **Unit: `build_default_config_with_skills_modes` with custom base** — verify it patches a non-default template correctly
4. **Integration: `init` with `--template`** — write a 2-agent template, run init, verify output matches template
5. **Integration: XDG fallback** — set `XDG_CONFIG_HOME` env var, place config, run init without `--template`, verify it's picked up
6. **E2E**: New scenario `03-init-template.sh` — end-to-end with template flag

## Open Questions

1. **Should `--template` work with `--wizard`?** Recommended: YES — template becomes the base config that the wizard patches. The wizard's skills mode selection still works because it patches `type =` lines in whatever template is provided.
2. **Template file naming**: Should we require the file to be named `config.toml` at XDG path, or accept any name? Recommendation: Fixed name `config.toml` at XDG, any name via `--template`.
3. **Should we print which source was used?** Recommended: YES, always show provenance in output for debuggability.

## Ready for Proposal

Yes — the approach is clear, risks are low, backward compat is preserved. The orchestrator should proceed to sdd-propose with Approach 1.
