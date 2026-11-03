import Base
import ../core/model.bend as M
import ../core/thin.bend as T
import ../core/thinq.bend as Q
import ../core/json.bend as J
import ../core/far.bend as F
import ../core/message-context.bend as C
import ./thin-view.bend as V
import ./thinstore.bend as TS

def ContextHistory.entries(rows: List<&2, List<&2, String>>) -> List<&2, M.Entry>:
  match rows:
    case Nil{}: Nil{}
    case row <> rest: T.Row.entry(Q.Q.row(row)) <> ContextHistory.entries(rest)

def ContextHistory.runs(rows: List<&2, List<&2, String>>, +thread: String) -> List<&2, M.Entry>:
  match rows:
    case Nil{}: Nil{}
    case (id <> head <> _) <> rest: M.Act{id, thread, "context", head, "", 0n} <> ContextHistory.runs(rest, thread)
    case _ <> rest: ContextHistory.runs(rest, thread)

def ContextHistory.read.local(skip: Bool, +db: String, +st: M.State, +thread: String, ids: List<&2, String>) -> IO(M.State):
  match skip:
    case True{}: IO.pure(M.State, st)
    case False{}:
      do IO<M.State>:
        +messages : List<&2, List<&2, String>> <- TS.Ts.query(db, Q.Q.sql.context.messages(), [thread], 2)
        +runs : List<&2, List<&2, String>> <- TS.Ts.query(db, Q.Q.sql.context.runs(), [thread, J.Json.show(J.Json.arr(Q.Q.json.nats(ids)))], 32)
        return V.State.window.file(V.State.window.file(st, C.Context.key(thread), ContextHistory.entries(messages)), C.Context.runs.key(thread), ContextHistory.runs(runs, thread))

def ContextHistory.read(+db: String, +st: M.State, +thread: String, ids: List<&2, String>) -> IO(M.State):
  ContextHistory.read.local(Bool.or(String.is_empty(db), Bool.or(String.is_empty(thread), F.Fr.is(thread))), db, st, thread, ids)
