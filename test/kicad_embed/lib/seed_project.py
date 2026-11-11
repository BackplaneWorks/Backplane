#!/usr/bin/env python3
"""Seed an isolated KiCad project copy + Backplane hub home + KiCad XDG config.

Usage:
  seed_project.py OUT REPO_ROOT

Creates:
  - $OUT/origin.git + $OUT/proj (git main branch, copied from test/files/fixtures/plot/ecc83-pp.*)
  - $OUT/proj/ecc83-pp.kicad_pro, .backplane.json, ecc83-pp.kicad_pcb.backplane.json (companion check)
  - $OUT/proj/proj_syms.kicad_sym + sym-lib-table, $OUT/proj/proj_fps.pretty/TestPad.kicad_mod + fp-lib-table
  - $OUT/fh/.config/kicad/{10.0,10.99}/{kicad_common.json,pcbnew.json,eeschema.json,sym-lib-table,fp-lib-table,design-block-lib-table}
    so stock KiCad 10.0.x and Backplane KiCad 10.99-bp skip STARTWIZARD and start KICAD_API_SERVER.
  - $OUT/home/events.jsonl (ProjectCreated + ThreadCreated) and $OUT/bin/* stand-in agent shims.
"""
import json
import os
import shutil
import stat
import subprocess
import sys
import time


def write(path: str, content: str, executable: bool = False) -> None:
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8", newline="\n") as f:
        f.write(content)
    if executable:
        os.chmod(path, os.stat(path).st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)


def main() -> None:
    out_dir = os.path.abspath(sys.argv[1])
    repo_root = os.path.abspath(sys.argv[2])
    proj = os.path.join(out_dir, "proj")
    origin = os.path.join(out_dir, "origin.git")
    fixtures = os.path.join(repo_root, "test", "files", "fixtures", "plot")

    os.makedirs(proj, exist_ok=True)
    # KEMBED_LAYOUT=repo: the KiCad project in boards/ecc83-pp/ of the repository (as in the person's SOT23_Card), the viewer naming the
    # repository's root; the default keeps it at the root
    sub = os.path.join("boards", "ecc83-pp") if os.environ.get("KEMBED_LAYOUT") == "repo" else ""
    kdir = os.path.join(proj, sub) if sub else proj
    os.makedirs(kdir, exist_ok=True)
    for ext in ("kicad_pcb", "kicad_sch", "kicad_prl"):
        src = os.path.join(fixtures, f"ecc83-pp.{ext}")
        if os.path.isfile(src):
            shutil.copy2(src, os.path.join(kdir, f"ecc83-pp.{ext}"))

    # Synthesize a clean .kicad_pro beside ecc83-pp.{kicad_pcb,kicad_sch}
    pro = {
        "board": {"design_settings": {"defaults": {"board_outline_line_width": 0.1}}},
        "libraries": {"pinned_footprint_libs": ["proj_fps"], "pinned_symbol_libs": ["proj_syms"]},
        "meta": {"filename": "ecc83-pp.kicad_pro", "version": 1},
        "schematic": {"legacy_lib_dir": "", "legacy_lib_list": []},
        "sheets": [["28f865a0-4433-4a53-bbd7-b62f276848e4", "Root"]],
    }
    write(os.path.join(kdir, "ecc83-pp.kicad_pro"), json.dumps(pro, indent=2) + "\n")

    # Canonical Backplane pointer + companion metadata to verify byte-preservation across saves/exports
    write(os.path.join(proj, ".backplane.json"), json.dumps({"project": os.path.join(sub, "ecc83-pp.kicad_pro") if sub else "ecc83-pp.kicad_pro"}, indent=2) + "\n")
    companion = {
        "format": "backplane-kicad-metadata",
        "items": {
            "00000000-0000-0000-0000-000054a581e5": {
                "custom_properties": {
                    "backplane_note": "preserve-across-gui-save-and-plot",
                }
            }
        },
        "version": 1,
    }
    write(os.path.join(kdir, "ecc83-pp.kicad_pcb.backplane.json"), json.dumps(companion, indent=2, sort_keys=True) + "\n")

    # Project-local symbol library + footprint library for Symbol Editor & Footprint Editor tests
    write(
        os.path.join(kdir, "proj_syms.kicad_sym"),
        """(kicad_symbol_lib
  (version 20231120)
  (generator "kicad_symbol_editor")
  (generator_version "9.0")
  (symbol "TestResistor"
    (pin_numbers hide)
    (pin_names (offset 0))
    (exclude_from_sim no)
    (in_bom yes)
    (on_board yes)
    (property "Reference" "R" (at 2.032 0 90) (effects (font (size 1.27 1.27))))
    (property "Value" "TestResistor" (at 0 0 90) (effects (font (size 1.27 1.27))))
    (property "Footprint" "proj_fps:TestPad" (at -1.778 0 90) (effects (font (size 1.27 1.27)) hide))
    (property "Datasheet" "~" (at 0 0 0) (effects (font (size 1.27 1.27)) hide))
    (property "MPN" "RC0603FR-0710KL" (at 0 0 0) (effects (font (size 1.27 1.27)) hide))
    (symbol "TestResistor_0_1"
      (rectangle (start -1.016 -2.54) (end 1.016 2.54) (stroke (width 0.254) (type default)) (fill (type none)))
    )
    (symbol "TestResistor_1_1"
      (pin passive line (at 0 3.81 270) (length 1.27) (name "~" (effects (font (size 1.27 1.27)))) (number "1" (effects (font (size 1.27 1.27)))))
      (pin passive line (at 0 -3.81 90) (length 1.27) (name "~" (effects (font (size 1.27 1.27)))) (number "2" (effects (font (size 1.27 1.27)))))
    )
  )
)
""",
    )
    write(
        os.path.join(kdir, "sym-lib-table"),
        '(sym_lib_table\n  (version 7)\n  (lib (name "proj_syms")(type "KiCad")(uri "${KIPRJMOD}/proj_syms.kicad_sym")(options "")(descr ""))\n)\n',
    )
    write(
        os.path.join(kdir, "proj_fps.pretty", "TestPad.kicad_mod"),
        """(footprint "TestPad"
  (version 20240108)
  (generator "pcbnew")
  (generator_version "9.0")
  (layer "F.Cu")
  (property "Reference" "REF**" (at 0 -2.5 0) (layer "F.SilkS") (uuid "11111111-2222-3333-4444-555555555501") (effects (font (size 1 1) (thickness 0.15))))
  (property "Value" "TestPad" (at 0 2.5 0) (layer "F.Fab") (uuid "11111111-2222-3333-4444-555555555502") (effects (font (size 1 1) (thickness 0.15))))
  (attr smd)
  (fp_rect (start -1.8 -1.1) (end 1.8 1.1) (stroke (width 0.05) (type solid)) (fill none) (layer "F.CrtYd") (uuid "11111111-2222-3333-4444-555555555503"))
  (pad "1" smd roundrect (at -0.8 0) (size 0.9 1.2) (layers "F.Cu" "F.Paste" "F.Mask") (roundrect_rratio 0.25) (uuid "11111111-2222-3333-4444-555555555504"))
  (pad "2" smd roundrect (at 0.8 0) (size 0.9 1.2) (layers "F.Cu" "F.Paste" "F.Mask") (roundrect_rratio 0.25) (uuid "11111111-2222-3333-4444-555555555505"))
)
""",
    )
    write(
        os.path.join(kdir, "fp-lib-table"),
        '(fp_lib_table\n  (version 7)\n  (lib (name "proj_fps")(type "KiCad")(uri "${KIPRJMOD}/proj_fps.pretty")(options "")(descr ""))\n)\n',
    )

    # Hierarchical schematic copy in a separate subfolder for hierarchy checks
    hier_src = os.path.join(fixtures, "complex_hierarchy.kicad_sch")
    if os.path.isfile(hier_src):
        shutil.copy2(hier_src, os.path.join(proj, "extra", "complex_hierarchy.kicad_sch") if False else os.path.join(proj, "complex_hierarchy_ref.txt"))
        os.remove(os.path.join(proj, "complex_hierarchy_ref.txt"))

    # KEMBED_PROJ_SRC=<a real project folder>: a copy of it instead of the fixture (its own .backplane.json names the board; never the original)
    src = os.environ.get("KEMBED_PROJ_SRC", "")
    if src:
        shutil.rmtree(proj)
        shutil.copytree(src, proj, ignore=shutil.ignore_patterns(".git", ".backplane", "mfg", "*.lck", "*-backups"))

    # Initialize bare origin.git + proj git repo on main so canonical.rs design_root works as in production
    subprocess.run(["git", "init", "--bare", "-b", "main", origin], check=True, stdout=subprocess.DEVNULL)
    git = lambda *a: subprocess.run(["git", "-C", proj, *a], check=True, stdout=subprocess.DEVNULL)
    git("init", "-b", "main")
    git("config", "user.name", "i2cjak")
    git("config", "user.email", "build@amemb.com")
    git("remote", "add", "origin", origin)
    git("add", ".")
    git("commit", "-m", "initial fixture board")
    git("push", "-u", "origin", "main")

    # Seed isolated KiCad config for both 10.0 (stock) and 10.99 (Backplane fork):
    # - suppresses STARTWIZARD (SettingsDirectoryValid + GlobalTablesValid + privacy prompts)
    # - enables KICAD_API_SERVER (api.enable_server = true)
    # - suppresses zone_fill_warning modal popup on 'B' hotkey
    common_cfg = {
        "meta": {"filename": "kicad_common.json", "version": 6},
        "api": {"enable_server": True, "interpreter_path": ""},
        "appearance": {"toolbar_icon_size": 24},
        "do_not_show_again": {
            "data_collection_prompt": True,
            "update_check_prompt": True,
            "zone_fill_warning": True,
        },
        "environment": {"vars": None},
        "system": {"check_for_updates": False, "WorkingDir": proj},
    }
    editor_cfg = lambda fname, ver_num: {
        "meta": {"filename": fname, "version": ver_num},
        "appearance": {"canvas_scale": 1.0, "show_page_limits": True},
        "aui": {
            "show_properties": False,
            "show_layer_manager": False,
            "show_hierarchy": False,
            "show_schematic_hierarchy": False,
            "show_search": False,
            "show_net_inspector": False,
            "design_blocks_show": False,
        },
        "graphics": {"canvas_type": 1},  # 1 = OpenGL GAL (OPENGL_GAL)
        "window": {"maximized": False, "size_x": 900, "size_y": 720, "pos_x": 40, "pos_y": 40},
    }
    sym_tpl = "/usr/share/kicad/template/sym-lib-table"
    fp_tpl = "/usr/share/kicad/template/fp-lib-table"
    sym_text = open(sym_tpl, encoding="utf-8").read() if os.path.isfile(sym_tpl) else "(sym_lib_table\n  (version 7)\n)\n"
    fp_text = open(fp_tpl, encoding="utf-8").read() if os.path.isfile(fp_tpl) else "(fp_lib_table\n  (version 7)\n)\n"

    for ver in ("10.0", "10.99"):
        cdir = os.path.join(out_dir, "fh", ".config", "kicad", ver)
        write(os.path.join(cdir, "kicad_common.json"), json.dumps(common_cfg, indent=2) + "\n")
        write(os.path.join(cdir, "pcbnew.json"), json.dumps(editor_cfg("pcbnew.json", 5), indent=2) + "\n")
        write(os.path.join(cdir, "fpedit.json"), json.dumps(editor_cfg("fpedit.json", 5), indent=2) + "\n")
        write(os.path.join(cdir, "eeschema.json"), json.dumps(editor_cfg("eeschema.json", 3), indent=2) + "\n")
        write(os.path.join(cdir, "symbol_editor.json"), json.dumps(editor_cfg("symbol_editor.json", 1), indent=2) + "\n")
        write(os.path.join(cdir, "3d_viewer.json"), json.dumps(editor_cfg("3d_viewer.json", 4), indent=2) + "\n")
        write(os.path.join(cdir, "sym-lib-table"), sym_text)
        write(os.path.join(cdir, "fp-lib-table"), fp_text)
        write(os.path.join(cdir, "design-block-lib-table"), "(design_block_lib_table\n  (version 7)\n)\n")

    # Seed Backplane hub events.jsonl and stand-in agent shims
    now = int(time.time())
    kicad_cli = os.environ.get("KICAD_CLI", "").strip()
    events = [
        {"$": "ProjectCreated", "id": "p1", "title": "ecc83-pp", "root": proj, "at": now},
        {
            "$": "ThreadCreated",
            "id": "t1",
            "project": "p1",
            "title": "Embedded KiCad E2E",
            "env": "local",
            "branch": "",
            "worktree": "",
            "provider": "claude",
            "model": "",
            "at": now,
        },
    ]
    if kicad_cli and os.path.isfile(kicad_cli):
        events.append({"$": "SettingSet", "key": "kicad.path", "value": kicad_cli, "at": now})
    # KEMBED_SEED_MESSAGE: an agent's message already in the thread (the cross-probe suite clicks the parts it names)
    seed_msg = os.environ.get("KEMBED_SEED_MESSAGE", "")
    if seed_msg:
        events.append({"$": "MessagePosted", "thread": "t1", "msg": "m2", "role": "assistant", "text": seed_msg, "at": now})
    write(os.path.join(out_dir, "home", "events.jsonl"), "".join(json.dumps(e, separators=(",", ":")) + "\n" for e in events))

    bun = shutil.which("bun") or "/usr/bin/env bun"
    standin = os.path.join(repo_root, "test", "tools", "claude_standin.ts")
    write(os.path.join(out_dir, "bin", "claude"), f'#!/bin/sh\nexec "{bun}" "{standin}" "$@"\n', executable=True)
    for blocked in ("codex", "grok", "muse", "opencode"):
        write(os.path.join(out_dir, "bin", blocked), "#!/bin/sh\nexit 1\n", executable=True)


if __name__ == "__main__":
    main()
