# Delta for Documentation

## ADDED Requirements

### Requirement: Commands Naming And Status Semantics Stay Consistent Across Documentation

Documentation that explains commands targets or the `status` command MUST distinguish the canonical
source directory from agent-specific destination directories.

The documentation MUST identify `.agents/commands/` as the canonical source directory for commands
content.

The documentation MUST identify agent-specific destination directories using their shipped paths,
including `.claude/commands/`, `.gemini/commands/`, and `.opencode/command/`.

Documentation for `agentsync status` MUST explain that `symlink` targets are validated as a single
destination-path symlink, while `symlink-contents` targets are validated as destination directories
whose managed child entries are the symlinks.

Documentation for `agentsync status` MUST explain that an existing empty `symlink-contents` source
directory is valid and that an empty destination directory for that target MUST NOT be described as
missing or "not a symlink" solely because it has no managed child entries yet.

#### Scenario: Reader learns canonical commands naming from configuration or README docs

- GIVEN a reader reviews configuration examples, migration guidance, or README content for commands
  targets
- WHEN the docs describe where commands content lives
- THEN the docs MUST identify `.agents/commands/` as the canonical source directory
- AND the docs MUST distinguish that source from agent-specific destinations such as
  `.claude/commands/`, `.gemini/commands/`, and `.opencode/command/`

#### Scenario: Reader learns sync-type-aware status semantics from CLI docs

- GIVEN a reader reviews CLI documentation for `agentsync status`
- WHEN the docs explain how status evaluates targets
- THEN the docs MUST explain that `symlink` checks the destination path itself as a symlink
- AND the docs MUST explain that `symlink-contents` checks the destination directory and expected
  managed child symlinks

#### Scenario: Reader is not misled by empty symlink-contents commands directories

- GIVEN a reader has migrated commands content into `.agents/commands/`
- AND one or more agent destinations currently correspond to an empty `symlink-contents` source
  directory
- WHEN the reader reviews status semantics in the docs
- THEN the docs MUST explain that the empty source directory is still valid
- AND the docs MUST explain that an empty destination directory is not, by itself, a missing-target
  or not-a-symlink problem for `symlink-contents`
