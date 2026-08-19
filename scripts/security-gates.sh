#!/usr/bin/env bash
set -euo pipefail

# Security regression tests and explanatory comments intentionally contain
# forbidden strings to document/assert that unsafe policies never return.
# Inspect executable production source only: stop at the first #[cfg(test)] and
# omit comment-only lines. Inline source remains scanned.
production_source() {
  awk '
    /^#\[cfg\(test\)\]/{exit}
    {
      trimmed=$0
      sub(/^[[:space:]]+/, "", trimmed)
      if (trimmed ~ /^\/\//) next
      print $0
    }
  ' "$1"
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

# Packaged E2E uses an in-process WebDriver HTTP server. Its transport must stay
# feature-gated and optional so release builds have no WebDriver listener or
# test-only IPC bridge.
node - <<'NODE'
const fs = require('node:fs');
const config = JSON.parse(fs.readFileSync('src-tauri/tauri.conf.json', 'utf8'));
if (config.app?.withGlobalTauri !== false) {
  throw new Error('production tauri.conf.json must keep app.withGlobalTauri=false');
}
const pkg = JSON.parse(fs.readFileSync('package.json', 'utf8'));
const packageNames = [
  ...Object.keys(pkg.dependencies || {}),
  ...Object.keys(pkg.devDependencies || {}),
  ...Object.keys(pkg.optionalDependencies || {}),
];
if (packageNames.some((name) => name.startsWith('@wdio/') || name === 'webdriverio')) {
  throw new Error('packaged E2E must use the dependency-free W3C client; JavaScript WebdriverIO packages are not permitted');
}
NODE

if grep -Fq 'tauri-plugin-wdio-webdriver' src-tauri/Cargo.toml; then
  if ! grep -Fq 'tauri-plugin-wdio-webdriver = { version = "1", optional = true }' src-tauri/Cargo.toml; then
    echo 'SECURITY GATE FAILED: packaged WebDriver dependency must remain optional.' >&2
    exit 1
  fi
  if ! awk '
    /#\[cfg\(feature = "e2e"\)\]/{guard=NR}
    /tauri_plugin_wdio_webdriver::init\(\)/{
      found=1
      if (!guard || NR-guard > 2) exit 2
    }
    END { if (!found) exit 3 }
  ' src-tauri/src/lib.rs; then
    echo 'SECURITY GATE FAILED: embedded WebDriver registration must be immediately guarded by feature="e2e".' >&2
    exit 1
  fi
fi

if [[ -f src-tauri/tauri.e2e.conf.json ]]; then
  if grep -Fq '"wdio:default"' src-tauri/tauri.e2e.conf.json; then
    echo 'SECURITY GATE FAILED: packaged smoke tests must not enable the WDIO execute/mock IPC bridge.' >&2
    exit 1
  fi
fi

echo 'RemoteOpsX production security source gates passed.'
