#!/bin/sh
# The GPUI client's pairing, for real: a private client (own config, own display) against a running temp-home hub. Every case is judged from the client's log
# and the hub's device list. Runs on the client's machine; the hub may be elsewhere.
#   BASE=http://HOST:PORT TOKEN=<hub token file> GPUI=<client binary> WORK=<a folder> RESTART='<cmd>' EXPIRE='<cmd>' test/e2e/pairing/gpui.sh
# START is how a client runs here: "sh gpui-start.sh NAME SECONDS ARGS..." with RIG naming its config folder (see LAUNCH). A case with no RESTART/EXPIRE is `not run`.
set -u
HERE=$(cd "$(dirname "$0")" && pwd); W=$WORK; L=$W/logs; mkdir -p "$L"
fails=0; passes=0
ok() { if [ "$2" = 0 ]; then echo "pass     $1"; passes=$((passes+1)); else echo "fail     $1"; fails=$((fails+1)); fi; }
hub() { PYTHONDONTWRITEBYTECODE=1 python3 "$HERE/rig.py" "$BASE" "$TOKEN" "$@"; }
steps() { printf '%s' "$2" > "$W/$1.steps.json"; }
inv() { hub invite "$W/inv.json" >/dev/null; python3 -c "import json,sys;print(json.load(open('$W/inv.json'))[sys.argv[1]])" "$1"; }
fresh() { rm -rf "$W/rig/$1"; }
run() { n=$1; s=$2; shift 2; "${LAUNCH_SH:-sh}" "$LAUNCH" "$n" "$s" "$@" >/dev/null 2>&1; }
says() { grep -q "$2" "$L/gpui-$1.out"; echo $?; }
saved() { if ls "$W/rig/$1/config/backplane-gpui/hubs/"*.token >/dev/null 2>&1; then echo 0; else echo 1; fi; }  # (0 saved, 1 not: ls itself says 2 when nothing matches)
live() { hub devices | grep -c " live "; }
addr=${BASE#*://}

echo "route $BASE, client on ${CLIENT_HOST:-$(hostname)} (GPUI client result)"
# a link naming loopback addresses is refused for a remote hub, so BASE must be an address another machine could use (or the cases below pair by address and code)
# link
fresh link; before=$(live); l=$(inv link); steps link "[{\"wait\":1500},{\"pair_fill\":[\"$l\",\"\",\"\"]},{\"click\":\"pair:go\"},{\"wait\":2500}]"
RIG=link run link 9
ok "link pairs and the client connects" "$(says link '"status":"Connected"')"
ok "link: the hub lists one more live device" "$([ "$(live)" = $((before+1)) ]; echo $?)"
# private: mode 600 on Unix; on Windows (Git's sh) the file's ACL: the user, SYSTEM and Administrators only, nothing inherited (icacls)
private() { case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*) a=$(icacls "$(cygpath -w "$1")" 2>/dev/null); echo "$a" | grep -q "$USERNAME" && ! echo "$a" | grep -qE 'Users:|Everyone|Authenticated Users|\(I\)' ;;
  *) [ "$(stat -c %a "$1" 2>/dev/null || stat -f %Lp "$1")" = 600 ] ;;
esac; }
hd="$W/rig/link/config/backplane-gpui"
ok "link: credential and hub saved privately" "$(private "$hd/hubs/$(ls "$hd/hubs" | grep -m1 '\.token$')" && private "$hd/hubs.json"; echo $?)"
sec=$(python3 -c "print('$l'.split('invite=')[1].split('&')[0])"); tok=$(cat "$W/rig/link/config/backplane-gpui/hubs/"*.token 2>/dev/null | head -1)
ok "link: no secret or credential in the client's log" "$(grep -qE "$sec|${tok:-nothing-saved}" "$L/gpui-link.out"; [ $? = 1 ]; echo $?)"
# short code
fresh code; before=$(live); c=$(inv code); steps code "[{\"wait\":1500},{\"pair_fill\":[\"\",\"$BASE\",\"$c\"]},{\"click\":\"pair:go\"},{\"wait\":2500}]"
RIG=code run code 9
ok "short code pairs and the client connects" "$(says code '"status":"Connected"')"
ok "short code: the hub lists one more live device" "$([ "$(live)" = $((before+1)) ]; echo $?)"
# refusals: the words shown, nothing saved
fresh wrong; steps wrong "[{\"wait\":1500},{\"pair_fill\":[\"\",\"$BASE\",\"ZZZZ2222\"]},{\"click\":\"pair:go\"},{\"wait\":1500}]"
RIG=wrong run wrong 6
ok "wrong code says so" "$(says wrong 'That code is not correct')"; ok "wrong code saves nothing" "$([ "$(saved wrong)" = 1 ]; echo $?)"
fresh used; steps used "[{\"wait\":1500},{\"pair_fill\":[\"$l\",\"\",\"\"]},{\"click\":\"pair:go\"},{\"wait\":1500}]"
RIG=used run used 6
ok "used link says so" "$(says used 'already been used')"; ok "used link saves nothing" "$([ "$(saved used)" = 1 ]; echo $?)"
fresh rev; c=$(inv code); i=$(python3 -c "import json;print(json.load(open('$W/inv.json'))['id'])"); hub uninvite "$i" >/dev/null
steps rev "[{\"wait\":1500},{\"pair_fill\":[\"\",\"$BASE\",\"$c\"]},{\"click\":\"pair:go\"},{\"wait\":1500}]"
RIG=rev run rev 6
ok "revoked code says so" "$(says rev 'was revoked')"
if [ -n "${EXPIRE:-}" ]; then
  fresh exp; c=$(inv code); sh -c "$EXPIRE"; hub wait
  steps exp "[{\"wait\":1500},{\"pair_fill\":[\"\",\"$BASE\",\"$c\"]},{\"click\":\"pair:go\"},{\"wait\":1500}]"
  RIG=exp run exp 6
  ok "expired code says so (expiresAt moved back in the registry, hub restarted)" "$(says exp 'invitation expired')"
else echo "not run  expired code: no EXPIRE"; fi
# a link from outside asks first
fresh ask; before=$(live); l=$(inv link); sch=$(python3 -c "import urllib.parse,sys;print('backplane://pair?url='+urllib.parse.quote(sys.argv[1],safe=''))" "$l")
steps ask '[{"wait":1500},{"click":"pair:cancel"},{"wait":800}]'
RIG=ask run ask 6 "$sch"
ok "backplane:// names the hub and asks" "$(says ask '"pair":"ask"')"; ok "Cancel pairs nothing" "$([ "$(saved ask)" = 1 ] && [ "$(live)" = "$before" ]; echo $?)"
steps ask2 '[{"wait":1500},{"click":"pair:yes"},{"wait":2500}]'
RIG=ask run ask2 9 "$sch"
ok "confirming the question pairs" "$(says ask2 '"status":"Connected"')"
# revoke while connected, then re-pair on the same client
rm -f "$W/link.steps.json"; RIG=link run revoke 12 & p=$!; sleep 6; hub revoke-all "$(hub devices | grep -m1 ' live ' | sed 's/ live .*//')" >/dev/null; wait $p
ok "revoked while connected: the client goes to Re-pair" "$(says revoke '"text":"re-pair"')"
l=$(inv link); steps repair "[{\"wait\":1500},{\"pair_fill\":[\"$l\",\"\",\"\"]},{\"click\":\"pair:go\"},{\"wait\":2500}]"
RIG=link run repair 9
ok "a revoked client starts at Re-pair" "$(says repair '"text":"re-pair"')"; ok "re-pair connects" "$(says repair '"status":"Connected"')"
# a hub restart
if [ -n "${RESTART:-}" ]; then
  RIG=link run restart 16 & p=$!; sleep 5; sh -c "$RESTART"; wait $p
  n=$(grep -c '"status":"Connected"' "$L/gpui-restart.out"); ok "reconnects after a hub restart with the same credential" "$([ "$n" -ge 2 ]; echo $?)"
else echo "not run  hub restart: no RESTART"; fi
echo "summary pass=$passes fail=$fails"; [ "$fails" = 0 ]
