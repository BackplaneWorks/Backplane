#!/usr/bin/env python3
"""Check an undo journey, or replay its saved real_use.json without starting KiCad."""
import json
import sys


def check_undo(results, skip=()):
    """1 when a required step did not happen; `skip` names steps a diagnostic run left out on purpose (KEMBED_UNDO_NO_CTRL_Y=1)."""
    required = (
        "undo_panel_edit_applies",
        "undo_ctrl_z_takes_it_back",
        "undo_ctrl_y_redoes_it",
        "undo_then_panel_edit_applies",
    )
    # the idle between Ctrl+Z and Ctrl+Y (KEMBED_UNDO_IDLE_S, on by default): a DRC ran in KiCad, and the schematic kept the pane
    if "undo_idle_stays_on_schematic" in results:
        required += ("undo_idle_ran_a_drc_in_kicad", "undo_idle_stays_on_schematic")
    failed = [name for name in required if name not in skip and results.get(name, {}).get("ok") != 1]
    for name in failed:
        print(f"FAIL {name}: expected ok=1, got {results.get(name, {}).get('ok')!r}", flush=True)
    return int(bool(failed))


if __name__ == "__main__":
    with open(sys.argv[1]) as source:
        sys.exit(check_undo(json.load(source)["results"]))
