# Specification: background-version-check

**Change**: background-version-check
**Date**: 2026-03-22
**Status**: DRAFT

---

## Purpose

Define the behavior of a non-blocking background version check that queries crates.io on every CLI
invocation and displays a one-time update hint when a newer version is available. The check runs on
a detached background thread, respects user opt-out via environment variables, and caches results to
avoid redundant network calls.

---

## Cache File

### Requirement: Cache File Format

The system SHALL store version check state in `~/.cache/agentsync/update-check.json`.

The cache file MUST contain the following JSON fields:

| Field              | Type    | Description                                    |
|--------------------|---------|------------------------------------------------|
| `last_checked`     | integer | Unix timestamp (seconds) of the last API fetch |
| `latest_version`   | string  | The newest version string from crates.io       |
| `notified_version` | string  | The version for which a hint has been printed  |

The cache file SHALL be created with `std::fs::create_dir_all` for parent directories if they do not
exist.

#### Scenario: Cache file does not exist

- GIVEN the cache file `~/.cache/agentsync/update-check.json` does not exist
- WHEN the version check runs
- THEN the system SHALL create the cache directory `~/.cache/agentsync/`
- AND the system SHALL treat this as a cache miss

#### Scenario: Cache file is valid and fresh

- GIVEN a valid cache file with `last_checked` within 24 hours and `notified_version` equal to
  `latest_version`
- WHEN the version check runs
- THEN the system SHALL skip the HTTP request
- AND no hint SHALL be printed

#### Scenario: Cache file is stale

- GIVEN a cache file with `last_checked` older than 24 hours
- WHEN the version check runs
- THEN the system SHALL fetch the crates.io API
- AND update `last_checked` and `latest_version` in the cache

---

## Thread Behavior

### Requirement: Detached Background Thread

The version check SHALL run on a detached background thread spawned via `std::thread::Builder`.

The thread SHALL be named `"agentsync-update-check"`.

The thread SHALL NOT be joined — it SHALL exit naturally when the process exits.

The thread SHALL NOT block the main CLI flow.

#### Scenario: Thread spawns on CLI invocation

- GIVEN a CLI invocation of `agentsync`
- WHEN the program starts
- THEN a background thread SHALL be spawned before CLI parsing
- AND the main thread SHALL continue immediately without waiting

#### Scenario: Process exits cancels thread

- GIVEN a background version check thread is running
- WHEN the CLI command completes and the process exits
- THEN any in-flight HTTP requests SHALL be cancelled
- AND no `join()` call SHALL be made

---

## Environment Opt-Out

### Requirement: Opt-Out via Environment Variables

The system SHALL skip the version check entirely when `AGENTSYNC_NO_UPDATE_CHECK=1` is set.

The system SHALL skip the version check entirely when `CI=true` is set.

Both checks SHALL be case-insensitive for the value.

#### Scenario: AGENTSYNC_NO_UPDATE_CHECK disables check

- GIVEN the environment variable `AGENTSYNC_NO_UPDATE_CHECK=1`
- WHEN `agentsync` is invoked
- THEN no background thread SHALL be spawned for version checking
- AND no network request SHALL be made
- AND no hint SHALL be printed

#### Scenario: CI=true disables check

- GIVEN the environment variable `CI=true`
- WHEN `agentsync` is invoked
- THEN no background thread SHALL be spawned for version checking
- AND no network request SHALL be made
- AND no hint SHALL be printed

#### Scenario: AGENTSYNC_NO_UPDATE_CHECK=0 does not opt out

- GIVEN the environment variable `AGENTSYNC_NO_UPDATE_CHECK=0`
- WHEN `agentsync` is invoked
- THEN the version check SHALL proceed normally

#### Scenario: ci=true (lowercase) still opts out

- GIVEN the environment variable `CI=true` (lowercase)
- WHEN `agentsync` is invoked
- THEN the version check SHALL be skipped (case-insensitive value check)

---

## TTY Detection

### Requirement: Output Only on TTY

The system SHALL only print the update hint to stderr when stderr is a TTY.

The system SHALL use the `is-terminal` crate to detect TTY status.

When stderr is not a TTY, the system SHALL still perform the background check and cache updates, but
SHALL NOT print output.

#### Scenario: stderr is a TTY

- GIVEN stderr is connected to a terminal
- WHEN a newer version is detected
- THEN the hint message SHALL be printed to stderr

#### Scenario: stderr is not a TTY

- GIVEN stderr is redirected to a file or pipe
- WHEN `agentsync` is invoked
- THEN no hint message SHALL be printed to stderr

---

## crates.io API

### Requirement: crates.io API Query

The system SHALL send a GET request to `https://crates.io/api/v1/crates/agentsync`.

The request SHALL use `reqwest::blocking::Client`.

The request timeout SHALL be 3 seconds.

On success, the system SHALL parse the JSON response and extract the `crate.newest_version` field.

#### Scenario: API request succeeds with newer version

- GIVEN the crates.io API returns a JSON response with `crate.newest_version = "0.4.0"`
- AND the current binary version is `"0.3.1"`
- WHEN the version check runs
- THEN the system SHALL parse `"0.4.0"` as the latest version
- AND SHALL compare it against the current version

#### Scenario: API request times out

- GIVEN the crates.io API does not respond within 3 seconds
- WHEN the timeout is reached
- THEN the request SHALL be cancelled silently
- AND no hint SHALL be printed
- AND no error SHALL propagate to the user

#### Scenario: API request fails with network error

- GIVEN a network error occurs during the API request
- WHEN the error is caught
- THEN the system SHALL log nothing to the user
- AND SHALL continue the CLI execution silently

#### Scenario: API returns non-200 status

- GIVEN the crates.io API returns a 4xx or 5xx status
- WHEN the response is received
- THEN the system SHALL treat this as a failed check
- AND SHALL print no hint
- AND SHALL continue silently

---

## Version Comparison

### Requirement: Semantic Version Comparison

The system SHALL parse versions using `semver::Version`.

Pre-release versions (e.g., `"0.4.0-beta.1"`) SHALL be ignored in comparisons and SHALL NOT trigger
hints.

The hint SHALL only print when the latest version is strictly greater than the running version.

#### Scenario: Newer stable version available

- GIVEN the latest version is `"0.4.0"` and the running version is `"0.3.1"`
- WHEN the comparison is made
- THEN a hint SHALL be printed
- AND `notified_version` SHALL be set to `"0.4.0"`

#### Scenario: Running version is latest

- GIVEN the latest version is `"0.3.1"` and the running version is `"0.3.1"`
- WHEN the comparison is made
- THEN no hint SHALL be printed

#### Scenario: Running version is newer than latest

- GIVEN the latest version is `"0.3.1"` and the running version is `"0.4.0-dev"`
- WHEN the comparison is made
- THEN no hint SHALL be printed

#### Scenario: Pre-release version ignored

- GIVEN the latest version is `"0.4.0-beta.1"` and the running version is `"0.3.1"`
- WHEN the comparison is made
- THEN no hint SHALL be printed
- AND pre-release SHALL be skipped

#### Scenario: semver parse failure

- GIVEN the crates.io API returns an invalid version string (e.g., `"not-a-version"`)
- WHEN parsing fails
- THEN the system SHALL skip the hint
- AND SHALL log nothing to the user

---

## Hint Output

### Requirement: One-Time Hint Per Version

The hint SHALL be printed to stderr only once per new version.

The `notified_version` field in the cache SHALL track which version has been notified.

The hint SHALL use the emoji prefix `💡` followed by the format:
`AgentSync {latest} available — you have {current}. Update: cargo install agentsync`

#### Scenario: New version first detected

- GIVEN a newer version `"0.4.0"` is detected
- AND `notified_version` in cache is `"0.3.1"` or absent
- WHEN the hint is printed
- THEN it SHALL appear exactly once
- AND `notified_version` SHALL be updated to `"0.4.0"`

#### Scenario: Same version notified before

- GIVEN a newer version `"0.4.0"` is detected
- AND `notified_version` in cache is already `"0.4.0"`
- WHEN the version check runs
- THEN no hint SHALL be printed
- AND the cache SHALL NOT be rewritten unnecessarily

#### Scenario: Different newer version after notification

- GIVEN version `"0.4.0"` was previously notified
- AND the new latest version is `"0.5.0"`
- WHEN the version check runs
- THEN a new hint SHALL be printed for `"0.5.0"`
- AND `notified_version` SHALL be updated to `"0.5.0"`

---

## Non-Functional Requirements

### NF-1: Performance

The `spawn_version_check()` call SHALL return immediately (< 1ms overhead).

The background thread SHALL NOT block the main CLI execution.

The HTTP request SHALL have a hard 3-second timeout.

### NF-2: Error Handling

All errors in the background thread SHALL be caught and silently dropped.

Network failures SHALL NOT print any error or warning to the user.

JSON parse failures SHALL NOT crash the CLI.

### NF-3: No Side Effects on Failure

If the cache file cannot be written, the system SHALL NOT fail the CLI.

If the thread cannot be spawned, the CLI SHALL continue normally.

### NF-4: Daemon Thread

The background thread SHALL be a daemon thread — it SHALL not prevent process exit.

The thread SHALL be spawned with `std::thread::Builder::spawn` and the handle SHALL be dropped
immediately.

---

## Edge Cases

### EC-1: Cache directory does not exist

- GIVEN `~/.cache/agentsync/` does not exist
- WHEN the cache is written
- THEN the system SHALL create the directory via `create_dir_all`
- AND SHALL proceed normally

### EC-2: Cache file is corrupted (invalid JSON)

- GIVEN the cache file exists but contains invalid JSON
- WHEN the cache is read
- THEN the system SHALL treat it as a cache miss
- AND SHALL fetch the API normally
- AND SHALL overwrite the corrupted file with valid JSON

### EC-3: Cache file has wrong schema (missing fields)

- GIVEN the cache file contains valid JSON but missing `last_checked`, `latest_version`, or
  `notified_version`
- WHEN the cache is read
- THEN the system SHALL treat it as a cache miss
- AND SHALL fetch the API normally

### EC-4: Cache file has invalid integer for last_checked

- GIVEN `last_checked` is a string instead of an integer
- WHEN the cache is deserialized
- THEN the system SHALL fall back to treating it as stale (cache miss)
- AND SHALL proceed with the API fetch

### EC-5: Version string from crates.io is pre-release only

- GIVEN crates.io returns only `"0.4.0-beta.1"` as the newest version
- WHEN the version is parsed
- THEN it SHALL be skipped
- AND no hint SHALL be printed
- AND `latest_version` SHALL NOT be updated in the cache (to avoid repeated skips)

### EC-6: semver parse failure on cached version

- GIVEN `latest_version` in the cache is `"invalid"`
- WHEN the version is parsed for comparison
- THEN the system SHALL skip the comparison
- AND SHALL attempt a fresh API fetch

### EC-7: Running version from `env!("CARGO_PKG_VERSION")` is unparseable

- GIVEN the compiled-in version string cannot be parsed as semver
- WHEN the comparison is attempted
- THEN no hint SHALL be printed
- AND no error SHALL be raised

### EC-8: Cache file is empty

- GIVEN the cache file exists but is empty (0 bytes)
- WHEN the cache is read
- THEN the system SHALL treat it as a cache miss
- AND SHALL fetch the API normally

### EC-9: Cache file is world-readable or has wrong permissions

- GIVEN the cache file has permissions that prevent writing
- WHEN the system attempts to write
- THEN the error SHALL be caught silently
- AND the CLI SHALL continue normally

### EC-10: Multiple CLI invocations race on cache write

- GIVEN two `agentsync` processes run simultaneously
- WHEN both attempt to write the cache
- THEN the last write wins (no locking required)
- AND no error SHALL propagate

---

## Acceptance Criteria

1. `spawn_version_check()` is called from `main()` before CLI parsing
2. Background thread is spawned with name `"agentsync-update-check"` and detached (no `join()`)
3. HTTP request goes to `https://crates.io/api/v1/crates/agentsync` with 3s timeout
4. Cache file is stored at `~/.cache/agentsync/update-check.json` with correct format
5. Cache TTL is 24 hours; fresh cache skips HTTP request
6. `AGENTSYNC_NO_UPDATE_CHECK=1` and `CI=true` skip all checks
7. Hint is printed to stderr only when stderr is a TTY
8. Hint prints exactly once per new version (tracked via `notified_version`)
9. Pre-release versions are ignored
10. Network errors, parse failures, and cache errors are silent
11. Thread does not block CLI execution
12. Thread is cancelled on process exit without `join()`
