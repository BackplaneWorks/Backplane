#!/usr/bin/env python3
"""The Android app's catalog: i18n/en.msg + i18n/ja.msg -> mobile/android/.../Msg.kt.

  python3 scripts/i18n-android.py           write Msg.kt
  python3 scripts/i18n-android.py --check   fail when Msg.kt is not what this writes (stale), or a message does not parse

Msg.kt holds the messages as written (the app's Tr.kt renders the ICU subset: {a}, {n, number}, plural, select, # and
apostrophe quoting), a typed accessor per key (a val for a plain message, a function for one with arguments, in English
first-appearance order: number/plural arguments are Number, select ones String, plain ones Any) and the reverse table Tr.t uses on English
display text from the hub: plain English -> key, and the templated keys worth matching, most specific first.
"""
import pathlib, re, sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
OUT = ROOT / "mobile/android/app/src/main/java/dev/backplane/mobile/Msg.kt"
ANDROID_SECTION = "# ---- android: new entries go here ----"


def load(p):
    """key -> (message with the file's escapes undone, line number, in the android section)."""
    d, android = {}, False
    for n, raw in enumerate(p.read_text(encoding="utf8").splitlines(), 1):
        l = raw.strip()
        if l.startswith("# ----"): android = l == ANDROID_SECTION
        if not l or l.startswith("#"): continue
        k, sep, v = l.partition(" = ")
        if not sep or not re.fullmatch(r"[a-z][a-z0-9]*(\.[a-z][a-z0-9]*)*", k): sys.exit(f"{p.name}:{n}: bad entry: {l[:60]}")
        if k in d: sys.exit(f"{p.name}:{n}: duplicate key {k}")
        out, i = [], 0
        while i < len(v):
            c = v[i]
            if c == "\\":
                e = v[i + 1:i + 2]
                if e == "s": out.append(" ")
                elif e == "n": out.append("\n")
                elif e == "\\": out.append("\\")
                else: sys.exit(f"{p.name}:{n}: unknown escape \\{e} in {k}")
                i += 2; continue
            out.append(c); i += 1
        d[k] = ("".join(out), n, android)
    return d


class Bad(Exception): pass


def parse(s, in_plural=False):
    """The ICU subset as nodes: str, ('arg', name), ('num', name), ('hash',), ('plural'|'select', name, [(case, nodes)])."""
    nodes, i, buf = [], 0, []
    def flush():
        if buf: nodes.append("".join(buf)); buf.clear()
    while i < len(s):
        c = s[i]
        if c == "'":
            nx = s[i + 1:i + 2]
            if nx == "'": buf.append("'"); i += 2; continue
            if nx and nx in "{}#|":
                j = i + 1
                while j < len(s):
                    if s[j] == "'":
                        if s[j + 1:j + 2] == "'": buf.append("'"); j += 2; continue
                        break
                    buf.append(s[j]); j += 1
                i = j + 1; continue
            buf.append("'"); i += 1; continue
        if c == "#" and in_plural: flush(); nodes.append(("hash",)); i += 1; continue
        if c == "}": raise Bad(f"stray }} at {i}")
        if c == "{":
            flush()
            j = close(s, i)
            inner = s[i + 1:j]
            parts = [x.strip() for x in inner.split(",", 2)]
            name = parts[0]
            if not re.fullmatch(r"[A-Za-z][A-Za-z0-9_]*", name): raise Bad(f"bad argument {inner!r}")
            if len(parts) == 1: nodes.append(("arg", name))
            elif len(parts) == 2 and parts[1] == "number": nodes.append(("num", name))
            elif len(parts) == 3 and parts[1] in ("plural", "select"):
                cases, body, k = [], parts[2], 0
                while k < len(body):
                    if body[k].isspace(): k += 1; continue
                    m = re.match(r"(=\d+|[a-z]+)\s*\{", body[k:])
                    if not m: raise Bad(f"bad case in {inner!r}")
                    start = k + m.end() - 1
                    e = close(body, start)
                    cases.append((m.group(1), parse(body[start + 1:e], in_plural or parts[1] == "plural")))
                    k = e + 1
                if not any(c == "other" for c, _ in cases): raise Bad(f"{parts[1]} without other: {inner!r}")
                nodes.append((parts[1], name, cases))
            else: raise Bad(f"bad placeholder {inner!r}")
            i = j + 1; continue
        buf.append(c); i += 1
    flush()
    return nodes


def close(s, i):
    d, j = 0, i
    while j < len(s):
        c = s[j]
        if c == "'" and s[j + 1:j + 2] and s[j + 1] in "{}#|'":
            if s[j + 1] == "'": j += 2; continue
            j += 1
            while j < len(s) and s[j] != "'": j += 1
            j += 1; continue
        if c == "{": d += 1
        elif c == "}":
            d -= 1
            if d == 0: return j
        j += 1
    raise Bad("unbalanced braces")


def args(nodes, out):
    """name -> kind ('n' number, 's' string, '' plain) in first-appearance order."""
    for n in nodes:
        if isinstance(n, str) or n[0] == "hash": continue
        name = n[1]
        kind = {"arg": "", "num": "n", "plural": "n", "select": "s"}[n[0]]
        prev = out.get(name)
        if prev is None or prev == "": out[name] = kind or (prev or "")
        elif kind and kind != prev: raise Bad(f"{name} is both a number and a select")
        if n[0] in ("plural", "select"):
            for _, sub in n[2]: args(sub, out)
    return out


def plain(nodes):
    return "".join(nodes) if all(isinstance(n, str) for n in nodes) else None


def literal_len(nodes):
    """Letters a template must show whatever its arguments: the least over each choice's branches."""
    t = 0
    for n in nodes:
        if isinstance(n, str): t += sum(ch.isalpha() for ch in n)
        elif n[0] in ("plural", "select"): t += min(literal_len(sub) for _, sub in n[2])
    return t


def kstr(s):
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"').replace("$", "\\$").replace("\n", "\\n").replace("\r", "\\r").replace("\t", "\\t") + '"'


KOTLIN_HARD = {"as", "break", "class", "continue", "do", "else", "false", "for", "fun", "if", "in", "interface", "is", "null", "object",
               "package", "return", "super", "this", "throw", "true", "try", "typealias", "typeof", "val", "var", "when", "while"}


def build():
    en, ja = load(ROOT / "i18n/en.msg"), load(ROOT / "i18n/ja.msg")
    bad = [f"missing ja: {k}" for k in en if k not in ja] + [f"extra ja: {k}" for k in ja if k not in en]
    keys = list(en)
    parsed_en, parsed_ja, kinds = {}, {}, {}
    for k in keys:
        try:
            parsed_en[k] = parse(en[k][0]); a = args(parsed_en[k], {})
            if k in ja:
                parsed_ja[k] = parse(ja[k][0]); b = args(parsed_ja[k], {})
                if set(a) != set(b): bad.append(f"args differ: {k}: {sorted(a)} vs {sorted(b)}")
                for name, kind in b.items():
                    if kind and a.get(name) and a[name] != kind: bad.append(f"{k}: {name} is a number in one language and a select in the other")
                    elif kind and not a.get(name): a[name] = kind
            kinds[k] = a
        except Bad as e:
            bad.append(f"{k}: {e}")
    if bad: print("\n".join(bad)); sys.exit(1)

    # plain English -> key: on a clash the android section's key wins, then the commonest Japanese, then the first
    by_text = {}
    for k in keys:
        p = plain(parsed_en[k])
        if p is not None and p.strip(): by_text.setdefault(p, []).append(k)
    exact = []
    for text, ks in by_text.items():
        own = [k for k in ks if en[k][2]]
        if own: pick = own[0]
        else:
            counts = {}
            for k in ks: counts[ja[k][0]] = counts.get(ja[k][0], 0) + 1
            best = max(counts.values()); pick = next(k for k in ks if counts[ja[k][0]] == best)
        exact.append((text, pick))
    # templates worth matching: at least four letters of their own, most specific first
    tmpl = [k for k in keys if kinds[k] and literal_len(parsed_en[k]) >= 4]
    # on a tie (the same English in two sections) the android section's key wins, as for plain text
    tmpl.sort(key=lambda k: (-literal_len(parsed_en[k]), 0 if en[k][2] else 1, keys.index(k)))

    idx = {k: i for i, k in enumerate(keys)}
    o = ["// Generated by scripts/i18n-android.py from i18n/en.msg and i18n/ja.msg. Do not edit: change the catalogs, then run it.",
         "package dev.backplane.mobile", "",
         "@Suppress(\"FunctionName\", \"ObjectPropertyName\", \"unused\", \"SpellCheckingInspection\")",
         "object Msg {"]
    CH = 250
    def table(name, vals):
        parts = []
        for c in range(0, len(vals), CH):
            fn = f"{name}{c // CH}"; parts.append(fn + "()")
            o.append(f"  private fun {fn}(): Array<String> = arrayOf(")
            o.extend(f"    {kstr(v)}," for v in vals[c:c + CH])
            o.append("  )")
        o.append(f"  internal val {name}: Array<String> by lazy {{ arrayOf(*{', *'.join(parts)}) }}")
    table("keys", keys)
    table("en", [en[k][0] for k in keys])
    table("ja", [ja[k][0] for k in keys])
    table("exactText", [t for t, _ in exact])
    o.append("  internal val exactKey: IntArray by lazy { intArrayOf(" + ", ".join(str(idx[k]) for _, k in exact) + ") }")
    o.append("  internal val templates: IntArray by lazy { intArrayOf(" + ", ".join(str(idx[k]) for k in tmpl) + ") }")
    o.append("  internal val argNames: Map<Int, Array<String>> by lazy { mapOf(" + ", ".join(
        f"{idx[k]} to arrayOf({', '.join(kstr(n) for n in kinds[k])})" for k in keys if kinds[k]) + ") }")
    o.append("")
    for k in keys:
        name = k.replace(".", "_")
        if kinds[k]:
            ps = []
            for a, kind in kinds[k].items():
                pn = f"`{a}`" if a in KOTLIN_HARD else a
                ps.append(f"{pn}: {'Number' if kind == 'n' else 'String' if kind == 's' else 'Any'}")
            vals = ", ".join((f"`{a}`" if a in KOTLIN_HARD else a) for a in kinds[k])
            o.append(f"  fun {name}({', '.join(ps)}): String = Tr.f({idx[k]}, arrayOf<Any>({vals}))")
        else:
            o.append(f"  val {name}: String get() = Tr.s({idx[k]})")
    o.append("}")
    return "\n".join(o) + "\n", len(keys), len(exact), len(tmpl)


def main():
    text, n, ne, nt = build()
    if "--check" in sys.argv[1:]:
        if not OUT.exists() or OUT.read_text(encoding="utf8") != text:
            print(f"stale: {OUT.relative_to(ROOT)}; run python3 scripts/i18n-android.py"); sys.exit(1)
        print(f"ok: {OUT.relative_to(ROOT)} is current ({n} keys, {ne} exact, {nt} templates)")
        return
    OUT.write_text(text, encoding="utf8")
    print(f"wrote {OUT.relative_to(ROOT)}: {n} keys, {ne} exact English texts, {nt} templates")


if __name__ == "__main__":
    main()
