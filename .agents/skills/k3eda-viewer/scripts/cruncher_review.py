#!/usr/bin/env python3
"""Generate KiCad Cruncher review artifacts from a disposable source snapshot."""
import argparse
import os
import shutil
import subprocess
import tempfile
from pathlib import Path


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("project", type=Path, help="KiCad .kicad_pro, .kicad_sch, or .kicad_pcb")
    parser.add_argument("--output", type=Path, help="New output directory (default: temporary)")
    parser.add_argument("--pcb-svg", action="store_true", help="Also generate assembly views for a project or schematic")
    args = parser.parse_args()
    project = args.project.resolve(strict=True)
    if project.suffix not in {".kicad_pro", ".kicad_sch", ".kicad_pcb"}:
        parser.error("expected a KiCad project, schematic, or PCB")
    output = args.output.resolve() if args.output else Path(tempfile.mkdtemp(prefix="k3eda-cruncher-"))
    if args.output:
        output.mkdir(parents=True, exist_ok=False)
    # Cruncher may create pcb.svg.config beside its input. Keep that behavior,
    # and any intermediate CLI exports, away from the editable project.
    with tempfile.TemporaryDirectory(prefix="k3eda-cruncher-input-") as temporary:
        snapshot = Path(temporary)
        for directory, dirs, files in os.walk(project.parent, followlinks=False):
            dirs[:] = [name for name in dirs if name not in {".git", ".history", "node_modules", ".venv"} and not (Path(directory) / name).is_symlink()]
            for name in files:
                source = Path(directory) / name
                if source.is_symlink() or (not source.suffix.startswith(".kicad_") and name != "pcb.svg.config"):
                    continue
                target = snapshot / source.relative_to(project.parent)
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(source, target)
        source = snapshot / project.name
        prefix = ["uv", "tool", "run", "--from", "kicad-cruncher==2026.8.30", "kicad-cruncher"]
        commands = []
        if project.suffix != ".kicad_pcb":
            commands.append(prefix + ["design", str(source), "-o", str(output / "design")])
        if project.suffix == ".kicad_pcb" or args.pcb_svg:
            board = source if project.suffix != ".kicad_sch" else source.with_suffix(".kicad_pcb")
            if not board.exists():
                parser.error("no sibling PCB found for assembly views")
            commands.append(prefix + ["pcb-svg", str(board), "-o", str(output / "pcb-svg")])
        env = os.environ.copy()
        env.pop("APPDIR", None)
        env.pop("APPIMAGE", None)
        for command in commands:
            subprocess.run(command, check=True, cwd=snapshot, env=env)
    print(output)


if __name__ == "__main__":
    main()
