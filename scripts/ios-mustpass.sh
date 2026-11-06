#!/bin/sh
# Must-pass checks on iOS that the simulator can drive through BACKPLANE_ACTS (no tap tools): the hub is the seeded
# stand-in hub on h@backplane (seed.sh + seedmd.sh); shots go to build/gpui/shots/mustpass/. usage: ios-mustpass.sh
cd "$(dirname "$0")/.." || exit 1
B=$PWD/build/gpui; M=$B/mac-ssh; U=E3BC1B9E-AE4D-498C-97AF-B22FE897B41F; P=75A16A8E-C40D-40C2-ABFA-F5E403486F10
SB='ssh -o BatchMode=yes h@backplane'
$SB 'cat ~/bp-gpui-ios-hub/home/token' > /tmp/lc-token; RSYNC_RSH=$M rsync -a /tmp/lc-token hs-mac-mini:bp-gpui-ios/seed7-token
T1=$($SB "grep -o '\"ThreadCreated\",\"id\":\"[^\"]*\"' ~/bp-gpui-ios-hub/home/events.jsonl | sed -n 2p | cut -d'\"' -f6")
T0=$($SB "grep -o '\"ThreadCreated\",\"id\":\"[^\"]*\"' ~/bp-gpui-ios-hub/home/events.jsonl | sed -n 1p | cut -d'\"' -f6")
K=backplane.tail9e0cf2.ts.net:3811
$M hs-mac-mini "export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer HUBURL=http://backplane.tail9e0cf2.ts.net:3811/ TOKFILE=~/bp-gpui-ios/seed7-token
D=~/bp-gpui-ios/shots
xcrun simctl boot $U 2>/dev/null; xcrun simctl uninstall $U dev.backplane.gpui
# 3: attachment: a preview in the composer, then sent
sh ~/bp-gpui-ios/ios-shot.sh $U mp-attach-preview 22 'ob-skip;;@attach' $T1
sh ~/bp-gpui-ios/ios-shot.sh $U mp-attach-sent 30 'ob-skip;;@type=attached picture;;@attach;;@send' $T1
# 14: a draft survives a thread switch, then a restart
sh ~/bp-gpui-ios/ios-shot.sh $U mp-draft-switch 30 'ob-skip;;@type=draft one;;@back;;select=$K|$T0;;@back;;select=$K|$T1' $T1
xcrun simctl terminate $U dev.backplane.gpui; sleep 2
sh ~/bp-gpui-ios/ios-shot.sh $U mp-draft-restart 22 'ob-skip' $T1
# 7, 15-20: markdown and the code block's Copy button, phone and iPad
sh ~/bp-gpui-ios/ios-shot.sh $U mp-markdown-phone 22 'ob-skip' $T1
xcrun simctl shutdown $U; xcrun simctl boot $P; xcrun simctl bootstatus $P -b >/dev/null; xcrun simctl uninstall $P dev.backplane.gpui
sh ~/bp-gpui-ios/ios-shot.sh $P mp-markdown-ipad 25 'ob-skip' $T1
xcrun simctl shutdown $P"
mkdir -p $B/shots/mustpass
for n in mp-attach-preview mp-attach-sent mp-draft-switch mp-draft-restart mp-markdown-phone mp-markdown-ipad; do RSYNC_RSH=$M rsync -a hs-mac-mini:bp-gpui-ios/shots/$n.png $B/shots/mustpass/; done
