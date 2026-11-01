import Base
import ./check.bend as C
import ../src/server/review-history.bend as R
import ../src/server/thin-view.bend as V
import ../src/core/model.bend as M
import ../src/core/review.bend as RV

# Hot startup leaves settled threads without entries. The private viewer
# reads complete review records without loading another conversation.
def cold() -> M.State:
  M.State.apply(M.State.empty(), M.ThreadCreated{"a", "", "Settled", M.Local{}, "", "", "codex", "", 1n})

def text() -> String:
  String.repeat("Full comment context. ", 40n)

def root() -> M.Entry:
  M.Act{"root", "a", "review", "add", RV.Review.op.add("root", RV.Anchor{"board.kicad_pcb", "", "board", "", 1.0, 2.0, ""}, text(), "person", ""), 2n}

def reply() -> M.Entry:
  M.Act{"reply", "a", "review", "add", RV.Review.op.add("reply", RV.Anchor{"", "", "", "", 0.0, 0.0, ""}, "Reply", "person", "root"), 3n}

def resolve() -> M.Entry:
  M.Act{"resolve", "a", "review", "resolve", RV.Review.op.resolve("root", "person"), 4n}

def reopen() -> M.Entry:
  M.Act{"reopen", "a", "review", "reopen", RV.Review.op.reopen("root", "person"), 5n}

def row(e: M.Entry) -> List<&2, String>:
  match e:
    case M.Act{+id, +th, +tone, +kind, +summary, +at}:
      [Nat.show(at), Nat.show(at), id, th, "", "act", tone, kind, summary, Nat.show(at)]
    case _: Nil{}

def rows() -> List<&2, List<&2, String>>:
  [row(root()), row(reply()), row(resolve()), row(M.Act{"foreign", "other", "review", "add", "Private", 2n})]

def ids(es: List<&2, M.Entry>) -> String:
  match es:
    case Nil{}: ""
    case e <> rest: M.Idx.id(e) ++ " " ++ ids(rest)

def main() -> IO(Unit):
  do IO<Unit>:
    +shown : M.State = R.ReviewHistory.attach(cold(), "a", rows())
    C.Check.str("settled thread recovers full long root and reply", Nat.show(List.length(&2, RV.Comment, RV.Review.of_state(shown, "a"))), "2")
    C.Check.str("stored resolution is applied after its root", Nat.show(RV.Review.count.open(RV.Review.of_state(shown, "a"))), "0")
    C.Check.str("full long review text is preserved", Bool.show(String.contains(RV.Review.md(RV.Review.of_state(shown, "a")), text())), "True")
    C.Check.str("review hydration does not populate chat", Nat.show(List.length(&2, M.Entry, M.State.entries.all(shown))), "0")
    C.Check.str("review hydration excludes foreign records", Nat.show(List.length(&2, M.Entry, M.State.reviews(shown, "other"))), "0")
    +known : M.State = M.State.post(cold(), reopen())
    +merged : M.State = R.ReviewHistory.attach(known, "a", rows())
    C.Check.str("an op ahead of the store remains newest", ids(M.State.reviews(merged, "a")), "reopen resolve reply root ")
    C.Check.str("new in-memory reopen wins", Nat.show(RV.Review.count.open(RV.Review.of_state(merged, "a"))), "1")
    +again : M.State = R.ReviewHistory.attach(merged, "a", rows())
    C.Check.str("repeated reads never duplicate review ops", ids(M.State.reviews(again, "a")), "reopen resolve reply root ")
    C.Check.str("empty store read keeps just-recorded op", ids(M.State.reviews(R.ReviewHistory.attach(known, "a", Nil{}), "a")), "reopen ")
    C.Check.str("failed store read keeps known state", ids(M.State.reviews(R.ReviewHistory.result(True{}, known, "a", rows()), "a")), "reopen ")
    C.Check.str("review projection keeps hydrated closed-thread records", ids(M.State.reviews(V.State.window.reviews.keep(shown, "a", M.State.empty()), "a")), "resolve reply root ")
    +local : M.State <- R.ReviewHistory.read("", known, "a")
    C.Check.str("no-store route keeps existing review", ids(M.State.reviews(local, "a")), "reopen ")
    +far : M.State <- R.ReviewHistory.read("unused", known, "remote~a")
    C.Check.str("remote thread never reads the local store", ids(M.State.reviews(far, "a")), "reopen ")
