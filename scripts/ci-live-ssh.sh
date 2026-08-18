#!/usr/bin/env bash
set -euo pipefail

ROOT="$(mktemp -d)"
USER_NAME="remoteopsx-ci"
PASSWORD='RemoteOpsX-CI-Secret-2026'
TARGET_PORT=22222
JUMP_PORT=22223

cleanup() {
  if [[ -n "${TARGET_PID:-}" ]]; then sudo kill "$TARGET_PID" 2>/dev/null || true; fi
  if [[ -n "${JUMP_PID:-}" ]]; then sudo kill "$JUMP_PID" 2>/dev/null || true; fi
  sudo userdel -r "$USER_NAME" 2>/dev/null || true
  rm -rf "$ROOT"
}
trap cleanup EXIT

sudo mkdir -p /run/sshd
if id "$USER_NAME" >/dev/null 2>&1; then sudo userdel -r "$USER_NAME" || true; fi
sudo useradd -m -s /bin/bash "$USER_NAME"
echo "$USER_NAME:$PASSWORD" | sudo chpasswd

ssh-keygen -q -t ed25519 -N '' -f "$ROOT/client_key"
ssh-keygen -q -t ed25519 -N '' -f "$ROOT/target_host_key"
ssh-keygen -q -t ed25519 -N '' -f "$ROOT/jump_host_key"
ssh-keygen -q -t ed25519 -N '' -f "$ROOT/alt_host_key"

sudo install -d -m 700 -o "$USER_NAME" -g "$USER_NAME" "/home/$USER_NAME/.ssh"
sudo install -m 600 -o "$USER_NAME" -g "$USER_NAME" "$ROOT/client_key.pub" "/home/$USER_NAME/.ssh/authorized_keys"

write_config() {
  local path="$1" port="$2" host_key="$3" pid_file="$4"
  cat >"$path" <<EOF
Port $port
ListenAddress 127.0.0.1
HostKey $host_key
PidFile $pid_file
AuthorizedKeysFile .ssh/authorized_keys
PasswordAuthentication yes
PubkeyAuthentication yes
KbdInteractiveAuthentication no
ChallengeResponseAuthentication no
UsePAM no
PermitRootLogin no
AllowUsers $USER_NAME
AllowTcpForwarding yes
GatewayPorts no
PermitTunnel no
X11Forwarding no
PrintMotd no
Subsystem sftp internal-sftp
LogLevel VERBOSE
EOF
}

write_config "$ROOT/target_sshd_config" "$TARGET_PORT" "$ROOT/target_host_key" "$ROOT/target.pid"
write_config "$ROOT/jump_sshd_config" "$JUMP_PORT" "$ROOT/jump_host_key" "$ROOT/jump.pid"

sudo /usr/sbin/sshd -D -e -f "$ROOT/target_sshd_config" >"$ROOT/target.log" 2>&1 &
TARGET_PID=$!
sudo /usr/sbin/sshd -D -e -f "$ROOT/jump_sshd_config" >"$ROOT/jump.log" 2>&1 &
JUMP_PID=$!

wait_port() {
  local port="$1"
  for _ in $(seq 1 80); do
    if (echo >/dev/tcp/127.0.0.1/"$port") >/dev/null 2>&1; then return 0; fi
    sleep 0.1
  done
  echo "sshd on port $port did not become ready" >&2
  cat "$ROOT/target.log" "$ROOT/jump.log" >&2 || true
  return 1
}
wait_port "$TARGET_PORT"
wait_port "$JUMP_PORT"

export REMOTEOPSX_TEST_HOST=127.0.0.1
export REMOTEOPSX_TEST_USER="$USER_NAME"
export REMOTEOPSX_TEST_TARGET_PORT="$TARGET_PORT"
export REMOTEOPSX_TEST_JUMP_PORT="$JUMP_PORT"
export REMOTEOPSX_TEST_PRIVATE_KEY="$ROOT/client_key"
export REMOTEOPSX_TEST_ALT_HOST_PUBLIC_KEY="$ROOT/alt_host_key.pub"
export REMOTEOPSX_INTEGRATION_PASSWORD="$PASSWORD"
export REMOTEOPSX_TEST_TEMP="$ROOT"

cargo test --locked --manifest-path src-tauri/Cargo.toml --features integration-fixture --test live_ssh -- --test-threads=1
