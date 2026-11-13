#!/bin/bash
# A private X display drawn by the machine's GPU, for real-hardware runs (REAL_HARDWARE=1): Xvfb has no GPU, so GPUI and KiCad's GAL
# would both draw with llvmpipe there and every frame time would be the CPU's.
#   a headless GNOME Shell (Wayland only, a virtual monitor, its own session bus and an empty private "system" bus so it reaches no
#   GDM, logind or upower of the person's) -> a rootful Xwayland on it (glamor: the GPU) -> i3 inside it, exactly as on Xvfb.
# Nothing shows on the person's screen and no input reaches their seat: XTest goes to this Xwayland only.
# Sourced by env_xvfb.sh; start_gpu_display sets DISP/DISPLAY like start_xvfb_and_wm. Every process is started with own (process groups).

gpu_display_available() { command -v gnome-shell >/dev/null && command -v Xwayland >/dev/null && command -v dbus-daemon >/dev/null && command -v dbus-run-session >/dev/null; }

start_gpu_display() {
  local d gs="$OUT/gpu" geo=${KEMBED_GEOMETRY:-1800x1050}
  mkdir -p "$gs/home" && mkdir -p -m 700 "$gs/run"
  own dbus-daemon --session --nofork --address="unix:path=$gs/sysbus" >"$gs/sysbus.log" 2>&1
  for _ in $(seq 30); do [ -S "$gs/sysbus" ] && break; sleep 0.1; done
  own env -i PATH=/usr/bin:/bin HOME="$gs/home" XDG_RUNTIME_DIR="$gs/run" XDG_CONFIG_HOME="$gs/home/.config" \
    DBUS_SYSTEM_BUS_ADDRESS="unix:path=$gs/sysbus" GVFS_DISABLE_FUSE=1 GIO_USE_VFS=local \
    dbus-run-session -- gnome-shell --headless --wayland --no-x11 --virtual-monitor "$geo" --wayland-display bpwl >"$gs/shell.log" 2>&1
  for _ in $(seq 150); do [ -S "$gs/run/bpwl" ] && grep -q "GNOME Shell started" "$gs/shell.log" 2>/dev/null && break; sleep 0.1; done
  [ -S "$gs/run/bpwl" ] || { echo "FAIL gpu display: headless shell did not start ($gs/shell.log)"; return 1; }
  d=$((720 + $$ % 200))
  while [ -e "/tmp/.X11-unix/X$d" ] || [ -e "/tmp/.X$d-lock" ]; do d=$((d + 1)); done
  DISP=":$d"
  own env -i PATH=/usr/bin:/bin HOME="$gs/home" XDG_RUNTIME_DIR="$gs/run" WAYLAND_DISPLAY=bpwl \
    Xwayland "$DISP" -geometry "${geo}" -nolisten tcp -noreset -verbose 3 >"$gs/xwayland.log" 2>&1
  for _ in $(seq 50); do [ -e "/tmp/.X11-unix/X$d" ] && break; sleep 0.1; done
  [ -e "/tmp/.X11-unix/X$d" ] || { echo "FAIL gpu display: Xwayland did not start ($gs/xwayland.log)"; return 1; }
  for _ in $(seq 30); do grep -q "glamor" "$gs/xwayland.log" && break; sleep 0.1; done
  grep -q "glamor" "$gs/xwayland.log" || echo "WARN gpu display: Xwayland has no glamor (software)"
  export DISPLAY="$DISP"
}
