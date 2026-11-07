#!/bin/bash
# The switch-over path measured: the Rust hub on the real data copy (events.jsonl only; the JS hub's store.db is not used), under a 3700M scope, stand-in agents, copy guard on.
# HOLD (COMMON.md): no Rust hub on a copy of the real log until the coordinator lifts it. This script therefore refuses the real copy and defaults to the synthetic
# real-size log in ~/.backplane-gpui-scratch/synthetic-real-size. (m0_baseline.mjs refuses it too, unless HOLD_LIFTED=coordinator-lifted.)
# usage: run_m0_rust.sh PHASES...   (first later steady load)   writes build/gpui/results/perf/m0-rust/
R=$(git -C "$(dirname "$0")" rev-parse --show-toplevel); D=$R/build/gpui/perf-run/rust
export HUB_BIN=$D/backplane-hub HUB_ARGS="[\"--policy-bin\",\"$D/bp-policy-pager\",\"--window-bin\",\"$D/window\"]" HUB_LABEL="Rust backplane-hub (gpui e086c32e2c, built on h@backplane)"
export SRC_HOME=${SRC_HOME:-$HOME/.backplane-gpui-scratch/synthetic-real-size}
case "$(readlink -f "$SRC_HOME")" in
  */.backplane-gpui-copy*|*/.backplane-bend*|*/.backplane-gpui|*/.backplane-gpui/*|*/store-copy-backup*|*/person-*|*/.local/share/backplane*)
    echo "refused: SRC_HOME=$SRC_HOME is a real log; a hold is on any Rust hub on a copy of the real log (COMMON.md). Use the synthetic log." >&2; exit 3 ;;
esac
export M0_HOME=$HOME/.backplane-gpui-scratch/perf-rust-m0 M0_OUT=$R/build/gpui/results/perf/m0-rust TZ=Etc/UTC
mkdir -p $M0_OUT
cd "$(dirname "$0")"
[ -n "$WATCH" ] && (WATCH_MS=500 bun m0_watch.mjs &)
for ph in "$@"; do bun m0_baseline.mjs $ph > $M0_OUT/$ph.stdout 2>&1; echo "$ph rc=$?"; tail -c 400 $M0_OUT/$ph.stdout; done
