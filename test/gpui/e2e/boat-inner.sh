#!/bin/sh
# Runs inside the Boat sandbox (repo root). The sandbox is the hard memory cap.
set -u; mkdir -p out/e2e; export NO_SCOPE=1 PERF_REPO=$PWD BOAT_SANDBOX=1
if command -v bun >/dev/null 2>&1; then RUN=bun; else curl -fsSL https://bun.sh/install | bash >/dev/null 2>&1; export PATH=$HOME/.bun/bin:$PATH; RUN=bun; fi
echo "runner: bun $($RUN --version); $(nproc) cpus; $(free -m | sed -n 2p)" | tee out/e2e/env.txt
provision() { # the input driver's tools, installed the same way for every mode (a fresh sandbox has none of them)
  sudo -n env DEBIAN_FRONTEND=noninteractive apt-get install -y -qq xdotool xclip python3-xlib x11-utils ibus ibus-hangul imagemagick dbus dbus-user-session python3-pip fcitx5 fcitx5-hangul >/dev/null 2>&1
  python3 -c 'import dbus_next' 2>/dev/null || python3 -m pip install --user -q dbus-next >/dev/null 2>&1 || python3 -m pip install --user -q --break-system-packages dbus-next >/dev/null 2>&1
  echo "provisioned: xdotool=$(command -v xdotool >/dev/null && echo y || echo n) xclip=$(command -v xclip >/dev/null && echo y || echo n) xlib=$(python3 -c 'import Xlib' 2>/dev/null && echo y || echo n) ibus-hangul=$(ls /usr/share/ibus/component/hangul.xml >/dev/null 2>&1 && echo y || echo n) dbus_next=$(python3 -c 'import dbus_next' 2>/dev/null && echo y || echo n)" | tee -a out/e2e/env.txt
}
case "$1" in
 selftest) $RUN test/gpui/e2e/lib/frames.test.mjs 2>&1 | tee out/e2e/frames-test.txt
           $RUN test/gpui/e2e/adapter.test.mjs 2>&1 | tee out/e2e/adapter-test.txt
           $RUN -e "import('./test/gpui/e2e/lib/capture.mjs').then(m=>console.log(JSON.stringify(m.tools())))" | tee out/e2e/tools.txt ;;
 input-selftest) provision; export IME_BACKEND=${IME_BACKEND:-fcitx5}; export BOAT_SANDBOX=1; $RUN test/gpui/e2e/input/selftest.mjs 2>&1 | tee out/e2e/input-selftest.txt ;;
 input-run) export IME_BACKEND=${IME_BACKEND:-fcitx5}; export BACKPLANE_GPUI_ASSETS=$PWD/test/gpui/e2e/bin/assets; chmod +x test/gpui/e2e/bin/*; provision; dbus-run-session -- $RUN test/gpui/e2e/input-run.mjs --hub-bin test/gpui/e2e/bin/hub --gpui-bin test/gpui/e2e/bin/gpui --out out/e2e/input > out/e2e/input-run.log 2>&1; rc=$?; grep -v '^dbus-daemon\|^fusermount\|^SpiRegistry' out/e2e/input-run.log | tail -60; echo "input-run exit code: $rc"; exit $rc ;;
 run) export BACKPLANE_GPUI_ASSETS=$PWD/test/gpui/e2e/bin/assets; export E2E_SESSION_MIN=${E2E_SESSION_MIN:-3}; $RUN test/gpui/e2e/lib/frames.test.mjs 2>&1 | tee out/e2e/frames-test.txt | tail -12; chmod +x test/gpui/e2e/bin/*; $RUN test/gpui/e2e/run.mjs --client gpui --hub-bin test/gpui/e2e/bin/hub --gpui-bin test/gpui/e2e/bin/gpui --journey "${2:-all}" --record --out out/e2e 2>&1 | tail -40 ;;
esac
