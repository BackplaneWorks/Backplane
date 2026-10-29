import Base
import ./check.bend as C
import ../src/server/thin-actor.bend as TA
import ../src/server/thin-view.bend as V
import ../src/core/client.bend as U
import ../src/core/model.bend as M
import ../src/core/json.bend as J

# Mech on the hub-drawn page (thin-actor.bend): a thread is chosen by
# moving the page, not by an action, so a thread whose viewer is on Mech,
# chosen again, never had its parts listed again. The actor drops the last
# listing's mark when the page moves ("@mech.at", Actor.scope.keep) and,
# before each render, asks for the parts of a Mech on screen once
# (Actor.mail_ready, client.bend's Mpage.sync.m), a read it sends itself
# (Actor.read_only).

def obj(text: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{})

def state() -> M.State:
  M.State.replay([M.ProjectCreated{"p", "Board", "/p", 1n}, M.ThreadCreated{"a", "p", "Here", M.Local{}, "", "", "claude", "", 2n},
    M.ThreadCreated{"b", "p", "There", M.Local{}, "", "", "claude", "", 3n}], M.State.empty())

def request(+th: String) -> J.Json:
  J.Json.obj([J.KV{"hub", J.Str{"hub"}}, J.KV{"cid", J.Str{"device"}}, J.KV{"target", V.Target.json("thread", th)},
    J.KV{"session", J.Json.obj([J.KV{"sel", J.Str{th}}, J.KV{"rid", J.Json.nat(100n)}, J.KV{"drafts", obj("{}")}, J.KV{"info", obj("{}")}, J.KV{"outbox", obj("{}")}])}])

def to.view(a: TA.Actor, +th: String, +view: String) -> TA.Actor:
  TA.Actor.refresh([a], True{}, state(), obj("{}"), obj("{}"), 100n, request(th), view)

def to(a: TA.Actor, +th: String) -> TA.Actor:
  to.view(a, th, "view")

def actor() -> TA.Actor:
  TA.Actor.refresh(Nil{}, True{}, state(), obj("{}"), obj("{}"), 100n, request("a"), "view")

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
    J.KV{"result", obj("{\"thread\":\"a\",\"root\":\"/p\",\"mech\":\"1 9 mech/a.step\"}")}])

# thread a on Mech, its parts listed, nothing left to send
def onmech() -> TA.Actor:
  +a = actor()
  +opened = action(a, "mech", "")
  +asked = J.Json.as_raw(J.Json.get(find(commands(opened), "mech.list"), "id"), "")
  TA.Actor.clean(TA.Actor.reply(TA.Actor.accept(a, opened, False{}), True{}, "hub", listing(asked)))

def reads(+a: TA.Actor) -> String:
  methods(TA.Actor.queued(TA.Actor.mail_ready(a)))

def main() -> IO(Unit):
  +a = onmech()
  do IO<Unit>:
    C.Check.str("on Mech, its part in 3D", U.Ui.view_get(TA.Actor.ui(a), "view.kind") ++ "|" ++ U.Ui.view_get(TA.Actor.ui(a), "view.path"), "3d|/p/mech/a.step")
    C.Check.str("a render that changed nothing asks for nothing", reads(a), "")
    C.Check.str("another thread, not on Mech: nothing", reads(TA.Actor.clean(TA.Actor.mail_ready(to(a, "b")))), "")
    C.Check.str("back to the thread on Mech: its parts are listed again", reads(to(TA.Actor.clean(TA.Actor.mail_ready(to(a, "b"))), "a")), "mech.list|")
    C.Check.str("once", reads(TA.Actor.clean(TA.Actor.mail_ready(to(TA.Actor.clean(TA.Actor.mail_ready(to(a, "b"))), "a")))), "")
    C.Check.str("the same page drawn again: nothing", reads(to(a, "a")), "")
    C.Check.str("a page loaded afresh on Mech lists them", reads(to.view(a, "a", "view2")), "mech.list|")
    C.Check.str("the read is the actor's own", Bool.show(TA.Actor.read_only(J.Json.obj([J.KV{"type", J.Str{"send"}}, J.KV{"text", J.Str{"{\"id\":5,\"m\":\"mech.list\",\"p\":{\"thread\":\"a\"}}"}}]))), "True")
