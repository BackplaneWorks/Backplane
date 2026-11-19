#!/usr/bin/env python3
"""Compare two rail-shots.sh output directories: per-page before/after pairs + score table.

Usage:
  rail_shots_compare.py [--share] BASE_DIR NEW_DIR [OUT_DIR]

Writes into a fresh, empty OUT_DIR (default: NEW_DIR/compare):
  pairs/<kind>-<page>.png   side-by-side BASE vs NEW images (cleared non-private shots only)
  headers-compare.png       only when both runs have schema>=2 privacy-cleared strips
  rails-compare.png         only when both runs have schema>=2 privacy-cleared strips
  score.json                structured comparability, common-task & consistency breakdown
  score.md                  human-readable round report
"""
from __future__ import annotations

import json
import os
import re
import sys
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

sys.path.insert(0, str(Path(__file__).resolve().parent))
import rail_shots_manifest as M  # noqa: E402

RAW_ERROR_RE = re.compile(r"PRESENTER_UNAVAILABLE|ECONNREFUSED|EACCES|ENOENT|panicked at|stack backtrace", re.IGNORECASE)

UNMEASURED_RULES = [
    ("B1", "One rail owns every page"),
    ("B2", "Single clear active indicator"),
    ("B5", "Consistent pane header chrome"),
    ("B7", "Keyboard shortcuts match rail"),
    ("B8", "Split / multi-pane coherence"),
    ("B9", "Mobile / phone rail parity"),
    ("B10", "Visual polish & spacing"),
]


def load_font(size: int = 13):
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


def has_complete_privacy(steps: dict) -> bool:
    return bool(steps.get("privacy_checked") is True and int(steps.get("evidence_schema") or 1) >= 2)


def collect_private_shots(steps: dict, prefix: str) -> list[str]:
    out: list[str] = []
    for kind, kdata in (steps.get("kinds") or {}).items():
        if "private_shots" in kdata:
            for sname, reasons in (kdata.get("private_shots") or {}).items():
                out.append(f"{prefix}/{kind}/{sname}: {'; '.join(reasons)}")
        else:
            for sname in (kdata.get("obstructions") or {}).keys():
                out.append(f"{prefix}/{kind}/{sname}: obstructed (legacy manifest)")
    return out


def check_comparability(base_steps: dict, new_steps: dict) -> tuple[bool, list[str]]:
    """Refuse a whole-run scored delta when embed mode, capabilities, kinds, schema, or task definitions differ."""
    reasons: list[str] = []

    b_schema = int(base_steps.get("evidence_schema") or 1)
    n_schema = int(new_steps.get("evidence_schema") or 1)
    if b_schema != n_schema:
        reasons.append(f"evidence_schema differs (base={b_schema}, new={n_schema})")

    b_embed = bool(base_steps.get("embed_mode"))
    n_embed = bool(new_steps.get("embed_mode"))
    if b_embed != n_embed:
        reasons.append(f"embed_mode differs (base={b_embed}, new={n_embed})")

    b_caps = sorted((base_steps.get("runtime") or {}).get("capabilities") or [])
    n_caps = sorted((new_steps.get("runtime") or {}).get("capabilities") or [])
    if b_caps != n_caps:
        reasons.append(f"runtime capabilities differ (base={b_caps}, new={n_caps})")

    b_kinds = sorted((base_steps.get("kinds") or {}).keys())
    n_kinds = sorted((new_steps.get("kinds") or {}).keys())
    if b_kinds != n_kinds:
        reasons.append(f"project kinds differ (base={b_kinds}, new={n_kinds})")

    b_defs = [(t.get("task"), t.get("name"), t.get("step_budget")) for t in (base_steps.get("scorecard_tasks") or [])]
    n_defs = [(t.get("task"), t.get("name"), t.get("step_budget")) for t in (new_steps.get("scorecard_tasks") or [])]
    if b_defs != n_defs:
        reasons.append("task definitions or step budgets differ")

    return len(reasons) == 0, reasons


def score_single_task(task: dict | None) -> dict:
    """Score one task honestly: 4 pts only when all 4 parts are evidenced; unavailable/missing/harness_failed are unscored."""
    if not task:
        return {"scored": False, "outcome": "missing", "points": None, "max_points": 0, "unmeasured_parts": [], "presses": 0, "budget": 0}
    outcome = str(task.get("outcome") or "unavailable")
    budget = int(task.get("step_budget") or 0)
    presses = int(task.get("presses_used") or 0)

    if outcome in ("unavailable", "harness_failed", "missing"):
        return {
            "scored": False,
            "outcome": outcome,
            "points": None,
            "max_points": 0,
            "unmeasured_parts": [],
            "presses": presses,
            "budget": budget,
            "note": task.get("note", ""),
        }

    if outcome == "task_failed":
        parts = task.get("parts") or {}
        unmeasured = [k for k, v in parts.items() if v == "not_measured"]
        return {
            "scored": True,
            "outcome": outcome,
            "points": 0,
            "max_points": 4,
            "unmeasured_parts": unmeasured,
            "presses": presses,
            "budget": budget,
            "note": task.get("note", ""),
        }

    # outcome == "passed"
    parts = task.get("parts")
    if isinstance(parts, dict) and parts:
        pts = sum(1 for k in ("found", "within_budget", "result_evidenced", "one_step_back") if parts.get(k) is True)
        unmeasured = [k for k in ("found", "within_budget", "result_evidenced", "one_step_back") if parts.get(k) == "not_measured"]
    else:
        # Legacy manifest without `parts`: do not fabricate one_step_back for tasks that do not verify return.
        tid = task.get("task")
        has_return = tid in (3, 8, 10)
        within = bool(task.get("within_budget", True))
        pts = (1 + (1 if within else 0) + 1 + (1 if has_return else 0))
        unmeasured = [] if has_return else ["one_step_back"]

    return {
        "scored": True,
        "outcome": outcome,
        "points": pts,
        "max_points": 4,
        "unmeasured_parts": unmeasured,
        "presses": presses,
        "budget": budget,
        "note": task.get("note", ""),
    }


def score_tasks(steps: dict) -> dict:
    by_id = {int(t["task"]): t for t in (steps.get("scorecard_tasks") or []) if "task" in t}
    rows = []
    earned = 0
    scored_max = 0
    unscored_count = 0
    for tid in range(1, 11):
        t = by_id.get(tid)
        st = score_single_task(t)
        st["task"] = tid
        st["name"] = (t or {}).get("name", f"Task {tid}")
        if st["scored"]:
            earned += int(st["points"])
            scored_max += int(st["max_points"])
        else:
            unscored_count += 1
        rows.append(st)
    return {
        "points": earned,
        "scored_max_points": scored_max,
        "total_max_points": 40,
        "unscored_tasks": unscored_count,
        "tasks": rows,
    }


def score_common_tasks(base_tasks: dict, new_tasks: dict) -> dict:
    """Compare only tasks that are scored on BOTH sides (excluding unavailable/missing/harness_failed)."""
    b_by_id = {t["task"]: t for t in base_tasks["tasks"]}
    n_by_id = {t["task"]: t for t in new_tasks["tasks"]}
    common_ids: list[int] = []
    b_pts = 0
    n_pts = 0
    max_pts = 0
    for tid in range(1, 11):
        bt = b_by_id.get(tid, {})
        nt = n_by_id.get(tid, {})
        if bt.get("scored") and nt.get("scored"):
            common_ids.append(tid)
            b_pts += int(bt["points"])
            n_pts += int(nt["points"])
            max_pts += 4
    return {
        "common_task_ids": common_ids,
        "base_points": b_pts,
        "new_points": n_pts,
        "max_points": max_pts,
        "delta": n_pts - b_pts,
    }


def score_consistency(steps: dict) -> dict:
    """Evaluate measurable consistency rules at 3 points each (matching the 10x3=30 scorecard).

    - B3 (3 pts, partial check): Known misplaced controls found on unobstructed, non-private opened pages.
    - B4 (3 pts, desktop availability): Absent resources disabled/hidden on desktop kinds (phones not measured).
    - B6 (3 pts = 1 loading + 1 empty + 1 error): Strictly unobstructed, non-private opened pages;
      disabled/hidden entries do not count as a page state; loading requires early_capture_ok == True;
      error checks for raw internal error codes; obstructed/private pages are unscored.
    - B1, B2, B5, B7, B8, B9, B10 (3 pts each): not measured by the harness.
    """
    kinds = steps.get("kinds") or {}
    canvas_pages = {"board", "schematic", "3d", "gerbers", "split", "mech", "split-after-mech"}

    b3_checked = 0
    b3_clean = 0
    b3_unscored = 0
    b3_misplaced: list[str] = []

    b4_checked = 0
    b4_clean = 0
    b4_unscored = 0
    b4_violations: list[str] = []

    b6_loading_checked = 0
    b6_loading_clean = 0
    b6_loading_violations: list[str] = []

    b6_empty_checked = 0
    b6_empty_clean = 0
    b6_empty_violations: list[str] = []

    b6_error_checked = 0
    b6_error_clean = 0
    b6_error_violations: list[str] = []

    b6_unscored_private = 0

    for kind, kdata in kinds.items():
        pages = kdata.get("pages") or {}
        for slug, pg in pages.items():
            if slug == "idle":
                continue

            rstate = pg.get("rail_state") or ""
            is_priv = bool(pg.get("private") or pg.get("obstructed"))
            opened_unobstructed = (
                rstate not in ("disabled", "hidden")
                and bool(pg.get("matched_expected"))
                and not is_priv
            )

            # B3: Known misplaced controls on unobstructed opened pages only
            if rstate not in ("disabled", "hidden"):
                if is_priv or not pg.get("matched_expected"):
                    b3_unscored += 1
                else:
                    ctls = pg.get("controls_ids")
                    viewer = pg.get("viewer") or {}
                    if isinstance(ctls, list) and ctls:
                        b3_checked += 1
                        bad = []
                        non_design_context = (slug in M.NON_DESIGN_PAGES) or (kind in ("no-design", "mech-only"))
                        if non_design_context:
                            for leaked in ("ui:Open in KiCad", "ui:Open in Altium", "ui:Edit"):
                                if leaked in ctls:
                                    bad.append(leaked)
                        if slug not in M.FOLLOW_PAGES and viewer.get("follow_present"):
                            bad.append("follow-agent")
                        if bad:
                            b3_misplaced.append(f"{kind}/{slug}: {', '.join(bad)}")
                        else:
                            b3_clean += 1

            # B4: Desktop availability on absent resources (unobstructed)
            if not pg.get("resource_present", True):
                if is_priv:
                    b4_unscored += 1
                else:
                    b4_checked += 1
                    sstate = pg.get("settled_state") or ""
                    if rstate in ("disabled", "hidden") and sstate != "refused":
                        b4_clean += 1
                    else:
                        b4_violations.append(f"{kind}/{slug}: rail_state={rstate or 'unknown'} settled={sstate}")

            # B6: Empty / loading / error separately, ONLY on unobstructed non-private opened pages.
            # Disabled/hidden entries are NOT page states and do not count toward B6.
            if rstate in ("disabled", "hidden"):
                continue
            if is_priv:
                b6_unscored_private += 1
                continue
            if not opened_unobstructed:
                continue

            sstate = pg.get("settled_state") or ""
            # Loading (1 pt): verified early capture record (early_capture_ok == True), not a constructed path
            b6_loading_checked += 1
            early_ok = (pg.get("early_capture_ok") is True) and not bool(pg.get("early_300ms_private") or pg.get("early_300ms_obstructed"))
            if early_ok:
                b6_loading_clean += 1
            else:
                b6_loading_violations.append(f"{kind}/{slug}: early_capture_ok={pg.get('early_capture_ok')}")

            # Empty (1 pt): empty-state opened pages settle cleanly as "empty" (and ready canvas pages are non-flat)
            if sstate == "empty":
                b6_empty_checked += 1
                err_txt = str((pg.get("viewer") or {}).get("error") or "")
                if not RAW_ERROR_RE.search(err_txt):
                    b6_empty_clean += 1
                else:
                    b6_empty_violations.append(f"{kind}/{slug}: raw error in empty state ({err_txt})")
            elif sstate == "ready" and slug in canvas_pages and pg.get("body_non_flat") is False:
                b6_empty_checked += 1
                b6_empty_violations.append(f"{kind}/{slug}: blank canvas body in ready state")

            # Error (1 pt): check error/refused states for raw internal codes
            err_txt = str((pg.get("viewer") or {}).get("error") or "")
            if sstate in ("error", "refused") or err_txt:
                b6_error_checked += 1
                if not RAW_ERROR_RE.search(err_txt) and sstate != "refused":
                    b6_error_clean += 1
                else:
                    b6_error_violations.append(f"{kind}/{slug}: state={sstate} error={err_txt!r}")

    def pts_of(clean: int, total: int, max_pts: float) -> float | None:
        if total == 0:
            return None
        return round(max_pts * (clean / total), 2)

    b6_loading_pts = pts_of(b6_loading_clean, b6_loading_checked, 1.0)
    b6_empty_pts = pts_of(b6_empty_clean, b6_empty_checked, 1.0)
    b6_error_pts = pts_of(b6_error_clean, b6_error_checked, 1.0)
    b6_parts = [p for p in (b6_loading_pts, b6_empty_pts, b6_error_pts) if p is not None]
    b6_total = round(sum(b6_parts), 2) if b6_parts else None
    b6_measured_max = float(len(b6_parts))

    return {
        "B3": {
            "label": "Known misplaced controls found (partial check; not a whole-rule score)",
            "max_points": 3,
            "checked": b3_checked,
            "clean": b3_clean,
            "unscored_obstructed_or_unopened": b3_unscored,
            "misplaced_count": len(b3_misplaced),
            "misplaced": b3_misplaced,
            "points": None,  # Explicitly partial: do not report as a whole-rule score
        },
        "B4": {
            "label": "Desktop availability (phones not measured)",
            "max_points": 3,
            "checked": b4_checked,
            "clean": b4_clean,
            "unscored_private": b4_unscored,
            "points": pts_of(b4_clean, b4_checked, 3.0),
            "violations": b4_violations,
        },
        "B6": {
            "label": "Empty / loading / error states (1 pt each; unobstructed opened pages only)",
            "max_points": 3,
            "measured_max_points": b6_measured_max,
            "points": b6_total,
            "unscored_private_or_obstructed": b6_unscored_private,
            "loading": {"checked": b6_loading_checked, "clean": b6_loading_clean, "points": b6_loading_pts, "max_points": 1, "violations": b6_loading_violations},
            "empty": {"checked": b6_empty_checked, "clean": b6_empty_clean, "points": b6_empty_pts, "max_points": 1, "violations": b6_empty_violations},
            "error": {"checked": b6_error_checked, "clean": b6_error_clean, "points": b6_error_pts, "max_points": 1, "violations": b6_error_violations},
        },
        "not_measured": [{"rule": code, "name": name, "max_points": 3, "status": "not measured by the harness"} for code, name in UNMEASURED_RULES],
    }


def make_pair_image(base_png: Path, new_png: Path, out_png: Path, title: str, base_label: str, new_label: str) -> None:
    font = load_font(13)
    with Image.open(base_png) as ib, Image.open(new_png) as inew:
        b = ib.convert("RGB")
        n = inew.convert("RGB")
        tw = 720
        bh = max(1, int(round(b.height * (tw / b.width))))
        nh = max(1, int(round(n.height * (tw / n.width))))
        b = b.resize((tw, bh))
        n = n.resize((tw, nh))
        head_h = 44
        gap = 8
        canvas = Image.new("RGB", (tw * 2 + gap * 3, head_h + max(bh, nh) + gap), (24, 26, 31))
        draw = ImageDraw.Draw(canvas)
        draw.text((gap, 6), title, fill=(236, 240, 248), font=font)
        draw.text((gap, 24), f"BASE: {base_label}", fill=(180, 190, 206), font=font)
        draw.text((tw + gap * 2, 24), f"NEW:  {new_label}", fill=(160, 224, 180), font=font)
        canvas.paste(b, (gap, head_h))
        canvas.paste(n, (tw + gap * 2, head_h))
        out_png.parent.mkdir(parents=True, exist_ok=True)
        canvas.save(out_png)


def make_strip_compare(base_png: Path, new_png: Path, out_png: Path, label: str) -> bool:
    if not base_png.exists() or not new_png.exists():
        return False
    font = load_font(14)
    with Image.open(base_png) as ib, Image.open(new_png) as inew:
        b = ib.convert("RGB")
        n = inew.convert("RGB")
        head_h = 28
        gap = 12
        canvas = Image.new("RGB", (b.width + n.width + gap * 3, head_h + max(b.height, n.height) + gap), (24, 26, 31))
        draw = ImageDraw.Draw(canvas)
        draw.text((gap, 6), f"BASE {label}", fill=(180, 190, 206), font=font)
        draw.text((b.width + gap * 2, 6), f"NEW {label}", fill=(160, 224, 180), font=font)
        canvas.paste(b, (gap, head_h))
        canvas.paste(n, (b.width + gap * 2, head_h))
        canvas.save(out_png)
    return True


def format_delta(new_v: float | None, base_v: float | None) -> str:
    if new_v is None or base_v is None:
        return "unscored"
    d = round(new_v - base_v, 2)
    return f"+{d:g}" if d > 0 else f"{d:g}"


def fmt_task_cell(t: dict) -> str:
    if not t.get("scored"):
        return f"`{t.get('outcome')}` (unscored)"
    um = t.get("unmeasured_parts") or []
    um_s = f"; unmeasured: {','.join(um)}" if um else ""
    return f"`{t['outcome']}` ({t['presses']}/{t['budget']}, {t['points']}/4pt{um_s})"


def main(argv: list[str] | None = None) -> int:
    raw = list(sys.argv[1:] if argv is None else argv)
    share = "--share" in raw
    args = [a for a in raw if a != "--share"]
    if len(args) < 2 or len(args) > 3:
        print("usage: rail_shots_compare.py [--share] BASE_DIR NEW_DIR [OUT_DIR]", file=sys.stderr)
        return 2

    base_dir = Path(args[0]).resolve()
    new_dir = Path(args[1]).resolve()
    out_dir = Path(args[2]).resolve() if len(args) > 2 else (new_dir / "compare")

    # Refuse a non-empty OUT_DIR so stale comparison outputs can never contaminate a run.
    if out_dir.exists() and any(out_dir.iterdir()):
        print(f"rail-shots compare: refusing non-empty OUT_DIR: {out_dir} (use a fresh directory)", file=sys.stderr)
        return 2

    base_steps_p = base_dir / "steps.json"
    new_steps_p = new_dir / "steps.json"
    if not base_steps_p.exists() or not new_steps_p.exists():
        print(f"rail-shots compare: missing steps.json in {base_dir} or {new_dir}", file=sys.stderr)
        return 2

    base_steps = json.loads(base_steps_p.read_text(encoding="utf-8"))
    new_steps = json.loads(new_steps_p.read_text(encoding="utf-8"))

    base_priv_ok = has_complete_privacy(base_steps)
    new_priv_ok = has_complete_privacy(new_steps)
    private_all = collect_private_shots(base_steps, "base") + collect_private_shots(new_steps, "new")

    if share:
        if not base_priv_ok or not new_priv_ok:
            print(
                f"rail-shots compare: --share refused: incomplete privacy metadata "
                f"(base privacy_checked={base_priv_ok}, new privacy_checked={new_priv_ok})",
                file=sys.stderr,
            )
            return 3
        if private_all:
            print(f"rail-shots compare: --share refused ({len(private_all)} private shot(s)):", file=sys.stderr)
            for item in private_all:
                print(f"  - {item}", file=sys.stderr)
            return 3

    out_dir.mkdir(parents=True, exist_ok=True)
    pairs_dir = out_dir / "pairs"
    pairs_dir.mkdir(parents=True, exist_ok=True)

    # Per-page before/after pairs strictly from cleared (non-private, unobstructed) shots
    pairs_written: list[str] = []
    pairs_skipped_private: list[str] = []
    common_kinds = [k for k in M.KINDS if k in (base_steps.get("kinds") or {}) and k in (new_steps.get("kinds") or {})]
    for kind in common_kinds:
        b_kdata = base_steps["kinds"][kind]
        n_kdata = new_steps["kinds"][kind]
        b_pages = b_kdata.get("pages") or {}
        n_pages = n_kdata.get("pages") or {}
        b_cleared = set(b_kdata["cleared_shots"]) if "cleared_shots" in b_kdata else None
        n_cleared = set(n_kdata["cleared_shots"]) if "cleared_shots" in n_kdata else None
        for slug, _, _, _ in M.PAGES:
            b_png = base_dir / kind / f"{slug}.png"
            n_png = new_dir / kind / f"{slug}.png"
            if not b_png.exists() or not n_png.exists():
                continue
            bp = b_pages.get(slug, {})
            np = n_pages.get(slug, {})
            b_is_priv = bool(bp.get("private") or bp.get("obstructed") or (b_cleared is not None and slug not in b_cleared))
            n_is_priv = bool(np.get("private") or np.get("obstructed") or (n_cleared is not None and slug not in n_cleared))
            if b_is_priv or n_is_priv:
                pairs_skipped_private.append(f"{kind}/{slug}")
                continue
            b_lbl = f"state={bp.get('settled_state', '?')} rail={bp.get('rail_state', '?')}"
            n_lbl = f"state={np.get('settled_state', '?')} rail={np.get('rail_state', '?')}"
            out_png = pairs_dir / f"{kind}-{slug}.png"
            make_pair_image(b_png, n_png, out_png, f"{kind} / {slug}", b_lbl, n_lbl)
            pairs_written.append(str(out_png))

    # Only compose strip comparisons when BOTH runs have schema>=2 privacy-cleared strips.
    strips_allowed = base_priv_ok and new_priv_ok
    headers_cmp = make_strip_compare(base_dir / "headers.png", new_dir / "headers.png", out_dir / "headers-compare.png", "headers.png") if strips_allowed else False
    rails_cmp = make_strip_compare(base_dir / "rails.png", new_dir / "rails.png", out_dir / "rails-compare.png", "rails.png") if strips_allowed else False

    comparable, incomp_reasons = check_comparability(base_steps, new_steps)
    b_tasks = score_tasks(base_steps)
    n_tasks = score_tasks(new_steps)
    common_tasks = score_common_tasks(b_tasks, n_tasks)
    b_cons = score_consistency(base_steps)
    n_cons = score_consistency(new_steps)

    report = {
        "base_dir": str(base_dir),
        "new_dir": str(new_dir),
        "out_dir": str(out_dir),
        "comparable": comparable,
        "incomparability_reasons": incomp_reasons,
        "strips_composed": strips_allowed,
        "pairs_written": len(pairs_written),
        "pairs_skipped_private": pairs_skipped_private,
        "headers_compare": str(out_dir / "headers-compare.png") if headers_cmp else None,
        "rails_compare": str(out_dir / "rails-compare.png") if rails_cmp else None,
        "tasks": {
            "base": b_tasks,
            "new": n_tasks,
            "whole_run_delta": (n_tasks["points"] - b_tasks["points"]) if (comparable and b_tasks["scored_max_points"] == n_tasks["scored_max_points"]) else None,
            "common_tasks": common_tasks,
        },
        "consistency": {"base": b_cons, "new": n_cons},
    }
    (out_dir / "score.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")

    lines = [
        "# Rail Shots Compare",
        "",
        f"- **Base**: `{base_dir}` (overall: `{base_steps.get('overall_outcome', '?')}`, schema={base_steps.get('evidence_schema', 1)})",
        f"- **New**:  `{new_dir}` (overall: `{new_steps.get('overall_outcome', '?')}`, schema={new_steps.get('evidence_schema', 1)})",
        f"- **Comparable**: `{comparable}`" + (f" ({'; '.join(incomp_reasons)})" if incomp_reasons else ""),
        f"- **Pairs written**: {len(pairs_written)} (`{pairs_dir}`), skipped private: {len(pairs_skipped_private)}",
        f"- **Strip comparisons (`headers-compare.png`, `rails-compare.png`)**: {'written (both runs privacy-cleared)' if strips_allowed else 'omitted (one or both runs lack schema>=2 privacy-cleared strips)'}",
        "",
    ]

    if not comparable:
        lines.extend([
            "> **Incomparable runs — whole-run scored delta refused.**",
            "> Reasons: " + "; ".join(incomp_reasons),
            "",
        ])

    lines.extend([
        "## Task Score Summary",
        "",
        f"- **Common scored tasks** ({len(common_tasks['common_task_ids'])} tasks: {common_tasks['common_task_ids']}): "
        f"Base **{common_tasks['base_points']}/{common_tasks['max_points']}** vs New **{common_tasks['new_points']}/{common_tasks['max_points']}** "
        f"(delta `{format_delta(common_tasks['new_points'], common_tasks['base_points'])}`)",
        f"- **Base scored tasks**: {b_tasks['points']}/{b_tasks['scored_max_points']} ({b_tasks['unscored_tasks']} unscored)",
        f"- **New scored tasks**: {n_tasks['points']}/{n_tasks['scored_max_points']} ({n_tasks['unscored_tasks']} unscored)",
        "",
        "| # | Task | Budget | Base | New | Delta |",
        "|---|---|---:|---|---|---:|",
    ])
    b_by_id = {t["task"]: t for t in b_tasks["tasks"]}
    n_by_id = {t["task"]: t for t in n_tasks["tasks"]}
    for tid in range(1, 11):
        bt = b_by_id[tid]
        nt = n_by_id[tid]
        name = nt.get("name") or bt.get("name") or f"Task {tid}"
        budget = nt.get("budget") or bt.get("budget") or 0
        delta_s = format_delta(nt["points"], bt["points"]) if (bt["scored"] and nt["scored"]) else "unscored"
        lines.append(f"| {tid} | {name} | {budget} | {fmt_task_cell(bt)} | {fmt_task_cell(nt)} | {delta_s} |")

    def fmt_opt_pts(val: float | None, mx: float) -> str:
        return "unscored" if val is None else f"{val:g}/{mx:g}"

    lines.extend([
        "",
        "## Consistency Rules (10 rules × 3 pts = 30 pts)",
        "",
        "| Rule | Scope | Base | New | Notes |",
        "|---|---|---|---|---|",
        f"| **B3** | {n_cons['B3']['label']} | {b_cons['B3']['misplaced_count']} misplaced ({b_cons['B3']['checked']} checked) | {n_cons['B3']['misplaced_count']} misplaced ({n_cons['B3']['checked']} checked, {n_cons['B3']['unscored_obstructed_or_unopened']} unscored) | partial check only |",
        f"| **B4** | {n_cons['B4']['label']} | {fmt_opt_pts(b_cons['B4']['points'], 3)} ({b_cons['B4']['clean']}/{b_cons['B4']['checked']}) | {fmt_opt_pts(n_cons['B4']['points'], 3)} ({n_cons['B4']['clean']}/{n_cons['B4']['checked']}, {n_cons['B4']['unscored_private']} unscored) | desktop kinds only |",
        f"| **B6** | {n_cons['B6']['label']} | {fmt_opt_pts(b_cons['B6']['points'], b_cons['B6']['measured_max_points'] or 3)} | {fmt_opt_pts(n_cons['B6']['points'], n_cons['B6']['measured_max_points'] or 3)} ({n_cons['B6']['unscored_private_or_obstructed']} unscored) | loading={fmt_opt_pts(n_cons['B6']['loading']['points'], 1)}, empty={fmt_opt_pts(n_cons['B6']['empty']['points'], 1)}, error={fmt_opt_pts(n_cons['B6']['error']['points'], 1)} |",
    ])
    for r in n_cons["not_measured"]:
        lines.append(f"| **{r['rule']}** | {r['name']} (3 pts) | not measured | not measured | not measured by the harness |")

    lines.extend(["", "## Consistency Findings (New)", ""])
    lines.append(f"### B3: Known misplaced controls found ({n_cons['B3']['misplaced_count']})")
    if not n_cons["B3"]["misplaced"]:
        lines.append("- None")
    else:
        for v in n_cons["B3"]["misplaced"]:
            lines.append(f"- `{v}`")
    lines.append("")

    lines.append(f"### B4: Desktop availability violations ({len(n_cons['B4']['violations'])})")
    if not n_cons["B4"]["violations"]:
        lines.append("- None")
    else:
        for v in n_cons["B4"]["violations"]:
            lines.append(f"- `{v}`")
    lines.append("")

    lines.append("### B6: Page state breakdown (unobstructed opened pages only)")
    for sub in ("loading", "empty", "error"):
        sd = n_cons["B6"][sub]
        lines.append(f"- **{sub}**: {fmt_opt_pts(sd['points'], 1)} ({sd['clean']}/{sd['checked']})" + (f" — violations: {', '.join(sd['violations'])}" if sd["violations"] else ""))
    lines.append("")

    md = "\n".join(lines) + "\n"
    (out_dir / "score.md").write_text(md, encoding="utf-8")
    print(md)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
