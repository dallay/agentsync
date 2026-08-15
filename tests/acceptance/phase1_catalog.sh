#!/usr/bin/env bash
set -euo pipefail

# Reproducible Phase 1 black-box acceptance target.
#
# Run against an already-built external executable:
#   cargo build --release
#   AGENTSYNC_BIN=target/release/agentsync \
#     AGENTSYNC_SOURCE_REPO=../agents-skills \
#     tests/acceptance/phase1_catalog.sh
#
# The harness copies only the three approved Phase 1 skill directories into temporary
# source fixtures. It never calls Rust modules directly and never changes either checkout.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
CALLER_DIR="$PWD"
AGENTSYNC_BIN="${AGENTSYNC_BIN:-$REPO_ROOT/target/release/agentsync}"
SOURCE_REPO="${AGENTSYNC_SOURCE_REPO:-$REPO_ROOT/../agents-skills}"
PHASE1_SKILLS=(drizzle-orm pydantic sqlalchemy)

if [[ "$AGENTSYNC_BIN" != /* ]]; then
    AGENTSYNC_BIN="$CALLER_DIR/$AGENTSYNC_BIN"
fi

fail() {
    printf '❌ %s\n' "$*" >&2
    exit 1
}

log_step() {
    printf '• %s\n' "$*"
}

if [ ! -x "$AGENTSYNC_BIN" ]; then
    printf 'AGENTSYNC_BIN target does not exist or is not executable: %s\n' "$AGENTSYNC_BIN" >&2
    printf 'Build a release-like target first with: cargo build --release\n' >&2
    exit 1
fi

if [ ! -d "$SOURCE_REPO/skills" ]; then
    fail "AGENTSYNC_SOURCE_REPO has no skills directory: $SOURCE_REPO"
fi

for skill_id in "${PHASE1_SKILLS[@]}"; do
    [ -d "$SOURCE_REPO/skills/$skill_id" ] ||
        fail "AGENTSYNC_SOURCE_REPO is missing migrated skill: skills/$skill_id"
done

TMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/agentsync-phase1-acceptance.XXXXXX")"
trap 'rm -rf "$TMP_ROOT"' EXIT

COMMAND_CWD="$TMP_ROOT/command-cwd"
SIBLING_LAYOUT="$TMP_ROOT/sibling-layout"
SIBLING_REPO="$SIBLING_LAYOUT/agents-skills"
OVERRIDE_REPO="$TMP_ROOT/override-repo"
EMPTY_REPO="$TMP_ROOT/empty-repo"
mkdir -p "$COMMAND_CWD" "$SIBLING_REPO/skills" "$OVERRIDE_REPO/skills" "$EMPTY_REPO/skills"

copy_phase1_skills() {
    local destination="$1"
    local skill_id

    for skill_id in "${PHASE1_SKILLS[@]}"; do
        cp -R "$SOURCE_REPO/skills/$skill_id" "$destination/skills/"
    done
}

copy_phase1_skills "$SIBLING_REPO"
copy_phase1_skills "$OVERRIDE_REPO"

for skill_id in "${PHASE1_SKILLS[@]}"; do
    printf '\n<!-- phase1 acceptance sibling source: %s -->\n' "$skill_id" \
        >> "$SIBLING_REPO/skills/$skill_id/SKILL.md"
done
printf '\n<!-- phase1 acceptance override source: drizzle-orm -->\n' \
    >> "$OVERRIDE_REPO/skills/drizzle-orm/SKILL.md"

DIRECT_PROJECT="$SIBLING_LAYOUT/direct-project"
OVERRIDE_PROJECT="$SIBLING_LAYOUT/override-project"
SUGGEST_PROJECT="$SIBLING_LAYOUT/suggest-project"
MISSING_PROJECT="$TMP_ROOT/missing-project"
mkdir -p "$DIRECT_PROJECT" "$OVERRIDE_PROJECT" "$SUGGEST_PROJECT" "$MISSING_PROJECT"

run_cli() {
    local project_root="$1"
    local stdout_path="$2"
    local stderr_path="$3"
    shift 3

    (
        cd "$COMMAND_CWD"
        unset AGENTSYNC_LOCAL_SKILLS_REPO AGENTSYNC_TEST_SKILL_SOURCE_DIR
        export AGENTSYNC_NO_UPDATE_CHECK=1
        export HOME="$TMP_ROOT/home"
        export RUST_LOG=off
        "$AGENTSYNC_BIN" --log-level warn skill --project-root "$project_root" "$@"
    ) >"$stdout_path" 2>"$stderr_path"
}

run_cli_with_source() {
    local source_root="$1"
    local project_root="$2"
    local stdout_path="$3"
    local stderr_path="$4"
    shift 4

    (
        cd "$COMMAND_CWD"
        unset AGENTSYNC_TEST_SKILL_SOURCE_DIR
        export AGENTSYNC_LOCAL_SKILLS_REPO="$source_root"
        export AGENTSYNC_NO_UPDATE_CHECK=1
        export HOME="$TMP_ROOT/home"
        export RUST_LOG=off
        "$AGENTSYNC_BIN" --log-level warn skill --project-root "$project_root" "$@"
    ) >"$stdout_path" 2>"$stderr_path"
}

assert_file() {
    local path="$1"
    [ -f "$path" ] || fail "expected file to exist: $path"
}

assert_absent() {
    local path="$1"
    [ ! -e "$path" ] || fail "expected path to be absent: $path"
}

assert_contains() {
    local path="$1"
    local expected="$2"

    python3 - "$path" "$expected" <<'PY'
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
expected = sys.argv[2]
if expected not in path.read_text():
    raise SystemExit(f"expected {expected!r} in {path}")
PY
}

assert_not_contains() {
    local path="$1"
    local unexpected="$2"

    python3 - "$path" "$unexpected" <<'PY'
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
unexpected = sys.argv[2]
if unexpected in path.read_text():
    raise SystemExit(f"did not expect {unexpected!r} in {path}")
PY
}

assert_json_status() {
    local path="$1"
    local expected_status="$2"
    local skill_id="$3"

    python3 - "$path" "$expected_status" "$skill_id" <<'PY'
import json
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
expected_status = sys.argv[2]
skill_id = sys.argv[3]
payload = json.loads(path.read_text())
actual_status = payload.get("status")
if actual_status != expected_status:
    raise SystemExit(
        f"{skill_id}: expected JSON status {expected_status!r}, got {actual_status!r}"
    )
PY
}

assert_registry_entry() {
    local project_root="$1"
    local skill_id="$2"
    local registry="$project_root/.agents/skills/registry.json"

    assert_file "$registry"
    python3 - "$registry" "$skill_id" <<'PY'
import json
import pathlib
import sys

registry = pathlib.Path(sys.argv[1])
skill_id = sys.argv[2]
payload = json.loads(registry.read_text())
skills = payload.get("skills", {})
if skill_id not in skills:
    raise SystemExit(f"registry {registry} is missing local key {skill_id!r}")
PY
}

log_step "Installing all three migrated skills through the external CLI from sibling fixtures"
for skill_id in "${PHASE1_SKILLS[@]}"; do
    stdout_path="$TMP_ROOT/direct-$skill_id.stdout"
    stderr_path="$TMP_ROOT/direct-$skill_id.stderr"
    run_cli "$DIRECT_PROJECT" "$stdout_path" "$stderr_path" install "$skill_id" --json
    assert_json_status "$stdout_path" installed "$skill_id"
    assert_file "$DIRECT_PROJECT/.agents/skills/$skill_id/SKILL.md"
    assert_registry_entry "$DIRECT_PROJECT" "$skill_id"
    assert_contains \
        "$DIRECT_PROJECT/.agents/skills/$skill_id/SKILL.md" \
        "phase1 acceptance sibling source: $skill_id"
done

assert_file "$DIRECT_PROJECT/.agents/skills/drizzle-orm/references/advanced-schemas.md"
assert_file "$DIRECT_PROJECT/.agents/skills/drizzle-orm/references/performance.md"
assert_file "$DIRECT_PROJECT/.agents/skills/drizzle-orm/references/query-patterns.md"
assert_file "$DIRECT_PROJECT/.agents/skills/drizzle-orm/references/vs-prisma.md"
assert_file "$DIRECT_PROJECT/.agents/skills/pydantic/references/full-source.md"
assert_file "$DIRECT_PROJECT/.agents/skills/sqlalchemy/references/full-source.md"
assert_file \
    "$DIRECT_PROJECT/.agents/skills/sqlalchemy/references/sql-quality-antipatterns.md"

log_step "Checking AGENTSYNC_LOCAL_SKILLS_REPO override precedence"
override_stdout="$TMP_ROOT/override.stdout"
override_stderr="$TMP_ROOT/override.stderr"
run_cli_with_source \
    "$OVERRIDE_REPO" \
    "$OVERRIDE_PROJECT" \
    "$override_stdout" \
    "$override_stderr" \
    install drizzle-orm --json
assert_json_status "$override_stdout" installed drizzle-orm
assert_contains \
    "$OVERRIDE_PROJECT/.agents/skills/drizzle-orm/SKILL.md" \
    "phase1 acceptance override source: drizzle-orm"
assert_not_contains \
    "$OVERRIDE_PROJECT/.agents/skills/drizzle-orm/SKILL.md" \
    "phase1 acceptance sibling source: drizzle-orm"

log_step "Checking missing curated content fails closed without external fallback"
missing_stdout="$TMP_ROOT/missing.stdout"
missing_stderr="$TMP_ROOT/missing.stderr"
if run_cli_with_source \
    "$EMPTY_REPO" \
    "$MISSING_PROJECT" \
    "$missing_stdout" \
    "$missing_stderr" \
    install pydantic; then
    fail "missing curated pydantic source unexpectedly installed"
fi
assert_contains "$missing_stderr" "refusing external fallback"
assert_absent "$MISSING_PROJECT/.agents/skills/pydantic"

log_step "Checking suggestion install propagates the supplied project root"
cat > "$SUGGEST_PROJECT/pyproject.toml" <<'EOF'
[project]
name = "phase1-suggestion-fixture"
version = "0.0.0"
dependencies = ["pydantic>=2"]
EOF

# The Python detector also recommends three generic Python skills. Mark those paths as already
# installed so the black-box install selects only the migrated pydantic source; no unrelated source
# fixture or external resolution is used by this acceptance path.
mkdir -p \
    "$SUGGEST_PROJECT/.agents/skills/best-practices" \
    "$SUGGEST_PROJECT/.agents/skills/python-executor" \
    "$SUGGEST_PROJECT/.agents/skills/python-testing-patterns"

suggest_stdout="$TMP_ROOT/suggest.stdout"
suggest_stderr="$TMP_ROOT/suggest.stderr"
run_cli \
    "$SUGGEST_PROJECT" \
    "$suggest_stdout" \
    "$suggest_stderr" \
    suggest --install --all --json

python3 - "$suggest_stdout" <<'PY'
import json
import pathlib
import sys

payload = json.loads(pathlib.Path(sys.argv[1]).read_text())
recommendation = next(
    (item for item in payload["recommendations"] if item["skill_id"] == "pydantic"),
    None,
)
if recommendation is None:
    raise SystemExit("suggestion output did not include pydantic")
if recommendation["provider_skill_id"] != "dallay/agents-skills/pydantic":
    raise SystemExit("pydantic suggestion lost its qualified provider identity")

allowed = {
    "best-practices",
    "python-executor",
    "python-testing-patterns",
    "pydantic",
}
results = payload["results"]
unexpected = {item["skill_id"] for item in results} - allowed
if unexpected:
    raise SystemExit(f"suggestion acceptance attempted unrelated skills: {sorted(unexpected)}")

pydantic_result = next(item for item in results if item["skill_id"] == "pydantic")
if pydantic_result["status"] != "installed":
    raise SystemExit(f"pydantic suggestion did not install: {pydantic_result}")
if any(item["status"] == "failed" for item in results):
    raise SystemExit(f"suggestion acceptance reported a failed install: {results}")
PY

assert_file "$SUGGEST_PROJECT/.agents/skills/pydantic/SKILL.md"
assert_file "$SUGGEST_PROJECT/.agents/skills/pydantic/references/full-source.md"
assert_registry_entry "$SUGGEST_PROJECT" pydantic
assert_contains \
    "$SUGGEST_PROJECT/.agents/skills/pydantic/SKILL.md" \
    "phase1 acceptance sibling source: pydantic"
assert_absent "$COMMAND_CWD/.agents/skills/pydantic"

printf '✅ Phase 1 black-box acceptance passed against %s\n' "$AGENTSYNC_BIN"
