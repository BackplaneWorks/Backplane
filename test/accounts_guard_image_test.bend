import Base
import ./check.bend as C
import ./accounts_ui_test.bend as Q
import ./chat_sidebar_projection_test.bend as F
import ./chat_sidebar_projection_display_test.bend as P
import ./sidebar_draft_image_test.bend as D
import ../src/core/accounts.bend as AC
import ../src/core/client.bend as U
import ../src/core/model.bend as M
import ../src/core/signal.bend as SG
import ../src/core/json.bend as J
import ../src/server/thin-actor.bend as A
import ../src/web/sidebar-slot.bend as S

def ui() -> U.Ui:
  U.Ui.flag_on(Q.ui(), "accounts")

def edited() -> U.Ui:
  U.Ui.scratch_set(ui(), "self", "CURRENT 未確定 日本語 👩🏽‍💻")

def node() -> J.Json:
  S.Slot.sidebar(J.Json.get_obj(P.frame(ui(), J.Null{}), "shown"))

# Every state change in this fixture is metadata (SettingSet/SessionSet),
# so SignalRevision.advance selects its actual post-commit canonical head.
# Ordinary draft edits keep both the state and that trusted revision.
def key(+u: U.Ui) -> String:
  A.SidebarImage.key.input(False{}, u, "epoch", "view", M.State.seq(U.Ui.state(u)))

def lookup(+after: U.Ui, cached: J.Json) -> J.Json:
  A.SidebarImage.lookup([A.SidebarImage{key(ui()), cached}], key(after))

def frame(+after: U.Ui, cached: J.Json) -> J.Json:
  J.Json.get_obj(P.frame(after, lookup(after, cached)), "shown")

def attr(xs: List<&2, J.Json>, +name: String) -> String:
  match xs:
    case Nil{}: ""
    case +h <> t:
      Bool.pick(String, String.eq(J.Json.get_str(h, "name", ""), name), J.Json.get_str(h, "value", ""), attr(t, name))

def find.hit(+j: J.Json, +tag: String, +key: String) -> Bool:
  Bool.and(String.eq(J.Json.get_str(j, "type", ""), "element"), Bool.and(String.eq(J.Json.get_str(j, "tag", ""), tag), String.eq(J.Json.get_str(j, "key", ""), key)))

def find(j: J.Json, +tag: String, +key: String, hit: Bool) -> List<&2, J.Json>:
  match j hit:
    case J.Obj{fs} True{}: [J.Obj{fs}]
    case J.Obj{fs} False{}: find(fs, tag, key, False{})
    case J.Field{_, +v, t} _: List.append(&2, J.Json, find(v, tag, key, find.hit(v, tag, key)), find(t, tag, key, False{}))
    case J.Arr{xs} _: find(xs, tag, key, False{})
    case J.Item{+h, t} _: List.append(&2, J.Json, find(h, tag, key, find.hit(h, tag, key)), find(t, tag, key, False{}))
    case _ _: Nil{}

def element(+j: J.Json, tag: String, key: String) -> J.Json:
  S.Slot.one(find(j, tag, key, False{}))

def textarea(j: J.Json) -> String:
  attr(J.Json.elems(J.Json.get_obj(element(j, "textarea", "composer"), "attrs")), "value")

def exists(j: J.Json) -> String:
  Bool.show(Bool.not(String.eq(J.Json.show(j), "null")))

def metadata(+snapshot: J.Json) -> U.Ui:
  +before = ui()
  +st = U.Ui.state(before)
  U.Ui.adopt.raw(before, M.State.replay(AC.Accounts.changes(st, 11n, J.Json.show(snapshot)), st))

def inventory() -> U.Ui:
  metadata(AC.Accounts.put(AC.Accounts.parse(Q.inventory()), "revision", J.Json.nat(7n)))

def catalog() -> U.Ui:
  metadata(J.Json.obj([J.KV{"revision", J.Json.nat(8n)}, J.KV{"profiles", J.Json.arr([
    AC.Accounts.put(AC.Accounts.profile(Q.state(), "A"), "catalog", J.Json.arr([J.Json.obj([J.KV{"id", J.Str{"new-catalog"}}])])),
    AC.Accounts.profile(Q.state(), "B")])}]))

def binding() -> U.Ui:
  +before = ui()
  +st = U.Ui.state(before)
  U.Ui.adopt.raw(before, M.State.replay(AC.Accounts.select(st, 11n, "self", "codex", "B", 2n, "session-A", 5n), st))

def exact(+after: U.Ui) -> String:
  F.equal(J.Json.show(frame(after, node())), J.Json.show(J.Json.get_obj(P.frame(after, J.Null{}), "shown")))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("SettingSet selects the actual post-commit metadata revision", Nat.show(SG.SignalRevision.advance(AC.Accounts.changes(Q.state(), 11n, J.Json.show(AC.Accounts.put(AC.Accounts.parse(Q.inventory()), "revision", J.Json.nat(7n)))), M.State.seq(Q.state()), M.State.seq(U.Ui.state(inventory())))), Nat.show(M.State.seq(U.Ui.state(inventory()))))
    C.Check.str("account/session selection selects actual metadata revision", Nat.show(SG.SignalRevision.advance(AC.Accounts.select(Q.state(), 11n, "self", "codex", "B", 2n, "session-A", 5n), M.State.seq(Q.state()), M.State.seq(U.Ui.state(binding())))), Nat.show(M.State.seq(U.Ui.state(binding()))))
    C.Check.str("real sidebar node is valid with account popup open", Bool.show(S.Slot.is(node(), "aside")), "True")
    C.Check.str("popup plus changed IME draft produces actual non-null lookup HIT", exists(lookup(edited(), node())), "True")
    C.Check.str("HIT returns the exact saved sidebar node", J.Json.show(lookup(edited(), node())), J.Json.show(node()))
    C.Check.str("complete retained-HIT frame equals fresh output", exact(edited()), "same")
    C.Check.str("complete HIT frame textarea holds current IME draft", textarea(frame(edited(), node())), "CURRENT 未確定 日本語 👩🏽‍💻")
    C.Check.str("account options are present in actual complete HIT frame", exists(element(frame(edited(), node()), "div", "account-picker")), "True")
    C.Check.str("current account label exists in actual account popup DOM", Bool.show(String.contains(J.Json.show(element(frame(edited(), node()), "div", "account-picker")), "Work")), "True")
    C.Check.str("other verified account option exists in popup DOM", Bool.show(String.contains(J.Json.show(element(frame(edited(), node()), "div", "account-picker")), "Personal")), "True")
    C.Check.str("inventory SettingSet revision makes actual lookup MISS", J.Json.show(lookup(inventory(), node())), "null")
    C.Check.str("inventory metadata MISS has exact full fresh fallback", exact(inventory()), "same")
    C.Check.str("binding change makes actual lookup MISS", J.Json.show(lookup(binding(), node())), "null")
    C.Check.str("binding change MISS has exact complete current frame", exact(binding()), "same")
    C.Check.str("catalog metadata revision makes actual lookup MISS", J.Json.show(lookup(catalog(), node())), "null")
    C.Check.str("catalog MISS has exact fresh fallback", exact(catalog()), "same")
    C.Check.str("transient live status makes actual lookup MISS", J.Json.show(lookup(U.Ui.info_set(ui(), "accounts.live", "{\"revision\":9}"), node())), "null")
    C.Check.str("malformed cached value actually reaches matching lookup", J.Json.show(lookup(edited(), J.Str{"malformed-cached-node"})), "\"malformed-cached-node\"")
    C.Check.str("malformed lookup value reaches fresh full-frame fallback", F.equal(J.Json.show(frame(edited(), J.Str{"malformed-cached-node"})), J.Json.show(J.Json.get_obj(P.frame(edited(), J.Null{}), "shown"))), "same")
    C.Check.str("malformed fallback still has the exact current textarea", textarea(frame(edited(), J.Str{"malformed-cached-node"})), "CURRENT 未確定 日本語 👩🏽‍💻")
    return Unit{}
