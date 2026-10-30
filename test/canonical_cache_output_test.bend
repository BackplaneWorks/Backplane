import Base
import ./check.bend as C
import ./signal_text_actor_test.bend as F
import ./canonical_cache_navigation_test.bend as N
import ../src/server/thin-actor.bend as A
import ../src/server/thin-view.bend as V
import ../src/server/thin-signals.bend as K
import ../src/core/model.bend as M
import ../src/core/client.bend as U
import ../src/core/json.bend as J
import ../src/core/thin.bend as T
import ../src/core/thin-live.bend as L

def same(a: String, b: String) -> String:
  Bool.pick(String, C.Check.same(a, b), "same", "different")

def request(+st: M.State, +target: String, online: Bool) -> J.Json:
  J.Json.obj([J.KV{"target", V.Target.json("thread", target)},
    J.KV{"session", V.Route.pack(U.Ui.online(U.Ui.select.k(U.Ui.flag_off(F.ui(st, 11n), "settings"), target), online))}])

def online(a: A.Actor, on: Bool) -> A.Actor:
  match a:
    case A.Actor{u, v, target, pending, reads, dirty, memo}:
      A.Actor{U.Ui.online(u, on), v, target, pending, reads, dirty, memo}

def moved(a: A.Actor, +st: M.State, +target: String, +view: String, +on: Bool) -> A.Actor:
  online(A.Actor.refresh([a], True{}, st, J.Json.obj([J.KV{"status", J.Str{"fresh recipient info"}}]), J.Json.obj(Nil{}), 11n, request(st, target, on), view), on)

def exact(+a: A.Actor, +st: M.State, +target: String, +epoch: String, +on: Bool) -> String:
  +p = request(st, target, on)
  +action : V.ActResult = V.ActResult{A.Actor.ui(a), Nil{}}
  same(J.Json.show(A.Rendered.result(A.Actor.render.web.revision(a, action, st, T.Index.empty(), p, epoch, 11n, Nil{}, L.Stream.blank(), M.State.seq(st)))),
    J.Json.show(A.Rendered.result(A.Actor.render.web.fresh(a, action, st, T.Index.empty(), p, epoch, 11n, Nil{}, L.Stream.blank()))))

def event(+e: M.Change) -> String:
  +st = M.State.apply(F.state(), e)
  exact(moved(F.seed(), st, "other", "new-view", True{}), st, "other", "epoch", True{})

def actor(r: A.Rendered) -> A.Actor:
  match r:
    case A.Rendered{a, _}: a

def again() -> String:
  +st = F.state()
  +a = moved(F.seed(), st, "other", "v2", True{})
  +p = request(st, "other", True{})
  +saved = actor(A.Actor.render.web.revision(a, V.ActResult{A.Actor.ui(a), Nil{}}, st, T.Index.empty(), p, "epoch", 11n, Nil{}, L.Stream.blank(), M.State.seq(st)))
  exact(moved(saved, st, "self", "v3", True{}), st, "self", "epoch", True{})

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("target and view move complete output equals fresh", exact(moved(F.seed(), F.state(), "other", "new-view", True{}), F.state(), "other", "epoch", True{}), "same")
    C.Check.str("epoch miss complete output equals fresh", exact(moved(F.seed(), F.state(), "other", "new-view", True{}), F.state(), "other", "new-epoch", True{}), "same")
    C.Check.str("offline move complete output equals fresh", exact(moved(F.seed(), F.state(), "other", "new-view", False{}), F.state(), "other", "epoch", False{}), "same")
    C.Check.str("updated title complete output equals fresh", event(M.ThreadRenamed{"other", "Fresh title", 5n}), "same")
    C.Check.str("updated approval complete output equals fresh", event(M.AskOpened{"approval", "other", "approval", "tool", "Allow", "", 5n}), "same")
    C.Check.str("updated status complete output equals fresh", event(M.TurnChanged{"other", M.Completed{}, 5n}), "same")
    C.Check.str("archival complete output equals fresh", event(M.ThreadArchived{"other", True{}, 5n}), "same")
    C.Check.str("second navigation retained cache complete output equals fresh", again(), "same")
    C.Check.str("offline case actually changes visibility", N.yes(U.Side.signals.online(A.Actor.ui(moved(F.seed(), F.state(), "other", "new-view", False{})))), "no")
    C.Check.str("rejected scalar save releases previous canonical slot", Nat.show(List.length(&2, A.Memo, A.Signals.keep(A.Sidebar.memos(A.Signals.save(False{}, F.seed(), K.SignalCache.obtain("epoch", F.state(), None{})))))), "0")
    C.Check.str("rejected text save releases previous canonical slot", Nat.show(List.length(&2, A.Memo, A.SignalText.keep(A.Sidebar.memos(A.SignalText.save(False{}, F.seed(), F.text()))))), "0")
    C.Check.str("moved destination never inherits the other draft as its composer", U.Ui.draft_text(A.Actor.ui(moved(F.seed(), F.state(), "other", "new-view", True{}))), "")
    C.Check.str("per-thread draft survives without becoming memo data", U.Ui.scratch(A.Actor.ui(moved(F.seed(), F.state(), "other", "new-view", True{})), "self"), "PRIVATE 日本語 IME draft")
