import Base
import ./check.bend as C
import ../src/server/thin-actor.bend as TA
import ../src/server/thin-view.bend as V
import ../src/core/client.bend as U
import ../src/core/model.bend as M
import ../src/core/json.bend as J

# Mech on the hub-drawn page (thin-actor.bend's Actor.watch): opened
# before its parts are known, it shows the first part in 3D when mech.list
# answers, and since a reply gives the page no command to send, the actor
# asks for the model itself (kicad.watch, run as a followup on the same
# socket and view). Without it the model was never read: Mech said
# "Preparing the model on the hub" for ever, and a model too big to show
# never said so.

def obj(text: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{})

def state() -> M.State:
  M.State.replay([M.ProjectCreated{"p", "Board", "/p", 1n}, M.ThreadCreated{"a", "p", "Here", M.Local{}, "", "", "claude", "", 2n}], M.State.empty())

def target() -> J.Json:
  V.Target.json("thread", "a")

def request() -> J.Json:
  J.Json.obj([J.KV{"hub", J.Str{"hub"}}, J.KV{"cid", J.Str{"device"}}, J.KV{"target", target()},
    J.KV{"session", J.Json.obj([J.KV{"sel", J.Str{"a"}}, J.KV{"rid", J.Json.nat(100n)}, J.KV{"drafts", obj("{}")}, J.KV{"info", obj("{}")}, J.KV{"outbox", obj("{}")}])}])

def actor() -> TA.Actor:
  TA.Actor.refresh(Nil{}, True{}, state(), obj("{}"), obj("{}"), 100n, request(), "view")

def action(+a: TA.Actor, +name: String, +value: String) -> V.ActResult:
  V.Action.run(True{}, TA.Actor.ui(a), J.Json.obj([J.KV{"action", J.Str{name}}, J.KV{"value", J.Str{value}}]), 100n)

def commands(r: V.ActResult) -> List<&2, J.Json>:
  match r:
    case V.ActResult{_, cs}: cs

def find(cs: List<&2, J.Json>, +method: String) -> J.Json:
  match cs:
    case Nil{}: J.Null{}
    case +c <> tail: Bool.pick(J.Json, String.eq(J.Json.get_str(TA.Actor.rpc(c), "m", ""), method), TA.Actor.rpc(c), find(tail, method))

def methods(cs: List<&2, J.Json>) -> String:
  match cs:
    case Nil{}: ""
    case +c <> tail: J.Json.get_str(TA.Actor.rpc(c), "m", "") ++ "|" ++ methods(tail)

def listing(+id: String) -> J.Json:
  J.Json.obj([J.KV{"t", J.Str{"reply"}}, J.KV{"id", J.Num{id}}, J.KV{"ok", J.Flag{True{}}},
    J.KV{"result", obj("{\"thread\":\"a\",\"root\":\"/p\",\"mech\":\"123 13631488 mech/huge.glb\"}")}])

def main() -> IO(Unit):
  +a = actor()
  +opened = action(a, "mech", "")
  +asked = J.Json.as_raw(J.Json.get(find(commands(opened), "mech.list"), "id"), "")
  +a1 = TA.Actor.accept(a, opened, False{})
  +a2 = TA.Actor.reply(a1, True{}, "hub", listing(asked))
  +watch = find(TA.Actor.queued(a2), "kicad.watch")
  do IO<Unit>:
    C.Check.str("Mech asks for the parts", Bool.show(String.is_empty(asked)), "False")
    C.Check.str("the parts known, the first shows in 3D", U.Ui.view_get(TA.Actor.ui(a2), "view.kind") ++ "|" ++ U.Ui.view_get(TA.Actor.ui(a2), "view.path"), "3d|/p/mech/huge.glb")
    C.Check.str("the actor asks for its model", J.Json.get_str(J.Json.get_obj(watch, "p"), "kind", "") ++ "|" ++ J.Json.get_str(J.Json.get_obj(watch, "p"), "path", ""), "3d|/p/mech/huge.glb")
    C.Check.str("a watch is a followup", Bool.show(TA.Actor.watch(J.Json.obj([J.KV{"type", J.Str{"send"}}, J.KV{"text", J.Str{J.Json.show(watch)}}]))), "True")
    C.Check.str("nothing else rides along", methods(TA.Actor.queued(a2)), "kicad.watch|")
