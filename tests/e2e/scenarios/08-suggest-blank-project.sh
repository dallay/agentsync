#!/bin/bash
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
source "$SCRIPT_DIR/../lib/assert.sh"
source "$SCRIPT_DIR/../lib/fixtures.sh"

log_step "Preparing blank-repo fixture for empty suggest"
REPO_ROOT=$(prepare_repo_from_fixture "blank-repo" "blank-repo-suggest")
cd "$REPO_ROOT"

log_step "Running read-only skill suggestion in JSON mode"
agentsync skill suggest --json > suggest.json

log_step "Asserting zero technology detections"
assert_json_expr "suggest.json" '.detections | length == 0'

log_step "Asserting zero skill recommendations"
assert_json_expr "suggest.json" '.recommendations | length == 0'

log_step "Running human-readable suggestion output"
agentsync skill suggest > suggest.txt

log_step "Asserting empty-state messages"
assert_file_contains "suggest.txt" "Detected technologies: none"
assert_file_contains "suggest.txt" "Recommended skills: none"

echo "✅ suggest blank-project scenario passed"
