#!/bin/sh
# A release's .deb, from its classic package and desktop app (nothing is
# compiled or relabelled here):
#
#   scripts/release-deb.sh v0.12.0 OUT linux-x64|linux-arm64 CLASSIC_TAR DESKTOP_TAR
#
# /opt/backplane holds the classic layout (backplane, backplane-serve, web/)
# and the desktop app in desktop/, where the launcher finds it; /usr/bin has
# backplane and backplane-serve (BACKPLANE_NO_UPDATE=1: the package updates
# it). The Linux sandbox is set up as root installs of Electron apps do:
# postinst makes desktop/runtime/chrome-sandbox root:root 4755 and installs
# an AppArmor profile that lets the app use user namespaces (Ubuntu 23.10+
# restricts them), loaded when AppArmor is there; postrm takes it out.
# The package is built with ar and tar, so any Linux can make it.
set -eu
[ $# -eq 5 ] || { sed -n '2,14p' "$0"; exit 2; }
abs() { case $1 in /*) printf '%s\n' "$1" ;; *) printf '%s/%s\n' "$PWD" "$1" ;; esac; }
ver=$1 out=$(abs "$2") plat=$3 classic=$(abs "$4") desktop=$(abs "$5")
cd "$(dirname "$0")/.."
case $plat in
  linux-x64) deb=amd64 ;;
  linux-arm64) deb=arm64 ;;
  *) echo "release-deb: no .deb for $plat" >&2; exit 2 ;;
esac
v=${ver#v}
# dpkg sorts ~ before anything, so a nightly sorts before its release
dv=$(printf '%s' "$v" | tr '-' '~')
mkdir -p "$out"
out=$(cd "$out" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
root=$work/root
mkdir -p "$root/opt" "$root/usr/bin" "$root/usr/share/applications" "$root/usr/share/icons/hicolor/scalable/apps" "$root/etc/apparmor.d"
tar -xzf "$classic" -C "$work"
[ -x "$work/backplane-$plat/backplane-serve" ] || { echo "release-deb: $classic has no backplane-$plat/backplane-serve" >&2; exit 1; }
mv "$work/backplane-$plat" "$root/opt/backplane"
mkdir "$work/desktop"
tar -xzf "$desktop" -C "$work/desktop"
[ -f "$work/desktop/backplane-$plat/runtime/chrome-sandbox" ] || { echo "release-deb: $desktop has no backplane-$plat/runtime/chrome-sandbox" >&2; exit 1; }
mv "$work/desktop/backplane-$plat" "$root/opt/backplane/desktop"
for b in backplane backplane-serve; do
  printf '#!/bin/sh\nBACKPLANE_NO_UPDATE=1 exec /opt/backplane/%s "$@"\n' "$b" > "$root/usr/bin/$b"
  chmod 755 "$root/usr/bin/$b"
done
sed 's|^Exec=.*|Exec=/usr/bin/backplane|' deploy/backplane.desktop > "$root/usr/share/applications/backplane.desktop"
cp assets/icon/backplane-desktop.svg "$root/usr/share/icons/hicolor/scalable/apps/"
for s in 64 128 256 512; do
  mkdir -p "$root/usr/share/icons/hicolor/${s}x$s/apps"
  cp "assets/icon/backplane-desktop-$s.png" "$root/usr/share/icons/hicolor/${s}x$s/apps/backplane-desktop.png"
done
cat > "$root/etc/apparmor.d/backplane" <<'EOF'
# Backplane's desktop app (Electron) needs user namespaces for its sandbox
abi <abi/4.0>,
include <tunables/global>

profile backplane /opt/backplane/desktop/runtime/backplane-desktop flags=(unconfined) {
  userns,

  include if exists <local/backplane>
}
EOF
# the newest glibc the native files ask for
glibc=$(objdump -T "$root/opt/backplane/backplane-serve" "$root/opt/backplane/desktop/runtime/backplane-desktop" 2>/dev/null | grep -o 'GLIBC_[0-9.]*' | sort -uV | tail -1)
glibc=${glibc#GLIBC_}
mkdir -p "$work/control"
cat > "$work/control/control" <<EOF
Package: backplane
Version: $dv
Architecture: $deb
Maintainer: i2cjak <i2cjak@users.noreply.github.com>
Installed-Size: $(du -sk "$root" | cut -f1)
Depends: libc6 (>= ${glibc:-2.31}), libgtk-3-0t64 | libgtk-3-0, libnss3, libnspr4, libgbm1, libasound2t64 | libasound2, libatk-bridge2.0-0t64 | libatk-bridge2.0-0, libcups2t64 | libcups2, libdrm2, libxkbcommon0, libxcomposite1, libxdamage1, libxfixes3, libxrandr2, libxss1, libxtst6, libsecret-1-0, xdg-utils
Recommends: git, kicad, tailscale
Homepage: https://github.com/i2cjak/Backplane
Section: devel
Priority: optional
Description: Agentic hardware development, with live KiCad viewers
 Agent threads (Claude Code, Codex) beside board, schematic and 3D viewers
 that follow the project's files.
EOF
cat > "$work/control/postinst" <<'EOF'
#!/bin/sh
set -e
if [ "$1" = configure ]; then
  chown root:root /opt/backplane/desktop/runtime/chrome-sandbox
  chmod 4755 /opt/backplane/desktop/runtime/chrome-sandbox
  if command -v apparmor_parser >/dev/null 2>&1 && [ -d /sys/kernel/security/apparmor ]; then
    apparmor_parser -r -T -W /etc/apparmor.d/backplane || echo "backplane: the AppArmor profile did not load; the sandbox uses chrome-sandbox" >&2
  fi
  command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database -q /usr/share/applications || true
fi
exit 0
EOF
cat > "$work/control/postrm" <<'EOF'
#!/bin/sh
set -e
if [ "$1" = remove ] || [ "$1" = purge ]; then
  if command -v apparmor_parser >/dev/null 2>&1 && [ -d /sys/kernel/security/apparmor ]; then
    apparmor_parser -R /etc/apparmor.d/backplane 2>/dev/null || true
  fi
fi
exit 0
EOF
printf '/etc/apparmor.d/backplane\n' > "$work/control/conffiles"
chmod 755 "$work/control/postinst" "$work/control/postrm"
tar -czf "$work/control.tar.gz" --owner=0 --group=0 --numeric-owner -C "$work/control" .
# root-owned files are never group- or world-writable
chmod -R go-w "$root"
tar -czf "$work/data.tar.gz" --owner=0 --group=0 --numeric-owner -C "$root" .
printf '2.0\n' > "$work/debian-binary"
name="backplane_${dv}_$deb.deb"
rm -f "$out/$name"
(cd "$work" && ar rc "$out/$name" debian-binary control.tar.gz data.tar.gz)
echo "$out/$name"
