#!/bin/sh
# The whole job, including tool setup, runs only on Boat with a verified
# 3.7GB/swap0 scope. OOMPolicy keeps the diagnostic parent alive to report it.
set -eu
[ "${BACKPLANE_HEAVY_BOAT_SANDBOX:-}" = 1 ] || { echo 'CAD install tests are Boat-only' >&2; exit 75; }
exec systemd-run --user --scope -q -p MemoryMax=3700000000 -p MemorySwapMax=0 -p OOMPolicy=continue -- scripts/heavy-boat.sh python3 scripts/cad-install-boat-checks.py "$@"
