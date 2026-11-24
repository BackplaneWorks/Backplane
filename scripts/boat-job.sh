#!/bin/bash
# Run one command on a Boat build box from this worktree: the launcher the test/**/boat-*-job.sh scripts call.
#   scripts/boat-job.sh [--timeout SECS] [--wait SECS] [--artifact PATH]... [--size S] [--idle S] [--exclude GLOB]... [--include-file PATH]... [--no-reuse] -- CMD...
# It runs on the pool boxes scripts/cargo-q and scripts/dev-build.sh offload to (scripts/lib-boat.sh: bp-build-N, made from the snapshot bp-build-base
# with no environment, so nothing of the person's is on them), one job per box under the same lease, so a job never shares a box with a build:
#   1. a free lease: every box busy, it waits (said every minute), at most --wait SECS when given (then exit 75);
#   2. the worktree copied exactly to ~/ws/src on the box (bo_sync: tracked and untracked files, nothing git ignores; an --include-file such as
#      stage/<binary> goes with it as long as git does not ignore it), each --artifact path removed there, the box's lifetime set to cover the run;
#   3. CMD from ~/ws/src with CI=1, under `timeout -k 30 SECS` (default 1800), streamed here and to the job's job.log, with its exit code;
#   4. each --artifact (a file or folder, relative to the worktree) copied back to <job>/artifacts/<its name>, however the command ended (a cancel too);
#   5. the lease released and the box's idle stop armed (bo_finish: scripts/boat-pool.sh reap).
# The job folder is $XDG_STATE_HOME/backplane/boat/jobs/<job> (~/.local/state/...); the first and last lines say it ("logs <folder>").
# --size, --idle, --exclude and --no-reuse are accepted from the launcher this replaces (removed in 3a37a3e8c3) and ignored: the pool's boxes have one
# size (BP_BOAT_TYPE), stop when idle, and keep their disk between jobs (apt installs, $HOME, ~/target), unlike that launcher's one-off sandboxes.
# Exit: the command's own code (200 becomes 1: it means "every box busy" to the lease); 124 timed out; 75 no lease within --wait; 98 no box could be used.
set -u
HERE=$(cd "$(dirname "$0")" && pwd); . "$HERE/lib-boat.sh"; BO_HERE=$HERE
say() { echo "boat-job: $*" >&2; }
TIMEOUT=1800; WAIT=; ART=(); ARGS=("$@")
while [ $# -gt 0 ]; do
  case "$1" in
    --) shift; break ;;
    --timeout) TIMEOUT=${2:-}; shift 2 ;;
    --wait) WAIT=${2:-}; shift 2 ;;
    --artifact) ART+=("${2%/}"); shift 2 ;;
    --include-file|--size|--idle|--exclude|--budget-usd) shift 2 ;;
    --no-reuse|--setup-bend|--no-setup-bend) shift ;;
    *) say "ignoring '$1' before --"; shift ;;
  esac
done
[ $# -gt 0 ] || { say "usage: scripts/boat-job.sh [OPTIONS] -- CMD..."; exit 2; }
case "$TIMEOUT${WAIT:-0}" in ''|*[!0-9]*) say "bad --timeout or --wait"; exit 2 ;; esac
for a in "${ART[@]}"; do case "$a" in ''|/*|*..*) say "bad --artifact '$a' (a path inside the worktree)"; exit 2 ;; esac; done

if [ -z "${BP_BOAT_IDX:-}" ]; then
  # the caller: wait for a lease, then run this script again under it (bo_try sets BP_BOAT_IDX). A TERM or ^C here reaches the job, which cleans up.
  bo_ready || { say "Boat cannot be used from here (boat, rsync or flock missing, BP_NO_BOAT set, or boat failed a moment ago)"; exit 98; }
  export BJ_PIDFILE; BJ_PIDFILE=$(mktemp "${TMPDIR:-/tmp}/boat-job.XXXXXX")
  child=; stop() { local p; p=$(cat "$BJ_PIDFILE" 2>/dev/null); [ -n "$p" ] && kill -"$1" "$p" 2>/dev/null; [ -n "$child" ] && wait "$child"; rm -f "$BJ_PIDFILE"; exit "$2"; }
  trap 'stop TERM 143' TERM HUP; trap 'stop INT 130' INT
  t0=$(date +%s); n=0
  while :; do
    bo_try "$0" "${ARGS[@]}" & child=$!; wait "$child"; rc=$?; child=
    [ "$rc" = 200 ] || { rm -f "$BJ_PIDFILE"; exit "$rc"; }
    w=$(( $(date +%s) - t0 )); [ -n "$WAIT" ] && [ "$w" -ge "$WAIT" ] && { say "no Boat box free within $WAIT s"; rm -f "$BJ_PIDFILE"; exit 75; }
    [ $((n % 12)) = 0 ] && say "every Boat box is busy; waiting ($w s)"; n=$((n + 1)); sleep 5
  done
fi

# under the lease
echo $$ > "${BJ_PIDFILE:-/dev/null}"
root=$(git rev-parse --show-toplevel) || exit 98
i=$BP_BOAT_IDX; unset BP_BOAT_IDX; job="$(date +%s)-$$"
OUT=${XDG_STATE_HOME:-$HOME/.local/state}/backplane/boat/jobs/$job; mkdir -p "$OUT/artifacts" || exit 98
echo "job $job: logs and artifacts in $OUT"
id=$(bo_box "$i") || { say "no box"; exit 98; }
say "running on Boat bp-build-$i ($id): $*"
pid=; rc=; cancelled=
finish() {   # however the run ended: artifacts back, the lease's box left to stop when idle
  local a e
  for a in "${ART[@]}"; do
    e=$(rsync -rlp --no-times -e "$HERE/boat-ssh.sh" "$id:/home/user/ws/src/$(printf '%q' "$a")" "$OUT/artifacts/" 2>&1 >/dev/null) && say "artifact $a: copied" \
      || say "artifact $a: not copied: $(printf '%s' "$e" | tail -1)"
  done
  bo_finish "$i" "$id" "$job"; echo "exit $rc · logs $OUT"; exit "$rc"; }
on_signal() { cancelled=1; rc=$1; [ -n "$pid" ] && { kill -TERM "$pid" 2>/dev/null; wait "$pid" 2>/dev/null; }; say "cancelled"; finish; }
trap 'on_signal 143' TERM HUP; trap 'on_signal 130' INT
bo_sync "$id" "$root" "$job" || { say "could not copy the worktree to $id"; rc=98; finish; }
bo_b extend "$id" --ttl $(( TIMEOUT + 900 )) > /dev/null 2>&1 || say "could not extend the box's lifetime; a long job may be cut off"
rm_art=; for a in "${ART[@]}"; do rm_art+="rm -rf $(printf '%q' "$a"); "; done
body="export CI=1; ulimit -s 65536 2>/dev/null; $rm_art exec timeout -k 30 $TIMEOUT $(printf '%q ' "$@")"
# the job script on the box (bo_run's, with its own traps here so a cancel still copies the artifacts back)
s=$({ echo '#!/bin/bash'; echo 'export PATH="$HOME/.cargo/bin:/usr/local/cargo/bin:$HOME/.bun/bin:/usr/local/bin:$PATH" BP_NO_BOAT=1 BP_ON_BOAT=1 BP_JOB=$HOME/.bpjobs/'"$job"
           bo_env; echo 'cd ~/ws/src || exit 96'; printf '%s\n' "$body"; })
bo_b exec "$id" --timeout 60 "echo $(printf '%s\n' "$s" | base64 -w0) | base64 -d > ~/.bpjobs/$job/run.sh" > /dev/null || { say "could not start the job on $id"; rc=98; finish; }
python3 -I "$HERE/boat-run.py" "$id" bash "/home/user/.bpjobs/$job/run.sh" > >(tee "$OUT/job.log") 2> >(tee -a "$OUT/job.log" >&2) & pid=$!
wait "$pid"; rc=$?; pid=
case "$rc" in
  124) say "timed out after $TIMEOUT s" ;;
  97) say "lost the connection to $id and could not get it back" ;;
  200) say "the command exited 200; reported as 1"; rc=1 ;;
esac
finish
