# Data Model

## Local Registry (`.agents/skills/registry.json`)

The registry tracks installed skills, their sources, and update metadata.

```json
{
  "schemaVersion": 1,
  "lastUpdated": "2026-01-27T12:00:00Z",
  "skills": {
    "skill-id": {
      "name": "skill-name",         // From SKILL.md frontmatter
      "version": "1.0.0",           // Optional, from manifest or commit SHA
      "description": "Description", // From SKILL.md
      "provider": "skills.sh",      // Source provider (managed internally by AgentSync)
      "source": "owner/repo",       // Original source (e.g., GitHub repo)
      "installedAt": "2026-01-27T12:00:00Z",
      "files": [                    // Installed files relative to skill root
        "SKILL.md",
        "assets/icon.png"
      ],
      "manifestHash": "sha256:..."  // Hash of SKILL.md for integrity
    }
  }
}
```

### Fields

- **schemaVersion**: Version of the registry file format.
- **skills**: Map of skill IDs to metadata.
- **skill-id**: Unique identifier (typically the directory name).
- **files**: List of files installed for cleanup/update logic.
- **manifestHash**: SHA-256 hash of `SKILL.md` to detect manual edits or updates.

## Manifest Format (`SKILL.md`)

Conforms to [Agent Skills Specification](https://agentskills.io/specification).

```yaml
---
name: skill-name           # Required: lowercase, alphanumeric + hyphens
description: Description   # Required: < 1024 chars
license: Apache-2.0        # Optional
metadata:                  # Optional map
  version: "1.0"
---

# Skill Body
Markdown content...
```

## Provider Handling

AgentSync uses internal provider modules (implementing a Provider trait internally) to support multiple sources. Third-party plugin/discovery is not supported.

### Search Result
```json
{
  "id": "skill-id",
  "name": "Skill Name",
  "description": "Short description",
  "source": "owner/repo",
  "downloads": 123
}
```

### Install Result
```json
{
  "id": "skill-id",
  "version": "commit-sha-or-semver",
  "files": [
    {
      "path": "SKILL.md",
      "content": "..." // OR url to download
    }
  ]
}
```
