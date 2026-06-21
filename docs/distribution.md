# Distribution

RemoteOpsX ships as a native Linux desktop app. Docker is not required.

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

## Local AppImage Install

```bash
chmod +x RemoteOpsX-x86_64.AppImage
./packaging/linux/install-appimage.sh ./RemoteOpsX-x86_64.AppImage
```

On Arch, install FUSE 2 if the AppImage does not launch:

```bash
sudo pacman -S fuse2
```

## Arch Package

The starter `PKGBUILD` is in `packaging/arch/PKGBUILD`.

Before publishing to AUR or a pacman repository:

1. Replace `OWNER` in `url` with the GitHub organization/user.
2. Copy `src-tauri/icons/128x128.png` to `packaging/arch/remoteopsx.png`.
3. Generate checksums:

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
