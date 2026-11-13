#!/bin/bash
# The bare-install journey, run ON a fresh Boat by scripts/bare-gate.sh (never on the desktop: it refuses unless user is `user`).
# A clean machine is a container from a stock distro image (IMAGE, default ubuntu:24.04) with nothing of ours in it; the harness (Xvfb, i3,
# xdotool, import, bun for the stand-in agent and the hub watcher) stays on the Boat host outside it. The container shares the host's network
# and X socket only; the person's home (/home/person) is a host folder so the harness can read the hub's token and logs.
#
#   test/bare/journey.sh OUT          env: IMAGE, SOURCE=release|archive, TAG (release: latest), ARCHIVE (archive: a native .tar.gz),
#                                          INSTALLER (archive: the install.sh made for it), FORK_DEMOS (dir with the fork's demos)
# Writes OUT/timings.tsv (step, seconds from first start or wall), OUT/findings.tsv (rank, where, what), OUT/checks.tsv (PASS/FAIL, what),
# OUT/shots/*.png, OUT/logs/*. Exit 1 on any FAIL.
set -u
OUT=$(realpath -m "$1"); HERE=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$HERE/../.." && pwd)
[ "$(id -un)" = user ] || { echo "journey.sh: runs on a Boat sandbox only (user $(id -un)); refusing" >&2; exit 3; }
IMAGE=${IMAGE:-ubuntu:24.04}; SOURCE=${SOURCE:-release}; TAG=${TAG:-latest}
mkdir -p "$OUT"/{shots,logs,home,standins/bin,projects}; : >"$OUT/timings.tsv"; : >"$OUT/findings.tsv"; : >"$OUT/checks.tsv"
C=bare-$$; DISP=:57; PH=/home/person; HH=$OUT/home; fails=0; T0=$(date +%s.%N); XP=; IP=
now() { python3 -c "import time;print(round(time.time()-$T0,1))"; }
tm() { printf '%s\t%s\n' "$1" "$2" >>"$OUT/timings.tsv"; echo "TIME $1 $2"; }
ok() { printf 'PASS\t%s\n' "$1" >>"$OUT/checks.tsv"; echo "PASS $1"; }
bad() { printf 'FAIL\t%s\n' "$1" >>"$OUT/checks.tsv"; echo "FAIL $1"; fails=$((fails+1)); }
cut_() { grep -qxF "$(printf "%s\t%s\t%s" "$1" "$2" "$3")" "$OUT/findings.tsv" 2>/dev/null && return 0; printf '%s\t%s\t%s\n' "$1" "$2" "$3" >>"$OUT/findings.tsv"; echo "CUT [$1] $2: $3"; }   # rank where what
in_box() { docker exec -i "$C" "$@"; }
as_person() { docker exec -i -u person -w $PH -e HOME=$PH "$C" "$@"; }
# every X call is bounded: a client that holds a grab (or a stuck server) cost a whole run once (46 min in rerun 3); a timeout is a FAIL
# that says which step and what the box's processes were doing (xstuck)
STEP=start; step() { STEP=$*; echo "STEP $(python3 -c "import time;print(round(time.time()-$T0))") $*"; }
# What it captures, for lane B's grab probe: does a fresh X client get an answer (no: a server grab, held by the one X client that is not
# waiting on its X socket); was the Backplane client still logging; each X client's state and stack from the host (the container gives
# no ptrace: host pids from docker top, /proc as root, eu-stack or gdb), Xvfb's too.
# xrun LIMIT CMD...: CMD with DISPLAY set, watched; past LIMIT seconds xstuck looks at it while it is still stuck, then it is killed (124)
xrun() { local lim=$1 p i; shift; DISPLAY=$DISP "$@" & p=$!
  for i in $(seq $((lim * 20))); do kill -0 $p 2>/dev/null || { wait $p; return $?; }; sleep 0.05; done
  xstuck "$*" $p; kill -9 $p 2>/dev/null; wait $p 2>/dev/null; return 124; }
xstuck() { [ -f "$OUT/logs/xstuck.txt" ] && return 0; local f=$OUT/logs/xstuck.txt pid sp=${2:-}
  { echo "step: $STEP ($1 timed out) at $(date -u +%T)"
    if [ -n "$sp" ]; then echo "--- the stuck call: pid $sp: $(tr '\0' ' ' </proc/$sp/cmdline 2>/dev/null)"
      grep -E '^State' /proc/$sp/status 2>/dev/null; echo "wchan $(cat /proc/$sp/wchan 2>/dev/null)  (read on the X socket: a grab; nanosleep/poll loop: a wait like --sync)"
      echo "syscall $(sudo -n cat /proc/$sp/syscall 2>/dev/null)"; ls -l /proc/$sp/fd 2>/dev/null | grep -o 'socket:.*' | head -3
      sudo -n cat /proc/$sp/stack 2>/dev/null | head -12; command -v eu-stack >/dev/null && sudo -n timeout 30 eu-stack -p $sp 2>&1 | head -40; fi
    if DISPLAY=$DISP timeout 5 xdpyinfo >/dev/null 2>&1; then echo "a fresh X client is answered: no server grab (the call itself was stuck)"
    else echo "a fresh X client is NOT answered in 5 s: the server is grabbed or stuck"; fi
    echo "client log: last line $(( $(date +%s) - $(stat -c %Y "$HH/win.log" 2>/dev/null || echo 0) )) s ago; last timed event: $(grep -o '"t_ms":[0-9.]*' "$HH/win.log" 2>/dev/null | tail -1)"
    echo "--- box processes"; in_box ps -eo pid,stat,wchan:24,etime,args 2>&1 | cut -c1-220
    echo "--- host side (docker top), per process: state, wchan, kernel stack, user stack"
    for pid in $(docker top "$C" -eo pid,args 2>/dev/null | awk 'NR>1 && /kicad|backplane-app|backplane-hub/ {print $1}') $XP; do
      echo "=== $pid $(tr '\0' ' ' </proc/$pid/cmdline 2>/dev/null | cut -c1-160)"
      grep -E '^(State|Threads)' /proc/$pid/status 2>/dev/null | tr '\n' ' '; echo "wchan $(cat /proc/$pid/wchan 2>/dev/null)"
      sudo -n cat /proc/$pid/stack 2>/dev/null | head -12
      if command -v eu-stack >/dev/null; then sudo -n timeout 30 eu-stack -p $pid 2>&1 | head -80
      elif command -v gdb >/dev/null; then sudo -n timeout 40 gdb -batch -p $pid -ex 'thread apply all bt 12' 2>&1 | grep -v '^\[' | head -80; fi
    done; } >"$f" 2>&1
  bad "an X call did not return during: $STEP ($1; logs/xstuck.txt: $(grep -m1 "fresh X client" "$f"))"; }
shot() { xrun 20 import -window root "$OUT/shots/$1.png" 2>/dev/null; return 0; }
cleanup() { docker rm -f "$C" >/dev/null 2>&1; [ -n "$IP" ] && kill "$IP" 2>/dev/null; [ -n "$XP" ] && kill "$XP" 2>/dev/null; }
trap cleanup EXIT

# --- harness on the host (never in the box) ---
need() { command -v "$1" >/dev/null 2>&1; }
need Xvfb && need i3 && need xdotool && need import && need xwininfo && need xdpyinfo && need eu-stack || sudo -n env DEBIAN_FRONTEND=noninteractive apt-get install -y -qq xvfb i3 xdotool imagemagick x11-utils elfutils >"$OUT/logs/harness-apt.log" 2>&1
need bun || { echo "no bun on the host for the stand-in agent" >&2; exit 3; }
Xvfb $DISP -screen 0 1600x1000x24 -nolisten tcp >"$OUT/logs/xvfb.log" 2>&1 & XP=$!; sleep 1.5
printf 'ipc-socket %s/i3.sock\nnew_window none\nfor_window [class=".*"] border none\ndefault_border none\n' "$OUT" >"$OUT/i3.conf"
DISPLAY=$DISP i3 -c "$OUT/i3.conf" >"$OUT/logs/i3.log" 2>&1 & IP=$!; sleep 1
X() { xrun 15 xdotool "$@"; }
pos() { python3 "$ROOT/test/onboarding/pos.py" "$HH/win.log" "$1" 2>/dev/null; }
click_id() { local p; for _ in $(seq ${2:-40}); do p=$(pos "$1") && { X mousemove $p click 1; return 0; }; sleep 0.5; done; return 1; }

# --- stand-in agents: bun and the stand-in in /opt/standins, the only agents this machine has ---
cp "$HERE/coedit_standin.py" "$OUT/standins/bin/claude"; printf '{"out":"%s","pcb":"%s"}\n' /home/person/coedit.json /home/person/Documents/pic_programmer/pic_programmer.kicad_pcb >"$OUT/standins/bin/coedit.conf"   # python3 is on the stock desktop (installed with the desktop runtime below)
for b in codex grok muse opencode agy pi omp; do printf '#!/bin/sh\nexit 1\n' >"$OUT/standins/bin/$b"; done
chmod +x "$OUT/standins/bin/"*

# --- the clean machine ---
docker pull -q "$IMAGE" >/dev/null || { bad "pull $IMAGE"; exit 1; }
docker run -d --name "$C" --network host --shm-size 2g -v /tmp/.X11-unix:/tmp/.X11-unix:ro -v "$HH:$PH" -v "$OUT/standins:/opt/standins:ro" \
  -v "$OUT/projects:/srv/projects" "$IMAGE" sleep infinity >/dev/null || { bad "start the box"; exit 1; }
in_box sh -c "getent passwd $(id -u) | cut -d: -f1 | xargs -r userdel 2>/dev/null; useradd -u $(id -u) -M -d $PH -s /bin/bash person" || { bad "make the person's user"; exit 1; }
chmod 755 "$HH"
case $IMAGE in ubuntu*|debian*) PM=apt;; fedora*) PM=dnf;; *) PM=apt;; esac
pkg_install() { if [ $PM = apt ]; then in_box sh -c "DEBIAN_FRONTEND=noninteractive apt-get install -y -qq $*" >>"$OUT/logs/box-pkg.log" 2>&1; else in_box dnf install -y -q "$@" >>"$OUT/logs/box-pkg.log" 2>&1; fi; }
[ $PM = apt ] && in_box apt-get update -qq >"$OUT/logs/box-pkg.log" 2>&1
in_box sh -c '(. /etc/os-release; echo "$PRETTY_NAME")' >"$OUT/os.txt"; echo "box: $(cat "$OUT/os.txt") from $IMAGE"

# The distro's desktop manifest (Ubuntu publishes one per release): a package in it is on every stock desktop of that release, so the box gets
# it as the desktop a person has; a library from a package NOT in it is a finding (ship it, or document it as a prerequisite).
MAN=$OUT/desktop.manifest; : >"$MAN"
if [ "${IMAGE%%:*}" = ubuntu ]; then
  v=${IMAGE#*:}; idx=$(curl -fsSL "https://releases.ubuntu.com/$v/" 2>/dev/null); m=$(printf '%s' "$idx" | grep -o "ubuntu-$v[.0-9]*-desktop-amd64.manifest" | sort -uV | tail -1)
  [ -n "$m" ] && curl -fsSL "https://releases.ubuntu.com/$v/$m" -o "$MAN" && echo "desktop manifest: $m ($(wc -l <"$MAN") packages)"
fi
on_desktop() { grep -qE "^$1(:amd64)?[[:space:]]" "$MAN"; }
# a helper box from the same image answers which package ships a library (apt-file), so the clean box gets nothing for the lookup
HELPER=bare-helper-$$
lookup_pkg() { # soname -> package
  if ! docker ps -q -f name=$HELPER | grep -q .; then
    docker run -d --name $HELPER "$IMAGE" sleep infinity >/dev/null
    docker exec $HELPER sh -c 'apt-get update -qq && DEBIAN_FRONTEND=noninteractive apt-get install -y -qq apt-file && apt-file update' >"$OUT/logs/helper.log" 2>&1
  fi
  local c; c=$(docker exec $HELPER apt-file search -l -x "/$1\$" 2>/dev/null | grep -v -- '-dbg$\|-dev$\|-tests$\|^lib32\|^libx32')
  # the package a desktop has, else the library package, else any
  for p in $c; do on_desktop "$p" && { echo "$p"; return; }; done
  printf '%s\n' $c | grep '^lib' | head -1 | grep . || printf '%s\n' $c | head -1
}
trap 'cleanup; docker rm -f $HELPER >/dev/null 2>&1' EXIT
# libs: every ELF under DIR whose libraries the box lacks -> desktop packages installed, others reported. ROUND names the step.
fill_libs() { # dir round
  local dir=$1 round=$2 miss pk added=
  for pass in 1 2 3 4; do
    # a runtime that carries its own loader (KiCad's: lib/ld-linux-x86-64.so.2, its launcher runs through it) is resolved the same way
    local ldd=ldd; in_box test -x "$dir/lib/ld-linux-x86-64.so.2" && ldd="$dir/lib/ld-linux-x86-64.so.2 --list --library-path $dir/lib"
    in_box sh -c "find '$dir' -maxdepth 3 -type f \\( -perm -u+x -o -name '*.so*' \\) 2>/dev/null | while read f; do head -c4 \"\$f\" | grep -q ELF && $ldd \"\$f\" 2>&1 | sed \"s|^|\$f: |\"; done" >"$OUT/logs/ldd-$round-$pass.txt"
    miss=$(awk '/=> not found/{print $2}' "$OUT/logs/ldd-$round-$pass.txt" | sort -u)
    # "version X not found": a library the build carries is older than what a system library linked with it needs
    grep -o "[^ ]*: version \`[^']*' not found (required by [^)]*)" "$OUT/logs/ldd-$round-$pass.txt" | sed 's|^[^:]*/||' | sort -u | head -5 | while read -r l; do
      [ "$pass" = 1 ] && cut_ 2 "$round" "a library it bundles is older than a system one needs: $l"; done
    [ -z "$miss" ] && break
    for so in $miss; do
      pk=$(lookup_pkg "$so")
      if [ -z "$pk" ]; then cut_ 2 "$round" "needs $so, which no $IMAGE package ships"; continue; fi
      # only what a stock desktop has goes in: anything else stays missing, so the run shows what a person would see
      if [ -s "$MAN" ] && ! on_desktop "$pk"; then cut_ 2 "$round" "needs $so (package $pk), which a stock $(cat "$OUT/os.txt") desktop does not have: ship it or document it"; continue; fi
      added="$added $pk"
    done
    [ -n "$added" ] && pkg_install $(printf '%s\n' $added | sort -u) || true
    [ -n "$added" ] || break
  done
  printf '%s\n' $added | sort -u | grep . >"$OUT/logs/baseline-$round.txt"; echo "desktop baseline for $round: $(wc -l <"$OUT/logs/baseline-$round.txt") packages"
}

# --- 1. install as the person: the documented one-liner ---
t=$(date +%s.%N)
# what the stock desktop has of the downloaders goes in (a container image has neither); what it lacks stays out
for p in curl wget ca-certificates; do on_desktop $p && pkg_install $p; done
if [ "$SOURCE" = release ]; then
  URL=https://github.com/BackplaneWorks/Backplane/releases/${TAG/#latest/latest/download}; [ "$TAG" = latest ] || URL=https://github.com/BackplaneWorks/Backplane/releases/download/$TAG
  URL=$URL/install.sh
  if ! in_box sh -c 'command -v curl' >/dev/null; then
    cut_ 1 install "curl is not on a stock $(cat "$OUT/os.txt") desktop (wget is): the documented 'curl -fsSL … | sh' says 'curl: command not found'"
    as_person sh -c "wget -qO- '$URL' | sh -s -- --quiet" >"$OUT/logs/install-wget.log" 2>&1
    if grep -q "is installed" "$OUT/logs/install-wget.log"; then ok "wget -qO- install.sh | sh installs with no curl"
    else bad "wget -qO- install.sh | sh installs with no curl: $(tail -1 "$OUT/logs/install-wget.log")"; pkg_install curl; fi
  fi
  [ -x "$HH/.local/bin/backplane-app" ] || as_person sh -c "curl -fsSL '$URL' | sh -s -- --quiet" >"$OUT/logs/install.log" 2>&1
else
  docker cp "$ARCHIVE" "$C:/tmp/bp.tar.gz"; docker cp "$INSTALLER" "$C:/tmp/install.sh"; in_box chmod 644 /tmp/bp.tar.gz /tmp/install.sh
  in_box sh -c 'command -v curl' >/dev/null || cut_ 1 install "curl is not on a stock $(cat "$OUT/os.txt") desktop (wget is): the documented 'curl -fsSL … | sh' says 'curl: command not found'"
  as_person env BACKPLANE_ARCHIVE=/tmp/bp.tar.gz sh /tmp/install.sh --quiet >"$OUT/logs/install.log" 2>&1
fi
[ -x "$HH/.local/bin/backplane-app" ] && ok "install.sh installed backplane-app" || { bad "install.sh failed: $(tail -3 "$OUT/logs/install.log" | tr '\n' ' ')"; exit 1; }
tm install "$(python3 -c "import time;print(round(time.time()-$t,1))")"
if [ "$SOURCE" = release ]; then
  SZ=$(curl -sfIL "$(grep -ho 'https://[^ ]*-native.tar.gz' "$OUT/logs/install"*.log | head -1)" | awk 'tolower($1)=="content-length:"{n=$2} END{print n+0}')
else SZ=$(stat -c %s "$ARCHIVE"); fi
echo "backplane archive: $((SZ/1048576)) MB" | tee "$OUT/backplane-size.txt"
if grep -q "Run: backplane-app" "$OUT/logs/install.log" && ! as_person bash -lc 'command -v backplane-app' >/dev/null 2>&1; then
  cut_ 4 install "~/.local/bin is not on PATH in a fresh shell, so 'backplane-app' as printed is 'command not found' until the next login"
elif grep -q "Run: $PH/.local/bin/backplane-app" "$OUT/logs/install.log"; then ok "the installer gives the full path to run while ~/.local/bin is not on PATH"; fi
BUILD=$HH/.local/share/backplane/builds/$(cat "$HH/.local/share/backplane/CURRENT")
echo "build: $(basename "$BUILD") commit $(cat "$BUILD/COMMIT" 2>/dev/null)"
t=$(date +%s.%N); fill_libs "$PH/.local/share/backplane/builds" backplane; tm desktop-libs-backplane "$(python3 -c "import time;print(round(time.time()-$t,1))")"
# what a desktop has without a library linking it: a Vulkan driver, GL, fonts, a session bus, xdg-open
RT="python3 libvulkan1 mesa-vulkan-drivers libgl1-mesa-dri libegl1 libglx-mesa0 fontconfig fonts-dejavu-core xdg-utils dbus-user-session"
for p in $RT; do [ ! -s "$MAN" ] || on_desktop $p || cut_ 3 runtime "$p is not on the stock desktop"; done; pkg_install $RT

# --- 2. first start, as from the applications menu ---
ENVS=(-e DISPLAY=$DISP -e GDK_BACKEND=x11 -e XDG_RUNTIME_DIR=$PH/.run -e BACKPLANE_AGENTS=/opt/standins/bin -e BACKPLANE_NO_TAILSCALE=1 -e BACKPLANE_NO_UPDATE=1
      -e BACKPLANE_ORDER_SERVER=http://127.0.0.1:9 -e BACKPLANE_GPUI_RECTS=1 -e LP_NUM_THREADS=2 -e LIBGL_ALWAYS_SOFTWARE=1 -e BARE_COEDIT_OUT=$PH/coedit.json -e BARE_COEDIT_PCB=$PH/Documents/pic_programmer/pic_programmer.kicad_pcb)
mkdir -p "$HH/.run"; chmod 700 "$HH/.run"
launch() { docker exec -d -u person -w $PH -e HOME=$PH "${ENVS[@]}" "$C" sh -c "echo \$\$ > $PH/.client.pid; exec $PH/.local/bin/backplane-app $* >>$PH/win.log 2>&1"; }
HUBH=$HH/.backplane/hub
hub_port() { cut -d' ' -f2 "$HUBH/hub.lock" 2>/dev/null; }
hub_up() { local p; p=$(hub_port); [ -n "$p" ] && curl --noproxy '*' -sf -m 2 "http://127.0.0.1:$p/hello" >/dev/null; }
hub() { timeout 90 bun "$HERE/hub.mjs" "$(hub_port)" "$HUBH/token" "$@"; }
info() { hub info "$1" | python3 -c "import json,sys; print(json.dumps(json.load(sys.stdin).get('$1')))"; }
setting() { python3 - "$HUBH/events.jsonl" "$1" <<'EOF'
import json,sys
v=''
for l in open(sys.argv[1], errors='replace'):
    try: e=json.loads(l)
    except Exception: continue
    if e.get('$')=='SettingSet' and e.get('key')==sys.argv[2]: v=e.get('value','')
print(v)
EOF
}
T0=$(date +%s.%N)
launch
for _ in $(seq 120); do pos first-use >/dev/null && break; sleep 0.5; done
pos first-use >/dev/null && { tm first-window "$(now)"; ok "the first window asks for the data folder"; } || { bad "no first window in 60 s: $(tail -5 "$HH/win.log" | tr '\n' ' ')"; shot 00-nothing; exit 1; }
shot 00-first; click_id first-use
for _ in $(seq 240); do hub_up && break; sleep 0.5; done
hub_up && { tm hub-up "$(now)"; ok "Use this folder started the hub (port $(hub_port))"; } || { bad "no hub in 120 s"; tail -20 "$HUBH/hub.log" 2>/dev/null; exit 1; }
sleep 4; shot 01-welcome

# --- 3. setup: Welcome -> Agents -> Design tools (KiCad installs by itself there) ---
step() { setting onboard.step; }
# Next (Enter) until the step with KiCad: `tools` in the five-step setup, `kicad` in 0.15's longer one
for _ in $(seq 30); do case $(step) in tools|kicad) break;; esac; s0=$(step); X key Return; for _ in $(seq 20); do [ "$(step)" != "$s0" ] && break; sleep 0.25; done; done
case $(step) in tools|kicad) ok "setup reached its KiCad step ($(step)) by Next alone";; *) bad "setup is at '$(step)', no KiCad step reached";; esac
tm kicad-step "$(now)"; sleep 2; shot 02-tools
# the install's state from what its script writes (any build): started = <home>/kicad/.install.log; done = a runtime with bin/kicad-cli and no
# lock left; failed = no lock and no runtime. Info kicad.install says the same where the build publishes it to a view socket.
KH=$HUBH/kicad
kst() { local d; d=$(ls -d "$KH"/10.* 2>/dev/null | sort -V | tail -1)
  if ls -d "$KH"/.install-*.lock >/dev/null 2>&1; then echo running; elif [ -n "$d" ] && [ -x "$d/bin/kicad-cli" ]; then echo done
  elif [ -f "$KH/.install.log" ]; then echo failed; else echo none; fi; }
for _ in $(seq 40); do [ "$(kst)" != none ] && break; sleep 0.5; done
if [ "$(kst)" = none ]; then
  cut_ 1 setup "setup's KiCad step does not install KiCad by itself: a person must find and press Install (this build: $(basename "$BUILD"))"
  hub rpc kicad.install '{"consent":true}' >"$OUT/logs/kicad-install-press.json"; echo "pressed Install: $(cat "$OUT/logs/kicad-install-press.json")"
else tm kicad-install-started "$(now)"; fi
for _ in $(seq 1800); do case $(kst) in done|failed) break;; esac; sleep 0.5; done
st=$(kst); echo "kicad install: $st; $(tail -1 "$KH/.install.log" 2>/dev/null)"; cp "$KH/.install.log" "$OUT/logs/kicad-install.log" 2>/dev/null
case $st in done) tm kicad-installed "$(now)"; ok "KiCad installed ($(tail -1 "$KH/.install.log"))";;
  failed) bad "KiCad install failed: $(tail -1 "$KH/.install.log")"; cut_ 1 kicad-fetch "the KiCad install failed: $(tail -1 "$KH/.install.log")";;
  *) bad "KiCad install never ended ($st)"; cut_ 1 kicad-fetch "the KiCad install never started or ended in 15 min ($st)";; esac
shot 03-kicad
KDIR=$(ls -d "$HUBH"/kicad/10.* 2>/dev/null | sort -V | tail -1)
if [ -n "$KDIR" ]; then
  KV=$(basename "$KDIR"); echo "$KV" >"$OUT/kicad-version.txt"
  # what a new person receives must be the newest Backplane KiCad release (GitHub's "latest"), or we never notice it falling behind
  # the releases setup downloads from (install.rs FROM), as the product does
  REPO=$(grep -o 'pub const FROM: &str = "github.com/[^"]*"' "$ROOT/native/host/src/kicad/install.rs" | sed 's|.*github.com/||; s|"$||')
  LATEST=$(curl -fsSL "https://api.github.com/repos/${REPO:-BackplaneWorks/Backplane_KiCad}/releases/latest" 2>/dev/null | python3 -c 'import json,sys; print(json.load(sys.stdin).get("tag_name","").replace("backplane-kicad-v",""))' 2>/dev/null)
  # a deliberate hold (test/bare/kicad-hold: "<version> <why>") turns "not the latest" into HOLD and a paper cut while the install is
  # exactly the held version; anything else still fails. The pin's bump deletes the file.
  HOLD=$(sed -n 1p "$HERE/kicad-hold" 2>/dev/null); HOLDV=${HOLD%% *}
  echo "fresh install got Backplane KiCad $KV; latest release $LATEST (${REPO:-?})${HOLD:+; hold: $HOLD}" | tee "$OUT/kicad-runtime.txt"
  if [ -z "$LATEST" ]; then bad "could not read the latest Backplane KiCad release from GitHub (${REPO:-?})"
  elif [ "$KV" = "$LATEST" ]; then ok "a fresh install gets the latest Backplane KiCad release ($KV)"
    [ -n "$HOLD" ] && bad "test/bare/kicad-hold ($HOLD) is still there while the install is the latest: delete it"
  elif [ -n "$HOLD" ] && [ "$KV" = "$HOLDV" ]; then printf 'HOLD\t%s\n' "a fresh install gets $KV, held back from the latest $LATEST: ${HOLD#* }" >>"$OUT/checks.tsv"
    echo "HOLD a fresh install gets $KV, held back from the latest $LATEST: ${HOLD#* }"; cut_ 2 runtime "a fresh install gets Backplane KiCad $KV while the latest release is $LATEST (held: ${HOLD#* })"
  else bad "a fresh install gets Backplane KiCad $KV, not the latest release $LATEST${HOLD:+ (nor the held $HOLDV)}"; cut_ 2 runtime "a fresh install gets Backplane KiCad $KV while the latest release is $LATEST (install.rs pins it)"; fi
  A=$(python3 -c "import json;print(json.load(open('$KDIR/manifest.json')).get('platform',''))" 2>/dev/null)
  SZ=$(curl -sfIL "https://github.com/${REPO:-BackplaneWorks/Backplane_KiCad}/releases/download/backplane-kicad-v$KV/backplane-kicad-linux-x86_64-$KV.tar.gz" | awk 'tolower($1)=="content-length:"{n=$2} END{print n+0}')
  echo "kicad $KV ($A): download $((SZ/1048576)) MB, installed $(du -sm "$KDIR" | cut -f1) MB" | tee "$OUT/kicad-size.txt"
  t=$(date +%s.%N); fill_libs "${KDIR/#$HH/$PH}" kicad; tm desktop-libs-kicad "$(python3 -c "import time;print(round(time.time()-$t,1))")"
  in_box sh -c "${KDIR/#$HH/$PH}/bin/kicad-cli --version" >"$OUT/logs/kicad-cli-version.txt" 2>&1 && ok "the installed kicad-cli runs ($(cat "$OUT/logs/kicad-cli-version.txt"))" || bad "the installed kicad-cli does not run: $(cat "$OUT/logs/kicad-cli-version.txt")"
fi
for _ in $(seq 30); do [ "$(step)" = project ] && break; s0=$(step); X key Return; for _ in $(seq 20); do [ "$(step)" != "$s0" ] && break; sleep 0.25; done; done
[ "$(step)" = project ] && ok "setup reached First project" || bad "setup is at '$(step)', not project"
shot 04-project
if [ -n "${KDIR:-}" ]; then . "$HERE/journey_project.sh"; else bad "no KiCad: the journey stops before the project"; fi

# the stock checks' own reports (not the project copies)
for d in "$HH"/compat-saved/* "$HH"/compat-demos/*; do [ -d "$d" ] || continue; n=$OUT/logs/compat/$(basename "$(dirname "$d")")-$(basename "$d"); mkdir -p "$n"
  for x in fork stock fork-on-export result.json downgrade-report.json fork.log stock.log; do [ -e "$d/$x" ] && cp -r "$d/$x" "$n/"; done; done
for f in "$HUBH/hub.log" "$HH/win.log" "$HH/standin.log" "$HH/coedit.json.argv" "$HUBH/kicad/.install.log"; do [ -f "$f" ] && cp "$f" "$OUT/logs/" 2>/dev/null; done
echo "fails=$fails"; [ "$fails" = 0 ]
