#!/usr/bin/env python3
"""Post-process rail-shots.sh captures into contact sheets, headers.png, rails.png, and steps.json."""
from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

PAGES = [
    ("idle", None, ""),
    ("board", "rail:Board", "board"),
    ("schematic", "rail:Schematic", "schematic"),
    ("3d", "rail:3D", "3d"),
    ("gerbers", "rail:Gerbers", "board"),
    ("split", "rail:Split", "split"),
    ("mech", "rail:Mech", "mech"),
    ("split-after-mech", "rail:Split", "split"),
    ("files", "rail:Files", "files"),
    ("diff", "rail:Diff", "diff"),
    ("review", "rail:Review", "review"),
    ("browser", "rail:Browser", "browser"),
]

# Expected hub `open` (or active rail kind) per project kind.
# On projects lacking a board/schematic/gerbers, the hub may refuse or show empty;
# we record what is expected when the resource exists vs what actually happened.
HAS_RESOURCE = {
    "full": {"board", "schematic", "3d", "gerbers", "split", "mech", "split-after-mech", "files", "diff", "review", "browser"},
    "schematic-only": {"schematic", "files", "diff", "review", "browser"},
    "mech-only": {"mech", "files", "diff", "review", "browser"},
    "no-design": {"files", "diff", "review", "browser"},
}


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


def parse_client_log(log_path: Path) -> dict:
    """Parse GPUI_METRIC lines from a client.log into per-page timeline data."""
    layout = {}
    controls = {}
    shown = {"open": "", "key": ""}
    current_mark = ""
    by_page: dict[str, dict] = {}
    close_info = {"pressed": False, "found": False, "pane": "", "shown_open": ""}
    all_controls_seen: dict[str, set[str]] = {}

    if not log_path.exists():
        return {"pages": by_page, "close": close_info, "controls_seen": {}}

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
        elif kind == "shown":
            shown = {
                "open": ev.get("open") or "",
                "key": ev.get("key") or "",
            }
        elif kind == "mark":
            current_mark = ev.get("name") or ""
            if ":" in current_mark:
                phase, page = current_mark.split(":", 1)
                if page == "close":
                    continue
                rec = by_page.setdefault(page, {
                    "presses": [],
                    "control_found": None,
                    "loading_pane": "",
                    "settled_pane": "",
                    "shown_open": "",
                    "shown_key": "",
                    "layout": {},
                    "controls_ids": [],
                    "shot_ok": None,
                    "loading_shot_ok": None,
                })
                if phase == "settled":
                    rec["layout"] = dict(layout)
                    rec["controls_ids"] = sorted(controls.keys())
                    all_controls_seen[page] = set(controls.keys())
        elif kind == "press":
            cid = ev.get("id") or ""
            found = bool(ev.get("found"))
            if current_mark == "press:close":
                close_info["pressed"] = True
                close_info["found"] = found
            elif current_mark.startswith("press:"):
                page = current_mark.split(":", 1)[1]
                rec = by_page.setdefault(page, {})
                rec.setdefault("presses", []).append(cid)
                rec["control_found"] = found
        elif kind == "read" and ev.get("what") == "pane":
            txt = ev.get("text") or ""
            if current_mark == "settled:close":
                close_info["pane"] = txt
                close_info["shown_open"] = shown.get("open", "")
            elif ":" in current_mark:
                phase, page = current_mark.split(":", 1)
                rec = by_page.setdefault(page, {})
                if phase == "loading":
                    rec["loading_pane"] = txt
                elif phase in ("settled", "page"):
                    rec["settled_pane"] = txt
                    rec["shown_open"] = shown.get("open", "")
                    rec["shown_key"] = shown.get("key", "")
                    rec["layout"] = dict(layout)
                    rec["controls_ids"] = sorted(controls.keys())
        elif kind == "shot":
            ok = bool(ev.get("ok"))
            if ":" in current_mark:
                phase, page = current_mark.split(":", 1)
                rec = by_page.setdefault(page, {})
                if phase == "loading":
                    rec["loading_shot_ok"] = ok
                else:
                    rec["shot_ok"] = ok
                    rec["layout"] = dict(layout)

    return {
        "pages": by_page,
        "close": close_info,
        "controls_seen": {k: sorted(v) for k, v in all_controls_seen.items()},
    }


def make_contact_sheet(kind_dir: Path, out_png: Path) -> bool:
    """Build a contact sheet for one project kind using ImageMagick montage."""
    ordered: list[Path] = []
    for page, _, _ in PAGES:
        if page != "idle":
            lp = kind_dir / f"{page}-loading.png"
            if lp.exists():
                ordered.append(lp)
        sp = kind_dir / f"{page}.png"
        if sp.exists():
            ordered.append(sp)
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
    """Build OUTDIR/headers.png (stacked pane headers) and OUTDIR/rails.png (side-by-side rails)."""
    font = load_font(12)
    small_font = load_font(10)

    header_rows: list[tuple[str, Image.Image | None, str]] = []
    rail_cols: list[tuple[str, Image.Image]] = []

    HEADER_H = 68

    for kind in kinds:
        kind_dir = outdir / kind
        pages_meta = parsed.get(kind, {}).get("pages", {})
        for page, _, _ in PAGES:
            img_path = kind_dir / f"{page}.png"
            if not img_path.exists():
                continue
            with Image.open(img_path) as im:
                im = im.convert("RGB")
                iw, ih = im.size
                lay = pages_meta.get(page, {}).get("layout") or {}
                topbar = lay.get("topbar") or {"x": 0, "y": 0, "w": iw, "h": 36}
                rail = lay.get("rail") or {"x": iw - 72, "y": 36, "w": 72, "h": ih - 36}
                main = lay.get("main") or {"x": 240, "y": 36, "w": iw - 240 - 72, "h": ih - 36}

                # Rail crop (always present)
                rc = crop_box(im, rail["x"], rail["y"], rail["w"], rail["h"])
                short_kind = {"full": "full", "schematic-only": "sch-only", "mech-only": "mech-only", "no-design": "no-design"}.get(kind, kind)
                short_page = {"split-after-mech": "split2", "schematic": "schem"}.get(page, page)
                rail_cols.append((f"{short_kind}\n{short_page}", rc))

                # Pane header crop (for non-idle pages)
                if page == "idle":
                    continue
                pane_x = main["x"] + main["w"]
                pane_w = rail["x"] - pane_x
                pane_y = topbar["y"] + topbar["h"]
                if pane_w >= 40:
                    hc = crop_box(im, pane_x, pane_y, pane_w, HEADER_H)
                    header_rows.append((f"{kind} / {page}", hc, ""))
                else:
                    pane_txt = pages_meta.get(page, {}).get("settled_pane", "")
                    err = pane_txt.split("|")[3] if pane_txt.count("|") >= 3 else ""
                    note = f"(viewer shut){': ' + err if err else ''}"
                    header_rows.append((f"{kind} / {page}", None, note))

    # Assemble OUTDIR/headers.png
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

    # Assemble OUTDIR/rails.png
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


def split_pane_read(raw: str) -> dict:
    parts = raw.split("|") if raw else []
    return {
        "page": parts[0] if len(parts) > 0 else "",
        "loading_line": parts[1] if len(parts) > 1 else "",
        "active_kind": parts[2] if len(parts) > 2 else "",
        "error": parts[3] if len(parts) > 3 else "",
    }


def build_steps_json(outdir: Path, kinds: list[str], parsed: dict[str, dict]) -> None:
    kinds_out: dict[str, dict] = {}
    for kind in kinds:
        pages_meta = parsed.get(kind, {}).get("pages", {})
        kind_pages: dict[str, dict] = {}
        avail = HAS_RESOURCE.get(kind, set())
        for page, rail_id, expected_open in PAGES:
            rec = pages_meta.get(page, {})
            lp = split_pane_read(rec.get("loading_pane", ""))
            sp = split_pane_read(rec.get("settled_pane", ""))
            shown_open = rec.get("shown_open", "")
            # Diff is carried in thread.diff (not thread.viewer.open), so active_kind / pane page shows diff
            if page == "diff":
                matched = (sp["page"] == "diff" or "diff" in sp["active_kind"])
            elif page == "gerbers":
                matched = (shown_open == "board" and sp["active_kind"] == "gerbers")
            elif page == "idle":
                matched = (shown_open == "" and sp["page"] == "")
            else:
                matched = (shown_open == expected_open)
            kind_pages[page] = {
                "presses": rec.get("presses", [] if rail_id is None else [rail_id]),
                "control_found": rec.get("control_found"),
                "resource_present": (page in avail) if page != "idle" else True,
                "expected_open": expected_open,
                "actual_open": shown_open,
                "actual_active_kind": sp["active_kind"],
                "shown_key": rec.get("shown_key", ""),
                "matched_expected": matched,
                "loading_300ms": lp,
                "settled": sp,
                "shot": str(outdir / kind / f"{page}.png"),
                "loading_shot": str(outdir / kind / f"{page}-loading.png") if page != "idle" else None,
            }
        kinds_out[kind] = {
            "pages": kind_pages,
            "close": parsed.get(kind, {}).get("close", {}),
        }

    # Evaluate the 10 scorecard tasks from issue #1317 on the `full` project kind.
    full = kinds_out.get("full", {})
    fp = full.get("pages", {})
    full_controls = parsed.get("full", {}).get("controls_seen", {})

    def task_entry(tid: int, name: str, target_steps: int, presses: list[str], expected: str, actual: str, passed: bool, note: str = "") -> dict:
        return {
            "task": tid,
            "name": name,
            "target_steps": target_steps,
            "presses": presses,
            "expected_open": expected,
            "actual_open": actual,
            "passed": passed,
            "note": note,
        }

    close_rec = full.get("close", {})
    close_pane = split_pane_read(close_rec.get("pane", ""))
    board_ctls = set(full_controls.get("board", []))
    gerbers_ctls = set(full_controls.get("gerbers", []))

    scorecard_tasks = [
        task_entry(
            1, "Open the board", 1,
            fp.get("board", {}).get("presses", ["rail:Board"]),
            "board", fp.get("board", {}).get("actual_open", ""),
            bool(fp.get("board", {}).get("matched_expected")),
        ),
        task_entry(
            2, "Open the 3D view of the board", 1,
            fp.get("3d", {}).get("presses", ["rail:3D"]),
            "3d", fp.get("3d", {}).get("actual_open", ""),
            bool(fp.get("3d", {}).get("matched_expected")),
        ),
        task_entry(
            3, "Get back to the schematic from any page", 1,
            fp.get("schematic", {}).get("presses", ["rail:Schematic"]),
            "schematic", fp.get("schematic", {}).get("actual_open", ""),
            bool(fp.get("schematic", {}).get("matched_expected")),
        ),
        task_entry(
            4, "Compare Gerbers to the board side by side", 2,
            fp.get("gerbers", {}).get("presses", ["rail:Gerbers"]),
            "gerbers", fp.get("gerbers", {}).get("actual_active_kind", ""),
            bool(fp.get("gerbers", {}).get("matched_expected")),
            note="gerbers opens overlay ('vs board' toggle); no side-by-side split yet" if "ui:vs board" not in gerbers_ctls else "",
        ),
        task_entry(
            5, "See schematic and board together and cross-probe a part", 2,
            fp.get("split", {}).get("presses", ["rail:Split"]),
            "split", fp.get("split", {}).get("actual_open", ""),
            bool(fp.get("split", {}).get("matched_expected")),
        ),
        task_entry(
            6, "Open a mech part", 2,
            fp.get("mech", {}).get("presses", ["rail:Mech"]),
            "mech", fp.get("mech", {}).get("actual_open", ""),
            bool(fp.get("mech", {}).get("matched_expected")),
        ),
        task_entry(
            7, "Run a simulation of the schematic", 2,
            ["rail:Simulation"],
            "simulation", "present" if "rail:Simulation" in board_ctls else "absent",
            "rail:Simulation" in board_ctls,
            note="rail:Simulation requires a KiCad runtime with sim-frame capability",
        ),
        task_entry(
            8, "Move between Backplane's viewer and embedded KiCad editor", 1,
            ["ui:Edit"],
            "kicad-edit", "present" if "ui:Edit" in board_ctls else "absent",
            "ui:Edit" in board_ctls,
            note="checks ui:Edit availability on Board page",
        ),
        task_entry(
            9, "Turn Follow agent on and see that it is on", 1,
            ["follow-agent"],
            "follow", "present" if "follow-agent" in board_ctls else "not_registered_as_ctl",
            "follow-agent" in board_ctls,
            note="follow-agent renders when thread.viewer.follow is present",
        ),
        task_entry(
            10, "Close the viewer and get the thread's full width back", 1,
            ["rail:Browser"],
            "", close_rec.get("shown_open", ""),
            bool(close_rec.get("found")) and close_rec.get("shown_open", "") == "" and close_pane["active_kind"] == "",
        ),
    ]

    summary = {
        "kinds": kinds_out,
        "scorecard_tasks": scorecard_tasks,
    }
    (outdir / "steps.json").write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")


def main() -> int:
    if len(sys.argv) < 3:
        print("usage: rail_shots_post.py OUTDIR KIND [KIND ...]", file=sys.stderr)
        return 2
    outdir = Path(sys.argv[1])
    kinds = sys.argv[2:]
    parsed = {k: parse_client_log(outdir / k / "client.log") for k in kinds}
    for k in kinds:
        make_contact_sheet(outdir / k, outdir / f"contact-{k}.png")
    build_strips(outdir, kinds, parsed)
    build_steps_json(outdir, kinds, parsed)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
