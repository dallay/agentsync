# AgentSync

[![CI](https://github.com/dallay/agentsync/actions/workflows/ci.yml/badge.svg)](https://github.com/dallay/agentsync/actions/workflows/ci.yml)
[![Release](https://github.com/dallay/agentsync/actions/workflows/release.yml/badge.svg)](https://github.com/dallay/agentsync/actions/workflows/release.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![GitHub release](https://img.shields.io/github/v/release/dallay/agentsync)](https://github.com/dallay/agentsync/releases)

A fast, portable CLI tool for synchronizing AI agent configurations across multiple AI coding
assistants using symbolic links.
![synchro.webp](website/docs/src/assets/synchro.webp)

## Why AgentSync?

Different AI coding tools expect configuration files in various locations:

| Tool               | Instructions                      | Commands             | Skills             |
|--------------------|-----------------------------------|----------------------|--------------------|
| **Claude Code**    | `CLAUDE.md`                       | `.claude/commands/`  | `.claude/skills/`  |
| **GitHub Copilot** | `.github/copilot-instructions.md` | `.github/agents/`    | -                  |
| **Gemini CLI**     | `GEMINI.md`                       | `.gemini/commands/`  | `.gemini/skills/`  |
| **Cursor**         | `AGENTS.md`                       | `.cursor/commands/`  | `.cursor/skills/`  |
| **VS Code**        | `AGENTS.md` (or `.vscode/*`)      | `.vscode/`           | -                  |
| **OpenCode**       | `AGENTS.md`                       | `.opencode/command/` | `.opencode/skill/` |

AgentSync maintains a **single source of truth** in `.agents/` and creates symlinks to all required
locations.

## Features

- 🔗 **Symlinks over copies** - Changes propagate instantly
- 📝 **TOML configuration** - Human-readable, easy to maintain
- 📋 **Gitignore management** - Automatically updates `.gitignore`
- 🖥️ **Cross-platform** - Linux, macOS, Windows
- 🚀 **CI-friendly** - Gracefully skips when binary unavailable
- ⚡ **Fast** - Single static binary, no runtime dependencies

## Installation

### Node.js Package Managers (Recommended)

If you have Node.js (>=18) installed, the easiest way to install AgentSync is through a package manager.

#### Global Installation

```bash

# Using npm

npm install -g @dallay/agentsync

# Using pnpm

pnpm add -g @dallay/agentsync

# Using yarn (Classic v1)

yarn global add @dallay/agentsync

# Using bun

bun i -g @dallay/agentsync
```

#### One-off Execution

If you want to run AgentSync without a permanent global installation:

```bash

# Using npx (npm)

npx @dallay/agentsync apply

# Using dlx (pnpm)

pnpm dlx @dallay/agentsync apply

# Using dlx (yarn v2+)

yarn dlx @dallay/agentsync apply

# Using bunx (bun)

bunx @dallay/agentsync apply
```

#### Local Installation (Dev Dependency)

```bash

# Using npm

npm install --save-dev @dallay/agentsync

# Using pnpm

pnpm add -D @dallay/agentsync

# Using yarn

yarn add -D @dallay/agentsync

# Using bun

bun add -d @dallay/agentsync
```

### From crates.io (Rust)

If you have Rust installed, you can install AgentSync directly from [crates.io](https://crates.io/crates/agentsync):

```bash
cargo install agentsync
```

### From GitHub Releases (Pre-built Binaries)

Download the latest release for your platform from the [GitHub Releases](https://github.com/dallay/agentsync/releases) page:

```bash

# macOS (Apple Silicon)

curl -LO https://github.com/dallay/agentsync/releases/latest/download/agentsync-aarch64-apple-darwin.tar.gz
tar xzf agentsync-aarch64-apple-darwin.tar.gz
sudo mv agentsync-*/agentsync /usr/local/bin/

# macOS (Intel)

curl -LO https://github.com/dallay/agentsync/releases/latest/download/agentsync-x86_64-apple-darwin.tar.gz
tar xzf agentsync-x86_64-apple-darwin.tar.gz
sudo mv agentsync-*/agentsync /usr/local/bin/

# Linux (x86_64)

curl -LO https://github.com/dallay/agentsync/releases/latest/download/agentsync-x86_64-unknown-linux-gnu.tar.gz
tar xzf agentsync-x86_64-unknown-linux-gnu.tar.gz
sudo mv agentsync-*/agentsync /usr/local/bin/

# Linux (ARM64)

curl -LO https://github.com/dallay/agentsync/releases/latest/download/agentsync-aarch64-unknown-linux-gnu.tar.gz
tar xzf agentsync-aarch64-unknown-linux-gnu.tar.gz
sudo mv agentsync-*/agentsync /usr/local/bin/
```

### From Source (Requires Rust 1.89+)

Install directly from the GitHub repository:

```bash
cargo install --git https://github.com/dallay/agentsync
```

Or clone and build manually:

```bash
git clone https://github.com/dallay/agentsync
cd agentsync
cargo build --release

# The binary will be available at ./target/release/agentsync

```

## Quick Start

1. **Initialize configuration** in your project:

```bash
cd your-project
agentsync init
```

This creates `.agents/agentsync.toml` with a default configuration.

2. **Edit the configuration** to match your needs (see [Configuration](#configuration))

3. **Apply the configuration**:

```bash
agentsync apply
```

4. **Add to your project setup** (e.g., `package.json`):

```json
{
  "scripts": {
    "prepare": "agentsync apply || true"
  }
}
```

## Usage

```bash

# Initialize a new configuration

agentsync init

# Apply configuration (create symlinks)

agentsync apply

# Clean existing symlinks before applying

agentsync apply --clean

# Remove all managed symlinks

agentsync clean

# Use a custom config file

agentsync apply --config /path/to/config.toml

# Dry run (show what would be done without making changes)

agentsync apply --dry-run

# Filter by agent

agentsync apply --agents claude,copilot

# Disable gitignore updates

agentsync apply --no-gitignore

# Verbose output

agentsync apply --verbose

# Show version

agentsync --version

# Manage skills

agentsync skill install <skill-id>
agentsync skill update <skill-id>
agentsync skill list
```

### Status

Verify the state of symlinks managed by AgentSync. Useful for local verification and CI.

```bash
agentsync status [--project-root <path>] [--json]
```

- `--project-root <path>`: Optional. Path to the project root to locate the agentsync config.
- `--json`: Output machine-readable JSON (pretty-printed).

Exit codes: 0 = no problems, 1 = problems detected (CI-friendly)

## Configuration

Configuration is stored in `.agents/agentsync.toml`:

```toml

# Source directory (relative to this config file)

source_dir = "."

# Gitignore management

[gitignore]
enabled = true
marker = "AI Agent Symlinks"
entries = [
    "CLAUDE.md",
    "GEMINI.md",
    ".github/copilot-instructions.md",
]

# Agent definitions

[agents.claude]
enabled = true
description = "Claude Code - Anthropic's AI coding assistant"

[agents.claude.targets.instructions]
source = "AGENTS.md"
destination = "CLAUDE.md"
type = "symlink"

[agents.claude.targets.commands]
source = "command"
destination = ".claude/commands"
type = "symlink-contents"
pattern = "*.agent.md"
```

### Variable Substitution (Templating)

AgentSync supports dynamic variables in instruction files (e.g., `AGENTS.md`) using `{{variable}}` syntax.

When you run `agentsync apply`, placeholders are replaced with project-specific data. Since symbolic links require physical files, AgentSync generates processed versions in `.agents/.cache/` and links to those instead of the originals.

#### Supported Variables

| Variable | Description | Source |
| :--- | :--- | :--- |
| `project_name` | The name of the project | `name` field in `package.json` |
| `git_branch` | The current active git branch | `git rev-parse --abbrev-ref HEAD` |
| *custom* | Any custom variable you define | `[vars]` section in `agentsync.toml` |

#### Example

**1. Define variables in `agentsync.toml`:**

```toml
[vars]
team_name = "Platform Engineering"
```

**2. Use variables in `.agents/AGENTS.md`:**

```markdown
# Project Instructions for {{project_name}}

Current Branch: {{git_branch}}
Maintaining Team: {{team_name}}
```

**3. Apply the configuration:**

```bash
agentsync apply
```

The resulting `CLAUDE.md` (or other linked destinations) will contain the substituted values.

### MCP Support (Model Context Protocol)

AgentSync can automatically generate MCP configuration files for supported agents (Claude Code,
GitHub Copilot, Gemini CLI, Cursor, VS Code, OpenCode).

This allows you to define MCP servers once in `agentsync.toml` and have them synchronized to all
agent-specific config files.

```toml
[mcp]
enabled = true

# Strategy for existing files: "merge" (default) or "overwrite"

# "merge" preserves existing servers but overwrites conflicts with TOML config

merge_strategy = "merge"

# Define servers once

[mcp_servers.filesystem]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "."]

[mcp_servers.git]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-git", "--repository", "."]

# Optional fields:

# env = { "KEY" = "VALUE" }

# disabled = false

```

#### Supported Agents (canonical)

AgentSync supports the following agents and will synchronize corresponding files/locations. This list is canonical — keep it in sync with `src/mcp.rs` (authoritative).

- **Claude Code** — `.mcp.json` (agent id: `claude`)
- **GitHub Copilot** — `.copilot/mcp-config.json` (agent id: `copilot`)
- **Gemini CLI** — `.gemini/settings.json` (agent id: `gemini`) — AgentSync will add `trust: true` when generating Gemini configs.
- **Cursor** — `.cursor/mcp.json` (agent id: `cursor`)
- **VS Code** — `.vscode/mcp.json` (agent id: `vscode`)
- **OpenCode** — `opencode.json` (agent id: `opencode`)

See `website/docs/src/content/docs/guides/mcp.mdx` for formatter details and merge behavior.

#### Merge Behavior

When `merge_strategy = "merge"`:

1. AgentSync reads the existing config file (if it exists).
2. It adds servers defined in `agentsync.toml`.
3. **Conflict Resolution**: If a server name exists in both, the definition in `agentsync.toml` **wins** and overwrites the existing one.
4. Existing servers NOT in `agentsync.toml` are preserved.

### Target Types

| Type               | Description                                           |
|--------------------|-------------------------------------------------------|
| `symlink`          | Create a symlink to the source file/directory         |
| `symlink-contents` | Create symlinks for each file in the source directory |

The `symlink-contents` type optionally supports a `pattern` field (glob pattern like `*.md`) to
filter which files to link.

## Project Structure

```
.agents/
├── agentsync.toml      # Configuration file
├── AGENTS.md           # Main agent instructions (single source)
├── .mcp.json           # MCP server configurations
├── command/            # Agent commands
│   ├── review.agent.md
│   └── test.agent.md
├── skills/             # Shared knowledge/skills
│   └── kotlin/
│       └── SKILL.md
└── prompts/            # Reusable prompts
    └── code-review.prompt.md
```

After running `agentsync apply`:

```
project-root/
├── CLAUDE.md           → .agents/AGENTS.md
├── GEMINI.md           → .agents/AGENTS.md
├── AGENTS.md           → .agents/AGENTS.md
├── .mcp.json           → .agents/.mcp.json
├── .claude/
│   └── commands/       → symlinks to .agents/command/*.agent.md
├── .gemini/
│   └── commands/       → symlinks to .agents/command/*.agent.md
└── .github/
    ├── copilot-instructions.md → .agents/AGENTS.md
    └── agents/         → symlinks to .agents/command/*.agent.md
```

## CI/CD Integration

AgentSync gracefully handles CI environments where the binary isn't available:

```json
{
  "scripts": {
    "agents:sync": "pnpm exec agentsync apply",
    "prepare": "lefthook install && pnpm run agents:sync"
  }
}
```

The symlinks are primarily for local development. CI builds typically don't need them.

### Installing in CI

If you need agentsync in CI, add it to your workflow:

```yaml
- name: Install agentsync
  run: |
    curl -LO https://github.com/dallay/agentsync/releases/latest/download/agentsync-x86_64-unknown-linux-gnu.tar.gz
    tar xzf agentsync-x86_64-unknown-linux-gnu.tar.gz
    sudo mv agentsync-*/agentsync /usr/local/bin/
```

## Getting Started (Development)

This project is a monorepo containing a Rust core and a JavaScript/TypeScript wrapper.

### Repository Structure

- `src/`: Core logic and CLI implementation in **Rust**.
- `npm/agentsync/`: **TypeScript** wrapper used for NPM distribution.
- `website/docs/`: Documentation site built with **Starlight**.
- `tests/`: Integration tests for the CLI.

### Prerequisites

- [**Rust**](https://www.rust-lang.org/tools/install) (1.89+ recommended)
- [**Node.js**](https://nodejs.org/) (v24.13.0+ recommended for development)
- [**pnpm**](https://pnpm.io/installation)

### Setup

1.  **Install JavaScript dependencies:**

    ```bash
    pnpm install
    ```

2.  **Build the Rust binary:**

    ```bash
    cargo build
    ```

### Common Commands

This project uses a `Makefile` to orchestrate common tasks.

-   **Run Rust tests:**

    ```bash
    make rust-test
    ```

-   **Run JavaScript tests:**

    ```bash
    make js-test
    ```

-   **Build all components:**

    ```bash
    make all
    ```

-   **Format the code:**

    ```bash
    make fmt
    ```

## Troubleshooting

### `PNPM_NO_MATURE_MATCHING_VERSION`

If `pnpm install` fails with this error, it's likely due to a strict package release age policy. You can try installing with `--ignore-scripts` or wait for the package to "mature" in the registry.

### Lefthook installation failure

If `pnpm install` fails during the `lefthook` setup, you can try:

```bash
pnpm install --ignore-scripts
```

### Symlink creation fails on Windows

Ensure you have Developer Mode enabled or run your terminal as Administrator, as Windows requires special permissions for creating symbolic links.

## Inspiration

- [Ruler](https://github.com/intellectronica/ruler) - Similar concept but copies files instead of
  using symlinks

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feat/amazing-feature`)
3. Commit your changes (`git commit -m 'feat: add amazing feature'`)
4. Push to the branch (`git push origin feat/amazing-feature`)
5. Open a Pull Request

## License

MIT License - see [LICENSE](LICENSE) for details.
