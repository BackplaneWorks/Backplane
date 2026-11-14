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
  "embed_checks_panel_e2e.sh:checks-panel"
  "embed_gerbers_e2e.sh:gerbers"
  # the editor row on a schematic every load must repair (test/files/fixtures/broken-toplevel, the shape of the person's Hubble_DK after a
  # bp.11 headless save): Board -> Schematic must draw (bp.11/bp.12-pre hung behind a hidden "automatically fixed" modal). Fork mode only.
  "embed_ux_e2e.sh:ux-broken-toplevel:KEMBED_PROJ_SRC=$REPO_ROOT/test/files/fixtures/broken-toplevel"
  "embed_mgmt_e2e.sh:mgmt"
  "embed_fab_e2e.sh:fab"
  # the person's edits in KiCad's plan view (bp.13+; skips itself on runtimes without them, and outside fork mode)
  "embed_plan_person_e2e.sh:plan-person"
  "embed_comment_e2e.sh:comment"
  "embed_prewarm_e2e.sh:prewarm"
  "embed_click_e2e.sh:click"
  "embed_thread_e2e.sh:thread"
  # a BOM edit lands for a symbol in a sub-sheet used twice (the fork's complex_hierarchy, copied), read back from the saved file (fork only)
  "embed_bom_sheets_e2e.sh:bom-sheets:BOM_CASE=twice"
  # Edit pressed before the viewer knows its file is kept and acted on (fork only)
  "embed_edit_early_e2e.sh:edit-early"
  # a start that misses its window is retried once by the hub; twice, the pane offers Try again (fork only)
  "embed_start_retry_e2e.sh:start-retry"
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
