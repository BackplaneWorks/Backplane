import Base
import ./check.bend as C
import ../src/server/thin-actor.bend as A
import ../src/server/thin-signals.bend as S
import ../src/server/thin-view.bend as V
import ../src/server/thin-sidebar.bend as SB
import ../src/server/thin-sidebar-wire.bend as W
import ../src/core/model.bend as M
import ../src/core/signal.bend as SG
import ../src/core/client.bend as U
import ../src/core/json.bend as J
import ../src/core/thin.bend as T
import ../src/core/thin-live.bend as L

def state() -> M.State:
  M.State.replay([M.ProjectCreated{"p", "Board", "/tmp/signal-revision", 1n},
    M.ThreadCreated{"self", "p", "Here", M.Local{}, "", "", "codex", "", 2n},
    M.ThreadCreated{"other", "p", "Other", M.Local{}, "", "", "codex", "", 3n},
    M.TurnChanged{"other", M.Running{}, 4n}], M.State.empty())

def yes(b: Bool) -> String:
  Bool.pick(String, b, "yes", "no")

def same(+a: String, +b: String) -> String:
  yes(C.Check.same(a, b))

def ui(st: M.State, clock: Nat) -> U.Ui:
  U.Ui.info_set(U.Ui.scratch_set(U.Ui.flag_on(U.Ui.select.k(U.Ui.adopt.raw(U.Ui.init(clock), st), "self"), "settings"), "self", "PRIVATE 日本語 IME draft"), "local.mailroom", "on")

def request() -> J.Json:
  J.Json.obj([J.KV{"target", V.Target.json("thread", "self")}, J.KV{"action", J.Str{"settings-page"}}, J.KV{"value", J.Str{"appearance"}}])

def cached() -> S.SignalCache:
  S.SignalCache.obtain("epoch", state(), None{})

def seed() -> A.Actor:
  A.Signals.save(True{}, A.Actor.make(ui(state(), 10n), "view", V.Target.json("thread", "self")), cached())

def refresh(+st: M.State, +u: U.Ui, target: String, view: String) -> A.Actor:
  A.Actor.refresh([seed()], True{}, st, J.Json.obj(Nil{}), J.Json.obj(Nil{}), 11n,
    J.Json.obj([J.KV{"target", V.Target.json("thread", target)}, J.KV{"session", V.Route.pack(u)}]), view)

def result(r: A.Rendered) -> J.Json:
  match r:
    case A.Rendered{_, res}: res

def equal(+st: M.State, +u: U.Ui, +stream: L.Stream) -> String:
  +p = request()
  +a = A.Actor.accept(seed(), V.ActResult{u, [J.Json.obj([J.KV{"type", J.Str{"keep"}}, J.KV{"id", J.Str{"self"}}, J.KV{"text", J.Str{"fresh command"}}])]}, False{})
  +act : V.ActResult = V.ActResult{u, [J.Json.obj([J.KV{"type", J.Str{"keep"}}, J.KV{"id", J.Str{"self"}}, J.KV{"text", J.Str{"fresh command"}}])]}
  same(J.Json.show(result(A.Actor.render.web(a, act, st, T.Index.empty(), p, "epoch", 11n, Nil{}, stream))),
    J.Json.show(result(A.Actor.render.web.fresh(a, act, st, T.Index.empty(), p, "epoch", 11n, Nil{}, stream))))

def event_reply(+e: M.Change) -> String:
  +st = M.State.apply(state(), e)
  equal(st, ui(st, 11n), L.Stream.blank())

def event_miss(+e: M.Change) -> String:
  +st = M.State.apply(state(), e)
  yes(S.SignalCache.hit(cached(), "epoch", M.State.seq(st)))

def after(+e: M.Change, id: String) -> String:
  +st = M.State.apply(state(), e)
  Nat.show(SG.Signal.at(S.SignalCache.signals(S.SignalCache.obtain("epoch", st, Some{cached()})), id))

def count(a: A.Actor) -> String:
  Nat.show(List.length(&2, A.Memo, A.Signals.keep(A.Sidebar.memos(a))))

def many(+n: Nat, m: Map<&2, Nat>) -> Map<&2, Nat>:
  match n:
    case 0n: m
    case 1n+k: many(k, Map.set(&2, Nat, m, Nat.show(k), 1n))

def offline(u: U.Ui) -> U.Ui:
  match u:
    case U.Ui{st, se, dr, li, op, _, now, ad, pa, ve, ri, wa, er, no, nf, ob, tr}:
      U.Ui{st, se, dr, li, op, False{}, now, ad, pa, ve, ri, wa, er, no, nf, ob, tr}

def retained() -> String:
  count(A.Actor.remember(W.Region.save(seed(), "private-key", "17"), A.Memo{"render", 10n, False{}, J.Null{}}))

def closed() -> M.State:
  M.State.replay([M.AskOpened{"approval", "other", "approval", "tool", "Allow", "", 5n}, M.AskClosed{"approval", "yes", 6n}], state())

def stream_index() -> String:
  +prepared = V.Render.ui(ui(state(), 11n), state(), T.Index.empty(), V.Target.json("thread", "self"), Nil{}, L.Stream.append(L.Stream.blank(), "live text", 2n), 11n)
  same(Nat.show(SG.Signal.at(SG.Signal.index(U.Ui.state(prepared)), "other")), Nat.show(SG.Signal.at(S.SignalCache.signals(cached()), "other")))

def clock_key() -> String:
  same(SB.Sidebar.key(ui(state(), 10n), "epoch", "view"), SB.Sidebar.key(ui(state(), 2000000000n), "epoch", "view"))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("same canonical revision hits", yes(S.SignalCache.hit(cached(), "epoch", M.State.seq(state()))), "yes")
    C.Check.str("new epoch misses", yes(S.SignalCache.hit(cached(), "new-epoch", M.State.seq(state()))), "no")
    C.Check.str("canonical sequence change misses", event_miss(M.AskOpened{"ask", "other", "input", "tool", "Choose", "", 5n}), "no")
    C.Check.str("clock-only complete reply matches fresh", equal(state(), ui(state(), 11n), L.Stream.blank()), "yes")
    C.Check.str("opened ask full reply is fresh", event_reply(M.AskOpened{"ask", "other", "input", "tool", "Choose", "", 5n}), "yes")
    C.Check.str("opened ask activity is blocked", after(M.AskOpened{"ask", "other", "input", "tool", "Choose", "", 5n}, "other"), "2")
    C.Check.str("owned watch full reply is fresh", event_reply(M.WorkSet{"self", "watch", "w\twatching\t\tbuild\twatch", 5n}), "yes")
    C.Check.str("owned watch activity, the turn over, is waiting", after(M.WorkSet{"self", "watch", "w\twatching\t\tbuild\twatch", 5n}, "self"), "6")
    C.Check.str("child ownership full reply is fresh", event_reply(M.TaskDelegated{"task", "self", "other", 1n, "codex", "", 5n}), "yes")
    C.Check.str("a delegated running child keeps its parent waiting (active bit) on miss", after(M.TaskDelegated{"task", "self", "other", 1n, "codex", "", 5n}, "self"), "6")
    C.Check.str("finished work full reply is fresh", event_reply(M.TurnChanged{"other", M.Completed{}, 5n}), "yes")
    C.Check.str("settled state full reply is fresh", event_reply(M.ThreadSettled{"self", False{}, 5n}), "yes")
    C.Check.str("auth hold full reply is fresh", event_reply(M.SettingSet{"auth.wait.other", "codex", 5n}), "yes")
    C.Check.str("online visibility is reapplied", equal(state(), offline(ui(state(), 11n)), L.Stream.blank()), "yes")
    C.Check.str("fresh info and model counter preserved", equal(state(), U.Ui.info_set(U.Ui.info_set(ui(state(), 11n), "models.v", "42"), "connection.status", "fresh"), L.Stream.blank()), "yes")
    C.Check.str("fresh draft preserved", equal(state(), U.Ui.scratch_set(ui(state(), 11n), "self", "new composed 日本語"), L.Stream.blank()), "yes")
    C.Check.str("fresh live timeline preserved", equal(state(), U.Ui.flag_off(ui(state(), 11n), "settings"), L.Stream.append(L.Stream.blank(), "stream delta 日本語", 2n)), "yes")
    C.Check.str("same target refresh retains signal memo", count(refresh(state(), ui(state(), 11n), "self", "view")), "1")
    C.Check.str("other target retains canonical signal memo", count(refresh(state(), ui(state(), 11n), "other", "view")), "1")
    C.Check.str("other view retains canonical signal memo", count(refresh(state(), ui(state(), 11n), "self", "new-view")), "1")
    C.Check.str("dirty actor keeps canonical signal memo (re-checked on read)", count(A.Actor.mark_dirty(seed())), "1")
    C.Check.str("ordinary render memo and wire receipt retain one signal memo", retained(), "1")
    C.Check.str("8192 entries remain bounded", yes(S.SignalCache.bounded(S.SignalCache.build("epoch", 1n, many(8192n, Map.new(&2, Nat))))), "yes")
    C.Check.str("8193 entries are never reusable", yes(S.SignalCache.hit(S.SignalCache.build("epoch", 1n, many(8193n, Map.new(&2, Nat))), "epoch", 1n)), "no")
    C.Check.str("tool approval full reply is fresh", event_reply(M.AskOpened{"approval", "other", "approval", "tool", "Allow", "", 5n}), "yes")
    C.Check.str("closed approval resumes visible activity", equal(closed(), ui(closed(), 11n), L.Stream.blank()), "yes")
    C.Check.str("pending count full reply is fresh", event_reply(M.PendingChanged{"other", 1n, 5n}), "yes")
    C.Check.str("failure full reply is fresh", event_reply(M.TurnChanged{"other", M.Failed{}, 5n}), "yes")
    C.Check.str("fallback held queue full reply is fresh", event_reply(M.TurnQueued{"self", "fallback:held:1", "pending request", 5n}), "yes")
    C.Check.str("reopened shelf full reply is fresh", event_reply(M.ThreadUnsettled{"self", 5n}), "yes")
    C.Check.str("ended watched work full reply is fresh", event_reply(M.WorkSet{"self", "watch", "w\t" ++ M.Bg.ended.mark() ++ "completed", 5n}), "yes")
    C.Check.str("launch parent ownership full reply is fresh", event_reply(M.ThreadLaunched{"other", "self", True{}, 5n}), "yes")
    C.Check.str("time-derived presentation retains clock invalidation", clock_key(), "no")
    C.Check.str("clock-only age transition full reply is fresh", equal(state(), ui(state(), 2000000000n), L.Stream.blank()), "yes")
    C.Check.str("window and stream do not alter canonical signal inputs", stream_index(), "yes")
    C.Check.str("info notification keeps canonical signals (re-checked on read)", count(A.Actor.info(seed(), J.Json.obj([J.KV{"info", J.Json.obj([J.KV{"models.v", J.Str{"new"}}])}]))), "1")
