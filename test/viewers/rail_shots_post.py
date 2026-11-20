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


def parse_embed_actions(kind_dir: Path) -> dict[str, dict]:
    out: dict[str, dict] = {}
    p = kind_dir / "embed_actions.jsonl"
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
        act = rec.get("action")
        if act:
            out[act] = rec
    return out


HEADER_STRIP_HEIGHT = 68.0


def _has_positive_rect(ctrl: dict) -> bool:
    r = ctrl.get("rect")
    if not isinstance(r, dict):
        return False
    try:
        return float(r.get("w", 0)) > 0 and float(r.get("h", 0)) > 0
    except (TypeError, ValueError):
        return False


def page_header_rect(layout: dict | None, header_h: float = HEADER_STRIP_HEIGHT) -> dict | None:
    """Compute the viewer pane's header bounding box from client layout rects, or None if geometry is missing."""
    if not isinstance(layout, dict):
        return None
    topbar = layout.get("topbar")
    rail = layout.get("rail")
    main = layout.get("main")
    if not (isinstance(topbar, dict) and isinstance(rail, dict) and isinstance(main, dict)):
        return None
    try:
        pane_x = float(main["x"]) + float(main["w"])
        pane_w = float(rail["x"]) - pane_x
        pane_y = float(topbar["y"]) + float(topbar["h"])
    except (KeyError, TypeError, ValueError):
        return None
    if pane_w <= 0 or header_h <= 0:
        return None
    return {"x": pane_x, "y": pane_y, "w": pane_w, "h": float(header_h)}


def rect_in_bounds(rect: dict | None, bounds: dict | None) -> bool:
    """Return True iff `rect` has positive area and overlaps `bounds` with positive area."""
    if not (isinstance(rect, dict) and isinstance(bounds, dict)):
        return False
    try:
        rx, ry, rw, rh = float(rect["x"]), float(rect["y"]), float(rect["w"]), float(rect["h"])
        bx, by, bw, bh = float(bounds["x"]), float(bounds["y"]), float(bounds["w"]), float(bounds["h"])
    except (KeyError, TypeError, ValueError):
        return False
    if rw <= 0 or rh <= 0 or bw <= 0 or bh <= 0:
        return False
    return min(rx + rw, bx + bw) > max(rx, bx) and min(ry + rh, by + bh) > max(ry, by)


def parse_client_log(log_path: Path) -> dict:
    """Parse GPUI_METRIC lines from client.log into per-shot state, settled tags, presses, keys, reads, probes, kicad reports, rail state/inventory, and layout snapshots."""
    layout: dict = {}
    controls: dict[str, dict] = {}
    probe_rects = {}
    probes: set[str] = set()
    rail_drawn: dict | None = None
    rail_drawn_seq = 0
    rail_metric_seen = False
    rail_events: list[tuple[int, dict]] = []
    rail_inventory: list[dict] | None = None
    current_mark = ""
    mark_seqs: dict[str, int] = {}
    mark_before_rail: dict[str, dict | None] = {}
    presses_by_mark: dict[str, list[dict]] = {}
    keys_by_mark: dict[str, list[dict]] = {}
    palette_reads_by_mark: dict[str, list[dict]] = {}
    rail_reads_by_mark: dict[str, list[dict]] = {}
    shots: dict[str, dict] = {}
    settled_tags: dict[str, dict] = {}
    kicad_tags: dict[str, dict] = {}
    refused_shots: list[dict] = []
    ev_seq = 0

    if not log_path.exists():
        return {
            "shots": shots,
            "settled_tags": settled_tags,
            "kicad_tags": kicad_tags,
            "presses": presses_by_mark,
            "keys": keys_by_mark,
            "palette_reads": palette_reads_by_mark,
            "rail_reads": rail_reads_by_mark,
            "mark_seqs": mark_seqs,
            "mark_before_rail": mark_before_rail,
            "rail_events": rail_events,
            "refused_shots": refused_shots,
            "rail_metric_seen": rail_metric_seen,
            "rail_inventory": rail_inventory,
        }

    for line in log_path.read_text(encoding="utf-8", errors="replace").splitlines():
        if "GPUI_METRIC " not in line:
            continue
        try:
            ev = json.loads(line.split("GPUI_METRIC ", 1)[1])
        except Exception:
            continue
        ev_seq += 1
        kind = ev.get("event")
        if kind == "layout" and isinstance(ev.get("rects"), dict):
            layout = ev["rects"]
        elif kind == "controls" and isinstance(ev.get("controls"), list):
            controls = {c.get("id", ""): c for c in ev["controls"] if isinstance(c, dict)}
        elif kind == "probes":
            pr = ev.get("probes") or ev.get("rects") or []
            probe_rects = pr if isinstance(pr, dict) else {r["id"]: r for r in pr if isinstance(r, dict) and "id" in r}
            if isinstance(pr, dict):
                probes = set(pr.keys())
            elif isinstance(pr, list):
                probes = {x.get("id", "") for x in pr if isinstance(x, dict) and x.get("id")}
        elif kind == "rail" and isinstance(ev.get("rail"), dict):
            rail_drawn = ev["rail"]
            rail_drawn_seq = ev_seq
            rail_metric_seen = True
            rail_events.append((ev_seq, dict(rail_drawn)))
        elif kind == "rail_inventory" and isinstance(ev.get("entries"), list):
            if ev["entries"]:
                rail_inventory = ev["entries"]
        elif kind == "mark":
            current_mark = ev.get("name") or ""
            if current_mark:
                mark_seqs[current_mark] = ev_seq
                mark_before_rail[current_mark] = dict(rail_drawn) if rail_drawn is not None else None
        elif kind == "keys":
            keys_by_mark.setdefault(current_mark, []).append({
                "keys": ev.get("keys") or "",
                "seq": ev_seq,
            })
        elif kind == "read" and ev.get("what") == "palette":
            parsed_pal: dict = {}
            try:
                parsed_pal = json.loads(ev.get("text") or "{}")
            except Exception:
                parsed_pal = {}
            palette_reads_by_mark.setdefault(current_mark, []).append({
                "supported": bool(ev.get("supported")),
                "seq": ev_seq,
                "snapshot": parsed_pal,
            })
        elif kind == "read" and ev.get("what") == "rail":
            rail_reads_by_mark.setdefault(current_mark, []).append({
                "supported": bool(ev.get("supported")),
                "seq": ev_seq,
            })
        elif kind in ("press", "click"):
            if kind == "press":
                delivered = bool(ev.get("found"))
            else:
                # `click` metrics omit `found`; never treat `action: "none"` (unwired Ctl::None) as delivered.
                act = ev.get("action")
                delivered = bool(ev.get("found")) or (
                    not ev.get("error")
                    and bool(act)
                    and act != "none"
                )
            hit_rect = bool(ev.get("hit_rect") or ev.get("rect_hit"))
            presses_by_mark.setdefault(current_mark, []).append({
                "id": ev.get("id") or "",
                "found": delivered,
                "hit_rect": hit_rect,
                "action": ev.get("action"),
                "seq": ev_seq,
            })
        elif kind == "kicad":
            tag = ev.get("tag") or ""
            if tag:
                kicad_tags[tag] = ev
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
                    "seq": ev_seq,
                    "mark": current_mark,
                    "rail_drawn": dict(rail_drawn) if rail_drawn is not None else None,
                    "rail_drawn_seq": rail_drawn_seq,
                }
        elif kind == "shot-ready":
            name = ev.get("name") or ""
            if name:
                hdr_rect = page_header_rect(layout)
                hdr_drawn = (
                    sorted(cid for cid, c in controls.items() if rect_in_bounds(c.get("rect"), hdr_rect))
                    if (hdr_rect is not None and len(controls) > 0)
                    else None
                )
                shots[name] = {
                    "name": name,
                    "state": ev.get("state", ""),
                    "settled": bool(ev.get("settled")),
                    "viewer": ev.get("viewer") or {},
                    "layout": dict(layout),
                    "header_rect": hdr_rect,
                    "controls_captured": len(controls) > 0,
                    "controls_ids": sorted(controls.keys()),
                    "drawn_controls_ids": sorted(cid for cid, c in controls.items() if _has_positive_rect(c)),
                    "header_drawn_controls_ids": hdr_drawn,
                    "probes_ids": sorted(probes),
                    "seq": ev_seq,
                    "rail_drawn": dict(rail_drawn) if rail_drawn is not None else None,
                    "rail_drawn_seq": rail_drawn_seq,
                    "probe_rects": probe_rects,
                    "header_controls": {k: v for k, v in controls.items() if k.startswith("pane-tool:") or k == "ui:vs board"},
                }

    return {
        "shots": shots,
        "settled_tags": settled_tags,
        "kicad_tags": kicad_tags,
        "presses": presses_by_mark,
        "keys": keys_by_mark,
        "palette_reads": palette_reads_by_mark,
        "rail_reads": rail_reads_by_mark,
        "mark_seqs": mark_seqs,
        "mark_before_rail": mark_before_rail,
        "rail_events": rail_events,
        "refused_shots": refused_shots,
        "rail_metric_seen": rail_metric_seen,
        "rail_inventory": rail_inventory,
    }


def _sensitive_tokens() -> list[str]:
    tokens: list[str] = []
    for env_key in ("REAL_HOME", "HOME"):
        val = os.environ.get(env_key, "").strip()
        if val and val.startswith("/home/") and len(val) > len("/home/"):
            tokens.append(val)
    user = os.environ.get("USER", "").strip()
    if user and user not in ("root", "user"):
        tokens.append(f"/home/{user}")
    return sorted(set(tokens))


EMBED_CANVAS_SHOTS = {
    "task8-edit",
    "task7-sim",
    "task7-sim-run",
    *M.EXTRA_EDIT_SHOTS,
    *(s for xs in M.EXTRA_MISSING_SHOTS.values() for s in xs),
}


def shot_privacy_reasons(cap: dict, srec: dict, shot_name: str = "", embed: bool = False) -> list[str]:
    """Return reasons a shot must be marked private: true (fail-closed on missing/failed inspection, dialog, user path, or KiCad canvas)."""
    reasons: list[str] = []
    if cap.get("inspect_ok") is not True:
        err = cap.get("inspect_error") or "inspect_ok missing or false"
        reasons.append(f"obstruction inspection failed or missing (fail-closed: {err})")
    if cap.get("obstructed"):
        titles = [o.get("title") or "/".join(o.get("class") or []) for o in (cap.get("obstructions") or [])]
        reasons.append(f"native dialog/window visible ({', '.join(t for t in titles if t) or 'unknown'})")
    if embed and shot_name in EMBED_CANVAS_SHOTS and os.environ.get("RAIL_EMBED_PATH_FREE") != "1":
        reasons.append("embedded KiCad canvas may display filesystem paths (not scannable from metadata)")
    tokens = _sensitive_tokens()
    if tokens:
        blob = json.dumps({
            "obstructions": cap.get("obstructions") or [],
            "error": (srec.get("viewer") or {}).get("error", ""),
            "diff_file_names": (srec.get("viewer") or {}).get("diff_file_names") or [],
        })
        for tok in tokens:
            if tok in blob:
                reasons.append(f"user home path visible in metadata ({tok})")
                break
    return reasons


def infer_rail_map(srec: dict) -> dict[str, dict]:
    """Build {rail_id_slug: {label, state, why}} from hub viewer.rail, falling back to probes vs controls."""
    out: dict[str, dict] = {}
    v = srec.get("viewer") or {}
    rail_rows = v.get("rail")
    if isinstance(rail_rows, list) and rail_rows:
        for r in rail_rows:
            rid = str(r.get("id") or "")
            if not rid:
                continue
            st = str(r.get("state") or "")
            out[rid] = {
                "label": str(r.get("label") or rid),
                "state": "disabled" if st == "disabled" else "enabled",
                "why": str(r.get("why") or ""),
            }
        return out
    ctls = set(srec.get("controls_ids") or [])
    prbs = set(srec.get("probes_ids") or [])
    for slug, rail_id, settle_kind, _ in M.PAGES:
        if not rail_id or slug == "split-after-mech":
            continue
        rid = settle_kind
        if rail_id in ctls:
            out[rid] = {"label": rail_id.split(":", 1)[1], "state": "enabled", "why": ""}
        elif rail_id in prbs:
            out[rid] = {"label": rail_id.split(":", 1)[1], "state": "disabled", "why": ""}
    return out


def make_contact_sheet(kind_dir: Path, out_png: Path, cleared_shots: set[str]) -> tuple[bool, list[str]]:
    excluded: list[str] = []
    ordered: list[Path] = []
    for s in M.contact_shots():
        p = kind_dir / f"{s}.png"
        if not p.exists():
            continue
        if s not in cleared_shots:
            excluded.append(s)
            continue
        ordered.append(p)
    if not ordered:
        return False, excluded
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
    return True, excluded


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


def build_strips(outdir: Path, kinds: list[str], parsed: dict[str, dict], cleared_by_kind: dict[str, set[str]]) -> None:
    """Build headers.png and rails.png strictly from the explicitly cleared (non-private) shot list."""
    font = load_font(12)
    small_font = load_font(10)
    header_rows: list[tuple[str, Image.Image | None, str]] = []
    rail_cols: list[tuple[str, Image.Image]] = []
    HEADER_H = 68

    for kind in kinds:
        kind_dir = outdir / kind
        shots_meta = parsed.get(kind, {}).get("shots", {})
        cleared = cleared_by_kind.get(kind, set())
        for slug, _, _, _ in M.PAGES:
            if slug not in cleared:
                continue
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
                short_kind = {"full": "full", "board-only": "brd-only", "schematic-only": "sch-only", "mech-only": "mech-only", "no-design": "no-design"}.get(kind, kind)
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


def evaluate_b1_kind(kind: str, parsed_kind: dict, rail_map: dict[str, dict], embed: bool = False) -> dict:
    """Evaluate Rule B1 (viewer rail entries + disabled entries + Terminal, Commands, and Simulation under --embed).

    Requires client-drawn rail lighting via `GPUI_METRIC {"event":"rail", ...}` (PR #1359).
    Disabled entries use `{"read":"rail"}` (`supported: true`) for a fresh post-input rail snapshot;
    `disabled_press` records delivery to the drawn disabled row (or `"unavailable"` if undelivered),
    and `disabled_palette` distinguishes `"target_absent"` from `"another_command_selected_and_run"`.
    """
    omitted = [] if embed else list(M.B1_OMITTED_ENTRIES)
    if not parsed_kind.get("rail_metric_seen"):
        return {
            "status": "unavailable",
            "reason": "client did not emit GPUI_METRIC event=rail (requires PR #1359)",
            "omitted_entries": omitted,
            "disabled_entries_measured": False,
            "entries": {},
        }

    settled_tags = parsed_kind.get("settled_tags") or {}
    presses_by_mark = parsed_kind.get("presses") or {}
    keys_by_mark = parsed_kind.get("keys") or {}
    pal_reads_by_mark = parsed_kind.get("palette_reads") or {}
    rail_reads_by_mark = parsed_kind.get("rail_reads") or {}
    kicad_tags = parsed_kind.get("kicad_tags") or {}
    mark_before_rail = parsed_kind.get("mark_before_rail") or {}
    rail_events: list[tuple[int, dict]] = parsed_kind.get("rail_events") or []

    def viewer_matches(eid: str, viewer: dict) -> bool:
        if eid == "diff":
            return bool(viewer.get("diff_open"))
        if eid == "gerbers":
            return viewer.get("open") == "board" and bool(viewer.get("gerbers"))
        return viewer.get("open") == eid and not viewer.get("diff_open")

    def viewer_closed(viewer: dict) -> bool:
        return viewer.get("open") == "" and not viewer.get("diff_open")

    def rail_lists(rd: dict | None) -> tuple[list[str], list[str], list[str]] | None:
        if not isinstance(rd, dict):
            return None
        lit, entries, dis = rd.get("lit"), rd.get("entries"), rd.get("disabled")
        if not (isinstance(lit, list) and isinstance(entries, list) and isinstance(dis, list)):
            return None
        return [str(x) for x in lit], [str(x) for x in entries], [str(x) for x in dis]

    def fresh_rail_between(after_seq: int, up_to_seq: int) -> dict | None:
        """Return the latest `event == "rail"` strictly after `after_seq` and <= `up_to_seq`, or None."""
        found: dict | None = None
        for seq, rd in rail_events:
            if after_seq < seq <= up_to_seq:
                found = rd
        return found

    def _open_empty_words(eid: str, viewer: dict) -> str:
        if eid == "review":
            return str(viewer.get("empty_text") or "").strip()
        return _empty_words_for_page(eid, viewer)

    def check_transition(
        mark_name: str,
        tag_name: str,
        label: str,
        input_seq: int | None,
        expect_disabled: bool,
        expect_before_lit: list[str],
        expect_after_lit: list[str],
        viewer_pred,
        eid: str = "",
    ) -> bool:
        if input_seq is None:
            return False
        rec = settled_tags.get(tag_name)
        if not rec or rec.get("settled") is not True:
            return False
        v_rec = rec.get("viewer") or {}
        st = rec.get("state")
        if expect_after_lit:
            if st == "empty":
                # Empty state counts as opened ONLY when the client reports non-empty designed empty-state words.
                if not _open_empty_words(eid, v_rec):
                    return False
            elif st != "ready":
                return False
        elif st != "ready":
            return False
        before = rail_lists(mark_before_rail.get(mark_name))
        if before is None:
            return False
        b_lit, b_entries, b_dis = before
        if label not in b_entries or b_lit != expect_before_lit:
            return False
        if expect_disabled and label not in b_dis:
            return False
        if not expect_disabled and label in b_dis:
            return False
        after = rail_lists(fresh_rail_between(input_seq, int(rec.get("seq") or 0)))
        if after is None:
            return False
        a_lit, a_entries, a_dis = after
        if label not in a_entries or a_lit != expect_after_lit:
            return False
        if expect_disabled and label not in a_dis:
            return False
        return bool(viewer_pred(v_rec))

    entries_out: dict[str, dict] = {}
    for eid, label, rail_id, settle_kind, bare_key, _pal_q in M.B1_ENTRIES:
        rentry = rail_map.get(settle_kind)
        if rentry is None:
            continue
        is_disabled = rentry.get("state") == "disabled"
        failed: list[str] = []
        checks: dict[str, bool] = {}

        if not is_disabled:
            # 1. press_open: press delivered with found=True, settled ready (or designed empty with words), fresh rail lit==[label]
            m_po = f"b1:press-open:{eid}"
            p_po = [p for p in presses_by_mark.get(m_po, []) if p.get("id") == rail_id and p.get("found")]
            po_ok = check_transition(m_po, f"b1-press-open-{eid}", label, p_po[0].get("seq") if p_po else None, False, [], [label], lambda v, e=eid: viewer_matches(e, v), eid=eid)
            checks["press_open"] = po_ok
            if not po_ok:
                failed.append("press_open")

            # 2. press_close: press delivered while lit==[label], settled closed, fresh rail lit==[]
            m_pc = f"b1:press-close:{eid}"
            p_pc = [p for p in presses_by_mark.get(m_pc, []) if p.get("id") == rail_id and p.get("found")]
            pc_ok = check_transition(m_pc, f"b1-press-close-{eid}", label, p_pc[0].get("seq") if p_pc else None, False, [label], [], viewer_closed, eid=eid)
            checks["press_close"] = pc_ok
            if not pc_ok:
                failed.append("press_close")

            # 3. key_open: bare key delivered, settled ready (or designed empty with words), fresh rail lit==[label]
            m_ko = f"b1:key-open:{eid}"
            k_ko = [k for k in keys_by_mark.get(m_ko, []) if k.get("keys") == bare_key]
            ko_ok = check_transition(m_ko, f"b1-key-open-{eid}", label, k_ko[0].get("seq") if k_ko else None, False, [], [label], lambda v, e=eid: viewer_matches(e, v), eid=eid)
            checks["key_open"] = ko_ok
            if not ko_ok:
                failed.append("key_open")

            # 4. key_close: bare key delivered while lit==[label], settled closed, fresh rail lit==[]
            m_kc = f"b1:key-close:{eid}"
            k_kc = [k for k in keys_by_mark.get(m_kc, []) if k.get("keys") == bare_key]
            kc_ok = check_transition(m_kc, f"b1-key-close-{eid}", label, k_kc[0].get("seq") if k_kc else None, False, [label], [], viewer_closed, eid=eid)
            checks["key_close"] = kc_ok
            if not kc_ok:
                failed.append("key_close")

            # 5. palette_open: palette read confirms open & selected_id == rail:<eid> before Enter, then Enter opens & lights [label]
            m_pal = f"b1:palette-open:{eid}"
            preads = [r for r in pal_reads_by_mark.get(m_pal, []) if r.get("supported") and (r.get("snapshot") or {}).get("open") is True and (r.get("snapshot") or {}).get("selected_id") == f"rail:{eid}"]
            enters = [k for k in keys_by_mark.get(m_pal, []) if k.get("keys") == "enter" and (not preads or int(k.get("seq") or 0) > int(preads[-1].get("seq") or 0))]
            pal_seq = enters[0].get("seq") if (preads and enters) else None
            pal_ok = check_transition(m_pal, f"b1-palette-open-{eid}", label, pal_seq, False, [], [label], lambda v, e=eid: viewer_matches(e, v), eid=eid)
            checks["palette_open"] = pal_ok
            if not pal_ok:
                failed.append("palette_open")
            entries_out[eid] = {
                "label": label,
                "disabled": False,
                "status": "measured",
                "passed": len(failed) == 0,
                "checks": checks,
                "failed": failed,
            }
        else:
            def check_disabled_noop(mark_name: str, tag_name: str, input_seq: int | None) -> bool | str:
                if input_seq is None:
                    return "unavailable"
                rreads = [r for r in rail_reads_by_mark.get(mark_name, []) if r.get("supported") and int(r.get("seq") or 0) > input_seq]
                if not rreads:
                    return "unavailable"
                rec = settled_tags.get(tag_name)
                if not rec or rec.get("settled") is not True or rec.get("state") != "ready":
                    return False
                before = rail_lists(mark_before_rail.get(mark_name))
                if before is None:
                    return False
                b_lit, b_entries, b_dis = before
                if label not in b_entries or label not in b_dis or b_lit != []:
                    return False
                after = rail_lists(fresh_rail_between(int(rreads[-1].get("seq") or 0), int(rec.get("seq") or 0)))
                if after is None:
                    return "unavailable"
                a_lit, a_entries, a_dis = after
                if label not in a_entries or label not in a_dis or a_lit != []:
                    return False
                return viewer_closed(rec.get("viewer") or {})

            # 1. disabled_press: delivered ONLY when click landed on the drawn row's hit rect (else unavailable)
            m_dp = f"b1:disabled-press:{eid}"
            p_dp = [p for p in presses_by_mark.get(m_dp, []) if p.get("id") == rail_id and p.get("found") and p.get("hit_rect")]
            dp_res: bool | str = check_disabled_noop(m_dp, f"b1-disabled-press-{eid}", p_dp[0].get("seq") if p_dp else None) if p_dp else "unavailable"

            # 2. disabled_key: bare key delivered + fresh read:rail no-op
            m_dk = f"b1:disabled-key:{eid}"
            k_dk = [k for k in keys_by_mark.get(m_dk, []) if k.get("keys") == bare_key]
            dk_res: bool | str = check_disabled_noop(m_dk, f"b1-disabled-key-{eid}", k_dk[0].get("seq") if k_dk else None)

            # 3. disabled_palette: requires positive evidence the whole filtered list omits rail:<eid>
            # (count == 0, or full filtered list in snapshot without target), else unavailable.
            m_dpal = f"b1:disabled-palette:{eid}"
            preads = [r for r in pal_reads_by_mark.get(m_dpal, []) if r.get("supported") and (r.get("snapshot") or {}).get("open") is True]
            pal_status = "unavailable"
            dpal_res: bool | str = "unavailable"
            if preads:
                snap = preads[-1].get("snapshot") or {}
                sel_id = str(snap.get("selected_id") or "")
                cnt = int(snap.get("count") or 0)
                read_seq = int(preads[-1].get("seq") or 0)
                enters = [k for k in keys_by_mark.get(m_dpal, []) if k.get("keys") == "enter" and int(k.get("seq") or 0) > read_seq]
                target_id = f"rail:{eid}"
                raw_list = snap.get("items") if isinstance(snap.get("items"), list) else (snap.get("ids") if isinstance(snap.get("ids"), list) else None)
                list_ids: list[str] | None = None
                if raw_list is not None:
                    list_ids = [
                        str(it.get("id") if isinstance(it, dict) else it)
                        for it in raw_list
                    ]
                if sel_id == target_id or (list_ids is not None and target_id in list_ids):
                    pal_status = "target_present"
                    dpal_res = check_disabled_noop(m_dpal, f"b1-disabled-palette-{eid}", enters[0].get("seq")) if enters else False
                elif cnt == 0 or (list_ids is not None and len(list_ids) >= cnt and not enters):
                    pal_status = "target_absent"
                    dpal_res = check_disabled_noop(m_dpal, f"b1-disabled-palette-{eid}", read_seq)
                elif cnt > 0 and enters:
                    pal_status = "another_command_selected_and_run"
                    dpal_res = "unavailable"
                else:
                    pal_status = "filtered_list_unobserved"
                    dpal_res = "unavailable"

            dis_checks = {
                "disabled_press": dp_res,
                "disabled_key": dk_res,
                "disabled_palette": dpal_res,
                "disabled_palette_status": pal_status,
            }
            applicable_dis = (("disabled_press", dp_res), ("disabled_key", dk_res), ("disabled_palette", dpal_res))
            failed_dis = [k for k, v in applicable_dis if v is False]
            all_dis_measured = all(isinstance(v, bool) for _, v in applicable_dis)
            if failed_dis:
                entries_out[eid] = {
                    "label": label,
                    "disabled": True,
                    "status": "measured",
                    "passed": False,
                    "checks": dis_checks,
                    "failed": failed_dis,
                }
            elif all_dis_measured:
                entries_out[eid] = {
                    "label": label,
                    "disabled": True,
                    "status": "measured",
                    "passed": True,
                    "checks": dis_checks,
                    "failed": [],
                }
            else:
                entries_out[eid] = {
                    "label": label,
                    "disabled": True,
                    "status": "unavailable",
                    "passed": None,
                    "reason": "not all disabled-entry assertions were measured",
                    "checks": dis_checks,
                    "failed": [],
                }

    # Tool entries: Terminal (toggles its panel via rail.lit) and Commands (opens/closes the palette via read:palette)
    def check_terminal_step(mark_name: str, tag_name: str, input_seq: int | None, expect_open: bool) -> bool | str:
        if input_seq is None:
            return "unavailable"
        rreads = [r for r in rail_reads_by_mark.get(mark_name, []) if r.get("supported") and int(r.get("seq") or 0) > input_seq]
        rec = settled_tags.get(tag_name)
        if not rreads or not rec:
            return "unavailable"
        if rec.get("settled") is not True:
            return False
        before = rail_lists(mark_before_rail.get(mark_name))
        if before is None:
            return "unavailable"
        b_lit, b_entries, _ = before
        if "Terminal" not in b_entries:
            return False
        # Require a real change from the opposite state.
        if expect_open and "Terminal" in b_lit:
            return False
        if (not expect_open) and "Terminal" not in b_lit:
            return False
        after = rail_lists(fresh_rail_between(int(rreads[-1].get("seq") or 0), int(rec.get("seq") or 0)))
        if after is None:
            return "unavailable"
        a_lit, a_entries, _ = after
        if "Terminal" not in a_entries:
            return False
        return ("Terminal" in a_lit) if expect_open else ("Terminal" not in a_lit)

    if "b1:terminal-press-open" in presses_by_mark or "b1:terminal-key-open" in keys_by_mark:
        tp_o = [p for p in presses_by_mark.get("b1:terminal-press-open", []) if p.get("id") == "rail:Terminal" and p.get("found")]
        tp_c = [p for p in presses_by_mark.get("b1:terminal-press-close", []) if p.get("id") == "rail:Terminal" and p.get("found")]
        tk_o = [k for k in keys_by_mark.get("b1:terminal-key-open", []) if k.get("keys") == "`"]
        tk_c = [k for k in keys_by_mark.get("b1:terminal-key-close", []) if k.get("keys") in ("shift-escape", "`")]
        t_checks = {
            "press_open": check_terminal_step("b1:terminal-press-open", "b1-terminal-press-open", tp_o[0].get("seq") if tp_o else None, True),
            "press_close": check_terminal_step("b1:terminal-press-close", "b1-terminal-press-close", tp_c[0].get("seq") if tp_c else None, False),
            "key_open": check_terminal_step("b1:terminal-key-open", "b1-terminal-key-open", tk_o[0].get("seq") if tk_o else None, True),
            "key_close": check_terminal_step("b1:terminal-key-close", "b1-terminal-key-close", tk_c[0].get("seq") if tk_c else None, False),
        }
        t_failed = [k for k, v in t_checks.items() if v is False]
        t_all_measured = all(isinstance(v, bool) for v in t_checks.values())
        if t_failed:
            entries_out["terminal"] = {"label": "Terminal", "disabled": False, "status": "measured", "passed": False, "checks": t_checks, "failed": t_failed}
        elif t_all_measured:
            entries_out["terminal"] = {"label": "Terminal", "disabled": False, "status": "measured", "passed": True, "checks": t_checks, "failed": []}
        else:
            entries_out["terminal"] = {"label": "Terminal", "disabled": False, "status": "unavailable", "passed": None, "reason": "not all Terminal transitions were measured", "checks": t_checks, "failed": []}

    if "b1:commands-press-open" in presses_by_mark or "b1:commands-press-open" in pal_reads_by_mark:
        cp_o = [p for p in presses_by_mark.get("b1:commands-press-open", []) if p.get("id") == "rail:Commands" and p.get("found")]
        pr_o = [r for r in pal_reads_by_mark.get("b1:commands-press-open", []) if r.get("supported") and (not cp_o or int(r.get("seq") or 0) > int(cp_o[0].get("seq") or 0))]
        ek_c = [k for k in keys_by_mark.get("b1:commands-escape-close", []) if k.get("keys") == "escape"]
        pr_c = [r for r in pal_reads_by_mark.get("b1:commands-escape-close", []) if r.get("supported") and ek_c and int(r.get("seq") or 0) > int(ek_c[0].get("seq") or 0)]
        c_open: bool | str = ((pr_o[-1].get("snapshot") or {}).get("open") is True) if (cp_o and pr_o) else "unavailable"
        # Close requires a delivered Escape from a confirmed-open palette followed by a post-Escape palette read.
        c_close: bool | str = ((pr_c[-1].get("snapshot") or {}).get("open") is False) if (c_open is True and ek_c and pr_c) else "unavailable"
        c_checks = {"press_open": c_open, "escape_close": c_close}
        c_failed = [k for k, v in c_checks.items() if v is False]
        c_all_measured = all(isinstance(v, bool) for v in c_checks.values())
        if c_failed:
            entries_out["commands"] = {"label": "Commands", "disabled": False, "status": "measured", "passed": False, "checks": c_checks, "failed": c_failed}
        elif c_all_measured:
            entries_out["commands"] = {"label": "Commands", "disabled": False, "status": "measured", "passed": True, "checks": c_checks, "failed": []}
        else:
            entries_out["commands"] = {"label": "Commands", "disabled": False, "status": "unavailable", "passed": None, "reason": "not all Commands transitions were measured", "checks": c_checks, "failed": []}

    # Simulation under --embed (when Simulation is present in this kind's rail):
    # requires BOTH Simulator frame shown AND fresh rail.lit containing Simulation; missing transition is unavailable.
    if embed and "simulation" in rail_map and ("b1:sim-press-open" in presses_by_mark or "b1-sim-open" in kicad_tags):
        sp_o = [p for p in presses_by_mark.get("b1:sim-press-open", []) if p.get("id") == "rail:Simulation" and p.get("found")]
        k_sim = kicad_tags.get("b1-sim-open") or {}
        rec_sim = settled_tags.get("b1-sim-open") or {}
        rreads_sim = [r for r in rail_reads_by_mark.get("b1:sim-press-open", []) if r.get("supported") and (not sp_o or int(r.get("seq") or 0) > int(sp_o[0].get("seq") or 0))]
        after_s = rail_lists(fresh_rail_between(int(rreads_sim[-1].get("seq") or 0), int(rec_sim.get("seq") or 0))) if (rreads_sim and rec_sim) else None
        if not (sp_o and rec_sim and after_s is not None):
            entries_out["simulation"] = {
                "label": "Simulation",
                "disabled": False,
                "status": "unavailable",
                "passed": None,
                "reason": "Simulation press/rail/settled transition not fully observed",
                "checks": {"press_open": "unavailable", "sim_lit": "unavailable", "sim_frame": "unavailable"},
                "failed": [],
            }
        else:
            sim_lit_ok = "Simulation" in after_s[0]
            sim_frame_ok = bool((rec_sim.get("viewer") or {}).get("sim_frame")) or any(
                isinstance(f, dict) and f.get("kind") == "SIMULATOR" for f in (k_sim.get("frames") or [])
            )
            sim_ok = bool(sim_lit_ok and sim_frame_ok)
            sim_failed = []
            if not sim_lit_ok:
                sim_failed.append("sim_lit")
            if not sim_frame_ok:
                sim_failed.append("sim_frame")
            entries_out["simulation"] = {
                "label": "Simulation",
                "disabled": False,
                "status": "measured",
                "passed": sim_ok,
                "checks": {"press_open": sim_ok, "sim_lit": sim_lit_ok, "sim_frame": sim_frame_ok},
                "failed": sim_failed,
            }

    dis_measured = any(e.get("disabled") and e.get("status") == "measured" for e in entries_out.values())
    status = "measured" if (not omitted and dis_measured) else "measured_partial"
    return {
        "status": status,
        "omitted_entries": omitted,
        "disabled_entries_measured": dis_measured,
        "entries": entries_out,
    }


def evaluate_scorecard(
    outdir: Path,
    full_parsed: dict,
    full_caps: dict[str, dict],
    full_client_ok: bool,
    mech_only_parsed: dict | None = None,
    embed: bool = False,
    embed_actions: dict[str, dict] | None = None,
) -> list[dict]:
    full_dir = outdir / "full"
    shots = full_parsed.get("shots", {})
    settled_tags = full_parsed.get("settled_tags", {})
    kicad_tags = full_parsed.get("kicad_tags", {})
    presses = full_parsed.get("presses", {})
    embed_actions = embed_actions or {}

    def shot_ok(name: str) -> tuple[bool, bool, list[dict]]:
        cap = full_caps.get(name)
        if not cap or not cap.get("ok") or cap.get("bytes", 0) <= 0 or cap.get("inspect_ok") is not True:
            return False, False, []
        return True, bool(cap.get("obstructed")), cap.get("obstructions") or []

    def body_check(name: str) -> dict:
        lay = (shots.get(name) or {}).get("layout") or {}
        return pane_body_stats(full_dir / f"{name}.png", lay)

    s_idle = shots.get("idle", {})
    idle_main = (s_idle.get("layout") or {}).get("main") or {}

    def check_closed_to_idle(shot_name: str, press_mark: str, expected_ctl_id: str) -> dict:
        sr = shots.get(shot_name, {})
        vr = sr.get("viewer") or {}
        ok, obs, oblist = shot_ok(shot_name)
        p_raw = presses.get(press_mark, [])
        p_ids = [p["id"] for p in p_raw]
        p_found = any(p.get("found") for p in p_raw)
        c_main = (sr.get("layout") or {}).get("main") or {}
        c_rail = (sr.get("layout") or {}).get("rail") or {}
        w_diff = abs(float(c_main.get("w", -1)) - float(idle_main.get("w", -999))) if idle_main and c_main else 999.0
        pane_w = float(c_rail.get("x", 0)) - (float(c_main.get("x", 0)) + float(c_main.get("w", 0))) if c_rail and c_main else 999.0
        closed_ok = (
            ok
            and not obs
            and p_ids == [expected_ctl_id]
            and p_found
            and vr.get("open") == ""
            and vr.get("active_kind") == ""
            and not vr.get("diff_open")
            and w_diff <= 2.0
            and abs(pane_w) <= 2.0
        )
        return {
            "ok": closed_ok,
            "expected_control": expected_ctl_id,
            "presses": p_ids,
            "press_found": p_found,
            "open": vr.get("open"),
            "active_kind": vr.get("active_kind"),
            "diff_open": bool(vr.get("diff_open")),
            "idle_main_w": idle_main.get("w"),
            "closed_main_w": c_main.get("w"),
            "width_diff_px": round(w_diff, 2),
            "pane_w_closed_px": round(pane_w, 2),
            "capture_ok": ok,
            "obstructed": obs,
            "obstructions": oblist,
        }

    def make_task(
        tid: int,
        name: str,
        budget: int,
        used_presses: list[str],
        outcome: str,
        evidence: dict,
        note: str = "",
        one_step_back: bool | str = "not_measured",
        found: bool | None = None,
    ) -> dict:
        within = len(used_presses) <= budget
        if found is None:
            found = bool(used_presses) if outcome not in ("unavailable", "harness_failed") else False
        parts = {
            "found": bool(found),
            "within_budget": within if outcome not in ("unavailable", "harness_failed") else False,
            "result_evidenced": outcome == "passed",
            "one_step_back": one_step_back,
        }
        return {
            "task": tid,
            "name": name,
            "step_budget": budget,
            "presses_used": len(used_presses),
            "within_budget": within,
            "presses": used_presses,
            "outcome": outcome,
            "parts": parts,
            "evidence": evidence,
            "note": note,
        }

    tasks: list[dict] = []

    # Task 1: Open the board (budget 1) + one step back (press rail:Board shuts viewer)
    s1 = shots.get("board", {})
    v1 = s1.get("viewer") or {}
    b1 = body_check("board")
    ok1, obs1, oblist1 = shot_ok("board")
    p1_raw = presses.get("press:board", [])
    p1 = [p["id"] for p in p1_raw]
    t1_back = check_closed_to_idle("task1-back", "press:task1-back", "rail:Board")
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
        {"state": s1.get("state"), "open": v1.get("open"), "plot_key": v1.get("plot_key"), "plot_none": v1.get("plot_none"), **b1, "capture_ok": ok1, "obstructed": obs1, "obstructions": oblist1, "one_step_back": t1_back},
        one_step_back=(t1_back["ok"] if "task1-back" in shots else "not_measured"),
        found=any(p.get("found") for p in p1_raw),
    ))

    # Task 2: Open the 3D view of the board (budget 1) + one step back (press rail:3D shuts viewer)
    s2 = shots.get("3d", {})
    v2 = s2.get("viewer") or {}
    b2 = body_check("3d")
    ok2, obs2, oblist2 = shot_ok("3d")
    p2_raw = presses.get("press:3d", [])
    p2 = [p["id"] for p in p2_raw]
    t2_back = check_closed_to_idle("task2-back", "press:task2-back", "rail:3D")
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
        {"state": s2.get("state"), "open": v2.get("open"), "key": v2.get("key"), "solid_key": v2.get("solid_key"), "solid_painted": v2.get("solid_painted"), "solid_painted_since_press": v2.get("solid_painted_since_press"), "solid_last_w": v2.get("solid_last_w"), "solid_last_h": v2.get("solid_last_h"), **b2, "capture_ok": ok2, "obstructed": obs2, "obstructions": oblist2, "one_step_back": t2_back},
        note=note2,
        one_step_back=(t2_back["ok"] if "task2-back" in shots else "not_measured"),
        found=any(p.get("found") for p in p2_raw),
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
        one_step_back=(o3 == "passed"),
    ))

    # Task 4: Compare Gerbers to the board side by side (budget 2)
    s4 = shots.get("task4-gerbers-vs-board", {})
    v4 = s4.get("viewer") or {}
    ok4, obs4, oblist4 = shot_ok("task4-gerbers-vs-board")
    p4_a = presses.get("press:gerbers", [])
    p4_b = presses.get("press:task4-gerbers-vs-board", [])
    p4 = [p["id"] for p in p4_a] + [p["id"] for p in p4_b]
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
        found=(any(p.get("found") for p in p4_a) and any(p.get("found") for p in p4_b)),
    ))

    # Task 5: See schematic and board together and cross-probe a known component (R1) (budget 2) + one step back
    s5 = shots.get("task5-split-probe", {})
    v5 = s5.get("viewer") or {}
    ok5, obs5, oblist5 = shot_ok("task5-split-probe")
    p5_raw = presses.get("press:split", [])
    p5 = [p["id"] for p in p5_raw] + ["tap:R1@slot2"]
    pcb_ref = v5.get("split_pcb_ref") or ""
    sch_ref = v5.get("split_sch_ref") or ""
    t5_closed = check_closed_to_idle("task5-back", "press:task5-back", "rail:Split")
    s5b = shots.get("task5-back", {})
    v5b = s5b.get("viewer") or {}
    # Preceding page before Split in M.PAGES is gerbers (after task4-gerbers-vs-board).
    prev_before_split = (shots.get("task4-gerbers-vs-board") or shots.get("gerbers") or {}).get("viewer") or {}
    prev_open = str(prev_before_split.get("open") or "")
    prev_gerbers = bool(prev_before_split.get("gerbers"))
    t5_returned_prev = (
        t5_closed["capture_ok"]
        and not t5_closed["obstructed"]
        and t5_closed["presses"] == ["rail:Split"]
        and t5_closed["press_found"]
        and prev_open not in ("", "split")
        and v5b.get("open") == prev_open
        and bool(v5b.get("gerbers")) == prev_gerbers
        and s5b.get("state") == "ready"
    )
    t5_back = {
        **t5_closed,
        "closed_viewer": t5_closed["ok"],
        "preceding_open": prev_open,
        "preceding_gerbers": prev_gerbers,
        "returned_to": "closed" if t5_closed["ok"] else ("gerbers" if (v5b.get("open") == "board" and v5b.get("gerbers")) else (v5b.get("open") or "split")),
        "ok": t5_closed["ok"] or t5_returned_prev,
    }
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
        {"state": s5.get("state"), "open": v5.get("open"), "panes_count": v5.get("panes_count"), "probe": v5.get("probe"), "split_pcb_ref": pcb_ref, "split_sch_ref": sch_ref, "split_picked_pcb": v5.get("split_picked_pcb"), "split_lit_sch": v5.get("split_lit_sch"), "split_lit_pcb": v5.get("split_lit_pcb"), "capture_ok": ok5, "obstructed": obs5, "obstructions": oblist5, "one_step_back": t5_back},
        note=note5,
        one_step_back=(t5_back["ok"] if "task5-back" in shots else "not_measured"),
        found=any(p.get("found") for p in p5_raw),
    ))

    # Task 6: Open a mech part (budget 2) + one step back (press rail:Mech shuts viewer)
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
    p6_raw = presses.get("press:mech", [])
    p6 = [p["id"] for p in p6_raw]
    t6_start = settled_tags.get("task6-back-start", {})
    t6_start_ok = bool(t6_start.get("settled")) and (t6_start.get("viewer") or {}).get("open") == "mech"
    t6_closed = check_closed_to_idle("task6-back", "press:task6-back", "rail:Mech")
    t6_back = {**t6_closed, "start_ok": t6_start_ok, "ok": t6_start_ok and t6_closed["ok"]}
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
        {"state": s6.get("state"), "open": v6.get("open"), "key": v6.get("key"), "solid_key": v6.get("solid_key"), "solid_painted": v6.get("solid_painted"), "solid_painted_since_press": v6.get("solid_painted_since_press"), "solid_last_w": v6.get("solid_last_w"), "solid_last_h": v6.get("solid_last_h"), **b6, "body_diff_vs_3d": diff_vs_3d, "body_diff_vs_mech_only": diff_vs_mech_only, "capture_ok": ok6, "obstructed": obs6, "obstructions": oblist6, "one_step_back": t6_back},
        note=note6,
        one_step_back=(t6_back["ok"] if "task6-back" in shots else "not_measured"),
        found=any(p.get("found") for p in p6_raw),
    ))

    # Task 7: Run a simulation of the schematic (budget 2: rail:Simulation -> Run)
    if embed:
        k7 = kicad_tags.get("task7-sim", {})
        k7_frames = k7.get("frames") or []
        k7_shown = next((f for f in k7_frames if f.get("xid") == k7.get("shown")), {})
        ok7a, obs7a, oblist7a = shot_ok("task7-sim")
        ok7b, obs7b, oblist7b = shot_ok("task7-sim-run")
        b7a = body_check("task7-sim")
        b7b = body_check("task7-sim-run")
        sim_act = embed_actions.get("sim-run", {})
        p7_raw = presses.get("press:task7-sim", [])
        p7 = [p["id"] for p in p7_raw] + (["key:r(Run)"] if sim_act.get("ok") else [])
        sim_diff = body_diff_ratio(
            full_dir / "task7-sim.png",
            (shots.get("task7-sim") or {}).get("layout") or {},
            full_dir / "task7-sim-run.png",
            (shots.get("task7-sim-run") or {}).get("layout") or {},
        )
        # Without a simulator completion observer, Task 7 is `unavailable` (not measurable until a later KiCad build
        # exposes a read of a GUI-started simulation). When an observer IS wired, "New Simulation Tab" proves task_failed.
        setup_dialog_open = any(o.get("title") == "New Simulation Tab" for o in oblist7b)
        unexpected_obs7b = [o for o in oblist7b if o.get("title") != "New Simulation Tab"]
        has_observer = bool(sim_act.get("observer"))
        result_summary = str(sim_act.get("result_summary") or "").strip()
        sim_result_evidenced = bool(has_observer and sim_act.get("result_evidenced") and result_summary) and not setup_dialog_open
        if not full_client_ok or not ok7a or not ok7b or obs7a or bool(unexpected_obs7b):
            o7 = "harness_failed"
        elif not has_observer:
            o7 = "unavailable"
        elif setup_dialog_open:
            o7 = "task_failed"
        elif (
            k7.get("state") == "ready"
            and k7.get("in_gpui_tree") is True
            and k7_shown.get("kind") == "SIMULATOR"
            and int(k7_shown.get("painted") or 0) >= 15
            and b7a["body_non_flat"]
            and b7b["body_non_flat"]
            and sim_act.get("ok") is True
            and sim_result_evidenced
            and len(p7) <= 2
        ):
            o7 = "passed"
        else:
            o7 = "task_failed"
        if o7 == "passed":
            note7 = ""
        elif o7 == "unavailable":
            note7 = "not measurable until a later KiCad build (no read of a GUI-started simulation)"
        elif setup_dialog_open:
            note7 = "pressing Run opened 'New Simulation Tab' setup dialog with no completed analysis or trace (fixture has no SPICE models; 0 points for opening a dialog)"
        elif not sim_result_evidenced:
            note7 = "observer reported no completed simulation analysis result (operating-point value or trace)"
        else:
            note7 = f"kicad_state={k7.get('state')} shown_kind={k7_shown.get('kind')} painted={k7_shown.get('painted')} sim_run_ok={sim_act.get('ok')} body_diff={sim_diff}"
        tasks.append(make_task(
            7, "Run a simulation of the schematic", 2, p7, o7,
            {
                "kicad_state": k7.get("state"),
                "in_gpui_tree": k7.get("in_gpui_tree"),
                "shown_kind": k7_shown.get("kind"),
                "shown_painted": k7_shown.get("painted"),
                "sim_run_action": sim_act,
                "observer": sim_act.get("observer"),
                "setup_dialog_open": setup_dialog_open,
                "sim_result_evidenced": sim_result_evidenced,
                "result_summary": sim_act.get("result_summary", ""),
                "body_open": b7a,
                "body_after_run": b7b,
                "body_diff_after_run": sim_diff,
                "capture_ok": ok7a and ok7b,
                "obstructed": obs7a or obs7b,
                "obstructions": oblist7a + oblist7b,
            },
            note=note7,
            found=any(p.get("found") for p in p7_raw),
        ))
    else:
        tasks.append(make_task(
            7, "Run a simulation of the schematic", 2, [], "unavailable",
            {"rail_simulation_present": "rail:Simulation" in (s1.get("controls_ids") or [])},
            note="run with --embed against a bp.13.2+ KiCad runtime with sim-frame capability",
        ))

    # Task 8: Move between Backplane's viewer and embedded KiCad editor (budget 1 per direction)
    if embed:
        k8_edit = kicad_tags.get("task8-edit", {})
        k8_view = kicad_tags.get("task8-view", {})
        k8_frames = k8_edit.get("frames") or []
        k8_shown = next((f for f in k8_frames if f.get("xid") == k8_edit.get("shown")), {})
        s8_view = shots.get("task8-view", {})
        v8_view = s8_view.get("viewer") or {}
        ok8a, obs8a, oblist8a = shot_ok("task8-edit")
        ok8b, obs8b, oblist8b = shot_ok("task8-view")
        b8a = body_check("task8-edit")
        b8b = body_check("task8-view")
        p8_edit_raw = presses.get("press:task8-edit", [])
        p8_view_raw = presses.get("press:task8-view", [])
        p8_edit = [p["id"] for p in p8_edit_raw]
        p8_view = [p["id"] for p in p8_view_raw]
        if not full_client_ok or not ok8a or not ok8b or obs8a or obs8b:
            o8 = "harness_failed"
        elif (
            k8_edit.get("state") == "ready"
            and k8_edit.get("in_gpui_tree") is True
            and k8_shown.get("kind") == "PCB"
            and int(k8_shown.get("painted") or 0) >= 15
            and b8a["body_non_flat"]
            and k8_view.get("ok") is False
            and s8_view.get("state") == "ready"
            and v8_view.get("open") == "board"
            and str(v8_view.get("plot_key", "")).startswith("0|")
            and b8b["body_non_flat"]
            and len(p8_edit) == 1
            and len(p8_view) == 1
        ):
            o8 = "passed"
        else:
            o8 = "task_failed"
        note8 = "" if o8 == "passed" else f"edit_state={k8_edit.get('state')} shown_kind={k8_shown.get('kind')} painted={k8_shown.get('painted')} view_kicad_ok={k8_view.get('ok')} view_state={s8_view.get('state')}"
        tasks.append(make_task(
            8, "Move between Backplane's viewer and embedded KiCad editor", 1, p8_edit, o8,
            {
                "edit_presses": p8_edit,
                "view_presses": p8_view,
                "edit_kicad_state": k8_edit.get("state"),
                "edit_in_gpui_tree": k8_edit.get("in_gpui_tree"),
                "edit_shown_kind": k8_shown.get("kind"),
                "edit_shown_painted": k8_shown.get("painted"),
                "view_kicad_detached": k8_view.get("ok") is False,
                "view_state": s8_view.get("state"),
                "view_open": v8_view.get("open"),
                "view_plot_key": v8_view.get("plot_key"),
                "body_edit": b8a,
                "body_view": b8b,
                "capture_ok": ok8a and ok8b,
                "obstructed": obs8a or obs8b,
                "obstructions": oblist8a + oblist8b,
            },
            note=note8,
            one_step_back=(k8_view.get("ok") is False and s8_view.get("state") == "ready" and len(p8_view) == 1 and any(p.get("found") for p in p8_view_raw)),
            found=any(p.get("found") for p in p8_edit_raw),
        ))
    else:
        tasks.append(make_task(
            8, "Move between Backplane's viewer and embedded KiCad editor", 1, [], "unavailable",
            {"ui_edit_present": "ui:Edit" in (s1.get("controls_ids") or [])},
            note="run with --embed against an embedded KiCad runtime",
        ))

    # Task 9: Turn Follow agent on and see that it is on (budget 1) + one step back (toggle off)
    s9 = shots.get("task9-follow-on", {})
    v9 = s9.get("viewer") or {}
    ok9, obs9, oblist9 = shot_ok("task9-follow-on")
    p9_raw = presses.get("press:task9-follow-on", [])
    p9 = [p["id"] for p in p9_raw]
    p9_found = any(p.get("found") for p in p9_raw)
    s9b = shots.get("task9-follow-off", {})
    v9b = s9b.get("viewer") or {}
    ok9b, obs9b, oblist9b = shot_ok("task9-follow-off")
    p9b_raw = presses.get("press:task9-follow-off", [])
    p9b = [p["id"] for p in p9b_raw]
    p9b_found = any(p.get("found") for p in p9b_raw)
    t9_back_ok = (
        ok9b
        and not obs9b
        and len(p9b) == 1
        and p9b_found
        and v9b.get("follow_present") is True
        and v9b.get("follow_on") is False
        and v9b.get("follow_rect_visible") is True
    )
    t9_back = {
        "ok": t9_back_ok,
        "presses": p9b,
        "press_found": p9b_found,
        "follow_present": v9b.get("follow_present"),
        "follow_on": v9b.get("follow_on"),
        "follow_rect_visible": v9b.get("follow_rect_visible"),
        "capture_ok": ok9b,
        "obstructed": obs9b,
        "obstructions": oblist9b,
    }
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
        {"press_found": p9_found, "follow_present": v9.get("follow_present"), "follow_on": v9.get("follow_on"), "follow_rect_visible": v9.get("follow_rect_visible"), "capture_ok": ok9, "obstructed": obs9, "obstructions": oblist9, "one_step_back": t9_back},
        one_step_back=(t9_back_ok if "task9-follow-off" in shots else "not_measured"),
        found=p9_found,
    ))

    # Task 10: Close the viewer and get the thread's full width back (budget 1)
    s10 = shots.get("task10-closed", {})
    v10 = s10.get("viewer") or {}
    ok10, obs10, oblist10 = shot_ok("task10-closed")
    p10_raw = presses.get("press:task10-closed", [])
    p10 = [p["id"] for p in p10_raw]
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
        and abs(pane_w_closed) <= 2.0
        and p10 == ["rail:Schematic"]
    ):
        o10 = "passed"
    else:
        o10 = "task_failed"
    tasks.append(make_task(
        10, "Close the viewer and get the thread's full width back", 1, p10, o10,
        {"open": v10.get("open"), "active_kind": v10.get("active_kind"), "idle_main_w": idle_main.get("w"), "closed_main_w": closed_main.get("w"), "width_diff_px": round(width_diff, 2), "pane_w_closed_px": round(pane_w_closed, 2), "capture_ok": ok10, "obstructed": obs10, "obstructions": oblist10},
        one_step_back=(o10 == "passed"),
        found=any(p.get("found") for p in p10_raw),
    ))

    return tasks


def _frame_sig(frames: list | None) -> list[tuple]:
    return sorted((f.get("kind"), bool(f.get("shown"))) for f in (frames or []) if isinstance(f, dict))


def _compare_hub_kicad(before_cap: dict | None, after_cap: dict | None) -> dict:
    """Compare hub `editor.kicad.<digest>` sessions at the two interval endpoints (keys, pid, generation, state, active, frames).

    Fail-closed: returns status="unavailable" if either endpoint lacks a complete session inventory.
    Records any warm-up session (present before with identical pid/generation/state/active/frames after) separately.
    """
    bs = (before_cap or {}).get("hub_kicad_sessions")
    as_ = (after_cap or {}).get("hub_kicad_sessions")
    if not (isinstance(bs, dict) and bs.get("ok") and bs.get("inventory_complete") is True
            and isinstance(as_, dict) and as_.get("ok") and as_.get("inventory_complete") is True):
        return {
            "assertion": "no session change observed at the two endpoints",
            "status": "unavailable",
            "reason": "complete hub editor.kicad.<digest> session inventory missing at one or both endpoints",
            "sessions_before": (bs or {}).get("sessions") if isinstance(bs, dict) else None,
            "sessions_after": (as_ or {}).get("sessions") if isinstance(as_, dict) else None,
            "warmup_sessions": [],
        }
    sess_b = bs.get("sessions") or {}
    sess_a = as_.get("sessions") or {}
    warmup: list[dict] = []
    changes: list[str] = []

    if set(sess_b.keys()) != set(sess_a.keys()):
        added = sorted(set(sess_a.keys()) - set(sess_b.keys()))
        removed = sorted(set(sess_b.keys()) - set(sess_a.keys()))
        changes.append(f"session set changed (added={added}, removed={removed})")
    for k, sv_a in sorted(sess_a.items()):
        sv_b = sess_b.get(k)
        if not sv_b:
            continue
        gen_b, gen_a = sv_b.get("generation"), sv_a.get("generation")
        same_gen = (gen_b == gen_a) if (gen_b is not None or gen_a is not None) else True
        same_pid = sv_b.get("pid") == sv_a.get("pid")
        same_state = sv_b.get("state") == sv_a.get("state")
        same_active = sv_b.get("active") == sv_a.get("active")
        same_frames = _frame_sig(sv_b.get("frames")) == _frame_sig(sv_a.get("frames"))
        if same_gen and same_pid and same_state and same_active and same_frames:
            warmup.append({"key": k, "pid": sv_a.get("pid"), "state": sv_a.get("state"), "generation": gen_a, "frames": sv_a.get("frames") or []})
        else:
            changes.append(
                f"session {k} changed (before pid={sv_b.get('pid')} state={sv_b.get('state')} gen={gen_b} active={sv_b.get('active')} frames={_frame_sig(sv_b.get('frames'))}; "
                f"after pid={sv_a.get('pid')} state={sv_a.get('state')} gen={gen_a} active={sv_a.get('active')} frames={_frame_sig(sv_a.get('frames'))})"
            )

    return {
        "assertion": "no session change observed at the two endpoints",
        "status": "changed" if changes else "unchanged",
        "reason": "; ".join(changes),
        "sessions_before": sess_b,
        "sessions_after": sess_a,
        "warmup_sessions": warmup,
    }


def _page_empty_words(page: str, viewer: dict) -> str:
    """Positive empty-state words from this page's own fields only (never solid_note on board/schematic)."""
    if page == "3d":
        return str(viewer.get("empty_text") or viewer.get("solid_note") or "").strip()
    return str(viewer.get("empty_text") or viewer.get("plot_none") or "").strip()


def evaluate_extra_checks(
    outdir: Path,
    kinds: list[str],
    parsed_by_kind: dict[str, dict],
    captures_by_kind: dict[str, dict[str, dict]],
    embed: bool = False,
) -> list[dict]:
    """Evaluate fail-closed extra checks (missing-document, no-editor pages, present-design Edit, scripted Edit) outside the 40-pt scorecard."""
    checks: list[dict] = []
    # Ordered settled page slugs to find the immediately preceding capture for interval comparisons.
    settled_order = [slug for slug, _, _, _ in M.PAGES]

    for kind in kinds:
        kdir = outdir / kind
        p = parsed_by_kind.get(kind) or {}
        shots = p.get("shots") or {}
        settled_tags = p.get("settled_tags") or {}
        kicad_tags = p.get("kicad_tags") or {}
        presses = p.get("presses") or {}
        keys_by_mark = p.get("keys") or {}
        caps = captures_by_kind.get(kind) or {}

        # 0. Swallowed-Escape check on full: part highlighted -> viewer closed -> ONE Escape -> bare 'b' opens Board
        if kind == "full":
            esc_keys = keys_by_mark.get("extra:escape-after-closed-probe") or []
            esc_rec = settled_tags.get("extra-escape-after-closed-probe")
            sent_esc = any(k.get("keys") == "escape" for k in esc_keys)
            sent_b = any(k.get("keys") == "b" for k in esc_keys)
            if not (sent_esc and sent_b and isinstance(esc_rec, dict)):
                esc_outcome = "unavailable"
                esc_note = "escape-after-closed-probe step not recorded in this run"
                esc_open = ""
                esc_state = ""
            else:
                esc_v = esc_rec.get("viewer") or {}
                esc_open = str(esc_v.get("open") or "")
                esc_state = str(esc_rec.get("state") or "")
                if esc_rec.get("settled") is True and esc_state == "ready" and esc_open == "board":
                    esc_outcome = "passed"
                    esc_note = ""
                else:
                    esc_outcome = "failed"
                    esc_note = f"one Escape after closing probed viewer did not leave composer; bare 'b' did not open Board (settled={esc_rec.get('settled')}, state={esc_state!r}, viewer.open={esc_open!r})"
            checks.append({
                "id": "full/escape-after-closed-probe",
                "kind": "full",
                "name": "One Escape after closing a probed viewer leaves composer so bare rail key opens Board",
                "outcome": esc_outcome,
                "note": esc_note,
                "evidence": {
                    "escape_sent": sent_esc,
                    "bare_key_sent": sent_b,
                    "settled": esc_rec.get("settled") if isinstance(esc_rec, dict) else None,
                    "state": esc_state,
                    "viewer_open": esc_open,
                },
            })

        # 0b. After B1 Simulation cleanup, verify KicadPane is detached/not covering the pane and
        #     hub `runtime.holder` is absent (or `runtime.session != "live"`).
        #     Parked warm KiCad `frames[].shown` and session `active` are recorded as notes only, not failure conditions.
        if embed and "schematic" in M.HAS_RESOURCE.get(kind, set()):
            k_sc = kicad_tags.get("b1-sim-cleanup-report")
            c_sc = caps.get("b1-sim-cleanup") or {}
            s_sc = shots.get("b1-sim-cleanup") or {}
            hk_sc = c_sc.get("hub_kicad_sessions") or {}
            hub_complete = bool(hk_sc.get("ok") and hk_sc.get("inventory_complete"))
            shown_hub_frames: list[str] = []
            live_holders: list[str] = []
            session_actives: dict[str, object] = {}
            if hub_complete:
                for sid, sinfo in (hk_sc.get("sessions") or {}).items():
                    session_actives[sid] = sinfo.get("active")
                    rt = sinfo.get("runtime") if isinstance(sinfo.get("runtime"), dict) else {}
                    holder = rt.get("holder")
                    rt_sess = rt.get("session")
                    if holder and rt_sess == "live":
                        live_holders.append(f"{sid}:holder={holder}")
                    for fr in (sinfo.get("frames") or []):
                        if isinstance(fr, dict) and fr.get("shown") is True:
                            shown_hub_frames.append(f"{sid}:{fr.get('kind', '?')}")
            has_pane_report = isinstance(k_sc, dict)
            obstructed_sc = bool(c_sc.get("obstructed"))
            if not (has_pane_report or hub_complete):
                sc_outcome = "unavailable"
                sc_note = "no b1-sim-cleanup frame report or hub session inventory captured"
            else:
                sc_fails: list[str] = []
                if obstructed_sc:
                    sc_fails.append("native dialog/window obstructing pane after Simulation cleanup")
                if has_pane_report:
                    if k_sc.get("in_gpui_tree") is True:
                        sc_fails.append("KicadPane still mounted in GPUI tree after Simulation cleanup")
                    if k_sc.get("shown"):
                        sc_fails.append(f"pane kicad report still has shown={k_sc.get('shown')!r}")
                if live_holders:
                    sc_fails.append(f"hub KiCad session still held live in pane: {', '.join(live_holders)}")
                sc_outcome = "failed" if sc_fails else "passed"
                sc_note = "; ".join(sc_fails)
            checks.append({
                "id": f"{kind}/sim-cleanup-no-frame",
                "kind": kind,
                "name": f"After B1 Simulation cleanup on {kind}, KicadPane is detached and no live holder covers the viewer",
                "outcome": sc_outcome,
                "note": sc_note,
                "evidence": {
                    "has_pane_report": has_pane_report,
                    "in_gpui_tree": (k_sc or {}).get("in_gpui_tree") if has_pane_report else None,
                    "pane_shown": (k_sc or {}).get("shown") if has_pane_report else None,
                    "viewer_open": (s_sc.get("viewer") or {}).get("open", ""),
                    "obstructed": obstructed_sc,
                    "hub_inventory_complete": hub_complete,
                    "live_holders": live_holders,
                    "hub_frame_shown": shown_hub_frames,
                    "hub_session_active": session_actives,
                    "hub_kicad_sessions": hk_sc if sc_outcome == "failed" else None,
                },
            })

        # A. Missing-document checks via viewshow (schematic-only: board, 3d; board-only: schematic)
        # Compare the first missing-doc check against the session inventory captured AT the confirmed idle reset.
        if kind in M.EXTRA_MISSING_SHOTS:
            missing_specs = (
                [("missing-board", "extra-missing-board", "board", "extra-missing-start-idle"),
                 ("missing-3d", "extra-missing-3d", "3d", "extra-missing-board")]
                if kind == "schematic-only"
                else [("missing-schematic", "extra-missing-schematic", "schematic", "extra-missing-start-idle")]
            )
            for cid, sname, expected_open, prev_shot in missing_specs:
                srec = shots.get(sname) or {}
                cap = caps.get(sname) or {}
                prev_cap = caps.get(prev_shot)
                png = kdir / f"{sname}.png"
                cap_ok = bool(cap.get("ok") and png.exists() and png.stat().st_size > 0)
                inspect_ok = bool(cap.get("inspect_ok"))
                obstructed = bool(cap.get("obstructed"))
                obstructions = cap.get("obstructions") or []
                controls_captured = bool(srec.get("controls_captured"))
                v = srec.get("viewer") or {}
                empty_text = _page_empty_words(expected_open, v)
                drawn_edit = [c for c in (srec.get("drawn_controls_ids") or []) if c in ("ui:Edit", "ui:Open in KiCad")]
                hk = _compare_hub_kicad(prev_cap, cap)
                unavail: list[str] = []
                fails: list[str] = []
                if not cap_ok:
                    outcome = "harness_failed"
                    fails.append("capture missing or failed")
                else:
                    if not inspect_ok:
                        unavail.append("obstruction inspection failed/missing")
                    elif obstructed:
                        titles = [o.get("title") or "/".join(o.get("class") or []) for o in obstructions]
                        fails.append(f"native dialog/obstruction ({', '.join(t for t in titles if t) or 'unknown'})")
                    if not controls_captured:
                        unavail.append("control inventory not captured")
                    elif drawn_edit:
                        fails.append(f"Edit/Open in KiCad offered ({', '.join(drawn_edit)})")
                    if v.get("open") != expected_open:
                        fails.append(f"viewer.open={v.get('open')!r} (expected {expected_open!r})")
                    if srec.get("state") != "empty":
                        fails.append(f"settled_state={srec.get('state')!r} (expected 'empty')")
                    if not empty_text:
                        fails.append("empty state has no words")
                    if hk["status"] == "unavailable":
                        unavail.append(hk["reason"])
                    elif hk["status"] == "changed":
                        fails.append(f"KiCad session/process changed: {hk['reason']}")
                    outcome = "failed" if fails else ("unavailable" if unavail else "passed")
                checks.append({
                    "id": f"{kind}/{cid}",
                    "kind": kind,
                    "name": f"Missing document ({expected_open}) on {kind}",
                    "outcome": outcome,
                    "note": "; ".join(fails + unavail),
                    "evidence": {
                        "shot": str(png),
                        "capture_ok": cap_ok,
                        "inspect_ok": inspect_ok,
                        "controls_captured": controls_captured,
                        "viewer_open": v.get("open", ""),
                        "settled_state": srec.get("state", ""),
                        "empty_text": empty_text,
                        "drawn_edit_controls": drawn_edit,
                        "obstructed": obstructed,
                        "obstructions": obstructions,
                        "hub_kicad": hk,
                    },
                })

            # B. No-editor pages (Files, Review, Browser, Mech, Split): NO drawn Edit/Open in KiCad, no session change at endpoints
            ref_shot = shots.get("idle") or (next(iter(shots.values())) if shots else {})
            rail_map = infer_rail_map(ref_shot)
            for slug in M.NO_EDITOR_CHECK_PAGES:
                srec = shots.get(slug) or {}
                cap = caps.get(slug) or {}
                idx = settled_order.index(slug)
                prev_cap = caps.get(settled_order[idx - 1]) if idx > 0 else None
                png = kdir / f"{slug}.png"
                cap_ok = bool(cap.get("ok") and png.exists() and png.stat().st_size > 0)
                inspect_ok = bool(cap.get("inspect_ok"))
                obstructed = bool(cap.get("obstructed"))
                controls_captured = bool(srec.get("controls_captured"))
                v_open = (srec.get("viewer") or {}).get("open", "")
                rentry = rail_map.get(slug) or {}
                if not rail_map:
                    rail_state = "unknown"
                else:
                    rail_state = rentry.get("state", "hidden" if slug not in rail_map else "unknown")
                drawn_edit = [c for c in (srec.get("drawn_controls_ids") or []) if c in ("ui:Edit", "ui:Open in KiCad")]
                hk = _compare_hub_kicad(prev_cap, cap)
                unavail = []
                fails = []
                if not cap_ok:
                    outcome = "harness_failed"
                    fails.append("capture missing or failed")
                elif v_open != slug:
                    if rail_state == "disabled":
                        outcome = "unavailable"
                        why = rentry.get("why") or "disabled"
                        unavail.append(f"rail entry {slug!r} confirmed disabled ({why}); page never opened")
                    elif rail_state in ("unknown", "hidden"):
                        outcome = "unavailable"
                        unavail.append(f"rail entry {slug!r} state={rail_state!r}; page never opened")
                    else:
                        outcome = "failed"
                        fails.append(f"enabled rail entry {slug!r} failed to open (rail_state={rail_state!r}, viewer.open={v_open!r})")
                else:
                    if not inspect_ok:
                        unavail.append("obstruction inspection failed/missing")
                    elif obstructed:
                        fails.append("obstructed by native dialog")
                    if not controls_captured:
                        unavail.append("control inventory not captured")
                    elif drawn_edit:
                        fails.append(f"Edit/Open in KiCad offered ({', '.join(drawn_edit)})")
                    if hk["status"] == "unavailable":
                        unavail.append(hk["reason"])
                    elif hk["status"] == "changed":
                        fails.append(f"KiCad session changed: {hk['reason']}")
                    outcome = "failed" if fails else ("unavailable" if unavail else "passed")
                checks.append({
                    "id": f"{kind}/no-edit-{slug}",
                    "kind": kind,
                    "name": f"No Edit control & no session change observed at the two endpoints on {kind}/{slug}",
                    "outcome": outcome,
                    "note": "; ".join(fails + unavail),
                    "evidence": {
                        "shot": str(png),
                        "rail_state": rail_state,
                        "viewer_open": v_open,
                        "controls_captured": controls_captured,
                        "drawn_edit_controls": drawn_edit,
                        "inspect_ok": inspect_ok,
                        "obstructed": obstructed,
                        "hub_kicad": hk,
                    },
                })

            # C. Present design pages: Edit/Open in KiCad MUST be drawn
            for slug in M.PRESENT_DESIGN_EDIT_PAGES.get(kind, []):
                srec = shots.get(slug) or {}
                cap = caps.get(slug) or {}
                png = kdir / f"{slug}.png"
                cap_ok = bool(cap.get("ok") and png.exists() and png.stat().st_size > 0)
                controls_captured = bool(srec.get("controls_captured"))
                drawn_edit = [c for c in (srec.get("drawn_controls_ids") or []) if c in ("ui:Edit", "ui:Open in KiCad")]
                if not cap_ok:
                    outcome, note = "harness_failed", "capture missing or failed"
                elif not controls_captured:
                    outcome, note = "unavailable", "control inventory not captured"
                elif drawn_edit:
                    outcome, note = "passed", ""
                else:
                    outcome, note = "failed", "Edit/Open in KiCad not drawn on present design page"
                checks.append({
                    "id": f"{kind}/has-edit-{slug}",
                    "kind": kind,
                    "name": f"Edit control present on {kind}/{slug}",
                    "outcome": outcome,
                    "note": note,
                    "evidence": {
                        "shot": str(png),
                        "controls_captured": controls_captured,
                        "drawn_edit_controls": drawn_edit,
                    },
                })

            # D. Scripted Edit from Files/Review/Browser under --embed -> switches to design page & paints SCH/PCB
            if embed:
                expected_frame = "SCH" if kind == "schematic-only" else "PCB"
                expected_page = "schematic" if kind == "schematic-only" else "board"
                prev_for_edit = M.EXTRA_MISSING_SHOTS[kind][-1]
                for slug, _, _ in M.EXTRA_EDIT_PAGES:
                    sname = f"extra-edit-{slug}"
                    cap = caps.get(sname) or {}
                    prev_cap = caps.get(prev_for_edit)
                    prev_for_edit = sname
                    png = kdir / f"{sname}.png"
                    cap_ok = bool(cap.get("ok") and png.exists() and png.stat().st_size > 0)
                    inspect_ok = bool(cap.get("inspect_ok"))
                    obstructed = bool(cap.get("obstructed"))
                    obstructions = cap.get("obstructions") or []
                    start_tag = settled_tags.get(f"{sname}-start") or {}
                    start_ok = bool(start_tag.get("settled") and start_tag.get("kind") == slug)
                    start_seq = start_tag.get("seq")
                    shot_seq = (shots.get(sname) or {}).get("seq")
                    p_evs = presses.get(f"press:{sname}", [])
                    edit_seqs = [
                        ev.get("seq")
                        for ev in p_evs
                        if ev.get("id") in ("script:kicad-edit", "ui:Edit") and ev.get("found") and isinstance(ev.get("seq"), int)
                    ]
                    press_delivered = bool(edit_seqs)
                    seq_order_ok = (
                        isinstance(start_seq, int)
                        and isinstance(shot_seq, int)
                        and any(start_seq < es < shot_seq for es in edit_seqs)
                    )
                    k = kicad_tags.get(f"{sname}-report") or kicad_tags.get(sname)
                    k_frames = (k or {}).get("frames") or []
                    k_shown = next((f for f in k_frames if f.get("xid") == (k or {}).get("shown")), {})
                    shown_painted = int(k_shown.get("painted") or 0)
                    hk = _compare_hub_kicad(prev_cap, cap)
                    unavail = []
                    fails = []
                    hfail = []
                    if not cap_ok:
                        hfail.append("capture missing or failed")
                    if not inspect_ok:
                        unavail.append("obstruction inspection failed/missing")
                    if not start_ok:
                        unavail.append(f"starting page tag {sname}-start not verified")
                    if not press_delivered:
                        unavail.append("ui:Edit press not delivered")
                    elif not seq_order_ok:
                        unavail.append(f"ui:Edit press order not verified (start_seq={start_seq}, edit_seqs={edit_seqs}, shot_seq={shot_seq})")
                    if k is None:
                        hfail.append("kicad report missing")
                    if not hfail and not unavail:
                        if obstructed:
                            titles = [o.get("title") or "/".join(o.get("class") or []) for o in obstructions]
                            fails.append(f"obstructed by native dialog ({', '.join(t for t in titles if t) or 'unknown'})")
                        if k.get("state") != "ready":
                            fails.append(f"kicad_state={k.get('state')!r}")
                        if k.get("in_gpui_tree") is not True:
                            fails.append(f"in_gpui_tree={k.get('in_gpui_tree')!r}")
                        if k_shown.get("kind") != expected_frame:
                            fails.append(f"shown_kind={k_shown.get('kind')!r} (expected {expected_frame!r})")
                        if shown_painted <= 0:
                            fails.append(f"shown_painted={shown_painted} (expected > 0)")
                        v_shot = (shots.get(sname) or {}).get("viewer") or {}
                        if v_shot.get("open") != expected_page:
                            fails.append(f"viewer.open={v_shot.get('open')!r} (expected {expected_page!r})")
                    outcome = "harness_failed" if hfail else ("failed" if fails else ("unavailable" if unavail else "passed"))
                    checks.append({
                        "id": f"{kind}/scripted-edit-{slug}",
                        "kind": kind,
                        "name": f"Scripted Edit from {slug} on {kind} (expect {expected_page}/{expected_frame})",
                        "outcome": outcome,
                        "note": "; ".join(hfail + fails + unavail),
                        "evidence": {
                            "shot": str(png),
                            "capture_ok": cap_ok,
                            "inspect_ok": inspect_ok,
                            "start_verified": start_ok,
                            "press_delivered": press_delivered,
                            "expected_page": expected_page,
                            "expected_frame": expected_frame,
                            "viewer_open": ((shots.get(sname) or {}).get("viewer") or {}).get("open", ""),
                            "kicad_state": (k or {}).get("state"),
                            "in_gpui_tree": (k or {}).get("in_gpui_tree"),
                            "shown_kind": k_shown.get("kind"),
                            "shown_painted": shown_painted,
                            "obstructed": obstructed,
                            "obstructions": obstructions,
                            "hub_kicad": hk,
                        },
                    })

        # E. No-design / mech-only scripted Edit on Files: requires verified start tag + delivered script:kicad-edit + no session started
        elif kind in M.NO_DESIGN_KINDS and embed:
            sname = "extra-edit-files"
            cap = caps.get(sname) or {}
            prev_cap = caps.get("browser")
            png = kdir / f"{sname}.png"
            cap_ok = bool(cap.get("ok") and png.exists() and png.stat().st_size > 0)
            inspect_ok = bool(cap.get("inspect_ok"))
            obstructed = bool(cap.get("obstructed"))
            start_tag = settled_tags.get(f"{sname}-start") or {}
            start_ok = bool(start_tag.get("settled") and start_tag.get("kind") == "files")
            start_seq = start_tag.get("seq")
            shot_seq = (shots.get(sname) or {}).get("seq")
            p_evs = presses.get(f"press:{sname}", [])
            edit_seqs = [
                ev.get("seq")
                for ev in p_evs
                if ev.get("id") in ("script:kicad-edit", "ui:Edit") and ev.get("found") and isinstance(ev.get("seq"), int)
            ]
            press_delivered = bool(edit_seqs)
            seq_order_ok = (
                isinstance(start_seq, int)
                and isinstance(shot_seq, int)
                and any(start_seq < es < shot_seq for es in edit_seqs)
            )
            v_shot = (shots.get(sname) or {}).get("viewer") or {}
            files_status = str(v_shot.get("files_status") or v_shot.get("status") or "").strip()
            hk = _compare_hub_kicad(prev_cap, cap)
            sess_a = hk.get("sessions_after") or {}
            k = kicad_tags.get(f"{sname}-report") or {}
            unavail = []
            fails = []
            if not cap_ok:
                outcome = "harness_failed"
                fails.append("capture missing or failed")
            else:
                if not inspect_ok:
                    unavail.append("obstruction inspection failed/missing")
                elif obstructed:
                    fails.append("obstructed by native dialog")
                if not start_ok:
                    unavail.append(f"starting page tag {sname}-start not verified")
                if not press_delivered:
                    unavail.append("script:kicad-edit press not delivered")
                elif not seq_order_ok:
                    unavail.append(f"script:kicad-edit press order not verified (start_seq={start_seq}, edit_seqs={edit_seqs}, shot_seq={shot_seq})")
                if not files_status:
                    unavail.append("Files status line not exposed in client state")
                if hk["status"] == "unavailable":
                    unavail.append(hk["reason"])
                elif sess_a:
                    fails.append(f"unexpected KiCad sessions: {list(sess_a.keys())}")
                if k.get("in_gpui_tree") is True:
                    fails.append("KicadPane unexpectedly mounted in GPUI tree")
                outcome = "failed" if fails else ("unavailable" if unavail else "passed")
            checks.append({
                "id": f"{kind}/scripted-edit-files",
                "kind": kind,
                "name": f"Scripted Edit from files on {kind} starts no KiCad (no session change observed at the two endpoints)",
                "outcome": outcome,
                "note": "; ".join(fails + unavail),
                "evidence": {
                    "shot": str(png),
                    "capture_ok": cap_ok,
                    "inspect_ok": inspect_ok,
                    "start_verified": start_ok,
                    "press_delivered": press_delivered,
                    "files_status": files_status,
                    "obstructed": obstructed,
                    "in_gpui_tree": k.get("in_gpui_tree"),
                    "hub_kicad": hk,
                },
            })

    return checks


def main(argv: list[str] | None = None) -> int:
    raw_args = list(sys.argv[1:] if argv is None else argv)
    embed = "--embed" in raw_args
    share = "--share" in raw_args
    pos_args = [a for a in raw_args if a not in ("--embed", "--share")]
    if len(pos_args) < 2:
        print("usage: rail_shots_post.py [--embed] [--share] OUTDIR KIND [KIND ...]", file=sys.stderr)
        return 2
    outdir = Path(pos_args[0])
    try:
        kinds = M.validate_kinds(pos_args[1:])
    except ValueError as e:
        print(f"rail-shots: {e}", file=sys.stderr)
        return 2

    parsed: dict[str, dict] = {}
    captures_by_kind: dict[str, dict[str, dict]] = {}
    embed_actions_by_kind: dict[str, dict[str, dict]] = {}
    cleared_by_kind: dict[str, set[str]] = {}
    kinds_out: dict[str, dict] = {}
    harness_errors: list[str] = []
    all_private: list[str] = []

    # Missing runtime.json is unknown (None), never fabricated as an empty capabilities list.
    runtime_info: dict | None = None
    rt_path = outdir / "runtime.json"
    if rt_path.exists():
        try:
            loaded = json.loads(rt_path.read_text(encoding="utf-8"))
            if isinstance(loaded, dict) and isinstance(loaded.get("capabilities"), list):
                runtime_info = loaded
        except Exception:
            runtime_info = None

    for kind in kinds:
        kdir = outdir / kind
        p = parse_client_log(kdir / "client.log")
        caps = parse_captures(kdir)
        eacts = parse_embed_actions(kdir)
        parsed[kind] = p
        captures_by_kind[kind] = caps
        embed_actions_by_kind[kind] = eacts

        exit_file = kdir / "client.exit"
        client_exit = int(exit_file.read_text().strip()) if exit_file.exists() else -1
        if client_exit != 0:
            harness_errors.append(f"{kind}: client exited with {client_exit}")
        if p.get("refused_shots"):
            harness_errors.append(f"{kind}: client refused {len(p['refused_shots'])} shot step(s)")

        kind_embed = embed
        expected = M.expected_shots(kind, embed=kind_embed)
        missing: list[str] = []
        failed_caps: list[str] = []
        obstructions_found: dict[str, list[dict]] = {}
        private_shots: dict[str, list[str]] = {}
        cleared_shots: set[str] = set()
        shots = p.get("shots", {})
        for sname in expected:
            rec = caps.get(sname)
            png = kdir / f"{sname}.png"
            cap_ok = bool(rec and rec.get("ok") and png.exists() and png.stat().st_size > 0)
            if not rec or not png.exists() or png.stat().st_size == 0:
                missing.append(sname)
            elif not rec.get("ok"):
                failed_caps.append(sname)
            if rec and rec.get("obstructed"):
                obstructions_found[sname] = rec.get("obstructions") or []
            reasons = shot_privacy_reasons(rec or {}, shots.get(sname) or {}, shot_name=sname, embed=kind_embed)
            if reasons:
                private_shots[sname] = reasons
                all_private.append(f"{kind}/{sname}: {'; '.join(reasons)}")
            elif cap_ok:
                cleared_shots.add(sname)

        cleared_by_kind[kind] = cleared_shots

        if missing:
            harness_errors.append(f"{kind}: missing captures: {', '.join(missing)}")
        if failed_caps:
            harness_errors.append(f"{kind}: failed captures: {', '.join(failed_caps)}")

        # Derive rail map (enabled / disabled / hidden) from idle shot (or first available shot).
        ref_shot = shots.get("idle") or (next(iter(shots.values())) if shots else {})
        rail_map = infer_rail_map(ref_shot)

        pages_summary: dict[str, dict] = {}
        presses = p.get("presses", {})
        avail = M.HAS_RESOURCE.get(kind, set())
        for slug, rail_id, settle_kind, expected_open in M.PAGES:
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

            if slug == "idle":
                r_state = "enabled"
                r_why = ""
            else:
                rentry = rail_map.get(settle_kind)
                if rentry is None:
                    r_state = "hidden"
                    r_why = ""
                else:
                    r_state = rentry["state"]
                    r_why = rentry.get("why", "")

            bstats = pane_body_stats(kdir / f"{slug}.png", srec.get("layout") or {}) if slug != "idle" else None
            p_reasons = private_shots.get(slug, [])
            ep_reasons = private_shots.get(f"{slug}-300ms", []) if slug != "idle" else []
            early_png = kdir / f"{slug}-300ms.png"
            early_cap_ok = bool(ecap.get("ok") and early_png.exists() and early_png.stat().st_size > 0) if slug != "idle" else False
            ev = erec.get("viewer") or {}
            early_loading_text = str(ev.get("loading_text") or ev.get("solid_stage") or "").strip() if slug != "idle" else ""
            empty_text = str(v.get("empty_text") or v.get("plot_none") or v.get("solid_note") or (v.get("diff_summary") if slug == "diff" else "") or "").strip()

            pages_summary[slug] = {
                "presses": [x["id"] for x in presses.get(f"press:{slug}", [])] if rail_id else [],
                "resource_present": (slug in avail) if slug != "idle" else True,
                "rail_state": r_state,
                "rail_why": r_why,
                "expected_open": expected_open,
                "actual_page": actual_page,
                "settled_state": srec.get("state", ""),
                "empty_text": empty_text,
                "matched_expected": matched,
                "body_non_flat": bstats["body_non_flat"] if bstats is not None else None,
                "header_rect": srec.get("header_rect"),
                "controls_ids": srec.get("controls_ids") or [],
                "drawn_controls_ids": srec.get("drawn_controls_ids") or [],
                "header_drawn_controls_ids": srec.get("header_drawn_controls_ids"),
                "obstructed": bool(cap.get("obstructed")),
                "obstructions": cap.get("obstructions") or [],
                "private": len(p_reasons) > 0,
                "private_reasons": p_reasons,
                "diff_open": v.get("diff_open") if slug == "diff" else None,
                "diff_files": v.get("diff_files") if slug == "diff" else None,
                "diff_summary": v.get("diff_summary") if slug == "diff" else None,
                "viewer": v,
                "early_300ms_state": erec.get("state", "") if slug != "idle" else None,
                "early_loading_text": early_loading_text if slug != "idle" else None,
                "early_capture_ok": early_cap_ok if slug != "idle" else None,
                "early_300ms_obstructed": bool(ecap.get("obstructed")) if slug != "idle" else None,
                "early_300ms_private": len(ep_reasons) > 0 if slug != "idle" else False,
                "shot": str(kdir / f"{slug}.png"),
                "early_shot": str(early_png) if slug != "idle" else None,
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
            "private_shots": private_shots,
            "cleared_shots": sorted(cleared_shots),
            "excluded_from_contact": [s for s in M.contact_shots() if (kdir / f"{s}.png").exists() and s not in cleared_shots],
            "rail": rail_map,
            "rail_metric_seen": bool(p.get("rail_metric_seen")),
            "rail_inventory": p.get("rail_inventory"),
            "b1_checks": evaluate_b1_kind(kind, p, rail_map, embed=embed),
            "pages": pages_summary,
        }

    full_client_ok = "full" in kinds_out and kinds_out["full"]["client_exit"] == 0 and not kinds_out["full"]["missing_shots"] and not kinds_out["full"]["failed_captures"]
    scorecard_tasks = evaluate_scorecard(
        outdir,
        parsed.get("full", {}),
        captures_by_kind.get("full", {}),
        full_client_ok,
        parsed.get("mech-only"),
        embed=embed,
        embed_actions=embed_actions_by_kind.get("full"),
    ) if "full" in kinds else []

    extra_checks = evaluate_extra_checks(outdir, kinds, parsed, captures_by_kind, embed=embed)

    if harness_errors or any(t["outcome"] == "harness_failed" for t in scorecard_tasks):
        overall = "harness_failed"
    elif any(t["outcome"] == "task_failed" for t in scorecard_tasks):
        overall = "task_failed"
    else:
        overall = "passed"

    # Under --share, refuse BEFORE writing steps.json or any derived images.
    if share and all_private:
        print(f"rail-shots: --share refused: {len(all_private)} private shot(s) detected:", file=sys.stderr)
        for item in all_private:
            print(f"  - {item}", file=sys.stderr)
        return 3

    task_part_coverage = {
        str(t["task"]): sorted(k for k, v in (t.get("parts") or {}).items() if v != "not_measured")
        for t in scorecard_tasks
    }
    summary = {
        "evidence_schema": 3,
        "privacy_checked": True,
        "overall_outcome": overall,
        "embed_mode": embed,
        "share_mode": share,
        "runtime": runtime_info,
        "private_shot_count": len(all_private),
        "harness_errors": harness_errors,
        "task_part_coverage": task_part_coverage,
        "kinds": kinds_out,
        "scorecard_tasks": scorecard_tasks,
        "extra_checks": extra_checks,
    }
    (outdir / "steps.json").write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")

    for kind in kinds:
        make_contact_sheet(outdir / kind, outdir / f"contact-{kind}.png", cleared_by_kind[kind])
    build_strips(outdir, kinds, parsed, cleared_by_kind)

    if harness_errors:
        for e in harness_errors:
            print(f"rail-shots: HARNESS FAILED: {e}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
