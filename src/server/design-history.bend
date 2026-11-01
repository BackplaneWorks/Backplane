import Base
import ./db.bend as D
import ../core/hist.bend as H
import ../core/model.bend as M
import ../core/snapshot.bend as S
import ../core/far.bend as F

# Hot boot keeps a recent chat window, so its State cannot supply the
# design's complete history. Read only the selected thread's snapshots
# and short labels for their source entries. Chat bodies stay paged.
def History.sql.steps() -> String:
  "SELECT seq,ord,id,thread,src,kind,role,akind,text,at FROM entry WHERE thread=?1 AND kind='act' AND role='step' AND seq<=?2 ORDER BY ord DESC"

def History.sql.sources() -> String:
  "SELECT e.seq,e.ord,e.id,e.thread,e.src,e.kind,e.role,e.akind,substr(e.text,1,240),e.at FROM entry AS e JOIN ("
    ++ "SELECT DISTINCT json_extract(CASE WHEN json_valid(text) THEN text ELSE '{}' END,'$.e') AS id FROM entry WHERE thread=?1 AND kind='act' AND role='step' AND seq<=?2"
    ++ ") AS s ON e.id=s.id WHERE e.thread=?1 AND e.seq<=?2 ORDER BY e.ord DESC"

def History.entries(rows: List<&2, List<&2, String>>, acc: List<&2, M.Entry>) -> List<&2, M.Entry>:
  match rows:
    case Nil{}: List.reverse(&2, M.Entry, acc)
    case r <> rest: History.entries(rest, S.Snap.entry(r) <> acc)

def History.failed(rows: List<&2, List<&2, String>>) -> Bool:
  match rows:
    case r <> _: String.eq(S.Snap.col(r, 0n), "\u{0}fail")
    case Nil{}: False{}

def History.loaded(failed: Bool, steps: List<&2, List<&2, String>>, sources: List<&2, List<&2, String>>, th: String, st: M.State) -> M.State:
  match failed:
    case True{}: st
    case False{}: H.Hist.install(History.entries(steps, Nil{}), History.entries(sources, Nil{}), th, st)

def History.read(needed: Bool, +db: String, +th: String, +st: M.State) -> IO(M.State):
  match needed:
    case False{}: IO.pure(M.State, st)
    case True{}:
      do IO<M.State>:
        +steps : List<&2, List<&2, String>> <- D.Db.query.t(db, History.sql.steps(), [D.DbText{th}, D.Db.nat(M.State.seq(st))], 4000000000)
        +sources : List<&2, List<&2, String>> <- D.Db.query.t(db, History.sql.sources(), [D.DbText{th}, D.Db.nat(M.State.seq(st))], 4000000000)
        return History.loaded(Bool.or(History.failed(steps), History.failed(sources)), steps, sources, th, st)

def History.restore(+db: String, +th: String, st: M.State) -> IO(M.State):
  History.read(Bool.and(Bool.not(String.is_empty(db)), Bool.and(Bool.not(String.is_empty(th)), Bool.not(F.Fr.is(th)))), db, th, st)
