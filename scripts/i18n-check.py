#!/usr/bin/env python3
"""Catalog gate: i18n/en.msg owns the keys; ja.msg must have every one, with the same argument names. Run from anywhere."""
import re, sys, pathlib
root = pathlib.Path(__file__).resolve().parent.parent / "i18n"
def load(p):
    d = {}
    for n, l in enumerate(p.read_text(encoding="utf8").splitlines(), 1):
        l = l.strip()
        if not l or l.startswith("#"): continue
        k, sep, v = l.partition(" = ")
        if not sep or not re.fullmatch(r"[a-z][a-z0-9]*(\.[a-z][a-z0-9]*)*", k): sys.exit(f"{p.name}:{n}: bad entry: {l[:60]}")
        if k in d: sys.exit(f"{p.name}:{n}: duplicate key {k}")
        d[k] = v
    return d
def block(s, i):
    """s[i] == '{': index of its matching '}'."""
    d = 0
    for j in range(i, len(s)):
        if s[j] == "{": d += 1
        elif s[j] == "}":
            d -= 1
            if d == 0: return j
    sys.exit(f"unbalanced braces: {s[:60]}")
def args(v):
    out, i = set(), 0
    while i < len(v):
        if v[i] == "{":
            j = block(v, i); inner = v[i + 1:j]; parts = [x.strip() for x in inner.split(",", 2)]
            out.add(parts[0])
            if len(parts) == 3 and parts[1] in ("plural", "select"):
                k = 0
                while k < len(parts[2]):
                    if parts[2][k] == "{": e = block(parts[2], k); out |= args(parts[2][k + 1:e]); k = e
                    k += 1
            i = j
        i += 1
    return out
en, ja = load(root / "en.msg"), load(root / "ja.msg")
bad = [f"missing ja: {k}" for k in en if k not in ja] + [f"extra ja (no en): {k}" for k in ja if k not in en]
bad += [f"args differ: {k}: {sorted(args(en[k]))} vs {sorted(args(ja[k]))}" for k in en if k in ja and args(en[k]) != args(ja[k])]
if bad: print("\n".join(bad)); sys.exit(1)
print(f"ok: {len(en)} keys, every one in ja")
