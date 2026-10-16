#!/bin/sh
# Break the generated-source cycle into a stable two-pass build.
set -eu
cd "$(dirname "$0")/.."
python3 scripts/gen-governance.py
scripts/check.sh "${1:-check}"
python3 scripts/gen-governance.py --check
