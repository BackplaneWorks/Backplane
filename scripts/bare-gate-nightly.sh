#!/bin/bash
# bare-gate-nightly.sh: the bare-install gate every night from this machine (no GitHub runner), on the tip of origin/main; then, when nightly.yml has
# published a new nightly, the update check (scripts/nightly-update-gate.sh: the nightly before it updated to it on a clean Boat).
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
  # the update check (scripts/nightly-update-gate.sh): once for each new nightly, the one before it installed on a clean Boat and updated to it by the
  # hub's own updater on the nightly channel; a failure is one more failed check in the message below. No nightly repo or no new nightly: nothing to do.
  NU=~/.cache/nightly-update.tested
  if command -v gh >/dev/null; then
    NEWN=$(gh release list -R BackplaneWorks/Backplane --limit 60 --json tagName -q '.[].tagName' 2>/dev/null | grep -E -- '-nightly\.[0-9]+\.[0-9]+$' | sort -V | tail -1)
    if [ -n "$NEWN" ] && [ "$NEWN" != "$(cat "$NU" 2>/dev/null)" ]; then
      (cd "$W" && scripts/nightly-update-gate.sh --to "$NEWN" --out "$D/update") >"$D/update.log" 2>&1; urc=$?
      if [ $urc = 0 ]; then echo "$NEWN" >"$NU"
      elif grep -q "no nightly before" "$D/update.log"; then echo "$NEWN" >"$NU"   # the first nightly: nothing to update from yet
      else mkdir -p "$D/run"; printf 'FAIL\tnightly update to %s: %s\n' "$NEWN" "$(grep -E '^FAIL|could not' "$D/update.log" | head -1 | cut -c1-150)" >>"$D/run/checks.tsv"; rc=1; fi
    fi
  fi
  { echo "$(date -Is) $SHA rc=$rc $D"; sed -n '/== timings/,$p' "$D/gate.log"; [ -f "$D/update.log" ] && tail -2 "$D/update.log"; } >~/.cache/bare-nightly.last
  # since when each check fails: ~/.cache/bare-nightly.fails keeps "<check>\t<first failing commit>\t<date>" for the checks failing now
  F=~/.cache/bare-nightly.fails; touch "$F"
  NOW=$(grep '^FAIL' "$D/run/checks.tsv" 2>/dev/null | cut -f2- | cut -c1-200)
  { while IFS= read -r c; do [ -z "$c" ] && continue; prev=$(awk -F'\t' -v c="$c" '$1==c{print $2"\t"$3}' "$F" | head -1)
      printf '%s\t%s\n' "$c" "${prev:-$SHA	$(date +%F)}"; done <<<"$NOW"; } >"$F.new"; mv "$F.new" "$F"
  if [ $rc != 0 ]; then
    H=${BACKPLANE_NIGHTLY_HOME:-$HOME/.backplane-gpui-fresh/home}; P=$(cut -d' ' -f2 "$H/hub.lock" 2>/dev/null)
    # one message that stands on its own: what failed and since when, the paper cuts, where the screenshots and logs are
    MSG=$(python3 - "$F" "$SHA" "$D" <<'PYM'
import sys, os
f, sha, d = sys.argv[1:4]
rows = [l.rstrip("\n").split("\t") for l in open(f) if l.strip()]
out = [f"Bare-install gate FAILED on {sha} (nightly {os.path.basename(d)}): {len(rows)} check(s)."]
for c, since, day in ((r + ["", ""])[:3] for r in rows):
    out.append(f"- {c} (failing since {since}{', ' + day if day else ''}{'; new tonight' if since == sha else ''})")
g = os.path.join(d, "gate.log")
cuts = []
if os.path.exists(g):
    lines = open(g, errors="replace").read().splitlines()
    if "== paper cuts" in "\n".join(lines):
        i = next(n for n, l in enumerate(lines) if l.startswith("== paper cuts"))
        cuts = [l.strip() for l in lines[i + 1:i + 12] if l.startswith("  [")]
if cuts: out += ["Paper cuts:"] + [f"  {c}" for c in cuts]
shots = os.path.join(d, "run", "shots")
if os.path.isdir(shots): out.append(f"Screenshots: {shots}/ ({len(os.listdir(shots))} files; 12-bom.png, 17-gerbers.png and the like by step)")
out.append(f"Log: {g}; the run's logs: {os.path.join(d, 'run', 'logs')}/")
print("\n".join(out))
PYM
)
    [ -n "$P" ] && bun "$W/test/bare/hub.mjs" "$P" "$H/token" rpc turn.start "$(python3 -c 'import json,sys;print(json.dumps({"thread":sys.argv[1],"text":sys.argv[2]}))' "$T" "$MSG")" >"$D/post.json" \
      || echo "nightly: could not post to $T (no hub at $H)" >>~/.cache/bare-nightly.last
  fi
  exit $rc ;;
*) sed -n '2,10p' "$0"; exit 2 ;;
esac
