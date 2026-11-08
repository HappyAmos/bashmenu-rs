#!/usr/bin/env bash
# ==============================================================================
# SCRIPT: install_basics.sh
# DESCRIPTION: Cross-platform basic tools installer for Termux, macOS, and
#              various Linux distributions (Debian/Ubuntu, Fedora/RHEL, Arch,
#              openSUSE, Alpine). Installs each app independently so failures do
#              not block others. Invokes custom fallback installer scripts if
#              available and generates a final report with status and solutions.
# USAGE:       install_basics.sh
# OPTIONS:     None.
# ==============================================================================

SCRIPT_DIR="$(cd -P "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

PACKAGES=(vim mc htop glow gum curl wget git shellcheck shfmt jq yq sc cmatrix)

IS_TERMUX=false
IS_MAC=false
if [ -n "$TERMUX_VERSION" ] || [[ "$PREFIX" == *"/com.termux/"* ]]; then
    IS_TERMUX=true
elif [ "$(uname)" = "Darwin" ]; then
    IS_MAC=true
fi

# Detect available package manager safely
PKG_MANAGER=""
if [ "$IS_TERMUX" = true ] && command -v pkg &> /dev/null; then
    PKG_MANAGER="pkg"
elif command -v apt-get &> /dev/null; then
    PKG_MANAGER="apt"
elif command -v dnf &> /dev/null; then
    PKG_MANAGER="dnf"
elif command -v yum &> /dev/null; then
    PKG_MANAGER="yum"
elif command -v pacman &> /dev/null; then
    PKG_MANAGER="pacman"
elif command -v zypper &> /dev/null; then
    PKG_MANAGER="zypper"
elif command -v apk &> /dev/null; then
    PKG_MANAGER="apk"
elif command -v brew &> /dev/null; then
    PKG_MANAGER="brew"
else
    echo "Error: Unsupported package manager."
    exit 1
fi

# Function to execute commands with root privileges if necessary
run_as_root() {
    if [ "$IS_TERMUX" = true ] || [ "$IS_MAC" = true ]; then
        "$@"
    else
        if [ "$(id -u)" = 0 ]; then
            "$@"
        elif command -v sudo &>/dev/null; then
            sudo "$@"
        else
            "$@"
        fi
    fi
}

detect_arch() {
    local raw_m
    raw_m="$(uname -m 2>/dev/null || echo "unknown")"
    case "$raw_m" in
        x86_64|amd64)                 echo "amd64" ;;
        aarch64|arm64|armv8*)         echo "arm64" ;;
        armv7*|armv6*|armhf|arm)      echo "arm" ;;
        i386|i686)                    echo "386" ;;
        *)                            echo "$raw_m" ;;
    esac
}

get_bin_dir() {
    if [ "$IS_TERMUX" = true ] && [ -n "$PREFIX" ] && [ -d "$PREFIX/bin" ]; then
        echo "$PREFIX/bin"
    elif [ "$(id -u)" = "0" ] || [ -w "/usr/local/bin" ]; then
        echo "/usr/local/bin"
    else
        mkdir -p "$HOME/.local/bin"
        echo "$HOME/.local/bin"
    fi
}

ensure_sc_symlink() {
    if ! command -v sc &>/dev/null && command -v sc-im &>/dev/null; then
        local target_bin
        target_bin="$(get_bin_dir)"
        local sc_im_bin
        sc_im_bin="$(command -v sc-im)"
        if [ -w "$target_bin" ]; then
            ln -sf "$sc_im_bin" "$target_bin/sc" 2>/dev/null || true
        elif [ "$(id -u)" = 0 ]; then
            ln -sf "$sc_im_bin" "$target_bin/sc" 2>/dev/null || true
        elif command -v sudo &>/dev/null; then
            run_as_root ln -sf "$sc_im_bin" "$target_bin/sc" 2>/dev/null || true
        fi
    fi
}

is_pkg_installed() {
    local pkg="$1"
    if command -v "$pkg" &>/dev/null; then
        return 0
    fi
    if [ "$pkg" = "sc" ] && command -v sc-im &>/dev/null; then
        ensure_sc_symlink
        return 0
    fi
    return 1
}

# Function to display tailored solutions for failed package installations
get_solution_for_pkg() {
    local pkg="$1"
    local arch
    arch="$(detect_arch)"
    local target_bin
    target_bin="$(get_bin_dir)"
    case "$pkg" in
        glow)
            echo "  - Issue: 'glow' is not in default repositories for many Linux distributions."
            echo "  - Solutions:"
            echo "      1. Run custom script: scripts/install_glow.sh"
            echo "      2. Use built-in markdown viewer: scripts/rich.sh <file> | scripts/pager.sh"
            echo "      3. Install using Homebrew: brew install glow"
            ;;
        gum)
            echo "  - Issue: 'gum' is missing from default package repositories."
            echo "  - Solutions:"
            echo "      1. Add Charm APT repository (https://repo.charm.sh)"
            echo "      2. Install using Go: go install github.com/charmbracelet/gum@latest"
            echo "      3. Install using Homebrew: brew install gum"
            ;;
        shfmt)
            echo "  - Issue: 'shfmt' may not be present in distribution repositories."
            echo "  - Solutions:"
            echo "      1. Install using Go: go install mvdan.cc/sh/v3/cmd/shfmt@latest"
            echo "      2. Download pre-built binary from https://github.com/mvdan/sh/releases"
            echo "      3. Install using Homebrew: brew install shfmt"
            ;;
        yq)
            echo "  - Issue: 'yq' failed to install via package manager."
            echo "  - Solutions:"
            echo "      1. Download binary: wget https://github.com/mikefarah/yq/releases/latest/download/yq_linux_${arch} -O ${target_bin}/yq && chmod +x ${target_bin}/yq"
            echo "      2. Install via Python pip: pip install yq"
            echo "      3. Install using Homebrew: brew install yq"
            ;;
        shellcheck)
            echo "  - Issue: 'shellcheck' failed to install via package manager."
            echo "  - Solutions:"
            echo "      1. Ensure 'universe' (Ubuntu) or 'epel' (RHEL/Fedora) repository is enabled."
            echo "      2. Install using Haskell cabal: cabal install shellcheck"
            echo "      3. Download binary from https://github.com/koalaman/shellcheck/releases"
            ;;
        sc)
            echo "  - Issue: 'sc' (Spreadsheet Calculator) is often packaged as 'sc-im' or requires extra repositories."
            echo "  - Solutions:"
            echo "      1. Debian/Ubuntu: sudo add-apt-repository -y universe && sudo apt-get update && sudo apt-get install -y sc (or sc-im)"
            echo "      2. macOS: brew install sc-im"
            echo "      3. Termux: pkg install sc-im"
            echo "      4. Fedora/RHEL: sudo dnf install epel-release && sudo dnf install sc-im"
            echo "      5. Arch Linux: sudo pacman -S sc-im (or yay -S sc)"
            echo "      6. Alpine Linux: enable community repo in /etc/apk/repositories && apk add sc-im"
            ;;
        cmatrix)
            echo "  - Issue: 'cmatrix' failed to install via package manager '$PKG_MANAGER'."
            echo "  - Solutions:"
            echo "      1. Debian/Ubuntu: sudo add-apt-repository -y universe && sudo apt-get update && sudo apt-get install -y cmatrix"
            echo "      2. RHEL/CentOS/Rocky/AlmaLinux: sudo dnf install epel-release && sudo dnf install cmatrix"
            echo "      3. Fedora: sudo dnf install cmatrix"
            echo "      4. Alpine Linux: enable community repo in /etc/apk/repositories && apk add cmatrix"
            echo "      5. macOS: brew install cmatrix"
            echo "      6. Termux: pkg install cmatrix"
            echo "      7. Arch Linux: sudo pacman -S cmatrix"
            ;;
        *)
            echo "  - Issue: '$pkg' could not be installed using package manager '$PKG_MANAGER'."
            echo "  - Solutions:"
            echo "      1. Check if package name differs on your OS."
            echo "      2. Search official project repo/website for manual installation instructions."
            ;;
    esac
}

echo "Installing basic packages: ${PACKAGES[*]}"
echo "Detected package manager: $PKG_MANAGER"
echo "Updating package index..."

case "$PKG_MANAGER" in
    apt)
        run_as_root apt-get update -qq || true
        ;;
    pkg)
        pkg update -y || true
        ;;
    pacman)
        run_as_root pacman -Sy --noconfirm &>/dev/null || true
        ;;
    apk)
        run_as_root apk update &>/dev/null || true
        ;;
    zypper)
        run_as_root zypper refresh &>/dev/null || true
        ;;
esac

INSTALLED=()
FAILED=()

for pkg in "${PACKAGES[@]}"; do
    echo ""
    echo "Processing package: $pkg"

    if is_pkg_installed "$pkg"; then
        echo "  [✓] $pkg is already installed."
        INSTALLED+=("$pkg")
        continue
    fi

    echo "  [→] Attempting to install $pkg via $PKG_MANAGER..."
    INSTALL_SUCCESS=false

    case "$PKG_MANAGER" in
        pkg)
            if [ "$pkg" = "sc" ]; then
                if pkg install -y sc 2>/dev/null || pkg install -y sc-im; then
                    ensure_sc_symlink
                    INSTALL_SUCCESS=true
                fi
            else
                if pkg install -y "$pkg"; then
                    INSTALL_SUCCESS=true
                fi
            fi
            ;;
        apt)
            if [ "$pkg" = "sc" ]; then
                if run_as_root apt-get install -y sc; then
                    INSTALL_SUCCESS=true
                else
                    if command -v add-apt-repository &>/dev/null; then
                        run_as_root add-apt-repository -y universe 2>/dev/null || true
                        run_as_root apt-get update -qq || true
                    fi
                    if run_as_root apt-get install -y sc 2>/dev/null || run_as_root apt-get install -y sc-im; then
                        ensure_sc_symlink
                        INSTALL_SUCCESS=true
                    fi
                fi
            elif [ "$pkg" = "cmatrix" ]; then
                if run_as_root apt-get install -y cmatrix; then
                    INSTALL_SUCCESS=true
                else
                    if command -v add-apt-repository &>/dev/null; then
                        run_as_root add-apt-repository -y universe 2>/dev/null || true
                        run_as_root apt-get update -qq || true
                    fi
                    if run_as_root apt-get install -y cmatrix; then
                        INSTALL_SUCCESS=true
                    fi
                fi
            else
                if run_as_root apt-get install -y "$pkg"; then
                    INSTALL_SUCCESS=true
                fi
            fi
            ;;
        dnf|yum)
            if [ "$pkg" = "cmatrix" ]; then
                if run_as_root "$PKG_MANAGER" install -y cmatrix; then
                    INSTALL_SUCCESS=true
                else
                    echo "  [→] Attempting to enable EPEL repository for cmatrix..."
                    run_as_root "$PKG_MANAGER" install -y epel-release 2>/dev/null || true
                    if run_as_root "$PKG_MANAGER" install -y cmatrix; then
                        INSTALL_SUCCESS=true
                    fi
                fi
            elif [ "$pkg" = "sc" ]; then
                if run_as_root "$PKG_MANAGER" install -y sc 2>/dev/null || run_as_root "$PKG_MANAGER" install -y sc-im 2>/dev/null; then
                    ensure_sc_symlink
                    INSTALL_SUCCESS=true
                else
                    echo "  [→] Attempting to enable EPEL repository for sc-im..."
                    run_as_root "$PKG_MANAGER" install -y epel-release 2>/dev/null || true
                    if run_as_root "$PKG_MANAGER" install -y sc-im; then
                        ensure_sc_symlink
                        INSTALL_SUCCESS=true
                    fi
                fi
            else
                if run_as_root "$PKG_MANAGER" install -y "$pkg"; then
                    INSTALL_SUCCESS=true
                fi
            fi
            ;;
        pacman)
            if [ "$pkg" = "sc" ]; then
                if run_as_root pacman -S --noconfirm sc 2>/dev/null || run_as_root pacman -S --noconfirm sc-im; then
                    ensure_sc_symlink
                    INSTALL_SUCCESS=true
                fi
            else
                if run_as_root pacman -S --noconfirm "$pkg"; then
                    INSTALL_SUCCESS=true
                fi
            fi
            ;;
        zypper)
            if [ "$pkg" = "sc" ]; then
                if run_as_root zypper install -y sc 2>/dev/null || run_as_root zypper install -y sc-im; then
                    ensure_sc_symlink
                    INSTALL_SUCCESS=true
                fi
            else
                if run_as_root zypper install -y "$pkg"; then
                    INSTALL_SUCCESS=true
                fi
            fi
            ;;
        apk)
            if [ "$pkg" = "cmatrix" ]; then
                if run_as_root apk add "$pkg"; then
                    INSTALL_SUCCESS=true
                else
                    if [ -f /etc/apk/repositories ] && grep -q '^#.*\/community' /etc/apk/repositories; then
                        run_as_root sed -i '/\/community/s/^#//' /etc/apk/repositories 2>/dev/null || true
                        run_as_root apk update &>/dev/null || true
                    fi
                    if run_as_root apk add "$pkg"; then
                        INSTALL_SUCCESS=true
                    fi
                fi
            elif [ "$pkg" = "sc" ]; then
                if run_as_root apk add sc 2>/dev/null; then
                    INSTALL_SUCCESS=true
                else
                    if [ -f /etc/apk/repositories ] && grep -q '^#.*\/community' /etc/apk/repositories; then
                        run_as_root sed -i '/\/community/s/^#//' /etc/apk/repositories 2>/dev/null || true
                        run_as_root apk update &>/dev/null || true
                    fi
                    if run_as_root apk add sc-im; then
                        ensure_sc_symlink
                        INSTALL_SUCCESS=true
                    fi
                fi
            else
                if run_as_root apk add "$pkg"; then
                    INSTALL_SUCCESS=true
                fi
            fi
            ;;
        brew)
            if [ "$pkg" = "sc" ]; then
                if brew install sc 2>/dev/null || brew install sc-im; then
                    ensure_sc_symlink
                    INSTALL_SUCCESS=true
                fi
            else
                if brew install "$pkg"; then
                    INSTALL_SUCCESS=true
                fi
            fi
            ;;
    esac

    if [ "$INSTALL_SUCCESS" = true ] && is_pkg_installed "$pkg"; then
        echo "  [✓] Successfully installed $pkg."
        INSTALLED+=("$pkg")
        continue
    fi

    # Fallback: check for package-specific script in scripts/ (e.g. install_glow.sh)
    FALLBACK_SCRIPT="$SCRIPT_DIR/install_${pkg}.sh"
    if [ -f "$FALLBACK_SCRIPT" ]; then
        echo "  [!] Package manager failed for $pkg. Trying fallback installer: install_${pkg}.sh..."
        chmod +x "$FALLBACK_SCRIPT" 2>/dev/null || true
        if bash "$FALLBACK_SCRIPT"; then
            if is_pkg_installed "$pkg"; then
                echo "  [✓] Successfully installed $pkg via fallback script."
                INSTALLED+=("$pkg")
                continue
            fi
        fi
    fi

    echo "  [✗] Failed to install $pkg."
    FAILED+=("$pkg")
done

echo ""
echo "============================================================"
echo "                   FINAL INSTALLATION REPORT                "
echo "============================================================"
echo "Total packages evaluated: ${#PACKAGES[@]}"
echo "Successfully installed / available: ${#INSTALLED[@]}"
echo "Failed: ${#FAILED[@]}"
echo "------------------------------------------------------------"

if [ ${#INSTALLED[@]} -gt 0 ]; then
    echo "Installed / Available Apps:"
    for pkg in "${INSTALLED[@]}"; do
        echo "  [✓] $pkg"
    done
fi

if [ ${#FAILED[@]} -gt 0 ]; then
    echo ""
    echo "Failed Apps and Possible Solutions:"
    for pkg in "${FAILED[@]}"; do
        echo ""
        echo "[✗] App: $pkg"
        get_solution_for_pkg "$pkg"
    done
    echo ""
    echo "============================================================"
    echo "Installation finished with warnings (${#FAILED[@]} package(s) failed)."
    exit 1
else
    echo ""
    echo "============================================================"
    echo "All basic tools are installed and available!"
    exit 0
fi
