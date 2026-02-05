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

echo "⏳ Waiting for mock provider at ${PROVIDER_URL}/sample-skill.zip..."
for i in {1..20}; do
    if curl -s -f "${PROVIDER_URL}/sample-skill.zip" > /dev/null; then
        echo "✅ Mock provider is up and serving the ZIP!"
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

echo "🎉 All E2E tests passed successfully!"
