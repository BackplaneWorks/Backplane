#!/usr/bin/env python3
"""The iOS app's words go through the catalog (i18n/en.msg, i18n/ja.msg; mobile/ios/Shared/Tr.swift). Fails when:
  - a key the Swift names (Tr.k("key"), a generated Tr.key_name(...)) is not in en.msg and ja.msg;
  - a literal given to Tr.t(...) is not an English catalog message (or is built with \\(...): use the key's generated function);
  - a string literal with words reaches the screen without the catalog: the first argument of Text( Label( Button( TextField( SecureField(
    Toggle( Menu( Section( Picker( DisclosureGroup( LabeledContent( Link( ProgressView( .navigationTitle( .accessibilityLabel(
    .accessibilityHint( .accessibilityValue( .alert( .confirmationDialog( .help( .dataBar( DHeading( DLoading( Text(verbatim:, a
    placeholder: or label: argument, client.say(...) or an assignment to a note the screen shows (notice, error, err, sent, linkNote,
    pairingError, failure);
  - TrCatalog.swift is stale (scripts/i18n-ios.py --check).
Technical literals (a product name, a code, a format) are listed in mobile/ios/i18n-allow.txt, each with a reason.
Usage: python3 scripts/check-ios-i18n.py   (scripts/check-ios-i18n.sh runs it with the generator's check)
"""
import importlib.util, pathlib, re, sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
IOS = ROOT / "mobile/ios"
spec = importlib.util.spec_from_file_location("gen", ROOT / "scripts/i18n-ios.py")
gen = importlib.util.module_from_spec(spec); spec.loader.exec_module(gen)

SINK_CALLS = {"Text", "Label", "Button", "TextField", "SecureField", "Toggle", "Menu", "Section", "Picker", "DisclosureGroup",
              "LabeledContent", "Link", "ShareLink", "DatePicker", "ProgressView", "DHeading", "DLoading", "say"}
SINK_MODS = {"navigationTitle", "accessibilityLabel", "accessibilityHint", "accessibilityValue", "alert", "confirmationDialog", "help", "dataBar", "say"}
SINK_LABELS = {"placeholder", "label", "verbatim", "prompt", "title", "empty", "hint"}
UIKIT_SET = {"accessibilityLabel", "accessibilityHint", "accessibilityValue", "text", "placeholder"}
NOTES = {"notice", "error", "err", "sent", "linkNote", "pairingError", "failure", "status"}


def tokens(src):
    """(kind, text, line, value, interp): kind str | id | op. value: a literal's text outside its \\( ) holes."""
    out, i, line, n = [], 0, 1, len(src)
    while i < n:
        c = src[i]
        if c == "\n": line += 1; i += 1; continue
        if c.isspace(): i += 1; continue
        if src.startswith("//", i):
            j = src.find("\n", i); i = n if j < 0 else j; continue
        if src.startswith("/*", i):
            j = src.find("*/", i + 2); j = n if j < 0 else j + 2; line += src.count("\n", i, j); i = j; continue
        if c == "#" and re.match(r'#+"', src[i:]):
            hashes = re.match(r"#+", src[i:]).group(0); q = '"""' if src.startswith('"""', i + len(hashes)) else '"'
            start = i + len(hashes) + len(q); end = src.find(q + hashes, start)
            end = n if end < 0 else end
            val = src[start:end]; out.append(("str", val, line, val, False)); line += val.count("\n"); i = end + len(q) + len(hashes); continue
        if c == '"':
            q = '"""' if src.startswith('"""', i) else '"'
            j, depth, val, interp, start_line = i + len(q), 0, [], False, line
            while j < n:
                if src[j] == "\\" and j + 1 < n:
                    if src[j + 1] == "(":
                        interp = True; k, d = j + 2, 1
                        while k < n and d:
                            if src[k] == "(": d += 1
                            elif src[k] == ")": d -= 1
                            elif src[k] == '"':  # a literal inside the hole
                                k += 1
                                while k < n and src[k] != '"':
                                    k += 2 if src[k] == "\\" else 1
                            k += 1
                        j = k; val.append("\x00"); continue
                    val.append({"n": "\n", "t": "\t", '"': '"', "\\": "\\"}.get(src[j + 1], src[j + 1])); j += 2; continue
                if src.startswith(q, j): break
                if src[j] == "\n": line += 1
                val.append(src[j]); j += 1
            out.append(("str", src[i:j + len(q)], start_line, "".join(val), interp)); i = j + len(q); continue
        m = re.match(r"[A-Za-z_][A-Za-z0-9_]*", src[i:])
        if m: out.append(("id", m.group(0), line, None, False)); i += m.end(); continue
        out.append(("op", c, line, None, False)); i += 1
    return out


def span(toks, k):
    """toks[k] is '(' : the index of its first argument's end (a top-level ',' or the closing ')')."""
    d = 0
    for j in range(k, len(toks)):
        t = toks[j]
        if t[0] == "op" and t[1] in "([{": d += 1
        elif t[0] == "op" and t[1] in ")]}":
            d -= 1
            if d == 0: return j
        elif t[0] == "op" and t[1] == "," and d == 1: return j
    return len(toks)


def expr_end(toks, k):
    """the end of an expression starting at toks[k] (a top-level ',' or an unmatched closer, or a line change at depth 0)."""
    d, line = 0, toks[k][2] if k < len(toks) else 0
    for j in range(k, len(toks)):
        t = toks[j]
        if d == 0 and t[2] != line and t[0] != "op": return j
        if t[0] == "op" and t[1] in "([{": d += 1
        elif t[0] == "op" and t[1] in ")]}":
            if d == 0: return j
            d -= 1
        elif t[0] == "op" and t[1] in ",;" and d == 0: return j
    return len(toks)


def tr_ranges(toks):
    """[(open, close, name)] of every Tr.name( ... ) call."""
    out = []
    for k in range(len(toks) - 3):
        if toks[k][:2] == ("id", "Tr") and toks[k + 1][:2] == ("op", ".") and toks[k + 2][0] == "id" and toks[k + 3][:2] == ("op", "("):
            d = 0
            for j in range(k + 3, len(toks)):
                if toks[j][0] == "op" and toks[j][1] in "([{": d += 1
                elif toks[j][0] == "op" and toks[j][1] in ")]}":
                    d -= 1
                    if d == 0: break
            out.append((k + 3, j, toks[k + 2][1]))
    return out


TECH_CALLS = {"accessibilityIdentifier", "Image", "systemImage", "UIImage", "font", "custom", "DispatchQueue", "Logger", "OSLog", "NSLog", "print"}
TECH_LABELS = {"systemName", "systemImage", "forKey", "named", "id", "identifier"}


def tech_ranges(toks):
    """[(open, close)] of calls whose literals are names, never words: an accessibility identifier, an SF Symbol, a font."""
    out = []
    for k in range(len(toks) - 1):
        if toks[k][0] == "id" and toks[k][1] in TECH_CALLS and toks[k + 1][:2] == ("op", "("):
            d = 0
            for j in range(k + 1, len(toks)):
                if toks[j][0] == "op" and toks[j][1] in "([{": d += 1
                elif toks[j][0] == "op" and toks[j][1] in ")]}":
                    d -= 1
                    if d == 0: break
            out.append((k + 1, j))
    return out


def compared(toks, j):
    """a literal a value is compared with, a dictionary key or a case: not words on screen."""
    p1 = toks[j - 1] if j else ("op", "")
    p2 = toks[j - 2] if j > 1 else ("op", "")
    n1 = toks[j + 1] if j + 1 < len(toks) else ("op", "")
    return p1[1] == "[" or n1[1] == "]" or (p1[1] == "=" and p2[1] in "=!") or p1[:2] == ("id", "case") or (p1[1] == "," and n1[1] == ":")


def words(v):
    return re.search(r"[A-Za-z]{2,}", v.replace("\x00", " ")) is not None


def main():
    en, ja = gen.load(ROOT / "i18n/en.msg"), gen.load(ROOT / "i18n/ja.msg")
    jak = {k for k, _, _ in ja}
    plain, funcs = set(), {}
    for k, v, _ in en:
        nodes = gen.message(v)
        if all(n[0] == "lit" for n in nodes): plain.add("".join(n[1] for n in nodes))
        elif gen.args(nodes, {}): funcs[gen.swift_name(k)] = k
        # a message whose choices leave no hole ("No files changed") is matched too
        if any(n[0] == "choice" for n in nodes):
            plain |= set(variants_plain(nodes))
    enk = {k for k, _, _ in en}
    allow = []
    for n, l in enumerate((IOS / "i18n-allow.txt").read_text(encoding="utf8").splitlines(), 1):
        if not l.strip() or l.startswith("#"): continue
        p = [x.strip() for x in l.split(" | ")]
        if len(p) != 3 or not p[2]: sys.exit(f"i18n-allow.txt:{n}: want `File.swift | literal | reason`")
        allow.append((p[0], p[1], n))
    used_allow, bad = set(), []
    files = sorted(list((IOS / "Backplane").rglob("*.swift")) + list((IOS / "Island").rglob("*.swift")) + list((IOS / "Shared").rglob("*.swift")))
    for f in files:
        if f.name in ("Tr.swift", "TrCatalog.swift"): continue
        toks = tokens(f.read_text(encoding="utf8"))
        rel = f.relative_to(IOS)
        trs = tr_ranges(toks)
        tech = tech_ranges(toks)
        def in_tr(j):
            return any(a < j < b for a, b, _ in trs) or any(a < j < b for a, b in tech) or (j >= 2 and toks[j - 1][:2] == ("op", ":") and toks[j - 2][0] == "id" and toks[j - 2][1] in TECH_LABELS)
        def allowed(v):
            for a in allow:
                if a[0] == f.name and a[1] in v.replace("\x00", "…"):
                    used_allow.add(a[2]); return True
            return False
        def flag(t, why):
            if not allowed(t[3]): bad.append(f"{rel}:{t[2]}: {why}: {t[1][:80]}")
        # keys and functions
        for a, b, name in trs:
            if name == "k":
                t = toks[a + 1]
                if t[0] == "str" and t[3] not in enk: bad.append(f"{rel}:{t[2]}: no key {t[3]!r} in en.msg")
                elif t[0] == "str" and t[3] not in jak: bad.append(f"{rel}:{t[2]}: no key {t[3]!r} in ja.msg")
            elif name == "t":
                for j in range(a + 1, b):
                    t = toks[j]
                    if t[0] != "str" or compared(toks, j) or any(a2 < j < b2 for a2, b2, n2 in trs if (a2, b2) != (a, b) and a < a2): continue
                    if t[4]: flag(t, "Tr.t of a built string (use the key's generated function)")
                    elif words(t[3]) and t[3] not in plain: flag(t, "not an English catalog message")
            elif name not in ("f", "lang", "resolve", "one", "match", "number", "render", "translate", "table", "cache", "category"):
                if name not in funcs: bad.append(f"{rel}:{toks[a][2]}: Tr.{name}: no such key with arguments in en.msg")
                elif funcs[name] not in jak: bad.append(f"{rel}:{toks[a][2]}: Tr.{name}: no ja entry")
        if "NativeMessages." in f.read_text(encoding="utf8"): bad.append(f"{rel}: NativeMessages is gone: use Tr")
        # what reaches the screen
        for k, t in enumerate(toks):
            lo = hi = None
            if t[0] == "id" and k + 1 < len(toks) and toks[k + 1][:2] == ("op", "("):
                prev = toks[k - 1] if k else ("op", "")
                if (t[1] in SINK_CALLS and prev[:2] != ("op", ".")) or (t[1] in SINK_MODS and prev[:2] == ("op", ".")):
                    lo, hi = k + 1, span(toks, k + 1)
            elif t[0] == "id" and t[1] in SINK_LABELS and k + 2 < len(toks) and toks[k + 1][:2] == ("op", ":") and toks[k + 2][:2] != ("op", "{"):
                lo, hi = k + 1, expr_end(toks, k + 2)
            elif t[0] == "id" and t[1] in NOTES and k + 1 < len(toks) and toks[k + 1][:2] == ("op", "=") and not (k + 2 < len(toks) and toks[k + 2][:2] == ("op", "=")):
                lo, hi = k + 1, expr_end(toks, k + 2)
                for j in range(lo, hi):
                    s = toks[j]
                    if s[0] == "str" and words(s[3]) and not in_tr(j) and not compared(toks, j) and (j == lo + 1 or toks[j - 1][1] in "?:"):
                        if s[4]: flag(s, "a built note (use the key's generated function)")
                        elif s[3] not in plain: flag(s, "a note that is not an English catalog message")
                continue
            elif t[0] == "id" and t[1] in UIKIT_SET and k + 2 < len(toks) and toks[k + 1][:2] == ("op", "=") and toks[k + 2][:2] != ("op", "=") and toks[k - 1][1] not in (",", "let", "var"):
                lo, hi = k + 1, expr_end(toks, k + 2)
            if lo is None: continue
            labelled = t[1] in SINK_LABELS
            for j in range(lo, hi):
                s = toks[j]
                if s[0] != "str" or not words(s[3]) or in_tr(j) or compared(toks, j): continue
                # a labelled argument's own value: the literal itself, or a branch of ?: or ??, never the arguments of a call in it
                if labelled and not (j == lo + 1 or toks[j - 1][1] in "?:" or toks[j - 2:j] == [("op", "?", toks[j - 2][2], None, False), ("op", "?", toks[j - 1][2], None, False)]): continue
                if True:
                    if t[1] == "say" and not s[4] and s[3] in plain: continue
                    flag(s, f"words on screen without the catalog ({t[1]})")
    unused = [a for a in allow if a[2] not in used_allow]
    for a in unused: bad.append(f"i18n-allow.txt:{a[2]}: unused entry {a[0]} | {a[1]}")
    if bad:
        print("\n".join(sorted(set(bad)))); print(f"{len(set(bad))} problems"); sys.exit(1)
    print(f"ok: iOS words go through the catalog ({len(files)} files)")


def variants_plain(nodes):
    """the English texts of a message's choices that leave no hole."""
    outs = [""]
    for n in nodes:
        if n[0] == "lit": outs = [o + n[1] for o in outs]
        elif n[0] in ("arg", "hash"): return []
        else:
            nxt = []
            for sub in n[3].values():
                for s in variants_plain(sub) if all(x[0] != "hash" for x in sub) else []:
                    nxt += [o + s for o in outs]
            outs = nxt
    return outs


if __name__ == "__main__":
    main()
