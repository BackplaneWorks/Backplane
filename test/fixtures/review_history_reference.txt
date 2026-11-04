import Base
import ../core/model.bend as M
import ../core/far.bend as FR
import ../core/snapshot.bend as SN
import ./db.bend as D
import ./thin-view.bend as V

# Settled threads have no entries in the hot startup state. Read this
# selected thread's complete review ops into the private render state.
def ReviewHistory.sql() -> String:
  "SELECT seq, ord, id, thread, src, kind, role, akind, text, at FROM entry WHERE thread = ?1 AND kind = 'act' AND role = 'review' AND seq <= ?2 ORDER BY seq"

def ReviewHistory.post(rows: List<&2, List<&2, String>>, st: M.State) -> M.State:
  match rows:
    case Nil{}: st
    case r <> rest: ReviewHistory.post(rest, M.State.post(st, SN.Snap.entry(r)))

def ReviewHistory.ids(es: List<&2, M.Entry>, seen: Map<&2, Bool>) -> Map<&2, Bool>:
  match es:
    case Nil{}: seen
    case +e <> rest: ReviewHistory.ids(rest, Map.set(&2, Bool, seen, M.Idx.id(e), True{}))

def ReviewHistory.found(r: Map<&2, Bool> & Bool) -> Bool:
  (_, found) = r
  found

def ReviewHistory.keep(found: Bool, e: M.Entry, acc: List<&2, M.Entry>) -> List<&2, M.Entry>:
  match found:
    case True{}: acc
    case False{}: e <> acc

def ReviewHistory.older(es: List<&2, M.Entry>, +seen: Map<&2, Bool>, acc: List<&2, M.Entry>) -> List<&2, M.Entry>:
  match es:
    case Nil{}: List.reverse(&2, M.Entry, acc)
    case +e <> rest:
      ReviewHistory.older(rest, seen, ReviewHistory.keep(ReviewHistory.found(Map.get(Bool, False{}, seen, M.Idx.id(e))), e, acc))

# A just-recorded op may precede its store write. Keep those newest ops,
# then the stored history, without replaying any op twice.
def ReviewHistory.merge(+known: List<&2, M.Entry>, stored: List<&2, M.Entry>) -> List<&2, M.Entry>:
  List.append(&2, M.Entry, known, ReviewHistory.older(stored, ReviewHistory.ids(known, Map.new(&2, Bool)), Nil{}))

def ReviewHistory.attach(+st: M.State, +thread: String, rows: List<&2, List<&2, String>>) -> M.State:
  V.State.window.file(st, M.Idx.review.key(thread), ReviewHistory.merge(M.State.reviews(st, thread), M.State.reviews(ReviewHistory.post(rows, M.State.empty()), thread)))

def ReviewHistory.failed(rows: List<&2, List<&2, String>>) -> Bool:
  match rows:
    case r <> _: String.eq(SN.Snap.col(r, 0n), "\u{0000}fail")
    case Nil{}: False{}

def ReviewHistory.result(failed: Bool, +st: M.State, +thread: String, rows: List<&2, List<&2, String>>) -> M.State:
  match failed:
    case True{}: st
    case False{}: ReviewHistory.attach(st, thread, rows)

def ReviewHistory.read.local(skip: Bool, +db: String, +st: M.State, +thread: String) -> IO(M.State):
  match skip:
    case True{}: IO.pure(M.State, st)
    case False{}:
      do IO<M.State>:
        +rows : List<&2, List<&2, String>> <- D.Db.query.t(db, ReviewHistory.sql(), [D.DbText{thread}, D.Db.nat(M.State.seq(st))], 4000000000)
        return ReviewHistory.result(ReviewHistory.failed(rows), st, thread, rows)

def ReviewHistory.read(+db: String, +st: M.State, +thread: String) -> IO(M.State):
  ReviewHistory.read.local(Bool.or(String.is_empty(db), Bool.or(String.is_empty(thread), FR.Fr.is(thread))), db, st, thread)
