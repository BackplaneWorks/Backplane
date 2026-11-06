#!/bin/sh
# usage: run.sh RUST_DIGEST_DIR LOGDIR   (RUST_DIGEST_DIR holds <family>.json from `rust-digest --json <family>.jsonl`, made on Boat)
# Compares each log's Bend digest (build/gpui/slim/digest, a finished binary) with the Rust one through the oracle, --skip none. Prints hashes and counts only.
R=${BP_ROOT:-/home/h/Documents/i2cjak/backplane-gpui}
rc=0
for f in "$2"/*.jsonl; do n=$(basename "$f" .jsonl)
  cat > /tmp/rust-art-$$.sh <<EOS
#!/bin/sh
cat "$1/$n.json"
EOS
  chmod +x /tmp/rust-art-$$.sh
  ( ulimit -s 1048576; echo "== $n"; bun $R/test/gpui/parity/oracle.mjs --log "$f" --skip none --bend "$R/build/gpui/slim/digest" --rust "/tmp/rust-art-$$.sh" 2>&1 | cut -c1-300 ) || rc=1
done
rm -f /tmp/rust-art-$$.sh; exit $rc
