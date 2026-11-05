#!/bin/sh
# JetBrains Mono on iOS: the font probe (@fonts) and a Markdown code block, iPhone and iPad, dark and light, then
# test/gpui/visual/check_fonts_ios.py on h@backplane (numpy/Pillow in a throwaway venv). Hub of our own, deleted at the end.
cd "$(dirname "$0")/../.." || exit 1
B=$PWD/build/gpui; M=$B/mac-ssh; SB='ssh -o BatchMode=yes h@backplane'; U=E3BC1B9E-AE4D-498C-97AF-B22FE897B41F; P=75A16A8E-C40D-40C2-ABFA-F5E403486F10; V=/home/h/bp-gpui-ios-hub
OUT=$B/results/ios/fonts; SH=$B/shots/ios; mkdir -p $OUT
$SB "mkdir -p $V/home $V/proj $V/bin $V/src/web $V/src/core $V/t $V/fonts"
rsync -a $B/hub/android/backplane-serve h@backplane:bp-gpui-ios-hub/hub
rsync -a test/tools/claude_standin.ts test/gpui/set-setting.mjs test/gpui/wire.mjs scripts/gpui/ios-seed.ts h@backplane:bp-gpui-ios-hub/t/tools/
rsync -a src/web/wire.js h@backplane:bp-gpui-ios-hub/src/web/; rsync -a src/core/cbor.bend h@backplane:bp-gpui-ios-hub/src/core/
rsync -a scripts/gpui/ios-hubrun.sh h@backplane:bp-gpui-ios-hub/run.sh
rsync -a assets/fonts/jetbrains-mono/ h@backplane:bp-gpui-ios-hub/fonts/
rsync -a test/gpui/visual/check_fonts.py test/gpui/visual/check_fonts_ios.py h@backplane:bp-gpui-ios-hub/t/
$SB "cd $V && ln -sfn t/tools tools; sh run.sh start | tail -2; B=\$HOME/.bun/bin/bun; HP=100.96.192.111:3811; \$B tools/ios-seed.ts \$HP $V/home $V/proj >/dev/null 2>&1; \$B tools/ios-seed.ts \$HP $V/home $V/proj new 2 >/dev/null 2>&1; sh run.sh stop >/dev/null
python3 - <<'PY'
import json,os
f=os.path.expanduser('~/bp-gpui-ios-hub/home/events.jsonl'); L=[json.loads(l) for l in open(f).read().split('\n') if l]
th=[e['id'] for e in L if e.get('\$')=='ThreadCreated'][1]; t=max(e.get('at',0) for e in L)+5
md='Code:\n\n\`\`\`python\ndef f(x):\n    return x * 2  # comment\n\`\`\`\n\nInline \`code_here\` and => != -> ligature candidates.\n'
open(f,'a').write(json.dumps({'\$':'MessagePosted','thread':th,'msg':'md-1','role':'assistant','text':md,'at':t},separators=(',',':'))+'\n'); print(th)
PY
rm -f home/store.db*; sh run.sh start | tail -2"
$SB "cat $V/home/token" > /tmp/lc-token; RSYNC_RSH=$M rsync -a /tmp/lc-token hs-mac-mini:bp-gpui-ios/seed7-token
T=$($SB "grep -o '\"ThreadCreated\",\"id\":\"[^\"]*\"' $V/home/events.jsonl | sed -n 2p | cut -d'\"' -f6")
scripts/gpui/ios-build.sh > $B/results/ios/build-fonts.log 2>&1; grep -E "error:|build seconds" $B/results/ios/build-fonts.log | head -2
for S in dark light; do
  $SB "cd $V/t/tools && \$HOME/.bun/bin/bun set-setting.mjs $V/home 100.96.192.111:3811 ui.theme $S; sleep 2"
  $M hs-mac-mini "export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer HUBURL=http://backplane.tail9e0cf2.ts.net:3811/ TOKFILE=~/bp-gpui-ios/seed7-token
xcrun simctl boot $U 2>/dev/null; xcrun simctl uninstall $U dev.backplane.gpui
sh ~/bp-gpui-ios/ios-shot.sh $U fonts-phone-$S 25 'ob-skip;;@fonts' $T
sh ~/bp-gpui-ios/ios-shot.sh $U code-phone-$S 25 'ob-skip' $T
xcrun simctl shutdown $U; xcrun simctl boot $P; xcrun simctl bootstatus $P -b >/dev/null; xcrun simctl uninstall $P dev.backplane.gpui
sh ~/bp-gpui-ios/ios-shot.sh $P fonts-ipad-$S 25 'ob-skip;;@fonts' $T
xcrun simctl shutdown $P"
  for n in fonts-phone-$S code-phone-$S fonts-ipad-$S; do RSYNC_RSH=$M rsync -a hs-mac-mini:bp-gpui-ios/shots/$n.png $SH/; done
done
rsync -a $SH/fonts-*.png h@backplane:bp-gpui-ios-hub/fonts/
$SB "cd $V && python3 -m venv venv && venv/bin/pip install -q numpy pillow 2>&1 | tail -1; cd t; for s in dark light; do for d in phone:3 ipad:2; do n=\${d%%:*}; k=\${d##*:}; echo == \$n \$s; ../venv/bin/python check_fonts_ios.py ../fonts/fonts-\$n-\$s.png ../fonts \$s \$k; done; done; ../venv/bin/python check_fonts_ios.py ../fonts/fonts-phone-dark.png ../fonts dark 3 --selftest" | tee $OUT/check.txt
$SB "sh $V/run.sh stop; rm -rf $V; ls -d $V 2>&1 | tail -1"
