#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SOURCE_REPO="${AGENTSYNC_SOURCE_REPO:-$REPO_ROOT/../agents-skills}"
VALIDATOR="$SOURCE_REPO/scripts/validate_provenance.py"

if [ ! -f "$VALIDATOR" ]; then
    printf 'provenance validator is missing: %s\n' "$VALIDATOR" >&2
    exit 1
fi

python3 "$VALIDATOR" --root "$SOURCE_REPO"
