import Base
import ./check.bend as C
import ../src/web/presentation.bend as P
import ../src/web/dom.bend as D
import ../src/core/json.bend as J

def fixture() -> D.Node:
  D.El{"main", "thread", Nil{}, [D.Txt{"canonical history"},
    D.El{"textarea", "composer", [D.At{"id", "composer"}, D.At{"value", "before"}], Nil{}},
    D.El{"div", "model-layer", [D.At{"style", "display:none"}, D.At{"aria-hidden", "true"}],
      [D.El{"button", "", [D.On{"click", "model"}, D.At{"data-value", "server-listed"}], [D.Txt{"server choice"}]}]}]}

def count.go(s: String, +pat: String, n: Nat) -> Nat:
  match s:
    case SNil{}: n
    case SCon{+c, +t}: count.go(t, pat, Nat.add(n, Bool.pick(Nat, String.starts_with(SCon{c, t}, pat), 1n, 0n)))

def count(s: String, +pat: String) -> Nat:
  count.go(s, pat, 0n)

def shaded() -> D.Node:
  D.El{"div", "model-layer", [D.At{"style", "display:contents"}, D.At{"aria-hidden", "false"}],
    [D.El{"div", "", [D.At{"class", "shade"}, D.At{"data-value", "models"}, D.On{"click", "flag"}], Nil{}},
     D.El{"div", "", [D.At{"class", "model-menu"}], Nil{}}]}

def text(open: Bool) -> String:
  J.Json.show(P.Node.write(P.Node.overlay(fixture(), "draft 日本語", open)))

def yes(b: Bool) -> String:
  Bool.pick(String, b, "yes", "no")

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("cached model layer shows immediately", yes(String.contains(text(True{}), "display:contents")), "yes")
    C.Check.str("closed model layer remains hidden", yes(String.contains(text(False{}), "display:none")), "yes")
    C.Check.str("model overlay preserves authoritative history", yes(String.contains(text(True{}), "canonical history")), "yes")
    C.Check.str("model overlay preserves server choice action", yes(String.contains(text(True{}), "server-listed")), "yes")
    C.Check.str("model overlay preserves selected draft", yes(String.contains(text(True{}), "draft 日本語")), "yes")
    C.Check.str("pure model overlay does not mutate input", yes(String.contains(J.Json.show(P.Node.write(fixture())), "display:none")), "yes")
    C.Check.str("a model layer opened here gets its shade", yes(String.contains(text(True{}), "\"shade\"")), "yes")
    C.Check.str("a closed model layer has no shade", yes(String.contains(text(False{}), "\"shade\"")), "no")
    C.Check.str("a shaded layer shut here loses it", yes(String.contains(J.Json.show(P.Node.write(P.Node.overlay(shaded(), "", False{}))), "\"shade\"")), "no")
    C.Check.str("a shaded layer kept open has one shade", Nat.show(count(J.Json.show(P.Node.write(P.Node.overlay(shaded(), "", True{}))), "\"shade\"")), "1")
