#!/usr/bin/env python3
"""The files outside native/gpui that the client's build reads, from its sources: every `#[path = "..."]` module and every
include_str!/include_bytes!/include! with a literal path, in native/gpui/src and build.rs, followed into the .rs files they name (a hub
file the client includes may include more). Printed relative to the repository root, one per line, sorted: what scripts/mac-build.sh
copies beside native/gpui so a fresh copy builds (Nov 20: native/winres.rs and host/src/{mouse,scopeprobe,bots/cat}.rs were missing).
Paths are resolved as rustc does: relative to the directory of the file that names them. A named file that does not exist is an
error (exit 1): the list would be wrong.

usage: gpui-outside-files.py [REPO_ROOT]   |   gpui-outside-files.py --self-test (a made-up tree: followed, comments ignored, missing is an error)
"""
import os, re, sys

PATH_ATTR = re.compile(r'#\[path\s*=\s*"([^"]+)"\s*\]')
COMMENT = re.compile(r'(?m)(^|\s)//.*$')
INCLUDE = re.compile(r'\binclude(?:_str|_bytes)?!\(\s*"([^"]+)"')

def named(path):
    try: text = open(path, encoding="utf-8", errors="replace").read()
    except OSError: return []
    text = COMMENT.sub("", text)  # a doc comment that names `#[path = ...]` is not one
    base = os.path.dirname(path)
    return [os.path.normpath(os.path.join(base, p)) for p in PATH_ATTR.findall(text) + INCLUDE.findall(text)]

def outside_files(ROOT):
    GPUI = os.path.join(ROOT, "native", "gpui")
    todo = [os.path.join(GPUI, "build.rs")]
    for d, dirs, files in os.walk(os.path.join(GPUI, "src")):
        todo += [os.path.join(d, f) for f in files if f.endswith(".rs")]
    seen, outside, missing = set(), set(), []
    while todo:
        f = todo.pop()
        if f in seen: continue
        seen.add(f)
        for p in named(f):
            if not os.path.exists(p): missing.append(f"{os.path.relpath(p, ROOT)} (named in {os.path.relpath(f, ROOT)})"); continue
            if os.path.commonpath([p, GPUI]) != GPUI:
                if os.path.commonpath([p, ROOT]) != ROOT: missing.append(f"{p} is outside the repository (named in {os.path.relpath(f, ROOT)})"); continue
                outside.add(os.path.relpath(p, ROOT))
                if p.endswith(".rs"): todo.append(p)
    return sorted(outside), sorted(missing)

def self_test():
    import tempfile
    with tempfile.TemporaryDirectory() as r:
        def w(rel, text): p = os.path.join(r, rel); os.makedirs(os.path.dirname(p), exist_ok=True); open(p, "w").write(text)
        w("native/gpui/build.rs", '#[path = "../winres.rs"]\nmod winres;\n')
        w("native/gpui/src/main.rs", '#[path = "../../host/src/mouse.rs"] mod mouse;\n// #[path = "../../nowhere.rs"] a comment\nconst F: &[u8] = include_bytes!("../../../assets/f.ttf");\nmod a;\n')
        w("native/gpui/src/a.rs", 'const S: &str = include_str!("a.txt");\n')
        w("native/gpui/src/a.txt", "inside\n")
        w("native/winres.rs", "//! names `#[path = \"../winres.rs\"] mod winres;` in a doc comment\n")
        w("native/host/src/mouse.rs", 'const B: &str = include_str!("badge.txt");\n')
        w("native/host/src/badge.txt", "x"); w("assets/f.ttf", "x")
        out, missing = outside_files(r)
        assert out == ["assets/f.ttf", "native/host/src/badge.txt", "native/host/src/mouse.rs", "native/winres.rs"], out
        assert not missing, missing
        w("native/gpui/src/a.rs", 'const S: &str = include_str!("gone.txt");\n')
        assert outside_files(r)[1] == ["native/gpui/src/gone.txt (named in native/gpui/src/a.rs)"], outside_files(r)
    print("gpui-outside-files: self-test ok")

if __name__ == "__main__":
    if sys.argv[1:] == ["--self-test"]: self_test(); sys.exit(0)
    out, missing = outside_files(os.path.abspath(sys.argv[1] if len(sys.argv) > 1 else os.path.join(os.path.dirname(__file__), "..")))
    if missing: print("gpui-outside-files: not found: " + "; ".join(missing), file=sys.stderr); sys.exit(1)
    print("\n".join(out))
