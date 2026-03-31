#!/bin/bash
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
source "$SCRIPT_DIR/../lib/assert.sh"
source "$SCRIPT_DIR/../lib/fixtures.sh"

log_step "Preparing blank repository fixture"
REPO_ROOT=$(prepare_repo_from_fixture "blank-repo" "blank-repo")
cd "$REPO_ROOT"

log_step "Running agentsync init"
agentsync init

assert_dir_exists ".agents"
assert_file_exists ".agents/AGENTS.md"
assert_file_exists ".agents/agentsync.toml"
assert_file_contains ".agents/agentsync.toml" "[agents.claude]"
assert_file_contains ".agents/agentsync.toml" "[agents.gemini]"
assert_file_contains ".agents/agentsync.toml" "[agents.opencode]"

log_step "Applying generated configuration"
agentsync apply --verbose

assert_symlink_exists "CLAUDE.md"
assert_symlink_exists "GEMINI.md"
assert_symlink_exists "OPENCODE.md"
assert_symlink_exists ".github/copilot-instructions.md"

echo "✅ init blank repository scenario passed"
