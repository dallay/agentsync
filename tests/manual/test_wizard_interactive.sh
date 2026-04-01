#!/usr/bin/env bash
# =============================================================================
# AgentSync Manual Test Battery — Wizard & Doctor Interactive Flows
# =============================================================================
#
# Usage:
#   cargo build && ./tests/manual/test_wizard_interactive.sh [test_number]
#
# Run a single test:
#   ./tests/manual/test_wizard_interactive.sh 3
#
# Run all tests sequentially (interactive — you drive each prompt):
#   ./tests/manual/test_wizard_interactive.sh
#
# Prerequisites:
#   - cargo build (debug binary at target/debug/agentsync)
#   - OR cargo build --release (release binary at target/release/agentsync)
#
# Each test creates an isolated temp directory, sets up a scenario, then
# launches agentsync interactively so you can exercise the prompts.
# After each test you'll see EXPECTED BEHAVIOR to verify against.
# =============================================================================

set -euo pipefail

# --- Configuration -----------------------------------------------------------
BINARY="${AGENTSYNC_BIN:-./target/debug/agentsync}"
TMPBASE=$(mktemp -d)
PASS_COUNT=0
FAIL_COUNT=0
SKIP_COUNT=0
TOTAL_TESTS=12

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
DIM='\033[2m'
RESET='\033[0m'

# --- Helpers -----------------------------------------------------------------
banner() {
    echo ""
    echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${RESET}"
    echo -e "${BOLD}  TEST $1: $2${RESET}"
    echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${RESET}"
}

expected() {
    echo ""
    echo -e "${YELLOW}  EXPECTED BEHAVIOR:${RESET}"
    while [ $# -gt 0 ]; do
        echo -e "${DIM}    → $1${RESET}"
        shift
    done
    echo ""
}

setup_dir() {
    local name="$1"
    local dir="$TMPBASE/$name"
    rm -rf "$dir"
    mkdir -p "$dir"
    echo "$dir"
}

prompt_result() {
    echo ""
    echo -e -n "${BOLD}  Did this test pass? [${GREEN}y${RESET}/${RED}n${RESET}/${YELLOW}s${RESET}kip]: ${RESET}"
    read -r answer
    case "$answer" in
        y|Y) PASS_COUNT=$((PASS_COUNT + 1)); echo -e "  ${GREEN}✔ PASS${RESET}" ;;
        s|S) SKIP_COUNT=$((SKIP_COUNT + 1)); echo -e "  ${YELLOW}⊘ SKIP${RESET}" ;;
        *)   FAIL_COUNT=$((FAIL_COUNT + 1)); echo -e "  ${RED}✘ FAIL${RESET}" ;;
    esac
}

should_run() {
    local test_num="$1"
    [ -z "${RUN_TEST:-}" ] || [ "${RUN_TEST}" = "$test_num" ]
}

# --- Pre-flight --------------------------------------------------------------
if [ ! -x "$BINARY" ]; then
    echo -e "${RED}Error: Binary not found at $BINARY${RESET}"
    echo "Run: cargo build"
    exit 1
fi

echo -e "${BOLD}AgentSync Manual Test Battery${RESET}"
echo -e "Binary: ${DIM}$BINARY${RESET}"
echo -e "Temp:   ${DIM}$TMPBASE${RESET}"
echo -e "Tests:  ${DIM}$TOTAL_TESTS${RESET}"
"$BINARY" --version
echo ""

RUN_TEST="${1:-}"
mkdir -p "$TMPBASE"

# =============================================================================
# TEST 1: Init (no wizard) — empty project
# =============================================================================
if should_run 1; then
    banner 1 "Init (no wizard) — empty project"
    DIR=$(setup_dir "t01-init-empty")
    (cd "$DIR" && git init -q)

    expected \
        "Creates .agents/ directory with AGENTS.md and agentsync.toml" \
        "Creates .agents/skills/ and .agents/commands/ directories" \
        "Shows 'Next steps' guidance" \
        "No interactive prompts"

    (cd "$DIR" && "$BINARY" init)
    echo ""
    echo -e "${DIM}  Resulting structure:${RESET}"
    find "$DIR/.agents" -type f | sort | sed "s|$DIR/||"
    prompt_result
fi

# =============================================================================
# TEST 2: Init --wizard — empty project (no agent files to discover)
# =============================================================================
if should_run 2; then
    banner 2 "Init --wizard — empty project (no existing agent files)"
    DIR=$(setup_dir "t02-wizard-empty")
    (cd "$DIR" && git init -q)

    expected \
        "Scans and finds 0 agent files" \
        "Says 'No existing agent files found'" \
        "Falls back to standard init (no prompts)" \
        "Creates .agents/ with default AGENTS.md and agentsync.toml"

    (cd "$DIR" && "$BINARY" init --wizard)
    echo ""
    echo -e "${DIM}  Resulting structure:${RESET}"
    find "$DIR/.agents" -type f | sort | sed "s|$DIR/||"
    prompt_result
fi

# =============================================================================
# TEST 3: Init --wizard — single instruction file (CLAUDE.md)
# =============================================================================
if should_run 3; then
    banner 3 "Init --wizard — single CLAUDE.md to migrate"
    DIR=$(setup_dir "t03-wizard-single-claude")
    (cd "$DIR" && git init -q)
    echo "# My Claude Instructions" > "$DIR/CLAUDE.md"

    expected \
        "Detects CLAUDE.md" \
        "Prompt 1: 'Would you like to migrate?' → answer YES" \
        "Prompt 2: 'Select files to migrate' → CLAUDE.md pre-selected, press Enter" \
        "Creates .agents/AGENTS.md with content from CLAUDE.md" \
        "Prompt: 'Back up original files?' → answer YES" \
        "Moves CLAUDE.md to .agents/backup/CLAUDE.md" \
        "Shows post-migration summary"

    (cd "$DIR" && "$BINARY" init --wizard)
    echo ""
    echo -e "${DIM}  Resulting structure:${RESET}"
    find "$DIR/.agents" -type f 2>/dev/null | sort | sed "s|$DIR/||"
    echo -e "${DIM}  CLAUDE.md still in root?${RESET} $([ -f "$DIR/CLAUDE.md" ] && echo "YES (not moved)" || echo "NO (moved to backup)")"
    prompt_result
fi

# =============================================================================
# TEST 4: Init --wizard — multiple instruction files (merge)
# =============================================================================
if should_run 4; then
    banner 4 "Init --wizard — multiple instruction files (Claude + Copilot + .cursorrules)"
    DIR=$(setup_dir "t04-wizard-multi-merge")
    (cd "$DIR" && git init -q)
    echo "# Claude rules" > "$DIR/CLAUDE.md"
    mkdir -p "$DIR/.github"
    echo "# Copilot rules" > "$DIR/.github/copilot-instructions.md"
    echo "# Windsurf rules" > "$DIR/.windsurfrules"

    expected \
        "Detects 3 instruction files" \
        "Prompt: 'Would you like to migrate?' → YES" \
        "Prompt: 'Select files' → keep all selected, Enter" \
        "Creates .agents/AGENTS.md with merged content (sections separated by ---)" \
        "Each section has '# Instructions from <path>' header" \
        "Prompt: 'Back up?' → YES" \
        "All 3 original files moved to .agents/backup/"

    (cd "$DIR" && "$BINARY" init --wizard)
    echo ""
    echo -e "${DIM}  AGENTS.md content preview:${RESET}"
    head -20 "$DIR/.agents/AGENTS.md" 2>/dev/null || echo "  (not created)"
    prompt_result
fi

# =============================================================================
# TEST 5: Init --wizard — with skills directory
# =============================================================================
if should_run 5; then
    banner 5 "Init --wizard — with Claude skills directory"
    DIR=$(setup_dir "t05-wizard-skills")
    (cd "$DIR" && git init -q)
    echo "# Instructions" > "$DIR/CLAUDE.md"
    mkdir -p "$DIR/.claude/skills/my-skill"
    echo "# Skill content" > "$DIR/.claude/skills/my-skill/SKILL.md"

    expected \
        "Detects CLAUDE.md + .claude/skills/" \
        "After file selection, prompts: 'How should this skills target sync?'" \
        "Options: 'symlink' vs 'symlink-contents' → pick either" \
        "Skills copied to .agents/skills/my-skill/" \
        "Post-init validation: no mode mismatch warnings"

    (cd "$DIR" && "$BINARY" init --wizard)
    echo ""
    echo -e "${DIM}  Skills dir:${RESET}"
    find "$DIR/.agents/skills" -type f 2>/dev/null | sort | sed "s|$DIR/||" || echo "  (empty)"
    prompt_result
fi

# =============================================================================
# TEST 6: Init --wizard — decline migration
# =============================================================================
if should_run 6; then
    banner 6 "Init --wizard — decline migration"
    DIR=$(setup_dir "t06-wizard-decline")
    (cd "$DIR" && git init -q)
    echo "# Claude" > "$DIR/CLAUDE.md"

    expected \
        "Detects CLAUDE.md" \
        "Prompt: 'Would you like to migrate?' → answer NO" \
        "Falls back to standard init" \
        "CLAUDE.md untouched in project root" \
        ".agents/AGENTS.md has default template content (not migrated)"

    (cd "$DIR" && "$BINARY" init --wizard)
    echo ""
    echo -e "${DIM}  CLAUDE.md still in root?${RESET} $([ -f "$DIR/CLAUDE.md" ] && echo "YES (untouched)" || echo "NO")"
    echo -e "${DIM}  AGENTS.md first line:${RESET} $(head -1 "$DIR/.agents/AGENTS.md" 2>/dev/null)"
    prompt_result
fi

# =============================================================================
# TEST 7: Init --wizard — re-init (config already exists, no --force)
# =============================================================================
if should_run 7; then
    banner 7 "Init --wizard — re-init without --force"
    DIR=$(setup_dir "t07-wizard-reinit")
    (cd "$DIR" && git init -q)
    echo "# Claude" > "$DIR/CLAUDE.md"

    # First init
    (cd "$DIR" && "$BINARY" init)

    echo -e "\n${YELLOW}  Now running wizard again (config already exists)...${RESET}\n"

    expected \
        "Detects CLAUDE.md" \
        "During config generation: 'Config already exists (use --force to overwrite)'" \
        "AGENTS.md already exists warning" \
        "Backup NOT offered (because AGENTS.md was preserved, not written)"

    (cd "$DIR" && "$BINARY" init --wizard)
    prompt_result
fi

# =============================================================================
# TEST 8: Init --wizard --force — overwrites existing config
# =============================================================================
if should_run 8; then
    banner 8 "Init --wizard --force — overwrites existing"
    DIR=$(setup_dir "t08-wizard-force")
    (cd "$DIR" && git init -q)
    echo "# Claude original" > "$DIR/CLAUDE.md"

    # First init
    (cd "$DIR" && "$BINARY" init)
    echo "MODIFIED" >> "$DIR/.agents/agentsync.toml"

    echo -e "\n${YELLOW}  Now running wizard with --force...${RESET}\n"

    expected \
        "Overwrites agentsync.toml (MODIFIED line should be gone)" \
        "Overwrites AGENTS.md with migrated content" \
        "Offers backup prompt"

    (cd "$DIR" && "$BINARY" init --wizard --force)
    echo ""
    echo -e "${DIM}  Config has MODIFIED?${RESET} $(grep -c MODIFIED "$DIR/.agents/agentsync.toml" 2>/dev/null && echo "YES (not overwritten!)" || echo "NO (correctly overwritten)")"
    prompt_result
fi

# =============================================================================
# TEST 9: Doctor — clean project (no issues)
# =============================================================================
if should_run 9; then
    banner 9 "Doctor — clean project after apply"
    DIR=$(setup_dir "t09-doctor-clean")
    (cd "$DIR" && git init -q)
    (cd "$DIR" && "$BINARY" init)
    (cd "$DIR" && "$BINARY" apply)

    expected \
        "Doctor reports 0 issues" \
        "All symlinks verified" \
        "Gitignore section present (if enabled)"

    (cd "$DIR" && "$BINARY" doctor)
    prompt_result
fi

# =============================================================================
# TEST 10: Doctor — missing gitignore markers
# =============================================================================
if should_run 10; then
    banner 10 "Doctor — gitignore marker mismatch (only start marker)"
    DIR=$(setup_dir "t10-doctor-markers")
    (cd "$DIR" && git init -q)
    (cd "$DIR" && "$BINARY" init)
    (cd "$DIR" && "$BINARY" apply)

    # Corrupt gitignore: remove end marker only
    if [ -f "$DIR/.gitignore" ]; then
        sed -i.bak '/# END AI Agent Symlinks/d' "$DIR/.gitignore"
        rm -f "$DIR/.gitignore.bak"
        echo -e "${DIM}  Removed end marker from .gitignore${RESET}"
    fi

    expected \
        "Doctor warns about gitignore managed section (marker mismatch)" \
        "Shows warning with yellow icon"

    (cd "$DIR" && "$BINARY" doctor)
    prompt_result
fi

# =============================================================================
# TEST 11: Doctor — missing entries in managed section
# =============================================================================
if should_run 11; then
    banner 11 "Doctor — stale/missing entries in gitignore managed section"
    DIR=$(setup_dir "t11-doctor-entries")
    (cd "$DIR" && git init -q)
    (cd "$DIR" && "$BINARY" init)
    (cd "$DIR" && "$BINARY" apply)

    # Add a fake entry inside the managed section (before end marker)
    if [ -f "$DIR/.gitignore" ]; then
        TMPFILE=$(mktemp)
        awk '/# END AI Agent Symlinks/ { print "FAKE_STALE_ENTRY.md" } { print }' "$DIR/.gitignore" > "$TMPFILE"
        mv "$TMPFILE" "$DIR/.gitignore"
        echo -e "${DIM}  Added fake entry to managed section${RESET}"
    fi

    expected \
        "Doctor warns about extra entries (FAKE_STALE_ENTRY.md)" \
        "Lists the extra entry"

    (cd "$DIR" && "$BINARY" doctor)
    prompt_result
fi

# =============================================================================
# TEST 12: Full roundtrip — wizard → apply → doctor → clean → doctor
# =============================================================================
if should_run 12; then
    banner 12 "Full roundtrip: wizard → apply → doctor → clean → doctor"
    DIR=$(setup_dir "t12-roundtrip")
    (cd "$DIR" && git init -q)
    echo "# My project instructions" > "$DIR/CLAUDE.md"
    mkdir -p "$DIR/.github"
    echo "# Copilot instructions" > "$DIR/.github/copilot-instructions.md"

    echo -e "${YELLOW}  Step 1: wizard init${RESET}"
    expected \
        "Migrate both files, accept defaults"
    (cd "$DIR" && "$BINARY" init --wizard)

    echo -e "\n${YELLOW}  Step 2: apply${RESET}"
    expected \
        "Creates symlinks, updates .gitignore"
    (cd "$DIR" && "$BINARY" apply)

    echo -e "\n${YELLOW}  Step 3: doctor (should be clean)${RESET}"
    expected \
        "0 issues"
    (cd "$DIR" && "$BINARY" doctor)

    echo -e "\n${YELLOW}  Step 4: status${RESET}"
    expected \
        "Shows all synced targets"
    (cd "$DIR" && "$BINARY" status)

    echo -e "\n${YELLOW}  Step 5: clean${RESET}"
    expected \
        "Removes all symlinks"
    (cd "$DIR" && "$BINARY" clean)

    echo -e "\n${YELLOW}  Step 6: doctor after clean${RESET}"
    expected \
        "Reports missing symlinks"
    (cd "$DIR" && "$BINARY" doctor)

    prompt_result
fi

# =============================================================================
# Summary
# =============================================================================
echo ""
echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${RESET}"
echo -e "${BOLD}  RESULTS${RESET}"
echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${RESET}"
echo -e "  ${GREEN}Passed:  $PASS_COUNT${RESET}"
echo -e "  ${RED}Failed:  $FAIL_COUNT${RESET}"
echo -e "  ${YELLOW}Skipped: $SKIP_COUNT${RESET}"
echo ""

# Cleanup prompt
echo -e -n "${DIM}  Clean up temp directories? [y/N]: ${RESET}"
read -r cleanup
if [ "$cleanup" = "y" ] || [ "$cleanup" = "Y" ]; then
    rm -rf "$TMPBASE"
    echo -e "  ${GREEN}Cleaned up $TMPBASE${RESET}"
fi

# Exit code
[ "$FAIL_COUNT" -eq 0 ]