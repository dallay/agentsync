# AgentSync

[![CI](https://github.com/dallay/agentsync/actions/workflows/ci.yml/badge.svg)](https://github.com/dallay/agentsync/actions/workflows/ci.yml)
[![Release](https://github.com/dallay/agentsync/actions/workflows/release.yml/badge.svg)](https://github.com/dallay/agentsync/actions/workflows/release.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![GitHub release](https://img.shields.io/github/v/release/dallay/agentsync)](https://github.com/dallay/agentsync/releases)

A fast, portable CLI tool for synchronizing AI agent configurations across multiple AI coding
assistants using symbolic links.

## Why AgentSync?

Different AI coding tools expect configuration files in different locations:

| Tool               | Instructions                      | Commands             | Skills             |
|--------------------|-----------------------------------|----------------------|--------------------|
| **Claude Code**    | `CLAUDE.md`                       | `.claude/commands/`  | `.claude/skills/`  |
| **GitHub Copilot** | `.github/copilot-instructions.md` | `.github/agents/`    | -                  |
| **Codex CLI**      | `AGENTS.md`                       | -                    | `.codex/skills/`   |
| **Gemini CLI**     | `GEMINI.md`                       | `.gemini/commands/`  | `.gemini/skills/`  |
| **OpenCode**       | `AGENTS.md`                       | `.opencode/command/` | `.opencode/skill/` |
| **Cursor**         | `.cursor/rules`                   | -                    | -                  |

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

### From GitHub Releases (Recommended)

Download the latest release for your platform:

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

### From Source (requires Rust 1.85+)

```bash
cargo install --git https://github.com/dallay/agentsync
```

Or clone and build:

```bash
git clone https://github.com/dallay/agentsync
cd agentsync
cargo build --release
# Binary at ./target/release/agentsync
```

### From crates.io (coming soon)

```bash
cargo install agentsync
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

# Use a custom config file
agentsync apply --config /path/to/config.toml

# Dry run (show what would be done without making changes)
agentsync apply --dry-run

# Show version
agentsync --version
```

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

### MCP Support (Model Context Protocol)

AgentSync can automatically generate MCP configuration files for supported agents (Claude Code,
GitHub Copilot, Gemini CLI, VS Code).

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

#### Supported Agents & File Locations

- **Claude Code**: `.mcp.json`
- **GitHub Copilot**: `.copilot/mcp-config.json`
- **Gemini CLI**: `.gemini/settings.json` (automatically adds `trust: true`)
- **VS Code**: `.vscode/mcp.json`
- **OpenCode**: `.opencode/mcp.json`

#### Merge Behavior

When `merge_strategy = "merge"`:

1. AgentSync reads the existing config file (if it exists).
2. It adds servers defined in `agentsync.toml`.
3. **Conflict Resolution**: If a server name exists in both, the definition in `agentsync.toml` *
   *wins** and overwrites the existing one.
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
    "agents:sync": "agentsync apply || echo 'agentsync not installed, skipping'",
    "prepare": "lefthook install && npm run agents:sync"
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
