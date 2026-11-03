import Base
import ./check.bend as C
import ../src/server/thin-body.bend as B
import ../src/core/model.bend as M
import ../src/core/thin.bend as T
import ../src/core/thinq.bend as Q

# A view over the size budget halves the thread's window around the
# reader's anchor (server.bend's Hub.thin.web_fit, thin-body.bend's
# Body.window). The anchor is the row at the top of the reader's screen;
# the rows on screen are newer than it. The window keeps the anchor and
# the rows after it, so a reader at the end still sees the newest reply
# (a big view, a viewer open, once dropped the newest rows: a phantom
# "2 new" pill, and a reply just stopped was not drawn).

def post(+n: Nat, i: T.Index) -> T.Index:
  match n:
    case 0n: i
    case 1n+k: T.Index.post(post(k, i), M.Msg{"m" ++ Nat.show(1n+k), "t0", M.Assistant{}, "reply " ++ Nat.show(1n+k), 1n+k})

def index() -> T.Index:
  post(10n, T.Index.empty())

def page() -> List<&2, String>:
  T.Index.page(index(), "t0", 0n, 32n)

def name(+i: T.Index, +id: String) -> String:
  M.Idx.id(T.Row.entry(T.Index.row(i, Q.Q.num(id))))

def names(+i: T.Index, ids: List<&2, String>) -> String:
  match ids:
    case Nil{}: ""
    case +id <> t: name(i, id) ++ " " ++ names(i, t)

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("the page lists the thread's rows", names(index(), page()), "m10 m9 m8 m7 m6 m5 m4 m3 m2 m1 ")
    C.Check.str("an anchor near the end keeps the newest rows", names(index(), B.Body.window(page(), index(), "m7")), "m10 m9 m8 m7 m6 ")
    C.Check.str("an anchor far back keeps it and the rows after it", names(index(), B.Body.window(page(), index(), "m2")), "m6 m5 m4 m3 m2 ")
    C.Check.str("no anchor keeps the newest rows", names(index(), B.Body.window(page(), index(), "")), "m10 m9 m8 m7 m6 ")
    C.Check.str("the newest row as anchor", names(index(), B.Body.window(page(), index(), "m10")), "m10 m9 m8 m7 m6 ")
    C.Check.str("halved again it converges on the anchor", names(index(), B.Body.window(B.Body.window(B.Body.window(page(), index(), "m2"), index(), "m2"), index(), "m2")), "m2 ")
