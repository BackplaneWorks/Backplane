#!/usr/bin/env python3
"""Discover KiCad viewer inputs and optionally update project selection."""
import argparse
import json
import os
from pathlib import Path
import re


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("workspace", type=Path)
    parser.add_argument("--pcb")
    parser.add_argument("--schematic")
    parser.add_argument("--gerbers", action="append")
    parser.add_argument("--write", action="store_true")
    args = parser.parse_args()
    root = args.workspace.resolve(strict=True)
    if not root.is_dir():
        parser.error("workspace must be a directory")
    config_path = root / ".k3eda.json"
    if config_path.is_symlink():
        parser.error(".k3eda.json must not be a symlink")
    try:
        config = json.loads(config_path.read_text()) if config_path.exists() else {}
        if not isinstance(config, dict):
            raise ValueError(".k3eda.json must contain an object")
    except (ValueError, OSError) as error:
        parser.error(str(error))
    found = {"pcb": [], "schematic": [], "gerbers": set()}
    for directory, dirs, files in os.walk(root, followlinks=False):
        dirs[:] = sorted(d for d in dirs if d not in {".git", ".history", "node_modules", ".venv"} and not (Path(directory) / d).is_symlink())
        for name in sorted(files):
            path = Path(directory) / name
            if path.is_symlink():
                continue
            rel = path.relative_to(root).as_posix()
            suffix = path.suffix.lower()
            if suffix == ".kicad_pcb":
                found["pcb"].append(rel)
            elif suffix == ".kicad_sch":
                found["schematic"].append(rel)
            elif suffix in {".gbr", ".ger", ".gtl", ".gbl", ".gto", ".gbo", ".gts", ".gbs", ".gta", ".gba", ".gtp", ".gbp", ".gm1", ".gko", ".drl", ".xln"} or re.fullmatch(r"\.g\d+", suffix):
                found["gerbers"].add(path.parent.relative_to(root).as_posix())
    found["gerbers"] = sorted(found["gerbers"])
    for key in ("pcb", "schematic", "gerbers"):
        value = getattr(args, key)
        if value is None:
            continue
        values = value if isinstance(value, list) else [value]
        normalized = []
        for item in values:
            path = root / item
            try:
                relative = path.resolve(strict=True).relative_to(root)
            except (ValueError, OSError):
                parser.error(f"{key} must exist inside the workspace: {item}")
            if any(p.is_symlink() for p in [path, *path.parents] if p != root):
                parser.error(f"symlinks are not viewer inputs: {item}")
            if ".history" in relative.parts:
                parser.error(f"history files are not viewer inputs: {item}")
            if key == "gerbers" and not path.is_dir():
                parser.error(f"Gerber path must be a directory: {item}")
            if key != "gerbers" and relative.as_posix() not in found[key]:
                parser.error(f"not a {key} file: {item}")
            normalized.append(relative.as_posix())
        config[key] = normalized if key == "gerbers" else normalized[0]
    if args.write:
        if not any(getattr(args, key) is not None for key in ("pcb", "schematic", "gerbers")):
            parser.error("--write requires at least one explicit selection")
        temporary = root / ".k3eda.json.tmp"
        with temporary.open("x") as output:
            json.dump(config, output, indent=2)
            output.write("\n")
        temporary.replace(config_path)
    print(json.dumps({"workspace": str(root), "config": config, "candidates": found, "written": args.write}, indent=2))


if __name__ == "__main__":
    main()
