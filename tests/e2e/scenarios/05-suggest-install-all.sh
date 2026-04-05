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
assert_file_contains "install-all.txt" "Completed suggest install: 13 installed, 0 already installed, 0 failed."

log_step "Re-running install-all to confirm already-installed behavior"
agentsync skill suggest --install --all > install-all-repeat.txt
assert_file_contains "install-all-repeat.txt" "already installed rust-async-patterns"
assert_file_contains "install-all-repeat.txt" "already installed docker-expert"
assert_file_contains "install-all-repeat.txt" "Completed suggest install: nothing installable to do (13 already installed)."

echo "✅ suggest install-all scenario passed"
