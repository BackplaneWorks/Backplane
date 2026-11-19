#!/usr/bin/env python3
"""Post-process rail-shots.sh captures into contact sheets, headers.png, rails.png, and steps.json."""
from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

sys.path.insert(0, str(Path(__file__).resolve().parent))
import rail_shots_manifest as M  # noqa: E402


def load_font(size: int = 12):
    for p in (
        "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    ):
        if os.path.exists(p):
            try:
                return ImageFont.truetype(p, size)
            except OSError:
                pass
    return ImageFont.load_default()


def parse_captures(kind_dir: Path) -> dict[str, dict]:
    out: dict[str, dict] = {}
    p = kind_dir / "captures.jsonl"
    if not p.exists():
        return out
    for line in p.read_text(encoding="utf-8", errors="replace").splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            rec = json.loads(line)
        except Exception:
            continue
        name = rec.get("name")
        if name:
            out[name] = rec
    return out


def parse_client_log(log_path: Path) -> dict:
    """Parse GPUI_METRIC lines from client.log into per-shot state, settled tags, presses, and layout snapshots."""
    layout: dict = {}
    controls: dict[str, dict] = {}
    current_mark = ""
    presses_by_mark: dict[str, list[dict]] = {}
    shots: dict[str, dict] = {}
    settled_tags: dict[str, dict] = {}
    refused_shots: list[dict] = []

    if not log_path.exists():
        return {"shots": shots, "settled_tags": settled_tags, "presses": presses_by_mark, "refused_shots": refused_shots}

    for line in log_path.read_text(encoding="utf-8", errors="replace").splitlines():
        if "GPUI_METRIC " not in line:
            continue
        try:
            ev = json.loads(line.split("GPUI_METRIC ", 1)[1])
        except Exception:
            continue
        kind = ev.get("event")
        if kind == "layout" and isinstance(ev.get("rects"), dict):
            layout = ev["rects"]
        elif kind == "controls" and isinstance(ev.get("controls"), list):
            controls = {c.get("id", ""): c for c in ev["controls"] if isinstance(c, dict)}
        elif kind == "mark":
            current_mark = ev.get("name") or ""
        elif kind == "press":
            presses_by_mark.setdefault(current_mark, []).append({
                "id": ev.get("id") or "",
                "found": bool(ev.get("found")),
                "action": ev.get("action"),
            })
        elif kind == "shot":
            if not ev.get("ok"):
                refused_shots.append(ev)
        elif kind == "settled":
            tag = ev.get("tag") or ""
            if tag:
                settled_tags[tag] = {
                    "tag": tag,
                    "kind": ev.get("kind", ""),
                    "state": ev.get("state", ""),
                    "settled": bool(ev.get("settled")),
                    "viewer": ev.get("viewer") or {},
                }
        elif kind == "shot-ready":
            name = ev.get("name") or ""
            if name:
                shots[name] = {
                    "name": name,
                    "state": ev.get("state", ""),
                    "settled": bool(ev.get("settled")),
                    "viewer": ev.get("viewer") or {},
                    "layout": dict(layout),
                    "controls_ids": sorted(controls.keys()),
                }

    return {"shots": shots, "settled_tags": settled_tags, "presses": presses_by_mark, "refused_shots": refused_shots}


def make_contact_sheet(kind_dir: Path, out_png: Path) -> bool:
    ordered = [kind_dir / f"{s}.png" for s in M.contact_shots() if (kind_dir / f"{s}.png").exists()]
    if not ordered:
        return False
    cmd = [
        "montage",
        "-background", "#181a1f",
        "-fill", "#e6e8ec",
        "-pointsize", "13",
        "-label", "%t",
        "-geometry", "360x225+4+4",
        "-tile", "4x",
        *[str(p) for p in ordered],
        str(out_png),
    ]
    subprocess.run(cmd, check=True)
    return True


def crop_box(img: Image.Image, x: float, y: float, w: float, h: float) -> Image.Image:
    iw, ih = img.size
    x0 = max(0, min(iw - 1, int(round(x))))
    y0 = max(0, min(ih - 1, int(round(y))))
    x1 = max(x0 + 1, min(iw, int(round(x + w))))
    y1 = max(y0 + 1, min(ih, int(round(y + h))))
    return img.crop((x0, y0, x1, y1))


def crop_pane_body(img: Image.Image, lay: dict) -> Image.Image | None:
    """Crop the viewer pane body (below header/floating tools and above bottom strip)."""
    iw, ih = img.size
    topbar = lay.get("topbar") or {"x": 0, "y": 0, "w": iw, "h": 36}
    rail = lay.get("rail") or {"x": iw - 72, "y": 36, "w": 72, "h": ih - 36}
    main = lay.get("main") or {"x": 240, "y": 36, "w": iw - 240 - 72, "h": ih - 36}
    pane_x = main["x"] + main["w"] + 4
    pane_w = rail["x"] - pane_x - 4
    pane_y = topbar["y"] + topbar["h"] + 76
    pane_h = ih - pane_y - 52
    if pane_w < 40 or pane_h < 40:
        return None
    return crop_box(img, pane_x, pane_y, pane_w, pane_h)


def _flat_pixels(im: Image.Image) -> list:
    return list(im.get_flattened_data()) if hasattr(im, "get_flattened_data") else list(im.getdata())


def pane_body_stats(img_path: Path, lay: dict) -> dict:
    """Independent pixel check: verify the pane body crop is not a flat single colour."""
    if not img_path.exists():
        return {"body_w": 0, "body_h": 0, "non_bg_ratio": 0.0, "body_non_flat": False}
    with Image.open(img_path) as im:
        body = crop_pane_body(im.convert("RGB"), lay)
        if body is None:
            return {"body_w": 0, "body_h": 0, "non_bg_ratio": 0.0, "body_non_flat": False}
        # Downsample slightly for speed while preserving all visible detail
        small = body.resize((min(body.width, 240), min(body.height, 240)))
        px = _flat_pixels(small)
        if not px:
            return {"body_w": body.width, "body_h": body.height, "non_bg_ratio": 0.0, "body_non_flat": False}
        bg = px[0]
        diff = sum(1 for r, g, b in px if abs(r - bg[0]) > 12 or abs(g - bg[1]) > 12 or abs(b - bg[2]) > 12)
        ratio = diff / len(px)
        return {
            "body_w": body.width,
            "body_h": body.height,
            "non_bg_ratio": round(ratio, 4),
            "body_non_flat": ratio >= 0.005,
        }


def body_diff_ratio(img_a: Path, lay_a: dict, img_b: Path, lay_b: dict) -> float | None:
    """Independent pixel check: fraction of pane body pixels differing between two shots."""
    if not img_a.exists() or not img_b.exists():
        return None
    with Image.open(img_a) as ia, Image.open(img_b) as ib:
        ba = crop_pane_body(ia.convert("RGB"), lay_a)
        bb = crop_pane_body(ib.convert("RGB"), lay_b)
        if ba is None or bb is None:
            return None
        sa = _flat_pixels(ba.resize((200, 200)))
        sb = _flat_pixels(bb.resize((200, 200)))
        diff = sum(1 for (r1, g1, b1), (r2, g2, b2) in zip(sa, sb) if abs(r1 - r2) > 12 or abs(g1 - g2) > 12 or abs(b1 - b2) > 12)
        return round(diff / len(sa), 4)


def build_strips(outdir: Path, kinds: list[str], parsed: dict[str, dict]) -> None:
    font = load_font(12)
    small_font = load_font(10)
    header_rows: list[tuple[str, Image.Image | None, str]] = []
    rail_cols: list[tuple[str, Image.Image]] = []
    HEADER_H = 68

    for kind in kinds:
        kind_dir = outdir / kind
        shots_meta = parsed.get(kind, {}).get("shots", {})
        for slug, _, _, _ in M.PAGES:
            img_path = kind_dir / f"{slug}.png"
            if not img_path.exists():
                continue
            with Image.open(img_path) as im:
                im = im.convert("RGB")
                iw, ih = im.size
                lay = shots_meta.get(slug, {}).get("layout") or {}
                topbar = lay.get("topbar") or {"x": 0, "y": 0, "w": iw, "h": 36}
                rail = lay.get("rail") or {"x": iw - 72, "y": 36, "w": 72, "h": ih - 36}
                main = lay.get("main") or {"x": 240, "y": 36, "w": iw - 240 - 72, "h": ih - 36}

                rc = crop_box(im, rail["x"], rail["y"], rail["w"], rail["h"])
                short_kind = {"full": "full", "schematic-only": "sch-only", "mech-only": "mech-only", "no-design": "no-design"}.get(kind, kind)
                short_page = {"split-after-mech": "split2", "schematic": "schem"}.get(slug, slug)
                rail_cols.append((f"{short_kind}\n{short_page}", rc))

                if slug == "idle":
                    continue
                pane_x = main["x"] + main["w"]
                pane_w = rail["x"] - pane_x
                pane_y = topbar["y"] + topbar["h"]
                if pane_w >= 40:
                    hc = crop_box(im, pane_x, pane_y, pane_w, HEADER_H)
                    header_rows.append((f"{kind} / {slug}", hc, ""))
                else:
                    err = (shots_meta.get(slug, {}).get("viewer") or {}).get("error", "")
                    note = f"(viewer shut){': ' + err if err else ''}"
                    header_rows.append((f"{kind} / {slug}", None, note))

    if header_rows:
        label_w = 210
        max_hw = max((c.width for _, c, _ in header_rows if c is not None), default=700)
        row_h = HEADER_H + 6
        total_w = label_w + max_hw + 8
        total_h = row_h * len(header_rows) + 4
        canvas = Image.new("RGB", (total_w, total_h), (24, 26, 31))
        draw = ImageDraw.Draw(canvas)
        for idx, (label, crop, note) in enumerate(header_rows):
            y = 2 + idx * row_h
            draw.line([(0, y + row_h - 1), (total_w, y + row_h - 1)], fill=(45, 49, 58))
            draw.text((8, y + 16), label, fill=(220, 224, 232), font=font)
            if crop is not None:
                canvas.paste(crop, (label_w, y + 2))
            else:
                draw.rectangle([(label_w, y + 2), (label_w + max_hw - 1, y + HEADER_H + 1)], fill=(32, 35, 42), outline=(60, 65, 76))
                draw.text((label_w + 10, y + 16), note[:110], fill=(170, 176, 190), font=font)
        canvas.save(outdir / "headers.png")

    if rail_cols:
        col_w = max(c.width for _, c in rail_cols)
        col_h = max(c.height for _, c in rail_cols)
        head_h = 34
        gap = 2
        total_w = len(rail_cols) * (col_w + gap) + gap
        total_h = head_h + col_h + 4
        canvas = Image.new("RGB", (total_w, total_h), (24, 26, 31))
        draw = ImageDraw.Draw(canvas)
        for idx, (label, crop) in enumerate(rail_cols):
            x = gap + idx * (col_w + gap)
            draw.multiline_text((x + 2, 4), label, fill=(220, 224, 232), font=small_font, spacing=2)
            canvas.paste(crop, (x, head_h))
        canvas.save(outdir / "rails.png")


def evaluate_scorecard(
    outdir: Path,
    full_parsed: dict,
    full_caps: dict[str, dict],
    full_client_ok: bool,
    mech_only_parsed: dict | None = None,
) -> list[dict]:
    full_dir = outdir / "full"
    shots = full_parsed.get("shots", {})
    settled_tags = full_parsed.get("settled_tags", {})
    presses = full_parsed.get("presses", {})

    def shot_ok(name: str) -> tuple[bool, bool, list[dict]]:
        cap = full_caps.get(name)
        if not cap or not cap.get("ok") or cap.get("bytes", 0) <= 0:
            return False, False, []
        return True, bool(cap.get("obstructed")), cap.get("obstructions") or []

    def body_check(name: str) -> dict:
        lay = (shots.get(name) or {}).get("layout") or {}
        return pane_body_stats(full_dir / f"{name}.png", lay)

    def make_task(
        tid: int,
        name: str,
        budget: int,
        used_presses: list[str],
        outcome: str,
        evidence: dict,
        note: str = "",
    ) -> dict:
        return {
            "task": tid,
            "name": name,
            "step_budget": budget,
            "presses_used": len(used_presses),
            "within_budget": len(used_presses) <= budget,
            "presses": used_presses,
            "outcome": outcome,
            "evidence": evidence,
            "note": note,
        }

    tasks: list[dict] = []

    # Task 1: Open the board (budget 1)
    s1 = shots.get("board", {})
    v1 = s1.get("viewer") or {}
    b1 = body_check("board")
    ok1, obs1, oblist1 = shot_ok("board")
    p1 = [p["id"] for p in presses.get("press:board", [])]
    if not full_client_ok or not ok1 or obs1:
        o1 = "harness_failed"
    elif (
        s1.get("state") == "ready"
        and v1.get("open") == "board"
        and not v1.get("gerbers")
        and str(v1.get("plot_key", "")).startswith("0|")
        and v1.get("plot_none", "") == ""
        and b1["body_non_flat"]
        and len(p1) <= 1
    ):
        o1 = "passed"
    else:
        o1 = "task_failed"
    tasks.append(make_task(
        1, "Open the board", 1, p1, o1,
        {"state": s1.get("state"), "open": v1.get("open"), "plot_key": v1.get("plot_key"), "plot_none": v1.get("plot_none"), **b1, "capture_ok": ok1, "obstructed": obs1, "obstructions": oblist1},
    ))

    # Task 2: Open the 3D view of the board (budget 1)
    s2 = shots.get("3d", {})
    v2 = s2.get("viewer") or {}
    b2 = body_check("3d")
    ok2, obs2, oblist2 = shot_ok("3d")
    p2 = [p["id"] for p in presses.get("press:3d", [])]
    if not full_client_ok or not ok2 or obs2:
        o2 = "harness_failed"
    elif (
        s2.get("state") == "ready"
        and v2.get("open") == "3d"
        and str(v2.get("key", "")).startswith("2|")
        and v2.get("solid_key") == v2.get("key")
        and int(v2.get("solid_painted_since_press") or 0) > 0
        and int(v2.get("solid_last_w") or 0) > 0
        and int(v2.get("solid_last_h") or 0) > 0
        and b2["body_non_flat"]
        and len(p2) <= 1
    ):
        o2 = "passed"
    else:
        o2 = "task_failed"
    note2 = "" if o2 == "passed" else (f"pane body is flat/blank (non_bg_ratio={b2['non_bg_ratio']})" if not b2["body_non_flat"] else f"state={s2.get('state')} solid_painted_since_press={v2.get('solid_painted_since_press')}")
    tasks.append(make_task(
        2, "Open the 3D view of the board", 1, p2, o2,
        {"state": s2.get("state"), "open": v2.get("open"), "key": v2.get("key"), "solid_key": v2.get("solid_key"), "solid_painted": v2.get("solid_painted"), "solid_painted_since_press": v2.get("solid_painted_since_press"), "solid_last_w": v2.get("solid_last_w"), "solid_last_h": v2.get("solid_last_h"), **b2, "capture_ok": ok2, "obstructed": obs2, "obstructions": oblist2},
        note=note2,
    ))

    # Task 3: Get back to the schematic from every page (budget 1 per transition; verify start page settled first)
    def start_matches(src_slug: str, st: dict) -> bool:
        if not st or not st.get("settled"):
            return False
        sv = st.get("viewer") or {}
        if src_slug == "diff":
            return bool(sv.get("diff_open"))
        if src_slug == "gerbers":
            return sv.get("open") == "board" and bool(sv.get("gerbers"))
        return sv.get("open") == src_slug

    t3_results: dict[str, dict] = {}
    t3_harness_bad = not full_client_ok
    t3_all_ok = True
    for src_slug, _, _ in M.TASK3_SOURCES:
        sname = f"task3-from-{src_slug}"
        start_rec = settled_tags.get(f"task3-start-{src_slug}", {})
        start_ok = start_matches(src_slug, start_rec)
        sr = shots.get(sname, {})
        vr = sr.get("viewer") or {}
        br = body_check(sname)
        okr, obsr, _ = shot_ok(sname)
        pr = [p["id"] for p in presses.get(f"press:{sname}", [])]
        if not okr or obsr:
            t3_harness_bad = True
            res = "harness_failed"
        elif (
            start_ok
            and sr.get("state") == "ready"
            and vr.get("open") == "schematic"
            and str(vr.get("plot_key", "")).startswith("1|")
            and vr.get("plot_none", "") == ""
            and br["body_non_flat"]
            and len(pr) == 1
        ):
            res = "passed"
        else:
            t3_all_ok = False
            res = "task_failed"
        t3_results[src_slug] = {
            "outcome": res,
            "start_ok": start_ok,
            "start_state": start_rec.get("state"),
            "start_open": (start_rec.get("viewer") or {}).get("open"),
            "presses": pr,
            "open": vr.get("open"),
            "plot_key": vr.get("plot_key"),
            "state": sr.get("state"),
            "body_non_flat": br["body_non_flat"],
        }
    o3 = "harness_failed" if t3_harness_bad else ("passed" if t3_all_ok else "task_failed")
    tasks.append(make_task(
        3, "Get back to the schematic from any page", 1, ["rail:Schematic"], o3,
        {"transitions": t3_results},
    ))

    # Task 4: Compare Gerbers to the board side by side (budget 2)
    s4 = shots.get("task4-gerbers-vs-board", {})
    v4 = s4.get("viewer") or {}
    ok4, obs4, oblist4 = shot_ok("task4-gerbers-vs-board")
    p4 = [p["id"] for p in presses.get("press:gerbers", [])] + [p["id"] for p in presses.get("press:task4-gerbers-vs-board", [])]
    if not full_client_ok or not ok4 or obs4:
        o4 = "harness_failed"
    elif (
        s4.get("state") == "ready"
        and int(v4.get("panes_count") or 0) >= 2
        and len(p4) <= 2
    ):
        o4 = "passed"
    else:
        o4 = "task_failed"
    note4 = (
        f"no side-by-side Gerber/board split (panes_count={v4.get('panes_count', 0)}); "
        f"'vs board' sets gerbersVsBoard={v4.get('gerbers_vs_board')} on a single overlay pane"
    )
    tasks.append(make_task(
        4, "Compare Gerbers to the board side by side", 2, p4, o4,
        {"state": s4.get("state"), "open": v4.get("open"), "gerbers": v4.get("gerbers"), "gerbers_vs_board": v4.get("gerbers_vs_board"), "panes_count": v4.get("panes_count"), "plot_key": v4.get("plot_key"), "capture_ok": ok4, "obstructed": obs4, "obstructions": oblist4},
        note=note4,
    ))

    # Task 5: See schematic and board together and cross-probe a known component (R1) (budget 2)
    s5 = shots.get("task5-split-probe", {})
    v5 = s5.get("viewer") or {}
    ok5, obs5, oblist5 = shot_ok("task5-split-probe")
    p5 = [p["id"] for p in presses.get("press:split", [])] + ["tap:R1@slot2"]
    pcb_ref = v5.get("split_pcb_ref") or ""
    sch_ref = v5.get("split_sch_ref") or ""
    if not full_client_ok or not ok5 or obs5:
        o5 = "harness_failed"
    elif (
        s5.get("state") == "ready"
        and v5.get("open") == "split"
        and int(v5.get("panes_count") or 0) >= 2
        and pcb_ref == "R1"
        and sch_ref == "R1"
        and bool(v5.get("split_lit_sch"))
        and bool(v5.get("split_lit_pcb"))
        and len(p5) <= 2
    ):
        o5 = "passed"
    else:
        o5 = "task_failed"
    note5 = "" if o5 == "passed" else f"expected R1 in both panes; got pcb_ref={pcb_ref!r} sch_ref={sch_ref!r} lit_sch={v5.get('split_lit_sch')} lit_pcb={v5.get('split_lit_pcb')} probe={v5.get('probe')!r}"
    tasks.append(make_task(
        5, "See schematic and board together and cross-probe a part", 2, p5, o5,
        {"state": s5.get("state"), "open": v5.get("open"), "panes_count": v5.get("panes_count"), "probe": v5.get("probe"), "split_pcb_ref": pcb_ref, "split_sch_ref": sch_ref, "split_picked_pcb": v5.get("split_picked_pcb"), "split_lit_sch": v5.get("split_lit_sch"), "split_lit_pcb": v5.get("split_lit_pcb"), "capture_ok": ok5, "obstructed": obs5, "obstructions": oblist5},
        note=note5,
    ))

    # Task 6: Open a mech part (budget 2)
    s6 = shots.get("mech", {})
    v6 = s6.get("viewer") or {}
    b6 = body_check("mech")
    diff_vs_3d = body_diff_ratio(full_dir / "mech.png", s6.get("layout") or {}, full_dir / "3d.png", s2.get("layout") or {})
    diff_ok = True if (not b2["body_non_flat"] or diff_vs_3d is None) else (diff_vs_3d >= 0.02)
    # Independent check against mech-only/mech.png (same bracket.step with no prior board 3D): catches stale board model even when 3d.png is blank
    mech_only_lay = ((mech_only_parsed or {}).get("shots", {}).get("mech") or {}).get("layout") or {}
    diff_vs_mech_only = body_diff_ratio(full_dir / "mech.png", s6.get("layout") or {}, outdir / "mech-only" / "mech.png", mech_only_lay)
    matches_mech_ref = True if diff_vs_mech_only is None else (diff_vs_mech_only <= 0.08)
    ok6, obs6, oblist6 = shot_ok("mech")
    p6 = [p["id"] for p in presses.get("press:mech", [])]
    if not full_client_ok or not ok6 or obs6:
        o6 = "harness_failed"
    elif (
        s6.get("state") == "ready"
        and v6.get("open") == "mech"
        and str(v6.get("key", "")).endswith("mech/bracket.step")
        and v6.get("solid_key") == v6.get("key")
        and int(v6.get("solid_painted_since_press") or 0) > 0
        and int(v6.get("solid_last_w") or 0) > 0
        and int(v6.get("solid_last_h") or 0) > 0
        and b6["body_non_flat"]
        and diff_ok
        and matches_mech_ref
        and len(p6) <= 2
    ):
        o6 = "passed"
    else:
        o6 = "task_failed"
    note6 = "" if o6 == "passed" else f"state={s6.get('state')} solid_painted_since_press={v6.get('solid_painted_since_press')} body_non_flat={b6['body_non_flat']} diff_vs_3d={diff_vs_3d} diff_vs_mech_only={diff_vs_mech_only}"
    tasks.append(make_task(
        6, "Open a mech part", 2, p6, o6,
        {"state": s6.get("state"), "open": v6.get("open"), "key": v6.get("key"), "solid_key": v6.get("solid_key"), "solid_painted": v6.get("solid_painted"), "solid_painted_since_press": v6.get("solid_painted_since_press"), "solid_last_w": v6.get("solid_last_w"), "solid_last_h": v6.get("solid_last_h"), **b6, "body_diff_vs_3d": diff_vs_3d, "body_diff_vs_mech_only": diff_vs_mech_only, "capture_ok": ok6, "obstructed": obs6, "obstructions": oblist6},
        note=note6,
    ))

    # Task 7: Run a simulation of the schematic (unavailable in this PR)
    tasks.append(make_task(
        7, "Run a simulation of the schematic", 2, [], "unavailable",
        {"rail_simulation_present": "rail:Simulation" in (s1.get("controls_ids") or [])},
        note="requires a KiCad runtime with sim-frame capability (as test/kicad_embed/embed_sim_e2e.sh uses)",
    ))

    # Task 8: Move between Backplane's viewer and embedded KiCad editor (unavailable in this PR)
    tasks.append(make_task(
        8, "Move between Backplane's viewer and embedded KiCad editor", 1, [], "unavailable",
        {"ui_edit_present": "ui:Edit" in (s1.get("controls_ids") or [])},
        note="requires an embedded KiCad editor session (not launched in this viewer-rail harness)",
    ))

    # Task 9: Turn Follow agent on and see that it is on (budget 1)
    s9 = shots.get("task9-follow-on", {})
    v9 = s9.get("viewer") or {}
    ok9, obs9, oblist9 = shot_ok("task9-follow-on")
    p9_raw = presses.get("press:task9-follow-on", [])
    p9 = [p["id"] for p in p9_raw]
    p9_found = any(p.get("found") for p in p9_raw)
    if not full_client_ok or not ok9 or obs9:
        o9 = "harness_failed"
    elif (
        p9_found
        and v9.get("follow_present") is True
        and v9.get("follow_on") is True
        and v9.get("follow_rect_visible") is True
        and len(p9) <= 1
    ):
        o9 = "passed"
    else:
        o9 = "task_failed"
    tasks.append(make_task(
        9, "Turn Follow agent on and see that it is on", 1, p9, o9,
        {"press_found": p9_found, "follow_present": v9.get("follow_present"), "follow_on": v9.get("follow_on"), "follow_rect_visible": v9.get("follow_rect_visible"), "capture_ok": ok9, "obstructed": obs9, "obstructions": oblist9},
    ))

    # Task 10: Close the viewer and get the thread's full width back (budget 1)
    s_idle = shots.get("idle", {})
    s10 = shots.get("task10-closed", {})
    v10 = s10.get("viewer") or {}
    ok10, obs10, oblist10 = shot_ok("task10-closed")
    p10 = [p["id"] for p in presses.get("press:task10-closed", [])]
    idle_main = (s_idle.get("layout") or {}).get("main") or {}
    closed_main = (s10.get("layout") or {}).get("main") or {}
    closed_rail = (s10.get("layout") or {}).get("rail") or {}
    width_diff = abs(float(closed_main.get("w", -1)) - float(idle_main.get("w", -999))) if idle_main and closed_main else 999.0
    pane_w_closed = float(closed_rail.get("x", 0)) - (float(closed_main.get("x", 0)) + float(closed_main.get("w", 0))) if closed_rail and closed_main else 999.0
    if not full_client_ok or not ok10 or obs10:
        o10 = "harness_failed"
    elif (
        v10.get("open") == ""
        and v10.get("active_kind") == ""
        and not v10.get("diff_open")
        and width_diff <= 2.0
        and pane_w_closed <= 2.0
        and len(p10) <= 1
    ):
        o10 = "passed"
    else:
        o10 = "task_failed"
    tasks.append(make_task(
        10, "Close the viewer and get the thread's full width back", 1, p10, o10,
        {"open": v10.get("open"), "active_kind": v10.get("active_kind"), "idle_main_w": idle_main.get("w"), "closed_main_w": closed_main.get("w"), "width_diff_px": round(width_diff, 2), "pane_w_closed_px": round(pane_w_closed, 2), "capture_ok": ok10, "obstructed": obs10, "obstructions": oblist10},
    ))

    return tasks


def main() -> int:
    if len(sys.argv) < 3:
        print("usage: rail_shots_post.py OUTDIR KIND [KIND ...]", file=sys.stderr)
        return 2
    outdir = Path(sys.argv[1])
    kinds = sys.argv[2:]

    parsed: dict[str, dict] = {}
    captures_by_kind: dict[str, dict[str, dict]] = {}
    kinds_out: dict[str, dict] = {}
    harness_errors: list[str] = []

    for kind in kinds:
        kdir = outdir / kind
        p = parse_client_log(kdir / "client.log")
        caps = parse_captures(kdir)
        parsed[kind] = p
        captures_by_kind[kind] = caps

        exit_file = kdir / "client.exit"
        client_exit = int(exit_file.read_text().strip()) if exit_file.exists() else -1
        if client_exit != 0:
            harness_errors.append(f"{kind}: client exited with {client_exit}")
        if p.get("refused_shots"):
            harness_errors.append(f"{kind}: client refused {len(p['refused_shots'])} shot step(s)")

        expected = M.expected_shots(kind)
        missing: list[str] = []
        failed_caps: list[str] = []
        obstructions_found: dict[str, list[dict]] = {}
        for sname in expected:
            rec = caps.get(sname)
            png = kdir / f"{sname}.png"
            if not rec or not png.exists() or png.stat().st_size == 0:
                missing.append(sname)
            elif not rec.get("ok"):
                failed_caps.append(sname)
            if rec and rec.get("obstructed"):
                obstructions_found[sname] = rec.get("obstructions") or []

        if missing:
            harness_errors.append(f"{kind}: missing captures: {', '.join(missing)}")
        if failed_caps:
            harness_errors.append(f"{kind}: failed captures: {', '.join(failed_caps)}")

        pages_summary: dict[str, dict] = {}
        shots = p.get("shots", {})
        presses = p.get("presses", {})
        avail = M.HAS_RESOURCE.get(kind, set())
        for slug, rail_id, _, expected_open in M.PAGES:
            srec = shots.get(slug, {})
            erec = shots.get(f"{slug}-300ms", {}) if slug != "idle" else {}
            v = srec.get("viewer") or {}
            cap = caps.get(slug, {})
            ecap = caps.get(f"{slug}-300ms", {}) if slug != "idle" else {}
            # Diff is read from thread.diff overlay, never viewer.open
            if slug == "diff":
                actual_page = "diff" if v.get("diff_open") else v.get("open", "")
                matched = bool(v.get("diff_open"))
            elif slug == "gerbers":
                actual_page = "gerbers" if (v.get("open") == "board" and v.get("gerbers")) else v.get("open", "")
                matched = bool(v.get("open") == "board" and v.get("gerbers"))
            elif slug == "idle":
                actual_page = v.get("open", "")
                matched = (v.get("open") == "" and not v.get("diff_open"))
            else:
                actual_page = v.get("open", "")
                matched = (v.get("open") == expected_open)

            pages_summary[slug] = {
                "presses": [x["id"] for x in presses.get(f"press:{slug}", [])] if rail_id else [],
                "resource_present": (slug in avail) if slug != "idle" else True,
                "expected_open": expected_open,
                "actual_page": actual_page,
                "settled_state": srec.get("state", ""),
                "matched_expected": matched,
                "obstructed": bool(cap.get("obstructed")),
                "obstructions": cap.get("obstructions") or [],
                "diff_open": v.get("diff_open") if slug == "diff" else None,
                "diff_files": v.get("diff_files") if slug == "diff" else None,
                "diff_summary": v.get("diff_summary") if slug == "diff" else None,
                "viewer": v,
                "early_300ms_state": erec.get("state", "") if slug != "idle" else None,
                "early_300ms_obstructed": bool(ecap.get("obstructed")) if slug != "idle" else None,
                "shot": str(kdir / f"{slug}.png"),
                "early_shot": str(kdir / f"{slug}-300ms.png") if slug != "idle" else None,
            }

            if slug == "diff":
                lcks = [n for n in (v.get("diff_file_names") or []) if str(n).endswith(".lck")]
                if lcks:
                    harness_errors.append(f"{kind}: diff leaked lock files: {', '.join(lcks)}")

        kinds_out[kind] = {
            "client_exit": client_exit,
            "expected_shots": len(expected),
            "captured_shots": sum(1 for s in expected if caps.get(s, {}).get("ok") and (kdir / f"{s}.png").exists()),
            "missing_shots": missing,
            "failed_captures": failed_caps,
            "obstructions": obstructions_found,
            "pages": pages_summary,
        }
        make_contact_sheet(kdir, outdir / f"contact-{kind}.png")

    build_strips(outdir, kinds, parsed)

    full_client_ok = "full" in kinds_out and kinds_out["full"]["client_exit"] == 0 and not kinds_out["full"]["missing_shots"] and not kinds_out["full"]["failed_captures"]
    scorecard_tasks = evaluate_scorecard(outdir, parsed.get("full", {}), captures_by_kind.get("full", {}), full_client_ok, parsed.get("mech-only")) if "full" in kinds else []

    if harness_errors or any(t["outcome"] == "harness_failed" for t in scorecard_tasks):
        overall = "harness_failed"
    elif any(t["outcome"] == "task_failed" for t in scorecard_tasks):
        overall = "task_failed"
    else:
        overall = "passed"

    summary = {
        "overall_outcome": overall,
        "harness_errors": harness_errors,
        "kinds": kinds_out,
        "scorecard_tasks": scorecard_tasks,
    }
    (outdir / "steps.json").write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")

    if harness_errors:
        for e in harness_errors:
            print(f"rail-shots: HARNESS FAILED: {e}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
