#!/usr/bin/env python3
"""The Android app's copy goes through the catalog (i18n/en.msg, ja.msg via Msg.kt and Tr.kt). Fails on:
  (a) a key the Kotlin names (Tr.key("…"), Tr.has("…"), Msg.<name>) that en.msg or ja.msg lacks;
  (b) a bare English string literal handed to what draws text: Text(…), DMono(…), DBtn(…), Icon(…, "…"), DField's hint, DBar's title,
      contentDescription/label/placeholder/hint/title = "…", the notes and errors a screen shows (notice/err/note/… = "…", say("…")),
      notification channels and titles. Technical literals that may stay sit in mobile/android/i18n-allow.txt, each with its reason.
Run from anywhere: python3 scripts/check-android-i18n.py [--list]  (--list prints every hit without failing)."""
import pathlib, re, sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SRC = ROOT / "mobile/android/app/src/main/java/dev/backplane/mobile"
ALLOW = ROOT / "mobile/android/i18n-allow.txt"
SKIP = {"Msg.kt", "Tr.kt"}

def keys(p):
    out = set()
    for l in p.read_text(encoding="utf8").splitlines():
        l = l.strip()
        if l and not l.startswith("#") and " = " in l: out.add(l.partition(" = ")[0])
    return out

# calls whose arguments are drawn (every literal inside them counts, but for the technical spots below)
DRAW_NAMES = r"K|ViewChip|VChip|Loading|TextPrompt|PageRows|TermFull|MicChip|Text|DMono|DBtn|DBar|StripCell|Icon|SidebarRow|ModelChip|DropdownMenuItem|TextButton|AlertDialog|DField|DPill|DChip|DNote|DHead|DLabel|Chip|DSection|ScanNote|DToggle|Choice|Seg|Badge|PageButton|RowButton|Pill|Label|Caption|Hint|Head|Row3|DTitle|SettingRow|DRowLabel|setContentTitle|setContentText|setTicker|NotificationChannel|Toast\.makeText|setName|setDescription"
DRAW = r"(?:" + DRAW_NAMES + r")\s*\("
DRAW_ONE = re.compile(r"(?:" + DRAW_NAMES.replace(r"\.", ".") + r")")
NAMED = r"\b(?:contentDescription|label|placeholder|hint|title|text|note|message|body|tip|caption|subtitle|headline|line|word|desc|summary|empty)\s*=\s*"
STATE = r"\b(?:notice|err|error|note|sent|status|pairingError|failure|why|msg|toast|problem|hintText|warn|info)\s*=\s*"
CALLS = r"\b(?:say|toast|fail|warn|notify|snack|showError|setError|alert)\s*\("
# a literal in these spots is a tag, an id, a protocol word or a format, never copy
TECH_BEFORE = re.compile(r"(?:testTag|has|str|starDecide|projectAct|threadAct|toRequestBody|lng|toMediaType[A-Za-z]*|header|addHeader|setRequestProperty|opt[A-Za-z]*|get[A-Za-z]*|put[A-Za-z]*|Tr\.key|Tr\.has|startsWith|endsWith|contains|removePrefix|removeSuffix|split|replace|equals|call|parse|Regex|launch|substringAfter|substringBefore|indexOf|Uri\.parse|appendQueryParameter|setGroup|Log\.[a-z])\s*\(\s*(?:[^()\"]*,\s*)?$|\btag\s*=\s*$|==\s*$|!=\s*$|\[\s*$|setOf\($|\bin\s+listOf\($")
LIT = re.compile(r'"((?:[^"\\$]|\\.|\$\{[^}]*\}|\$[A-Za-z_][A-Za-z0-9_]*|\$)*)"')

def words(s):
    t = re.sub(r"\$\{[^}]*\}|\$[A-Za-z_][A-Za-z0-9_]*", " ", s)
    t = re.sub(r"\\.", " ", t)
    return re.search(r"[A-Za-z]{2,}", t) is not None

def strip_comments(src):
    out, i, n = [], 0, len(src)
    while i < n:
        if src.startswith("//", i):
            j = src.find("\n", i); j = n if j < 0 else j; out.append(" " * (j - i)); i = j
        elif src.startswith("/*", i):
            j = src.find("*/", i + 2); j = n if j < 0 else j + 2; out.append(re.sub(r"[^\n]", " ", src[i:j])); i = j
        elif src[i] == '"':
            if src.startswith('"""', i):
                j = src.find('"""', i + 3) + 3; out.append(src[i:j]); i = j
            else:
                j = i + 1
                while j < n and src[j] != '"':
                    if src[j] == "\\": j += 1
                    elif src[j] == "$" and src[j + 1:j + 2] == "{":
                        d = 1; j += 2
                        while j < n and d:
                            if src[j] == "{": d += 1
                            elif src[j] == "}": d -= 1
                            j += 1
                        continue
                    j += 1
                out.append(src[i:j + 1]); i = j + 1
        elif src[i] == "'" and src[i + 2:i + 3] == "'" or src.startswith("'\\", i):
            j = src.find("'", i + 1 + (1 if src[i + 1] == "\\" else 0)); out.append(src[i:j + 1]); i = j + 1
        else:
            out.append(src[i]); i += 1
    return "".join(out)

def span(src, i):
    """src[i] is '(': the end of its argument list."""
    d, j = 0, i
    while j < len(src):
        c = src[j]
        if c == '"':
            m = LIT.match(src, j)
            if m: j = m.end(); continue
        if c in "([{": d += 1
        elif c in ")]}":
            d -= 1
            if d == 0: return j
        j += 1
    return len(src)

def hits_in(name, src):
    out = set()
    def lits(a, b):
        for m in LIT.finditer(src, a, b):
            if not words(m.group(1)): continue
            # a dotted id (a test tag, a key) is no copy
            if re.fullmatch(r"[a-z0-9]+(?:\.[a-z0-9_${}.]+)+", m.group(1)): continue
            # Tr.t("…") on a hub's word: fine when it is a catalog message (checked in main)
            if re.search(r"Tr\.t\(\s*$|Tr\.t\([^\n]*\.ifEmpty\s*\{\s*$", src[max(0, m.start() - 80):m.start()]): continue
            before = src[max(0, m.start() - 120):m.start()]
            if TECH_BEFORE.search(before): continue
            # a protocol word handed to a call inside the drawn one (act("mode", …), comment(…, "resolve")) is no copy
            if re.fullmatch(r"[a-z][a-z-]*", m.group(1)):
                d, j = 0, m.start() - 1
                while j > a:
                    c = src[j]
                    if c in ")]}": d += 1
                    elif c in "([{":
                        if d == 0: break
                        d -= 1
                    j -= 1
                if j > a and src[j] == "(":
                    fn = re.search(r"([A-Za-z_][A-Za-z0-9_.]*)\s*$", src[max(0, j - 40):j])
                    if fn and not DRAW_ONE.fullmatch(fn.group(1).split(".")[-1]): continue
            after = src[m.end():m.end() + 6]
            if re.match(r"\s*(->|==|!=|to\b)", after): continue
            out.add((src.count("\n", 0, m.start()) + 1, m.group(1)))
    for m in re.finditer(DRAW, src):
        if m.start() > 0 and (src[m.start() - 1].isalnum() or src[m.start() - 1] in "_."):
            if not src[:m.start()].endswith("Toast."): continue
        o = m.end() - 1; lits(o, span(src, o))
    for pat in (NAMED, STATE):
        for m in re.finditer(pat, src):
            j = m.end()
            # the right-hand side: one expression, to the end of the line or its closing bracket
            e = src.find("\n", j); e = len(src) if e < 0 else e
            seg = src[j:e]; d = 0
            for k, c in enumerate(seg):
                if c in "([{": d += 1
                elif c in ")]}":
                    if d == 0: e = j + k; break
                    d -= 1
                elif c in ",;" and d == 0: e = j + k; break
            lits(j, e)
    for m in re.finditer(CALLS, src):
        o = m.end() - 1; lits(o, span(src, o))
    return out

def main():
    en, ja = keys(ROOT / "i18n/en.msg"), keys(ROOT / "i18n/ja.msg")
    msg_names = {k.replace(".", "_") for k in en}
    english = set()
    for l in (ROOT / "i18n/en.msg").read_text(encoding="utf8").splitlines():
        l = l.strip()
        if l and not l.startswith("#") and " = " in l: english.add(l.partition(" = ")[2].replace("\\s", " "))
    allow = {}
    for n, l in enumerate(ALLOW.read_text(encoding="utf8").splitlines() if ALLOW.exists() else [], 1):
        if not l.strip() or l.startswith("#"): continue
        lit, sep, why = l.partition("\t#")
        if not sep or not why.strip(): sys.exit(f"i18n-allow.txt:{n}: an entry is <literal><TAB># <reason>")
        allow[lit] = why.strip()
    bad, used_allow = [], set()
    for f in sorted(SRC.glob("*.kt")):
        if f.name in SKIP: continue
        raw = f.read_text(encoding="utf8"); src = strip_comments(raw)
        for m in re.finditer(r'Tr\.(?:key|has)\(\s*"([^"]+)"\s*\)', src):
            if m.group(1) not in en or m.group(1) not in ja: bad.append(f"{f.name}: key {m.group(1)} is not in en.msg and ja.msg")
        for m in re.finditer(r'Tr\.t\((?:[^\n"]*\.ifEmpty\s*\{\s*)?"((?:[^"\\$]|\\.)*)"', src):
            if m.group(1) not in english: bad.append(f"{f.name}: Tr.t fallback \"{m.group(1)}\" is no English catalog message")
        for m in re.finditer(r"\bMsg\.([a-z][a-z0-9_]*)", src):
            if m.group(1) not in msg_names: bad.append(f"{f.name}: Msg.{m.group(1)} is no key of en.msg")
        # menus keep their English labels as ids (tags, the danger colour) and draw them through Tr.t: each must be a catalog message
        routed = [(m.end() - 1, span(src, m.end() - 1)) for m in re.finditer(r"\bActionSheet\s*\(", src)]
        routed += [(m.start(), src.find("\n}", m.start())) for m in re.finditer(r"\bfun threadActions\b", src)]
        menu_lits = set()
        for a, b in routed:
            for m in LIT.finditer(src, a, b):
                if not words(m.group(1)) or re.fullmatch(r"[a-z0-9]+(?:\.[a-z0-9_${}.]+)+|[a-z]+", m.group(1)): continue
                menu_lits.add(m.start())
                if m.group(1) not in english: bad.append(f"{f.name}:{src.count(chr(10), 0, m.start()) + 1}: menu label \"{m.group(1)}\" is drawn through Tr.t but is no English catalog message")
        for line, lit in sorted(hits_in(f.name, src)):
            if lit in allow: used_allow.add(lit); continue
            bad.append(f"{f.name}:{line}: bare English \"{lit}\": route it through Msg/Tr, or allow it in mobile/android/i18n-allow.txt with a reason")
    stale = sorted(set(allow) - used_allow)
    bad += [f"i18n-allow.txt: \"{s}\" is allowed but no longer found (remove it)" for s in stale]
    if "--list" in sys.argv[1:]: print("\n".join(bad)); print(len(bad), "hits"); return
    if bad: print("\n".join(bad)); print(f"{len(bad)} problems"); sys.exit(1)
    print(f"ok: android copy goes through the catalog ({len(used_allow)} allowed technical literals)")

if __name__ == "__main__":
    main()
