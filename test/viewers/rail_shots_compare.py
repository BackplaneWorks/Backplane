#!/usr/bin/env python3
"""Compare two rail-shots.sh output directories: per-page before/after pairs + score table.

Usage:
  rail_shots_compare.py [--share] BASE_DIR NEW_DIR [OUT_DIR]

Writes into a fresh, empty OUT_DIR (default: NEW_DIR/compare):
  pairs/<kind>-<page>.png          side-by-side BASE vs NEW images (positively cleared shots)
  pairs/private-<kind>-<page>.png  non-share mode only: pairs lacking positive clearance
  headers-compare.png              only when both runs have schema>=2 per-shot privacy clearance
  rails-compare.png                only when both runs have schema>=2 per-shot privacy clearance
  score.json                       structured comparability, common-task & consistency breakdown
  score.md                         human-readable round report
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

# Exact 10 consistency rules from https://github.com/BackplaneWorks/Backplane/issues/1317 (in order and words).
SCORECARD_RULES = [
    ("B1", "One selection behaviour for every rail entry (press, press again, key, palette)"),
    ("B2", "Every page has the same header: what and which file on the left, the page's tools in the middle, the pane's controls on the right, in one order"),
    ("B3", "A control shows only where it applies (no Open in KiCad on Browser or Files)"),
    ("B4", "The entries for a project (board only, schematic only, mech only, no design) come from the hub as one value; desktop, iOS and Android draw the same list"),
    ("B5", "Every entry has an icon, a label, a tooltip, a key and a palette command"),
    ("B6", "Every page has a designed empty, loading and error state; no raw error codes, no blank pane"),
    ("B7", "One split and compare model (which panes, how to add or drop one, how Gerbers and versions compare)"),
    ("B8", "One word per thing everywhere (rail, header, tooltip, palette, phones)"),
    ("B9", "The embedded KiCad tab strip and the rail agree on what is lit, and the strip looks like the pane's header"),
    ("B10", "No two routes to the same thing that disagree (Review's tabs and the strip under the board)"),
]
RULE_BY_ID = dict(SCORECARD_RULES)


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


def has_complete_privacy(steps: dict) -> tuple[bool, list[str]]:
    """Validate per-kind, per-shot privacy clearance (not just top-level flags)."""
    issues: list[str] = []
    if int(steps.get("evidence_schema") or 1) < 2:
        issues.append(f"evidence_schema={steps.get('evidence_schema', 1)} (< 2)")
    if steps.get("privacy_checked") is not True:
        issues.append("privacy_checked is not true")
    kinds = steps.get("kinds")
    if not isinstance(kinds, dict) or not kinds:
        issues.append("no kinds present")
        return False, issues
    for kind, kdata in kinds.items():
        if not isinstance(kdata, dict):
            issues.append(f"{kind}: invalid kind entry")
            continue
        cleared = kdata.get("cleared_shots")
        priv = kdata.get("private_shots")
        missing = set(kdata.get("missing_shots") or [])
        if not isinstance(cleared, list) or not isinstance(priv, dict):
            issues.append(f"{kind}: missing per-shot cleared_shots/private_shots")
            continue
        cleared_set = set(cleared)
        priv_set = set(priv.keys())
        pages = kdata.get("pages") or {}
        for slug, pg in pages.items():
            if not isinstance(pg, dict) or not isinstance(pg.get("private"), bool):
                issues.append(f"{kind}/{slug}: missing boolean private field")
                continue
            if slug not in cleared_set and slug not in priv_set and slug not in missing:
                issues.append(f"{kind}/{slug}: not accounted for in cleared_shots/private_shots/missing_shots")
    return len(issues) == 0, issues


def collect_private_shots(steps: dict, prefix: str) -> list[str]:
    out: list[str] = []
    for kind, kdata in (steps.get("kinds") or {}).items():
        if "private_shots" in kdata and isinstance(kdata["private_shots"], dict):
            for sname, reasons in kdata["private_shots"].items():
                out.append(f"{prefix}/{kind}/{sname}: {'; '.join(reasons)}")
        else:
            for sname in (kdata.get("obstructions") or {}).keys():
                out.append(f"{prefix}/{kind}/{sname}: obstructed (legacy manifest)")
    return out


def check_comparability(base_steps: dict, new_steps: dict) -> tuple[bool, list[str]]:
    """Refuse a whole-run scored delta when schema, embed mode, runtime capabilities, kinds, or task defs differ.

    Missing runtime evidence is unknown (not []); two unknown runtimes are NOT comparable.
    """
    reasons: list[str] = []

    b_schema = int(base_steps.get("evidence_schema") or 1)
    n_schema = int(new_steps.get("evidence_schema") or 1)
    if b_schema != n_schema:
        reasons.append(f"evidence_schema differs (base={b_schema}, new={n_schema})")

    b_embed = bool(base_steps.get("embed_mode"))
    n_embed = bool(new_steps.get("embed_mode"))
    if b_embed != n_embed:
        reasons.append(f"embed_mode differs (base={b_embed}, new={n_embed})")

    b_rt = base_steps.get("runtime")
    n_rt = new_steps.get("runtime")
    b_caps = b_rt.get("capabilities") if isinstance(b_rt, dict) and isinstance(b_rt.get("capabilities"), list) else None
    n_caps = n_rt.get("capabilities") if isinstance(n_rt, dict) and isinstance(n_rt.get("capabilities"), list) else None
    if b_caps is None or n_caps is None:
        reasons.append(f"runtime capabilities unknown (base={'known' if b_caps is not None else 'unknown'}, new={'known' if n_caps is not None else 'unknown'})")
    elif sorted(b_caps) != sorted(n_caps):
        reasons.append(f"runtime capabilities differ (base={sorted(b_caps)}, new={sorted(n_caps)})")

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
    """Score one task honestly:
    - Four parts recorded independently: found, within_budget, result_evidenced, one_step_back.
    - A failed task keeps the parts it earned in `parts`, while its total points are 0.
    - A passed task earns the sum of evidenced True parts (4 only when all four are evidenced).
    - unavailable / harness_failed / missing are unscored (points=None, never 0).
    """
    if not task:
        return {
            "scored": False,
            "outcome": "missing",
            "points": None,
            "max_points": 0,
            "parts": {},
            "unmeasured_parts": [],
            "presses": 0,
            "budget": 0,
        }
    outcome = str(task.get("outcome") or "unavailable")
    budget = int(task.get("step_budget") or 0)
    presses = int(task.get("presses_used") or 0)

    raw_parts = task.get("parts")
    if isinstance(raw_parts, dict) and raw_parts:
        parts = {
            "found": raw_parts.get("found", False),
            "within_budget": raw_parts.get("within_budget", False),
            "result_evidenced": raw_parts.get("result_evidenced", outcome == "passed"),
            "one_step_back": raw_parts.get("one_step_back", "not_measured"),
        }
    else:
        tid = task.get("task")
        has_return = tid in (3, 8, 10)
        within = bool(task.get("within_budget", presses <= budget))
        parts = {
            "found": bool(task.get("presses") or presses > 0) if outcome not in ("unavailable", "harness_failed") else False,
            "within_budget": within if outcome not in ("unavailable", "harness_failed") else False,
            "result_evidenced": outcome == "passed",
            "one_step_back": (outcome == "passed") if has_return else "not_measured",
        }

    unmeasured = [k for k, v in parts.items() if v == "not_measured"]

    if outcome in ("unavailable", "harness_failed", "missing"):
        return {
            "scored": False,
            "outcome": outcome,
            "points": None,
            "max_points": 0,
            "parts": parts,
            "unmeasured_parts": unmeasured,
            "presses": presses,
            "budget": budget,
            "note": task.get("note", ""),
        }

    if outcome == "task_failed" or parts.get("result_evidenced") is not True:
        return {
            "scored": True,
            "outcome": outcome,
            "points": 0,
            "max_points": 4,
            "parts": parts,
            "unmeasured_parts": unmeasured,
            "presses": presses,
            "budget": budget,
            "note": task.get("note", ""),
        }

    pts = sum(1 for k in ("found", "within_budget", "result_evidenced", "one_step_back") if parts.get(k) is True)
    return {
        "scored": True,
        "outcome": outcome,
        "points": pts,
        "max_points": 4,
        "parts": parts,
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
    scored_ids: list[int] = []
    unscored_count = 0
    for tid in range(1, 11):
        t = by_id.get(tid)
        st = score_single_task(t)
        st["task"] = tid
        st["name"] = (t or {}).get("name", f"Task {tid}")
        if st["scored"]:
            earned += int(st["points"])
            scored_max += int(st["max_points"])
            scored_ids.append(tid)
        else:
            unscored_count += 1
        rows.append(st)
    return {
        "points": earned,
        "scored_max_points": scored_max,
        "scored_task_ids": scored_ids,
        "total_max_points": 40,
        "unscored_tasks": unscored_count,
        "tasks": rows,
    }


def score_common_tasks(base_tasks: dict, new_tasks: dict) -> dict:
    """Compare only tasks scored on BOTH sides whose id, definition (name), and step budget match."""
    b_by_id = {t["task"]: t for t in base_tasks["tasks"]}
    n_by_id = {t["task"]: t for t in new_tasks["tasks"]}
    common_ids: list[int] = []
    mismatched_defs: list[int] = []
    b_pts = 0
    n_pts = 0
    max_pts = 0
    for tid in range(1, 11):
        bt = b_by_id.get(tid, {})
        nt = n_by_id.get(tid, {})
        if not (bt.get("scored") and nt.get("scored")):
            continue
        if bt.get("name") != nt.get("name") or bt.get("budget") != nt.get("budget"):
            mismatched_defs.append(tid)
            continue
        common_ids.append(tid)
        b_pts += int(bt["points"])
        n_pts += int(nt["points"])
        max_pts += 4
    return {
        "common_task_ids": common_ids,
        "excluded_definition_mismatch": mismatched_defs,
        "base_points": b_pts,
        "new_points": n_pts,
        "max_points": max_pts,
        "delta": n_pts - b_pts,
    }


def score_consistency(steps: dict) -> dict:
    """Evaluate measurable consistency rules at 3 points each (10 rules of issue #1317).

    - B3 (partial check, no whole-rule score): Known misplaced controls on unobstructed opened pages.
    - B4 (partial check, no whole-rule score): Desktop absent-resource availability only (phones not measured).
    - B6 (3 pts = 1 loading + 1 empty + 1 error): Strictly unobstructed, non-private opened pages.
      * loading: credit ONLY when early_capture_ok == True AND early_300ms_state == "loading" with non-empty loading text;
        a generic early capture without a designed loading state is unscored (never a pass).
      * empty: credit ONLY when settled_state == "empty" with non-empty empty-state words and no raw error code;
        a blank canvas body in a ready state is a violation (never credit for empty).
      * error: checks error/refused states for raw internal codes; unscored when none triggered.
    - B1, B2, B5, B7, B8, B9, B10: not measured by the harness.
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
    b6_loading_unscored = 0
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

            # B3: Known misplaced controls on unobstructed opened pages only (partial check)
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

            # B4: Desktop absent-resource availability only (partial check; phones not measured)
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

            # B6: Strictly unobstructed, non-private opened pages (disabled/hidden do not count)
            if rstate in ("disabled", "hidden"):
                continue
            if is_priv:
                b6_unscored_private += 1
                continue
            if not opened_unobstructed:
                continue

            sstate = pg.get("settled_state") or ""

            # Loading (1 pt): credit ONLY when early capture is verified AND reports a designed loading state + text.
            early_ok = (pg.get("early_capture_ok") is True) and not bool(pg.get("early_300ms_private") or pg.get("early_300ms_obstructed"))
            early_state = str(pg.get("early_300ms_state") or "")
            early_text = str(pg.get("early_loading_text") or "").strip()
            if early_state == "loading":
                b6_loading_checked += 1
                if early_ok and early_text:
                    b6_loading_clean += 1
                else:
                    b6_loading_violations.append(f"{kind}/{slug}: early_capture_ok={early_ok} loading_text={early_text!r}")
            else:
                # Generic early capture (e.g. already ready or no reported loading state): unscored, never a pass.
                b6_loading_unscored += 1

            # Empty (1 pt): credit ONLY when page reports settled_state == "empty" with empty-state words;
            # a blank canvas body in ready state is a violation.
            if sstate == "empty":
                b6_empty_checked += 1
                err_txt = str((pg.get("viewer") or {}).get("error") or "")
                empty_txt = str(pg.get("empty_text") or "").strip()
                if empty_txt and not RAW_ERROR_RE.search(err_txt):
                    b6_empty_clean += 1
                else:
                    b6_empty_violations.append(f"{kind}/{slug}: empty_text={empty_txt!r} error={err_txt!r}")
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
            "rule": RULE_BY_ID["B3"],
            "scope": "Known misplaced controls found (partial check; not a whole-rule score)",
            "max_points": 3,
            "checked": b3_checked,
            "clean": b3_clean,
            "unscored_obstructed_or_unopened": b3_unscored,
            "misplaced_count": len(b3_misplaced),
            "misplaced": b3_misplaced,
            "points": None,
        },
        "B4": {
            "rule": RULE_BY_ID["B4"],
            "scope": "Desktop absent-resource availability only (partial check; phones not measured)",
            "max_points": 3,
            "checked": b4_checked,
            "clean": b4_clean,
            "unscored_private": b4_unscored,
            "violations": b4_violations,
            "points": None,
        },
        "B6": {
            "rule": RULE_BY_ID["B6"],
            "scope": "Empty / loading / error states (1 pt each; unobstructed opened pages only)",
            "max_points": 3,
            "measured_max_points": b6_measured_max,
            "points": b6_total,
            "unscored_private_or_obstructed": b6_unscored_private,
            "loading": {"checked": b6_loading_checked, "clean": b6_loading_clean, "unscored_non_loading": b6_loading_unscored, "points": b6_loading_pts, "max_points": 1, "violations": b6_loading_violations},
            "empty": {"checked": b6_empty_checked, "clean": b6_empty_clean, "points": b6_empty_pts, "max_points": 1, "violations": b6_empty_violations},
            "error": {"checked": b6_error_checked, "clean": b6_error_clean, "points": b6_error_pts, "max_points": 1, "violations": b6_error_violations},
        },
        "rules": [
            {"rule": code, "text": text, "max_points": 3, "status": "measured_partial" if code in ("B3", "B4", "B6") else "not measured by the harness"}
            for code, text in SCORECARD_RULES
        ],
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
    parts = t.get("parts") or {}
    earned_parts = [k for k in ("found", "within_budget", "result_evidenced", "one_step_back") if parts.get(k) is True]
    fail_parts = f"; parts: {','.join(earned_parts)}" if t.get("outcome") == "task_failed" and earned_parts else ""
    return f"`{t['outcome']}` ({t['presses']}/{t['budget']}, {t['points']}/4pt{fail_parts}{um_s})"


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

    base_priv_ok, base_priv_issues = has_complete_privacy(base_steps)
    new_priv_ok, new_priv_issues = has_complete_privacy(new_steps)
    private_all = collect_private_shots(base_steps, "base") + collect_private_shots(new_steps, "new")

    if share:
        if not base_priv_ok or not new_priv_ok:
            print(
                f"rail-shots compare: --share refused: incomplete per-shot privacy clearance "
                f"(base={base_priv_issues or 'ok'}, new={new_priv_issues or 'ok'})",
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

    # Per-page before/after pairs: require positive per-shot clearance for normal pairs;
    # in non-share mode, uncleared/legacy/private pairs are marked private and named `private-...`.
    cleared_pairs: list[str] = []
    private_pairs: list[str] = []
    common_kinds = [k for k in M.KINDS if k in (base_steps.get("kinds") or {}) and k in (new_steps.get("kinds") or {})]
    for kind in common_kinds:
        b_kdata = base_steps["kinds"][kind]
        n_kdata = new_steps["kinds"][kind]
        b_pages = b_kdata.get("pages") or {}
        n_pages = n_kdata.get("pages") or {}
        b_cleared = set(b_kdata["cleared_shots"]) if isinstance(b_kdata.get("cleared_shots"), list) else None
        n_cleared = set(n_kdata["cleared_shots"]) if isinstance(n_kdata.get("cleared_shots"), list) else None
        for slug, _, _, _ in M.PAGES:
            b_png = base_dir / kind / f"{slug}.png"
            n_png = new_dir / kind / f"{slug}.png"
            if not b_png.exists() or not n_png.exists():
                continue
            bp = b_pages.get(slug, {})
            np = n_pages.get(slug, {})
            b_pos_clear = (b_cleared is not None and slug in b_cleared and not bp.get("private") and not bp.get("obstructed"))
            n_pos_clear = (n_cleared is not None and slug in n_cleared and not np.get("private") and not np.get("obstructed"))
            b_lbl = f"state={bp.get('settled_state', '?')} rail={bp.get('rail_state', '?')}"
            n_lbl = f"state={np.get('settled_state', '?')} rail={np.get('rail_state', '?')}"
            if b_pos_clear and n_pos_clear:
                out_png = pairs_dir / f"{kind}-{slug}.png"
                make_pair_image(b_png, n_png, out_png, f"{kind} / {slug}", b_lbl, n_lbl)
                cleared_pairs.append(str(out_png))
            else:
                out_png = pairs_dir / f"private-{kind}-{slug}.png"
                make_pair_image(b_png, n_png, out_png, f"[PRIVATE / UNCLEARED] {kind} / {slug}", b_lbl, n_lbl)
                private_pairs.append(str(out_png))

    strips_allowed = base_priv_ok and new_priv_ok
    headers_cmp = make_strip_compare(base_dir / "headers.png", new_dir / "headers.png", out_dir / "headers-compare.png", "headers.png") if strips_allowed else False
    rails_cmp = make_strip_compare(base_dir / "rails.png", new_dir / "rails.png", out_dir / "rails-compare.png", "rails.png") if strips_allowed else False

    comparable, incomp_reasons = check_comparability(base_steps, new_steps)
    b_tasks = score_tasks(base_steps)
    n_tasks = score_tasks(new_steps)
    common_tasks = score_common_tasks(b_tasks, n_tasks)
    b_cons = score_consistency(base_steps)
    n_cons = score_consistency(new_steps)

    # Whole-run delta requires comparability AND identical sets of SCORED task ids on both sides.
    same_scored_ids = b_tasks["scored_task_ids"] == n_tasks["scored_task_ids"]
    if not same_scored_ids and comparable:
        comparable = False
        incomp_reasons.append(f"scored task id sets differ (base={b_tasks['scored_task_ids']}, new={n_tasks['scored_task_ids']})")
    whole_run_delta = (n_tasks["points"] - b_tasks["points"]) if (comparable and same_scored_ids) else None

    report = {
        "base_dir": str(base_dir),
        "new_dir": str(new_dir),
        "out_dir": str(out_dir),
        "comparable": comparable,
        "incomparability_reasons": incomp_reasons,
        "strips_composed": strips_allowed,
        "cleared_pairs": cleared_pairs,
        "private_pairs": private_pairs,
        "pairs_written": len(cleared_pairs),
        "pairs_private": len(private_pairs),
        "headers_compare": str(out_dir / "headers-compare.png") if headers_cmp else None,
        "rails_compare": str(out_dir / "rails-compare.png") if rails_cmp else None,
        "tasks": {
            "base": b_tasks,
            "new": n_tasks,
            "whole_run_delta": whole_run_delta,
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
        f"- **Cleared pairs**: {len(cleared_pairs)}, **private/uncleared pairs (`private-*`)**: {len(private_pairs)} (`{pairs_dir}`)",
        f"- **Strip comparisons (`headers-compare.png`, `rails-compare.png`)**: {'written (both runs per-shot privacy-cleared)' if strips_allowed else 'omitted (one or both runs lack per-shot schema>=2 privacy clearance)'}",
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
        same_def = (bt.get("name") == nt.get("name") and bt.get("budget") == nt.get("budget"))
        delta_s = format_delta(nt["points"], bt["points"]) if (bt["scored"] and nt["scored"] and same_def) else "unscored"
        lines.append(f"| {tid} | {name} | {budget} | {fmt_task_cell(bt)} | {fmt_task_cell(nt)} | {delta_s} |")

    def fmt_opt_pts(val: float | None, mx: float) -> str:
        return "unscored" if val is None else f"{val:g}/{mx:g}"

    lines.extend([
        "",
        "## Consistency Rules (issue #1317: 10 rules × 3 pts = 30 pts)",
        "",
        "| Rule | Issue #1317 Rule (3 pts each) | Base | New | Notes |",
        "|---|---|---|---|---|",
    ])
    for code, text in SCORECARD_RULES:
        if code == "B3":
            lines.append(
                f"| **B3** | {text} | {b_cons['B3']['misplaced_count']} misplaced ({b_cons['B3']['checked']} checked) | "
                f"{n_cons['B3']['misplaced_count']} misplaced ({n_cons['B3']['checked']} checked, {n_cons['B3']['unscored_obstructed_or_unopened']} unscored) | partial check only |"
            )
        elif code == "B4":
            lines.append(
                f"| **B4** | {text} | {len(b_cons['B4']['violations'])} violations ({b_cons['B4']['checked']} checked) | "
                f"{len(n_cons['B4']['violations'])} violations ({n_cons['B4']['checked']} checked, {n_cons['B4']['unscored_private']} unscored) | partial check: desktop absent resources only |"
            )
        elif code == "B6":
            lines.append(
                f"| **B6** | {text} | {fmt_opt_pts(b_cons['B6']['points'], b_cons['B6']['measured_max_points'] or 3)} | "
                f"{fmt_opt_pts(n_cons['B6']['points'], n_cons['B6']['measured_max_points'] or 3)} ({n_cons['B6']['unscored_private_or_obstructed']} unscored) | "
                f"loading={fmt_opt_pts(n_cons['B6']['loading']['points'], 1)}, empty={fmt_opt_pts(n_cons['B6']['empty']['points'], 1)}, error={fmt_opt_pts(n_cons['B6']['error']['points'], 1)} |"
            )
        else:
            lines.append(f"| **{code}** | {text} | not measured | not measured | not measured by the harness |")

    lines.extend(["", "## Consistency Findings (New)", ""])
    lines.append(f"### B3: Known misplaced controls found ({n_cons['B3']['misplaced_count']})")
    if not n_cons["B3"]["misplaced"]:
        lines.append("- None")
    else:
        for v in n_cons["B3"]["misplaced"]:
            lines.append(f"- `{v}`")
    lines.append("")

    lines.append(f"### B4: Desktop absent-resource availability violations ({len(n_cons['B4']['violations'])})")
    if not n_cons["B4"]["violations"]:
        lines.append("- None")
    else:
        for v in n_cons["B4"]["violations"]:
            lines.append(f"- `{v}`")
    lines.append("")

    lines.append("### B6: Page state breakdown (unobstructed opened pages only)")
    for sub in ("loading", "empty", "error"):
        sd = n_cons["B6"][sub]
        extra = f", {sd['unscored_non_loading']} generic early unscored" if sub == "loading" else ""
        lines.append(f"- **{sub}**: {fmt_opt_pts(sd['points'], 1)} ({sd['clean']}/{sd['checked']}{extra})" + (f" — violations: {', '.join(sd['violations'])}" if sd["violations"] else ""))
    lines.append("")

    md = "\n".join(lines) + "\n"
    (out_dir / "score.md").write_text(md, encoding="utf-8")
    print(md)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
