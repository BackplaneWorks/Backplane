#!/usr/bin/env bash
# The phones' downloads (docs/file-links.md) against a live hub on a temp home: a 120 MB file resumed after the relay drops the connection, and a 403.
# The apps' own download loops run it (Android FileFetchRun.kt in FileFetchLiveTest on the JVM; iOS FileFetchRun.swift in FileFetchLiveTests on the Mac mini).
#   test/mobile/file-links-live.sh HUB android     the JVM test here (gradle)
#   test/mobile/file-links-live.sh HUB ios         the Swift one on hs-mac-mini, through an ssh tunnel to the relay (run it under the mac-heavy lock)
set -euo pipefail
HUB=$(realpath "$1"); MODE=${2:-android}
ROOT=$(cd "$(dirname "$0")/../.." && pwd); . "$ROOT/scripts/lib-bptmp.sh"; . "$ROOT/test/e2e/lib/own-group.sh"
T=$(mktemp -d "$BP_TMP/fl-live-XXXX"); H=$T/home; mkdir -p "$H/attachments" "$T/agents"
trap 'own_stop || true; rm -rf "$T"' EXIT
# free ports (the relay takes RP and RP+1 for its control): never a port in use, such as adb's 5037
free() { python3 -c 'import socket,random,sys
for _ in range(200):
    p=random.randrange(int(sys.argv[1]),int(sys.argv[2]))
    try:
        for q in (p,p+1):
            s=socket.socket(); s.bind(("127.0.0.1",q)); s.close()
        print(p); break
    except OSError: pass' "$1" "$2"; }
P=$(free 3800 4600); RP=$(free 6100 6900)
head -c $(( 120 << 20 )) /dev/urandom > "$H/attachments/big.bin"
echo "not for clients" > "$H/notes.txt"   # under the hub's home but not its attachments: 403 "that file is not shared"
SHA=$(sha256sum "$H/attachments/big.bin" | cut -d' ' -f1)
BACKPLANE_AGENTS=$T/agents own "$HUB" --home "$H" --port $P --host 127.0.0.1 --no-tailscale --foreground > "$T/hub.log" 2>&1
own python3 "$ROOT/scripts/mobile/tcp-relay.py" 127.0.0.1 $RP 127.0.0.1 $P "$T/cut"
for i in $(seq 1 100); do curl -sf "http://127.0.0.1:$P/hello" > /dev/null && [ -s "$H/token" ] && break; sleep 0.2; done
TOK=$(cat "$H/token")
enc() { python3 -c 'import sys,urllib.parse; print(urllib.parse.quote(sys.argv[1], safe=""))' "$1"; }
U="http://127.0.0.1:$RP/file?path=$(enc "$H/attachments/big.bin")"; DENY="http://127.0.0.1:$P/file?path=$(enc "$H/notes.txt")"; CUT="http://127.0.0.1:$(( RP + 1 ))"
echo "hub :$P relay :$RP big.bin sha256 $SHA"
curl -s -o /dev/null -w "direct check: %{http_code} %{size_download} bytes\n" -H "Authorization: Bearer $TOK" -r 0-9 "$U"
curl -s -w " <- deny check %{http_code}\n" -H "Authorization: Bearer $TOK" "$DENY"
case $MODE in
android)
  cd "$ROOT/mobile/android"; [ -f local.properties ] || echo "sdk.dir=$HOME/Android-sdk-writable" > local.properties
  BP_LIVE_URL=$U BP_LIVE_TOKEN=$TOK BP_LIVE_SHA256=$SHA BP_LIVE_CUT=$CUT BP_LIVE_DENY=$DENY JAVA_HOME=${JAVA_HOME:-/usr/lib/jvm/java-17-openjdk-amd64} \
    ./gradlew --no-daemon :app:testDebugUnitTest --tests dev.backplane.mobile.FileFetchLiveTest --rerun -i > "$T/gradle.log" 2>&1 || true
  grep -E "live |FileFetchLiveTest >|FAILED" "$T/gradle.log" | head -20
  X=app/build/test-results/testDebugUnitTest/TEST-dev.backplane.mobile.FileFetchLiveTest.xml
  grep -ho 'testsuite name="[^"]*" tests="[0-9]*" skipped="[0-9]*" failures="[0-9]*" errors="[0-9]*"' $X
  grep -q 'tests="2" skipped="0" failures="0" errors="0"' $X ;;
ios)
  D=bp-fl-live; S=$ROOT/mobile/ios
  ssh hs-mac-mini "rm -rf ~/$D && mkdir -p ~/$D"
  scp -q "$S/Backplane/FileFetch.swift" "$S/Backplane/FileFetchRun.swift" hs-mac-mini:$D/ && scp -q "$S/Tests/FileFetchLiveTests.swift" hs-mac-mini:$D/main.swift
  # the Mac reaches the relay (and its control port, and the hub for the 403) through the tunnel, at the same ports on its own loopback
  ssh -o ExitOnForwardFailure=yes -R $RP:127.0.0.1:$RP -R $(( RP + 1 )):127.0.0.1:$(( RP + 1 )) -R $P:127.0.0.1:$P hs-mac-mini \
    "lockf -k /tmp/bp-mac-heavy.lockf sh -c 'export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer; cd ~/$D && xcrun swiftc -O -o t FileFetch.swift FileFetchRun.swift main.swift && BP_LIVE_URL=\"$U\" BP_LIVE_TOKEN=$TOK BP_LIVE_SHA256=$SHA BP_LIVE_CUT=$CUT BP_LIVE_DENY=\"$DENY\" ./t; r=\$?; cd; rm -rf ~/$D; exit \$r'" ;;
esac
