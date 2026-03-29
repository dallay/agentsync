#!/bin/bash
set -eo pipefail

echo "🚀 Starting AgentSync E2E Suite..."

# --- 1. SETUP SIMULATED ENVIRONMENT ---
echo "📂 Setting up simulated paths..."
# Use a configurable base directory for simulated paths, defaulting to /home/tester
E2E_BASE_DIR="${E2E_BASE_DIR:-/home/tester}"
export HOME="${E2E_BASE_DIR}/fake_home"
export PROJECT_ROOT="${E2E_BASE_DIR}/app/my-project"
export CURSOR_CONFIG="$HOME/.config/Cursor/User/globalStorage"

mkdir -p "$CURSOR_CONFIG"
mkdir -p "$PROJECT_ROOT"
cd "$PROJECT_ROOT"

# --- 2. TEST: INIT ---
echo "▶️ Testing: agentsync init"
agentsync init
if [ -f ".agents/AGENTS.md" ]; then
    echo "✅ Success: .agents directory and AGENTS.md created."
else
    echo "❌ Failure: .agents/AGENTS.md not found."
    exit 1
fi

# --- 3. TEST: APPLY (Symlink) ---
echo "▶️ Testing: agentsync apply"
# Create a dummy rule file in the project
mkdir -p .agents/rules
echo "# Test Rule Content" > .agents/rules/test-rule.md

# Update agentsync.toml to include the custom cursor agent
# We use an absolute path for destination to simulate linking to system config
cat <<EOF > .agents/agentsync.toml
source_dir = "."

[agents.claude]
enabled = true
targets.instructions = { source = "AGENTS.md", destination = "CLAUDE.md", type = "symlink" }

[agents.cursor]
enabled = true
targets.test_rule = { source = "rules/test-rule.md", destination = "$CURSOR_CONFIG/test-rule.md", type = "symlink" }
EOF

agentsync apply --verbose

# Verify symlink
LINK_PATH="$CURSOR_CONFIG/test-rule.md"
if [ -L "$LINK_PATH" ]; then
    echo "✅ Success: Symlink created at $LINK_PATH"
    # The readlink will be a relative path from $CURSOR_CONFIG to .agents/rules/test-rule.md
    # agentsync calculates the relative path automatically.
    TARGET=$(readlink "$LINK_PATH")
    echo "DEBUG: readlink $LINK_PATH -> $TARGET"
    if [[ "$TARGET" == *"test-rule.md" ]]; then
        echo "✅ Success: Symlink points to a test-rule.md file."
    else
        echo "❌ Failure: Symlink points to unexpected target: $TARGET"
        exit 1
    fi
else
    echo "❌ Failure: Symlink not created at $LINK_PATH"
    ls -la "$CURSOR_CONFIG"
    exit 1
fi

# --- 4. TEST: SKILL INSTALL (Remote) ---
echo "▶️ Testing: agentsync skill install (Remote Mock)"
# The mock-provider serves sample-skill.zip at http://mock-provider/sample-skill.zip
# We use the PROVIDER_URL env var from docker-compose
SKILL_URL="${PROVIDER_URL}/sample-skill.zip"

echo "⏳ Waiting for mock provider at $PROVIDER_URL..."
for i in {1..10}; do
    if curl -s -f "$PROVIDER_URL" > /dev/null; then
        echo "✅ Mock provider is up!"
        break
    fi
    echo "..."
    sleep 1
done

agentsync skill install --source "$SKILL_URL" sample-skill

# Verify skill installation
SKILL_DIR=".agents/skills/sample-skill"
if [ -f "$SKILL_DIR/SKILL.md" ]; then
    echo "✅ Success: Skill installed from remote ZIP."
else
    echo "❌ Failure: Skill manifest not found at $SKILL_DIR/SKILL.md"
    exit 1
fi

# --- 5. TEST: SKILL INSTALL (Real from skills.sh source) ---
if [ "${RUN_REAL_SKILL_TEST}" = "1" ]; then
    echo "▶️ Testing: agentsync skill install (Real GitHub ZIP)"

    # Use a real skill from skills.sh (pinned to a specific commit)
    REAL_SKILL_URL="https://github.com/anthropics/skills/archive/69c0b1a0674149f27b61b2635f935524b6add202.zip"
    GITHUB_DOMAIN="github.com"

    # Preflight network check
    echo "⏳ Checking connectivity to ${GITHUB_DOMAIN}..."
    if ! curl -s --head --request GET "https://${GITHUB_DOMAIN}" | grep "200 OK" > /dev/null; then
        echo "❌ Failure: GitHub is unreachable. Aborting real skill test."
        exit 1
    fi
    echo "✅ GitHub is reachable."

    agentsync skill install --source "$REAL_SKILL_URL" frontend-design

    # Verify skill installation from monorepo/GitHub ZIP
    SKILL_DIR=".agents/skills/frontend-design"
    if [ -f "$SKILL_DIR/SKILL.md" ]; then
        echo "✅ Success: frontend-design skill installed from GitHub ZIP."
        # Check that it found the right subdirectory by looking for the name in the manifest
        # The manifest uses YAML frontmatter
        if grep -q "name: frontend-design" "$SKILL_DIR/SKILL.md"; then
            echo "✅ Success: Found correct SKILL.md content."
        else
            echo "❌ Failure: SKILL.md content does not match expected skill name."
            cat "$SKILL_DIR/SKILL.md"
            exit 1
        fi
    else
        echo "❌ Failure: Skill manifest not found at $SKILL_DIR/SKILL.md"
        if [ -d "$SKILL_DIR" ]; then
            echo "Contents of $SKILL_DIR:"
            tree "$SKILL_DIR" || ls -R "$SKILL_DIR"
        fi
        exit 1
    fi
else
    echo "⏭️ Skipping real skill test (RUN_REAL_SKILL_TEST not set to 1)."
fi

# --- 6. TEST: CLEAN ---
echo "▶️ Testing: agentsync clean"
agentsync clean

if [ ! -e "$LINK_PATH" ]; then
    echo "✅ Success: Symlink removed."
else
    echo "❌ Failure: Symlink still exists at $LINK_PATH"
    exit 1
fi

# --- 7. TEST: AGENT ADOPTION (Claude with skills + commands) ---
echo "▶️ Testing: Agent adoption — existing Claude repo with skills and commands"
ADOPT_DIR="${E2E_BASE_DIR}/app/adopt-test"
mkdir -p "$ADOPT_DIR"
cd "$ADOPT_DIR"

# Simulate an existing Claude repo
mkdir -p .claude/skills/debugging
cat > .claude/skills/debugging/SKILL.md << 'SKILL_EOF'
---
name: debugging
version: 1.0.0
description: Debugging helpers
---
# Debugging Skill
SKILL_EOF

mkdir -p .claude/skills/testing
cat > .claude/skills/testing/SKILL.md << 'SKILL_EOF'
---
name: testing
version: 1.0.0
description: Testing helpers
---
# Testing Skill
SKILL_EOF

mkdir -p .claude/commands
echo "# Review the code for issues" > .claude/commands/review.md

echo "# Project Instructions" > CLAUDE.md

# Run init (non-wizard, creates .agents/ structure)
agentsync init

if [ -d ".agents" ] && [ -f ".agents/agentsync.toml" ]; then
    echo "✅ Success: .agents/ created via init."
else
    echo "❌ Failure: .agents/ not created."
    exit 1
fi

# Simulate wizard migration: copy skills and commands into .agents/
cp -r .claude/skills/debugging .agents/skills/
cp -r .claude/skills/testing .agents/skills/
mkdir -p .agents/commands
cp .claude/commands/review.md .agents/commands/
cp CLAUDE.md .agents/AGENTS.md

# Write a config with skills + commands targets
cat > .agents/agentsync.toml << 'CONFIG_EOF'
source_dir = "."

[gitignore]
enabled = false

[agents.claude]
enabled = true

[agents.claude.targets.instructions]
source = "AGENTS.md"
destination = "CLAUDE.md"
type = "symlink"

[agents.claude.targets.skills]
source = "skills"
destination = ".claude/skills"
type = "symlink-contents"

[agents.claude.targets.commands]
source = "commands"
destination = ".claude/commands"
type = "symlink-contents"
CONFIG_EOF

# Remove originals to prove apply recreates them via symlinks
rm -rf .claude
rm -f CLAUDE.md

# Apply
agentsync apply --verbose

# Verify CLAUDE.md symlink
if [ -L "CLAUDE.md" ]; then
    echo "✅ Success: CLAUDE.md symlink created."
else
    echo "❌ Failure: CLAUDE.md symlink not found."
    exit 1
fi

# Verify skills symlinks
if [ -L ".claude/skills/debugging" ]; then
    echo "✅ Success: .claude/skills/debugging symlink created."
else
    echo "❌ Failure: .claude/skills/debugging symlink not found."
    ls -la .claude/skills/ 2>/dev/null || echo "  .claude/skills/ does not exist"
    exit 1
fi

if [ -L ".claude/skills/testing" ]; then
    echo "✅ Success: .claude/skills/testing symlink created."
else
    echo "❌ Failure: .claude/skills/testing symlink not found."
    exit 1
fi

# Verify commands symlink
if [ -L ".claude/commands/review.md" ]; then
    echo "✅ Success: .claude/commands/review.md symlink created."
else
    echo "❌ Failure: .claude/commands/review.md symlink not found."
    ls -la .claude/commands/ 2>/dev/null || echo "  .claude/commands/ does not exist"
    exit 1
fi

# Verify content is accessible through symlinks
if grep -q "Debugging Skill" .claude/skills/debugging/SKILL.md; then
    echo "✅ Success: Skill content accessible through symlink."
else
    echo "❌ Failure: Skill content not accessible through symlink."
    exit 1
fi

echo "✅ Agent adoption test passed!"

# --- 8. TEST: MULTI-AGENT ADOPTION (Claude + Gemini + Codex) ---
echo "▶️ Testing: Multi-agent adoption — Claude + Gemini + Codex"
MULTI_DIR="${E2E_BASE_DIR}/app/multi-adopt-test"
mkdir -p "$MULTI_DIR"
cd "$MULTI_DIR"

# Simulate multi-agent repo
echo "# Claude rules" > CLAUDE.md
echo "# Gemini rules" > GEMINI.md

mkdir -p .claude/skills/debug-skill
cat > .claude/skills/debug-skill/SKILL.md << 'SKILL_EOF'
---
name: debug-skill
version: 1.0.0
---
# Debug
SKILL_EOF

mkdir -p .gemini/skills/review-skill
cat > .gemini/skills/review-skill/SKILL.md << 'SKILL_EOF'
---
name: review-skill
version: 1.0.0
---
# Review
SKILL_EOF

mkdir -p .codex/skills/format-skill
cat > .codex/skills/format-skill/SKILL.md << 'SKILL_EOF'
---
name: format-skill
version: 1.0.0
---
# Format
SKILL_EOF

# Init
agentsync init

# Simulate wizard: merge all skills into .agents/skills/
cp -r .claude/skills/debug-skill .agents/skills/
cp -r .gemini/skills/review-skill .agents/skills/
cp -r .codex/skills/format-skill .agents/skills/

# Merge instructions
cat CLAUDE.md GEMINI.md > .agents/AGENTS.md

# Config for all three agents
cat > .agents/agentsync.toml << 'CONFIG_EOF'
source_dir = "."

[gitignore]
enabled = false

[agents.claude]
enabled = true

[agents.claude.targets.instructions]
source = "AGENTS.md"
destination = "CLAUDE.md"
type = "symlink"

[agents.claude.targets.skills]
source = "skills"
destination = ".claude/skills"
type = "symlink-contents"

[agents.gemini]
enabled = true

[agents.gemini.targets.instructions]
source = "AGENTS.md"
destination = "GEMINI.md"
type = "symlink"

[agents.gemini.targets.skills]
source = "skills"
destination = ".gemini/skills"
type = "symlink-contents"

[agents.codex]
enabled = true

[agents.codex.targets.skills]
source = "skills"
destination = ".codex/skills"
type = "symlink-contents"
CONFIG_EOF

# Remove originals
rm -rf .claude .gemini .codex
rm -f CLAUDE.md GEMINI.md

# Apply
agentsync apply --verbose

# Verify all agents get skills from the shared source
FAIL=0
for AGENT_DIR in .claude .gemini .codex; do
    for SKILL in debug-skill review-skill format-skill; do
        if [ -L "${AGENT_DIR}/skills/${SKILL}" ]; then
            echo "✅ ${AGENT_DIR}/skills/${SKILL} symlink exists."
        else
            echo "❌ ${AGENT_DIR}/skills/${SKILL} symlink missing!"
            FAIL=1
        fi
    done
done

# Verify instruction symlinks
if [ -L "CLAUDE.md" ] && [ -L "GEMINI.md" ]; then
    echo "✅ Success: Instruction symlinks created for Claude and Gemini."
else
    echo "❌ Failure: Instruction symlinks missing."
    FAIL=1
fi

if [ "$FAIL" -eq 1 ]; then
    echo "❌ Multi-agent adoption test FAILED."
    exit 1
fi

echo "✅ Multi-agent adoption test passed!"

echo "🎉 All E2E tests passed successfully!"
