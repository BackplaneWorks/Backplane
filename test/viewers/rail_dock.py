#!/usr/bin/env python3
"""Docking/tiling measurement helpers for Scorecard Task 4 and Rule B7.

Designed against:
  - Hub layout model (`native/host/src/present/layout.rs`): `viewer.layout` + `viewer.tiles`
  - PR #1414 (`rail-x`): `GPUI_METRIC {"event":"tiles","tiles":[...],"splitters":[...]}` and `drag` script step
  - PR #1417 (`rail-v`): `layout-add`, `layout-close`, `layout-swap`, `layout-resize`,
    `layout-maximize`, `layout-restore`, `layout-overlay`, and catalog-backed refusals.

Every check stays `unavailable` until a build emits `GPUI_METRIC {"event":"tiles", ...}` and
draws real multi-tile layouts. Every visual assertion enforces per-shot privacy/obstruction
clearance (fail-closed on missing/failed `inspect_ok` or native dialogs).
"""

from __future__ import annotations

import json
import re
from pathlib import Path

from PIL import Image

DOCK_B7_CHECKS = (
    "add_beside_directions",
    "close",
    "swap",
    "resize_drag",
    "maximize_restore",
    "overlay_on_off",
    "incompatible_overlay_refusal",
)

RAW_REFUSAL_CODE_RE = re.compile(
    r"\b(ERR_[A-Z0-9_]+|INTERNAL_[A-Z0-9_]+|PRESENTER_[A-Z0-9_]+|ECONNREFUSED|ENOENT|HTTP\s+5\d\d|Refusal::|panic|backtrace)\b"
)


def _rect_tuple(r: dict | None) -> tuple[float, float, float, float] | None:
    if not isinstance(r, dict):
        return None
    try:
        x, y, w, h = float(r["x"]), float(r["y"]), float(r["w"]), float(r["h"])
    except (KeyError, TypeError, ValueError):
        return None
    if w <= 0 or h <= 0:
        return None
    return x, y, w, h


def _rects_disjoint(a: tuple[float, float, float, float], b: tuple[float, float, float, float], tol: float = 2.0) -> bool:
    ax, ay, aw, ah = a
    bx, by, bw, bh = b
    return (
        ax + aw <= bx + tol
        or bx + bw <= ax + tol
        or ay + ah <= by + tol
        or by + bh <= ay + tol
    )


def _rect_close(a: tuple[float, float, float, float], b: tuple[float, float, float, float], tol: float = 6.0) -> bool:
    return all(abs(u - v) <= tol for u, v in zip(a, b))


def _flat_pixels(im: Image.Image) -> list[tuple[int, int, int]]:
    if hasattr(im, "get_flattened_data"):
        return list(im.get_flattened_data())
    return list(im.getdata())


def tile_body_non_flat(img_path: Path, rect: dict | None, header_h: float = 34.0) -> bool | None:
    """Return True if the tile's painted body below its header is non-flat, False if blank/flat, None if unreadable."""
    rt = _rect_tuple(rect)
    if rt is None or not img_path.exists():
        return None
    x, y, w, h = rt
    with Image.open(img_path) as im:
        im = im.convert("RGB")
        iw, ih = im.size
        x0 = max(0, min(iw, int(round(x + 4))))
        y0 = max(0, min(ih, int(round(y + min(header_h, h * 0.25)))))
        x1 = max(x0, min(iw, int(round(x + w - 4))))
        y1 = max(y0, min(ih, int(round(y + h - 4))))
        if x1 - x0 < 12 or y1 - y0 < 12:
            return None
        crop = im.crop((x0, y0, x1, y1))
        colors = crop.getcolors(maxcolors=4096)
        if colors is None:
            return True
        if len(colors) <= 1:
            return False
        total = sum(c for c, _ in colors)
        dominant = max(c for c, _ in colors)
        return (dominant / total) < 0.992


def tile_region_diff(img_a: Path, img_b: Path, rect: dict | None, header_h: float = 34.0) -> float | None:
    """Return fraction of pixels differing (>12 per channel) inside `rect` between two PNGs."""
    rt = _rect_tuple(rect)
    if rt is None or not img_a.exists() or not img_b.exists():
        return None
    x, y, w, h = rt
    with Image.open(img_a) as ia, Image.open(img_b) as ib:
        ia = ia.convert("RGB")
        ib = ib.convert("RGB")
        iw, ih = min(ia.width, ib.width), min(ia.height, ib.height)
        x0 = max(0, min(iw, int(round(x + 4))))
        y0 = max(0, min(ih, int(round(y + min(header_h, h * 0.25)))))
        x1 = max(x0, min(iw, int(round(x + w - 4))))
        y1 = max(y0, min(ih, int(round(y + h - 4))))
        if x1 - x0 < 12 or y1 - y0 < 12:
            return None
        ca = ia.crop((x0, y0, x1, y1)).resize((160, 160))
        cb = ib.crop((x0, y0, x1, y1)).resize((160, 160))
        pa = _flat_pixels(ca)
        pb = _flat_pixels(cb)
        if not pa:
            return None
        changed = sum(
            1 for (r1, g1, b1), (r2, g2, b2) in zip(pa, pb)
            if abs(r1 - r2) > 12 or abs(g1 - g2) > 12 or abs(b1 - b2) > 12
        )
        return round(changed / len(pa), 4)


def _tile_source(tile: dict, viewer: dict) -> str:
    """Extract a tile's own source/identity from `viewer.tiles[id]` or `viewer.layout`."""
    tid = str(tile.get("id") or "")
    vtiles = viewer.get("tiles")
    if isinstance(vtiles, dict) and tid and isinstance(vtiles.get(tid), dict):
        tinfo = vtiles[tid]
        for k in ("source", "identity", "file", "path", "key"):
            val = str(tinfo.get(k) or "").strip()
            if val:
                return val
    vlay = viewer.get("layout")
    if isinstance(vlay, dict):
        ltiles = vlay.get("tiles")
        if isinstance(ltiles, dict) and tid and isinstance(ltiles.get(tid), dict):
            tinfo = ltiles[tid]
            for k in ("source", "identity", "file", "path"):
                val = str(tinfo.get(k) or "").strip()
                if val:
                    return val
    for k in ("source", "identity", "file", "key"):
        val = str(tile.get(k) or "").strip()
        if val:
            return val
    return ""


def _splitter_between(
    splitters: list[dict],
    a_rect: tuple[float, float, float, float],
    b_rect: tuple[float, float, float, float],
    axis: str,
    tol: float = 12.0,
) -> dict | None:
    """Return a splitter with matching `axis` positioned between `a_rect` and `b_rect`, or None."""
    for sp in splitters:
        if not isinstance(sp, dict) or sp.get("axis") != axis:
            continue
        sr = _rect_tuple(sp.get("rect"))
        if sr is None:
            continue
        sx, sy, sw, sh = sr
        scx, scy = sx + sw / 2.0, sy + sh / 2.0
        if axis == "x":
            left, right = (a_rect, b_rect) if a_rect[0] <= b_rect[0] else (b_rect, a_rect)
            if (left[0] + left[2] - tol) <= scx <= (right[0] + tol):
                return sp
        elif axis == "y":
            top, bot = (a_rect, b_rect) if a_rect[1] <= b_rect[1] else (b_rect, a_rect)
            if (top[1] + top[3] - tol) <= scy <= (bot[1] + tol):
                return sp
    return None


def _pane_rect_from_layout(layout: dict | None) -> tuple[float, float, float, float] | None:
    if not isinstance(layout, dict):
        return None
    tb, mn, rl = layout.get("topbar"), layout.get("main"), layout.get("rail")
    if not (isinstance(tb, dict) and isinstance(mn, dict) and isinstance(rl, dict)):
        return None
    try:
        px = float(mn["x"]) + float(mn["w"])
        pw = float(rl["x"]) - px
        py = float(tb["y"]) + float(tb["h"])
        ph = float(mn["h"])
    except (KeyError, TypeError, ValueError):
        return None
    if pw <= 0 or ph <= 0:
        return None
    return px, py, pw, ph


def _shot_cleared(full_dir: Path, sname: str, caps: dict[str, dict], shots: dict[str, dict]) -> tuple[bool, str]:
    """Fail-closed per-shot capture + obstruction/privacy clearance."""
    cap = caps.get(sname) or {}
    png = full_dir / f"{sname}.png"
    if not (cap.get("ok") and png.exists() and png.stat().st_size > 0):
        return False, f"capture missing or failed ({sname})"
    if cap.get("inspect_ok") is not True:
        return False, f"obstruction inspection missing/failed ({sname})"
    if cap.get("obstructed"):
        return False, f"obstructed by native dialog ({sname})"
    if sname not in shots:
        return False, f"shot-ready missing ({sname})"
    return True, ""


def build_task4_dock_steps(settle_timeout_ms: int = 15000) -> list[dict]:
    """Script steps for Task 4 (Gerbers -> Add beside -> Board, then close added tile -> one step back)."""
    tmo = min(settle_timeout_ms, 8000)
    return [
        {"mark": "press:task4-dock-gerbers"},
        {"press": "rail:Gerbers"},
        {"shot": "task4-dock-gerbers", "settle": "gerbers", "timeout_ms": tmo},
        {"mark": "press:task4-dock-split"},
        {"action": ["layout-add", json.dumps({"kind": "board", "side": "right"})]},
        {"wait": 300},
        {"shot": "task4-dock-split", "settle": "board", "timeout_ms": tmo},
        {"mark": "press:task4-dock-back"},
        {"action": ["layout-close", json.dumps({"tile": "focused"})]},
        {"wait": 300},
        {"shot": "task4-dock-back", "settle": "gerbers", "timeout_ms": tmo},
    ]


def build_b7_steps(settle_timeout_ms: int = 15000) -> list[dict]:
    """Script steps for Rule B7 (one split and compare model) on `full`."""
    tmo = min(settle_timeout_ms, 8000)
    steps: list[dict] = []
    # 1. Add beside in all 4 directions (starting from Board)
    for side in ("right", "left", "bottom", "top"):
        steps.extend([
            {"press": "rail:Board"},
            {"wait_settled": "board", "tag": f"b7-add-{side}-start", "timeout_ms": tmo},
            {"mark": f"b7:add-{side}"},
            {"action": ["layout-add", json.dumps({"kind": "schematic", "side": side})]},
            {"wait": 300},
            {"shot": f"b7-add-{side}", "settle": "schematic", "timeout_ms": tmo},
            {"mark": f"b7:close-after-{side}"},
            {"action": ["layout-close", json.dumps({"tile": "focused"})]},
            {"wait": 250},
            {"shot": f"b7-close-{side}", "settle": "board", "timeout_ms": tmo},
        ])
    # 2. Swap + Resize (drag splitter) + Maximize/Restore (from Board + Schematic side by side)
    steps.extend([
        {"press": "rail:Board"},
        {"wait_settled": "board", "tag": "b7-pair-start", "timeout_ms": tmo},
        {"action": ["layout-add", json.dumps({"kind": "schematic", "side": "right"})]},
        {"wait": 300},
        {"shot": "b7-before-swap", "settle": "schematic", "timeout_ms": tmo},
        {"mark": "b7:swap"},
        {"action": ["layout-swap", json.dumps({"a": "t0", "b": "t1"})]},
        {"wait": 300},
        {"shot": "b7-after-swap", "settle": "schematic", "timeout_ms": tmo},
        {"mark": "b7:resize-drag"},
        {"drag": {"from": "splitter:0", "to": [980, 420], "moves": 6}},
        {"wait": 300},
        {"shot": "b7-after-resize", "settle": "schematic", "timeout_ms": tmo},
        {"mark": "b7:maximize"},
        {"action": ["layout-maximize", json.dumps({"tile": "focused"})]},
        {"wait": 250},
        {"shot": "b7-maximized", "settle": "schematic", "timeout_ms": tmo},
        {"mark": "b7:restore"},
        {"action": ["layout-restore", "{}"]},
        {"wait": 250},
        {"shot": "b7-restored", "settle": "schematic", "timeout_ms": tmo},
    ])
    # 3. Overlay on/off (Board + Gerbers) + incompatible overlay refusal (Board + Files)
    steps.extend([
        {"press": "rail:Board"},
        {"wait_settled": "board", "tag": "b7-overlay-base", "timeout_ms": tmo},
        {"shot": "b7-overlay-plain", "settle": "board", "timeout_ms": tmo},
        {"mark": "b7:overlay-on"},
        {"action": ["layout-add", json.dumps({"kind": "gerbers", "side": "right"})]},
        {"wait": 250},
        {"action": ["layout-overlay", json.dumps({"base": "t0", "overlay": "t1", "mode": "diff"})]},
        {"wait": 300},
        {"shot": "b7-overlay-on", "settle": "board", "timeout_ms": tmo},
        {"mark": "b7:overlay-off"},
        {"action": ["layout-overlay", json.dumps({"base": "t0", "overlay": None})]},
        {"wait": 300},
        {"shot": "b7-overlay-off", "settle": "board", "timeout_ms": tmo},
        {"mark": "b7:overlay-incompatible"},
        {"action": ["layout-add", json.dumps({"kind": "files", "side": "right"})]},
        {"wait": 250},
        {"action": ["layout-overlay", json.dumps({"base": "t0", "overlay": "t1", "mode": "diff"})]},
        {"wait": 250},
        {"shot": "b7-overlay-incompat", "settle": "files", "timeout_ms": tmo},
    ])
    return steps


def evaluate_task4_dock(
    full_dir: Path,
    parsed_full: dict,
    caps_full: dict[str, dict],
    full_client_ok: bool,
) -> dict | None:
    """Evaluate Task 4 via the docking tile inventory (`Gerbers -> Add beside -> Board`, + one step back).

    Returns `None` when the build does not emit `GPUI_METRIC event=tiles` or did not run the
    `task4-dock-split` step, so callers fall back cleanly (`unavailable` or legacy check).
    """
    if not parsed_full.get("tiles_metric_seen"):
        return None
    shots = parsed_full.get("shots") or {}
    if "task4-dock-split" not in shots:
        return None

    ok_g, err_g = _shot_cleared(full_dir, "task4-dock-gerbers", caps_full, shots)
    ok_s, err_s = _shot_cleared(full_dir, "task4-dock-split", caps_full, shots)
    ok_b, err_b = _shot_cleared(full_dir, "task4-dock-back", caps_full, shots)

    if not full_client_ok or not ok_s:
        return {
            "outcome": "harness_failed",
            "note": err_s or "client failed or task4-dock-split capture uncleared",
            "one_step_back": False,
            "evidence": {"split_err": err_s, "back_err": err_b},
        }

    s_split = shots["task4-dock-split"]
    tiles = s_split.get("tiles") or []
    splitters = s_split.get("splitters") or []
    v_split = s_split.get("viewer") or {}

    fails: list[str] = []
    if len(tiles) != 2:
        fails.append(f"expected 2 tiles, got {len(tiles)}")
    kinds = {str(t.get("kind") or "") for t in tiles}
    if kinds != {"gerbers", "board"}:
        fails.append(f"expected tile kinds {{'gerbers', 'board'}}, got {sorted(kinds)}")

    rects = [_rect_tuple(t.get("rect")) for t in tiles]
    if any(r is None for r in rects):
        fails.append("one or more tiles missing positive-area rect")
    elif len(rects) == 2 and not _rects_disjoint(rects[0], rects[1]):
        fails.append("tile rects overlap")

    splitter = _splitter_between(splitters, rects[0], rects[1], "x") if (len(rects) == 2 and all(rects)) else None
    if splitter is None:
        fails.append("no vertical ('x') splitter found between Gerbers and Board tiles")

    sources = [_tile_source(t, v_split) for t in tiles]
    if len(sources) == 2:
        if not sources[0] or not sources[1]:
            fails.append(f"tile source missing ({sources})")
        elif sources[0] == sources[1]:
            fails.append(f"both tiles report identical source ({sources[0]!r})")

    bodies = [tile_body_non_flat(full_dir / "task4-dock-split.png", t.get("rect")) for t in tiles]
    if any(b is not True for b in bodies):
        fails.append(f"tile body flat or unreadable ({bodies})")

    # One step back: closing the added tile returns the surviving Gerbers tile to full width
    back_ok = False
    back_note = err_b
    if ok_b:
        s_back = shots["task4-dock-back"]
        b_tiles = s_back.get("tiles") or []
        b_splitters = s_back.get("splitters") or []
        b_rect = _rect_tuple(b_tiles[0].get("rect")) if len(b_tiles) == 1 else None
        ref_w: float | None = None
        if ok_g:
            g_tiles = shots["task4-dock-gerbers"].get("tiles") or []
            if len(g_tiles) == 1:
                gr = _rect_tuple(g_tiles[0].get("rect"))
                if gr:
                    ref_w = gr[2]
        if ref_w is None:
            pr = _pane_rect_from_layout(s_back.get("layout"))
            if pr:
                ref_w = pr[2]
        b_body = tile_body_non_flat(full_dir / "task4-dock-back.png", b_tiles[0].get("rect")) if len(b_tiles) == 1 else None
        if (
            len(b_tiles) == 1
            and str(b_tiles[0].get("kind") or "") == "gerbers"
            and len(b_splitters) == 0
            and b_rect is not None
            and ref_w is not None
            and b_rect[2] >= 0.95 * ref_w
            and b_body is True
        ):
            back_ok = True
            back_note = ""
        else:
            back_note = (
                f"close did not restore full-width Gerbers tile "
                f"(tiles={len(b_tiles)}, splitters={len(b_splitters)}, w={b_rect[2] if b_rect else None}, ref_w={ref_w}, body={b_body})"
            )

    outcome = "passed" if (not fails and back_ok) else "task_failed"
    return {
        "outcome": outcome,
        "note": "; ".join(fails + ([back_note] if (back_note and not back_ok) else [])),
        "one_step_back": back_ok,
        "evidence": {
            "tiles": tiles,
            "splitters": splitters,
            "sources": sources,
            "bodies_non_flat": bodies,
            "one_step_back_ok": back_ok,
            "one_step_back_note": back_note,
        },
    }


def evaluate_b7_dock(
    full_dir: Path,
    parsed_full: dict,
    caps_full: dict[str, dict],
) -> dict:
    """Evaluate Rule B7 (one split and compare model) from tile inventory, drag events, refusals, and PNGs.

    Returns `status: "unavailable"` until a build emits `GPUI_METRIC event=tiles`.
    """
    if not parsed_full.get("tiles_metric_seen"):
        return {
            "status": "unavailable",
            "reason": "client did not emit GPUI_METRIC event=tiles (requires docking PR #1414 + renderer)",
            "checked": 0,
            "clean": 0,
            "violations": [],
            "checks": {k: {"status": "unavailable", "passed": None, "note": "no tiles metric"} for k in DOCK_B7_CHECKS},
        }

    shots = parsed_full.get("shots") or {}
    drags = parsed_full.get("drags") or {}
    refusals = parsed_full.get("refusals") or {}
    checks: dict[str, dict] = {}

    def _rec(status: str, passed: bool | None, note: str = "", evidence: dict | None = None) -> dict:
        return {"status": status, "passed": passed, "note": note, "evidence": evidence or {}}

    # 1. add_beside_directions (right, left, bottom, top)
    dir_results: dict[str, bool | str] = {}
    dir_notes: list[str] = []
    for side, axis in (("right", "x"), ("left", "x"), ("bottom", "y"), ("top", "y")):
        sname = f"b7-add-{side}"
        if sname not in shots:
            dir_results[side] = "unavailable"
            continue
        ok, err = _shot_cleared(full_dir, sname, caps_full, shots)
        if not ok:
            dir_results[side] = "unavailable"
            dir_notes.append(f"{side}: {err}")
            continue
        ts = shots[sname].get("tiles") or []
        sps = shots[sname].get("splitters") or []
        if len(ts) != 2:
            dir_results[side] = False
            dir_notes.append(f"{side}: expected 2 tiles, got {len(ts)}")
            continue
        by_kind = {str(t.get("kind") or ""): t for t in ts}
        if "board" not in by_kind or "schematic" not in by_kind:
            dir_results[side] = False
            dir_notes.append(f"{side}: unexpected kinds {sorted(by_kind)}")
            continue
        rb = _rect_tuple(by_kind["board"].get("rect"))
        rs = _rect_tuple(by_kind["schematic"].get("rect"))
        if not (rb and rs and _rects_disjoint(rb, rs)):
            dir_results[side] = False
            dir_notes.append(f"{side}: invalid or overlapping rects")
            continue
        pos_ok = (
            (side == "right" and rs[0] > rb[0])
            or (side == "left" and rs[0] < rb[0])
            or (side == "bottom" and rs[1] > rb[1])
            or (side == "top" and rs[1] < rb[1])
        )
        sp_ok = _splitter_between(sps, rb, rs, axis) is not None
        b1 = tile_body_non_flat(full_dir / f"{sname}.png", by_kind["board"].get("rect"))
        b2 = tile_body_non_flat(full_dir / f"{sname}.png", by_kind["schematic"].get("rect"))
        if pos_ok and sp_ok and b1 is True and b2 is True:
            dir_results[side] = True
        else:
            dir_results[side] = False
            dir_notes.append(f"{side}: pos_ok={pos_ok} sp_ok={sp_ok} bodies=({b1},{b2})")

    if any(v is False for v in dir_results.values()):
        checks["add_beside_directions"] = _rec("measured", False, "; ".join(dir_notes), {"directions": dir_results})
    elif all(v is True for v in dir_results.values()):
        checks["add_beside_directions"] = _rec("measured", True, "", {"directions": dir_results})
    else:
        checks["add_beside_directions"] = _rec("unavailable", None, "; ".join(dir_notes) or "not all 4 directions captured", {"directions": dir_results})

    # 2. close (survivor restored to full pane after closing added tile)
    close_sname = next((f"b7-close-{s}" for s in ("right", "left", "bottom", "top") if f"b7-close-{s}" in shots), None)
    if close_sname is None:
        checks["close"] = _rec("unavailable", None, "no b7-close-* shot captured")
    else:
        ok, err = _shot_cleared(full_dir, close_sname, caps_full, shots)
        if not ok:
            checks["close"] = _rec("unavailable", None, err)
        else:
            srec = shots[close_sname]
            ts = srec.get("tiles") or []
            sps = srec.get("splitters") or []
            pr = _pane_rect_from_layout(srec.get("layout"))
            tr = _rect_tuple(ts[0].get("rect")) if len(ts) == 1 else None
            body = tile_body_non_flat(full_dir / f"{close_sname}.png", ts[0].get("rect")) if len(ts) == 1 else None
            width_ok = bool(tr and (pr is None or tr[2] >= 0.95 * pr[2]))
            ok_close = len(ts) == 1 and len(sps) == 0 and width_ok and body is True
            checks["close"] = _rec(
                "measured",
                ok_close,
                "" if ok_close else f"tiles={len(ts)} splitters={len(sps)} width_ok={width_ok} body={body}",
                {"shot": close_sname, "tiles": ts, "splitters": sps},
            )

    # 3. swap (two tiles exchange rects and pixels change)
    if "b7-before-swap" not in shots or "b7-after-swap" not in shots:
        checks["swap"] = _rec("unavailable", None, "swap shots not captured")
    else:
        ok1, e1 = _shot_cleared(full_dir, "b7-before-swap", caps_full, shots)
        ok2, e2 = _shot_cleared(full_dir, "b7-after-swap", caps_full, shots)
        if not (ok1 and ok2):
            checks["swap"] = _rec("unavailable", None, e1 or e2)
        else:
            t_pre = {str(t.get("kind") or t.get("id")): _rect_tuple(t.get("rect")) for t in (shots["b7-before-swap"].get("tiles") or [])}
            t_post = {str(t.get("kind") or t.get("id")): _rect_tuple(t.get("rect")) for t in (shots["b7-after-swap"].get("tiles") or [])}
            common = sorted(k for k in (set(t_pre) & set(t_post)) if t_pre[k] and t_post[k])
            if len(common) != 2:
                checks["swap"] = _rec("measured", False, f"expected 2 matching tiles across swap, got {common}")
            else:
                k0, k1 = common
                swapped = _rect_close(t_pre[k0], t_post[k1]) and _rect_close(t_pre[k1], t_post[k0]) and not _rect_close(t_pre[k0], t_post[k0])
                diff0 = tile_region_diff(full_dir / "b7-before-swap.png", full_dir / "b7-after-swap.png", shots["b7-after-swap"]["tiles"][0].get("rect"))
                pix_ok = (diff0 is not None and diff0 > 0.01)
                ok_swap = bool(swapped and pix_ok)
                checks["swap"] = _rec(
                    "measured",
                    ok_swap,
                    "" if ok_swap else f"swapped_rects={swapped} pixel_diff={diff0}",
                    {"before": t_pre, "after": t_post, "pixel_diff": diff0},
                )

    # 4. resize_drag (drag splitter ok=true, ratio changes >= 0.05, both bodies redraw non-flat at new size)
    if "b7-after-swap" not in shots or "b7-after-resize" not in shots:
        checks["resize_drag"] = _rec("unavailable", None, "resize shots not captured")
    else:
        ok1, e1 = _shot_cleared(full_dir, "b7-after-swap", caps_full, shots)
        ok2, e2 = _shot_cleared(full_dir, "b7-after-resize", caps_full, shots)
        drag_evs = [d for d in drags.get("b7:resize-drag", []) if d.get("ok") is True]
        if not (ok1 and ok2):
            checks["resize_drag"] = _rec("unavailable", None, e1 or e2)
        elif not drag_evs:
            checks["resize_drag"] = _rec("unavailable", None, "no delivered drag event (ok=true) recorded for b7:resize-drag")
        else:
            pre_ts = sorted([r for t in (shots["b7-after-swap"].get("tiles") or []) if (r := _rect_tuple(t.get("rect")))], key=lambda r: r[0])
            post_ts_raw = [t for t in (shots["b7-after-resize"].get("tiles") or []) if _rect_tuple(t.get("rect"))]
            post_ts = sorted([(_rect_tuple(t.get("rect")), t) for t in post_ts_raw], key=lambda pair: pair[0][0])
            if len(pre_ts) != 2 or len(post_ts) != 2:
                checks["resize_drag"] = _rec("measured", False, f"expected 2 tiles before/after resize ({len(pre_ts)} -> {len(post_ts)})")
            else:
                r_pre = pre_ts[0][2] / (pre_ts[0][2] + pre_ts[1][2])
                r_post = post_ts[0][0][2] / (post_ts[0][0][2] + post_ts[1][0][2])
                ratio_delta = abs(r_post - r_pre)
                bodies = [tile_body_non_flat(full_dir / "b7-after-resize.png", t.get("rect")) for _, t in post_ts]
                diffs = [tile_region_diff(full_dir / "b7-after-swap.png", full_dir / "b7-after-resize.png", t.get("rect")) for _, t in post_ts]
                redrawn = all(b is True for b in bodies) and all(d is not None and d > 0.005 for d in diffs)
                ok_rsz = ratio_delta >= 0.05 and redrawn
                checks["resize_drag"] = _rec(
                    "measured",
                    ok_rsz,
                    "" if ok_rsz else f"ratio {r_pre:.3f}->{r_post:.3f} (delta={ratio_delta:.3f}), bodies={bodies}, diffs={diffs}",
                    {"ratio_before": round(r_pre, 3), "ratio_after": round(r_post, 3), "bodies": bodies, "diffs": diffs},
                )

    # 5. maximize_restore (maximize -> 1 full-pane tile, 0 splitters; restore -> 2 tiles + splitter)
    if "b7-maximized" not in shots or "b7-restored" not in shots:
        checks["maximize_restore"] = _rec("unavailable", None, "maximize/restore shots not captured")
    else:
        ok1, e1 = _shot_cleared(full_dir, "b7-maximized", caps_full, shots)
        ok2, e2 = _shot_cleared(full_dir, "b7-restored", caps_full, shots)
        if not (ok1 and ok2):
            checks["maximize_restore"] = _rec("unavailable", None, e1 or e2)
        else:
            sm, sr = shots["b7-maximized"], shots["b7-restored"]
            tm, spm = sm.get("tiles") or [], sm.get("splitters") or []
            tr, spr = sr.get("tiles") or [], sr.get("splitters") or []
            pm = _pane_rect_from_layout(sm.get("layout"))
            rm = _rect_tuple(tm[0].get("rect")) if len(tm) == 1 else None
            max_ok = (
                len(tm) == 1
                and len(spm) == 0
                and rm is not None
                and (pm is None or (rm[2] >= 0.95 * pm[2] and rm[3] >= 0.95 * pm[3]))
                and tile_body_non_flat(full_dir / "b7-maximized.png", tm[0].get("rect")) is True
            )
            res_rects = [_rect_tuple(t.get("rect")) for t in tr]
            res_ok = (
                len(tr) == 2
                and all(res_rects)
                and _rects_disjoint(res_rects[0], res_rects[1])
                and len(spr) >= 1
                and all(tile_body_non_flat(full_dir / "b7-restored.png", t.get("rect")) is True for t in tr)
            )
            ok_mr = bool(max_ok and res_ok)
            checks["maximize_restore"] = _rec(
                "measured",
                ok_mr,
                "" if ok_mr else f"max_ok={max_ok} (tiles={len(tm)}, splitters={len(spm)}), res_ok={res_ok} (tiles={len(tr)}, splitters={len(spr)})",
            )

    # 6. overlay_on_off (Board + Gerbers overlay on -> overlay present + pixels differ; off -> overlay cleared)
    if not all(k in shots for k in ("b7-overlay-plain", "b7-overlay-on", "b7-overlay-off")):
        checks["overlay_on_off"] = _rec("unavailable", None, "overlay shots not captured")
    else:
        ok0, e0 = _shot_cleared(full_dir, "b7-overlay-plain", caps_full, shots)
        ok1, e1 = _shot_cleared(full_dir, "b7-overlay-on", caps_full, shots)
        ok2, e2 = _shot_cleared(full_dir, "b7-overlay-off", caps_full, shots)
        if not (ok0 and ok1 and ok2):
            checks["overlay_on_off"] = _rec("unavailable", None, e0 or e1 or e2)
        else:
            s_on, s_off = shots["b7-overlay-on"], shots["b7-overlay-off"]
            t_on, t_off = s_on.get("tiles") or [], s_off.get("tiles") or []
            v_on, v_off = s_on.get("viewer") or {}, s_off.get("viewer") or {}

            def _has_gerber_overlay(v: dict, ts: list[dict]) -> bool:
                if v.get("gerbers_vs_board") is True:
                    return True
                vtiles = v.get("tiles") if isinstance(v.get("tiles"), dict) else {}
                for t in list(vtiles.values()) + ts:
                    if isinstance(t, dict) and isinstance(t.get("overlay"), dict) and t["overlay"].get("kind") == "gerbers":
                        return True
                return False

            on_flag = _has_gerber_overlay(v_on, t_on)
            off_cleared = not _has_gerber_overlay(v_off, t_off)
            on_body = tile_body_non_flat(full_dir / "b7-overlay-on.png", t_on[0].get("rect")) if t_on else None
            off_body = tile_body_non_flat(full_dir / "b7-overlay-off.png", t_off[0].get("rect")) if t_off else None
            ov_diff = tile_region_diff(full_dir / "b7-overlay-plain.png", full_dir / "b7-overlay-on.png", t_on[0].get("rect")) if t_on else None
            ok_ov = bool(on_flag and off_cleared and on_body is True and off_body is True and (ov_diff is not None and ov_diff > 0.005))
            checks["overlay_on_off"] = _rec(
                "measured",
                ok_ov,
                "" if ok_ov else f"on_flag={on_flag} off_cleared={off_cleared} bodies=({on_body},{off_body}) diff={ov_diff}",
                {"overlay_diff": ov_diff},
            )

    # 7. incompatible_overlay_refusal (Board + Files overlay refused in human words, no raw codes, layout unchanged)
    if "b7-overlay-incompat" not in shots:
        checks["incompatible_overlay_refusal"] = _rec("unavailable", None, "incompatible overlay shot not captured")
    else:
        ok, err = _shot_cleared(full_dir, "b7-overlay-incompat", caps_full, shots)
        if not ok:
            checks["incompatible_overlay_refusal"] = _rec("unavailable", None, err)
        else:
            s_inc = shots["b7-overlay-incompat"]
            v_inc = s_inc.get("viewer") or {}
            ts_inc = s_inc.get("tiles") or []
            ref_texts = [str(r.get("text") or "").strip() for r in refusals.get("b7:overlay-incompatible", []) if str(r.get("text") or "").strip()]
            v_ref = str(v_inc.get("refusal") or v_inc.get("error") or "").strip()
            if v_ref:
                ref_texts.append(v_ref)
            msg = ref_texts[-1] if ref_texts else ""
            raw_code = bool(RAW_REFUSAL_CODE_RE.search(msg)) if msg else False
            any_overlay = any(isinstance(t.get("overlay"), dict) for t in ts_inc)
            ok_ref = bool(msg and not raw_code and len(ts_inc) == 2 and not any_overlay)
            checks["incompatible_overlay_refusal"] = _rec(
                "measured",
                ok_ref,
                "" if ok_ref else f"refusal_text={msg!r} raw_code={raw_code} tiles={len(ts_inc)} any_overlay={any_overlay}",
                {"refusal_text": msg, "tiles": len(ts_inc)},
            )

    measured = [(k, c) for k, c in checks.items() if c["status"] == "measured" and isinstance(c["passed"], bool)]
    if not measured:
        return {
            "status": "unavailable",
            "reason": "tiles metric seen, but no B7 docking transitions were captured",
            "checked": 0,
            "clean": 0,
            "violations": [],
            "checks": checks,
        }
    clean = sum(1 for _, c in measured if c["passed"] is True)
    violations = [f"{k}: {c['note'] or 'failed'}" for k, c in measured if c["passed"] is False]
    all_measured = len(measured) == len(DOCK_B7_CHECKS)
    return {
        "status": "measured" if all_measured else "measured_partial",
        "reason": "",
        "checked": len(measured),
        "clean": clean,
        "violations": violations,
        "checks": checks,
    }
