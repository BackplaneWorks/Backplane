#!/usr/bin/env python3
"""A test's copy of a person's or a fixture's KiCad project, without KiCad's own scratch.

KiCad keeps working files beside a project: an autosave (`_autosave-<file>`), lock files (`<file>.lck`, `~<file>.lck`), backups
(`*-bak`, `*-backups/`, `#auto_saved_files#`), its local history (`.history/`), the per-person view state (`*.kicad_prl`) and the
footprint cache (`fp-info-cache`). Copied into a test project they change what KiCad does on open: an autosave newer than the board
makes it stop on "File Open Warning" (a recover question), a lock on "is open elsewhere". The one list of them is here; every suite
copies through `copy_project` (Python) or `project_copy.py SRC DST` (shell). The source is only read: nothing in the person's project
is touched, moved or deleted.

usage: project_copy.py SRC DST [--skip PATTERN ...] [--symlinks] [--fresh]
  --skip     more names to leave out (a suite's own: mfg, build, docs ...), shell patterns like the list below
  --symlinks keep symlinks as links (default: copy what they point at)
  --fresh    remove DST first (default: copy into it, keeping what is there, as `rsync -a SRC/ DST/` did)
"""
import fnmatch
import os
import shutil
import sys

# KiCad's own scratch beside a project: never copied into a test's project
SCRATCH = ("_autosave-*", "*.lck", "~*.lck", "*-bak", "#auto_saved_files#", ".history", "*.kicad_prl", "fp-info-cache",
           "*-backups", "*.kicad_prl.bak")
# what is the person's or the repository's, never the design's
NOT_DESIGN = (".git", ".backplane")


def skipped(name, extra=()):
    """Whether a file or folder of this name is left out of a test's copy."""
    return any(fnmatch.fnmatch(name, p) for p in SCRATCH + NOT_DESIGN + tuple(extra))


def copy_project(src, dst, extra=(), symlinks=False, fresh=False):
    """Copy the project folder `src` into `dst` without KiCad's scratch (SCRATCH) and `extra`. `src` is only read."""
    if fresh and os.path.exists(dst):
        shutil.rmtree(dst)
    shutil.copytree(src, dst, ignore=lambda _d, names: [n for n in names if skipped(n, extra)], symlinks=symlinks, dirs_exist_ok=True)
    return dst


def selftest():
    """A project with KiCad's scratch beside the board (an autosave newer than it, locks, history, view state, cache, backups):
    the copy has the design and none of the scratch, and the source is byte for byte as it was."""
    import hashlib, tempfile
    root = tempfile.mkdtemp(prefix="project-copy-"); src = os.path.join(root, "proj"); dst = os.path.join(root, "copy")
    design = {"b.kicad_pro": "{}", "b.kicad_pcb": "(kicad_pcb)", "b.kicad_sch": "(kicad_sch)", "sub/s.kicad_sch": "(kicad_sch)", "fp.pretty/x.kicad_mod": "(footprint)", "mfg/g.gbr": "G"}
    scratch = {"_autosave-b.kicad_pcb": "(kicad_pcb newer)", "sub/_autosave-s.kicad_sch": "x", "b.kicad_pcb.lck": "l", "~b.kicad_sch.lck": "l", "b.kicad_pcb-bak": "x",
               "#auto_saved_files#": "x", ".history/.git/HEAD": "ref", ".history/b.kicad_pcb": "x", "b.kicad_prl": "{}", "fp-info-cache": "c", "b-backups/b.zip": "z", ".git/HEAD": "ref"}
    for f, s in {**design, **scratch}.items():
        os.makedirs(os.path.dirname(os.path.join(src, f)), exist_ok=True); open(os.path.join(src, f), "w").write(s)
    def tree(d): return {os.path.relpath(os.path.join(a, f), d): hashlib.sha256(open(os.path.join(a, f), "rb").read()).hexdigest() for a, _, fs in os.walk(d) for f in fs}
    before = tree(src)
    copy_project(src, dst, extra=("mfg",))
    got = set(tree(dst))
    want = {f for f in design if not f.startswith("mfg/")}
    assert got == want, f"copied {sorted(got)}, want {sorted(want)}"
    assert tree(src) == before, "the source was changed"
    shutil.rmtree(root)
    print("project_copy selftest: ok (the design copied; autosave, locks, history, view state, cache, backups and mfg left out; the source untouched)")


def main(argv):
    if argv == ["--selftest"]:
        return selftest()
    args, extra, symlinks, fresh = [], [], False, False
    it = iter(argv)
    for a in it:
        if a == "--skip": extra.append(next(it))
        elif a == "--symlinks": symlinks = True
        elif a == "--fresh": fresh = True
        else: args.append(a)
    if len(args) != 2:
        sys.exit(__doc__)
    copy_project(args[0].rstrip("/"), args[1].rstrip("/"), extra, symlinks, fresh)


if __name__ == "__main__":
    main(sys.argv[1:])
