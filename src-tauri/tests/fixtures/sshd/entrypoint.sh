#!/bin/sh
# Provision the test user with the injected public key, then run sshd in the
# foreground on port 2222. PUBLIC_KEY / USER_NAME come from `docker run -e`.
set -e

USER_NAME="${USER_NAME:-ops}"

if ! id "$USER_NAME" >/dev/null 2>&1; then
    useradd -m -s /bin/bash "$USER_NAME"
fi

HOME_DIR="$(getent passwd "$USER_NAME" | cut -d: -f6)"
mkdir -p "$HOME_DIR/.ssh"
printf '%s\n' "$PUBLIC_KEY" > "$HOME_DIR/.ssh/authorized_keys"
chmod 700 "$HOME_DIR/.ssh"
chmod 600 "$HOME_DIR/.ssh/authorized_keys"
chown -R "$USER_NAME:$USER_NAME" "$HOME_DIR/.ssh"

# Generate host keys if missing.
ssh-keygen -A >/dev/null 2>&1

exec /usr/sbin/sshd -D -p 2222
