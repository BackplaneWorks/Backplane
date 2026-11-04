import Base
import ./db.bend as D

# A transient IO page, never the authoritative review fold. SQLite owns the
# index and cursor walk; ReviewHistory keeps the existing pure state fold.
def ReviewWindow.budget() -> D.HistoryBudget:
  D.HistoryBudget{128, 1048576}

def ReviewWindow.progress(after: Nat, next: Nat, done: Bool) -> Bool:
  Bool.or(done, Nat.is_gt(next, after))

def ReviewWindow.read(h: String, thread: String, after: Nat, through: Nat) -> IO(D.HistoryPage):
  D.Db.review_page(h, thread, after, through, ReviewWindow.budget())

# The caller supplies its pure fold. A later read failure discards the
# private partial fold; it cannot replace the caller's complete baseline.
def ReviewWindow.fold(-A: Type, +h: String, +thread: String, +after: Nat, +through: Nat, value: A, +step: List<&2, List<&2, String>> -> A -> A) -> IO(Maybe<&1, A>):
  do IO<Maybe<&1, A>>:
    page : D.HistoryPage <- ReviewWindow.read(h, thread, after, through)
    ReviewWindow.fold.page(A, h, thread, after, through, value, step, page)

def ReviewWindow.fold.page(-A: Type, +h: String, +thread: String, +after: Nat, +through: Nat, value: A, +step: List<&2, List<&2, String>> -> A -> A, page: D.HistoryPage) -> IO(Maybe<&1, A>):
  match page:
    case D.HistoryPage{rows, +next, done, error}:
      ReviewWindow.fold.checked(A, Bool.and(String.is_empty(error), ReviewWindow.progress(after, next, done)), h, thread, next, through, done, value, step, rows)

def ReviewWindow.fold.checked(-A: Type, ok: Bool, +h: String, +thread: String, +next: Nat, +through: Nat, done: Bool, value: A, +step: List<&2, List<&2, String>> -> A -> A, rows: List<&2, List<&2, String>>) -> IO(Maybe<&1, A>):
  match ok done:
    case False{} _: IO.pure(Maybe<&1, A>, None{})
    case True{} True{}: IO.pure(Maybe<&1, A>, Some{step(rows, value)})
    case True{} False{}: ReviewWindow.fold(A, h, thread, next, through, step(rows, value), step)
