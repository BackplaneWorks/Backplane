#!/bin/bash
# bare-gate-nightly.sh: the bare-install gate every night from this machine (no GitHub runner), on the tip of origin/main.
#   scripts/bare-gate-nightly.sh install THREAD   a user systemd timer (03:30 local) that runs `run THREAD`; the PERSON runs this (agents never
#                                                start timers on the person's machine; docs/bare-install-gate.md)
#   scripts/bare-gate-nightly.sh run THREAD       one run now: scripts/bare-gate.sh --commit origin/main from a worktree of its own; on a FAIL only,
#                                                a message to THREAD (a thread of this machine's hub) with the failed checks and the paper cuts
#   scripts/bare-gate-nightly.sh status | uninstall
# The message goes through the person's own hub as a paired client does (its token, loopback): BACKPLANE_NIGHTLY_HOME names the hub's home
# (default ~/.backplane-gpui-fresh/home). Results: $BP_TMP/bare-nightly/<date>/ (the janitor keeps a day; the last summary is in ~/.cache/bare-nightly.last).
set -uo pipefail
HERE=$(cd "$(dirname "$0")" && pwd); REPO=$(git -C "$HERE" rev-parse --path-format=absolute --git-common-dir | sed 's|/.git$||')
UNIT=bare-gate-nightly
case ${1:-} in
install)
  T=${2:?THREAD}; mkdir -p ~/.config/systemd/user
  printf '[Unit]\nDescription=Backplane bare-install gate (nightly)\n[Service]\nType=oneshot\nNice=10\nExecStart=%s run %s\n' "$REPO/scripts/bare-gate-nightly.sh" "$T" >~/.config/systemd/user/$UNIT.service
  printf '[Unit]\nDescription=Backplane bare-install gate (nightly)\n[Timer]\nOnCalendar=*-*-* 03:30\nPersistent=true\n[Install]\nWantedBy=timers.target\n' >~/.config/systemd/user/$UNIT.timer
  systemctl --user daemon-reload && systemctl --user enable --now $UNIT.timer && systemctl --user list-timers $UNIT.timer --no-pager ;;
uninstall) systemctl --user disable --now $UNIT.timer; rm -f ~/.config/systemd/user/$UNIT.{service,timer}; systemctl --user daemon-reload ;;
status) systemctl --user list-timers $UNIT.timer --no-pager; cat ~/.cache/bare-nightly.last 2>/dev/null ;;
run)
  T=${2:?THREAD}
  # a user service starts with a bare PATH and no shell profile: boat (~/.ascii/bin), bun (~/.bun/bin) and cargo's tools are not on it
  export PATH="$HOME/.ascii/bin:$HOME/.bun/bin:$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
  for t in boat bun git python3; do command -v $t >/dev/null || { echo "$(date -Is) nightly: $t not found on PATH ($PATH); skipped" | tee ~/.cache/bare-nightly.last; exit 1; }; done
  . "$REPO/scripts/lib-bptmp.sh"
  D=$BP_TMP/bare-nightly/$(date +%Y%m%d-%H%M); mkdir -p "$D"
  [ "$(df -BG --output=avail / | tail -1 | tr -dc 0-9)" -ge 20 ] || { echo "nightly: under 20 GB free; skipped" | tee ~/.cache/bare-nightly.last; exit 0; }
  W=$REPO/.backplane/worktrees/bare-nightly
  git -C "$REPO" fetch -q origin && { [ -d "$W" ] || git -C "$REPO" worktree add -q --detach "$W" origin/main; } && git -C "$W" checkout -q --detach origin/main || { echo "$(date -Is) nightly: could not check out origin/main in $W" | tee ~/.cache/bare-nightly.last; exit 1; }
  SHA=$(git -C "$W" rev-parse --short origin/main)
  (cd "$W" && scripts/bare-gate.sh --commit origin/main --out "$D/run") >"$D/gate.log" 2>&1; rc=$?
  { echo "$(date -Is) $SHA rc=$rc $D"; sed -n '/== timings/,$p' "$D/gate.log"; } >~/.cache/bare-nightly.last
  if [ $rc != 0 ]; then
    H=${BACKPLANE_NIGHTLY_HOME:-$HOME/.backplane-gpui-fresh/home}; P=$(cut -d' ' -f2 "$H/hub.lock" 2>/dev/null)
    MSG=$(printf 'Bare-install gate FAILED on %s (nightly, %s). Failed checks and paper cuts:\n%s\nLog: %s' "$SHA" "$(date +%F)" \
      "$(sed -n '/== paper cuts/,$p' "$D/gate.log" | head -40)" "$D/gate.log")
    [ -n "$P" ] && bun "$W/test/bare/hub.mjs" "$P" "$H/token" rpc turn.start "$(python3 -c 'import json,sys;print(json.dumps({"thread":sys.argv[1],"text":sys.argv[2]}))' "$T" "$MSG")" >"$D/post.json" 2>&1 \
      || echo "nightly: could not post to $T (no hub at $H)" >>~/.cache/bare-nightly.last
  fi
  exit $rc ;;
*) sed -n '2,10p' "$0"; exit 2 ;;
esac
