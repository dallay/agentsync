# Tasks: Init User Template

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | 250–350 |
| 400-line budget risk | Low |
| Chained PRs recommended | No |
| Suggested split | single PR |
| Delivery strategy | ask-on-risk |
| Chain strategy | single-pr |

Decision needed before apply: No
Chained PRs recommended: No
Chain strategy: single-pr
400-line budget risk: Low

## Phase 1: Core Resolution Functions (Foundation)

- [x] 1.1 Add `resolve_user_config_path() -> Option<PathBuf>` in `src/init.rs` — checks `$XDG_CONFIG_HOME/agentsync/config.toml` then `$HOME/.config/agentsync/config.toml` (REQ-02, REQ-09)
- [x] 1.2 Add `resolve_config_template(template_path: Option<&Path>) -> Result<String>` in `src/init.rs` — precedence: explicit path → XDG → `DEFAULT_CONFIG`; hard-fail on explicit invalid, warn+fallback on XDG invalid (REQ-03, REQ-05, REQ-08)
- [x] 1.3 Write unit tests: explicit valid file returns content; explicit missing file errors with path; explicit invalid TOML errors with parse details; XDG found returns content; XDG invalid warns and returns `DEFAULT_CONFIG`; `HOME` unset skips gracefully; no XDG + no flag returns `DEFAULT_CONFIG` (REQ-02, REQ-03, REQ-05, REQ-08, REQ-09)

## Phase 2: CLI Wiring

- [x] 2.1 Add `template: Option<PathBuf>` field with `#[arg(long)]` to `Init` struct in `src/main.rs` (~line 334) (REQ-01)
- [x] 2.2 Pass `template_path` through handler at lines 398–419 to `init()` and `init_wizard()` calls (REQ-01)

## Phase 3: Init & Wizard Integration

- [x] 3.1 Change `init()` signature to accept `config_content: &str`, replace `DEFAULT_CONFIG` usage at line 244 with param (REQ-04)
- [x] 3.2 In the handler, call `resolve_config_template()` before `init()`, pass resolved content (REQ-03)
- [x] 3.3 Add provenance output: `"(from template: {path})"` or `"(from user config: {path})"` after config write (REQ-07)
- [x] 3.4 Change `init_wizard()` and `init_wizard_experimental_tui()` signatures to accept `template_path: Option<&Path>` (REQ-06)
- [x] 3.5 Change `build_default_config_with_skills_modes()` to accept `base_config: &str` instead of using `DEFAULT_CONFIG.lines()` (REQ-06)
- [x] 3.6 Thread template through wizard's 3 fallback `init()` calls (lines 1667, 1686, 1695) and `build_default_config_with_skills_modes()` call (line 1738) (REQ-06)

## Phase 4: Testing

- [x] 4.1 Integration test: `init --template valid.toml` writes template content with provenance message (REQ-01, REQ-07, REQ-10)
- [x] 4.2 Integration test: `init` without flag + XDG file present → uses XDG content (REQ-02)
- [x] 4.3 Integration test: `init` without flag, no XDG → writes `DEFAULT_CONFIG` (backward compat) (REQ-10)
- [x] 4.4 Integration test: `--template` overrides XDG when both exist (REQ-03)
- [ ] 4.5 Integration test: `init --wizard --template` uses template as wizard base (REQ-06)
- [x] 4.6 Verify existing E2E `01-init-blank.sh` and `02-init-adoption.sh` pass unchanged (REQ-10)

## Phase 5: Documentation

- [ ] 5.1 Update `website/docs/src/content/docs/reference/cli.mdx`: document `--template` flag and XDG auto-discovery
- [ ] 5.2 Update `website/docs/src/content/docs/getting-started.mdx`: mention user template option
- [ ] 5.3 Update `website/docs/src/content/docs/reference/configuration.mdx`: add user-level template section
