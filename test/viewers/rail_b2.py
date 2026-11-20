"""Run rail-shots.sh --b2 CLIENT HUB ASSETS EMPTY_OUTDIR for the three-width header check."""
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
                "?gerbers-gerbview", "?gerbers-legend"],
    "split": ["?boards-btn", "split-pick-sch", "split-pick-pcb", "split-pick-3d", "?split-probe"],
    "diff": ["git-commit_push", "git-commit_push_pr", "revert-ask"],
    "review": ["rv-0", "rv-1", "rv-2", "rv-3"],
    "files": ["fm-up", "fm-new", "fm-upload", "fm-zip"],
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
        actions = {
            "board": ["follow-agent", "kicad-edit"],
            "schematic": ["follow-agent", "kicad-edit"],
            "split": ["follow-agent"] + (["kicad-edit"] if "kicad-edit" in observed else []),
            "review": ["kicad-edit"] if "kicad-edit" in observed else [],
            "files": ["kicad-edit"] if "kicad-edit" in observed else [],
        }
        expected += actions.get(page, [])
        for result in [wide, narrow] + ([reports["615"][page]] if "615" in reports else []):
            if "tools" not in result:
                continue
            ids = result["tools"]
            current = expected
            if page == "gerbers" and result is narrow:
                # GerbView availability arrives asynchronously in each isolated client.
                present = ids + result.get("menu_rows", [])
                current = [key.lstrip("?") for key in schema
                           if not key.startswith("?") or key[1:] in present]
            inline = [key for key in current if key in ids]
            missing = [key for key in current if key not in ids]
            order_ok = ids == inline and (
                (not result["more"] and not missing) if result is not narrow else
                (result["more"] == bool(missing) and result.get("menu_rows", []) == missing))
            result["checks"]["expected_tools"] = order_ok
            if not order_ok:
                result["outcome"] = "task_failed"


def script():
    steps = [{"select": 0}]
    for title in (["Review"] if os.environ.get("RAIL_B2_KIND") == "no-design" else IDENTITY):
        page = title.lower()
        steps.extend([{"mark": page}, {"press": f"rail:{title}"},
                      {"wait_settled": page, "timeout_ms": 20000}])
        if page == "review":
            steps.append({"wait": 1200})
        if page == "files":
            steps.append({"files": "select:README.md"})
        steps.append({"shot": page, "settle": page, "timeout_ms": 20000})
        if os.environ.get("RAIL_B2_SIZE") == "minimum":
            steps.extend([{"wait": 300}, {"shot": page + "-menu"}])
            if page == "gerbers":
                steps.extend([{"wait": 300}, {"shot": "gerbers-after", "settle": page, "timeout_ms": 90000}])
            else:
                steps.append({"keys": "escape"})
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
        basename = Path(source).name
        for keep in range(1, len(basename)):
            tail = (keep+1)//2
            labels.add(f"{title} · {basename[:keep//2]}…{basename[-tail:]}")
        if "/" in source:
            labels.add(f"{title} · …/{Path(source).name}")
    names = [identities[0] if len(identities) == 1 else "", f"pane-tools:{page}", f"pane-close:{page}"]
    header = probes.get(f"pane-header:{page}")
    v = shot.get("viewer", {})
    current = ("diff" if v.get("diff_open") else "gerbers" if v.get("open") == "board" and v.get("gerbers") else v.get("open"))
    checks = {"page": current == page, "settled": shot["settled"],
              "identity": label in labels, "row_height": bool(header and reference and abs(header["h"]-reference["h"]) <= 1),
              "header_aligned": bool(header and max(abs(a-b) for a,b in zip(box(header), bounds)) <= 2),
              "width": abs(right-left-320) <= 2 if size == "minimum" else abs(right-left-(743 if page == "split" else 615)) <= 2 if size == "615" else right-left >= 600}
    rects = [box(probes[n]) if n in probes else None for n in names]
    identity, tools, close = rects
    checks["identity_width"] = bool(identity and identity[2]-identity[0] >= identity_font(round(height*12/32)).getlength(label))
    checks["close_at_right"] = bool(close and 0 <= right-close[2] <= height/2)
    checks["identity_painted"] = bool(identity and contains(bounds, identity) and label in labels
                                      and identity_pixels(image, identity, label, height))
    checks["close_painted"] = bool(close and contains(bounds, close) and close_pixels(image, close))
    checks["order"] = bool(all(rects) and identity[2] <= tools[0] <= tools[2] <= close[0]
                           and bounds[1] <= tools[1] <= tools[3] <= bounds[3])
    tool_ids, more, checks["tool_and_control_layout"] = check_tools(image, probes, page, tools, close, bounds)
    return {"tools": tool_ids, "more": more, "outcome": "passed" if all(checks.values()) else "task_failed", "checks": checks,
            "identity": label, "expected": wanted, "pane": bounds, "rects": dict(zip(names, rects)),
            "crop": crop.name}


def content_evidence(image, shot, page, kind, files_selected):
    viewer = shot["viewer"]
    state = shot["state"]
    body = (box(shot["layout"]["main"])[2], box(shot["layout"]["topbar"])[3]+32,
            box(shot["layout"]["rail"])[0], image.height)
    if page == "review":
        rows = [key for key, rect in shot["probe_rects"].items()
                if key.startswith("mgmt:bom:row:") and contains(body, box(rect)) and painted(image, box(rect))]
        empty = viewer.get("empty_text")
        ok = (state == "empty" and empty == "Nothing to review yet" and not rows
              if kind == "no-design" else state == "ready" and "mgmt:bom:row:C1" in rows)
        return ok, dict(state=state, painted_rows=rows, empty_text=empty)
    if page == "files":
        return state == "ready" and files_selected, dict(selected_readme=files_selected)
    if page == "diff":
        names = viewer.get("diff_file_names", [])
        return state == "ready" and "README.md" in names, dict(files=names)
    # Colored drawing pixels below the controls distinguish a drawing from a status line.
    x, y, right, bottom = body
    colors = image.crop((x+4, y+64, right-4, bottom-24)).convert("RGB").getcolors(image.width*image.height)
    ink = sum(n for n, rgb in colors if max(rgb)-min(rgb) > 40)
    key = viewer.get("plot_key", "")
    if page == "split":
        ok = viewer.get("panes_count") == 3 and viewer.get("solid_painted", 0) > 0
    else:
        ok = key and key == viewer.get("key") and not viewer.get("plot_none")
    return bool(ok and state == "ready" and ink > 100), dict(state=state, colored_pixels=ink,
        plot_kind=key.split("|")[0], panes=viewer.get("panes_count"), solid_frames=viewer.get("solid_painted"))


def theme_evidence(image, shot, theme):
    left = box(shot["layout"]["main"])[2] + 4
    right = box(shot["layout"]["rail"])[0] - 4
    top = box(shot["layout"]["topbar"])[3] + 64
    body = image.crop((left, top, right, image.height-24)).convert("RGB")
    count, rgb = max(body.getcolors(body.width*body.height))
    palette = {"light": (250, 250, 248), "dark": (13, 15, 18)}
    return rgb == palette[theme] and count > body.width*body.height/2, dict(
        requested=theme, dominant_rgb=rgb, pixels=count, total=body.width*body.height)


def evaluate(directory, size, rc, kind="full", theme="dark"):
    fixture = directory / kind
    parsed = parse_client_log(fixture / "client.log")
    captures = parse_captures(fixture)
    results = {}
    for title in (["Review"] if kind == "no-design" else IDENTITY):
        page = title.lower()
        cap = captures.get(page, {})
        shot = parsed["shots"].get(page, {})
        try:
            if rc or not cap.get("ok") or not shot or shot_privacy_reasons(cap, shot):
                raise ValueError(f"missing or uncleared capture, runner exit {rc}")
            with Image.open(fixture / f"{page}.png") as image:
                result = measure(image, shot, title, size, fixture / f"{page}-header.png")
                selected = False
                if page == "files":
                    for line in (fixture / "client.log").read_text().splitlines():
                        if "GPUI_METRIC " in line:
                            event = json.loads(line.split("GPUI_METRIC ", 1)[1])
                            if event.get("event") == "files" and event.get("step") == "select:README.md":
                                selected = event.get("ok") is True
                result["checks"]["content"], result["body"] = content_evidence(image, shot, page, kind, selected)
                result["shot"] = str((fixture / f"{page}.png").resolve())
                if kind == "full" and page in ("review", "diff", "files"):
                    result["checks"]["theme"], result["theme"] = theme_evidence(image, shot, theme)
                result["crop"] = f"{kind}/{page}-header.png"
                if page == "review" and size != "minimum":
                    result["checks"]["four_inline_tabs"] = result["tools"][:4] == TOOL_ORDER["review"]
                if not all(result["checks"].values()):
                    result["outcome"] = "task_failed"
            if size == "minimum" and result.get("more"):
                name = page + "-menu"
                menu_shot, menu_cap = parsed["shots"][name], captures[name]
                if not menu_cap.get("ok") or shot_privacy_reasons(menu_cap, menu_shot):
                    raise ValueError("uncleared menu capture")
                probes = menu_shot["probe_rects"]
                menu = probes.get(f"pane-menu:{page}")
                rows = []
                with Image.open(fixture / (name + ".png")) as image:
                    if menu and contains((0, 0, *image.size), box(menu)):
                        for key, rect in probes.items():
                            prefix = f"pane-menu-row:{page}:"
                            if key.startswith(prefix):
                                rows.append((key[len(prefix):], box(rect)))
                        if not rows:
                            prefix = f"pane-tool:{page}:"
                            rows = [(key[len(prefix):], box(rect)) for key, rect in probes.items()
                                    if key.startswith(prefix) and contains(box(menu), box(rect))]
                        rows.sort(key=lambda row: row[1][1])
                        result["checks"]["painted_rows"] = bool(rows) and all(
                            contains(box(menu), rect) and painted(image, rect) and
                            (i == 0 or rows[i-1][1][3] <= rect[1])
                            for i, (_, rect) in enumerate(rows))
                        image.crop(box(menu)).save(directory / (name + ".png"))
                    else:
                        result["checks"]["painted_rows"] = False
                result["menu_rows"] = [key for key, _ in rows]
                prefix = f"pane-tool:{page}:"
                inventory = shot["header_controls"]
                folded = {k[len(prefix):]: v for k, v in inventory.items()
                          if k.startswith(prefix) and v.get("folded") is True}
                trigger = shot["probe_rects"][f"pane-more:{page}"]
                result["checks"]["folded_inventory"] = (
                    set(folded) == set(result["menu_rows"]) - (
                        {"gerbers-gerbview"} if page == "gerbers" and
                        prefix + "gerbers-gerbview" not in inventory else set()) and all(
                        entry.get("rect") and box(entry["rect"]) == box(trigger)
                        for entry in folded.values()))
                menu_trigger = probes.get(f"pane-more:{page}")
                menu_inventory = {key[len(prefix):]: v for key, v in menu_shot["header_controls"].items()
                                  if key.startswith(prefix) and v.get("folded")}
                result["checks"]["menu_inventory"] = bool(menu_trigger and
                    set(menu_inventory) == set(result["menu_rows"]) and all(
                        entry.get("rect") and box(entry["rect"]) == box(menu_trigger)
                        for entry in menu_inventory.values()))
                if page == "gerbers":
                    alias = inventory.get("ui:vs board", {})
                    result["checks"]["comparison_alias"] = bool(alias.get("folded") and alias.get("wired")
                        and alias.get("rect") and box(alias["rect"]) == box(trigger))
                result["checks"]["opened_by_click"] = bool(cap.get("click", {}).get("ok"))
                if page == "gerbers":
                    after = parsed["shots"]["gerbers-after"]
                    after_cap = captures["gerbers-after"]
                    if not after_cap.get("ok") or shot_privacy_reasons(after_cap, after):
                        raise ValueError("uncleared post-click capture")
                    before_state = menu_shot["viewer"].get("gerbers_vs_board")
                    after_state = after["viewer"].get("gerbers_vs_board")
                    result["effect"] = {"before": before_state, "after": after_state,
                                        "click": menu_cap.get("click"), "shots": [name, "gerbers-after"]}
                    result["checks"]["folded_tool_effect"] = (bool(menu_cap.get("click", {}).get("ok"))
                        and "gerbers-vs-board" in result["menu_rows"] and before_state is False
                        and after_state is True and after["settled"])
                if not all(result["checks"].values()):
                    result["outcome"] = "task_failed"
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
    theme = os.environ.get("RAIL_B2_THEME", "dark")
    if theme not in ("dark", "light"):
        raise SystemExit("RAIL_B2_THEME must be light or dark")
    theme_file = root.resolve() / "theme.json"
    theme_file.write_text(json.dumps({"theme": theme, "text_pct": 100}))
    reports = {}
    for size in ("615", "wide", "minimum"):
        directory = root / size
        rc = subprocess.run(["bash", str(Path(__file__).with_name("rail-shots.sh")),
                             client, hub, assets, str(directory)],
                            env=dict(os.environ, BACKPLANE_GPUI_LOCAL=str(theme_file), RAIL_B2_SIZE=size, RAIL_KINDS="full no-design")).returncode
        reports[size] = evaluate(directory, size, rc, theme=theme)
        reports[size]["review-empty"] = evaluate(directory, size, rc, "no-design", theme)["review"]
        if all("crop" in r for r in reports[size].values()):
            crops = [Image.open(directory / r["crop"]) for r in reports[size].values()]
            strip = Image.new("RGB", (max(im.width for im in crops), sum(im.height for im in crops)))
            y = 0
            for im in crops:
                strip.paste(im, (0, y))
                y += im.height
                im.close()
            strip.save(root / (size + "-strip.png"))
    tool_orders(reports)
    tool_orders({size: {**pages, "review": pages["review-empty"]} for size, pages in reports.items()})
    outcomes = {r["outcome"] for results in reports.values() for r in results.values()}
    overall = next(o for o in ("harness_failed", "task_failed", "passed") if o in outcomes)
    (root / "b2.json").write_text(json.dumps({"outcome": overall, "sizes": reports}, indent=2) + "\n")
    return int(overall != "passed")


if __name__ == "__main__":
    if sys.argv[1:] == ["--script"]:
        print(json.dumps(script()))
    else:
        sys.exit(main(*sys.argv[1:]))
