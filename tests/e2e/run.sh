#!/bin/bash
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)

echo "🚀 Starting AgentSync E2E Suite..."

resolve_scenario() {
    local scenario="$1"
    if [[ "$scenario" == */* ]]; then
        printf '%s\n' "$scenario"
        return
    fi

    printf '%s/scenarios/%s\n' "$SCRIPT_DIR" "$scenario"
}

collect_default_scenarios() {
    local scenario
    while IFS= read -r scenario; do
        printf '%s\n' "$scenario"
    done < <(printf '%s\n' "$SCRIPT_DIR"/scenarios/*.sh | sort)
}

SCENARIOS=()

if [ "$#" -gt 0 ]; then
    for scenario in "$@"; do
        SCENARIOS+=("$(resolve_scenario "$scenario")")
    done
elif [ -n "${E2E_SCENARIOS:-}" ]; then
    read -r -a requested_scenarios <<< "${E2E_SCENARIOS}"
    for scenario in "${requested_scenarios[@]}"; do
        SCENARIOS+=("$(resolve_scenario "$scenario")")
    done
else
    while IFS= read -r scenario; do
        SCENARIOS+=("$scenario")
    done < <(collect_default_scenarios)
fi

for scenario in "${SCENARIOS[@]}"; do
    echo
    echo "▶️ Running scenario: $(basename "$scenario")"
    /bin/bash "$scenario"
done

echo
echo "🎉 All E2E scenarios passed successfully!"
