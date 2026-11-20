#!/bin/sh
# The Windows gate of scripts/land.sh: the hub's tests on Windows, on this checkout's HEAD (the rebased tree land.sh gated on Linux), on one of
# our Azure VMs, before land.sh pushes anything. land.sh runs it under the bp-win-gate lock (scripts/bp-lock.sh), so landings queue for it.
#   1. a VM with one owner at a time: bp-win (4 cores, the UI test box) only when the gate can take bp-win-ui at once, which it then holds for the whole
#      run (the UI tests' timing rows are never run beside it); else bp-winbuild (32 cores, the KiCad build VM) when it can take winbuild-vm at once (a
#      KiCad build deallocates the VM when it ends). Neither lock is waited for (BP_LOCK_TRY). No gate starts that would cross a VM's daily auto-shutdown
#      (bp-win 07:00 UTC, bp-winbuild 05:00 America/New_York, 10:00 UTC in winter): it needs the VM's last measured gate time (in LAND_WIN_STATE,
#      default ~/.cache/bp-land-windows/<vm>.minutes; LAND_WIN_EST_MIN, 45, until one is measured) plus LAND_WIN_MARGIN_MIN (15) before it, and the
#      run is cut off 5 minutes before the shutdown. The shutdowns are cost guards and are never fought. A deallocated VM is started (az vm start)
#      and waited for, at most LAND_WIN_START_S (480) seconds.
#   2. the tree: git push over ssh into C:\bp\gate\src on the VM (refs/heads/gate there; nothing goes to GitHub), checked out detached at HEAD and
#      cleaned (git clean -ffdx), keeping native\host\target: an incremental build between gates. A new VM's repository is seeded once from the public
#      GitHub repository; Rust (rustup, stable MSVC) and cargo-nextest are installed if they are missing.
#   3. the tests, as ci.yml's hub job runs them on windows-latest: cargo build --locked --bin bp-sh, then cargo nextest run --locked --no-fail-fast
#      --retries 1 -E "$(sh scripts/ci-hub-windows-filter.sh)", with TMP/TEMP in C:\bp\gate\tmp. While it runs, the VM's watchdog hold
#      (C:\ProgramData\bp-watchdog\hold-until.txt) is kept at least LAND_WIN_HOLD_MIN (90) minutes ahead (never shortened) and put back at the end.
#      The whole output goes to <git-dir>/land-windows.log.
#   4. nothing of the gate stays on the VM: a sweep stops every process it made (the run's shell and its children, cargo, nextest, rustc, the test
#      programs and their stand-ins: whatever runs from or names C:\bp\gate) and deletes C:\bp\gate\tmp, before each run (what a killed gate left) and
#      after it, on every exit: passed, failed, timed out, interrupted or terminated. Other lanes' processes are never touched.
#   5. the verdict: main itself is not green on Windows, so a test listed in scripts/land-windows-known.txt (each with why) does not hold a landing when it
#      fails; any other failure does, and so does a Windows build that fails. Flaky passes, known failures, and known ones that pass again are named.
# Exit: 0 passed; 1 the tests (or the Windows build) failed; 75 held: no VM could run it, with one line naming each VM tried and why.
# A VM lost under a run (ssh drops: deallocated, shut down) or a run cut off is a VM that could not run it, never a pass: the next VM is tried.
# Env: LAND_WIN_VMS (bp-win bp-winbuild), LAND_WIN_SSH (ssh), LAND_WIN_AZ (az), WINVM_RG (BP-WIN-RG), LAND_WIN_START_S, LAND_WIN_EST_MIN, LAND_WIN_MARGIN_MIN,
# LAND_WIN_STATE, LAND_WIN_RUN_MAX_S (7200), LAND_WIN_THREADS (bp-winbuild 16), LAND_WIN_HOLD_MIN, LAND_WIN_LOG, BACKPLANE_LOCK_DIR (/tmp: where a held lock is seen).
set -u
here=$(cd "$(dirname "$0")" && pwd)
ssh_prog=${LAND_WIN_SSH:-ssh}; az_prog=${LAND_WIN_AZ:-az}; rg=${WINVM_RG:-BP-WIN-RG}
start_s=${LAND_WIN_START_S:-480}; hold_min=${LAND_WIN_HOLD_MIN:-90}
est_min=${LAND_WIN_EST_MIN:-45}; margin_min=${LAND_WIN_MARGIN_MIN:-15}; run_max_s=${LAND_WIN_RUN_MAX_S:-7200}
state=${LAND_WIN_STATE:-$HOME/.cache/bp-land-windows}
log=${LAND_WIN_LOG:-$(git rev-parse --git-dir)/land-windows.log}
say() { printf '%s\n' "land: windows: $*" >&2; }
# vm | ssh hosts | its daily shutdown (tz hour) | its owner's lock, which the gate takes for the run
vm_row() { case "$1" in
  bp-win) echo "bp-win|UTC 7|bp-win-ui";;
  bp-winbuild) echo "bp-winbuild bp-winbuild-vnet|America/New_York 5|winbuild-vm";;
  *) echo "$1||";; esac; }
rssh() { "$ssh_prog" -o BatchMode=yes -o ConnectTimeout=15 -o ServerAliveInterval=30 -o ServerAliveCountMax=6 "$@"; }
# a VM's expected gate time: its last measured one, else LAND_WIN_EST_MIN
need_min() { n=$(cat "$state/$1.minutes" 2>/dev/null) && [ "$n" -ge 0 ] 2>/dev/null || n=$est_min; echo "$n"; }
# the shutdown's hour in UTC, for the line (05:00 New York is 10:00 UTC in winter, 09:00 in summer)
shut_utc() { TZ=UTC date -d "TZ=\"$1\" $2:00" +%H:%M 2>/dev/null || echo "$2:00 $1"; }
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
    why=; left=
    if [ -n "$shut" ] && need=$(( $(need_min "$vm") + margin_min )) && left=$(until_min $shut) && [ "$left" -lt "$need" ]; then
      why="a gate now would cross its $(shut_utc $shut) UTC auto-shutdown (it needs about $need min with the margin, $left min are left)"
    elif [ -n "$extra" ] && lock_held "$extra"; then why="$extra is held (its owner's work runs there; the gate does not run beside it)"
    else
      : > "$whyf"; rc=0
      if [ -n "$extra" ]; then LAND_WIN_WHY=$whyf LAND_WIN_LEFT=${left:-} BP_LOCK_TRY=1 "$here/bp-lock.sh" "$extra" -- sh "$0" --vm "$vm" || rc=$?
      else LAND_WIN_WHY=$whyf LAND_WIN_LEFT=${left:-} sh "$0" --vm "$vm" || rc=$?; fi
      [ "$rc" = 0 ] && exit 0
      [ "$rc" -lt 128 ] || { say "$vm: the gate was stopped (signal $((rc - 128))); swept"; exit 1; }
      [ "$rc" = 75 ] || exit 1
      why=$(cat "$whyf"); [ -n "$why" ] || why="$extra is held (its owner's work runs there; the gate does not run beside it)"
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
# the run's PowerShell carries the token bp-land-gate in its command line, so the sweep finds it and everything under it
bash_ps="\$env:BP_GATE = 'bp-land-gate'; \$p = Get-Process -Id \$PID; \$p.PriorityClass = 'BelowNormal'; & 'C:\\Program Files\\Git\\bin\\bash.exe' -s"
# The sweep: only what the gate made. A process that runs from under C:\bp\gate (the test programs, bp-sh, the stand-ins in C:\bp\gate\tmp), whose
# command line carries the token bp-land-gate (the run's PowerShell) or names C:\bp\gate\src\native\host\ (cargo and nextest by --manifest-path,
# rustc by its out-dir), the run's bash (C:\bp\gate\run.pid), and their descendants (younger than their parent); never this sweep or its
# ancestors, and never a process that merely names C:\bp\gate (another lane looking at it). Then C:\bp\gate\tmp goes. Sent encoded (UTF-16LE base64), so its own command line names no path.
sweep_ps='$ErrorActionPreference = "SilentlyContinue"; $g = "C:\bp\gate"
$all = @(Get-CimInstance Win32_Process); $by = @{}; foreach ($p in $all) { $by[[int]$p.ProcessId] = $p }
$mine = @{}; $q = [int]$PID; while ($by.ContainsKey($q) -and -not $mine.ContainsKey($q)) { $mine[$q] = 1; $q = [int]$by[$q].ParentProcessId }
$hit = @{}
foreach ($p in $all) { $e = [string]$p.ExecutablePath; $c = [string]$p.CommandLine
  if ($e -like "$g\*" -or $c -like "*bp-land-gate*" -or $c -like "*$g\src\native\host\*") { $hit[[int]$p.ProcessId] = 1 } }
$f = "$g\run.pid"; if (Test-Path $f) { $r = 0; if ([int]::TryParse((Get-Content $f -Raw).Trim(), [ref]$r) -and $by.ContainsKey($r)) { $hit[$r] = 1 } }
do { $n = $hit.Count; foreach ($p in $all) { $pp = [int]$p.ParentProcessId
  if ($hit.ContainsKey($pp) -and -not $hit.ContainsKey([int]$p.ProcessId) -and $p.CreationDate -ge $by[$pp].CreationDate) { $hit[[int]$p.ProcessId] = 1 } } } while ($hit.Count -ne $n)
$gone = @(); foreach ($id in @($hit.Keys)) { if (-not $mine.ContainsKey($id)) { $gone += "$($by[$id].Name)#$id"; Stop-Process -Id $id -Force } }
Start-Sleep -Milliseconds 500; Remove-Item -Recurse -Force "$g\tmp", $f
"sweep: stopped $($gone.Count)$(if ($gone.Count) { ": " + ($gone -join " ") }); C:\bp\gate\tmp removed"'
sweep_cmd="powershell -NoProfile -NonInteractive -EncodedCommand $(printf '%s' "$sweep_ps" | iconv -f utf-8 -t utf-16le | base64 -w0)"
sweep() { # sweep WHEN: what it stopped goes to the log, and is said when it found any
  out=$(timeout 180 "$ssh_prog" -o BatchMode=yes -o ConnectTimeout=15 "$host" "$sweep_cmd" 2>&1 | tr -d '\r' | grep -a '^sweep:') || out="sweep: FAILED (no answer from $host)"
  printf '%s %s\n' "$1" "$out" >> "$log"
  case "$out" in "sweep: stopped 0;"*) ;; *) say "$1 $out";; esac; }
rpid=; done_=
finish() { [ -n "$done_" ] && return; done_=1; [ -z "$rpid" ] || kill "$rpid" 2>/dev/null; sweep "after the run:"; }
trap finish EXIT; trap 'exit 130' INT; trap 'exit 143' TERM; trap 'exit 129' HUP
: > "$log"; sweep "before the run (what an earlier gate left):"
# the repository there (seeded once from GitHub's public main: the push below then sends only what it lacks), and nextest
rssh "$host" "$bash_ps" >> "$log" 2>&1 <<'EOF'
set -e
mkdir -p /c/bp/gate; cd /c/bp/gate
if [ ! -d src/.git ]; then git init -q src; git -C src config core.autocrlf false; git -C src config receive.denyCurrentBranch ignore
  git -C src fetch -q https://github.com/BackplaneWorks/Backplane main:refs/heads/seed; fi
export PATH="$HOME/.cargo/bin:$PATH"
# a VM that has no Rust yet (bp-winbuild built only KiCad): rustup's stable MSVC toolchain, once (Visual Studio's build tools are there)
command -v cargo >/dev/null || { curl -fsSL -o /c/bp/gate/rustup-init.exe https://win.rustup.rs/x86_64 && /c/bp/gate/rustup-init.exe -y -q --profile minimal --default-toolchain stable && rm -f /c/bp/gate/rustup-init.exe; }
command -v cargo-nextest >/dev/null || { mkdir -p ~/.cargo/bin && curl -fsSL https://get.nexte.st/latest/windows-tar | tar -xzf - -C ~/.cargo/bin; }
EOF
rc=$?; [ $rc = 0 ] || { [ $rc = 255 ] && unavailable "ssh dropped while preparing ($host)"; say "preparing $vm failed:"; tail -n 20 "$log" >&2; unavailable "could not prepare C:\\bp\\gate (see $log)"; }
git -c core.sshCommand="$ssh_prog -o BatchMode=yes -o ConnectTimeout=15" push -q -f --receive-pack='git receive-pack' "$host:C:/bp/gate/src" "$sha:refs/heads/gate" >> "$log" 2>&1 ||
  unavailable "could not push the tree to $host (see $log)"
# test threads: nextest's default (one per core) on bp-win's 4, as GitHub's runner; at most 16 on bp-winbuild's 32, where the hub's timing-bound
# tests (written for 4 cores) failed both tries beside 31 others and passed alone (a_job_holds_its_lock..., checkpoint::stop_while..., 2026-11-20)
threads=; case "$vm" in bp-winbuild) threads="--test-threads ${LAND_WIN_THREADS:-16}";; *) [ -z "${LAND_WIN_THREADS:-}" ] || threads="--test-threads $LAND_WIN_THREADS";; esac
# the run's bound: LAND_WIN_RUN_MAX_S, and always 5 minutes before the VM's auto-shutdown
max_s=$run_max_s; [ -z "${LAND_WIN_LEFT:-}" ] || { b=$(( (LAND_WIN_LEFT - 5) * 60 - ($(date +%s) - t0) )); [ "$b" -ge "$max_s" ] || max_s=$b; }
[ "$max_s" -gt 0 ] || unavailable "too close to its auto-shutdown to run"
# the run; its own exit codes: 0 passed, 1 failed (build or tests). In the background, so a signal here is handled at once (and the sweep runs)
timeout -s TERM "$max_s" "$ssh_prog" -o BatchMode=yes -o ConnectTimeout=15 -o ServerAliveInterval=30 -o ServerAliveCountMax=6 "$host" "$bash_ps" >> "$log" 2>&1 <<EOF &
set -u
cat /proc/\$\$/winpid > /c/bp/gate/run.pid 2>/dev/null
f=/c/ProgramData/bp-watchdog/hold-until.txt; old=; new=\$(date -u -d "+$hold_min minutes" +%FT%TZ)
if [ -d /c/ProgramData/bp-watchdog ]; then old=\$(tr -d '\r\n\357\273\277' < "\$f" 2>/dev/null || true)
  if [ -z "\$old" ] || [ "\$(printf '%.19s' "\$old")" \< "\$(printf '%.19s' "\$new")" ]; then printf '%s' "\$new" > "\$f" && echo "hold: \$new" || new=; else new=; fi; fi
put_back() { [ -n "\$new" ] && [ "\$(cat "\$f" 2>/dev/null)" = "\$new" ] || return 0; if [ -n "\$old" ]; then printf '%s' "\$old" > "\$f"; else rm -f "\$f"; fi; }
trap put_back EXIT
export PATH="\$HOME/.cargo/bin:\$PATH"
# TMP/TEMP: a Windows folder, as GitHub's runner has (D:\\a\\_temp); Git bash's /tmp makes the hub's stand-ins report /tmp/... paths
mkdir -p /c/bp/gate/tmp; export TMP=/c/bp/gate/tmp TEMP=/c/bp/gate/tmp
cd /c/bp/gate/src && git checkout -q -f --detach $sha && git clean -q -ffdx -e /native/host/target || exit 1
echo "tree \$(git rev-parse HEAD^{tree}) at $sha"
m='C:\\bp\\gate\\src\\native\\host\\Cargo.toml'   # cargo and nextest name it: the sweep finds them by it
cargo build --manifest-path "\$m" --locked --bin bp-sh || exit 1
cd native/host && cargo nextest run --manifest-path "\$m" --locked --no-fail-fast --retries 1 $threads -E "\$(sh ../../scripts/ci-hub-windows-filter.sh)" || exit 1
EOF
rpid=$!; rc=0; wait "$rpid" || rc=$?; rpid=
dt=$(( $(date +%s) - t0 )); took="$((dt / 60))m$((dt % 60))s"
[ $rc = 255 ] && unavailable "the connection dropped after ${took} of the run (deallocated or shut down?); see $log"
[ $rc = 124 ] && unavailable "the run was cut off after ${took}, its bound (${max_s}s: LAND_WIN_RUN_MAX_S, or 5 min before the auto-shutdown); see $log"
summary=$(grep -a 'Summary \[' "$log" | tail -n 1 | sed 's/^ *//')
[ -z "$summary" ] || { mkdir -p "$state"; echo $(( (dt + 59) / 60 )) > "$state/$vm.minutes"; }   # the measured time, for the next shutdown check
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
