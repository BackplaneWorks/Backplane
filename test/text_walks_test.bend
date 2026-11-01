import Base
import ./check.bend as C
import ../src/core/ref-origin.bend as O
import ../src/core/ring.bend as RG
import ../src/core/voice.bend as V

# Per-character walks test where a match starts with the cheap test first
# (Origin.start, Ring.at, Voice.fix.at). Each must answer exactly what the
# old test did: String.starts_with (and String.drop) of SCon{c, t}, which
# built a string as long as the rest of the text at every character. Here
# the old formulas are the reference, at every suffix of each sample.

def ring.old(+c: Char, +t: String, +pat: String) -> Bool:
  String.starts_with(SCon{c, t}, pat)

def voice.old(pv: Bool, +c: Char, +t: String, +w: String, +lw: Nat) -> Bool:
  Bool.and(Bool.not(pv), Bool.and(String.starts_with(SCon{c, t}, w), Bool.not(V.Voice.fix.head(String.drop(SCon{c, t}, lw)))))

def origin.old(+c: Char, +t: String, +acc: String, +mode: Nat) -> Bool:
  Bool.and(Nat.is_eq(mode, 0n), Bool.and(O.Origin.boundary(acc), String.starts_with(SCon{c, t}, "backplane://")))

def same(+a: Bool, +b: Bool) -> Bool:
  Bool.or(Bool.and(a, b), Bool.and(Bool.not(a), Bool.not(b)))

def ring.at.go(s: String, +pat: String, ok: Bool) -> Bool:
  match s:
    case SNil{}: ok
    case SCon{+c, +t}: ring.at.go(t, pat, Bool.and(ok, same(RG.Ring.at(c, t, pat), ring.old(c, t, pat))))

def voice.at.go(s: String, +w: String, ok: Bool) -> Bool:
  match s:
    case SNil{}: ok
    case SCon{+c, +t}:
      voice.at.go(t, w, Bool.and(ok, Bool.and(
        same(V.Voice.fix.at(False{}, c, t, w, String.length(w)), voice.old(False{}, c, t, w, String.length(w))),
        same(V.Voice.fix.at(True{}, c, t, w, String.length(w)), voice.old(True{}, c, t, w, String.length(w))))))

def origin.at.one(+c: Char, +t: String, +acc: String) -> Bool:
  Bool.and(Bool.and(same(O.Origin.start(c, t, acc, 0n), origin.old(c, t, acc, 0n)), same(O.Origin.start(c, t, acc, 1n), origin.old(c, t, acc, 1n))),
    same(O.Origin.start(c, t, acc, 2n), origin.old(c, t, acc, 2n)))

def origin.at.go(s: String, +acc: String, ok: Bool) -> Bool:
  match s:
    case SNil{}: ok
    case SCon{+c, +t}: origin.at.go(t, acc, Bool.and(ok, origin.at.one(c, t, acc)))

def texts() -> List<&2, String>:
  ["", "a", "b", "cad cadence, cad. scad cad", "key cad key ca key cadx", "x\r\n--y\r\n\r\nbody\r\n-", "\r\n\r\n\r\n--",
   "backplane://thread/t xbackplane://p bbackplane:/ backplan b backplane:", "日本 cad 語 b", "ab ab a"]

def ring.all(ts: List<&2, String>, +pat: String) -> Bool:
  match ts:
    case Nil{}: True{}
    case t <> r: Bool.and(ring.at.go(t, pat, True{}), ring.all(r, pat))

def voice.all(ts: List<&2, String>, +w: String) -> Bool:
  match ts:
    case Nil{}: True{}
    case t <> r: Bool.and(voice.at.go(t, w, True{}), voice.all(r, w))

def origin.all(ts: List<&2, String>, +acc: String) -> Bool:
  match ts:
    case Nil{}: True{}
    case t <> r: Bool.and(origin.at.go(t, acc, True{}), origin.all(r, acc))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("Ring.at is starts_with of SCon{c, t}: \\r\\n--", Bool.show(ring.all(texts(), "\r\n--")), "True")
    C.Check.str("Ring.at: \\r\\n\\r\\n", Bool.show(ring.all(texts(), "\r\n\r\n")), "True")
    C.Check.str("Ring.at: one character", Bool.show(ring.all(texts(), "b")), "True")
    C.Check.str("Ring.at: the empty pattern", Bool.show(ring.all(texts(), "")), "True")
    C.Check.str("Voice.fix.at is the old test: cad", Bool.show(voice.all(texts(), "cad")), "True")
    C.Check.str("Voice.fix.at: a word with a space", Bool.show(voice.all(texts(), "key cad")), "True")
    C.Check.str("Voice.fix.at: one letter", Bool.show(voice.all(texts(), "a")), "True")
    C.Check.str("Voice.fix.at: not a word character", Bool.show(voice.all(texts(), ".")), "True")
    C.Check.str("Origin.start is the old test: after nothing", Bool.show(origin.all(texts(), "")), "True")
    C.Check.str("Origin.start: after a space", Bool.show(origin.all(texts(), " ")), "True")
    C.Check.str("Origin.start: after a letter", Bool.show(origin.all(texts(), "x")), "True")
    C.Check.str("Origin.start: after a slash", Bool.show(origin.all(texts(), "/")), "True")
    C.Check.str("Voice.fix still fixes", V.Voice.fix("cad cadence, cad. scad cad", "cad", "CAD"), "CAD cadence, CAD. scad CAD")
    C.Check.str("Ring.until still cuts", RG.Ring.until("value\r\n--rest", "\r\n--"), "value")
