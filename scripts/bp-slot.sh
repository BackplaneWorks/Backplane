#!/bin/sh
# Runs on h@backplane: one job in one of N build slots. usage: bp-slot.sh LANE BASE64_COMMAND
# flock -o closes the lock's descriptor before the job starts: a stray child (a test's stand-in `sleep`) must never keep a slot.
LANE=$1; CMD=$(printf %s "$2" | base64 -d); B=$HOME/bp-gpui-build; N=${BP_SLOTS:-3}
export PATH=$HOME/.cargo/bin:$PATH CARGO_BUILD_JOBS=5 CARGO_TARGET_DIR=$B/$LANE/.target SCCACHE_CACHE_SIZE=20G
# This tree's .cargo/config.toml wants clang + mold; the builder has neither. A set RUSTFLAGS skips the config's rustflags,
# and the linker env var beats its `linker`, so link with the system cc there.
if ! command -v clang >/dev/null 2>&1; then
  export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=cc RUSTFLAGS="${RUSTFLAGS:-} --cfg bp_cc_link"
fi
[ -x "$HOME/.cargo/bin/sccache" ] && export RUSTC_WRAPPER=$HOME/.cargo/bin/sccache
cd "$B/$LANE" || exit 70
# Disk guard (scripts/boat-disk.py, as on the Boat boxes): over 80 % of the disk, stale incremental state, then lane trees unused for two days, then older build output go
# until under 70 %; never this lane or one a running job is in. One guard at a time; what went is in $B/.disk.log, its first line on stderr.
if [ -f scripts/boat-disk.py ] && command -v python3 > /dev/null 2>&1; then
  g=$(flock -n "$B/.disk.lock" python3 -I scripts/boat-disk.py guard --lanes "$B" --keep "$LANE" --high "${BP_DISK_HIGH:-80}" --low "${BP_DISK_LOW:-70}" 2>&1) || true
  [ -n "$g" ] && { printf '%s\n' "$g" | sed "s|^|$(date '+%F %T') $LANE |" >> "$B/.disk.log"; printf '%s\n' "$g" | head -1 >&2; }
fi
while :; do
  s=1
  while [ "$s" -le "$N" ]; do
    flock -n -o -E 200 "$B/.slot.$s" timeout 3000 systemd-run --user --scope -q -- sh -c "$CMD"
    rc=$?; [ "$rc" = 200 ] || exit "$rc"
    s=$((s+1))
  done
  sleep 5
done
