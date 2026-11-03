import Base
import ./check.bend as C
import ./autosettle_test.bend as F
import ../src/core/model.bend as M
import ../src/core/signal.bend as S
import ../src/web/view.bend as V
import ../src/web/dom.bend as D
import ../src/core/i18n.bend as I

def attrs(n: D.Node) -> List<&2, D.Attr>:
  match n:
    case D.El{_, _, attrs, _}: attrs
    case _: Nil{}

def dot(+s: M.State) -> String:
  V.View.attr(attrs(V.View.dot(I.En{}, S.Signal.name(S.Signal.at(S.Signal.index(s), "t")))), "class")

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("finished sidebar dot is green", Bool.show(String.contains(dot(F.base(True{}, Nil{})), "finished")), "True")
    C.Check.str("settled sidebar DOM has settled class", Bool.show(String.contains(dot(F.settled()), "settled")), "True")
    C.Check.str("settled sidebar DOM has no green class", Bool.show(String.contains(dot(F.settled()), "finished")), "False")
