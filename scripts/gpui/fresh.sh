#!/bin/bash
# Fresh mode: the Rust hub and the GPUI window on a NEW home of their own. No copy of any store, no link to the live one, no stand-ins, no side-by-side limits:
# real agent turns, approvals and Improvements work as in a normal install. It stays apart from the running Backplane by its own home, port and token
# (the hub refuses to start on a home that looks like a copy of a real store, and the window refuses port 3787 and the live home).
#   scripts/gpui/fresh.sh [start]     start (or reopen the window on) the fresh hub, on your screen ($DISPLAY)
#   scripts/gpui/fresh.sh stop        stop the window and the hub (the home stays: your projects and threads)
#   scripts/gpui/fresh.sh status
#   scripts/gpui/fresh.sh pair        a pairing link for your phone (with FRESH_TAILNET_PORT: the https tailnet address first, the tailnet IP second); single use, short lived
#   scripts/gpui/fresh.sh reset       stop and delete the fresh home (asks first)
#   scripts/gpui/fresh.sh log         where the logs are
# Env: FRESH_DIR (~/.backplane-gpui-fresh), FRESH_PORT (3797), FRESH_MEM (6G, the hub's cap), FRESH_AGENT ("claude:claude-sonnet-5-5": provider:model for new threads;
# empty for the hub's own default), FRESH_TAILNET_PORT (a free port, e.g. 8451: also serve the hub to your phone over HTTPS on your tailnet, `--tailnet-only`; needs a hub
# build that has the flag; `stop` removes that one `tailscale serve --https=<port>` entry, never `serve reset`), BIN (a folder with backplane-hub, backplane-gpui, assets/; default: the dev build `dev-cut.sh` published last).
set -eu
D=${FRESH_DIR:-$HOME/.backplane-gpui-fresh}; PORT=${FRESH_PORT:-3797}; H=$D/home; MEM=${FRESH_MEM:-6G}; AGENT=${FRESH_AGENT-claude:claude-sonnet-5-5}
DEV=${GPUI_DEV_DIR:-$HOME/.backplane-gpui-dev}
if [ -z "${BIN:-}" ]; then [ -f "$DEV/CURRENT" ] && BIN=$DEV/builds/$(head -1 "$DEV/CURRENT") || { echo "no build: set BIN=<folder with backplane-hub and backplane-gpui>" >&2; exit 1; }; fi
die() { echo "fresh: $*" >&2; exit 1; }
alive() { [ -n "${1:-}" ] && kill -0 "$1" 2>/dev/null; }
pid() { cat "$D/$1.pid" 2>/dev/null || true; }
case "$PORT" in 3787|3788) die "port $PORT is Backplane's";; esac
case "$(realpath -m "$D")" in "$HOME/.backplane-bend"*|"$HOME/.backplane-gpui"|"$HOME/.backplane-gpui/"*|"$HOME/.backplane-gpui-copy"*|"$HOME/.backplane-gpui-side"*|"$HOME/.local/share/backplane"*) die "$D is another install's folder";; esac
cmd=${1:-start}
case "$cmd" in
  status) for w in hub client; do p=$(pid $w); if alive "$p"; then echo "$w: running, pid $p"; else echo "$w: not running"; fi; done
          [ -f "$H/token" ] && { printf 'Authorization: Bearer %s\n' "$(cat "$H/token")" | curl -s -m2 -H @- "http://127.0.0.1:$PORT/hello" | head -c 200; echo; }
          [ -f "$D/tailnet.port" ] && { grep -a "on your tailnet:" "$D/hub.log" | tail -1 || echo "tailnet: not up (see $D/hub.log)"; }; exit 0;;
  pair) alive "$(pid hub)" || die "the hub is not running"
        printf 'Authorization: Bearer %s\n' "$(cat "$H/token")" | curl -sf -m5 -X POST -H @- "http://127.0.0.1:$PORT/auth/invitations" | python3 -c "import json,sys;d=json.load(sys.stdin);print('routes:',', '.join(d['routes']));print('code:',d['code']);print('link:',d['link'])" || die "the hub gave no invitation"; exit 0;;
  log) echo "$D/hub.log  $D/client.log"; exit 0;;
  stop) for w in client hub; do p=$(pid $w); alive "$p" && kill "$p"; rm -f "$D/$w.pid"; done
        [ -f "$D/tailnet.port" ] && { tp=$(cat "$D/tailnet.port")
          # only our own entry: it must still proxy to this hub's port; if another process took the port over, it is theirs and stays
          if tailscale serve status 2>/dev/null | awk -v p=":$tp " -v t="http://127.0.0.1:$PORT" 'index($0,"ts.net"p){on=1;next} /^https:/{on=0} on&&$NF==t{f=1} END{exit f?0:1}'; then tailscale serve --https="$tp" off >/dev/null 2>&1 || true; echo "removed the tailscale serve entry on $tp"; else echo "left the tailscale serve entry on $tp alone (it does not proxy to this hub)"; fi
          rm -f "$D/tailnet.port"; }
        echo stopped; exit 0;;
  reset) printf 'Delete %s (all fresh projects and threads)? [y/N] ' "$D"; read -r a; [ "$a" = y ] || exit 1
         for w in client hub; do p=$(pid $w); alive "$p" && kill "$p"; done; sleep 1; rm -rf "$D"; echo deleted; exit 0;;
  start) ;;
  *) sed -n 2,13p "$0"; exit 2;;
esac
[ -x "$BIN/backplane-hub" ] && [ -x "$BIN/backplane-gpui" ] || die "no hub and window in $BIN"
[ -n "${DISPLAY:-}" ] || die "no \$DISPLAY"
mkdir -p "$H"; chmod 700 "$D" "$H"
if [ -s "$H/events.jsonl" ] && [ ! -f "$D/.fresh" ]; then die "$H holds a log that fresh mode did not make: pick another FRESH_DIR"; fi
touch "$D/.fresh"; [ -e "$H/hub.owner" ] || echo "fresh.sh" > "$H/hub.owner"   # this home is ours (the hub refuses an unmarked log over a megabyte as someone else's data)
if ! alive "$(pid hub)"; then
  ! curl -s -m1 -o /dev/null "http://127.0.0.1:$PORT/hello" || die "port $PORT already answers (another hub): set FRESH_PORT"
  if [ ! -s "$H/events.jsonl" ] && [ -n "$AGENT" ]; then   # a new home: new threads start on this provider and model
    t=$(date +%s); printf '{"$":"SettingSet","key":"agent.provider","value":"%s","at":%s}\n{"$":"SettingSet","key":"agent.model","value":"%s","at":%s}\n' "${AGENT%%:*}" "$t" "${AGENT#*:}" "$t" > "$H/events.jsonl"
  fi
  # the hub: your own HOME (the agents' sign-ins are yours), no copy switches, in its own memory-capped scope
  TN=; if [ -n "${FRESH_TAILNET_PORT:-}" ]; then case "$FRESH_TAILNET_PORT" in 443|3787|3788|"$PORT") die "FRESH_TAILNET_PORT $FRESH_TAILNET_PORT is not free to use";; esac; if tailscale serve status 2>/dev/null | grep -q "ts.net:$FRESH_TAILNET_PORT "; then die "tailscale serve port $FRESH_TAILNET_PORT is already used on this machine (tailscale serve status): pick another FRESH_TAILNET_PORT"; fi; TN="--tailnet-only"; echo "$FRESH_TAILNET_PORT" > "$D/tailnet.port"; fi
  ( exec setsid env BACKPLANE_NO_UPDATE=1 ${FRESH_TAILNET_PORT:+BACKPLANE_HTTPS_PORT=$FRESH_TAILNET_PORT} ${BACKPLANE_COMMIT:+BACKPLANE_COMMIT=$BACKPLANE_COMMIT} \
      systemd-run --user --scope -q -p "MemoryMax=$MEM" -p MemorySwapMax=0 -- "$BIN/backplane-hub" --home "$H" --port "$PORT" --host 127.0.0.1 ${TN:---no-tailscale} --foreground \
      > "$D/hub.log" 2>&1 < /dev/null ) &
  echo $! > "$D/hub.pid"
  for i in $(seq 1 100); do curl -sf -m1 "http://127.0.0.1:$PORT/hello" > /dev/null 2>&1 && break; alive "$(pid hub)" || { tail -5 "$D/hub.log"; die "the hub exited"; }; sleep 0.1; done
  echo "hub on 127.0.0.1:$PORT, home $H"
  if [ -n "$TN" ]; then   # the tailnet listener and `tailscale serve` come up just after the hub starts answering: say when they have, or that they have not
    for i in $(seq 1 100); do grep -aq "on your tailnet:" "$D/hub.log" 2>/dev/null && break; alive "$(pid hub)" || break; sleep 0.2; done
    if grep -a "on your tailnet:" "$D/hub.log" 2>/dev/null | tail -1; then echo "pair your phone: $0 pair"; else echo "fresh: the tailnet address did not come up in 20 s: tailscale up? see $D/hub.log" >&2; fi
  fi
fi
alive "$(pid client)" && { echo "window already open (pid $(pid client))"; exit 0; }
( exec setsid env DISPLAY="$DISPLAY" ${BACKPLANE_GPUI_ASSETS:+BACKPLANE_GPUI_ASSETS="$BACKPLANE_GPUI_ASSETS"} BACKPLANE_GPUI_ASSETS="${BACKPLANE_GPUI_ASSETS:-$BIN/assets}" \
    "$BIN/backplane-gpui" --url "http://127.0.0.1:$PORT" --token-file "$H/token" > "$D/client.log" 2>&1 < /dev/null ) &
echo $! > "$D/client.pid"; echo "window open on $DISPLAY (pid $(pid client)); stop with: $0 stop"
