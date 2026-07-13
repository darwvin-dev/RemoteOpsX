#!/usr/bin/env bash
set -euo pipefail

rm -f "${HOME}/.local/bin/remoteopsx"
rm -f "${HOME}/.local/share/applications/remoteopsx.desktop"
rm -f "${HOME}/.local/share/icons/hicolor/128x128/apps/remoteopsx.png"

if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "${HOME}/.local/share/applications" >/dev/null 2>&1 || true
fi

echo "Removed RemoteOpsX AppImage integration."
