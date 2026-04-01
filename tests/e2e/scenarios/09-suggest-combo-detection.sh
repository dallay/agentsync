#!/bin/bash
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
source "$SCRIPT_DIR/../lib/assert.sh"
source "$SCRIPT_DIR/../lib/fixtures.sh"

log_step "Preparing combo-stack fixture (Next.js + Supabase + React)"
REPO_ROOT=$(prepare_repo_from_fixture "combo-stack" "combo-stack-suggest")
cd "$REPO_ROOT"

log_step "Running read-only skill suggestion in JSON mode"
agentsync skill suggest --json > suggest.json

# --- Individual technology detections ---

log_step "Asserting nextjs detection"
assert_json_expr "suggest.json" '.detections | any(.technology == "nextjs")'

log_step "Asserting supabase detection"
assert_json_expr "suggest.json" '.detections | any(.technology == "supabase")'

log_step "Asserting react detection"
assert_json_expr "suggest.json" '.detections | any(.technology == "react")'

# --- Combo-triggered recommendations ---

log_step "Asserting nextjs-supabase combo skill recommendation"
assert_json_expr "suggest.json" '.recommendations | any(.skill_id == "supabase-postgres-best-practices")'

log_step "Asserting Next.js individual skills also present"
assert_json_expr "suggest.json" '.recommendations | any(.skill_id | contains("next-best-practices"))'

echo "✅ suggest combo-detection scenario passed"
