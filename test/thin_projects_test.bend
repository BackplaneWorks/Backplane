import Base
import ./check.bend as C
import ../src/core/json.bend as J
import ../src/core/model.bend as M
import ../src/core/thin.bend as T
import ../src/server/thin-view.bend as V
import ../src/core/client.bend as U
import ../src/core/thin-wire.bend as W

# Every project is in a thin view's window, with its few most recent
# threads, whatever the thin index holds: after a hot boot it holds only the
# open threads, and projects whose threads were all settled vanished from
# the sidebar (live, 2026-10-22).

def made(+id: String, +p: String, +at: Nat) -> M.Change:
  M.ThreadCreated{id, p, "T " ++ id, M.Local{}, "", "", "claude", "", at}

def all() -> List<&2, M.Change>:
  [M.ProjectCreated{"p1", "Open", "/p1", 1n}, M.ProjectCreated{"p2", "Settled", "/p2", 1n}, M.ProjectCreated{"p3", "Many", "/p3", 1n},
   made("t1", "p1", 100n), M.MessagePosted{"t1", "m1", M.User{}, "hi", 101n}, M.TurnChanged{"t1", M.Running{}, 102n},
   made("t2", "p2", 10n), M.MessagePosted{"t2", "m2", M.User{}, "old", 11n}, M.ThreadSettled{"t2", True{}, 12n},
   made("q1", "p3", 20n), made("q2", "p3", 21n), made("q3", "p3", 22n), made("q4", "p3", 23n), made("q5", "p3", 24n), made("q6", "p3", 25n),
   M.ProjectCreated{"p4", "Done", "/p4", 1n},
   made("s1", "p4", 40n), made("s2", "p4", 41n), made("s3", "p4", 42n), made("s4", "p4", 43n), made("s5", "p4", 44n), made("s6", "p4", 45n), made("s7", "p4", 46n),
   M.ThreadSettled{"s1", True{}, 50n}, M.ThreadSettled{"s2", True{}, 51n}, M.ThreadSettled{"s3", True{}, 52n}, M.ThreadSettled{"s4", True{}, 53n},
   M.ThreadSettled{"s5", True{}, 54n}, M.ThreadSettled{"s6", True{}, 55n}, M.ThreadSettled{"s7", True{}, 56n}]

def open() -> List<&2, M.Change>:
  [M.ProjectCreated{"p1", "Open", "/p1", 1n}, made("t1", "p1", 100n), M.MessagePosted{"t1", "m1", M.User{}, "hi", 101n}, M.TurnChanged{"t1", M.Running{}, 102n}]

def st() -> M.State:
  M.State.replay(all(), M.State.empty())

# the index as a hot boot has it: built over the open threads only
def win() -> M.State:
  V.State.window(st(), T.Index.of(M.State.replay(open(), M.State.empty())), J.Json.obj([J.KV{"kind", J.Str{"thread"}}, J.KV{"id", J.Str{"t1"}}]), Nil{})

def win.opened(+opened: List<&2, String>) -> M.State:
  V.State.window.opened(st(), T.Index.of(M.State.replay(open(), M.State.empty())), J.Json.obj([J.KV{"kind", J.Str{"thread"}}, J.KV{"id", J.Str{"t1"}}]), Nil{}, opened)

def projects(+s: M.State) -> String:
  match s:
    case M.State{ps, _, _, _, _, _, _, _, _, _, _}: String.join(Map.keys(&2, M.Project, ps), ",")

def threads(+s: M.State) -> String:
  match s:
    case M.State{_, ts, _, _, _, _, _, _, _, _, _}: String.join(Map.keys(&2, M.Thread, ts), ",")

def has(+xs: String, +x: String) -> String:
  Bool.show(String.contains(xs, x))

def search.many(+n: Nat, i: T.Index) -> T.Index:
  match n:
    case 0n: i
    case 1n+k: search.many(k, T.Index.post(i, M.Msg{"ctx-" ++ Nat.show(k), "s1", M.Assistant{}, "UniiChat", 60n}))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("every project is in the window", projects(win()), "p1,p2,p3,p4")
    C.Check.str("a collapsed project's settled count is all of them", M.Setting.get(win(), U.Side.settled.key("p4"), ""), "7")
    C.Check.str("the sidebar's Settled N reads it, whatever the window holds", Nat.show(U.Side.settled.n(win(), "p4", 5n)), "7")
    C.Check.str("a project with nothing settled has no count", M.Setting.get(win(), U.Side.settled.key("p3"), "none"), "none")
    C.Check.str("a project whose threads are all settled shows its thread", has(threads(win()), "t2"), "True")
    C.Check.str("the open thread is there", has(threads(win()), "t1"), "True")
    C.Check.str("a project's five most recent threads", Bool.show(Bool.and(String.contains(threads(win()), "q6"), String.contains(threads(win()), "q2"))), "True")
    C.Check.str("and no more", has(threads(win()), "q1"), "False")
    C.Check.str("a project whose settled threads are opened has them all", has(threads(win.opened(["p3"])), "q1"), "True")
    C.Check.str("opening one project brings no other's", has(threads(win.opened(["p2"])), "q1"), "False")
    +index : T.Index = T.Index.post(T.Index.post(T.Index.of(st()), M.Msg{"ctx-old", "s1", M.Assistant{}, "UniiChat infinite context", 57n}), M.Msg{"ctx-here", "t1", M.Assistant{}, "UniiChat here too", 58n})
    +found : J.Json = V.Search.render(index, st(), J.Json.obj([J.KV{"target", V.Target.json("thread", "t1")}, J.KV{"query", J.Str{"UniiChat"}}]), "epoch", 0n)
    +rows : J.Json = J.Json.get_obj(J.Json.get_obj(found, "search"), "rows")
    C.Check.str("conversation search finds both current and hidden threads", Nat.show(List.length(&2, J.Json, J.Json.elems(rows))), "2")
    C.Check.str("the hidden settled conversation is a selectable result", Bool.show(String.contains(J.Json.show(rows), "\"thread\":\"s1\"")), "True")
    +paged : J.Json = V.Search.render(search.many(30n, T.Index.of(st())), st(), J.Json.obj([J.KV{"target", V.Target.json("thread", "t1")}, J.KV{"query", J.Str{"UniiChat"}}]), "epoch", 0n)
    +page : J.Json = J.Json.get_obj(paged, "search")
    +cursor : String = J.Json.get_str(page, "cursor", "")
    C.Check.str("global search retains the result bound", Nat.show(List.length(&2, J.Json, J.Json.elems(J.Json.get_obj(page, "rows")))), "25")
    C.Check.str("additional global matches require an explicit next page", Bool.show(J.Json.get_bool(page, "hasMore", False{})), "True")
    C.Check.str("global cursor stays bound to the selected target", Bool.show(Maybe.is_some(&2, Nat, W.Cursor.read(cursor, "epoch", V.Target.json("thread", "t1"), "UniiChat"))), "True")
    C.Check.str("a result thread cannot borrow the selected target's cursor", Bool.show(Maybe.is_some(&2, Nat, W.Cursor.read(cursor, "epoch", V.Target.json("thread", "s1"), "UniiChat"))), "False")
    C.Check.str("search issues no selection or other commands", Nat.show(List.length(&2, J.Json, J.Json.elems(J.Json.get_obj(paged, "cmds")))), "0")
