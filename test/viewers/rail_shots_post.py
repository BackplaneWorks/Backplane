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
    """Parse GPUI_METRIC lines from client.log into per-shot state, presses, and layout snapshots."""
    layout: dict = {}
    controls: dict[str, dict] = {}
    current_mark = ""
    presses_by_mark: dict[str, list[dict]] = {}
    shots: dict[str, dict] = {}
    refused_shots: list[dict] = []

    if not log_path.exists():
        return {"shots": shots, "presses": presses_by_mark, "refused_shots": refused_shots}

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

    return {"shots": shots, "presses": presses_by_mark, "refused_shots": refused_shots}


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
    full_parsed: dict,
    full_caps: dict[str, dict],
    full_client_ok: bool,
) -> list[dict]:
    shots = full_parsed.get("shots", {})
    presses = full_parsed.get("presses", {})

    def shot_ok(name: str) -> tuple[bool, bool, list[dict]]:
        cap = full_caps.get(name)
        if not cap or not cap.get("ok") or cap.get("bytes", 0) <= 0:
            return False, False, []
        return True, bool(cap.get("obstructed")), cap.get("obstructions") or []

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
        and len(p1) <= 1
    ):
        o1 = "passed"
    else:
        o1 = "task_failed"
    tasks.append(make_task(
        1, "Open the board", 1, p1, o1,
        {"state": s1.get("state"), "open": v1.get("open"), "plot_key": v1.get("plot_key"), "plot_none": v1.get("plot_none"), "capture_ok": ok1, "obstructed": obs1, "obstructions": oblist1},
    ))

    # Task 2: Open the 3D view of the board (budget 1)
    s2 = shots.get("3d", {})
    v2 = s2.get("viewer") or {}
    ok2, obs2, oblist2 = shot_ok("3d")
    p2 = [p["id"] for p in presses.get("press:3d", [])]
    if not full_client_ok or not ok2 or obs2:
        o2 = "harness_failed"
    elif (
        s2.get("state") == "ready"
        and v2.get("open") == "3d"
        and str(v2.get("key", "")).startswith("2|")
        and v2.get("solid_key") == v2.get("key")
        and int(v2.get("solid_painted") or 0) > 0
        and len(p2) <= 1
    ):
        o2 = "passed"
    else:
        o2 = "task_failed"
    tasks.append(make_task(
        2, "Open the 3D view of the board", 1, p2, o2,
        {"state": s2.get("state"), "open": v2.get("open"), "key": v2.get("key"), "solid_key": v2.get("solid_key"), "solid_painted": v2.get("solid_painted"), "capture_ok": ok2, "obstructed": obs2, "obstructions": oblist2},
    ))

    # Task 3: Get back to the schematic from every page (budget 1 per transition)
    t3_results: dict[str, dict] = {}
    t3_harness_bad = not full_client_ok
    t3_all_ok = True
    for src_slug, _, _ in M.TASK3_SOURCES:
        sname = f"task3-from-{src_slug}"
        sr = shots.get(sname, {})
        vr = sr.get("viewer") or {}
        okr, obsr, _ = shot_ok(sname)
        pr = [p["id"] for p in presses.get(f"press:{sname}", [])]
        if not okr or obsr:
            t3_harness_bad = True
            res = "harness_failed"
        elif (
            sr.get("state") == "ready"
            and vr.get("open") == "schematic"
            and str(vr.get("plot_key", "")).startswith("1|")
            and vr.get("plot_none", "") == ""
            and len(pr) == 1
        ):
            res = "passed"
        else:
            t3_all_ok = False
            res = "task_failed"
        t3_results[src_slug] = {"outcome": res, "presses": pr, "open": vr.get("open"), "plot_key": vr.get("plot_key"), "state": sr.get("state")}
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
        f"'vs board' sets gerbersVsBoard={v4.get('gerbers_vs_board')} on a single overlay pane (key={v4.get('plot_key', '')})"
    )
    tasks.append(make_task(
        4, "Compare Gerbers to the board side by side", 2, p4, o4,
        {"state": s4.get("state"), "open": v4.get("open"), "gerbers": v4.get("gerbers"), "gerbers_vs_board": v4.get("gerbers_vs_board"), "panes_count": v4.get("panes_count"), "plot_key": v4.get("plot_key"), "capture_ok": ok4, "obstructed": obs4, "obstructions": oblist4},
        note=note4,
    ))

    # Task 5: See schematic and board together and cross-probe a part (budget 2)
    s5 = shots.get("task5-split-probe", {})
    v5 = s5.get("viewer") or {}
    ok5, obs5, oblist5 = shot_ok("task5-split-probe")
    p5 = [p["id"] for p in presses.get("press:split", [])] + ["tap:slot2"]
    picked_one = bool(v5.get("split_picked_pcb")) or bool(v5.get("split_picked_sch"))
    lit_both = bool(v5.get("split_lit_sch")) and bool(v5.get("split_lit_pcb"))
    if not full_client_ok or not ok5 or obs5:
        o5 = "harness_failed"
    elif (
        s5.get("state") == "ready"
        and v5.get("open") == "split"
        and int(v5.get("panes_count") or 0) >= 2
        and picked_one
        and (lit_both or bool(v5.get("probe")))
        and len(p5) <= 2
    ):
        o5 = "passed"
    else:
        o5 = "task_failed"
    tasks.append(make_task(
        5, "See schematic and board together and cross-probe a part", 2, p5, o5,
        {"state": s5.get("state"), "open": v5.get("open"), "panes_count": v5.get("panes_count"), "probe": v5.get("probe"), "split_picked_sch": v5.get("split_picked_sch"), "split_picked_pcb": v5.get("split_picked_pcb"), "split_lit_sch": v5.get("split_lit_sch"), "split_lit_pcb": v5.get("split_lit_pcb"), "capture_ok": ok5, "obstructed": obs5, "obstructions": oblist5},
    ))

    # Task 6: Open a mech part (budget 2)
    s6 = shots.get("mech", {})
    v6 = s6.get("viewer") or {}
    ok6, obs6, oblist6 = shot_ok("mech")
    p6 = [p["id"] for p in presses.get("press:mech", [])]
    if not full_client_ok or not ok6 or obs6:
        o6 = "harness_failed"
    elif (
        s6.get("state") == "ready"
        and v6.get("open") == "mech"
        and str(v6.get("key", "")).endswith("mech/bracket.step")
        and v6.get("solid_key") == v6.get("key")
        and int(v6.get("solid_painted") or 0) > 0
        and len(p6) <= 2
    ):
        o6 = "passed"
    else:
        o6 = "task_failed"
    tasks.append(make_task(
        6, "Open a mech part", 2, p6, o6,
        {"state": s6.get("state"), "open": v6.get("open"), "key": v6.get("key"), "solid_key": v6.get("solid_key"), "solid_painted": v6.get("solid_painted"), "capture_ok": ok6, "obstructed": obs6, "obstructions": oblist6},
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
    scorecard_tasks = evaluate_scorecard(parsed.get("full", {}), captures_by_kind.get("full", {}), full_client_ok) if "full" in kinds else []

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
