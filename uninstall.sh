#!/bin/sh
# ==============================================================================
# SCRIPT: uninstall.sh
# DESCRIPTION: Uninstalls BashMenu-RS binaries, shortcuts, manual pages,
#              and configuration.
# USAGE:       ./uninstall.sh [-y|--yes] [--keep-settings|--keep-config]
# ==============================================================================

set -e

AUTO_CONFIRM=false
KEEP_SETTINGS=""

for arg in "$@"; do
    case "$arg" in
        -y|--yes) AUTO_CONFIRM=true ;;
        -k|--keep-settings|--keep-config) KEEP_SETTINGS="true" ;;
        --remove-settings|--remove-config) KEEP_SETTINGS="false" ;;
        -h|--help)
            echo "Usage: ./uninstall.sh [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  -y, --yes            Non-interactive mode (defaults to removing configs unless -k is given)"
            echo "  -k, --keep-settings  Preserve ~/.config/bashmenu settings and notebooks"
            echo "  --remove-settings    Remove ~/.config/bashmenu settings and notebooks"
            echo "  -h, --help           Show this help message"
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

MAN_DIR="$HOME/.local/share/man/man1"
if [ -n "${PREFIX:-}" ] && [ -d "$PREFIX/share/man/man1" ]; then
    MAN_DIR="$PREFIX/share/man/man1"
elif [ "$(id -u)" = "0" ]; then
    MAN_DIR="/usr/local/share/man/man1"
fi

echo "==================================================================="
echo "              Uninstalling BashMenu-RS                             "
echo "==================================================================="

echo "==> Removing binaries from $BIN_DIR..."
for bin in bashmenu bm ymlcheck menuedit shortcutedit themeedit bashedit bmpager cheat rich; do
    target="$BIN_DIR/$bin"
    if [ -f "$target" ] || [ -L "$target" ]; then
        rm -f "$target"
        echo "  [-] Removed $target"
    fi
done

echo "==> Removing manual page..."
for manpage in "$MAN_DIR/bashmenu.1" "$MAN_DIR/bm.1"; do
    if [ -f "$manpage" ] || [ -L "$manpage" ]; then
        rm -f "$manpage"
        echo "  [-] Removed $manpage"
    fi
done

CONFIG_DIR="$HOME/.config/bashmenu"
CACHE_DIR="$HOME/.cache/bashmenu"

remove_configs=false
if [ "$KEEP_SETTINGS" = "true" ]; then
    remove_configs=false
    echo "  [+] Preserving settings and notebooks in $CONFIG_DIR"
elif [ "$KEEP_SETTINGS" = "false" ]; then
    remove_configs=true
elif [ "$AUTO_CONFIRM" = "true" ]; then
    remove_configs=true
else
    if [ -d "$CONFIG_DIR" ] || [ -d "$CACHE_DIR" ]; then
        printf "==> Keep your settings and notebooks in ~/.config/bashmenu? [Y/n]: "
        read -r resp
        case "$resp" in
            [nN]|[nN][oO]) remove_configs=true ;;
            *) remove_configs=false ;;
        esac
    fi
fi

if [ "$remove_configs" = "true" ]; then
    [ -d "$CONFIG_DIR" ] && rm -rf "$CONFIG_DIR" && echo "  [-] Removed $CONFIG_DIR"
    [ -d "$CACHE_DIR" ] && rm -rf "$CACHE_DIR" && echo "  [-] Removed $CACHE_DIR"
else
    echo "  [+] Settings and notebooks kept at $CONFIG_DIR"
fi

echo ""
echo "==================================================================="
echo "              BashMenu-RS Successfully Uninstalled                 "
echo "==================================================================="
