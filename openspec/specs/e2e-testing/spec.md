# E2E Testing Specification

**Change**: issue-496-async-http-refactor
**Date**: 2026-08-11
**Status**: ACTIVE

---

## Purpose

Define behavior for end-to-end catalog integrity tests that verify shipped curated skill entries are reachable at their pinned commits. The tests run async HTTP requests against the GitHub API and are gated behind `RUN_E2E=1`.

---

## Requirements

### Requirement: E2E Catalog Integrity Test Uses Async Client

The catalog integrity test SHALL use `reqwest::Client` (async) with `#[tokio::test]` instead of `reqwest::blocking::Client`.

The test SHALL retain the existing 15-second timeout and retry logic.

The `RUN_E2E=1` gate SHALL remain unchanged.

#### Scenario: All curated entries reachable — async client

- GIVEN all curated registry entries have reachable SKILL.md files at their pinned commits
- WHEN the E2E catalog integrity test runs with `RUN_E2E=1`
- THEN the async HTTP requests SHALL succeed for each entry
- AND the test SHALL pass with no failures

#### Scenario: Retry on transient failure with async client

- GIVEN the first HTTP request to the GitHub API fails with a transient error
- WHEN the retry logic executes
- THEN a second async request SHALL be sent after a 2-second delay
- AND if successful, the entry SHALL be marked OK

#### Scenario: Timeout on slow endpoint with context

- GIVEN the GitHub API does not respond within 15 seconds
- WHEN the timeout is reached
- THEN the request SHALL be cancelled
- AND the entry SHALL be added to the failure list with timeout context
- AND the test SHALL fail with a panic showing all failures

#### Scenario: Non-200 HTTP response carries status context

- GIVEN the GitHub API returns a 404 for an entry
- WHEN the response is received
- THEN the failure SHALL include the HTTP status code
- AND the test SHALL fail with a descriptive message

#### Scenario: Network error carries diagnostic context

- GIVEN a network error occurs during an HTTP request
- WHEN the error is caught after retry
- THEN the failure SHALL include the network error description
- AND the test SHALL fail with a panic showing all failures
