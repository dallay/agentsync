## [1.3.0](https://github.com/dallay/agentsync/compare/v1.2.0...v1.3.0) (2026-01-15)

### ✨ Features

* add MCP (Model Context Protocol) config generation for Claude, Copilot, Gemini, VS Code, and OpenCode ([f555e95](https://github.com/dallay/agentsync/commit/f555e95f7afcf282af31f6f5554ab42d35e4fbb0))

### ♻️ Refactors

* extract standard MCP config helpers and deduplicate formatter logic ([2a9a025](https://github.com/dallay/agentsync/commit/2a9a02528c0261f15dd2320a1af361d1b781f2f5))

### 📝 Documentation

* add AGENTS.md for build, lint, and test commands ([2d3f1a4](https://github.com/dallay/agentsync/commit/2d3f1a4ba6df11866fdd61d05e467bf4603826ef))
* improve README formatting and clarify MCP and target type sections ([a63df17](https://github.com/dallay/agentsync/commit/a63df17545943b7ce761b6cfd7b638cd18bf0386))

## [1.2.0](https://github.com/dallay/agentsync/compare/v1.1.1...v1.2.0) (2026-01-15)

### ✨ Features

* add NPM wrapper for npx distribution ([0658101](https://github.com/dallay/agentsync/commit/0658101f7cf48fca2229caf849ecec3812b94ba2))
* add npx wrapper for agentsync and publish platform-specific NPM packages ([3f2ed71](https://github.com/dallay/agentsync/commit/3f2ed71224d8b732a7b7e8df99dac283f5f7e7eb))

## [1.1.1](https://github.com/dallay/agentsync/compare/v1.1.0...v1.1.1) (2026-01-15)

### 🐛 Bug Fixes

* remove deprecated CLI tests using cargo_bin ([680893e](https://github.com/dallay/agentsync/commit/680893e4f6816a6870fb6813b3152842591fcdb2))

## [1.1.0](https://github.com/dallay/agentsync/compare/v1.0.0...v1.1.0) (2026-01-15)

### ✨ Features

* add Docker support and publish workflow to release pipeline ([62c8ac0](https://github.com/dallay/agentsync/commit/62c8ac02a725521b250909f910f652dd6b9ffdc1))
* complete release workflow with docker support and security hardening ([e5ef7eb](https://github.com/dallay/agentsync/commit/e5ef7eb0f8e9fa11cef5a0dc7991fbc92a26297d))
* update actions/checkout version to v6 in CI and release workflows ([34a6a7d](https://github.com/dallay/agentsync/commit/34a6a7d112885ca1545805ad6bfc4d5f6f3fa0b2))
* update Rust toolchain version in release workflow ([0485d00](https://github.com/dallay/agentsync/commit/0485d006bfaadb3a19c6ff0c59649d04cc2f964c))

### 🐛 Bug Fixes

* replace dtolnay/rust-action with correct rust-toolchain action in CI ([59fd667](https://github.com/dallay/agentsync/commit/59fd6670b8956ec387eceaa8c394913d2d6327ef))
* resolve clippy warnings and formatting issues ([60a7674](https://github.com/dallay/agentsync/commit/60a767478d96f5428f2f8a41cf5004bf5b7696f8))

## 1.0.0 (2026-01-15)

### ✨ Features

* initial release of AgentSync ([04fa108](https://github.com/dallay/agentsync/commit/04fa108cd5ad51f75a436186c6a18b7925e8ebf2))
