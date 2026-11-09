#!/usr/bin/env python3
"""Add catalog entries from stdin: lines `key<TAB>English<TAB>日本語` (key `-` makes one from the English: `<prefix>.<words>`).
    scripts/i18n-add.py <section> [prefix] < entries.tsv
They go at the end of that section of i18n/en.msg and i18n/ja.msg (the `# ---- <section>: ... ----` line to the next `# ----`), unless the key or the
exact English is already in the catalog (then nothing is added; the existing key is printed). Text is written as is: \\s \\n \\\\ escapes follow docs/i18n/README.md.
Then run scripts/i18n-check.py."""
import re, sys, pathlib, hashlib
root = pathlib.Path(__file__).resolve().parent.parent / "i18n"
section = sys.argv[1]; prefix = sys.argv[2] if len(sys.argv) > 2 else section
def entries(p):
    d = {}
    for l in p.read_text(encoding="utf8").splitlines():
        l = l.strip()
        if l and not l.startswith("#"): k, _, v = l.partition(" = "); d[k] = v
    return d
en_p, ja_p = root / "en.msg", root / "ja.msg"
en, ja = entries(en_p), entries(ja_p)
by_text = {}
for k, v in en.items(): by_text.setdefault(v, k)
def esc(s):
    s = s.replace("\\", "\\\\").replace("\n", "\\n")
    if s.startswith(" "): s = "\\s" + s[1:]
    if s.endswith(" "): s = s[:-1] + "\\s"
    return s
def mk(text):
    words = [w for w in re.findall(r"[a-z0-9]+", re.sub(r"\{[^}]*\}", " ", text.lower())) if w[0].isalpha()][:5] or ["text"]
    base = f"{prefix}." + ".".join(words); k = base; n = 2
    while k in en: k = f"{base}{n}"; n += 1
    return k
add_en, add_ja = [], []
for line in sys.stdin.read().splitlines():
    if not line.strip() or line.startswith("#"): continue
    parts = line.split("\t")
    if len(parts) != 3: sys.exit(f"need key<TAB>en<TAB>ja: {line[:70]}")
    k, e, j = parts[0].strip(), esc(parts[1].strip()), esc(parts[2].strip())
    if e in by_text and k in ("-", by_text[e]): print("have", by_text[e], "=", e[:50]); continue
    if k == "-": k = mk(e)
    if k in en: print("key exists, skipped:", k); continue
    en[k] = e; by_text[e] = k; add_en.append(f"{k} = {e}"); add_ja.append(f"{k} = {j}")
def put(p, lines):
    if not lines: return
    t = p.read_text(encoding="utf8").split("\n")
    i = next((n for n, l in enumerate(t) if l.startswith(f"# ---- {section}:")), None)
    if i is None: sys.exit(f"no section {section} in {p.name}")
    j = next(n for n in range(i + 1, len(t)) if t[n].startswith("# ----"))
    k = j
    while k > i + 1 and not t[k - 1].strip(): k -= 1
    t[k:k] = lines
    p.write_text("\n".join(t), encoding="utf8")
put(en_p, add_en); put(ja_p, add_ja)
print(f"added {len(add_en)} to {section}")
