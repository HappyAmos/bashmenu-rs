#!/usr/bin/env bash
# ==============================================================================
# SCRIPT: cheat.sh
# DESCRIPTION: Terminal cheatsheet CLI wrapper trampoline. Resolves paths and
#              executes the compiled cheat binary (with fallback to cheat.py).
# USAGE:       cheat.sh [topic] [options]
# ==============================================================================
# Version: 0.1.0
# Author:  HA Bash Menu

export LANG=en_US.UTF-8
export LC_ALL=en_US.UTF-8

# Helper function to exit or return depending on how the script was run
safe_exit() {
    if [[ "${BASH_SOURCE[0]}" != "${0}" ]]; then
        return "$1" # Sourced: return to parent shell safely
    else
        exit "$1"   # Executed: exit subshell
    fi
}

# Canonical symlink resolution to determine true script directory
SOURCE="${BASH_SOURCE[0]}"
while [ -L "$SOURCE" ]; do
  DIR="$(cd -P "$(dirname "$SOURCE")" && pwd)"
  SOURCE="$(readlink "$SOURCE")"
  [[ $SOURCE != /* ]] && SOURCE="$DIR/$SOURCE"
done
SCRIPT_DIR="$(cd -P "$(dirname "$SOURCE")" && pwd)"
REPO_ROOT="$(cd -P "${SCRIPT_DIR}/.." && pwd)"
CANONICAL_SELF="${SCRIPT_DIR}/cheat.sh"

# Prioritize native compiled cheat binary, avoiding self-referential loop
for candidate in \
    "${REPO_ROOT}/target/release/cheat" \
    "${REPO_ROOT}/target/debug/cheat" \
    "$(command -v cheat 2>/dev/null)" \
    "${HOME}/.local/bin/cheat" \
    "/usr/local/bin/cheat"; do
    if [ -n "$candidate" ] && [ -x "$candidate" ]; then
        cand_real="$(readlink -f "$candidate" 2>/dev/null || realpath "$candidate" 2>/dev/null || echo "$candidate")"
        if [ "$cand_real" != "$CANONICAL_SELF" ] && [ -f "$candidate" ]; then
            exec "$candidate" "$@"
        fi
    fi
done

# Fallback to Python if native binary has not yet been compiled
pushd "$SCRIPT_DIR" > /dev/null || safe_exit 1

if [ -f "${SCRIPT_DIR}/.venv/bin/activate" ]; then
    # shellcheck source=/dev/null
    source "${SCRIPT_DIR}/.venv/bin/activate"
    python "${SCRIPT_DIR}/cheat.py" "$@"
    RET=$?
    deactivate
    popd > /dev/null || safe_exit 1
    safe_exit "$RET"
else
    python3 "${SCRIPT_DIR}/cheat.py" "$@"
    RET=$?
    popd > /dev/null || safe_exit 1
    safe_exit "$RET"
fi
