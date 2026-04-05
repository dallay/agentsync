#!/bin/bash

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
E2E_ROOT=$(cd "$SCRIPT_DIR/.." && pwd)

export E2E_BASE_DIR="${E2E_BASE_DIR:-/home/tester}"
export HOME="${E2E_BASE_DIR}/fake_home"
export E2E_WORK_ROOT="${E2E_BASE_DIR}/app"

setup_e2e_dirs() {
    mkdir -p "$HOME" "$E2E_WORK_ROOT"
}

repo_fixture_path() {
    local fixture_name="$1"
    printf '%s/fixtures/repos/%s\n' "$E2E_ROOT" "$fixture_name"
}

prepare_repo_from_fixture() {
    local fixture_name="$1"
    local repo_name="$2"
    local fixture_path
    fixture_path=$(repo_fixture_path "$fixture_name")
    local target_path="${E2E_WORK_ROOT}/${repo_name}"

    setup_e2e_dirs
    rm -rf "$target_path"
    mkdir -p "$target_path"
    cp -R "$fixture_path"/. "$target_path"/

    printf '%s\n' "$target_path"
}

create_skill_source() {
    local root="$1"
    local skill_id="$2"
    local skill_dir="${root}/${skill_id}"

    mkdir -p "$skill_dir"
    cat >"${skill_dir}/SKILL.md" <<EOF
---
name: ${skill_id}
version: 1.0.0
description: Fixture skill for ${skill_id}
---
# ${skill_id}
EOF
}

prepare_skill_sources() {
    local root="$1"
    shift

    rm -rf "$root"
    mkdir -p "$root"

    local skill_id
    for skill_id in "$@"; do
        create_skill_source "$root" "$skill_id"
    done
}

prepare_default_skill_sources() {
    local root="$1"

    prepare_skill_sources \
        "$root" \
        accessibility \
        astrolicious-astro \
        best-practices \
        brainstorming \
        core-web-vitals \
        docker-expert \
        frontend-design \
        github-actions \
        makefile \
        nothing-design \
        performance \
        pinned-tag \
        pr-creator \
        rust-async-patterns \
        seo \
        skill-creator \
        sql-optimization-patterns \
        web-quality-audit \
        webapp-testing \
        grafana-dashboards \
        markdown-a11y
}

wait_for_mock_provider() {
    local provider_url="${PROVIDER_URL:-}"
    [ -n "$provider_url" ] || return 0

    for _ in $(seq 1 20); do
        if curl --connect-timeout 2 --max-time 5 -sSf "$provider_url" >/dev/null 2>&1; then
            return 0
        fi
        sleep 1
    done

    fail "Mock provider did not become ready: ${provider_url}"
}

run_with_tty() {
    local input_text="$1"
    local command="$2"
    local input_file
    local old_trap

    if ! command -v script >/dev/null 2>&1; then
        fail "The 'script' command is required for interactive E2E flows"
    fi

    input_file=$(mktemp)
    old_trap=$(trap -p EXIT || true)
    trap 'rm -f "$input_file"' EXIT
    printf '%b' "$input_text" > "$input_file"
    script -qec "$command" /dev/null < "$input_file"
    rm -f "$input_file"

    if [ -n "$old_trap" ]; then
        eval "$old_trap"
    else
        trap - EXIT
    fi
}
