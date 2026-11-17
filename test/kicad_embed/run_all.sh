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
  # an agent routes in plan view; the Plan panel rows there (bp.13 batch 11 plan-view-api; skips itself without it)
  "embed_plan_route_live_e2e.sh:plan-route-live"
  "embed_comment_e2e.sh:comment"
  "embed_prewarm_e2e.sh:prewarm"
  "embed_click_e2e.sh:click"
  # a reserved key in KiCad opens the palette; dismissed where it was opened, the keyboard is KiCad's again (fork)
  "embed_lend_e2e.sh:lend"
  # KiCad's own Close (Ctrl+W) in the editor: the pane follows it back to View; Cancel in KiCad's save prompt keeps the editor (fork)
  "embed_close_e2e.sh:close"
  # a fresh profile's editors in a narrow pane (bp.13 autopanels): the canvas at least 60 % at 611, 480, 1000 px; the panels back at Full
  # size; a closed panel stays closed (fork)
  "embed_narrow_pane_e2e.sh:narrow-pane"
  # Ctrl+Tab to a schematic opening cold stays there (KiCad's own show request for the board just after it is not followed) (fork)
  "embed_tab_bounce_e2e.sh:tab-bounce"
  # the Backplane KiCad update offered at Edit: a row of its own above the pane, never over it; Not now kept per version (fork)
  "embed_update_row_e2e.sh:update-row"
  # the first Edit on a fresh home paints within 30 s, 10 of 10, KiCad starting 12 s late with an agent's kicad_ipc while it starts: no
  # KiCad "File Open Warning", one process holds the project's lock (the release check's Linux stall, Nov 16) (fork)
  "embed_fresh_edit_e2e.sh:fresh-edit:FRESH_AGENT=during"
  # another live KiCad holds the project's lock: the pane asks Open anyway / Cancel before KiCad opens it (no silent override, no hidden
  # File Open Warning); Cancel starts nothing; a stale lock is cleared and logged (fork)
  "embed_foreign_lock_e2e.sh:foreign-lock"
  # a container is never destroyed with KiCad's holder in it: the window closed, killed, a non-keep detach (the schematic shown), and the
  # window closed with a cold show pending: KiCad alive 20 s after (the same process), the next Edit adopts and paints it, 10 of 10 (fork)
  "embed_client_exit_e2e.sh:client-close"
  "embed_client_exit_e2e.sh:client-kill:CX_EXIT=kill"
  "embed_client_exit_e2e.sh:client-detach:CX_EXIT=detach"
  "embed_client_exit_e2e.sh:client-close-pending:CX_AT=pending"
  # a click on one of KiCad's DRC markers keeps the editor where it is; Escape shuts a side panel before the viewer (fork; skips without Hubble_DK)
  "embed_marker_click_e2e.sh:marker-click"
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

# the second column: the core suites again under GNOME's own window manager (KEMBED_WM=mutter: a private headless gnome-shell and its
# rootless Xwayland, as the person's desktop runs; never their :0). It needs the GPU display, so it runs with REAL_HARDWARE=1 unless
# KEMBED_WMS says otherwise ("i3" only, or "i3 mutter"). About 9 more minutes (window, click, editors, handoff, fullsize, strip_narrow,
# dialog_focus, ux, lend: 542 s under mutter against 525 s under i3, batch 10)
WMS=${KEMBED_WMS:-$([ "${REAL_HARDWARE:-0}" = 1 ] && echo "i3 mutter" || echo i3)}
TOTAL=${#SUITES[@]}
if [ "${EMBED_MODE:-spike}" = fork ] && [[ " $WMS " == *" mutter "* ]]; then
  for script in embed_window_e2e.sh embed_click_e2e.sh embed_editors_e2e.sh embed_handoff_e2e.sh embed_fullsize_e2e.sh embed_strip_narrow_e2e.sh embed_dialog_focus_e2e.sh embed_ux_e2e.sh embed_lend_e2e.sh; do
    tag="mutter-${script#embed_}"; tag=${tag%_e2e.sh}; TOTAL=$((TOTAL + 1))
    echo "=== Running $script under mutter -> $OUT_ROOT/$tag ==="
    if env KEMBED_WM=mutter REAL_HARDWARE=1 OUT="$OUT_ROOT/$tag" "$HERE/$script"; then echo "=== OK $script (mutter) ==="
    else echo "=== FAILED $script (mutter) ==="; FAILED=$((FAILED + 1)); fi
  done
fi

echo "=== Summary: $((TOTAL - FAILED))/$TOTAL suites passed (artifacts: $OUT_ROOT) ==="
[ "$FAILED" -eq 0 ]
