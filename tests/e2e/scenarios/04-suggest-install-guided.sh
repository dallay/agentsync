#!/bin/bash
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
source "$SCRIPT_DIR/../lib/assert.sh"
source "$SCRIPT_DIR/../lib/fixtures.sh"

log_step "Preparing mixed-stack repository for guided install"
REPO_ROOT=$(prepare_repo_from_fixture "mixed-stack-a" "mixed-stack-a-guided")
SKILL_SOURCE_ROOT="${E2E_BASE_DIR}/skill-sources-guided"
prepare_skill_sources \
    "$SKILL_SOURCE_ROOT" \
    accessibility \
    best-practices \
    core-web-vitals \
    docker-expert \
    frontend-design \
    github-actions \
    makefile \
    performance \
    pinned-tag \
    rust-async-patterns \
    seo

export AGENTSYNC_TEST_SKILL_SOURCE_DIR="$SKILL_SOURCE_ROOT"

log_step "Running guided install in a pseudo-TTY"
run_with_tty "\n" "cd '$REPO_ROOT' && agentsync skill suggest --install"

cd "$REPO_ROOT"
for skill_id in \
    rust-async-patterns \
    best-practices \
    frontend-design \
    accessibility \
    performance \
    core-web-vitals \
    seo \
    github-actions \
    pinned-tag \
    docker-expert \
    makefile; do
    assert_file_exists ".agents/skills/${skill_id}/SKILL.md"
done

agentsync skill suggest --json > post-install.json
assert_json_expr "post-install.json" '.recommendations | all(.installed == true)'

echo "✅ guided suggest install scenario passed"
