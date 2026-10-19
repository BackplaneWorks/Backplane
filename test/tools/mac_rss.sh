#!/bin/sh
# Run on the actual Darwin host, with a disposable private directory.
set -eu
[ "$(uname -s)" = Darwin ] || { echo 'requires macOS' >&2; exit 2; }
root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
tmp=$(mktemp -d)
trap 'rm -f "$tmp/rss" "$tmp/out" "$tmp/err"; rmdir "$tmp"' EXIT
if "$root/scripts/mac-rss.sh" "$tmp/rss" sh -c 'printf "child stdout\n"; printf "child stderr\n" >&2; exit 23' >"$tmp/out" 2>"$tmp/err"; then
  echo 'lost child failure' >&2; exit 1
else status=$?; fi
[ "$status" = 23 ]
[ "$(cat "$tmp/out")" = 'child stdout' ]
grep -q '^child stderr$' "$tmp/err"
awk 'NF != 1 || $1 !~ /^[0-9]+$/ || $1 <= 0 { exit 1 } END { if (NR != 1) exit 1 }' "$tmp/rss"
# BSD time may add its own status diagnostic but must preserve child stderr.
"$root/scripts/mac-rss.sh" "$tmp/rss" true
awk '$1 <= 0 {exit 1} END {if(NR != 1) exit 1}' "$tmp/rss"
echo MAC_RSS_STDIO_STATUS_KIB_PASS
