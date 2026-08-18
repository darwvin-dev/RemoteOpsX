#!/usr/bin/env bash
set -euo pipefail

fail_if_present() {
  local pattern="$1"
  local message="$2"
  shift 2
  if grep -RInE -- "$pattern" "$@"; then
    echo "SECURITY GATE FAILED: $message" >&2
    exit 1
  fi
}

# SSH identity must always be explicit through the app-managed known_hosts file.
fail_if_present 'StrictHostKeyChecking=(accept-new|no)' \
  'SSH transport must not silently accept first-seen or changed host keys.' \
  src-tauri/src

# FreeRDP must never bypass certificate validation or carry stored passwords in argv.
fail_if_present '/cert:ignore' \
  'FreeRDP certificate validation must not be disabled.' \
  src-tauri/src
fail_if_present '(/p:|"/p:")' \
  'FreeRDP passwords must not be placed in process argv.' \
  src-tauri/src

# Known dangerous credential styles should never be introduced in process builders.
fail_if_present '(--password|-password)[= ]' \
  'Do not add literal password command-line arguments; use stdin/env/keyring-safe transport.' \
  src-tauri/src

echo 'RemoteOpsX security source gates passed.'
