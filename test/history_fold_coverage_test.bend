import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/fold.bend as F
import ../src/core/i18n.bend as I
import ../src/web/view.bend as V
import ../src/web/dom.bend as D

def call(+id: String) -> M.Entry:
  M.Act{id, "thread", "tool", "shell", "private call body is not coverage", 0n}

def attr(as: List<&2, D.Attr>) -> String:
  match as:
    case Nil{}: ""
    case D.At{"data-fold-ids", value} <> _: value
    case _ <> t: attr(t)

def coverage(ns: List<&2, D.Node>) -> String:
  match ns:
    case D.El{_, _, as, _} <> _: attr(as)
    case _: ""

def sample(+open: Bool) -> List<&2, D.Node>:
  V.View.entries.role(I.En{}, F.RHead{"actual:a", "calls", open},
    [call("actual:a"), call("actual:b"), call("next-run")],
    [F.RIn{open}, F.RHead{"next-run", "next", False{}}], [D.Txt{"body"}])

def many(+n: Nat) -> List<&2, M.Entry>:
  match n:
    case 0n: Nil{}
    case 1n+k: call(Nat.show(k)) <> many(k)

def roles(n: Nat) -> List<&2, F.Role>:
  match n:
    case 0n: Nil{}
    case 1n+k: F.RIn{False{}} <> roles(k)

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("closed run uses actual IDs and stops at next head", coverage(sample(False{})), "[\"actual:a\",\"actual:b\"]")
    C.Check.str("open run has the same coverage", coverage(sample(True{})), "[\"actual:a\",\"actual:b\"]")
    C.Check.str("coverage contains no hidden body nodes", Nat.show(List.length(&2, D.Node, sample(False{}))), "1")
    C.Check.str("coverage remains bounded for compatibility history", Nat.show(List.length(&2, String, V.View.fold.tail(31n, many(100n), roles(100n)))), "31")
