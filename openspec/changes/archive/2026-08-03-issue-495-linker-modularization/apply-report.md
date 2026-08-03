# Apply Report: issue-495-linker-modularization

## Layer

- Strategy: `github-stacked-prs`
- Final implementation layer: `issue-495-linker-apply-clean`
- Delivery: five code layers followed by archived SDD documentation layers
- Scope: behavior-preserving linker extraction

## Completed

- Moved `src/linker.rs` to `src/linker/mod.rs` and declared private `apply`, `clean`, `discovery`,
  `paths`, and `symlinks` modules.
- Extracted path canonicalization, destination safety, TOCTOU revalidation, safe unlink validation,
  path caching, and relative-path calculation into `paths.rs`.
- Extracted symlink creation/update, contents linking, backups, removal, accounting, and Unix/
  Windows gates into `symlinks.rs`.
- Extracted nested-glob walking, excludes, templates, matching, and discovery caching into
  `discovery.rs`.
- Extracted synchronization application, source/compression resolution, module-map application,
  and cleanup orchestration into `apply.rs` and `clean.rs`.
- Preserved the public facade, shared `Linker` state, inline tests, callers, output, counters,
  cache resets, security checks, and synchronous execution model.

## Verification

- `cargo fmt --all -- --check` — PASS.
- `cargo check --all-targets --all-features` — PASS after review fixes.
- `cargo test --lib linker` — 83 passed.
- Focused security/integration tests — 12 passed.
- `cargo test --test all_tests unit::linker_security` — 11 passed.
- `cargo test --all-features` — PASS after review fixes; doctest examples from the generated
  docstring commit were removed because they were not valid repository examples.
- `cargo clippy --all-targets --all-features -- -D warnings` — PASS.

## Final Status

The implementation was verified and archived. No base specification, public API, or caller was
changed. Windows runtime validation is delegated to CI; the initial Windows compile failure from
the module extraction was fixed by restoring the module-local `FileTypeExt` import.
