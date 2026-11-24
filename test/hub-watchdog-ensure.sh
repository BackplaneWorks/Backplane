#!/bin/bash
# scripts/hub-watchdog.sh ensure (what fresh-watch.sh runs at each start), against a stand-in systemctl and a temp config folder: units that are not the script's own
# (Nov 18: the timer overwritten with `systemctl show` text, which systemd refused) or a timer that is not running are written and started again, and logged;
# units that are right and a running timer are left alone, silently.
#   test/hub-watchdog-ensure.sh        exit 0 when it holds
set -eu
HERE=$(cd "$(dirname "$0")/.." && pwd)
. "$HERE/scripts/lib-bptmp.sh"
T=$(mktemp -d "${BP_TMP:-${TMPDIR:-/tmp}}/bp-wdensure-XXXX"); trap 'rm -rf "$T"' EXIT
D=$T/fresh; U=$T/config/systemd/user; mkdir -p "$D/home" "$U" "$T/bin"
fail() { echo "FAIL: $*"; echo "--- watchdog.log"; cat "$D/watchdog.log" 2>/dev/null; echo "--- systemctl calls"; cat "$T/calls" 2>/dev/null; exit 1; }
# the stand-in: records each call; is-active answers from $T/active
cat > "$T/bin/systemctl" <<SH
#!/bin/sh
echo "\$*" >> "$T/calls"
case "\$*" in *is-active*) [ -e "$T/active" ];; *"enable --now"*) : > "$T/active";; *) exit 0;; esac
SH
chmod +x "$T/bin/systemctl"
run() { XDG_CONFIG_HOME=$T/config FRESH_DIR=$D WATCHDOG_SYSTEMCTL=$T/bin/systemctl bash "$HERE/scripts/hub-watchdog.sh" ensure; }
# what was on the desktop: a timer with no time in it
printf '[Unit]\nDescription=Backplane hub watchdog, every 30 seconds\n\n[Timer]\n# TimersMonotonic={ OnBootUSec=1min ; next_elapse=1min }\nUnit=backplane-hub-watchdog.service\n' > "$U/backplane-hub-watchdog.timer"
out=$(run) || fail "ensure failed: $out"
grep -q '^OnUnitInactiveSec=30' "$U/backplane-hub-watchdog.timer" || fail "the timer was not written again"
grep -q "ExecStart=$HERE/scripts/hub-watchdog.sh check" "$U/backplane-hub-watchdog.service" || fail "the service was not written"
grep -q -- "--user enable --now backplane-hub-watchdog.timer" "$T/calls" || fail "the timer was not started"
grep -q "repaired: service differs.*timer differs.*timer not active" "$D/watchdog.log" || fail "the repair is not logged with why"
# right units and a running timer: nothing done, nothing said
: > "$T/calls"; out=$(run) || fail "second ensure failed"
[ -z "$out" ] || fail "said something with nothing to repair: $out"
! grep -q enable "$T/calls" || fail "started again with nothing to repair"
# a timer that stopped (a logout) is started again, units untouched
rm -f "$T/active"; : > "$T/calls"; run > /dev/null || fail "third ensure failed"
grep -q "enable --now" "$T/calls" || fail "a stopped timer was not started"
grep -q "repaired: timer not active" "$D/watchdog.log" || fail "the restart is not logged"
echo "hub-watchdog-ensure: ok"
