#!/usr/bin/env python3
# scripts/stamp-version.py on made-up emitted C laid out the way bend lays
# out STAT_IMG: each literal a chain of two-word cells built from its last
# character, equal tails shared between literals (bend's memo on the cell's
# text). Checks: the slot stamped (and every other string unchanged),
# stamping again a no-op, and every unsafe case refused: too long, no slot,
# two slots, a slot whose cells another string shares (an old slot without
# its terminator next to a "~~~" literal).
#   python3 test/tools/stamp_version_test.py
import os, subprocess, sys, tempfile
here = os.path.dirname(os.path.abspath(__file__))
tool = os.path.join(here, "..", "..", "scripts", "stamp-version.py")
TERM = "~@BPVERSIONSLOT"
def slot(v):
    return v + "~" * (40 - len(v)) + TERM

class Img:
    # bend's FL.img and FL.lits: a cell is pushed once per distinct text
    def __init__(self):
        self.words, self.memo = [], {}
    def lit(self, s):
        nxt = "term_pak(CID_SNIL, 0)"
        for c in reversed(s):
            cell = "%dull, %s" % (ord(c), nxt)
            if cell not in self.memo:
                self.memo[cell] = len(self.words)
                self.words += ["%dull" % ord(c), nxt]
            nxt = "term_ctr(CID_SCON, STAT_OFF + %d)" % self.memo[cell]
        return int(nxt.split("+ ")[1][:-1]) if "+" in nxt else None
    def c(self, refs=()):
        code = "".join("  r0 = term_ctr(CID_SCON, STAT_OFF + %d);\n" % r for r in refs)
        return "// prelude\nCONSTV u64 STAT_IMG[] = { " + ", ".join(self.words) + " };\n" + code

def emitted(strings, extra=()):
    img = Img()
    heads = [img.lit(s) for s in strings]
    return img, img.c([h for h in heads if h is not None] + list(extra))

def run(text, ver):
    with tempfile.NamedTemporaryFile("w", suffix=".c", delete=False) as f:
        f.write(text)
    r = subprocess.run([sys.executable, tool, f.name, ver], capture_output=True, text=True)
    out = open(f.name).read()
    os.unlink(f.name)
    return r.returncode, out, r.stderr

fail = 0
def check(name, ok, info=""):
    global fail
    print(("ok   " if ok else "FAIL ") + name + ("" if ok else "  " + info))
    fail |= not ok

v = "0.11.2-dev.20261015.1222"
others = ["hello", "~~~", "a ~", "dev", "0.1.0-dev"]
_, base = emitted(others + [slot("0.1.0-dev")] + ["bye~~~"])
_, want = emitted(others + [slot(v)] + ["bye~~~"])
rc, out, err = run(base, v)
# the stamped C is what an emit of the stamped slot would be, cell for cell
# where the slot's cells are its own (here: same cell order, only codes differ)
check("stamps the slot", rc == 0 and len(out.split(",")) == len(base.split(",")), err)
check("only the slot's codes changed", rc == 0 and sum(a != b for a, b in zip(base.split(", "), out.split(", "))) == sum(a != b for a, b in zip(slot("0.1.0-dev"), slot(v))), err)
rc2, out2, _ = run(out, v)
check("stamping again is a no-op", rc2 == 0 and out2 == out)
_, plain = emitted(["hello", "bye"])
rc, _, err = run(plain, v)
check("no slot is refused", rc != 0 and "0 times" in err, err)
img = Img(); h1 = img.lit(slot("0.1.0-dev")); text2 = img.c([h1, h1])
# a second, separate copy of the slot (not shared: built by hand)
words = img.words[:]
base2 = len(words)
cells = []
nxt = "term_pak(CID_SNIL, 0)"
for c in reversed(slot("0.1.0-dev")):
    words += ["%dull" % ord(c), nxt]
    nxt = "term_ctr(CID_SCON, STAT_OFF + %d)" % (len(words) - 2)
two = "CONSTV u64 STAT_IMG[] = { " + ", ".join(words) + " };\n"
rc, _, err = run(two, v)
check("two slots are refused", rc != 0 and "2 times" in err, err)
rc, _, err = run(base, "x" * 41)
check("a version too long is refused", rc != 0)
rc, _, err = run(base, "1.2~3")
check("a version with the fill is refused", rc != 0)
# a slot without the terminator shares its tilde tail with "~~~" and "bye~~~"
img = Img(); img.lit("~~~"); img.lit("bye~~~"); h = img.lit("0.1.0-dev" + "~" * 31 + "~@BPVERSIONSLOT")
rc, out3, err = run(img.c([h]), v)
check("the terminated slot is its own even next to ~~~ strings", rc == 0, err)
img = Img(); t = img.lit("~@BPVERSIONSLOT"); h = img.lit(slot("0.1.0-dev"))
rc, _, err = run(img.c([h, t]), v)
check("sharing only the terminator is fine (it never changes)", rc == 0, err)
img = Img(); t = img.lit("~~~~~~~~~~" + TERM); h = img.lit(slot("0.1.0-dev"))
rc, _, err = run(img.c([h, t]), v)
check("a literal sharing cells of the body is refused", rc != 0 and "shared" in err, err)
rc, out, _ = run(base, "0.1.0-dev")
check("the default version leaves it as it is", rc == 0 and out == base)
sys.exit(fail)
