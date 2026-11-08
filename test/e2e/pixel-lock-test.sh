#!/bin/bash
# The Pixel rows keep to the lock. Needs no phone: adb is a stand-in that logs every call.   test/e2e/pixel-lock-test.sh
#  1. the lock wrapper (lib/pixel-lock.sh): while a run holds the lock nobody else gets it; when the run ends, by exit, by TERM or by KILL of its shell, everything it started is gone
#     (a background adb loop and a hub-like sleeper) and not one adb call is logged after the lock is free
#  2. the input guard (lib/pixel-adb.sh): with the phone showing Google Play, `inp` makes no `input` call at all; with the gpui app on top it does
#  3. pixel-f-rows.sh makes no adb call except through A()/inp(), and takes the lock itself (no caller's flock)
set -u; HERE=$(cd "$(dirname "$0")" && pwd); T=$(mktemp -d "${TMPDIR:-/tmp}/pixel-lock-test.XXXXXX"); trap 'pkill -f -- "$T/" 2>/dev/null; rm -rf "$T"' EXIT
fails=0; ok() { echo "PASS $1"; }; no() { echo "FAIL $1"; fails=$((fails+1)); }
LOG=$T/adb.log; FAKE=$T/fake-adb; export FAKE_LOG=$LOG PIXEL_LOCK=$T/test.lock
cat > $FAKE <<'EOF'
#!/bin/bash
echo "$(date +%s.%N) $$ $*" >> "$FAKE_LOG"
case "$*" in *"dumpsys activity"*) echo "  topResumedActivity=ActivityRecord{1 u0 ${FAKE_TOP:-dev.backplane.mobile.gpui}/.MainActivity t5}";; esac
EOF
chmod +x $FAKE; export PIXEL_ADB=$FAKE
# --- 1 ---
cat > $T/child.sh <<EOF
#!/bin/bash
. "$HERE/lib/pixel-lock.sh"; pixel_locked_run "\$0" "\$@"
(while :; do "$FAKE" -s X shell input tap 1 1; sleep 0.1; done) &          # an adb loop that outlives the script
(exec -a "$T/hub-like" sleep 300) &                                          # a hub-like child
"$FAKE" -s X shell input tap 2 2
case "\${1:-}" in term) sleep 2; kill -TERM \$\$;; kill) sleep 2; kill -KILL \$\$;; esac
sleep 3
EOF
chmod +x $T/child.sh
for mode in exit term kill; do
  : > $LOG; $T/child.sh $mode & w=$!
  sleep 1.2; if flock -n -x "$PIXEL_LOCK" true; then no "$mode: the lock was free while the run was going"; else ok "$mode: nobody else gets the lock while the run goes"; fi
  wait $w 2>/dev/null; sleep 0.3
  if flock -n -x "$PIXEL_LOCK" true; then ok "$mode: the lock is free once the run ends"; else no "$mode: the lock is still held after the run"; fi
  n1=$(wc -l < $LOG); sleep 1; n2=$(wc -l < $LOG)
  [ "$n1" = "$n2" ] && ok "$mode: no adb call after the lock was released ($n1 calls, none later)" || no "$mode: $((n2-n1)) adb call(s) after the lock was released"
  pgrep -f -- "$T/hub-like" > /dev/null && no "$mode: a child survived the run" || ok "$mode: nothing the run started survives it"
done
# --- 2 ---
. "$HERE/lib/pixel-adb.sh"
: > $LOG; FAKE_TOP="com.android.vending/.AssetBrowserActivity" inp tap 5 5 2>/dev/null; rc=$?
if [ $rc -ne 0 ] && ! grep -q "shell input" $LOG; then ok "Google Play on top: inp refuses and makes no input call"; else no "Google Play on top: inp let an input through (rc=$rc)"; fi
: > $LOG; inp text "x"; if grep -q "shell input text x" $LOG; then ok "the gpui app on top: inp types"; else no "the gpui app on top: inp did not type"; fi
# --- 3 ---
raw=$(grep -nE '(^|[^A-Za-z_])adb( |$)' "$HERE/pixel-f-rows.sh" | grep -vE '^[0-9]+:\s*#|`adb reverse`' | head -3)
[ -z "$raw" ] && ok "pixel-f-rows.sh has no adb call of its own: all go through A()/inp()" || no "raw adb in pixel-f-rows.sh: $raw"
grep -q 'pixel_locked_run "\$0" "\$@"' "$HERE/pixel-f-rows.sh" && ok "pixel-f-rows.sh takes the lock itself" || no "pixel-f-rows.sh does not take the lock"
grep -q 'flock' "$HERE/../../scripts/e2e-linux.sh" && no "scripts/e2e-linux.sh wraps the phone rows in its own flock (a deadlock with the script's own)" || ok "scripts/e2e-linux.sh does not wrap the phone rows in a second flock"
echo; [ $fails = 0 ] && echo "all passed" || echo "$fails failed"; exit $fails
