#!/bin/bash
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
source "$SCRIPT_DIR/../lib/assert.sh"
source "$SCRIPT_DIR/../lib/fixtures.sh"

log_step "Preparing mega monorepo fixture"
REPO_ROOT=$(prepare_repo_from_fixture "mega-monorepo" "mega-monorepo")
cd "$REPO_ROOT"

log_step "Running read-only suggestion on the mega monorepo"
agentsync skill suggest --json > mega-suggest.json

assert_json_expr "mega-suggest.json" '.detections | any(.technology == "rust")'
assert_json_expr "mega-suggest.json" '.detections | any(.technology == "node_typescript")'
assert_json_expr "mega-suggest.json" '.detections | any(.technology == "astro")'
assert_json_expr "mega-suggest.json" '.detections | any(.technology == "github_actions")'
assert_json_expr "mega-suggest.json" '.detections | any(.technology == "docker")'
assert_json_expr "mega-suggest.json" '.detections | any(.technology == "make")'
assert_json_expr "mega-suggest.json" '.detections | any(.technology == "python")'
assert_json_expr "mega-suggest.json" '.detections | any(.technology == "rust" and .confidence == "high")'
assert_json_expr "mega-suggest.json" '.detections | any(.technology == "node_typescript" and .confidence == "high")'
assert_json_expr "mega-suggest.json" '.detections | any(.technology == "astro" and .confidence == "high")'

echo "✅ suggest mega-monorepo scenario passed"
