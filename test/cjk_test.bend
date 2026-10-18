import Base
import ./check.bend as T
import ../src/core/cjk.bend as C
import ../src/core/vt.bend as VT
import ../src/gfx/text.bend as X
import ../src/core/edit.bend as E

# A synthetic FreeType face: one pixel per cell, exactly two for wide
# characters. No system fonts or effects are used by these tests.
def face() -> X.Face:
  X.Face{97, True{}, 1, 1, 0, 1, 64, "synthetic cells", X.GTab.none(7n)}

def cache(s: String, acc: Map<&2, X.TG>) -> Map<&2, X.TG>:
  match s:
    case SNil{}:
      acc
    case SCon{Chr{+cp}, t}:
      cache(t, Map.set(&2, X.TG, acc, X.Text.key(97, cp), X.TG.blank(Bool.pick(U32, C.Cjk.wide(cp), 128, 64))))

def sample() -> String:
  "日本語漢字かなカナ。、「」（）ーあいうえおきくけこ新しいスレッドボットプロジェクト設定PRを開いたテストは通過ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789 []{}(),.:;!?…"

def wrap(+c: Map<&2, X.TG>, s: String, +w: U32) -> List<&2, String>:
  X.Text.wrap(c, face(), s, w)

def segs(ss: List<&2, C.Seg>, acc: String) -> String:
  match ss:
    case Nil{}:
      acc
    case C.Seg{s, gap} <> t:
      segs(t, acc ++ Bool.pick(String, gap, "+", "-") ++ s ++ "|")

def lossless(+s: String) -> IO(Unit):
  T.Check.str("segments reconstruct [" ++ s ++ "]", C.Cjk.join(C.Cjk.segs(s)), s)

def safe(s: String, prev: U32, first: Bool) -> Bool:
  match s:
    case SNil{}:
      Bool.not(C.Cjk.no_end(prev))
    case SCon{Chr{+cp}, t}:
      Bool.and(Bool.not(Bool.and(first, C.Cjk.no_start(cp))), safe(t, cp, False{}))

def lines.ok(ls: List<&2, String>, +c: Map<&2, X.TG>, +w: U32) -> Bool:
  match ls:
    case Nil{}:
      True{}
    case +s <> t:
      safe(s, 0, True{}) && U32.is_le(X.Text.width(c, face(), s), w) && lines.ok(t, c, w)

# Reinsert exactly the original break delimiter, if one was dropped.
# A kana boundary has no delimiter; retained runs of spaces stay in l.
def restore(ls: List<&2, String>, +source: String, acc: String) -> String:
  match ls:
    case Nil{}:
      acc
    case +l <> t:
      +rest = String.drop(source, String.length(l))
      +gap = E.Edit.gap(rest)
      restore(t, String.drop(rest, gap), acc ++ l ++ String.take(rest, gap))

def positions(ns: List<&2, Nat>, acc: String) -> String:
  match ns:
    case Nil{}:
      acc
    case n <> t:
      positions(t, acc ++ Nat.show(n) ++ "|")

def paragraph(+c: Map<&2, X.TG>, +s: String, +w: U32) -> IO(Unit):
  do IO<Unit>:
    T.Check.str("wrapped reconstruction", restore(wrap(c, s, w), s, ""), s)
    T.Check.str("width and kinsoku", Bool.show(lines.ok(wrap(c, s, w), c, w)), "True")

def starts(s: String) -> Bool:
  match s:
    case SNil{}:
      True{}
    case SCon{Chr{+cp}, t}:
      Bool.not(C.Cjk.can_break(12354, cp)) && starts(t)

def ends(s: String) -> Bool:
  match s:
    case SNil{}:
      True{}
    case SCon{Chr{+cp}, t}:
      Bool.not(C.Cjk.can_break(cp, 12354)) && ends(t)

def long.text(n: Nat, acc: String) -> String:
  match n:
    case 0n:
      acc
    case 1n+k:
      long.text(k, SCon{'あ', acc})

def long.count(s: String, +ok: Bool, +n: U32) -> U32:
  match s:
    case SNil{}:
      Bool.pick(U32, ok, n, 0)
    case SCon{Chr{cp}, t}:
      long.count(t, Bool.and(ok, U32.is_eq(cp, 12354)), (n + 1 : U32))

def long.lines(ls: List<&2, String>, +ok: Bool, +n: U32) -> U32:
  match ls:
    case Nil{}:
      Bool.pick(U32, ok, n, 0)
    case +l <> t:
      +count = long.count(l, True{}, 0)
      long.lines(t, Bool.and(ok, U32.is_le(count, 4)), (n + count : U32))

def main() -> IO(Unit):
  do IO<Unit>:
    +c : Map<&2, X.TG> = cache(sample(), Map.new(&2, X.TG))
    T.Check.str("Japanese segments", segs(C.Cjk.segs("日本語。"), ""), "-日|-本|-語。|")
    T.Check.str("mixed segments", segs(C.Cjk.segs("PRを開いた。"), ""), "-PR|-を|-開|-い|-た。|")
    T.Check.str("quote group", segs(C.Cjk.segs("あ「い。」う"), ""), "-あ|-「い。」|-う|")
    lossless("")
    lossless(" ")
    lossless("  日本")
    lossless("日本  語")
    lossless("日本  ")
    lossless("  PRを 開いた。  ")
    lossless("a\nb\t日本")
    T.Check.str("all no-start punctuation", Bool.show(starts("、。，．・：；？！ー～）」』】〕〉》｝］｠〙〗’”ぁぃぅぇぉっゃゅょゎァィゥェォッャュョヮヵヶ々〻‐゠–〜)]},.:;!?")), "True")
    T.Check.str("all no-end punctuation", Bool.show(ends("（「『【〔〈《｛［｟〘〖‘“([{")), "True")
    T.Check.str("ASCII never breaks", Bool.show(C.Cjk.can_break(65, 66)), "False")
    T.Check.str("kana break", Bool.show(C.Cjk.can_break(12354, 12356)), "True")
    T.Check.str("wide supplementary", Bool.show(C.Cjk.wide(131072)), Bool.show(VT.Vt.wide(131072)))
    T.Check.str("exact two-cell lines", String.join(wrap(c, "あいうえおきくけ", 8), "|"), "あいうえ|おきくけ")
    paragraph(c, "日本語。「かなカナ」、新しいスレッド。", 12)
    paragraph(c, "PRを開いた。テストは通過", 10)
    paragraph(c, "ああ「いいうう」、ええーおお。", 8)
    T.Check.str("wrap reconstructs spaces and break gap", restore(wrap(c, "  PRを  開いた。  ", 8), "  PRを  開いた。  ", ""), "  PRを  開いた。  ")
    paragraph(c, "あ「abcdefghijklmnop」い", 6)
    T.Check.str("quoted long Latin boundaries", String.join(wrap(c, "あ「abcdefghijklmnop」い", 6), "|"), "あ|「abcd|efghij|klmno|p」い")
    T.Check.str("mixed long Latin token", String.join(wrap(c, "あabcdefghijklmnopい", 5), ""), "あabcdefghijklmnopい")
    T.Check.str("space break exact lines", String.join(wrap(c, "PRあい testかな", 6), "|"), "PRあい|testか|な")
    T.Check.str("space break caret starts", positions(E.Edit.starts(wrap(c, "PRあい testかな", 6), "PRあい testかな"), ""), "0|5|10|")
    T.Check.str("kana break exact lines", String.join(wrap(c, "PRあいうえtest", 6), "|"), "PRあい|うえ|test")
    T.Check.str("kana break caret starts", positions(E.Edit.starts(wrap(c, "PRあいうえtest", 6), "PRあいうえtest"), ""), "0|4|6|")
    T.Check.str("protected unicode wrap", String.join(X.Text.wrap(Map.new(&2, X.TG), X.Text.bitmap(), "日本語 😀🚀🙂\nabcde  x\n", 24), "|"), "日本語|😀🚀🙂|abc|de |x|")
    T.Check.str("empty wrap", String.join(wrap(c, "", 8), "|"), "")
    T.Check.str("ASCII baseline", String.join(X.Text.wrap(Map.new(&2, X.TG), X.Text.bitmap(), "alpha beta gamma delta", 80), "|"), "alpha beta|gamma|delta")
    T.Check.str("ASCII spaces baseline", String.join(X.Text.wrap(Map.new(&2, X.TG), X.Text.bitmap(), " leading  middle trailing ", 80), "|"), " leading |middle|trailing ")
    T.Check.str("ASCII chunk baseline", String.join(X.Text.wrap(Map.new(&2, X.TG), X.Text.bitmap(), "abcdefghijklmnop", 40), "|"), "abcde|fghij|klmno|p")
    T.Check.str("ellipsis drops opening punctuation", X.Text.fit(c, face(), "あ「いうえお", 5), "あ…")
    T.Check.str("ellipsis narrow budget", X.Text.fit(c, face(), "あいう", 0), "…")
    T.Check.str("9k CJK segment reconstruction", U32.show(long.count(C.Cjk.join(C.Cjk.segs(long.text(9000n, ""))), True{}, 0)), "9000")
    T.Check.str("9k CJK wrap content and widths", U32.show(long.lines(wrap(c, long.text(9000n, ""), 8), True{}, 0)), "9000")
    T.Check.str("ellipsis keeps code points", X.Text.fit(c, face(), "あいうえお", 5), "あい…")
