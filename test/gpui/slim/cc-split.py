#!/usr/bin/env python3
# Split the one large C file `bend X -o out.c` emits into N translation
# units that compile in parallel (and each in far less memory).
#
#   scripts/cc-split.py build/app.c build/app-split 8
#
# The emitted file is: a prelude (runtime, tables, spins), the segments
# (one `WL_CASE(F)` function each, calling one another by musttail), then
# the rest of the runtime and the effects. Every unit gets the prelude;
# the segments are dealt out over units 1..N; unit 0 keeps the rest.
# Segments become hidden (not static) so units can call each other's, and
# the runtime's mutable globals are defined in unit 0 and extern elsewhere.
import os
import re
import sys

src, out, n = sys.argv[1], sys.argv[2], int(sys.argv[3])
text = open(src).read()

SEG = "\n// Segments\n// ========\n"
TAIL = "\n// A task enters through its words"
GLOB = "\n// Globals\n// =======\n"
# bend 2.0.36 ends the globals at the Tables section and returns Term, 2.0.25
# ended them at the device-program comment and returned Reply
GLOB_END = "\n// Tables\n// ======\n"
if text.count(GLOB_END) != 1:
    GLOB_END = "\n// The device program compiles from the binary's own text."
WL_FN = "#define WL_FN      static PRESERVE(preserve_none) __attribute__((noinline)) "
WL_FN += "Reply" if WL_FN + "Reply" in text else "Term"

for mark in (SEG, TAIL, GLOB, GLOB_END, WL_FN):
    if text.count(mark) != 1:
        sys.exit("cc-split: the emitted C changed shape (%r)" % mark.strip())

seg_at = text.index(SEG)
tail_at = text.index(TAIL)
# bend 2.0.36 puts that comment before the segments, wraps each segment in
# its own `#if !DEVICE`, and the tail starts at the runtime's FID_ENTER case
ENTER = "\n  WL_CASE(FID_ENTER)"
per_seg_if = text.count(ENTER) == 1 and text.index(ENTER) > tail_at
if per_seg_if:
    tail_at = text.index(ENTER)
prelude, segs, tail = text[:seg_at], text[seg_at:tail_at], text[tail_at:]

prelude = prelude.replace(WL_FN,
    '#define WL_FN      __attribute__((visibility("hidden"))) PRESERVE(preserve_none) __attribute__((noinline)) ' + WL_FN.split()[-1])

# the globals: `static T name...;` (maybe with an initializer)
g0, g1 = prelude.index(GLOB), prelude.index(GLOB_END)
glob = prelude[g0:g1]
decl = re.compile(r"^static (?!const )((?:_Atomic )?[A-Za-z_][A-Za-z0-9_]*(?:\s+|\s*\*\s*))([A-Za-z_][A-Za-z0-9_]*)((?:\[[^\]]*\])*)([^;=]*)(=[^;]*)?;", re.M)

def as_extern(m):
    return "extern %s%s%s%s;" % (m.group(1), m.group(2), m.group(3), m.group(4))

def as_def(m):
    return "%s%s%s%s%s;" % (m.group(1), m.group(2), m.group(3), m.group(4), m.group(5) or "")

# functions the prelude declares and the tail defines (corpus_grow,
# f32_show...): shared, so hidden rather than static on both sides
proto = re.compile(r"^static ([A-Za-z_][A-Za-z0-9_ ]*?\**)\s*\b([A-Za-z_][A-Za-z0-9_]*)\(([^;{]*)\);$", re.M)
shared = [m.group(2) for m in proto.finditer(prelude)
          if re.search(r"(?m)^static [^;{]*\b%s\([^;]*\{" % re.escape(m.group(2)), tail)]
for name in shared:
    pat = r"(?m)^static ([^;{(]*\b%s\()" % re.escape(name)
    prelude = re.sub(pat, r'__attribute__((visibility("hidden"))) \1', prelude)
    tail = re.sub(pat, r'__attribute__((visibility("hidden"))) \1', tail)
g0, g1 = prelude.index(GLOB), prelude.index(GLOB_END)

pre_ext = prelude[:g0] + decl.sub(as_extern, glob) + prelude[g1:]
pre_def = prelude[:g0] + decl.sub(as_def, glob) + prelude[g1:]

# The "Spins" section is STAT_IMG (the static data image, which only unit
# 0's runtime reads) and one `INLINE|FAR Term spin_N(...) {...}` per helper.
# A unit keeps only the helpers its segments reach (transitively), so an
# edit to one helper changes only the units that use it (which lets
# build-app.sh reuse the objects of every other unit).
SPINS = "\n// Spins\n// =====\n"
WORK = "\n// Work\n// ====\n"
IMG = "\nCONSTV u64 STAT_IMG[]"
SPIN_DEF = re.compile(r"(?m)^(?=(?:INLINE|FAR) Term spin_\d+\()")
SPIN_REF = re.compile(r"\bspin_\d+\b")
for mark in (SPINS, WORK, IMG):
    if prelude.count(mark) != 1:
        sys.exit("cc-split: the emitted C changed shape (%r)" % mark.strip())

def spin_parts(pre):
    """(text before STAT_IMG, STAT_IMG, {spin: text}, spin order, text from Work on)"""
    a, b = pre.index(SPINS), pre.index(WORK)
    sec = pre[a:b]
    k = sec.index(IMG)
    e = sec.index(";\n", k) + 2
    blks = SPIN_DEF.split(sec[e:])
    if blks[0].strip():
        sys.exit("cc-split: text before the first spin")
    names, body = [], {}
    for blk in blks[1:]:
        m = re.match(r"(?:INLINE|FAR) Term (spin_\d+)\(", blk)
        if not m:
            sys.exit("cc-split: unexpected text in the spins section")
        names.append(m.group(1))
        body[m.group(1)] = blk
    return pre[:a] + sec[:k] + "\n", sec[k:e], body, names, pre[b:]

def with_spins(pre, used, image):
    head, img, body, names, rest = spin_parts(pre)
    need, todo = set(), SPIN_REF.findall(used + rest)
    while todo:
        x = todo.pop()
        if x in need or x not in body:
            continue
        need.add(x)
        todo.extend(SPIN_REF.findall(body[x]))
    return head + (img if image else "") + "".join(body[x] for x in names if x in need) + rest

if "STAT_IMG" in spin_parts(prelude)[4]:
    sys.exit("cc-split: STAT_IMG is read before the segments")

# segments: the body between the section header's `#if !DEVICE` and the
# last `#endif`
body = segs[len(SEG):].lstrip("\n")
open_if = "#if !DEVICE\n"
lead = ""
if per_seg_if:
    first = "#if !DEVICE\n  WL_CASE("
    if first not in body:
        sys.exit("cc-split: no segment found")
    lead, body = body[:body.index(first)], body[body.index(first):]
    parts = re.split(r"(?m)^(?=#if !DEVICE\n  WL_CASE\()", body)
    open_if, close_if = "", ""
    head, cases = "", parts[1:] if parts[0] == "" else parts
    tail = lead + tail
else:
    if not body.startswith(open_if):
        sys.exit("cc-split: segments do not open with #if !DEVICE")
    body = body[len(open_if):]
    close = body.rstrip().rfind("#endif")
    body = body[:close]
    parts = re.split(r"(?m)^(?=  WL_CASE\()", body)
    head, cases = parts[0], parts[1:]
    close_if = "#endif\n"
if head.strip():
    sys.exit("cc-split: text before the first segment")

# deal the segments out in contiguous runs of similar size
total = sum(len(c) for c in cases)
units, cur, size = [], [], 0
for c in cases:
    cur.append(c)
    size += len(c)
    if size >= total / n and len(units) < n - 1:
        units.append(cur)
        cur, size = [], 0
units.append(cur)

os.makedirs(out, exist_ok=True)
for f in os.listdir(out):
    if f.startswith("u") and f.endswith(".c"):
        os.remove(os.path.join(out, f))
with open(os.path.join(out, "u0.c"), "w") as f:
    f.write(with_spins(pre_def, tail, True) + SEG + tail)
for i, u in enumerate(units, 1):
    mine = "".join(u)
    with open(os.path.join(out, "u%d.c" % i), "w") as f:
        f.write(with_spins(pre_ext, mine, False) + SEG + open_if + mine + close_if)
print(len(units) + 1)
