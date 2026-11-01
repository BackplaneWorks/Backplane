#!/bin/bash
set -eu
R=<capture-root>
D=$R/run/A-capture
case ${1:-} in
start)
 if test -f "$D/hub.pid"; then
  oldpid=$(cat "$D/hub.pid")
  if test -r "/proc/$oldpid/cmdline" && tr '\0' ' ' < "/proc/$oldpid/cmdline" | grep -q -- "$D/home"; then echo 'Own hub already running' >&2; exit 1; fi
 fi
 rm -f "$D/ns.pid"
 ln -sfn <kicad-copy> "$D/home/kicad"
 setsid -f unshare -rn sh -c "ip link set lo up; echo \$\$ > '$D/ns.pid'; exec sleep infinity"
 for i in {1..50}; do test -s "$D/ns.pid" && break; sleep .1; done
 setsid -f nsenter -t "$(cat "$D/ns.pid")" -U -n --preserve-credentials sh -c "echo \$\$ > '$D/hub.pid'; cd '$D'; exec env -u DISPLAY -u WAYLAND_DISPLAY HOME='$D/fakehome' XDG_CACHE_HOME='$D/fakehome/.cache' XDG_CONFIG_HOME='$D/fakehome/.config' XDG_DATA_HOME='$D/fakehome/.local/share' BACKPLANE_AGENTS='$D/agents' BACKPLANE_ORDER_SERVER=http://127.0.0.1:9 BACKPLANE_NO_UPDATE=1 BACKPLANE_KISTACK_SYNC=0 BACKPLANE_PEERS= BACKPLANE_VAULT_KEYRING=file '$R/app/backplane-serve' --headless --foreground --home '$D/home' --port 47810 --no-tailscale --web '$R/app/web' > '$D/hub.log' 2>&1"
 for i in {1..100}; do
  if nsenter -t "$(cat "$D/ns.pid")" -U -n --preserve-credentials curl -fs -o /dev/null -m 1 http://127.0.0.1:47810/hello; then exit 0; fi
  sleep .2
 done
 echo 'Own hub did not become ready; inspect A-capture/hub.log' >&2; exit 1
 ;;
desk)
 LD_LIBRARY_PATH=<xvfb>/usr/lib setsid -f sh -c "echo \$\$ > '$D/xvfb.pid'; exec <xvfb>/usr/bin/Xvfb :741 -screen 0 1920x1080x24 -nolisten tcp > '$D/xvfb.log' 2>&1"
 sleep 1
 setsid -f nsenter -t "$(cat "$D/ns.pid")" -U -n --preserve-credentials sh -c "echo \$\$ > '$D/desk.pid'; exec env -u ELECTRON_RUN_AS_NODE -u WAYLAND_DISPLAY -u ELECTRON_OZONE_PLATFORM_HINT DISPLAY=:741 DBUS_SESSION_BUS_ADDRESS=unix:path=/nonexistent HOME='$D/fakehome' BACKPLANE_AGENTS='$D/agents' BACKPLANE_ORDER_SERVER=http://127.0.0.1:9 BACKPLANE_NO_UPDATE=1 BACKPLANE_KISTACK_SYNC=0 BACKPLANE_PEERS= BACKPLANE_VAULT_KEYRING=file BACKPLANE_HOME='$D/home' BACKPLANE_SERVICE_VERSION=0.13.2-dev.20261101.1606 '$R/app/desktop/runtime/backplane-desktop' --no-sandbox --ozone-platform=x11 --force-device-scale-factor=2 --remote-debugging-port=47811 > '$D/desk.log' 2>&1"
 for i in {1..100}; do
  if nsenter -t "$(cat "$D/ns.pid")" -U -n --preserve-credentials curl -fs -o /dev/null -m 1 http://127.0.0.1:47811/json/version; then exit 0; fi
  sleep .2
 done
 echo 'Own desktop did not become ready; inspect A-capture/desk.log' >&2; exit 1
 ;;
node) shift; nsenter -t "$(cat "$D/ns.pid")" -U -n --preserve-credentials env -u DISPLAY -u WAYLAND_DISPLAY HOME="$D/fakehome" node "$@" ;;
sh) shift; nsenter -t "$(cat "$D/ns.pid")" -U -n --preserve-credentials "$@" ;;
stop)
 for pair in 'desk remote-debugging-port=47811' 'hub A-capture/home' 'xvfb Xvfb' 'ns sleep'; do
  read -r name marker <<< "$pair"
  if test -f "$D/$name.pid"; then p=$(cat "$D/$name.pid"); if test -r "/proc/$p/cmdline" && tr '\0' ' ' < "/proc/$p/cmdline" | grep -q -- "$marker"; then kill "$p"; fi; fi
 done ;;
esac
