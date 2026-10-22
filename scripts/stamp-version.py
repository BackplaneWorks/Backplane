#!/usr/bin/env python3
# Stamp the build's version into the JS bend emitted, in place of the slot
# src/core/version.bend holds (Version.slot), so the version is no input of
# the emit: one tree emits the same JS whatever version it is built as, and
# scripts/build-js.sh can reuse it (BACKPLANE_EMIT_CACHE).
#
#   scripts/stamp-version.py build/js/backplane-serve/stamped.js 0.11.2-dev.20261015.1222
#   scripts/stamp-version.py build/js/backplane-serve/stamped.js <40-hex sha> commit
#   (a .js file, the dev service's emit, has each slot as one literal)
#     the commit slot (Version.commit.slot) the same way: a git sha, or
#     nothing stamped ("unknown")
#
# A string literal is a chain of cells in STAT_IMG: two words each, the
# character's code (`<n>ull`) and the rest (`term_ctr(CID_SCON, STAT_OFF + k)`,
# or `term_pak(CID_SNIL, 0)` at the end). bend shares equal tails between
# literals, so the slot ends in a fixed terminator no other string ends in;
# then no cell of it can be another string's. The stamper finds the chain
# that spells the unstamped slot by following the cells (not by where they
# lie), insists on exactly one, and that each cell it rewrites (the 40 of the
# body) is pointed at only by the cell before it anywhere in the file (its
# head may be used from code), and rewrites only those cells' codes. Stamping again with the same version is a no-op; any
# other case (no slot, two, a shared cell, too long) stops the build.
import os
import re
import sys

BODY = 40
FILL = "~"
TERM = "~@BPVERSIONSLOT"
DEFAULT = "0.1.0-dev"
if len(sys.argv) > 3 and sys.argv[3] == "commit":
    TERM = "~@BPCOMMITSLOT"
    DEFAULT = "unknown"
    if not re.fullmatch(r"[0-9a-f]{40}", sys.argv[2]):
        sys.exit("stamp-version: %r is not a 40-hex commit" % sys.argv[2])
elif len(sys.argv) > 3:
    sys.exit("stamp-version: unknown slot %r" % sys.argv[3])

def slot(ver):
    return ver + FILL * (BODY - len(ver)) + TERM

path, ver = sys.argv[1], sys.argv[2]
if FILL in ver or "@" in ver or len(ver) > BODY or not ver.isprintable() or not ver:
    sys.exit("stamp-version: %r does not fit the slot" % ver)

text = open(path).read()
if path.endswith(".js"):
    # the JS bend emitted (the dev service): the slot is one string
    # literal; stamped (or stamped before with this version) exactly once
    lits = re.findall(r'"([^"\\]*' + re.escape(TERM) + r')"', text)
    if len(lits) != 1:
        sys.exit("stamp-version: %d slots ending %s in %s, want one" % (len(lits), TERM, path))
    text = text.replace('"' + lits[0] + '"', '"' + slot(ver) + '"')
    tmp = path + ".stamp"
    open(tmp, "w").write(text)
    os.replace(tmp, path)
    sys.exit(0)
head = "CONSTV u64 STAT_IMG[] = {"
i0 = text.find(head)
if i0 < 0 or text.count(head) != 1:
    sys.exit("stamp-version: no single STAT_IMG in %s" % path)
i0 += len(head)
i1 = text.index("};", i0)
body = text[i0:i1]

# the image's words, split at the commas between them
words, depth, cur = [], 0, []
for ch in body:
    if ch == "(":
        depth += 1
    elif ch == ")":
        depth -= 1
    if ch == "," and depth == 0:
        words.append("".join(cur))
        cur = []
    else:
        cur.append(ch)
words.append("".join(cur))
w = [x.strip() for x in words]

CODE = re.compile(r"^(\d+)ull$")
NEXT = re.compile(r"^term_ctr\(CID_SCON, STAT_OFF \+ (\d+)\)$")
END = "term_pak(CID_SNIL, 0)"

def follow(k, n):
    # the cells of the string at word k, at most n characters: [(cell, code)]
    out = []
    for _ in range(n + 1):
        if k + 1 >= len(w):
            return None
        m = CODE.match(w[k])
        if not m:
            return None
        out.append((k, int(m.group(1))))
        nx = w[k + 1]
        if nx == END:
            return out
        m2 = NEXT.match(nx)
        if not m2:
            return None
        k = int(m2.group(1))
    return None

def find(s):
    first = ord(s[0])
    hits = []
    for k in range(0, len(w) - 1):
        m = CODE.match(w[k])
        if not m or int(m.group(1)) != first:
            continue
        cells = follow(k, len(s))
        if cells and "".join(chr(c) for _, c in cells) == s:
            hits.append(cells)
    return hits

want, have = slot(ver), slot(DEFAULT)
if want == have:
    if len(find(have)) == 1:
        sys.exit(0)
    sys.exit("stamp-version: found the %s slot %d times" % (TERM, len(find(have))))
slots = find(have)
if not slots:
    if len(find(want)) == 1:
        sys.exit(0)  # stamped already
    sys.exit("stamp-version: found the version slot 0 times")
if len(slots) != 1:
    sys.exit("stamp-version: found the version slot %d times" % len(slots))
cells = slots[0]
# the cells it rewrites (the body; the terminator never changes) must be
# its own: every one but the head pointed at exactly once in the whole file
# (by the cell before it), code included
refs = {}
for m2 in re.finditer(r"STAT_OFF \+ (\d+)\)", text):
    o = int(m2.group(1))
    refs[o] = refs.get(o, 0) + 1
for k, _ in cells[1:BODY]:
    if refs.get(k, 0) != 1:
        sys.exit("stamp-version: a cell of the slot is shared (%d uses of STAT_OFF + %d)" % (refs.get(k, 0), k))
for (k, _), c in zip(cells, want):
    w[k] = "%dull" % ord(c)
out = text[:i0] + " " + ", ".join(w) + " " + text[i1:]
# the rest of the file must be untouched: only the codes changed
if len(out) - len(text) != sum(len(str(ord(c))) - len(str(old)) for (_, old), c in zip(cells, want)):
    sys.exit("stamp-version: STAT_IMG is not laid out as expected (its spacing)")
tmp = path + ".stamp"
open(tmp, "w").write(out)
os.replace(tmp, path)
