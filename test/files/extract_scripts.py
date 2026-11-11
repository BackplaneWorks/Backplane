#!/usr/bin/env python3
"""Extracts the shell scripts of src/server/git (zero-argument defs built from string literals, `++`, `String.join`
and other such defs) into native/host/src/files/scripts/*.sh, so the Rust hub runs the same text as hub.
  extract_scripts.py          write the files
  extract_scripts.py --check  fail when a file differs from what git says now
"""
import re, sys, os
ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))
SRC = "\n".join(open(os.path.join(ROOT, f)).read() for f in ["src/server/git", "src/server/pdf", "src/core/links", "src/server/server", "src/server/review", "src/server/kexport"]).replace("GT.Git.", "Git.").replace("LK.Link.file.cmd(\"open\")", "Link.file.open()")
OUT = os.path.join(ROOT, "native/host/src/files/scripts")
WANT = {"Git.safe.sh": "safe.sh", "Git.index.seed.sh": "index_seed.sh", "Git.step.script": "step.sh", "Git.step.migrate.sh": "step_migrate.sh",
        "Git.refs.script": "refs.sh", "Git.diff.cap.sh": "diff_cap.sh",
        "Pdf.sh": "pdf.sh", "Link.file.open": "file_open.sh", "Link.file.reveal": "file_reveal.sh", "Mech.cmd": "mech_list.sh", "RvJob.resolve.sh": "resolve.sh", "Git.init.sh": "init.sh",
        "Rvc.prep.sh": "review_prep.sh", "Rvc.bound.sh": "review_bound.sh", "RvJob.pr.sh": "review_pr.sh", "Rvr.prep.sh": "release_prep.sh", "Rvr.output.sh": "release_output.sh", "Rvr.files.sh": "release_files.sh",
        "Rvr.commit.sh": "release_commit.sh", "Rvr.source.sh": "release_source.sh", "Rvr.tag.sh": "release_tag.sh", "KExport.guard": "kexport_guard.sh", "KExport.open_script": "kexport_open.sh"}
defs = {}
for m in re.finditer(r"^def ([A-Z][A-Za-z]*\.[A-Za-z_.]+)\(\) -> String:\n((?:  .*\n|\n)+?)(?=\S|\Z)", SRC, re.M):
    defs[m.group(1)] = m.group(2).strip("\n")

# FreeCAD's scripts (freecad_gui.py, freecad_console.py, freecad_batch.py) are their own source now, in native/host/src/files/scripts: the
# Bun step2glb they were taken from is gone (native/step2glb meshes natively; FreeCAD is the last resort).

def lit(s, i):
    assert s[i] == '"'
    i += 1; out = []
    while s[i] != '"':
        if s[i] == "\\":
            c = s[i + 1]; out.append({"n": "\n", "t": "\t", "\\": "\\", '"': '"', "r": "\r"}[c]); i += 2
        else: out.append(s[i]); i += 1
    return "".join(out), i + 1

def ev(s):
    s = s.strip(); i = 0; vals = []
    def ws():
        nonlocal i
        while i < len(s) and s[i] in " \n\t": i += 1
    def term():
        nonlocal i
        ws()
        if s[i] == '"': v, i = lit(s, i); return v
        m = re.match(r"String\.join\(\[", s[i:])
        if m:
            i += m.end(); items = []
            while True:
                ws()
                if s[i] == "]": i += 1; break
                items.append(expr()); ws()
                if s[i] == ",": i += 1
            ws(); assert s[i] == ","; i += 1; sep = term(); ws(); assert s[i] == ")"; i += 1
            return sep.join(items)
        m = re.match(r"([A-Z][A-Za-z]*\.[A-Za-z_.]+)\(\)", s[i:])
        if m:
            i += m.end(); return call(m.group(1))
        raise SystemExit("cannot evaluate at: " + s[i:i + 60])
    def expr():
        nonlocal i
        v = term(); ws()
        while s[i:i + 2] == "++": i += 2; v += term(); ws()
        return v
    return expr()

def call(name):
    return ev(defs[name])

if __name__ == "__main__":
    check = "--check" in sys.argv; bad = 0
    os.makedirs(OUT, exist_ok=True)
    for f, text in [(f, call(name)) for name, f in WANT.items()]:
        p = os.path.join(OUT, f)
        if check:
            if not os.path.exists(p) or open(p).read() != text: print("DIFFERS:", f); bad = 1
        else: open(p, "w").write(text)
    if check and not bad: print("scripts match git")
    sys.exit(bad)
