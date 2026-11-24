#!/usr/bin/env python3
"""The files and folders outside native/host that the hub's build and tests read, from its sources (what scripts/land.sh's Windows gate
watches besides native/host, so a new embed can never be missed: web/dist was, Nov 20). From every .rs file in native/host (src, tests,
build.rs; vendor and target left out) and the .rs files outside it that they name, followed:
  - `#[path = "..."]` modules and include_str!/include_bytes!/include! with a literal path, resolved from the naming file's folder (rustc's rule);
  - include_*!(concat!(env!("CARGO_MANIFEST_DIR"), "<lit>", ...)): native/host + <lit>; when more follows (a macro's file name), the folder
    the literal leads into (webingress.rs embeds web/dist/<file> this way);
  - CARGO_MANIFEST_DIR (env! or env::var) joined with a literal on the same line (build.rs's ../../deploy; tests that read a script by path);
  - Cargo.toml path dependencies.
Printed relative to the repository root, one per line, sorted, those outside native/host only. A compile-time input that does not exist is an
error (exit 1): the list would be wrong. A runtime join that does not exist is skipped (a test may make it), and so is the repository root itself.

usage: hub-outside-files.py [--crate native/gpui] [REPO_ROOT]   |   hub-outside-files.py --self-test     (another crate's the same way: --crate)
"""
import os, re, sys

PATH_ATTR = re.compile(r'#\[path\s*=\s*"([^"]+)"\s*\]')
COMMENT = re.compile(r'(?m)(^|\s)//.*$')
INCLUDE = re.compile(r'\binclude(?:_str|_bytes)?!\(\s*"([^"]+)"')
INCLUDE_CONCAT = re.compile(r'\binclude(?:_str|_bytes)?!\(\s*concat!\(\s*env!\(\s*"CARGO_MANIFEST_DIR"\s*\)\s*,\s*"([^"]+)"\s*(,)?')
MANIFEST_JOIN = re.compile(r'(?:env!\(\s*"CARGO_MANIFEST_DIR"\s*\)|env::var\(\s*"CARGO_MANIFEST_DIR"\s*\)).*?\.join\(\s*"([^"]+)"\s*\)')
PATH_DEP = re.compile(r'path\s*=\s*"([^"]+)"')

def named(path, manifest):
    """(compile-time inputs, runtime joins) a .rs file names, as absolute normalized paths."""
    try: text = open(path, encoding="utf-8", errors="replace").read()
    except OSError: return [], []
    text = COMMENT.sub("", text)  # a doc comment that names `#[path = ...]` is not one
    base = os.path.dirname(path)
    rel = lambda p: os.path.normpath(os.path.join(base, p))
    built = [rel(p) for p in PATH_ATTR.findall(text) + INCLUDE.findall(text)]
    for lit, more in INCLUDE_CONCAT.findall(text):
        p = os.path.normpath(manifest + "/" + lit.lstrip("/"))
        built.append(p if not more else (p if lit.endswith("/") else os.path.dirname(p)))
    joins = [os.path.normpath(os.path.join(manifest, p)) for p in MANIFEST_JOIN.findall(text)]
    return built, joins

def outside_files(ROOT, crate="native/host"):
    ROOT = os.path.abspath(ROOT); HOST = os.path.join(ROOT, *crate.split("/"))
    todo = []
    for d, dirs, files in os.walk(HOST):
        dirs[:] = [x for x in dirs if x not in ("target", "vendor", ".git")]
        todo += [os.path.join(d, f) for f in files if f.endswith(".rs")]
    seen, outside, missing = set(), set(), []
    def keep(p, f, required):
        if not os.path.exists(p):
            if required: missing.append(f"{os.path.relpath(p, ROOT)} (named in {os.path.relpath(f, ROOT)})")
            return
        if os.path.commonpath([p, HOST]) == HOST: return
        if os.path.commonpath([p, ROOT]) != ROOT:
            if required: missing.append(f"{p} is outside the repository (named in {os.path.relpath(f, ROOT)})")
            return
        if p == ROOT: return  # a join to the root reads something under it at run time; what, the source does not say
        outside.add(os.path.relpath(p, ROOT))
        if p.endswith(".rs"): todo.append(p)
    while todo:
        f = todo.pop()
        if f in seen: continue
        seen.add(f)
        built, joins = named(f, HOST)
        for p in built: keep(p, f, True)
        for p in joins: keep(p, f, False)
    try:
        for p in PATH_DEP.findall(open(os.path.join(HOST, "Cargo.toml")).read()): keep(os.path.normpath(os.path.join(HOST, p)), os.path.join(HOST, "Cargo.toml"), True)
    except OSError: pass
    return sorted(outside), sorted(missing)

def self_test():
    import tempfile
    with tempfile.TemporaryDirectory() as r:
        def w(rel, text): p = os.path.join(r, rel); os.makedirs(os.path.dirname(p), exist_ok=True); open(p, "w").write(text)
        w("native/host/build.rs", '#[path = "../winres.rs"]\nmod winres;\nfn k() { let root = Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap()).join("../../deploy"); }\n')
        w("native/host/src/main.rs", '#[path = "../../shared/x.rs"] mod x;\n// include_str!("../../nowhere.txt") a comment\nconst F: &[u8] = include_bytes!("../../../assets/f.ttf");\nmod web;\n')
        w("native/host/src/web.rs", 'macro_rules! asset { ($p:literal) => { include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../web/dist/", $p)) } }\n'
                                   'const M: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../web/manifest.json"));\n'
                                   'fn t() { let s = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/tool.sh"); let r = Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."); let g = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../made-by-test"); }\n')
        w("native/shared/x.rs", 'const S: &str = include_str!("x.txt");\n'); w("native/shared/x.txt", "x")
        w("native/winres.rs", "//! names `#[path = \"../gone.rs\"] mod g;` in a doc comment\n")
        w("assets/f.ttf", "x"); w("web/dist/index.html", "x"); w("web/manifest.json", "{}"); w("deploy/kistack.lock", "x"); w("scripts/tool.sh", "x")
        w("native/host/Cargo.toml", '[dependencies]\nlocal = { path = "../local-dep" }\nv = { path = "vendor/v" }\n'); w("native/local-dep/Cargo.toml", "x"); w("native/host/vendor/v/Cargo.toml", "x")
        out, missing = outside_files(r)
        assert out == ["assets/f.ttf", "deploy", "native/local-dep", "native/shared/x.rs", "native/shared/x.txt", "native/winres.rs", "scripts/tool.sh", "web/dist", "web/manifest.json"], out
        assert not missing, missing
        w("native/shared/x.rs", 'const S: &str = include_str!("gone.txt");\n')
        assert outside_files(r)[1] == ["native/shared/gone.txt (named in native/shared/x.rs)"], outside_files(r)
    print("hub-outside-files: self-test ok")

if __name__ == "__main__":
    if sys.argv[1:2] == ["--self-test"]: self_test(); sys.exit(0)
    a = sys.argv[1:]; crate = "native/host"
    if a[:1] == ["--crate"]: crate = a[1]; a = a[2:]
    root = a[0] if a else os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
    out, missing = outside_files(root, crate)
    if missing: print("hub-outside-files: named but missing:\n  " + "\n  ".join(missing), file=sys.stderr); sys.exit(1)
    print("\n".join(out))
