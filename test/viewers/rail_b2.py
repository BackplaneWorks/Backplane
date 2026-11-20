"""Run rail-shots.sh --b2 CLIENT HUB ASSETS EMPTY_OUTDIR for the two-width header check."""
import json
import os
from pathlib import Path
import subprocess
import sys

from functools import lru_cache

from PIL import Image, ImageChops, ImageDraw, ImageFont

from rail_shots_post import parse_captures, parse_client_log, shot_privacy_reasons

IDENTITY = {
    "Board": "ecc83-pp.kicad_pcb", "Schematic": "ecc83-pp.kicad_sch",
    "Gerbers": "mfg/gerbers", "Split": "ecc83-pp.kicad_pcb",
    "Diff": "Thread changes", "Review": "Working copy", "Files": "Project",
}


# A question mark marks a capability- or content-dependent tool. The fixture
# has one board, one root crumb, a real change and the four standard Review tabs.
TOOL_ORDER = {
    "board": ["?boards-btn", "layers-btn", "plot-fit-0"],
    "schematic": ["?boards-btn", "layers-btn", "plot-fit-0", "?sheets-btn"],
    "gerbers": ["?boards-btn", "layers-btn", "plot-fit-0", "gerbers-vs-board",
                "?gerbers-gerbview", "?gerbers-legend", "?gerbers-note"],
    "split": ["?boards-btn", "split-pick-sch", "split-pick-pcb", "split-pick-3d", "?split-probe"],
    "diff": ["git-commit_push", "git-commit_push_pr", "revert-ask"],
    "review": ["rv-0", "rv-1", "rv-2", "rv-3"],
    "files": ["fm-up", "crumb-0", "fm-new", "fm-upload", "fm-zip"],
}


def tool_orders(reports):
    for page, schema in TOOL_ORDER.items():
        wide, narrow = (reports[size][page] for size in ("wide", "minimum"))
        if "tools" not in wide:
            if narrow.get("outcome") == "passed":
                narrow.update(outcome="harness_failed", reason="wide tool reference unavailable")
            continue
        observed = wide["tools"]
        expected = [key.lstrip("?") for key in schema
                    if not key.startswith("?") or key[1:] in observed]
        for result in (wide, narrow):
            if "tools" not in result:
                continue
            ids = result["tools"]
            # Main offers the project editor on these three pages. #1353
            # limits Edit to pages with their own editor. B2 checks placement
            # in either version; applicability is the separate B3 check.
            allowed = {
                "board": [["follow-agent", "ui:Edit"]],
                "schematic": [["follow-agent", "ui:Edit"]],
                "split": [["follow-agent"], ["follow-agent", "ui:Edit"]],
                "review": [[], ["ui:Edit"]], "files": [[], ["ui:Edit"]],
            }.get(page, [[]])
            required = wide["inline_controls"]
            controls = result["inline_controls"]
            controls_ok = required in allowed and (controls == required or
                (result is narrow and page in ("board", "schematic", "split") and not controls))
            overflow = len(ids) < len(expected) or controls != required
            order_ok = (ids == expected and not result["more"]) if result is wide else (
                ids == expected[:len(ids)] and result["more"] == overflow)
            result["checks"]["expected_tools"] = order_ok
            result["checks"]["expected_inline_controls"] = controls_ok
            if not order_ok or not controls_ok:
                result["outcome"] = "task_failed"


def script():
    steps = [{"select": 0}]
    for title in IDENTITY:
        page = title.lower()
        steps.extend([{"mark": page}, {"press": f"rail:{title}"},
                      {"shot": page, "settle": page, "timeout_ms": 20000}])
    return steps


def box(rect):
    x, y, w, h = (rect[k] for k in ("x", "y", "w", "h"))
    return tuple(round(i) for i in (x, y, x + w, y + h))


def contains(outer, inner):
    x, y, r, b = inner
    return outer[0] <= x < r <= outer[2] and outer[1] <= y < b <= outer[3]


def painted(image, bounds):
    # Ignore the control's border. Count contrasting glyph/icon pixels in its interior.
    x, y, r, b = bounds
    if r-x < 5 or b-y < 5:
        return False
    hist = image.crop((x+2, y+2, r-2, b-2)).convert("L").histogram()
    background = max(range(256), key=hist.__getitem__)
    return sum(count for level, count in enumerate(hist) if abs(level-background) >= 32) >= 4


def close_pixels(image, bounds):
    x, y, r, b = bounds
    if r-x < 8 or b-y < 8:
        return False
    icon = image.crop((x+2, y+2, r-2, b-2)).convert("L")
    hist = icon.histogram()
    bg = max(range(256), key=hist.__getitem__)
    ink = icon.point(lambda p: 255 if abs(p-bg) >= 32 else 0)
    if not ink.getbbox():
        return False
    # The two diagonals light the corners and cross in the center.
    bits = ink.crop(ink.getbbox()).resize((3, 3), Image.Resampling.BOX)
    a, b, c, d, e, f, g, h, i = bits.getdata()
    return min(a, c, e, g, i) > max(b, d, f, h) + 16


@lru_cache(maxsize=4)
def identity_font(size):
    # The isolated client's default Latin font. Fontconfig also resolves its fallback.
    path, index = subprocess.check_output(
        ["fc-match", "Noto Sans Mono CJK JP", "-f", "%{file}\n%{index}"], text=True
    ).splitlines()
    return ImageFont.truetype(path, size, index=int(index))


def identity_pixels(image, bounds, expected, row_height):
    """Match the expected glyphs to screenshot ink, including the end of the basename."""
    actual = image.crop(bounds).convert("L")
    hist = actual.histogram()
    background = max(range(256), key=hist.__getitem__)
    actual = actual.point(lambda p: 255 if abs(p-background) >= 35 else 0)
    font = identity_font(max(1, round(row_height * 12 / 32)))
    sample = Image.new("L", (round(font.getlength(expected)) + 8, row_height * 2))
    ImageDraw.Draw(sample).text((2, 0), expected, fill=255, font=font)
    sample = sample.point(lambda p: 255 if p >= 55 else 0)
    if not actual.getbbox() or not sample.getbbox():
        return False
    actual = actual.crop(actual.getbbox())
    sample = sample.crop(sample.getbbox())
    if abs(actual.width-sample.width) > 3 or abs(actual.height-sample.height) > 3:
        return False
    # GPUI and FreeType can round subpixel glyph origins differently. Compare
    # local chunks so a matching prefix cannot compensate for a blank suffix.
    # Antialiasing can add a leading ink column in one rasterizer. Pick a
    # single offset for the entire word line, retaining every chunk check.
    shifts = [sample.crop((dx, 0, dx+actual.width, actual.height)) for dx in (-1, 0, 1)]
    aligned = max(shifts, key=lambda candidate: ImageChops.darker(actual, candidate).histogram()[255])
    for x in range(0, actual.width, 24):
        region = (x, 0, min(x+24, actual.width), actual.height)
        a, b = actual.crop(region), aligned.crop(region)
        common = ImageChops.darker(a, b).histogram()[255]
        total = a.histogram()[255] + b.histogram()[255]
        if total > 8 and 2 * common < 0.62 * total:
            return False
    return True


def check_tools(image, probes, page, tools, close, bounds):
    prefix = f"pane-tool:{page}:"
    items = sorted(((key[len(prefix):], box(rect)) for key, rect in probes.items()
                    if key.startswith(prefix)), key=lambda item: item[1][0])
    more = probes.get(f"pane-more:{page}")
    drawn = items + ([("more", box(more))] if more else [])
    end = tools[0] if tools else 0
    valid = True
    for _, rect in drawn:
        valid &= bool(tools and contains(tools, rect) and contains(bounds, rect)
                      and rect[0] >= end and painted(image, rect))
        end = rect[2]
    controls = [box(probes[k]) for k in ("follow-agent", "ui:Edit") if k in probes]
    if tools and close:
        end = tools[2]
        for rect in controls:
            valid &= contains(bounds, rect) and end <= rect[0] and rect[2] <= close[0] and painted(image, rect)
            end = rect[2]
    return [k for k, _ in items], more is not None, valid


def measure(image, shot, title, size, crop):
    page = title.lower()
    source = IDENTITY[title]
    probes = shot["probe_rects"]
    layout = shot["layout"]
    left = box(layout["main"])[2]
    right = box(layout["rail"])[0]
    top = box(layout["topbar"])[3]
    reference = probes.get("pane-row-reference")
    height = round(reference["h"]) if reference else 32
    bounds = (left, top, right, top + height)
    if not contains((0, 0, *image.size), bounds):
        return {"outcome": "harness_failed", "reason": "invalid measured pane bounds"}
    image.crop(bounds).save(crop)
    prefix = f"pane-identity:{page}:"
    identities = [key for key in probes if key.startswith(prefix)]
    label = identities[0][len(prefix):] if len(identities) == 1 else ""
    wanted = f"{title} · {source}"
    labels = {wanted}
    if size == "minimum":
        if "/" in source:
            labels.add(f"{title} · …/{Path(source).name}")
    names = [identities[0] if len(identities) == 1 else "", f"pane-tools:{page}", f"pane-close:{page}"]
    header = probes.get(f"pane-header:{page}")
    v = shot.get("viewer", {})
    current = ("diff" if v.get("diff_open") else "gerbers" if v.get("open") == "board" and v.get("gerbers") else v.get("open"))
    checks = {"page": current == page, "settled": shot["settled"],
              "identity": label in labels, "row_height": bool(header and reference and abs(header["h"]-reference["h"]) <= 1),
              "header_aligned": bool(header and max(abs(a-b) for a,b in zip(box(header), bounds)) <= 2),
              "width": abs(right-left-320) <= 2 if size == "minimum" else right-left >= 600}
    rects = [box(probes[n]) if n in probes else None for n in names]
    identity, tools, close = rects
    checks["identity_painted"] = bool(identity and contains(bounds, identity) and label in labels
                                      and identity_pixels(image, identity, label, height))
    checks["close_painted"] = bool(close and contains(bounds, close) and close_pixels(image, close))
    checks["order"] = bool(all(rects) and identity[2] <= tools[0] <= tools[2] <= close[0]
                           and bounds[1] <= tools[1] <= tools[3] <= bounds[3])
    tool_ids, more, checks["tool_and_control_layout"] = check_tools(image, probes, page, tools, close, bounds)
    return {"inline_controls": [k for k in ("follow-agent", "ui:Edit") if k in probes],
            "tools": tool_ids, "more": more, "outcome": "passed" if all(checks.values()) else "task_failed", "checks": checks,
            "identity": label, "expected": wanted, "pane": bounds, "rects": dict(zip(names, rects)),
            "crop": crop.name}


def evaluate(directory, size, rc):
    fixture = directory / "full"
    parsed = parse_client_log(fixture / "client.log")
    captures = parse_captures(fixture)
    results = {}
    for title in IDENTITY:
        page = title.lower()
        cap = captures.get(page, {})
        shot = parsed["shots"].get(page, {})
        try:
            if rc or not cap.get("ok") or not shot or shot_privacy_reasons(cap, shot):
                raise ValueError(f"missing or uncleared capture, runner exit {rc}")
            with Image.open(fixture / f"{page}.png") as image:
                result = measure(image, shot, title, size, directory / f"{page}-header.png")
            if not any(p["found"] and p["id"] == f"rail:{title}" for p in parsed["presses"].get(page, [])):
                result.update(outcome="task_failed", reason="rail press missed")
        except (OSError, KeyError, TypeError, ValueError, subprocess.SubprocessError) as e:
            result = {"outcome": "harness_failed", "reason": str(e)}
        results[page] = result
    return results


def main(client, hub, assets, output):
    root = Path(output)
    root.mkdir(parents=True, exist_ok=True)
    if any(root.iterdir()):
        raise SystemExit("B2 needs an empty output directory")
    reports = {}
    for size in ("wide", "minimum"):
        directory = root / size
        rc = subprocess.run(["bash", str(Path(__file__).with_name("rail-shots.sh")),
                             client, hub, assets, str(directory)],
                            env=dict(os.environ, RAIL_B2_SIZE=size, RAIL_KINDS="full")).returncode
        reports[size] = evaluate(directory, size, rc)
    tool_orders(reports)
    outcomes = {r["outcome"] for results in reports.values() for r in results.values()}
    overall = next(o for o in ("harness_failed", "task_failed", "passed") if o in outcomes)
    (root / "b2.json").write_text(json.dumps({"outcome": overall, "sizes": reports}, indent=2) + "\n")
    return int(overall != "passed")


if __name__ == "__main__":
    if sys.argv[1:] == ["--script"]:
        print(json.dumps(script()))
    else:
        sys.exit(main(*sys.argv[1:]))
