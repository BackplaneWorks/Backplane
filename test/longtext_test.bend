import Base
import ../src/core/i18n.bend as I
import ./check.bend as C
import ../src/core/client.bend as U
import ../src/core/diff.bend as DF
import ../src/core/attach.bend as AT
import ../src/core/refs.bend as RF
import ../src/core/model.bend as M
import ../src/core/hist.bend as HI

# Long texts the clients label or search a line at a time: a fork's
# handed-over context (thousands of characters) went through String.lines,
# which recurses once per character, and the browser stopped drawing the
# fork (it could not be opened or deleted). These walks never nest.

# a long text: s doubled n times (tail-recursive)
def Long.go(n: Nat, +s: String) -> String:
  match n:
    case 0n:
      s
    case 1n+k:
      Long.go(k, s ++ s)

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("first line", DF.Diff.first("one\ntwo"), "one")
    C.Check.str("first line, no break", DF.Diff.first("one"), "one")
    C.Check.str("first line, empty", DF.Diff.first(""), "")
    C.Check.str("a fork's context labels its first line", U.Qu.label(I.En{}, "handoff:t1", "[Backplane] This thread is a fork of Board bringup\n" ++ Long.go(16n, "earlier conversation. ")), "This thread is a fork of Board bringup (goes with the next message)")
    C.Check.str("a long text's first line", Nat.show(String.length(DF.Diff.first(Long.go(16n, "x") ++ "\nnext"))), "65536")
    C.Check.str("a long text's lines", Nat.show(String.length(String.join(DF.Diff.lines(Long.go(16n, "word ") ++ "\n" ++ Long.go(16n, "more ")), "|"))), "655361")
    # a pasted log in a message or a draft: its body, the word being typed,
    # the references it makes (each was a walk per character or per line)
    +log : String = Long.go(14n, "line of a log\n")
    C.Check.str("a long message's body", Nat.show(String.length(AT.Att.body(log ++ "end"))), Nat.show(String.length(log ++ "end")))
    C.Check.str("the word typed after a long draft", RF.Refs.token(log ++ "see @mis"), "@mis")
    # the history's step label: a long message's first line, trimmed
    C.Check.str("a step labels a long message by its first line", HI.Hist.line("  Paste this \n" ++ log), "Paste this")
    C.Check.str("a long message names nothing", Nat.show(List.length(&2, RF.Hit, RF.Refs.found(M.State.empty(), "t1", log))), "0")
