#!/bin/sh
# Thread-count sweep: for each count, a fresh pair through the logging proxy, then the bridge's own size check on the captured join (harness).
# needs: emulator up, proxy on 3815->3814, hub on a fresh home (HOMEDIR) already seeded with a project.
set -u
ROOT=$(cd "$(dirname "$0")/.." && pwd); E=$ROOT/scripts/android-emu.sh; H=$HOME/bp-gpui-android-hub; HD=$1; PID=$2; shift 2
R=$ROOT/build/gpui/results/android/iso; cur=${CUR:-1}
for n in "$@"; do
  add=$((n - cur)); [ $add -gt 0 ] && timeout 120 bun $ROOT/scripts/android-seed.ts 100.64.84.41:3814 $HD $PID $add >/dev/null 2>&1; cur=$n
  have=$(grep -c ThreadCreated $HD/events.jsonl)
  $E adb shell pm clear dev.backplane.mobile.gpui >/dev/null; $E adb shell pm grant dev.backplane.mobile.gpui android.permission.POST_NOTIFICATIONS
  PORT=3815 TOKENFILE=$HD/token $ROOT/scripts/android-pair.sh >/dev/null; sleep 12
  f2=$(ls -t $R/sweep.log.*s2c_2.bin | head -1); f1=$(ls -t $R/sweep.log.*s2c_1.bin | head -1)
  sz=$(stat -c %s $f2)
  res=$(cd $ROOT/build/gpui/android/harness && bun ${REPLAY:-replay.ts} $f1 $f2 2>&1 | tail -2 | tr '\n' ' ' | sed -E 's/.*phase":"([a-z]+)".*"F":([0-9]+).*/\1 F=\2/')
  echo "threads=$have cbor_frame_bytes=$sz bridge: $res" | tee -a $R/sweep.txt
done
