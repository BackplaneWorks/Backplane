#!/bin/sh
# Runs inside the Boat sandbox (repo root). The sandbox is the hard memory cap.
set -u; mkdir -p out/e2e; export NO_SCOPE=1 PERF_REPO=$PWD BOAT_SANDBOX=1
if command -v bun >/dev/null 2>&1; then RUN=bun; else curl -fsSL https://bun.sh/install | bash >/dev/null 2>&1; export PATH=$HOME/.bun/bin:$PATH; RUN=bun; fi
# Dev builds are linked against the desktop's glibc (2.43), newer than a Boat box's: a mode that runs the shipped client runs itself again inside
# test/e2e/desktop.Dockerfile (Ubuntu 26.04) when the client cannot load here (E2E_IN_CONTAINER=1 inside). --init so a timeout's TERM ends it;
# the container is removed however this ends.
case "${1:-}" in input-run|input-deadkey|run)
  if [ -z "${E2E_IN_CONTAINER:-}" ] && [ -f test/e2e/bin/gpui ] && ldd test/e2e/bin/gpui 2>&1 | grep -q 'GLIBC.*not found'; then
    echo "runner: the client needs a newer glibc than this box has; running '$1' in test/e2e/desktop.Dockerfile"
    sudo -n docker build -q --build-arg UID="$(id -u)" -t bp-e2e-desktop - < test/e2e/desktop.Dockerfile > out/e2e/docker-build.log 2>&1 || { tail -20 out/e2e/docker-build.log; echo "runner: the container did not build"; exit 3; }
    name=bp-e2e-$$; trap 'sudo -n docker rm -f "$name" >/dev/null 2>&1' EXIT; trap 'sudo -n docker kill "$name" >/dev/null 2>&1; exit 143' TERM HUP INT
    sudo -n docker run --rm --init --name "$name" --shm-size 1g -u u -e E2E_IN_CONTAINER=1 -v "$PWD:/ws" -v "$(command -v bun):/usr/local/bin/bun:ro" -w /ws bp-e2e-desktop sh test/e2e/boat-inner.sh "$@" & c=$!
    wait $c; exit $?
  fi ;;
esac
echo "runner: bun $($RUN --version); $(nproc) cpus; $(free -m | sed -n 2p)" | tee out/e2e/env.txt
provision() { # the input driver's tools, installed the same way for every mode (a fresh sandbox has none of them)
  sudo -n env DEBIAN_FRONTEND=noninteractive apt-get install -y -qq xdotool xclip python3-xlib x11-utils ibus ibus-hangul imagemagick dbus dbus-user-session python3-pip fcitx5 fcitx5-hangul >/dev/null 2>&1
  python3 -c 'import dbus_next' 2>/dev/null || python3 -m pip install --user -q dbus-next >/dev/null 2>&1 || python3 -m pip install --user -q --break-system-packages dbus-next >/dev/null 2>&1
  echo "provisioned: xdotool=$(command -v xdotool >/dev/null && echo y || echo n) xclip=$(command -v xclip >/dev/null && echo y || echo n) xlib=$(python3 -c 'import Xlib' 2>/dev/null && echo y || echo n) ibus-hangul=$(ls /usr/share/ibus/component/hangul.xml >/dev/null 2>&1 && echo y || echo n) dbus_next=$(python3 -c 'import dbus_next' 2>/dev/null && echo y || echo n)" | tee -a out/e2e/env.txt
}
case "$1" in
 selftest) $RUN test/e2e/lib/frames.test.mjs 2>&1 | tee out/e2e/frames-test.txt
           $RUN test/e2e/adapter.test.mjs 2>&1 | tee out/e2e/adapter-test.txt
           $RUN -e "import('./test/e2e/lib/capture.mjs').then(m=>console.log(JSON.stringify(m.tools())))" | tee out/e2e/tools.txt ;;
 input-selftest) provision; export IME_BACKEND=${IME_BACKEND:-fcitx5}; export BOAT_SANDBOX=1; $RUN test/e2e/input/selftest.mjs 2>&1 | tee out/e2e/input-selftest.txt ;;
 input-run) export IME_BACKEND=${IME_BACKEND:-fcitx5}; export BACKPLANE_GPUI_ASSETS=$PWD/test/e2e/bin/assets; chmod +x test/e2e/bin/*; provision; dbus-run-session -- $RUN test/e2e/input-run.mjs --hub-bin test/e2e/bin/hub --gpui-bin test/e2e/bin/gpui --out out/e2e/input > out/e2e/input-run.log 2>&1; rc=$?; grep -v '^dbus-daemon\|^fusermount\|^SpiRegistry' out/e2e/input-run.log | tail -60; echo "input-run exit code: $rc"; exit $rc ;;
 input-deadkey) export IME_BACKEND=ibus; export BACKPLANE_GPUI_ASSETS=$PWD/test/e2e/bin/assets; chmod +x test/e2e/bin/*; provision; sudo -n env DEBIAN_FRONTEND=noninteractive apt-get install -y -qq x11-xkb-utils >/dev/null 2>&1
      dbus-run-session -- $RUN test/e2e/input/deadkey-run.mjs --hub-bin test/e2e/bin/hub --gpui-bin test/e2e/bin/gpui --out out/e2e/deadkey > out/e2e/deadkey-run.log 2>&1; rc=$?; grep -v '^dbus-daemon\|^fusermount\|^SpiRegistry\|^Not connected' out/e2e/deadkey-run.log | tail -20; echo "input-deadkey exit code: $rc"; exit $rc ;;
 run) export BACKPLANE_GPUI_ASSETS=$PWD/test/e2e/bin/assets; export E2E_SESSION_MIN=${E2E_SESSION_MIN:-3}; $RUN test/e2e/lib/frames.test.mjs 2>&1 | tee out/e2e/frames-test.txt | tail -12; chmod +x test/e2e/bin/*; $RUN test/e2e/run.mjs --client gpui --hub-bin test/e2e/bin/hub --gpui-bin test/e2e/bin/gpui --journey "${2:-all}" --record --out out/e2e 2>&1 | tail -40 ;;
esac
