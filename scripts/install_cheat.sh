#!/usr/bin/env bash
# ==============================================================================
# SCRIPT: install_cheat.sh
# DESCRIPTION: Clones a private Github repository of cheatsheets into a local
#              directory. Uses get_api_key.sh to securely fetch the Git PAT
#              from a local network pc.
# USAGE:       install_cheat.sh
# OPTIONS:     None.
# ==============================================================================

# Clones a cheatsheet Git repository into INSTALL_DIR.
# Override CHEAT_REPO_URL, CHEAT_INSTALL_DIR, and CHEAT_TOKEN_FILE via env vars.
SCRIPT_DIR="$(cd -P "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INSTALL_DIR="${CHEAT_INSTALL_DIR:-$HOME/cheat}"
CHEAT_REPO_URL="${CHEAT_REPO_URL:-}"
CHEAT_TOKEN_FILE="${CHEAT_TOKEN_FILE:-gitpat.txt}"

if [ -z "$CHEAT_REPO_URL" ]; then
    echo "Error: Set CHEAT_REPO_URL (e.g. https://github.com/<user>/cheat.git) before running." >&2
    exit 1
fi

mkdir -p "$INSTALL_DIR" &>/dev/null
cd "$INSTALL_DIR" &>/dev/null || exit 1

if [ -n "$(ls -A "$INSTALL_DIR" 2>/dev/null)" ]; then
    echo "Directory is NOT empty, aborting git clone."
    exit 1
else
    echo "Directory is empty, cloning git repository into $INSTALL_DIR..."
    if [ -n "${CHEAT_GIT_USER:-}" ] && [ -x "$SCRIPT_DIR/get_api_key.sh" ]; then
        TOKEN="$("$SCRIPT_DIR/get_api_key.sh" "$CHEAT_TOKEN_FILE")"
        REPO_STRIPPED="${CHEAT_REPO_URL#https://}"
        git clone "https://${CHEAT_GIT_USER}:${TOKEN}@${REPO_STRIPPED}" .
    else
        git clone "$CHEAT_REPO_URL" .
    fi
fi
