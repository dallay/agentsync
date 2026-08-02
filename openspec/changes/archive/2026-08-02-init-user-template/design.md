# Design: Init User Template

## Technical Approach

Add a single `resolve_config_template()` function in `src/init.rs` as the resolution point for config template content. Thread the resolved string through `init()`, `init_wizard()`, and `build_default_config_with_skills_modes()` by changing their signatures. XDG discovery uses `std::env::var` — no new dependencies.

## Architecture Decisions

| Decision | Alternatives | Rationale |
|----------|-------------|-----------|
| Single `resolve_config_template()` fn in init.rs | Config method, TemplateResolver struct | It's a string lookup with fallback — a standalone fn is simplest. No state to manage. |
| No `dirs` crate for XDG | Add `dirs` or `directories` crate | `std::env::var("HOME")` + `XDG_CONFIG_HOME` is sufficient. Project already uses env vars directly. Windows users use `--template`. |
| Warn-and-fallback on invalid XDG file | Hard-fail on any invalid template | Explicit `--template` = user intent = hard-fail. Implicit XDG discovery = convenience = graceful degradation. |
| Pass `base_config: &str` to `build_default_config_with_skills_modes` | Global mutable state, config object | Minimal signature change, keeps function pure. |

## Data Flow

```
CLI --template flag
        │
        ▼
resolve_config_template(template_path)
  ├─ --template given? → read file → validate TOML → return content
  ├─ XDG file exists?  → read file → validate (warn on fail) → return content
  └─ neither?          → return DEFAULT_CONFIG
        │
        ▼
  config_content: String
        │
   ┌────┴────┐
   │         │
 init()   init_wizard()
   │         │
   │    build_default_config_with_skills_modes(base_config, modes)
   │         │
   ▼         ▼
 fs::write(config_path, content)
```

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `src/main.rs` | Modify | Add `template: Option<PathBuf>` to `Init` struct (~line 334). Pass to `init()`/`init_wizard()` at lines 412-418. |
| `src/init.rs` | Modify | New `resolve_config_template()` + `resolve_user_config_path()` fns. Change `init()` (line 197): add `config_content: &str` param, replace `DEFAULT_CONFIG` at line 244. Change `build_default_config_with_skills_modes()` (line 1075): add `base_config: &str` param, replace `DEFAULT_CONFIG.lines()` at line 1079. Change `init_wizard()` (line 1583): add `template_path: Option<&Path>`, resolve template, thread to `init()` calls (lines 1667, 1686, 1695) and `build_default_config_with_skills_modes()` (line 1738). Same for `init_wizard_experimental_tui()`. |
| `website/docs/.../cli.mdx` | Modify | Document `--template` flag and XDG fallback behavior. |

## Interfaces / Contracts

```rust
/// Resolve config template content by precedence:
/// 1. Explicit --template path (hard error if invalid)
/// 2. XDG user config (warn + fallback if invalid)
/// 3. DEFAULT_CONFIG
pub fn resolve_config_template(template_path: Option<&Path>) -> Result<String>

/// Check XDG paths for user config. Returns None if nothing found.
fn resolve_user_config_path() -> Option<PathBuf>

// Changed signatures:
pub fn init(project_root: &Path, force: bool, config_content: &str) -> Result<()>
pub fn init_wizard(project_root: &Path, force: bool, template_path: Option<&Path>) -> Result<()>
pub fn init_wizard_experimental_tui(project_root: &Path, force: bool, template_path: Option<&Path>) -> Result<()>
fn build_default_config_with_skills_modes(base_config: &str, modes: &BTreeMap<String, SyncType>) -> String
```

XDG resolution logic:
```rust
fn resolve_user_config_path() -> Option<PathBuf> {
    // 1. $XDG_CONFIG_HOME/agentsync/config.toml
    // 2. $HOME/.config/agentsync/config.toml
    // Returns None if neither exists
}
```

## Testing Strategy

| Layer | What to Test | Approach |
|-------|-------------|----------|
| Unit | `resolve_config_template()` — all 3 precedence cases | Temp files + env var override |
| Unit | `resolve_user_config_path()` — env var combinations | Set/unset `XDG_CONFIG_HOME`, `HOME` |
| Unit | `build_default_config_with_skills_modes()` with custom base | Pass non-default TOML, verify patching |
| Unit | Invalid TOML via `--template` → error | Assert anyhow error with context |
| Unit | Invalid XDG file → warn + fallback | Assert returns `DEFAULT_CONFIG` content |
| Integration | `init --template my.toml` | Temp dir, write template, run init, verify output |
| Integration | `init --wizard --template` | Verify wizard uses template as base |
| Integration | XDG auto-discovery | Set env, place file, run init without flag |
| Existing | `test_default_agents_md_contains_sections` | Unaffected — AGENTS.md unchanged |
| Existing | E2E `01-init-blank.sh` | Unaffected — no flag = same behavior |

## Migration / Rollout

No migration required. Purely additive feature — no flag means identical behavior to current code.

## Open Questions

- [x] Should `--template` work with `--wizard`? → YES, template becomes wizard base config
- [x] XDG file naming? → Fixed `config.toml`, any name via `--template`
- [ ] Should we validate template with full `Config::load` deserialization or just TOML parse? Recommendation: full `Config::load` to catch semantic errors (unknown agent names, invalid sync types).
