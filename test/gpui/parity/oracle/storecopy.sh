#!/bin/sh
# The oracle on the store copy, on this machine only, under the 3.7 GiB cap with no swap. Prints counts and hashes, never content.
# usage: storecopy.sh LINES...   (the Bend fold must fit the cap; a prefix that does not is reported, not forced)
R=/home/h/Documents/i2cjak/backplane-gpui
LOG=$HOME/.backplane-gpui-copy/events.jsonl
for n in "$@"; do
  echo "== prefix $n lines"
  ( ulimit -s 1048576; systemd-run --user --scope -q -p MemoryMax=3700M -p MemorySwapMax=0 -- bun $R/test/gpui/parity/oracle.mjs --log $LOG --lines $n \
      --skip default --bend "/usr/bin/time -f BEND_%e_s_%M_KiB $R/build/gpui/slim/digest" --rust "/usr/bin/time -f RUST_%e_s_%M_KiB $R/build/gpui/slim/rust-digest" 2>&1 | grep -v '^$' | cut -c1-300 )
done
