#!/bin/bash
# worktree-gc.sh [--apply] [--hours N] [--tmp-hours N] [--merged-minutes N] [--sccache-size 20G] [--quiet]: the disk janitor. Frees what lanes and builds leave behind:
#   - a lane's worktree under .backplane/worktrees whose branch has landed (an ancestor of origin/main that has moved since it was made: a fresh branch is no landing),
#     whose tree is clean (no untracked files either) and that nothing has written to for --merged-minutes (60) is removed with its target/ dirs and its branch;
#   - a landed branch's worktree that is not clean keeps its tree and loses its target/ dirs (idle --merged-minutes);
#   - any other worktree's target/ dirs that nothing has written to for --hours (24) are deleted (a rebuild fills them again, mostly from sccache);
#   - top-level entries of $BP_TMP (~/.cache/bp-tmp: lanes' scratch targets, temp homes, logs) that nothing has written to for --tmp-hours (24) are deleted;
#   - test leftovers in /tmp and $BP_TMP (bp-<area>-<pid>-*, bpt-XXXXXX) whose owner is dead go after 24 h (--tmp-minutes, 1440);
#   - under pressure (Nov 17: bp-tmp reached 160 GB, the disk 98%): $BP_TMP is held under --tmp-cap (GC_TMP_CAP, 60G) and the shared cross-build targets $BP_TARGET under
#     --target-cap (GC_TARGET_CAP, 40G), and while the disk has under --low-gb (GC_LOW_GB, 150) free the oldest unused entries of both go too, oldest last write first, until
#     both hold; only entries idle --evict-minutes (120), held by nothing and not kept; the pause file does not stop this (it stops the routine sweeps);
#   - the sccache cache is trimmed, oldest first, to its cap (--sccache-size, else SCCACHE_CACHE_SIZE, else 20G; K, M, G or T) when it has grown past it.
# Every removal is checked again on the spot, just before it (live): a process holding it now (cwd, executable, open file, TMPDIR, CARGO_TARGET_DIR), anything in it written
# or made within its idle time, or a .bp-keep marks it in use and it stays. (Nov 16: the held/fresh lists are read once, a walk takes up to minutes on a busy box, and folders
# made during the walk (lane B's bp-kembed run folder, a land's janitor-test repo, test runs' bpt dirs) were taken as "idle 24 h" mid-run.)
# A pause file (GC_PAUSE, default ~/.cache/bp-janitor.pause; its text says why) stops the /tmp and bp-tmp sweeps; worktrees, target/ dirs and sccache go on. Every run appends
# what it freed to the log main-build.sh gives it ($GPUI_DEV_DIR/janitor.log, ~/.backplane-gpui-dev/janitor.log), which any lane can read.
# A .bp-keep file (untracked: .gitignore has it) at the top of a bp-tmp entry or a worktree keeps it whole, idle or not: what release gates come back to between runs (forkgate/, pp-tests/: Nov 16, a cleanup took a gate's checkout mid-run).
# Nothing is touched that a process has as its cwd, executable, open file, TMPDIR or CARGO_TARGET_DIR (Nov 11: 157 GB of bp-tmp and 137 GB of lanes' targets filled the disk).
# Never touches the main checkout, a locked worktree, a worktree with unlanded commits or edits (only its target/), a file not this user's, or across a mount.
# Dry run unless --apply. Every run prints what it frees (or would) and a total; --quiet prints the total only. One run at a time (a second one leaves at once).
# Env for tests: GC_REPO (the clone), GC_UPSTREAM (default origin/main, else main), GC_TMP_BASES (default "/tmp $BP_TMP"), GC_LOCK, SCCACHE_DIR, GC_CTIME=0 (count only mtimes: a test cannot age a ctime).
set -u; HERE=$(cd "$(dirname "$(readlink -f "$0")")" && pwd); . "$HERE/lib-bptmp.sh"   # (the guard runs it through the link $DEV/janitor: beside the link is no lib-bptmp.sh and no clone)
renice -n 19 $$ > /dev/null 2>&1; ionice -c3 -p $$ > /dev/null 2>&1   # never in the way of a build or the hub
# caps: larger on a data drive (BP_DATA=/data, 2.2 TB: lib-bptmp.sh), as on a home disk before it
dcap=60G; if [ "$BP_DATA" = "$HOME/.cache" ]; then tcap=40G; else tcap=300G; fi   # $BP_TMP is on the home disk either way (lib-bptmp.sh)
APPLY=0; HOURS=24; TMPH=24; MMIN=60; TMIN=1440; QUIET=0; SCC_SIZE=; TMP_CAP=${GC_TMP_CAP:-$dcap}; TGT_CAP=${GC_TARGET_CAP:-$tcap}; LOW_GB=${GC_LOW_GB:-150}; EVMIN=120
# the data drive's bp-tmp (Nov 17: the home bp-tmp's idle entries moved there, links left behind): its idle entries go by the same 24 h rule
LEGACY_TMP=${GC_LEGACY_TMP-$BP_DATA/bp-tmp}; [ "$(readlink -f "$LEGACY_TMP" 2>/dev/null)" = "$(readlink -f "$BP_TMP")" ] && LEGACY_TMP=""
while [ $# -gt 0 ]; do case "$1" in
  --apply) APPLY=1;; --quiet) QUIET=1;; --hours) HOURS=$2; shift;; --days) HOURS=$(( $2 * 24 )); shift;; --tmp-hours) TMPH=$2; shift;;
  --merged-minutes) MMIN=$2; shift;; --tmp-minutes) TMIN=$2; shift;; --sccache-size) SCC_SIZE=$2; shift;;
  --tmp-cap) TMP_CAP=$2; shift;; --target-cap) TGT_CAP=$2; shift;; --low-gb) LOW_GB=$2; shift;; --evict-minutes) EVMIN=$2; shift;;
esac; shift; done
REPO=${GC_REPO:-$(git -C "$HERE" worktree list --porcelain | awk 'NR==1{print substr($0,10)}')}
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
HELD=$(mktemp); ENVS=$(mktemp); NOTED=$(mktemp); trap 'rm -f "$HELD" "$ENVS" "$NOTED"' EXIT
# the hub's work notes (noted, below): GC_WORK_NOTES (dirs; tests), else the hub home its jobs are given, and every Backplane home of this user
if [ -n "${GC_WORK_NOTES+x}" ]; then WN=$GC_WORK_NOTES; else WN="${BACKPLANE_LOCK_HOME:+$BACKPLANE_LOCK_HOME/work} ${BACKPLANE_HOME:+$BACKPLANE_HOME/work} $(ls -d "$HOME"/.backplane*/home/work "$HOME"/.backplane/work 2>/dev/null | tr '\n' ' ')"; fi
for w in $WN; do [ -d "$w" ] && find "$w" -mindepth 2 -maxdepth 2 -name cmd -mmin -$(( ${GC_WORK_HOURS:-24} * 60 )) -exec cat {} + 2>/dev/null; done > "$NOTED"
for e in /proc/[0-9]*/environ; do tr '\0' '\n' < "$e" 2>/dev/null | grep -aE '^(TMPDIR|CARGO_TARGET_DIR)='; done 2>/dev/null | sort -u > "$ENVS"
{ find /proc/[0-9]*/cwd /proc/[0-9]*/exe -maxdepth 0 -printf '%l\n'; find /proc/[0-9]*/fd -mindepth 1 -maxdepth 1 -printf '%l\n'; sed 's/^[A-Z_]*=//' "$ENVS"; } 2>/dev/null \
  | sed 's/ (deleted)$//' | grep '^/' | sort -u > "$HELD"
# kept DIR: a .bp-keep someone put there, never one git tracks in that checkout (Nov 16: the repo carried an empty .bp-keep at its root since Oct 21, so every
# worktree and every copy of the tree looked kept and the janitor freed nothing while the disk filled to 99%)
kept() { [ -e "$1/.bp-keep" ] || return 1; ! git -C "$1" ls-files --error-unmatch -- .bp-keep > /dev/null 2>&1; }
# live PATH MINUTES: it can go now: no .bp-keep, nothing in it (itself included) written or made in the last MINUTES, no process holding it at this moment (a fresh look, not $HELD)
live() { local q=$1; kept "$q" && return 1
  [ -n "$(find "$q" -xdev $(since "$2") -print -quit 2>/dev/null)" ] && return 1
  find /proc/[0-9]*/cwd /proc/[0-9]*/exe -maxdepth 0 -printf '%l\n' 2>/dev/null | sed 's/ (deleted)$//' | awk -v q="$q" 'BEGIN{p = q "/"} $0 == q || index($0, p) == 1 {f = 1; exit} END{exit !f}' && return 1
  find /proc/[0-9]*/fd -mindepth 1 -maxdepth 1 -printf '%l\n' 2>/dev/null | sed 's/ (deleted)$//' | awk -v q="$q" 'BEGIN{p = q "/"} $0 == q || index($0, p) == 1 {f = 1; exit} END{exit !f}' && return 1
  local qre; qre=$(printf '%s' "$q" | sed 's/[][\\.^$*+?(){}|]/\\&/g')
  grep -qsaE "^(TMPDIR|CARGO_TARGET_DIR)=$qre(/|$)" <(for e in /proc/[0-9]*/environ; do tr '\0' '\n' < "$e"; done 2>/dev/null) && return 1
  # a process naming it in its command line: an ssh master's ControlPath socket is no open file (Nov 17, bp-kembed-remote-*/cm of a running remote suite)
  grep -qsaE "(^|[=:[:space:]])$qre(/|[[:space:]]|$)" <(for c in /proc/[0-9]*/cmdline; do tr '\0' '\n' < "$c"; done 2>/dev/null) && return 1
  noted "$q" && return 1
  return 0; }
# noted PATH: a work_run note of the last GC_WORK_HOURS (24: background work runs 2 h at most) names it, as written ($HOME, ~) or through bp-tmp's link to the
# data drive. A job can sit for many minutes writing nothing and with nothing open there (a GUI test waiting on a window: KiCad lead, Nov 17). The hub's work
# notes are <home>/work/<id>/cmd; a lock a work_run holds carries that job's command, and a lock held by a shell (bp-lock.sh) is its process, checked above.
noted() { local q=$1 a r; set -- "$q"; case "$q" in "$HOME"/*) set -- "$@" "~${q#"$HOME"}" "\$HOME${q#"$HOME"}" "\${HOME}${q#"$HOME"}";; esac
  if [ -n "$LEGACY_TMP" ]; then case "$q" in "$LEGACY_TMP"/*) r=$BP_TMP/${q#"$LEGACY_TMP"/}; set -- "$@" "$r"; case "$r" in "$HOME"/*) set -- "$@" "~${r#"$HOME"}" "\$HOME${r#"$HOME"}" "\${HOME}${r#"$HOME"}";; esac;; esac; fi
  for a in "$@"; do grep -qsF -- "$a" "$NOTED" && return 0; done; return 1; }
held() { awk -v q="$1" 'BEGIN{p = q "/"} $0 == q || index($0, p) == 1 {f = 1; exit} END{exit !f}' "$HELD"; }
# recent PATH MINUTES: something under it was written or made (ctime: a copy that kept old mtimes counts) in the last MINUTES; target/ dirs only count for themselves
CT=1; [ "${GC_CTIME:-1}" = 0 ] && CT=0
since() { if [ "$CT" = 1 ]; then echo "( -mmin -$1 -o -cmin -$1 )"; else echo "-mmin -$1"; fi; }
recent() { local p=(); [ "${3:-}" = all ] || p=(-mindepth 1 -name target -prune -o); [ -n "$(find "$1" -xdev "${p[@]}" $(since "$2") -print -quit 2>/dev/null)" ]; }
landed() { git -C "$REPO" merge-base --is-ancestor "refs/heads/$1" "$UP" 2>/dev/null && [ "$(git -C "$REPO" reflog show --format=%H "refs/heads/$1" -- 2>/dev/null | wc -l)" -gt 1 ]; }
# a worktree's target dirs: real ones, and the data drive's dirs its links point at (lib-bptmp.sh bp_target_link)
targets() { local t r; for t in "$1"/target "$1"/native/*/target "$1"/native/*/target-guard; do
  if [ -L "$t" ]; then r=$(readlink -f "$t"); case "$r" in "$BP_DATA"/target/*) [ -d "$r" ] && echo "$r";; esac; elif [ -d "$t" ]; then echo "$t"; fi; done; }

# Worktrees: path, branch (empty when detached), locked
git -C "$REPO" worktree list --porcelain | awk '/^worktree /{w=substr($0,10); b=""; l=0} /^branch /{b=substr($0,8)} /^locked/{l=1} /^$/{if (w!="") print w "\t" b "\t" l; w=""} END{if (w!="") print w "\t" b "\t" l}' \
  > "$HELD.wt"
while IFS=$'\t' read -r w ref locked; do
  case "$w" in */.backplane/worktrees/*) ;; *) continue;; esac
  [ -d "$w" ] || continue; held "$w" && continue   # a running thread, build or shell works in it
  kept "$w" && continue                  # kept on purpose (a gate's checkout), target/ dirs too
  b=${ref#refs/heads/}; lnd=0; [ -n "$ref" ] && landed "$b" && lnd=1
  clean=0; [ -z "$(git --no-optional-locks -C "$w" status --porcelain 2>/dev/null)" ] && clean=1   # never the index refresh: it would make the worktree look used
  if [ "$lnd" = 1 ] && [ "$clean" = 1 ] && [ "$locked" = 0 ] && ! recent "$w" "$MMIN" && ! recent "$(git -C "$w" rev-parse --absolute-git-dir 2>/dev/null || echo "$w")" "$MMIN"; then
    live "$w" "$MMIN" || continue
    note worktree "$w" "$b landed in $UP, clean"; N_WT=$((N_WT + 1))
    [ "$APPLY" = 1 ] && git -C "$REPO" worktree remove --force "$w" 2>/dev/null && git -C "$REPO" branch -D "$b" > /dev/null 2>&1
    continue
  fi
  for t in $(targets "$w"); do
    if [ "$lnd" = 1 ]; then recent "$t" "$MMIN" all && continue; why="$b landed"; else recent "$t" $((HOURS * 60)) all && continue; why="idle ${HOURS} h"; fi
    live "$t" "$([ "$lnd" = 1 ] && echo "$MMIN" || echo $((HOURS * 60)))" || continue
    note target "$t" "$why"; N_TG=$((N_TG + 1)); wipe "$t"
    case "$t" in "$BP_DATA"/target/*) [ "$APPLY" = 1 ] && mkdir -p "$t";; esac   # a link's dir comes back empty: cargo cannot build through a dangling link
  done
done < "$HELD.wt"
rm -f "$HELD.wt"

# the data drive's worktree target dirs whose worktree is gone (removed with its link): idle --merged-minutes and held by nothing
if [ -d "$BP_DATA/target" ]; then
  declare -A linked=()
  while IFS= read -r w; do for t in "$w"/target "$w"/native/*/target "$w"/native/*/target-guard; do [ -L "$t" ] || continue; r=$(readlink -f "$t"); r=${r#"$BP_DATA"/target/}; linked[${r%%/*}]=1; done
  done < <(git -C "$REPO" worktree list --porcelain 2>/dev/null | awk '/^worktree /{print substr($0,10)}')
  for o in "$BP_DATA"/target/*; do [ -d "$o" ] && [ -O "$o" ] || continue; [ -n "${linked[${o##*/}]:-}" ] && continue
    live "$o" "$MMIN" || continue; note target "$o" "its worktree is gone"; N_TG=$((N_TG + 1)); wipe "$o"; done
fi

PAUSE=${GC_PAUSE:-${XDG_CACHE_HOME:-$HOME/.cache}/bp-janitor.pause}; TMP_ON=1
if [ -e "$PAUSE" ]; then TMP_ON=0; say "janitor: /tmp and bp-tmp sweeps paused ($PAUSE: $(head -c 200 "$PAUSE" | tr '\n' ' '))"; fi
[ -z "${GC_TEST_AFTER_SCAN:-}" ] || sh -c "$GC_TEST_AFTER_SCAN"   # tests: make what appears between the scans and the removals
# $BP_TMP: whole top-level entries idle for --tmp-hours that nothing holds. Its sccache/ (the sccache server's TMPDIR) and claude/ (agents' CLAUDE_CODE_TMPDIR) stay.
for TB in "$BP_TMP" $LEGACY_TMP; do [ "$TMP_ON" = 1 ] && [ -d "$TB" ] || continue
  declare -A busy_t=() fresh_t=()
  while IFS= read -r l; do busy_t[$l]=1; done < <(grep -F "$TB/" "$HELD" | awk -v p="$TB/" 'index($0, p) == 1 {s = substr($0, length(p) + 1); sub(/\/.*/, "", s); print s}' | sort -u)
  # one walk lists the top-level names with anything written lately
  while IFS= read -r l; do fresh_t[$l]=1; done < <(find "$TB" -xdev -mindepth 1 \( -path "$TB/sccache" -o -path "$TB/claude" \) -prune -o $(since $((TMPH * 60))) -printf '%P\n' 2>/dev/null | awk -F/ '!s[$1]++ {print $1}')
  for e in "$TB"/* "$TB"/.[!.]*; do
    [ -e "$e" ] || [ -L "$e" ] || continue; n=${e#"$TB"/}
    [ -L "$e" ] && [ -e "$e" ] && continue   # a link to where an entry moved (the data drive): it goes when what it points at is gone
    case "$n" in sccache|claude) continue;; esac
    [ "$e" -ef "$LOCK" ] && continue
    [ -O "$e" ] || continue; [ -n "${busy_t[$n]:-}" ] && continue; [ -n "${fresh_t[$n]:-}" ] && continue
    kept "$e" && continue   # kept on purpose (a gate's scripts, venv, fixtures)
    live "$e" $((TMPH * 60)) || continue   # made, written or opened since the lists above were read
    note tmp "$e" "idle ${TMPH} h"; N_TMP=$((N_TMP + 1)); wipe "$e"
  done
done

# Test leftovers (lanes' test runs) in /tmp and $BP_TMP, sooner than the rule above: only top-level entries of this kind, made by this user, never a symlink, never across
# a mount, only when idle for --tmp-minutes and nothing uses them: bp-<area>-<pid>-<name> whose pid is dead (bp-chrome-<pid> is a real hub's, not a test's: left alone), and the
# per-run bpt-XXXXXX dirs of scripts/lib-buildslot.sh (bpt_new) that no process has as its cwd or TMPDIR; a failed run's bpt dir (.bpt-failed) is evidence, bounded there.
for base in ${GC_TMP_BASES:-/tmp $BP_TMP $LEGACY_TMP}; do [ "$TMP_ON" = 1 ] && [ -d "$base" ] || continue
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
    [ -n "${act[$b]:-}" ] && continue; held "$d" && continue; kept "$d" && continue; live "$d" "$TMIN" || continue
    note tmp "$d" "test leftover, owner gone"; N_TMP=$((N_TMP + 1)); wipe "$d"
  done
done

# Pressure: a size cap per base and a free-space floor for the disk, oldest unused entries first (see the top). Sizes: K, M, G or T.
tokb() { printf '%s' "$1" | awk '{n = $0 + 0; u = toupper(substr($0, length($0)))} END{m = (u == "T") ? 1073741824 : (u == "G") ? 1048576 : (u == "M") ? 1024 : (u == "K") ? 1 : 1 / 1024; print int(n * m)}'; }
availk() { df -Pk "$1" 2>/dev/null | awk 'NR == 2 {print $4}'; }
EVK=0; lowk=$((LOW_GB * 1048576))
evict() { local base=$1 capk=$2 have n t e k free why
  [ -d "$base" ] || return 0; have=$(kb "$base"); have=${have:-0}
  pressed() { free=$(( $(availk "$base") + EVK )); [ "$have" -gt "$capk" ] || [ "$free" -lt "$lowk" ]; }
  pressed || return 0
  # each top-level entry with the time of its newest write, oldest first (one walk)
  while IFS=' ' read -r t n; do pressed || break
    e=$base/$n; case "$n" in sccache|claude|"") continue;; esac
    [ -e "$e" ] && [ -O "$e" ] && [ ! "$e" -ef "$LOCK" ] || continue
    kept "$e" && continue; live "$e" "$EVMIN" || continue
    if [ "$have" -gt "$capk" ]; then why="$base over its cap $(human "$capk")"; else why="disk under ${LOW_GB} GB free"; fi
    k=$(kb "$e"); k=${k:-0}; note tmp "$e" "$why, oldest unused"; N_TMP=$((N_TMP + 1)); wipe "$e"; have=$((have - k)); EVK=$((EVK + k))
  done < <(find "$base" -xdev -mindepth 1 -printf '%T@ %P\n' 2>/dev/null | awk '{split($2, a, "/"); n = a[1]; t = int($1); if (!(n in m) || t > m[n]) m[n] = t} END{for (n in m) print m[n], n}' | sort -n)
}
evict "$BP_TMP" "$(tokb "$TMP_CAP")"; evict "$BP_TARGET" "$(tokb "$TGT_CAP")"
a=$(availk "${BP_TMP:-$HOME}"); [ -z "$a" ] || [ "$a" -ge "$lowk" ] || say "janitor: the disk has $(human "$a") free, under ${LOW_GB} GB, with nothing more it may remove"

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
