#!/usr/bin/env python3
"""A SPICE deck through the KiCad runtime's own libngspice (the runtime ships the library and its code models, no `ngspice` program):
  ngs.py RUNTIME DECK.cir 'ac dec 50 1 1meg' ['meas ac f3 when vdb(out)=-3' ...] [--vec NAME ...] [--compat psa]  -> JSON on stdout
  module: run(rt, deck, commands, vecs=(), compat="", cwd=None, timeout=600) -> {"ok", "log", "vecs": {name: [..] | [[re, im], ..]}}
Each run is a process of its own (the library keeps one circuit and may exit the process). The deck is the netlist text (title line first, `.end`
last; its `.include` paths are relative to cwd); the commands are ngspice's interactive ones, run in order; a vector is asked for by its name in
the last plot (`v(out)`, `frequency`, a `meas` result or a `let`) or with its plot (`tran1.v(out)`)."""
import ctypes, glob, json, os, re, subprocess, sys

class Cx(ctypes.Structure): _fields_ = [("re", ctypes.c_double), ("im", ctypes.c_double)]
class Vec(ctypes.Structure):
    _fields_ = [("name", ctypes.c_char_p), ("type", ctypes.c_int), ("flags", ctypes.c_short), ("real", ctypes.POINTER(ctypes.c_double)),
                ("comp", ctypes.POINTER(Cx)), ("length", ctypes.c_int)]

def child(rt, deck, commands, vecs, compat):
    lib = ctypes.CDLL(rt + "/lib/libngspice.so.0"); log = []
    PR = ctypes.CFUNCTYPE(ctypes.c_int, ctypes.c_char_p, ctypes.c_int, ctypes.c_void_p)
    EX = ctypes.CFUNCTYPE(ctypes.c_int, ctypes.c_int, ctypes.c_bool, ctypes.c_bool, ctypes.c_int, ctypes.c_void_p)
    def pr(s, i, p): log.append(s.decode("utf-8", "replace")); return 0
    def ex(st, im, q, i, p): log.append("ngspice asked to exit (%d)" % st); return 0
    keep = (PR(pr), PR(pr), EX(ex)); lib.ngSpice_Init(keep[0], keep[1], keep[2], None, None, None, None)
    lib.ngGet_Vec_Info.restype = ctypes.POINTER(Vec); lib.ngSpice_CurPlot.restype = ctypes.c_char_p
    cmd = lambda c: lib.ngSpice_Command(c.encode())
    if compat: cmd("set ngbehavior=" + compat)
    for cm in sorted(glob.glob(rt + "/lib/ngspice/*.cm")): cmd("codemodel " + cm)
    lines = [l.encode() for l in deck.splitlines()]; arr = (ctypes.c_char_p * (len(lines) + 1))(*lines, None)
    rc = lib.ngSpice_Circ(arr); out = {"ok": rc == 0, "vecs": {}}
    for c in commands:
        log.append("> " + c)
        if cmd(c): out["ok"] = False
    for v in vecs:
        p = lib.ngGet_Vec_Info(v.encode())
        if not p: out["vecs"][v] = None; continue
        x = p.contents
        out["vecs"][v] = [x.real[i] for i in range(x.length)] if x.real else [[x.comp[i].re, x.comp[i].im] for i in range(x.length)]
    out["plot"] = (lib.ngSpice_CurPlot() or b"").decode(); out["log"] = "\n".join(log)
    bad = [l for l in log if re.search(r"\berror\b|unknown subckt|could not find|singular matrix|timestep too small|no such vector|\baborted\b|\bfatal\b|cannot recover", l, re.I)]
    out["problems"] = bad[:20]
    return out

def run(rt, deck, commands, vecs=(), compat="", cwd=None, timeout=600):
    r = subprocess.run([sys.executable, os.path.abspath(__file__), "--child"], input=json.dumps({"rt": rt, "deck": deck, "commands": list(commands), "vecs": list(vecs), "compat": compat}),
                       capture_output=True, text=True, cwd=cwd, timeout=timeout)
    try: return json.loads(r.stdout.strip().splitlines()[-1])
    except Exception: return {"ok": False, "vecs": {}, "log": (r.stdout + r.stderr)[-3000:], "problems": ["ngspice ended without an answer (exit %d)" % r.returncode]}

if __name__ == "__main__":
    if sys.argv[1] == "--child":
        j = json.load(sys.stdin); fd = os.dup(1); os.dup2(2, 1)  # whatever the library prints itself goes to stderr; stdout carries the answer only
        o = child(j["rt"], j["deck"], j["commands"], j["vecs"], j["compat"]); os.write(fd, (json.dumps(o) + "\n").encode()); os._exit(0)
    a = sys.argv[1:]; vecs = []; compat = ""
    while "--vec" in a: i = a.index("--vec"); vecs.append(a[i + 1]); del a[i:i + 2]
    if "--compat" in a: i = a.index("--compat"); compat = a[i + 1]; del a[i:i + 2]
    o = run(a[0], open(a[1]).read(), a[2:], vecs, compat, cwd=os.path.dirname(os.path.abspath(a[1])))
    print(json.dumps(o, indent=1))
