# @dallay/agentsync

[![npm version](https://img.shields.io/npm/v/@dallay/agentsync.svg)](https://www.npmjs.com/package/@dallay/agentsync)  
[![license](https://img.shields.io/npm/l/@dallay/agentsync.svg)](https://github.com/dallay/agentsync/blob/main/LICENSE)  
[![repository](https://img.shields.io/badge/repo-dallay%2Fagentsync-blue)](https://github.com/dallay/agentsync)

Effortlessly synchronize AI agent configurations across tools like Copilot, Claude, Cursor, and other MCP-compatible servers using symbolic links and an intuitive CLI.

🌟 **[Explore the Full Documentation Here](https://dallay.github.io/agentsync/)**

---

## ✨ Key Features

- **Simple CLI**: Manage symbolic links with minimal setup.
- **Multi-assistant support**: Compatible across Copilot, Claude, Gemini, Cursor, VS Code and OpenCode.
    See the full list and file locations in the main documentation: https://dallay.github.io/agentsync/ (or the repo README).
- **Cross-platform binaries**: Available for Linux, MacOS, and Windows.
- **Node.js integration**: Use programmatically within your applications.

---

## 🚀 Installation

Make sure you have Node.js (>=18) installed.

### Using `pnpm` (recommended):

```bash
pnpm install -g @dallay/agentsync
```

### Using `npm`:

```bash
npm install -g @dallay/agentsync
```

### Using `yarn`:

```bash
yarn global add @dallay/agentsync
```

### Using `bun`:

```bash
bun add -g @dallay/agentsync
```

Verify installation:

```bash
agentsync --help
```

---

## 🛠️ Usage

### Managing Configurations

#### Sync Configurations:

Run the following to create symbolic links across your AI coding assistants:

```bash
agentsync apply
```

#### Clean Configurations:

Remove previously created symbolic links:

```bash
agentsync clean
```

#### Status and Diagnostics:

Check the status of managed symlinks or run a diagnostic health check:

```bash
# Verify the state of symlinks and report drift
agentsync status

# Run a comprehensive diagnostic and health check
agentsync doctor
```

🎯 **Example Workflows**:

- **Programmatic Usage in Node.js**:

   ```javascript
   const { main } = require('@dallay/agentsync');

   main(['apply']).catch((error) => {
      console.error(error);
      process.exit(1);
   });
   ```

- **Integrate with npm scripts**:
   Add configuration syncing to your npm scripts to automate process workflows.
   For example, in your `package.json`:

   ```json
   {
     "scripts": {
       "precommit": "pnpm exec agentsync apply --dry-run",
       "prepare": "pnpm exec agentsync apply"
     }
   }
   ```

- For complex workflows, see the [detailed API documentation](https://dallay.github.io/agentsync/).

## MCP & Skills

- AgentSync supports MCP generation for multiple agents (Claude, Copilot, Gemini, Cursor, VS Code, OpenCode). The canonical list and file locations live in the repo README and in the docs site (guides/mcp).
- Skills live under `.agents/skills/` in the project.
- **Manage skills via CLI**:
  ```bash
  agentsync skill install <skill-id>
  agentsync skill update <skill-id>
  agentsync skill uninstall <skill-id>
  ```

---

## 👷 Development

This package is part of the [AgentSync mono-repo](https://github.com/dallay/agentsync). It serves as a Node.js distribution layer for the high-performance Rust core.

### Prerequisites

- [**pnpm**](https://pnpm.io/): Dependency manager.
- [**Node.js**](https://nodejs.org/) >= 24.13.0.
- [**Rust**](https://www.rust-lang.org/): For building the core CLI (1.89+).

### Setup and Build

From the monorepo root:

1. **Install dependencies and build core**:
   ```bash
   make install
   ```

2. **Build the NPM package**:
   ```bash
   make js-build
   ```

### Common Commands

- **Run tests and type checks**: `make js-test`
- **Format code**: `make fmt`
- **Full verification**: `make verify-all`

For more details on the Rust core development, see the [main README](https://github.com/dallay/agentsync/blob/main/README.md).

---

## 🛠️ Troubleshooting

### `PNPM_NO_MATURE_MATCHING_VERSION`

If `pnpm install` fails with this error, it's likely due to a strict package release age policy. You can try installing with `--ignore-scripts` or wait for the package to "mature" in the registry.

### Lefthook installation failure

If `pnpm install` fails during the `lefthook` setup, you can try:

```bash
pnpm install --ignore-scripts
```

---

## 🌐 Resources

- **Project Repository**: [GitHub Repository](https://github.com/dallay/agentsync)
- **Submit Issues**: [GitHub Issues](https://github.com/dallay/agentsync/issues)
- **Explore Full Documentation**: [Documentation Website](https://dallay.github.io/agentsync/)

---

## 📜 License

MIT License. See the [LICENSE](https://github.com/dallay/agentsync/blob/main/LICENSE) for details.

---

## 🙏 Acknowledgments

Special thanks to the developer community for their contributions and feedback. For suggestions and improvements, feel free to open a pull request!

---

📣 Ready? Start syncing agent configs today with `@dallay/agentsync`!
