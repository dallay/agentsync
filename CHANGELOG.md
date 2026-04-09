## [1.38.0](https://github.com/dallay/agentsync/compare/v1.37.0...v1.38.0) (2026-04-03)


### Features

* add dallay/agents-skills deterministic resolution and catalog entries ([#296](https://github.com/dallay/agentsync/issues/296)) ([cdcd4ab](https://github.com/dallay/agentsync/commit/cdcd4ab378e01f4a3efb6ac2fb40f2d002418da2))


### Performance

* implement caching for NestedGlob and project root ([#293](https://github.com/dallay/agentsync/issues/293)) ([039a868](https://github.com/dallay/agentsync/commit/039a86827fb20c07b907a9ace47e2a62c25a3f7c))


### Documentation

* add managed agent config layout section to wizard-generated AGENTS.md ([705836b](https://github.com/dallay/agentsync/commit/705836b58c6ddd5d8245d0b35ba2951e4b2f8cc7))
* improve formatting and clarity in proposal, spec, and tasks documentation ([93d5eb3](https://github.com/dallay/agentsync/commit/93d5eb3883c1bebd75289f362ec2f3b469f064e2))
* mark TASK-07 as complete in tasks.md ([cca30e6](https://github.com/dallay/agentsync/commit/cca30e6c82adee247c3e198f005c7dc25d633c09))


### Chores

* **deps:** bump the npm_and_yarn group across 2 directories with 1 update ([#289](https://github.com/dallay/agentsync/issues/289)) ([0d72b77](https://github.com/dallay/agentsync/commit/0d72b77028142d2ffe8c9178716c3a9bd72cf7b8))
* **deps:** lock file maintenance ([#292](https://github.com/dallay/agentsync/issues/292)) ([244590a](https://github.com/dallay/agentsync/commit/244590a425b437ea597f7a9af8920deeb69e3aa9))
* **deps:** update dependency @dallay/agentsync to v1.37.0 ([#291](https://github.com/dallay/agentsync/issues/291)) ([b4e2fb8](https://github.com/dallay/agentsync/commit/b4e2fb846d19d973f3b423d25c46c3cb6a4e6afa))
* **deps:** update dependency @iconify/json to v2.2.458 ([#294](https://github.com/dallay/agentsync/issues/294)) ([0cc22fe](https://github.com/dallay/agentsync/commit/0cc22fe236db2ee9de30e7543d8da8152c70d3b0))
* **deps:** update docker/login-action action to v4.1.0 ([#295](https://github.com/dallay/agentsync/issues/295)) ([4fb81a5](https://github.com/dallay/agentsync/commit/4fb81a58ee3412fc85d045b5b662cca25017ca79))
* **deps:** update rust crate zip to v8.5.0 ([#285](https://github.com/dallay/agentsync/issues/285)) ([56b5cef](https://github.com/dallay/agentsync/commit/56b5cef976b16aab53b55ea16cc0183f93b10274))

## [1.42.4](https://github.com/dallay/agentsync/compare/v1.42.3...v1.42.4) (2026-04-09)


### Bug Fixes

* **security:** restrict permissions on generated MCP configs ([#331](https://github.com/dallay/agentsync/issues/331)) ([8b0ff43](https://github.com/dallay/agentsync/commit/8b0ff43b4d0bd2d30dc203d437816f7f3958c4ee))

## [1.42.3](https://github.com/dallay/agentsync/compare/v1.42.2...v1.42.3) (2026-04-06)


### Bug Fixes

* **install:** cleanup download.tmp after extracting remote skill archives ([7344cc4](https://github.com/dallay/agentsync/commit/7344cc4c2e83b4d0edea0595d7312399c3f33600))
* **install:** cleanup download.tmp after extracting remote skill archives ([#330](https://github.com/dallay/agentsync/issues/330)) ([d9ddad8](https://github.com/dallay/agentsync/commit/d9ddad8d018b6db1777167c5311d62b19039b86c))


### Documentation

* improve formatting and clarity in documentation files ([90c6d9b](https://github.com/dallay/agentsync/commit/90c6d9b1a678cdb9d429e92264bb12ff556d0215))

## [1.42.2](https://github.com/dallay/agentsync/compare/v1.42.1...v1.42.2) (2026-04-06)


### Bug Fixes

* resolve PR review issues ([#327](https://github.com/dallay/agentsync/issues/327)) ([ecad40f](https://github.com/dallay/agentsync/commit/ecad40f9cf3fbfe415ad39c51cd3d1ab330d281f))


### Performance

* optimize linker path processing and validation ([#318](https://github.com/dallay/agentsync/issues/318)) ([d114693](https://github.com/dallay/agentsync/commit/d114693967e4a17beef38688a386d14b5c281cf5))

## [1.42.1](https://github.com/dallay/agentsync/compare/v1.42.0...v1.42.1) (2026-04-06)


### Bug Fixes

* harden catalog skill installation validation ([9ebd5ac](https://github.com/dallay/agentsync/commit/9ebd5ac1148d4331da81a40b61cfcd5f08918b74))
* tighten catalog install source resolution ([97f5f71](https://github.com/dallay/agentsync/commit/97f5f71ea69836977827905dc316b04e8a447032))


### Tests

* update catalog combo expectations ([be6932d](https://github.com/dallay/agentsync/commit/be6932d19c9fe37d84fe6d22aa44ae540b1ef3ba))


### Chores

* **deps:** lock file maintenance ([#321](https://github.com/dallay/agentsync/issues/321)) ([f10842d](https://github.com/dallay/agentsync/commit/f10842dfbbec27c80ee8464a19c1019dc0a6789d))

## [1.42.0](https://github.com/dallay/agentsync/compare/v1.41.0...v1.42.0) (2026-04-05)


### Features

* add '@biomejs/*' to minimumReleaseAgeExclude in pnpm workspace configuration ([76d7da1](https://github.com/dallay/agentsync/commit/76d7da1cb5f49868b4f8159f463515a6e61d4b65))
* add `nested-glob` target type for monorepo/multi-module AGENTS.md discovery ([#234](https://github.com/dallay/agentsync/issues/234)) ([4ea1d59](https://github.com/dallay/agentsync/commit/4ea1d59f7fec6dbf237e1f6dd5abdf3715d5326e))
* add AgentSync configuration, AI agent symlink management, and Rust skill docs ([9c7cd87](https://github.com/dallay/agentsync/commit/9c7cd87d8f3116ae4180922ae5161b37dd070d2a))
* add agentsync doctor command for advanced diagnostics ([#137](https://github.com/dallay/agentsync/issues/137)) ([cc9f040](https://github.com/dallay/agentsync/commit/cc9f040eb57350b6af4ee9549f1f82ddc29ff511))
* add ASCII banner to CLI output ([#129](https://github.com/dallay/agentsync/issues/129)) ([b58b28f](https://github.com/dallay/agentsync/commit/b58b28fc4ec42f63ce44bc45b03ac4277fe90e50))
* add background version check with local cache ([d6b66e6](https://github.com/dallay/agentsync/commit/d6b66e635a142abb98fc7c32aaac13df5ee88463)), closes [#242](https://github.com/dallay/agentsync/issues/242)
* add Cursor MCP support ([#90](https://github.com/dallay/agentsync/issues/90)) ([c49235e](https://github.com/dallay/agentsync/commit/c49235e08a1c7a215847551090226236758abdff))
* add dallay/agents-skills deterministic resolution and catalog entries ([#296](https://github.com/dallay/agentsync/issues/296)) ([cdcd4ab](https://github.com/dallay/agentsync/commit/cdcd4ab378e01f4a3efb6ac2fb40f2d002418da2))
* add Docker support and publish workflow to release pipeline ([40f16a2](https://github.com/dallay/agentsync/commit/40f16a25c6a91918ba3aba29eaa2ecc72711eba8))
* add Docker support and publish workflow to release pipeline ([62c8ac0](https://github.com/dallay/agentsync/commit/62c8ac02a725521b250909f910f652dd6b9ffdc1))
* add GitHub Actions for label management and synchronization ([77e0e08](https://github.com/dallay/agentsync/commit/77e0e08e8708542f9b925266be635c113c84870b))
* add GitHub URL conversion for ZIP downloads and implement condition-based waiting skill ([#160](https://github.com/dallay/agentsync/issues/160)) ([86be61b](https://github.com/dallay/agentsync/commit/86be61b94fa83f3d9fc106fb6e6e4714a44fbb47))
* add interactive configuration wizard and default agents support ([#133](https://github.com/dallay/agentsync/issues/133)) ([2f3d290](https://github.com/dallay/agentsync/commit/2f3d290343db2cbfc5ee1dcf97ce092c6d48a960))
* Add interactive wizard for migrating existing agent configurations ([#117](https://github.com/dallay/agentsync/issues/117)) ([fa33554](https://github.com/dallay/agentsync/commit/fa3355434e93e72a8a418b1bef1839acf7447309))
* add MCP (Model Context Protocol) config generation for Claude, Copilot, Gemini, VS Code, and OpenCode ([82ddf0a](https://github.com/dallay/agentsync/commit/82ddf0ac53c76f95a7e71c38f59c2e213d001b78))
* add MCP (Model Context Protocol) config generation for Claude, Copilot, Gemini, VS Code, and OpenCode ([f555e95](https://github.com/dallay/agentsync/commit/f555e95f7afcf282af31f6f5554ab42d35e4fbb0))
* add NPM wrapper for npx distribution ([0658101](https://github.com/dallay/agentsync/commit/0658101f7cf48fca2229caf849ecec3812b94ba2))
* add npx wrapper for agentsync and publish platform-specific NPM packages ([28a56a2](https://github.com/dallay/agentsync/commit/28a56a214cd806e186702fd1df7df711588a6c97))
* add npx wrapper for agentsync and publish platform-specific NPM packages ([3f2ed71](https://github.com/dallay/agentsync/commit/3f2ed71224d8b732a7b7e8df99dac283f5f7e7eb))
* add OpenAI Codex CLI support ([7f3ac8c](https://github.com/dallay/agentsync/commit/7f3ac8ce31a9fdbc739735db2ed0e998907fccfe)), closes [#51](https://github.com/dallay/agentsync/issues/51)
* add repository-based skill suggestions ([#272](https://github.com/dallay/agentsync/issues/272)) ([a534ace](https://github.com/dallay/agentsync/commit/a534ace5b8542cc800489f6a017dce48bd06cd48))
* add status command to cli ([#96](https://github.com/dallay/agentsync/issues/96)) ([8708f8a](https://github.com/dallay/agentsync/commit/8708f8aa58566826d80f50c83b69a435e94b1a37))
* Add support for 34 new AI agents ([#195](https://github.com/dallay/agentsync/issues/195)) ([49a576d](https://github.com/dallay/agentsync/commit/49a576d3fa95dbf42ecad589e5bca8f21b464d93))
* add wizard agent config layout guidance ([fa435fe](https://github.com/dallay/agentsync/commit/fa435feb0e7afe0532a9d1e24faf878768f9f715))
* added documentation website ([cc2d328](https://github.com/dallay/agentsync/commit/cc2d328830c324b22a88dfecd742a32e76da42e3))
* Cache fs::canonicalize results to reduce I/O ([a19fe36](https://github.com/dallay/agentsync/commit/a19fe3685001d8463faf87acaf95d39308057071))
* **ci:** install gcc-12 on ubuntu runners to avoid aws-lc-sys gcc bug ([0847d01](https://github.com/dallay/agentsync/commit/0847d0119b6ccf0d94bd983a978dc18d4d7d14ec))
* **ci:** update checkout action and pnpm install flags in workflows ([6381b76](https://github.com/dallay/agentsync/commit/6381b76be4ea9d068aefab00a443d782b2f937c2))
* **ci:** update deploy-docs workflow to use --no-frozen-lockfile for pnpm install ([dbca1c6](https://github.com/dallay/agentsync/commit/dbca1c62ce56aeb4f98a689f98cd929273cfca26))
* complete release workflow with docker support and security hardening ([e5ef7eb](https://github.com/dallay/agentsync/commit/e5ef7eb0f8e9fa11cef5a0dc7991fbc92a26297d))
* **deps:** release 1.x.x ([d03622c](https://github.com/dallay/agentsync/commit/d03622c42ff31c3d603ea1de964c467a15750366))
* detect and migrate existing agent skills, commands, and configs during init wizard ([#259](https://github.com/dallay/agentsync/issues/259)) ([a3a9802](https://github.com/dallay/agentsync/commit/a3a980232901e70cc267dd4b72fc10b81df3c229))
* **docs:** improve CLI and configuration documentation ([599a39d](https://github.com/dallay/agentsync/commit/599a39d039561b269ad8b830c1e902906801c07d))
* **docs:** improve CLI and configuration documentation ([2b9f349](https://github.com/dallay/agentsync/commit/2b9f349a55b5b4582c400d198a8df0a369ef5391))
* enhance issue labeler with word-boundary regex matching ([1341a0d](https://github.com/dallay/agentsync/commit/1341a0df9a7939aeb5a2d3a90cfa970feb829907))
* Enhance skill installation with provider ID resolution and UX improvements ([#319](https://github.com/dallay/agentsync/issues/319)) ([21533c2](https://github.com/dallay/agentsync/commit/21533c26955acc5f6c62968d81b529781ab83efd))
* enhance variable substitution with logging and robust tests ([30b1025](https://github.com/dallay/agentsync/commit/30b102521c4aa7c7dc9e2ff8259681bdf7a68b44))
* first release 0.1.0 ([743f490](https://github.com/dallay/agentsync/commit/743f490d2feabfcf418880f16a5737b8f40ac863))
* first release 0.1.0 ([041d51d](https://github.com/dallay/agentsync/commit/041d51d1adcfac2ed9695fddb7e45bd7a072ba03))
* ignore backup files in .gitignore by default ([#179](https://github.com/dallay/agentsync/issues/179)) ([fa2ab34](https://github.com/dallay/agentsync/commit/fa2ab34d154dc87d440c220eda7906a30cbcc55e))
* Implement autoskills discovery and update Windows setup documentation ([#288](https://github.com/dallay/agentsync/issues/288)) ([f6fb8c8](https://github.com/dallay/agentsync/commit/f6fb8c870a4ab4bd9a3b63d140225c002add11a1))
* implement variable substitution (templating) for instruction files ([375aabb](https://github.com/dallay/agentsync/commit/375aabb4d022cc55123eaabad61df8434e72e0b7))
* implement variable substitution (templating) for instruction files ([d370f16](https://github.com/dallay/agentsync/commit/d370f167117513d07f8bc1bdf50d44823e37cb2c))
* **init:** add bundled agentsync skill installation flow ([2db0403](https://github.com/dallay/agentsync/commit/2db0403647ef03c1584501ebab2f65ac6eb18d7b))
* **init:** expand scan_agent_files to detect 32 agents; add tests; cargo test passes ([#203](https://github.com/dallay/agentsync/issues/203)) ([9b29ac6](https://github.com/dallay/agentsync/commit/9b29ac60bddaf1fbdddcda04475e23b52a8e726d))
* initial release of AgentSync ([04fa108](https://github.com/dallay/agentsync/commit/04fa108cd5ad51f75a436186c6a18b7925e8ebf2))
* **mcp:** add Codex MCP support and unify agent alias resolution ([#140](https://github.com/dallay/agentsync/issues/140)) ([0013d71](https://github.com/dallay/agentsync/commit/0013d7116630686d5efa7fe283fa937d2c6f67a1))
* migrate from semantic-release to release-please ([#237](https://github.com/dallay/agentsync/issues/237)) ([f531f01](https://github.com/dallay/agentsync/commit/f531f010b74457cd292e7a6d91650b0acc446385))
* migrate package naming to [@dallay](https://github.com/dallay) scope and update platform dependencies ([0b9d351](https://github.com/dallay/agentsync/commit/0b9d351755664282b8a4cd67da3ddad4868985ae))
* nested agent context ([#242](https://github.com/dallay/agentsync/issues/242)) ([79bc0fa](https://github.com/dallay/agentsync/commit/79bc0fa5660e09dc799c0b86400b82aeb64bfe53))
* Optimize directory iteration and MCP config generation ([#229](https://github.com/dallay/agentsync/issues/229)) ([9a01e2b](https://github.com/dallay/agentsync/commit/9a01e2bab597ce9416b386beaada3a1427247ad7))
* refine GitHub label management and labeling rules ([90d07e6](https://github.com/dallay/agentsync/commit/90d07e6384451d50bdf66b80c77d0579fa5c4609))
* **release:** build x86_64-unknown-linux-gnu natively instead of using cross ([ef1a2e7](https://github.com/dallay/agentsync/commit/ef1a2e7b18650ede524d6f9883e0ed7e72d0f2ef))
* **release:** reset version to 0.1.0 and initialize changelog ([c7391c7](https://github.com/dallay/agentsync/commit/c7391c7ef933dfed646e0e1a21024cbc22acb70b))
* skills sh integration ([#123](https://github.com/dallay/agentsync/issues/123)) ([3a2a7ef](https://github.com/dallay/agentsync/commit/3a2a7efde50ea592f01ec31deca67d4fd82b50b8))
* skills sh integration ([#94](https://github.com/dallay/agentsync/issues/94)) ([051908f](https://github.com/dallay/agentsync/commit/051908f88c6c4aa092b1dce74809df2b391ccae8))
* **skills:** add agentsync guidance and broaden design recommendations ([#314](https://github.com/dallay/agentsync/issues/314)) ([407de7c](https://github.com/dallay/agentsync/commit/407de7c08576c4a935852dfb843d754af6e636da))
* **suggest:** add 37 technology detections across 9 categories ([541b750](https://github.com/dallay/agentsync/commit/541b750f8a784a6e2b64b2b8678060ef0c7b0797))
* **suggest:** add API technology detection — GraphQL, gRPC, tRPC, OpenAPI ([#309](https://github.com/dallay/agentsync/issues/309)) ([7bae96b](https://github.com/dallay/agentsync/commit/7bae96bc3e2c62a36a0068ff4411edc1d49471bc)), closes [#301](https://github.com/dallay/agentsync/issues/301)
* symlink entire skills directory instead of individual skill entries ([#261](https://github.com/dallay/agentsync/issues/261)) ([c645fa0](https://github.com/dallay/agentsync/commit/c645fa06064f7eb4b1efd87841751e302ca61ccd))
* update actions/checkout version to v6 in CI and release workflows ([34a6a7d](https://github.com/dallay/agentsync/commit/34a6a7d112885ca1545805ad6bfc4d5f6f3fa0b2))
* update first-interaction action version and enhance greeting messages in workflow ([29b3657](https://github.com/dallay/agentsync/commit/29b3657f367aa42b68644d5b9b89c68324f6ea20))
* update Rust toolchain version in release workflow ([0485d00](https://github.com/dallay/agentsync/commit/0485d006bfaadb3a19c6ff0c59649d04cc2f964c))


### Bug Fixes

* add `checks: write` permission for rustsec/audit-check ([#204](https://github.com/dallay/agentsync/issues/204)) ([a3cef14](https://github.com/dallay/agentsync/commit/a3cef1438dda015b998ec37faec308e4c1765a40))
* **agentsync:** correct symlink destination in config and improve prepare script for cross-platform compatibility ([a9c6039](https://github.com/dallay/agentsync/commit/a9c6039b261b52800339be497cb0c32acef3a422))
* **ci:** correct quoting and escaping for regex that verifies packaged binary executable bit ([35a957e](https://github.com/dallay/agentsync/commit/35a957ef8b290743ff25d3d8a0370b15a8a5e62c))
* **ci:** ensure agentsync binary is executable and validate before publish ([78b47dc](https://github.com/dallay/agentsync/commit/78b47dc86ce53b390a81e6f532b65281d624eb97))
* **ci:** grant checks:write permission to audit job ([#196](https://github.com/dallay/agentsync/issues/196)) ([af78e34](https://github.com/dallay/agentsync/commit/af78e3449bb6df5894338fa2cae30df7d5210036))
* **ci:** skip contributor-report for bot-authored PRs ([414b465](https://github.com/dallay/agentsync/commit/414b46555a276c2c443363dd09609d31740762e8))
* **ci:** sync pnpm-lock.yaml and migrate devDependencies to catalog ([d1079bf](https://github.com/dallay/agentsync/commit/d1079bf3f4ad09d31f97c73549e0741382a8a2c5))
* **ci:** update tar permission regex to correctly validate owner execute bit for packaged binary ([544bfa8](https://github.com/dallay/agentsync/commit/544bfa817231578cedc00c60689f78b65306dd30))
* **config:** update site and base settings in Astro config; bump @astrojs/starlight to 0.37.4 ([60564f5](https://github.com/dallay/agentsync/commit/60564f51bb5e761452fd78e92cf7d0e0c5d2babb))
* **config:** update site URL to dallay.github.io/agentsync in Astro config ([13a2957](https://github.com/dallay/agentsync/commit/13a295721560a76a9487c6d217938ae717648508))
* **deps:** downgrade @astrojs/starlight to 0.37.3; update pnpm lockfile with agentsync 1.14.1 optional dependencies ([9e24dfc](https://github.com/dallay/agentsync/commit/9e24dfc230bd3f7d9d30d94c26e5d5c552f4cd7c))
* **deps:** update cargo major upgrades ([2841cdf](https://github.com/dallay/agentsync/commit/2841cdf0c9eff4e27cf87cdf3fb300dcb9b47555))
* **deps:** update major upgrades ([#243](https://github.com/dallay/agentsync/issues/243)) ([abfaca4](https://github.com/dallay/agentsync/commit/abfaca44cac466759120253983e6f270636bc153))
* **docker:** remove pinned apk versions to fix build failure ([702f6c3](https://github.com/dallay/agentsync/commit/702f6c31a3ed8598afe4bd86a3783d6bb97e86c8))
* **docs:** improve sidebar active menu item contrast ([#153](https://github.com/dallay/agentsync/issues/153)) ([b90c50c](https://github.com/dallay/agentsync/commit/b90c50c6b051ec70737ccc6d986f92459b41e5a5))
* **greetings:** update action input keys to use underscores and switch to double quotes ([c99ecc9](https://github.com/dallay/agentsync/commit/c99ecc97f2b351f20181c6bd9f7daff9cb6d8af9))
* improve wizard gitignore workflows and documentation ([#278](https://github.com/dallay/agentsync/issues/278)) ([8d8c44c](https://github.com/dallay/agentsync/commit/8d8c44c009446bb4c5862ffcdf4119f29d5744b8))
* keep a single backup file per destination ([16a057a](https://github.com/dallay/agentsync/commit/16a057ac0c0f451f4ceeb5f7155058d96be94de0))
* **linker:** streamline dry-run directory creation logging ([b2d5b30](https://github.com/dallay/agentsync/commit/b2d5b304f4e10539b39f00d4c35a80dca9d9c077))
* make JSON MCP output ordering deterministic ([#141](https://github.com/dallay/agentsync/issues/141)) ([f308f2f](https://github.com/dallay/agentsync/commit/f308f2f880b73bcba77b75da538efcad059035d2))
* **opencode:** use standard opencode.json path and add schema validation ([fab593b](https://github.com/dallay/agentsync/commit/fab593b34d5c2c07078efc8bf84a4f23dc038168))
* path traversal vulnerability in symlink destinations ([#280](https://github.com/dallay/agentsync/issues/280)) ([6a72530](https://github.com/dallay/agentsync/commit/6a72530e45c5c0ce0c05c79354b04afc87fcdc2e))
* preserve existing skills symlink layouts in init wizard ([#262](https://github.com/dallay/agentsync/issues/262)) ([e81b4c5](https://github.com/dallay/agentsync/commit/e81b4c5e6293d9af929697b8b29d3a6982625ba8))
* remove deprecated CLI tests using cargo_bin ([680893e](https://github.com/dallay/agentsync/commit/680893e4f6816a6870fb6813b3152842591fcdb2))
* **renovate:** add schema, unify schedules, enable vulnerability alerts, and refine grouping ([99786aa](https://github.com/dallay/agentsync/commit/99786aa75bfd2604218a9be39fb0c096513729d1))
* **renovate:** remove  top-level key to satisfy Renovate config schema ([d9bd246](https://github.com/dallay/agentsync/commit/d9bd24679cade28b4eba9941f6f74ae4caac6437))
* replace dtolnay/rust-action with correct rust-toolchain action in CI ([59fd667](https://github.com/dallay/agentsync/commit/59fd6670b8956ec387eceaa8c394913d2d6327ef))
* resolve clippy warnings and formatting issues ([60a7674](https://github.com/dallay/agentsync/commit/60a767478d96f5428f2f8a41cf5004bf5b7696f8))
* **scripts:** avoid TOCTOU by updating files via file descriptor ([#68](https://github.com/dallay/agentsync/issues/68)) ([b130af5](https://github.com/dallay/agentsync/commit/b130af516d59b7cbb06ba999f57483abea01a824))
* **setup:** skip agents:sync step in CI environments ([da3e7e7](https://github.com/dallay/agentsync/commit/da3e7e7133f5a2d65311600e0ed3bd8924c229a5))
* simplify backup naming to use fixed .bak extension ([#181](https://github.com/dallay/agentsync/issues/181)) ([a915697](https://github.com/dallay/agentsync/commit/a91569775fa046484b3f3104f8de55676c357dda))
* simplify Cargo.toml version replacement and add debug logs ([178a7ca](https://github.com/dallay/agentsync/commit/178a7ca548d428d6b7bed0922c15e3fe3efe721a))
* **skills:** use provider_skill_id for suggest --install resolution ([#315](https://github.com/dallay/agentsync/issues/315)) ([dc2b4ac](https://github.com/dallay/agentsync/commit/dc2b4ac9334970c61458b5de760e9fe3c08be6ec))
* surface nested-glob walk errors ([0e21606](https://github.com/dallay/agentsync/commit/0e21606f46bc61b600342024875b20b66da0ec09))
* sync release-please config to bump all npm versions automatically ([3c35bb5](https://github.com/dallay/agentsync/commit/3c35bb59d283911c60967bfc17c99f51dbf2e351))
* **sync-optional-deps:** handle missing package.json and safely update file in place ([ad95603](https://github.com/dallay/agentsync/commit/ad95603d041f04d709876ac26006cf6b8c9e7aee))
* update bytes crate to v1.11.1 to resolve security vulnerability ([#128](https://github.com/dallay/agentsync/issues/128)) ([dc863ea](https://github.com/dallay/agentsync/commit/dc863ea43ca5ba9cebb7da567c5673d0810e7732))
* update Docker base image to 22.04 and add cleanup for Docker resources in CI ([#138](https://github.com/dallay/agentsync/issues/138)) ([97cc5a3](https://github.com/dallay/agentsync/commit/97cc5a369b5fb459ab37592629e5eed9b85c6f32))
* update release-please version to 1.30.0 ([57ef42b](https://github.com/dallay/agentsync/commit/57ef42b4f4532f15d56f98cc7eb50e984eb17769))
* update release-please-action to googleapis/release-please-action@v4.4.0 ([b30474d](https://github.com/dallay/agentsync/commit/b30474d4fe30059908ff046b0ee37380211a1d35))
* update release-please-action to v4.1.1 (e4dc86b) ([0c7b165](https://github.com/dallay/agentsync/commit/0c7b1651f7ec61d2eb53cdf472ba891e625e512a))
* use plural .opencode/skills/ directory for OpenCode ([#209](https://github.com/dallay/agentsync/issues/209)) ([ecf7c9e](https://github.com/dallay/agentsync/commit/ecf7c9e5d6ae2e08c592128d8fe6db7b998d17c2))
* use release-please@^17.3.0 instead of ^19.0.0 ([5149476](https://github.com/dallay/agentsync/commit/51494763b9dc362789f1e81c97ddc11db3bda147))
* use stable version tags for GitHub Actions ([#112](https://github.com/dallay/agentsync/issues/112)) ([753f88c](https://github.com/dallay/agentsync/commit/753f88c72da79b4286a392b819cad02ffcd4ebe6))


### Performance

* Deduplicate MCP shared paths and skip redundant writes ([#227](https://github.com/dallay/agentsync/issues/227)) ([5edafc7](https://github.com/dallay/agentsync/commit/5edafc7f540eea395caa6144dc39da066cf05b64))
* implement caching for NestedGlob and project root ([#293](https://github.com/dallay/agentsync/issues/293)) ([039a868](https://github.com/dallay/agentsync/commit/039a86827fb20c07b907a9ace47e2a62c25a3f7c))
* implement content-check for gitignore updates ([#263](https://github.com/dallay/agentsync/issues/263)) ([5ec9fbe](https://github.com/dallay/agentsync/commit/5ec9fbe4a326b51f0e26e40cfc5de8e0834cebb3))
* implement I/O and compression caching in Linker ([fd71b66](https://github.com/dallay/agentsync/commit/fd71b66792ec8db772bd6b10e19601d102e9d87f))
* optimize AGENTS.md compression by reducing allocations ([#157](https://github.com/dallay/agentsync/issues/157)) ([3bc989b](https://github.com/dallay/agentsync/commit/3bc989b89c878d2a11b021853d4449d0fe8144f3))
* optimize configuration management and serialization ([be830a7](https://github.com/dallay/agentsync/commit/be830a70accbcd15122214ea6b5e074ad2e77b55))
* optimize configuration management using BTreeMap ([f5fa2d7](https://github.com/dallay/agentsync/commit/f5fa2d7c94678eca29fc2c34136a3fe02e8fa186))
* optimize gitignore entry generation ([#115](https://github.com/dallay/agentsync/issues/115)) ([9b82489](https://github.com/dallay/agentsync/commit/9b8248944a79e69c8f00ac286ef4b5df8646f27c))
* Optimize glob pattern matching in `linker.rs` ([#76](https://github.com/dallay/agentsync/issues/76)) ([5b48c0a](https://github.com/dallay/agentsync/commit/5b48c0a14a66e26f6319ee9b864371b2da678d73))
* optimize I/O, compression caching, and dry-run messaging ([f04c0f7](https://github.com/dallay/agentsync/commit/f04c0f7fb141ad4762befff6e2871510a98d7f37))
* optimize Linker with compression and I/O caching ([#176](https://github.com/dallay/agentsync/issues/176)) ([7c6f190](https://github.com/dallay/agentsync/commit/7c6f190fff780e84f42ca2a18aa8b0e39a32a225))
* optimize MCP config generation by avoiding redundant deep clones ([#132](https://github.com/dallay/agentsync/issues/132)) ([067bee8](https://github.com/dallay/agentsync/commit/067bee80547ba4893a32c45ee945d735297d2589))
* optimize MCP configuration generation and merging ([#214](https://github.com/dallay/agentsync/issues/214)) ([6242318](https://github.com/dallay/agentsync/commit/62423182a4c4772083415d5dc44eb5b01e105936))
* optimize nested-glob traversal by skipping excluded directories ([783449a](https://github.com/dallay/agentsync/commit/783449a4abc3614fbd3e07a466ca53010e7281ef))
* refine Linker caching and dry-run messaging ([23c345c](https://github.com/dallay/agentsync/commit/23c345cc7d7e6e8414f0ba6ecfb4bd58cbc9776c))
* skip redundant I/O in write_compressed_agents_md ([#240](https://github.com/dallay/agentsync/issues/240)) ([9fcd4ae](https://github.com/dallay/agentsync/commit/9fcd4ae3fce2e75ff1bdeea30bb9911dedd88782))


### Documentation

* add AGENTS.md for build, lint, and test commands ([2d3f1a4](https://github.com/dallay/agentsync/commit/2d3f1a4ba6df11866fdd61d05e467bf4603826ef))
* add agentsync-docs workspace and npm scripts for documentation site ([d954b19](https://github.com/dallay/agentsync/commit/d954b1939f41027375f115ba7ff732c6c9846186))
* Add clean command to README usage section ([bea06cf](https://github.com/dallay/agentsync/commit/bea06cf17a65fd4bbfd449f53ecd4d18a96912f2))
* add comprehensive installation instructions to README ([3459052](https://github.com/dallay/agentsync/commit/34590521ceb82a5a2548c675f60382d85d39adae))
* add comprehensive installation instructions with MD022/MD031 fixes ([f7d7f19](https://github.com/dallay/agentsync/commit/f7d7f19cf7d4cf15fb06397a8b049c39eab70a88))
* add git hook automation guide for apply ([#281](https://github.com/dallay/agentsync/issues/281)) ([be4660d](https://github.com/dallay/agentsync/commit/be4660de94ae89bef059bc6e3d26910a0cabfa7f))
* add initial README with usage, configuration, and contribution guidelines ([b03390d](https://github.com/dallay/agentsync/commit/b03390dedaeaadd3b3c7755199adc70f04e11e70))
* add managed agent config layout section to wizard-generated AGENTS.md ([705836b](https://github.com/dallay/agentsync/commit/705836b58c6ddd5d8245d0b35ba2951e4b2f8cc7))
* Add missing community standard files ([#150](https://github.com/dallay/agentsync/issues/150)) ([aa62397](https://github.com/dallay/agentsync/commit/aa62397837ccc980176ac91558530d5966cc66fa))
* add project logo to README; update favicon with new design ([b099034](https://github.com/dallay/agentsync/commit/b099034294025020b8fe661c0cb0ae025a426c51))
* address PR feedback and fix docs build ([f23a1fe](https://github.com/dallay/agentsync/commit/f23a1fe962f357c0e1e4084d3a9ae7831005d615))
* address PR feedback and unify CLI flags ([328fa21](https://github.com/dallay/agentsync/commit/328fa21f7b33686419f6fdf70301098c0caf2ea9))
* Audit and update all project documentation ([e8ba33e](https://github.com/dallay/agentsync/commit/e8ba33e8ddd8e38217a902eca0d3ab9898a6d9f3))
* audit and update documentation for CLI accuracy and mono-repo structure ([#104](https://github.com/dallay/agentsync/issues/104)) ([cd04aec](https://github.com/dallay/agentsync/commit/cd04aec45bbeec8fe13780913518ac419dbc824a))
* audit and update monorepo documentation for CLI accuracy ([a021344](https://github.com/dallay/agentsync/commit/a021344a81e1a096a1550c8b94c0324b57dbb85f))
* comprehensive documentation audit and monorepo updates ([0b1c4e2](https://github.com/dallay/agentsync/commit/0b1c4e2abf152c650631812f8cb43eb27b003aec))
* comprehensive documentation audit and update ([#255](https://github.com/dallay/agentsync/issues/255)) ([7faee1d](https://github.com/dallay/agentsync/commit/7faee1d5b9a1e810c9e667425bc12ca9e846fbc0))
* Correct OpenCode MCP path in README ([#77](https://github.com/dallay/agentsync/issues/77)) ([8783592](https://github.com/dallay/agentsync/commit/8783592bcb7ead428d9c0fc2468926937ac04136))
* enhance README.md with improved installation instructions and checksum verification ([#152](https://github.com/dallay/agentsync/issues/152)) ([80c46c6](https://github.com/dallay/agentsync/commit/80c46c6165876c4649f948b6d00b95a2a739d086))
* expand skills documentation and agent support ([#95](https://github.com/dallay/agentsync/issues/95)) ([5beae14](https://github.com/dallay/agentsync/commit/5beae142f951cbb5369a140a99fd346576fd0d26))
* finalize address of PR feedback and refine architecture detection ([3b86f6e](https://github.com/dallay/agentsync/commit/3b86f6e4b3e87dcc8b8347f7c9ecab5637e7167d))
* fix changelog ordering and multiple documentation inconsistencies ([#308](https://github.com/dallay/agentsync/issues/308)) ([2ccca0b](https://github.com/dallay/agentsync/commit/2ccca0b949598924f11f025f36f7a9d91a46d73d))
* fix JSON formatting in getting started script example ([b3ce430](https://github.com/dallay/agentsync/commit/b3ce43079df04f30aae81bc0ff98eabc70d477a8))
* fix JSON formatting in getting started script example ([e855f55](https://github.com/dallay/agentsync/commit/e855f551995eed9a24d07d0f2435274735094d2b))
* improve accuracy of symlink-contents description in README.md ([6f10f58](https://github.com/dallay/agentsync/commit/6f10f58145e3f32e9538d18c9e154d5cac072cd7))
* improve accuracy of symlink-contents description in README.md ([c905380](https://github.com/dallay/agentsync/commit/c90538055053881f48484e2d853ad2177180edb4))
* improve formatting and clarity in proposal, spec, and tasks documentation ([93d5eb3](https://github.com/dallay/agentsync/commit/93d5eb3883c1bebd75289f362ec2f3b469f064e2))
* Improve project documentation ([2cae016](https://github.com/dallay/agentsync/commit/2cae016de13d4a96e7551af2d994f031b7fbc771))
* improve README formatting and clarify MCP and target type sections ([a63df17](https://github.com/dallay/agentsync/commit/a63df17545943b7ce761b6cfd7b638cd18bf0386))
* mark TASK-07 as complete in tasks.md ([cca30e6](https://github.com/dallay/agentsync/commit/cca30e6c82adee247c3e198f005c7dc25d633c09))
* refine Bun and Yarn installation instructions in README ([0baabb1](https://github.com/dallay/agentsync/commit/0baabb18fc5835066755e7f7b3c14184664d9ca1))
* remove unimplemented agentsync skill list command ([#126](https://github.com/dallay/agentsync/issues/126)) ([2b090c7](https://github.com/dallay/agentsync/commit/2b090c76b017e4e518c5f440a75c0f6969dec0cb))
* separate global install from one-off execution in README ([8fcb5ce](https://github.com/dallay/agentsync/commit/8fcb5ce2a989b0504a64d49166151bd6222b70de))
* **specs:** add retrospecs for core modules ([#283](https://github.com/dallay/agentsync/issues/283)) ([9f8878b](https://github.com/dallay/agentsync/commit/9f8878b5a72d1210b2dd1228cfc1e1e30deac579))
* synchronize documentation with Rust source code ([#311](https://github.com/dallay/agentsync/issues/311)) ([820a396](https://github.com/dallay/agentsync/commit/820a3966535563e290b0ba03974a818ab78e4694))
* unify project directory flag to --project-root ([0ebb993](https://github.com/dallay/agentsync/commit/0ebb993a462592db365582aac5b4fa6b8d470e25))
* update OpenCode documentation reference in agent table ([ffe5b5e](https://github.com/dallay/agentsync/commit/ffe5b5e2a8a41182dd791a203ca7121da40c0bf2))
* update project structure example in README.md ([758bbdf](https://github.com/dallay/agentsync/commit/758bbdf94bdbdc04d114d04450b52054e69ed692))
* update README.md for accuracy ([#156](https://github.com/dallay/agentsync/issues/156)) ([ab6c304](https://github.com/dallay/agentsync/commit/ab6c304f4c463fccd94495c41b1cd169a8dbb07b))
* update README.md for accuracy ([#75](https://github.com/dallay/agentsync/issues/75)) ([278372e](https://github.com/dallay/agentsync/commit/278372e90292fbcdf88013a2938f8c289835a6e4))
* update README.md for accuracy and formatting ([#159](https://github.com/dallay/agentsync/issues/159)) ([8cb9f19](https://github.com/dallay/agentsync/commit/8cb9f19b2302456f0e4536f7a37ad9af5645e605))
* update README.md for accuracy and include MCP servers ([#136](https://github.com/dallay/agentsync/issues/136)) ([a7a90c6](https://github.com/dallay/agentsync/commit/a7a90c6021ff9eb3a1c2676e5925539cddf9051c))
* update README.md for accuracy and project standards ([#228](https://github.com/dallay/agentsync/issues/228)) ([a1cf760](https://github.com/dallay/agentsync/commit/a1cf760b3abbc0e3c185576bf7afe547e3f40efb))
* update README.md for accuracy and supported flags ([2738d25](https://github.com/dallay/agentsync/commit/2738d25dba188f750a5dc9f7c6a648112f65a73f))
* update README.md for CLI accuracy ([872b1d5](https://github.com/dallay/agentsync/commit/872b1d5be1684e8b1c2d09bfda67db8cd346dbae))
* update README.md for CLI accuracy and standards ([#177](https://github.com/dallay/agentsync/issues/177)) ([9e9fa2d](https://github.com/dallay/agentsync/commit/9e9fa2d18c65b8505a82a996665414468425dde1))
* update README.md for CLI accuracy and supported agents ([#111](https://github.com/dallay/agentsync/issues/111)) ([fec5ad3](https://github.com/dallay/agentsync/commit/fec5ad3d3e6f563b296196bc97322b3b2757e1d4))
* update README.md for CLI accuracy and supported agents ([#162](https://github.com/dallay/agentsync/issues/162)) ([4bddaeb](https://github.com/dallay/agentsync/commit/4bddaeb01b7cbdb98b6726f74ee7f6a6d74779ac))
* update README.md for installation instructions and accuracy ([#143](https://github.com/dallay/agentsync/issues/143)) ([917fb54](https://github.com/dallay/agentsync/commit/917fb544d6e59c51e992ed7bc4247f191be3e4f2))
* Update README.md to include clean command ([8d8221b](https://github.com/dallay/agentsync/commit/8d8221b3077b02340fdd79b9f2058ba534a5a9b3))
* Update README.md with accurate CLI usage ([#84](https://github.com/dallay/agentsync/issues/84)) ([7f3306b](https://github.com/dallay/agentsync/commit/7f3306bbb9fa64c11bbc6ea881c44230e8624eac))
* use professional placeholders for branch and commit examples in README ([5718ca2](https://github.com/dallay/agentsync/commit/5718ca2aee4f573469000b91759cc5574ae91eec))


### Code Refactoring

* centralize nested-glob traversal ([a394af0](https://github.com/dallay/agentsync/commit/a394af08a275170d513163c2483fb7d7732de256))
* **ci:** migrate workflows to dallay/common-actions reusable workflows ([#142](https://github.com/dallay/agentsync/issues/142)) ([6a07053](https://github.com/dallay/agentsync/commit/6a0705385b3b7b0bf69f1dc482f37dcf6e34f73e))
* extract standard MCP config helpers and deduplicate formatter logic ([2a9a025](https://github.com/dallay/agentsync/commit/2a9a02528c0261f15dd2320a1af361d1b781f2f5))


### Continuous Integration

* add labeler config to map paths to repo labels ([84b0d05](https://github.com/dallay/agentsync/commit/84b0d058eaf9b3544be9396218f6f883fe0e2321))
* Add PR title linting workflow ([#71](https://github.com/dallay/agentsync/issues/71)) ([f080e84](https://github.com/dallay/agentsync/commit/f080e84639d5c97caaff393d8a320d051ccd3d55))
* Configure SonarCloud project and organization keys ([4cf4971](https://github.com/dallay/agentsync/commit/4cf4971350f54ccef8bcddb753c1427d19363b9a))
* Ensure make verify-all reports failures correctly ([#110](https://github.com/dallay/agentsync/issues/110)) ([85befaf](https://github.com/dallay/agentsync/commit/85befafd54159b82fa78728b21547801e9aa5f00))
* fix circular dependency issue in NPM publication using --no-optional ([964c2e0](https://github.com/dallay/agentsync/commit/964c2e08b991b24d06efb2add5d3d20aec458df9))
* fix pnpm action version to v4 ([78e14ee](https://github.com/dallay/agentsync/commit/78e14ee98ff767b8576a6be4e0470da810a26eee))
* install pnpm in semantic-release job ([6a5036c](https://github.com/dallay/agentsync/commit/6a5036c5128ec8d8ab70126b33249278ce37fb1e))
* install ripgrep and make packaged binary exec-bit check robust ([83d8f04](https://github.com/dallay/agentsync/commit/83d8f047b7071c0d94a5ed57f0c9d53a31e883fc))
* integrate E2E tests into GitHub Actions workflow ([#134](https://github.com/dallay/agentsync/issues/134)) ([e45a910](https://github.com/dallay/agentsync/commit/e45a9108b8e2091bbe52f7b93ccdc7a78c2b5e76))
* rebuild release workflow and fix all pnpm publication issues ([8e2fa92](https://github.com/dallay/agentsync/commit/8e2fa92156f9a5300bb38a4d2945a8848b7b35eb))
* refactor deploy-docs workflow to use explicit pnpm and Node.js setup steps ([9d6dbaa](https://github.com/dallay/agentsync/commit/9d6dbaa31c979abde8decefb6fd7df91e076ef11))
* **release:** avoid npm workspace parsing during semantic-release (NPM_CONFIG_WORKSPACES=false) ([cc7ae95](https://github.com/dallay/agentsync/commit/cc7ae950a85ca3954a0f772288a211315a05fbc0))
* **release:** install dependencies with pnpm 10.28.0 and cache pnpm store before semantic-release ([9917818](https://github.com/dallay/agentsync/commit/9917818747ddd567eaed2e55e5136d432bd56fc3))
* **release:** remove NPM_CONFIG_WORKSPACES hack; address conflicting npm workspace flags at source ([6baec76](https://github.com/dallay/agentsync/commit/6baec76611f6181e5e94049aa6891bdf3274de3e))
* **release:** run semantic-release via CLI and compute outputs from tags ([61fee27](https://github.com/dallay/agentsync/commit/61fee27a17137ec235077f6ed214b7b84d5e5190))
* use pnpm for all npm publishing jobs ([d131bd1](https://github.com/dallay/agentsync/commit/d131bd1291727aa3aefd46a4ac9e360ba7ab6286))


### Tests

* add comprehensive tests for CLI commands and configuration handling ([e3a645c](https://github.com/dallay/agentsync/commit/e3a645c308bc80e60508f6c1dee37d533c8c2b97))
* add comprehensive tests for CLI commands and configuration handling ([b2e3706](https://github.com/dallay/agentsync/commit/b2e37069ef5ff9f09a189900384ca2112ea6b6e6))
* Add E2E test for real skill installation and improve archive handling ([#118](https://github.com/dallay/agentsync/issues/118)) ([5df093f](https://github.com/dallay/agentsync/commit/5df093f8b64eb948f4e42d4350eb186b914cc2aa))
* improve OpenCode fs_server assertions for clarity and idiomatic checks ([519bd80](https://github.com/dallay/agentsync/commit/519bd8099fd42b2751b84b8721e22a84505ec1a6))


### Chores

* add indexmap as dependency for serde_json in Cargo.lock ([ad8f0e8](https://github.com/dallay/agentsync/commit/ad8f0e83cf5b091fda1596b942deb172369eb1b8))
* add lefthook configuration and enhance scripts for quality gates and dependency management ([d5a7c56](https://github.com/dallay/agentsync/commit/d5a7c5603eb5ff4901dc7d15a5f692b91402d582))
* add Makefile with common JS, Rust, and docs workflow targets ([33f76b8](https://github.com/dallay/agentsync/commit/33f76b8a63e4e679afa739ece11005d27aaf6d9e))
* add pnpm workspace configuration for agentsync package and release catalog ([771b790](https://github.com/dallay/agentsync/commit/771b7903da7303b18e54a32eb528705d8ebc30dd))
* align versions and fix various bugs in init and documentation ([5e0f0e0](https://github.com/dallay/agentsync/commit/5e0f0e004722b24669ec668f3667f3637b3598f5))
* apply security and robustness improvements to update-versions script ([f8fd8af](https://github.com/dallay/agentsync/commit/f8fd8aff9074dfe5ae1f84fd882a991345044866))
* **ci:** refine Renovate config for improved automerge and dependency grouping ([43d1c9a](https://github.com/dallay/agentsync/commit/43d1c9ac2c0d8229afa9517c9d2fb2d1eb26504d))
* **ci:** refine Renovate config for improved automerge and dependency grouping ([76bd9e8](https://github.com/dallay/agentsync/commit/76bd9e84114928046b24dc51ebcb3c4b6c172512))
* **ci:** use stable Rust toolchain in rust-toolchain.toml to support cross installation ([422aaf4](https://github.com/dallay/agentsync/commit/422aaf44171f080dbf4d198830809ce3bf222d9f))
* **config:** migrate config renovate.json ([dcb8167](https://github.com/dallay/agentsync/commit/dcb816757bb8664c5458f60b496833769717ba5d))
* Configure Renovate ([92bd023](https://github.com/dallay/agentsync/commit/92bd0238dffebc7df6320d25514cf3e6a7cdf9a4))
* **deps:** add @dallay/agentsync platform packages v1.8.1 as optional dependencies ([26f888f](https://github.com/dallay/agentsync/commit/26f888f8c3b653474a09858b1ceceafb1234a42a))
* **deps:** add @dallay/agentsync platform packages v1.8.2 as optional dependencies ([3796b44](https://github.com/dallay/agentsync/commit/3796b44162748c93db9126767cdc3b0978294afb))
* **deps:** bump aws-lc-sys in the cargo group across 1 directory ([#213](https://github.com/dallay/agentsync/issues/213)) ([e18d0d2](https://github.com/dallay/agentsync/commit/e18d0d2d2b5b8b6f9bddd565a7a3c7ea609ce284))
* **deps:** bump the npm_and_yarn group across 2 directories with 1 update ([#289](https://github.com/dallay/agentsync/issues/289)) ([0d72b77](https://github.com/dallay/agentsync/commit/0d72b77028142d2ffe8c9178716c3a9bd72cf7b8))
* **deps:** downgrade agentsync optionalDependencies to 1.7.0 ([8c2eb44](https://github.com/dallay/agentsync/commit/8c2eb4498ff18f7bd0db596c64d66792e10230c9))
* **deps:** downgrade optionalDependencies and package version to 1.7.0 ([d1145fc](https://github.com/dallay/agentsync/commit/d1145fc9469e2e9d082e28c0c50c43b5329c28d5))
* **deps:** lock file maintenance ([9a1c8c6](https://github.com/dallay/agentsync/commit/9a1c8c6c4e705e6584ae7e44bd878d8d54736350))
* **deps:** lock file maintenance ([#101](https://github.com/dallay/agentsync/issues/101)) ([7eeb033](https://github.com/dallay/agentsync/commit/7eeb033d2e3efc44843b64bccbfe10b719ef775a))
* **deps:** lock file maintenance ([#120](https://github.com/dallay/agentsync/issues/120)) ([e144957](https://github.com/dallay/agentsync/commit/e1449578083309fb541ef1dcaebe0cd750319393))
* **deps:** lock file maintenance ([#223](https://github.com/dallay/agentsync/issues/223)) ([75f82ed](https://github.com/dallay/agentsync/commit/75f82ed56ecde06e389974949f90df4fda4bde62))
* **deps:** lock file maintenance ([#292](https://github.com/dallay/agentsync/issues/292)) ([244590a](https://github.com/dallay/agentsync/commit/244590a425b437ea597f7a9af8920deeb69e3aa9))
* **deps:** lock file maintenance ([#44](https://github.com/dallay/agentsync/issues/44)) ([ab39881](https://github.com/dallay/agentsync/commit/ab398810d03a4b33d806184a74c383d5a31e49cd))
* **deps:** lock file maintenance ([#48](https://github.com/dallay/agentsync/issues/48)) ([8e30d20](https://github.com/dallay/agentsync/commit/8e30d20ebaa069a9ae645c773a34bf03f2f7f67c))
* **deps:** lock file maintenance ([#59](https://github.com/dallay/agentsync/issues/59)) ([7dbc116](https://github.com/dallay/agentsync/commit/7dbc11631841037685c2a47bf4445b2644d39cbb))
* **deps:** lock file maintenance ([#89](https://github.com/dallay/agentsync/issues/89)) ([f01ad4d](https://github.com/dallay/agentsync/commit/f01ad4d437eb702ea62de4050fc299ca3342a6bf))
* **deps:** update actions/cache action to v4.3.0 ([3be6f52](https://github.com/dallay/agentsync/commit/3be6f52a6e1a656119f9729aaf312d1be64cd9ad))
* **deps:** update actions/cache action to v4.3.0 ([ce6f38f](https://github.com/dallay/agentsync/commit/ce6f38f926feaa48be4654b6036317b651ce774c))
* **deps:** update actions/checkout action to v6.0.2 ([#144](https://github.com/dallay/agentsync/issues/144)) ([366cad9](https://github.com/dallay/agentsync/commit/366cad960f4f598e0e1b636156e5027f701d0855))
* **deps:** update actions/checkout action to v6.0.2 ([#154](https://github.com/dallay/agentsync/issues/154)) ([7e93876](https://github.com/dallay/agentsync/commit/7e9387637b2e7f13ceb7059f85d432c9e6babd33))
* **deps:** update actions/checkout digest to de0fac2 ([#147](https://github.com/dallay/agentsync/issues/147)) ([17b5079](https://github.com/dallay/agentsync/commit/17b50790906a6c475a1d91179ec53b1f49483752))
* **deps:** update actions/create-github-app-token digest to fee1f7d ([#245](https://github.com/dallay/agentsync/issues/245)) ([bc37660](https://github.com/dallay/agentsync/commit/bc376601ed118e3e5c36758456f22c5262d2a81e))
* **deps:** update actions/download-artifact action to v4.3.0 ([a0b1df9](https://github.com/dallay/agentsync/commit/a0b1df9af8a738669323aa67c92ae1fcb62d110b))
* **deps:** update actions/download-artifact action to v4.3.0 ([e97a773](https://github.com/dallay/agentsync/commit/e97a773fd4c862314f1c314323ef8097f97a99dd))
* **deps:** update actions/labeler action to v6 ([#72](https://github.com/dallay/agentsync/issues/72)) ([3d99cbe](https://github.com/dallay/agentsync/commit/3d99cbe2c55cec4e3fc624bfdbca3f1b93cf0088))
* **deps:** update actions/setup-node action to v4.4.0 ([3ca3436](https://github.com/dallay/agentsync/commit/3ca34368a6de7962875c563c129ddd60a74d2401))
* **deps:** update actions/setup-node action to v4.4.0 ([65c3221](https://github.com/dallay/agentsync/commit/65c3221901cd2298bc3dfb867a32aca484624693))
* **deps:** update actions/setup-node digest to 1d0ff46 ([fc0e621](https://github.com/dallay/agentsync/commit/fc0e62119479ef75f3410c3715bce981c42d7d10))
* **deps:** update actions/setup-node digest to 1d0ff46 ([0586884](https://github.com/dallay/agentsync/commit/0586884f29eebe41ff9b67b91489ef171943a031))
* **deps:** update actions/setup-node digest to 53b8394 ([#220](https://github.com/dallay/agentsync/issues/220)) ([788147a](https://github.com/dallay/agentsync/commit/788147a615676770cc333d6f2cc7d1c14b93b23b))
* **deps:** update actions/upload-artifact action to v4.6.2 ([f4f0bc9](https://github.com/dallay/agentsync/commit/f4f0bc99cf7a80b78fdcc05833e0a5b9f4b7deca))
* **deps:** update actions/upload-artifact action to v4.6.2 ([d373cd5](https://github.com/dallay/agentsync/commit/d373cd5b0629e39cfe52a9577bd9535c4b98d4e0))
* **deps:** update alpine docker tag to v3.23 ([4ea707b](https://github.com/dallay/agentsync/commit/4ea707b277a723fcaa7cda0492e6591d46aea94b))
* **deps:** update alpine docker tag to v3.23 ([d7992e8](https://github.com/dallay/agentsync/commit/d7992e86cc48bb8ce762509efa771dfbc1403594))
* **deps:** update cargo dependencies ([#30](https://github.com/dallay/agentsync/issues/30)) ([55abb17](https://github.com/dallay/agentsync/commit/55abb17e620778afd20480d72741667b1eb15de1))
* **deps:** update cargo dependencies ([#320](https://github.com/dallay/agentsync/issues/320)) ([24fc3eb](https://github.com/dallay/agentsync/commit/24fc3eb29881e7f252156bcd726d315ce515cb62))
* **deps:** update cargo dependencies ([#43](https://github.com/dallay/agentsync/issues/43)) ([436aead](https://github.com/dallay/agentsync/commit/436aead2fa7ec6581a0ca489bed42bb78902f00c))
* **deps:** update codecov/codecov-action action to v6 ([#282](https://github.com/dallay/agentsync/issues/282)) ([dc82e0a](https://github.com/dallay/agentsync/commit/dc82e0a198659d92b812cc1d756940f938aa279b))
* **deps:** update cycjimmy/semantic-release-action digest to 17c6867 ([#211](https://github.com/dallay/agentsync/issues/211)) ([f0c1cd4](https://github.com/dallay/agentsync/commit/f0c1cd43ca4b9971d13d7e3c81fe8aa2b4be3bf1))
* **deps:** update cycjimmy/semantic-release-action digest to 1c8dbaa ([#165](https://github.com/dallay/agentsync/issues/165)) ([813f632](https://github.com/dallay/agentsync/commit/813f6329404281a4e4a63c79604a9ac024d952a7))
* **deps:** update cycjimmy/semantic-release-action digest to acb3d1c ([#148](https://github.com/dallay/agentsync/issues/148)) ([fb1f63b](https://github.com/dallay/agentsync/commit/fb1f63b11114cc4d91056db97ef86582c0ce5efe))
* **deps:** update dallay/common-actions digest to ae3cdbb ([#166](https://github.com/dallay/agentsync/issues/166)) ([f25e6cc](https://github.com/dallay/agentsync/commit/f25e6cc3f98f8acd314f46c188c76eee37a86f9f))
* **deps:** update dependency @astrojs/starlight to v0.37.4 ([#85](https://github.com/dallay/agentsync/issues/85)) ([7b29982](https://github.com/dallay/agentsync/commit/7b299824b61e1fb8edf6196deee245236aa28909))
* **deps:** update dependency @astrojs/starlight to v0.37.6 ([#145](https://github.com/dallay/agentsync/issues/145)) ([696a634](https://github.com/dallay/agentsync/commit/696a6349b1ae1052839ad148cd9fc4dcc87b60d9))
* **deps:** update dependency @astrojs/starlight to v0.37.6 ([#155](https://github.com/dallay/agentsync/issues/155)) ([978a4ad](https://github.com/dallay/agentsync/commit/978a4ada35ebe3af1387391ec951d7f1f98feb7b))
* **deps:** update dependency @biomejs/biome to v2.3.13 ([#83](https://github.com/dallay/agentsync/issues/83)) ([1812212](https://github.com/dallay/agentsync/commit/18122127629a0d08a5fc531aa236f0d23d0dc57b))
* **deps:** update dependency @dallay/agentsync to v1.14.0 ([#49](https://github.com/dallay/agentsync/issues/49)) ([0126e90](https://github.com/dallay/agentsync/commit/0126e90e7ae2f42ff553a10948e0347b967109ea))
* **deps:** update dependency @dallay/agentsync to v1.14.5 ([#78](https://github.com/dallay/agentsync/issues/78)) ([2b46d56](https://github.com/dallay/agentsync/commit/2b46d5613cec8c64f6a233b13c2bbf30f88ff719))
* **deps:** update dependency @dallay/agentsync to v1.19.0 ([#99](https://github.com/dallay/agentsync/issues/99)) ([977c093](https://github.com/dallay/agentsync/commit/977c093d54ff12f208445b3a1b722c4bdfdba8c3))
* **deps:** update dependency @dallay/agentsync to v1.23.0 ([#119](https://github.com/dallay/agentsync/issues/119)) ([b3fa994](https://github.com/dallay/agentsync/commit/b3fa9942ebccde321be076d480f3c1f04f68294c))
* **deps:** update dependency @dallay/agentsync to v1.24.0 ([#130](https://github.com/dallay/agentsync/issues/130)) ([c802e6a](https://github.com/dallay/agentsync/commit/c802e6a5d7be46f11ea3bad22c55c7245ee52c41))
* **deps:** update dependency @dallay/agentsync to v1.27.1 ([#146](https://github.com/dallay/agentsync/issues/146)) ([6052571](https://github.com/dallay/agentsync/commit/60525710b7d4b0dc39be4952edf061a00431a77e))
* **deps:** update dependency @dallay/agentsync to v1.27.2 ([#158](https://github.com/dallay/agentsync/issues/158)) ([9051e28](https://github.com/dallay/agentsync/commit/9051e286dc8f4d93694049c0c8416484e8d5da7d))
* **deps:** update dependency @dallay/agentsync to v1.29.0 ([#167](https://github.com/dallay/agentsync/issues/167)) ([68eb89f](https://github.com/dallay/agentsync/commit/68eb89fdcb45dc8b7f04eb791f087cb5c62ab493))
* **deps:** update dependency @dallay/agentsync to v1.29.0 ([#202](https://github.com/dallay/agentsync/issues/202)) ([86b8f3a](https://github.com/dallay/agentsync/commit/86b8f3a79eff0271cccac780931e02b4e0334e15))
* **deps:** update dependency @dallay/agentsync to v1.34.0 ([#250](https://github.com/dallay/agentsync/issues/250)) ([7aa37c6](https://github.com/dallay/agentsync/commit/7aa37c6e2b329197595289f26fcf6f651377045c))
* **deps:** update dependency @dallay/agentsync to v1.37.0 ([#291](https://github.com/dallay/agentsync/issues/291)) ([b4e2fb8](https://github.com/dallay/agentsync/commit/b4e2fb846d19d973f3b423d25c46c3cb6a4e6afa))
* **deps:** update dependency @dallay/agentsync-linux-x64 to v1.28.0 ([#161](https://github.com/dallay/agentsync/issues/161)) ([ba6593f](https://github.com/dallay/agentsync/commit/ba6593f6930d8edd1f529bdb09e966445359ff42))
* **deps:** update dependency @dallay/agentsync-linux-x64 to v1.7.0 ([58dfbd4](https://github.com/dallay/agentsync/commit/58dfbd4bfb0f39e48ea3b5b98eccb66f03287932))
* **deps:** update dependency @dallay/agentsync-linux-x64 to v1.7.0 ([b28eb2b](https://github.com/dallay/agentsync/commit/b28eb2b774890d73f511ea2dc37a6b4066539b6f))
* **deps:** update dependency @iconify/json to v2.2.458 ([#294](https://github.com/dallay/agentsync/issues/294)) ([0cc22fe](https://github.com/dallay/agentsync/commit/0cc22fe236db2ee9de30e7543d8da8152c70d3b0))
* **deps:** update dependency astro to v5.17.3 ([#191](https://github.com/dallay/agentsync/issues/191)) ([ca24e2b](https://github.com/dallay/agentsync/commit/ca24e2b5ef2a0e7312b9e30955cc7d06401677d6))
* **deps:** update dependency astro to v5.18.0 ([f8b5c94](https://github.com/dallay/agentsync/commit/f8b5c947322547e267794d364f70df5e2a7881e1))
* **deps:** update dependency astro to v5.18.1 ([#232](https://github.com/dallay/agentsync/issues/232)) ([1c328b2](https://github.com/dallay/agentsync/commit/1c328b2323f622ac2ed8c069007a9259af989bef))
* **deps:** update dependency node to v24 ([#58](https://github.com/dallay/agentsync/issues/58)) ([3406098](https://github.com/dallay/agentsync/commit/3406098cc8d2f6f402207ef0088ee37a5e5ed1cb))
* **deps:** update devdependencies ([#122](https://github.com/dallay/agentsync/issues/122)) ([b863b3a](https://github.com/dallay/agentsync/commit/b863b3a14eaa9b48b1146a27dad8dbe8dbc74392))
* **deps:** update devdependencies ([#206](https://github.com/dallay/agentsync/issues/206)) ([f8508af](https://github.com/dallay/agentsync/commit/f8508aff3b9d301143f5396c50bcb43497c9d9f1))
* **deps:** update devdependencies ([#238](https://github.com/dallay/agentsync/issues/238)) ([1154a97](https://github.com/dallay/agentsync/commit/1154a97f022370a8f760716f1a23e670d9ef6d0b))
* **deps:** update docker/build-push-action action to v6.18.0 ([34f336b](https://github.com/dallay/agentsync/commit/34f336b3f9463576b2c2f2dcd5655c77d8b63578))
* **deps:** update docker/build-push-action action to v6.18.0 ([3276fce](https://github.com/dallay/agentsync/commit/3276fce5616ad23249dcc07ff4f804881856be0b))
* **deps:** update docker/login-action action to v3.6.0 ([846462b](https://github.com/dallay/agentsync/commit/846462b403995021cfdfe72aee687853154aa05b))
* **deps:** update docker/login-action action to v3.6.0 ([1f6ae53](https://github.com/dallay/agentsync/commit/1f6ae534465c5898a17b791280dca852706811a0))
* **deps:** update docker/login-action action to v4.1.0 ([#295](https://github.com/dallay/agentsync/issues/295)) ([4fb81a5](https://github.com/dallay/agentsync/commit/4fb81a58ee3412fc85d045b5b662cca25017ca79))
* **deps:** update docker/metadata-action action to v5.10.0 ([77658ba](https://github.com/dallay/agentsync/commit/77658ba7c1c05b72a66f04dfbf0a3dc3b7fe8404))
* **deps:** update docker/metadata-action action to v5.10.0 ([7ac0711](https://github.com/dallay/agentsync/commit/7ac071181e7cc813faccbbea36ff5ba4c2176a84))
* **deps:** update docker/setup-buildx-action action to v3.12.0 ([1561d52](https://github.com/dallay/agentsync/commit/1561d5290fc6de961f411b44cb54d498cd755af7))
* **deps:** update docker/setup-buildx-action action to v3.12.0 ([a284d39](https://github.com/dallay/agentsync/commit/a284d397caa0da5db98e9a959b44b57cce806227))
* **deps:** update docker/setup-qemu-action action to v3.7.0 ([ec99fe2](https://github.com/dallay/agentsync/commit/ec99fe20c9dddad93da04bb2ff0018825787447a))
* **deps:** update docker/setup-qemu-action action to v3.7.0 ([7181c42](https://github.com/dallay/agentsync/commit/7181c4279be2dfe6aed397e3b937a4f56dd0b03a))
* **deps:** update dtolnay/rust-toolchain digest to 3c5f7ea ([#268](https://github.com/dallay/agentsync/issues/268)) ([64a8682](https://github.com/dallay/agentsync/commit/64a8682a787efb32f7737e265dddcfc790365dcc))
* **deps:** update dtolnay/rust-toolchain digest to efa25f7 ([#212](https://github.com/dallay/agentsync/issues/212)) ([6942078](https://github.com/dallay/agentsync/commit/69420781f6b42bb3e156d87aada45f004ba6cfd2))
* **deps:** update dtolnay/rust-toolchain digest to f7ccc83 ([2d5e21a](https://github.com/dallay/agentsync/commit/2d5e21a6c58435df41eb2a13053d0f7a0a837c76))
* **deps:** update dtolnay/rust-toolchain digest to f7ccc83 ([7e12524](https://github.com/dallay/agentsync/commit/7e125245552602d80742133fab504035ed863c74))
* **deps:** update github actions ([#221](https://github.com/dallay/agentsync/issues/221)) ([42a4349](https://github.com/dallay/agentsync/commit/42a43490303f935a802abec2809a0462e0f1cca6))
* **deps:** update github actions ([#244](https://github.com/dallay/agentsync/issues/244)) ([d266a22](https://github.com/dallay/agentsync/commit/d266a22ad0687f250ff83b7f3fd8db4b4abb65b8))
* **deps:** update github actions ([#98](https://github.com/dallay/agentsync/issues/98)) ([1c20b02](https://github.com/dallay/agentsync/commit/1c20b0236efbe135a86e4ba3365ce3e0950987f9))
* **deps:** update major upgrades ([77f54c6](https://github.com/dallay/agentsync/commit/77f54c6c5f628e9d6d3d3b576ddede3e0c035378))
* **deps:** update major upgrades ([0df9a39](https://github.com/dallay/agentsync/commit/0df9a397d57c8ab5cc8e77c838fb32ed994fecc9))
* **deps:** update major upgrades ([48e758f](https://github.com/dallay/agentsync/commit/48e758f01b289544a8e9fc4ca27f5a1ac487f546))
* **deps:** update major upgrades ([#114](https://github.com/dallay/agentsync/issues/114)) ([6087268](https://github.com/dallay/agentsync/commit/608726899ee93da7af3c189107f72a087a49110f))
* **deps:** update major upgrades ([#207](https://github.com/dallay/agentsync/issues/207)) ([f4a0add](https://github.com/dallay/agentsync/commit/f4a0addea4d451660b09f7c9ddeab11436c96ab8))
* **deps:** update major upgrades ([#264](https://github.com/dallay/agentsync/issues/264)) ([af72b18](https://github.com/dallay/agentsync/commit/af72b186bb81ffad06468536b04814454e5a427e))
* **deps:** update node.js to v24.13.1 ([#174](https://github.com/dallay/agentsync/issues/174)) ([904f253](https://github.com/dallay/agentsync/commit/904f253a325678366ce8099878715e84aca944c8))
* **deps:** update node.js to v24.14.0 ([#222](https://github.com/dallay/agentsync/issues/222)) ([e02702a](https://github.com/dallay/agentsync/commit/e02702afa38a99025ec380978b526a34b6f138d8))
* **deps:** update node.js to v24.14.1 ([#267](https://github.com/dallay/agentsync/issues/267)) ([7c49f45](https://github.com/dallay/agentsync/commit/7c49f457cd776062e4a5cd9817d65051d11ab3b7))
* **deps:** update rust crate rand to 0.9 ([#100](https://github.com/dallay/agentsync/issues/100)) ([68ba22a](https://github.com/dallay/agentsync/commit/68ba22ab8c347323c7c2ae159b496d28fdfb25bd))
* **deps:** update rust crate thiserror to v2.0.18 ([#50](https://github.com/dallay/agentsync/issues/50)) ([2a91f1e](https://github.com/dallay/agentsync/commit/2a91f1eee7102b8e642492430b227898b5397317))
* **deps:** update rust crate zip to v8.5.0 ([#285](https://github.com/dallay/agentsync/issues/285)) ([56b5cef](https://github.com/dallay/agentsync/commit/56b5cef976b16aab53b55ea16cc0183f93b10274))
* **deps:** update rust docker tag to v1.92 ([77eed62](https://github.com/dallay/agentsync/commit/77eed622af0b90375c688905fcd6b2a7a7397a47))
* **deps:** update rust docker tag to v1.92 ([1d7d9aa](https://github.com/dallay/agentsync/commit/1d7d9aa12af85fd1726319e469dc8a645569f9e7))
* **deps:** update rust docker tag to v1.93 ([#81](https://github.com/dallay/agentsync/issues/81)) ([3a8899d](https://github.com/dallay/agentsync/commit/3a8899db2e273840be7914ef72b25e48b43f6542))
* **deps:** update rust docker tag to v1.94 ([#286](https://github.com/dallay/agentsync/issues/286)) ([1fa1fbf](https://github.com/dallay/agentsync/commit/1fa1fbfda9df599d40795079661e6da294538185))
* **deps:** update softprops/action-gh-release action to v2.5.0 ([360dd2d](https://github.com/dallay/agentsync/commit/360dd2d432a9ad74a4e3fdc5d20f7cad6c4e639e))
* **deps:** update softprops/action-gh-release action to v2.5.0 ([a51470d](https://github.com/dallay/agentsync/commit/a51470d9f8ec06e14314011f28c8318331f5838f))
* **deps:** update sonarsource/sonarqube-scan-action digest to 299e4b7 ([#284](https://github.com/dallay/agentsync/issues/284)) ([b506483](https://github.com/dallay/agentsync/commit/b506483a0e0e09886d266dfe7aac0400549a42c3))
* **deps:** update toml-related dependencies in Cargo.lock ([cd13c7f](https://github.com/dallay/agentsync/commit/cd13c7f36c3bec70296184af63f6a98976db6aab))
* **docker:** remove version pin for ca-certificates in apk install ([af7a7dd](https://github.com/dallay/agentsync/commit/af7a7dd0a1178a15d36b915cb1e93c4d2d6c30aa))
* **docs:** cleanup docs & components — replace legacy CodeBlock with @astrojs/starlight Code, fix paths, clarify MCP guide, add reduced-motion a11y, trim Cargo keywords ([5d06660](https://github.com/dallay/agentsync/commit/5d06660f96318083331868fbe647222b1332310b))
* **labeler:** update labeler.yml to use changed-files syntax and add pnpm-lock.yaml to backend label ([bff4466](https://github.com/dallay/agentsync/commit/bff44669f3aa492d5d3fd83106f09b2bc10e013b))
* **main:** release 1.31.0 ([#239](https://github.com/dallay/agentsync/issues/239)) ([58d66cd](https://github.com/dallay/agentsync/commit/58d66cdacd3133377cec6c7836025090248db096))
* **main:** release 1.32.0 ([60fe77f](https://github.com/dallay/agentsync/commit/60fe77f26fa2d06ff948255b32bec7e44abd4d87))
* **main:** release 1.33.0 ([29444fd](https://github.com/dallay/agentsync/commit/29444fd26f0cb26724699760fc723c407a2621ac))
* **main:** release 1.33.1 ([#248](https://github.com/dallay/agentsync/issues/248)) ([98aa7e3](https://github.com/dallay/agentsync/commit/98aa7e31b5a75a2d1978d5b788e42d1786e2502b))
* **main:** release 1.34.0 ([#249](https://github.com/dallay/agentsync/issues/249)) ([da5171d](https://github.com/dallay/agentsync/commit/da5171db8e2b2e42504fe436b7f8a589277b051b))
* **main:** release 1.35.0 ([#260](https://github.com/dallay/agentsync/issues/260)) ([fbdb000](https://github.com/dallay/agentsync/commit/fbdb0000570f545f1e7348f3412ce0f23ccd7ffb))
* **main:** release 1.35.1 ([#265](https://github.com/dallay/agentsync/issues/265)) ([cae8ccf](https://github.com/dallay/agentsync/commit/cae8ccf816e81b2b32b37ccceda06945250595b7))
* **main:** release 1.35.2 ([#266](https://github.com/dallay/agentsync/issues/266)) ([40a0283](https://github.com/dallay/agentsync/commit/40a0283a3506c9fa5e969b4236adea8ae01d0f3e))
* **main:** release 1.36.0 ([#271](https://github.com/dallay/agentsync/issues/271)) ([67096d0](https://github.com/dallay/agentsync/commit/67096d0243024fc5af94ddae338087aed733b66b))
* **main:** release 1.37.0 ([#287](https://github.com/dallay/agentsync/issues/287)) ([c9b4540](https://github.com/dallay/agentsync/commit/c9b45404a2896ba5c14406f51a4d5d26ca331be7))
* **main:** release 1.38.0 ([#290](https://github.com/dallay/agentsync/issues/290)) ([9c41432](https://github.com/dallay/agentsync/commit/9c41432157507732c51133fe8cbc5697984feabe))
* **main:** release 1.39.0 ([#310](https://github.com/dallay/agentsync/issues/310)) ([daa344b](https://github.com/dallay/agentsync/commit/daa344bc5dca388170c0f261b93603e8a0d572e4))
* **main:** release 1.40.0 ([#312](https://github.com/dallay/agentsync/issues/312)) ([0094d36](https://github.com/dallay/agentsync/commit/0094d362673102a9ba2498a42f14b7ab6f84ebb5))
* **main:** release 1.40.1 ([8f62612](https://github.com/dallay/agentsync/commit/8f626123f098449519d0bbd4680ed8173dca97a4))
* **main:** release 1.41.0 ([263fd66](https://github.com/dallay/agentsync/commit/263fd66ea490bfae88815c9ac9e1a16f9eda2306))
* make version update script more robust for Cargo.toml ([2aa7d70](https://github.com/dallay/agentsync/commit/2aa7d70f238a190b86bc34d3a63795dc9f95fd33))
* **makefile:** update agentsync targets and js build command; remove rust-build from all target ([645c76d](https://github.com/dallay/agentsync/commit/645c76df81e09dc9ee9c785c2d09ea701cd74e79))
* **release:** 🔖 1.0.0 [skip ci] ([25f854b](https://github.com/dallay/agentsync/commit/25f854bc1d2a431f81f426f60f644a2199ed0e5b))
* **release:** 🔖 1.1.0 [skip ci] ([c1a347d](https://github.com/dallay/agentsync/commit/c1a347dac49830ddc99bc56b2eac4350903ac962))
* **release:** 🔖 1.1.1 [skip ci] ([5c23095](https://github.com/dallay/agentsync/commit/5c23095edb61185ea0d89c09e9a8ae1e6f59c2e9))
* **release:** 🔖 1.10.0 [skip ci] ([5d6353e](https://github.com/dallay/agentsync/commit/5d6353e6bb08e6b9b9f245939fb624f38ee4d5c8))
* **release:** 🔖 1.11.0 [skip ci] ([040e02f](https://github.com/dallay/agentsync/commit/040e02f0d773e28d6d4ec560f91e94f5bb6581a1))
* **release:** 🔖 1.12.0 [skip ci] ([123c2f8](https://github.com/dallay/agentsync/commit/123c2f817a0f0ffb14cb9a550640c531e3c7a516))
* **release:** 🔖 1.13.0 [skip ci] ([267f4de](https://github.com/dallay/agentsync/commit/267f4de06aa04b208dbab2d0ba15938b330ef5e2))
* **release:** 🔖 1.14.0 [skip ci] ([88c68bc](https://github.com/dallay/agentsync/commit/88c68bc8fa526c1e8f136ec55bcc505f11b3a778))
* **release:** 🔖 1.14.1 [skip ci] ([65be1f1](https://github.com/dallay/agentsync/commit/65be1f15e960168ee29542476f0e90f0ddb98bb8))
* **release:** 🔖 1.14.2 [skip ci] ([daaaf4c](https://github.com/dallay/agentsync/commit/daaaf4c753ec819b6589831ffc8ee36a2ace3c17))
* **release:** 🔖 1.14.3 [skip ci] ([1b2700b](https://github.com/dallay/agentsync/commit/1b2700b6e3c4f52915fd32a399c976d34276107c))
* **release:** 🔖 1.14.4 [skip ci] ([04fb710](https://github.com/dallay/agentsync/commit/04fb710e7e430c15ff91d3c958e69f899d312b30))
* **release:** 🔖 1.14.5 [skip ci] ([3de7076](https://github.com/dallay/agentsync/commit/3de7076952f04329d36225f21190bd5cf2dcdade))
* **release:** 🔖 1.15.0 [skip ci] ([a1e4164](https://github.com/dallay/agentsync/commit/a1e416488a1317afb1377c330d693c2bed0cbb90))
* **release:** 🔖 1.16.0 [skip ci] ([1f90fea](https://github.com/dallay/agentsync/commit/1f90fea372f187b1d057909d485e444bc7f70e06))
* **release:** 🔖 1.17.0 [skip ci] ([143cf03](https://github.com/dallay/agentsync/commit/143cf037fde15bf2a954f7b37512e174b35c75f1))
* **release:** 🔖 1.18.0 [skip ci] ([d234110](https://github.com/dallay/agentsync/commit/d2341105177b39f8e15d1ac71e23d629b05aa126))
* **release:** 🔖 1.19.0 [skip ci] ([63c5a3e](https://github.com/dallay/agentsync/commit/63c5a3e0e77f53c082b348e256dac33ffa5c63aa))
* **release:** 🔖 1.2.0 [skip ci] ([0b7e7ea](https://github.com/dallay/agentsync/commit/0b7e7ea59a97ec46769d82854235b38121f2deec))
* **release:** 🔖 1.20.0 [skip ci] ([8b343bd](https://github.com/dallay/agentsync/commit/8b343bd7b005aba61c0c07c541ca8e7c5af94947))
* **release:** 🔖 1.21.0 [skip ci] ([77d1bdb](https://github.com/dallay/agentsync/commit/77d1bdb28bf1bbd1d103831767369c809d2a52a3))
* **release:** 🔖 1.21.1 [skip ci] ([89515ee](https://github.com/dallay/agentsync/commit/89515eefa6e23011904c43f12d44c072566a13c2))
* **release:** 🔖 1.21.2 [skip ci] ([b05aed7](https://github.com/dallay/agentsync/commit/b05aed778431a208ba1ae85d46e8578cdb0fc2a5))
* **release:** 🔖 1.22.0 [skip ci] ([dc26473](https://github.com/dallay/agentsync/commit/dc264733676e8d44cc23892c79e423a2e4f477ce))
* **release:** 🔖 1.23.0 [skip ci] ([b54a541](https://github.com/dallay/agentsync/commit/b54a541886d7261d7a15a7d549d8173a52e2c8c8))
* **release:** 🔖 1.23.1 [skip ci] ([3d0c2e6](https://github.com/dallay/agentsync/commit/3d0c2e6fff97726e7d0377e48cd7f808630cfa07))
* **release:** 🔖 1.24.0 [skip ci] ([cb06674](https://github.com/dallay/agentsync/commit/cb066745d794ffea6cf805e8ed3bc74374b4ec22))
* **release:** 🔖 1.25.0 [skip ci] ([0c51d9a](https://github.com/dallay/agentsync/commit/0c51d9acaf3688e366dfa03b019658b7f3325b46))
* **release:** 🔖 1.26.0 [skip ci] ([415448d](https://github.com/dallay/agentsync/commit/415448d42afec9230298ba3523a4a529770ee341))
* **release:** 🔖 1.26.1 [skip ci] ([ccdbbae](https://github.com/dallay/agentsync/commit/ccdbbae25d0b6d64bee7f43e525b7be46862f538))
* **release:** 🔖 1.26.2 [skip ci] ([ba183f3](https://github.com/dallay/agentsync/commit/ba183f3804a67d7d53562381bf0d3a7ea480698f))
* **release:** 🔖 1.27.0 [skip ci] ([8c475a2](https://github.com/dallay/agentsync/commit/8c475a28df4bf7b40da85aee3356fa96896504ac))
* **release:** 🔖 1.27.1 [skip ci] ([cea90a1](https://github.com/dallay/agentsync/commit/cea90a1037d76cc8c5b9f7ce13b89f696612d1ea))
* **release:** 🔖 1.27.2 [skip ci] ([bddadd8](https://github.com/dallay/agentsync/commit/bddadd8dcf0a2ea3a9f8748652b43bba77e2d232))
* **release:** 🔖 1.28.0 [skip ci] ([9e71080](https://github.com/dallay/agentsync/commit/9e71080eb0c2603f30f515daa359670afd5424b1))
* **release:** 🔖 1.29.0 [skip ci] ([7e5777e](https://github.com/dallay/agentsync/commit/7e5777e666c0b8af408a867915bfc9ef68415e7c))
* **release:** 🔖 1.3.0 [skip ci] ([001b155](https://github.com/dallay/agentsync/commit/001b1550e61c9f45e9755fa05f7de56839d04406))
* **release:** 🔖 1.30.0 [skip ci] ([568d6e8](https://github.com/dallay/agentsync/commit/568d6e898b36e44a3466ee3ef9a0e830a28c5d10))
* **release:** 🔖 1.4.0 [skip ci] ([d1fef3b](https://github.com/dallay/agentsync/commit/d1fef3b100be6b8f3a7f442317072ec77a5229cd))
* **release:** 🔖 1.5.0 [skip ci] ([7c5384e](https://github.com/dallay/agentsync/commit/7c5384ec08492a20b92b72153833aa1f98395eed))
* **release:** 🔖 1.6.0 [skip ci] ([621b4dd](https://github.com/dallay/agentsync/commit/621b4dd1d85644d6c1ecf3ce6ad76aba26689465))
* **release:** 🔖 1.7.0 [skip ci] ([31af00b](https://github.com/dallay/agentsync/commit/31af00b7fc1bbcaee3f417ff422c3ecf9c0d6505))
* **release:** 🔖 1.8.0 [skip ci] ([7ade01f](https://github.com/dallay/agentsync/commit/7ade01f74760a57b154584b79745c1f5be353925))
* **release:** 🔖 1.8.1 [skip ci] ([5cf2f42](https://github.com/dallay/agentsync/commit/5cf2f425acffe9ef917ecf93050d224407ffa2f1))
* **release:** 🔖 1.8.2 [skip ci] ([89ebe65](https://github.com/dallay/agentsync/commit/89ebe65db13b3abba046112cb89caf668bd9c29d))
* **release:** 🔖 1.8.3 [skip ci] ([28343e5](https://github.com/dallay/agentsync/commit/28343e5b399865c454cd8d8509529e3febc43b85))
* **release:** 🔖 1.9.0 [skip ci] ([182530b](https://github.com/dallay/agentsync/commit/182530bcdeb7429d30529f49aecd317634768658))
* **release:** add GitHub App token generation, support beta/alpha branches, CI node 24, packageManager pnpm@9.5.0 and cross-platform prepare script ([d70e705](https://github.com/dallay/agentsync/commit/d70e705b3c6bb47ba2b8cb131edc007496d56fb3))
* **release:** add script to update project versions and sync optional dependencies ([bc45c57](https://github.com/dallay/agentsync/commit/bc45c57c9ce27db580fb3c960797eb87e808d1a7))
* **release:** automate optionalDependencies bump via semantic-release prepare and git ([722b799](https://github.com/dallay/agentsync/commit/722b799382df026cfe44eb1bd83220cb0b1a0800))
* **release:** bump to 1.11.1 and sync optional deps + update lockfile ([f4b0172](https://github.com/dallay/agentsync/commit/f4b01728dc566dab97338c80a35c05c54118ea30))
* **release:** bump to 1.11.1 and sync optional deps + update lockfile ([568c6cd](https://github.com/dallay/agentsync/commit/568c6cddb06d1763e06e78e116bc5e17736eb8c9))
* **release:** bump version to 1.7.1, update optionalDependencies, add sync-optional-deps script, and adjust npm version in CI ([a2b0740](https://github.com/dallay/agentsync/commit/a2b074086221be2b70f632e9011598caaf53d36c))
* **release:** bump version to 1.8.0, update optionalDependencies, automate version sync, and refine release workflow ([44a4722](https://github.com/dallay/agentsync/commit/44a4722a2d11848fb7191b7227d4ea8aa1d05a97))
* **release:** revert to version 1.7.0, update dependencies, and enhance GitHub App token usage in CI ([816012b](https://github.com/dallay/agentsync/commit/816012b620fbf4ec3f4e9b4e538ba1e331b75c13))
* **release:** update optionalDependencies to 1.8.0 and pin npm@10 in CI ([c872754](https://github.com/dallay/agentsync/commit/c8727543ff92f85ecf89cd9314a6a072bb5d9177))
* remove old Spec-Driven Development scripts and templates ([0a9209c](https://github.com/dallay/agentsync/commit/0a9209c4204496894d0525c433da066f9f5e6a24))
* **setup:** replace shell prepare script with setup.js for git hooks and docs symlink ([36dbec9](https://github.com/dallay/agentsync/commit/36dbec930c3b2ccabf0384d51e8b5d85c5287262))
* update @astrojs/starlight to version 0.38.0 ([06776a6](https://github.com/dallay/agentsync/commit/06776a6ab3684bbaa0a81308c29ebe78ff3f98bf))
* update agentsync optional dependencies to v1.34.0 in pnpm lockfile ([5c2b06a](https://github.com/dallay/agentsync/commit/5c2b06a8cfc4a781da2689409828c4d77399db1f))
* update Cargo.lock after adding is-terminal dependency ([0ef6738](https://github.com/dallay/agentsync/commit/0ef6738828bea189d8a2fc297b45008d414178c4))
* update Node.js version to 24 and switch to pnpm for build and typecheck commands ([1b9baaf](https://github.com/dallay/agentsync/commit/1b9baafd8aa433e753985f52176b7044a0dda893))
* update packaging to set platform-specific agentsync binary in package.json ([#65](https://github.com/dallay/agentsync/issues/65)) ([04b74fa](https://github.com/dallay/agentsync/commit/04b74fa93a397e9bc714db86d8d602fbd9751c91))
* update release workflow to use semantic-release GitHub Action ([1d874c9](https://github.com/dallay/agentsync/commit/1d874c9e222d7e35ce7cc6bb64c6907c73bce284))
* update semantic-release action version and add debug output to release workflow ([2e9e035](https://github.com/dallay/agentsync/commit/2e9e0355d94b253e11dcc6f4785daf55f06ef4cf))
* Update semantic-release-action version in workflow ([#131](https://github.com/dallay/agentsync/issues/131)) ([ab82cc8](https://github.com/dallay/agentsync/commit/ab82cc8971d356377b53d55220141d3a5e87bd94))
* **workspace:** add comment clarifying minimumReleaseAge value in pnpm-workspace.yaml ([0e075bd](https://github.com/dallay/agentsync/commit/0e075bd9b8032e9ab50d6e06c61a889e4bd35ade))

## [1.41.0](https://github.com/dallay/agentsync/compare/v1.40.1...v1.41.0) (2026-04-05)


### Features

* Enhance skill installation with provider ID resolution and UX improvements ([#319](https://github.com/dallay/agentsync/issues/319)) ([21533c2](https://github.com/dallay/agentsync/commit/21533c26955acc5f6c62968d81b529781ab83efd))

## [1.40.1](https://github.com/dallay/agentsync/compare/v1.40.0...v1.40.1) (2026-04-04)


### Bug Fixes

* **skills:** use provider_skill_id for suggest --install resolution ([#315](https://github.com/dallay/agentsync/issues/315)) ([dc2b4ac](https://github.com/dallay/agentsync/commit/dc2b4ac9334970c61458b5de760e9fe3c08be6ec))

## [1.40.0](https://github.com/dallay/agentsync/compare/v1.39.0...v1.40.0) (2026-04-04)


### Features

* **skills:** add agentsync guidance and broaden design recommendations ([#314](https://github.com/dallay/agentsync/issues/314)) ([407de7c](https://github.com/dallay/agentsync/commit/407de7c08576c4a935852dfb843d754af6e636da))


### Documentation

* synchronize documentation with Rust source code ([#311](https://github.com/dallay/agentsync/issues/311)) ([820a396](https://github.com/dallay/agentsync/commit/820a3966535563e290b0ba03974a818ab78e4694))

## [1.39.0](https://github.com/dallay/agentsync/compare/v1.38.0...v1.39.0) (2026-04-03)


### Features

* **suggest:** add 37 technology detections across 9 categories ([541b750](https://github.com/dallay/agentsync/commit/541b750f8a784a6e2b64b2b8678060ef0c7b0797))
* **suggest:** add API technology detection — GraphQL, gRPC, tRPC, OpenAPI ([#309](https://github.com/dallay/agentsync/issues/309)) ([7bae96b](https://github.com/dallay/agentsync/commit/7bae96bc3e2c62a36a0068ff4411edc1d49471bc)), closes [#301](https://github.com/dallay/agentsync/issues/301)


### Documentation

* fix changelog ordering and multiple documentation inconsistencies ([#308](https://github.com/dallay/agentsync/issues/308)) ([2ccca0b](https://github.com/dallay/agentsync/commit/2ccca0b949598924f11f025f36f7a9d91a46d73d))

## [1.37.0](https://github.com/dallay/agentsync/compare/v1.36.0...v1.37.0) (2026-04-01)


### Features

* Implement autoskills discovery and update Windows setup documentation ([#288](https://github.com/dallay/agentsync/issues/288)) ([f6fb8c8](https://github.com/dallay/agentsync/commit/f6fb8c870a4ab4bd9a3b63d140225c002add11a1))


### Documentation

* **specs:** add retrospecs for core modules ([#283](https://github.com/dallay/agentsync/issues/283)) ([9f8878b](https://github.com/dallay/agentsync/commit/9f8878b5a72d1210b2dd1228cfc1e1e30deac579))


### Chores

* **deps:** update rust docker tag to v1.94 ([#286](https://github.com/dallay/agentsync/issues/286)) ([1fa1fbf](https://github.com/dallay/agentsync/commit/1fa1fbfda9df599d40795079661e6da294538185))
* **deps:** update sonarsource/sonarqube-scan-action digest to 299e4b7 ([#284](https://github.com/dallay/agentsync/issues/284)) ([b506483](https://github.com/dallay/agentsync/commit/b506483a0e0e09886d266dfe7aac0400549a42c3))

## [1.36.0](https://github.com/dallay/agentsync/compare/v1.35.2...v1.36.0) (2026-04-01)


### Features

* add repository-based skill suggestions ([#272](https://github.com/dallay/agentsync/issues/272)) ([a534ace](https://github.com/dallay/agentsync/commit/a534ace5b8542cc800489f6a017dce48bd06cd48))
* add wizard agent config layout guidance ([fa435fe](https://github.com/dallay/agentsync/commit/fa435feb0e7afe0532a9d1e24faf878768f9f715))


### Bug Fixes

* improve wizard gitignore workflows and documentation ([#278](https://github.com/dallay/agentsync/issues/278)) ([8d8c44c](https://github.com/dallay/agentsync/commit/8d8c44c009446bb4c5862ffcdf4119f29d5744b8))
* path traversal vulnerability in symlink destinations ([#280](https://github.com/dallay/agentsync/issues/280)) ([6a72530](https://github.com/dallay/agentsync/commit/6a72530e45c5c0ce0c05c79354b04afc87fcdc2e))


### Documentation

* add git hook automation guide for apply ([#281](https://github.com/dallay/agentsync/issues/281)) ([be4660d](https://github.com/dallay/agentsync/commit/be4660de94ae89bef059bc6e3d26910a0cabfa7f))


### Chores

* **deps:** update codecov/codecov-action action to v6 ([#282](https://github.com/dallay/agentsync/issues/282)) ([dc82e0a](https://github.com/dallay/agentsync/commit/dc82e0a198659d92b812cc1d756940f938aa279b))
* **deps:** update dtolnay/rust-toolchain digest to 3c5f7ea ([#268](https://github.com/dallay/agentsync/issues/268)) ([64a8682](https://github.com/dallay/agentsync/commit/64a8682a787efb32f7737e265dddcfc790365dcc))
* **deps:** update node.js to v24.14.1 ([#267](https://github.com/dallay/agentsync/issues/267)) ([7c49f45](https://github.com/dallay/agentsync/commit/7c49f457cd776062e4a5cd9817d65051d11ab3b7))

## [1.35.2](https://github.com/dallay/agentsync/compare/v1.35.1...v1.35.2) (2026-03-29)


### Performance

* implement content-check for gitignore updates ([#263](https://github.com/dallay/agentsync/issues/263)) ([5ec9fbe](https://github.com/dallay/agentsync/commit/5ec9fbe4a326b51f0e26e40cfc5de8e0834cebb3))

## [1.35.1](https://github.com/dallay/agentsync/compare/v1.35.0...v1.35.1) (2026-03-29)


### Chores

* **deps:** update major upgrades ([#264](https://github.com/dallay/agentsync/issues/264)) ([af72b18](https://github.com/dallay/agentsync/commit/af72b186bb81ffad06468536b04814454e5a427e))

## [1.35.0](https://github.com/dallay/agentsync/compare/v1.34.0...v1.35.0) (2026-03-29)


### Features

* symlink entire skills directory instead of individual skill entries ([#261](https://github.com/dallay/agentsync/issues/261)) ([c645fa0](https://github.com/dallay/agentsync/commit/c645fa06064f7eb4b1efd87841751e302ca61ccd))


### Bug Fixes

* keep a single backup file per destination ([16a057a](https://github.com/dallay/agentsync/commit/16a057ac0c0f451f4ceeb5f7155058d96be94de0))
* preserve existing skills symlink layouts in init wizard ([#262](https://github.com/dallay/agentsync/issues/262)) ([e81b4c5](https://github.com/dallay/agentsync/commit/e81b4c5e6293d9af929697b8b29d3a6982625ba8))
* surface nested-glob walk errors ([0e21606](https://github.com/dallay/agentsync/commit/0e21606f46bc61b600342024875b20b66da0ec09))


### Documentation

* comprehensive documentation audit and update ([#255](https://github.com/dallay/agentsync/issues/255)) ([7faee1d](https://github.com/dallay/agentsync/commit/7faee1d5b9a1e810c9e667425bc12ca9e846fbc0))


### Code Refactoring

* centralize nested-glob traversal ([a394af0](https://github.com/dallay/agentsync/commit/a394af08a275170d513163c2483fb7d7732de256))


### Chores

* **deps:** update dependency @dallay/agentsync to v1.34.0 ([#250](https://github.com/dallay/agentsync/issues/250)) ([7aa37c6](https://github.com/dallay/agentsync/commit/7aa37c6e2b329197595289f26fcf6f651377045c))
* update agentsync optional dependencies to v1.34.0 in pnpm lockfile ([5c2b06a](https://github.com/dallay/agentsync/commit/5c2b06a8cfc4a781da2689409828c4d77399db1f))

## [1.34.0](https://github.com/dallay/agentsync/compare/v1.33.1...v1.34.0) (2026-03-28)


### Features

* detect and migrate existing agent skills, commands, and configs during init wizard ([#259](https://github.com/dallay/agentsync/issues/259)) ([a3a9802](https://github.com/dallay/agentsync/commit/a3a980232901e70cc267dd4b72fc10b81df3c229))


### Bug Fixes

* sync release-please config to bump all npm versions automatically ([3c35bb5](https://github.com/dallay/agentsync/commit/3c35bb59d283911c60967bfc17c99f51dbf2e351))


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

## [1.28.0](https://github.com/dallay/agentsync/compare/v1.27.2...v1.28.0) (2026-02-09)

### ✨ Features

* add GitHub URL conversion for ZIP downloads and implement condition-based waiting skill ([#160](https://github.com/dallay/agentsync/issues/160)) ([86be61b](https://github.com/dallay/agentsync/commit/86be61b94fa83f3d9fc106fb6e6e4714a44fbb47))

### 📝 Documentation

* update README.md for accuracy and formatting ([#159](https://github.com/dallay/agentsync/issues/159)) ([8cb9f19](https://github.com/dallay/agentsync/commit/8cb9f19b2302456f0e4536f7a37ad9af5645e605))

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
