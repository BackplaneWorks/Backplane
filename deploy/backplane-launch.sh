#!/bin/sh
# `backplane`, installed beside backplane-serve. There
# is no window binary any more: this opens the desktop app when the install
# has one and a display is there, and otherwise runs the hub, as before:
#   backplane [--home DIR] ...            the desktop app, else the hub (with
#                                         a display, its page in the browser)
#   backplane --headless [--foreground]   the hub, in the background unless
#                                         --foreground (log: HOME/headless.log)
#   backplane --render MODEL...           not in this build: says so
# BACKPLANE_DESKTOP names the desktop app's launcher; otherwise it is
# desktop/backplane-open (Linux) or desktop/Backplane.app (macOS) beside this.
set -eu
self=$0
while [ -L "$self" ]; do
  link=$(readlink "$self")
  case $link in /*) self=$link ;; *) self=$(dirname "$self")/$link ;; esac
done
here=$(CDPATH= cd -- "$(dirname -- "$self")" && pwd)
case ${1:-} in
  --render) echo "backplane: part renders are not available in this build" >&2; exit 2 ;;
  --help|-h|--version) exec "$here/backplane-serve" "$@" ;;
esac
headless=0 fg=0 home= prev=
for a in "$@"; do
  case $a in --headless) headless=1 ;; --foreground) fg=1 ;; esac
  [ "$prev" != --home ] || home=$a
  prev=$a
done
desktop=
for d in "${BACKPLANE_DESKTOP:-}" "$here/desktop/backplane-open" "$here/desktop/Backplane.app/Contents/MacOS/Backplane"; do
  if [ -n "$d" ] && [ -x "$d" ]; then desktop=$d; break; fi
done
shown=0
[ -z "${DISPLAY:-}${WAYLAND_DISPLAY:-}" ] || shown=1
[ "$(uname -s)" != Darwin ] || shown=1
if [ $headless = 0 ] && [ -n "$desktop" ] && [ $shown = 1 ]; then
  [ -z "$home" ] || export BACKPLANE_HOME="$home"
  exec "$desktop"
fi
if [ $headless = 1 ] && [ $fg = 0 ]; then
  dir=${home:-${HOME:-/tmp}/.backplane-bend}
  log=$dir/headless.log
  mkdir -p "$dir"
  if command -v setsid >/dev/null 2>&1; then
    setsid "$here/backplane-serve" "$@" --foreground >>"$log" 2>&1 </dev/null &
  else
    nohup "$here/backplane-serve" "$@" --foreground >>"$log" 2>&1 </dev/null &
  fi
  echo "Backplane runs in the background (pid $!), its log in $log. Stop it: kill $!"
  exit 0
fi
# a display but no desktop app (a hub-only install, or one a 0.11 updater
# moved to this release): the hub's page opens in the browser once it answers
if [ $headless = 0 ] && [ $shown = 1 ]; then
  port=3787 prev=
  for a in "$@"; do [ "$prev" != --port ] || port=$a; prev=$a; done
  opener=xdg-open
  [ "$(uname -s)" != Darwin ] || opener=open
  if command -v "$opener" >/dev/null 2>&1 && command -v curl >/dev/null 2>&1; then
    ( i=0
      while [ $i -lt 150 ]; do
        curl -fs -o /dev/null --max-time 1 "http://127.0.0.1:$port/hello" && { "$opener" "http://127.0.0.1:$port/"; break; }
        sleep 0.2; i=$((i + 1))
      done ) >/dev/null 2>&1 </dev/null &
  fi
fi
exec "$here/backplane-serve" "$@"
