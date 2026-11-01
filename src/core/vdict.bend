import Base
import ./json.bend as J
import ./voice.bend as V

# The dictionary and what dictation does with it (pure; laws vd_*, vs_*).
#
# Entries are settings, so they live in the hub's log and every client and
# every engine sees the same ones:
#   voice.e.<key>  one entry, written as a person types it in bulk:
#                  "KiCad"                  a term
#                  "KiCad = key cad, kai cad" a term and how it sounds
#                  "jason -> JSON"          a replacement
#                  "!um"                    a word to drop
#   voice.l.<heard> a learned replacement "<to>\t<confirmed>\t<undone>",
#                  from the person fixing a word (Fix…) or undoing a fix;
#                  it applies by itself once confirmed twice and never undone
# An empty value removes either (model.bend's Setting.blank_gone).
#
# A final segment goes through Vd.fix: drop words, replacements and how
# terms sound (exact), terms written another way ("kicad", "stm 32"), then
# near misses that sound alike ("Kai CAD" for KiCad), applied by themselves
# only where the engine itself was unsure, else offered as a guess.
#
# Live words (Settle, Vs.*): each new hypothesis of the open segment is
# compared with the last; words two in a row agree on, far enough from its
# end, are committed and never change again; the rest is the tail, which
# alone may be revised. The segment's last hypothesis commits what is left.

# a heard word: as written, its probability (percent; 100 unknown), start
# and end (centiseconds from the segment's start; 0 0 unknown)
type VW is Data:
  VW{w: String, p: Nat, s: Nat, e: Nat}

# an exact rule: what to write for a heard phrase, and whether it applies by
# itself (a learned one not yet trusted is only offered)
type VR is Data:
  VR{out: String, auto: Bool}

# a term for near misses: its sound key, its letters (lower, no spaces), it
type VF is Data:
  VF{key: String, flat: String, out: String}

# the prepared dictionary: exact phrases, terms by their letters, terms for
# near misses, the terms as written (keywords, unsure checks)
type VDict is Data:
  VDict{exact: Map<&2, VR>, flat: Map<&2, String>, fuzzy: List<&2, VF>, terms: List<&2, String>}

# a fix made, for the strip under the composer (and its undo)
type VFx is Data:
  VFx{heard: String, out: String}

type VFix is Data:
  VFix{words: List<&2, VW>, fixes: List<&2, VFx>, ask: List<&2, V.Unsure>}

def VW.w(x: VW) -> String:
  match x:
    case VW{w, _, _, _}:
      w

def VW.p(x: VW) -> Nat:
  match x:
    case VW{_, p, _, _}:
      p

def VW.s(x: VW) -> Nat:
  match x:
    case VW{_, _, s, _}:
      s

def VW.e(x: VW) -> Nat:
  match x:
    case VW{_, _, _, e}:
      e

# Characters
# ----------

# . , ! ? ; : " ' ( ) [ ] “ ” ‘ ’ … around a word
def Vd.punct(+c: U32) -> Bool:
  Bool.or(Bool.or(Bool.or(U32.is_eq(c, 46), U32.is_eq(c, 44)), Bool.or(U32.is_eq(c, 33), U32.is_eq(c, 63))),
    Bool.or(Bool.or(Bool.or(U32.is_eq(c, 59), U32.is_eq(c, 58)), Bool.or(U32.is_eq(c, 34), U32.is_eq(c, 39))),
      Bool.or(Bool.or(Bool.or(U32.is_eq(c, 40), U32.is_eq(c, 41)), Bool.or(U32.is_eq(c, 91), U32.is_eq(c, 93))),
        Bool.or(Bool.or(U32.is_eq(c, 8220), U32.is_eq(c, 8221)), Bool.or(Bool.or(U32.is_eq(c, 8216), U32.is_eq(c, 8217)), U32.is_eq(c, 8230))))))

def Vd.pc(c: Char) -> Bool:
  match c:
    case Chr{+x}:
      Vd.punct(x)

# leading punctuation (in order) and the rest
# whether s opens with punctuation
def Vd.headp(+s: String) -> Bool:
  match s:
    case SNil{}:
      False{}
    case SCon{+c, _}:
      Vd.pc(c)

def Vd.lead.go(s: String, p: Bool, acc: String) -> String & String:
  match s p:
    case SCon{+c, +t} True{}:
      Vd.lead.go(t, Vd.headp(t), SCon{c, acc})
    case s2 _:
      (String.reverse(acc), s2)

def Vd.lead(+s: String) -> String & String:
  Vd.lead.go(s, Vd.headp(s), SNil{})

# a word's leading punctuation, its core, its trailing punctuation
type Pw is Data:
  Pw{lead: String, core: String, trail: String}

def Vd.split.r(+l: String, r: String & String) -> Pw:
  match r:
    case (+tr, +cr):
      Pw{l, String.reverse(cr), String.reverse(tr)}

def Vd.split.m(a: String & String) -> Pw:
  match a:
    case (+l, +rest):
      Vd.split.r(l, Vd.lead(String.reverse(rest)))

def Vd.split(+w: String) -> Pw:
  Vd.split.m(Vd.lead(w))

def Pw.lead(x: Pw) -> String:
  match x:
    case Pw{l, _, _}:
      l

def Pw.core(x: Pw) -> String:
  match x:
    case Pw{_, c, _}:
      c

def Pw.trail(x: Pw) -> String:
  match x:
    case Pw{_, _, t}:
      t

# a word as compared: no punctuation around it, lower case
def Vd.norm(+w: String) -> String:
  String.to_lower(Pw.core(Vd.split(w)))

# spaces out
def Vd.flat(+s: String) -> String:
  V.Voice.unspace(s)

# runs of spaces made one, trimmed, lower case (a heard phrase)
def Vd.phrase.go(ws: List<&2, String>, +acc: List<&2, String>) -> List<&2, String>:
  match ws:
    case Nil{}:
      List.reverse(&2, String, acc)
    case +h <> t:
      Vd.phrase.go(t, Bool.pick(List<&2, String>, String.is_empty(Vd.norm(h)), acc, Vd.norm(h) <> acc))

def Vd.phrase(+s: String) -> String:
  String.join(Vd.phrase.go(String.split(String.trim(s), ' '), Nil{}), " ")

def Vd.cls.l(+c: U32) -> U32:
  Bool.pick(U32, Bool.or(Bool.or(Bool.or(U32.is_eq(c, 97), U32.is_eq(c, 101)), Bool.or(U32.is_eq(c, 105), U32.is_eq(c, 111))), Bool.or(U32.is_eq(c, 117), U32.is_eq(c, 121))), 0,
    Bool.pick(U32, Bool.or(U32.is_eq(c, 104), U32.is_eq(c, 119)), 1,
      Bool.pick(U32, U32.is_eq(c, 98), 112,
        Bool.pick(U32, U32.is_eq(c, 100), 116,
          Bool.pick(U32, Bool.or(Bool.or(U32.is_eq(c, 99), U32.is_eq(c, 103)), U32.is_eq(c, 113)), 107,
            Bool.pick(U32, U32.is_eq(c, 118), 102,
              Bool.pick(U32, U32.is_eq(c, 122), 115, c)))))))

# Sound keys
# ----------

# a letter's sound class: 0 vowel (kept only first, as 'a'), 1 silent
# (h w), else the letter it stands for (b/p, d/t, c/g/k/q, f/v, s/z, x ks)
def Vd.cls.k(lo: Bool, +c: U32) -> U32:
  match lo:
    case False{}:
      c
    case True{}:
      Vd.cls.l(c)

def Vd.cls(+c: U32) -> U32:
  Vd.cls.k(U32.is_ge(c, 97), c)

# the key so far (reversed), the last class written, whether at the start
type Sk is Data:
  Sk{acc: String, last: U32, first: Bool}

def Vd.sk.put(+k: U32, st: Sk) -> Sk:
  match st:
    case Sk{+acc, +last, first}:
      Bool.pick(Sk, U32.is_eq(k, last), Sk{acc, last, False{}}, Sk{SCon{Chr{k}, acc}, k, False{}})

def Vd.sk.x(x: Bool, +k: U32, st: Sk) -> Sk:
  match x:
    case True{}:
      Vd.sk.put(115, Vd.sk.put(107, st))
    case False{}:
      Vd.sk.put(k, st)

def Vd.sk.mid.k(quiet: Bool, +k: U32, st: Sk) -> Sk:
  match quiet:
    case True{}:
      st
    case False{}:
      Vd.sk.x(U32.is_eq(k, 120), k, st)

def Vd.sk.mid(+k: U32, st: Sk) -> Sk:
  Vd.sk.mid.k(U32.is_le(k, 1), k, st)

def Vd.sk.step(+k: U32, st: Sk) -> Sk:
  match st:
    case Sk{acc, last, True{}}:
      Vd.sk.put(Bool.pick(U32, U32.is_eq(k, 0), 97, k), Sk{acc, last, True{}})
    case Sk{acc, last, False{}}:
      Vd.sk.mid(k, Sk{acc, last, False{}})

def Vd.sk.go(s: String, st: Sk) -> String:
  match s:
    case SNil{}:
      match st:
        case Sk{acc, _, _}:
          String.reverse(acc)
    case SCon{Chr{+c}, t}:
      Vd.sk.go(t, Vd.sk.step(Vd.cls(c), st))

# how a word sounds, roughly ("kicad", "kaicad" and "keycad" all "kt")
def Vd.key(+flat: String) -> String:
  Vd.sk.go(flat, Sk{SNil{}, 2, True{}})

# Entries
# -------

def Vd.e.pre() -> String:
  "voice.e."

def Vd.l.pre() -> String:
  "voice.l."

# the text before the first sep (all of it when none)
def Vd.left.go(s: String, hit: Bool, +sep: String, acc: String) -> String:
  match s hit:
    case SCon{+c, +t} False{}:
      Vd.left.go(t, String.starts_with(t, sep), sep, SCon{c, acc})
    case _ _:
      String.reverse(acc)

def Vd.left(+s: String, +sep: String) -> String:
  Vd.left.go(s, String.starts_with(s, sep), sep, SNil{})

# an entry's setting key: a term by its spelling, a replacement by what is
# heard, a drop word by "!" and the word (all lower case)
def Vd.ekey.k(drop: Bool, arrow: Bool, +line: String) -> String:
  match drop arrow:
    case True{} _:
      "!" ++ Vd.phrase(String.drop(line, 1n))
    case False{} True{}:
      Vd.phrase(Vd.left(line, "->")) ++ ">"
    case False{} False{}:
      String.to_lower(String.trim(Vd.left(line, "=")))

def Vd.ekey(+line: String) -> String:
  Vd.ekey.k(String.starts_with(String.trim(line), "!"), String.contains(line, "->"), String.trim(line))

def Vd.ekey.setting(+line: String) -> String:
  Vd.e.pre() ++ Vd.ekey(line)

# the text after the first sep ("" when none)
def Vd.right(+s: String, +sep: String) -> String:
  String.drop(s, Nat.add(String.length(Vd.left(s, sep)), String.length(sep)))

# a line read: blank or a comment → none; else the line trimmed
def Vd.line.ok(+line: String) -> Bool:
  Bool.not(Bool.or(String.is_empty(String.trim(line)), String.starts_with(String.trim(line), "#")))

# Building the dictionary
# -----------------------

def Vd.empty() -> VDict:
  VDict{Map.new(&2, VR), Map.new(&2, String), Nil{}, Nil{}}

def Vd.put.exact(d: VDict, +heard: String, +out: String, auto: Bool) -> VDict:
  match d:
    case VDict{+ex, fl, fz, ts}:
      VDict{Bool.pick(Map<&2, VR>, String.is_empty(heard), ex, Map.set(&2, VR, ex, heard, VR{out, auto})), fl, fz, ts}

def Vd.put.term(d: VDict, +t: String) -> VDict:
  match d:
    case VDict{ex, fl, fz, ts}:
      VDict{ex, Map.set(&2, String, fl, Vd.flat(String.to_lower(t)), t), VF{Vd.key(Vd.flat(String.to_lower(t))), Vd.flat(String.to_lower(t)), t} <> fz, List.append(&2, String, ts, [t])}

def Vd.sounds(ss: List<&2, String>, +t: String, d: VDict) -> VDict:
  match ss:
    case Nil{}:
      d
    case +h <> rest:
      Vd.sounds(rest, t, Vd.put.exact(d, Vd.phrase(h), t, True{}))

def Vd.add.k(drop: Bool, arrow: Bool, +line: String, d: VDict) -> VDict:
  match drop arrow:
    case True{} _:
      Vd.put.exact(d, Vd.phrase(String.drop(line, 1n)), "", True{})
    case False{} True{}:
      Vd.put.exact(d, Vd.phrase(Vd.left(line, "->")), String.trim(Vd.right(line, "->")), True{})
    case False{} False{}:
      Vd.sounds(String.split(Vd.right(line, "="), ','), String.trim(Vd.left(line, "=")), Vd.put.term(d, String.trim(Vd.left(line, "="))))

# one entry line into the dictionary (blank and # lines are nothing)
def Vd.add.ok(ok: Bool, +line: String, d: VDict) -> VDict:
  match ok:
    case True{}:
      Vd.add.k(String.starts_with(String.trim(line), "!"), String.contains(line, "->"), String.trim(line), d)
    case False{}:
      d

def Vd.add(+line: String, d: VDict) -> VDict:
  Vd.add.ok(Vd.line.ok(line), line, d)

def Vd.lines(ls: List<&2, String>, d: VDict) -> VDict:
  match ls:
    case Nil{}:
      d
    case +h <> t:
      Vd.lines(t, Vd.add(h, d))

# a learned replacement: "<to>\t<confirmed>\t<undone>"
type VL is Data:
  VL{out: String, yes: Nat, no: Nat}

def Vd.num.go(s: String, +acc: Nat) -> Nat:
  match s:
    case SNil{}:
      acc
    case SCon{Chr{+c}, t}:
      Vd.num.go(t, Bool.pick(Nat, Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57)), Nat.add(Nat.mul(acc, 10n), U32.to_nat(U32.sub(c, 48))), acc))

def Vd.num(s: String) -> Nat:
  Vd.num.go(s, 0n)

def Vd.vl.of(ps: List<&2, String>) -> VL:
  match ps:
    case +o <> y <> n <> _:
      VL{o, Vd.num(y), Vd.num(n)}
    case +o <> y <> Nil{}:
      VL{o, Vd.num(y), 0n}
    case +o <> Nil{}:
      VL{o, 0n, 0n}
    case Nil{}:
      VL{"", 0n, 0n}

def Vd.vl(+v: String) -> VL:
  Vd.vl.of(String.split(v, '\t'))

def Vd.vl.show(x: VL) -> String:
  match x:
    case VL{o, y, n}:
      o ++ "\t" ++ Nat.show(y) ++ "\t" ++ Nat.show(n)

def Vd.trust() -> Nat:
  2n

# trusted: confirmed at least twice, never undone (law vd_learn_trusted)
def Vd.trusted(x: VL) -> Bool:
  match x:
    case VL{_, +y, +n}:
      Bool.and(Nat.is_ge(y, Vd.trust()), Nat.is_eq(n, 0n))

def Vd.learned.one(+heard: String, x: VL, d: VDict) -> VDict:
  match x:
    case VL{+o, +y, +n}:
      Vd.put.exact(d, heard, o, Vd.trusted(VL{o, y, n}))

# a setting's key (less its prefix) and value
type VPair is Data:
  VPair{k: String, v: String}

def Vd.learned(hs: List<&2, VPair>, d: VDict) -> VDict:
  match hs:
    case Nil{}:
      d
    case VPair{+h, +v} <> t:
      Vd.learned(t, Vd.learned.one(h, Vd.vl(v), d))

# the dictionary from entry lines and learned (heard, value) pairs; learned
# go first, so an entry the person wrote wins over one it learned
def Vd.of(+lines: List<&2, String>, +learned: List<&2, VPair>) -> VDict:
  Vd.lines(lines, Vd.learned(learned, Vd.empty()))

def Vd.terms(d: VDict) -> List<&2, String>:
  match d:
    case VDict{_, _, _, ts}:
      ts

# a learned value after the person confirmed (yes) or undid (no) it
def Vd.learn.k(fresh: Bool, x: VL, +out: String, +yes: Bool) -> VL:
  match fresh x:
    case True{} _:
      VL{out, Bool.pick(Nat, yes, 1n, 0n), Bool.pick(Nat, yes, 0n, 1n)}
    case False{} VL{o, +y, +n}:
      VL{o, Bool.pick(Nat, yes, 1n+y, y), Bool.pick(Nat, yes, n, 1n+n)}

def VL.out(x: VL) -> String:
  match x:
    case VL{o, _, _}:
      o

def VL.yes(x: VL) -> Nat:
  match x:
    case VL{_, y, _}:
      y

# the local models (the helper's MODELS): v2 unless v3 is asked for
def Vd.model.id(+m: String) -> String:
  Bool.pick(String, String.eq(m, "parakeet-v3"), "parakeet-v3", "parakeet-v2")

# "heard → out" confirmed or undone once more; a different out starts over
def Vd.learn(+old: String, +out: String, yes: Bool) -> String:
  Vd.vl.show(Vd.learn.k(Bool.or(String.is_empty(old), Bool.not(String.eq(VL.out(Vd.vl(old)), out))), Vd.vl(old), out, yes))

# The correction pass
# -------------------

# a match at the head: how many words, what to write, and how sure
# (0 by itself, 1 only as a guess)
type VM is Data:
  VM{n: Nat, out: String, ask: Bool}

def Vd.core.list(ws: List<&2, VW>, acc: List<&2, String>) -> List<&2, String>:
  match ws:
    case Nil{}:
      List.reverse(&2, String, acc)
    case VW{+w, _, _, _} <> t:
      Vd.core.list(t, Vd.norm(w) <> acc)

def Vd.surf.go(ws: List<&2, VW>, acc: List<&2, String>) -> List<&2, String>:
  match ws:
    case Nil{}:
      List.reverse(&2, String, acc)
    case VW{+w, _, _, _} <> t:
      Vd.surf.go(t, Pw.core(Vd.split(w)) <> acc)

def Vd.surface(ws: List<&2, VW>) -> String:
  String.join(Vd.surf.go(ws, Nil{}), " ")

# the least probability among words
def Vd.pmin(ws: List<&2, VW>, +acc: Nat) -> Nat:
  match ws:
    case Nil{}:
      acc
    case VW{_, +p, _, _} <> t:
      Vd.pmin(t, Bool.pick(Nat, Nat.is_lt(p, acc), p, acc))

# below this the engine was unsure enough for a sound-alike to apply by itself
def Vd.unsure_p() -> Nat:
  85n

def Vd.near.ok(+a: String, +b: String) -> Bool:
  V.Voice.near(a, b, Bool.pick(Nat, Nat.is_le(String.length(b), 5n), 1n, Nat.div(String.length(b), 3n)))

def Vd.fz.hit(same: Bool, +ff: String, +fl: String, +fo: String) -> Maybe<&2, String>:
  match same:
    case True{}:
      Bool.pick(Maybe<&2, String>, Vd.near.ok(fl, ff), Some{fo}, None{})
    case False{}:
      None{}

def Vd.fz.go(fs: List<&2, VF>, +k: String, +fl: String, found: Maybe<&2, String>) -> Maybe<&2, String>:
  match fs found:
    case _ Some{x}:
      Some{x}
    case Nil{} None{}:
      None{}
    case VF{+fk, +ff, +fo} <> t None{}:
      Vd.fz.go(t, k, fl, Vd.fz.hit(Bool.and(String.eq(fk, k), Bool.not(String.eq(ff, fl))), ff, fl, fo))

def Vd.m.fuzzy(m: Maybe<&2, String>, +n: Nat, +p: Nat) -> Maybe<&2, VM>:
  match m:
    case Some{o}:
      Some{VM{n, o, Nat.is_ge(p, Vd.unsure_p())}}
    case None{}:
      None{}

def Vd.got.s(r: Map<&2, String> & String) -> String:
  (_, v) = r
  v

def Vd.got.r(r: Map<&2, VR> & VR) -> VR:
  (_, v) = r
  v

def Vd.some(+s: String) -> Maybe<&2, String>:
  Bool.pick(Maybe<&2, String>, String.is_empty(s), None{}, Some{s})

def Vd.m.flat(m: Maybe<&2, String>, +n: Nat, +surf: String, +fl: String, +fz: List<&2, VF>, +p: Nat) -> Maybe<&2, VM>:
  match m:
    case Some{+o}:
      Bool.pick(Maybe<&2, VM>, String.eq(o, surf), None{}, Some{VM{n, o, False{}}})
    case None{}:
      Bool.pick(Maybe<&2, VM>, Nat.is_lt(String.length(fl), 3n), None{}, Vd.m.fuzzy(Vd.fz.go(fz, Vd.key(fl), fl, None{}), n, p))

def Vd.m.exact(+has: Bool, r: VR, +n: Nat, +surf: String, +fl: String, d: VDict, +p: Nat) -> Maybe<&2, VM>:
  match has r d:
    case True{} VR{+o, +a} _:
      Some{VM{n, o, Bool.not(a)}}
    case False{} _ VDict{_, +flm, +fz, _}:
      Vd.m.flat(Vd.some(Vd.got.s(Map.get(String, "", flm, fl))), n, surf, fl, fz, p)

# a rule found: a drop (auto, nothing to write) or anything to write
def Vd.found(+r: VR) -> Bool:
  match r:
    case VR{+o, +a}:
      Bool.or(a, Bool.not(String.is_empty(o)))

def Vd.try.at(+n: Nat, +ph: String, +surf: String, +p: Nat, +ex: Map<&2, VR>, +d: VDict) -> Maybe<&2, VM>:
  Vd.m.exact(Vd.found(Vd.got.r(Map.get(VR, VR{"", False{}}, ex, ph))), Vd.got.r(Map.get(VR, VR{"", False{}}, ex, ph)), n, surf, Vd.flat(ph), d, p)

def Vd.try.k(ok: Bool, +n: Nat, +ws: List<&2, VW>, +d: VDict) -> Maybe<&2, VM>:
  match ok d:
    case False{} _:
      None{}
    case True{} VDict{+ex, _, _, _}:
      Vd.try.at(n, String.join(Vd.core.list(List.take(&2, VW, ws, n), Nil{}), " "), Vd.surface(List.take(&2, VW, ws, n)), Vd.pmin(List.take(&2, VW, ws, n), 100n), ex, d)

# n words at the head, matched (None when fewer are left)
def Vd.try(+n: Nat, +ws: List<&2, VW>, +d: VDict) -> Maybe<&2, VM>:
  Vd.try.k(Nat.is_le(n, List.length(&2, VW, ws)), n, ws, d)

def Vd.first(a: Maybe<&2, VM>, b: Maybe<&2, VM>) -> Maybe<&2, VM>:
  match a:
    case Some{x}:
      Some{x}
    case None{}:
      b

# the longest phrase first
def Vd.best(+ws: List<&2, VW>, +d: VDict) -> Maybe<&2, VM>:
  Vd.first(Vd.try(3n, ws, d), Vd.first(Vd.try(2n, ws, d), Vd.try(1n, ws, d)))

# what to write, when all lower case, keeps the capital the first heard word
# had (a sentence's start); anything with a capital is written as it is
def Vd.cap.k(up: Bool, +o: String) -> String:
  match up o:
    case True{} SCon{c, t}:
      SCon{Char.to_upper(c), t}
    case _ o2:
      o2

def Vd.cap(+o: String, +first: String) -> String:
  Vd.cap.k(Bool.and(V.Voice.upper(V.Voice.first(Pw.core(Vd.split(first)))), String.eq(o, String.to_lower(o))), o)

# the corrected words so far (reversed), fixes and guesses (reversed)
type Fs is Data:
  Fs{out: List<&2, VW>, fixes: List<&2, VFx>, ask: List<&2, V.Unsure>, skip: Nat}

# a dropped word's closing punctuation goes onto the word before it
def Vd.trail.onto(out: List<&2, VW>, +tr: String) -> List<&2, VW>:
  match out:
    case Nil{}:
      Nil{}
    case VW{w, p, s, e} <> t:
      VW{w ++ tr, p, s, e} <> t

def Vd.lastw(ws: List<&2, VW>) -> VW:
  Maybe.default(&2, VW, List.last(&2, VW, ws), VW{"", 100n, 0n, 0n})

def Vd.firstw(ws: List<&2, VW>) -> VW:
  Maybe.default(&2, VW, List.head(&2, VW, ws), VW{"", 100n, 0n, 0n})

def Vd.apply.out.k(gone: Bool, +o: String, +taken: List<&2, VW>, out: List<&2, VW>, fx: List<&2, VFx>, ak: List<&2, V.Unsure>) -> Fs:
  match gone:
    case True{}:
      Fs{Vd.trail.onto(out, Pw.trail(Vd.split(VW.w(Vd.lastw(taken))))), VFx{Vd.surface(taken), ""} <> fx, ak, Nat.sub(List.length(&2, VW, taken), 1n)}
    case False{}:
      Fs{VW{Pw.lead(Vd.split(VW.w(Vd.firstw(taken)))) ++ o ++ Pw.trail(Vd.split(VW.w(Vd.lastw(taken)))), Vd.pmin(taken, 100n), VW.s(Vd.firstw(taken)), VW.e(Vd.lastw(taken))} <> out,
        VFx{Vd.surface(taken), o} <> fx, ak, Nat.sub(List.length(&2, VW, taken), 1n)}

def Vd.apply.out(+o: String, +n: Nat, +ws: List<&2, VW>, st: Fs) -> Fs:
  match st:
    case Fs{out, fx, ak, _}:
      Vd.apply.out.k(String.is_empty(o), Vd.cap(o, VW.w(Vd.firstw(ws))), List.take(&2, VW, ws, n), out, fx, ak)

def Vd.keep.words(ws: List<&2, VW>, out: List<&2, VW>) -> List<&2, VW>:
  match ws:
    case Nil{}:
      out
    case +h <> t:
      Vd.keep.words(t, h <> out)

def Vd.ask.k(+o: String, +n: Nat, +ws: List<&2, VW>, st: Fs) -> Fs:
  match st:
    case Fs{out, fx, ak, _}:
      Fs{Vd.keep.words(List.take(&2, VW, ws, n), out), fx, V.Unsure{Vd.surface(List.take(&2, VW, ws, n)), o} <> ak, Nat.sub(n, 1n)}

def Vd.step.ask(ask: Bool, +o: String, +n: Nat, +ws: List<&2, VW>, st: Fs) -> Fs:
  match ask:
    case True{}:
      Vd.ask.k(o, n, ws, st)
    case False{}:
      Vd.apply.out(o, n, ws, st)

def Vd.step.m(m: Maybe<&2, VM>, +ws: List<&2, VW>, st: Fs) -> Fs:
  match m st:
    case None{} Fs{out, fx, ak, _}:
      Fs{Vd.firstw(ws) <> out, fx, ak, 0n}
    case Some{VM{+n, +o, +ask}} st2:
      Vd.step.ask(ask, o, n, ws, st2)

# one word: still inside the last match (skipped), or matched from here
def Vd.fix.one.k(free: Bool, +ws: List<&2, VW>, +d: VDict, st: Fs) -> Fs:
  match free st:
    case True{} st2:
      Vd.step.m(Vd.best(ws, d), ws, st2)
    case False{} Fs{out, fx, ak, +k}:
      Fs{out, fx, ak, Nat.sub(k, 1n)}

def Vd.skip(+st: Fs) -> Nat:
  match st:
    case Fs{_, _, _, k}:
      k

def Vd.fix.go(ws: List<&2, VW>, +d: VDict, +st: Fs) -> Fs:
  match ws:
    case Nil{}:
      st
    case +h <> +t:
      Vd.fix.go(t, d, Vd.fix.one.k(Nat.is_eq(Vd.skip(st), 0n), h <> t, d, st))

def Vd.fix.fin(st: Fs) -> VFix:
  match st:
    case Fs{out, fx, ak, _}:
      VFix{List.reverse(&2, VW, out), List.reverse(&2, VFx, fx), List.reverse(&2, V.Unsure, ak)}

# a segment's words corrected (laws vd_fix_*)
def Vd.fix(+d: VDict, +ws: List<&2, VW>) -> VFix:
  Vd.fix.fin(Vd.fix.go(ws, d, Fs{Nil{}, Nil{}, Nil{}, 0n}))

def VFix.words(x: VFix) -> List<&2, VW>:
  match x:
    case VFix{w, _, _}:
      w

def VFix.fixes(x: VFix) -> List<&2, VFx>:
  match x:
    case VFix{_, f, _}:
      f

def VFix.ask(x: VFix) -> List<&2, V.Unsure>:
  match x:
    case VFix{_, _, a}:
      a

def Vd.wtext.go(ws: List<&2, VW>, acc: List<&2, String>) -> List<&2, String>:
  match ws:
    case Nil{}:
      List.reverse(&2, String, acc)
    case VW{+w, _, _, _} <> t:
      Vd.wtext.go(t, w <> acc)

# the words as text
def Vd.text(ws: List<&2, VW>) -> String:
  String.join(Vd.wtext.go(ws, Nil{}), " ")

# words the engine itself was unsure of (under half), as questions with no
# guess
def Vd.low_p() -> Nat:
  50n

def Vd.low.go(ws: List<&2, VW>, +acc: List<&2, V.Unsure>) -> List<&2, V.Unsure>:
  match ws:
    case Nil{}:
      List.reverse(&2, V.Unsure, acc)
    case VW{+w, +p, _, _} <> t:
      Vd.low.go(t, Bool.pick(List<&2, V.Unsure>, Nat.is_lt(p, Vd.low_p()), V.Unsure{Pw.core(Vd.split(w)), ""} <> acc, acc))

def Vd.has.ask(us: List<&2, V.Unsure>, +w: String, acc: Bool) -> Bool:
  match us:
    case Nil{}:
      acc
    case V.Unsure{+uw, _} <> t:
      Vd.has.ask(t, w, Bool.or(acc, String.eq(String.to_lower(uw), String.to_lower(w))))

def Vd.ask.merge(xs: List<&2, V.Unsure>, +ig: List<&2, String>, +acc: List<&2, V.Unsure>) -> List<&2, V.Unsure>:
  match xs:
    case Nil{}:
      List.reverse(&2, V.Unsure, acc)
    case V.Unsure{+w, +g} <> t:
      Vd.ask.merge(t, ig, Bool.pick(List<&2, V.Unsure>, Bool.or(Vd.has.ask(acc, w, False{}), V.Voice.dict.has(ig, w)), acc, V.Unsure{w, g} <> acc))

# what to ask about after a segment: the guesses the pass offered, then the
# words the engine was unsure of, then today's near misses and technical
# words; each once, at most five
def Vd.asks(+d: VDict, +ig: List<&2, String>, +fx: VFix) -> List<&2, V.Unsure>:
  V.Voice.cap(V.Unsure, V.Voice.max_unsure(),
    Vd.ask.merge(List.append(&2, V.Unsure, VFix.ask(fx), List.append(&2, V.Unsure, Vd.low.go(VFix.words(fx), Nil{}), V.Voice.unsure(Vd.terms(d), ig, Vd.text(VFix.words(fx))))), ig, Nil{}))

# Words from the helper
# ---------------------

def Vd.jnum(+j: J.Json) -> Nat:
  match j:
    case J.Num{+r}:
      Vd.num(r)
    case _:
      0n

def Vd.jw.of(xs: List<&2, J.Json>) -> Maybe<&2, VW>:
  match xs:
    case J.Str{+w} <> s <> e <> p <> _:
      Some{VW{w, Vd.jnum(p), Vd.jnum(s), Vd.jnum(e)}}
    case _:
      None{}

def Vd.jw(+j: J.Json) -> Maybe<&2, VW>:
  Vd.jw.of(J.Json.elems(j))

def Vd.jws.put(m: Maybe<&2, VW>, acc: List<&2, VW>) -> List<&2, VW>:
  match m:
    case Some{w}:
      w <> acc
    case None{}:
      acc

def Vd.jws.go(js: List<&2, J.Json>, acc: List<&2, VW>) -> List<&2, VW>:
  match js:
    case Nil{}:
      List.reverse(&2, VW, acc)
    case +h <> t:
      Vd.jws.go(t, Vd.jws.put(Vd.jw(h), acc))

# [[word, start cs, end cs, percent], ...]
def Vd.words.json(+j: J.Json) -> List<&2, VW>:
  Vd.jws.go(J.Json.elems(j), Nil{})

# a text with no timings (the cloud's): its words split at spaces
def Vd.words.text.go(ws: List<&2, String>, +acc: List<&2, VW>) -> List<&2, VW>:
  match ws:
    case Nil{}:
      List.reverse(&2, VW, acc)
    case +h <> t:
      Vd.words.text.go(t, Bool.pick(List<&2, VW>, String.is_empty(h), acc, VW{h, 100n, 0n, 0n} <> acc))

def Vd.words.text(+s: String) -> List<&2, VW>:
  Vd.words.text.go(String.split(String.trim(s), ' '), Nil{})

def Vd.fixes.js(fs: List<&2, VFx>) -> List<&2, J.Json>:
  match fs:
    case Nil{}:
      Nil{}
    case VFx{h, o} <> t:
      J.Json.obj([J.KV{"heard", J.Str{h}}, J.KV{"out", J.Str{o}}]) <> Vd.fixes.js(t)

def Vd.fixes.json(fs: List<&2, VFx>) -> J.Json:
  J.Json.arr(Vd.fixes.js(fs))

# The dictionary as a request carries it (bothub.bend's Vreq): "entries",
# the lines; "learned", [heard, value] pairs
def Vd.js.lines(xs: List<&2, J.Json>, acc: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      List.reverse(&2, String, acc)
    case J.Str{+t} <> rest:
      Vd.js.lines(rest, t <> acc)
    case _ <> rest:
      Vd.js.lines(rest, acc)

def Vd.js.pair(xs: List<&2, J.Json>) -> Maybe<&2, VPair>:
  match xs:
    case J.Str{+k} <> J.Str{+v} <> _:
      Some{VPair{k, v}}
    case _:
      None{}

def Vd.js.put(m: Maybe<&2, VPair>, acc: List<&2, VPair>) -> List<&2, VPair>:
  match m:
    case Some{x}:
      x <> acc
    case None{}:
      acc

def Vd.js.pairs(xs: List<&2, J.Json>, acc: List<&2, VPair>) -> List<&2, VPair>:
  match xs:
    case Nil{}:
      List.reverse(&2, VPair, acc)
    case +h <> rest:
      Vd.js.pairs(rest, Vd.js.put(Vd.js.pair(J.Json.elems(h)), acc))

def Vd.of.json(+p: J.Json) -> VDict:
  Vd.of(Vd.js.lines(J.Json.elems(J.Json.get_obj(p, "entries")), Nil{}), Vd.js.pairs(J.Json.elems(J.Json.get_obj(p, "learned")), Nil{}))

# Settle: live words that never change
# ------------------------------------

# the segment's item, where its committed words end (cs; timed engines),
# how many words are committed (untimed), the last committed word, the
# last hypothesis
type VSet is Data:
  VSet{item: String, cut: Nat, n: Nat, last: String, prev: List<&2, VW>}

# what a hypothesis gives: words to commit now, the tail, the state after
type VStep is Data:
  VStep{commit: List<&2, VW>, tail: List<&2, VW>, st: VSet}

def Vs.empty() -> VSet:
  VSet{"", 0n, 0n, "", Nil{}}

def VStep.commit(x: VStep) -> List<&2, VW>:
  match x:
    case VStep{c, _, _}:
      c

def VStep.tail(x: VStep) -> List<&2, VW>:
  match x:
    case VStep{_, t, _}:
      t

def VStep.st(x: VStep) -> VSet:
  match x:
    case VStep{_, _, s}:
      s

# far enough from the end of what was heard to be settled (cs)
def Vs.margin() -> Nat:
  40n

def Vs.mid(+x: VW) -> Nat:
  Nat.div(Nat.add(VW.s(x), VW.e(x)), 2n)

# words whose middle is at or after cut
def Vs.after.go(ws: List<&2, VW>, +cut: Nat, +acc: List<&2, VW>) -> List<&2, VW>:
  match ws:
    case Nil{}:
      List.reverse(&2, VW, acc)
    case +h <> t:
      Vs.after.go(t, cut, Bool.pick(List<&2, VW>, Nat.is_ge(Vs.mid(h), cut), h <> acc, acc))

# a word committed last, heard again a little later, is not new
def Vs.dedupe(ws: List<&2, VW>, +last: String, +cut: Nat) -> List<&2, VW>:
  match ws:
    case +h <> +t:
      Bool.pick(List<&2, VW>, Bool.and(Bool.and(Bool.not(String.is_empty(last)), String.eq(Vd.norm(VW.w(h)), last)), Nat.is_lt(VW.s(h), Nat.add(cut, 10n))), t, h <> t)
    case Nil{}:
      Nil{}

# the words not yet committed
def Vs.tail(timed: Bool, +st: VSet, +ws: List<&2, VW>) -> List<&2, VW>:
  match timed st:
    case True{} VSet{_, +cut, _, +last, _}:
      Vs.dedupe(Vs.after.go(ws, cut, Nil{}), last, cut)
    case False{} VSet{_, _, +n, _, _}:
      List.drop(&2, VW, ws, n)

# how many words at the head of two lists are the same
def Vs.heads(+a: List<&2, VW>, +b: List<&2, VW>) -> Bool:
  match a b:
    case +x <> _ +y <> _:
      String.eq(VW.w(x), VW.w(y))
    case _ _:
      False{}

def Vs.agree.go(a: List<&2, VW>, b: List<&2, VW>, same: Bool, +acc: Nat) -> Nat:
  match a b same:
    case _ <> +at _ <> +bt True{}:
      Vs.agree.go(at, bt, Vs.heads(at, bt), 1n+acc)
    case _ _ _:
      acc

def Vs.agree(+a: List<&2, VW>, +b: List<&2, VW>, +acc: Nat) -> Nat:
  Vs.agree.go(a, b, Vs.heads(a, b), acc)

# how many words at the head end before limit (cs)
def Vs.before(ws: List<&2, VW>, +limit: Nat, +acc: Nat) -> Nat:
  match ws:
    case Nil{}:
      acc
    case +h <> t:
      Bool.pick(Nat, Nat.is_le(VW.e(h), limit), Vs.before(t, limit, 1n+acc), acc)

# the first word is one the engine was unsure of
def Vs.low(+rs: List<&2, VW>) -> Bool:
  match rs:
    case VW{_, +p, _, _} <> _:
      Nat.is_lt(p, Vd.unsure_p())
    case Nil{}:
      False{}

def Vs.hold.go(rs: List<&2, VW>, +k: Nat, drop: Bool) -> Nat:
  match rs drop:
    case _ <> +t True{}:
      Vs.hold.go(t, Nat.sub(k, 1n), Vs.low(t))
    case _ _:
      k

# unsure words at the end of what would be committed wait for the next
# ones, so a term heard as two words ("Kai CAD") is corrected as one
def Vs.hold(+k: Nat, +ws: List<&2, VW>) -> Nat:
  Vs.hold.go(List.reverse(&2, VW, List.take(&2, VW, ws, k)), k, Vs.low(List.reverse(&2, VW, List.take(&2, VW, ws, k))))

def Vs.min(+a: Nat, +b: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_le(a, b), a, b)

# how many tail words to commit: agreed with the last hypothesis, never the
# last word (two for an untimed engine), settled before the end, then held
# back over unsure ones
def Vs.count(timed: Bool, +tail: List<&2, VW>, +prev: List<&2, VW>, +end: Nat) -> Nat:
  Vs.hold(Vs.min(Vs.agree(tail, prev, 0n), Bool.pick(Nat, timed, Vs.min(Nat.sub(List.length(&2, VW, tail), 1n), Vs.before(tail, Nat.sub(end, Vs.margin()), 0n)),
    Nat.sub(List.length(&2, VW, tail), 2n))), tail)

def Vs.next.k(none: Bool, timed: Bool, +c: List<&2, VW>, +rest: List<&2, VW>, +ws: List<&2, VW>, +item: String, +cut: Nat, +n: Nat, +last: String) -> VStep:
  match none:
    case True{}:
      VStep{Nil{}, rest, VSet{item, cut, n, last, ws}}
    case False{}:
      VStep{c, rest, VSet{item, Bool.pick(Nat, timed, VW.e(Vd.lastw(c)), cut), Nat.add(n, List.length(&2, VW, c)), Vd.norm(VW.w(Vd.lastw(c))), ws}}

def Vs.next(timed: Bool, +k: Nat, +tail: List<&2, VW>, +ws: List<&2, VW>, st: VSet) -> VStep:
  match st:
    case VSet{+item, +cut, +n, +last, _}:
      Vs.next.k(Nat.is_eq(k, 0n), timed, List.take(&2, VW, tail, k), List.drop(&2, VW, tail, k), ws, item, cut, n, last)

def Vs.prev(+st: VSet) -> List<&2, VW>:
  match st:
    case VSet{_, _, _, _, p}:
      p

def Vs.item(+st: VSet) -> String:
  match st:
    case VSet{i, _, _, _, _}:
      i

# a state for item: kept for the same item, else fresh
def Vs.for(+st: VSet, +item: String) -> VSet:
  Bool.pick(VSet, String.eq(Vs.item(st), item), st, VSet{item, 0n, 0n, "", Nil{}})

def Vs.hyp.k(+timed: Bool, +s: VSet, +ws: List<&2, VW>, +end: Nat) -> VStep:
  Vs.next(timed, Vs.count(timed, Vs.tail(timed, s, ws), Vs.tail(timed, s, Vs.prev(s)), end), Vs.tail(timed, s, ws), ws, s)

# a new hypothesis of the open segment (end: how much audio it covers, cs)
def Vs.hyp(timed: Bool, +st: VSet, +item: String, +ws: List<&2, VW>, +end: Nat) -> VStep:
  Vs.hyp.k(timed, Vs.for(st, item), ws, end)

# the segment's last hypothesis: whatever is not committed is, now
def Vs.done(timed: Bool, +st: VSet, +item: String, +ws: List<&2, VW>) -> VStep:
  VStep{Vs.tail(timed, Vs.for(st, item), ws), Nil{}, Vs.empty()}
