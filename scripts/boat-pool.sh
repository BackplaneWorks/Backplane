#!/bin/bash
# The Boat build boxes that scripts/cargo-q and scripts/dev-build.sh use when the local build slots are all taken (scripts/lib-boat.sh).
#   scripts/boat-pool.sh status          the pool: each box, its state, how long idle, what it has cost
#   scripts/boat-pool.sh reap            stop the boxes idle for BP_BOAT_IDLE seconds (default 300); runs by itself after every offloaded job and ends when none is running
#   scripts/boat-pool.sh stop            stop every pool box now (the disk is kept 14 days; the next job resumes it)
#   scripts/boat-pool.sh base            (re)build the snapshot every box starts from: a sandbox with NO environment (no logins, no tokens), scripts/boat-base.sh on it
#   scripts/boat-pool.sh delete          delete every pool box (not the snapshot)
set -u
HERE=$(cd "$(dirname "$0")" && pwd); . "$HERE/lib-boat.sh"
d=$(bo_dir); mkdir -p "$d"
# the pool boxes now: "N id state"
boxes() { bo_b list --all --json 2>/dev/null | bo_py 'import sys,json,re
for x in json.load(sys.stdin)["sandboxes"]:
    m = re.fullmatch(r"bp-build-(\d+)", x.get("name") or "")
    if m and x.get("state") not in ("deleted", "deleting"): print(m.group(1), x["id"], x["state"])'; }
live() { case "$1" in ready|idle|running|provisioned|starting|cloning) return 0;; esac; return 1; }
leased() { ( flock -n 9 ) 9> "$d/boat.$1" && return 1; return 0; }
case "${1:-status}" in
  status) now=$(date +%s); boxes | while read -r n id st; do
      l=$(cat "$d/boat.$n.last" 2>/dev/null || echo 0); s="idle $(( now - l ))s"; leased "$n" && s="in use"
      echo "bp-build-$n $id $st ($s) $(bo_b usage "$id" --json 2>/dev/null | bo_py 'import sys,json;j=json.load(sys.stdin);print("%ds, $%.3f" % (j["seconds"], j["dollars"]))' 2>/dev/null)"; done ;;
  reap) exec 8> "$d/boat.reaper"; flock -n 8 || exit 0
      while :; do act=0; now=$(date +%s)
        while read -r n id st; do live "$st" || continue
          if leased "$n"; then act=1; continue; fi
          l=$(cat "$d/boat.$n.last" 2>/dev/null || echo 0)
          if [ $(( now - l )) -ge "${BP_BOAT_IDLE:-300}" ]; then bo_b stop "$id" > /dev/null 2>&1 && echo "$(date +%T) stopped bp-build-$n ($id) after $(( now - l ))s idle" >> "$d/boat.log"; else act=1; fi
        done < <(boxes)
        [ "$act" = 0 ] && exit 0; sleep 20; done ;;
  stop) boxes | while read -r n id st; do live "$st" && bo_b stop "$id" > /dev/null 2>&1 && echo "stopped bp-build-$n"; done ;;
  delete) boxes | while read -r n id st; do bo_b delete "$id" --yes > /dev/null 2>&1 && echo "deleted bp-build-$n"; done ;;
  base) # a clean source box: no environment, so no logins of the person's are on it, then the snapshot
      src=$(bo_b new --name bp-build-base-src --environment bp-build --no-env --type default --ttl 3000 --json 2>&1 | tail -1 | bo_py 'import sys,json;print(json.load(sys.stdin)["id"])') || exit 1
      bo_b scp "$HERE/boat-base.sh" "$src:/home/user/boat-base.sh" > /dev/null 2>&1
      python3 -I "$HERE/boat-run.py" "$src" bash -c "'BP_BOAT_RUST=$(rustc --version | awk '{print $2}') bash /home/user/boat-base.sh 2>&1 | tail -3'" || exit 1
      bo_b snapshot "$src" "${BP_BOAT_BASE:-bp-build-base}" && bo_b delete "$src" --yes > /dev/null 2>&1 ;;
  *) sed -n '2,8p' "$0"; exit 2 ;;
esac
