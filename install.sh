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
    echo "  [!] Cargo/Rust toolchain is not detected on your system."
    OS_NAME="$(uname -s 2>/dev/null || echo "Unknown")"

    # Interactive prompt if connected to terminal
    if [ -t 0 ]; then
        printf "  [?] Would you like to install Rust now? [y/N]: "
        read -r reply
        case "$reply" in
            [yY]|[yY][eE][sS])
                echo "==> Installing Rust toolchain..."
                case "$OS_NAME" in
                    OpenBSD)
                        if [ "$(id -u)" = "0" ]; then
                            pkg_add rust
                        elif command -v doas >/dev/null 2>&1; then
                            doas pkg_add rust
                        else
                            echo "Error: Root or doas privileges required to run 'pkg_add rust'." >&2
                            exit 1
                        fi
                        ;;
                    *)
                        if command -v curl >/dev/null 2>&1; then
                            curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
                        elif command -v wget >/dev/null 2>&1; then
                            wget --https-only -qO- https://sh.rustup.rs | sh -s -- -y
                        else
                            echo "Error: Neither curl nor wget was found. Please install curl or wget first." >&2
                            exit 1
                        fi
                        ;;
                esac

                # Source cargo environment immediately for this session
                if [ -f "$HOME/.cargo/env" ]; then
                    # shellcheck disable=SC1091
                    . "$HOME/.cargo/env"
                fi
                export PATH="$HOME/.cargo/bin:$PATH"
                ;;
            *)
                echo "Aborting installation. Rust is required to build BashMenu." >&2
                exit 1
                ;;
        esac
    else
        echo "Error: Cargo is required to compile BashMenu, but is not installed." >&2
        echo "Install Rust via https://rustup.rs (or 'pkg_add rust' on OpenBSD)." >&2
        exit 1
    fi
fi

# Final check for cargo
if ! command -v cargo >/dev/null 2>&1; then
    echo "Error: Cargo is required to compile BashMenu, but was not found in PATH." >&2
    echo "Please ensure '$HOME/.cargo/bin' is in your PATH and rerun ./install.sh" >&2
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
for bin in bashmenu ymlcheck menuedit shortcutedit themeedit bashedit bmpager cheat; do
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
ln -sf "$SCRIPT_DIR/scripts/rich.sh" "$BIN_DIR/rich"
echo "  [+] Created shortcut 'rich' -> $BIN_DIR/rich"

# Initialize user config directory if missing
CONFIG_DIR="$HOME/.config/bashmenu"
mkdir -p "$CONFIG_DIR"
for cfg in bashmenu.yml bashmenu.mnu bashmenu.themes shortcuts.yml cheat.yml; do
    fixture="$SCRIPT_DIR/tests/fixtures/$cfg"
    dest="$CONFIG_DIR/$cfg"
    if [ ! -f "$dest" ] && [ -f "$fixture" ]; then
        cp "$fixture" "$dest"
        echo "  [+] Initialized user config: $dest"
    fi
done

# Symlink assets and scripts into user config directory if present
if [ -d "$SCRIPT_DIR/assets" ]; then
    ln -sfn "$SCRIPT_DIR/assets" "$CONFIG_DIR/assets"
    echo "  [+] Symlinked assets -> $CONFIG_DIR/assets"
fi
if [ -d "$SCRIPT_DIR/scripts" ]; then
    ln -sfn "$SCRIPT_DIR/scripts" "$CONFIG_DIR/scripts"
    echo "  [+] Symlinked scripts -> $CONFIG_DIR/scripts"
fi
if [ -f "$SCRIPT_DIR/bashmenu.md" ]; then
    ln -sfn "$SCRIPT_DIR/bashmenu.md" "$CONFIG_DIR/bashmenu.md"
    echo "  [+] Symlinked bashmenu.md -> $CONFIG_DIR/bashmenu.md"
fi

# Initialize user notebooks directory if missing
if [ -d "$SCRIPT_DIR/notebooks" ]; then
    mkdir -p "$CONFIG_DIR/notebooks"
    for nb in "$SCRIPT_DIR/notebooks"/*.yml; do
        if [ -f "$nb" ]; then
            nb_name="$(basename "$nb")"
            if [ ! -f "$CONFIG_DIR/notebooks/$nb_name" ]; then
                cp "$nb" "$CONFIG_DIR/notebooks/$nb_name"
                echo "  [+] Initialized user notebook: $CONFIG_DIR/notebooks/$nb_name"
            fi
        fi
    done
fi

# Install manual page into man1 directory
MAN_DIR="$HOME/.local/share/man/man1"
if [ -n "${PREFIX:-}" ] && [ -d "$PREFIX/share/man/man1" ]; then
    MAN_DIR="$PREFIX/share/man/man1"
elif [ "$(id -u)" = "0" ]; then
    MAN_DIR="/usr/local/share/man/man1"
fi
mkdir -p "$MAN_DIR"
if [ -f "$SCRIPT_DIR/bashmenu.1" ]; then
    rm -f "$MAN_DIR/bashmenu.1"
    cp -f "$SCRIPT_DIR/bashmenu.1" "$MAN_DIR/bashmenu.1"
    rm -f "$MAN_DIR/bm.1"
    ln -sf "$MAN_DIR/bashmenu.1" "$MAN_DIR/bm.1"
    echo "  [+] Installed manual page -> $MAN_DIR/bashmenu.1 (and alias $MAN_DIR/bm.1)"
    if command -v mandb >/dev/null 2>&1; then
        mandb -q -u 2>/dev/null || mandb -q "$HOME/.local/share/man" 2>/dev/null || true
    fi
fi

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

# Verify MANPATH if user has custom MANPATH set
if [ -n "${MANPATH:-}" ]; then
    case ":$MANPATH:" in
        *":$HOME/.local/share/man:"*|*":$MAN_DIR:"*) ;;
        *)
            echo ""
            echo "  [!] Notice: Custom MANPATH is set and does not include $HOME/.local/share/man."
            echo "      Add '$HOME/.local/share/man' to your MANPATH to view 'man bashmenu'."
            ;;
    esac
fi

echo ""
echo "==================================================================="
echo "            BashMenu-RS Successfully Installed!                    "
echo "==================================================================="
echo "  Commands installed:"
echo "    bashmenu     - Main menu system (shortcut: bm)"
echo "    cheat        - Interactive cheatsheet lookup tool"
echo "    menuedit     - Visual menu tree editor"
echo "    shortcutedit - Visual keyboard shortcuts editor (shortcuts.yml)"
echo "    themeedit    - Visual theme & palette editor"
echo "    bashedit     - Text editor with swap file recovery"
echo "    bmpager      - Standalone native terminal pager with markdown support"
echo "    ymlcheck     - YAML schema and syntax validator"
echo "==================================================================="
