#!/bin/bash
# Japanese on a Linux machine with no CJK font: the client in a bwrap sandbox whose only fonts are DejaVu and Liberation (no fonts-noto-cjk, no user or
# cached fonts; `fc-list :lang=ja` is checked empty), on a private Xvfb, LANG=ja_JP.UTF-8, a fresh HOME. Screenshots the --fonts swatch and the
# window's first screen. Proof for the bundled Noto Sans CJK JP (native/gpui/src/art.rs CJK_FACES); run it with an old client for the "before".
#   test/visual/cjk-nofont.sh BIN OUTDIR NAME     (Xvfb from $XVFB_ROOT, default ~/.local/xvfb-root; i3 and ImageMagick's import on PATH)
set -u
BIN=$(realpath "$1"); OUT=$(realpath -m "$2"); NAME=$3; mkdir -p "$OUT"
X=${XVFB_ROOT:-$HOME/.local/xvfb-root}; XL=${XVFB_LIB:-$HOME/.cache/xvfb-root/usr/lib/x86_64-linux-gnu}
if [ -z "${CJK_INSIDE:-}" ]; then
  F=$(mktemp -d); mkdir -p "$F/home" "$F/fonts"
  exec bwrap --dev-bind / / --tmpfs /usr/share/fonts --ro-bind /usr/share/fonts/truetype/dejavu /usr/share/fonts/truetype/dejavu \
    --ro-bind /usr/share/fonts/truetype/liberation /usr/share/fonts/truetype/liberation --tmpfs /usr/local/share/fonts --tmpfs /var/cache/fontconfig \
    --tmpfs "$HOME/.local/share/fonts" --tmpfs "$HOME/.cache/fontconfig" --tmpfs /tmp --bind "$OUT" "$OUT" \
    --setenv CJK_INSIDE 1 --setenv XVFB_ROOT "$X" --setenv XVFB_LIB "$XL" --setenv HOME "$F/home" --setenv XDG_CACHE_HOME "$F/home/.cache" --setenv XDG_CONFIG_HOME "$F/home/.config" --setenv XDG_DATA_HOME "$F/home/.local" \
    --setenv LANG ja_JP.UTF-8 --unsetenv LC_ALL --unsetenv LC_MESSAGES --unsetenv LANGUAGE --unsetenv WAYLAND_DISPLAY --unsetenv DBUS_SESSION_BUS_ADDRESS --unsetenv I3SOCK \
    bash "$0" "$BIN" "$OUT" "$NAME"
fi
D=:7$((RANDOM % 80 + 10))
{ echo "fc-list :lang=ja (should be empty):"; fc-list :lang=ja; echo "fonts:"; fc-list : family | sort -u; } > "$OUT/$NAME-fc.txt"
LD_LIBRARY_PATH=$XL "$X/usr/bin/Xvfb" $D -screen 0 1280x800x24 -nolisten tcp -xkbdir /usr/share/X11/xkb > "$OUT/$NAME-xvfb.log" 2>&1 & XP=$!; sleep 1.5
printf 'font pango:monospace 8\nnew_window none\nfor_window [class=".*"] border none\ndefault_border none\nhide_edge_borders both\n' > /tmp/i3.conf
DISPLAY=$D i3 -c /tmp/i3.conf > /dev/null 2>&1 & IP=$!; sleep 1
export DISPLAY=$D VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json LP_NUM_THREADS=4 BACKPLANE_NO_UPDATE=1
shot() { local n=$1; shift; "$BIN" "$@" > "$OUT/$NAME-$n.log" 2>&1 & local G=$!; sleep 12; import -window root "$OUT/$NAME-$n.png"; kill $G 2>/dev/null; wait $G 2>/dev/null; }
shot fonts --url http://127.0.0.1:1 --token-file /dev/null --fonts --theme dark
shot window --url http://127.0.0.1:1 --token-file /dev/null --theme dark
kill $IP $XP 2>/dev/null; wait 2>/dev/null
grep -h '"font"' "$OUT/$NAME"-*.log | sort -u
cat "$OUT/$NAME-fc.txt" | head -3
