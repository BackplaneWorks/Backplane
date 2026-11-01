import Base
import ./check.bend as C
import ../src/server/thin-actor.bend as A
import ../src/server/thin-view.bend as V
import ../src/server/thin-sidebar.bend as S
import ../src/web/sidebar-slot.bend as SS
import ../src/core/model.bend as M
import ../src/core/client.bend as U
import ../src/core/json.bend as J
import ../src/core/thin.bend as T
import ../src/core/thin-live.bend as L

def state() -> M.State:
  M.State.replay([M.ProjectCreated{"p", "Board", "/tmp/sidebar", 1n},
    M.ThreadCreated{"self", "p", "Here", M.Local{}, "", "", "codex", "", 2n},
    M.ThreadCreated{"other", "p", "Other", M.Local{}, "", "", "codex", "", 3n}], M.State.empty())

# stamped as every web session is (app.bend's stamp: mailroom, the rail and
# the top bar), so an action's own stamp leaves the region key alone
def ui() -> U.Ui:
  U.Ui.info_set(U.Ui.info_set(U.Ui.info_set(U.Ui.scratch_set(U.Ui.scratch_set(U.Ui.flag_on(U.Ui.select.k(U.Ui.adopt.raw(U.Ui.init(10n), state()), "self"), "settings"), "@f.kpath", ""), "self", "PRIVATE 日本語 draft"), "local.mailroom", "on"), "local.workspace.layout", "rail-v1"), "local.workspace.chrome", "top-v1")

def key(u: U.Ui) -> String:
  S.Sidebar.key(u, "epoch", "view")

def same(+a: String, +b: String) -> String:
  Bool.pick(String, C.Check.same(a, b), "same", "different")

def yes(b: Bool) -> String:
  Bool.pick(String, b, "yes", "no")

def tick(u: U.Ui) -> U.Ui:
  match u:
    case U.Ui{st, sel, dr, live, op, on, _, ad, pa, v, rid, want, error, note, nf, ob, stores}:
      U.Ui{st, sel, dr, live, op, on, 11n, ad, pa, v, rid, want, error, note, nf, ob, stores}

def request(action: String, value: String) -> J.Json:
  J.Json.obj([J.KV{"target", V.Target.json("thread", "self")}, J.KV{"action", J.Str{action}}, J.KV{"value", J.Str{value}}])

def prepared(u: U.Ui) -> U.Ui:
  V.Render.ui(u, state(), T.Index.empty(), V.Target.json("thread", "self"), Nil{}, L.Stream.blank(), 10n)

def cached(+u: U.Ui) -> A.Actor:
  A.Sidebar.save(True{}, A.Actor.make(u, "view", V.Target.json("thread", "self")), key(prepared(u)), SS.Slot.sidebar(V.Render.web(prepared(u), J.Json.obj(Nil{}))))

def result(r: A.Rendered) -> J.Json:
  match r:
    case A.Rendered{_, value}: value

def equal_reply(+p: J.Json, +before: U.Ui) -> String:
  +action = V.Action.run(True{}, before, p, 10n)
  +a = A.Actor.accept(cached(before), action, False{})
  +got = result(A.Actor.render.full(True{}, a, action, state(), T.Index.empty(), p, "epoch", 10n, Nil{}, L.Stream.blank()))
  +want = V.Obj.put(V.Render.acted_live(action, True{}, state(), T.Index.empty(), p, "epoch", 10n, Nil{}, L.Stream.blank()), "mailDraftContext", A.Actor.mail_context(A.Actor.ui(a)))
  same(J.Json.show(got), J.Json.show(want))

def slot_request() -> String:
  +p = request("settings-page", "appearance")
  +a = A.Actor.accept(cached(ui()), V.Action.run(True{}, ui(), p, 10n), False{})
  +node = A.Sidebar.lookup(S.Sidebar.request(p, A.Actor.ui(a)), A.Sidebar.memos(a), key(prepared(A.Actor.ui(a))))
  yes(SS.Slot.is(node, "aside"))

def target_changed() -> String:
  +p = J.Json.obj([J.KV{"target", V.Target.json("thread", "other")}, J.KV{"session", V.Route.pack(ui())}])
  +a = A.Actor.refresh([cached(ui())], True{}, state(), J.Json.obj(Nil{}), J.Json.obj(Nil{}), 10n, p, "view")
  Nat.show(List.length(&2, A.Memo, A.Sidebar.keep(A.Sidebar.memos(a))))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("Settings category reuses exact server sidebar", slot_request(), "yes")
    C.Check.str("warm Settings reply including draft/window/commands equals fresh", equal_reply(request("settings-page", "appearance"), ui()), "same")
    C.Check.str("Close reply retains every ordered ordinary command", equal_reply(request("flag", "settings"), ui()), "same")
    C.Check.str("canonical event invalidates region", same(key(ui()), key(U.Ui.adopt.raw(ui(), M.State.apply(state(), M.SettingSet{"ui.theme", "light", 4n})))), "different")
    C.Check.str("clock invalidates age and cat presentation", same(key(ui()), key(tick(ui()))), "different")
    C.Check.str("canonical info invalidates region", same(key(ui()), key(U.Ui.info_set(ui(), "connection.status", "changed"))), "different")
    C.Check.str("language input invalidates region", same(key(ui()), key(U.Ui.scratch_set(ui(), "@lang", "ja"))), "different")
    C.Check.str("search invalidates region", same(key(ui()), key(U.Ui.scratch_set(ui(), "@f.pq", "Board"))), "different")
    C.Check.str("fold invalidates region", same(key(ui()), key(U.Ui.flag_on(ui(), "conversation-history"))), "different")
    C.Check.str("draft edit conservatively invalidates region", same(key(ui()), key(U.Ui.scratch_set(ui(), "self", "PRIVATE 日本語 edit"))), "different")
    C.Check.str("Settings category fields alone keep region identity", same(key(ui()), key(U.Act.ui(U.Settings.go(ui(), "appearance")))), "same")
    C.Check.str("view scope invalidates region", same(key(ui()), S.Sidebar.key(ui(), "epoch", "new-view")), "different")
    C.Check.str("epoch invalidates region", same(key(ui()), S.Sidebar.key(ui(), "new-epoch", "view")), "different")
    C.Check.str("new target drops retained private region", target_changed(), "0")
    C.Check.str("non Settings actions never opt into reuse", yes(S.Sidebar.request(request("term-key", "x"), ui())), "no")
    C.Check.str("slot has unique internal identity", Nat.show(SS.Slot.count(J.Json.obj([J.KV{"type", J.Str{"element"}}, J.KV{"tag", J.Str{"bp-server-sidebar-slot"}}, J.KV{"key", J.Str{"sidebar"}}]))), "1")
