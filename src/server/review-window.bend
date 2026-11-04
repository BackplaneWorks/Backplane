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

# A later read failure discards the private partial fold; it cannot replace
# the caller's complete baseline. The pure step is a closed Bend template,
# rather than a reusable runtime closure (functions are affine in Bend).
def ReviewWindow.check(-A: Type, after: Nat, next: Nat, done: Bool, error: String, value: A) -> Maybe<&1, A>:
  match Bool.and(String.is_empty(error), ReviewWindow.progress(after, next, done)):
    case False{}: None{}
    case True{}: Some{value}

def ReviewWindow.fold(~A: Type, ~step: List<&2, List<&2, String>> -> A -> A, +h: String, +thread: String, +after: Nat, +through: Nat, value: A) -> IO(Maybe<&1, A>):
  do IO<Maybe<&1, A>>:
    page : D.HistoryPage <- ReviewWindow.read(h, thread, after, through)
    match page:
      case D.HistoryPage{rows, +next, +done, error}:
        match ReviewWindow.check(A, after, next, done, error, value):
          case None{}: IO.pure(Maybe<&1, A>, None{})
          case Some{prior}:
            match done:
              case True{}: IO.pure(Maybe<&1, A>, Some{step(rows, prior)})
              case False{}: ReviewWindow.fold(~A, ~step, h, thread, next, through, step(rows, prior))
