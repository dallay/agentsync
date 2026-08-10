# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

Primary user: individual developers who use two or more AI coding assistants (Claude, Copilot, Cursor, Gemini, Codex, VS Code, OpenCode) and want to stop manually copying/pasting configuration files across projects and tools.

Situation: they maintain a `.agents/` directory as the single source of truth, run `agentsync apply` from the CLI, and need clear, trustworthy documentation to install, configure, sync, and troubleshoot that workflow.

## Product Purpose

AgentSync is a fast, portable CLI tool that synchronizes AI agent configurations across multiple AI coding assistants using symbolic links. It exists so a developer can change a configuration once in `.agents/` and every supported assistant sees the change immediately — no copy-paste, no drift, no per-agent manual edits.

The docs site exists to move a developer from "heard about it / installed it" to a working synced setup, and to serve as the durable reference for configuration, sync types, MCP, skills, and the CLI.

Success means the reader can install, configure, and verify a working sync quickly and confidently, and can return to the reference pages for exact syntax and behavior.

## Positioning

The mechanism a neighboring tool could not truthfully copy: AgentSync uses symbolic links, not copies. Change a file once in `.agents/` and every supported assistant sees it immediately. That single-source-of-truth via symlinks is the core claim; "instant propagation" and "no copy-paste" follow directly from it.

## Operating Context

- CLI-first product: install globally via npm/pnpm/yarn/bun or run one-off with npx/pnpm dlx.
- Developers run `agentsync apply`, `clean`, `init`, `status`, `doctor`, `skill` from a terminal, often in CI-friendly flows.
- Cross-platform: macOS, Linux, Windows (Windows has a dedicated symlink setup guide).
- Docs deploy to GitHub Pages at `dallay.github.io/agentsync`; Astro 7 + Starlight 0.41, content in `website/docs/src/content/docs/`.
- Repo: `dallay/agentsync` on GitHub (social icon links there).

## Capabilities and Constraints

Confirmed functionality (documented in this site):

- Four sync types per target: `symlink`, `symlink-contents`, `nested-glob` (destination template placeholders `{relative_path}`, `{file_name}`, `{stem}`, `{ext}`), `module-map`.
- `agentsync.toml` configuration (project root or `.agents/`), parsed into typed config.
- MCP server config generation in agent-specific formats (JSON/TOML).
- Skills management: install, uninstall, update, suggest, detect from external providers (skills.sh or local archives).
- Managed `.gitignore` section with marker-delimited blocks; opt-out supported for teams that commit managed destinations.
- Subcommands: `apply`, `clean`, `init` (with wizard), `status`, `doctor`, `skill`.
- npm wrapper package `@dallay/agentsync` dispatching to platform binaries; Rust CLI (edition 2024).

Technical constraint: docs content is authored in the Astro content collection (`website/docs/src/content/docs/`); the `docs/` path at repo root is a symlink to the docs source.

Terminology: "agents" = AI coding assistants; "sync types" = the four linking strategies above; "managed" blocks = marker-delimited sections owned by AgentSync.

## Brand Commitments

- Name: AgentSync. Title on the docs site: "AgentSync".
- Voice: technical and direct. Clear, precise, developer-oriented copy; no marketing fluff, no invented claims.
- Existing visual implementation is the incumbent system (Starlight theme with custom Hero/Footer components, Geist Sans/Mono fonts, custom.css). This PRODUCT.md does not establish a visual world; DESIGN.md (via `document`) records the incumbent one.

## Evidence on Hand

- Real, authored documentation in the content collection: Quick Start, Getting Started, Sync Types, MCP, Skills, Gitignore team workflows, Windows symlink setup, Git hook automation, CLI reference, Configuration reference, Status output contract, contributing guides.
- Live codebase: `src/` (Rust CLI), `npm/agentsync/` (TypeScript wrapper), `website/docs/` (this site).
- Accessibility commitment established as SDD capability `docs-site-a11y` (WCAG 2.2 AA) — see `openspec/specs/docs-site-a11y/spec.md`.

Absences that future work must not fabricate: no testimonials, no case studies, no usage benchmarks, no pricing, no licensing claims beyond what the repo states, no third-party affiliations beyond the documented integrations.

## Product Principles

1. Truth over copy: the docs describe exactly what the CLI does; never claim behavior the tool does not have.
2. The reader's job is a working sync: structure docs so a developer reaches a verified `apply` quickly, then deepens.
3. One source of truth everywhere: the single `.agents/` model is the organizing idea for both the product and the documentation.
4. Portable and precise: cross-platform correctness matters; platform-specific behavior (e.g., Windows symlinks) is called out explicitly.
5. Accessible by default: docs meet WCAG 2.2 AA — contrast, reduced motion, and touch targets are maintained, not bolted on.

## Accessibility & Inclusion

- Established standard: WCAG 2.2 AA (capability `docs-site-a11y`).
- Known requirements from the recent audit/remediation: dark-mode footer text contrast, reduced-motion must restore hero visibility (no content hidden), touch targets ≥ 44px for interactive controls (search, theme select, tabs), no duplicated hidden text in components.
