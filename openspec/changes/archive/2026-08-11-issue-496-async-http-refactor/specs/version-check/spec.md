# Delta for version-check

## MODIFIED Requirements

### Requirement: crates.io API Query

The request SHALL use `reqwest::Client` (async) instead of `reqwest::blocking::Client`.

The request timeout SHALL be 3 seconds.

On success, the system SHALL parse the JSON response and extract the `crate.newest_version` field.

HTTP errors (timeout, connection failure, non-200 status) SHALL carry useful diagnostic context. A timeout detected while decoding the response body SHALL map to `Timeout`, not `ParseError`.

#### Scenario: API request succeeds with async client

- GIVEN the crates.io API returns a JSON response with `crate.newest_version = "0.4.0"`
- AND the current binary version is `"0.3.1"`
- WHEN the version check runs
- THEN the system SHALL parse `"0.4.0"` as the latest version
- AND SHALL compare it against the current version

#### Scenario: API request times out with context

- GIVEN the crates.io API does not respond within 3 seconds
- WHEN the timeout is reached
- THEN the request SHALL be cancelled silently
- AND the error SHALL carry timeout context (url and duration)
- AND no hint SHALL be printed
- AND no error SHALL propagate to the user

#### Scenario: API request fails with connection error

- GIVEN a connection error occurs during the API request
- WHEN the error is caught
- THEN the error SHALL carry the URL and the connection reason
- AND SHALL continue the CLI execution silently
- AND no hint SHALL be printed

#### Scenario: API returns non-200 status with context

- GIVEN the crates.io API returns a 4xx or 5xx status
- WHEN the response is received
- THEN the system SHALL record the URL and status code in the error context
- AND SHALL treat this as a failed check
- AND SHALL print no hint
- AND SHALL continue silently

---

### Requirement: Detached Background Thread

The version check SHALL run on a detached background thread spawned via `std::thread::Builder` with the explicit name `"agentsync-update-check"`.

The thread SHALL create its own Tokio runtime with `tokio::runtime::Runtime::new()` and execute the async check with `Runtime::block_on`; the check SHALL NOT use `tokio::spawn` or rely on an external runtime.

The thread SHALL be spawned after `Cli::parse` returns, as implemented by `main.rs`.

The thread SHALL NOT be joined — it SHALL exit naturally when the process exits.

The thread SHALL NOT block the main CLI flow.

#### Scenario: Detached thread spawns after CLI parsing

- GIVEN a CLI invocation of `agentsync`
- WHEN the program has parsed the CLI arguments
- THEN a detached background thread SHALL be spawned for the version check
- AND the main thread SHALL continue immediately without waiting

#### Scenario: Process exit terminates detached thread

- GIVEN a background thread is running the version check
- WHEN the CLI command completes and the process exits
- THEN the thread SHALL NOT prevent process exit
- AND no explicit thread handle SHALL be retained

---

### Requirement: Synchronous Path Documentation

Any function in the update check path that MUST remain synchronous SHALL be documented with a `// SAFETY:` or `// Note: runs on sync path` comment.

#### Scenario: Synchronous cache operations are documented

- GIVEN the cache load and save operations
- WHEN the code is reviewed
- THEN each synchronous-only operation SHALL have a comment explaining why it cannot be async
- OR the operation SHALL be marked with `// Note: sync path`
