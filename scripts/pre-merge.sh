#!/bin/sh
# All merge-tree and LAWS/PROOF validation runs on capped Boat workers.
# The local process only transports a pushed HEAD and checks receipt identity.
#   scripts/pre-merge.sh [origin/BRANCH|COMMIT]  (default origin/main)
set -eu
[ "$#" -le 1 ] || { echo "usage: scripts/pre-merge.sh [origin/BRANCH|COMMIT]" >&2; exit 2; }
cd "$(dirname "$0")/.."
exec python3 -IB scripts/pre-merge-boat.py dispatch "${1:-origin/main}"
