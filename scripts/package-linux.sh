#!/bin/sh
# Package what scripts/package.sh laid out (dist/pkg/backplane-<plat>/) for
# Linux desktops:
#   dist/backplane-<version>-<plat>.AppImage   one file, runs anywhere
#   dist/backplane_<version>_<arch>.deb         /opt/backplane, /usr/bin links
#   scripts/package-linux.sh v0.9.0 linux-x64
# Both update through their package, not in place (BACKPLANE_NO_UPDATE).
# appimagetool is downloaded into build/ unless APPIMAGETOOL names one.
set -eu
cd "$(dirname "$0")/.."
ver=$1 plat=$2
src="dist/pkg/backplane-$plat"
[ -x "$src/backplane" ] || { echo "run scripts/package.sh $ver $plat first"; exit 1; }
case $plat in
  linux-x64) arch=x86_64 deb=amd64 ;;
  linux-arm64) arch=aarch64 deb=arm64 ;;
  *) echo "no Linux packages for $plat"; exit 1 ;;
esac
# dpkg sorts ~ before anything, so a nightly sorts before its release
v=${ver#v}
dv=$(printf '%s' "$v" | tr '-' '~')
# the newest glibc the binaries ask for
glibc=$(objdump -T "$src/backplane" "$src/backplane-serve" | grep -o 'GLIBC_[0-9.]*' | sort -uV | tail -1)
glibc=${glibc#GLIBC_}
work=build/pkg-linux
rm -rf "$work"
mkdir -p "$work"

# AppImage: the files under usr/lib/backplane (the app finds web/ beside
# itself), the desktop file and icon at the top, as appimagetool wants
app="$work/AppDir"
mkdir -p "$app/usr/lib" "$app/usr/share/icons/hicolor/256x256/apps"
cp -R "$src" "$app/usr/lib/backplane"
cp deploy/backplane.desktop "$app/backplane.desktop"
cp assets/icon/backplane-256.png "$app/backplane.png"
cp assets/icon/backplane-256.png "$app/usr/share/icons/hicolor/256x256/apps/backplane.png"
cat > "$app/AppRun" <<'EOF'
#!/bin/sh
here=$(dirname "$(readlink -f "$0")")
export BACKPLANE_NO_UPDATE=1
exec "$here/usr/lib/backplane/backplane" "$@"
EOF
chmod +x "$app/AppRun"
tool=${APPIMAGETOOL:-build/appimagetool-$arch.AppImage}
if [ ! -x "$tool" ]; then
  curl -fsSL -o "$tool" "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-$arch.AppImage"
  chmod +x "$tool"
fi
# CI runners have no FUSE: the tool runs extracted
ARCH=$arch APPIMAGE_EXTRACT_AND_RUN=1 "$tool" --no-appstream "$app" "dist/backplane-$ver-$plat.AppImage"

# .deb: built with ar and tar, so any Linux can make it
root="$work/deb"
mkdir -p "$root/opt" "$root/usr/bin" "$root/usr/share/applications" "$root/usr/share/icons/hicolor/scalable/apps"
cp -R "$src" "$root/opt/backplane"
for b in backplane backplane-serve; do
  printf '#!/bin/sh\nBACKPLANE_NO_UPDATE=1 exec /opt/backplane/%s "$@"\n' "$b" > "$root/usr/bin/$b"
  chmod 755 "$root/usr/bin/$b"
done
cp deploy/backplane.desktop "$root/usr/share/applications/"
cp assets/icon/backplane-desktop.svg "$root/usr/share/icons/hicolor/scalable/apps/backplane.svg"
for s in 64 128 256 512; do
  mkdir -p "$root/usr/share/icons/hicolor/${s}x$s/apps"
  cp "assets/icon/backplane-$s.png" "$root/usr/share/icons/hicolor/${s}x$s/apps/backplane.png"
done
mkdir -p "$work/control"
cat > "$work/control/control" <<EOF
Package: backplane
Version: $dv
Architecture: $deb
Maintainer: i2cjak <i2cjak@users.noreply.github.com>
Installed-Size: $(du -sk "$root" | cut -f1)
Depends: libc6 (>= $glibc), libx11-6
Recommends: git, kicad, tailscale
Homepage: https://github.com/i2cjak/Backplane
Section: devel
Priority: optional
Description: Agentic hardware development, with live KiCad viewers
 Agent threads (Claude Code, Codex) beside board, schematic and 3D viewers
 that follow the project's files, in a native window.
EOF
tar -czf "$work/control.tar.gz" --owner=0 --group=0 --numeric-owner -C "$work/control" .
tar -czf "$work/data.tar.gz" --owner=0 --group=0 --numeric-owner -C "$root" .
printf '2.0\n' > "$work/debian-binary"
out="dist/backplane_${dv}_$deb.deb"
rm -f "$out"
(cd "$work" && ar rc "../../$out" debian-binary control.tar.gz data.tar.gz)
ls -la "dist/backplane-$ver-$plat.AppImage" "$out"
