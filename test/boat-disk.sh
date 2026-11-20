#!/bin/bash
# scripts/boat-disk.py, the Boat build box disk guard: on a fake home measured against a small cap (--cap-gb), it removes old job dirs always, and only over --high:
# old incremental state first, then the cold target dir, never the hot one, never what a symlink points at; `report` is one JSON line. Runs anywhere, in a second.
set -u
HERE=$(cd "$(dirname "$0")/.." && pwd); G="$HERE/scripts/boat-disk.py"
T=$(mktemp -d "${TMPDIR:-/tmp}/boat-disk-test.XXXXXX") || exit 1
trap 'rm -rf -- "$T"' EXIT
H=$T/home; fail=0
ok() { echo "ok   $1"; }
no() { echo "FAIL $1"; fail=1; }
mb() { mkdir -p "$(dirname "$1")"; dd if=/dev/urandom of="$1" bs=1M count="$2" status=none; touch -d "$3" "$1"; }
old() { touch -h -d "$2" "$1"; }   # a dir's own mtime, after its files were made

build() {
  rm -rf -- "$H"; mkdir -p "$H/target" "$H/.bpjobs" "$T/outside"
  mb "$H/.bpjobs/1-1/list" 1 "3 days ago"; old "$H/.bpjobs/1-1" "3 days ago"            # a job that died: gone whatever the disk
  mb "$H/.bpjobs/2-2/list" 1 "now"                                                   # the running job: kept
  # hot: used a minute ago; its incremental has a stale crate hash (5 days) and a live one
  mb "$H/target/hot/debug/deps/libnew.rlib" 2 "now"
  mb "$H/target/hot/debug/incremental/crate-old/s-1/x.bin" 4 "5 days ago"; old "$H/target/hot/debug/incremental/crate-old/s-1" "5 days ago"; old "$H/target/hot/debug/incremental/crate-old" "5 days ago"
  mb "$H/target/hot/debug/incremental/crate-new/s-2/x.bin" 3 "now"
  # cold: last built 4 days ago
  mb "$H/target/cold/fast/deps/libold.rlib" 6 "4 days ago"
  mb "$H/target/cold/fast/incremental/crate-c/s-3/x.bin" 2 "4 days ago"
  for d in "$H/target/cold/fast/incremental/crate-c/s-3" "$H/target/cold/fast/incremental/crate-c" "$H/target/cold/fast/incremental" "$H/target/cold/fast/deps" "$H/target/cold/fast" "$H/target/cold"; do old "$d" "4 days ago"; done
  mb "$T/outside/precious" 1 "9 days ago"; ln -sfn "$T/outside" "$H/target/link"; old "$H/target/link" "9 days ago"
}

# 1. under the high mark: only the dead job goes
build
out=$(python3 -I "$G" guard --home "$H" --cap-gb 1 --high 80 --low 70)
[ ! -e "$H/.bpjobs/1-1" ] && [ -e "$H/.bpjobs/2-2" ] && ok "a dead job dir goes, the live one stays" || no "job dirs: $out"
[ -e "$H/target/cold/fast/deps/libold.rlib" ] && [ -e "$H/target/hot/debug/incremental/crate-old" ] && ok "under the high mark no build output goes" || no "pruned under the mark: $out"

# 2. over the high mark (~19 MB on a 16 MB cap): the stale incremental first, then the cold dir; the hot dir's live state stays
build
out=$(python3 -I "$G" guard --home "$H" --cap-gb 0.016 --high 80 --low 70)
[ ! -e "$H/target/hot/debug/incremental/crate-old" ] && ok "a stale incremental crate goes" || no "stale incremental kept: $out"
[ -e "$H/target/hot/debug/incremental/crate-new" ] && [ -e "$H/target/hot/debug/deps/libnew.rlib" ] && ok "the hot target dir keeps its live state" || no "hot dir pruned: $out"
[ ! -e "$H/target/cold/fast/deps/libold.rlib" ] && ok "old deps of the cold dir go" || no "cold deps kept: $out"
[ -f "$T/outside/precious" ] && ok "a symlink in target/ is not followed" || no "followed a symlink: $out"
echo "$out" | grep -q '^boat-disk: .*% -> .*%.*removed' && ok "it logs what it removed" || no "no log line: $out"
pct=$(python3 -I "$G" report --home "$H" --cap-gb 0.016 | python3 -I -c 'import sys,json;print(json.load(sys.stdin)["pct"])')
python3 -I -c "import sys;sys.exit(0 if float('$pct') < 80 else 1)" && ok "under the high mark afterwards ($pct%)" || no "still $pct% full: $out"

# 3. --dry-run removes nothing; report is one JSON line with the fields boat-pool.sh status reads
build
python3 -I "$G" guard --home "$H" --cap-gb 0.02 --dry-run > /dev/null
[ -e "$H/target/cold/fast/deps/libold.rlib" ] && [ -e "$H/.bpjobs/1-1" ] && ok "--dry-run removes nothing" || no "--dry-run removed files"
python3 -I "$G" report --home "$H" --cap-gb 1 | python3 -I -c 'import sys,json;j=json.load(sys.stdin);assert all(k in j for k in ("at","pct","used_gb","size_gb","target_gb","sccache_gb"))' && ok "report is one JSON line" || no "report"

# 4. a home of / or a missing one is refused
python3 -I "$G" guard --home / > /dev/null 2>&1; [ $? = 2 ] && ok "refuses / as the home" || no "accepted / as the home"
python3 -I "$G" guard --home "$T/nope" > /dev/null 2>&1; [ $? = 2 ] && ok "refuses a missing home" || no "accepted a missing home"

# 5. sccache over SCCACHE_CACHE_SIZE: oldest files go down to 90 % of the cap
build; mb "$H/.cache/sccache/a/old" 4 "5 days ago"; mb "$H/.cache/sccache/b/new" 4 "now"
SCCACHE_DIR= SCCACHE_CACHE_SIZE=6M python3 -I "$G" guard --home "$H" --cap-gb 1 > /dev/null
[ ! -e "$H/.cache/sccache/a/old" ] && [ -e "$H/.cache/sccache/b/new" ] && ok "sccache is trimmed to its cap, oldest first" || no "sccache trim"

# 6. the lazy home sets a directory's own mtime to when it was hydrated: age comes from the files inside
build; touch -h "$H/target/hot/debug/incremental/crate-old" "$H/target/cold" "$H/target/cold/fast" "$H/target/cold/fast/incremental/crate-c"
out=$(python3 -I "$G" guard --home "$H" --cap-gb 0.016 --high 80 --low 70)
[ ! -e "$H/target/hot/debug/incremental/crate-old" ] && [ ! -e "$H/target/cold/fast/deps/libold.rlib" ] && ok "a freshly hydrated dir with old files counts as old" || no "dir mtime trusted: $out"

# 7. lib-boat.sh bo_out reads the command's stdout from both shapes `boat exec --json` gives: one object, or a stream of lines for a longer command
out=$(bash -c '. "$1/scripts/lib-boat.sh"; { printf "%s\n" "{\"stdout\":\"one\\n\",\"exitCode\":0}"; printf "%s\n" "{\"type\":\"attached\"}" "{\"type\":\"stdout\",\"data\":\"two\\n\"}" "{\"type\":\"stderr\",\"data\":\"no\\n\"}" "{\"type\":\"exit\",\"exitCode\":0}"; } | bo_out' _ "$HERE")
[ "$out" = "$(printf 'one\ntwo')" ] && ok "bo_out reads both exec output shapes" || no "bo_out gave: $out"

exit "$fail"
