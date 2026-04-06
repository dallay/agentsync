#!/bin/bash
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
source "$SCRIPT_DIR/../lib/assert.sh"
source "$SCRIPT_DIR/../lib/fixtures.sh"

log_step "Preparing mixed-stack repository for install-all"
REPO_ROOT=$(prepare_repo_from_fixture "mixed-stack-a" "mixed-stack-a-install-all")
SKILL_SOURCE_ROOT="${E2E_BASE_DIR}/skill-sources-install-all"
prepare_default_skill_sources "$SKILL_SOURCE_ROOT"

export AGENTSYNC_TEST_SKILL_SOURCE_DIR="$SKILL_SOURCE_ROOT"

cd "$REPO_ROOT"

log_step "Running non-interactive install-all flow"
agentsync skill suggest --install --all > install-all.txt
assert_file_contains "install-all.txt" "Installing 13 recommended skills..."
assert_file_contains "install-all.txt" "installed rust-async-patterns"
assert_file_contains "install-all.txt" "installed docker-expert"
assert_file_contains "install-all.txt" "Recommendation install summary"
assert_file_contains "install-all.txt" "Installed: 13"
assert_file_contains "install-all.txt" "Already installed: 0"
assert_file_contains "install-all.txt" "Failed: 0"

log_step "Re-running install-all to confirm already-installed behavior"
agentsync skill suggest --install --all > install-all-repeat.txt
assert_file_contains "install-all-repeat.txt" "already installed rust-async-patterns"
assert_file_contains "install-all-repeat.txt" "already installed docker-expert"
assert_file_contains "install-all-repeat.txt" "Recommendation install summary"
assert_file_contains "install-all-repeat.txt" "Installed: 0"
assert_file_contains "install-all-repeat.txt" "Already installed: 13"
assert_file_contains "install-all-repeat.txt" "Failed: 0"
assert_file_contains "install-all-repeat.txt" "Note: nothing installable to do."

echo "✅ suggest install-all scenario passed"
