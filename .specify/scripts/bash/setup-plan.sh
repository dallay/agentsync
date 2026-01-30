#!/usr/bin/env bash

set -e

# Parse command line arguments
JSON_MODE=false

for arg in "$@"; do
    case "$arg" in
        --json) 
            JSON_MODE=true 
            ;;
        --help|-h) 
            echo "Usage: $0 [--json]"
            echo "  --json    Output results in JSON format"
            echo "  --help    Show this help message"
            exit 0 
            ;;
        *) 
            # Non-option positional arguments are not used by this script; ignore
            ;;
    esac
done

# Get script directory and load common functions
SCRIPT_DIR="$(CDPATH="" cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

# Global JSON escape helper
json_escape() {
    # read value from stdin and JSON-encode it
    if command -v python3 >/dev/null 2>&1; then
        python3 - <<'PY'
import json,sys
data = sys.stdin.read()
print(json.dumps(data))
PY
    else
        # Fallback to jq if python3 not available
        jq -R -s -c '.'
    fi
}

# Get all paths and variables from common functions
eval "$(get_feature_paths)"

# Check if we're on a proper feature branch (only for git repos)
check_feature_branch "$CURRENT_BRANCH" "$HAS_GIT" || exit 1

# Ensure the feature directory exists
mkdir -p "$FEATURE_DIR"

# Copy plan template if it exists, only if IMPL_PLAN doesn't already exist
if [[ -f "$IMPL_PLAN" ]]; then
    echo "Plan already exists at $IMPL_PLAN; skipping creation."
else
    TEMPLATE="$REPO_ROOT/.specify/templates/plan-template.md"
    if [[ -f "$TEMPLATE" ]]; then
        cp "$TEMPLATE" "$IMPL_PLAN"
        echo "Copied plan template to $IMPL_PLAN"
    else
        echo "Warning: Plan template not found at $TEMPLATE"
        # Create a basic plan file if template doesn't exist
        touch "$IMPL_PLAN"
    fi
fi

# Output results
if $JSON_MODE; then
    esc_spec=$(printf '%s' "$FEATURE_SPEC" | json_escape)
    esc_plan=$(printf '%s' "$IMPL_PLAN" | json_escape)
    esc_dir=$(printf '%s' "$FEATURE_DIR" | json_escape)
    esc_branch=$(printf '%s' "$CURRENT_BRANCH" | json_escape)
    esc_git=$(printf '%s' "$HAS_GIT" | json_escape)
    printf '{"FEATURE_SPEC":%s,"IMPL_PLAN":%s,"SPECS_DIR":%s,"BRANCH":%s,"HAS_GIT":%s}\n' \
        "$esc_spec" "$esc_plan" "$esc_dir" "$esc_branch" "$esc_git"
else
    echo "FEATURE_SPEC: $FEATURE_SPEC"
    echo "IMPL_PLAN: $IMPL_PLAN" 
    echo "SPECS_DIR: $FEATURE_DIR"
    echo "BRANCH: $CURRENT_BRANCH"
    echo "HAS_GIT: $HAS_GIT"
fi
