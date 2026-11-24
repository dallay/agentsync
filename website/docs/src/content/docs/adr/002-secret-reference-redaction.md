---
title: "ADR-002: Secret Reference, Redaction, and Reproducibility Contract"
description: "Establishes the contract for secret references, redaction, and reproducibility in AgentSync."
---

# ADR-002: Secret Reference, Redaction, and Reproducibility Contract

**Status:** Accepted  
**Date:** 2026-11-23  
**Issue:** #702  
**Epic:** #686  
**Unblocks:** #688, #689, #690, #691, #694, #696, #699

## Context

AgentSync handles MCP server configurations that may contain secrets (API keys, tokens, credentials). These secrets must never be persisted in lockfiles, diagnostic output, logs, or repository contents. This ADR establishes a single contract for secret references, redaction, and reproducibility.

## Decision

### Secret Reference Syntax

The only accepted late-bound reference syntax is `${ENV_NAME}` where `ENV_NAME` is a valid environment variable name (alphanumeric and underscores, starting with a letter or underscore).

**Escaping Rules:**
- To represent a literal `${` in configuration, use `$$` as an escape prefix: `$$${ENV_NAME}` produces the literal string `${ENV_NAME}`
- The `$$` escape is only recognized at the start of a `${...}` pattern

**Unsupported Provider Behavior:**
- If a provider does not support environment variable expansion, the reference is passed through as-is
- No automatic fallback to literal values

### SecretRef vs Opaque Sensitive Literal Distinction

**SecretRef:** A string matching `${ENV_NAME}` pattern where `ENV_NAME` is a valid identifier. These are safe to log and serialize.

**Opaque Sensitive Literal:** Any string that does not match the SecretRef pattern but appears in a sensitive context (env values, headers, command args for sensitive flags). These must be redacted.

**Classification Rules:**
- Never classify an arbitrary string as safe without evidence
- Sensitive keys are identified by pattern matching against known sensitive key names (see `is_sensitive_mcp_key` in `src/plugins.rs`)
- All other values in sensitive contexts are treated as opaque and redacted

### Redaction Matrix

| Output Channel | SecretRef | Opaque Sensitive Literal |
|----------------|-----------|--------------------------|
| stdout (human) | Show reference name | `<redacted>` |
| stdout (JSON) | Show reference name | `<redacted>` |
| stderr | Show reference name | `<redacted>` |
| Errors | Show reference name | `<redacted>` |
| Tracing logs | Show reference name | `<redacted>` |
| Lockfiles | Show reference name | Never included |
| Hashing | Hash the reference, not resolved value | Never hash literals |
| Dry-run previews | Show reference name | `<redacted>` |

### Legacy Plaintext Value Handling

**Backward-Compatible Parse:**
- Existing configurations with plaintext secrets continue to work
- These values are treated as opaque sensitive literals

**Privacy-Safe Diagnostics:**
- When detecting plaintext secrets in sensitive contexts, emit a warning diagnostic
- Never include the actual secret value in the diagnostic message

**Explicit Remediation:**
- Provide clear migration guidance: replace `sk-abc123` with `${MY_API_KEY}`
- No automatic rewriting of existing configurations

### Profile Merge Constraints

**Security Rule:** A lower-trust profile cannot weaken secret-handling or MCP approval restrictions.

**Merge Semantics:**
- Secret references from base config are preserved
- Profile overlays cannot introduce plaintext secrets where the base uses references
- MCP approval restrictions from base config cannot be relaxed by profiles

### Threat Examples

1. **Credentials in headers:** `Authorization: Bearer sk-abc123` → redacted
2. **URL query strings:** `https://api.example.com?token=sk-abc123` → `https://api.example.com?token=<redacted>`
3. **Command arguments:** `--api-key sk-abc123` → `--api-key=<redacted>`
4. **Malformed references:** `${INVALID-NAME}` → treated as opaque literal, redacted
5. **Environment references in URLs:** `https://${API_HOST}/path` → preserved as reference

### Test Fixture Plan

**Secret Canaries:**
- Use distinct, non-functional test values: `TEST_SECRET_CANARY_12345`
- Verify these values never appear in:
  - stdout output
  - stderr output
  - JSON output
  - Log files
  - Lockfile data
  - Error messages

**Test Scenarios:**
1. Explicit MCP with `${ENV}` reference → reference preserved
2. Explicit MCP with plaintext secret → redacted in output
3. Plugin MCP with sensitive headers → redacted
4. Inspect output with secrets → redacted
5. Import with secrets → redacted
6. Profile merge with secrets → redacted
7. Lock generation with secrets → references only

### Network/Secret Manager Calls

**Read-Only Flows:**
- No network calls during inspection, validation, or audit
- No secret manager API calls during read-only operations
- Environment variable resolution happens at runtime, not during configuration processing

## Consequences

### Positive
- Secrets never leak through any output channel
- Deterministic output for testing and CI
- Clear migration path from plaintext secrets
- Profile merges cannot weaken security

### Negative
- Users must manage environment variables externally
- No built-in secret storage or rotation
- Legacy configs require manual migration

### Neutral
- Existing redaction code in `src/inspect.rs` already implements most of this contract
- The `is_sensitive_mcp_key` function in `src/plugins.rs` provides the key classification

## References

- `src/inspect.rs` - Redaction functions (`redact_args`, `redact_url`, `classify_values`)
- `src/plugins.rs` - Key classification (`is_sensitive_mcp_key`, `is_environment_reference`)
- `src/config.rs` - MCP server configuration types
- ADR-001 - ResolvedConfiguration contract
