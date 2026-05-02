#!/bin/bash

fail() {
    echo "❌ $*" >&2
    exit 1
}

log_step() {
    echo "   • $*"
}

assert_file_exists() {
    local path="$1"
    [ -f "$path" ] || fail "Expected file to exist: $path"
}

assert_dir_exists() {
    local path="$1"
    [ -d "$path" ] || fail "Expected directory to exist: $path"
}

assert_path_not_exists() {
    local path="$1"
    [ ! -e "$path" ] || fail "Expected path to be absent: $path"
}

assert_symlink_exists() {
    local path="$1"
    [ -L "$path" ] || fail "Expected symlink to exist: $path"
    [ -e "$path" ] || fail "Expected symlink target to exist: $path"
}

assert_file_contains() {
    local path="$1"
    local expected="$2"
    grep -F -- "$expected" "$path" >/dev/null || fail "Expected '$expected' in $path"
}

assert_file_equals() {
    local path="$1"
    local expected_path="$2"
    cmp -s "$expected_path" "$path" || fail "Expected $path to match $expected_path"
}

assert_json_expr() {
    local path="$1"
    local expr="$2"
    jq -e "$expr" "$path" >/dev/null || fail "Expected jq expression to pass for $path: $expr"
}
