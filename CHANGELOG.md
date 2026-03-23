## [1.28.0](https://github.com/dallay/agentsync/compare/v1.27.2...v1.28.0) (2026-02-09)

### ✨ Features

* add GitHub URL conversion for ZIP downloads and implement condition-based waiting skill ([#160](https://github.com/dallay/agentsync/issues/160)) ([86be61b](https://github.com/dallay/agentsync/commit/86be61b94fa83f3d9fc106fb6e6e4714a44fbb47))

### 📝 Documentation

* update README.md for accuracy and formatting ([#159](https://github.com/dallay/agentsync/issues/159)) ([8cb9f19](https://github.com/dallay/agentsync/commit/8cb9f19b2302456f0e4536f7a37ad9af5645e605))

## [1.33.2](https://github.com/dallay/agentsync/compare/v1.33.1...v1.33.2) (2026-03-23)


### Chores

* update @astrojs/starlight to version 0.38.0 ([06776a6](https://github.com/dallay/agentsync/commit/06776a6ab3684bbaa0a81308c29ebe78ff3f98bf))

## [1.33.1](https://github.com/dallay/agentsync/compare/v1.33.0...v1.33.1) (2026-03-22)


### Chores

* **deps:** lock file maintenance ([#223](https://github.com/dallay/agentsync/issues/223)) ([75f82ed](https://github.com/dallay/agentsync/commit/75f82ed56ecde06e389974949f90df4fda4bde62))
* **deps:** update github actions ([#244](https://github.com/dallay/agentsync/issues/244)) ([d266a22](https://github.com/dallay/agentsync/commit/d266a22ad0687f250ff83b7f3fd8db4b4abb65b8))

## [1.33.0](https://github.com/dallay/agentsync/compare/v1.32.0...v1.33.0) (2026-03-22)


### Features

* add background version check with local cache ([d6b66e6](https://github.com/dallay/agentsync/commit/d6b66e635a142abb98fc7c32aaac13df5ee88463)), closes [#242](https://github.com/dallay/agentsync/issues/242)


### Bug Fixes

* **deps:** update major upgrades ([#243](https://github.com/dallay/agentsync/issues/243)) ([abfaca4](https://github.com/dallay/agentsync/commit/abfaca44cac466759120253983e6f270636bc153))


### Chores

* update Cargo.lock after adding is-terminal dependency ([0ef6738](https://github.com/dallay/agentsync/commit/0ef6738828bea189d8a2fc297b45008d414178c4))

## [1.32.0](https://github.com/dallay/agentsync/compare/v1.31.0...v1.32.0) (2026-03-22)


### Features

* nested agent context ([#242](https://github.com/dallay/agentsync/issues/242)) ([79bc0fa](https://github.com/dallay/agentsync/commit/79bc0fa5660e09dc799c0b86400b82aeb64bfe53))


### Bug Fixes

* **ci:** skip contributor-report for bot-authored PRs ([414b465](https://github.com/dallay/agentsync/commit/414b46555a276c2c443363dd09609d31740762e8))
* update release-please-action to googleapis/release-please-action@v4.4.0 ([b30474d](https://github.com/dallay/agentsync/commit/b30474d4fe30059908ff046b0ee37380211a1d35))


### Performance

* skip redundant I/O in write_compressed_agents_md ([#240](https://github.com/dallay/agentsync/issues/240)) ([9fcd4ae](https://github.com/dallay/agentsync/commit/9fcd4ae3fce2e75ff1bdeea30bb9911dedd88782))


### Continuous Integration

* Configure SonarCloud project and organization keys ([4cf4971](https://github.com/dallay/agentsync/commit/4cf4971350f54ccef8bcddb753c1427d19363b9a))


### Chores

* **deps:** update actions/create-github-app-token digest to fee1f7d ([#245](https://github.com/dallay/agentsync/issues/245)) ([bc37660](https://github.com/dallay/agentsync/commit/bc376601ed118e3e5c36758456f22c5262d2a81e))
* **deps:** update actions/setup-node digest to 53b8394 ([#220](https://github.com/dallay/agentsync/issues/220)) ([788147a](https://github.com/dallay/agentsync/commit/788147a615676770cc333d6f2cc7d1c14b93b23b))
* **deps:** update devdependencies ([#238](https://github.com/dallay/agentsync/issues/238)) ([1154a97](https://github.com/dallay/agentsync/commit/1154a97f022370a8f760716f1a23e670d9ef6d0b))

## [1.31.0](https://github.com/dallay/agentsync/compare/v1.30.0...v1.31.0) (2026-03-22)


### Features

* add `nested-glob` target type for monorepo/multi-module AGENTS.md discovery ([#234](https://github.com/dallay/agentsync/issues/234)) ([4ea1d59](https://github.com/dallay/agentsync/commit/4ea1d59f7fec6dbf237e1f6dd5abdf3715d5326e))
* migrate from semantic-release to release-please ([#237](https://github.com/dallay/agentsync/issues/237)) ([f531f01](https://github.com/dallay/agentsync/commit/f531f010b74457cd292e7a6d91650b0acc446385))
* Optimize directory iteration and MCP config generation ([#229](https://github.com/dallay/agentsync/issues/229)) ([9a01e2b](https://github.com/dallay/agentsync/commit/9a01e2bab597ce9416b386beaada3a1427247ad7))


### Bug Fixes

* **deps:** update cargo major upgrades ([2841cdf](https://github.com/dallay/agentsync/commit/2841cdf0c9eff4e27cf87cdf3fb300dcb9b47555))
* update release-please version to 1.30.0 ([57ef42b](https://github.com/dallay/agentsync/commit/57ef42b4f4532f15d56f98cc7eb50e984eb17769))
* update release-please-action to v4.1.1 (e4dc86b) ([0c7b165](https://github.com/dallay/agentsync/commit/0c7b1651f7ec61d2eb53cdf472ba891e625e512a))
* use release-please@^17.3.0 instead of ^19.0.0 ([5149476](https://github.com/dallay/agentsync/commit/51494763b9dc362789f1e81c97ddc11db3bda147))


### Performance

* Deduplicate MCP shared paths and skip redundant writes ([#227](https://github.com/dallay/agentsync/issues/227)) ([5edafc7](https://github.com/dallay/agentsync/commit/5edafc7f540eea395caa6144dc39da066cf05b64))
* optimize configuration management and serialization ([be830a7](https://github.com/dallay/agentsync/commit/be830a70accbcd15122214ea6b5e074ad2e77b55))
* optimize configuration management using BTreeMap ([f5fa2d7](https://github.com/dallay/agentsync/commit/f5fa2d7c94678eca29fc2c34136a3fe02e8fa186))


### Documentation

* update README.md for accuracy and project standards ([#228](https://github.com/dallay/agentsync/issues/228)) ([a1cf760](https://github.com/dallay/agentsync/commit/a1cf760b3abbc0e3c185576bf7afe547e3f40efb))


### Chores

* **deps:** update dependency astro to v5.18.0 ([f8b5c94](https://github.com/dallay/agentsync/commit/f8b5c947322547e267794d364f70df5e2a7881e1))
* **deps:** update dependency astro to v5.18.1 ([#232](https://github.com/dallay/agentsync/issues/232)) ([1c328b2](https://github.com/dallay/agentsync/commit/1c328b2323f622ac2ed8c069007a9259af989bef))
* **deps:** update devdependencies ([#206](https://github.com/dallay/agentsync/issues/206)) ([f8508af](https://github.com/dallay/agentsync/commit/f8508aff3b9d301143f5396c50bcb43497c9d9f1))
* **deps:** update github actions ([#221](https://github.com/dallay/agentsync/issues/221)) ([42a4349](https://github.com/dallay/agentsync/commit/42a43490303f935a802abec2809a0462e0f1cca6))
* **deps:** update node.js to v24.14.0 ([#222](https://github.com/dallay/agentsync/issues/222)) ([e02702a](https://github.com/dallay/agentsync/commit/e02702afa38a99025ec380978b526a34b6f138d8))
* remove old Spec-Driven Development scripts and templates ([0a9209c](https://github.com/dallay/agentsync/commit/0a9209c4204496894d0525c433da066f9f5e6a24))

## [1.27.2](https://github.com/dallay/agentsync/compare/v1.27.1...v1.27.2) (2026-02-08)

### 🚀 Performance

* optimize AGENTS.md compression by reducing allocations ([#157](https://github.com/dallay/agentsync/issues/157)) ([3bc989b](https://github.com/dallay/agentsync/commit/3bc989b89c878d2a11b021853d4449d0fe8144f3))

### 📝 Documentation

* update README.md for accuracy ([#156](https://github.com/dallay/agentsync/issues/156)) ([ab6c304](https://github.com/dallay/agentsync/commit/ab6c304f4c463fccd94495c41b1cd169a8dbb07b))

## [1.27.1](https://github.com/dallay/agentsync/compare/v1.27.0...v1.27.1) (2026-02-07)

### 🐛 Bug Fixes

* **docs:** improve sidebar active menu item contrast ([#153](https://github.com/dallay/agentsync/issues/153)) ([b90c50c](https://github.com/dallay/agentsync/commit/b90c50c6b051ec70737ccc6d986f92459b41e5a5))

### 📝 Documentation

* Add missing community standard files ([#150](https://github.com/dallay/agentsync/issues/150)) ([aa62397](https://github.com/dallay/agentsync/commit/aa62397837ccc980176ac91558530d5966cc66fa))
* enhance README.md with improved installation instructions and checksum verification ([#152](https://github.com/dallay/agentsync/issues/152)) ([80c46c6](https://github.com/dallay/agentsync/commit/80c46c6165876c4649f948b6d00b95a2a739d086))
* update README.md for installation instructions and accuracy ([#143](https://github.com/dallay/agentsync/issues/143)) ([917fb54](https://github.com/dallay/agentsync/commit/917fb544d6e59c51e992ed7bc4247f191be3e4f2))

## [1.27.0](https://github.com/dallay/agentsync/compare/v1.26.2...v1.27.0) (2026-02-07)

### ✨ Features

* **mcp:** add Codex MCP support and unify agent alias resolution ([#140](https://github.com/dallay/agentsync/issues/140)) ([0013d71](https://github.com/dallay/agentsync/commit/0013d7116630686d5efa7fe283fa937d2c6f67a1))

### 🐛 Bug Fixes

* make JSON MCP output ordering deterministic ([#141](https://github.com/dallay/agentsync/issues/141)) ([f308f2f](https://github.com/dallay/agentsync/commit/f308f2f880b73bcba77b75da538efcad059035d2))

## [1.26.2](https://github.com/dallay/agentsync/compare/v1.26.1...v1.26.2) (2026-02-07)

### ♻️ Refactors

* **ci:** migrate workflows to dallay/common-actions reusable workflows ([#142](https://github.com/dallay/agentsync/issues/142)) ([6a07053](https://github.com/dallay/agentsync/commit/6a0705385b3b7b0bf69f1dc482f37dcf6e34f73e))

## [1.26.1](https://github.com/dallay/agentsync/compare/v1.26.0...v1.26.1) (2026-02-07)

### 🐛 Bug Fixes

* update Docker base image to 22.04 and add cleanup for Docker resources in CI ([#138](https://github.com/dallay/agentsync/issues/138)) ([97cc5a3](https://github.com/dallay/agentsync/commit/97cc5a369b5fb459ab37592629e5eed9b85c6f32))

## [1.26.0](https://github.com/dallay/agentsync/compare/v1.25.0...v1.26.0) (2026-02-06)

### ✨ Features

* add agentsync doctor command for advanced diagnostics ([#137](https://github.com/dallay/agentsync/issues/137)) ([cc9f040](https://github.com/dallay/agentsync/commit/cc9f040eb57350b6af4ee9549f1f82ddc29ff511))

### 📝 Documentation

* update README.md for accuracy and include MCP servers ([#136](https://github.com/dallay/agentsync/issues/136)) ([a7a90c6](https://github.com/dallay/agentsync/commit/a7a90c6021ff9eb3a1c2676e5925539cddf9051c))

## [1.25.0](https://github.com/dallay/agentsync/compare/v1.24.0...v1.25.0) (2026-02-05)

### ✨ Features

* add '@biomejs/*' to minimumReleaseAgeExclude in pnpm workspace configuration ([76d7da1](https://github.com/dallay/agentsync/commit/76d7da1cb5f49868b4f8159f463515a6e61d4b65))
* add interactive configuration wizard and default agents support ([#133](https://github.com/dallay/agentsync/issues/133)) ([2f3d290](https://github.com/dallay/agentsync/commit/2f3d290343db2cbfc5ee1dcf97ce092c6d48a960))

### 🚀 Performance

* optimize MCP config generation by avoiding redundant deep clones ([#132](https://github.com/dallay/agentsync/issues/132)) ([067bee8](https://github.com/dallay/agentsync/commit/067bee80547ba4893a32c45ee945d735297d2589))

## [1.24.0](https://github.com/dallay/agentsync/compare/v1.23.1...v1.24.0) (2026-02-05)

### ✨ Features

* add ASCII banner to CLI output ([#129](https://github.com/dallay/agentsync/issues/129)) ([b58b28f](https://github.com/dallay/agentsync/commit/b58b28fc4ec42f63ce44bc45b03ac4277fe90e50))

### 📝 Documentation

* remove unimplemented agentsync skill list command ([#126](https://github.com/dallay/agentsync/issues/126)) ([2b090c7](https://github.com/dallay/agentsync/commit/2b090c76b017e4e518c5f440a75c0f6969dec0cb))

## [1.23.1](https://github.com/dallay/agentsync/compare/v1.23.0...v1.23.1) (2026-02-05)

### 🐛 Bug Fixes

* update bytes crate to v1.11.1 to resolve security vulnerability ([#128](https://github.com/dallay/agentsync/issues/128)) ([dc863ea](https://github.com/dallay/agentsync/commit/dc863ea43ca5ba9cebb7da567c5673d0810e7732))

## [1.23.0](https://github.com/dallay/agentsync/compare/v1.22.0...v1.23.0) (2026-02-03)

### ✨ Features

* Add interactive wizard for migrating existing agent configurations ([#117](https://github.com/dallay/agentsync/issues/117)) ([fa33554](https://github.com/dallay/agentsync/commit/fa3355434e93e72a8a418b1bef1839acf7447309))

## [1.22.0](https://github.com/dallay/agentsync/compare/v1.21.2...v1.22.0) (2026-02-03)

### ✨ Features

* skills sh integration ([#123](https://github.com/dallay/agentsync/issues/123)) ([3a2a7ef](https://github.com/dallay/agentsync/commit/3a2a7efde50ea592f01ec31deca67d4fd82b50b8))

## [1.21.2](https://github.com/dallay/agentsync/compare/v1.21.1...v1.21.2) (2026-02-01)

### 🚀 Performance

* optimize gitignore entry generation ([#115](https://github.com/dallay/agentsync/issues/115)) ([9b82489](https://github.com/dallay/agentsync/commit/9b8248944a79e69c8f00ac286ef4b5df8646f27c))

## [1.21.1](https://github.com/dallay/agentsync/compare/v1.21.0...v1.21.1) (2026-02-01)

### 🐛 Bug Fixes

* use stable version tags for GitHub Actions ([#112](https://github.com/dallay/agentsync/issues/112)) ([753f88c](https://github.com/dallay/agentsync/commit/753f88c72da79b4286a392b819cad02ffcd4ebe6))

### 📝 Documentation

* update README.md for CLI accuracy and supported agents ([#111](https://github.com/dallay/agentsync/issues/111)) ([fec5ad3](https://github.com/dallay/agentsync/commit/fec5ad3d3e6f563b296196bc97322b3b2757e1d4))

## [1.21.0](https://github.com/dallay/agentsync/compare/v1.20.0...v1.21.0) (2026-01-31)

### ✨ Features

* enhance variable substitution with logging and robust tests ([30b1025](https://github.com/dallay/agentsync/commit/30b102521c4aa7c7dc9e2ff8259681bdf7a68b44))
* implement variable substitution (templating) for instruction files ([375aabb](https://github.com/dallay/agentsync/commit/375aabb4d022cc55123eaabad61df8434e72e0b7))
* implement variable substitution (templating) for instruction files ([d370f16](https://github.com/dallay/agentsync/commit/d370f167117513d07f8bc1bdf50d44823e37cb2c))

## [1.20.0](https://github.com/dallay/agentsync/compare/v1.19.0...v1.20.0) (2026-01-31)

### ✨ Features

* add GitHub Actions for label management and synchronization ([77e0e08](https://github.com/dallay/agentsync/commit/77e0e08e8708542f9b925266be635c113c84870b))
* enhance issue labeler with word-boundary regex matching ([1341a0d](https://github.com/dallay/agentsync/commit/1341a0df9a7939aeb5a2d3a90cfa970feb829907))
* refine GitHub label management and labeling rules ([90d07e6](https://github.com/dallay/agentsync/commit/90d07e6384451d50bdf66b80c77d0579fa5c4609))

### 📝 Documentation

* add comprehensive installation instructions to README ([3459052](https://github.com/dallay/agentsync/commit/34590521ceb82a5a2548c675f60382d85d39adae))
* add comprehensive installation instructions with MD022/MD031 fixes ([f7d7f19](https://github.com/dallay/agentsync/commit/f7d7f19cf7d4cf15fb06397a8b049c39eab70a88))
* audit and update documentation for CLI accuracy and mono-repo structure ([#104](https://github.com/dallay/agentsync/issues/104)) ([cd04aec](https://github.com/dallay/agentsync/commit/cd04aec45bbeec8fe13780913518ac419dbc824a))
* refine Bun and Yarn installation instructions in README ([0baabb1](https://github.com/dallay/agentsync/commit/0baabb18fc5835066755e7f7b3c14184664d9ca1))
* separate global install from one-off execution in README ([8fcb5ce](https://github.com/dallay/agentsync/commit/8fcb5ce2a989b0504a64d49166151bd6222b70de))

## [1.19.0](https://github.com/dallay/agentsync/compare/v1.18.0...v1.19.0) (2026-01-30)

### ✨ Features

* **release:** build x86_64-unknown-linux-gnu natively instead of using cross ([ef1a2e7](https://github.com/dallay/agentsync/commit/ef1a2e7b18650ede524d6f9883e0ed7e72d0f2ef))

## [1.18.0](https://github.com/dallay/agentsync/compare/v1.17.0...v1.18.0) (2026-01-30)

### ✨ Features

* **ci:** install gcc-12 on ubuntu runners to avoid aws-lc-sys gcc bug ([0847d01](https://github.com/dallay/agentsync/commit/0847d0119b6ccf0d94bd983a978dc18d4d7d14ec))

## [1.17.0](https://github.com/dallay/agentsync/compare/v1.16.0...v1.17.0) (2026-01-30)

### ✨ Features

* add status command to cli ([#96](https://github.com/dallay/agentsync/issues/96)) ([8708f8a](https://github.com/dallay/agentsync/commit/8708f8aa58566826d80f50c83b69a435e94b1a37))

### 📝 Documentation

* expand skills documentation and agent support ([#95](https://github.com/dallay/agentsync/issues/95)) ([5beae14](https://github.com/dallay/agentsync/commit/5beae142f951cbb5369a140a99fd346576fd0d26))

## [1.16.0](https://github.com/dallay/agentsync/compare/v1.15.0...v1.16.0) (2026-01-30)

### ✨ Features

* add Cursor MCP support ([#90](https://github.com/dallay/agentsync/issues/90)) ([c49235e](https://github.com/dallay/agentsync/commit/c49235e08a1c7a215847551090226236758abdff))

## [1.15.0](https://github.com/dallay/agentsync/compare/v1.14.5...v1.15.0) (2026-01-30)

### ✨ Features

* skills sh integration ([#94](https://github.com/dallay/agentsync/issues/94)) ([051908f](https://github.com/dallay/agentsync/commit/051908f88c6c4aa092b1dce74809df2b391ccae8))

### 🚀 Performance

* Optimize glob pattern matching in `linker.rs` ([#76](https://github.com/dallay/agentsync/issues/76)) ([5b48c0a](https://github.com/dallay/agentsync/commit/5b48c0a14a66e26f6319ee9b864371b2da678d73)), closes [#87](https://github.com/dallay/agentsync/issues/87) [#78](https://github.com/dallay/agentsync/issues/78) [#83](https://github.com/dallay/agentsync/issues/83) [#84](https://github.com/dallay/agentsync/issues/84) [#81](https://github.com/dallay/agentsync/issues/81) [#77](https://github.com/dallay/agentsync/issues/77) [#75](https://github.com/dallay/agentsync/issues/75)

### 📝 Documentation

* Correct OpenCode MCP path in README ([#77](https://github.com/dallay/agentsync/issues/77)) ([8783592](https://github.com/dallay/agentsync/commit/8783592bcb7ead428d9c0fc2468926937ac04136))
* update README.md for accuracy ([#75](https://github.com/dallay/agentsync/issues/75)) ([278372e](https://github.com/dallay/agentsync/commit/278372e90292fbcdf88013a2938f8c289835a6e4))
* Update README.md with accurate CLI usage ([#84](https://github.com/dallay/agentsync/issues/84)) ([7f3306b](https://github.com/dallay/agentsync/commit/7f3306bbb9fa64c11bbc6ea881c44230e8624eac))

## [1.14.5](https://github.com/dallay/agentsync/compare/v1.14.4...v1.14.5) (2026-01-24)

### 🐛 Bug Fixes

* **sync-optional-deps:** handle missing package.json and safely update file in place ([ad95603](https://github.com/dallay/agentsync/commit/ad95603d041f04d709876ac26006cf6b8c9e7aee))

## [1.14.4](https://github.com/dallay/agentsync/compare/v1.14.3...v1.14.4) (2026-01-24)

### 🐛 Bug Fixes

* **setup:** skip agents:sync step in CI environments ([da3e7e7](https://github.com/dallay/agentsync/commit/da3e7e7133f5a2d65311600e0ed3bd8924c229a5))

## [1.14.3](https://github.com/dallay/agentsync/compare/v1.14.2...v1.14.3) (2026-01-24)

### 🐛 Bug Fixes

* **scripts:** avoid TOCTOU by updating files via file descriptor ([#68](https://github.com/dallay/agentsync/issues/68)) ([b130af5](https://github.com/dallay/agentsync/commit/b130af516d59b7cbb06ba999f57483abea01a824))

### 📝 Documentation

* Add clean command to README usage section ([bea06cf](https://github.com/dallay/agentsync/commit/bea06cf17a65fd4bbfd449f53ecd4d18a96912f2))
* add project logo to README; update favicon with new design ([b099034](https://github.com/dallay/agentsync/commit/b099034294025020b8fe661c0cb0ae025a426c51))
* Audit and update all project documentation ([e8ba33e](https://github.com/dallay/agentsync/commit/e8ba33e8ddd8e38217a902eca0d3ab9898a6d9f3))

## [1.14.2](https://github.com/dallay/agentsync/compare/v1.14.1...v1.14.2) (2026-01-23)

### 🐛 Bug Fixes

* **config:** update site and base settings in Astro config; bump @astrojs/starlight to 0.37.4 ([60564f5](https://github.com/dallay/agentsync/commit/60564f51bb5e761452fd78e92cf7d0e0c5d2babb))
* **deps:** downgrade @astrojs/starlight to 0.37.3; update pnpm lockfile with agentsync 1.14.1 optional dependencies ([9e24dfc](https://github.com/dallay/agentsync/commit/9e24dfc230bd3f7d9d30d94c26e5d5c552f4cd7c))

## [1.14.1](https://github.com/dallay/agentsync/compare/v1.14.0...v1.14.1) (2026-01-23)

### 🐛 Bug Fixes

* **config:** update site URL to dallay.github.io/agentsync in Astro config ([13a2957](https://github.com/dallay/agentsync/commit/13a295721560a76a9487c6d217938ae717648508))

## [1.14.0](https://github.com/dallay/agentsync/compare/v1.13.0...v1.14.0) (2026-01-23)

### ✨ Features

* **ci:** update deploy-docs workflow to use --no-frozen-lockfile for pnpm install ([dbca1c6](https://github.com/dallay/agentsync/commit/dbca1c62ce56aeb4f98a689f98cd929273cfca26))

## [1.13.0](https://github.com/dallay/agentsync/compare/v1.12.0...v1.13.0) (2026-01-23)

### ✨ Features

* **ci:** update checkout action and pnpm install flags in workflows ([6381b76](https://github.com/dallay/agentsync/commit/6381b76be4ea9d068aefab00a443d782b2f937c2))

## [1.12.0](https://github.com/dallay/agentsync/compare/v1.11.0...v1.12.0) (2026-01-23)

### ✨ Features

* **deps:** release 1.x.x ([d03622c](https://github.com/dallay/agentsync/commit/d03622c42ff31c3d603ea1de964c467a15750366))

## [1.11.0](https://github.com/dallay/agentsync/compare/v1.10.0...v1.11.0) (2026-01-23)

### ✨ Features

* added documentation website ([cc2d328](https://github.com/dallay/agentsync/commit/cc2d328830c324b22a88dfecd742a32e76da42e3))

### 📝 Documentation

* add agentsync-docs workspace and npm scripts for documentation site ([d954b19](https://github.com/dallay/agentsync/commit/d954b1939f41027375f115ba7ff732c6c9846186))
* fix JSON formatting in getting started script example ([b3ce430](https://github.com/dallay/agentsync/commit/b3ce43079df04f30aae81bc0ff98eabc70d477a8))
* fix JSON formatting in getting started script example ([e855f55](https://github.com/dallay/agentsync/commit/e855f551995eed9a24d07d0f2435274735094d2b))
* update OpenCode documentation reference in agent table ([ffe5b5e](https://github.com/dallay/agentsync/commit/ffe5b5e2a8a41182dd791a203ca7121da40c0bf2))

## [1.10.0](https://github.com/dallay/agentsync/compare/v1.9.0...v1.10.0) (2026-01-22)

### ✨ Features

* Cache fs::canonicalize results to reduce I/O ([a19fe36](https://github.com/dallay/agentsync/commit/a19fe3685001d8463faf87acaf95d39308057071))

### 📝 Documentation

* add initial README with usage, configuration, and contribution guidelines ([b03390d](https://github.com/dallay/agentsync/commit/b03390dedaeaadd3b3c7755199adc70f04e11e70))

## [1.9.0](https://github.com/dallay/agentsync/compare/v1.8.3...v1.9.0) (2026-01-22)

### ✨ Features

* add OpenAI Codex CLI support ([7f3ac8c](https://github.com/dallay/agentsync/commit/7f3ac8ce31a9fdbc739735db2ed0e998907fccfe)), closes [#51](https://github.com/dallay/agentsync/issues/51)

## [1.8.3](https://github.com/dallay/agentsync/compare/v1.8.2...v1.8.3) (2026-01-22)

### 🐛 Bug Fixes

* **ci:** correct quoting and escaping for regex that verifies packaged binary executable bit ([35a957e](https://github.com/dallay/agentsync/commit/35a957ef8b290743ff25d3d8a0370b15a8a5e62c))
* **ci:** ensure agentsync binary is executable and validate before publish ([78b47dc](https://github.com/dallay/agentsync/commit/78b47dc86ce53b390a81e6f532b65281d624eb97))
* **ci:** update tar permission regex to correctly validate owner execute bit for packaged binary ([544bfa8](https://github.com/dallay/agentsync/commit/544bfa817231578cedc00c60689f78b65306dd30))

## [1.8.2](https://github.com/dallay/agentsync/compare/v1.8.1...v1.8.2) (2026-01-17)

### 🐛 Bug Fixes

* **opencode:** use standard opencode.json path and add schema validation ([fab593b](https://github.com/dallay/agentsync/commit/fab593b34d5c2c07078efc8bf84a4f23dc038168))

## [1.8.1](https://github.com/dallay/agentsync/compare/v1.8.0...v1.8.1) (2026-01-17)

### 🐛 Bug Fixes

* **docker:** remove pinned apk versions to fix build failure ([702f6c3](https://github.com/dallay/agentsync/commit/702f6c31a3ed8598afe4bd86a3783d6bb97e86c8))

## [1.8.0](https://github.com/dallay/agentsync/compare/v1.7.0...v1.8.0) (2026-01-17)

### ✨ Features

* add AgentSync configuration, AI agent symlink management, and Rust skill docs ([9c7cd87](https://github.com/dallay/agentsync/commit/9c7cd87d8f3116ae4180922ae5161b37dd070d2a))
* **docs:** improve CLI and configuration documentation ([599a39d](https://github.com/dallay/agentsync/commit/599a39d039561b269ad8b830c1e902906801c07d))
* **docs:** improve CLI and configuration documentation ([2b9f349](https://github.com/dallay/agentsync/commit/2b9f349a55b5b4582c400d198a8df0a369ef5391))

### 🐛 Bug Fixes

* **agentsync:** correct symlink destination in config and improve prepare script for cross-platform compatibility ([a9c6039](https://github.com/dallay/agentsync/commit/a9c6039b261b52800339be497cb0c32acef3a422))
* **ci:** sync pnpm-lock.yaml and migrate devDependencies to catalog ([d1079bf](https://github.com/dallay/agentsync/commit/d1079bf3f4ad09d31f97c73549e0741382a8a2c5))
* **greetings:** update action input keys to use underscores and switch to double quotes ([c99ecc9](https://github.com/dallay/agentsync/commit/c99ecc97f2b351f20181c6bd9f7daff9cb6d8af9))
* **renovate:** add schema, unify schedules, enable vulnerability alerts, and refine grouping ([99786aa](https://github.com/dallay/agentsync/commit/99786aa75bfd2604218a9be39fb0c096513729d1))
* **renovate:** remove  top-level key to satisfy Renovate config schema ([d9bd246](https://github.com/dallay/agentsync/commit/d9bd24679cade28b4eba9941f6f74ae4caac6437))
* simplify Cargo.toml version replacement and add debug logs ([178a7ca](https://github.com/dallay/agentsync/commit/178a7ca548d428d6b7bed0922c15e3fe3efe721a))

### 📝 Documentation

* Improve project documentation ([2cae016](https://github.com/dallay/agentsync/commit/2cae016de13d4a96e7551af2d994f031b7fbc771))
* use professional placeholders for branch and commit examples in README ([5718ca2](https://github.com/dallay/agentsync/commit/5718ca2aee4f573469000b91759cc5574ae91eec))

## [1.7.0](https://github.com/dallay/agentsync/compare/v1.6.0...v1.7.0) (2026-01-16)

### ✨ Features

* migrate package naming to [@dallay](https://github.com/dallay) scope and update platform dependencies ([0b9d351](https://github.com/dallay/agentsync/commit/0b9d351755664282b8a4cd67da3ddad4868985ae))

## [1.6.0](https://github.com/dallay/agentsync/compare/v1.5.0...v1.6.0) (2026-01-16)

### ✨ Features

* first release 0.1.0 ([743f490](https://github.com/dallay/agentsync/commit/743f490d2feabfcf418880f16a5737b8f40ac863))
* first release 0.1.0 ([041d51d](https://github.com/dallay/agentsync/commit/041d51d1adcfac2ed9695fddb7e45bd7a072ba03))

# Agent Sync Changelog
