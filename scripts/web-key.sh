#!/bin/sh
# The key the built web client (dist/web) is kept under (scripts/dev.sh's
# build_web_dist): scripts/inputs-key.sh over what bend reads for
# src/web/app.bend (BP_KEY_ROOTS), every other file in src/web (the page,
# its scripts, styles and icons) and the scripts that post-process it.
# Fails closed: a reference in the page, a script or a style that does not
# resolve to a file in src/web prints nothing and exits 1, so the web is
# built afresh.
#   scripts/web-key.sh
set -u
cd "$(dirname "$0")/.." || exit 1
w=src/web
[ -f "$w/index.html" ] && [ -f "$w/app.bend" ] || exit 1
# every ./ reference: the page's src/href, the scripts' imports, url() in css
refs=$( { sed -n 's/.*\(src\|href\)="\([^"]*\)".*/\2/p' "$w/index.html"
          sed -n "s/.*\(from\|import(\)[[:space:]]*[\"']\([^\"']*\)[\"'].*/\2/p" "$w"/*.js
          sed -n 's/.*url(["'"'"']\{0,1\}\([^)"'"'"']*\).*/\1/p' "$w"/*.css; } | sort -u)
for r in $refs; do
  case $r in
    ./*) [ -f "$w/${r#./}" ] || exit 1 ;;
    data:*|http:*|https:*|\#*) ;;
    *) exit 1 ;;
  esac
done
files=$(find "$w" -type f ! -name '*.bend' | LC_ALL=C sort) || exit 1
# shellcheck disable=SC2086
BP_KEY_ROOTS=$w/app.bend scripts/inputs-key.sh web1 $files scripts/web-key.sh \
  scripts/js-unmarshal.py scripts/js-native.py scripts/inputs-key.sh scripts/bend-closure.py
