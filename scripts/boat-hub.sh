#!/bin/bash
# The always-on hub on a persistent Boat sandbox (docs/BOAT-HUB.md). Run from the desktop; nothing here touches the desktop's own hub or home.
#
#   scripts/boat-hub.sh create              the sandbox bp-hub (small, no auto-stop, no TTL, no Boat environment: none of the account's stored logins)
#   scripts/boat-hub.sh install [BUILD]     copy a published main build (default: ~/.backplane-gpui-dev/CURRENT) to the box, lay out the guard's builds
#                                           folder, install tailscale, the systemd units (bp-hub.service, bp-hub-health.timer); does not start the hub
#   scripts/boat-hub.sh update [--quiet]    the desktop's newest main build to the box: copied, then run on probation by the box's guard (one failed
#                                           health check goes back); main-build.sh calls this after each publish when the box is set up
#   scripts/boat-hub.sh status              the box, the hub's /healthz, build, memory, and the measured cost
#   scripts/boat-hub.sh cost [--json|--save]  hours used by bp-hub, its rate, the month at this rate, plan hours left; --save writes the JSON to
#                                           ~/.config/backplane/boat-hub/cost.json here and on the box, where each hub's Settings, Network shows it
#   scripts/boat-hub.sh dry-run             a COPY of the desktop's home on the box (BACKPLANE_COPY=1, no tailnet, no agents, no secrets/providers/voice),
#                                           started from the installed build on port 3899: fold time, memory, /healthz; then stopped and deleted
#   scripts/boat-hub.sh tailnet             join the tailnet with the vault secret tailscale-boat-hub (through secret_run: TS_KEY_FILE names the file)
#   scripts/boat-hub.sh start|stop          the box's hub service (the cutover starts it; see docs/BOAT-HUB.md)
#
# Env: BP_HUB_NAME (bp-hub), BP_HUB_TYPE (small), BP_HUB_PORT (3797), DEV (~/.backplane-gpui-dev).
set -euo pipefail
HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
NAME=${BP_HUB_NAME:-bp-hub}; TYPE=${BP_HUB_TYPE:-small}; PORT=${BP_HUB_PORT:-3797}; DEV=${DEV:-$HOME/.backplane-gpui-dev}
CONF=$HOME/.config/backplane/boat-hub; R=/home/user/bp     # R: the guard's prefix on the box (builds/<id>, CURRENT, backplane, home/)
say() { [ -n "${QUIET:-}" ] || echo "boat-hub: $*" >&2; }
die() { echo "boat-hub: $*" >&2; exit 1; }
b() { boat --no-update "$@"; }
py() { local c=$1; shift; python3 -I -c "$c" "$@"; }

# the id of the box named $NAME that is not deleted ("" when none)
box_id() { b list --all --json | py 'import sys,json
s=[x for x in json.load(sys.stdin)["sandboxes"] if x.get("name")==sys.argv[1] and x.get("state") not in ("deleted","deleting")]
print(s[0]["id"] if s else "")' "$NAME"; }
need_box() { ID=$(box_id); [ -n "$ID" ] || die "no sandbox $NAME: scripts/boat-hub.sh create"; }
on() { b ssh "$ID" -- "$@"; }

cmd_create() {
  ID=$(box_id); if [ -n "$ID" ]; then say "$NAME is $ID"; else
    b new --type "$TYPE" --no-auto-stop --no-env --name "$NAME" --json | tail -1 > /dev/null; ID=$(box_id); [ -n "$ID" ] || die "create failed"; fi
  # no TTL and no auto-stop, again (also on a box made by hand)
  b extend "$ID" --no-auto-stop --json > /dev/null
  mkdir -p "$CONF"; echo "$ID" > "$CONF/id"; say "$NAME: $ID (type $TYPE, no auto-stop)"
}

# the units on the box: the guard under systemd (restarts on any exit, starts at boot), and a health check every 2 minutes that restarts the
# service after 3 failed /healthz in a row (the guard itself already restarts a hung hub; this catches a hung guard)
units() { cat <<EOF
=bp-hub.service
[Unit]
Description=Backplane hub (always-on, docs/BOAT-HUB.md)
After=network-online.target tailscaled.service
Wants=network-online.target
[Service]
User=user
WorkingDirectory=/home/user
Environment=HOME=/home/user BACKPLANE_HTTPS_PORT=443 BACKPLANE_NO_UPDATE=1 BACKPLANE_NO_DESKTOP_APP=1 BACKPLANE_NAME=boat-hub
ExecStart=$R/backplane hub run --home $R/home --port $PORT --host 127.0.0.1 --builds $R --log $R/hub.log --pidfile $R/hub.pid -- --tailnet-only --foreground --headless
Restart=always
RestartSec=5
KillMode=mixed
TimeoutStopSec=90
[Install]
WantedBy=multi-user.target
=bp-hub-health.service
[Unit]
Description=Backplane hub health check
[Service]
Type=oneshot
User=user
ExecStart=/bin/bash $R/health.sh
=bp-hub-health.timer
[Unit]
Description=Backplane hub health check every 2 minutes
[Timer]
OnBootSec=3min
OnUnitActiveSec=2min
[Install]
WantedBy=timers.target
EOF
}
health_sh() { cat <<EOF
#!/bin/bash
# bp-hub-health: /healthz on loopback; three misses in a row restart bp-hub (only when the service is meant to run)
f=$R/health.fails; systemctl is-enabled -q bp-hub || exit 0
if curl -fsS -m 10 -o /dev/null http://127.0.0.1:$PORT/healthz; then echo 0 > \$f; exit 0; fi
n=\$(( \$(cat \$f 2>/dev/null || echo 0) + 1 )); echo \$n > \$f; echo "\$(date -Is) healthz miss \$n" >> $R/health.log
[ \$n -ge 3 ] && { echo "\$(date -Is) restarting bp-hub" >> $R/health.log; echo 0 > \$f; sudo -n systemctl restart bp-hub; }
exit 0
EOF
}

# copy build $1 (a folder name under $DEV/builds) to $R/builds/$1 on the box: the hub, the guard, voice, assets, COMMIT and EXPECT. The desktop
# window, CEF and the browser helper stay here (Bub's browser tab on the box is a later step: docs/BOAT-HUB.md).
push_build() { local id=$1 src=$DEV/builds/$1 v c
  [ -x "$src/backplane-hub" ] && [ -x "$src/backplane" ] || die "no hub and guard in $src"
  v=$("$src/backplane-hub" --version 2>/dev/null | awk '{print $NF}'); c=$(head -c 40 "$src/COMMIT")
  if on test -x "$R/builds/$id/backplane-hub" > /dev/null 2>&1; then say "build $id already on the box"; return 0; fi
  on mkdir -p "$R/builds/.in-$id"
  rsync -rlp --checksum -e "$HERE/boat-ssh.sh" --include=/backplane-hub --include=/backplane --include=/backplane-voice --include=/COMMIT \
    --include=/silero_vad.onnx --include='/assets/***' --exclude='*' "$src/" "$ID:$R/builds/.in-$id/" > /dev/null
  printf '{"version":"%s","commit":"%s"}\n' "$v" "$c" | on "cat > $R/builds/.in-$id/EXPECT && $R/builds/.in-$id/backplane-hub --version > /dev/null && mv $R/builds/.in-$id $R/builds/$id"
  say "build $id ($v, $c) on the box"
}

cmd_install() {
  need_box; local id=${1:-$(head -1 "$DEV/CURRENT")}
  on "sudo -n true" || die "no sudo on the box"
  on "command -v tailscale > /dev/null || (curl -fsSL https://tailscale.com/install.sh | sudo -n sh > /dev/null 2>&1); command -v tailscale > /dev/null && sudo -n systemctl enable --now tailscaled > /dev/null 2>&1; mkdir -p $R/builds $R/home"
  push_build "$id"
  # the first build: CURRENT and the guard beside it (an update never replaces the program running it)
  on "[ -s $R/CURRENT ] || echo $id > $R/CURRENT; cp $R/builds/\$(head -1 $R/CURRENT)/backplane $R/backplane.new && mv $R/backplane.new $R/backplane"
  health_sh | on "cat > $R/health.sh && chmod +x $R/health.sh"
  units | py 'import sys,re,base64
for blk in sys.stdin.read().split("\n=")[0:]:
    blk=blk.lstrip("=")
    if not blk.strip(): continue
    name,body=blk.split("\n",1)
    print(name, base64.b64encode(body.encode()).decode())' | while read -r n body; do
      on "echo $body | base64 -d | sudo -n tee /etc/systemd/system/$n > /dev/null" < /dev/null; done
  on "sudo -n systemctl daemon-reload && sudo -n systemctl enable --now bp-hub-health.timer > /dev/null 2>&1"
  mkdir -p "$CONF"; echo "$ID" > "$CONF/id"
  say "installed $id; the hub service is NOT enabled (the cutover does that: docs/BOAT-HUB.md)"
}

cmd_update() {
  [ "${1:-}" = --quiet ] && QUIET=1
  [ -s "$CONF/id" ] || { say "no box set up"; return 0; }
  need_box; local id cur
  id=$(head -1 "$DEV/CURRENT"); cur=$(on "head -1 $R/CURRENT 2>/dev/null" || true)
  [ "$id" = "$cur" ] && { say "box runs $id already"; return 0; }
  push_build "$id"
  if on "systemctl is-active -q bp-hub"; then
    on "BACKPLANE_HOME=$R/home $R/backplane hub update $id" && say "box: $id on probation (the guard goes back to $cur if it fails)"
  else on "echo $id > $R/CURRENT"; say "box: CURRENT is $id (the hub is not running)"; fi
  # keep three builds: CURRENT, and the two before it (the guard pins its own good builds in .pins/)
  cmd_cost --save || true
  on "cd $R/builds && ls -1t | grep -v '^\.' | tail -n +4 | while read -r d; do [ \"\$d\" = \"\$(head -1 $R/CURRENT)\" ] || [ -e .pins/\$d ] || rm -rf -- \"\$d\"; done" || true
}

cmd_cost() {
  need_box
  local u l; u=$(b usage "$ID" --json 2>/dev/null || echo '{}'); l=$(b limits --all --json)
  py 'import sys,json
u=json.loads(sys.argv[1] or "{}"); l=json.loads(sys.argv[2]); t=sys.argv[3]; j=sys.argv[4] in ("--json","--save")
rate=l["overagePrices"]["vmHourDollars"]*{"small":0.5,"default":1,"large":2}.get(t,1)
pi=l.get("planIncluded") or {}
secs=u.get("seconds",0)/(u.get("billingMultiplier") or 1)   # wall-clock seconds (Boat counts seconds at the type multiplier)
import time,datetime
pe=pi.get("periodEnd","")
try: pes=int(datetime.datetime.fromisoformat(pe.replace("Z","+00:00")).timestamp())
except Exception: pes=0
o={"box":sys.argv[5],"type":t,"rate_hour":round(rate,4),"multiplier":{"small":0.5,"default":1,"large":2}.get(t,1),"measured":int(time.time()),"period_end_s":pes,"used_hours":round(secs/3600,2),"used_dollars":round(u.get("dollars",0),3),"since":u.get("since",""),
   "month_hours":720,"month_dollars":round(720*rate,2),"plan_hours_left":round(pi.get("vmHoursRemaining",0),1),"plan_hours":round(pi.get("vmHours",0),1),
   "period_end":pi.get("periodEnd","")}
if j: print(json.dumps(o))
else: print("%(box)s %(type)s: $%(rate_hour)s/h, %(used_hours)s h so far ($%(used_dollars)s); 24/7 is $%(month_dollars)s a month; plan: %(plan_hours_left)s of %(plan_hours)s h left until %(period_end)s"%o)' \
    "$u" "$l" "$TYPE" "${1:-}" "$NAME" > "$CONF/cost.json.new" || { rm -f "$CONF/cost.json.new"; die "cannot measure"; }
  if [ "${1:-}" = --save ]; then
    mv "$CONF/cost.json.new" "$CONF/cost.json"
    # the box shows the same in its Settings (it has no Boat login of its own to measure with)
    on "mkdir -p ~/.config/backplane/boat-hub && cat > ~/.config/backplane/boat-hub/cost.json.new && mv ~/.config/backplane/boat-hub/cost.json.new ~/.config/backplane/boat-hub/cost.json" < "$CONF/cost.json" || say "could not copy the cost to the box"
    QUIET=1
  else cat "$CONF/cost.json.new"; rm -f "$CONF/cost.json.new"; fi
}

cmd_status() {
  need_box; b info "$ID" --json | py 'import sys,json; s=json.load(sys.stdin)["sandbox"]; print("box %s %s %s, %s vCPU %s GB, ttl %s, archiveAfter %s, health %s"%(s["name"],s["id"],s["state"],s["vcpu"],s["memoryGB"],s["ttlSeconds"],s["archiveAfter"],s["health"]))'
  on "echo service: \$(systemctl is-enabled bp-hub 2>/dev/null)/\$(systemctl is-active bp-hub 2>/dev/null) health-timer: \$(systemctl is-active bp-hub-health.timer); echo build: \$(cat $R/CURRENT 2>/dev/null); curl -fsS -m 5 http://127.0.0.1:$PORT/healthz 2>/dev/null | head -c 300; echo; tailscale status --self --peers=false 2>/dev/null | head -1; free -m | sed -n 2p" || true
  cmd_cost; cmd_cost --save
}

cmd_tailnet() {
  need_box; [ -n "${TS_KEY_FILE:-}" ] && [ -s "$TS_KEY_FILE" ] || die "run through secret_run with files {TS_KEY_FILE: tailscale-boat-hub}"
  # the key goes to the box on stdin into a private file, is used once, and is removed; never on a command line
  on "umask 077; cat > /tmp/.tskey" < "$TS_KEY_FILE"
  on "sudo -n tailscale up --auth-key=file:/tmp/.tskey --hostname=boat-hub --operator=user --accept-dns=true; rc=\$?; rm -f /tmp/.tskey; exit \$rc"
  on "tailscale status --self --peers=false | head -1"
}

cmd_start() { need_box; on "sudo -n systemctl enable --now bp-hub" && say "bp-hub started"; }
cmd_stop() { need_box; on "sudo -n systemctl disable --now bp-hub" && say "bp-hub stopped and disabled"; }

# a copy of the desktop's home, run on the box as a copy (docs/COPY-GUARD.md): proves the build folds the real log on this size, and measures it
cmd_dry_run() {
  need_box; local H=$HOME/.backplane-gpui-fresh/home D=/home/user/bp-dry cur
  cur=$(on "head -1 $R/CURRENT") || die "install first"
  on "mkdir -p $D/home"
  say "copying the home (no secrets, providers, voice, browser profile, checkpoints) to $D"
  rsync -rlp --delete -e "$HERE/boat-ssh.sh" --exclude=/secrets --exclude=/providers --exclude=/voice --exclude=/browser-profile --exclude=/ckpt \
    --exclude=/guard --exclude=/hub.lock --exclude=/token --exclude='/events.jsonl.before-*' --exclude=/quarantine --exclude=/devices-v1.json \
    "$H/" "$ID:$D/home/" > /dev/null
  on "set -e; cd $D; mkdir -p agents; rm -f hub.log
    t0=\$(date +%s.%N)
    BACKPLANE_COPY=1 BACKPLANE_AGENTS=$D/agents BACKPLANE_NO_UPDATE=1 setsid $R/builds/$cur/backplane-hub --home $D/home --port 3899 --host 127.0.0.1 --no-tailscale --foreground --headless > hub.log 2>&1 &
    pid=\$!; echo \$pid > hub.pid; ok=; for i in \$(seq 1 300); do
      h=\$(curl -fsS -m 3 http://127.0.0.1:3899/healthz 2>/dev/null || true)
      case \"\$h\" in *'\"folding\":0'*'\"phase\":\"policy state booted\"'*|*'\"phase\":\"policy state booted\"'*'\"folding\":0'*) ok=1; break;; *) sleep 1;; esac; kill -0 \$pid 2>/dev/null || break; done
    t1=\$(date +%s.%N); echo \"healthz: \$h\"; echo \"ready after: \$(echo \"\$t1 - \$t0\" | bc) s\"
    sleep 20; echo \"rss after 20 s: \$(ps -o rss= -p \$pid | awk '{printf \"%d MB\", \$1/1024}')\"; free -m | sed -n 2p
    kill \$pid 2>/dev/null; wait \$pid 2>/dev/null || true; echo '--- hub.log (last 15)'; tail -15 hub.log; [ -n \"\$ok\" ]"
  local rc=$?; on "rm -rf $D"; say "dry run copy removed from the box"; return $rc
}

case "${1:-}" in
  create) cmd_create;; install) shift; cmd_install "$@";; update) shift; cmd_update "$@";; status) cmd_status;; cost) shift; cmd_cost "$@";;
  dry-run) cmd_dry_run;; tailnet) cmd_tailnet;; start) cmd_start;; stop) cmd_stop;;
  *) sed -n '2,17p' "$0" | sed 's/^# \{0,1\}//'; exit 2;;
esac
