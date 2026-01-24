---
name: pinned-tag
description: >
  Skill for managing "pinned tags" and commit SHAs, primarily for GitHub Actions security.
  Trigger: When fixing "Unpinned tag for a non-immutable Action" warnings or when pinning dependencies to specific git versions.
license: Apache-2.0
metadata:
  author: prowler-cloud
  version: "1.1"
---

## Overview
This skill helps locate and validate tags in remote repositories using `git ls-remote --tags`, resolving the specific commit SHA for a tag (including annotated tags), and providing patches to pin dependencies. It specifically targets **GitHub Actions** security best practices by encouraging the use of full commit SHAs instead of mutable tags.

## When to Use
- You need to fix "Unpinned tag for a non-immutable Action in workflow" security alerts.
- You want to ensure reproducibility by pinning a GitHub Action or dependency to a specific commit SHA.
- You need to verify if a tag exists before updating a manifest or CI workflow.

## Critical Patterns
- **Immutability for Actions**: For GitHub Actions, always prefer the full 40-character commit SHA over a tag name. Tags are mutable and can be moved, leading to security risks or broken builds.
- **Annotated vs Lightweight Tags**: 
  - **Annotated Tags**: `git ls-remote` returns two entries. The one ending in `^{}` is the "peeled" reference pointing directly to the commit object. **ALWAYS use this one**.
  - **Lightweight Tags**: Return only one entry, which is the commit SHA.
  - **Selection Logic**: When resolving, always sort the results and take the last one to ensure `^{}` is preferred over the tag object SHA.
- **Verification**: Always verify the resolved SHA belongs to the expected tag before applying changes.

## Commands

1. **Resolve Tag to SHA (GitHub Actions Friendly)**:
   ```bash
   # Example: Resolve 'v2' tag for actions/checkout
   # This command ensures that for annotated tags, the commit SHA (peeled tag ^{}) is selected
   # by sorting and taking the last entry (where ^{} alphabetically follows the base tag).
   git ls-remote --tags https://github.com/actions/checkout.git | rg "refs/tags/v2(\^\{\})?$" | sort | tail -n 1 | awk '{print $1}'
   ```

2. **List all remote tags**:
   ```bash
   git ls-remote --tags https://github.com/owner/repo.git
   ```

3. **Check for specific tag existence**:
   ```bash
   git ls-remote --tags https://github.com/owner/repo.git | rg "refs/tags/v1.2.3"
   ```

## Workflow for GitHub Actions
1. **Identify the Action**: Find the `uses: owner/repo@tag` line in the workflow YAML.
2. **Resolve SHA**: Run `git ls-remote --tags https://github.com/owner/repo.git` filtered by the tag.
3. **Apply Patch**: Replace the tag with the SHA and add the tag name as a comment for readability.
   - *Before*: `uses: actions/checkout@v3`
   - *After*: `uses: actions/checkout@f43a0e5ff2bd294095638e18286ca9a3d1956744 # v3`

## Limitations
- Cannot access private repositories without proper credentials (SSH/Token).
- Some actions may not use standard semver tags; always double-check the remote refs.

## Commit Policy
- This skill does NOT create commits automatically without explicit permission.
- Suggested commit message: `sec(ci): pin <action-name> to commit SHA for immutability`

## Resources
- [GitHub Security Best Practices for Actions](https://docs.github.com/en/actions/security-guides/security-hardening-for-github-actions#using-third-party-actions)
- Tools: `git`, `rg`, `awk`
