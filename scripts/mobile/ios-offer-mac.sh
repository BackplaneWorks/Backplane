#!/bin/sh
# On the Mac (ios-offer-journey.sh copies it there): the home hub's offer and failover (docs/pairing.md, Home hub) in the Simulator, against two stand-in hubs
# (test/mobile/offer_stub.py): "desk" offers "boat" with the phone's own token. One screenshot at each moment, and what each stand-in was asked.
#   ios-offer-mac.sh DIR UDID       DIR holds ios/ (the app, built), offer_stub.py, out/ (shots and logs)
set -u
export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
D=$1; U=$2; OUT=$D/out; APP=$D/ios/build/dd/Build/Products/Debug-iphonesimulator/Backplane.app; B=dev.backplane.gpui
T=0123456789abcdef0123456789abcdef; DESK=http://127.0.0.1:3821; BOAT=http://127.0.0.1:3822
rm -rf "$OUT"; mkdir -p "$OUT"
desk() { /usr/bin/python3 "$D/offer_stub.py" 3821 desk $T $BOAT same > "$OUT/desk.log" 2>&1 & DP=$!; sleep 1; }
/usr/bin/python3 "$D/offer_stub.py" 3822 boat $T > "$OUT/boat.log" 2>&1 & BP=$!
desk
stop() { kill $DP $BP 2>/dev/null; xcrun simctl terminate "$U" $B >/dev/null 2>&1; }
trap stop EXIT
xcrun simctl boot "$U" 2>/dev/null; xcrun simctl bootstatus "$U" -b >/dev/null 2>&1
xcrun simctl terminate "$U" $B >/dev/null 2>&1; xcrun simctl uninstall "$U" $B >/dev/null 2>&1; xcrun simctl install "$U" "$APP" || exit 1
launch() { xcrun simctl terminate "$U" $B >/dev/null 2>&1; sleep 1; env SIMCTL_CHILD_BACKPLANE_LINK="$1" xcrun simctl launch "$U" $B -app.alerts '<false/>' >/dev/null; }
shot() { xcrun simctl io "$U" screenshot "$OUT/$1.png" >/dev/null 2>&1; echo "shot $1"; }
seen() { echo "-- $1 asked of $2:"; curl -s -m3 "$3/__seen" | /usr/bin/python3 -c 'import json,sys; [print("  ", json.dumps(x)) for x in json.load(sys.stdin)]' 2>/dev/null || echo "   (down)"; }
log() { xcrun simctl spawn "$U" log show --last "$1" --predicate 'process == "Backplane"' --style compact 2>/dev/null | grep -E "BPPAIR" | sed 's/.*BPPAIR/BPPAIR/' | grep -v 'token='; }
# 1. paired with desk alone: the offer is seen, boat is added silently with the same token, desk stays in use
launch "$DESK/#token=$T"; sleep 12; shot 1-on-desk-boat-added
seen 1 desk $DESK; seen 1 boat $BOAT
# 2. desk stops: after about 20 s the phone is on boat
kill $DP; sleep 35; shot 2-desk-down-on-boat
seen 2 boat $BOAT
# 3. desk answers again: back on desk
desk; sleep 25; shot 3-desk-back
seen 3 desk $DESK
# 4. a start with desk asleep: the list (desk first, boat kept) and the home from before; boat within about 20 s
kill $DP; launch "$DESK/#token=$T $BOAT/#token=$T"; sleep 40; shot 4-start-desk-asleep-on-boat
log 3m > "$OUT/bppair.log"; echo "-- the app said:"; cat "$OUT/bppair.log"
