#!/bin/sh
# ==============================================================================
# SCRIPT: uninstall.sh
# DESCRIPTION: Uninstalls BashMenu-RS binaries, shortcuts, and configuration.
# USAGE:       ./uninstall.sh [-y|--yes]
# ==============================================================================

set -e

AUTO_CONFIRM=false
for arg in "$@"; do
    case "$arg" in
        -y|--yes) AUTO_CONFIRM=true ;;
        -h|--help)
            echo "Usage: ./uninstall.sh [-y|--yes]"
            exit 0
            ;;
    esac
done

BIN_DIR="$HOME/.local/bin"
if [ -n "${PREFIX:-}" ] && [ -d "$PREFIX/bin" ]; then
    BIN_DIR="$PREFIX/bin"
elif [ "$(id -u)" = "0" ]; then
    BIN_DIR="/usr/local/bin"
fi

echo "==================================================================="
echo "              Uninstalling BashMenu-RS                             "
echo "==================================================================="

echo "==> Removing binaries from $BIN_DIR..."
for bin in bashmenu bm ymlcheck menuedit themeedit bashedit bmpager cheat rich; do
    target="$BIN_DIR/$bin"
    if [ -f "$target" ] || [ -L "$target" ]; then
        rm -f "$target"
        echo "  [-] Removed $target"
    fi
done

CONFIG_DIR="$HOME/.config/bashmenu"
CACHE_DIR="$HOME/.cache/bashmenu"

remove_configs=false
if [ "$AUTO_CONFIRM" = "true" ]; then
    remove_configs=true
else
    if [ -d "$CONFIG_DIR" ] || [ -d "$CACHE_DIR" ]; then
        printf "==> Remove configuration files and caches in ~/.config/bashmenu? [y/N]: "
        read -r resp
        case "$resp" in
            [yY]|[yY][eE][sS]) remove_configs=true ;;
            *) remove_configs=false ;;
        esac
    fi
fi

if [ "$remove_configs" = "true" ]; then
    [ -d "$CONFIG_DIR" ] && rm -rf "$CONFIG_DIR" && echo "  [-] Removed $CONFIG_DIR"
    [ -d "$CACHE_DIR" ] && rm -rf "$CACHE_DIR" && echo "  [-] Removed $CACHE_DIR"
fi

echo ""
echo "==================================================================="
echo "              BashMenu-RS Successfully Uninstalled                 "
echo "==================================================================="
