#!/bin/sh
# Compose fallback with current-main entry/notification laws; keep full-suite evidence.
set -eu
export BUN_JSC_forceRAMSize=4294967296 CHECK_JOBS=2
scripts/check.sh all
bend test/exe_test.bend
