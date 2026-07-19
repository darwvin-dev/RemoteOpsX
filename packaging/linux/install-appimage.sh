#!/usr/bin/env bash
set -euo pipefail

APPIMAGE="${1:-RemoteOpsX-x86_64.AppImage}"
APP_NAME="remoteopsx"
INSTALL_DIR="${HOME}/.local/bin"
APP_DIR="${HOME}/.local/share/applications"
ICON_DIR="${HOME}/.local/share/icons/hicolor/128x128/apps"

if [[ "${REMOTEOPSX_SKIP_DEPENDENCIES:-0}" != "1" ]]; then
  SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
  bash "${SCRIPT_DIR}/bootstrap-dependencies.sh" --runtime
fi

if [[ ! -f "${APPIMAGE}" ]]; then
  echo "AppImage not found: ${APPIMAGE}" >&2
  echo "Usage: $0 path/to/RemoteOpsX-x86_64.AppImage" >&2
  exit 1
fi

mkdir -p "${INSTALL_DIR}" "${APP_DIR}" "${ICON_DIR}"
install -m 0755 "${APPIMAGE}" "${INSTALL_DIR}/${APP_NAME}"

if [[ -f "src-tauri/icons/128x128.png" ]]; then
  install -m 0644 "src-tauri/icons/128x128.png" "${ICON_DIR}/${APP_NAME}.png"
fi

sed "s|Exec=remoteopsx|Exec=${INSTALL_DIR}/${APP_NAME}|" \
  packaging/linux/remoteopsx.desktop > "${APP_DIR}/${APP_NAME}.desktop"

if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "${APP_DIR}" >/dev/null 2>&1 || true
fi

if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache "${HOME}/.local/share/icons/hicolor" >/dev/null 2>&1 || true
fi

echo "Installed RemoteOpsX to ${INSTALL_DIR}/${APP_NAME}"
