#!/bin/sh
# Type-check every Bend file, then prove every law. CI runs exactly this.
#
# Usage: scripts/check.sh [all|check|proof]
#   all (default): type-check, then prove
#   check:         type-check only (CI's check job)
#   proof:         prove only, i.e. bend PROOF.bend (CI's proof job)
#
# The type-check no longer runs `bend FILE --check-only` once per file.
# Bend checks every def of every file a program imports (verified on
# 2.0.25: a type error in an unused def of an imported file fails the
# importer's check), so scripts/check-manifest.py generates a few
# manifest files that between them import every tracked .bend file
# exactly once, and only the manifests are checked. Coverage is proven
# mechanically on every run: generation and a separate re-read both
# fail unless the manifests' imports are exactly the tracked files.
# LAWS.bend is still checked on its own, so its TODO handling below is
# unchanged, and PROOF.bend is still run as its own program. If a
# manifest check fails, every file is checked individually, as before,
# so the report names the file that is broken.
#
# CHECK_JOBS: parallel bend processes (default: nproc/4, at least 1,
# at most 6; CI sets it to the runner's core count). CHECK_SHARDS:
# number of manifests (default: 1; one manifest already loads each
# file's source only once, so extra shards only trade more total work
# and memory for parallelism when the single check is the bottleneck).
set -eu
cd "$(dirname "$0")/.."
python3 scripts/i18n-gen.py --check
# PROOF.bend outgrew the checker's stack (it overflowed at random on main):
# a 1 GB main-thread stack, and the engine allowed to use 512 MB of it. A
# concrete size, not "unlimited": with unlimited the engine falls back to a
# small default and still overflows.
ulimit -s 1048576 2>/dev/null || true
export BUN_JSC_maxPerThreadStackUsage=${BUN_JSC_maxPerThreadStackUsage:-536870912}
. scripts/bendok.sh
export BEND_NO_TELEMETRY=1
mode=${1:-all}
case $mode in
  all|check|proof) ;;
  *) echo "usage: scripts/check.sh [all|check|proof]" >&2; exit 2 ;;
esac

cores=$(nproc 2>/dev/null || echo 4)
jobs=${CHECK_JOBS:-$(( cores / 4 ))}
if [ "$jobs" -lt 1 ]; then jobs=1; fi
if [ "$jobs" -gt 6 ]; then jobs=6; fi
shards=${CHECK_SHARDS:-1}

work=$(mktemp -d .bend-check.XXXXXX)
trap 'rm -rf "$work"' 0 1 2 15

# The original per-file verdict, kept for LAWS.bend and for the
# diagnostic fallback: the output text decides (bend exits 1 on a
# failed check, but the script has always judged by the text).
# Prints nothing on success; on failure prints the report, returns 1.
check_one() {
  crc=0
  out=$(bend "$1" --check-only 2>&1) || crc=$?
  case $out in
    *"TODO"*)
      if [ "$1" = LAWS.bend ]; then return 0; fi
      echo "$1: $out"; return 1 ;;
    *) bend_verdict "$crc" "$out" && return 0
      echo "$1 (status $crc):"; echo "$out"; return 1 ;;
  esac
}

fail=0
start=$(date +%s)
# A leftover merge conflict fails at once, by name, before any bend runs
# (one uploaded to Boat sent the check into its per-file fallback for hours)
# (plain grep, not git grep: a Boat sandbox gets the files without .git)
conf=$(grep -rnI -E '^(<<<<<<<|>>>>>>>)( |$)' --exclude-dir=.git --exclude-dir=node_modules --exclude-dir=build --exclude-dir=dist \
  --include='*.bend' --include='*.c' --include='*.h' --include='*.js' --include='*.ts' --include='*.mjs' \
  --include='*.py' --include='*.sh' --include='*.yml' --include='*.json' . 2>/dev/null || true)
if [ -n "$conf" ]; then
  echo "check.sh: merge conflict markers:"
  echo "$conf"
  exit 1
fi
# the inputs' key, taken before anything is checked (scripts/verified.sh)
vkey=""
[ "$mode" = all ] && vkey=$(scripts/verified.sh key check 2>/dev/null || true)

# The proof runs after the type-check, not beside it: run together they
# overflowed the proof's stack (the type-check takes seconds anyway).

if [ "$mode" != proof ]; then
  # Tracked files, listed before any manifest exists so the generated
  # manifests themselves are never part of the list.
  git ls-files --cached --others --exclude-standard '*.bend' | grep -v '^PROOF.bend$' >"$work/files.txt"
  grep -v '^LAWS.bend$' "$work/files.txt" >"$work/covered.txt" || true
  python3 scripts/check-manifest.py generate "$work/m" "$shards" $(cat "$work/covered.txt")
  python3 scripts/check-manifest.py verify "$work/m" $(cat "$work/covered.txt")
  # Manifests plus the separately checked LAWS.bend must be the list.
  mcount=$(grep -h -c '^import ' "$work"/m/manifest-*.bend | awk '{s+=$1} END {print s+0}')
  mcount=$(( mcount - shards ))
  laws_in_list=$(grep -c '^LAWS.bend$' "$work/files.txt" || true)
  total=$(wc -l <"$work/files.txt")
  total=$(echo $total)
  if [ $(( mcount + laws_in_list )) -ne "$total" ]; then
    echo "check.sh: coverage covers $(( mcount + laws_in_list )) of $total tracked files" >&2
    exit 1
  fi

  # Check the manifests $jobs at a time; LAWS.bend runs beside them.
  # Wait only for these pids: a bare `wait` would also reap the proof
  # started above, and waiting for it a second time fails under set -e.
  i=0
  pids=""
  for m in "$work"/m/manifest-*.bend; do
    ( rc=0; BUN_JSC_forceRAMSize=4294967296 nice -n 19 bend "$m" --check-only >"$work/m$i.out" 2>&1 || rc=$?
      echo $rc >"$work/m$i.rc" ) &
    pids="$pids $!"
    i=$(( i + 1 ))
    if [ $(( i % jobs )) -eq 0 ]; then
      for p in $pids; do wait $p || true; done
      pids=""
    fi
  done
  ( rc=0; check_one LAWS.bend >"$work/laws.out" 2>&1 || rc=$?; echo $rc >"$work/laws.rc" ) &
  pids="$pids $!"
  for p in $pids; do wait $p || true; done
  bad=0
  i=0
  for m in "$work"/m/manifest-*.bend; do
    # a manifest passes only with its own success verdict and status: a
    # checker killed before writing anything must not count as a pass
    out=$(cat "$work/m$i.out")
    rc=$(cat "$work/m$i.rc" 2>/dev/null || echo missing)
    bend_verdict "$rc" "$out" || { echo "$m (status $rc):"; echo "$out"; bad=1; }
    i=$(( i + 1 ))
  done
  cat "$work/laws.out"
  if [ "$(cat "$work/laws.rc" 2>/dev/null || echo missing)" != 0 ]; then fail=1; fi
  if [ $bad = 1 ]; then
    fail=1
    # Re-checking every file one at a time took hours and looked like a
    # hang; the manifest's own error above names the cause. Opt in to it
    # with CHECK_FALLBACK=1 (at most CHECK_FALLBACK_MAX files, default 40).
    if [ "${CHECK_FALLBACK:-0}" = 1 ]; then
      max=${CHECK_FALLBACK_MAX:-40}
      echo "check.sh: a manifest check failed; re-checking at most $max files to name it"
      n=0
      while IFS= read -r f; do
        [ "$n" -ge "$max" ] && { echo "check.sh: stopped after $max files"; break; }
        check_one "$f" || true
        n=$(( n + 1 ))
      done <"$work/files.txt"
    else
      echo "check.sh: a manifest check failed (its error is above); CHECK_FALLBACK=1 re-checks files one by one"
    fi
  fi
  echo "check: $(( $(date +%s) - start ))s"
fi

if [ "$mode" = proof ]; then
  pstart=$(date +%s)
  prc=0; pout=$(bend PROOF.bend 2>&1) || prc=$?
  echo "$pout"
  bend_verdict "$prc" "$pout" || fail=1
  echo "proof: $(( $(date +%s) - pstart ))s"
elif [ "$mode" = all ]; then
  ( rc=0; bend PROOF.bend >"$work/proof.out" 2>&1 || rc=$?; echo $rc >"$work/proof.rc" )
  cat "$work/proof.out"
  bend_verdict "$(cat "$work/proof.rc" 2>/dev/null || echo missing)" "$(cat "$work/proof.out")" || fail=1
  echo "proof: after check, total $(( $(date +%s) - start ))s"
fi

if [ $fail != 0 ]; then exit 1; fi
# a full pass (type-check and every proof) is recorded for these inputs
# (scripts/verified.sh); a check or proof alone records nothing
# (an `if`, not `[ ] &&`: as the last command a false test would be the
# script's status, so `check.sh proof` and `check.sh check` exited 1 on a pass)
if [ "$mode" = all ]; then scripts/verified.sh mark check "$vkey" || true; fi
exit 0
