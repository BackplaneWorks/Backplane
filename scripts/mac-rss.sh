#!/bin/sh
# BSD time reports resident bytes; build stage reports use KiB everywhere.
# Keep the child's stdout, stderr and exit status intact.
set -u
report=$1
shift
raw="$report.raw.$$"
if /usr/bin/time -l -o "$raw" "$@"; then status=0; else status=$?; fi
if [ -f "$raw" ]; then
  awk '/maximum resident set size/ { printf "%.0f\n", $1 / 1024; exit }' "$raw" > "$report"
  rm -f "$raw"
fi
exit "$status"
