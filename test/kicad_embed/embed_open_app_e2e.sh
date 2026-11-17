#!/bin/bash
# Open in KiCad (kicad.open: KiCad's own window, beside the selected kicad-cli) on Backplane's own runtime on a computer where it has
# never run: the editor, never KiCad's first-run "KiCad Setup" wizard (bp-win, Nov 17: that wizard took the foreground from Backplane).
#   A1  KiCad's window comes up, and no "KiCad Setup" window at any time; the seed is in the runtime's own config folder (10.99), with
#       KiCad's update checks off and no crash-reporting opt-in
# fork only (a stock KiCad's first run is the person's: not judged here).
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_open_app_e2e: fork only"; exit 0; }
[ "${1:-}" != "" ] && CLIENT_BIN=$1
[ "${2:-}" != "" ] && HUB_BIN=$2
discover_bins; [ -x "$REAL_HOME/.bun/bin/bun" ] && PATH="$REAL_HOME/.bun/bin:$PATH"; discover_kicad; init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT" >/dev/null 2>&1
# a computer where KiCad has never run: none of the harness's KiCad settings
rm -rf "$OUT/fh/.config/kicad" "$OUT/fh/.local/share/kicad"
start_xvfb_and_wm; start_hub
hub_rpc '[{"method":"kicad.open","params":{"thread":"t1","key":""}}]' >"$OUT/open.log" 2>&1 || true
echo "INFO open: $(grep -a '"kicad.open"' "$OUT/open.log" | head -1 | cut -c1-200)"
WIZ=0; WIN=0
for _ in $(seq 240); do
  T=$(xwininfo -root -tree 2>/dev/null)
  echo "$T" | grep -q '"KiCad Setup"' && WIZ=1
  # KiCad's own top level (its class; the title's em dash does not convert under the harness's LC_ALL=C)
  echo "$T" | grep -q '("org.kicad.kicad" "org.kicad.kicad")' && WIN=1
  [ $WIN = 1 ] && break
  sleep 0.5
done
sleep 3; xwininfo -root -tree 2>/dev/null | grep -q '"KiCad Setup"' && WIZ=1
echo "INFO KiCad processes: $(pgrep -u "$(id -u)" -af "kicad" 2>/dev/null | grep -F "$OUT" | cut -c1-160 | tr '\n' '|')"
echo "INFO windows: $(xwininfo -root -tree 2>/dev/null | grep -v 'has no name' | grep -o '0x[0-9a-f]* "[^"]*":.*' | grep -iv 'i3' | head -8 | tr '\n' '|' | cut -c1-600)"
CFG=$(ls -d "$OUT"/fh/.config/kicad/10.99 2>/dev/null || true)
UPD=$(python3 -c 'import json,sys; k=json.load(open(sys.argv[1])); print(int(k["system"]["check_for_kicad_updates"] is False and k["pcm"]["check_for_updates"] is False))' "$CFG/kicad.json" 2>/dev/null || echo 0)
check "A1 Open in KiCad on a fresh computer: KiCad's window, never its first-run wizard; seeded in 10.99, update checks off" "$([ $WIN = 1 ] && [ $WIZ = 0 ] && [ -n "$CFG" ] && [ "$UPD" = 1 ] && echo 1 || echo 0)" "window $WIN, wizard $WIZ, seed ${CFG:-none}, update checks off $UPD; $(grep -o 'kicad open: .*' "$OUT/hub.log" | head -1 | cut -c1-160)"
for p in $(pgrep -u "$(id -u)" -f "$OUT/proj" 2>/dev/null); do ps -o args= -p "$p" | grep -q -- "--embedded" || kill "$p" 2>/dev/null || true; done
finish_suite "embed_open_app_e2e ($EMBED_MODE)"
