import Base
import ../core/model.bend as M
import ../core/subs.bend as SB
import ../core/far.bend as FR
import ../core/snapshot.bend as SN
import ./db.bend as D
import ./thin-view.bend as V

# Hot startup keeps a small chat window, not every subagent's history.
# Read only the selected conversation's starts and bounded activity into
# the private render state; canonical actions and other threads stay out.
def SubHistory.sql() -> String:
  "WITH starts AS (SELECT seq, role FROM entry WHERE thread = ?1 AND kind = 'act' AND akind GLOB 'start*' ORDER BY seq DESC LIMIT ?2), "
    ++ "recent AS (SELECT seq FROM entry WHERE thread = ?1 ORDER BY seq DESC LIMIT ?2), "
    ++ "steps AS (SELECT seq, row_number() OVER (PARTITION BY role ORDER BY seq DESC) AS rn FROM entry WHERE thread = ?1 AND kind = 'act' AND role IN (SELECT role FROM starts)), "
    ++ "wanted AS (SELECT seq FROM starts UNION SELECT seq FROM recent UNION SELECT seq FROM steps WHERE rn <= ?2) "
    ++ "SELECT seq, ord, id, thread, src, kind, role, akind, "
    ++ "CASE WHEN instr(text, char(10)) > 0 THEN substr(text, 1, instr(text, char(10)) - 1) ELSE text END, at "
    ++ "FROM entry WHERE thread = ?1 AND seq IN (SELECT seq FROM wanted) ORDER BY seq"

def SubHistory.limit() -> U32:
  U32.from_nat(Nat.add(Nat.mul(SB.Subs.window(), SB.Subs.window()), Nat.mul(2n, SB.Subs.window())))

def SubHistory.post(rows: List<&2, List<&2, String>>, st: M.State) -> M.State:
  match rows:
    case Nil{}: st
    case r <> rest: SubHistory.post(rest, M.State.post(st, SN.Snap.entry(r)))

def SubHistory.failed(rows: List<&2, List<&2, String>>) -> Bool:
  match rows:
    case r <> _: String.eq(SN.Snap.col(r, 0n), "\u{0000}fail")
    case Nil{}: False{}

def SubHistory.attach(+st: M.State, +thread: String, +rows: List<&2, List<&2, String>>) -> M.State:
  +own = SubHistory.post(rows, M.State.empty())
  +starts = SB.Subs.starts(own, thread)
  V.State.window.subs.calls(starts, own, V.State.window.file(V.State.window.file(st, M.Bg.log(thread), M.State.mine(own, M.Bg.log(thread))), M.Idx.sub.starts(M.Bg.log(thread)), starts))

def SubHistory.result(failed: Bool, +st: M.State, +thread: String, rows: List<&2, List<&2, String>>) -> M.State:
  match failed:
    case True{}: st
    case False{}: SubHistory.attach(st, thread, rows)

def SubHistory.read.local(skip: Bool, +db: String, +st: M.State, +thread: String) -> IO(M.State):
  match skip:
    case True{}: IO.pure(M.State, st)
    case False{}:
      do IO<M.State>:
        +rows : List<&2, List<&2, String>> <- D.Db.query.t(db, SubHistory.sql(), [D.DbText{M.Bg.log(thread)}, D.Db.nat(SB.Subs.window())], SubHistory.limit())
        return SubHistory.result(SubHistory.failed(rows), st, thread, rows)

def SubHistory.read(+db: String, +st: M.State, +thread: String) -> IO(M.State):
  SubHistory.read.local(Bool.or(String.is_empty(db), Bool.or(String.is_empty(thread), FR.Fr.is(thread))), db, st, thread)
