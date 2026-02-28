# AgentSync Docs

This documentation site provides usage, guides, and technical reference for AgentSync: a fast, portable CLI for synchronizing AI agent configurations, commands, and MCP servers across major coding assistants.

## Where the docs live
- Documentation source: `website/docs/src/content/docs/`
- Docs homepage: `website/docs/src/content/docs/index.mdx`

## Key pages (quick links)
- Getting started: `website/docs/src/content/docs/guides/getting-started.mdx` — basic workflow, init/apply
- MCP guides: `website/docs/src/content/docs/guides/mcp.mdx` — supported agents, file locations and merge behavior
- Configuration reference: `website/docs/src/content/docs/reference` — TOML examples and target types

## Developer & contribution hints
- For development or contributing to the documentation, see the main project `README.md` in the repo root for overall setup and build steps.
- If you're updating supported agents or MCP formatters, make sure to cross-check `src/mcp.rs` (authoritative formatters and agent IDs).

If you want me to update or add specific doc pages (for example, a dedicated 'Skills' guide), let me know.
