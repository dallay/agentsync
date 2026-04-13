# Delta for Core Sync Engine

## MODIFIED Requirements

### Requirement: Symlink-Contents Sync Type (Apply)

The system MUST create individual symlinks for each eligible item inside the source directory at the
destination directory for targets with `type = "symlink-contents"`.

The destination directory MUST be created if it does not exist.

Each symlink MUST use a relative path from the destination to the source item.

An existing source directory with zero eligible child items MUST be treated as a valid source
directory, not as a missing source.

(Previously: The system created individual symlinks for each item inside the source directory, but
the requirement did not explicitly define the empty existing source-directory case.)

#### Scenario: Empty existing source directory is a valid no-entry source

- GIVEN a config with `source = "commands"`, `destination = ".claude/commands"`, and
  `type = "symlink-contents"`
- AND the source directory exists
- AND the source directory contains zero eligible child items
- WHEN `linker.sync()` is run
- THEN the system MUST treat the target as having a valid source directory
- AND the system MUST NOT treat the target as a missing-source case solely because no child entries
  are present

## ADDED Requirements

### Requirement: Sync-Type-Aware Status Validation

The `status` command MUST validate each target according to that target's configured `sync_type`.

For targets with `type = "symlink"`, status MUST evaluate the destination path itself as the managed
symlink target.

For targets with `type = "symlink-contents"`, status MUST evaluate the destination path as a managed
directory container and MUST evaluate the expected managed child entries inside that directory.

For `symlink-contents`, status MUST derive the expected child-entry set from the eligible items in
the configured source directory.

If a `symlink-contents` source directory exists but has zero eligible child items, status MUST treat
that target as valid solely with respect to child-entry presence and MUST NOT report the target as
missing or "not a symlink" solely because the destination directory contains no managed child
entries.

#### Scenario: Status validates a symlink target as a single managed symlink

- GIVEN a target configured with `type = "symlink"`
- AND the destination path exists as a symlink to the expected source
- WHEN `agentsync status` is run
- THEN status MUST evaluate the destination path itself as the managed object
- AND the target MUST be reported as in sync

#### Scenario: Status validates a symlink-contents target as a directory container

- GIVEN a target configured with `type = "symlink-contents"`
- AND the source directory contains eligible child items `a.md` and `b.md`
- AND the destination path exists as a real directory
- AND the destination directory contains symlinks for `a.md` and `b.md` pointing to the expected
  source items
- WHEN `agentsync status` is run
- THEN status MUST evaluate the destination as a managed directory container
- AND the target MUST be reported as in sync

#### Scenario: Empty symlink-contents source directory does not create false drift

- GIVEN a target configured with `type = "symlink-contents"`
- AND the source directory exists but contains zero eligible child items
- AND the destination path exists as a real directory with no managed child entries
- WHEN `agentsync status` is run
- THEN status MUST NOT report the target as missing solely because no child entries exist
- AND status MUST NOT report the destination directory as "not a symlink" solely because the target
  uses `symlink-contents`

#### Scenario: Status detects a missing expected child symlink

- GIVEN a target configured with `type = "symlink-contents"`
- AND the source directory contains eligible child items `a.md` and `b.md`
- AND the destination directory contains a correct symlink for `a.md`
- AND the destination directory lacks the expected child entry for `b.md`
- WHEN `agentsync status` is run
- THEN status MUST report the target as drifted
- AND the reported drift MUST identify that an expected managed child entry is missing

#### Scenario: Status detects a wrong child type inside a symlink-contents destination

- GIVEN a target configured with `type = "symlink-contents"`
- AND the source directory contains an eligible child item `a.md`
- AND the destination path exists as a real directory
- AND the destination child `a.md` exists as a regular file or directory instead of a symlink to the
  expected source item
- WHEN `agentsync status` is run
- THEN status MUST report the target as drifted
- AND the reported drift MUST identify that the managed child entry has the wrong type or target

#### Scenario: Status detects an invalid destination type for symlink-contents

- GIVEN a target configured with `type = "symlink-contents"`
- AND the source directory exists
- AND the destination path exists as a non-directory object
- WHEN `agentsync status` is run
- THEN status MUST report the target as drifted
- AND the reported drift MUST identify that the destination type is invalid for a managed
  `symlink-contents` container
