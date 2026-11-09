#!/usr/bin/env python3
"""The iOS app's messages from i18n/en.msg and i18n/ja.msg (docs/i18n/README.md).

Writes mobile/ios/Shared/TrCatalog.swift (the catalog table and a typed function for every key with arguments; the runtime that reads it is
mobile/ios/Shared/Tr.swift) and mobile/ios/Backplane/{en,ja}.lproj/InfoPlist.strings (the OS permission purposes).
  python3 scripts/i18n-ios.py           regenerate
  python3 scripts/i18n-ios.py --check   exit 1 when a generated file is stale
"""
import pathlib, re, sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
CAT = ROOT / "i18n"
OUT = ROOT / "mobile/ios/Shared/TrCatalog.swift"
PLIST = {"NSCameraUsageDescription": "native.permission.camera", "NSMicrophoneUsageDescription": "native.permission.microphone"}
IOS_SECTION = "# ---- ios: new entries go here ----"
KEYWORDS = set("""associatedtype class deinit enum extension fileprivate func import init inout internal let open operator private precedencegroup protocol public
rethrows static struct subscript typealias var break case catch continue default defer do else fallthrough for guard if in repeat return throw switch where while
Any as await false is nil self Self super throws true try""".split())


def load(path):
    """[(key, raw message, in the ios section)] in file order."""
    out, ios, seen = [], False, set()
    for n, line in enumerate(path.read_text(encoding="utf8").splitlines(), 1):
        s = line.strip()
        if s.startswith("# ----"):
            ios = s == IOS_SECTION
            continue
        if not s or s.startswith("#"):
            continue
        k, sep, v = s.partition(" = ")
        if not sep or not re.fullmatch(r"[a-z][a-z0-9]*(\.[a-z][a-z0-9]*)*", k):
            sys.exit(f"{path.name}:{n}: bad entry: {s[:60]}")
        if k in seen:
            sys.exit(f"{path.name}:{n}: duplicate key {k}")
        seen.add(k)
        for bad in ("\t", '"""#'):
            if bad in v:
                sys.exit(f"{path.name}:{n}: {bad!r} cannot be carried in a message")
        if re.search(r"\\[^sn\\]", v.replace("\\\\", "")):
            sys.exit(f"{path.name}:{n}: unknown escape in {k}")
        out.append((k, v, ios))
    return out


def unescape(v):
    return re.sub(r"\\(.)", lambda m: {"s": " ", "n": "\n", "\\": "\\"}[m.group(1)], v)


# A small ICU parser, the same subset as Tr.swift: it finds every argument and its kind, and refuses what the runtime would read wrongly.
def parse(s, i=0, depth=0, plural=False):
    """(nodes, index): nodes are ("lit", text) | ("arg", name, kind) | ("hash",) | ("choice", name, kind, {branch: nodes})."""
    nodes, buf = [], []
    while i < len(s):
        c = s[i]
        if c == "'":
            if i + 1 < len(s) and s[i + 1] == "'":
                buf.append("'"); i += 2; continue
            if i + 1 < len(s) and s[i + 1] in "{}#|":
                j = i + 1
                while j < len(s):
                    if s[j] == "'":
                        if j + 1 < len(s) and s[j + 1] == "'":
                            buf.append("'"); j += 2; continue
                        break
                    buf.append(s[j]); j += 1
                i = j + 1; continue
            buf.append(c); i += 1; continue
        if c == "}":
            if depth == 0:
                raise ValueError("unbalanced }")
            break
        if c == "#" and plural:
            if buf: nodes.append(("lit", "".join(buf))); buf = []
            nodes.append(("hash",)); i += 1; continue
        if c == "{":
            if buf: nodes.append(("lit", "".join(buf))); buf = []
            j = i + 1
            m = re.match(r"\s*([A-Za-z_][A-Za-z0-9_]*)\s*(?:,\s*(number|plural|select)\s*)?(,|\})", s[j:])
            if not m:
                raise ValueError(f"bad argument at {s[i:i+30]!r}")
            name, kind, end = m.group(1), m.group(2), m.group(3)
            j += m.end()
            if end == "}" or kind == "number":
                if end == ",":
                    raise ValueError("number takes no style")
                nodes.append(("arg", name, "num" if kind == "number" else "text"))
                i = j; continue
            if kind not in ("plural", "select"):
                raise ValueError(f"unknown argument type in {s[i:i+30]!r}")
            branches = {}
            while True:
                m = re.match(r"\s*(=\d+|[A-Za-z_][A-Za-z0-9_]*)\s*\{", s[j:])
                if not m:
                    m = re.match(r"\s*\}", s[j:])
                    if not m:
                        raise ValueError(f"bad branch at {s[j:j+30]!r}")
                    j += m.end(); break
                b = m.group(1); j += m.end()
                sub, j = parse(s, j, depth + 1, plural or kind == "plural")
                if j >= len(s) or s[j] != "}":
                    raise ValueError("unclosed branch")
                j += 1
                branches[b] = sub
            if "other" not in branches:
                raise ValueError(f"{name}: a choice needs other")
            nodes.append(("choice", name, "num" if kind == "plural" else "sel", branches))
            i = j; continue
        buf.append(c); i += 1
    if buf: nodes.append(("lit", "".join(buf)))
    return nodes, i


def message(v):
    nodes, i = parse(unescape(v))
    if i != len(unescape(v)):
        raise ValueError("trailing }")
    return nodes


def args(nodes, out):
    """name -> kind, in first-appearance order (num wins over text; num with sel is an error)."""
    for n in nodes:
        if n[0] in ("arg", "choice"):
            name, kind = n[1], n[2]
            old = out.get(name)
            if old and {old, kind} == {"num", "sel"}:
                raise ValueError(f"{name} is a number and a select")
            if old is None or old == "text":
                out[name] = kind
            if n[0] == "choice":
                for sub in n[3].values():
                    args(sub, out)
    return out


def swift_name(key):
    return key.replace(".", "_")


def lit(s):
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"').replace("\n", "\\n") + '"'


def generate():
    en, ja = load(CAT / "en.msg"), load(CAT / "ja.msg")
    jad = {k: v for k, v, _ in ja}
    missing = [k for k, _, _ in en if k not in jad]
    if missing:
        sys.exit("missing ja: " + ", ".join(missing[:10]) + " (run scripts/i18n-check.py)")
    rows, funcs = [], []
    # the reverse table takes the first message for an English text: the iOS section's entries go first
    for k, v, _ in sorted(en, key=lambda e: not e[2]):
        rows.append(f"{k}\t{v}\t{jad[k]}")
    for k, v, _ in en:
        try:
            a = args(message(v), {})
            aj = args(message(jad[k]), {})
        except ValueError as e:
            sys.exit(f"{k}: {e}")
        if set(a) != set(aj):
            sys.exit(f"{k}: arguments differ between en and ja")
        for name, kind in aj.items():
            if a[name] == "text" and kind != "text":
                a[name] = kind
        if not a:
            continue
        name = swift_name(k)
        params = ", ".join(f"{'`' + n + '`' if n in KEYWORDS else n}: {'Int' if kind == 'num' else 'String'}" for n, kind in a.items())
        ref = lambda n: "`" + n + "`" if n in KEYWORDS else n   # a Swift keyword as a parameter is quoted where it is used too
        body = ", ".join(f'"{n}": {"String(" + ref(n) + ")" if kind == "num" else ref(n)}' for n, kind in a.items())
        funcs.append(f"    /// {k} = {v}\n    static func {name}({params}) -> String {{ f(\"{k}\", [{body}]) }}")
    table = "\n".join(rows)
    src = (
        "// Generated by scripts/i18n-ios.py from i18n/en.msg and i18n/ja.msg. Do not edit: change the catalogs and run it again.\n"
        "// Read by Tr.swift. Rows: key, English, Japanese, separated by tabs, the catalogs' own escapes and ICU.\n\n"
        "extension Tr {\n"
        '    static let catalog = #"""\n' + table + '\n"""#\n\n'
        + "\n\n".join(funcs) + "\n}\n"
    )
    files = {OUT: src}
    for tag, d in (("en", {k: v for k, v, _ in en}), ("ja", jad)):
        entries = "".join(f'"{pk}" = {lit(plain(d[ck]))};\n' for pk, ck in PLIST.items())
        files[ROOT / f"mobile/ios/Backplane/{tag}.lproj/InfoPlist.strings"] = "/* Generated by scripts/i18n-ios.py from i18n/" + tag + ".msg. Do not edit. */\n" + entries
    return files


def plain(v):
    nodes = message(v)
    if any(n[0] != "lit" for n in nodes):
        sys.exit("an OS message must be plain text")
    return "".join(n[1] for n in nodes)


def main():
    files = generate()
    if "--check" in sys.argv[1:]:
        stale = [str(p.relative_to(ROOT)) for p, s in files.items() if not p.exists() or p.read_text(encoding="utf8") != s]
        if stale:
            print("stale (run python3 scripts/i18n-ios.py): " + ", ".join(stale)); sys.exit(1)
        print(f"ok: iOS messages current")
        return
    for p, s in files.items():
        p.write_text(s, encoding="utf8")
    print(f"wrote {', '.join(str(p.relative_to(ROOT)) for p in files)}")


if __name__ == "__main__":
    main()
