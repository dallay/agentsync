#!/bin/bash
set -eo pipefail

echo "🚀 Starting AgentSync E2E Suite..."

# --- 1. SETUP SIMULATED ENVIRONMENT ---
echo "📂 Setting up simulated paths..."
export HOME="/home/tester/fake_home"
export PROJECT_ROOT="/home/tester/app/my-project"
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

# --- 5. TEST: CLEAN ---
echo "▶️ Testing: agentsync clean"
agentsync clean

if [ ! -e "$LINK_PATH" ]; then
    echo "✅ Success: Symlink removed."
else
    echo "❌ Failure: Symlink still exists at $LINK_PATH"
    exit 1
fi

echo "🎉 All E2E tests passed successfully!"
