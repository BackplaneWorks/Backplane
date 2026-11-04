import Base
import ./check.bend as C
import ../src/core/client.bend as U
import ../src/core/json.bend as J
import ../src/core/i18n.bend as I
import ../src/core/msgs.bend as MSG
import ../src/web/dom.bend as D
import ../src/web/view.bend as V
import ../src/web/presentation.bend as WP
import ../src/core/modelpick.bend as MPK

# Frozen pre-change expression and recursive row builder are the oracle.
def old.label(+info: String, +provider: String, +model: String) -> String:
  J.Json.get_str(J.Json.get_obj(U.Models.about(info, provider), "labels"), Bool.pick(String, String.is_empty(model), "default", model), "")

def old.rows(+lc: I.Loc, ms: List<&2, String>, +cur: String, +info: String, +prov: String) -> List<&2, D.Node>:
  match ms:
    case Nil{}: Nil{}
    case +m <> tail:
      D.El{"button", "", [D.At{"class", Bool.pick(String, String.eq(m, cur), "row on own", "row")}, D.At{"title", MSG.T.model.choose(lc, Bool.pick(String, String.is_empty(m), MSG.T.model.default.article(lc), m))}, D.At{"data-value", m}, D.On{"click", "mp-model"}],
        [D.Dom.span("name", Bool.pick(String, String.is_empty(m), MSG.T.common.default(lc), m)), D.Dom.span("label", old.label(info, prov, m))]} <> old.rows(lc, tail, cur, info, prov)

def nodes(ns: List<&2, D.Node>) -> String:
  J.Json.show(WP.Node.write(D.El{"div", "test", Nil{}, ns}))

def same(a: String, b: String) -> String:
  Bool.pick(String, C.Check.same(a, b), "same", "different")

def exact(+lc: I.Loc, +ms: List<&2, String>, +cur: String, +info: String, +prov: String) -> String:
  same(nodes(V.View.models.loc(lc, ms, cur, cur, info, prov)), nodes(old.rows(lc, ms, cur, info, prov)))

def catalog() -> String:
  "{\"codex\":[\"a\",\"b\"],\"about\":{\"codex\":{\"labels\":{\"default\":\"Default name\",\"a\":\"日本語 · A\",\"b\":\"B & <label>\",\"wrong\":17}},\"claude\":{\"labels\":{\"a\":\"Other provider\"}}}}"

# the whole picker (view.bend's View.mp.*): its model column holds the
# rows exactly, the agents beside it and the meter after
def menu(+lc: I.Loc, +info: String, +prov: String, +q: String) -> String:
  +ms = U.Models.shown(info, prov, q)
  +whole = nodes(V.View.menu.loc(lc, True{}, Nil{}, "a", info, prov, Nil{}, "high", q, False{}))
  +rows = nodes(old.rows(lc, ms, "a", U.Models.view(info, prov, ms), prov))
  Bool.pick(String, Bool.and(String.contains(whole, String.drop(String.take(rows, Nat.sub(String.length(rows), 2n)), 62n)),
    Bool.and(String.contains(whole, "mp-agent"), String.contains(whole, "mp-effort"))), "same", "different")

# a phone's picker (View.mp.open's phone, from the connection's form): a
# head with a close, the body, the CLI's first five models and the rest
# under More models, unfolded when the chosen model is there; the desktop's
# (no form) has none of it
def long() -> String:
  "{\"claude\":[\"default\",\"opus\",\"claude-fable-5-1\",\"sonnet\",\"haiku\",\"claude-sonnet-5\",\"claude-opus-5\",\"claude-opus-4-8\",\"claude-opus-4-7\"],\"about\":{\"claude\":{\"labels\":{\"opus\":\"Opus 5.5 · For complex work\"}}}}"

def picker(+form: String, +cur: String, +q: String) -> String:
  nodes(V.View.mp.loc(I.En{}, True{}, Map.set(&2, String, Map.set(&2, String, Map.set(&2, String, Map.new(&2, String), "models", long()), "models.oc", long()), "local.workspace.form", form),
    MPK.MpCur{1n, "claude", cur, "high"}, MPK.MpCur{1n, "claude", cur, "high"}, q, False{}))

def has(+hay: String, +needle: String) -> String:
  Bool.pick(String, String.contains(hay, needle), "yes", "no")

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("empty row list stays exact", exact(I.En{}, Nil{}, "", catalog(), "codex"), "same")
    C.Check.str("default and selected rows keep all attributes", exact(I.En{}, ["", "a", "b"], "a", catalog(), "codex"), "same")
    C.Check.str("Japanese titles/default/labels remain exact", exact(I.Ja{}, ["", "a", "b"], "", catalog(), "codex"), "same")
    C.Check.str("unknown provider remains blank", exact(I.En{}, ["", "a"], "a", catalog(), "missing"), "same")
    C.Check.str("other provider remains isolated", exact(I.En{}, ["a"], "a", catalog(), "claude"), "same")
    C.Check.str("malformed catalog stays exact", exact(I.En{}, ["", "a"], "", "not json", "codex"), "same")
    C.Check.str("numeric label stays blank", exact(I.En{}, ["wrong"], "wrong", catalog(), "codex"), "same")
    C.Check.str("missing label stays blank", exact(I.En{}, ["absent"], "", catalog(), "codex"), "same")
    C.Check.str("default id and empty id keep same lookup", exact(I.En{}, ["default", ""], "default", catalog(), "codex"), "same")
    C.Check.str("duplicates/order unchanged", exact(I.En{}, ["b", "a", "a", ""], "a", catalog(), "codex"), "same")
    C.Check.str("nonobject labels stay exact", exact(I.En{}, ["a"], "", "{\"about\":{\"codex\":{\"labels\":[\"bad\"]}}}", "codex"), "same")
    C.Check.str("fresh catalog replaces prior labels", U.Models.label.given(U.Models.labels("{\"about\":{\"codex\":{\"labels\":{\"a\":\"Fresh label\"}}}}", "codex"), "a"), "Fresh label")
    C.Check.str("whole current model menu exact", menu(I.En{}, catalog(), "codex", ""), "same")
    C.Check.str("whole searched menu exact", menu(I.En{}, catalog(), "codex", "日本語"), "same")
    C.Check.str("whole Japanese menu exact", menu(I.Ja{}, catalog(), "codex", ""), "same")
    C.Check.str("whole empty search result exact", menu(I.En{}, catalog(), "codex", "unmatched"), "same")
    C.Check.str("phone picker has a head with a close", has(picker("phone", "opus", ""), "mp-top"), "yes")
    C.Check.str("phone picker folds the later models", has(picker("phone", "opus", ""), "More models (4)"), "yes")
    C.Check.str("phone fold shut while the chosen model is above it", has(picker("phone", "opus", ""), "\"open\""), "no")
    C.Check.str("phone fold open while the chosen model is in it", has(picker("phone", "claude-opus-4-7", ""), "\"open\""), "yes")
    C.Check.str("phone search lists all it finds, unfolded", has(picker("phone", "opus", "opus"), "More models"), "no")
    C.Check.str("desktop picker has no phone head", has(picker("", "opus", ""), "mp-top"), "no")
    C.Check.str("desktop picker folds nothing", has(picker("", "opus", ""), "More models"), "no")
