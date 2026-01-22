# @dallay/agentsync

Sync AI agent configurations across multiple coding assistants using symbolic links.

[![npm version](https://img.shields.io/npm/v/@dallay/agentsync.svg)](https://www.npmjs.com/package/@dallay/agentsync)
[![license](https://img.shields.io/npm/l/@dallay/agentsync.svg)](./LICENSE)
[![repo](https://img.shields.io/badge/repo-dallay%2Fagentsync-blue)](https://github.com/dallay/agentsync)

Purpose
-------
agentsync helps you keep AI agent configuration files synchronized across multiple AI coding assistants (e.g. Copilot, Claude, Gemini) by managing symbolic links and a simple CLI.

Quick links
-----------
- Repository: https://github.com/dallay/agentsync
- Issues: https://github.com/dallay/agentsync/issues

Installation
------------
Install locally with pnpm (recommended):

pnpm add @dallay/agentsync

Run without installing with pnpm dlx:

pnpm dlx @dallay/agentsync --help

Or with npx:

npx @dallay/agentsync --help

CLI Usage
---------
The package exposes a CLI executable named `agentsync`.

Basic help:

agentsync --help

Common workflows:

- Create or update symlinks from a central config directory to target assistant config locations.
- Validate existing symlinks and report missing or broken links.
- Export or import agent configurations.

Configuration
-------------
agentsync uses a small configuration file to define sources and targets for synchronization. Example (adjust to your environment):

```json
{
  "sources": [
    "~/.config/agents/common",
    "./shared-agent-configs"
  ],
  "targets": {
    "copilot": "~/.config/copilot/agents",
    "claude": "~/.config/claude/agents",
    "gemini": "~/.config/gemini/agents"
  },
  "options": {
    "dryRun": false,
    "force": false
  }
}
```

Programmatic usage
-------------------
This package can be executed as a CLI. If you need to call core functionality from Node, require the package entry point and use the public functions exported from `lib/` (build artifacts). Generate typed API docs with TypeDoc if you want accurate signatures.

Example (run CLI programmatically):

```js
// run the CLI handler as a programmatic entry point
const { main } = require('@dallay/agentsync');
main(process.argv).catch(err => {
  console.error(err);
  process.exit(1);
});
```

Note: Replace the snippet above with the real exported function names if your code exposes a different API. Prefer inspecting `lib/index.js` or generating TypeDoc output to document precise signatures.

Development
-----------
Build and type-check (this project uses TypeScript):

pnpm install
pnpm run typecheck
pnpm run build

Useful scripts defined in package.json:

- typecheck — tsc --noEmit
- build — tsc
- clean — remove generated `lib/`

Publishing
----------
This package is published to npm as @dallay/agentsync. The repository sets `publishConfig.access=public`.

Before publishing:

1. Ensure `lib/` is built (pnpm run build).
2. Verify package.json fields: `main`, `files`, `bin`, `repository`, `bugs`, `homepage`.
3. Ensure LICENSE is present and tests/CI are green.

pnpm publish --access public

Contributing
------------
Contributions welcome. Keep it simple:

1. Fork the repo and create a feature branch.
2. pnpm install
3. pnpm run typecheck && pnpm run build
4. Open a PR with a clear description and link to any related issue.

We use Conventional Commits to generate changelogs. Keep commits focused and small.

Security
--------
Report security issues by opening a private issue on the repository or emailing the maintainer listed in package.json.

License
-------
MIT — see LICENSE

More documentation
------------------
For full API docs and examples, generate TypeDoc or add an `docs/` or `examples/` directory. If you want, I can generate a TypeDoc-based docs bundle and add CI to publish it to GitHub Pages.
