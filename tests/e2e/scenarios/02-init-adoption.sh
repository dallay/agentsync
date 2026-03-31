#!/bin/bash
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
source "$SCRIPT_DIR/../lib/assert.sh"
source "$SCRIPT_DIR/../lib/fixtures.sh"

log_step "Preparing multi-agent adoption fixture"
REPO_ROOT=$(prepare_repo_from_fixture "ai-adoption" "ai-adoption")

log_step "Running agentsync init --wizard through a pseudo-TTY"
run_with_tty "\n\n\n\n\n\n\n\n\n\n\n\n" "cd '$REPO_ROOT' && agentsync init --wizard"

cd "$REPO_ROOT"

assert_dir_exists ".agents"
assert_file_exists ".agents/agentsync.toml"
assert_file_exists ".agents/AGENTS.md"
assert_file_exists ".agents/.cursor/mcp.json"

for skill_id in debugging review-skill format-skill cursor-skill opencode-skill; do
    assert_file_exists ".agents/skills/${skill_id}/SKILL.md"
done

for command_name in review.md analyze.md fix.md; do
    assert_file_exists ".agents/commands/${command_name}"
done

assert_file_contains ".agents/AGENTS.md" "Claude adoption instructions"
assert_file_contains ".agents/AGENTS.md" "Gemini adoption instructions"
assert_file_contains ".agents/AGENTS.md" "Root agent instructions for Codex"
assert_file_contains ".agents/AGENTS.md" "OpenCode adoption instructions"
assert_file_contains ".agents/AGENTS.md" "Copilot adoption instructions"

log_step "Removing original agent files to verify apply recreates them"
rm -rf .claude .gemini .codex .cursor .opencode .github .vscode
rm -f CLAUDE.md GEMINI.md OPENCODE.md AGENTS.md opencode.json

agentsync apply --verbose

assert_symlink_exists "CLAUDE.md"
assert_symlink_exists "GEMINI.md"
assert_symlink_exists "OPENCODE.md"
assert_symlink_exists "AGENTS.md"
assert_symlink_exists ".claude/skills"
assert_symlink_exists ".gemini/skills"
assert_symlink_exists ".codex/skills"
assert_symlink_exists ".opencode/skills"
assert_symlink_exists ".opencode/command/fix.md"
assert_symlink_exists ".github/copilot-instructions.md"

echo "✅ init adoption repository scenario passed"
