#!/bin/sh
# iOS logo + cats evidence with the hub, the web reference and the image checks all on h@backplane (nothing heavy on the
# desktop): needs ~/bp-gpui-ios-hub seeded (seed.sh). Mac simulators for the iOS shots. usage: ios-lc-remote.sh
cd "$(dirname "$0")/.." || exit 1
B=$PWD/build/gpui; M=$B/mac-ssh; O=$B/results/ios/lc; V=/home/h/bp-gpui-ios-hub
SB='ssh -o BatchMode=yes h@backplane'
HX='export PW=$HOME/.bun/install/cache/playwright-core@1.63.0@@@1/index.mjs CHROME=$HOME/.cache/ms-playwright/chromium-1243/chrome-linux64/chrome'
mkdir -p $O $B/shots/logo-cats
# the bridge, the hub's token and a thread to open go to the Mac
$SB 'cat ~/bp-gpui-ios-hub/home/token' > /tmp/lc-token; RSYNC_RSH=$M rsync -a /tmp/lc-token hs-mac-mini:bp-gpui-ios/seed7-token
$SB "grep -o '\"ThreadCreated\",\"id\":\"[^\"]*\"' ~/bp-gpui-ios-hub/home/events.jsonl | head -1 | cut -d'\"' -f6" > /tmp/lc-thread.txt; RSYNC_RSH=$M rsync -a /tmp/lc-thread.txt hs-mac-mini:bp-gpui-ios/lc-thread.txt
RSYNC_RSH=$M rsync -a scripts/ios-lc.sh hs-mac-mini:bp-gpui-ios/
for S in dark light; do
  $SB "cd $V/t/tools && \$HOME/.bun/bin/bun set-setting.mjs $V/home 100.96.192.111:3811 ui.theme $S; sleep 2; cd $V; $HX; for p in phone tablet; do timeout 250 \$HOME/.bun/bin/bun t/tools/web-phone-shots.mjs http://100.96.192.111:3811 \$(cat home/token) $V/shots $S \$p 2>&1 | head -3; done"
  $M hs-mac-mini "sh ~/bp-gpui-ios/ios-lc.sh E3BC1B9E-AE4D-498C-97AF-B22FE897B41F phone $S; sh ~/bp-gpui-ios/ios-lc.sh 75A16A8E-C40D-40C2-ABFA-F5E403486F10 ipad $S"
done
RSYNC_RSH=$M rsync -a hs-mac-mini:bp-gpui-ios/lc/ $O/
# the checks, on the box that has numpy/Pillow
$SB "mkdir -p $V/repo/test/visual $V/repo/assets/icon $V/lc"
rsync -a test/visual/ h@backplane:bp-gpui-ios-hub/repo/test/visual/ --exclude '*.json'
rsync -a assets/icon/backplane.svg h@backplane:bp-gpui-ios-hub/repo/assets/icon/
rsync -a $O/ h@backplane:bp-gpui-ios-hub/lc/
$SB "cd $V/repo/test/visual && PY=$V/venv/bin/python; \$PY make_manifest_ios.py $V/lc $V/shots . ; echo mk \$?; for f in manifest-ios-*.json; do n=\${f%.json}; \$PY check_logo_cats.py \$f > check-\$n.txt; echo \"\$n exit \$? pass \$(grep -c ^PASS check-\$n.txt) fail \$(grep -c ^FAIL check-\$n.txt)\"; \$PY side_by_side.py \$f ios-web-vs-native-\${n#manifest-ios-}.png; done; \$PY prove_ios_fails.py manifest-ios-phone-dark.json | tail -5; grep -h ^FAIL check-*.txt | head"
# bring the evidence back (disk only)
rsync -a h@backplane:bp-gpui-ios-hub/shots/ $B/shots/logo-cats/
rsync -a $O/*.png $B/shots/logo-cats/
rsync -a h@backplane:bp-gpui-ios-hub/repo/test/visual/manifest-ios-*.json test/visual/
rsync -a h@backplane:bp-gpui-ios-hub/repo/test/visual/ios-web-vs-native-*.png $B/shots/logo-cats/
rsync -a h@backplane:bp-gpui-ios-hub/repo/test/visual/check-*.txt $B/results/ios/
