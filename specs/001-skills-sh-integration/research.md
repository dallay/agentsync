# Phase 0: Research & Technical Strategy

## 1. Skills.sh Ecosystem

### Registry API
The `skills.sh` registry is accessible via a public API:
- **Endpoint**: `https://skills.sh/api/search`
- **Method**: `GET`
- **Query**: `?q=<term>`
- **Response**: JSON object containing a list of skills.
  ```json
  {
    "query": "term",
    "searchType": "fuzzy",
    "skills": [
      {
        "id": "skill-name",
        "name": "Skill Name",
        "installs": 123,
        "topSource": "owner/repo"
      }
    ]
  }
  ```
- **Interpretation**: The `id` corresponds to the `name` field in the `SKILL.md` frontmatter. `topSource` identifies the GitHub repository hosting the skill.

### Skill Format (Agent Skills Specification)
- **Structure**: A directory containing `SKILL.md`.
- **Frontmatter**: YAML block with `name`, `description`, etc.
- **Content**: Markdown instructions.
- **Auxiliary**: `assets/`, `scripts/`, `references/`.
- **Reference**: [https://agentskills.io/specification](https://agentskills.io/specification)

### Installation Logic (Reverse Engineered)
Based on the `skills` CLI documentation and API:
1. **Discovery**: Resolve `skill-id` to a source repository (via API or direct user input).
2. **Acquisition**: Clone or download the repository (shallow clone preferred for performance).
3. **Extraction**: Locate the directory containing `SKILL.md` with the matching `name`.
4. **Installation**: Copy the skill directory to `.agents/skills/<skill-id>`.

## 2. Provider Design (Internal Only)

Providers are implemented as internal Rust modules, each supporting a set of skill sources/APIs (e.g., skills.sh by default). No runtime plugin protocol or subprocess discovery is used. To add new providers, contributors extend the codebase with new Provider trait implementations or handler modules.

## 3. Storage & State

### Local Registry
- **Location**: `.agents/skills/registry.json`
- **Purpose**: Track installed skills to ensure idempotency and support updates.
- **Schema**:
  ```json
  {
    "schemaVersion": 1,
    "skills": {
      "skill-id": {
        "version": "1.0.0", // from manifest or commit hash
        "provider": "skills.sh",
        "source": "owner/repo",
        "installedAt": "2024-01-27T...",
        "files": ["SKILL.md", "assets/logo.png"]
      }
    }
  }
  ```

### Directory Structure
```text
.agents/
  skills/
    registry.json
    <skill-id>/
      SKILL.md
      assets/
```

## 4. Security Considerations
- **Manifest Validation**: MUST parse and validate `SKILL.md` frontmatter against the spec.
- **Code Execution**: The spec mentions `scripts/`. AgentSync MUST NOT execute these during install. We will warn the user if `scripts/` are present.
- **Path Traversal**: Validate filenames in the skill archive to prevent writing outside `.agents/skills/<skill-id>`.
