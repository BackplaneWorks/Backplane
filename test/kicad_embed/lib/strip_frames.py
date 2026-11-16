#!/usr/bin/env python3
"""strip_frames.py CLIENT_LOG FROM_MARK TO_MARK PANE_X PANE_W [require_chip]: the strip as each painted frame drew it between two client
marks (BACKPLANE_GPUI_RECTS probes): in every frame with KiCad's job bar (kicad:progress), the bar and the checks chip (when shown on the
strip's line) lie whole between the tabs and the buttons, and the buttons are inside the pane. Prints "<ok> <frames with the bar> <frames
with the bar and the chip> <detail>"; ok also needs a frame with both when require_chip is 1."""
import json, sys
log, a, b, px, pw = sys.argv[1], sys.argv[2], sys.argv[3], float(sys.argv[4]), float(sys.argv[5])
need_chip = len(sys.argv) > 6 and sys.argv[6] == "1"
on, frames = False, []
for l in open(log, errors="ignore"):
    if f'"name":"{a}","event":"mark"' in l: on = True
    if f'"name":"{b}","event":"mark"' in l: break
    if on and '"event":"probes"' in l: frames.append({x["id"]: x for x in json.loads(l[l.index("{"):])["rects"]})
bad, with_bar, with_both = [], 0, 0
for r in frames:
    bar, chip = r.get("kicad:progress"), r.get("kicad:checks-chip")
    btn = [r.get(k) for k in ("kicad:full", "kicad:screen", "kicad:popout")]
    if not bar or not all(btn): continue
    with_bar += 1
    row = btn[0]["y"]; first = min(x["x"] for x in btn); tabs_end = max((x["x"] + x["w"] for k, x in r.items() if k.startswith("kicad-tab:")), default=px)
    on_line = lambda x: x is not None and abs(x["y"] - row) < 14
    chip_shown = on_line(chip); with_both += bar is not None and on_line(bar) and chip_shown
    for name, x in (("bar", bar), ("chip", chip)):
        if on_line(x) and not (x["x"] >= tabs_end - 0.5 and x["x"] + x["w"] <= first + 0.5): bad.append(f"{name} {x['x']:.0f}+{x['w']:.0f} outside {tabs_end:.0f}..{first:.0f}")
    if not all(x["x"] >= px - 1 and x["x"] + x["w"] <= px + pw + 1 for x in btn): bad.append("a button outside the pane")
ok = with_bar > 0 and not bad and (with_both > 0 or not need_chip)
print(int(ok), with_bar, with_both, json.dumps(bad[:3]))
