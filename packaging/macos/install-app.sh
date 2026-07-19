#!/usr/bin/env bash
set -euo pipefail

APP_PATH="${1:-RemoteOpsX.app}"
if [[ ! -d "${APP_PATH}" || "${APP_PATH}" != *.app ]]; then
  echo "Application bundle not found: ${APP_PATH}" >&2
  echo "Usage: $0 path/to/RemoteOpsX.app" >&2
  exit 1
fi

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
if [[ "${REMOTEOPSX_SKIP_DEPENDENCIES:-0}" != "1" ]]; then
  bash "${SCRIPT_DIR}/bootstrap-dependencies.sh" --runtime
fi

mkdir -p "${HOME}/Applications"
destination="${HOME}/Applications/RemoteOpsX.app"
if [[ -e "${destination}" ]]; then
  read -r -p "Replace the existing ${destination}? [y/N] " reply
  [[ "${reply}" =~ ^[Yy]$ ]] || { echo "Cancelled."; exit 0; }
  rm -rf "${destination}"
fi
cp -R "${APP_PATH}" "${destination}"
echo "Installed RemoteOpsX to ${destination}"
