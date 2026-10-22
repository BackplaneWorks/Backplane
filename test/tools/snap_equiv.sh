#!/bin/sh
# MANUAL / BENCH RUN ONLY: never part of scripts/test.sh or CI. On a real
# log one phase holds the whole folded state (the full fold peaked at
# 15.3 GB on the 351,525-line copy; build 11.2 GB, hot 4.6 GB): run it
# under bp-admit with --mem 16 or more, on a COPY of a home only. It starts
# no hub (it only folds and reads store.db), so no work or agent can run.
# S3's hot-boot equivalence on a copy of a real home, one folded state per
# process (a real log's state is many GB on the JS runtime):
#   test/tools/snap_equiv.sh HOME [CUT...]     CUT: bytes or "50%" (default 100%)
# For each cut: store.db from the lines before it, Hot.load'ed and dumped,
# the whole log folded and dumped, compared by snap_equiv_cmp.py. Never on
# the live home: it rewrites HOME/store.db. Each phase's peak RSS printed.
set -u
home=$1; shift
[ $# -gt 0 ] || set -- 100%
here=$(cd "$(dirname "$0")/../.." && pwd)
work=$home/../snap_equiv.work
mkdir -p "$work"
export BEND_NO_TELEMETRY=1
fail=0
for cut in "$@"; do
  echo "== cut $cut"
  /usr/bin/time -f "  [build: peak %M KB, %e s]" bend "$here/test/native/snap_equiv.bend" build "$home" "$cut" 2>&1 | grep -v "^  at byte" || exit 2
  rm -f "$work/hot.jsonl" "$work/full.jsonl"
  /usr/bin/time -f "  [hot: peak %M KB, %e s]" bend "$here/test/native/snap_equiv.bend" hot "$home" "$work/hot.jsonl" 2>&1 | tail -3
  [ -s "$work/hot.jsonl" ] || { echo "cut $cut: the hot phase wrote nothing"; exit 2; }
  ids=$(python3 -c 'import json,sys
for l in open(sys.argv[1]):
    x=json.loads(l)
    if x and x[0]=="E": print(x[1])' "$work/hot.jsonl" | tr '\n' ' ')
  /usr/bin/time -f "  [full: peak %M KB, %e s]" bend "$here/test/native/snap_equiv.bend" full "$home" "$work/full.jsonl" $ids 2>&1 | tail -3
  [ -s "$work/full.jsonl" ] || { echo "cut $cut: the full phase wrote nothing"; exit 2; }
  python3 "$here/test/tools/snap_equiv_cmp.py" "$work/full.jsonl" "$work/hot.jsonl" "$cut"
  rc=$?; [ $rc = 1 ] && fail=1
done
exit $fail
