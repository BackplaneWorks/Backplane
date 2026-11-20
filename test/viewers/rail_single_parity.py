#!/usr/bin/env python3
"""Capture and compare the 12 settled single-pane states in dark/light at 100/125%.

capture CHECKOUT REV CLIENT HUB ASSETS ROOT main|pr dark|light 100|125
compare ROOT

Use binaries built at REV. Capture records their SHA-256 and runs one fixture
under the caller's rail-heavy lock. Compare writes pixels.json, plus the two
crops and an exact residual mask for each difference outside the Add button.
All boxes in the report use screen pixels [left, top, right, bottom), the
measured button rect is also preserved before rounding to covering pixels.
"""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import re
import subprocess

from PIL import Image, ImageChops

from rail_shots_driver import build_script
from rail_shots_post import parse_client_log

PAGES = ("board", "schematic", "3d", "gerbers", "task4-gerbers-vs-board", "split",
         "mech", "split-after-mech", "files", "diff", "review", "browser")


def capture(args):
    revision = subprocess.check_output(["git", "-C", args.checkout, "rev-parse", "HEAD"], text=True).strip()
    if revision != args.rev:
        raise ValueError(f"expected checkout {args.rev}, got {revision}")
    dest = args.root / args.variant / f"{args.theme}-{args.percent}"
    dest.mkdir(parents=True, exist_ok=False)
    steps = []
    for step in build_script("full"):
        if step.get("shot") in PAGES:
            steps.extend([{"wait": 4000}, step])
        elif step.get("shot") and "settle" in step:
            steps.append({"wait_settled": step["settle"], "tag": step["shot"],
                          "timeout_ms": step["timeout_ms"]})
        elif "shot" not in step:
            steps.append(step)
        if step.get("shot") == "browser":
            break
    if [s["shot"] for s in steps if "shot" in s] != list(PAGES):
        raise ValueError("the fixture route changed")
    script = dest / "script.json"
    script.write_text(json.dumps(steps + [{"quit": True}]))
    hashes = {}
    for label in ("client", "hub"):
        with getattr(args, label).open("rb") as stream:
            digest = hashlib.sha256()
            for block in iter(lambda: stream.read(1024 * 1024), b""):
                digest.update(block)
        hashes[label] = digest.hexdigest()
    manifest = dict(revision=revision, hashes=hashes, theme=args.theme, percent=args.percent)
    env = dict(os.environ, RAIL_KINDS="full", RAIL_THEME=args.theme, RAIL_TEXT_PCT=args.percent,
               RAIL_WINDOW_SIZE="standard", RAIL_SCRIPT_FILE=str(script.resolve()))
    subprocess.run([Path(__file__).with_name("rail-shots.sh").resolve(), args.client, args.hub,
                    args.assets, dest / "run"], env=env, check=True)
    (dest / "manifest.json").write_text(json.dumps(manifest, indent=2))


def pane_bounds(shot):
    main, rail = (shot["layout"][k] for k in ("main", "rail"))
    return tuple(round(p) for p in (main["x"] + main["w"], main["y"], rail["x"], main["y"] + main["h"]))


def capture_data(directory):
    full = directory / "run" / "full"
    shots = parse_client_log(full / "client.log")["shots"]
    checks = {c["name"]: c for c in map(json.loads, (full / "captures.jsonl").read_text().splitlines())}
    for page in PAGES:
        c = checks[page]
        if not (c["ok"] and c["inspect_ok"] and not c["obstructed"] and shots[page]["settled"]):
            raise ValueError(f"missing settled, unobstructed capture for {full}/{page}")
    return full, shots


def compare_page(main, pr, page, dest):
    crop = []
    geometry = []
    for directory, shots in (main, pr):
        bounds = pane_bounds(shots[page])
        with Image.open(directory / f"{page}.png") as image:
            if not (0 <= bounds[0] < bounds[2] <= image.width and 0 <= bounds[1] < bounds[3] <= image.height):
                raise ValueError(f"invalid pane bounds {bounds} at {directory}/{page}")
            crop.append(image.convert("RGB").crop(bounds))
        geometry.append(bounds)
    result = dict(main_pane=geometry[0], pr_pane=geometry[1])
    if geometry[0] != geometry[1]:
        result["geometry_changed"] = True
        dest.mkdir(exist_ok=True, parents=True)
        for label, image in zip(("main", "pr"), crop):
            image.save(dest / f"{page}-{label}.png")
        if crop[0].size != crop[1].size:
            return result
    left, top, _, _ = geometry[1]
    channels = ImageChops.difference(*crop).split()
    changed = ImageChops.lighter(channels[0], ImageChops.lighter(*channels[1:])).point(lambda x: 255 if x else 0)
    result["changed_pixels"] = changed.histogram()[255]
    result["states"] = [data[1][page]["state"] for data in (main, pr)]
    buttons = {k: v for k,v in pr[1][page]["probe_rects"].items() if re.fullmatch(r"tile:[^:]+:add-beside", k)}
    result["buttons"] = buttons
    if len(buttons) > 1:
        raise ValueError(f"ambiguous Add button on single page {page}: {buttons}")
    for rect in buttons.values():
        x, y, w, h = (rect[k] for k in ("x", "y", "w", "h"))
        pixel_box = (math.floor(x), math.floor(y), math.ceil(x+w), math.ceil(y+h))
        if not (left <= x < x+w <= geometry[1][2] and top <= y < y+h <= geometry[1][3]):
            raise ValueError(f"Add button outside {page} pane: {rect}")
        result["button_pixels_box"] = pixel_box
        changed.paste(0, tuple(v - (left if i % 2 == 0 else top) for i,v in enumerate(pixel_box)))
    result["outside_pixels"] = changed.histogram()[255]
    box = changed.getbbox()
    if box:
        result["outside_box"] = [v + (left if i % 2 == 0 else top) for i,v in enumerate(box)]
        dest.mkdir(exist_ok=True, parents=True)
        for label, image in zip(("main", "pr", "outside"), [*crop, changed]):
            image.save(dest / f"{page}-{label}.png")
    return result


def compare(root):
    report, builds = {}, {}
    for theme in ("dark", "light"):
        for pct in ("100", "125"):
            config = f"{theme}-{pct}"
            inputs = []
            for variant in ("main", "pr"):
                directory = root / variant / config
                manifest = json.loads((directory / "manifest.json").read_text())
                build = (manifest["revision"], manifest["hashes"])
                if builds.setdefault(variant, build) != build or (manifest["theme"], manifest["percent"]) != (theme, pct):
                    raise ValueError(f"capture provenance differs at {directory}")
                inputs.append(capture_data(directory))
            report[config] = {page: compare_page(*inputs, page, root / "diffs" / config) for page in PAGES}
    (root / "pixels.json").write_text(json.dumps(dict(builds=builds, pages=report), indent=2) + "\n")
    rows = [r for pages in report.values() for r in pages.values()]
    issues = sum(bool(r.get("geometry_changed") or r["outside_pixels"]) for r in rows)
    print(f"{len(rows)} crops, {issues} with layout or pixel differences outside the measured Add button")
    return int(bool(issues))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    cmds = parser.add_subparsers(dest="command", required=True)
    cap = cmds.add_parser("capture")
    for field in ("checkout", "rev", "client", "hub", "assets", "root"):
        cap.add_argument(field, type=str if field=="rev" else Path)
    cap.add_argument("variant", choices=("main","pr"))
    cap.add_argument("theme", choices=("dark","light"))
    cap.add_argument("percent", choices=("100","125"))
    cmds.add_parser("compare").add_argument("root", type=Path)
    args = parser.parse_args()
    if args.command == "capture":
        capture(args)
    else:
        raise SystemExit(compare(args.root))
