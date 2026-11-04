import Base
import ../src/core/client.bend as U
import ../src/core/model.bend as M
import ../src/core/modelpick.bend as MP
import ../src/core/accounts.bend as A
import ../src/core/json.bend as J
import ../src/core/i18n.bend as I
import ../src/web/dom.bend as D
import ../src/web/view.bend as V
import ../src/web/presentation.bend as WP

# The pre-optimization render expression, frozen as a whole-DOM oracle.
def old.rows(+lc: I.Loc, +nf: Map<&2, String>, +c: MP.MpCur, +now: MP.MpCur, +q: String) -> List<&2, D.Node>:
  +ms = U.Mpk.models(nf, c, now, q)
  V.View.mp.open(lc, ms, U.Models.view(U.Models.of(nf, MP.MpCur.agent(c)), MP.MpCur.agent(c), ms), c, now, U.Mpk.steps(nf, c), q,
    U.Models.search(U.Models.menu(U.Models.of(nf, MP.MpCur.agent(c)), MP.MpCur.agent(c))), String.eq(U.Str.get(nf, "local.workspace.form"), "phone"))

def info(catalog: String, oc: String, phone: String) -> Map<&2, String>:
  Map.set(&2, String, Map.set(&2, String, Map.set(&2, String, Map.new(&2, String), "models", catalog), "models.oc", oc), "local.workspace.form", phone)

def nodes(ns: List<&2, D.Node>) -> String:
  J.Json.show(WP.Node.write(D.El{"div", "test", Nil{}, ns}))

def locale(ja: String) -> I.Loc:
  Bool.pick(I.Loc, String.eq(ja, "ja"), I.Ja{}, I.En{})

def old(catalog: String, oc: String, +agent: String, current: String, selected: String, effort: String, q: String, ja: String, own: String, phone: String) -> String:
  nodes(old.rows(locale(ja), info(catalog, oc, phone), MP.MpCur{1n, agent, selected, effort}, MP.MpCur{1n, own, current, "high"}, q))

def fresh(catalog: String, oc: String, +agent: String, current: String, selected: String, effort: String, q: String, ja: String, own: String, phone: String) -> String:
  nodes(V.View.mp.shown(locale(ja), info(catalog, oc, phone), MP.MpCur{1n, agent, selected, effort}, MP.MpCur{1n, own, current, "high"}, q))

def state(binding: String, inventory: String) -> M.State:
  M.State.replay([M.SettingSet{"accounts.inventory", inventory, 1n}, M.SettingSet{A.Accounts.binding.key("t", "codex"), binding, 1n}], M.State.empty())

def old.overlay(+text: String, +binding: String, +inventory: String) -> String:
  +st = state(binding, inventory)
  J.Json.show(A.Accounts.overlay.k(String.is_empty(A.Accounts.id(st, "t", "claude")),
    A.Accounts.overlay.k(String.is_empty(A.Accounts.id(st, "t", "codex")), A.Accounts.parse(text), st, "t", "codex"), st, "t", "claude"))

def fresh.overlay(text: String, binding: String, inventory: String) -> String:
  A.Accounts.overlay(text, state(binding, inventory), "t")
