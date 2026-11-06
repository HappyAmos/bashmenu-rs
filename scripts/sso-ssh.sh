#!/usr/bin/env bash
# ==============================================================================
# SCRIPT:      sso-ssh.sh
# PURPOSE:     Automate passwordless SSH key generation and setup between client
#              and a remote target host.
# DESCRIPTION: Connects to a remote server, generates a dedicated Ed25519 keypair
#              on the remote host, authorizes the public key, downloads the
#              private key to ~/.ssh, configures ~/.ssh/config, and verifies
#              passwordless connection capability.
# USAGE:       sso-ssh.sh <[user@]server> [port]
# EXAMPLES:    sso-ssh.sh ubuntu@192.168.1.50
#              sso-ssh.sh ubuntu@192.168.1.50 2222
#              sso-ssh.sh myserver.local
# ARGUMENTS:   <[user@]server>  Remote target host with optional username.
#              [port]           SSH port on target host (defaults to 22).
# ==============================================================================
set -euo pipefail

# Usage check
if [[ $# -lt 1 ]]; then
    echo "Usage: $0 <[user@]server> [port]"
    echo "Example: $0 ubuntu@192.168.1.50 22"
    exit 1
fi

TARGET="$1"
PORT="${2:-22}"

# Extract host part to name the local key file cleanly
HOST_ONLY="${TARGET#*@}"
USER_PART="${TARGET%@*}"
KEY_NAME="id_ed25519_${HOST_ONLY//[^a-zA-Z0-9._-]/_}"
LOCAL_KEY_PATH="$HOME/.ssh/$KEY_NAME"
# shellcheck disable=SC2088
REMOTE_TMP_KEY="~/.ssh/tmp_key_$$"

# 1. Prepare local ~/.ssh directory
mkdir -p "$HOME/.ssh"
chmod 700 "$HOME/.ssh"

if [[ -f "$LOCAL_KEY_PATH" ]]; then
    echo "Error: Local key '$LOCAL_KEY_PATH' already exists. Aborting to prevent overwrite."
    exit 1
fi

echo "[*] Connecting to $TARGET (port $PORT) to generate and authorize keys..."

# 2. Generate key on server, add to authorized_keys, output private key to client, and clean up server
ssh -p "$PORT" "$TARGET" "
    set -e
    mkdir -p ~/.ssh && chmod 700 ~/.ssh
    ssh-keygen -t ed25519 -f $REMOTE_TMP_KEY -N '' -C 'client-key-for-$HOST_ONLY' -q
    cat ${REMOTE_TMP_KEY}.pub >> ~/.ssh/authorized_keys
    chmod 600 ~/.ssh/authorized_keys
    cat $REMOTE_TMP_KEY
    rm -f $REMOTE_TMP_KEY ${REMOTE_TMP_KEY}.pub
" > "$LOCAL_KEY_PATH"

# Verify the key was actually downloaded
if [[ ! -s "$LOCAL_KEY_PATH" ]]; then
    rm -f "$LOCAL_KEY_PATH"
    echo "[!] Failed to generate or download the key."
    exit 1
fi

chmod 600 "$LOCAL_KEY_PATH"
echo "[+] Private key saved to $LOCAL_KEY_PATH"

# 3. Add an entry to ~/.ssh/config if not already present so `ssh server` works automatically
SSH_CONFIG="$HOME/.ssh/config"
touch "$SSH_CONFIG"
chmod 600 "$SSH_CONFIG"

if ! grep -q "IdentityFile $LOCAL_KEY_PATH" "$SSH_CONFIG" 2>/dev/null; then
    {
        echo ""
        echo "Host $HOST_ONLY"
        [[ "$USER_PART" != "$HOST_ONLY" ]] && echo "    User $USER_PART"
        [[ "$PORT" != "22" ]] && echo "    Port $PORT"
        echo "    IdentityFile $LOCAL_KEY_PATH"
        echo "    IdentitiesOnly yes"
    } >> "$SSH_CONFIG"
    echo "[+] Added host entry for '$HOST_ONLY' to $SSH_CONFIG"
fi

# 4. Test passwordless login
echo "[*] Testing passwordless connection..."
if ssh -p "$PORT" -i "$LOCAL_KEY_PATH" -o BatchMode=yes -o ConnectTimeout=5 "$TARGET" "echo '[+] Passwordless SSH login successful!'"; then
    echo "[+] Done! You can now connect with: ssh $TARGET"
else
    echo "[!] Warning: Key was installed, but automatic test failed. Check server sshd_config (PubkeyAuthentication yes)."
fi
