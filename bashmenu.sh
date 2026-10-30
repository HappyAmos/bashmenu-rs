#!/bin/sh
# shellcheck shell=bash
# ==============================================================================
# SCRIPT: bashmenu.sh
# DESCRIPTION: Primary BashMenu launcher trampoline for the compiled Rust engine.
#              Automatically detects release/debug binaries, builds on-demand
#              if missing, and manages command shortcuts.
# USAGE:       bashmenu.sh [options]
# ==============================================================================
# Author:  HA Bash Menu (Rust Port)

# --------------------------------------------------------------------------
# POSIX /bin/sh Trampoline: Auto-elevate to Bash if invoked with sh
# --------------------------------------------------------------------------
if [ -z "$BASH_VERSION" ]; then
    if command -v bash >/dev/null 2>&1; then
        exec bash "$0" "$@"
    else
        echo "Error: 'bash' is required to launch BashMenu, but is not installed." >&2
        exit 1
    fi
fi

set -e

# Canonical script directory resolution (resolves symlinks)
SOURCE="${BASH_SOURCE[0]}"
while [ -L "$SOURCE" ]; do
    DIR="$(cd -P "$(dirname "$SOURCE")" && pwd)"
    SOURCE="$(readlink "$SOURCE")"
    case "$SOURCE" in
        /*) ;;
        *) SOURCE="$DIR/$SOURCE" ;;
    esac
done
SCRIPT_DIR="$(cd -P "$(dirname "$SOURCE")" && pwd)"

# UTF-8 locale handling
if command -v locale >/dev/null 2>&1; then
    if locale -a 2>/dev/null | grep -qiE "^en_US\.(utf-?8)$"; then
        export LANG=en_US.UTF-8
        export LC_ALL=en_US.UTF-8
    elif locale -a 2>/dev/null | grep -qiE "^c\.(utf-?8)$"; then
        export LANG=C.UTF-8
        export LC_ALL=C.UTF-8
    else
        export LANG="${LANG:-C.UTF-8}"
        export LC_ALL="${LC_ALL:-C.UTF-8}"
    fi
fi

# Locate compiled bashmenu binary in order of preference
locate_binary() {
    if [ -x "$SCRIPT_DIR/target/release/bashmenu" ]; then
        echo "$SCRIPT_DIR/target/release/bashmenu"
    elif [ -x "$HOME/.local/bin/bashmenu" ]; then
        echo "$HOME/.local/bin/bashmenu"
    elif [ -x "$SCRIPT_DIR/target/debug/bashmenu" ]; then
        echo "$SCRIPT_DIR/target/debug/bashmenu"
    elif command -v bashmenu >/dev/null 2>&1; then
        command -v bashmenu
    else
        echo ""
    fi
}

# Parse top-level management flags
case "${1:-}" in
    --build|--build-release)
        echo "==> Building BashMenu release workspace..."
        if ! command -v cargo >/dev/null 2>&1; then
            echo "Error: Cargo is not installed. Please install Rust toolchain: https://rustup.rs" >&2
            exit 1
        fi
        cd "$SCRIPT_DIR" && cargo build --release --workspace
        echo "==> Release build complete: $SCRIPT_DIR/target/release/bashmenu"
        exit 0
        ;;
    --install)
        shift
        exec "$SCRIPT_DIR/install.sh" "$@"
        ;;
    --uninstall)
        shift
        exec "$SCRIPT_DIR/uninstall.sh" "$@"
        ;;
    --install-bm)
        BIN_DIR="$HOME/.local/bin"
        [ "$(id -u)" = "0" ] && BIN_DIR="/usr/local/bin"
        mkdir -p "$BIN_DIR"
        ln -sf "$SCRIPT_DIR/bashmenu.sh" "$BIN_DIR/bm"
        echo "  [+] Created 'bm' shortcut -> $BIN_DIR/bm"
        exit 0
        ;;
    -h|--help)
        echo "BashMenu - Modern High-Performance Terminal Menu System (Rust)"
        echo ""
        echo "Usage: ./bashmenu.sh [OPTIONS] [-- [BASHMENU_OPTIONS]]"
        echo ""
        echo "Launcher Options:"
        echo "  --build           Build release binaries with Cargo"
        echo "  --install         Run user environment installer"
        echo "  --uninstall       Run uninstaller"
        echo "  --install-bm      Install 'bm' shortcut into ~/.local/bin"
        echo "  -h, --help        Show this launcher help"
        echo ""
        echo "Direct BashMenu Options:"
        echo "  -m, --menu <FILE>   Path to menu definition (default: bashmenu.mnu)"
        echo "  -c, --config <FILE> Path to config file (default: bashmenu.yml)"
        echo "  -t, --theme <NAME>  Override active theme"
        exit 0
        ;;
esac

BASHMENU_BIN="$(locate_binary)"

# Export BASHMENU_DIR so child processes and macro engines know the install root
export BASHMENU_DIR="$SCRIPT_DIR"

# If binary is missing, attempt to compile on-demand
if [ -z "$BASHMENU_BIN" ] || [ ! -x "$BASHMENU_BIN" ]; then
    echo "==> BashMenu binary not found. Compiling release binary with Cargo..."
    if command -v cargo >/dev/null 2>&1; then
        (cd "$SCRIPT_DIR" && cargo build --release -p bashmenu-cli)
        BASHMENU_BIN="$(locate_binary)"
    else
        echo "Error: BashMenu binary not found and 'cargo' is not installed." >&2
        echo "Please install Rust and Cargo to compile: https://rustup.rs" >&2
        exit 1
    fi
fi

# Ensure default menu and config fixtures exist in current working directory if missing
if [ ! -f "bashmenu.mnu" ]; then
    if [ -f "$SCRIPT_DIR/bashmenu.mnu" ]; then
        cp "$SCRIPT_DIR/bashmenu.mnu" ./bashmenu.mnu 2>/dev/null || true
    elif [ -f "$SCRIPT_DIR/tests/fixtures/bashmenu.mnu" ]; then
        cp "$SCRIPT_DIR/tests/fixtures/bashmenu.mnu" ./bashmenu.mnu 2>/dev/null || true
    fi
fi
if [ ! -f "bashmenu.yml" ]; then
    if [ -f "$SCRIPT_DIR/bashmenu.yml" ]; then
        cp "$SCRIPT_DIR/bashmenu.yml" ./bashmenu.yml 2>/dev/null || true
    elif [ -f "$SCRIPT_DIR/tests/fixtures/bashmenu.yml" ]; then
        cp "$SCRIPT_DIR/tests/fixtures/bashmenu.yml" ./bashmenu.yml 2>/dev/null || true
    fi
fi
if [ ! -f "bashmenu.themes" ]; then
    if [ -f "$SCRIPT_DIR/bashmenu.themes" ]; then
        cp "$SCRIPT_DIR/bashmenu.themes" ./bashmenu.themes 2>/dev/null || true
    elif [ -f "$SCRIPT_DIR/tests/fixtures/bashmenu.themes" ]; then
        cp "$SCRIPT_DIR/tests/fixtures/bashmenu.themes" ./bashmenu.themes 2>/dev/null || true
    fi
fi

# Execute compiled Rust binary
exec "$BASHMENU_BIN" "$@"
