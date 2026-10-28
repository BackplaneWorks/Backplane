import Base
import ./check.bend as C
import ../src/core/json.bend as J
import ../src/core/model.bend as M
import ../src/core/client.bend as U
import ../src/server/thin-actor.bend as A
import ../src/server/thin-view.bend as V

# An action that moves the hub-drawn page to another thread and asks the hub
# at once (a project's Intent…, Fab.sync after a move) has its answer
# applied in the thread it opened: the request is owed to the view the
# action left (thin-actor.bend's Actor.accept/Actor.owed/Actor.reply), not
# the one it was clicked in. Anything owed elsewhere is still dropped.
def obj(+text: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{})

def state() -> M.State:
  M.State.replay([M.ProjectCreated{"p", "Lamp", "/lamp", 1n},
    M.ThreadCreated{"a", "p", "Lamp thread", M.Local{}, "", "/lamp", "codex", "", 1n},
    M.ThreadCreated{"c", "p", "Other thread", M.Local{}, "", "/lamp", "codex", "", 1n},
    M.ProjectCreated{"q", "Amp", "/amp", 1n},
    M.ThreadCreated{"b", "q", "Amp thread", M.Local{}, "", "/amp", "codex", "", 1n}], M.State.empty())

def target(+id: String) -> J.Json:
  V.Target.json("thread", id)

# the page on thread a, with a request "7" it asked there earlier
def actor() -> A.Actor:
  A.Actor{U.Ui.select.k(U.Ui.adopt(U.Ui.init(100n), state()), "a"), "view-a", target("a"), Map.set(&2, String, Map.new(&2, String), "7", A.Actor.scope(target("a"))),
    Nil{}, False{}, Nil{}}

def act(+action: String, +value: String) -> V.ActResult:
  V.Action.run(True{}, A.Actor.ui(actor()), J.Json.obj([J.KV{"action", J.Str{action}}, J.KV{"value", J.Str{value}}]), 100n)

# the id of the first request an action sends
def asked.cs(cs: List<&2, J.Json>) -> String:
  match cs:
    case Nil{}: ""
    case +c <> rest: Bool.pick(String, String.eq(J.Json.get_str(c, "type", ""), "send"),
      J.Json.as_raw(J.Json.get(A.Actor.rpc(c), "id"), ""), asked.cs(rest))

def asked(r: V.ActResult) -> String:
  match r:
    case V.ActResult{_, cs}: asked.cs(cs)

# Intent… on project q: thread b opens, and its intent is asked for
def moved() -> A.Actor:
  A.Actor.accept(actor(), act("proj-intent", "q"), False{})

def owed(+a: A.Actor, +id: String) -> String:
  match a:
    case A.Actor{_, _, _, p, _, _, _}: A.Actor.pending_scope(p, id)

# the page opening view v on thread id (view.open)
def opened(+a: A.Actor, +id: String, +v: String) -> A.Actor:
  A.Actor.refresh([a], True{}, state(), obj("{}"), obj("{}"), 100n,
    J.Json.obj([J.KV{"target", target(id)}, J.KV{"cid", J.Str{"device"}}, J.KV{"session", obj("{\"info\":{},\"drafts\":{},\"outbox\":{}}")}]), v)

# the hub's answer to fab.look for thread th
def answer(+id: String, +th: String) -> J.Json:
  J.Json.obj([J.KV{"t", J.Str{"reply"}}, J.KV{"id", J.Num{id}}, J.KV{"ok", J.Flag{True{}}},
    J.KV{"result", J.Json.obj([J.KV{"thread", J.Str{th}}, J.KV{"goal", J.Str{"A small amplifier"}}, J.KV{"intent", J.Str{"# Goal\nA small amplifier"}}])}])

def answered(+a: A.Actor, +id: String, +th: String) -> A.Actor:
  A.Actor.message(a, True{}, "hub", answer(id, th))

def check(+name: String, +yes: Bool) -> IO(Unit):
  C.Check.str(name, Bool.pick(String, yes, "yes", "no"), "yes")

def main() -> IO(Unit):
  do IO<Unit>:
    check("Intent… on another project moves the view to its thread", String.eq(J.Json.show(V.Target.after(A.Actor.ui(moved()))), J.Json.show(target("b"))))
    check("and asks the hub for its intent", Bool.not(String.is_empty(asked(act("proj-intent", "q")))))
    C.Check.str("the request is owed to the thread it opened", owed(moved(), asked(act("proj-intent", "q"))), A.Actor.scope(target("b")))
    C.Check.str("still owed once the page has opened it", owed(opened(moved(), "b", "view-b"), asked(act("proj-intent", "q"))), A.Actor.scope(target("b")))
    C.Check.str("the answer after the page opened it is applied", U.Ui.info(A.Actor.ui(answered(opened(moved(), "b", "view-b"), asked(act("proj-intent", "q")), "b")), "fab.line:b"), "A small amplifier")
    check("and the page is told to look again", A.Actor.dirty(answered(A.Actor.clean(opened(moved(), "b", "view-b")), asked(act("proj-intent", "q")), "b")))
    check("the intent shows over it", U.Intent.on(A.Actor.ui(answered(opened(moved(), "b", "view-b"), asked(act("proj-intent", "q")), "b"))))
    C.Check.str("the intent's text", U.Intent.text(A.Actor.ui(answered(opened(moved(), "b", "view-b"), asked(act("proj-intent", "q")), "b"))), "# Goal\nA small amplifier")
    C.Check.str("an answer before the page opened it is applied", U.Ui.info(A.Actor.ui(answered(moved(), asked(act("proj-intent", "q")), "b")), "fab.line:b"), "A small amplifier")
    C.Check.str("and kept when the page opens it", U.Ui.info(A.Actor.ui(opened(answered(moved(), asked(act("proj-intent", "q")), "b"), "b", "view-b")), "fab.line:b"), "A small amplifier")
    C.Check.str("answered once: no longer owed", owed(answered(opened(moved(), "b", "view-b"), asked(act("proj-intent", "q")), "b"), asked(act("proj-intent", "q"))), "")
    C.Check.str("a request asked before the move is not owed to the new view", owed(opened(moved(), "b", "view-b"), "7"), "")
    C.Check.str("the page going elsewhere drops what was owed to the thread opened", owed(opened(opened(moved(), "b", "view-b"), "c", "view-c"), asked(act("proj-intent", "q"))), "")
    C.Check.str("so its answer then is not applied", U.Ui.info(A.Actor.ui(answered(opened(opened(moved(), "b", "view-b"), "c", "view-c"), asked(act("proj-intent", "q")), "b")), "fab.line:b"), "")
    check("nor does it ask for a look", Bool.not(A.Actor.dirty(answered(A.Actor.clean(opened(opened(moved(), "b", "view-b"), "c", "view-c")), asked(act("proj-intent", "q")), "b"))))
    C.Check.str("Intent… on the page's own project: owed to the same view", owed(A.Actor.accept(actor(), act("proj-intent", "p"), False{}), asked(act("proj-intent", "p"))), A.Actor.scope(target("a")))
    C.Check.str("an earlier request on the same view stays owed", owed(A.Actor.accept(actor(), act("proj-intent", "p"), False{}), "7"), A.Actor.scope(target("a")))
    C.Check.str("an unknown answer changes nothing", U.Ui.info(A.Actor.ui(answered(moved(), "99", "b")), "fab.line:b"), "")
