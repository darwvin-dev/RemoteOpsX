#!/usr/bin/env bash
set -euo pipefail

# Security regression tests intentionally contain forbidden strings to assert
# that unsafe policies never return. Inspect only production sections (before
# the first #[cfg(test)]) so those test fixtures cannot create false positives.
production_source() {
  awk '/^#\[cfg\(test\)\]/{exit} {print}' "$1"
}

fail_in_production() {
  local pattern="$1"
  local message="$2"
  shift 2
  local file
  for file in "$@"; do
    if production_source "$file" | grep -nE -- "$pattern"; then
      echo "SECURITY GATE FAILED in $file: $message" >&2
      exit 1
    fi
  done
}

ssh_transport_files=(
  src-tauri/src/host_identity.rs
  src-tauri/src/ssh_manager.rs
  src-tauri/src/sftp_manager.rs
  src-tauri/src/tunnel_manager.rs
  src-tauri/src/pty_manager.rs
)

process_builder_files=(
  src-tauri/src/ssh_manager.rs
  src-tauri/src/sftp_manager.rs
  src-tauri/src/tunnel_manager.rs
  src-tauri/src/rdp_adapter.rs
  src-tauri/src/vnc_adapter.rs
  src-tauri/src/ftp_manager.rs
)

fail_in_production 'StrictHostKeyChecking=(accept-new|no)' \
  'SSH transport must not silently accept first-seen or changed host keys.' \
  "${ssh_transport_files[@]}"

fail_in_production '/cert:ignore' \
  'FreeRDP certificate validation must not be disabled.' \
  src-tauri/src/rdp_adapter.rs

fail_in_production '(/p:|"/p:")' \
  'FreeRDP passwords must not be placed in process argv.' \
  src-tauri/src/rdp_adapter.rs

fail_in_production '(--password|-password)[= ]' \
  'Do not add literal password command-line arguments; use stdin/env/keyring-safe transport.' \
  "${process_builder_files[@]}"

echo 'RemoteOpsX production security source gates passed.'
