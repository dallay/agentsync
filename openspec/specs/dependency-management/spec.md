# Dependency Management Specification

**Change**: issue-496-async-http-refactor
**Date**: 2026-08-11
**Status**: ACTIVE

---

## Purpose

Define constraints on the `reqwest` dependency after the async refactor — specifically the removal of the `blocking` feature and the guarantee that no `reqwest::blocking` usage remains in the codebase.

---

## Requirements

### Requirement: Blocking Feature Removed from Cargo.toml

After all three file conversions verify green, the `reqwest` entry in `Cargo.toml` SHALL have the `"blocking"` feature removed.

#### Scenario: Cargo.toml no longer has blocking feature

- GIVEN all three file conversions compile and pass tests
- WHEN the Cargo.toml is updated
- THEN `reqwest = { version = "0.13.3", features = ["json", "gzip", "stream"] }`
- AND `"blocking"` SHALL NOT appear in the features list

---

### Requirement: No reqwest::blocking in Source or Tests

After the migration, `reqwest::blocking` SHALL NOT appear in any source file under `src/` or test file under `tests/`.

#### Scenario: No blocking imports after migration

- GIVEN the refactor is complete
- WHEN `grep -r "reqwest::blocking" src/ tests/` is run
- THEN the search SHALL return no results

---

### Requirement: Error Context on HTTP Operations

All HTTP error types (timeout, connection, non-200 status) SHALL carry useful context that enables diagnosis without guessing.

#### Scenario: HTTP errors carry timeout vs connection vs status context

- GIVEN a failed HTTP operation in `fetch_latest_version` or `resolve_via_search`
- WHEN the error is caught and logged or returned
- THEN the error context SHALL distinguish between:
  - Timeout errors (request exceeded time limit)
  - Connection errors (DNS, TCP, TLS failures — including redirect loops, which surface with the reason text)
  - Non-200 status errors (4xx/5xx responses with status codes)
