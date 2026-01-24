# @dallay/agentsync

[![npm version](https://img.shields.io/npm/v/@dallay/agentsync.svg)](https://www.npmjs.com/package/@dallay/agentsync)  
[![license](https://img.shields.io/npm/l/@dallay/agentsync.svg)](https://github.com/dallay/agentsync/blob/main/LICENSE)  
[![repository](https://img.shields.io/badge/repo-dallay%2Fagentsync-blue)](https://github.com/dallay/agentsync)  

**Version:** 1.14.2  
**License:** MIT  

Effortlessly synchronize AI agent configurations across tools like Copilot, Claude, Cursor, and other MCP-compatible servers using symbolic links and an intuitive CLI.

🌟 **[Explore the Full Documentation Here](https://dallay.github.io/agentsync/)**

---

## ✨ Key Features

- **Simple CLI**: Manage symbolic links with minimal setup.
- **Multi-assistant support**: Compatible across Copilot, Claude, Gemini, and more.
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

---

## 👷 Development

### Prerequisites
- [pnpm](https://pnpm.io/): Dependency manager.
- [Node.js](https://nodejs.org/) >= 18.

### Steps
1. Clone the repository:
   ```bash
   git clone https://github.com/dallay/agentsync.git
   cd agentsync
   ```

2. Install dependencies:
   ```bash
   pnpm install
   ```

3. Build:
   ```bash
   pnpm run build
   ```

4. Run type checks:
   ```bash
   pnpm run typecheck
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
