import Base
import ./check.bend as C
import ../src/server/thin-actor.bend as A
import ../src/server/thin-side-index.bend as X
import ../src/server/thin-signal-text.bend as TXT
import ../src/core/signal.bend as CORE
import ../src/server/thin-signals.bend as SG
import ../src/server/thin-view.bend as V
import ../src/server/thin-sidebar-wire.bend as W
import ../src/core/model.bend as M
import ../src/core/json.bend as J
import ../src/core/client.bend as U

def state() -> M.State:
  M.State.replay([M.ProjectCreated{"p", "Board", "/tmp/signal-revision", 1n},
    M.ThreadCreated{"self", "p", "Here", M.Local{}, "", "", "codex", "", 2n},
    M.ThreadCreated{"other", "p", "Other", M.Local{}, "", "", "codex", "", 3n},
    M.TurnChanged{"other", M.Running{}, 4n}], M.State.empty())

def ui(st: M.State, clock: Nat) -> U.Ui:
  U.Ui.info_set(U.Ui.scratch_set(U.Ui.flag_on(U.Ui.select.k(U.Ui.adopt.raw(U.Ui.init(clock), st), "self"), "settings"), "self", "PRIVATE 日本語 IME draft"), "local.mailroom", "on")

def base() -> A.Actor:
  A.SideIndex.save(True{}, A.Signals.save(True{}, A.Actor.make(ui(state(), 10n), "view", V.Target.json("thread", "self")), SG.SignalCache.obtain("epoch", state(), None{})), X.IndexCache.obtain("epoch", state(), None{}))

def text() -> TXT.SignalTextCache:
  TXT.SignalTextCache.build("epoch", M.State.seq(state()), True{}, CORE.Signal.index(state()))

def seed() -> A.Actor:
  A.SignalText.save(True{}, base(), text())

def count(a: A.Actor) -> String:
  Nat.show(List.length(&2, A.Memo, A.SignalText.keep(A.Sidebar.memos(a))))

def refresh(target: String, view: String) -> A.Actor:
  A.Actor.refresh([seed()], True{}, state(), J.Json.obj(Nil{}), J.Json.obj(Nil{}), 11n,
    J.Json.obj([J.KV{"target", V.Target.json("thread", target)}, J.KV{"session", V.Route.pack(ui(state(), 11n))}]), view)

def remembered() -> A.Actor:
  A.Actor.remember(W.Region.save(seed(), "key", "receipt"), A.Memo{"picture", 10n, False{}, J.Null{}})

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("same owner and target retain text", count(refresh("self", "view")), "1")
    C.Check.str("other target retains canonical text", count(refresh("other", "view")), "1")
    C.Check.str("other view retains canonical text", count(refresh("self", "other-view")), "1")
    C.Check.str("dirty action keeps canonical text (re-checked on read)", count(A.Actor.mark_dirty(seed())), "1")
    C.Check.str("info update keeps canonical text (re-checked on read)", count(A.Actor.info(seed(), J.Json.obj([J.KV{"info", J.Json.obj([J.KV{"models.v", J.Str{"fresh"}}])}]))), "1")
    C.Check.str("new schematic state drops text", count(A.Actor.sheets(seed(), J.Json.obj(Nil{}))), "0")
    C.Check.str("render remember preserves one text entry", count(remembered()), "1")
    C.Check.str("metadata save preserves raw signals", Nat.show(List.length(&2, A.Memo, A.Signals.keep(A.Sidebar.memos(seed())))), "1")
    C.Check.str("metadata save preserves transport receipt", W.Region.token(A.Sidebar.memos(remembered()), "key"), "receipt")
    C.Check.str("saving metadata preserves text", count(A.SideIndex.save(True{}, seed(), X.IndexCache.obtain("epoch", state(), None{}))), "1")
    C.Check.str("saving raw signals preserves text", count(A.Signals.save(True{}, seed(), SG.SignalCache.obtain("epoch", state(), None{}))), "1")
    C.Check.str("text save preserves one plain metadata entry", Nat.show(List.length(&2, A.Memo, A.SideIndex.keep(A.Sidebar.memos(seed())))), "1")
