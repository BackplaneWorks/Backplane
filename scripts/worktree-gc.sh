#!/bin/bash
# worktree-gc.sh [--apply] [--hours N] [--tmp-hours N] [--merged-minutes N] [--sccache-size 20G] [--quiet]: the disk janitor. Frees what lanes and builds leave behind:
#   - a lane's worktree under .backplane/worktrees whose branch has landed (an ancestor of origin/main that has moved since it was made: a fresh branch is no landing),
#     whose tree is clean (no untracked files either) and that nothing has written to for --merged-minutes (60) is removed with its target/ dirs and its branch;
#   - a landed branch's worktree that is not clean keeps its tree and loses its target/ dirs (idle --merged-minutes);
#   - any other worktree's target/ dirs that nothing has written to for --hours (24) are deleted (a rebuild fills them again, mostly from sccache);
#   - top-level entries of $BP_TMP (~/.cache/bp-tmp: lanes' scratch targets, temp homes, logs) that nothing has written to for --tmp-hours (24) are deleted;
#   - test leftovers in /tmp and $BP_TMP (bp-<area>-<pid>-*, bpt-XXXXXX) whose owner is dead go after 30 minutes (--tmp-minutes);
#   - the sccache cache is trimmed, oldest first, to its cap (--sccache-size, else SCCACHE_CACHE_SIZE, else 20G; K, M, G or T) when it has grown past it.
# Nothing is touched that a process has as its cwd, executable, open file, TMPDIR or CARGO_TARGET_DIR (Nov 11: 157 GB of bp-tmp and 137 GB of lanes' targets filled the disk).
# Never touches the main checkout, a locked worktree, a worktree with unlanded commits or edits (only its target/), a file not this user's, or across a mount.
# Dry run unless --apply. Every run prints what it frees (or would) and a total; --quiet prints the total only. One run at a time (a second one leaves at once).
# Env for tests: GC_REPO (the clone), GC_UPSTREAM (default origin/main, else main), GC_TMP_BASES (default "/tmp $BP_TMP"), GC_LOCK, SCCACHE_DIR, GC_CTIME=0 (count only mtimes: a test cannot age a ctime).
set -u; . "$(dirname "$0")/lib-bptmp.sh"
renice -n 19 $$ > /dev/null 2>&1; ionice -c3 -p $$ > /dev/null 2>&1   # never in the way of a build or the hub
APPLY=0; HOURS=24; TMPH=24; MMIN=60; TMIN=30; QUIET=0; SCC_SIZE=
while [ $# -gt 0 ]; do case "$1" in
  --apply) APPLY=1;; --quiet) QUIET=1;; --hours) HOURS=$2; shift;; --days) HOURS=$(( $2 * 24 )); shift;; --tmp-hours) TMPH=$2; shift;;
  --merged-minutes) MMIN=$2; shift;; --tmp-minutes) TMIN=$2; shift;; --sccache-size) SCC_SIZE=$2; shift;;
esac; shift; done
REPO=${GC_REPO:-$(git -C "$(dirname "$0")" worktree list --porcelain | awk 'NR==1{print substr($0,10)}')}
UP=${GC_UPSTREAM:-}; [ -n "$UP" ] || { git -C "$REPO" rev-parse -q --verify refs/remotes/origin/main > /dev/null && UP=origin/main || UP=${MAIN_BRANCH:-main}; }
say() { [ "$QUIET" = 1 ] || echo "$@"; }
LOCK=${GC_LOCK:-${XDG_CACHE_HOME:-$HOME/.cache}/bp-janitor.lock}; exec 9> "$LOCK"; flock -n 9 || { say "janitor: another run is at work"; exit 0; }
T0=$(date +%s); say "== $(date '+%F %T') janitor $([ "$APPLY" = 1 ] && echo apply || echo 'dry run')"

FREED=0; N_WT=0; N_TG=0; N_TMP=0; SCC=0
kb() { du -sk --one-file-system "$1" 2>/dev/null | cut -f1; }
human() { awk -v k="${1:-0}" 'BEGIN{ if (k >= 1048576) printf "%.1f GB", k / 1048576; else printf "%d MB", k / 1024 }'; }
# note KIND PATH WHY: measured and said before it goes
note() { local k; k=$(kb "$2"); k=${k:-0}; FREED=$((FREED + k)); say "$(printf '%-8s %9s  %s' "$1" "$(human "$k")" "$2")${3:+  ($3)}"; }
wipe() { [ "$APPLY" = 1 ] || return 0; if [ -L "$1" ] || [ ! -d "$1" ]; then rm -f -- "$1"; else chmod -R u+rwX -- "$1" 2>/dev/null; rm -rf --one-file-system -- "$1"; fi; }

# What processes hold: cwd, executable, open files (what lsof lists), and the dirs their TMPDIR / CARGO_TARGET_DIR name. Read once.
HELD=$(mktemp); ENVS=$(mktemp); trap 'rm -f "$HELD" "$ENVS"' EXIT
for e in /proc/[0-9]*/environ; do tr '\0' '\n' < "$e" 2>/dev/null | grep -aE '^(TMPDIR|CARGO_TARGET_DIR)='; done 2>/dev/null | sort -u > "$ENVS"
{ find /proc/[0-9]*/cwd /proc/[0-9]*/exe -maxdepth 0 -printf '%l\n'; find /proc/[0-9]*/fd -mindepth 1 -maxdepth 1 -printf '%l\n'; sed 's/^[A-Z_]*=//' "$ENVS"; } 2>/dev/null \
  | sed 's/ (deleted)$//' | grep '^/' | sort -u > "$HELD"
held() { awk -v q="$1" 'BEGIN{p = q "/"} $0 == q || index($0, p) == 1 {f = 1; exit} END{exit !f}' "$HELD"; }
# recent PATH MINUTES: something under it was written or made (ctime: a copy that kept old mtimes counts) in the last MINUTES; target/ dirs only count for themselves
CT=1; [ "${GC_CTIME:-1}" = 0 ] && CT=0
since() { if [ "$CT" = 1 ]; then echo "( -mmin -$1 -o -cmin -$1 )"; else echo "-mmin -$1"; fi; }
recent() { local p=(); [ "${3:-}" = all ] || p=(-mindepth 1 -name target -prune -o); [ -n "$(find "$1" -xdev "${p[@]}" $(since "$2") -print -quit 2>/dev/null)" ]; }
landed() { git -C "$REPO" merge-base --is-ancestor "refs/heads/$1" "$UP" 2>/dev/null && [ "$(git -C "$REPO" reflog show --format=%H "refs/heads/$1" -- 2>/dev/null | wc -l)" -gt 1 ]; }
targets() { for t in "$1"/target "$1"/native/*/target; do [ -d "$t" ] && [ ! -L "$t" ] && echo "$t"; done; }

# Worktrees: path, branch (empty when detached), locked
git -C "$REPO" worktree list --porcelain | awk '/^worktree /{w=substr($0,10); b=""; l=0} /^branch /{b=substr($0,8)} /^locked/{l=1} /^$/{if (w!="") print w "\t" b "\t" l; w=""} END{if (w!="") print w "\t" b "\t" l}' \
  > "$HELD.wt"
while IFS=$'\t' read -r w ref locked; do
  case "$w" in */.backplane/worktrees/*) ;; *) continue;; esac
  [ -d "$w" ] || continue; held "$w" && continue   # a running thread, build or shell works in it
  b=${ref#refs/heads/}; lnd=0; [ -n "$ref" ] && landed "$b" && lnd=1
  clean=0; [ -z "$(git --no-optional-locks -C "$w" status --porcelain 2>/dev/null)" ] && clean=1   # never the index refresh: it would make the worktree look used
  if [ "$lnd" = 1 ] && [ "$clean" = 1 ] && [ "$locked" = 0 ] && ! recent "$w" "$MMIN" && ! recent "$(git -C "$w" rev-parse --absolute-git-dir 2>/dev/null || echo "$w")" "$MMIN"; then
    note worktree "$w" "$b landed in $UP, clean"; N_WT=$((N_WT + 1))
    [ "$APPLY" = 1 ] && git -C "$REPO" worktree remove --force "$w" 2>/dev/null && git -C "$REPO" branch -D "$b" > /dev/null 2>&1
    continue
  fi
  for t in $(targets "$w"); do
    if [ "$lnd" = 1 ]; then recent "$t" "$MMIN" all && continue; why="$b landed"; else recent "$t" $((HOURS * 60)) all && continue; why="idle ${HOURS} h"; fi
    note target "$t" "$why"; N_TG=$((N_TG + 1)); wipe "$t"
  done
done < "$HELD.wt"
rm -f "$HELD.wt"

# $BP_TMP: whole top-level entries idle for --tmp-hours that nothing holds. Its sccache/ (the sccache server's TMPDIR) and claude/ (agents' CLAUDE_CODE_TMPDIR) stay.
if [ -d "$BP_TMP" ]; then
  declare -A busy_t=() fresh_t=()
  while IFS= read -r l; do busy_t[$l]=1; done < <(grep -F "$BP_TMP/" "$HELD" | awk -v p="$BP_TMP/" 'index($0, p) == 1 {s = substr($0, length(p) + 1); sub(/\/.*/, "", s); print s}' | sort -u)
  # one walk lists the top-level names with anything written lately
  while IFS= read -r l; do fresh_t[$l]=1; done < <(find "$BP_TMP" -xdev -mindepth 1 \( -path "$BP_TMP/sccache" -o -path "$BP_TMP/claude" \) -prune -o $(since $((TMPH * 60))) -printf '%P\n' 2>/dev/null | awk -F/ '!s[$1]++ {print $1}')
  for e in "$BP_TMP"/* "$BP_TMP"/.[!.]*; do
    [ -e "$e" ] || [ -L "$e" ] || continue; n=${e#"$BP_TMP"/}
    case "$n" in sccache|claude) continue;; esac
    [ "$e" -ef "$LOCK" ] && continue
    [ -O "$e" ] || continue; [ -n "${busy_t[$n]:-}" ] && continue; [ -n "${fresh_t[$n]:-}" ] && continue
    note tmp "$e" "idle ${TMPH} h"; N_TMP=$((N_TMP + 1)); wipe "$e"
  done
fi

# Test leftovers (lanes' test runs) in /tmp and $BP_TMP, sooner than the rule above: only top-level entries of this kind, made by this user, never a symlink, never across
# a mount, only when idle for --tmp-minutes and nothing uses them: bp-<area>-<pid>-<name> whose pid is dead (bp-chrome-<pid> is a real hub's, not a test's: left alone), and the
# per-run bpt-XXXXXX dirs of scripts/lib-buildslot.sh (bpt_new) that no process has as its cwd or TMPDIR; a failed run's bpt dir (.bpt-failed) is evidence, bounded there.
for base in ${GC_TMP_BASES:-/tmp $BP_TMP}; do [ -d "$base" ] || continue
  declare -A act=(); while IFS= read -r l; do act[$l]=1; done < <(find "$base" -xdev -mindepth 1 ! -path "$base/bp*" -prune -o -mmin -"$TMIN" -printf '%P\n' 2>/dev/null | awk -F/ '!s[$1]++ {print $1}')
  re='^bp-[a-z0-9_]+-([0-9]+)(-.*)?$'
  for d in "$base"/bp-*-[0-9]* "$base"/bpt-??????; do
    [ -d "$d" ] && [ ! -L "$d" ] && [ -O "$d" ] || continue
    b=${d#"$base"/}
    case "$b" in
      bpt-*) [ -e "$d/.bpt-failed" ] && continue; grep -qxF "TMPDIR=$d" "$ENVS" && continue;;
      bp-chrome-*) continue;;
      *) [[ $b =~ $re ]] && [ ! -e "/proc/${BASH_REMATCH[1]}" ] || continue;;
    esac
    [ -n "${act[$b]:-}" ] && continue; held "$d" && continue
    note tmp "$d" "test leftover, owner gone"; N_TMP=$((N_TMP + 1)); wipe "$d"
  done
done

# sccache: its server evicts to the size it was started with; a server started with a bigger SCCACHE_CACHE_SIZE (or none) lets the cache pass the machine's cap
SD=${SCCACHE_DIR:-$HOME/.cache/sccache}
if [ -d "$SD" ]; then
  cap=${SCC_SIZE:-${SCCACHE_CACHE_SIZE:-20G}}
  capk=$(printf '%s' "$cap" | awk '{n = $0 + 0; u = toupper(substr($0, length($0)))} END{m = (u == "T") ? 1073741824 : (u == "G") ? 1048576 : (u == "M") ? 1024 : (u == "K") ? 1 : 1 / 1024; print int(n * m)}')
  [ "${capk:-0}" -gt 0 ] || capk=$((20 * 1048576)); have=$(kb "$SD"); have=${have:-0}
  if [ "$have" -gt $((capk * 105 / 100)) ]; then
    want=$((capk * 90 / 100)); over=$((have - want))
    # oldest first (last read or written); a file a compile has open stays readable to it
    while read -r k f; do [ "$over" -le 0 ] && break
      [ "$APPLY" = 1 ] && rm -f -- "$f"; over=$((over - k)); SCC=$((SCC + k))
    done < <(find "$SD" -xdev -type f -printf '%A@ %T@ %k %p\n' 2>/dev/null | awk '{t = ($1 > $2) ? $1 : $2; $1 = $2 = ""; print t $0}' | sort -n | cut -d' ' -f2-)
    FREED=$((FREED + SCC)); say "$(printf '%-8s %9s  %s' sccache "$(human "$SCC")" "$SD")  (was $(human "$have"), cap $cap)"
  fi
fi

msg="freed $(human "$FREED") ($N_WT worktrees, $N_TG target dirs, $N_TMP temp entries, sccache $(human "$SCC")) in $(( $(date +%s) - T0 )) s"
[ "$APPLY" = 1 ] && echo "janitor: $msg" || echo "janitor (dry run): would have $msg"
