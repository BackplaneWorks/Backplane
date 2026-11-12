#!/bin/bash
# install.sh with no curl, as on a stock Ubuntu desktop: it downloads with wget; with neither it says what to install. No network: a stand-in
# wget "downloads" a made-up archive. Private HOME and prefix under a temp dir; nothing is started (BACKPLANE_NO_DESKTOP, no BACKPLANE_START).
#   test/bare/install_fallback_test.sh        exit 1 on any FAIL
set -u
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
T=$(mktemp -d "${TMPDIR:-/tmp}/bp-instfb.XXXXXX"); trap 'rm -rf "$T"' EXIT; fails=0
ok() { echo "PASS $1"; }; bad() { echo "FAIL $1"; fails=$((fails+1)); }
mkdir -p "$T/pkg/assets/icon" "$T/nocurl" "$T/none"
for f in backplane-hub backplane backplane-gpui; do printf '#!/bin/sh\necho %s\n' $f >"$T/pkg/$f"; chmod +x "$T/pkg/$f"; done
A=$T/backplane-v9.9.9-linux-x64-native.tar.gz; tar -C "$T/pkg" -czf "$A" .
sum=$(sha256sum "$A" | cut -d' ' -f1)
sed -e "s|@REPO@|BackplaneWorks/Backplane|" -e "s|@VERSION@|v9.9.9|" -e "s|@SHA_LINUX_X64@|$sum|" -e "s|@SHA_LINUX_ARM64@|$sum|" -e "s|@SHA_DARWIN_ARM64@||" \
  "$ROOT/deploy/install.sh.in" >"$T/install.sh"
# a PATH with everything this machine has but curl and wget
for d in /usr/local/bin /usr/bin /bin; do for f in "$d"/*; do n=${f##*/}; case $n in curl|wget) continue;; esac; [ -e "$T/none/$n" ] || ln -s "$f" "$T/none/$n"; done; done
cp -a "$T/none/." "$T/nocurl/"
# the stand-in wget: logs its arguments, writes the archive to -O when the URL is the release's
cat >"$T/nocurl/wget" <<EOF
#!/bin/sh
echo "\$*" >>"$T/wget.args"; o=; u=
while [ \$# -gt 0 ]; do case \$1 in -O) o=\$2; shift;; http*) u=\$1;; esac; shift; done
case \$u in https://github.com/BackplaneWorks/Backplane/releases/download/v9.9.9/backplane-v9.9.9-linux-x64-native.tar.gz) cp "$A" "\$o";; *) exit 4;; esac
EOF
chmod +x "$T/nocurl/wget"
run() { env -i HOME="$T/h-$1" PATH="$T/$1" BACKPLANE_NO_DESKTOP=1 sh "$T/install.sh" --quiet >"$T/$1.log" 2>&1; }
mkdir -p "$T/h-nocurl" "$T/h-none"
if run nocurl && [ -x "$T/h-nocurl/.local/share/backplane/builds/v9.9.9/backplane-hub" ]; then ok "no curl: install.sh downloads with wget and installs"; else bad "no curl: $(tail -2 "$T/nocurl.log" | tr '\n' ' ')"; fi
grep -q -- '--https-only' "$T/wget.args" 2>/dev/null && ok "wget is told https only" || bad "wget called without --https-only: $(cat "$T/wget.args" 2>/dev/null)"
if run none; then bad "neither curl nor wget: install.sh went on"; else
  grep -q "curl or wget is needed" "$T/none.log" && ok "neither: install.sh says curl or wget is needed" || bad "neither: said $(tail -1 "$T/none.log")"; fi
echo "fails=$fails"; [ $fails = 0 ]
