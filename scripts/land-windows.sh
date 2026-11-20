#!/bin/sh
# The Windows gate of scripts/land.sh: the hub's tests on Windows, on this checkout's HEAD (the rebased tree land.sh gated on Linux), on one of
# our Azure VMs, before land.sh pushes anything. land.sh runs it under the bp-win-gate lock (scripts/bp-lock.sh), so landings queue for the VM.
#   1. a VM: bp-win (4 cores, the UI test box), else bp-winbuild (32 cores, the KiCad build VM; only while its winbuild-vm lock is free, which this then
#      holds, since a KiCad build deallocates the VM when it ends). Neither is started within LAND_WIN_GUARD_MIN (45) minutes of its daily
#      auto-shutdown (bp-win 07:00 UTC, bp-winbuild 05:00 America/New_York): the shutdown is a cost guard and is never fought. A deallocated VM is
#      started (az vm start) and waited for, at most LAND_WIN_START_S (480) seconds.
#   2. the tree: git push over ssh into C:\bp\gate\src on the VM (refs/heads/gate there; nothing goes to GitHub), checked out detached at HEAD and
#      cleaned (git clean -ffdx), keeping native\host\target: an incremental build between gates. A new VM's repository is seeded once from the public
#      GitHub repository, and cargo-nextest is installed if it is missing.
#   3. the tests, as ci.yml's hub job runs them on windows-latest, in native\host: cargo build --locked --bin bp-sh, then
#      cargo nextest run --locked --no-fail-fast --retries 1 -E "$(sh ../../scripts/ci-hub-windows-filter.sh)", at BelowNormal priority (bp-win's UI
#      tests, under bp-win-ui, keep the CPU first). While it runs, the VM's watchdog hold (C:\ProgramData\bp-watchdog\hold-until.txt) is kept at least
#      LAND_WIN_HOLD_MIN (90) minutes ahead (never shortened) and put back as it was at the end. The whole output goes to <git-dir>/land-windows.log.
#   4. the verdict: main itself is not green on Windows, so a test listed in scripts/land-windows-known.txt (each with why) does not hold a landing when it
#      fails; any other failure does, and so does a Windows build that fails. Flaky passes, known failures, and known ones that pass again are named.
# Exit: 0 passed; 1 the tests (or the Windows build) failed; 75 held: no VM could run it, with one line naming each VM tried and why.
# A VM lost under a run (ssh drops: deallocated, shut down) is a VM that could not run it, never a pass: the next VM is tried.
# Env: LAND_WIN_VMS (bp-win bp-winbuild), LAND_WIN_SSH (ssh), LAND_WIN_AZ (az), WINVM_RG (BP-WIN-RG), LAND_WIN_START_S, LAND_WIN_GUARD_MIN, LAND_WIN_HOLD_MIN,
# LAND_WIN_LOG, BACKPLANE_LOCK_DIR (/tmp: where a held winbuild-vm lock is seen).
set -u
here=$(cd "$(dirname "$0")" && pwd)
ssh_prog=${LAND_WIN_SSH:-ssh}; az_prog=${LAND_WIN_AZ:-az}; rg=${WINVM_RG:-BP-WIN-RG}
start_s=${LAND_WIN_START_S:-480}; guard_min=${LAND_WIN_GUARD_MIN:-45}; hold_min=${LAND_WIN_HOLD_MIN:-90}
log=${LAND_WIN_LOG:-$(git rev-parse --git-dir)/land-windows.log}
say() { printf '%s\n' "land: windows: $*" >&2; }
# vm | ssh hosts | its daily shutdown (tz hour) | the lock it needs besides bp-win-gate
vm_row() { case "$1" in
  bp-win) echo "bp-win|UTC 7|";;
  bp-winbuild) echo "bp-winbuild bp-winbuild-vnet|America/New_York 5|winbuild-vm";;
  *) echo "$1||";; esac; }
rssh() { "$ssh_prog" -o BatchMode=yes -o ConnectTimeout=15 -o ServerAliveInterval=30 -o ServerAliveCountMax=6 "$@"; }
# minutes from now to the next HH:00 in a time zone
until_min() { set -- $(TZ=$1 date +'%H %M') "$2"; h=${1#0}; m=${2#0}; d=$(( $3 * 60 - (h * 60 + m) )); [ $d -ge 0 ] || d=$((d + 1440)); echo $d; }
lock_held() { f=${BACKPLANE_LOCK_DIR:-/tmp}/bp-$1.lock; [ -e "$f" ] && command -v flock >/dev/null 2>&1 && ! flock -n "$f" true; }

if [ "${1:-}" != --vm ]; then
  # the VMs in order; each attempt runs in its own process (under its extra lock), and says why it could not run in $why
  whyf=$(mktemp "${TMPDIR:-/tmp}/land-win-why.XXXXXX"); trap 'rm -f "$whyf"' EXIT
  tried=
  for vm in ${LAND_WIN_VMS:-bp-win bp-winbuild}; do
    IFS='|' read -r hosts shut extra <<EOF
$(vm_row "$vm")
EOF
    why=
    if [ -n "$shut" ] && [ "$guard_min" -gt 0 ] && m=$(until_min $shut) && [ "$m" -lt "$guard_min" ]; then why="its daily auto-shutdown is in $m min"
    elif [ -n "$extra" ] && lock_held "$extra"; then why="$extra is held (a KiCad build deallocates it when it ends)"
    else
      : > "$whyf"; rc=0
      if [ -n "$extra" ]; then LAND_WIN_WHY=$whyf "$here/bp-lock.sh" "$extra" -- sh "$0" --vm "$vm" || rc=$?
      else LAND_WIN_WHY=$whyf sh "$0" --vm "$vm" || rc=$?; fi
      [ "$rc" = 0 ] && exit 0
      [ "$rc" = 75 ] || exit 1
      why=$(cat "$whyf"); [ -n "$why" ] || why="could not run the gate"
    fi
    say "$vm: $why"
    tried="${tried:+$tried; }$vm: $why"
  done
  printf '%s\n' "land: HELD: the Windows gate could not run, so nothing is pushed or merged. Tried ${tried:-no VM (LAND_WIN_VMS is empty)}. Land again when one is up; LAND_WINDOWS=skip only on a person's or lead's word." >&2
  exit 75
fi

# --vm VM: reach it, send the tree, run the tests
vm=$2
IFS='|' read -r hosts shut extra <<EOF
$(vm_row "$vm")
EOF
unavailable() { printf '%s' "$*" > "${LAND_WIN_WHY:-/dev/null}"; exit 75; }
reach() { for h in $hosts; do rssh "$h" 'exit 0' >/dev/null 2>&1 && { echo "$h"; return 0; }; done; return 1; }
host=$(reach) || {
  power=$("$az_prog" vm get-instance-view -g "$rg" -n "$vm" --query "instanceView.statuses[?starts_with(code,'PowerState/')].code|[0]" -o tsv 2>/dev/null) ||
    unavailable "no ssh answer, and az could not read its power state (az login?)"
  power=${power#PowerState/}
  case "$power" in
    running) unavailable "running, but ssh does not answer";;
    deallocated|stopped|deallocating|stopping|starting) ;;
    *) unavailable "no ssh answer, power state '${power:-unknown}'";;
  esac
  [ "$start_s" -gt 0 ] || unavailable "$power (LAND_WIN_START_S=0: not started)"
  say "$vm is $power: starting it (at most ${start_s}s)"
  end=$(( $(date +%s) + start_s ))
  [ "$power" = starting ] || timeout "$start_s" "$az_prog" vm start -g "$rg" -n "$vm" -o none 2>/dev/null || unavailable "$power, and az vm start failed"
  while :; do host=$(reach) && break; [ "$(date +%s)" -lt "$end" ] || unavailable "started, but ssh did not answer within ${start_s}s"; sleep 5; done
}
sha=$(git rev-parse HEAD)
t0=$(date +%s)
say "on $vm ($host): sending $(git rev-parse --short HEAD)"
bash_ps="\$p = Get-Process -Id \$PID; \$p.PriorityClass = 'BelowNormal'; & 'C:\\Program Files\\Git\\bin\\bash.exe' -s"
# the repository there (seeded once from GitHub's public main: the push below then sends only what it lacks), and nextest
rssh "$host" "$bash_ps" > "$log" 2>&1 <<'EOF'
set -e
mkdir -p /c/bp/gate; cd /c/bp/gate
if [ ! -d src/.git ]; then git init -q src; git -C src config core.autocrlf false; git -C src config receive.denyCurrentBranch ignore
  git -C src fetch -q https://github.com/BackplaneWorks/Backplane main:refs/heads/seed; fi
command -v cargo-nextest >/dev/null || curl -fsSL https://get.nexte.st/latest/windows-tar | tar -xzf - -C ~/.cargo/bin
EOF
rc=$?; [ $rc = 0 ] || { [ $rc = 255 ] && unavailable "ssh dropped while preparing ($host)"; say "preparing $vm failed:"; tail -n 20 "$log" >&2; unavailable "could not prepare C:\\bp\\gate (see $log)"; }
git -c core.sshCommand="$ssh_prog -o BatchMode=yes -o ConnectTimeout=15" push -q -f --receive-pack='git receive-pack' "$host:C:/bp/gate/src" "$sha:refs/heads/gate" >> "$log" 2>&1 ||
  unavailable "could not push the tree to $host (see $log)"
# the run; its own exit codes: 0 passed, 1 failed (build or tests)
rssh "$host" "$bash_ps" >> "$log" 2>&1 <<EOF
set -u
f=/c/ProgramData/bp-watchdog/hold-until.txt; old=; new=\$(date -u -d "+$hold_min minutes" +%FT%TZ)
if [ -d /c/ProgramData/bp-watchdog ]; then old=\$(tr -d '\r\n\357\273\277' < "\$f" 2>/dev/null || true)
  if [ -z "\$old" ] || [ "\$(printf '%.19s' "\$old")" \< "\$(printf '%.19s' "\$new")" ]; then printf '%s' "\$new" > "\$f" && echo "hold: \$new" || new=; else new=; fi; fi
put_back() { [ -n "\$new" ] && [ "\$(cat "\$f" 2>/dev/null)" = "\$new" ] || return 0; if [ -n "\$old" ]; then printf '%s' "\$old" > "\$f"; else rm -f "\$f"; fi; }
trap put_back EXIT
# TMP/TEMP: a Windows folder, as GitHub's runner has (D:\\a\\_temp); Git bash's /tmp makes the hub's stand-ins report /tmp/... paths. Emptied each run.
rm -rf /c/bp/gate/tmp; mkdir -p /c/bp/gate/tmp; export TMP=/c/bp/gate/tmp TEMP=/c/bp/gate/tmp
cd /c/bp/gate/src && git checkout -q -f --detach $sha && git clean -q -ffdx -e /native/host/target || exit 1
echo "tree \$(git rev-parse HEAD^{tree}) at $sha"
cd native/host && cargo build --locked --bin bp-sh || exit 1
cargo nextest run --locked --no-fail-fast --retries 1 -E "\$(sh ../../scripts/ci-hub-windows-filter.sh)" || exit 1
EOF
rc=$?
dt=$(( $(date +%s) - t0 )); took="$((dt / 60))m$((dt % 60))s"
[ $rc = 255 ] && unavailable "the connection dropped after ${took} of the run (deallocated or shut down?); see $log"
summary=$(grep -a 'Summary \[' "$log" | tail -n 1 | sed 's/^ *//')
# the final results: nextest lists them after its Summary line (FLAKY passed on a retry; FAIL, TIMEOUT, SIG* failed every try)
final=$(awk '/Summary \[/ { f = 1; next } f' "$log" | grep -a -E '^ *(TRY [0-9]+ )?(FLAKY|FAIL|TIMEOUT|SIG[A-Z]+|ABORT|LEAK-FAIL)' | sed -E 's/^ *//')
name() { awk '{ print $NF }'; }
flaky=$(printf '%s\n' "$final" | grep -a '^FLAKY' | name | sort -u)
failed=$(printf '%s\n' "$final" | grep -a -v '^FLAKY' | grep -a . | name | sort -u)
known=$(sed 's/#.*//' "$here/land-windows-known.txt" 2>/dev/null | awk 'NF { print ($1 == "flaky" ? $2 : $1) }' | sort -u)
steady=$(sed 's/#.*//' "$here/land-windows-known.txt" 2>/dev/null | awk 'NF && $1 != "flaky" { print $1 }')
new_fail=$(printf '%s\n' "$failed" | grep -a . | grep -a -v -x -F "$known")
old_fail=$(printf '%s\n' "$failed" | grep -a . | grep -a -x -F "$known")
healed=$(for k in $steady; do grep -a -q -E "^ *PASS \[.* $k\$" "$log" && ! printf '%s\n' "$failed" | grep -q -x -F "$k" && echo "$k"; done)
list() { printf '%s\n' "$1" | grep -a . | sed 's/^/    /' >&2; }
[ -z "$flaky" ] || { say "flaky, passed on its retry:"; list "$flaky"; }
[ -z "$old_fail" ] || { say "known Windows failures (scripts/land-windows-known.txt), still failing, not this change's:"; list "$old_fail"; }
[ -z "$healed" ] || { say "these known failures pass now: take them off scripts/land-windows-known.txt:"; list "$healed"; }
if [ -z "$summary" ]; then
  say "FAILED on $vm after $took: the Windows build failed, no test ran"
  grep -a -E -A6 '^error(\[E[0-9]+\])?:' "$log" | head -n 40 >&2
elif [ $rc = 0 ] || [ -z "$new_fail" ]; then say "PASSED on $vm in $took: $summary"; exit 0
else say "FAILED on $vm after $took: $summary; failing on Windows, and not known to:"; list "$new_fail"; fi
say "the whole output: $log"
exit 1
