#!/bin/bash
# The outside watchdog: the last layer, for when the hub, its guard or the watcher cannot help themselves (docs/HUB-RESILIENCE.md). Bub runs inside the hub, so nothing inside it can bring it back.
# It shares nothing with the hub or fresh-watch.sh: a systemd --user timer runs one `check` every 30 s, and all it keeps between checks is a small state folder.
#   scripts/hub-watchdog.sh check       one look; what the timer runs
#   scripts/hub-watchdog.sh status      what it last saw, and whether the timer is on
#   scripts/hub-watchdog.sh install     write the user units and turn the timer on
#   scripts/hub-watchdog.sh uninstall   turn it off and remove the units
# What a check does: /hello on the hub, and the watcher's process.
#   A hub still starting is left to its guard, for WATCHDOG_BOOT seconds. A person's `fresh.sh stop` is respected.
#   After WATCHDOG_TRIES failed checks: `fresh.sh stop`, `fresh.sh start`. If the hub still does not answer, `dev-cut.sh back` and the same again.
#   A dead watcher is started again. Each restart waits longer than the one before (WATCHDOG_BACKOFF, doubling, at most WATCHDOG_BACKOFF_MAX), so it can never become a restart loop.
#   The hub's disk and the temp folder: either nearly full (under 1 GiB, or under 5 % and 10 GiB: the guard's rule) is logged and put on the desktop once, and again when there is room.
#   A full /tmp (a RAM tmpfs shared by every agent) fails agents' writes with ENOSPC or EDQUOT just as a full disk does; restarting the hub does not free it.
#   A start that failed on the accounts ("could not start claude: Accounts are not available", a profile to reconnect): the hub retries those itself, so any in the log
#   is a guard failure. Each one is logged, put on the desktop once a batch, and the hub is asked to start those turns again (POST /dev/account-retry, the token on stdin).
#   Every action goes to $D/watchdog.log.
# Env: FRESH_DIR, FRESH_PORT, GPUI_DEV_DIR, FRESH_TAILNET_PORT, DISPLAY (as fresh.sh), WATCHDOG_TRIES (3), WATCHDOG_BOOT (900 s), WATCHDOG_WAIT (120 s for a restart to answer), WATCHDOG_BACKOFF (120 s),
#      WATCHDOG_BACKOFF_MAX (3600 s), WATCHDOG_CALM (600 s of health before the backoff resets), WATCHDOG_WATCHER (0: do not look after fresh-watch.sh).
set -u
unset BACKPLANE_AGENT_OWNER   # the hub reaps what carries its mark: nothing started from here may
HERE=$(cd "$(dirname "$0")" && pwd)
D=${FRESH_DIR:-$HOME/.backplane-gpui-fresh}; PORT=${FRESH_PORT:-3797}; DEV=${GPUI_DEV_DIR:-$HOME/.backplane-gpui-dev}
TRIES=${WATCHDOG_TRIES:-3}; BOOT=${WATCHDOG_BOOT:-900}; WAIT=${WATCHDOG_WAIT:-120}; BACK=${WATCHDOG_BACKOFF:-120}; BACKMAX=${WATCHDOG_BACKOFF_MAX:-3600}; CALM=${WATCHDOG_CALM:-600}; WATCHER=${WATCHDOG_WATCHER:-1}
FRESH=${WATCHDOG_FRESH:-$HERE/fresh.sh}; CUT=${WATCHDOG_CUT:-$HERE/dev-cut.sh}; WATCH=${WATCHDOG_WATCH:-$HERE/fresh-watch.sh}
ST=$D/.watchdog; LOG=$D/watchdog.log; UNITS=${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user
log() { printf '%s %s\n' "$(date '+%F %T')" "$*" >> "$LOG"; }
get() { cat "$ST/$1" 2>/dev/null || echo "${2:-0}"; }
put() { printf '%s\n' "$2" > "$ST/$1"; }
now() { date +%s; }
hello() { curl -sf -m5 -o /dev/null "http://127.0.0.1:$PORT/hello"; }
# the hub's own word, when /hello has none: ok, degraded, starting, hung, down, or nothing at all
health() { curl -s -m5 "http://127.0.0.1:$PORT/healthz" 2>/dev/null | python3 -I -c 'import json,sys
try: print(json.loads(sys.stdin.read()).get("status",""))
except ValueError: pass'; }
# space: "PATH FREE TOTAL" in bytes
space() { df -B1 --output=target,avail,size "$1" 2>/dev/null | awk 'NR==2 {print $1, $2, $3}'; }
low() { [ "$1" -lt 1073741824 ] || { [ $(( $1 * 20 )) -lt "$2" ] && [ "$1" -lt 10737418240 ]; }; }   # the guard's rule (plan.rs low_space)
space_check() {
  local k p mnt free total
  for k in home temp; do
    if [ "$k" = home ]; then p=$D; else p=${TMPDIR:-/tmp}; fi
    read -r mnt free total <<< "$(space "$p")"; [ -n "${total:-}" ] || continue
    if low "$free" "$total"; then
      [ "$(get "low-$k")" = 1 ] && continue; put "low-$k" 1
      local m="$mnt is nearly full: $((free/1048576)) MB free of $((total/1073741824)) GB"; [ "$k" = temp ] && m="$m; agents' temporary files will fail to write (ENOSPC, EDQUOT)"
      log "$m"; command -v notify-send > /dev/null && notify-send -a Backplane "Backplane: $mnt nearly full" "$m" 2>/dev/null &
    elif [ "$(get "low-$k")" = 1 ]; then put "low-$k" 0; log "room again on $mnt: $((free/1073741824)) GB free"; fi
  done
}
# account_check: the spawn failures on the accounts written since the last look (the log's offset in $ST/acct-offset; the first look starts at the end, so history is not news)
ACCT_RE='"kind":"spawn","summary":"could not start [a-z]*: [^"]*(Accounts are not available|Reconnect and select this account|status check did not finish|select this account again)'
account_check() {
  local ev=$D/home/events.jsonl off size new n ths r
  [ -f "$ev" ] || return 0
  size=$(stat -c %s "$ev" 2>/dev/null || echo 0); off=$(get acct-offset -1)
  if [ "$off" = -1 ] || [ "$size" -lt "$off" ]; then put acct-offset "$size"; return 0; fi
  [ "$size" -gt "$off" ] || return 0
  new=$(tail -c +"$((off + 1))" "$ev" | head -c "$((size - off))" | grep -E "$ACCT_RE")
  put acct-offset "$size"
  [ -n "$new" ] || return 0
  n=$(printf '%s\n' "$new" | wc -l); ths=$(printf '%s\n' "$new" | grep -o '"thread":"[^"]*"' | cut -d'"' -f4 | sort -u | tr '\n' ' ')
  log "account guard: $n start(s) failed on the accounts: ${ths% }; asking the hub to start them again"
  command -v notify-send > /dev/null && notify-send -a Backplane "Backplane: a turn could not start on an account" "$n start(s): ${ths% }. Retrying." 2>/dev/null &
  r=$(printf 'Authorization: Bearer %s\n' "$(cat "$D/home/token" 2>/dev/null)" | curl -s -m10 -X POST -H @- "http://127.0.0.1:$PORT/dev/account-retry" 2>/dev/null) || r="no answer"
  log "account guard: the hub answered: $r"
}
watcher_up() { pgrep -f "fresh-watch\.sh" > /dev/null; }
relaunching() { # the watcher is swapping builds right now: its own status says so, and it is fresh
  local f=$DEV/STATUS; [ -f "$f" ] && grep -q '"phase":"relaunching"' "$f" && [ $(( $(now) - $(stat -c %Y "$f") )) -lt "$WAIT" ]
}
restart_hub() {
  log "restarting: fresh.sh stop, fresh.sh start"
  FRESH_RESTART=1 "$FRESH" stop >> "$LOG" 2>&1
  "$FRESH" start >> "$LOG" 2>&1 || log "fresh.sh start failed"
  local i=0; while [ "$i" -lt "$WAIT" ]; do if hello; then log "the hub answers again"; return 0; fi; sleep 1; i=$((i+1)); done
  log "the hub did not answer in $WAIT s"; return 1
}
start_watcher() {
  log "fresh-watch.sh is not running: starting it"
  if command -v systemd-run > /dev/null && [ -z "${WATCHDOG_NO_SCOPE:-}" ]; then
    setsid systemd-run --user --scope -q -- "$WATCH" >> "$D/watch.log" 2>&1 < /dev/null &
  else setsid "$WATCH" >> "$D/watch.log" 2>&1 < /dev/null & fi
}

check() {
  mkdir -p "$ST"
  # one check at a time: a restart can outlast the timer. flock(1) holds the lock itself (its descriptor is close-on-exec), so nothing a check starts carries it.
  # It was `exec 9> lock; flock -n 9`: fresh-watch.sh started from a check inherited descriptor 9, and so did the hub, the window and every agent it started after;
  # each later check found the lock taken and left without a word (silent from 2026-11-08 21:20 to 11-09). The old file `lock` may still be held by those: a new name.
  [ -n "${WATCHDOG_LOCKED:-}" ] || exec env WATCHDOG_LOCKED=1 flock -n -E 0 "$ST/check.lock" "$0" check
  unset WATCHDOG_LOCKED
  if [ -e "$D/.stopped" ]; then [ "$(get said-stopped)" = 1 ] || { log "the hub was stopped by hand: leaving it stopped"; put said-stopped 1; }; put fails 0; exit 0; fi
  put said-stopped 0
  space_check
  if [ "$WATCHER" = 1 ] && ! watcher_up; then
    if [ "$(now)" -ge "$(get watcher-after)" ]; then start_watcher; put watcher-after "$(( $(now) + BACK ))"; fi
  fi
  if hello; then
    [ "$(get fails)" = 0 ] || log "the hub answers again"
    # a healthy check says so on the first one and then hourly, so the log shows the watchdog is alive without a line every 30 s
    [ $(( $(now) - $(get said-ok) )) -lt 3600 ] || { log "healthy: /hello on $PORT answers ($(health)), fresh-watch.sh $(watcher_up && echo alive || echo not running)"; put said-ok "$(now)"; }
    put fails 0; put started-at 0
    account_check
    if [ "$(get step)" != 0 ] && [ $(( $(now) - $(get last-restart) )) -ge "$CALM" ]; then log "calm for $CALM s: backoff reset"; put step 0; put after 0; fi
    return
  fi
  relaunching && return
  if [ "$(health)" = starting ]; then # its guard is on it; a start that never ends is still a failure
    [ "$(get started-at)" != 0 ] || put started-at "$(now)"
    [ $(( $(now) - $(get started-at) )) -lt "$BOOT" ] && return
  fi
  local f=$(( $(get fails) + 1 )); put fails "$f"
  [ "$f" -ge "$TRIES" ] || { [ "$f" != 1 ] || log "/hello on $PORT does not answer (health: $(health))"; return; }
  if [ "$(now)" -lt "$(get after)" ]; then return; fi
  local step; step=$(get step); local wait=$(( BACK << (step > 10 ? 10 : step) )); [ "$wait" -le "$BACKMAX" ] || wait=$BACKMAX
  put step "$((step+1))"; put last-restart "$(now)"; put after "$(( $(now) + wait ))"
  log "/hello failed $f checks: restart $((step+1)), the next no sooner than $wait s from now"
  if restart_hub; then put fails 0; return; fi
  log "the restarted build still fails: dev-cut.sh back"
  "$CUT" back >> "$LOG" 2>&1 || log "no build to go back to"
  if restart_hub; then put fails 0; else log "still down; next try after the backoff"; fi
}

case "${1:-}" in
  check) check;;
  status) mkdir -p "$ST"; echo "fails: $(get fails), restarts in a row: $(get step), next no sooner than: $( [ "$(get after)" -gt "$(now)" ] && date -d "@$(get after)" || echo now)"; systemctl --user is-active backplane-hub-watchdog.timer 2>/dev/null; tail -5 "$LOG" 2>/dev/null;;
  install) mkdir -p "$UNITS"
    for u in service timer; do sed -e "s|@SCRIPT@|$HERE/hub-watchdog.sh|g" -e "s|@TAILNET@|${FRESH_TAILNET_PORT:-8451}|g" -e "s|@DISPLAY@|${DISPLAY:-:0}|g" -e "s|@MEM@|${FRESH_MEM:-20G}|g" "$HERE/systemd/backplane-hub-watchdog.$u" > "$UNITS/backplane-hub-watchdog.$u"; done
    systemctl --user daemon-reload && systemctl --user enable --now backplane-hub-watchdog.timer && { mkdir -p "$ST"; put said-ok 0; log "installed: a check every 30 s ($HERE/hub-watchdog.sh)"; echo "the watchdog is on: log in $LOG"; };;
  uninstall) systemctl --user disable --now backplane-hub-watchdog.timer; log "uninstalled"; rm -f "${UNITS:?}/backplane-hub-watchdog.service" "${UNITS:?}/backplane-hub-watchdog.timer"; systemctl --user daemon-reload; echo "the watchdog is off";;
  *) sed -n 2,14p "$0"; exit 2;;
esac
