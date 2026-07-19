# Distribution

RemoteOpsX ships as a native Linux and macOS desktop app. Docker is not required.

## GitHub Releases

The release workflow builds Linux bundles on tagged releases:

```bash
git tag v0.1.0
git push origin v0.1.0
```

Expected release assets:

- `RemoteOpsX-x86_64.AppImage`
- Debian package (`.deb`)
- RPM package (`.rpm`)
- macOS disk image (`.dmg`)

## macOS

The macOS build uses the native Keychain for secrets and the built-in Screen
Sharing application for VNC. FreeRDP is installed with Homebrew for RDP.

Install build dependencies:

```bash
npm run deps:build
```

For an unpacked application bundle, the installer prepares runtime dependencies
before copying RemoteOpsX into `~/Applications`:

```bash
bash packaging/macos/install-app.sh path/to/RemoteOpsX.app
```

## Local AppImage Install

The installer first detects the Linux distribution and offers to install all
runtime dependencies through its official package manager. It does not make
system changes until the user confirms the package list.

```bash
chmod +x RemoteOpsX-x86_64.AppImage
./packaging/linux/install-appimage.sh ./RemoteOpsX-x86_64.AppImage
```

Set `REMOTEOPSX_SKIP_DEPENDENCIES=1` only when dependencies are already managed
by the host image or an administrator:

```bash
REMOTEOPSX_SKIP_DEPENDENCIES=1 ./packaging/linux/install-appimage.sh ./RemoteOpsX-x86_64.AppImage
```

For source development, one command installs Rust, Tauri build requirements,
FreeRDP, and VNC development libraries on Arch, Debian/Ubuntu, Fedora, or
openSUSE:

```bash
npm run deps:build
```

## Arch Package

The starter `PKGBUILD` is in `packaging/arch/PKGBUILD`.

Before publishing to AUR or a pacman repository:

1. Copy `src-tauri/icons/128x128.png` to `packaging/arch/remoteopsx.png`.
2. Generate checksums:

   ```bash
   cd packaging/arch
   updpkgsums
   makepkg --printsrcinfo > .SRCINFO
   makepkg -si
   ```

For a private pacman repository, build the package and add it to a repo database:

```bash
makepkg -s
repo-add remoteopsx.db.tar.gz remoteopsx-bin-*.pkg.tar.zst
```
