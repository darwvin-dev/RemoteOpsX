#!/usr/bin/env bash
set -euo pipefail

MODE="runtime"
ASSUME_YES=0
DRY_RUN=0

usage() {
  cat <<'EOF'
Install RemoteOpsX Linux dependencies using the host package manager.

Usage: ./packaging/linux/bootstrap-dependencies.sh [--runtime|--build] [--yes] [--dry-run]

  --runtime  Install dependencies needed by the packaged application (default)
  --build    Install runtime plus Rust/Tauri and native protocol build dependencies
  --yes      Skip the confirmation prompt
  --dry-run  Print the detected package manager and packages without changes
EOF
}

for argument in "$@"; do
  case "${argument}" in
    --runtime) MODE="runtime" ;;
    --build) MODE="build" ;;
    --yes|-y) ASSUME_YES=1 ;;
    --dry-run) DRY_RUN=1 ;;
    --help|-h) usage; exit 0 ;;
    *) echo "Unknown option: ${argument}" >&2; usage >&2; exit 2 ;;
  esac
done

if [[ "${EUID}" -eq 0 ]]; then
  SUDO=()
elif command -v sudo >/dev/null 2>&1; then
  SUDO=(sudo)
else
  echo "sudo is required to install system packages." >&2
  exit 1
fi

manager=""
for candidate in pacman apt-get dnf zypper; do
  if command -v "${candidate}" >/dev/null 2>&1; then
    manager="${candidate}"
    break
  fi
done

if [[ -z "${manager}" ]]; then
  echo "Unsupported distribution: pacman, apt, dnf, or zypper was not found." >&2
  echo "See docs/distribution.md for the dependency list." >&2
  exit 1
fi

runtime_packages=()
build_packages=()
case "${manager}" in
  pacman)
    runtime_packages=(fuse2 openssh sshpass curl freerdp tigervnc libsecret)
    build_packages=(base-devel rustup webkit2gtk-4.1 libayatana-appindicator librsvg libvncserver clang cmake pkgconf)
    install_command=("${SUDO[@]}" pacman -S --needed)
    [[ "${ASSUME_YES}" -eq 1 ]] && install_command+=(--noconfirm)
    ;;
  apt-get)
    runtime_packages=(fuse3 openssh-client sshpass curl tigervnc-viewer gnome-keyring)
    if apt-cache show freerdp3-x11 >/dev/null 2>&1; then
      runtime_packages+=(freerdp3-x11)
      build_packages+=(freerdp3-dev)
    else
      runtime_packages+=(freerdp2-x11)
      build_packages+=(freerdp2-dev)
    fi
    if apt-cache show rustup >/dev/null 2>&1; then
      build_packages+=(rustup)
    else
      build_packages+=(cargo rustc)
    fi
    build_packages+=(build-essential libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev libssl-dev libvncserver-dev clang cmake pkg-config)
    install_command=("${SUDO[@]}" apt-get install)
    [[ "${ASSUME_YES}" -eq 1 ]] && install_command+=(-y)
    ;;
  dnf)
    runtime_packages=(fuse openssh-clients sshpass curl freerdp tigervnc gnome-keyring)
    build_packages=(gcc gcc-c++ make rust cargo webkit2gtk4.1-devel libappindicator-gtk3-devel librsvg2-devel openssl-devel freerdp-devel libvncserver-devel clang cmake pkgconf-pkg-config)
    install_command=("${SUDO[@]}" dnf install)
    [[ "${ASSUME_YES}" -eq 1 ]] && install_command+=(-y)
    ;;
  zypper)
    runtime_packages=(fuse openssh sshpass curl freerdp tigervnc gnome-keyring)
    build_packages=(gcc gcc-c++ make rust cargo webkit2gtk-4_1-devel libappindicator3-devel librsvg-devel libopenssl-devel freerdp-devel libvncserver-devel clang cmake pkg-config)
    install_command=("${SUDO[@]}" zypper install)
    [[ "${ASSUME_YES}" -eq 1 ]] && install_command+=(--non-interactive)
    ;;
esac

packages=("${runtime_packages[@]}")
if [[ "${MODE}" == "build" ]]; then
  packages+=("${build_packages[@]}")
fi

echo "RemoteOpsX will install ${MODE} dependencies with ${manager}:"
printf '  %s\n' "${packages[@]}"
if [[ "${DRY_RUN}" -eq 1 ]]; then
  exit 0
fi
if [[ "${ASSUME_YES}" -ne 1 ]]; then
  read -r -p "Continue? [y/N] " reply
  [[ "${reply}" =~ ^[Yy]$ ]] || { echo "Cancelled."; exit 0; }
fi

if [[ "${manager}" == "apt-get" ]]; then
  "${SUDO[@]}" apt-get update
fi
"${install_command[@]}" "${packages[@]}"

if [[ "${MODE}" == "build" ]] && command -v rustup >/dev/null 2>&1; then
  rustup toolchain install stable
  rustup default stable
fi

echo "RemoteOpsX ${MODE} dependencies are installed."
