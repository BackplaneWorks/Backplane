#!/bin/sh
# The iOS data app's Clone from git journey (test/e2e/ios data-clone) on the simulator against a Rust hub of our own (scripts/mobile/data-hub.sh, stand-in
# agents) that runs with a temp HOME (clones go to its ~/Documents, never the person's) and a stand-in gh (BACKPLANE_GH): signed out at first, its device
# sign-in prints code ABCD-1234 and signs in as "alex" after 20 s, then lists two made-up repositories. The clones are local bare repositories (file://).
# usage: scripts/mobile/ios-clone-journey.sh     Start with work_run {lock: "mac-heavy"}; builds the app on the Mac first (its own folder there).
#   env: HUBBIN (default native/host/target/fast/backplane-hub), PORT (3871), HOST (this machine's tailnet address), SIM_UDID, KEEP=1 keeps the temp folder
set -u
. "$(dirname "$0")/../lib-bptmp.sh"
ROOT=$(cd "$(dirname "$0")/../.." && pwd); MAIN=${MAIN:-$(dirname "$(git -C "$ROOT" rev-parse --path-format=absolute --git-common-dir)")}
M=${M:-$ROOT/build/gpui/mac-ssh}; [ -x "$M" ] || M=$MAIN/build/gpui/mac-ssh
U=${SIM_UDID:-E3BC1B9E-AE4D-498C-97AF-B22FE897B41F}; D=${D:-bp-gpui-ios-clone}; OUT=$ROOT/build/mobile/ios-clone; rm -rf "$OUT"; mkdir -p "$OUT"
PORT=${PORT:-3871}; HOST=${HOST:-$(tailscale ip -4 | head -1)}; HUBBIN=${HUBBIN:-$ROOT/native/host/target/fast/backplane-hub}
H=$BP_TMP/clone-ios-hub-$PORT; rm -rf "$H"; mkdir -p "$H/user/Documents/kit" "$H/repos" "$H/gh"
echo "something else" > "$H/user/Documents/kit/notes.txt"   # ~/Documents/kit holds something: the default becomes kit-2, and "kit" is refused
G="git -c user.email=t@example.com -c user.name=t -c init.defaultBranch=main"
# kit.git: two KiCad projects (the hub sets .backplane.json and holds its note for reading) and ~120 MB that does not compress, so the clone runs long enough to be seen and cancelled
mkdir -p "$H/src/kit" && cd "$H/src/kit" && $G init -q && echo '{}' > kit.kicad_pro && printf '(kicad_pcb (version 20240108) (generator test))\n' > kit.kicad_pcb \
  && printf '(kicad_sch (version 20231120) (generator test))\n' > kit.kicad_sch && mkdir -p old && echo '{}' > old/rev1.kicad_pro && for i in 1 2 3; do head -c 40000000 /dev/urandom > blob$i.bin; done && $G add -A && $G commit -qm kit \
  && $G clone -q --bare "$H/src/kit" "$H/repos/kit.git" || exit 1
mkdir -p "$H/src/notes" && cd "$H/src/notes" && $G init -q && echo "# notes" > README.md && $G add -A && $G commit -qm notes && $G clone -q --bare "$H/src/notes" "$H/repos/notes.git" || exit 1
rm -rf "$H/src"; cd "$ROOT"
cat > "$H/gh/gh" <<EOF
#!/bin/sh
case "\$1 \$2" in
"auth status") if [ -f $H/gh/in ]; then printf 'github.com\n  ✓ Logged in to github.com account alex (keyring)\n  - Active account: true\n'; exit 0; fi; echo "You are not logged into any GitHub hosts." >&2; exit 1 ;;
"auth login") echo "! First copy your one-time code: ABCD-1234"; echo "Open this URL to continue in your web browser: https://github.com/login/device"; sleep 20; touch $H/gh/in; echo "✓ Logged in as alex"; exit 0 ;;
"api user/orgs") exit 0 ;;
"repo list") printf '[{"nameWithOwner":"alex/kit-board","description":"A test board","isPrivate":true,"pushedAt":"2026-11-01T00:00:00Z"},{"nameWithOwner":"alex/notes","description":"","isPrivate":false,"pushedAt":"2026-10-01T00:00:00Z"}]\n' ;;
*) exit 1 ;;
esac
EOF
chmod +x "$H/gh/gh"
HOME=$H/user BACKPLANE_GH=$H/gh/gh H=$H/hub PORT=$PORT HOST=$HOST HUBBIN=$HUBBIN sh "$ROOT/scripts/mobile/data-hub.sh" start || exit 1
TOK=$(cat "$H/hub/home/token")
stop() { H=$H/hub sh "$ROOT/scripts/mobile/data-hub.sh" stop; cp "$H/hub/hub.log" "$OUT/hub.log" 2>/dev/null; [ "${KEEP:-}" = 1 ] || rm -rf "$H"; }
trap stop EXIT
M=$M D=$D MAIN=$MAIN "$ROOT/scripts/mobile/ios-check.sh" | tail -8
printf %s "$TOK" > "$OUT/token"; RSYNC_RSH=$M rsync -a "$OUT/token" hs-mac-mini:$D/token; rm -f "$OUT/token"
RSYNC_RSH=$M rsync -a --delete "$ROOT/test/e2e/ios/" hs-mac-mini:$D/e2e/
$M hs-mac-mini "cd ~/$D/e2e && rm -rf ~/$D/out && SIM_UDID=$U OUT=\$HOME/$D/out HUB_HOST=$HOST HUB_PORT=$PORT HUB_TOKEN_FILE=\$HOME/$D/token CLONE_REPO=file://$H/repos/kit.git CLONE_REPO2=file://$H/repos/notes.git APP=\$HOME/$D/ios/build/dd/Build/Products/Debug-iphonesimulator/Backplane.app sh ./run.sh data-clone; cat ~/$D/out/data-clone.assert.txt; rm -f ~/$D/token; xcrun simctl shutdown $U"
RSYNC_RSH=$M rsync -a hs-mac-mini:$D/out/ "$OUT/"
ls "$H/user/Documents/" > "$OUT/documents.txt" 2>&1
python3 -c "
import json;j=json.load(open('$OUT/data-clone.json'));print(j['status'], j.get('blocked'), j.get('notes'));[print(s['do'],s['ok'],s.get('result_ms')) for s in j['metrics']['steps']]"
