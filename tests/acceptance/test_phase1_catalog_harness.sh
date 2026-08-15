#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
HARNESS="$REPO_ROOT/tests/acceptance/phase1_catalog.sh"

if [ ! -x "$HARNESS" ]; then
    printf 'phase1 acceptance harness is missing or not executable: %s\n' "$HARNESS" >&2
    exit 1
fi

set +e
output=$(AGENTSYNC_BIN="$REPO_ROOT/target/does-not-exist" "$HARNESS" 2>&1)
status=$?
set -e

if [ "$status" -eq 0 ]; then
    printf 'expected the acceptance harness to reject a missing binary\n' >&2
    exit 1
fi

case "$output" in
    *"AGENTSYNC_BIN target does not exist or is not executable"*)
        printf 'phase1 acceptance harness missing-binary contract passed\n'
        ;;
    *)
        printf 'unexpected missing-binary failure:\n%s\n' "$output" >&2
        exit 1
        ;;
esac
