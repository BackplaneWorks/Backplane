#!/bin/sh
# The Mac mini gate's helper as a LaunchAgent in h's GUI session (no sudo): run on the Mac, over ssh.
#   gate_agent.sh build     compile gate_helper.m into ~/bp-gate-m/gate_helper
#   gate_agent.sh up        write ~/Library/LaunchAgents/dev.backplane.gate-helper.plist and bootstrap it in gui/$UID
#   gate_agent.sh ask REQ   one request to the running helper (windows | displays | active PID | virtual N WxH)
#   gate_agent.sh down      bootout, remove the plist and the socket (after every run: the gate's change list says so)
# Nothing it starts needs a TCC grant: window bounds and owners, the active app and displays are not guarded.
set -eu
DIR=$HOME/bp-gate-m; LABEL=dev.backplane.gate-helper; PLIST=$HOME/Library/LaunchAgents/$LABEL.plist; SOCK=$DIR/helper.sock
HERE=$(cd "$(dirname "$0")" && pwd)
case "${1:-}" in
build)
  mkdir -p "$DIR"
  DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer clang -fobjc-arc -Wall -O1 \
    -framework Foundation -framework AppKit -framework CoreGraphics "$HERE/gate_helper.m" -o "$DIR/gate_helper"
  echo "built $DIR/gate_helper" ;;
up)
  [ -x "$DIR/gate_helper" ] || { echo "build first" >&2; exit 1; }
  launchctl print "gui/$(id -u)" >/dev/null 2>&1 || { echo "no GUI session for $(id -un): log in first" >&2; exit 2; }
  mkdir -p "$HOME/Library/LaunchAgents"
  cat > "$PLIST" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>Label</key><string>$LABEL</string>
  <key>ProgramArguments</key><array><string>$DIR/gate_helper</string><string>serve</string><string>$SOCK</string></array>
  <key>RunAtLoad</key><true/>
  <key>KeepAlive</key><true/>
  <key>ProcessType</key><string>Interactive</string>
  <key>StandardErrorPath</key><string>$DIR/helper.log</string>
</dict></plist>
EOF
  launchctl bootstrap "gui/$(id -u)" "$PLIST"
  for _ in 1 2 3 4 5 6 7 8 9 10; do [ -S "$SOCK" ] && break; sleep 0.5; done
  [ -S "$SOCK" ] && echo "up: $LABEL ($SOCK)" || { echo "the helper did not start: $(tail -5 "$DIR/helper.log" 2>/dev/null)" >&2; exit 3; } ;;
ask)
  shift; REQ="$*" SOCK="$SOCK" /usr/bin/python3 -c '
import os,socket
s=socket.socket(socket.AF_UNIX); s.connect(os.environ["SOCK"]); s.sendall((os.environ["REQ"]+"\n").encode())
print(s.makefile().readline().rstrip())' ;;
down)
  launchctl bootout "gui/$(id -u)/$LABEL" 2>/dev/null || true
  rm -f "$PLIST" "$SOCK"
  echo "down: $LABEL unloaded, plist and socket removed" ;;
*) echo "usage: $0 build|up|ask REQ|down" >&2; exit 64 ;;
esac
