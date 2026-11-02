import Base
import ../src/server/thin-sidebar.bend as ACTORKEY
import ./check.bend as C
import ../src/server/thin-actor.bend as A
import ../src/server/thin-side-index.bend as X
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

def old.ui(st: M.State, clock: Nat) -> U.Ui:
  U.Ui.info_set(U.Ui.scratch_set(U.Ui.flag_on(U.Ui.select.k(U.Ui.adopt.raw(U.Ui.init(clock), st), "self"), "settings"), "self", "PRIVATE 日本語 IME draft"), "local.mailroom", "on")

def seed() -> A.Actor:
  A.SideIndex.save(True{}, A.Signals.save(True{}, A.Actor.make(old.ui(state(), 10n), "view", V.Target.json("thread", "self")), SG.SignalCache.obtain("epoch", state(), None{})), X.IndexCache.obtain("epoch", state(), None{}))

def count(a: A.Actor) -> String:
  Nat.show(List.length(&2, A.Memo, A.SideIndex.keep(A.Sidebar.memos(a))))

def refresh(target: String, view: String) -> A.Actor:
  A.Actor.refresh([seed()], True{}, state(), J.Json.obj(Nil{}), J.Json.obj(Nil{}), 11n,
    J.Json.obj([J.KV{"target", V.Target.json("thread", target)}, J.KV{"session", V.Route.pack(old.ui(state(), 11n))}]), view)

def remembered() -> A.Actor:
  A.Actor.remember(W.Region.save(seed(), "key", "receipt"), A.Memo{"picture", 10n, False{}, J.Null{}})

def image.node() -> J.Json:
  J.Json.obj([J.KV{"tag", J.Str{"aside"}}, J.KV{"key", J.Str{"sidebar"}}, J.KV{"text", J.Str{"rendered rows"}}])

def image.seed() -> A.Actor:
  A.SidebarImage.save(True{}, W.Region.save(seed(), "picture", "receipt"), "owned", image.node())

def image.count(a: A.Actor) -> String:
  Nat.show(List.length(&2, A.Memo, A.SidebarImage.keep(A.Sidebar.memos(a))))

def image.refresh(target: String, view: String) -> A.Actor:
  A.Actor.refresh([image.seed()], True{}, state(), J.Json.obj(Nil{}), J.Json.obj(Nil{}), 11n,
    J.Json.obj([J.KV{"target", V.Target.json("thread", target)}, J.KV{"session", V.Route.pack(old.ui(state(), 11n))}]), view)

def equal(a: String, b: String) -> String:
  Bool.pick(String, C.Check.same(a, b), "same", "different")

def image.key(u: U.Ui, epoch: String, view: String, revision: Nat) -> String:
  A.SidebarImage.key(False{}, u, epoch, view, revision)

def ordinary(st: M.State, clock: Nat) -> U.Ui:
  U.Ui.flag_off(old.ui(st, clock), "settings")

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("private exact key returns own rows", J.Json.show(A.SidebarImage.lookup(A.Sidebar.memos(image.seed()), "owned")), J.Json.show(image.node()))
    C.Check.str("different key cannot read cached rows", J.Json.show(A.SidebarImage.lookup(A.Sidebar.memos(image.seed()), "other")), "null")
    C.Check.str("same actor and target retain one image", image.count(image.refresh("self", "view")), "1")
    C.Check.str("different target drops image", image.count(image.refresh("other", "view")), "0")
    C.Check.str("different view drops image", image.count(image.refresh("self", "other")), "0")
    # within the same view the image is kept: its key is every input it is
    # drawn from, so a change misses it (c286c324f, Actor.same.view)
    C.Check.str("dirty action keeps the keyed image", image.count(A.Actor.mark_dirty(image.seed())), "1")
    C.Check.str("accepted info update keeps the keyed image", image.count(A.Actor.info(image.seed(), J.Json.obj([J.KV{"info", J.Json.obj([J.KV{"models.v", J.Str{"new"}}])}]))), "1")
    C.Check.str("schematic reply drops image", image.count(A.Actor.sheets(image.seed(), J.Json.obj(Nil{}))), "0")
    C.Check.str("remember preserves exactly one image", image.count(A.Actor.remember(image.seed(), A.Memo{"picture", 10n, False{}, J.Null{}})), "1")
    C.Check.str("replacement still retains only one image", image.count(A.SidebarImage.save(True{}, image.seed(), "next", J.Null{})), "1")
    C.Check.str("disabled save clears image", image.count(A.SidebarImage.save(False{}, image.seed(), "next", J.Null{})), "0")
    C.Check.str("image cannot provide wire authority", W.Region.key(A.Sidebar.memos(image.seed())), "")
    C.Check.str("image does not change wire receipt", W.Region.token(A.Sidebar.memos(image.seed()), "picture"), "receipt")
    C.Check.str("projection save retains raw signals", Nat.show(List.length(&2, A.Memo, A.Signals.keep(A.Sidebar.memos(image.seed())))), "1")
    C.Check.str("projection save retains plain index", count(image.seed()), "1")
    C.Check.str("ordinary history shares render metadata key", equal(image.key(ordinary(state(), 10n), "epoch", "view", 1n), image.key(ordinary(M.State.apply(state(), M.MessagePosted{"self", "a", M.Assistant{}, "new body", 10n}), 10n), "epoch", "view", 1n)), "same")
    C.Check.str("ordinary wire picture keeps full sequence", equal(ACTORKEY.Sidebar.key.for(False{}, ordinary(state(), 10n), "epoch", "view", 1n), ACTORKEY.Sidebar.key.for(False{}, ordinary(M.State.apply(state(), M.MessagePosted{"self", "a", M.Assistant{}, "new body", 10n}), 10n), "epoch", "view", 1n)), "different")
    C.Check.str("clock always fences render cache", equal(image.key(ordinary(state(), 10n), "epoch", "view", 1n), image.key(ordinary(state(), 11n), "epoch", "view", 1n)), "different")
    C.Check.str("epoch always fences render cache", equal(image.key(ordinary(state(), 10n), "epoch", "view", 1n), image.key(ordinary(state(), 10n), "other", "view", 1n)), "different")
    C.Check.str("owner always fences render cache", equal(image.key(ordinary(state(), 10n), "epoch", "view", 1n), image.key(ordinary(state(), 10n), "epoch", "other", 1n)), "different")
    C.Check.str("trusted revision fences render cache", equal(image.key(ordinary(state(), 10n), "epoch", "view", 1n), image.key(ordinary(state(), 10n), "epoch", "view", 2n)), "different")
    C.Check.str("current draft fences render cache", equal(image.key(ordinary(state(), 10n), "epoch", "view", 1n), image.key(U.Ui.scratch_set(ordinary(state(), 10n), "self", "Changed IME"), "epoch", "view", 1n)), "different")
    C.Check.str("expansion flag fences render cache", equal(image.key(ordinary(state(), 10n), "epoch", "view", 1n), image.key(U.Ui.flag_on(ordinary(state(), 10n), "active-settled"), "epoch", "view", 1n)), "different")
    C.Check.str("current model info fences render cache", equal(image.key(ordinary(state(), 10n), "epoch", "view", 1n), image.key(U.Ui.info_set(ordinary(state(), 10n), "models", "Fresh models"), "epoch", "view", 1n)), "different")
