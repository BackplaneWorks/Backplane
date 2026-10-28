import Base
import ./check.bend as C
import ./chat_sidebar_projection_test.bend as F
import ./chat_sidebar_projection_display_test.bend as P
import ./sidebar_draft_image_test.bend as D
import ../src/core/accounts.bend as A
import ../src/core/client.bend as U
import ../src/core/model.bend as M
import ../src/core/json.bend as J

def inventory() -> String:
  "{\"profiles\":[{\"id\":\"A\",\"provider\":\"codex\",\"epoch\":2,\"state\":\"connected\",\"label\":\"Work\",\"catalog\":[{\"id\":\"a-model\",\"efforts\":[\"low\"]}]},{\"id\":\"B\",\"provider\":\"codex\",\"epoch\":2,\"state\":\"connected\",\"label\":\"Personal\",\"catalog\":[{\"id\":\"b-model\"}]}]}"

def state() -> M.State:
  M.State.replay([M.SettingSet{"accounts.inventory", inventory(), 5n}, M.SettingSet{A.Accounts.binding.key("self", "codex"), A.Accounts.binding.show("A", 2n), 5n}], F.state())

def ui() -> U.Ui:
  F.ordinary(state(), 10n)

def after(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}: u

def sent(cs: List<&2, U.Cmd>) -> String:
  match cs:
    case Nil{}: ""
    case U.Send{text} <> _: text
    case _ <> t: sent(t)

def request(a: U.Act) -> J.Json:
  match a:
    case U.Act{_, cs}: A.Accounts.parse(sent(cs))

def params(a: U.Act) -> J.Json:
  J.Json.get_obj(request(a), "p")

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("selection preserves current draft", U.Ui.draft_text(after(U.Acv.act(ui(), "acct-select", "B"))), U.Ui.draft_text(ui()))
    C.Check.str("selection names exact profile", J.Json.get_str(params(U.Acv.act(ui(), "acct-select", "B")), "profile", ""), "B")
    C.Check.str("selection compares previous binding", J.Json.get_str(params(U.Acv.act(ui(), "acct-select", "B")), "expected", ""), A.Accounts.binding.show("A", 2n))
    C.Check.str("selection names target generation", Nat.show(J.Json.get_nat(params(U.Acv.act(ui(), "acct-select", "B")), "epoch", 0n)), "2")
    C.Check.str("send captures account ownership in durable outbox request", J.Json.get_str(params(U.Ui.send.rpc(ui(), "message", "")), "accountBinding", ""), A.Accounts.binding.show("A", 2n))
    C.Check.str("model request carries the same account fence", J.Json.get_str(params(U.Ui.model(ui(), "a-model")), "accountBinding", ""), A.Accounts.binding.show("A", 2n))
    C.Check.str("named draft edit retains sidebar image", F.equal(D.key(ui()), D.key(U.Ui.scratch_set(ui(), "self", "未確定 日本語 👩🏽‍💻"))), "same")
    C.Check.str("named IME retained sidebar keeps complete current frame exact", D.exact(ui(), U.Ui.scratch_set(ui(), "self", "未確定 日本語 👩🏽‍💻")), "same")
    C.Check.str("account picker scratch invalidates conservatively", F.equal(D.key(ui()), D.key(U.Ui.scratch_set(ui(), "@acct.provider", "claude"))), "different")
    C.Check.str("account form has exact fresh fallback", D.exact(ui(), U.Ui.scratch_set(ui(), "@f.accountlabel", "Personal")), "same")
    C.Check.str("login status changes invalidate private image", F.equal(D.key(ui()), D.key(U.Ui.info_set(ui(), "accounts.live", "{\"revision\":4}"))), "different")
    C.Check.str("login status miss keeps complete frame exact", D.exact(ui(), U.Ui.info_set(ui(), "accounts.live", "{\"revision\":4}")), "same")
    C.Check.str("catalog follows selected account only", J.Json.show(J.Json.get_obj(A.Accounts.parse(A.Accounts.overlay("{\"codex\":[\"legacy\"],\"claude\":[\"other\"]}", state(), "self")), "codex")), "[\"a-model\"]")
    C.Check.str("other provider catalog is preserved", J.Json.show(J.Json.get_obj(A.Accounts.parse(A.Accounts.overlay("{\"codex\":[\"legacy\"],\"claude\":[\"other\"]}", state(), "self")), "claude")), "[\"other\"]")
    C.Check.str("wrong generation has no catalog", J.Json.show(J.Json.arr(A.Accounts.catalog(M.State.replay([M.SettingSet{A.Accounts.binding.key("self", "codex"), A.Accounts.binding.show("A", 9n), 6n}], state()), "self", "codex"))), "[]")
    C.Check.str("unknown named profile never labels as default", A.Accounts.label(M.State.replay([M.SettingSet{A.Accounts.binding.key("self", "codex"), A.Accounts.binding.show("missing", 2n), 6n}], state()), "self", "codex"), "Unavailable account")
    return Unit{}
