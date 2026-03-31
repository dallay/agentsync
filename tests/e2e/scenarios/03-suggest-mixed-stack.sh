#!/bin/bash
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
source "$SCRIPT_DIR/../lib/assert.sh"
source "$SCRIPT_DIR/../lib/fixtures.sh"

log_step "Preparing mixed-stack demo repository"
REPO_ROOT=$(prepare_repo_from_fixture "mixed-stack-a" "mixed-stack-a-suggest")
cd "$REPO_ROOT"

log_step "Running read-only skill suggestion in JSON mode"
agentsync skill suggest --json > suggest.json

assert_json_expr "suggest.json" '.detections | any(.technology == "rust")'
assert_json_expr "suggest.json" '.detections | any(.technology == "node_typescript")'
assert_json_expr "suggest.json" '.detections | any(.technology == "astro")'
assert_json_expr "suggest.json" '.detections | any(.technology == "github_actions")'
assert_json_expr "suggest.json" '.detections | any(.technology == "docker")'
assert_json_expr "suggest.json" '.detections | any(.technology == "make")'
assert_json_expr "suggest.json" '.detections | any(.technology == "python")'
assert_json_expr "suggest.json" '.recommendations | any(.skill_id == "rust-async-patterns")'
assert_json_expr "suggest.json" '.recommendations | any(.skill_id == "best-practices")'
assert_json_expr "suggest.json" '.recommendations | any(.skill_id == "frontend-design")'
assert_json_expr "suggest.json" '.recommendations | any(.skill_id == "github-actions")'
assert_json_expr "suggest.json" '.recommendations | any(.skill_id == "docker-expert")'
assert_json_expr "suggest.json" '.recommendations | any(.skill_id == "makefile")'

log_step "Running human-readable suggestion output"
agentsync skill suggest > suggest.txt
assert_file_contains "suggest.txt" "Detected technologies"
assert_file_contains "suggest.txt" "rust-async-patterns"
assert_file_contains "suggest.txt" "github-actions"

echo "✅ suggest mixed-stack scenario passed"
