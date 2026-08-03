# Design: CLI Output Extraction and Canonical Agent Documentation

## Technical Approach

Keep command handlers in `src/main.rs` as orchestration only. Move the pure apply/clean/init
line builders, `print_lines`, and banner emission into `src/output.rs`; retain direct progress
printed by `src/init.rs` and linker operations. The moved functions preserve their current
strings, ordering, blank lines, and `HumanFormatter` color decisions.

Make `src/mcp.rs` the sole typed source for *native MCP* documentation metadata. Add a stable
metadata view to `McpAgent` (ID, display name, documented destination, format, global flag, and
notes), while keeping path resolution, formatter dispatch, aliases, and generation behavior in
their existing methods. Validate marked Markdown fragments against that registry rather than
introducing a data-driven runtime registry or conflating configurable-only agents in
`src/agent_ids.rs`.

## Architecture Decisions

| Decision | Choice | Alternatives / rationale |
|---|---|---|
| Presentation boundary | `src/output.rs`, with renderer functions `pub(crate)` | A new output submodule would add files without improving boundaries. Keeping JSON status in `commands/status.rs` avoids changing its contract. |
| Renderer API | Borrow inputs and return `Vec<String>`; `print_lines(&[String])` remains `pub(crate)` | Returning strings preserves existing testability and line order; a writer abstraction would risk unrelated I/O refactoring. |
| MCP canonical source | Typed `McpAgent` registry plus public metadata accessor | A TOML/JSON manifest would duplicate enum behavior and weaken compile-time coverage. `agent_ids.rs` remains the alias/configurable-agent boundary. |
| Documentation mechanism | Stable `<!-- agentsync:mcp:start/end -->` fragments validated by a Rust integration test | Generation could create noisy multi-document diffs. Validation is deterministic, requires no runtime dependency, and fails on omissions, duplicates, IDs, or metadata drift. |

## Data Flow

```text
CLI handler (main.rs) → output renderer → Vec<String> → print_lines → stdout
McpAgent::all() → documentation_metadata() → doc validator → Markdown fragments
Linker/McpGenerator → existing SyncResult/McpSyncResult → unchanged renderers
```

The validator reads the four governed documents, extracts each marked block, renders the
canonical rows in deterministic `McpAgent::all()` order, and compares exact block contents.
Shared destinations (Copilot and VS Code) and Claude Desktop's global/OS-dependent path are
represented explicitly in metadata notes; validation never calls filesystem path resolution.

## File Changes

| File | Action | Description |
|---|---|---|
| `src/main.rs` | Modify | Remove moved renderers/emitter and import output APIs; leave orchestration and command behavior intact. |
| `src/output.rs` | Modify | Own apply/clean/init renderers, banner, and line emission; migrate their unit tests. |
| `src/mcp.rs` | Modify | Add typed native-MCP documentation metadata and accessor; preserve generation methods. |
| `tests/mcp_documentation.rs` | Create | Validate all marked MCP fragments against `McpAgent` metadata. |
| `README.md`, `npm/agentsync/README.md`, `website/docs/src/content/docs/guides/mcp.mdx`, `openspec/specs/mcp-generation/spec.md` | Modify | Replace duplicated native-agent blocks with exact markers/content, including Claude Desktop. |
| `.github/workflows/ci.yml` | Modify | Add a fast `cargo test --test mcp_documentation` job (or focused step) before full matrix work. |

## Interfaces / Contracts

The implementation should expose only the metadata needed by docs:

```rust
pub struct McpAgentDocumentation {
    pub id: &'static str,
    pub name: &'static str,
    pub destination: &'static str,
    pub format: &'static str,
    pub global: bool,
    pub notes: &'static str,
}

impl McpAgent {
    pub fn documentation(&self) -> McpAgentDocumentation;
}
```

The validator's contract is exact marker pairing, one block per governed file, and byte-for-byte
canonical block equality. No CLI output, MCP config schema, aliases, defaults, or paths change.

## Testing Strategy

| Layer | What to Test | Approach |
|---|---|---|
| Unit | Every moved renderer, plain/color output, blank lines, summaries, and init next steps | Move existing `main.rs` assertions with the functions; add exact `Vec<String>` assertions for previously uncovered renderers. |
| Integration | Registry completeness and documentation drift | `tests/mcp_documentation.rs` checks all eight native agents, unique canonical IDs, marker validity, exact rows, and Claude Desktop/global notes. |
| Regression | CLI preservation and MCP generation | Run existing focused CLI/MCP tests plus `cargo test --all-features`; compare representative `apply --dry-run`, `clean --dry-run`, and MCP outputs before/after using captured plain output. |

## Migration / Rollout

No migration or feature flag. Land extraction and validator/docs updates together so CI cannot
observe a canonical registry without its governed documentation. Rollback is a normal revert;
runtime data and generated user files are unaffected.

## Open Questions

- [x] No blocking questions remain. Native MCP support is intentionally distinct from configurable-agent support.
