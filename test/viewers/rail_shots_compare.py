#!/usr/bin/env python3
"""Compare two rail-shots.sh output directories: per-page before/after pairs + score table.

Usage:
  rail_shots_compare.py [--share] BASE_DIR NEW_DIR [OUT_DIR]

Writes into OUT_DIR (default: NEW_DIR/compare):
  pairs/<kind>-<page>.png   side-by-side BASE vs NEW images
  headers-compare.png       BASE/headers.png vs NEW/headers.png
  rails-compare.png         BASE/rails.png vs NEW/rails.png
  score.json                structured score and delta breakdown
  score.md                  human-readable round report (tasks 40 pts + consistency rules)
"""
from __future__ import annotations

import json
import os
import sys
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

sys.path.insert(0, str(Path(__file__).resolve().parent))
import rail_shots_manifest as M  # noqa: E402


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


def task_points(task: dict) -> int:
    if task.get("outcome") != "passed":
        return 0
    return 4 if task.get("within_budget", True) else 2


def score_tasks(steps: dict) -> dict:
    tasks = steps.get("scorecard_tasks") or []
    rows = []
    total = 0
    for t in tasks:
        pts = task_points(t)
        total += pts
        rows.append({
            "task": t.get("task"),
            "name": t.get("name", ""),
            "budget": t.get("step_budget", 0),
            "presses": t.get("presses_used", 0),
            "outcome": t.get("outcome", "unavailable"),
            "points": pts,
            "note": t.get("note", ""),
        })
    return {"points": total, "max_points": 40, "tasks": rows}


def score_consistency(steps: dict) -> dict:
    """Evaluate measurable consistency rules from steps.json.

    - B3 (Header controls only where they apply): Open in KiCad / Edit absent on non-design
      pages (files, diff, review, browser) and non-design projects (no-design, mech-only);
      Follow agent only on Board, Schematic, Split.
    - B4 (Rail honesty): absent resources are disabled or hidden, never refused.
    - B6 (Empty / loading / error state per page): early shot present, settled state is one of
      ready/empty/error/disabled/hidden (no timeout/refused), and ready pages are not flat/blank.
    """
    kinds = steps.get("kinds") or {}

    b3_checked = 0
    b3_clean = 0
    b3_violations: list[str] = []

    b4_checked = 0
    b4_clean = 0
    b4_violations: list[str] = []

    b6_checked = 0
    b6_clean = 0
    b6_violations: list[str] = []

    valid_settled = {"ready", "empty", "error", "disabled", "hidden"}
    canvas_pages = {"board", "schematic", "3d", "gerbers", "split", "mech", "split-after-mech"}

    for kind, kdata in kinds.items():
        pages = kdata.get("pages") or {}
        for slug, pg in pages.items():
            if slug == "idle":
                continue

            rstate = pg.get("rail_state") or ""

            # B3: Header controls only where they apply (skip disabled/hidden entries where the page did not open)
            ctls = pg.get("controls_ids")
            viewer = pg.get("viewer") or {}
            if isinstance(ctls, list) and ctls and rstate not in ("disabled", "hidden"):
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
                    b3_violations.append(f"{kind}/{slug}: {', '.join(bad)}")
                else:
                    b3_clean += 1

            # B4: Rail honesty for absent resources
            if not pg.get("resource_present", True):
                b4_checked += 1
                sstate = pg.get("settled_state") or ""
                if rstate in ("disabled", "hidden") and sstate != "refused":
                    b4_clean += 1
                else:
                    b4_violations.append(f"{kind}/{slug}: rail_state={rstate or 'unknown'} settled={sstate}")

            # B6: Empty / loading / error state present per page
            b6_checked += 1
            sstate = pg.get("settled_state") or ""
            early_exists = bool(pg.get("early_shot"))
            body_ok = (pg.get("body_non_flat") is not False) if (sstate == "ready" and slug in canvas_pages) else True
            if early_exists and sstate in valid_settled and body_ok:
                b6_clean += 1
            else:
                why = []
                if not early_exists:
                    why.append("no-early-shot")
                if sstate not in valid_settled:
                    why.append(f"state={sstate or 'missing'}")
                if not body_ok:
                    why.append("blank-body")
                b6_violations.append(f"{kind}/{slug}: {','.join(why)}")

    def ratio_pts(clean: int, total: int, max_pts: int = 10) -> float:
        if total == 0:
            return 0.0
        return round(max_pts * (clean / total), 1)

    return {
        "header_controls": {
            "checked": b3_checked,
            "clean": b3_clean,
            "points": ratio_pts(b3_clean, b3_checked),
            "max_points": 10,
            "violations": b3_violations,
        },
        "rail_honesty": {
            "checked": b4_checked,
            "clean": b4_clean,
            "points": ratio_pts(b4_clean, b4_checked),
            "max_points": 10,
            "violations": b4_violations,
        },
        "page_states": {
            "checked": b6_checked,
            "clean": b6_clean,
            "points": ratio_pts(b6_clean, b6_checked),
            "max_points": 10,
            "violations": b6_violations,
        },
    }


def make_pair_image(base_png: Path, new_png: Path, out_png: Path, title: str, base_label: str, new_label: str) -> None:
    font = load_font(13)
    with Image.open(base_png) as ib, Image.open(new_png) as inew:
        b = ib.convert("RGB")
        n = inew.convert("RGB")
        # Scale down to 720px wide each so pairs are fast to inspect side-by-side
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


def collect_private_shots(steps: dict, prefix: str) -> list[str]:
    out: list[str] = []
    for kind, kdata in (steps.get("kinds") or {}).items():
        for sname, reasons in (kdata.get("private_shots") or {}).items():
            out.append(f"{prefix}/{kind}/{sname}: {'; '.join(reasons)}")
        # Fallback for older steps.json without private_shots: obstructed shots are private
        if "private_shots" not in kdata:
            for sname in (kdata.get("obstructions") or {}).keys():
                out.append(f"{prefix}/{kind}/{sname}: obstructed")
    return out


def format_delta(new_v: float, base_v: float) -> str:
    d = round(new_v - base_v, 1)
    if d > 0:
        return f"+{d:g}"
    return f"{d:g}"


def main() -> int:
    raw = sys.argv[1:]
    share = "--share" in raw
    args = [a for a in raw if a != "--share"]
    if len(args) < 2:
        print("usage: rail_shots_compare.py [--share] BASE_DIR NEW_DIR [OUT_DIR]", file=sys.stderr)
        return 2

    base_dir = Path(args[0]).resolve()
    new_dir = Path(args[1]).resolve()
    out_dir = Path(args[2]).resolve() if len(args) > 2 else (new_dir / "compare")

    base_steps_p = base_dir / "steps.json"
    new_steps_p = new_dir / "steps.json"
    if not base_steps_p.exists() or not new_steps_p.exists():
        print(f"rail-shots compare: missing steps.json in {base_dir} or {new_dir}", file=sys.stderr)
        return 2

    base_steps = json.loads(base_steps_p.read_text(encoding="utf-8"))
    new_steps = json.loads(new_steps_p.read_text(encoding="utf-8"))

    private_all = collect_private_shots(base_steps, "base") + collect_private_shots(new_steps, "new")
    if share and private_all:
        print(f"rail-shots compare: --share refused ({len(private_all)} private shot(s)):", file=sys.stderr)
        for item in private_all:
            print(f"  - {item}", file=sys.stderr)
        return 3

    out_dir.mkdir(parents=True, exist_ok=True)
    pairs_dir = out_dir / "pairs"
    pairs_dir.mkdir(parents=True, exist_ok=True)

    # Per-page before/after pairs
    pairs_written: list[str] = []
    pairs_skipped_private: list[str] = []
    common_kinds = [k for k in M.KINDS if k in (base_steps.get("kinds") or {}) and k in (new_steps.get("kinds") or {})]
    for kind in common_kinds:
        b_pages = (base_steps["kinds"][kind].get("pages") or {})
        n_pages = (new_steps["kinds"][kind].get("pages") or {})
        for slug, _, _, _ in M.PAGES:
            b_png = base_dir / kind / f"{slug}.png"
            n_png = new_dir / kind / f"{slug}.png"
            if not b_png.exists() or not n_png.exists():
                continue
            bp = b_pages.get(slug, {})
            np = n_pages.get(slug, {})
            if bp.get("private") or bp.get("obstructed") or np.get("private") or np.get("obstructed"):
                pairs_skipped_private.append(f"{kind}/{slug}")
                continue
            b_lbl = f"state={bp.get('settled_state', '?')} rail={bp.get('rail_state', '?')}"
            n_lbl = f"state={np.get('settled_state', '?')} rail={np.get('rail_state', '?')}"
            out_png = pairs_dir / f"{kind}-{slug}.png"
            make_pair_image(b_png, n_png, out_png, f"{kind} / {slug}", b_lbl, n_lbl)
            pairs_written.append(str(out_png))

    headers_cmp = make_strip_compare(base_dir / "headers.png", new_dir / "headers.png", out_dir / "headers-compare.png", "headers.png")
    rails_cmp = make_strip_compare(base_dir / "rails.png", new_dir / "rails.png", out_dir / "rails-compare.png", "rails.png")

    b_tasks = score_tasks(base_steps)
    n_tasks = score_tasks(new_steps)
    b_cons = score_consistency(base_steps)
    n_cons = score_consistency(new_steps)

    report = {
        "base_dir": str(base_dir),
        "new_dir": str(new_dir),
        "out_dir": str(out_dir),
        "pairs_written": len(pairs_written),
        "pairs_skipped_private": pairs_skipped_private,
        "headers_compare": str(out_dir / "headers-compare.png") if headers_cmp else None,
        "rails_compare": str(out_dir / "rails-compare.png") if rails_cmp else None,
        "tasks": {"base": b_tasks, "new": n_tasks, "delta": n_tasks["points"] - b_tasks["points"]},
        "consistency": {"base": b_cons, "new": n_cons},
    }
    (out_dir / "score.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")

    lines = [
        "# Rail Shots Compare",
        "",
        f"- **Base**: `{base_dir}` (overall: `{base_steps.get('overall_outcome', '?')}`)",
        f"- **New**:  `{new_dir}` (overall: `{new_steps.get('overall_outcome', '?')}`)",
        f"- **Pairs written**: {len(pairs_written)} (`{pairs_dir}`), skipped private: {len(pairs_skipped_private)}",
        "",
        "## Score Summary",
        "",
        "| Category | Base | New | Delta |",
        "|---|---:|---:|---:|",
        f"| Tasks (40) | {b_tasks['points']}/40 | {n_tasks['points']}/40 | {format_delta(n_tasks['points'], b_tasks['points'])} |",
        f"| Header controls only where they apply (B3) | {b_cons['header_controls']['points']}/10 ({b_cons['header_controls']['clean']}/{b_cons['header_controls']['checked']}) | {n_cons['header_controls']['points']}/10 ({n_cons['header_controls']['clean']}/{n_cons['header_controls']['checked']}) | {format_delta(n_cons['header_controls']['points'], b_cons['header_controls']['points'])} |",
        f"| Rail honesty on absent resources (B4) | {b_cons['rail_honesty']['points']}/10 ({b_cons['rail_honesty']['clean']}/{b_cons['rail_honesty']['checked']}) | {n_cons['rail_honesty']['points']}/10 ({n_cons['rail_honesty']['clean']}/{n_cons['rail_honesty']['checked']}) | {format_delta(n_cons['rail_honesty']['points'], b_cons['rail_honesty']['points'])} |",
        f"| Empty / loading / error state per page (B6) | {b_cons['page_states']['points']}/10 ({b_cons['page_states']['clean']}/{b_cons['page_states']['checked']}) | {n_cons['page_states']['points']}/10 ({n_cons['page_states']['clean']}/{n_cons['page_states']['checked']}) | {format_delta(n_cons['page_states']['points'], b_cons['page_states']['points'])} |",
        "",
        "## Tasks (40 pts)",
        "",
        "| # | Task | Budget | Base | New | Delta |",
        "|---|---|---:|---|---|---:|",
    ]
    b_by_id = {t["task"]: t for t in b_tasks["tasks"]}
    n_by_id = {t["task"]: t for t in n_tasks["tasks"]}
    for tid in range(1, 11):
        bt = b_by_id.get(tid, {"name": "", "budget": 0, "presses": 0, "outcome": "missing", "points": 0})
        nt = n_by_id.get(tid, {"name": bt.get("name", ""), "budget": bt.get("budget", 0), "presses": 0, "outcome": "missing", "points": 0})
        name = nt.get("name") or bt.get("name") or f"Task {tid}"
        budget = nt.get("budget") or bt.get("budget") or 0
        lines.append(
            f"| {tid} | {name} | {budget} | `{bt['outcome']}` ({bt['presses']}/{budget}, {bt['points']}pt) | "
            f"`{nt['outcome']}` ({nt['presses']}/{budget}, {nt['points']}pt) | {format_delta(nt['points'], bt['points'])} |"
        )

    lines.extend(["", "## Consistency Violations (New)", ""])
    for key, title in (
        ("header_controls", "Header controls (B3)"),
        ("rail_honesty", "Rail honesty (B4)"),
        ("page_states", "Page states (B6)"),
    ):
        viols = n_cons[key]["violations"]
        lines.append(f"### {title} ({len(viols)})")
        if not viols:
            lines.append("- None")
        else:
            for v in viols[:30]:
                lines.append(f"- `{v}`")
            if len(viols) > 30:
                lines.append(f"- ... and {len(viols) - 30} more")
        lines.append("")

    md = "\n".join(lines) + "\n"
    (out_dir / "score.md").write_text(md, encoding="utf-8")
    print(md)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
