# Delta for skill-recommendations

## MODIFIED Requirements

### Requirement: Provider Skill Resolution Uses Async HTTP

`resolve_via_search()` SHALL use `reqwest::Client` (async) instead of `reqwest::blocking::Client`.

The resolution SHALL apply the bridge pattern: use `tokio::runtime::Handle::try_current()` to detect an existing runtime, and either run the async HTTP call inline via `handle.block_on()` or spin up a temporary runtime.

HTTP errors (timeout, connection failure, non-200 status) SHALL carry useful diagnostic context.

#### Scenario: resolve_via_search succeeds with async client

- GIVEN a skills.sh API response containing a matching skill
- WHEN `resolve_via_search()` is called
- THEN the async HTTP request SHALL succeed
- AND the skill download URL SHALL be constructed and returned

#### Scenario: resolve_via_search bridges via try_current when runtime exists

- GIVEN `resolve_via_search()` is called from a context where a Tokio runtime is already active
- WHEN the bridge pattern checks `Handle::try_current()`
- THEN the HTTP call SHALL run via `handle.block_on()`
- AND the result SHALL be returned without creating a new runtime

#### Scenario: resolve_via_search creates temporary runtime when none exists

- GIVEN `resolve_via_search()` is called from a synchronous context with no Tokio runtime
- WHEN the bridge pattern checks `Handle::try_current()`
- THEN a temporary `Runtime::new()` SHALL be created
- AND the HTTP call SHALL run via `rt.block_on()`
- AND the result SHALL be returned

#### Scenario: resolve_via_search timeout carries diagnostic context

- GIVEN the skills.sh API does not respond within 10 seconds
- WHEN the timeout is reached
- THEN the error SHALL include timeout context
- AND no crash or user-facing error SHALL occur

#### Scenario: resolve_via_search connection error carries context

- GIVEN a connection error occurs during the HTTP request
- WHEN the error is caught
- THEN the error SHALL include connection diagnostic information
- AND the error SHALL be returned as a not-found or resolution failure

#### Scenario: resolve_via_search non-200 response carries context

- GIVEN the skills.sh API returns a 4xx or 5xx status
- WHEN the response is received
- THEN the error SHALL include the status code
- AND the resolution SHALL fail gracefully
