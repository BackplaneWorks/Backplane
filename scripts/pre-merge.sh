#!/bin/sh
# All merge-tree and LAWS/PROOF validation runs on capped Boat workers.
# The local process only transports a pushed HEAD and checks receipt identity.
# Guard-changing PRs require an explicit reviewed exact-head acknowledgement.
#   scripts/pre-merge.sh [origin/BRANCH|COMMIT] [--allow-reviewed-guard-change HEAD]
# Explicit approved runner: --runner /absolute/path --runner-sha256 SHA256
# Older runners: --runner-transport exclude (same one-worker-file payload).
set -eu
cd "$(dirname "$0")/.."
exec python3 -IB scripts/pre-merge-boat.py dispatch "$@"
