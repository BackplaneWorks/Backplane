#!/usr/bin/env python3
"""Which Backplane KiCad runtime a KiCad really is, proved rather than guessed from where its executable sits.

Since e9371ff87 the hub may start the GUI through a view of the runtime made for the computer's own loader (native/host/src/kicad/hostlibs.rs):
<home>/kicad-embed-cache/hostlibs/<runtime folder>/{kicad, lib/, made}. `kicad` is a copy of the runtime's wrapper whose last line runs
`<computer's ld-linux> --library-path <view>/lib <root>/bin/kicad.real`, `lib/` links into the runtime's own lib/, and `made` records the
runtime it was made from (first line ok|refused, second the runtime's root). So /proc/<pid>/exe is the computer's loader, and argv[0] in the
hub's log is the view's copy: neither names the runtime.

  kicad_runtime.py pid PID ASKED_KICAD_BIN       the running process: its kicad.real from its command line, confirmed mapped (maps),
                                                 the view's recorded origin when it ran through one
  kicad_runtime.py program PATH ASKED_KICAD_BIN  a program the hub started (its log's argv[0]): a view's origin, else the runtime it is in
Prints JSON {root, how, commit, asked_root, asked_commit, same, why}; exit 0 when the runtime is the one asked for, 1 when it is not, 2 when
it cannot be told (that is a failure for the caller too: nothing proved).
"""
import json, os, sys


def commit_of(root):
    """The runtime's own identity: BACKPLANE_COMMIT (bp.13 builds all say 10.99.0-unknown-bp.13), else SOURCE_COMMIT, else manifest's."""
    for f in ("BACKPLANE_COMMIT", "SOURCE_COMMIT"):
        try:
            v = open(os.path.join(root, f)).read().strip()
            if v: return f"{f}:{v}"
        except OSError: pass
    try:
        m = json.load(open(os.path.join(root, "manifest.json")))
        v = m.get("sourceCommit") or m.get("source_commit")
        if v: return f"manifest:{v}"
    except (OSError, ValueError): pass
    return ""


def runtime_of_bin(path):
    """<root>/bin/<anything> -> <root> (real path)."""
    return os.path.dirname(os.path.dirname(os.path.realpath(path)))


def view_origin(view):
    """A hostlibs view's recorded origin (the runtime's root), checked against what the view itself is: its wrapper's root= line and
    its lib/ links. None when `view` is not a view; raises ValueError when it is one that does not agree with itself."""
    made = os.path.join(view, "made")
    if not os.path.isfile(made): return None
    lines = open(made).read().split("\n")
    if len(lines) < 2 or lines[0] != "ok": raise ValueError(f"{made}: not a view in use ({lines[0]!r})")
    origin = os.path.realpath(lines[1])
    wrapper = os.path.join(view, "kicad")
    if os.path.isfile(wrapper):
        roots = [l.rstrip("\n")[len("root="):].strip("'") for l in open(wrapper) if l.startswith("root=")]
        if roots and os.path.realpath(roots[0].replace("'\\''", "'")) != origin:
            raise ValueError(f"{wrapper}: root= {roots[0]} is not the recorded origin {origin}")
    lib = os.path.join(view, "lib")
    links = [os.path.join(lib, n) for n in sorted(os.listdir(lib))[:20]] if os.path.isdir(lib) else []
    stray = [l for l in links if os.path.islink(l) and not os.path.realpath(l).startswith(origin + os.sep)]
    if stray: raise ValueError(f"{lib}: links outside {origin}: {stray[:3]}")
    return origin


def from_pid(pid):
    exe = os.readlink(f"/proc/{pid}/exe")
    argv = [a.decode("utf-8", "replace") for a in open(f"/proc/{pid}/cmdline", "rb").read().split(b"\0") if a]
    real = next((a for a in argv if a.endswith(".real") and os.sep + "bin" + os.sep in a), None)
    if real is None:
        # started straight (no wrapper's loader line): the executable is the program
        return runtime_of_bin(exe), f"exe {exe}"
    maps = open(f"/proc/{pid}/maps").read()
    if real not in maps and os.path.realpath(real) not in maps: raise ValueError(f"{real} (from the command line) is not mapped in pid {pid}")
    real = os.path.realpath(real)
    root = runtime_of_bin(real)
    how = f"exe {exe}, loaded {real} (cmdline + maps)"
    if "--library-path" in argv:
        libdir = argv[argv.index("--library-path") + 1]
        origin = view_origin(os.path.dirname(libdir))
        if origin is not None:
            if origin != root: raise ValueError(f"the view {os.path.dirname(libdir)} was made from {origin}, but {real} is loaded")
            how += f", through the view {os.path.dirname(libdir)} made from it"
    return root, how


def from_program(path):
    origin = view_origin(os.path.dirname(os.path.realpath(path)))
    if origin is not None: return origin, f"view {os.path.dirname(path)} made from it"
    return runtime_of_bin(path), f"program {path}"


def main():
    mode, what, asked = sys.argv[1], sys.argv[2], sys.argv[3]
    out = {"asked_root": runtime_of_bin(asked) if asked else ""}
    try:
        root, how = from_pid(int(what)) if mode == "pid" else from_program(what)
    except (OSError, ValueError, IndexError) as e:
        out.update(same=False, why=str(e)); print(json.dumps(out)); return 2
    out.update(root=root, how=how, commit=commit_of(root), asked_commit=commit_of(out["asked_root"]) if asked else "")
    same = (not asked) or (root == out["asked_root"] and out["commit"] == out["asked_commit"])
    out["same"] = same
    if not same: out["why"] = f"runs {root} ({out['commit'] or 'no commit'}), asked {out['asked_root']} ({out['asked_commit'] or 'no commit'})"
    print(json.dumps(out)); return 0 if same else 1


if __name__ == "__main__":
    sys.exit(main())
