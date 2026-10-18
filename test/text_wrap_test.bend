import Base
import ./check.bend as C
import ../src/core/i18n.bend as I
import ../src/gfx/text.bend as X
import ../src/app/bots.bend as BV
import ../src/app/layout.bend as L
import ../src/core/bot.bend as BT
import ../src/core/botui.bend as BU
import ../src/core/client.bend as U
import ../src/core/edit.bend as E
import ../src/core/cjk.bend as CJ
import ../src/app/nui.bend as N

# The previous eager implementation, retained only as an output oracle.
def Old.count(s: String, +cache: Map<&2, X.TG>, +f: X.Face, +max: U32, +used: U32) -> Nat:
  match s:
    case SNil{}:
      0n
    case SCon{Chr{+cp}, t}:
      +u2 = (used + X.TG.adv(X.Text.get(cache, f, cp)) : U32)
      X.Fit.count.k(U32.is_le(u2, max), Old.count(t, cache, f, max, u2))

# characters that fit, at least one; computed once per chunk
def Old.take.k(+c: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_eq(c, 0n), 1n, c)

def Old.take(+s: String, +cache: Map<&2, X.TG>, +f: X.Face, +max: U32) -> Nat:
  Old.take.k(Old.count(s, cache, f, max, 0))

def Old.chunk.go(n: Nat, empty: Bool, +k: Nat, +s: String, +cache: Map<&2, X.TG>, +f: X.Face, +max: U32) -> List<&2, String>:
  match n empty:
    case 1n+m False{}:
      +r = String.drop(s, k)
      String.take(s, k) <> Old.chunk.go(m, String.is_empty(r), Old.take(r, cache, f, max), r, cache, f, max)
    case _ _:
      Nil{}

def Old.chunk(n: Nat, empty: Bool, +s: String, +cache: Map<&2, X.TG>, +f: X.Face, +max: U32) -> List<&2, String>:
  Old.chunk.go(n, empty, Old.take(s, cache, f, max), s, cache, f, max)

def Old.chunks(empty: Bool, +w: String, +cache: Map<&2, X.TG>, +f: X.Face, +max: U32) -> List<&2, String>:
  match empty:
    case True{}:
      [""]
    case False{}:
      Old.chunk(1n+String.length(w), False{}, w, cache, f, max)

def Old.words(ws: List<&2, String>, a: X.PAcc, +cache: Map<&2, X.TG>, +f: X.Face, +sp: U32, +max: U32) -> X.PAcc:
  match ws:
    case Nil{}:
      a
    case +w <> t:
      Old.words(t, X.PWrap.pieces(Old.chunks(String.is_empty(w), w, cache, f, max), a, cache, f, sp, max), cache, f, sp, max)

def Old.line(s: String, +cache: Map<&2, X.TG>, +f: X.Face, +px: U32) -> List<&2, String>:
  X.PWrap.fin(Old.words(String.split(s, ' '), X.PAcc{Nil{}, "", 0, True{}}, cache, f, X.TG.adv(X.Text.get(cache, f, 32)), (px * 64 : U32)))

def Old.lines(ls: List<&2, String>, +cache: Map<&2, X.TG>, +f: X.Face, +px: U32) -> List<&2, String>:
  match ls:
    case Nil{}:
      Nil{}
    case l <> t:
      List.append(&2, String, Old.line(l, cache, f, px), Old.lines(t, cache, f, px))

def Old.wrap(+cache: Map<&2, X.TG>, +f: X.Face, s: String, +px: U32) -> List<&2, String>:
  Old.lines(String.split(s, '\n'), cache, f, px)

# Length-prefixed lines compare exact boundaries, including empty lines.
def show(ls: List<&2, String>, acc: String) -> String:
  match ls:
    case Nil{}:
      acc
    case +l <> t:
      show(t, acc ++ Nat.show(String.length(l)) ++ ":" ++ l ++ "|")

def word(n: Nat, acc: String) -> String:
  match n:
    case 0n:
      acc
    case 1n+k:
      word(k, SCon{Chr{120}, acc})

def compare(+cache: Map<&2, X.TG>, +f: X.Face, +s: String, +px: U32, name: String) -> IO(Unit):
  C.Check.str(name, show(X.Text.wrap(cache, f, s, px), ""), show(Old.wrap(cache, f, s, px), ""))

def cjk.expected.plain.ja(px: U32) -> List<&2, String>:
  match px:
    case 0:
      ["日", "本", "語", "漢", "字", "か", "な", "カ", "ナ", "日", "本", "語"]
    case 1:
      ["日", "本", "語", "漢", "字", "か", "な", "カ", "ナ", "日", "本", "語"]
    case 7:
      ["日", "本", "語", "漢", "字", "か", "な", "カ", "ナ", "日", "本", "語"]
    case 8:
      ["日", "本", "語", "漢", "字", "か", "な", "カ", "ナ", "日", "本", "語"]
    case 16:
      ["日本", "語漢", "字か", "なカ", "ナ", "日本", "語"]
    case 24:
      ["日本語", "漢字か", "なカナ", "日本語"]
    case 97:
      ["日本語漢字かなカナ 日本", "語"]
    case 780:
      ["日本語漢字かなカナ 日本語"]
    case _:
      Nil{}

def cjk.expected.plain.emoji(px: U32) -> List<&2, String>:
  match px:
    case 0:
      ["😀", "🚀", "🙂", "👩", "‍", "🔬", "🏳", "️", "‍", "🌈", "👍", "🏽"]
    case 1:
      ["😀", "🚀", "🙂", "👩", "‍", "🔬", "🏳", "️", "‍", "🌈", "👍", "🏽"]
    case 7:
      ["😀", "🚀", "🙂", "👩", "‍", "🔬", "🏳", "️", "‍", "🌈", "👍", "🏽"]
    case 8:
      ["😀", "🚀", "🙂", "👩", "‍", "🔬", "🏳", "️", "‍", "🌈", "👍", "🏽"]
    case 16:
      ["😀🚀", "🙂👩", "‍🔬", "🏳", "️‍", "🌈", "👍🏽"]
    case 24:
      ["😀🚀🙂", "👩‍🔬", "🏳️‍", "🌈 👍", "🏽"]
    case 97:
      ["😀🚀🙂👩‍🔬 🏳️‍🌈", "👍🏽"]
    case 780:
      ["😀🚀🙂👩‍🔬 🏳️‍🌈 👍🏽"]
    case _:
      Nil{}

def cjk.expected.custom.ja(px: U32) -> List<&2, String>:
  match px:
    case 0:
      ["日", "本", "語", "漢", "字", "か", "な", "カ", "ナ", "日", "本", "語"]
    case 8:
      ["日", "本", "語", "漢", "字", "か", "な", "カ", "ナ", "日", "本", "語"]
    case 24:
      ["日本", "語漢字", "かなカ", "ナ", "日本", "語"]
    case 97:
      ["日本語漢字かなカナ", "日本語"]
    case _:
      Nil{}

def cjk.expected.custom.emoji(px: U32) -> List<&2, String>:
  match px:
    case 0:
      ["😀", "🚀", "🙂", "👩", "‍", "🔬", "🏳", "️", "‍", "🌈", "👍", "🏽"]
    case 8:
      ["😀", "🚀", "🙂", "👩‍", "🔬", "🏳", "️‍", "🌈", "👍", "🏽"]
    case 24:
      ["😀", "🚀🙂👩‍", "🔬 🏳", "️‍🌈", "👍🏽"]
    case 97:
      ["😀🚀🙂👩‍🔬 🏳️‍🌈 👍🏽"]
    case _:
      Nil{}

def cjk.expected(custom: Bool, +px: U32, emoji: Bool) -> List<&2, String>:
  match custom emoji:
    case False{} False{}:
      cjk.expected.plain.ja(px)
    case False{} True{}:
      cjk.expected.plain.emoji(px)
    case True{} False{}:
      cjk.expected.custom.ja(px)
    case True{} True{}:
      cjk.expected.custom.emoji(px)

def cjk.edge(s: String, prev: U32, first: Bool) -> Bool:
  match s:
    case SNil{}:
      Bool.not(CJ.Cjk.no_end(prev))
    case SCon{Chr{+cp}, t}:
      Bool.not(Bool.and(first, CJ.Cjk.no_start(cp))) && cjk.edge(t, cp, False{})

def cjk.edges(ls: List<&2, String>) -> Bool:
  match ls:
    case Nil{}:
      True{}
    case l <> t:
      cjk.edge(l, 0, True{}) && cjk.edges(t)

def cjk.restore(ls: List<&2, String>, +source: String, acc: String) -> String:
  match ls:
    case Nil{}:
      acc
    case +l <> t:
      +rest = String.drop(source, String.length(l))
      +gap = E.Edit.gap(rest)
      cjk.restore(t, String.drop(rest, gap), acc ++ l ++ String.take(rest, gap))

def cjk.sample(+cache: Map<&2, X.TG>, +f: X.Face, +s: String, +px: U32, +emoji: Bool) -> IO(Unit):
  do IO<Unit>:
    +ls : List<&2, String> = X.Text.wrap(cache, f, s, px)
    C.Check.str("CJK/emoji exact boundaries", show(ls, ""), show(cjk.expected(U32.is_eq(X.TG.adv(X.Text.get(cache, f, 26085)), 1024), px, emoji), ""))
    C.Check.str("CJK/emoji reconstruct break gaps", cjk.restore(ls, s, ""), s)
    C.Check.str("CJK/emoji kinsoku", Bool.show(cjk.edges(ls)), "True")

def samples(+cache: Map<&2, X.TG>, +f: X.Face, +px: U32) -> IO(Unit):
  do IO<Unit>:
    compare(cache, f, "", px, "empty")
    compare(cache, f, "plain words and a longer sentence", px, "normal")
    compare(cache, f, "  leading  repeated spaces  ", px, "spaces")
    compare(cache, f, "\n\nline one\nline two\n", px, "newlines")
    cjk.sample(cache, f, "日本語漢字かなカナ 日本語", px, False{})
    cjk.sample(cache, f, "😀🚀🙂👩‍🔬 🏳️‍🌈 👍🏽", px, True{})
    compare(cache, f, "aaaa bbb ccc xx a", px, "variable advances")
    compare(cache, f, word(Bool.pick(Nat, U32.is_lt(px, 8), 128n, 2048n), ""), px, "long unbroken oracle")

def widths(ws: List<&2, U32>, +cache: Map<&2, X.TG>, +f: X.Face) -> IO(Unit):
  match ws:
    case Nil{}:
      IO.pure(Unit, Unit{})
    case w <> t:
      IO.bind(Unit, Unit, samples(cache, f, w), u => widths(t, cache, f))

# Synthetic fractional advances, wider CJK and emoji, and zero-width
# joiners exercise cached metrics independently of the host's fonts.
def metrics() -> Map<&2, X.TG>:
  Map.set(&2, X.TG, Map.set(&2, X.TG, Map.set(&2, X.TG, Map.set(&2, X.TG,
    Map.set(&2, X.TG, Map.new(&2, X.TG), X.Text.key(0, 97), X.TG.blank(211)),
    X.Text.key(0, 32), X.TG.blank(129)), X.Text.key(0, 26085), X.TG.blank(1024)),
    X.Text.key(0, 128512), X.TG.blank(1280)), X.Text.key(0, 8205), X.TG.blank(0))

def ty(+f: X.Face) -> L.Ty:
  L.Ty{Map.new(&2, X.TG), f, f, f, f, f, I.En{}}

def post(+id: String, +text: String) -> List<&2, BT.Post>:
  [BT.Post{id, "", "you", text, 0n, 1n}]

def got(+ws: Map<&2, BV.PW>, +room: String, +w: U32, +id: String) -> BV.PW:
  BV.Bv.wrap.got(Map.get(BV.PW, BV.PW{False{}, "", Nil{}}, BV.Bv.wraps.group(ws, room, w), BV.Bv.wrap.key(id, w)))

def has(p: BV.PW) -> String:
  match p:
    case BV.PW{True{}, _, _}:
      "yes"
    case _:
      "no"

def seed(+id: String, +w: U32, text: String, lines: List<&2, String>) -> Map<&2, BV.PW>:
  Map.set(&2, BV.PW, BV.Bv.wraps.none(), BV.Bv.wrap.key(id, w), BV.PW{True{}, text, lines})

def fill(+n: Nat, +ty: L.Ty, ws: Map<&2, BV.PW>) -> Map<&2, BV.PW>:
  match n:
    case 0n:
      ws
    case 1n+k:
      fill(k, ty, BV.Bv.wraps.conversation(post("same-id", "hello"), Nat.show(k), ws, ty, 24))

def cache() -> IO(Unit):
  do IO<Unit>:
    +t : L.Ty = ty(X.Text.bitmap())
    +a : Map<&2, BV.PW> = BV.Bv.wraps.keep(BV.Bv.wraps.none(), BV.Bv.wraps.key("a", 24), seed("p", 24, "hello", ["cached hit"]))
    +b : Map<&2, BV.PW> = BV.Bv.wraps.conversation(post("p", "world"), "b", a, t, 24)
    +back : Map<&2, BV.PW> = BV.Bv.wraps.conversation(post("p", "hello"), "a", b, t, 24)
    C.Check.str("switch back uses cached lines", show(BV.Bv.wrap(BV.Bv.wraps.group(back, "a", 24), t, "p", "hello", 24), ""), "10:cached hit|")
    C.Check.str("same post id in another conversation", show(BV.Bv.wrap(BV.Bv.wraps.group(back, "b", 24), t, "p", "world", 24), ""), "3:wor|2:ld|")
    +wide : Map<&2, BV.PW> = BV.Bv.wraps.conversation(post("p", "hello"), "a", back, t, 80)
    C.Check.str("width variant retained", has(got(wide, "a", 24, "p")), "yes")
    C.Check.str("new width wraps separately", show(BV.Bv.wrap(BV.Bv.wraps.group(wide, "a", 80), t, "p", "hello", 80), ""), "5:hello|")
    +changed : Map<&2, BV.PW> = BV.Bv.wraps.conversation(post("p", "changed"), "a", wide, t, 24)
    C.Check.str("changed text invalidates", show(BV.Bv.wrap(BV.Bv.wraps.group(changed, "a", 24), t, "p", "changed", 24), ""), "3:cha|3:nge|1:d|")
    +gone : Map<&2, BV.PW> = BV.Bv.wraps.conversation(Nil{}, "a", changed, t, 24)
    C.Check.str("removed posts pruned", has(got(gone, "a", 24, "p")), "no")
    +eight : Map<&2, BV.PW> = fill(8n, t, BV.Bv.wraps.none())
    +recent : Map<&2, BV.PW> = BV.Bv.wraps.conversation(post("same-id", "hello"), "7", eight, t, 24)
    +nine : Map<&2, BV.PW> = BV.Bv.wraps.conversation(post("same-id", "hello"), "new", recent, t, 24)
    C.Check.str("eviction frees group maps", Nat.show(List.length(&2, String, Map.keys(&2, BV.PW, nine))), "9")
    C.Check.str("eight groups bounded", Nat.show(List.length(&2, String, BV.Bv.wraps.keys(nine))), "8")
    C.Check.str("recent group retained", has(got(nine, "7", 24, "same-id")), "yes")
    C.Check.str("least recent group evicted", has(got(nine, "6", 24, "same-id")), "no")
    C.Check.str("ordinary thread preserves groups", Nat.show(List.length(&2, String, BV.Bv.wraps.keys(BV.Bv.wraps.of(BU.VThread{}, U.Ui.init(1n), nine, t, 24)))), "8")
    C.Check.str("Settings preserves groups", Nat.show(List.length(&2, String, BV.Bv.wraps.keys(BV.Bv.wraps.k(True{}, t, N.Nui.init(U.Ui.init(1n), 1400, 900), nine)))), "8")
    C.Check.str("explicit glyph reset clears groups", Nat.show(List.length(&2, String, BV.Bv.wraps.keys(BV.Bv.wraps.none()))), "0")

# Count/check every code point without a recursive equality over one
# giant String (the default test runner has a much smaller stack).
def allx(s: String, +ok: Bool) -> Bool:
  match s:
    case SNil{}:
      ok
    case SCon{Chr{cp}, t}:
      allx(t, Bool.and(ok, U32.is_eq(cp, 120)))

def shape(ls: List<&2, String>, +ok: Bool) -> Bool:
  match ls:
    case Nil{}:
      ok
    case +l <> t:
      shape(t, Bool.and(ok, Bool.and(allx(l, True{}), Nat.is_le(String.length(l), 97n))))

def total(ls: List<&2, String>, acc: Nat) -> Nat:
  match ls:
    case Nil{}:
      acc
    case l <> t:
      total(t, Nat.add(acc, String.length(l)))

# A size that made the old suffix walk quadratic. Check content and count
# without running that quadratic oracle at 9k in the test.
def long() -> IO(Unit):
  do IO<Unit>:
    +s : String = word(9000n, "")
    +ls : List<&2, String> = X.Text.wrap(Map.new(&2, X.TG), X.Text.bitmap(), s, 776)
    C.Check.str("9k unbroken content and widths", Bool.pick(String, shape(ls, True{}), "yes", "no"), "yes")
    C.Check.str("9k all characters kept", Nat.show(total(ls, 0n)), "9000")
    C.Check.str("9k unbroken line count", Nat.show(List.length(&2, String, ls)), "93")
    C.Check.str("9k last piece", Nat.show(String.length(Maybe.default(&2, String, List.last(&2, String, ls), ""))), "76")
    C.Check.str("line starts consume suffix", Nat.show(Maybe.default(&2, Nat, List.last(&2, Nat, E.Edit.starts(ls, s)), 0n)), "8924")

def main() -> IO(Unit):
  do IO<Unit>:
    widths([0, 1, 7, 8, 16, 24, 97, 780], Map.new(&2, X.TG), X.Text.bitmap())
    widths([0, 8, 24, 97], metrics(), X.Text.bitmap())
    long()
    cache()
