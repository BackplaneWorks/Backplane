#!/bin/sh
# Compile one C unit of scripts/build-app.sh to its object, or take the
# object of an earlier build of the very same text:
#
#   scripts/cc-unit.sh build/cc/backplane/u3.c
#
# The key is a hash of the unit as the compiler sees it (preprocessed, so
# every header it includes, from wherever CPATH or -I find it, counts), the
# compiler's identity and the flags (BP_CC_ID, BP_CFLAGS, set by
# build-app.sh), so a hit is exactly what the compile would write.
# BP_OBJ_CACHE is the folder ("" = no cache). A unit that names the baked
# Google client (bp_google.h or BP_GOOGLE_*), or any build whose flags do,
# never enters the cache: its object may hold a credential.
set -eu
c=$1
o=${c%.c}.o
dir=${BP_OBJ_CACHE:-}
key=""
secret=0
grep -q 'bp_google\.h\|BP_GOOGLE' "$c" && secret=1
case "${BP_CFLAGS:-}" in *BP_GOOGLE*) secret=1 ;; esac
if [ -n "$dir" ] && [ $secret = 0 ]; then
  if command -v sha256sum >/dev/null 2>&1; then sum="sha256sum"; else sum="shasum -a 256"; fi
  # shellcheck disable=SC2086
  "$CC" -std=c11 $BP_CFLAGS -w -E -P "$c" >"$o.i" || { rm -f "$o.i"; echo "cc -E failed: $c" >&2; exit 255; }
  pre=$($sum <"$o.i" | cut -c1-64)
  rm -f "$o.i"
  key=$(printf '%s\n%s\n%s\n' "${BP_CC_ID:-}" "${BP_CFLAGS:-}" "$pre" | $sum | cut -c1-40)
  if [ -f "$dir/$key.o" ]; then
    cp "$dir/$key.o" "$o" && touch "$dir/$key.o" && { echo hit >"$o.how"; echo "hit  $c" >&2; exit 0; }
  fi
fi
# shellcheck disable=SC2086
# (its peak RSS, KB, beside the object when /usr/bin/time is there)
if [ -x /usr/bin/time ]; then
  /usr/bin/time -f %M -o "$o.rss" "$CC" -std=c11 $BP_CFLAGS -w -c "$c" -o "$o" || { echo "cc failed: $c" >&2; exit 255; }
else
  "$CC" -std=c11 $BP_CFLAGS -w -c "$c" -o "$o" || { echo "cc failed: $c" >&2; exit 255; }
fi
echo cc >"$o.how"
if [ -n "$key" ]; then
  mkdir -p "$dir"
  cp "$o" "$dir/$key.o.$$" && mv "$dir/$key.o.$$" "$dir/$key.o"
fi
echo "cc   $c" >&2
