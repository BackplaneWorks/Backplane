import Base
import ./check.bend as C
import ./fixtures/workspace-rail.bend as F
import ../src/core/client.bend as U
import ../src/core/json.bend as J
import ../src/server/thin-view.bend as S
import ../src/server/thin-actor.bend as TA
import ../src/web/view.bend as V
import ../src/web/app.bend as A
import ../src/web/presentation.bend as P

def layout(+kind: String, +bot: Bool) -> U.Ui:
  S.Session.renderer(True{}, F.start(bot, ""), J.Json.obj([J.KV{"workspaceLayout", J.Str{kind}}]))

def chrome(+kind: String, +top: String, +bot: Bool) -> U.Ui:
  S.Session.renderer(True{}, F.start(bot, ""), J.Json.obj([J.KV{"workspaceLayout", J.Str{kind}}, J.KV{"workspaceChrome", J.Str{top}}]))

def page(+u: U.Ui) -> String:
  J.Json.show(P.Node.write(V.View.page(u)))

# the rail's keys, each its own (a bare key presses exactly one button)
def keys.has(xs: List<&2, String>, +k: String) -> Bool:
  match xs:
    case Nil{}: False{}
    case x <> t: Bool.or(String.eq(x, k), keys.has(t, k))

def keys.distinct(ks: List<&2, String>) -> Bool:
  match ks:
    case Nil{}: True{}
    case +k <> +t: Bool.and(Bool.not(keys.has(t, k)), keys.distinct(t))

def has_wrapper(+u: U.Ui) -> String:
  Bool.show(String.contains(F.frame(False{}, False{}, u), "thread-content"))

def request(kind: String) -> J.Json:
  J.Json.obj([J.KV{"target", S.Target.json("thread", "t")}, J.KV{"workspaceLayout", J.Str{kind}},
    J.KV{"session", J.Json.obj([J.KV{"sel", J.Str{"t"}}, J.KV{"rid", J.Num{"100"}},
      J.KV{"drafts", J.Json.obj([J.KV{"t", J.Str{"Unsent fixture draft"}}])}, J.KV{"open", J.Json.obj(Nil{})}, J.KV{"info", J.Json.obj(Nil{})}])}])

def actor(xs: List<&2, TA.Actor>, kind: String, hint: String) -> TA.Actor:
  TA.Actor.refresh(xs, True{}, U.Ui.state(F.start(False{}, "")),
    J.Json.obj([J.KV{"local.workspace.layout", J.Str{hint}}]), J.Json.obj(Nil{}), 20n, request(kind), "view")

def rail(a: TA.Actor) -> String:
  Bool.show(V.View.rail(TA.Actor.ui(a)))

def main() -> IO(Unit):
  +tp = page(chrome("rail-v1", "top-v1", False{}))
  +tf = F.frame(False{}, False{}, chrome("rail-v1", "top-v1", False{}))
  do IO<Unit>:
    C.Check.str("absent capability selects flat DOM", has_wrapper(S.Session.renderer(True{}, F.start(False{}, ""), J.Json.obj(Nil{}))), "False")
    C.Check.str("unknown capability selects flat DOM", has_wrapper(layout("rail-v2", False{})), "False")
    C.Check.str("exact capability selects rail DOM", has_wrapper(layout("rail-v1", False{})), "True")
    C.Check.str("action stamp preserves legacy renderer", Bool.show(V.View.rail(A.stamp(layout("legacy", False{})))), "False")
    C.Check.str("local fresh renderer uses rail", Bool.show(V.View.rail(A.stamp(U.Ui.init(1n)))), "True")
    C.Check.str("legacy keeps draft", U.Ui.scratch(layout("legacy", False{}), "t"), "Unsent fixture draft")
    C.Check.str("rail keeps draft", U.Ui.scratch(layout("rail-v1", False{}), "t"), "Unsent fixture draft")
    C.Check.str("legacy bot has no rail wrapper", Bool.show(String.contains(F.frame(False{}, True{}, layout("legacy", True{})), "thread-content")), "False")
    C.Check.str("legacy bot has no new page menu", Bool.show(String.contains(F.frame(False{}, True{}, layout("legacy", True{})), "bot-mobile-menu")), "False")
    C.Check.str("non-string capability is legacy", Bool.show(V.View.rail(S.Session.renderer(True{}, F.start(False{}, ""), J.Json.obj([J.KV{"workspaceLayout", J.Num{"1"}}])))), "False")
    C.Check.str("reconnect removes old rail hint without opt-in", Bool.show(V.View.rail(S.Session.renderer(True{}, layout("rail-v1", False{}), J.Json.obj(Nil{})))), "False")
    C.Check.str("new capability supersedes stale legacy hint", Bool.show(V.View.rail(S.Session.renderer(True{}, layout("legacy", False{}), request("rail-v1")))), "True")
    C.Check.str("navigation preserves legacy session", Bool.show(V.View.rail(F.step(layout("legacy", False{}), "view-kind", "board"))), "False")
    C.Check.str("new actor absent opt-in ignores global rail hint", rail(actor(Nil{}, "", "rail-v1")), "False")
    C.Check.str("legacy actor info refresh stays legacy", rail(actor([actor(Nil{}, "", "")], "", "rail-v1")), "False")
    C.Check.str("rail actor info refresh stays rail", rail(actor([actor(Nil{}, "rail-v1", "")], "rail-v1", "legacy")), "True")
    C.Check.str("actor rail to legacy restamp on reconnect", rail(actor([actor(Nil{}, "rail-v1", "")], "", "rail-v1")), "False")
    C.Check.str("fresh legacy actor cannot inherit another session rail", rail(actor(Nil{}, "", "rail-v1")), "False")
    C.Check.str("absent top capability keeps the thread's header", Bool.show(String.contains(F.frame(False{}, False{}, layout("rail-v1", False{})), "thread-head")), "True")
    C.Check.str("absent top capability draws no top bar", Bool.show(String.contains(page(layout("rail-v1", False{})), "topbar")), "False")
    C.Check.str("top capability selects the top bar", Bool.show(V.View.chrome(chrome("rail-v1", "top-v1", False{}))), "True")
    C.Check.str("top bar names the thread and its state", Bool.show(Bool.and(String.contains(tp, "\"topbar\""), Bool.and(String.contains(tp, "Navigation fixture"), String.contains(tp, "kv state")))), "True")
    C.Check.str("top bar replaces the thread's header", Bool.show(String.contains(tf, "thread-head")), "False")
    C.Check.str("top bar replaces the sidebar's brand", Bool.show(String.contains(tp, "\"brand\"")), "False")
    C.Check.str("top bar keeps a bot's own header", Bool.show(String.contains(F.frame(False{}, True{}, chrome("rail-v1", "top-v1", True{})), "bot-head")), "True")
    C.Check.str("unknown top capability is off", Bool.show(V.View.chrome(chrome("rail-v1", "top-v2", False{}))), "False")
    C.Check.str("top bar needs the rail", Bool.show(V.View.chrome(chrome("legacy", "top-v1", False{}))), "False")
    C.Check.str("local fresh renderer uses the top bar", Bool.show(V.View.chrome(A.stamp(U.Ui.init(1n)))), "True")
    C.Check.str("reconnect without opt-in drops the top bar", Bool.show(V.View.chrome(S.Session.renderer(True{}, chrome("rail-v1", "top-v1", False{}), request("rail-v1")))), "False")
    C.Check.str("rail views carry their keycaps", Bool.show(Bool.and(String.contains(tf, "Board · B"), String.contains(tf, "3D · 3"))), "True")
    C.Check.str("rail keys are each their own", Bool.show(keys.distinct([V.View.workspace.key("terminal"), V.View.workspace.key("commands"), V.View.workspace.key("board"), V.View.workspace.key("schematic"), V.View.workspace.key("3d"), V.View.workspace.key("split"),
      V.View.workspace.key("mech"), V.View.workspace.key("files"), V.View.workspace.key("diff"), V.View.workspace.key("review"), V.View.workspace.key("browser")])), "True")
    C.Check.str("top bar sidebar ends with the keys' legend", Bool.show(String.contains(tp, "keys-legend")), "True")
    C.Check.str("actor refresh retains caller draft", U.Ui.scratch(TA.Actor.ui(actor([actor(Nil{}, "rail-v1", "")], "", "rail-v1")), "t"), "Unsent fixture draft")
