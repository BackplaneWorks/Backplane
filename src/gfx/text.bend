import Base
import ./font.bend as F
import ./raster.bend as R
import ./draw.bend as D
import ./fontfx.bend as FX
import ../core/cjk.bend as C

# System-font text: faces found by fontconfig and rasterized by FreeType
# (fontfx.bend), glyphs cached in Bend state, and draw ops for anti-aliased
# glyph bitmaps (R.SMask). A face that cannot load falls back to the
# Spleen 8x16 bitmap font, so text always draws.
#
# The effect runs only for glyphs missing from the cache: Text.warm fills
# ASCII once per face, Text.ensure fills whatever a new string needs.
# Measuring, wrapping and drawing are pure and read the cache; a glyph
# still missing measures as half an em and draws nothing.

# a cached glyph: advance (26.6), bitmap left / top (+ 32768), size, and
# its coverage as a tree of 2^d words
type TG is Data:
  TG{adv: U32, left: U32, top: U32, w: U32, h: U32, d: Nat, m: R.Mask}

# a glyph a frame wants fetched: face, code point, miss advance (Text.wants)
type Want is Data:
  Want{id: U32, cp: U32, miss: U32}

# the face's ASCII glyphs (code points 0-127) as a tree of 2^7 leaves, so
# the glyphs text draws most are found without the cache's string keys; a
# leaf not filled (ok False) is looked up in the cache
type GTab is Data:
  GLeaf{ok: Bool, g: TG}
  GNode{lo: GTab, hi: GTab}

# A face: `ft` says FreeType (else the bitmap font); asc / desc / lh are
# the font's own pixel metrics; miss is the advance (26.6) of a glyph not
# in the cache; name is the family fontconfig chose.
type Face is Data:
  Face{id: U32, ft: Bool, px: U32, asc: U32, desc: U32, lh: U32, miss: U32, name: String, tab: GTab}

type FileIx is Data:
  FileIx{family: String, file: String, index: U32}

def Text.px(+v: U32) -> U32:
  ((v + 32 : U32) / 64 : U32)

# generic names accept whatever fontconfig picks for them
def Text.generic(+name: String) -> Bool:
  Bool.or(Bool.or(String.eq(name, "monospace"), String.eq(name, "sans-serif")),
    Bool.or(Bool.or(String.eq(name, "serif"), String.eq(name, "system-ui")), String.eq(name, "sans")))

# the family part of a pattern ("DejaVu Sans:weight=bold" -> "dejavu sans")
def Text.family(+pat: String) -> String:
  String.to_lower(Maybe.default(&2, String, List.head(&2, String, String.split(pat, ':')), ""))

def Text.accept(+pat: String, +fam: String) -> Bool:
  Bool.or(Text.generic(Text.family(pat)), String.eq(Text.family(pat), String.to_lower(fam)))

def Text.found.k(ok: Bool, fam: String, file: String, +index: U32) -> Maybe<&2, FileIx>:
  match ok:
    case True{}:
      Some{FileIx{fam, file, index}}
    case False{}:
      None{}

def Text.found(+pat: String, r: String & String & U32) -> Maybe<&2, FileIx>:
  (+fam, rest) = r
  (file, index) = rest
  Text.found.k(Text.accept(pat, fam), fam, file, index)

def Text.hit(+pat: String, r: Result<&1, &1, U32 & String, String & String & U32>) -> Maybe<&2, FileIx>:
  match r:
    case Done{v}:
      Text.found(pat, v)
    case Fail{_}:
      None{}

def Text.pick.k(m: Maybe<&2, FileIx>, next: IO(Maybe<&2, FileIx>)) -> IO(Maybe<&2, FileIx>):
  match m:
    case Some{f}:
      IO.pure(Maybe<&2, FileIx>, Some{f})
    case None{}:
      next

# the first family of a CSS-like stack that the system really has
# ("Berkeley Mono" only if installed; generic names always resolve)
def Text.pick(stack: List<&2, String>) -> IO(Maybe<&2, FileIx>):
  match stack:
    case Nil{}:
      IO.pure(Maybe<&2, FileIx>, None{})
    case +pat <> t:
      IO.bind(Result<&1, &1, U32 & String, String & String & U32>, Maybe<&2, FileIx>, FX.Font.find(pat),
        r => Text.pick.k(Text.hit(pat, r), Text.pick(t)))

# Glyphs
# ------

def Mask.depth(+n: U32) -> Nat:
  Bool.pick(Nat, U32.is_le(n, 1), 0n, 1n+U32.log2((n - 1 : U32)))

def Mask.word(xs: List<&2, U32>) -> R.Mask:
  match xs:
    case Nil{}:
      R.MWord{0}
    case h <> _:
      R.MWord{h}

def Mask.w(xs: List<&2, U32>, +i: Nat) -> U32:
  Maybe.default(&2, U32, List.head(&2, U32, List.drop(&2, U32, xs, i)), 0)

# the first 16 words of xs as one leaf (zeros past the end)
def Mask.leaf16(+xs: List<&2, U32>) -> R.Mask:
  R.M16{Mask.w(xs, 0n), Mask.w(xs, 1n), Mask.w(xs, 2n), Mask.w(xs, 3n), Mask.w(xs, 4n), Mask.w(xs, 5n), Mask.w(xs, 6n), Mask.w(xs, 7n),
    Mask.w(xs, 8n), Mask.w(xs, 9n), Mask.w(xs, 10n), Mask.w(xs, 11n), Mask.w(xs, 12n), Mask.w(xs, 13n), Mask.w(xs, 14n), Mask.w(xs, 15n)}

# the first 2^d words of xs as a tree of words (zeros past the end)
def Mask.small(d: Nat, +xs: List<&2, U32>) -> R.Mask:
  match d:
    case 0n:
      Mask.word(xs)
    case 1n+k0:
      +k = k0
      +half = U32.to_nat(U32.shln(1, k))
      lo hi = Mask.small(k, List.take(&2, U32, xs, half)) Mask.small(k, List.drop(&2, U32, xs, half))
      R.MPair{lo, hi}

# 2^(e+4) words of xs as a tree of 16-word leaves (raster.bend's M16)
def Mask.big(e: Nat, +xs: List<&2, U32>) -> R.Mask:
  match e:
    case 0n:
      Mask.leaf16(xs)
    case 1n+k0:
      +k = k0
      +half = U32.to_nat(U32.shln(16, k))
      lo hi = Mask.big(k, List.take(&2, U32, xs, half)) Mask.big(k, List.drop(&2, U32, xs, half))
      R.MPair{lo, hi}

def Mask.of.k(big: Bool, +d: Nat, +xs: List<&2, U32>) -> R.Mask:
  match big:
    case True{}:
      Mask.big(Nat.sub(d, 4n), xs)
    case False{}:
      Mask.small(d, xs)

# the first 2^d words of xs as a tree (zeros past the end); from 16 words
# up, its last four levels are 16-word leaves, so a glyph is a few nodes
def Mask.of(+d: Nat, +xs: List<&2, U32>) -> R.Mask:
  Mask.of.k(Nat.is_ge(d, 4n), d, xs)

def TG.blank(+miss: U32) -> TG:
  TG{miss, 32768, 32768, 0, 0, 0n, R.MWord{0}}

# a glyph not in the cache yet (a fetched one's corner is biased by 32768,
# so never 0)
def TG.none(+miss: U32) -> TG:
  TG{miss, 0, 0, 0, 0, 0n, R.MWord{0}}

def TG.missing(g: TG) -> Bool:
  match g:
    case TG{_, +l, +t, _, _, _, _}:
      Bool.and(U32.is_eq(l, 0), U32.is_eq(t, 0))

# Font.glyph's words as a glyph
def TG.decode(ws: List<&2, U32>, +miss: U32) -> TG:
  match ws:
    case _ <> +adv <> +l <> +t <> +w <> +h <> cov:
      +d = Mask.depth((((w * h : U32) + 3 : U32) / 4 : U32))
      TG{adv, l, t, w, h, d, Mask.of(d, cov)}
    case _:
      TG.blank(miss)

def Text.key(+id: U32, +cp: U32) -> String:
  U32.show(id) ++ "/" ++ U32.show(cp)

def Text.got(r: Map<&2, TG> & TG) -> TG:
  (_, g) = r
  g

def Text.get.map(+cache: Map<&2, TG>, +id: U32, +cp: U32, +miss: U32) -> TG:
  Text.got(Map.get(TG, TG.none(miss), cache, Text.key(id, cp)))

def GTab.leaf(g: GTab, +cache: Map<&2, TG>, +id: U32, +cp: U32, +miss: U32) -> TG:
  match g:
    case GLeaf{True{}, g}:
      g
    case _:
      Text.get.map(cache, id, cp, miss)

def GTab.half(hi: Bool, lo: GTab, h: GTab) -> GTab:
  match hi:
    case True{}:
      h
    case False{}:
      lo

# the leaf for cp, d levels down (bit d-1 picks the half)
def GTab.at(+d: Nat, t: GTab, +cp: U32, +bit: U32) -> GTab:
  match d t:
    case 1n+k GNode{lo, hi}:
      GTab.at(k, GTab.half(U32.is_ne(U32.and(cp, bit), 0), lo, hi), cp, U32.shr(bit))
    case _ _:
      t

def Text.get.k(ascii: Bool, +cache: Map<&2, TG>, +id: U32, +cp: U32, +miss: U32, +tab: GTab) -> TG:
  match ascii:
    case True{}:
      GTab.leaf(GTab.at(7n, tab, cp, 64), cache, id, cp, miss)
    case False{}:
      Text.get.map(cache, id, cp, miss)

# an empty table, d levels deep
def GTab.none(+d: Nat) -> GTab:
  match d:
    case 0n:
      GLeaf{False{}, TG.blank(0)}
    case 1n+k:
      GNode{GTab.none(k), GTab.none(k)}

# Faces
# -----

def Text.bitmap() -> Face:
  Face{0, False{}, 16, 12, 4, 16, 512, "Spleen 8x16", GTab.none(7n)}

def Text.face.m(+id: U32, +px: U32, ms: List<&2, U32>, +name: String) -> Face:
  match ms:
    case +a <> +d <> +h <> _:
      Face{id, True{}, px, Text.px(a), Text.px(d), Text.px(h), (px * 32 : U32), name, GTab.none(7n)}
    case _:
      Text.bitmap()

def Text.face.o(r: Result<&1, &1, U32 & String, U32>, +px: U32, +name: String) -> IO(Face):
  match r:
    case Done{+id}:
      IO.bind(List<&2, U32>, Face, FX.Font.metrics(id), ms => IO.pure(Face, Text.face.m(id, px, ms, name)))
    case Fail{_}:
      IO.pure(Face, Text.bitmap())

def Text.face.f(m: Maybe<&2, FileIx>, +px: U32) -> IO(Face):
  match m:
    case Some{FileIx{+fam, +file, +index}}:
      IO.bind(Result<&1, &1, U32 & String, U32>, Face, FX.Font.open(file, index, px), r => Text.face.o(r, px, fam))
    case None{}:
      IO.pure(Face, Text.bitmap())

# a face for the first available family of the stack at px pixels per em;
# the bitmap font when none loads (no fontconfig / FreeType)
def Text.face(stack: List<&2, String>, +px: U32) -> IO(Face):
  IO.bind(Maybe<&2, FileIx>, Face, Text.pick(stack), m => Text.face.f(m, px))

def GTab.one(m: Maybe<&2, TG>) -> GTab:
  match m:
    case Some{g}:
      GLeaf{True{}, g}
    case None{}:
      GLeaf{False{}, TG.blank(0)}

def Text.cached.k(r: Map<&2, TG> & Bool, +id: U32, +cp: U32) -> Maybe<&2, TG>:
  match r:
    case (+c, True{}):
      Some{Text.got(Map.get(TG, TG.blank(0), c, Text.key(id, cp)))}
    case (_, False{}):
      None{}

def Text.cached(+cache: Map<&2, TG>, +id: U32, +cp: U32) -> Maybe<&2, TG>:
  Text.cached.k(Map.has(&2, TG, cache, Text.key(id, cp)), id, cp)

# the table of the cache's glyphs for face id, code points from cp, 2^d of them
def GTab.of(+d: Nat, +cache: Map<&2, TG>, +id: U32, +cp: U32) -> GTab:
  match d:
    case 0n:
      GTab.one(Text.cached(cache, id, cp))
    case 1n+k:
      GNode{GTab.of(k, cache, id, cp), GTab.of(k, cache, id, (cp + U32.shln(1, k) : U32))}

# the face with its ASCII table filled from the cache (once warmed)
def Face.tabled(f: Face, +cache: Map<&2, TG>) -> Face:
  match f:
    case Face{+id, ft, px, asc, desc, lh, miss, name, _}:
      Face{id, ft, px, asc, desc, lh, miss, name, GTab.of(7n, cache, id, 0)}

def Text.get(+cache: Map<&2, TG>, +f: Face, +cp: U32) -> TG:
  match f:
    case Face{+id, _, _, _, _, _, +miss, _, +tab}:
      Text.get.k(U32.is_lt(cp, 128), cache, id, cp, miss, tab)

def Text.fetch(+cache: Map<&2, TG>, +id: U32, +cp: U32, +miss: U32) -> IO(Map<&2, TG>):
  IO.bind(List<&2, U32>, Map<&2, TG>, FX.Font.glyph(id, cp), ws => IO.pure(Map<&2, TG>, Map.set(&2, TG, cache, Text.key(id, cp), TG.decode(ws, miss))))

def Text.one.k(r: Map<&2, TG> & Bool, +id: U32, +cp: U32, +miss: U32) -> IO(Map<&2, TG>):
  match r:
    case (+cache, True{}):
      IO.pure(Map<&2, TG>, cache)
    case (+cache, False{}):
      Text.fetch(cache, id, cp, miss)

def Text.one(cache: Map<&2, TG>, +id: U32, +cp: U32, +miss: U32) -> IO(Map<&2, TG>):
  Text.one.k(Map.has(&2, TG, cache, Text.key(id, cp)), id, cp, miss)

# the cache with the wanted glyphs fetched (each once: a glyph no font
# has is kept blank, so it is not wanted again)
def Text.fill(ws: List<&2, Want>, cache: Map<&2, TG>) -> IO(Map<&2, TG>):
  match ws:
    case Nil{}:
      IO.pure(Map<&2, TG>, cache)
    case Want{+id, +cp, +miss} <> t:
      IO.bind(Map<&2, TG>, Map<&2, TG>, Text.one(cache, id, cp, miss), c2 => Text.fill(t, c2))

def Text.ensure.go(s: String, cache: Map<&2, TG>, +id: U32, +miss: U32) -> IO(Map<&2, TG>):
  match s:
    case SNil{}:
      IO.pure(Map<&2, TG>, cache)
    case SCon{Chr{+cp}, t}:
      IO.bind(Map<&2, TG>, Map<&2, TG>, Text.one(cache, id, cp, miss), c2 => Text.ensure.go(t, c2, id, miss))

# the cache with every glyph of s for the face (the effect runs only for
# the missing ones; the bitmap face needs none)
def Text.ensure(cache: Map<&2, TG>, f: Face, s: String) -> IO(Map<&2, TG>):
  match f:
    case Face{+id, True{}, _, _, _, _, +miss, _, _}:
      Text.ensure.go(s, cache, id, miss)
    case Face{_, False{}, _, _, _, _, _, _, _}:
      IO.pure(Map<&2, TG>, cache)

def Text.ascii.go(n: Nat, acc: String) -> String:
  match n:
    case 0n:
      acc
    case 1n+i0:
      +i = i0
      Text.ascii.go(i, SCon{Chr{U32.from_nat(Nat.add(i, 32n))}, acc})

# printable ASCII, " " to "~", and the punctuation the UI draws
def Text.ascii() -> String:
  Text.ascii.go(95n, "…·—–•→✓")

# warm-up: ASCII (and the UI's punctuation) for the face
def Text.warm(cache: Map<&2, TG>, f: Face) -> IO(Map<&2, TG>):
  Text.ensure(cache, f, Text.ascii())

# Measuring
# ---------

def TG.adv(g: TG) -> U32:
  match g:
    case TG{adv, _, _, _, _, _, _}:
      adv

# the advance of s in 26.6 pixels, added to acc
def Text.adv(s: String, +cache: Map<&2, TG>, +f: Face, acc: U32) -> U32:
  match s:
    case SNil{}:
      acc
    case SCon{Chr{+cp}, t}:
      Text.adv(t, cache, f, (acc + TG.adv(Text.get(cache, f, cp)) : U32))

def Text.width.k(ft: Bool, +cache: Map<&2, TG>, +f: Face, s: String) -> U32:
  match ft:
    case True{}:
      Text.px(Text.adv(s, cache, f, 0))
    case False{}:
      (U32.from_nat(String.length(s)) * 8 : U32)

def Face.ft(f: Face) -> Bool:
  match f:
    case Face{_, ft, _, _, _, _, _, _, _}:
      ft

def Face.asc(f: Face) -> U32:
  match f:
    case Face{_, _, _, asc, _, _, _, _, _}:
      asc

def Face.lh(f: Face) -> U32:
  match f:
    case Face{_, _, _, _, _, lh, _, _, _}:
      lh

def Face.name(f: Face) -> String:
  match f:
    case Face{_, _, _, _, _, _, _, name, _}:
      name

# the width of s in pixels
def Text.width(+cache: Map<&2, TG>, +f: Face, s: String) -> U32:
  Text.width.k(Face.ft(f), cache, f, s)

# the baseline of a line box of height lh whose top is y (CSS half-leading)
def Text.baseline(+f: Face, +y: U32, +lh: U32) -> U32:
  match f:
    case Face{_, _, _, +asc, +desc, _, _, _, _}:
      (y + ((lh - (asc + desc : U32) : U32) / 2 : U32) + asc : U32)

# Wrapping (proportional)
# -----------------------

def Fit.count.k(ok: Bool, rest: Nat) -> Nat:
  match ok:
    case True{}:
      1n+rest
    case False{}:
      0n

# The decision belongs to the character just examined. Branch before
# touching the suffix: eager arguments to Fit.count.k walked all of it,
# even when the first character overflowed.
def Fit.count.go(s: String, fits: Bool, +cache: Map<&2, TG>, +f: Face, +max: U32, +used: U32) -> Nat:
  match s fits:
    case _ False{}:
      0n
    case SNil{} True{}:
      1n
    case SCon{Chr{+cp}, t} True{}:
      +u2 = (used + TG.adv(Text.get(cache, f, cp)) : U32)
      1n+Fit.count.go(t, U32.is_le(u2, max), cache, f, max, u2)

# how many leading characters of s fit in max (26.6) after used
def Fit.count(s: String, +cache: Map<&2, TG>, +f: Face, +max: U32, +used: U32) -> Nat:
  match s:
    case SNil{}:
      0n
    case SCon{Chr{+cp}, t}:
      +u2 = (used + TG.adv(Text.get(cache, f, cp)) : U32)
      Fit.count.go(t, U32.is_le(u2, max), cache, f, max, u2)

# Always make progress, even when a single glyph is wider than the line.
def Fit.atleast(n: Nat) -> Nat:
  match n:
    case 0n:
      1n
    case 1n+k:
      1n+k

# Pieces newest first, so even a token broken into many narrow lines
# never builds a call stack proportional to the number of lines.
def Fit.chunk.go(n: Nat, empty: Bool, +s: String, +cache: Map<&2, TG>, +f: Face, +max: U32, acc: List<&2, String>) -> List<&2, String>:
  match n empty:
    case 1n+m False{}:
      +k = Fit.atleast(Fit.count(s, cache, f, max, 0))
      +rest = String.drop(s, k)
      Fit.chunk.go(m, String.is_empty(rest), rest, cache, f, max, String.take(s, k) <> acc)
    case _ _:
      List.reverse(&2, String, acc)

# s in pieces that each fit in max (at least one character each)
def Fit.chunk(n: Nat, empty: Bool, +s: String, +cache: Map<&2, TG>, +f: Face, +max: U32) -> List<&2, String>:
  Fit.chunk.go(n, empty, s, cache, f, max, Nil{})

# lines so far (newest first), the line being filled, and its advance
type PAcc is Data:
  PAcc{lines: List<&2, String>, cur: String, len: U32, first: Bool}

def PWrap.put.k(first: Bool, fits: Bool, a: PAcc, +w: String, +ww: U32, +sp: U32) -> PAcc:
  match first fits a:
    case True{} _ PAcc{ls, _, _, _}:
      PAcc{ls, w, ww, False{}}
    case False{} True{} PAcc{ls, cur, +len, _}:
      PAcc{ls, cur ++ " " ++ w, (len + sp + ww : U32), False{}}
    case False{} False{} PAcc{ls, cur, _, _}:
      PAcc{cur <> ls, w, ww, False{}}

def PWrap.put(a: PAcc, +w: String, +ww: U32, +sp: U32, +max: U32) -> PAcc:
  match a:
    case PAcc{ls, cur, +len, +first}:
      PWrap.put.k(first, U32.is_le((len + sp + ww : U32), max), PAcc{ls, cur, len, first}, w, ww, sp)

def PWrap.pieces(ps: List<&2, String>, a: PAcc, +cache: Map<&2, TG>, +f: Face, +sp: U32, +max: U32) -> PAcc:
  match ps:
    case Nil{}:
      a
    case +p <> t:
      PWrap.pieces(t, PWrap.put(a, p, Text.adv(p, cache, f, 0), sp, max), cache, f, sp, max)

# a word's pieces; an empty word (between two spaces, or after a last
# one) is kept, so every space the text has is on a line (the caret after
# a typed space moves at once, runs of spaces show)
def PWrap.chunks(empty: Bool, +w: String, +cache: Map<&2, TG>, +f: Face, +max: U32) -> List<&2, String>:
  match empty:
    case True{}:
      [""]
    case False{}:
      Fit.chunk(1n+String.length(w), False{}, w, cache, f, max)

def PWrap.words(ws: List<&2, String>, a: PAcc, +cache: Map<&2, TG>, +f: Face, +sp: U32, +max: U32) -> PAcc:
  match ws:
    case Nil{}:
      a
    case +w <> t:
      PWrap.words(t, PWrap.pieces(PWrap.chunks(String.is_empty(w), w, cache, f, max), a, cache, f, sp, max), cache, f, sp, max)

def PWrap.fin(a: PAcc) -> List<&2, String>:
  match a:
    case PAcc{ls, cur, _, _}:
      List.reverse(&2, String, cur <> ls)

# CJK boundaries drop nothing. A space used as a line break is dropped,
# exactly as in PWrap, so Edit.starts can recover each source position.
# Reverse the current line to avoid copying it as each character arrives.
type JAcc is Data:
  JAcc{lines: List<&2, String>, rev: String, len: U32, first: Bool}

def JWrap.put.k(first: Bool, fits: Bool, a: JAcc, +w: String, +ww: U32, +gap: Bool, +sp: U32) -> JAcc:
  match first fits a:
    case True{} _ JAcc{ls, _, _, _}:
      JAcc{ls, C.Cjk.rev(w, ""), ww, False{}}
    case False{} True{} JAcc{ls, +rev, +len, _}:
      JAcc{ls, C.Cjk.rev(w, Bool.pick(String, gap, SCon{' ', rev}, rev)), (len + sp + ww : U32), False{}}
    case False{} False{} JAcc{ls, rev, _, _}:
      JAcc{C.Cjk.rev(rev, "") <> ls, C.Cjk.rev(w, ""), ww, False{}}

def JWrap.put(a: JAcc, +w: String, +gap: Bool, +cache: Map<&2, TG>, +f: Face, +max: U32) -> JAcc:
  match a:
    case JAcc{ls, rev, +len, +first}:
      +ww = Text.adv(w, cache, f, 0)
      +sp = Bool.pick(U32, gap, TG.adv(Text.get(cache, f, 32)), 0)
      JWrap.put.k(first, U32.is_le((len + sp + ww : U32), max), JAcc{ls, rev, len, first}, w, ww, gap, sp)

def JWrap.pieces(ps: List<&2, String>, a: JAcc, +gap: Bool, +cache: Map<&2, TG>, +f: Face, +max: U32) -> JAcc:
  match ps:
    case Nil{}:
      a
    case p <> t:
      JWrap.pieces(t, JWrap.put(a, p, gap, cache, f, max), False{}, cache, f, max)

# An overlong Latin word can share a segment with its surrounding CJK
# brackets. Emergency cuts may split that word, but still keep brackets
# with their contents. If no legal cut fits, allow the smallest group to
# overhang rather than orphaning punctuation.
def JFit.best(legal: Bool, before: Bool, best: Nat, n: Nat) -> Nat:
  match legal before:
    case True{} True{}:
      n
    case _ _:
      best

def JFit.done.k(have: Bool, legal: Bool, best: Nat, n: Nat) -> Maybe<&2, Nat>:
  match have legal:
    case True{} _:
      Some{best}
    case False{} True{}:
      Some{n}
    case False{} False{}:
      None{}

def JFit.done(reached: Bool, legal: Bool, +best: Nat, n: Nat) -> Maybe<&2, Nat>:
  match reached:
    case False{}:
      None{}
    case True{}:
      JFit.done.k(Nat.is_ne(best, 0n), legal, best, n)

def JFit.scan(s: String, done: Maybe<&2, Nat>, +limit: Nat, +best: Nat, +n: Nat, +prev: U32) -> Nat:
  match s done:
    case _ Some{k}:
      k
    case SNil{} None{}:
      n
    case SCon{Chr{+cp}, t} None{}:
      +legal = Bool.and(Bool.not(C.Cjk.no_end(prev)), Bool.not(C.Cjk.no_start(cp)))
      +b2 = JFit.best(legal, Nat.is_le(n, limit), best, n)
      JFit.scan(t, JFit.done(Nat.is_ge(n, limit), legal, b2, n), limit, b2, 1n+n, cp)

def JFit.take(s: String, +limit: Nat) -> Nat:
  match s:
    case SNil{}:
      0n
    case SCon{Chr{cp}, t}:
      JFit.scan(t, None{}, limit, 0n, 1n, cp)

def JFit.chunk(n: Nat, empty: Bool, +s: String, +cache: Map<&2, TG>, +f: Face, +max: U32, acc: List<&2, String>) -> List<&2, String>:
  match n empty:
    case 1n+m False{}:
      +k = JFit.take(s, Fit.atleast(Fit.count(s, cache, f, max, 0)))
      +rest = String.drop(s, k)
      JFit.chunk(m, String.is_empty(rest), rest, cache, f, max, String.take(s, k) <> acc)
    case _ _:
      List.reverse(&2, String, acc)

def JWrap.chunks(empty: Bool, +w: String, +cache: Map<&2, TG>, +f: Face, +max: U32) -> List<&2, String>:
  match empty:
    case True{}:
      [""]
    case False{}:
      JFit.chunk(1n+String.length(w), False{}, w, cache, f, max, Nil{})

def JWrap.segs(ss: List<&2, C.Seg>, a: JAcc, +cache: Map<&2, TG>, +f: Face, +max: U32) -> JAcc:
  match ss:
    case Nil{}:
      a
    case C.Seg{+w, gap} <> t:
      JWrap.segs(t, JWrap.pieces(JWrap.chunks(String.is_empty(w), w, cache, f, max), a, gap, cache, f, max), cache, f, max)

def JWrap.fin(a: JAcc) -> List<&2, String>:
  match a:
    case JAcc{ls, rev, _, _}:
      List.reverse(&2, String, C.Cjk.rev(rev, "") <> ls)

def Text.wrap.line.k(cjk: Bool, s: String, +cache: Map<&2, TG>, +f: Face, +px: U32) -> List<&2, String>:
  match cjk:
    case True{}:
      JWrap.fin(JWrap.segs(C.Cjk.segs(s), JAcc{Nil{}, "", 0, True{}}, cache, f, (px * 64 : U32)))
    case False{}:
      PWrap.fin(PWrap.words(String.split(s, ' '), PAcc{Nil{}, "", 0, True{}}, cache, f, TG.adv(Text.get(cache, f, 32)), (px * 64 : U32)))

# one paragraph line (no newlines) wrapped to px pixels; "" stays one
# blank line
def Text.wrap.line(+s: String, +cache: Map<&2, TG>, +f: Face, +px: U32) -> List<&2, String>:
  Text.wrap.line.k(C.Cjk.has(s), s, cache, f, px)

def Text.wrap.lines(ls: List<&2, String>, +cache: Map<&2, TG>, +f: Face, +px: U32) -> List<&2, String>:
  match ls:
    case Nil{}:
      Nil{}
    case l <> t:
      List.append(&2, String, Text.wrap.line(l, cache, f, px), Text.wrap.lines(t, cache, f, px))

# text with newlines, wrapped greedily by words to px pixels (a word
# wider than a line is broken between characters)
def Text.wrap(+cache: Map<&2, TG>, +f: Face, s: String, +px: U32) -> List<&2, String>:
  Text.wrap.lines(String.split(s, '\n'), cache, f, px)

# Drop opening punctuation from an ellipsized prefix. Walk its reversed
# suffix only; the fit already counted complete code points.
def Fit.close.head(s: String) -> Bool:
  match s:
    case SNil{}:
      False{}
    case SCon{Chr{cp}, _}:
      C.Cjk.no_end(cp)

def Fit.close.go(s: String, open: Bool) -> String:
  match s open:
    case SNil{} _:
      ""
    case SCon{_, +t} True{}:
      Fit.close.go(t, Fit.close.head(t))
    case s False{}:
      C.Cjk.rev(s, "")

def Fit.close.rev(+s: String) -> String:
  Fit.close.go(s, Fit.close.head(s))

def Fit.close(s: String) -> String:
  Fit.close.rev(C.Cjk.rev(s, ""))

def Fit.budget(+max: U32, +need: U32) -> U32:
  Bool.pick(U32, U32.is_ge(max, need), (max - need : U32), 0)

def Text.fit.k(fits: Bool, +cache: Map<&2, TG>, +f: Face, +s: String, +px: U32) -> String:
  match fits:
    case True{}:
      s
    case False{}:
      Fit.close(String.take(s, Fit.count(s, cache, f, Fit.budget((px * 64 : U32), TG.adv(Text.get(cache, f, 8230))), 0))) ++ "…"

# s cut with an ellipsis to fit px pixels
def Text.fit(+cache: Map<&2, TG>, +f: Face, +s: String, +px: U32) -> String:
  Text.fit.k(U32.is_le(Text.width(cache, f, s), px), cache, f, s, px)

# Drawing
# -------

def Text.op.k(empty: Bool, +c: D.Clip, +gx: U32, +gy: U32, +w: U32, +h: U32, d: Nat, m: R.Mask, +color: U32, rest: List<&2, R.Op>) -> List<&2, R.Op>:
  match empty:
    case True{}:
      rest
    case False{}:
      D.Clip.cut(c, gx, gy, (gx + w : U32), (gy + h : U32), color, 255, R.SMask{gx, gy, w, d, m}) <> rest

# one glyph with its pen at pixel x on the baseline
def Text.op(g: TG, +c: D.Clip, +x: U32, +base: U32, +color: U32, rest: List<&2, R.Op>) -> List<&2, R.Op>:
  match g:
    case TG{_, +l, +t, +w, +h, d, m}:
      Text.op.k(Bool.or(U32.is_eq((w * h : U32), 0), U32.is_lt((x + l : U32), 32768)), c, (x + l - 32768 : U32), (base + 32768 - t : U32), w, h, d, m, color, rest)

# Glyphs wanted
# -------------
# A glyph not in the cache draws nothing and leaves a want in its place:
# an op with an empty box far off the frame (no region keeps it) whose
# colour is the code point and whose mask names the face (gx) and its
# miss advance (gy), with no width. The window gathers them after layout
# (Text.wants), fetches those glyphs (Text.fill) and lays out again.

def Want.op(+id: U32, +cp: U32, +miss: U32) -> R.Op:
  R.Op{4294967295, 4294967295, 4294967295, 4294967295, cp, 0, R.SMask{id, miss, 0, 0n, R.MWord{0}}}

def Want.k(missing: Bool, +id: U32, +cp: U32, +miss: U32, rest: List<&2, R.Op>) -> List<&2, R.Op>:
  match missing:
    case True{}:
      Want.op(id, cp, miss) <> rest
    case False{}:
      rest

def Face.id(f: Face) -> U32:
  match f:
    case Face{id, _, _, _, _, _, _, _, _}:
      id

def Face.miss(f: Face) -> U32:
  match f:
    case Face{_, _, _, _, _, _, miss, _, _}:
      miss

def Text.run(s: String, +cache: Map<&2, TG>, +f: Face, +c: D.Clip, +x: U32, +pen: U32, +base: U32, +color: U32) -> List<&2, R.Op>:
  match s:
    case SNil{}:
      Nil{}
    case SCon{Chr{+cp}, t}:
      +g = Text.get(cache, f, cp)
      Want.k(TG.missing(g), Face.id(f), cp, Face.miss(f), Text.op(g, c, (x + Text.px(pen) : U32), base, color, Text.run(t, cache, f, c, x, (pen + TG.adv(g) : U32), base, color)))

# a glyph's id in the glyph table: its face and code point (the cache's
# key, Text.key, as a number)
def Text.gid(+face: U32, +cp: U32) -> U32:
  U32.or(U32.shln(face, 21n), U32.and(cp, 2097151))

# The glyph table (raster.bend's Tab): the cache's glyphs by id, made once
# per fill (layout.bend's Ty), for the tiles to draw text from ids

type GE is Data:
  GE{id: U32, d: Nat, m: R.Mask}

def Tab.num(s: String, acc: U32) -> U32:
  match s:
    case SNil{}:
      acc
    case SCon{Chr{+c}, t}:
      Tab.num(t, ((acc * 10 : U32) + (c - 48 : U32) : U32))

def Tab.digits(s: String) -> Bool:
  match s:
    case SNil{}:
      True{}
    case SCon{Chr{+c}, t}:
      Bool.and(Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57)), Tab.digits(t))

def Tab.key.w(empty: Bool, +id: U32, d: Nat, m: R.Mask, rest: List<&2, GE>) -> List<&2, GE>:
  match empty:
    case True{}:
      rest
    case False{}:
      GE{id, d, m} <> rest

def Tab.key.k(ok: Bool, +a: String, +b: String, g: TG, rest: List<&2, GE>) -> List<&2, GE>:
  match ok g:
    case True{} TG{_, _, _, +w, +h, d, m}:
      Tab.key.w(U32.is_eq((w * h : U32), 0), Text.gid(Tab.num(a, 0), Tab.num(b, 0)), d, m, rest)
    case False{} _:
      rest

def Tab.key.parts(ps: List<&2, String>, g: TG, rest: List<&2, GE>) -> List<&2, GE>:
  match ps:
    case +a <> +b <> Nil{}:
      Tab.key.k(Bool.and(Bool.and(Tab.digits(a), Tab.digits(b)), Bool.and(Bool.not(String.is_empty(a)), Bool.not(String.is_empty(b)))), a, b, g, rest)
    case _:
      rest

# the cache's glyphs (keys "face/cp"; others, the icons, are left out)
def Tab.entries(m: Map<&2, TG>, acc: List<&2, GE>) -> List<&2, GE>:
  match m:
    case MTip{}:
      acc
    case MLeaf{+k, v}:
      Tab.key.parts(String.split(k, '/'), v, acc)
    case MNode{_, lo, hi}:
      Tab.entries(lo, Tab.entries(hi, acc))

def GE.le(a: GE, b: GE) -> Bool:
  match a b:
    case GE{x, _, _} GE{y, _, _}:
      U32.is_le(x, y)

def Tab.node(mid: List<&2, GE>, l: R.Tab, r: R.Tab) -> R.Tab:
  match mid:
    case GE{id, d, m} <> _:
      R.TBr{id, d, m, l, r}
    case Nil{}:
      R.TNil{}

# the first n of the sorted entries as a balanced tree, d levels at most
# (none: n is 0)
def Tab.of(d: Nat, none: Bool, +xs: List<&2, GE>, +n: Nat) -> R.Tab:
  match d none:
    case 0n _:
      R.TNil{}
    case _ True{}:
      R.TNil{}
    case 1n+k0 False{}:
      +k = k0
      +h = Nat.div(n, 2n)
      +r = Nat.sub(Nat.sub(n, h), 1n)
      Tab.node(List.drop(&2, GE, xs, h), Tab.of(k, Nat.is_eq(h, 0n), List.take(&2, GE, xs, h), h), Tab.of(k, Nat.is_eq(r, 0n), List.drop(&2, GE, xs, Nat.add(h, 1n)), r))

def Text.table.k(+xs: List<&2, GE>) -> R.Tab:
  Tab.of(32n, List.is_empty(&2, GE, xs), xs, List.length(&2, GE, xs))

# the glyph table of the cache
def Text.table(m: Map<&2, TG>) -> R.Tab:
  Text.table.k(List.sort(GE, GE.le, Tab.entries(m, Nil{})))

# Runs
# ----
# A line of text is one op: its glyphs as an R.Run tree (in string order),
# shaded glyph by glyph exactly as one op per glyph was (raster.bend's
# Run.px). A glyph sits in the run's own coordinates: from the pen's start
# x and the baseline, less 32768 each (so every glyph lies right of and
# below the origin), and a moved run moves no glyph. Glyphs a frame still
# wants follow the run as want ops.

# the glyphs placed so far (last first), the box they cover in the frame,
# and the wants
type RAcc is Data:
  RAcc{gs: List<&2, R.Run>, x0: U32, y0: U32, x1: U32, y1: U32, ws: List<&2, R.Op>}

# Layout keeps ids directly; the mask-bearing path remains a reference
# for direct raster callers and byte-for-byte rendering checks.
def RAcc.glyph(keyed: Bool, +lx: U32, +ly: U32, +w: U32, +h: U32, +id: U32, d: Nat, m: R.Mask) -> R.Run:
  match keyed:
    case True{}:
      R.RKey{lx, ly, w, h, id}
    case False{}:
      R.RGlyph{lx, ly, w, h, id, d, m}

def RAcc.put.k(keyed: Bool, empty: Bool, +lx: U32, +ly: U32, +w: U32, +h: U32, +id: U32, d: Nat, m: R.Mask, +x: U32, +base: U32, a: RAcc) -> RAcc:
  match empty a:
    case True{} _:
      a
    case False{} RAcc{gs, +x0, +y0, +x1, +y1, ws}:
      RAcc{RAcc.glyph(keyed, lx, ly, w, h, id, d, m) <> gs, U32.min(x0, (x + lx - 32768 : U32)), U32.min(y0, (base + ly - 32768 : U32)),
        U32.max(x1, (x + lx + w - 32768 : U32)), U32.max(y1, (base + ly + h - 32768 : U32)), ws}

# one glyph, its pen pen pixels from x on the baseline (as Text.op placed
# it; an empty one, one left of the frame or one above its top draws
# nothing, as there)
def RAcc.put(keyed: Bool, g: TG, +id: U32, +x: U32, +pen: U32, +base: U32, a: RAcc) -> RAcc:
  match g:
    case TG{_, +l, +t, +w, +h, d, m}:
      RAcc.put.k(keyed, Bool.or(Bool.or(U32.is_eq((w * h : U32), 0), U32.is_lt((x + pen + l : U32), 32768)), U32.is_lt((base + 32768 : U32), t)),
        (pen + l : U32), (65536 - t : U32), w, h, id, d, m, x, base, a)

def RAcc.want(+id: U32, +cp: U32, +miss: U32, a: RAcc) -> RAcc:
  match a:
    case RAcc{gs, x0, y0, x1, y1, ws}:
      RAcc{gs, x0, y0, x1, y1, Want.op(id, cp, miss) <> ws}

def RAcc.want.k(missing: Bool, +id: U32, +cp: U32, +miss: U32, a: RAcc) -> RAcc:
  match missing:
    case True{}:
      RAcc.want(id, cp, miss, a)
    case False{}:
      a

# the glyphs of s placed, pen from x (26.6) on the baseline
def Text.glyphs(+keyed: Bool, s: String, +cache: Map<&2, TG>, +f: Face, +x: U32, +pen: U32, +base: U32, a: RAcc) -> RAcc:
  match s:
    case SNil{}:
      a
    case SCon{Chr{+cp}, t}:
      +g = Text.get(cache, f, cp)
      Text.glyphs(keyed, t, cache, f, x, (pen + TG.adv(g) : U32), base, RAcc.put(keyed, g, Text.gid(Face.id(f), cp), x, Text.px(pen), base, RAcc.want.k(TG.missing(g), Face.id(f), cp, Face.miss(f), a)))

# a list of runs (last first) paired up, last first still
def Run.level(xs: List<&2, R.Run>) -> List<&2, R.Run>:
  match xs:
    case a <> b <> t:
      R.Run.pair(b, a) <> Run.level(t)
    case a <> Nil{}:
      [a]
    case Nil{}:
      Nil{}

# the glyphs as a balanced tree, built level by level
def Run.up(d: Nat, xs: List<&2, R.Run>) -> R.Run:
  match d xs:
    case _ Nil{}:
      R.RNone{}
    case _ a <> Nil{}:
      a
    case 0n a <> _:
      a
    case 1n+p _:
      Run.up(p, Run.level(xs))

def Text.runop.k(empty: Bool, gs: List<&2, R.Run>, +x0: U32, +y0: U32, +x1: U32, +y1: U32, ws: List<&2, R.Op>, +c: D.Clip, +x: U32, +base: U32, +color: U32) -> List<&2, R.Op>:
  match empty:
    case True{}:
      ws
    case False{}:
      D.Clip.cut(c, x0, y0, x1, y1, color, 255, R.SRun{(x - 32768 : U32), (base - 32768 : U32), Run.up(32n, gs)}) <> ws

def Text.runop(a: RAcc, +c: D.Clip, +x: U32, +base: U32, +color: U32) -> List<&2, R.Op>:
  match a:
    case RAcc{+gs, +x0, +y0, +x1, +y1, ws}:
      Text.runop.k(List.is_empty(&2, R.Run, gs), gs, x0, y0, x1, y1, ws, c, x, base, color)

def Want.of.k(want: Bool, +id: U32, +cp: U32, +miss: U32, acc: List<&2, Want>) -> List<&2, Want>:
  match want:
    case True{}:
      Want{id, cp, miss} <> acc
    case False{}:
      acc

def Want.of(o: R.Op, acc: List<&2, Want>) -> List<&2, Want>:
  match o:
    case R.Op{_, _, _, _, +cp, +a, R.SMask{+id, +miss, +w, _, _}}:
      Want.of.k(Bool.and(U32.is_eq(w, 0), U32.is_eq(a, 0)), id, cp, miss, acc)
    case _:
      acc

# the glyphs a frame's ops want (tail recursive: frames have many ops)
def Text.wants(ops: List<&2, R.Op>, acc: List<&2, Want>) -> List<&2, Want>:
  match ops:
    case Nil{}:
      acc
    case o <> t:
      Text.wants(t, Want.of(o, acc))

def Text.draw.k(ft: Bool, +keyed: Bool, s: String, +cache: Map<&2, TG>, +f: Face, +c: D.Clip, +x: U32, +base: U32, +color: U32) -> List<&2, R.Op>:
  match ft:
    case True{}:
      Text.runop(Text.glyphs(keyed, s, cache, f, x, 0, base, RAcc{Nil{}, 4294967295, 4294967295, 0, 0, Nil{}}), c, x, base, color)
    case False{}:
      D.Draw.text(s, c, x, (base - 12 : U32), color)

# one line of text, pen starting at x on the given baseline
def Text.draw(+cache: Map<&2, TG>, +f: Face, s: String, +c: D.Clip, +x: U32, +base: U32, +color: U32) -> List<&2, R.Op>:
  Text.draw.k(Face.ft(f), False{}, s, cache, f, c, x, base, color)

# one line of text in a line box of height lh whose top is y
def Text.line(+cache: Map<&2, TG>, +f: Face, s: String, +c: D.Clip, +x: U32, +y: U32, +lh: U32, +color: U32) -> List<&2, R.Op>:
  Text.draw(cache, f, s, c, x, Text.baseline(f, y, lh), color)

# The app's line layout stores only glyph ids and geometry, so the tile
# planner compares it without a per-frame mask-to-key conversion.
def Text.keydraw(+cache: Map<&2, TG>, +f: Face, s: String, +c: D.Clip, +x: U32, +base: U32, +color: U32) -> List<&2, R.Op>:
  Text.draw.k(Face.ft(f), True{}, s, cache, f, c, x, base, color)

def Text.keyline(+cache: Map<&2, TG>, +f: Face, s: String, +c: D.Clip, +x: U32, +y: U32, +lh: U32, +color: U32) -> List<&2, R.Op>:
  Text.keydraw(cache, f, s, c, x, Text.baseline(f, y, lh), color)
