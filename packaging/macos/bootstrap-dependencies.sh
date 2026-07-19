#!/usr/bin/env bash
set -euo pipefail

MODE="runtime"
ASSUME_YES=0
DRY_RUN=0

for argument in "$@"; do
  case "${argument}" in
    --runtime) MODE="runtime" ;;
    --build) MODE="build" ;;
    --yes|-y) ASSUME_YES=1 ;;
    --dry-run) DRY_RUN=1 ;;
    --help|-h)
      echo "Usage: $0 [--runtime|--build] [--yes] [--dry-run]"
      exit 0
      ;;
    *) echo "Unknown option: ${argument}" >&2; exit 2 ;;
  esac
done

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "This installer is for macOS. Use packaging/linux on Linux." >&2
  exit 1
fi

if ! command -v brew >/dev/null 2>&1; then
  echo "Homebrew is required to manage RemoteOpsX native dependencies." >&2
  echo "Install it from https://brew.sh and run this command again." >&2
  exit 1
fi

# OpenSSH, curl, Keychain and the built-in VNC Screen Sharing client ship with macOS.
formulae=(freerdp sshpass)
casks=()
if [[ "${MODE}" == "build" ]]; then
  formulae+=(rust node pkg-config cmake llvm libvncserver)
fi

echo "RemoteOpsX will install ${MODE} dependencies with Homebrew:"
printf '  formula: %s\n' "${formulae[@]}"
if [[ "${#casks[@]}" -gt 0 ]]; then
  printf '  cask: %s\n' "${casks[@]}"
fi

if [[ "${DRY_RUN}" -eq 1 ]]; then
  exit 0
fi
if [[ "${ASSUME_YES}" -ne 1 ]]; then
  read -r -p "Continue? [y/N] " reply
  [[ "${reply}" =~ ^[Yy]$ ]] || { echo "Cancelled."; exit 0; }
fi

brew install "${formulae[@]}"
if [[ "${#casks[@]}" -gt 0 ]]; then
  brew install --cask "${casks[@]}"
fi
echo "RemoteOpsX ${MODE} dependencies are installed."
