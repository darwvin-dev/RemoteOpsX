#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
case "$(uname -s)" in
  Linux) exec bash "${ROOT_DIR}/packaging/linux/bootstrap-dependencies.sh" "$@" ;;
  Darwin) exec bash "${ROOT_DIR}/packaging/macos/bootstrap-dependencies.sh" "$@" ;;
  *) echo "RemoteOpsX dependency bootstrap supports Linux and macOS." >&2; exit 1 ;;
esac
