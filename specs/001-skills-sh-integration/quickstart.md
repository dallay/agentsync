# Quickstart: Skills.sh Integration

## Overview

The `agentsync` tool supports installing and managing AI agent skills from the `skills.sh` ecosystem and other built-in providers managed internally by the AgentSync team. Skills are downloaded to your local `.agents/skills/` directory and tracked in `registry.json`.

## Installation

Ensure you have the latest `agentsync` version (built from `001-skills-sh-integration` branch).

## Commands

### 1. Install a Skill

Install a skill by its ID (from skills.sh) or direct source (GitHub).

```bash
# Install by ID (searches skills.sh registry)
agentsync skill install rust-async-patterns

# Install from specific source (GitHub)
agentsync skill install vercel-labs/agent-skills/skills/frontend-design
```

### 2. Search for Skills

Find skills using keywords.

```bash
agentsync skill search "rust"
```

### 3. Update Skills

Update installed skills to the latest version.

```bash
# Update a specific skill
agentsync skill update rust-async-patterns

# Update all installed skills
agentsync skill update --all
```

### 4. List Installed Skills

View currently installed skills and their versions.

```bash
agentsync skill list
```

## Configuration

Skills are stored in `.agents/skills/`. Support for additional providers is managed internally; to add a new provider, contributors extend the source code base (no external plugins or binaries required).

## Validation

All installed skills are automatically validated against the [Agent Skills Specification](https://agentskills.io/specification). Invalid skills will fail to install with a detailed error message.
