import Base
import ./check.bend as C
import ../src/server/thin-view.bend as V
import ../src/core/thin.bend as T
import ../src/core/model.bend as M
import ../src/core/review.bend as RV

# Review ops are semantic records, independent of the bounded chat body.
def comment(+thread: String, +id: String, +text: String) -> M.Change:
  M.ActivityLogged{thread, id, "review", "add", RV.Review.op.add(id, RV.Anchor{"board.kicad_pcb", "", "board", "", 12.0, 24.0, ""}, text, "person", ""), 2n}

def state() -> M.State:
  M.State.apply(M.State.apply(M.State.apply(M.State.apply(M.State.empty(),
    M.ThreadCreated{"a", "", "Selected", M.Local{}, "", "", "codex", "", 1n}),
    comment("a", "long", String.repeat("Comment with context. ", 40n))),
    comment("other", "private", "Other thread review")),
    M.MessagePosted{"a", "newest", M.Assistant{}, "Newer chat", 3n})

def window(+st: M.State, bounded: Bool, rows: List<&2, String>) -> M.State:
  V.State.window.body(bounded, st, T.Index.of(st), V.Target.json("thread", "a"), rows)

def check(+bounded: Bool) -> IO(Unit):
  do IO<Unit>:
    +st : M.State = state()
    +empty : M.State = window(st, bounded, Nil{})
    +shown : M.State = window(st, bounded, ["1"])
    C.Check.str("old review survives empty chat page " ++ Bool.show(bounded), Nat.show(List.length(&2, RV.Comment, RV.Review.of_state(empty, "a"))), "1")
    C.Check.str("long review survives clipped visible activity " ++ Bool.show(bounded), RV.Review.md(RV.Review.of_state(shown, "a")), RV.Review.md(RV.Review.of_state(st, "a")))
    C.Check.str("foreign review stays outside projection " ++ Bool.show(bounded), Nat.show(List.length(&2, M.Entry, M.State.reviews(shown, "other"))), "0")
    C.Check.str("chat page stays bounded " ++ Bool.show(bounded), Nat.show(List.length(&2, M.Entry, M.State.mine(empty, "a"))), "0")
    C.Check.str("canonical review is unchanged " ++ Bool.show(bounded), Nat.show(List.length(&2, RV.Comment, RV.Review.of_state(st, "a"))), "1")

def main() -> IO(Unit):
  do IO<Unit>:
    check(False{})
    check(True{})
