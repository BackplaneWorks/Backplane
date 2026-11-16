#!/usr/bin/env python3
"""kicad_runtime.py's program mode on a made-up runtime and hostlibs view: python3 -I test/kicad_embed/lib/kicad_runtime_test.py"""
import json, os, subprocess, sys, tempfile
HERE = os.path.dirname(os.path.abspath(__file__)); K = os.path.join(HERE, "kicad_runtime.py")
def run(*a): p = subprocess.run([sys.executable, K, *a], capture_output=True, text=True); return p.returncode, json.loads(p.stdout or "{}")
def runtime(base, name, commit):
    r = os.path.join(base, name); os.makedirs(os.path.join(r, "bin")); os.makedirs(os.path.join(r, "lib"))
    open(os.path.join(r, "bin", "kicad"), "w").write("#!/bin/sh\n"); open(os.path.join(r, "lib", "libwx.so"), "w").write("")
    open(os.path.join(r, "BACKPLANE_COMMIT"), "w").write(commit + "\n"); return r
def view(base, r, origin=None, root_line=None, link_to=None, state="ok"):
    v = os.path.join(base, "hostlibs", os.path.basename(r)); os.makedirs(os.path.join(v, "lib"))
    os.symlink(os.path.join(link_to or r, "lib", "libwx.so"), os.path.join(v, "lib", "libwx.so"))
    open(os.path.join(v, "kicad"), "w").write(f"#!/usr/bin/env sh\nroot='{root_line or r}'\nprogram=kicad\nexec '/lib64/ld-linux-x86-64.so.2' --library-path '{v}/lib' \"$root/bin/$program.real\" \"$@\"\n")
    open(os.path.join(v, "made"), "w").write(f"{state}\n{origin or r}\n/lib64/ld-linux-x86-64.so.2\n"); return v
fails = 0
def expect(what, got, want):
    global fails
    print(("PASS " if got == want else "FAIL ") + what + ("" if got == want else f" (got {got}, want {want})")); fails += got != want
with tempfile.TemporaryDirectory() as t:
    a = runtime(t, "rt-a", "aaa"); b = runtime(t, "rt-b", "bbb")
    v = view(t, a)
    rc, o = run("program", os.path.join(v, "kicad"), os.path.join(a, "bin", "kicad")); expect("a view made from the runtime asked for is it", rc, 0)
    rc, o = run("program", os.path.join(v, "kicad"), os.path.join(b, "bin", "kicad")); expect("... and is not another runtime", rc, 1)
    rc, o = run("program", os.path.join(a, "bin", "kicad"), os.path.join(a, "bin", "kicad")); expect("the runtime's own wrapper (no view) is it", rc, 0)
    t2 = os.path.join(t, "x"); os.makedirs(t2)
    v2 = view(t2, a, root_line=b); rc, o = run("program", os.path.join(v2, "kicad"), os.path.join(a, "bin", "kicad")); expect("a view whose wrapper names another root cannot be told", rc, 2)
    t3 = os.path.join(t, "y"); os.makedirs(t3)
    v3 = view(t3, a, link_to=b); rc, o = run("program", os.path.join(v3, "kicad"), os.path.join(a, "bin", "kicad")); expect("a view linking into another runtime cannot be told", rc, 2)
    t4 = os.path.join(t, "z"); os.makedirs(t4)
    v4 = view(t4, a, state="refused"); rc, o = run("program", os.path.join(v4, "kicad"), os.path.join(a, "bin", "kicad")); expect("a refused view is not one in use", rc, 2)
sys.exit(1 if fails else 0)
