#!/bin/bash
# Remote operation from other machines: a guarded hub held by `feed.sh hold` on one machine, driven by clients on others: Check now (update), Restart hub and
# Roll back, each judged on the hub's machine (the guard's status.json and BAD) and by /hello from here.
#   test/update/clients.sh HUB CLIENT...
#     HUB: local | mac | backplane      where the hub runs (its feed.sh and bin: see below)
#     CLIENT: gpui@backplane | gpui@local | phone      the Linux GPUI client on that machine (private Xvfb rig), or the Pixel (PHONE, default 52220DLAQ001FC)
# Each client gets a hub of its own (a rollback marks v0.0.2 bad). Runs on the desktop. Env: GPUI_LOCAL (the client binary here), APK (installed first when given).
set -u
HERE=$(cd "$(dirname "$0")" && pwd); ROOT=$HERE/../..; HUB=${1:?usage: clients.sh HUB CLIENT...}; shift
MACSSH=${MACSSH:-/home/h/Documents/i2cjak/backplane-gpui/build/gpui/mac-ssh hs-mac-mini}
case $HUB in
  local) RUN="bash -c"; IP=$(ip -4 -o route get 192.168.4.1 | sed -n 's/.* src \([0-9.]*\).*/\1/p'); SRC=$ROOT; BIN=$ROOT/build/e2e-bin; PRE=;;
  mac) RUN="$MACSSH"; IP=${MAC_IP:-10.0.0.166}; SRC='$HOME/bp-remote-op/src'; BIN='$HOME/bp-remote-op/bin'; PRE='export PATH=/opt/homebrew/bin:$PATH;';;
  backplane) RUN="ssh -o BatchMode=yes h@backplane"; IP=${BP_IP:-192.168.4.23}; SRC='$HOME/bp-remote-op/src'; BIN='$HOME/bp-remote-op/bin'; PRE='export PATH=$HOME/.cargo/bin:$PATH;';;
  *) echo "HUB: local, mac or backplane"; exit 2;;
esac
W=$ROOT/build/lane/clients-$HUB; mkdir -p "$W"; PORT=${UPD_PORT:-3960}; PASS=0; FAILS=(); PHONE=${PHONE:-52220DLAQ001FC}
r() { $RUN "$PRE $*"; }
ok() { PASS=$((PASS+1)); printf 'PASS %-16s %s\n' "$CUR" "$*"; }
no() { FAILS+=("$CUR"); printf 'FAIL %-16s %s\n' "$CUR" "$*"; }
until_() { local n=$1; shift; local i=0; while [ "$i" -lt $((n/2)) ]; do if "$@"; then return 0; fi; sleep 2; i=$((i+1)); done; return 1; }

# --- the held hub
hold() {
  PORT=$((PORT+1)); : > "$W/$CUR.hold"
  $RUN "$PRE cd $SRC && UPD_HOST=$IP UPD_PORT=$PORT UPD_DIR=$(dirname "$BIN") bash test/update/feed.sh $BIN hold" > "$W/$CUR.hold" 2>&1 & HP=$!
  local i; for i in $(seq 120); do D=$(sed -n 's/^READY \([^ ]*\) .*/\1/p' "$W/$CUR.hold"); [ -n "$D" ] && break; sleep 1; done
  [ -n "$D" ] || { no "the hub never came up: $(tail -3 "$W/$CUR.hold")"; return 1; }
  A=$IP:$PORT; r cat "$D/home/token" > "$W/$CUR.token"; chmod 600 "$W/$CUR.token"
}
unhold() { r touch "$D/stop" 2>/dev/null; wait "$HP" 2>/dev/null; }
st() { r "python3 -c 'import json,sys
try: print(json.load(open(sys.argv[1])).get(sys.argv[2],\"\"))
except Exception: pass' $D/home/guard/status.json $1"; }
bad() { r "grep -q '^$1' $D/builds/BAD" 2>/dev/null; }
badwhy() { r "grep '^$1' $D/builds/BAD | cut -f2" 2>/dev/null; }
hello() { curl -s -m3 "http://$A/hello" | python3 -I "$HERE/cbor_hello.py"; }
on() { [ "$(st build)" = "$1" ] && [ "$(st stable)" = True ] && [ "$(hello | cut -d' ' -f1)" = "${1#v}" ]; }
link() { curl -s -m5 -X POST -H "Authorization: Bearer $(cat "$W/$CUR.token")" "http://$A/auth/invitations" | python3 -I -c 'import json,sys;print(json.loads(sys.stdin.read(),strict=False)["link"])'; }

# --- the GPUI client: a private rig (own Xvfb, D-Bus, config) on its machine, steps from a file; its config (the pairing) kept between runs
gpui() { # gpui NAME SECONDS STEPS_JSON
  local n=$1 s=$2; printf '%s' "$3" > "$W/$n.steps.json"
  if [ "$CH" = local ]; then XVFB=${XVFB:-} WORK=$W/rig RIG=$CUR GPUI=$GPUI_LOCAL sh "$HERE/gpui-rig.sh" "$n" "$s" "$W/$n.steps.json" > "$W/gpui-$n.out" 2>&1
  else ssh -o BatchMode=yes "h@$CH" "mkdir -p ~/bp-remote-op/rig"; scp -q "$HERE/gpui-rig.sh" "$W/$n.steps.json" "h@$CH:bp-remote-op/rig/"
    ssh -o BatchMode=yes "h@$CH" "I3ROOT=\$HOME/.cache/e2e-pair/wm/root WORK=\$HOME/bp-remote-op/rig RIG=$CUR GPUI=\$HOME/bp-remote-op/bin/backplane-gpui sh bp-remote-op/rig/gpui-rig.sh $n $s bp-remote-op/rig/$n.steps.json" > "$W/gpui-$n.out" 2>&1; fi
}
# a palette command, pressed twice (the second press inside its confirm window): what a person does from the Commands box
twice() { local k="{\"keys\":\"ctrl-k\"},{\"wait\":700},{\"keys\":\"$1\"},{\"wait\":500},{\"keys\":\"enter\"},{\"wait\":900}"; echo "[{\"wait\":5000},$k,$k,{\"wait\":3000}]"; }
spell() { echo "$1" | sed 's/./& /g; s/   / space /g' | sed 's/  */ /g'; }

client_gpui() {
  rm -rf "$W/rig/$CUR"; [ "$CH" = local ] || ssh -o BatchMode=yes "h@$CH" "rm -rf ~/bp-remote-op/rig/rig/$CUR"
  gpui pair 12 "[{\"wait\":1500},{\"pair_fill\":[\"$(link)\",\"\",\"\"]},{\"click\":\"pair:go\"},{\"wait\":4000}]"
  if ! grep -q '"status":"Connected"' "$W/gpui-pair.out"; then no "the client did not pair: $(grep -o 'error[^,]*' "$W/gpui-pair.out" | head -2)"; return; fi
  until_ 90 r test -s "$D/home/update/releases.json"   # the hub's own first check has passed: from here only Check now looks before the hour
  r touch "$D/publish"; sleep 2
  gpui check 14 '[{"wait":5000},{"action":["update-check",""]},{"wait":5000}]'
  if ! until_ 180 on v0.0.2; then no "Check now from the client did not install v0.0.2 (on $(st build))"; return; fi
  local pid; pid=$(st hubPid)
  gpui restart 22 "$(twice "$(spell 'restart hub')")"
  if ! until_ 120 eval '[ "$(st hubPid)" != "$pid" ] && on v0.0.2'; then no "Restart hub from the client did not restart v0.0.2 (pid $pid -> $(st hubPid))"; return; fi
  gpui rollback 22 "$(twice "$(spell 'roll back')")"
  if ! until_ 120 bad v0.0.2; then no "Roll back hub from the client did not mark v0.0.2 bad (on $(st build))"; return; fi
  if ! until_ 120 on v0.0.1; then no "not back on v0.0.1"; return; fi
  ok "from $CH to $HUB ($A): paired, Check now installed v0.0.2, Restart hub restarted it, Roll back hub: $(badwhy v0.0.2), on v0.0.1"
}

client_phone() {
  exec 9> "$HOME/.cache/pixel-$PHONE.lock"; echo "  waiting for the phone's lock"; flock 9
  local P="python3 -I $HERE/phone.py $PHONE" PKG=dev.backplane.mobile.gpui SH=$W/phone-$CUR; mkdir -p "$SH"
  want_() { local i; for i in $(seq ${2:-20}); do $P has "$1" && return 0; sleep 1; done; $P shot "$SH/fail.png"; return 1; }
  [ -z "${APK:-}" ] || adb -s "$PHONE" install -r "$APK" > /dev/null
  local L; L=$(link | python3 -I -c 'import sys,urllib.parse;print(urllib.parse.quote(sys.stdin.read().strip(),safe=""))')
  adb -s "$PHONE" shell pm clear $PKG > /dev/null; adb -s "$PHONE" shell pm grant $PKG android.permission.POST_NOTIFICATIONS > /dev/null 2>&1; adb -s "$PHONE" shell am force-stop $PKG   # (a cleared app would ask about notifications over the pairing prompt)
  adb -s "$PHONE" shell "am start -W -n $PKG/dev.backplane.mobile.DataActivity -d 'bpgpui://pair?url=$L'" > /dev/null
  if ! { want_ "Pair with this hub?" 30 && sleep 1 && $P tap Pair exact -1; }; then no "the app did not offer to pair"
  elif ! { for _ in $(seq 30); do $P has "Pair with this hub?" || break; sleep 1; done; want_ Settings 30; }; then no "the app did not pair"
  elif ! { $P tap Settings && want_ UPDATES && $P seek "Check now"; }; then no "no Check now"
  else
    until_ 90 r test -s "$D/home/update/releases.json"; r touch "$D/publish"; sleep 2; $P tap "Check now"
    if ! until_ 180 on v0.0.2; then no "Check now on the phone did not install v0.0.2 (on $(st build))"
    else
      local pid; pid=$(st hubPid); $P shot "$SH/1-updated.png"
      adb -s "$PHONE" shell input keyevent KEYCODE_BACK; sleep 1
      if ! { $P tap Settings && want_ UPDATES && $P tap Hub exact && want_ "Restart hub" 30 && $P tap "Restart hub" exact && sleep 1 && $P tap "Restart hub" exact -1; }; then no "could not press Restart hub"
      elif ! until_ 120 eval '[ "$(st hubPid)" != "$pid" ] && on v0.0.2'; then no "Restart hub from the phone did not restart v0.0.2"
      elif ! { want_ "Roll back to" 60 && $P tap "Roll back to" && sleep 1 && $P tap "Roll back to" -1; }; then no "could not press Roll back"
      elif ! { until_ 120 bad v0.0.2 && until_ 120 on v0.0.1; }; then no "Roll back from the phone did not go back (on $(st build))"
      else $P shot "$SH/2-rolledback.png"; ok "from the Pixel to $HUB ($A, LAN): Check now installed v0.0.2, Restart hub restarted it, Roll back: $(badwhy v0.0.2), on v0.0.1"; fi
    fi
  fi
  adb -s "$PHONE" shell am force-stop $PKG; exec 9>&-
}

for c in "$@"; do
  CUR=$c; CH=${c#*@}
  hold || continue
  case $c in gpui@*) client_gpui;; phone) client_phone;; *) no "no such client";; esac
  unhold
done
echo "clients on $HUB: $PASS passed, ${#FAILS[@]} failed${FAILS:+: ${FAILS[*]}}"; [ ${#FAILS[@]} = 0 ]
