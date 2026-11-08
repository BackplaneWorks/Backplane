#!/bin/bash
# Builds and tests on a Boat sandbox when the machine's build slots are all taken (sourced by lib-buildslot.sh; used by scripts/cargo-q and scripts/dev-build.sh).
#
# What happens: bq_run (lib-buildslot.sh) finds no free local slot and, instead of waiting, runs the same script under a BOAT LEASE (a flock file like a slot,
# BUILD_LOCK_DIR/boat.N, N below BP_BOAT_POOL). The script then
#   1. bo_box   finds or starts pool box bp-build-N (resumes it if stopped; creates it from the snapshot bp-build-base otherwise);
#   2. bo_sync  copies the exact worktree state (tracked and untracked files, committed or not, nothing git ignores) with rsync over `boat ssh`; only changed
#               files travel, and the box deletes what the worktree does not have;
#   3. bo_run   runs the same command there (scripts/cargo-q or scripts/dev-build.sh itself, so the slots, sccache, -j and test TMPDIR rules are the same),
#               streaming its output, with the same exit code;
#   4. bo_pull  copies the artifacts back to the paths a local run would have written, after a sha256 check;
#   5. bo_finish stops the job's leftovers and arms the idle stop.
# Anything that goes wrong BEFORE the command starts is exit code 98: the caller then waits for a local slot as it always did. BP_NO_BOAT=1 never offloads.
# Secrets: nothing of the person's goes to a box. The box is made from a snapshot taken on a sandbox with no environment (`--no-env`, env bp-build injects nothing),
# the job gets only the variables bo_env allows (build knobs, never a name that looks like a credential), and only `git ls-files` sources are copied (no .git, no home).
# Env: BP_NO_BOAT, BP_BOAT_POOL (boxes, default 2), BP_BOAT_TYPE (small|default|large|xlarge; default large), BP_BOAT_IDLE (seconds idle before the box is stopped, default 300),
#      BP_BOAT_COOLDOWN (seconds without offloading after boat itself failed, default 300), BP_BOAT_PASS (extra variable names to forward), BP_BOAT_BASE (snapshot name).
BO_HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
bo_dir() { echo "${BUILD_LOCK_DIR:-$HOME/.cache/backplane-build.locks}"; }
bo_say() { echo "boat: $*" >&2; }
bo_b() { boat --no-update "$@"; }
# the python helper for JSON: bo_py CODE ARGS... (stdin is the JSON)
bo_py() { local c=$1; shift; python3 -I -c "$c" "$@"; }

# Offloading is possible: not switched off, not already on a box, the tools are here, boat did not fail a moment ago.
bo_ready() {
  [ -z "${BP_NO_BOAT:-}" ] && [ -z "${BP_ON_BOAT:-}" ] || return 1
  local c f; for c in boat python3 rsync git flock base64 gzip sha256sum; do command -v "$c" > /dev/null 2>&1 || return 1; done
  f=$(bo_dir)/boat.down
  [ -f "$f" ] && [ $(( $(date +%s) - $(stat -c %Y "$f" 2>/dev/null || echo 0) )) -lt "${BP_BOAT_COOLDOWN:-300}" ] && return 1
  return 0
}
bo_down() { mkdir -p "$(bo_dir)"; : > "$(bo_dir)/boat.down"; bo_say "$*; building locally (no offload for ${BP_BOAT_COOLDOWN:-300}s)"; }

# bo_try CMD...: run CMD under the first free boat lease (BP_BOAT_IDX=N in its environment). 200 = every lease is taken; 98 = CMD said the box was unavailable.
bo_try() { local d n=${BP_BOAT_POOL:-2} i rc; d=$(bo_dir); mkdir -p "$d"
  for ((i = 0; i < n; i++)); do flock -n -E 200 -o "$d/boat.$i" env BP_BOAT_IDX=$i "$@"; rc=$?; [ "$rc" -ne 200 ] && return "$rc"; done; return 200; }

# bo_box IDX: print the id of pool box bp-build-IDX, running (started, resumed or created from the snapshot)
bo_box() { local i=$1 name=bp-build-$1 rec id st n=0 j
  while [ "$n" -lt 40 ]; do
    rec=$(bo_b list --all --json 2>/dev/null | bo_py 'import sys,json
n=sys.argv[1]
try: s=[x for x in json.load(sys.stdin)["sandboxes"] if x.get("name")==n and x.get("state") not in ("deleted","deleting")]
except Exception: sys.exit(1)
s.sort(key=lambda x: x["state"] in ("stopped", "archived"))   # a running box before a stopped one (`list` without --all leaves the stopped ones out)
print(*(s[0]["id"], s[0]["state"]) if s else ("", "none"))' "$name") || { bo_say "cannot list sandboxes"; return 1; }
    id=${rec% *}; st=${rec##* }
    case "$st" in
      ready|idle|running|provisioned) echo "$id"; return 0;;
      stopped|archived) bo_say "resuming $name ($id)"; bo_b resume "$id" > /dev/null 2>&1 || { bo_say "resume of $id failed"; return 1; };;
      none) bo_say "creating $name from the snapshot ${BP_BOAT_BASE:-bp-build-base}"
            j=$(bo_b new --from "${BP_BOAT_BASE:-bp-build-base}" --name "$name" --environment bp-build --no-env --type "${BP_BOAT_TYPE:-large}" --ttl 1800 --json 2>&1 | tail -1)
            case "$j" in *'"ready"'*) ;; *) bo_say "cannot create $name: ${j:0:200}"; return 1;; esac;;
      *) sleep 3;;
    esac
    n=$((n + 1)); [ "$st" = stopped ] && [ "$n" -ge 2 ] && sleep 1
  done; bo_say "$name never became ready"; return 1; }

# The names a job may carry to the box: build knobs. Never a name that looks like a credential, whatever BP_BOAT_PASS says.
bo_env() { local v; for v in $(compgen -e); do
    case "$v" in
      *TOKEN*|*KEY*|*SECRET*|*PASS*|*CRED*|*AUTH*|*COOKIE*|*SESSION*) continue;;
      RUST_BACKTRACE|RUST_LOG|RUST_TEST_THREADS|RUSTFLAGS|RUSTDOCFLAGS|CARGO_INCREMENTAL|CARGO_TERM_*|CARGO_PROFILE_*|CARGO_BUILD_JOBS|BACKPLANE_COMMIT|PROFILE|GPUI_FEATURES|NO_SCCACHE|JOBS|BP_TEST_*|BPT_KEEP) ;;
      *) case " ${BP_BOAT_PASS:-} " in *" $v "*) ;; *) continue;; esac;;
    esac
    printf 'export %s=%q\n' "$v" "${!v}"
  done; }

# bo_sync ID ROOT JOB: the worktree at ROOT, exactly, in ~/ws/src on the box
bo_sync() { local id=$1 root=$2 job=$3 lf rc=0
  lf=$(mktemp "${BP_TMP:-${TMPDIR:-/tmp}}/bo-list.XXXXXX") || return 1
  git -C "$root" ls-files -z --cached --others --exclude-standard > "$lf" || { rm -f "$lf"; return 1; }
  rsync -rlp --checksum --no-times --ignore-missing-args --from0 --files-from="$lf" -e "$BO_HERE/boat-ssh.sh" "$root/" "$id:/home/user/ws/src/" > /dev/null || rc=$?
  if [ "$rc" = 0 ]; then
    bo_b exec "$id" --timeout 120 "mkdir -p ~/.bpjobs/$job && echo $(gzip -c "$lf" | base64 -w0) | base64 -d | gunzip > ~/.bpjobs/$job/list && python3 -I ~/ws/src/scripts/boat-prune.py ~/ws/src ~/.bpjobs/$job/list" > /dev/null || rc=$?
  fi
  rm -f "$lf"; return "$rc"; }

# bo_run ID JOB CWD BODY: write the job script (the allowed env, cd CWD, BODY) to the box and run it; stdout and stderr are the command's, the exit code is its own
bo_run() { local id=$1 job=$2 cwd=$3 body=$4 s
  s=$({ echo '#!/bin/bash'; echo 'export PATH="$HOME/.cargo/bin:/usr/local/cargo/bin:$HOME/.bun/bin:/usr/local/bin:$PATH" BP_NO_BOAT=1 BP_ON_BOAT=1 BP_JOB=$HOME/.bpjobs/'"$job"
             bo_env; printf 'cd ~/ws/src/%q || exit 96\n' "$cwd"; printf '%s\n' "$body"; })
  bo_b exec "$id" --timeout 60 "echo $(printf '%s\n' "$s" | base64 -w0) | base64 -d > ~/.bpjobs/$job/run.sh" > /dev/null || return 98
  python3 -I "$BO_HERE/boat-run.py" "$id" bash "/home/user/.bpjobs/$job/run.sh" & BO_PID=$!
  trap 'kill -TERM "$BO_PID" 2>/dev/null; wait "$BO_PID" 2>/dev/null; exit 143' TERM HUP; trap 'kill -INT "$BO_PID" 2>/dev/null; wait "$BO_PID" 2>/dev/null; exit 130' INT
  wait "$BO_PID"; local rc=$?; trap - TERM HUP INT; [ "$rc" = 97 ] && { bo_say "lost the connection to $id; running locally instead"; return 98; }; return "$rc"; }

# bo_pull ID JOB REMOTEBASE LOCALBASE: the files listed in the job's `artifacts` (paths relative to REMOTEBASE), verified against the job's `sha256`, into LOCALBASE
bo_pull() { local id=$1 job=$2 rb=$3 lb=$4 lf sf rc=0
  lf=$(mktemp "${BP_TMP:-${TMPDIR:-/tmp}}/bo-art.XXXXXX"); sf=$(mktemp "${BP_TMP:-${TMPDIR:-/tmp}}/bo-sha.XXXXXX")
  bo_b exec "$id" --json --timeout 60 "cat ~/.bpjobs/$job/artifacts" 2>/dev/null | bo_py 'import sys,json;sys.stdout.write(json.load(sys.stdin)["stdout"])' > "$lf"
  bo_b exec "$id" --json --timeout 60 "cat ~/.bpjobs/$job/sha256" 2>/dev/null | bo_py 'import sys,json;sys.stdout.write(json.load(sys.stdin)["stdout"])' > "$sf"
  [ -s "$lf" ] || { rm -f "$lf" "$sf"; return 0; }
  mkdir -p "$lb"
  rsync -rlp --no-times --files-from="$lf" -e "$BO_HERE/boat-ssh.sh" "$id:$rb/" "$lb/" > /dev/null || rc=$?
  # the artifact is hashed again here: what was built on the box is what is on this disk
  [ "$rc" = 0 ] && { (cd "$lb" && sha256sum -c --quiet "$sf" > /dev/null) || { bo_say "an artifact does not match the hash computed on the box"; rc=99; }; }
  rm -f "$lf" "$sf"; return "$rc"; }

# bo_finish IDX ID JOB: after a job, however it ended
bo_finish() { local i=$1 id=$2 job=$3 d; d=$(bo_dir)
  [ -n "$id" ] && bo_b exec "$id" --timeout 30 "rm -rf ~/.bpjobs/$job" > /dev/null 2>&1
  date +%s > "$d/boat.$i.last"
  [ -n "$id" ] && bo_b extend "$id" --ttl $(( ${BP_BOAT_IDLE:-300} + 600 )) > /dev/null 2>&1   # the server's own limit, if this machine dies before the reaper below stops the box
  ( setsid nohup "$BO_HERE/boat-pool.sh" reap > /dev/null 2>&1 < /dev/null & ) 2> /dev/null; }

# ---- scripts/cargo-q on a box ------------------------------------------------------------------------------------------------------------------------------------
# bo_cargo_ok ARGS...: the cargo command can run on a box and give the same result: a command that builds or tests, with no path that only this machine has
bo_cargo_ok() { local root a
  case "${1:-}" in test|build|check|clippy|doc|bench|t|b|c) ;; *) return 1;; esac
  root=$(git rev-parse --show-toplevel 2> /dev/null) || return 1; case "$PWD/" in "$root"/*) ;; *) return 1;; esac
  for a in "$@"; do case "$a" in
    --no-run|--target-dir|--target-dir=*|--out-dir|--out-dir=*|--message-format|--message-format=*|--artifact-dir|--artifact-dir=*|--config|--config=*|--manifest-path|--manifest-path=*|-Z*) return 1;;
    /*) case "$a" in "$root"/*) ;; *) return 1;; esac;; esac; done
  return 0; }
# bo_cargo ARGS...: run `scripts/cargo-q ARGS` on box BP_BOAT_IDX, in the same directory of the same worktree. Returns the command's exit code, or 98 when the box could not be used.
bo_cargo() { local i=${BP_BOAT_IDX} id job root rel slug body rc lt tdir args=() a build=
  root=$(git rev-parse --show-toplevel) || return 98; rel=${PWD#"$root"}; rel=${rel#/}; slug=${rel//\//-}; slug=${slug:-root}
  job="$(date +%s)-$$"; unset BP_BOAT_IDX
  id=$(bo_box "$i") || { bo_down "no box"; return 98; }
  bo_say "all build slots are busy: cargo $1 runs on Boat bp-build-$i ($id) in $rel"
  bo_sync "$id" "$root" "$job" || { bo_down "could not copy the worktree to $id"; bo_finish "$i" "$id" "$job"; return 98; }
  for a in "$@"; do [ "$a" = --offline ] || args+=("$a"); done          # the box has the network and no registry mirror
  [ "$1" = build ] || [ "$1" = b ] && build=1
  [ -n "$build" ] && args+=(--message-format=json-render-diagnostics)
  body="export CARGO_TARGET_DIR=\$HOME/target/$slug; T=\$CARGO_TARGET_DIR"
  if [ -n "$build" ]; then body+=$'\n'"bash ~/ws/src/scripts/cargo-q $(printf '%q ' "${args[@]}") > \$BP_JOB/msgs; rc=\$?"$'\n'"[ \$rc = 0 ] && python3 -I ~/ws/src/scripts/boat-artifacts.py \$BP_JOB/msgs \$T \$BP_JOB/artifacts \$BP_JOB/sha256"$'\n''exit $rc'
  else body+=$'\n'"exec bash ~/ws/src/scripts/cargo-q $(printf '%q ' "${args[@]}")"; fi
  bo_run "$id" "$job" "$rel" "$body"; rc=$?
  if [ "$rc" = 98 ]; then bo_down "the run could not start"; bo_finish "$i" "$id" "$job"; return 98; fi
  if [ -n "$build" ] && [ "$rc" = 0 ]; then
    tdir=${CARGO_TARGET_DIR:-}; [ -n "$tdir" ] || { lt=$(cargo locate-project --workspace --message-format plain 2> /dev/null) && tdir=$(dirname "$lt")/target; }
    if [ -z "$tdir" ] || ! bo_pull "$id" "$job" "/home/user/target/$slug" "$tdir"; then bo_say "could not copy the build back"; rc=1; fi
  fi
  bo_finish "$i" "$id" "$job"; return "$rc"; }
