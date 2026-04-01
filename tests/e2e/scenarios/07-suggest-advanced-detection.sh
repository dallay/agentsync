#!/bin/bash
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
source "$SCRIPT_DIR/../lib/assert.sh"
source "$SCRIPT_DIR/../lib/fixtures.sh"

log_step "Preparing advanced-detection-stack fixture"
REPO_ROOT=$(prepare_repo_from_fixture "advanced-detection-stack" "advanced-detection-suggest")
cd "$REPO_ROOT"

log_step "Running read-only skill suggestion in JSON mode"
agentsync skill suggest --json > suggest.json

# --- Technology detections ---

log_step "Asserting package_patterns detection (azure via ^@azure/)"
assert_json_expr "suggest.json" '.detections | any(.technology == "azure")'

log_step "Asserting packages + package_patterns detection (clerk via @clerk/nextjs)"
assert_json_expr "suggest.json" '.detections | any(.technology == "clerk")'

log_step "Asserting packages detection (cloudflare via wrangler)"
assert_json_expr "suggest.json" '.detections | any(.technology == "cloudflare")'

log_step "Asserting config_file_content detection (durable_objects in wrangler.json)"
assert_json_expr "suggest.json" '.detections | any(.technology == "cloudflare_durable_objects")'

log_step "Asserting file_extensions detection (web_frontend via .tsx, .css)"
assert_json_expr "suggest.json" '.detections | any(.technology == "web_frontend")'

log_step "Asserting packages detection (nextjs via next)"
assert_json_expr "suggest.json" '.detections | any(.technology == "nextjs")'

# --- Skill recommendations ---

log_step "Asserting Azure skill recommendations"
assert_json_expr "suggest.json" '.recommendations | any(.skill_id | contains("azure"))'

log_step "Asserting Clerk skill recommendations"
assert_json_expr "suggest.json" '.recommendations | any(.skill_id | contains("clerk"))'

log_step "Asserting Cloudflare skill recommendations"
assert_json_expr "suggest.json" '.recommendations | any(.skill_id | contains("cloudflare"))'

log_step "Asserting Durable Objects skill recommendation"
assert_json_expr "suggest.json" '.recommendations | any(.skill_id == "cloudflare-durable-objects")'

log_step "Asserting web frontend skill recommendations"
assert_json_expr "suggest.json" '.recommendations | any(.skill_id | contains("frontend-design"))'

echo "✅ suggest advanced-detection scenario passed"
