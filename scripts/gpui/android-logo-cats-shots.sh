#!/bin/sh
# The logo and cats on Android vs the web, dark and light, on a seeded stand-in hub with bots (home.bots, port 3814, already running
# with BACKPLANE_AGENTS checked). Needs the emulator up. usage: android-logo-cats-shots.sh APK
set -eu
ROOT=$(cd "$(dirname "$0")/../.." && pwd); E=$ROOT/scripts/gpui/android-emu.sh; U="python3 $ROOT/scripts/gpui/android-ui.py"; H=$HOME/bp-gpui-android-hub
S=$ROOT/build/gpui/shots/logo-cats; mkdir -p $S/raw $S/web-raw; APK=$1; TOK=$H/home.bots/token
for sc in dark light; do
  bun $ROOT/test/gpui/set-setting.mjs $H/home.bots 100.64.84.41:3814 ui.theme $sc; sleep 1
  bun $ROOT/scripts/gpui/android-web-ref.mjs http://100.64.84.41:3814 $(cat $TOK) $S/web-raw $sc
  $ROOT/scripts/gpui/android-fresh.sh $APK 3814 $TOK $sc; sleep 3
  B=$S/raw/android-$sc-boxes.json
  $E adb exec-out screencap -p > $S/raw/android-$sc-1-threads.png
  set -- $($U bounds Backplane exact); echo "{\"logo\":[$1,$2,$(( $3 - $1 )),$(( $4 - $2 ))]," > $B.part
  xy=$($U find Bots exact); $E adb shell input tap $xy; sleep 3
  $E adb exec-out screencap -p > $S/raw/android-$sc-2-bots.png
  printf '"rows":{' >> $B.part; first=1
  for n in Bub Mochi Pixel Ember Nori Zed; do set -- $($U rowbox $n); cy=$(( ($2 + $4) / 2 )); [ $first = 1 ] || printf ',' >> $B.part; first=0; printf '"%s":[%d,%d,74,74]' $n $(( $1 + 42 )) $(( cy - 37 )) >> $B.part; done; printf '},' >> $B.part
  set -- $($U find Mochi); $E adb shell input tap $1 $2; sleep 4
  $E adb exec-out screencap -p > $S/raw/android-$sc-3-bot.png
  set -- $($U bounds Mochi exact 1); t=$2; l=$1; set -- $($U bounds working exact 0); b=$4
  cy=$(( (t + b) / 2 )); echo "\"header\":[$(( l - 37 - 110 )),$(( cy - 55 )),110,110]" >> $B.part
  # the mention popup: `@` in this bot's composer lists the bots (names, no cats: as the web)
  set -- $($U find "Ask the agent"); $E adb shell input tap $1 $2; sleep 1; $E adb shell input text "@"; sleep 5
  $E adb exec-out screencap -p > $S/raw/android-$sc-4-mention.png
  $E adb shell input keyevent 67; $E adb shell input keyevent 4; sleep 1
  # the bot's Settings tab: the look picker
  $E adb shell input swipe 900 515 100 515 200; sleep 1; set -- $($U find Settings exact); $E adb shell input tap $1 $2; sleep 3
  for i in 1 2 3; do $U find "Look" exact >/dev/null 2>&1 && break; $E adb shell input swipe 540 1900 540 900 300; sleep 1; done
  $E adb exec-out screencap -p > $S/raw/android-$sc-6-looks.png
  printf ',"looks":[' >> $B.part; first=1
  for k in 1 2 3 4 5 6; do set -- $($U clickables 126 126 | sed -n ${k}p); [ -n "${1:-}" ] || continue; [ $first = 1 ] || printf ',' >> $B.part; first=0; printf '[%d,%d,105,105]' $(( $1 + 8 )) $(( $2 + 8 )) >> $B.part; done; printf ']' >> $B.part
  # the room den and its posts
  $E adb shell input keyevent 4; sleep 2; $U has Rooms exact || { $E adb shell input keyevent 4; sleep 2; }
  set -- $($U find Rooms exact); $E adb shell input tap $1 $2; sleep 2; set -- $($U find den); $E adb shell input tap $1 $2; sleep 4
  $E adb exec-out screencap -p > $S/raw/android-$sc-5-room.png
  printf ',"posts":{' >> $B.part; first=1
  for n in Bub Mochi Pixel Ember; do set -- $($U bounds $n exact); [ -n "${1:-}" ] || continue; [ $first = 1 ] || printf ',' >> $B.part; first=0; printf '"%s":[%d,%d,74,74]' $n 42 $2 >> $B.part; done; printf '}' >> $B.part
  printf '}' >> $B.part
  tr -d '\n' < $B.part > $B; rm -f $B.part; cat $B; echo
  python3 $ROOT/scripts/gpui/android-logo-cats-assemble.py $sc $B
done
