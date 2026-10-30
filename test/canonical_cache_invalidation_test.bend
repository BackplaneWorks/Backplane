import Base
import ./check.bend as C
import ./signal_text_actor_test.bend as F
import ./canonical_cache_navigation_test.bend as N
import ./canonical_cache_output_test.bend as O
import ./sidebar_index_cache_test.bend as IX
import ../src/server/thin-actor.bend as A
import ../src/server/thin-side-index.bend as X
import ../src/server/thin-signals.bend as K
import ../src/server/thin-signal-text.bend as TXT
import ../src/server/thin-view.bend as V
import ../src/core/client.bend as U
import ../src/core/model.bend as M
import ../src/core/json.bend as J

# Exercise the real automatically generated read and its owned completion,
# rather than manufacturing a pending command or suppressing the fab timer.
def fab(+a: A.Actor) -> V.ActResult:
  V.ActResult.web(U.Fab.sync(U.Act{U.Ui.flag_off(A.Actor.ui(a), "settings"), Nil{}}))

def commands(r: V.ActResult) -> List<&2, J.Json>:
  match r:
    case V.ActResult{_, cs}: cs

def read(cs: List<&2, J.Json>) -> J.Json:
  match cs:
    case Nil{}: J.Null{}
    case c <> _: A.Actor.rpc(c)

def requested() -> A.Actor:
  A.Actor.accept(F.seed(), fab(F.seed()), False{})

def info(a: A.Actor) -> A.Actor:
  A.Actor.info(a, J.Json.obj([J.KV{"info", J.Json.obj([
    J.KV{"drive.self", J.Str{"new-browser-owner"}},
    J.KV{"models.v", J.Str{"recipient-specific-new-models"}}])}]))

def reply() -> J.Json:
  J.Json.obj([J.KV{"t", J.Str{"reply"}}, J.KV{"id", J.Str{J.Json.as_raw(J.Json.get(read(commands(fab(F.seed()))), "id"), "")}},
    J.KV{"ok", J.Flag{True{}}}, J.KV{"result", J.Json.obj([
      J.KV{"thread", J.Str{"self"}}, J.KV{"goal", J.Str{"fresh fab goal"}}, J.KV{"intent", J.Str{"fresh fab intent"}}])}])

def completed() -> A.Actor:
  A.Actor.reply(info(requested()), True{}, "public-hub", reply())

def pending(a: A.Actor) -> String:
  match a:
    case A.Actor{_, _, _, p, _, _, _}: Nat.show(List.length(&2, String, Map.keys(&2, String, p)))

def rejected() -> A.Actor:
  A.SideIndex.save(X.IndexCache.bounded(IX.too_big()), F.seed(), IX.too_big())

def full(+st: M.State, epoch: String, +on: Bool) -> String:
  O.exact(O.moved(completed(), st, "other", "next-view", on), st, "other", epoch, on)

def event(+e: M.Change) -> String:
  full(M.State.apply(F.state(), e), "epoch", True{})

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("actual automatic fab read is generated", J.Json.get_str(read(commands(fab(F.seed()))), "m", ""), "fab.look")
    C.Check.str("actual fab read creates owned pending request", pending(requested()), "1")
    C.Check.str("targeted info preserves three bounded canonical slots", N.count(info(requested())), "3")
    C.Check.str("owned fab reply preserves three bounded canonical slots", N.count(completed()), "3")
    C.Check.str("owned fab reply actually settles pending request", pending(completed()), "0")
    C.Check.str("owned fab reply actually installs fresh card", U.Ui.info(A.Actor.ui(completed()), "fab.intent:self"), "fresh fab intent")
    C.Check.str("info and owned completion still mark actor dirty", N.yes(A.Actor.dirty(completed())), "yes")
    C.Check.str("fab info completion navigation retains exactly canonical slots", N.total(N.refresh(completed(), "other", "new-view")), "3")
    C.Check.str("dirty invalidation keeps only existing body plus canonical", N.total(A.Actor.mark_dirty(N.tainted(F.seed()))), "4")
    C.Check.str("info keeps only existing body and sent receipt plus canonical", N.total(info(N.tainted(F.seed()))), "5")
    C.Check.str("oversized metadata fixture really refuses retention", N.yes(X.IndexCache.bounded(IX.too_big())), "no")
    C.Check.str("oversized metadata save releases previously retained slot", Nat.show(List.length(&2, A.Memo, A.SideIndex.keep(A.Sidebar.memos(rejected())))), "0")
    C.Check.str("metadata refusal preserves unrelated scalar and text slots", N.count(rejected()), "2")
    C.Check.str("recipient owner info really updates", U.Ui.info(A.Actor.ui(completed()), "drive.self"), "new-browser-owner")
    C.Check.str("recipient info completion output matches fresh", full(F.state(), "epoch", True{}), "same")
    C.Check.str("post-fab epoch replacement output matches fresh", full(F.state(), "replacement-epoch", True{}), "same")
    C.Check.str("post-fab visibility replacement output matches fresh", full(F.state(), "epoch", False{}), "same")
    C.Check.str("post-fab status replacement output matches fresh", event(M.TurnChanged{"other", M.Completed{}, 5n}), "same")
    C.Check.str("post-fab approval replacement output matches fresh", event(M.AskOpened{"new-ask", "other", "approval", "tool", "Fresh approval", "", 5n}), "same")
    C.Check.str("post-fab canonical title replacement output matches fresh", event(M.ThreadRenamed{"other", "Fresh canonical title", 5n}), "same")
    C.Check.str("scalar old revision cannot hit", N.yes(K.SignalCache.hit(K.SignalCache.obtain("epoch", F.state(), None{}), "epoch", 5n)), "no")
    C.Check.str("metadata old revision cannot hit", N.yes(X.IndexCache.hit(X.IndexCache.obtain("epoch", F.state(), None{}), "epoch", 5n)), "no")
    C.Check.str("text old revision cannot hit", N.yes(TXT.SignalTextCache.hit(F.text(), "epoch", 5n, True{})), "no")
    C.Check.str("fab completion preserves original IME draft", U.Ui.scratch(A.Actor.ui(completed()), "self"), "PRIVATE 日本語 IME draft")
