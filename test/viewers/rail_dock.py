#!/usr/bin/env python3
r"""Docking/tiling measurement helpers for Scorecard Task 4 and Rule B7.

## Required Renderer & Script Contract (for Tile Renderer / Worker A)

Every assertion in Task 4 and Rule B7 reads ONLY from the metrics below and cleared screenshots
(never guessed header heights, never stale carried-forward inventories):

| Event / Source | Field | Type | Meaning |
|---|---|---|---|
| `GPUI_METRIC {"event":"tiles", ...}` | `tiles` | `list[dict]` | Currently painted tiles; emitted on every tile/layout/frame change. |
| `tiles[]` | `id` | `str` | Stable hub `TileId` (matches leaf `tile.id` under `viewer.layout.root`). |
| `tiles[]` | `kind` | `str` | Page kind (`board`, `schematic`, `gerbers`, `3d`, `mech`, `files`, `diff`, `review`, `browser`, `simulation`). |
| `tiles[]` | `source` | `str` | Non-empty tile source/identity shown by this tile (e.g. `mfg/gerbers`, `ecc83-pp.kicad_pcb`). |
| `tiles[]` | `rect` | `{"x","y","w","h": float}` | Painted outer tile bounds in screenshot coordinates (positive area). |
| `tiles[]` | `body` | `{"x","y","w","h": float}` | Measured painted body bounds BELOW the tile header in screenshot coordinates (positive area). |
| `tiles[]` | `frame` | `int` | Monotonic per-tile presented-frame sequence (advances whenever the tile redraws for a new viewport/source/overlay). |
| `tiles[]` | `focused` | `bool` | Whether this tile currently owns focus. |
| `tiles[]` | `overlay` | `dict \| null` | Explicit overlay state on this tile: `{"kind": "gerbers", ...}` when active, `null` when off (key must be present to observe overlay off). |
| `GPUI_METRIC {"event":"tiles", ...}` | `splitters` | `list[dict]` | Currently painted splitters between tiles. |
| `tiles` event | `refusal_metric` | `bool` | `true` when this build emits the `refusal` metric for every hub refusal; without it a missing refusal after a delivered overlay press is `unavailable`, with it a failure. |
| `splitters[]` | `id` | `str` | Stable hub `SplitId` (matches split node id under `viewer.layout.root`). |
| `splitters[]` | `rect` | `{"x","y","w","h": float}` | Painted splitter bounds in screenshot coordinates (positive area). |
| `splitters[]` | `axis` | `"x" \| "y"` | Renderer axis translated from hub `Axis`: hub `"row"` -> `"x"`, hub `"col"` -> `"y"`. |
| `GPUI_METRIC {"event":"drag", ...}` | `ok` | `bool` (strict) | Boolean `true` iff the pointer drag was dispatched (string `"false"` is rejected). |
| `GPUI_METRIC {"event":"drag", ...}` | `splitter` | `str` | Control / splitter id targeted by the drag. |
| `GPUI_METRIC {"event":"drag", ...}` | `from`, `to` | `[float, float]` | Start and end pointer coordinates in window pixels (must move along splitter axis). |
| `GPUI_METRIC {"event":"refusal", ...}` | `code`, `text` | `str`, `str` | Catalog refusal code (e.g. `incompatibleOverlay` / `viewer.layout.refusal.incompatibleOverlay`) and localized human words (no raw internal codes). |
| `viewer.layout` (hub picture, in the `viewer` block of settled/shot metrics) | `root` | `dict` | Hub `Layout` (`native/host/src/present/layout.rs`): `{"type":"tile","id","kind","source","overlay"}` leaves and `{"type":"split","id","axis":"row"\|"col","ratio","a","b"}`. No other shape is read (no `tiles` list, no `leaf`/`tile` wrappers). |
| `crate::script::tiles_metric(started, tiles, splitters)` | call | fn | The renderer calls this on every tile/layout/frame change: it emits the `tiles` event AND stores the inventory that script selectors resolve against. |
| `crate::script::set_splitter_drag(f)` | call | `Fn(&str id, [f32;2] from, [f32;2] to, u32 moves) -> bool` | The renderer registers how a scripted drag is delivered as a real pointer drag on splitter `id`; returns whether it was dispatched. Without it the `drag` step reports `ok:false, error:"no_drag_handler"`. |
| script step `press` with a selector | `@tile[kind=K,source=S,focused\|unfocused].VERB` | str | Resolved by the client from the drawn inventory (exactly one match, else `found:false`) to control id `tile:<tile id>:VERB`. The renderer registers these controls (`Ctl` entries): `add-beside`, `close`, `swap` (with the other tile), `maximize`, `restore`, `overlay-gerbers` (toggle). The `press` metric carries the resolved `id` and the `selector`. |
| script step `drag` | `{"drag":{"splitter":"@splitter[axis=x]"\|"<id>","dx":N,"dy":N,"moves":N}}` | obj | Drags the drawn splitter along its axis (`dx` for `x`, `dy` for `y`); emits `drag` with `ok`, `splitter`, `axis`, `from`, `to`, `moves`. |
| menu entries after `add-beside` | `add-beside:<kind>` / `add-beside:<kind>:<right\|left\|bottom\|top>` | control ids | Page and side entries (kind and side are fixed words, not tile ids). `add-beside:<kind>` means right. |
| `GPUI_METRIC {"event":"refusal"}` | emitted when the hub refuses a layout change | event | After `overlay-gerbers` on the Files tile: `code` is `incompatibleOverlay`, `text` human words. The refusal must leave `viewer.layout` unchanged. |

| splitter drag hook | `set_splitter_drag` | real pointer path | The hook must deliver a pointer press, moves and release at the splitter's drawn rect THROUGH the window's hit-testing (as the `pointer` step does for rail rows); calling the hub resize or setting a ratio directly is not a drag and makes `resize_drag` meaningless. |
| incompatible overlay | `@tile[kind=files].overlay-gerbers` | control offered by the UI | Exercised only through a control the product really draws. If the UI never offers an overlay on a Files tile, the press is not delivered and `incompatible_overlay_refusal` is `unavailable` BY DESIGN (never a pass); to measure it the renderer must either offer that control and let the hub refuse, or the driver is changed to another pair the UI offers and the hub refuses. |
| `viewer.layout` before/after the refusal | `viewer_state_json` forwards the hub's `viewer.layout` untouched into the `viewer` block of `settled`/`shot-ready` metrics | tree | Starting tree must be exactly Board + Files; layouts before and after must be equal; two empty dicts are not "equal". |

Task 4 route: `rail:Gerbers` -> `@tile[kind=gerbers].add-beside` -> `add-beside:board` (the exact ordered ids, each delivered) -> (one step back) `@tile[kind=board].close`.
Freshness: each shot's tile inventory must carry a `tiles` event whose seq is after the last DELIVERED input (press or drag) of its transition.

Every check stays `unavailable` until a build emits `GPUI_METRIC {"event":"tiles", ...}` and
draws real multi-tile layouts. Every visual assertion enforces per-shot privacy/obstruction
clearance (fail-closed on missing/failed `inspect_ok`, native dialogs, or uncleared shots).
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

HUB_AXIS_TO_RENDERER = {
    "row": "x",
    "col": "y",
}

INCOMPATIBLE_OVERLAY_CODES = {
    "incompatibleOverlay",
    "viewer.layout.refusal.incompatibleOverlay",
    "incompatible_overlay",
}

RAW_REFUSAL_CODE_RE = re.compile(
    r"\b(ERR_[A-Z0-9_]+|INTERNAL_[A-Z0-9_]+|PRESENTER_[A-Z0-9_]+|ECONNREFUSED|ENOENT|HTTP\s+5\d\d|Refusal::|panic|backtrace|incompatibleOverlay)\b"
)


def axis_from_hub(hub_axis: str | None) -> str | None:
    """Translate hub `layout::Axis` (`row`/`col`) explicitly to renderer splitter axis (`x`/`y`)."""
    if not isinstance(hub_axis, str):
        return None
    return HUB_AXIS_TO_RENDERER.get(hub_axis.lower())


def layout_leaves(layout: dict | None) -> dict[str, dict]:
    """Hub `viewer.layout` (`native/host/src/present/layout.rs`): `{tile_id: tile}` for the `{"type":"tile",...}` leaves under `root`.

    The hub's tree is `Node::Tile {id, kind, source, overlay}` and `Node::Split {id, axis: "row"|"col", ratio, a, b}`
    (serde tag `type`); nothing else is accepted, so a build that sends another shape earns nothing here.
    """
    out: dict[str, dict] = {}
    if not isinstance(layout, dict):
        return out

    def _walk(node: object) -> None:
        if not isinstance(node, dict):
            return
        if node.get("type") == "tile" and node.get("id"):
            out[str(node["id"])] = node
        elif node.get("type") == "split":
            _walk(node.get("a"))
            _walk(node.get("b"))

    _walk(layout.get("root"))
    return out


def layout_splits(layout: dict | None) -> dict[str, dict]:
    """`{split_id: {"axis": "x"|"y", "ratio": float}}` for the hub's `Node::Split`s, `row` -> `x` and `col` -> `y` translated explicitly."""
    out: dict[str, dict] = {}
    if not isinstance(layout, dict):
        return out

    def _walk(node: object) -> None:
        if isinstance(node, dict) and node.get("type") == "split":
            out[str(node.get("id") or "")] = {"axis": axis_from_hub(node.get("axis")), "ratio": node.get("ratio")}
            _walk(node.get("a"))
            _walk(node.get("b"))

    _walk(layout.get("root"))
    return out


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


def tile_body_non_flat(img_path: Path, body_rect: dict | None) -> bool | None:
    """Return True if the measured `body` rect (below header, from metric) is non-flat, False if flat, None if missing/unreadable."""
    rt = _rect_tuple(body_rect)
    if rt is None or not img_path.exists():
        return None
    x, y, w, h = rt
    with Image.open(img_path) as im:
        im = im.convert("RGB")
        iw, ih = im.size
        x0 = max(0, min(iw, int(round(x + 2))))
        y0 = max(0, min(ih, int(round(y + 2))))
        x1 = max(x0, min(iw, int(round(x + w - 2))))
        y1 = max(y0, min(ih, int(round(y + h - 2))))
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


def tile_bodies_pixel_diff(img_a: Path, rect_a: dict | None, img_b: Path, rect_b: dict | None) -> float | None:
    """Compare two measured body rects (same or different PNGs) resized to a common grid; return fraction of differing pixels."""
    ra = _rect_tuple(rect_a)
    rb = _rect_tuple(rect_b)
    if ra is None or rb is None or not img_a.exists() or not img_b.exists():
        return None
    with Image.open(img_a) as ia, Image.open(img_b) as ib:
        ia = ia.convert("RGB")
        ib = ib.convert("RGB")
        def _crop(im: Image.Image, r: tuple[float, float, float, float]) -> Image.Image | None:
            x, y, w, h = r
            iw, ih = im.size
            x0 = max(0, min(iw, int(round(x + 2))))
            y0 = max(0, min(ih, int(round(y + 2))))
            x1 = max(x0, min(iw, int(round(x + w - 2))))
            y1 = max(y0, min(ih, int(round(y + h - 2))))
            if x1 - x0 < 12 or y1 - y0 < 12:
                return None
            return im.crop((x0, y0, x1, y1)).resize((160, 160))
        ca = _crop(ia, ra)
        cb = _crop(ib, rb)
        if ca is None or cb is None:
            return None
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
    """A tile's own source: the tile metric's `source`, else the hub layout leaf (`viewer.layout.root`) with the same id."""
    val = str(tile.get("source") or "").strip()
    if val:
        return val
    leaf = layout_leaves(viewer.get("layout")).get(str(tile.get("id") or ""))
    return str((leaf or {}).get("source") or "").strip()


def _splitter_between(
    splitters: list[dict],
    a_rect: tuple[float, float, float, float],
    b_rect: tuple[float, float, float, float],
    axis: str,
    tol: float = 12.0,
) -> dict | None:
    """Return a splitter with matching renderer `axis` ('x'/'y') positioned between `a_rect` and `b_rect`, or None."""
    for sp in splitters:
        if not isinstance(sp, dict):
            continue
        if sp.get("axis") != axis:
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


def _shot_cleared(
    full_dir: Path,
    sname: str,
    caps: dict[str, dict],
    shots: dict[str, dict],
    cleared_shots: set[str] | None = None,
) -> tuple[bool, str]:
    """Fail-closed per-shot capture + obstruction/privacy clearance (consulting `cleared_shots` when provided)."""
    if cleared_shots is not None and sname not in cleared_shots:
        return False, f"shot not in run's per-shot privacy clearance ({sname})"
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


def _fresh_tiles_after_mark(shot: dict, mark_seqs: dict[str, int], mark_name: str, events: list[dict] | None = None) -> tuple[bool, str]:
    """A tile inventory drawn strictly AFTER the transition: after the last delivered event (press, drag, refusal) of its mark.

    With no delivered event the transition never happened, so nothing is fresh; the mark alone is not enough.
    """
    mseq = mark_seqs.get(mark_name)
    tseq = shot.get("tiles_seq")
    if not isinstance(mseq, int) or mseq <= 0:
        return False, f"mark {mark_name!r} not recorded"
    delivered = [int(e.get("seq") or 0) for e in (events or []) if e.get("found", e.get("ok")) is True]
    if not delivered:
        return False, f"no delivered input for {mark_name!r}"
    after = max(delivered)
    if not isinstance(tseq, int) or tseq <= after:
        return False, f"stale tile inventory (tiles_seq={tseq} <= delivered input seq={after} for {mark_name!r})"
    return True, ""


def _chain(presses: dict[str, list[dict]], mark: str) -> list[dict]:
    """The presses of one transition's mark, only when EVERY one was delivered (a half-delivered route is not a transition)."""
    evs = presses.get(mark) or []
    return evs if evs and all(e.get("found") is True for e in evs) else []


def _reset_idle(tag: str, tmo: int) -> list[dict]:
    """Back to no open page, so a following `rail:Board` opens Board (pressed on a focused Board it would close it)."""
    return [
        {"keys": "ctrl-l"},
        {"wait": 60},
        {"keys": "escape"},
        {"wait": 60},
        {"keys": "escape"},
        {"wait": 60},
        {"wait_settled": "", "tag": tag, "timeout_ms": min(tmo, 3000)},
    ]


def _add_beside(kind_sel: str, kind: str, side: str | None = None) -> list[dict]:
    """The user route: the tile's Add beside control, then the menu entry for the page (and side). Tile ids come from the drawn inventory."""
    entry = f"add-beside:{kind}" + (f":{side}" if side else "")
    return [{"press": f"@tile[{kind_sel}].add-beside"}, {"wait": 120}, {"press": entry}, {"wait": 300}]


def _exact_chain(presses: dict[str, list[dict]], mark: str, expected: list[str]) -> list[dict]:
    """The presses of one mark, only when their ids are EXACTLY `expected`, in order, each delivered (executed)."""
    evs = presses.get(mark) or []
    ok = [e.get("id") for e in evs] == expected and all(e.get("found") is True for e in evs)
    return evs if ok else []


def build_task4_dock_steps(settle_timeout_ms: int = 15000) -> list[dict]:
    """User-facing Task 4: Gerbers -> Add beside... -> Board in the UI, then close the added tile (one step back).

    No direct hub `layout-*` action; tile ids are selectors resolved by the client from the drawn inventory
    (`@tile[kind=gerbers].add-beside` -> `tile:<real id>:add-beside`). Resets to idle first so `rail:Gerbers` never toggles Gerbers off.
    """
    tmo = min(settle_timeout_ms, 8000)
    return [
        *_reset_idle("task4-dock-start-idle", tmo),
        {"mark": "press:task4-dock-gerbers"},
        {"press": "rail:Gerbers"},
        {"shot": "task4-dock-gerbers", "settle": "gerbers", "timeout_ms": tmo},
        {"mark": "press:task4-dock-split"},
        *_add_beside("kind=gerbers", "board"),
        {"shot": "task4-dock-split", "settle": "board", "timeout_ms": tmo},
        {"mark": "press:task4-dock-back"},
        {"press": "@tile[kind=board].close"},
        {"wait": 300},
        {"shot": "task4-dock-back", "settle": "gerbers", "timeout_ms": tmo},
    ]


def build_b7_steps(settle_timeout_ms: int = 15000) -> list[dict]:
    """Rule B7 (one split and compare model) steps on `full`, all through the UI controls.

    Idle reset before every trial (never `rail:Board` on a focused Board). Tiles and the splitter are selectors the client resolves
    from the drawn inventory; the drag is a pointer drag on that splitter (`ok` only if one was dispatched).
    """
    tmo = min(settle_timeout_ms, 8000)
    steps: list[dict] = []
    for side in ("right", "left", "bottom", "top"):
        steps += _reset_idle(f"b7-add-{side}-idle", tmo)
        steps += [
            {"press": "rail:Board"},
            {"wait_settled": "board", "tag": f"b7-add-{side}-start", "timeout_ms": tmo},
            {"mark": f"b7:add-{side}"},
            *_add_beside("kind=board", "schematic", side),
            {"shot": f"b7-add-{side}", "settle": "schematic", "timeout_ms": tmo},
            {"mark": f"b7:close-{side}"},
            {"press": "@tile[kind=schematic].close"},
            {"wait": 250},
            {"shot": f"b7-close-{side}", "settle": "board", "timeout_ms": tmo},
        ]
    steps += _reset_idle("b7-pair-idle", tmo)
    steps += [
        {"press": "rail:Board"},
        {"wait_settled": "board", "tag": "b7-pair-start", "timeout_ms": tmo},
        {"mark": "b7:pair"},
        *_add_beside("kind=board", "schematic", "right"),
        {"shot": "b7-before-swap", "settle": "schematic", "timeout_ms": tmo},
        {"mark": "b7:swap"},
        {"press": "@tile[kind=schematic].swap"},
        {"wait": 300},
        {"shot": "b7-after-swap", "settle": "schematic", "timeout_ms": tmo},
        {"mark": "b7:resize-drag"},
        {"drag": {"splitter": "@splitter[axis=x]", "dx": -90, "moves": 6}},
        {"wait": 300},
        {"shot": "b7-after-resize", "settle": "schematic", "timeout_ms": tmo},
        {"mark": "b7:maximize"},
        {"press": "@tile[kind=schematic].maximize"},
        {"wait": 250},
        {"shot": "b7-maximized", "settle": "schematic", "timeout_ms": tmo},
        {"mark": "b7:restore"},
        {"press": "@tile[kind=schematic].restore"},
        {"wait": 250},
        {"shot": "b7-restored", "settle": "schematic", "timeout_ms": tmo},
    ]
    steps += _reset_idle("b7-overlay-idle", tmo)
    steps += [
        {"press": "rail:Board"},
        {"wait_settled": "board", "tag": "b7-overlay-base", "timeout_ms": tmo},
        {"shot": "b7-overlay-plain", "settle": "board", "timeout_ms": tmo},
        {"mark": "b7:overlay-on"},
        {"press": "@tile[kind=board].overlay-gerbers"},
        {"wait": 300},
        {"shot": "b7-overlay-on", "settle": "board", "timeout_ms": tmo},
        {"mark": "b7:overlay-off"},
        {"press": "@tile[kind=board].overlay-gerbers"},
        {"wait": 300},
        {"shot": "b7-overlay-off", "settle": "board", "timeout_ms": tmo},
        {"mark": "b7:overlay-files"},
        *_add_beside("kind=board", "files", "right"),
        {"shot": "b7-overlay-incompat-before", "settle": "files", "timeout_ms": tmo},
        {"mark": "b7:overlay-incompatible"},
        {"press": "@tile[kind=files].overlay-gerbers"},
        {"wait": 250},
        {"shot": "b7-overlay-incompat", "settle": "files", "timeout_ms": tmo},
    ]
    return steps


def evaluate_task4_dock(
    full_dir: Path,
    parsed_full: dict,
    caps_full: dict[str, dict],
    full_client_ok: bool,
    cleared_shots: set[str] | None = None,
) -> dict | None:
    """Evaluate Task 4 via the docking tile inventory (`Gerbers -> Add beside -> Board`, + one step back).

    Returns `None` when the build does not emit `GPUI_METRIC event=tiles` or did not capture
    `task4-dock-split`, so callers stay `unavailable` / legacy until a build draws tiles.
    """
    if not parsed_full.get("tiles_metric_seen"):
        return None
    shots = parsed_full.get("shots") or {}
    if "task4-dock-split" not in shots:
        return None

    mark_seqs = parsed_full.get("mark_seqs") or {}
    presses = parsed_full.get("presses") or {}

    # The inventory predates the Add beside control. A single painted Gerbers
    # tile and a measured missing control establish that this route is unavailable.
    start = shots.get("task4-dock-gerbers", {})
    tiles = start.get("tiles") or []
    if full_client_ok and start.get("settled") and start.get("controls_captured") and len(tiles) == 1 and tiles[0].get("kind") == "gerbers":
        control = f"tile:{tiles[0]['id']}:add-beside"
        attempts = presses.get("press:task4-dock-split", [])
        if control not in start.get("controls_ids", []) and any(p.get("id") == control and p.get("found") is False for p in attempts):
            return {"outcome": "unavailable", "note": "Single Gerbers tile has no Add beside control",
                    "one_step_back": False, "found": False, "presses": [],
                    "evidence": {"tile": tiles[0]["id"], "missing_control": control}}

    ok_g, err_g = _shot_cleared(full_dir, "task4-dock-gerbers", caps_full, shots, cleared_shots)
    ok_s, err_s = _shot_cleared(full_dir, "task4-dock-split", caps_full, shots, cleared_shots)
    ok_b, err_b = _shot_cleared(full_dir, "task4-dock-back", caps_full, shots, cleared_shots)

    if not full_client_ok or not ok_g or not ok_s:
        return {
            "outcome": "unavailable" if (not ok_g and ok_s) else "harness_failed",
            "note": err_g or err_s or "client failed or Task 4 docking captures uncleared",
            "one_step_back": False,
            "found": False,
            "presses": [],
            "evidence": {"gerbers_err": err_g, "split_err": err_s, "back_err": err_b},
        }

    # Require real observed delivery for the user-facing Task 4 route (never fabricated found=True).
    p_g = [p for p in presses.get("press:task4-dock-gerbers", []) if p.get("found")]
    p_s = [p for p in presses.get("press:task4-dock-split", []) if p.get("found")]
    p_b = [p for p in presses.get("press:task4-dock-back", []) if p.get("found")]
    observed_presses = [p["id"] for p in p_g] + [p["id"] for p in p_s]
    found_route = bool(p_g and p_s)

    s_g = shots["task4-dock-gerbers"]
    s_split = shots["task4-dock-split"]

    # Fresh tile inventory for every shot: after the last delivered press of the EXACT ordered route
    # (rail Gerbers by id, the drawn Gerbers tile's `add-beside`, then `add-beside:board`, then the drawn Board tile's `close`).
    fg_ok, fg_err = _fresh_tiles_after_mark(s_g, mark_seqs, "press:task4-dock-gerbers", _exact_chain(presses, "press:task4-dock-gerbers", ["rail:Gerbers"]))
    if not fg_ok:
        return {
            "outcome": "unavailable",
            "note": fg_err,
            "one_step_back": False,
            "found": found_route,
            "presses": observed_presses,
            "evidence": {"fresh_gerbers": fg_err},
        }
    g_tiles = s_g.get("tiles") or []
    v_g = s_g.get("viewer") or {}
    if len(g_tiles) != 1 or str(g_tiles[0].get("kind") or "") != "gerbers":
        return {
            "outcome": "task_failed",
            "note": f"starting Gerbers shot did not draw 1 gerbers tile (got {len(g_tiles)})",
            "one_step_back": False,
            "found": found_route,
            "presses": observed_presses,
            "evidence": {"init_tiles": g_tiles},
        }
    gid = str(g_tiles[0].get("id") or "")
    fs_ok, fs_err = _fresh_tiles_after_mark(
        s_split, mark_seqs, "press:task4-dock-split",
        _exact_chain(presses, "press:task4-dock-split", [f"tile:{gid}:add-beside", "add-beside:board"]),
    )
    if not fs_ok:
        return {
            "outcome": "unavailable",
            "note": fs_err,
            "one_step_back": False,
            "found": found_route,
            "presses": observed_presses,
            "evidence": {"fresh_split": fs_err},
        }

    init_id = str(g_tiles[0].get("id") or "")
    init_src = _tile_source(g_tiles[0], v_g)
    init_rect = _rect_tuple(g_tiles[0].get("rect"))

    tiles = s_split.get("tiles") or []
    splitters = s_split.get("splitters") or []
    v_split = s_split.get("viewer") or {}

    fails: list[str] = []
    if not found_route:
        fails.append("Task 4 user-facing presses not delivered")
    board_ids = [str(t.get("id") or "") for t in tiles if t.get("kind") == "board"]
    if len(tiles) != 2:
        fails.append(f"expected 2 tiles, got {len(tiles)}")
    kinds = {str(t.get("kind") or "") for t in tiles}
    if kinds != {"gerbers", "board"}:
        fails.append(f"expected tile kinds {{'gerbers', 'board'}}, got {sorted(kinds)}")

    rects = [_rect_tuple(t.get("rect")) for t in tiles]
    body_rects = [_rect_tuple(t.get("body")) for t in tiles]
    if any(r is None for r in rects):
        return {
            "outcome": "unavailable",
            "note": "tile metric missing outer `rect` (measured geometry absent, not a product failure)",
            "one_step_back": False,
            "found": found_route,
            "presses": observed_presses,
            "evidence": {"tiles": tiles},
        }
    if len(rects) == 2 and not _rects_disjoint(rects[0], rects[1]):
        fails.append("tile rects overlap")
    if any(b is None for b in body_rects):
        return {
            "outcome": "unavailable",
            "note": "tile metric missing measured `body` rect",
            "one_step_back": False,
            "found": found_route,
            "presses": observed_presses,
            "evidence": {"tiles": tiles},
        }

    splitter = _splitter_between(splitters, rects[0], rects[1], "x") if (len(rects) == 2 and all(rects)) else None
    if splitter is None:
        fails.append("no vertical ('x') splitter found between Gerbers and Board tiles")

    sources = [_tile_source(t, v_split) for t in tiles]
    if len(sources) == 2:
        if not sources[0] or not sources[1]:
            fails.append(f"tile source missing ({sources})")
        elif sources[0] == sources[1]:
            fails.append(f"both tiles report identical source ({sources[0]!r})")

    split_png = full_dir / "task4-dock-split.png"
    bodies = [tile_body_non_flat(split_png, t.get("body")) for t in tiles]
    if any(b is not True for b in bodies):
        fails.append(f"tile body flat or unreadable ({bodies})")
    inter_diff = tile_bodies_pixel_diff(split_png, tiles[0].get("body"), split_png, tiles[1].get("body")) if len(tiles) == 2 else None
    if inter_diff is None or inter_diff <= 0.01:
        fails.append(f"the two tile bodies do not differ visually in pixels (diff={inter_diff})")

    # One step back: closing the added tile returns the original Gerbers tile (same id & source) to full width
    back_ok = False
    back_note = err_b
    back_unavailable = not ok_b  # an uncaptured/uncleared return shot is not a product failure
    if ok_b:
        s_back = shots["task4-dock-back"]
        fb_ok, fb_err = _fresh_tiles_after_mark(s_back, mark_seqs, "press:task4-dock-back", _exact_chain(presses, "press:task4-dock-back", [f"tile:{board_ids[0]}:close"] if board_ids else []))
        if not fb_ok:
            back_note, back_unavailable = fb_err, True
        elif not p_b:
            back_note, back_unavailable = "close press not delivered for task4-dock-back", True
        elif init_rect is None or (len(s_back.get("tiles") or []) == 1 and _rect_tuple(s_back["tiles"][0].get("rect")) is None):
            # both width observations are required: a missing rect in the initial or return shot is a gap, not width_restored=False
            back_note = "missing outer rect in the initial or return Gerbers shot (width not observed)"
            back_unavailable = True
        else:
            b_tiles = s_back.get("tiles") or []
            b_splitters = s_back.get("splitters") or []
            v_back = s_back.get("viewer") or {}
            b_rect = _rect_tuple(b_tiles[0].get("rect")) if len(b_tiles) == 1 else None
            b_id = str(b_tiles[0].get("id") or "") if len(b_tiles) == 1 else ""
            b_src = _tile_source(b_tiles[0], v_back) if len(b_tiles) == 1 else ""
            b_body = tile_body_non_flat(full_dir / "task4-dock-back.png", b_tiles[0].get("body")) if len(b_tiles) == 1 else None
            same_survivor = bool(init_id and b_id == init_id and init_src and b_src == init_src)
            width_restored = bool(b_rect and init_rect and b_rect[2] >= 0.95 * init_rect[2])
            if (
                len(b_tiles) == 1
                and str(b_tiles[0].get("kind") or "") == "gerbers"
                and len(b_splitters) == 0
                and same_survivor
                and width_restored
                and b_body is True
            ):
                back_ok = True
                back_note = ""
            else:
                back_note = (
                    f"close did not restore original full-width Gerbers tile "
                    f"(tiles={len(b_tiles)}, splitters={len(b_splitters)}, id={b_id!r} vs {init_id!r}, "
                    f"src={b_src!r} vs {init_src!r}, width_ok={width_restored}, body={b_body})"
                )

    if not fails and back_ok:
        outcome = "passed"
    elif not fails and back_unavailable:
        outcome = "unavailable"
    else:
        outcome = "task_failed"
    return {
        "outcome": outcome,
        "note": "; ".join(fails + ([back_note] if (back_note and not back_ok) else [])),
        "one_step_back": back_ok,
        "found": found_route,
        "presses": observed_presses,
        "evidence": {
            "init_tile_id": init_id,
            "init_source": init_src,
            "tiles": tiles,
            "splitters": splitters,
            "sources": sources,
            "bodies_non_flat": bodies,
            "inter_tile_pixel_diff": inter_diff,
            "one_step_back_ok": back_ok,
            "one_step_back_note": back_note,
        },
    }


def evaluate_b7_dock(
    full_dir: Path,
    parsed_full: dict,
    caps_full: dict[str, dict],
    cleared_shots: set[str] | None = None,
) -> dict:
    """Evaluate Rule B7 (one split and compare model) from fresh tile inventories, drag events, refusals, and PNGs.

    Returns `status: "unavailable"` until a build emits `GPUI_METRIC event=tiles`.
    Scoring denominator is ALWAYS all 7 checks (unavailable checks are counted as not earned).
    """
    total_checks = len(DOCK_B7_CHECKS)
    if not parsed_full.get("tiles_metric_seen"):
        return {
            "status": "unavailable",
            "reason": "client did not emit GPUI_METRIC event=tiles (requires docking PR #1414 + renderer)",
            "total": total_checks,
            "checked": 0,
            "clean": 0,
            "unavailable_count": total_checks,
            "violations": [],
            "checks": {k: {"status": "unavailable", "passed": None, "note": "no tiles metric"} for k in DOCK_B7_CHECKS},
        }

    shots = parsed_full.get("shots") or {}
    mark_seqs = parsed_full.get("mark_seqs") or {}
    drags = parsed_full.get("drags") or {}
    presses = parsed_full.get("presses") or {}
    refusals = parsed_full.get("refusals") or {}
    checks: dict[str, dict] = {}

    def _rec(status: str, passed: bool | None, note: str = "", evidence: dict | None = None) -> dict:
        return {"status": status, "passed": passed, "note": note, "evidence": evidence or {}}

    def _tile_frame(t: dict) -> int | None:
        f = t.get("frame")
        return f if (isinstance(f, int) and not isinstance(f, bool)) else None

    # 1. add_beside_directions (right, left, bottom, top)
    dir_results: dict[str, bool | str] = {}
    dir_notes: list[str] = []
    for side, axis in (("right", "x"), ("left", "x"), ("bottom", "y"), ("top", "y")):
        sname = f"b7-add-{side}"
        if sname not in shots:
            dir_results[side] = "unavailable"
            continue
        ok, err = _shot_cleared(full_dir, sname, caps_full, shots, cleared_shots)
        if not ok:
            dir_results[side] = "unavailable"
            dir_notes.append(f"{side}: {err}")
            continue
        fr_ok, fr_err = _fresh_tiles_after_mark(shots[sname], mark_seqs, f"b7:add-{side}", _chain(presses, f"b7:add-{side}"))
        if not fr_ok:
            dir_results[side] = "unavailable"
            dir_notes.append(f"{side}: {fr_err}")
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
        bb = _rect_tuple(by_kind["board"].get("body"))
        bs = _rect_tuple(by_kind["schematic"].get("body"))
        if not (bb and bs):
            dir_results[side] = "unavailable"
            dir_notes.append(f"{side}: missing measured body rect")
            continue
        if rb is None or rs is None:
            dir_results[side] = "unavailable"
            dir_notes.append(f"{side}: missing outer tile rect")
            continue
        if not _rects_disjoint(rb, rs):
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
        png = full_dir / f"{sname}.png"
        b1 = tile_body_non_flat(png, by_kind["board"].get("body"))
        b2 = tile_body_non_flat(png, by_kind["schematic"].get("body"))
        diff12 = tile_bodies_pixel_diff(png, by_kind["board"].get("body"), png, by_kind["schematic"].get("body"))
        if pos_ok and sp_ok and b1 is True and b2 is True and (diff12 is not None and diff12 > 0.01):
            dir_results[side] = True
        else:
            dir_results[side] = False
            dir_notes.append(f"{side}: pos_ok={pos_ok} sp_ok={sp_ok} bodies=({b1},{b2}) diff={diff12}")

    if any(v is False for v in dir_results.values()):
        checks["add_beside_directions"] = _rec("measured", False, "; ".join(dir_notes), {"directions": dir_results})
    elif all(v is True for v in dir_results.values()):
        checks["add_beside_directions"] = _rec("measured", True, "", {"directions": dir_results})
    else:
        checks["add_beside_directions"] = _rec("unavailable", None, "; ".join(dir_notes) or "not all 4 directions captured", {"directions": dir_results})

    # 2. close (missing pane geometry -> unavailable, never waived)
    close_side = next((s for s in ("right", "left", "bottom", "top") if f"b7-close-{s}" in shots), None)
    if close_side is None:
        checks["close"] = _rec("unavailable", None, "no b7-close-* shot captured")
    else:
        close_sname = f"b7-close-{close_side}"
        ok, err = _shot_cleared(full_dir, close_sname, caps_full, shots, cleared_shots)
        if not ok:
            checks["close"] = _rec("unavailable", None, err)
        else:
            srec = shots[close_sname]
            fr_ok, fr_err = _fresh_tiles_after_mark(srec, mark_seqs, f"b7:close-{close_side}", _chain(presses, f"b7:close-{close_side}"))
            pr = _pane_rect_from_layout(srec.get("layout"))
            if not fr_ok:
                checks["close"] = _rec("unavailable", None, fr_err)
            elif pr is None:
                checks["close"] = _rec("unavailable", None, "missing pane layout geometry for close check")
            else:
                ts = srec.get("tiles") or []
                sps = srec.get("splitters") or []
                tr = _rect_tuple(ts[0].get("rect")) if len(ts) == 1 else None
                body = tile_body_non_flat(full_dir / f"{close_sname}.png", ts[0].get("body")) if len(ts) == 1 else None
                width_ok = bool(tr and tr[2] >= 0.95 * pr[2])
                ok_close = len(ts) == 1 and len(sps) == 0 and width_ok and body is True
                checks["close"] = _rec(
                    "measured",
                    ok_close,
                    "" if ok_close else f"tiles={len(ts)} splitters={len(sps)} width_ok={width_ok} body={body}",
                    {"shot": close_sname, "tiles": ts, "splitters": sps},
                )

    # 3. swap (per-tile identity, swapped rects, advancing frame seq per tile, non-flat distinct bodies)
    if "b7-before-swap" not in shots or "b7-after-swap" not in shots:
        checks["swap"] = _rec("unavailable", None, "swap shots not captured")
    else:
        ok1, e1 = _shot_cleared(full_dir, "b7-before-swap", caps_full, shots, cleared_shots)
        ok2, e2 = _shot_cleared(full_dir, "b7-after-swap", caps_full, shots, cleared_shots)
        if not (ok1 and ok2):
            checks["swap"] = _rec("unavailable", None, e1 or e2)
        else:
            fr_ok, fr_err = _fresh_tiles_after_mark(shots["b7-after-swap"], mark_seqs, "b7:swap", _chain(presses, "b7:swap"))
            if not fr_ok:
                checks["swap"] = _rec("unavailable", None, fr_err)
            else:
                pre_by_id = {str(t.get("id") or ""): t for t in (shots["b7-before-swap"].get("tiles") or []) if t.get("id")}
                post_by_id = {str(t.get("id") or ""): t for t in (shots["b7-after-swap"].get("tiles") or []) if t.get("id")}
                common = sorted(set(pre_by_id) & set(post_by_id))
                if len(common) != 2:
                    checks["swap"] = _rec("measured", False, f"expected 2 matching tile ids across swap, got {common}")
                else:
                    id0, id1 = common
                    r_pre0, r_pre1 = _rect_tuple(pre_by_id[id0].get("rect")), _rect_tuple(pre_by_id[id1].get("rect"))
                    r_post0, r_post1 = _rect_tuple(post_by_id[id0].get("rect")), _rect_tuple(post_by_id[id1].get("rect"))
                    f_pre0, f_pre1 = _tile_frame(pre_by_id[id0]), _tile_frame(pre_by_id[id1])
                    f_post0, f_post1 = _tile_frame(post_by_id[id0]), _tile_frame(post_by_id[id1])
                    if any(x is None for x in (f_pre0, f_pre1, f_post0, f_post1)):
                        checks["swap"] = _rec("unavailable", None, "tile metric missing per-tile `frame` sequence for swap")
                    elif not (r_pre0 and r_pre1 and r_post0 and r_post1):
                        checks["swap"] = _rec("measured", False, "missing tile rects across swap")
                    else:
                        swapped = _rect_close(r_pre0, r_post1) and _rect_close(r_pre1, r_post0) and not _rect_close(r_pre0, r_post0)
                        frames_adv = (f_post0 > f_pre0) and (f_post1 > f_pre1)
                        src_kept = (
                            _tile_source(pre_by_id[id0], shots["b7-before-swap"].get("viewer") or {})
                            == _tile_source(post_by_id[id0], shots["b7-after-swap"].get("viewer") or {})
                            and _tile_source(pre_by_id[id1], shots["b7-before-swap"].get("viewer") or {})
                            == _tile_source(post_by_id[id1], shots["b7-after-swap"].get("viewer") or {})
                        )
                        png_post = full_dir / "b7-after-swap.png"
                        b0 = tile_body_non_flat(png_post, post_by_id[id0].get("body"))
                        b1 = tile_body_non_flat(png_post, post_by_id[id1].get("body"))
                        inter = tile_bodies_pixel_diff(png_post, post_by_id[id0].get("body"), png_post, post_by_id[id1].get("body"))
                        ok_swap = bool(swapped and frames_adv and src_kept and b0 is True and b1 is True and (inter is not None and inter > 0.01))
                        checks["swap"] = _rec(
                            "measured",
                            ok_swap,
                            "" if ok_swap else f"swapped={swapped} frames_adv={frames_adv} src_kept={src_kept} bodies=({b0},{b1}) inter_diff={inter}",
                        )

    # 4. resize_drag (strict bool ok=True, splitter id + motion, ratio delta >= 0.05, per-tile frame advances + non-flat distinct bodies)
    if "b7-after-swap" not in shots or "b7-after-resize" not in shots:
        checks["resize_drag"] = _rec("unavailable", None, "resize shots not captured")
    else:
        ok1, e1 = _shot_cleared(full_dir, "b7-after-swap", caps_full, shots, cleared_shots)
        ok2, e2 = _shot_cleared(full_dir, "b7-after-resize", caps_full, shots, cleared_shots)
        drag_evs = [
            d for d in drags.get("b7:resize-drag", [])
            if d.get("ok") is True and str(d.get("splitter") or "").strip()
        ]
        if not (ok1 and ok2):
            checks["resize_drag"] = _rec("unavailable", None, e1 or e2)
        elif not drag_evs:
            checks["resize_drag"] = _rec("unavailable", None, "no delivered drag event (strict ok=True with splitter id) recorded for b7:resize-drag")
        else:
            fr_ok, fr_err = _fresh_tiles_after_mark(shots["b7-after-resize"], mark_seqs, "b7:resize-drag", drags.get("b7:resize-drag", []))
            if not fr_ok:
                checks["resize_drag"] = _rec("unavailable", None, fr_err)
            else:
                d0 = drag_evs[-1]
                d_from, d_to = d0.get("from"), d0.get("to")
                sp_drawn = {str(sp.get("id") or ""): sp for sp in (shots["b7-after-swap"].get("splitters") or []) if isinstance(sp, dict)}
                sp_axis = (sp_drawn.get(str(d0.get("splitter") or "")) or {}).get("axis")
                moved_ok = False
                if isinstance(d_from, list) and isinstance(d_to, list) and len(d_from) == 2 and len(d_to) == 2 and sp_axis in ("x", "y"):
                    along, across = (0, 1) if sp_axis == "x" else (1, 0)
                    moved_ok = abs(float(d_to[along]) - float(d_from[along])) >= 10.0 and abs(float(d_to[across]) - float(d_from[across])) <= 3.0
                pre_by_id = {str(t.get("id") or ""): t for t in (shots["b7-after-swap"].get("tiles") or []) if t.get("id")}
                post_by_id = {str(t.get("id") or ""): t for t in (shots["b7-after-resize"].get("tiles") or []) if t.get("id")}
                common = sorted(set(pre_by_id) & set(post_by_id))
                if len(common) != 2:
                    checks["resize_drag"] = _rec("measured", False, f"expected 2 matching tiles across resize, got {common}")
                else:
                    id0, id1 = common
                    f_pre0, f_pre1 = _tile_frame(pre_by_id[id0]), _tile_frame(pre_by_id[id1])
                    f_post0, f_post1 = _tile_frame(post_by_id[id0]), _tile_frame(post_by_id[id1])
                    if any(x is None for x in (f_pre0, f_pre1, f_post0, f_post1)):
                        checks["resize_drag"] = _rec("unavailable", None, "tile metric missing per-tile `frame` sequence for resize")
                    else:
                        pre_rs = sorted([_rect_tuple(pre_by_id[i].get("rect")) for i in common], key=lambda r: r[0] if r else 0)
                        post_rs = sorted([_rect_tuple(post_by_id[i].get("rect")) for i in common], key=lambda r: r[0] if r else 0)
                        if not (all(pre_rs) and all(post_rs)):
                            checks["resize_drag"] = _rec("measured", False, "missing tile rects across resize")
                        else:
                            r_pre = pre_rs[0][2] / (pre_rs[0][2] + pre_rs[1][2])
                            r_post = post_rs[0][2] / (post_rs[0][2] + post_rs[1][2])
                            ratio_delta = abs(r_post - r_pre)
                            frames_adv = (f_post0 > f_pre0) and (f_post1 > f_pre1)
                            png_rsz = full_dir / "b7-after-resize.png"
                            b0 = tile_body_non_flat(png_rsz, post_by_id[id0].get("body"))
                            b1 = tile_body_non_flat(png_rsz, post_by_id[id1].get("body"))
                            inter = tile_bodies_pixel_diff(png_rsz, post_by_id[id0].get("body"), png_rsz, post_by_id[id1].get("body"))
                            ok_rsz = bool(moved_ok and ratio_delta >= 0.05 and frames_adv and b0 is True and b1 is True and (inter is not None and inter > 0.01))
                            checks["resize_drag"] = _rec(
                                "measured",
                                ok_rsz,
                                "" if ok_rsz else f"moved_along_axis={moved_ok} (splitter {d0.get('splitter')!r} axis {sp_axis!r} in the drawn inventory) ratio {r_pre:.3f}->{r_post:.3f} frames_adv={frames_adv} bodies=({b0},{b1}) inter={inter}",
                                {"splitter": d0.get("splitter"), "ratio_before": round(r_pre, 3), "ratio_after": round(r_post, 3)},
                            )

    # 5. maximize_restore (missing pane geometry -> unavailable, never waived)
    if "b7-maximized" not in shots or "b7-restored" not in shots:
        checks["maximize_restore"] = _rec("unavailable", None, "maximize/restore shots not captured")
    else:
        ok1, e1 = _shot_cleared(full_dir, "b7-maximized", caps_full, shots, cleared_shots)
        ok2, e2 = _shot_cleared(full_dir, "b7-restored", caps_full, shots, cleared_shots)
        if not (ok1 and ok2):
            checks["maximize_restore"] = _rec("unavailable", None, e1 or e2)
        else:
            sm, sr = shots["b7-maximized"], shots["b7-restored"]
            fm_ok, fm_err = _fresh_tiles_after_mark(sm, mark_seqs, "b7:maximize", _chain(presses, "b7:maximize"))
            fr_ok, fr_err = _fresh_tiles_after_mark(sr, mark_seqs, "b7:restore", _chain(presses, "b7:restore"))
            pm = _pane_rect_from_layout(sm.get("layout"))
            if not (fm_ok and fr_ok):
                checks["maximize_restore"] = _rec("unavailable", None, fm_err or fr_err)
            elif pm is None:
                checks["maximize_restore"] = _rec("unavailable", None, "missing pane layout geometry for maximize check")
            else:
                tm, spm = sm.get("tiles") or [], sm.get("splitters") or []
                tr, spr = sr.get("tiles") or [], sr.get("splitters") or []
                rm = _rect_tuple(tm[0].get("rect")) if len(tm) == 1 else None
                max_ok = (
                    len(tm) == 1
                    and len(spm) == 0
                    and rm is not None
                    and rm[2] >= 0.95 * pm[2]
                    and rm[3] >= 0.95 * pm[3]
                    and tile_body_non_flat(full_dir / "b7-maximized.png", tm[0].get("body")) is True
                )
                res_rects = [_rect_tuple(t.get("rect")) for t in tr]
                png_res = full_dir / "b7-restored.png"
                res_ok = (
                    len(tr) == 2
                    and all(res_rects)
                    and _rects_disjoint(res_rects[0], res_rects[1])
                    and len(spr) >= 1
                    and all(tile_body_non_flat(png_res, t.get("body")) is True for t in tr)
                )
                ok_mr = bool(max_ok and res_ok)
                checks["maximize_restore"] = _rec(
                    "measured",
                    ok_mr,
                    "" if ok_mr else f"max_ok={max_ok} (tiles={len(tm)}, splitters={len(spm)}), res_ok={res_ok} (tiles={len(tr)}, splitters={len(spr)})",
                )

    # 6. overlay_on_off (explicit overlay field observed on AND observed explicitly off + picture changed back)
    if not all(k in shots for k in ("b7-overlay-plain", "b7-overlay-on", "b7-overlay-off")):
        checks["overlay_on_off"] = _rec("unavailable", None, "overlay shots not captured")
    else:
        ok0, e0 = _shot_cleared(full_dir, "b7-overlay-plain", caps_full, shots, cleared_shots)
        ok1, e1 = _shot_cleared(full_dir, "b7-overlay-on", caps_full, shots, cleared_shots)
        ok2, e2 = _shot_cleared(full_dir, "b7-overlay-off", caps_full, shots, cleared_shots)
        if not (ok0 and ok1 and ok2):
            checks["overlay_on_off"] = _rec("unavailable", None, e0 or e1 or e2)
        else:
            s_on, s_off = shots["b7-overlay-on"], shots["b7-overlay-off"]
            fon_ok, fon_err = _fresh_tiles_after_mark(s_on, mark_seqs, "b7:overlay-on", _chain(presses, "b7:overlay-on"))
            foff_ok, foff_err = _fresh_tiles_after_mark(s_off, mark_seqs, "b7:overlay-off", _chain(presses, "b7:overlay-off"))
            if not (fon_ok and foff_ok):
                checks["overlay_on_off"] = _rec("unavailable", None, fon_err or foff_err)
            else:
                t_on, t_off = s_on.get("tiles") or [], s_off.get("tiles") or []
                v_on, v_off = s_on.get("viewer") or {}, s_off.get("viewer") or {}

                def _overlay_state(v: dict, ts: list[dict]) -> str:
                    """Return 'on', 'off', or 'unobserved' (missing field is NEVER treated as 'off')."""
                    if ts and isinstance(ts[0], dict) and "overlay" in ts[0]:
                        ov = ts[0]["overlay"]
                        if isinstance(ov, dict) and ov.get("kind") == "gerbers":
                            return "on"
                        if ov is None or ov is False:
                            return "off"
                    if "gerbers_vs_board" in v and isinstance(v["gerbers_vs_board"], bool):
                        return "on" if v["gerbers_vs_board"] else "off"
                    return "unobserved"

                st_on = _overlay_state(v_on, t_on)
                st_off = _overlay_state(v_off, t_off)
                if st_on == "unobserved" or st_off == "unobserved":
                    checks["overlay_on_off"] = _rec("unavailable", None, f"overlay field unobserved (on={st_on}, off={st_off})")
                else:
                    on_body = tile_body_non_flat(full_dir / "b7-overlay-on.png", t_on[0].get("body")) if t_on else None
                    off_body = tile_body_non_flat(full_dir / "b7-overlay-off.png", t_off[0].get("body")) if t_off else None
                    diff_on = tile_bodies_pixel_diff(full_dir / "b7-overlay-plain.png", (shots["b7-overlay-plain"].get("tiles") or [{}])[0].get("body"), full_dir / "b7-overlay-on.png", t_on[0].get("body")) if t_on else None
                    diff_back = tile_bodies_pixel_diff(full_dir / "b7-overlay-on.png", t_on[0].get("body"), full_dir / "b7-overlay-off.png", t_off[0].get("body")) if (t_on and t_off) else None
                    ok_ov = bool(
                        st_on == "on"
                        and st_off == "off"
                        and on_body is True
                        and off_body is True
                        and (diff_on is not None and diff_on > 0.005)
                        and (diff_back is not None and diff_back > 0.005)
                    )
                    checks["overlay_on_off"] = _rec(
                        "measured",
                        ok_ov,
                        "" if ok_ov else f"st_on={st_on} st_off={st_off} bodies=({on_body},{off_body}) diff_on={diff_on} diff_back={diff_back}",
                        {"diff_on": diff_on, "diff_back": diff_back},
                    )

    # 7. incompatible_overlay_refusal (code + human text with no raw code + before/after layout equal)
    if "b7-overlay-incompat-before" not in shots or "b7-overlay-incompat" not in shots:
        checks["incompatible_overlay_refusal"] = _rec("unavailable", None, "incompatible overlay before/after shots not captured")
    else:
        ok0, e0 = _shot_cleared(full_dir, "b7-overlay-incompat-before", caps_full, shots, cleared_shots)
        ok1, e1 = _shot_cleared(full_dir, "b7-overlay-incompat", caps_full, shots, cleared_shots)
        if not (ok0 and ok1):
            checks["incompatible_overlay_refusal"] = _rec("unavailable", None, e0 or e1)
        else:
            s_pre, s_inc = shots["b7-overlay-incompat-before"], shots["b7-overlay-incompat"]
            v_pre, v_inc = s_pre.get("viewer") or {}, s_inc.get("viewer") or {}
            lay_pre, lay_post = v_pre.get("layout"), v_inc.get("layout")
            leaves_pre, leaves_post = layout_leaves(lay_pre), layout_leaves(lay_post)

            def _unavail(note: str) -> None:
                checks["incompatible_overlay_refusal"] = _rec("unavailable", None, note)

            files_ids = [i for i, t in leaves_pre.items() if t.get("kind") == "files"]
            if not leaves_pre or not leaves_post:
                # `{}`, a missing key or a tree with no tile leaves is not an observed layout (two empties are not "unchanged")
                _unavail("hub layout (`viewer.layout`, tile leaves under `root`) not observed before and after the refused overlay")
            elif {t.get("kind") for t in leaves_pre.values()} != {"board", "files"} or len(leaves_pre) != 2:
                _unavail(f"starting tree is not exactly Board + Files ({sorted(str(t.get('kind')) for t in leaves_pre.values())})")
            else:
                # Bound to the delivered press: that exact control on the drawn Files tile, then a refusal reported AFTER it
                chain = _exact_chain(presses, "b7:overlay-incompatible", [f"tile:{files_ids[0]}:overlay-gerbers"])
                if not chain:
                    _unavail(
                        f"overlay press not delivered on the drawn Files tile {files_ids[0]!r} "
                        "(a build that does not offer Files overlay leaves this check unavailable by design)"
                    )
                else:
                    press_seq = max(int(e.get("seq") or 0) for e in chain)
                    ref_evs = [r for r in refusals.get("b7:overlay-incompatible", []) if int(r.get("seq") or 0) > press_seq]
                    code = str((ref_evs[-1].get("code") if ref_evs else "") or "").strip()
                    msg = str((ref_evs[-1].get("text") if ref_evs else "") or "").strip()
                    code_ok = code in INCOMPATIBLE_OVERLAY_CODES
                    raw_code = bool(RAW_REFUSAL_CODE_RE.search(msg)) if msg else False
                    layout_eq = lay_pre == lay_post
                    ok_ref = bool(ref_evs and code_ok and msg and not raw_code and layout_eq)
                    # A delivered press with NO refusal event is a failure only where refusal observation is independently confirmed
                    # in this run (the renderer's `tiles` event declares `refusal_metric: true`); otherwise nothing was observable.
                    if not ref_evs and not parsed_full.get("refusal_metric_declared"):
                        checks["incompatible_overlay_refusal"] = _rec(
                            "unavailable", None,
                            "no refusal event after the delivered press, and this run never confirmed that refusals are observable (`tiles` event `refusal_metric`)",
                        )
                    else:
                        checks["incompatible_overlay_refusal"] = _rec(
                            "measured",
                            ok_ref,
                            "" if ok_ref else (
                                "no refusal observed after the delivered overlay press (refusals are observable in this run)" if not ref_evs
                                else f"code={code!r} (ok={code_ok}) text={msg!r} raw_code={raw_code} layout_eq={layout_eq}"
                            ),
                            {"code": code, "refusal_text": msg, "layout_equal": layout_eq, "press_seq": press_seq},
                        )

    measured = [(k, c) for k, c in checks.items() if c["status"] == "measured" and isinstance(c["passed"], bool)]
    if not measured:
        return {
            "status": "unavailable",
            "reason": "tiles metric seen, but no B7 docking transitions were captured",
            "total": total_checks,
            "checked": 0,
            "clean": 0,
            "unavailable_count": total_checks,
            "violations": [],
            "checks": checks,
        }
    clean = sum(1 for _, c in measured if c["passed"] is True)
    violations = [f"{k}: {c['note'] or 'failed'}" for k, c in measured if c["passed"] is False]
    all_measured = len(measured) == total_checks
    return {
        "status": "measured" if all_measured else "measured_partial",
        "reason": "",
        "total": total_checks,
        "checked": len(measured),
        "clean": clean,
        "unavailable_count": total_checks - len(measured),
        "violations": violations,
        "checks": checks,
    }
