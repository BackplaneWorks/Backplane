#!/usr/bin/env python3
"""plot_modes.py OUT.json ORACLE BPPLOT [--fuzz N]: bp-plot against the Bend child beyond the plain corpus: sheet trees, info lookups, files in a
commit, comparisons, files half written, missing or malformed, mutated boards and schematics. Every case compares exit code, stdout, stderr
(trimmed) and the info file, byte for byte. Runs only finished binaries; run it inside a 3.7 GiB scope."""
import hashlib, json, os, random, re, shutil, subprocess, sys
D = os.path.expanduser("~/.backplane-bend/kicad/10.99.0-bp.4/share/kicad/demos")
out, oracle, rust = sys.argv[1:4]
a = sys.argv[4:]
fuzz = int(a[a.index("--fuzz") + 1]) if "--fuzz" in a else 200
W = os.path.abspath("build/gpui/files-run/plotport/modes"); shutil.rmtree(W, ignore_errors=True); os.makedirs(W)
oracle, rust = os.path.abspath(oracle), os.path.abspath(rust)
def run(words, si=None):
    if si and os.path.exists(si): os.remove(si)
    p = subprocess.run(words, stdin=subprocess.DEVNULL, capture_output=True, timeout=300)
    inf = open(si, "rb").read() if si and os.path.exists(si) else None
    return p.returncode, p.stdout, p.stderr.strip(), inf
res = {}; fails = []
def case(group, name, ow, rw, si=None):
    r = res.setdefault(group, [0, 0])
    x = run(ow, si); y = run(rw, si)
    r[0] += 1
    if x == y: r[1] += 1
    else:
        d = [n for n, u, v in zip(("code", "stdout", "stderr", "infos"), x, y) if u != v]
        fails.append({"group": group, "case": name, "differs": d, "oracle": [x[0], len(x[1]), x[2][:200].decode("utf-8", "replace")], "rust": [y[0], len(y[1]), y[2][:200].decode("utf-8", "replace")]})
def plot(group, name, kind, path):
    si = f"{W}/i.infos"
    case(group, name, [oracle, "--plot", str(kind), path, si], [rust, str(kind), path, si], si)
# sheet trees
schs = sorted(os.path.join(r, n) for r, _, ns in os.walk(D) for n in ns if n.endswith(".kicad_sch"))
for f in schs: case("tree", os.path.relpath(f, D), [oracle, "--plot-tree", f], [rust, "--tree", f])
# info lookups
rng = random.Random(7)
for f in [D + "/ecc83/ecc83-pp.kicad_pcb", D + "/ecc83/ecc83-pp.kicad_sch", D + "/cm5_minima/CM5_MINIMA_3.kicad_pcb", D + "/cm5_minima/USB.kicad_sch"]:
    si = f"{W}/look.infos"
    run([rust, "0" if f.endswith("pcb") else "1", f, si])
    t = open(si, encoding="utf-8").read()
    sigs = re.findall(r"(?m)^(\d+) \d+ ", t)
    for s in rng.sample(sigs, min(30, len(sigs))) + ["1", "0"]:
        for idx in ("0", "1", "3", "999"): case("info", f"{os.path.basename(f)} {s} {idx}", [oracle, "--plot-info", si, s, idx], [rust, "--info", si, s, idx])
# a repository of two commits
R = f"{W}/repo"; os.makedirs(R + "/sub")
def git(*w): subprocess.run(["git", "-c", "user.name=t", "-c", "user.email=t@t", "-C", R, *w], check=True, capture_output=True)
git("init", "-q")
for n in ("ecc83-pp.kicad_pcb", "ecc83-pp.kicad_sch"): shutil.copy(f"{D}/ecc83/{n}", R)
for n in ("complex_hierarchy.kicad_sch", "ampli_ht.kicad_sch", "complex_hierarchy.kicad_pcb"): shutil.copy(f"{D}/complex_hierarchy/{n}", R + "/sub")
git("add", "."); git("commit", "-qm", "a")
ca = subprocess.check_output(["git", "-C", R, "rev-parse", "HEAD"], text=True).strip()
def edit(p, f):
    t = open(p, encoding="utf-8", errors="surrogateescape").read(); t2 = f(t); assert t != t2, p
    open(p, "w", encoding="utf-8", errors="surrogateescape").write(t2)
n = [0]
def nudge(m):
    n[0] += 1
    return f"(start {float(m[1]) + 1}" if n[0] % 7 == 0 else m[0]
edit(R + "/ecc83-pp.kicad_pcb", lambda t: re.sub(r"\(start ([\d.]+)", nudge, t))
edit(R + "/ecc83-pp.kicad_pcb", lambda t: re.sub(r"\n\t\(segment.*?\n\t\)", "", t, count=5, flags=re.S))
edit(R + "/ecc83-pp.kicad_sch", lambda t: re.sub(r"\n\t\(wire.*?\n\t\)", "", t, count=3, flags=re.S).replace('"R1"', '"R1000"'))
os.remove(R + "/sub/ampli_ht.kicad_sch")
git("add", "-A"); git("commit", "-qm", "b")
cb = subprocess.check_output(["git", "-C", R, "rev-parse", "HEAD"], text=True).strip()
g = R + "/.git"
def ver(c, rel): return f"git\t{g}\t{c}\t{len(R)}\t{R}/{rel}"
for c, cn in ((ca, "a"), (cb, "b")):
    for rel, k in (("ecc83-pp.kicad_pcb", 0), ("ecc83-pp.kicad_sch", 1), ("sub/complex_hierarchy.kicad_sch", 1), ("sub/ampli_ht.kicad_sch", 1), ("sub/complex_hierarchy.kicad_pcb", 0), ("nothing.kicad_pcb", 0), ("nothing.kicad_sch", 1)):
        plot("version", f"{cn} {rel}", k, ver(c, rel))
    case("tree-version", cn, [oracle, "--plot-tree", ver(c, "sub/complex_hierarchy.kicad_sch")], [rust, "--tree", ver(c, "sub/complex_hierarchy.kicad_sch")])
for rel, k in (("ecc83-pp.kicad_pcb", 0), ("ecc83-pp.kicad_sch", 1), ("sub/complex_hierarchy.kicad_pcb", 0)):
    a_, b_ = ver(ca, rel), ver(cb, rel)
    for nm, p in (("a-b", f"diff\n{a_}\n{b_}"), ("b-a", f"diff\n{b_}\n{a_}"), ("self", f"diff\n{a_}\n{a_}"), ("new", f"diff\n\n{b_}"), ("gone", f"diff\n{a_}\n"), ("live", f"diff\n{a_}\n{R}/{rel}"),
                  ("bits", f"diff\n{a_}\n{b_}\n{1 << 12}"), ("bits2", f"diff\n{a_}\n{b_}\n{(1 << 14) | (1 << 3)}"), ("badbits", f"diff\n{a_}\n{b_}\nx1")):
        plot("compare", f"{rel} {nm}", k, p)
plot("compare", "sheet gone", 1, f"diff\n{ver(ca, 'sub/ampli_ht.kicad_sch')}\n{ver(cb, 'sub/ampli_ht.kicad_sch')}")
# what is not a finished file
P = f"{W}/bad"; os.makedirs(P)
base = open(f"{D}/ecc83/ecc83-pp.kicad_pcb", "rb").read(); sch = open(f"{D}/ecc83/ecc83-pp.kicad_sch", "rb").read()
def bad(name, b, k):
    f = f"{P}/{name}"; open(f, "wb").write(b); plot("file", name, k, f)
for k, b, e in ((0, base, "pcb"), (1, sch, "sch")):
    bad(f"empty.{e}", b"", k); bad(f"cut-half.{e}", b[:len(b) // 2], k); bad(f"cut-end.{e}", b[:-1], k); bad(f"blank.{e}", b"   \n", k); bad(f"word.{e}", b"hello", k); bad(f"paren.{e}", b"()", k); bad(f"one.{e}", b"(x)", k)
    bad(f"cut-string.{e}", b"(a (b \"cd", k); bad(f"extra.{e}", b + b")", k); bad(f"twice.{e}", b + b, k); bad(f"nul.{e}", b.replace(b"(uuid", b"(uu\0id", 40), k); bad(f"badutf.{e}", b.replace(b"\"R1\"", b"\"R\xff\xfe1\""), k)
    bad(f"crlf.{e}", b.replace(b"\n", b"\r\n"), k); bad(f"escape.{e}", b.replace(b"\"R1\"", b"\"R\\n1\\\"\\t~{X}_{y}{slash}\""), k); bad(f"unicode.{e}", b.replace(b"\"R1\"", "\"Ω日本語😀µ±°\"".encode()), k)
    plot("file", f"missing.{e}", k, f"{P}/missing.{e}")
    plot("file", f"folder.{e}", k, P)
# mutations
tok = re.compile(rb"-?\d+\.\d+|-?\d+|\"[^\"\\]*\"|yes|no|hide|left|right|top|bottom|mirror|italic|bold")
swap = [b"0", b"-1", b"1e3", b"1e400", b"-1e400", b"inf", b"nan", b"-0", b"0.0000001", b"16777217", b"4294967296", b"1.5e-7", b".5", b"5.", b"+2", b"\"\"", b"hide", b"yes", b"no", b"\"${REFERENCE}\"", b"\"~{A}\"", b"xyz", b"()", b"(a)", b"123456789012", b"270", b"90", b"180", b"-90", b"45.5", b"359.999"]
small = [(0, f"{D}/ecc83/ecc83-pp.kicad_pcb"), (1, f"{D}/ecc83/ecc83-pp.kicad_sch"), (1, f"{D}/complex_hierarchy/ampli_ht.kicad_sch"), (0, f"{D}/microwave/microwave.kicad_pcb"), (1, f"{D}/simulation/rectifier/rectifier.kicad_sch")]
srcs = [(k, open(f, "rb").read()) for k, f in small]
for i in range(fuzz):
    k, b = srcs[i % len(srcs)]
    ms = list(tok.finditer(b)); picks = sorted(rng.sample(range(len(ms)), 40))
    o = []; at = 0
    for j in picks: o += [b[at:ms[j].start()], rng.choice(swap)]; at = ms[j].end()
    o.append(b[at:])
    f = f"{P}/fuzz.{'pcb' if k == 0 else 'sch'}"; open(f, "wb").write(b"".join(o)); keep = f"{P}/fuzz-{i}"
    before = len(fails)
    plot("fuzz", str(i), k, f)
    if len(fails) > before: shutil.copy(f, keep)
json.dump({"results": res, "fails": fails}, open(out, "w"), indent=0)
for k, v in res.items(): print(k, f"{v[1]}/{v[0]} identical")
print("differences", len(fails))
for x in fails[:30]: print(json.dumps(x)[:300])
