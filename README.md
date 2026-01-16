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
| **Gemini CLI**     | `GEMINI.md`                       | `.gemini/commands/`  | `.gemini/skills/`  |
| **OpenCode**       | -                                 | `.opencode/command/` | `.opencode/skill/` |
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

## Getting Started

Follow these steps to get AgentSync up and running in your project:

### 1. Initialize Configuration

First, navigate to your project's root directory and run the `init` command:

```bash
cd your-project
agentsync init
```

This command creates a new `.agents/agentsync.toml` file with a default configuration, which you can customize to fit your needs.

### 2. Customize the Configuration

Next, open the `.agents/agentsync.toml` file and modify it to match your project's requirements. You can define which AI agents to support, specify the source and destination for your configuration files, and set up advanced options like Gitignore management and MCP support. For more details, see the [Configuration](#configuration) section.

### 3. Apply the Configuration

Once you're satisfied with your configuration, apply it by running the `apply` command:

```bash
agentsync apply
```

This command will create the necessary symbolic links, synchronize your MCP configurations, and update your `.gitignore` file, if enabled.

### 4. Add to Your Project Setup (Optional)

To ensure your configurations are always up-to-date, you can add AgentSync to your project's setup scripts. For example, in a `package.json` file, you can add the following:

```json
{
  "scripts": {
    "prepare": "agentsync apply || true"
  }
}
```

This will run `agentsync apply` automatically every time you install your project's dependencies.

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

# Dry run (show what would be done)
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

- [Ruler](https://github.com/intellectronica/ruler) - A similar tool that copies files instead of using symbolic links.

## Troubleshooting

If you encounter any issues while using AgentSync, here are a few common troubleshooting steps:

- **Permission Denied**: If you see a "Permission Denied" error when creating symbolic links, make sure you have the necessary permissions in the target directories. On Windows, you may need to run your terminal as an administrator.
- **File Not Found**: If AgentSync reports that a source file or directory is not found, double-check that the paths in your `.agents/agentsync.toml` file are correct and relative to the project root.
- **Configuration Issues**: If your configuration is not being applied as expected, run `agentsync apply --dry-run` to see a detailed preview of the changes without actually modifying your files.

If you continue to experience issues, please [open an issue](https://github.com/dallay/agentsync/issues) on our GitHub repository.

## Contributing

We welcome contributions from the community! If you'd like to get involved, please follow these steps:

1. **Fork the Repository**: Create your own fork of the [AgentSync repository](https://github.com/dallay/agentsync).
2. **Create a Feature Branch**: Make a new branch for your changes (`git checkout -b feat/your-amazing-feature`).
3. **Commit Your Changes**: Commit your work with a clear and descriptive message (`git commit -m 'feat: add amazing feature'`).
4. **Push to Your Branch**: Push your changes to your forked repository (`git push origin feat/your-amazing-feature`).
5. **Open a Pull Request**: Submit a pull request to the `main` branch of the official repository.

Before submitting your pull request, please make sure your code is well-tested and follows the existing style of the project.

## License

MIT License - see [LICENSE](LICENSE) for details.
