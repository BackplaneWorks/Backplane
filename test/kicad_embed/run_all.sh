#!/bin/bash
# Sequential runner for all three embedded KiCad E2E suites (strictly one KiCad process at a time).
# Usage:
#   EMBED_MODE=spike|stock|fork test/kicad_embed/run_all.sh [OUT_ROOT]
set -euo pipefail

HERE=$(cd "$(dirname "$0")" && pwd)
REPO_ROOT=$(cd "$HERE/../.." && pwd)
# shellcheck source=../../scripts/lib-bptmp.sh
. "$REPO_ROOT/scripts/lib-bptmp.sh"

OUT_ROOT=${1:-$(mktemp -d "$BP_TMP/bp-kembed-all.XXXXXX")}
mkdir -p "$OUT_ROOT"
OUT_ROOT=$(cd "$OUT_ROOT" && pwd)

SUITES=(
  "embed_window_e2e.sh:window"
  "embed_editors_e2e.sh:editors"
  "embed_agent_resilience_e2e.sh:resilience"
  "embed_ux_e2e.sh:ux"
  "embed_xprobe_e2e.sh:xprobe"
  "embed_bp9_e2e.sh:bp9"
  "embed_checks_e2e.sh:checks"
  # the editor row on a schematic every load must repair (test/files/fixtures/broken-toplevel, the shape of the person's Hubble_DK after a
  # bp.11 headless save): Board -> Schematic must draw (bp.11/bp.12-pre hung behind a hidden "automatically fixed" modal). Fork mode only.
  "embed_ux_e2e.sh:ux-broken-toplevel:KEMBED_PROJ_SRC=$REPO_ROOT/test/files/fixtures/broken-toplevel"
  "embed_mgmt_e2e.sh:mgmt"
  "embed_comment_e2e.sh:comment"
)

FAILED=0
for entry in "${SUITES[@]}"; do
  IFS=: read -r script tag extra <<<"$entry"
  if [ -n "$extra" ] && [ "${EMBED_MODE:-spike}" != "fork" ]; then echo "=== SKIP $tag (fork mode only) ==="; continue; fi
  echo "=== Running $script${extra:+ ($extra)} (EMBED_MODE=${EMBED_MODE:-spike}) -> $OUT_ROOT/$tag ==="
  if env ${extra:+"$extra"} OUT="$OUT_ROOT/$tag" "$HERE/$script"; then
    echo "=== OK $script ==="
  else
    echo "=== FAILED $script ==="
    FAILED=$((FAILED + 1))
  fi
done

echo "=== Summary: $((${#SUITES[@]} - FAILED))/${#SUITES[@]} suites passed (artifacts: $OUT_ROOT) ==="
[ "$FAILED" -eq 0 ]
