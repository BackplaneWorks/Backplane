#!/usr/bin/env python3
"""Law table for the Rust port (docs/gpui/PORT.md, "Laws").

Reads LAWS.bend (never edits it or PROOF.bend) and scripts/gpui/law-map.tsv, the small data file:

    ported <src path>              # a Bend source file whose logic has moved (or is wrapped) in Rust
    law <name> bend                # the logic stays in Bend; Rust calls it; bend PROOF.bend still checks it
    law <name> rust <test name>    # a Rust test (fn <name> under native/) states the same claim
    (a law with no line is `unported`; its Bend code still stands)

Statuses: bend, rust <test>, unported, retired <reason>, client (client-side Bend in the web/phone bundles, unchanged), window
(native-window code the GPUI client replaces; the table says what GPUI does instead, or that it has no equivalent yet).
Seed: docs/gpui/PORT-CLASSIFICATION.md's file table (K -> bend, R -> unported, C -> client, D -> window; files it does
not list default by location). That classification is +-10% until each law is checked; the table's `seed` column says so.
retired: the law guards only the web client (decision: native apps only). Seeded when every file the law names is a src/web/*.bend file
outside the import closure of src/mobile/app.bend (the phone bundle); a law that also names anything native stays `client`. Override
with `law <name> client` or `law <name> retired <reason>`.
A law is "about" the source file whose LAWS.bend import alias its statement names most. Fails (exit 1) when
  - a law about a `ported` file resolves (data file, else seed) to neither `bend` nor `rust`,
  - a `rust` line names a test that is not a fn in native/**/*.rs (vendor/ and target/ skipped),
  - a line names a law that does not exist, or a `ported` path that does not exist.
usage: law-map.py [--check]     (default rewrites the table after the marker in docs/gpui/PORT.md; --check only verifies)
"""
import os, re, sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
LAWS = os.path.join(ROOT, "LAWS.bend")
MAP = os.path.join(ROOT, "scripts/gpui/law-map.tsv")
DOC = os.path.join(ROOT, "docs/gpui/PORT.md")
CLS = os.path.join(ROOT, "docs/gpui/PORT-CLASSIFICATION.md")
MARK = "<!-- law-map: generated below by scripts/gpui/law-map.py -->"

def parse_laws():
    t = open(LAWS).read()
    alias = {}
    for m in re.finditer(r"^import\s+(\S+)\s+as\s+(\w+)\s*$", t, re.M):
        alias[m.group(2)] = os.path.normpath(os.path.join("", m.group(1)))
    laws = []  # (name, set of source files)
    parts = re.split(r"(?m)^(?=law \S+:)", t)
    for p in parts:
        m = re.match(r"law (\S+?):", p)
        if not m:
            continue
        cnt = {}
        for a in re.findall(r"\b([A-Z][A-Z0-9]*)\.[A-Za-z_]", p):
            if a in alias:
                cnt[alias[a]] = cnt.get(alias[a], 0) + 1
        top = sorted(cnt.items(), key=lambda kv: (-kv[1], kv[0]))
        laws.append((m.group(1), {top[0][0]} if top else set(), set(cnt)))
    return laws

def _imports(f):
    try:
        t = open(os.path.join(ROOT, f)).read()
    except OSError:
        return []
    return [os.path.normpath(os.path.join(os.path.dirname(f), m)) for m in re.findall(r"^import\s+(\S+\.bend)\s+as", t, re.M)]

def web_only():
    seen, st = set(), ["src/mobile/app.bend"]
    while st:
        f = st.pop()
        if f in seen:
            continue
        seen.add(f)
        st += _imports(f)
    return {"src/web/" + f for f in os.listdir(os.path.join(ROOT, "src/web")) if f.endswith(".bend")} - seen

WINDOW_NOTE = {
    "src/core/cef.bend": "no equivalent yet (GPUI has no embedded Chromium; the browser tab is not ported)",
    "src/app/nui.bend": "GPUI input handling, native/gpui/src/input.rs (equivalence unchecked)",
    "src/app/layout.bend": "GPUI layout, native/gpui/src/main.rs (equivalence unchecked)",
}
def classes():
    """file -> K/R/C/D from the classification's table; location defaults for the rest."""
    c = {}
    for m in re.finditer(r"^\| `([^`]+)` \| *\d+ *\| *([KRCD]) ", open(CLS).read(), re.M):
        c[m.group(1)] = m.group(2)
    return c
def klass(path, table):
    if path in table:
        return table[path]
    for pre, k in (("src/core/", "K"), ("src/mobile/", "C"), ("src/web/", "C"), ("src/app/", "D"), ("src/gfx/", "D"), ("src/server/", "R")):
        if path.startswith(pre):
            return k
    return "?"
SEED = {"K": "bend", "R": "unported", "C": "client", "D": "window", "?": "unported"}

def rust_tests():
    names = set()
    for d, dirs, files in os.walk(os.path.join(ROOT, "native")):
        dirs[:] = [x for x in dirs if x not in ("vendor", "target")]
        for f in files:
            if f.endswith(".rs"):
                names |= set(re.findall(r"\bfn\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(", open(os.path.join(d, f), errors="replace").read()))
    return names

def parse_map():
    ported, status, errs = [], {}, []
    if not os.path.exists(MAP):
        return ported, status, errs
    for n, line in enumerate(open(MAP), 1):
        line = line.split("#", 1)[0].strip()
        if not line:
            continue
        w = line.split()
        if w[0] == "ported" and len(w) == 2:
            ported.append(os.path.normpath(w[1]))
        elif w[0] == "law" and len(w) == 3 and w[2] == "bend":
            status[w[1]] = ("bend", "")
        elif w[0] == "law" and len(w) == 3 and w[2] in ("client", "unported", "window"):
            status[w[1]] = (w[2], "")
        elif w[0] == "law" and len(w) >= 4 and w[2] == "retired":
            status[w[1]] = ("retired", " ".join(w[3:]))
        elif w[0] == "law" and len(w) == 4 and w[2] == "rust":
            status[w[1]] = ("rust", w[3])
        else:
            errs.append("law-map.tsv:%d: cannot read %r" % (n, line))
    return ported, status, errs

def main():
    check = "--check" in sys.argv
    laws = parse_laws()
    ported, status, errs = parse_map()
    names = {n for n, _, _ in laws}
    tests = rust_tests()
    for p in ported:
        if not os.path.exists(os.path.join(ROOT, p)):
            errs.append("ported path does not exist: " + p)
    for n, (k, t) in status.items():
        if n not in names:
            errs.append("unknown law: " + n)
        if k == "rust" and t not in tests:
            errs.append("law %s names Rust test %s, which does not exist under native/" % (n, t))
    pset = set(ported)
    table = classes()
    count = {"bend": 0, "rust": 0, "unported": 0, "client": 0, "window": 0, "retired": 0}
    rows = []
    wo = web_only()
    retired = []
    for n, srcs, allf in laws:
        src = next(iter(srcs)) if srcs else ""
        cls = klass(src, table) if src else "?"
        if n in status:
            k, t = status[n]; seed = "map"
        else:
            k, t = SEED[cls], ""; seed = "seed %s +-10%%" % cls if src else "none"
            if allf and allf <= wo:
                k, t, seed = "retired", "web client only: " + ", ".join(sorted(allf)) + " are outside the phone bundle's imports", "seed web-only"
        if srcs & pset and k not in ("bend", "rust"):
            errs.append("law %s is about ported code (%s) and is %s, not bend or rust" % (n, src, k))
        count[k] += 1
        if k == "retired":
            retired.append((n, t))
        note = ""
        if k == "window":
            note = WINDOW_NOTE.get(src, "GPUI replaces this window code (native/gpui); equivalence unchecked, no law-level mapping yet")
        rows.append("| `%s` | %s | %s | `%s` | %s |" % (n, (k + " `" + t + "`") if k == "rust" else ("retired" if k == "retired" else k), seed, src or "-", note))
    for e in errs:
        print("law-map: " + e, file=sys.stderr)
    body = ("\n\nLaws in LAWS.bend: %d. bend %d, rust %d, unported %d, client %d, window %d, retired %d. Data file: `scripts/gpui/law-map.tsv`; "
        "ported sources: %s. `seed` is the Bend stream's file-level classification (`docs/gpui/PORT-CLASSIFICATION.md`), accurate to about 10%% until each law is checked; "
        "`file` is the source file the law names most.\n\n| law | status | seed | file | note |\n|---|---|---|---|---|\n%s\n") % (
        len(laws), count["bend"], count["rust"], count["unported"], count["client"], count["window"], count["retired"], ", ".join("`%s`" % p for p in ported) or "none yet", "\n".join(rows))
    cands = [(n, sorted(allf & wo)) for n, _, allf in laws if allf & wo and n not in {r[0] for r in retired}]
    body += ("\n### Retired laws (web client only)\n\nDecision: native apps only, no web page. A law is retired only when every file it names is web-only "
        "(a src/web/*.bend file outside the phone bundle's imports); a law that also names a native-shared file is never retired, and when unsure it stays `client`.\n\n"
        + ("\n".join("- `%s`: %s" % (n, t) for n, t in retired) if retired else "None seeded.") +
        "\n\nLaws that name a web-only file but also native-shared code, so kept (review by hand):\n\n" + "\n".join("- `%s` names %s" % (n, ", ".join("`%s`" % f for f in fs)) for n, fs in cands) + "\n")
    doc = open(DOC).read()
    head = doc.split(MARK)[0] + MARK
    if check:
        if doc != head + body:
            errs.append("docs/gpui/PORT.md law table is out of date: run scripts/gpui/law-map.py")
            print("law-map: " + errs[-1], file=sys.stderr)
    else:
        open(DOC, "w").write(head + body)
    print("laws %d: bend %d rust %d unported %d client %d window %d retired %d%s" % (len(laws), count["bend"], count["rust"], count["unported"], count["client"], count["window"], count["retired"], "" if not errs else "  FAIL (%d)" % len(errs)))
    sys.exit(1 if errs else 0)

main()
