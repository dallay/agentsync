#!/usr/bin/env bash
# =============================================================================
# Launcher: builds Docker image and runs manual tests inside a container
# =============================================================================
#
# Usage:
#   ./tests/manual/run_manual_tests.sh          # Run all tests
#   ./tests/manual/run_manual_tests.sh 5        # Run test #5 only
#   ./tests/manual/run_manual_tests.sh shell    # Drop into shell (explore freely)
#
# =============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
IMAGE_NAME="agentsync-manual-tests"
TEST_NUM="${1:-}"

echo "Building Docker image (this caches after the first run)..."
docker build \
    -t "$IMAGE_NAME" \
    -f "$PROJECT_ROOT/tests/manual/Dockerfile.manual" \
    "$PROJECT_ROOT"

echo ""

if [ "$TEST_NUM" = "shell" ]; then
    echo "Dropping into interactive shell..."
    echo "  agentsync is at: /usr/local/bin/agentsync"
    echo "  test script at:  /tests/test_wizard_interactive.sh"
    echo "  workspace:       /workspace (empty, safe to play)"
    echo ""
    docker run --rm -it "$IMAGE_NAME"
elif [ -n "$TEST_NUM" ]; then
    echo "Running test #$TEST_NUM..."
    docker run --rm -it "$IMAGE_NAME" -c "/tests/test_wizard_interactive.sh $TEST_NUM"
else
    echo "Running all manual tests..."
    docker run --rm -it "$IMAGE_NAME" -c "/tests/test_wizard_interactive.sh"
fi
