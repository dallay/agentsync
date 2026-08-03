# Exploration: GitHub issues #494 and #500 together

### Current State


Issue #494: `src/main.rs` is both CLI orchestration and presentation layer. It already imports `src/output.rs` for `OutputMode`, `HumanFormatter`, `LabelKind`, and `output_mode`, but retains nearly all apply/clean presentation helpers: `render_phase`, `render_dry_run_notice`, `render_clean_phase_with_color`, `render_sync_phase_with_color`, `render_gitignore_phase_with_color`, `render_mcp_phase`, `render_count`, `render_apply_summary_with_color`, `render_clean_summary_with_color`, `render_mcp_summary_with_color`, `print_lines`, `print_header`, and `init_next_steps_lines` (roughly lines 36–282 and 664–667). `handle_apply`, `handle_clean`, `handle_init`, `handle_apply_gitignore`, and `handle_apply_mcp` orchestrate filesystem/config work and call those renderers. `src/commands/status.rs` independently owns status rendering (`render_status_entry`, `render_status_hint`, `render_status_summary`) and JSON serialization (`serde_json::to_string_pretty`), while `src/output.rs` currently only owns output-mode detection and formatter primitives. Human output is line-oriented, uses `colored`, and disables color when JSON, stdout is not a TTY, `NO_COLOR` is set, `CLICOLOR=0`, or `TERM=dumb`. Status JSON is an array of `StatusEntry`; non-zero problems exit with code 1. Apply/clean do not have a JSON mode and print directly from orchestration and `init` itself.

Issue #500: MCP support is structurally centralized in `src/mcp.rs` through `McpAgent` and its methods `all`, `id`, `name`, `config_path`, `resolved_config_path`, `is_global`, `formatter`, and `from_id`. The current canonical runtime list is eight agents: Claude Code, Claude Desktop, GitHub Copilot, Codex CLI, Gemini CLI, VS Code, Cursor, and OpenCode. `src/agent_ids.rs` separately contains canonical IDs/aliases for those MCP agents and 25+ configurable-only agents, plus convention filenames and ignore patterns. Documentation is duplicated in `README.md`, `npm/agentsync/README.md`, `website/docs/src/content/docs/guides/mcp.mdx`, `website/docs/src/content/docs/reference/configuration.mdx`, and `openspec/specs/mcp-generation/spec.md`; the OpenSpec MCP table is already stale because it omits Claude Desktop. README explicitly says its list is canonical while also saying `src/mcp.rs` is authoritative, and website README instructs maintainers to cross-check source manually. There is no dedicated `.github/workflows` documentation-drift validation job beyond the existing Rust check/fmt/clippy/test/build/audit/E2E jobs. No scripts currently generate or validate the lists.

### Affected Areas

- `src/main.rs` — orchestration mixed with apply/clean/init human rendering; extraction must preserve exact line order, labels, spacing, color behavior, and test helper behavior.
- `src/output.rs` — existing output-mode/formatter boundary is the natural home for reusable human renderers and possibly output emission helpers.
- `src/commands/status.rs` — separate human/JSON status presentation; avoid changing its JSON contract or accidentally coupling status domain validation to generic output formatting.
- `src/commands/status_tests.rs` and `src/main.rs` tests — existing renderer assertions are mostly unit-level and currently private to the binary crate; move tests with symbols or add equivalent output-contract tests.
- `src/mcp.rs` — runtime MCP registry and agent metadata are currently spread across enum methods and formatter dispatch; likely canonical source for MCP agent documentation metadata, but Claude Desktop global-path behavior must remain explicit.
- `src/agent_ids.rs` — second agent registry for aliases, configurable-only agents, convention filenames, and ignore patterns; any “all supported agents” definition must distinguish native MCP agents from configurable sync agents.
- `src/linker.rs` / `src/config.rs` — consume MCP IDs and filters and generate known ignore patterns; metadata changes must not alter selection or filesystem behavior.
- `README.md` and `npm/agentsync/README.md` — duplicated MCP/native and broader supported-agent claims; README has conflicting “canonical” language.
- `website/docs/src/content/docs/guides/mcp.mdx` — detailed MCP table, global-agent explanation, and behavior; currently includes all eight MCP agents.
- `website/docs/src/content/docs/reference/configuration.mdx` — known-ignore-pattern table, currently only a partial list (through windsurf in the inspected section), not equivalent to MCP support.
- `openspec/specs/mcp-generation/spec.md` — durable MCP spec table and code references; currently omits Claude Desktop from the supported-agent table and should be treated as a governed artifact, not an independently hand-maintained runtime registry.
- `.github/workflows/ci.yml` — existing CI entry point; add a focused, fast drift-validation step/job rather than coupling docs validation to full Rust builds.
- `scripts/` — only setup/version scripts exist; a small deterministic validator/generator could live here, or validation could be a Rust test if metadata is exposed safely.

### Approaches

1. **Canonical Rust metadata + generated documentation fragments** — Extend `McpAgent` (or a dedicated public metadata table adjacent to it) with stable ID, display name, destination, format, global flag, and notes; generate/check marked fragments in README/docs/OpenSpec/npm README via a Rust or small script tool.
   - Pros: runtime and docs derive from the same typed registry; prevents omissions such as Claude Desktop; preserves enum/formatter behavior; CI can fail deterministically.
   - Cons: requires choosing generated-fragment markers and handling Markdown table rendering; broader configurable-agent list still needs a separate canonical registry or explicit scope.
   - Effort: Medium.

2. **Data file as canonical source + code/docs generation** — Create a versioned TOML/JSON/YAML manifest containing native MCP and configurable agent metadata, load it for runtime or build-time generation, and generate docs from it.
   - Pros: easy for tooling and documentation generation; one source can cover MCP paths, aliases, configurable agents, and ignore patterns.
   - Cons: runtime enum methods currently encode formatter dispatch and global path semantics; moving behavior into data risks invalid combinations and weak typing; larger migration and more behavior-change risk.
   - Effort: High.

3. **Docs validator only, with runtime list unchanged** — Keep `McpAgent::all()` and `agent_ids.rs` as code sources, parse selected Markdown tables in CI, and compare their IDs/rows against source-derived expected values.
   - Pros: smallest product change; directly addresses drift and can remove manual keep-in-sync wording; low implementation risk.
   - Cons: still has multiple code registries and duplicated metadata; parser/format changes can make CI brittle; cannot make README, npm README, docs, and OpenSpec share one canonical representation without maintaining comparison rules.
   - Effort: Low/Medium.

### Recommendation

Use Approach 1 for the native MCP surface, with a narrow explicit boundary: make one typed MCP metadata registry the canonical source for native MCP agent IDs, names, destinations, format classification, global status, and documentation notes; retain formatter dispatch and OS-specific path resolution as behavior in `McpAgent`. Generate or validate marked documentation fragments in `README.md`, `npm/agentsync/README.md`, `guides/mcp.mdx`, and `openspec/specs/mcp-generation/spec.md`, and add a fast CI validation job. Separately define whether #500’s “supported-agent” claim means native MCP agents only or all configurable agents. If it means all agents, extend the same registry concept to `agent_ids.rs`’s configurable IDs and metadata, but do not conflate native MCP capability with generic symlink support. Remove “keep in sync” prose once CI owns the invariant. For #494, move pure renderers and output helpers into `src/output.rs` while leaving command handlers and domain operations in `main.rs`; keep JSON serialization contracts in command modules unless a shared output abstraction is specifically justified.

### Risks

- Human output is an implicit CLI contract: moving functions can accidentally change whitespace, ANSI application order, blank lines, banner text, or summary labels; preserve current strings and add/retain exact renderer tests.
- `src/init.rs` writes user-facing progress directly, so a complete “presentation extraction” cannot be achieved by changing only `main.rs`; scope #494 to main-owned rendering first and explicitly leave operation-progress output for a follow-up if needed.
- `McpAgent::all()` currently includes Claude Desktop, but `openspec/specs/mcp-generation/spec.md` omits it; docs validation will surface an existing mismatch immediately and should update the spec artifact in the implementation phase.
- `src/agent_ids.rs` contains many configurable-only agents and aliases not represented by `McpAgent`; a single “supported agents” table can be semantically wrong unless native MCP and configurable sync support are separated.
- Copilot and VS Code share `.vscode/mcp.json`; generated validators must compare IDs/metadata, not assume one destination per agent.
- Claude Desktop has a global OS-dependent path and is disabled by default; documentation must retain this distinction rather than flattening it into a project-relative destination.
- OpenSpec source references use line ranges that will become stale after extraction; update references or use symbol-level references in the future.
- Generated docs can create large noisy diffs; use narrowly marked fragments or a validator with stable expected tables and keep the 400-line review budget in mind.

### Ready for Proposal

Yes — the proposal can proceed if it explicitly separates (a) native MCP support from (b) configurable-agent support, names the documentation surfaces to govern, and requires exact human/JSON output preservation for #494. The main unresolved product decision is whether #500’s canonical list must cover only MCP agents or every configurable agent supported by `agent_ids.rs`.
