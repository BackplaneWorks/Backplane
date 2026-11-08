#!/bin/sh
# New TestFlight crashes of the iOS app, for a check-in: scripts/mobile/testflight-crashes.sh [--days N] [--all]
# Runs scripts/mobile/asc_crashes.py on the Mac that cuts TestFlight builds (MAC, default hs-mac-mini), which holds the App Store Connect key the uploads
# use; nothing secret is here or printed. Read-only. One JSON line per crash not reported before, then {"summary": ...}. Logs stay on the Mac (~/bp-gpui-ios/crashes).
# MAC_SSH: an ssh to use instead of plain ssh (for a key of its own).
here=$(cd "$(dirname "$0")" && pwd); mac=${MAC:-hs-mac-mini}; ssh=${MAC_SSH:-ssh}
$ssh "$mac" 'mkdir -p ~/bp-gpui-ios && cat > ~/bp-gpui-ios/asc_crashes.py' < "$here/asc_crashes.py" || { echo '{"error":"cannot reach the Mac"}'; exit 1; }
exec $ssh "$mac" "python3 ~/bp-gpui-ios/asc_crashes.py $*"
