#!/bin/sh
# The iOS 0.14 import card end to end: the import rig (ios-migration-rig.sh) on this machine, the app built and its unit tests run on the Mac, the Simulator's
# screenshots of the banner and the card while it runs, when it is done, and after Continue. Run under the Backplane lock mac-heavy (work_run lock); the Mac
# side holds its lockf too. Shots land in $OUT (default $BP_TMP/imig/shots).
#   scripts/mobile/ios-migration-check.sh        env: M (the ssh wrapper), SIM_UDID, OUT, HUB
set -u
ROOT=$(cd "$(dirname "$0")/../.." && pwd); . "$ROOT/scripts/lib-bptmp.sh"
M=${M:-$(git -C "$(dirname "$0")" worktree list --porcelain | awk 'NR==1{print substr($0,10)}')/build/gpui/mac-ssh}; U=${SIM_UDID:-E3BC1B9E-AE4D-498C-97AF-B22FE897B41F}; OUT=${OUT:-$BP_TMP/imig/shots}
IP=$(tailscale ip -4 | head -1); PORT=3861; D=bp-imig; R=$BP_TMP/imig-rig
mkdir -p "$OUT"
RSYNC_RSH=$M rsync -a --delete --exclude build "$ROOT/mobile/ios/" hs-mac-mini:$D/ios/ || exit 1
$M hs-mac-mini "mkdir -p $D/root/mobile/ios $D/root/scripts/mobile $D/root/test/mobile $D/root/test/fixtures; rm -rf $D/out" || exit 1
RSYNC_RSH=$M rsync -a --delete "$ROOT/mobile/ios/Backplane" "$ROOT/mobile/ios/Tests" hs-mac-mini:$D/root/mobile/ios/ || exit 1
RSYNC_RSH=$M rsync -a "$ROOT/scripts/mobile/ios-unit-tests.sh" "$ROOT/scripts/mobile/ios-migration-mac.sh" hs-mac-mini:$D/root/scripts/mobile/ || exit 1
RSYNC_RSH=$M rsync -a "$ROOT/test/mobile/hubs_cases.json" hs-mac-mini:$D/root/test/mobile/ || exit 1
RSYNC_RSH=$M rsync -a "$ROOT/test/fixtures/voice-edits.json" hs-mac-mini:$D/root/test/fixtures/ || exit 1
# the app and its unit tests first: the import runs a few minutes from the hub's start
$M hs-mac-mini "lockf -k /tmp/bp-mac-heavy.lockf sh \$HOME/$D/root/scripts/mobile/ios-migration-mac.sh build \$HOME/$D" || exit 1
"$ROOT/scripts/mobile/ios-migration-rig.sh" start || exit 1
RSYNC_RSH=$M rsync -a "$R/home/token" hs-mac-mini:$D/token || exit 1
$M hs-mac-mini "lockf -k /tmp/bp-mac-heavy.lockf sh \$HOME/$D/root/scripts/mobile/ios-migration-mac.sh shots \$HOME/$D http://$IP:$PORT $U"
RSYNC_RSH=$M rsync -a hs-mac-mini:$D/out/ "$OUT/"
journalctl --user -u bp-imig-hub --no-pager 2>/dev/null | grep -i import | tail -5
"$ROOT/scripts/mobile/ios-migration-rig.sh" stop
ls -la "$OUT"
