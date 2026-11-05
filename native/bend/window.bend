import Base
import ../../src/core/model.bend as M
import ../../src/core/thin.bend as T
import ../../src/core/thinq.bend as Q
import ../../src/core/json.bend as J
import ../../src/core/thin-live.bend as TL
import ../../src/server/thin-window.bend as TW
import ./kcommon.bend as C
import ./policy.bend as PO

# The `window` of a thread's page, computed the way the JS hub computes it (thin-view.bend Render.window.context,
# here src/server/thin-window.bend) from the page Rust read out of its own index. One request file:
#   window \t THREAD \t EPOCH \t BEFORE \t BELOW \t ABOVE \t REQUEST-JSON
#   one SQL row per line (the cells of policy.bend: seq ord id thread src kind role akind text at, \ \t \n escaped)
# Answers the window as JSON text. State and index are built as Ts.page and State.window.* build them: the page index
# from the rows, the window state from the page's entries oldest first.

def Wn.ents(ids: List<&2, String>, +idx: T.Index, acc: List<&2, M.Entry>) -> List<&2, M.Entry>:
  match ids:
    case Nil{}: acc
    case id <> rest: Wn.ents(rest, idx, T.Row.preview(T.Index.row(idx, Q.Q.num(id))) <> acc)

def Wn.post(es: List<&2, M.Entry>, +st: M.State) -> M.State:
  match es:
    case Nil{}: st
    case +e <> rest: Wn.post(rest, M.State.post(st, e))

def Wn.show(+idx: T.Index, +th: String, +p: J.Json, +epoch: String, +ids: List<&2, String>) -> String:
  J.Json.show(TW.Render.window.context(Wn.post(Wn.ents(ids, idx, Nil{}), M.State.empty()), TL.Stream.blank(), idx, th, p, epoch, ids))

def Wn.page(+idx: T.Index, +th: String, +epoch: String, +before: Nat, +p: J.Json) -> String:
  Wn.show(idx, th, p, epoch, T.Index.page(idx, th, before, 32n))

def Wn.go(+th: String, +epoch: String, +before: Nat, +below: Nat, +above: Nat, +p: J.Json, cells: List<&2, List<&2, String>>) -> String:
  Wn.page(T.Index.window.above(T.Index.window(Q.Q.rows(cells), th, below, T.Index.empty()), th, above), th, epoch, before, p)

def Wn.head(cs: List<&2, String>, cells: List<&2, List<&2, String>>) -> String:
  match cs:
    case _ <> th <> epoch <> before <> below <> above <> pj <> Nil{}:
      Wn.go(th, epoch, C.K.digits(before, 0n), C.K.digits(below, 0n), C.K.digits(above, 0n), Maybe.default(&2, J.Json, J.Json.parse(pj), J.Null{}), cells)
    case _: "error request"

def Wn.run(ls: List<&2, String>) -> String:
  match ls:
    case Nil{}: "error empty"
    case head <> rest: Wn.head(String.split(head, '\t'), PO.Un.rows(rest, Nil{}))

def main() -> IO(Unit):
  do IO<Unit>:
    xs : List<&1, String> <- IO.args()
    text : String <- PO.Io.slurp(PO.Pol.path(xs))
    IO.write(Wn.run(String.split(text, '\n')))
