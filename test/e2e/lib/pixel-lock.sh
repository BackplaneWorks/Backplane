# Sourced by every script that drives the Pixel (test/e2e/pixel-f-rows.sh): the whole run happens inside ONE flock on the phone's lock, taken by the script itself.
#   . "$HERE/lib/pixel-lock.sh"; pixel_locked_run "$0" "$@"        (first lines of the script; it returns only inside the lock)
# The lock is held by a small wrapper that outlives everything the script starts: it runs the script in a process group of its own and kills that group (a hub, a helper, an adb
# loop that was left running) before the lock is let go. So no adb call can happen after the lock is released, and none from a caller that never took it.
# -o: the lock's descriptor is not inherited, so a child that survives could never hold the lock for an hour; the group kill is what makes that safe.
PIXEL_LOCK=${PIXEL_LOCK:-$HOME/.cache/pixel-52220DLAQ001FC.lock}
# (the wrapper uses procps kill: the builtin kill of dash does not take -- before a process group)
pixel_locked_run() {
  [ -n "${PIXEL_LOCK_HELD:-}" ] && return 0
  mkdir -p "$(dirname "$PIXEL_LOCK")"
  exec flock -x -o "$PIXEL_LOCK" env PIXEL_LOCK_HELD=1 sh -c '
    setsid "$@" & pg=$!
    trap "/usr/bin/kill -TERM -- -$pg 2>/dev/null" TERM INT HUP
    wait $pg; rc=$?
    /usr/bin/kill -KILL -- -$pg 2>/dev/null
    exit $rc' sh "$@"
}
