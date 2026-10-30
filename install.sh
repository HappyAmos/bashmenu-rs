#!/bin/sh
# ==============================================================================
# SCRIPT: install.sh
# DESCRIPTION: Builds release binaries and installs BashMenu-RS system tools
#              into $HOME/.local/bin.
# USAGE:       ./install.sh
# ==============================================================================

set -e

# Canonical script directory
SCRIPT_DIR="$(cd -P "$(dirname "$0")" 2>/dev/null && pwd)"

echo "==================================================================="
echo "              Installing BashMenu-RS System                        "
echo "==================================================================="

# Verify Cargo toolchain
if ! command -v cargo >/dev/null 2>&1; then
    echo "Error: Cargo is required to compile BashMenu, but is not installed." >&2
    echo "Install Rust and Cargo via rustup: https://rustup.rs" >&2
    exit 1
fi

# Determine target binary directory
BIN_DIR="$HOME/.local/bin"
if [ -n "${PREFIX:-}" ] && [ -d "$PREFIX/bin" ]; then
    BIN_DIR="$PREFIX/bin"
elif [ "$(id -u)" = "0" ]; then
    BIN_DIR="/usr/local/bin"
fi
mkdir -p "$BIN_DIR"

echo "==> Building all release binaries (workspace)..."
(cd "$SCRIPT_DIR" && cargo build --release --workspace)

echo "==> Installing binaries to $BIN_DIR..."
for bin in bashmenu ymlcheck menuedit themeedit bashedit; do
    src="$SCRIPT_DIR/target/release/$bin"
    if [ -f "$src" ]; then
        rm -f "$BIN_DIR/$bin"
        cp -f "$src" "$BIN_DIR/$bin"
        chmod +x "$BIN_DIR/$bin"
        echo "  [+] Installed $bin -> $BIN_DIR/$bin"
    else
        echo "  [!] Warning: Binary $bin not found in target/release" >&2
    fi
done

# Install 'bm' convenience shortcut
ln -sf "$BIN_DIR/bashmenu" "$BIN_DIR/bm"
echo "  [+] Created shortcut 'bm' -> $BIN_DIR/bm"

# Install companion tool shortcuts
ln -sf "$SCRIPT_DIR/scripts/pager.sh" "$BIN_DIR/bmpager"
echo "  [+] Created shortcut 'bmpager' -> $BIN_DIR/bmpager"
ln -sf "$SCRIPT_DIR/scripts/cheat.sh" "$BIN_DIR/cheat"
echo "  [+] Created shortcut 'cheat' -> $BIN_DIR/cheat"
ln -sf "$SCRIPT_DIR/scripts/rich.sh" "$BIN_DIR/rich"
echo "  [+] Created shortcut 'rich' -> $BIN_DIR/rich"

# Initialize user config directory if missing
CONFIG_DIR="$HOME/.config/bashmenu"
mkdir -p "$CONFIG_DIR"
for cfg in bashmenu.yml bashmenu.mnu bashmenu.themes; do
    fixture="$SCRIPT_DIR/tests/fixtures/$cfg"
    dest="$CONFIG_DIR/$cfg"
    if [ ! -f "$dest" ] && [ -f "$fixture" ]; then
        cp "$fixture" "$dest"
        echo "  [+] Initialized user config: $dest"
    fi
done

# Verify PATH
case ":$PATH:" in
    *":$BIN_DIR:"*) ;;
    *)
        echo ""
        echo "  [!] Notice: $BIN_DIR is not currently in your PATH."
        for rc in "$HOME/.bashrc" "$HOME/.zshrc" "$HOME/.profile"; do
            if [ -f "$rc" ] && ! grep -q "$BIN_DIR" "$rc"; then
                echo "export PATH=\"$BIN_DIR:\$PATH\"" >> "$rc"
                echo "  [+] Added $BIN_DIR to $rc"
            fi
        done
        echo "      Run 'export PATH=\"$BIN_DIR:\$PATH\"' or restart your terminal."
        ;;
esac

echo ""
echo "==================================================================="
echo "            BashMenu-RS Successfully Installed!                    "
echo "==================================================================="
echo "  Commands installed:"
echo "    bashmenu  - Main menu system (shortcut: bm)"
echo "    menuedit  - Visual menu tree editor"
echo "    themeedit - Visual theme & palette editor"
echo "    bashedit  - Text editor with swap file recovery"
echo "    ymlcheck  - YAML schema and syntax validator"
echo "==================================================================="
