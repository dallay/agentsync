# Sentinel Journal - AgentSync Security

## 2025-05-15 - Path Traversal in Symlink Destinations

**Vulnerability:** Symlink destinations derived from untrusted `agentsync.toml` were joined to the
project root without validation, allowing a malicious configuration to create symlinks at arbitrary
locations on the filesystem (e.g., `../../.ssh/authorized_keys`).
**Learning:** Even when using `Path::join`, if the second path is absolute, it overwrites the first.
And even if relative, `..` components can escape the intended root. Rust's `std::path::Path` does
not automatically restrict paths to a base directory.
**Prevention:** Always validate that the resolved destination path is relative and does not contain
`..` components before joining with the project root, or use `canonicalize` and `starts_with` to
ensure the final path remains within the sandbox.
