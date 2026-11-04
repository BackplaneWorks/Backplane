#!/bin/sh
# Pair the emulator's app with the shared hub (needs ~/.backplane-gpui/token; never printed).
ROOT=$(cd "$(dirname "$0")/../.." && pwd); T=$(cat ~/.backplane-gpui/token)
L=$(curl -s -m8 -X POST -H "Authorization: Bearer $T" http://desktop.tail9e0cf2.ts.net:3799/auth/invitations | python3 -c 'import json,sys,urllib.parse;print(urllib.parse.quote(json.loads(sys.stdin.read(),strict=False)["link"],safe=""))')
"$ROOT/scripts/gpui/android-emu.sh" adb shell am start -W -a android.intent.action.VIEW -d "'bpgpui://pair?url=$L'" dev.backplane.mobile.gpui | grep -E "Status|Complete"
