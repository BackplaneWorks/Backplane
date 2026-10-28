import Base
import ./json.bend as J

# Dictation. The user keeps a dictionary of terms, sent to the transcriber
# as keywords. The live model gives no confidence scores, so after each
# transcript the words that look misheard are offered back to the user to
# correct or add to the dictionary: a near miss of a dictionary term (one
# word, or two heard as one term), or a technical-looking word nobody taught
# it. Walks over the transcript are tail calls (texts run long).

# a word to ask about; guess "" when there is no suggestion
type Unsure is Data:
  Unsure{word: String, guess: String}

def Voice.max_keywords() -> Nat:
  100n

def Voice.max_unsure() -> Nat:
  5n

# the first n items (laws voice_*_bounded)
def Voice.cap(-A: Data, n: Nat, xs: List<&2, A>) -> List<&2, A>:
  match n xs:
    case 0n _:
      Nil{}
    case 1n+p Nil{}:
      Nil{}
    case 1n+p +h <> t:
      h <> Voice.cap(A, p, t)

# Characters
# ----------

# a letter or digit; non-ASCII counts as a letter, except punctuation and
# symbols (U+2000..U+2BFF)
def Voice.alnum(+c: U32) -> Bool:
  Bool.or(Bool.or(Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57)), Bool.and(U32.is_ge(c, 65), U32.is_le(c, 90))),
    Bool.or(Bool.and(U32.is_ge(c, 97), U32.is_le(c, 122)), Bool.and(U32.is_ge(c, 192), Bool.not(Bool.and(U32.is_ge(c, 8192), U32.is_le(c, 11263))))))

# - _ . ' and ’ join letters inside a word
def Voice.joiner(+c: U32) -> Bool:
  Bool.or(Bool.or(U32.is_eq(c, 45), U32.is_eq(c, 95)), Bool.or(Bool.or(U32.is_eq(c, 46), U32.is_eq(c, 39)), U32.is_eq(c, 8217)))

def Voice.wordc(c: Char) -> Bool:
  match c:
    case Chr{+x}:
      Voice.alnum(x)

# the form terms are compared in: trimmed, lower case
def Voice.key(s: String) -> String:
  String.to_lower(String.trim(s))

# Dictionary
# ----------

# entries are kept trimmed, so only the asked term is trimmed
def Voice.dict.has.go(ds: List<&2, String>, +k: String, acc: Bool) -> Bool:
  match ds:
    case Nil{}:
      acc
    case h <> t:
      Voice.dict.has.go(t, k, Bool.or(acc, String.eq(String.to_lower(h), k)))

# the term is in the dictionary (any case)
def Voice.dict.has(ds: List<&2, String>, term: String) -> Bool:
  Voice.dict.has.go(ds, Voice.key(term), False{})

def Voice.dict.add.k(had: Bool, blank: Bool, ds: List<&2, String>, t: String) -> List<&2, String>:
  match had blank:
    case True{} _:
      ds
    case False{} True{}:
      ds
    case False{} False{}:
      List.append(&2, String, ds, [t])

# the trimmed term appended, unless blank or already there (laws voice_add_*)
def Voice.dict.add(+ds: List<&2, String>, +term: String) -> List<&2, String>:
  Voice.dict.add.k(Voice.dict.has(ds, term), String.is_empty(String.trim(term)), ds, String.trim(term))

def Voice.dict.keep(drop: Bool, h: String, acc: List<&2, String>) -> List<&2, String>:
  match drop:
    case True{}:
      acc
    case False{}:
      h <> acc

def Voice.dict.remove.go(ds: List<&2, String>, +k: String, acc: List<&2, String>) -> List<&2, String>:
  match ds:
    case Nil{}:
      List.reverse(&2, String, acc)
    case +h <> t:
      Voice.dict.remove.go(t, k, Voice.dict.keep(String.eq(String.to_lower(h), k), h, acc))

# every entry equal to the term (any case) dropped
def Voice.dict.remove(ds: List<&2, String>, term: String) -> List<&2, String>:
  Voice.dict.remove.go(ds, Voice.key(term), Nil{})

# the text being split into lines, and the lines so far (reversed)
type Ln is Data:
  Ln{cur: String, out: List<&2, String>}

def Voice.lines.step(nl: Bool, c: Char, st: Ln) -> Ln:
  match nl st:
    case True{} Ln{cur, out}:
      Ln{SNil{}, String.reverse(cur) <> out}
    case False{} Ln{cur, out}:
      Ln{SCon{c, cur}, out}

def Voice.lines.fin(st: Ln) -> List<&2, String>:
  match st:
    case Ln{cur, out}:
      List.reverse(&2, String, String.reverse(cur) <> out)

def Voice.lines.go(s: String, st: Ln) -> List<&2, String>:
  match s:
    case SNil{}:
      Voice.lines.fin(st)
    case SCon{+c, t}:
      Voice.lines.go(t, Voice.lines.step(Char.is_eq(c, '\n'), c, st))

def Voice.dict.put.k(skip: Bool, t: String, acc: List<&2, String>) -> List<&2, String>:
  match skip:
    case True{}:
      acc
    case False{}:
      t <> acc

def Voice.dict.put(+t: String, +acc: List<&2, String>) -> List<&2, String>:
  Voice.dict.put.k(Bool.or(String.is_empty(t), Voice.dict.has(acc, t)), t, acc)

def Voice.dict.dedupe(xs: List<&2, String>, acc: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      List.reverse(&2, String, acc)
    case h <> t:
      Voice.dict.dedupe(t, Voice.dict.put(String.trim(h), acc))

# one term per line (\r\n too), trimmed, blanks and repeats (any case) dropped
def Voice.dict.parse(text: String) -> List<&2, String>:
  Voice.dict.dedupe(Voice.lines.go(text, Ln{SNil{}, Nil{}}), Nil{})

def Voice.dict.show(ds: List<&2, String>) -> String:
  String.join(ds, "\n")

# what the transcriber is sent: the first max_keywords terms
def Voice.keywords(ds: List<&2, String>) -> List<&2, String>:
  Voice.cap(String, Voice.max_keywords(), ds)

# Edit distance
# -------------

def Voice.min(+a: Nat, +b: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_le(a, b), a, b)

# one row of the table: ps is the row above, diag and left its neighbours
def Voice.lev.row(bs: String, ps: List<&2, Nat>, +ca: U32, +diag: Nat, +left: Nat, acc: List<&2, Nat>) -> List<&2, Nat>:
  match bs ps:
    case SNil{} _:
      List.reverse(&2, Nat, acc)
    case SCon{_, _} Nil{}:
      List.reverse(&2, Nat, acc)
    case SCon{Chr{+cb}, bt} +up <> pt:
      +v = Voice.min(Voice.min(Nat.add(diag, Bool.pick(Nat, U32.is_eq(ca, cb), 0n, 1n)), 1n+up), 1n+left)
      Voice.lev.row(bt, pt, ca, up, v, v <> acc)

def Voice.lev.next(+bs: String, +ca: U32, row: List<&2, Nat>) -> List<&2, Nat>:
  match row:
    case Nil{}:
      Nil{}
    case +p0 <> pt:
      Voice.lev.row(bs, pt, ca, p0, 1n+p0, [1n+p0])

def Voice.lev.go(as: String, +bs: String, row: List<&2, Nat>) -> List<&2, Nat>:
  match as:
    case SNil{}:
      row
    case SCon{Chr{+ca}, at}:
      Voice.lev.go(at, bs, Voice.lev.next(bs, ca, row))

# Levenshtein distance (exact; callers bound the lengths first)
def Voice.lev(a: String, +b: String) -> Nat:
  Maybe.default(&2, Nat, List.last(&2, Nat, Voice.lev.go(a, b, List.range(1n+String.length(b)))), 0n)

def Voice.near.k(ok: Bool, a: String, +b: String, +k: Nat) -> Bool:
  match ok:
    case True{}:
      Nat.is_le(Voice.lev(a, b), k)
    case False{}:
      False{}

# within k edits; strings whose lengths differ by more are never walked
def Voice.near(+a: String, +b: String, +k: Nat) -> Bool:
  Voice.near.k(Bool.and(Nat.is_le(Nat.sub(String.length(a), String.length(b)), k), Nat.is_le(Nat.sub(String.length(b), String.length(a)), k)), a, b, k)

# how many edits a term tolerates: 1 up to 5 characters, else 2
def Voice.bound(+t: String) -> Nat:
  Bool.pick(Nat, Nat.is_le(String.length(t), 5n), 1n, 2n)

# Dictionary terms, prepared
# --------------------------

# a term, its lower form, its lower form with camel humps spaced
# ("KiCad" → "ki cad"), that without spaces, and whether it has two parts
type Dt is Data:
  Dt{term: String, low: String, spaced: String, flat: String, multi: Bool}

type Cm is Data:
  Cm{lower: Bool, acc: String}

def Voice.camel.step(up: Bool, lo: Bool, c: Char, st: Cm) -> Cm:
  match up st:
    case True{} Cm{True{}, acc}:
      Cm{lo, SCon{c, SCon{' ', acc}}}
    case _ Cm{_, acc}:
      Cm{lo, SCon{c, acc}}

def Voice.camel.fin(st: Cm) -> String:
  match st:
    case Cm{_, acc}:
      String.reverse(acc)

def Voice.camel.go(s: String, st: Cm) -> String:
  match s:
    case SNil{}:
      Voice.camel.fin(st)
    case SCon{+c, t}:
      Voice.camel.go(t, Voice.camel.step(Char.is_upper(c), Char.is_lower(c), c, st))

# a space before each capital that follows a lower-case letter
def Voice.camel(s: String) -> String:
  Voice.camel.go(s, Cm{False{}, SNil{}})

def Voice.keepc(drop: Bool, c: Char, acc: String) -> String:
  match drop:
    case True{}:
      acc
    case False{}:
      SCon{c, acc}

def Voice.unspace.go(s: String, acc: String) -> String:
  match s:
    case SNil{}:
      String.reverse(acc)
    case SCon{+c, t}:
      Voice.unspace.go(t, Voice.keepc(Char.is_eq(c, ' '), c, acc))

def Voice.unspace(s: String) -> String:
  Voice.unspace.go(s, SNil{})

def Voice.dt(+d: String) -> Dt:
  +sp = String.to_lower(Voice.camel(d))
  Dt{d, String.to_lower(d), sp, Voice.unspace(sp), String.contains(sp, " ")}

def Voice.dts(ds: List<&2, String>) -> List<&2, Dt>:
  match ds:
    case Nil{}:
      Nil{}
    case h <> t:
      Voice.dt(h) <> Voice.dts(t)

# Words of a transcript
# ---------------------

# a word as it appeared, and whether exactly one space separates it from
# the word before (only such pairs are read as one term)
type Tok is Data:
  Tok{w: String, near: Bool}

# a character: part of a word, a joiner inside one, or neither
type Kd is Data:
  KW{}
  KJ{}
  KO{}

def Voice.kind.k(w: Bool, j: Bool) -> Kd:
  match w j:
    case True{} _:
      KW{}
    case False{} True{}:
      KJ{}
    case False{} False{}:
      KO{}

def Voice.kind(+c: U32) -> Kd:
  Voice.kind.k(Voice.alnum(c), Voice.joiner(c))

# the word so far (reversed), joiners after it that wait for a letter
# (reversed), whether the gap so far is one space, the word's own `near`,
# and the words done (reversed)
type Tz is Data:
  Tz{cur: String, pend: String, sp: Bool, near: Bool, out: List<&2, Tok>}

def Voice.tz.step(k: Kd, c: Char, st: Tz) -> Tz:
  match k st:
    case KW{} Tz{SNil{}, p, sp, nr, out}:
      Tz{SCon{c, SNil{}}, SNil{}, False{}, sp, out}
    case KW{} Tz{SCon{h, t}, p, sp, nr, out}:
      Tz{SCon{c, String.append(p, SCon{h, t})}, SNil{}, sp, nr, out}
    case KJ{} Tz{SNil{}, p, sp, nr, out}:
      Tz{SNil{}, p, False{}, nr, out}
    case KJ{} Tz{SCon{h, t}, p, sp, nr, out}:
      Tz{SCon{h, t}, SCon{c, p}, sp, nr, out}
    case KO{} Tz{SNil{}, p, sp, nr, out}:
      Tz{SNil{}, p, False{}, nr, out}
    case KO{} Tz{SCon{h, t}, p, sp, nr, out}:
      Tz{SNil{}, SNil{}, Bool.and(String.is_empty(p), Char.is_eq(c, ' ')), False{}, Tok{String.reverse(SCon{h, t}), nr} <> out}

def Voice.tz.fin(st: Tz) -> List<&2, Tok>:
  match st:
    case Tz{SNil{}, p, sp, nr, out}:
      List.reverse(&2, Tok, out)
    case Tz{SCon{h, t}, p, sp, nr, out}:
      List.reverse(&2, Tok, Tok{String.reverse(SCon{h, t}), nr} <> out)

def Voice.tz.go(s: String, st: Tz) -> List<&2, Tok>:
  match s:
    case SNil{}:
      Voice.tz.fin(st)
    case SCon{Chr{+x}, t}:
      Voice.tz.go(t, Voice.tz.step(Voice.kind(x), Chr{x}, st))

# runs of letters and digits, with - _ . ' inside them (not at the ends)
def Voice.words(text: String) -> List<&2, Tok>:
  Voice.tz.go(text, Tz{SNil{}, SNil{}, False{}, False{}, Nil{}})

# What a word looks like
# ----------------------

# has letters, digits, how many capitals, lower case, a capital right
# after a lower-case letter, and whether the last character was lower
type Sh is Data:
  Sh{alpha: Bool, digit: Bool, ups: Nat, lower: Bool, hump: Bool, prev: Bool}

def Voice.sh.step(+u: Bool, +l: Bool, +d: Bool, a: Bool, st: Sh) -> Sh:
  match st:
    case Sh{al, di, n, lo, hu, pv}:
      Sh{Bool.or(al, Bool.and(a, Bool.not(d))), Bool.or(di, d), Nat.add(n, Bool.pick(Nat, u, 1n, 0n)), Bool.or(lo, l), Bool.or(hu, Bool.and(u, pv)), l}

def Voice.sh.go(s: String, st: Sh) -> Sh:
  match s:
    case SNil{}:
      st
    case SCon{+c, t}:
      Voice.sh.go(t, Voice.sh.step(Char.is_upper(c), Char.is_lower(c), Char.is_digit(c), Voice.wordc(c), st))

# leading digits counted, then the rest (reversed)
type Ld is Data:
  Ld{on: Bool, n: Nat, rest: String}

def Voice.ld.step(d: Bool, c: Char, st: Ld) -> Ld:
  match d st:
    case True{} Ld{True{}, n, r}:
      Ld{True{}, 1n+n, r}
    case d2 Ld{on, n, r}:
      Ld{False{}, n, SCon{c, r}}

def Voice.ld.go(s: String, st: Ld) -> Ld:
  match s:
    case SNil{}:
      st
    case SCon{+c, t}:
      Voice.ld.go(t, Voice.ld.step(Char.is_digit(c), c, st))

def Voice.units() -> List<&2, String>:
  ["st", "nd", "rd", "th", "s", "am", "pm", "k"]

# capitals that are ordinary words
def Voice.stop() -> List<&2, String>:
  ["I", "A", "OK", "AM", "PM", "US", "UK", "TV", "ID", "PC", "AI", "OS", "FAQ", "FYI", "CEO", "ASAP", "ETA", "BTW", "LOL", "OMG"]

def Voice.unit.k(st: Ld) -> Bool:
  match st:
    case Ld{on, 0n, r}:
      False{}
    case Ld{on, 1n+p, r}:
      Voice.dict.has(Voice.units(), String.reverse(r))

# a number with an ordinary suffix: 2nd, 1990s, 5pm, 10k
def Voice.unit(w: String) -> Bool:
  Voice.unit.k(Voice.ld.go(w, Ld{True{}, 0n, SNil{}}))

def Voice.tech.k(st: Sh, +w: String) -> Bool:
  match st:
    case Sh{al, di, +n, lo, hu, pv}:
      Bool.or(Bool.or(Bool.and(Bool.and(al, di), Bool.not(Voice.unit(w))),
        Bool.and(Bool.and(Nat.is_ge(n, 2n), Bool.not(lo)), Bool.not(Voice.dict.has(Voice.stop(), w)))), hu)

# looks technical: letters with digits (stm32f4, R12), all capitals
# (not a stop word), or a capital inside a word (iPhone)
def Voice.tech(+w: String) -> Bool:
  Voice.tech.k(Voice.sh.go(w, Sh{False{}, False{}, 0n, False{}, False{}, False{}}), w)

# Near misses
# -----------

def Voice.hit(b: Bool, t: String) -> Maybe<&2, String>:
  match b:
    case True{}:
      Some{t}
    case False{}:
      None{}

def Voice.one.step(found: Maybe<&2, String>, +w: String, d: Dt) -> Maybe<&2, String>:
  match found d:
    case Some{g} d2:
      Some{g}
    case None{} Dt{term, +lo, sp, fl, mu}:
      Voice.hit(Bool.and(Bool.not(String.eq(w, lo)), Voice.near(w, lo, Voice.bound(lo))), term)

# the first term one word (lower case) is a near miss of
def Voice.one.go(ds: List<&2, Dt>, +w: String, found: Maybe<&2, String>) -> Maybe<&2, String>:
  match ds:
    case Nil{}:
      found
    case h <> t:
      Voice.one.go(t, w, Voice.one.step(found, w, h))

# two words against a two-part term: exactly it, a near miss of it (word,
# term), or neither
type Pr is Data:
  PNo{}
  PIn{}
  PGuess{word: String, term: String}

def Voice.pair.k(same: Bool, hit: Bool, wd: String, term: String) -> Pr:
  match same hit:
    case True{} _:
      PIn{}
    case False{} True{}:
      PGuess{wd, term}
    case False{} False{}:
      PNo{}

def Voice.pair.step(r: Pr, +wd: String, +sp: String, +fl: String, d: Dt) -> Pr:
  match r d:
    case PIn{} d2:
      PIn{}
    case PGuess{w, g} d2:
      PGuess{w, g}
    case PNo{} Dt{term, +lo, +dsp, +dfl, mu}:
      Voice.pair.k(String.eq(sp, lo), Bool.and(mu, Bool.or(Voice.near(sp, dsp, Voice.bound(dsp)), Voice.near(fl, dfl, Voice.bound(dfl)))), wd, term)

def Voice.pair.go(ds: List<&2, Dt>, +wd: String, +sp: String, +fl: String, r: Pr) -> Pr:
  match ds:
    case Nil{}:
      r
    case h <> t:
      Voice.pair.go(t, wd, sp, fl, Voice.pair.step(r, wd, sp, fl, h))

# a word and the next one, when one space apart ("key cad" ~ KiCad)
def Voice.pair(+a: String, nx: Maybe<&2, Tok>, +dts: List<&2, Dt>) -> Pr:
  match nx:
    case Some{Tok{+b, True{}}}:
      Voice.pair.go(dts, a ++ " " ++ b, String.to_lower(a ++ " " ++ b), String.to_lower(a ++ b), PNo{})
    case _:
      PNo{}

def Voice.one.if(long: Bool, +a: String, +dts: List<&2, Dt>) -> Maybe<&2, String>:
  match long:
    case True{}:
      Voice.one.go(dts, String.to_lower(a), None{})
    case False{}:
      None{}

def Voice.single.k(m: Maybe<&2, String>, tech: Bool) -> Maybe<&2, String>:
  match m tech:
    case Some{g} t2:
      Some{g}
    case None{} True{}:
      Some{""}
    case None{} False{}:
      None{}

# one word: the term it nearly is (3+ characters), else "" when it looks
# technical, else nothing to ask
def Voice.single(+a: String, +dts: List<&2, Dt>) -> Maybe<&2, String>:
  Voice.single.k(Voice.one.if(Nat.is_ge(String.length(a), 3n), a, dts), Voice.tech(a))

# Clarifications
# --------------

# whether to skip the next word (read as a pair), what was found
# (reversed), and how many
type Us is Data:
  Us{skip: Bool, found: List<&2, Unsure>, n: Nat}

def Voice.un.seen(f: List<&2, Unsure>, +k: String, acc: Bool) -> Bool:
  match f:
    case Nil{}:
      acc
    case Unsure{w, g} <> t:
      Voice.un.seen(t, k, Bool.or(acc, String.eq(String.to_lower(w), k)))

def Voice.un.add.k(drop: Bool, skip: Bool, u: Unsure, f: List<&2, Unsure>, n: Nat) -> Us:
  match drop:
    case True{}:
      Us{skip, f, n}
    case False{}:
      Us{skip, u <> f, 1n+n}

# kept unless in the dictionary, ignored, or already asked (any case)
def Voice.un.add(skip: Bool, u: Unsure, +ds: List<&2, String>, +ig: List<&2, String>, +f: List<&2, Unsure>, n: Nat) -> Us:
  match u:
    case Unsure{+w, g}:
      Voice.un.add.k(Bool.or(Bool.or(Voice.dict.has(ds, w), Voice.dict.has(ig, w)), Voice.un.seen(f, Voice.key(w), False{})), skip, Unsure{w, g}, f, n)

def Voice.un.one(r: Maybe<&2, String>, +a: String, +ds: List<&2, String>, +ig: List<&2, String>, f: List<&2, Unsure>, n: Nat) -> Us:
  match r:
    case None{}:
      Us{False{}, f, n}
    case Some{g}:
      Voice.un.add(False{}, Unsure{a, g}, ds, ig, f, n)

def Voice.un.pair(pr: Pr, +a: String, +dts: List<&2, Dt>, +ds: List<&2, String>, +ig: List<&2, String>, f: List<&2, Unsure>, n: Nat) -> Us:
  match pr:
    case PIn{}:
      Us{True{}, f, n}
    case PGuess{w, g}:
      Voice.un.add(True{}, Unsure{w, g}, ds, ig, f, n)
    case PNo{}:
      Voice.un.one(Voice.single(a, dts), a, ds, ig, f, n)

def Voice.un.room(room: Bool, +a: String, nx: Maybe<&2, Tok>, +dts: List<&2, Dt>, +ds: List<&2, String>, +ig: List<&2, String>, f: List<&2, Unsure>, n: Nat) -> Us:
  match room:
    case True{}:
      Voice.un.pair(Voice.pair(a, nx, dts), a, dts, ds, ig, f, n)
    case False{}:
      Us{False{}, f, n}

def Voice.un.step(st: Us, h: Tok, nx: Maybe<&2, Tok>, +dts: List<&2, Dt>, +ds: List<&2, String>, +ig: List<&2, String>) -> Us:
  match st h:
    case Us{True{}, f, n} h2:
      Us{False{}, f, n}
    case Us{False{}, f, +n} Tok{+a, nr}:
      Voice.un.room(Nat.is_lt(n, Voice.max_unsure()), a, nx, dts, ds, ig, f, n)

def Voice.un.fin(st: Us) -> List<&2, Unsure>:
  match st:
    case Us{s, f, n}:
      List.reverse(&2, Unsure, f)

def Voice.un.go(ts: List<&2, Tok>, +dts: List<&2, Dt>, +ds: List<&2, String>, +ig: List<&2, String>, st: Us) -> List<&2, Unsure>:
  match ts:
    case Nil{}:
      Voice.un.fin(st)
    case +h <> +t:
      Voice.un.go(t, dts, ds, ig, Voice.un.step(st, h, List.head(&2, Tok, t), dts, ds, ig))

# the words of a transcript to ask about, in order, at most max_unsure,
# never one in the dictionary or ignored (law voice_unsure_bounded)
def Voice.unsure(+ds: List<&2, String>, +ig: List<&2, String>, text: String) -> List<&2, Unsure>:
  Voice.cap(Unsure, Voice.max_unsure(), Voice.un.go(Voice.words(text), Voice.dts(ds), ds, ig, Us{False{}, Nil{}, 0n}))

# Corrections
# -----------

# characters still to skip (inside a replaced word), whether the last one
# was a letter or digit, the text so far (reversed)
type Fx is Data:
  Fx{skip: Nat, prev: Bool, acc: String}

def Voice.fix.head(s: String) -> Bool:
  match s:
    case SNil{}:
      False{}
    case SCon{c, t}:
      Voice.wordc(c)

# the word starts here, and neither side touches a letter or digit
def Voice.fix.at(pv: Bool, +s: String, +w: String, +lw: Nat) -> Bool:
  Bool.and(Bool.not(pv), Bool.and(String.starts_with(s, w), Bool.not(Voice.fix.head(String.drop(s, lw)))))

def Voice.fix.hit(hit: Bool, +c: Char, +r: String, +lw: Nat, acc: String) -> Fx:
  match hit:
    case True{}:
      Fx{Nat.sub(lw, 1n), Voice.wordc(c), String.reverse.go(r, acc)}
    case False{}:
      Fx{0n, Voice.wordc(c), SCon{c, acc}}

def Voice.fix.step(st: Fx, +c: Char, +s: String, +w: String, +r: String, +lw: Nat) -> Fx:
  match st:
    case Fx{1n+k, pv, acc}:
      Fx{k, Voice.wordc(c), acc}
    case Fx{0n, pv, acc}:
      Voice.fix.hit(Voice.fix.at(pv, s, w, lw), c, r, lw, acc)

def Voice.fix.fin(st: Fx) -> String:
  match st:
    case Fx{k, pv, acc}:
      String.reverse(acc)

def Voice.fix.go(s: String, +w: String, +r: String, +lw: Nat, st: Fx) -> String:
  match s:
    case SNil{}:
      Voice.fix.fin(st)
    case SCon{+c, +t}:
      Voice.fix.go(t, w, r, lw, Voice.fix.step(st, c, SCon{c, t}, w, r, lw))

def Voice.fix.k(same: Bool, t: String, +w: String, +r: String) -> String:
  match same:
    case True{}:
      t
    case False{}:
      Voice.fix.go(t, w, r, String.length(w), Fx{0n, False{}, SNil{}})

# every whole-word `word` (exact case, may hold spaces) in text replaced by
# repl; an empty word or repl == word leaves the text as it is (laws
# voice_fix_*)
def Voice.fix(t: String, w: String, +r: String) -> String:
  match w:
    case SNil{}:
      t
    case SCon{+h, +tl}:
      Voice.fix.k(String.eq(SCon{h, tl}, r), t, SCon{h, tl}, r)

# JSON and prose
# --------------

def Voice.unsure.one(u: Unsure) -> J.Json:
  match u:
    case Unsure{w, g}:
      J.Json.obj([J.KV{"word", J.Str{w}}, J.KV{"guess", J.Str{g}}])

def Voice.unsure.js(us: List<&2, Unsure>) -> List<&2, J.Json>:
  match us:
    case Nil{}:
      Nil{}
    case h <> t:
      Voice.unsure.one(h) <> Voice.unsure.js(t)

# [{"word": .., "guess": ..}, ..]
def Voice.unsure.json(us: List<&2, Unsure>) -> J.Json:
  J.Json.arr(Voice.unsure.js(us))

def Voice.unsure.keep(u: Unsure, acc: List<&2, Unsure>) -> List<&2, Unsure>:
  match u:
    case Unsure{SNil{}, g}:
      acc
    case Unsure{SCon{h, t}, g}:
      Unsure{SCon{h, t}, g} <> acc

def Voice.unsure.go(xs: List<&2, J.Json>, acc: List<&2, Unsure>) -> List<&2, Unsure>:
  match xs:
    case Nil{}:
      List.reverse(&2, Unsure, acc)
    case +o <> t:
      Voice.unsure.go(t, Voice.unsure.keep(Unsure{J.Json.get_str(o, "word", ""), J.Json.get_str(o, "guess", "")}, acc))

# the inverse of unsure.json; items without a word are dropped
def Voice.unsure.parse(j: J.Json) -> List<&2, Unsure>:
  Voice.unsure.go(J.Json.elems(j), Nil{})

def Voice.note.guess(none: Bool, g: String) -> String:
  match none:
    case True{}:
      ""
    case False{}:
      " (" ++ g ++ "?)"

def Voice.note.items(us: List<&2, Unsure>) -> List<&2, String>:
  match us:
    case Nil{}:
      Nil{}
    case Unsure{w, +g} <> t:
      ("\"" ++ w ++ "\"" ++ Voice.note.guess(String.is_empty(g), g)) <> Voice.note.items(t)

# one line for an agent: Possibly misheard: "key cad" (KiCad?), "stm32f4".
# Ask if unsure.  ("" when there is nothing to ask)
def Voice.note(us: List<&2, Unsure>) -> String:
  match us:
    case Nil{}:
      ""
    case +h <> +t:
      "Possibly misheard: " ++ String.join(Voice.note.items(h <> t), ", ") ++ ". Ask if unsure."

# Into the draft
# --------------
# A finished segment goes in at the caret (the draft's end for a client
# without one) and reads on from what is there: a space before it unless
# the draft is blank there or ends in a space or an opening bracket, a
# space after it when a word follows. Mid-sentence (nothing before it but
# a sentence's end, . ! ? or a new line, counts as a start) its first word
# loses the capital the transcriber gives every segment, unless it is "I",
# a dictionary term (`firm`), or not a plain capitalised word; before a
# lower-case word its closing period goes. The caret lands after it.

type Ins is Data:
  Ins{text: String, caret: Nat}

def Ins.text(i: Ins) -> String:
  match i:
    case Ins{t, _}:
      t

def Ins.caret(i: Ins) -> Nat:
  match i:
    case Ins{_, c}:
      c

# a character's code, 0 for none
def Voice.code(m: Maybe<&2, Char>) -> U32:
  match m:
    case Some{Chr{+c}}:
      c
    case None{}:
      0

def Voice.last(+s: String) -> U32:
  Voice.code(String.get(s, Nat.sub(String.length(s), 1n)))

def Voice.first(+s: String) -> U32:
  Voice.code(String.get(s, 0n))

def Voice.space(+c: U32) -> Bool:
  Bool.or(Bool.or(U32.is_eq(c, 32), U32.is_eq(c, 9)), Bool.or(U32.is_eq(c, 10), U32.is_eq(c, 13)))

# ( [ { " “ ‘
def Voice.opens(+c: U32) -> Bool:
  Bool.or(Bool.or(Bool.or(U32.is_eq(c, 40), U32.is_eq(c, 91)), Bool.or(U32.is_eq(c, 123), U32.is_eq(c, 34))), Bool.or(U32.is_eq(c, 8220), U32.is_eq(c, 8216)))

def Voice.lower(+c: U32) -> Bool:
  Bool.and(U32.is_ge(c, 97), U32.is_le(c, 122))

def Voice.upper(+c: U32) -> Bool:
  Bool.and(U32.is_ge(c, 65), U32.is_le(c, 90))

# a space goes before the segment
def Voice.gap(+before: String) -> Bool:
  Bool.not(Bool.or(U32.is_eq(Voice.last(before), 0), Bool.or(Voice.space(Voice.last(before)), Voice.opens(Voice.last(before)))))

# 0 (nothing), . ! ?
def Voice.stop.c(+c: U32) -> Bool:
  Bool.or(Bool.or(U32.is_eq(c, 0), U32.is_eq(c, 46)), Bool.or(U32.is_eq(c, 33), U32.is_eq(c, 63)))

# what comes next starts a sentence
def Voice.starts(+before: String) -> Bool:
  Bool.or(U32.is_eq(Voice.last(before), 10), Voice.stop.c(Voice.last(String.trim_end(before))))

# the first word as a plain capitalised word: 0 at its first character, 1
# inside it, 2 past it, 3 not plain
def Voice.plain.step(st: Nat, +c: U32) -> Nat:
  match st:
    case 0n:
      Bool.pick(Nat, Voice.upper(c), 1n, 3n)
    case 1n:
      Bool.pick(Nat, Bool.or(Voice.lower(c), Bool.or(U32.is_eq(c, 39), U32.is_eq(c, 8217))), 1n, Bool.pick(Nat, Voice.alnum(c), 3n, 2n))
    case +k:
      k

def Voice.plain.go(s: String, st: Nat) -> Nat:
  match s:
    case SNil{}:
      st
    case SCon{Chr{+c}, t}:
      Voice.plain.go(t, Voice.plain.step(st, c))

def Voice.plain.k(st: Nat) -> Bool:
  match st:
    case 1n:
      True{}
    case 2n:
      True{}
    case _:
      False{}

def Voice.plain(s: String) -> Bool:
  Voice.plain.k(Voice.plain.go(s, 0n))

# "I", "I'm", "I've": the capital stays
def Voice.self(+s: String) -> Bool:
  Bool.and(U32.is_eq(Voice.first(s), 73), Bool.not(Voice.lower(Voice.code(String.get(s, 1n)))))

def Voice.lower1(s: String) -> String:
  match s:
    case SNil{}:
      SNil{}
    case SCon{c, t}:
      SCon{Char.to_lower(c), t}

def Voice.casing.k(go: Bool, s: String) -> String:
  match go:
    case True{}:
      Voice.lower1(s)
    case False{}:
      s

def Voice.casing(firm: Bool, mid: Bool, +s: String) -> String:
  Voice.casing.k(Bool.and(Bool.and(mid, Bool.not(firm)), Bool.and(Voice.plain(s), Bool.not(Voice.self(s)))), s)

def Voice.unstop.k(go: Bool, +s: String) -> String:
  match go:
    case True{}:
      String.take(s, Nat.sub(String.length(s), 1n))
    case False{}:
      s

# the closing period dropped before a lower-case word (an ellipsis stays)
def Voice.unstop(next: Bool, +s: String) -> String:
  Voice.unstop.k(Bool.and(next, Bool.and(String.ends_with(s, "."), Bool.not(String.ends_with(s, "..")))), s)

def Voice.put.go(firm: Bool, +before: String, +after: String, +seg: String) -> Ins:
  +s = Voice.unstop(Voice.lower(Voice.first(String.trim_start(after))), Voice.casing(firm, Bool.not(Voice.starts(before)), seg))
  +lead = Bool.pick(String, Voice.gap(before), " ", "")
  Ins{before ++ lead ++ s ++ Bool.pick(String, Voice.alnum(Voice.first(after)), " ", "") ++ after, Nat.add(String.length(before), Nat.add(String.length(lead), String.length(s)))}

def Voice.put.k(blank: Bool, firm: Bool, +draft: String, +at: Nat, +seg: String) -> Ins:
  match blank:
    case True{}:
      Ins{draft, at}
    case False{}:
      Voice.put.go(firm, String.take(draft, at), String.drop(draft, at), seg)

# a segment into the draft at `at` (past the end means the end; laws
# voice_put_*)
def Voice.put(firm: Bool, +draft: String, +at: Nat, +text: String) -> Ins:
  Voice.put.k(String.is_empty(String.trim(text)), firm, draft, Bool.pick(Nat, Nat.is_le(at, String.length(draft)), at, String.length(draft)), String.trim(text))

def Voice.firm.go(ds: List<&2, String>, +s: String, acc: Bool) -> Bool:
  match ds:
    case Nil{}:
      acc
    case +h <> t:
      Voice.firm.go(t, s, Bool.or(acc, Bool.and(Bool.not(String.is_empty(h)), String.starts_with(s, String.to_lower(h)))))

# the segment opens with a dictionary term, whose capital stays
def Voice.firm(ds: List<&2, String>, text: String) -> Bool:
  Voice.firm.go(ds, String.to_lower(String.trim(text)), False{})

# The OpenAI key
# --------------

def Voice.openai.blank(+c: U32) -> Bool:
  Bool.or(U32.is_le(c, 32), U32.is_eq(c, 127))

def Voice.openai.blanks(s: String) -> Bool:
  match s:
    case SNil{}:
      False{}
    case SCon{Chr{c}, t}:
      Bool.or(Voice.openai.blank(c), Voice.openai.blanks(t))

# an OpenAI key: "sk-", over 20 characters, no spaces or control
# characters (the only kind the hub keeps: server voice.bend's Vo.set_key;
# the web's Save checks it before anything is sent)
def Voice.openai.ok(+k: String) -> Bool:
  Bool.and(String.starts_with(k, "sk-"), Bool.and(Nat.is_gt(String.length(k), 20n), Bool.not(Voice.openai.blanks(k))))
