#!/bin/sh
# E2E tests against 5 real public repositories.
#
# This is the Phase 1 release gate: proves that 8v check works on real-world
# code, not just synthetic fixtures. Each repo exercises a different stack.
#
# Repos under test:
#   ripgrep     — Rust (BurntSushi/ripgrep)
#   requests    — Python (psf/requests)
#   fzf         — Go (junegunn/fzf)
#   typescript  — TypeScript (microsoft/TypeScript)
#   aspnetcore  — .NET (dotnet/aspnetcore)
#
# For each repo the test verifies:
#   1. Clone succeeds
#   2. 8v check exits with a valid code (0=pass, 1=violations or user error).
#      Exit 2 is reserved for clap parse failures and should never happen here.
#      Any other exit code means a crash or internal error — that is a bug.
#   3. --json output is valid JSON
#   4. At least one result entry exists
#   5. The expected stack is detected
#
# Usage:
#   sh scripts/test-real-repos.sh
#
# Requirements: git, cargo, python3
# Optional:     gtimeout (macOS coreutils) or timeout (Linux) — prevents hangs

set -eu

WORKSPACE_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$WORKSPACE_ROOT"

PASS=0
FAIL=0
CLONE_DIR=""

# ── Helpers ───────────────────────────────────────────────────────────────────

ok() {
    PASS=$((PASS + 1))
    echo "ok: $*"
}

fail() {
    FAIL=$((FAIL + 1))
    echo "FAIL: $*" >&2
}

cleanup() {
    [ -n "$CLONE_DIR" ] && rm -rf "$CLONE_DIR"
}
trap cleanup EXIT

# Cross-platform timeout — gtimeout on macOS (brew install coreutils), timeout on Linux
if command -v gtimeout >/dev/null 2>&1; then
    _TIMEOUT=gtimeout
elif command -v timeout >/dev/null 2>&1; then
    _TIMEOUT=timeout
else
    _TIMEOUT=""
    echo "warning: no timeout command found — tests may hang on large repos" >&2
fi

run_with_timeout() {
    secs="$1"; shift
    if [ -n "$_TIMEOUT" ]; then
        "$_TIMEOUT" "$secs" "$@"
    else
        "$@"
    fi
}

# ── Build 8v ─────────────────────────────────────────────────────────────────

echo "Building 8v..."
cargo build -p o8v --quiet
BIN="$WORKSPACE_ROOT/target/debug/8v"
[ -x "$BIN" ] || { echo "FAIL: binary not found at $BIN" >&2; exit 1; }
ok "binary built: $BIN"

CLONE_DIR=$(mktemp -d)

# ── check_repo ────────────────────────────────────────────────────────────────
#
# check_repo NAME URL EXPECTED_STACK [TIMEOUT_SECS]
#
# Shallow-clones URL, runs 8v check --json, and asserts:
#   - Exit code in {0, 1, 2}   — no crash
#   - Valid JSON output         — renderer worked
#   - results array non-empty   — something was detected and checked
#   - expected stack present    — correct stack detected

check_repo() {
    name="$1"
    url="$2"
    expected_stack="$3"
    timeout_secs="${4:-120}"

    echo ""
    echo "── $name ($expected_stack) ──"

    repo_dir="$CLONE_DIR/$name"

    # Shallow clone — one commit, one branch, no blobs for unneeded history
    echo "  cloning $url..."
    if ! run_with_timeout 90 git clone --depth 1 --single-branch --quiet "$url" "$repo_dir" 2>/dev/null; then
        fail "$name: git clone failed or timed out"
        return
    fi
    ok "$name: cloned"

    # Run check with JSON output written directly to a temp file.
    # Do NOT capture via $() — echo "$var" interprets \n inside JSON strings,
    # which corrupts the content and causes false parse failures.
    json_file=$(mktemp)

    set +e
    run_with_timeout "$timeout_secs" "$BIN" check "$repo_dir" --json > "$json_file" 2>/dev/null
    exit_code=$?
    set -e

    # Valid exit codes: 0 (all passed), 1 (violations or user error).
    # Exit 2 is clap-only and should not appear here.
    # 124 = timeout, anything else = crash / internal error.
    case "$exit_code" in
        0|1) ok "$name: exit $exit_code (valid 8v exit code)" ;;
        2)   rm -f "$json_file"; fail "$name: exit 2 (clap parse failure — unexpected)"; return ;;
        124)   rm -f "$json_file"; fail "$name: timed out after ${timeout_secs}s"; return ;;
        *)     rm -f "$json_file"; fail "$name: unexpected exit code $exit_code — likely a crash"; return ;;
    esac

    # JSON file must be non-empty and parseable
    if [ ! -s "$json_file" ]; then
        rm -f "$json_file"
        fail "$name: --json produced empty output"
        return
    fi

    if ! python3 -c "import json; json.load(open('$json_file'))" 2>/dev/null; then
        rm -f "$json_file"
        fail "$name: --json output is not valid JSON"
        return
    fi
    ok "$name: JSON output valid"

    # results array must be non-empty — at least one project was detected and checked
    result_count=$(python3 -c "
import json
d = json.load(open('$json_file'))
print(len(d.get('results', [])))
" 2>/dev/null || echo "0")

    if [ "$result_count" -eq 0 ]; then
        rm -f "$json_file"
        fail "$name: results array is empty — no projects detected"
        return
    fi
    ok "$name: $result_count project(s) detected"

    # Expected stack must appear in at least one result entry
    stack_found=$(python3 -c "
import json
d = json.load(open('$json_file'))
stacks = [r.get('stack','') for r in d.get('results',[])]
print('yes' if '$expected_stack' in stacks else 'no')
" 2>/dev/null || echo "no")

    if [ "$stack_found" = "yes" ]; then
        ok "$name: stack '$expected_stack' detected"
    else
        # Show what was detected to help diagnose
        detected=$(python3 -c "
import json
d = json.load(open('$json_file'))
stacks = sorted({r.get('stack','') for r in d.get('results',[])})
print(', '.join(stacks) if stacks else 'none')
" 2>/dev/null || echo "unknown")
        fail "$name: expected stack '$expected_stack', got: $detected"
    fi

    rm -f "$json_file"
}

# ── Run against 5 real repos ──────────────────────────────────────────────────
#
# Ordered fastest → slowest to surface failures early.
# Timeouts are generous — these repos have real tool chains to run.

check_repo "ripgrep"    "https://github.com/BurntSushi/ripgrep"   "rust"       120
check_repo "requests"   "https://github.com/psf/requests"         "python"     120
check_repo "fzf"        "https://github.com/junegunn/fzf"         "go"         120
check_repo "typescript" "https://github.com/microsoft/TypeScript" "typescript" 240
check_repo "aspnetcore" "https://github.com/dotnet/aspnetcore"    "dotnet"     300

# ── Summary ───────────────────────────────────────────────────────────────────

echo ""
echo "────────────────────────────────"
echo "Results: $PASS passed, $FAIL failed"
echo ""

if [ "$FAIL" -gt 0 ]; then
    exit 1
fi
