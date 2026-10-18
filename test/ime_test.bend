import Base
import ./check.bend as C
import ../src/core/ime.bend as I

def b(x: Bool) -> String:
  Bool.pick(String, x, "yes", "no")

def runs(rs: List<&2, I.Run>) -> String:
  match rs:
    case Nil{}:
      ""
    case I.Run{lo, hi, hot} <> t:
      Nat.show(lo) ++ "-" ++ Nat.show(hi) ++ Bool.pick(String, hot, "!", ".") ++ " " ++ runs(t)

def main() -> IO(Unit):
  # にほんご typed, then the second clause converted
  +a = I.Ime.draw(I.Ime.none(), "t:composer", 0n, 0n, "にほんご", "2222")
  +c = I.Ime.draw(a, "t:composer", 0n, 4n, "日本語", "122")
  +f = I.Ime.draw(a, "t:composer", 1n, 2n, "ホン", "11")
  +m = I.Ime.mark(a, 1n, 2n, "44")
  +w = I.Ime.draw(I.Ime.none(), "o", 0n, 0n, "a😀日", "012")
  do IO<Unit>:
    C.Check.str("a draft is not touched", I.Ime.shown(I.Ime.none(), "abc", 1n), "abc")
    C.Check.str("shown at the caret", I.Ime.shown(a, "abcd", 2n), "abにほんごcd")
    C.Check.str("caret inside the preedit", Nat.show(I.Ime.caret(I.Ime.at(a, 2n), 2n)), "4")
    C.Check.str("caret past the end is clamped", Nat.show(I.Ime.caret(I.Ime.at(a, 99n), 2n)), "6")
    C.Check.str("a replaces the whole", I.Ime.text(c), "日本語")
    C.Check.str("replacement fits the feedback", I.Ime.show(c), "t:composer\u{1f}0\u{1f}122\u{1f}日本語")
    C.Check.str("an inner range", I.Ime.text(f), "にホンご")
    C.Check.str("an inner range's feedback", I.Ime.show(f), "t:composer\u{1f}0\u{1f}2112\u{1f}にホンご")
    C.Check.str("feedback only keeps the text", I.Ime.text(m), "にほんご")
    C.Check.str("feedback only changes the marks", runs(I.Ime.runs(m, 0n)), "0-1. 1-3! 3-4. ")
    C.Check.str("runs are offset by the caret", runs(I.Ime.runs(m, 5n)), "5-6. 6-8! 8-9. ")
    C.Check.str("one clause", runs(I.Ime.runs(I.Ime.draw(I.Ime.none(), "o", 0n, 0n, "あ", "4"), 0n)), "0-1! ")
    C.Check.str("a wide and an astral character count once", Nat.show(I.Ime.len(w)), "3")
    C.Check.str("a deletion", I.Ime.text(I.Ime.draw(a, "t:composer", 1n, 2n, "", "")), "にご")
    C.Check.str("a deletion past the end is clamped", I.Ime.text(I.Ime.draw(a, "t:composer", 3n, 9n, "", "")), "にほん")
    C.Check.str("an insertion", I.Ime.text(I.Ime.draw(a, "t:composer", 2n, 0n, "ー", "2")), "にほーんご")
    C.Check.str("controls are dropped", I.Ime.text(I.Ime.draw(I.Ime.none(), "o", 0n, 0n, "a\nb\u{1f}c", "00000")), "abc")
    C.Check.str("cleared is off", b(I.Ime.on(I.Ime.clear(a))), "no")
    C.Check.str("emptied is off", b(I.Ime.on(I.Ime.draw(a, "t:composer", 0n, 4n, "", ""))), "no")
    C.Check.str("another owner starts afresh", I.Ime.text(I.Ime.draw(a, "t:other", 0n, 0n, "x", "2")), "x")
    C.Check.str("another owner's range is not applied", I.Ime.show(I.Ime.draw(a, "t:other", 1n, 2n, "x", "2")), "t:other\u{1f}0\u{1f}2\u{1f}x")
    C.Check.str("the owner keeps it", b(I.Ime.on(I.Ime.sync(a, "t:composer"))), "yes")
    C.Check.str("a focus change drops it", b(I.Ime.on(I.Ime.sync(a, "t:other"))), "no")
    C.Check.str("a dropped one leaves the text alone", I.Ime.shown(I.Ime.sync(a, "t:other"), "abcd", 2n), "abcd")
    C.Check.str("kept as text", I.Ime.show(I.Ime.read(I.Ime.show(c))), I.Ime.show(c))
    C.Check.str("unreadable is off", b(I.Ime.on(I.Ime.read("x"))), "no")
    C.Check.str("empty is off", b(I.Ime.on(I.Ime.read(""))), "no")
