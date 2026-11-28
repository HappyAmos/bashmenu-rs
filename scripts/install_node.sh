#!/usr/bin/env bash
# ==============================================================================
# SCRIPT: install_node.sh
# DESCRIPTION: Cross-platform installer for Node.js and npm via system package
#              managers, winget, or nvm.
# USAGE:       install_node.sh
# OPTIONS:     None.
# ==============================================================================

# Detect OS
OS_TYPE=$(uname -s)

case "$OS_TYPE" in
    MINGW*|MSYS*|CYGWIN*)
        echo "=========================================="
        echo "Windows detected. Installing Node.js..."
        echo "=========================================="
        if command -v winget.exe >/dev/null 2>&1 || command -v winget >/dev/null 2>&1; then
            winget install OpenJS.NodeJS.LTS || true
        else
            echo "Please install Node.js via official installer: https://nodejs.org/"
        fi
        exit 0
        ;;
esac

if [ -n "$TERMUX_VERSION" ] || [[ "${PREFIX:-}" == *"/com.termux/"* ]]; then
    echo "=========================================="
    echo "🤖 Termux (Android) detected. Installing Node.js LTS via pkg..."
    echo "=========================================="
    pkg install -y nodejs-lts || pkg install -y nodejs
    echo "Node version: $(node -v 2>/dev/null || echo 'N/A')"
    echo "npm version:  $(npm -v 2>/dev/null || echo 'N/A')"
    exit 0
fi

run_root() {
    if [ "$(id -u)" = "0" ]; then
        "$@"
    elif command -v sudo >/dev/null 2>&1; then
        sudo "$@"
    else
        "$@"
    fi
}

compute_sha256() {
    local file="$1"
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$file" | awk '{print $1}'
    elif command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "$file" | awk '{print $1}'
    else
        echo ""
    fi
}

# Downloads an installer script to a temporary file, verifies an expected SHA256
# checksum when provided, executes it with bash, and removes the temporary file.
run_verified_installer() {
    local name="$1"
    local url="$2"
    local expected_sha="$3"
    local manual_url="$4"
    local tmp_script
    tmp_script="$(mktemp "${TMPDIR:-/tmp}/${name}-install.XXXXXX.sh")"

    if ! curl --proto '=https' --tlsv1.2 -fsSL "$url" -o "$tmp_script" || [ ! -s "$tmp_script" ]; then
        rm -f "$tmp_script"
        echo "Error: Failed to download ${name} installer. Install manually via ${manual_url}" >&2
        return 1
    fi

    local actual_sha
    actual_sha="$(compute_sha256 "$tmp_script")"
    if [ -n "$expected_sha" ]; then
        if [ -z "$actual_sha" ] || [ "$actual_sha" != "$expected_sha" ]; then
            rm -f "$tmp_script"
            echo "Error: ${name} installer SHA256 mismatch (expected ${expected_sha}, got ${actual_sha:-unavailable})." >&2
            echo "Install manually from ${manual_url}" >&2
            return 1
        fi
        echo "  [+] Verified ${name} installer SHA256: ${actual_sha}"
    elif [ -n "$actual_sha" ]; then
        echo "  [*] Downloaded ${name} installer (SHA256: ${actual_sha}; manual install: ${manual_url})"
    fi

    /bin/bash "$tmp_script"
    local rc=$?
    rm -f "$tmp_script"
    return "$rc"
}

if [ "$OS_TYPE" = "Darwin" ]; then
    echo "=========================================="
    echo "🍏 macOS detected. Setting up via Homebrew..."
    echo "=========================================="

    # Check if Homebrew is installed
    if ! command -v brew &> /dev/null; then
        echo "Homebrew not found. Installing Homebrew..."
        run_verified_installer \
            "homebrew" \
            "https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh" \
            "${HOMEBREW_INSTALL_SHA256:-}" \
            "https://brew.sh" || exit 1
        
        # Add Homebrew to PATH for the current session depending on architecture
        if [ -f /opt/homebrew/bin/brew ]; then
            eval "$(/opt/homebrew/bin/brew shellenv)"
        elif [ -f /usr/local/bin/brew ]; then
            eval "$(/usr/local/bin/brew shellenv)"
        fi
    else
        echo "Homebrew is already installed."
        brew update
    fi

    # Install Node and NPM directly via Homebrew
    echo "Installing Node.js and npm via Homebrew..."
    brew install node

    # Verify installation
    echo "=========================================="
    echo "✅ Installation Complete!"
    echo "Node version: $(node -v)"
    echo "npm version:  $(npm -v)"
    echo "=========================================="

elif [ "$OS_TYPE" = "Linux" ]; then
    echo "=========================================="
    echo "🐧 Linux detected. Running NVM setup..."
    echo "=========================================="
    
    # Determine Package Manager and install dependencies
    if command -v apt-get &> /dev/null; then
        echo "Detected Debian/Ubuntu-based system."
        run_root apt-get update
        run_root apt-get install -y curl git build-essential
    elif command -v dnf &> /dev/null; then
        echo "Detected Fedora/RHEL-based system."
        run_root dnf groupinstall -y "Development Tools"
        run_root dnf install -y curl git
    elif command -v pacman &> /dev/null; then
        echo "Detected Arch Linux-based system."
        run_root pacman -Syu --noconfirm base-devel curl git
    elif command -v apk &> /dev/null; then
        echo "Detected Alpine Linux-based system."
        run_root apk add build-base curl git
    elif command -v zypper &> /dev/null; then
        echo "Detected openSUSE-based system."
        run_root zypper install -y -t pattern devel_basis
        run_root zypper install -y curl git
    else
        echo "Warning: Unknown package manager. Proceeding with NVM install anyway..."
    fi

    # Install NVM (pinned v0.40.1 with SHA256 verification)
    echo "Installing NVM (Node Version Manager)..."
    NVM_V0_40_1_SHA256="abdb525ee9f5b48b34d8ed9fc67c6013fb0f659712e401ecd88ab989b3af8f53"
    run_verified_installer \
        "nvm" \
        "https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.1/install.sh" \
        "$NVM_V0_40_1_SHA256" \
        "https://github.com/nvm-sh/nvm#installing-and-updating" || exit 1

    # Load NVM into current session
    export NVM_DIR="$HOME/.nvm"
    # shellcheck source=/dev/null
    [ -s "$NVM_DIR/nvm.sh" ] && \. "$NVM_DIR/nvm.sh"

    # Install Node LTS
    echo "Installing Node.js LTS version via NVM..."
    nvm install --lts

    # Verify installation
    echo "=========================================="
    echo "✅ Installation Complete!"
    echo "Node version: $(node -v)"
    echo "npm version:  $(npm -v)"
    echo "=========================================="
else
    echo "❌ Unsupported operating system: $OS_TYPE"
    exit 1
fi
