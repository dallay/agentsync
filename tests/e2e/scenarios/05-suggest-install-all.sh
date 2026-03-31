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
agentsync skill suggest --install --all --json > install-all.json
assert_json_expr "install-all.json" '.results | any(.skill_id == "rust-async-patterns" and .status == "installed")'
assert_json_expr "install-all.json" '.results | any(.skill_id == "docker-expert" and .status == "installed")'

log_step "Re-running install-all to confirm already-installed behavior"
agentsync skill suggest --install --all --json > install-all-repeat.json
assert_json_expr "install-all-repeat.json" '.results | length > 0 and all(.status == "already_installed")'

echo "✅ suggest install-all scenario passed"
