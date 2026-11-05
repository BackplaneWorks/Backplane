import Base
import ./check.bend as C
import ../src/core/json.bend as J
import ../src/core/model.bend as M
import ../src/core/client.bend as U
import ../src/core/usage.bend as Z
import ../src/server/thin-actor.bend as A
import ../src/server/thin-view.bend as V
import ../src/core/thin-wire.bend as TW

# Synthetic data only. A closed ledger larger than the actual rejected
# frame must not cross the UI boundary or change any displayed usage row.
def obj(s: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(s), J.Null{})

def job(state: String, +extra: String) -> J.Json:
  J.Json.obj([J.KV{"state", J.Str{state}}, J.KV{"size", J.Str{"small"}}, J.KV{"thread", J.Str{"t"}}, J.KV{"machine", J.Str{"fixture"}},
    J.KV{"why", J.Str{"needs review"}}, J.KV{"runtime_s", J.Num{"3"}}, J.KV{"started_at", J.Num{"10"}}, J.KV{"reserved_cs", J.Null{}}, J.KV{"command", J.Str{extra}}])

def open_jobs(n: Nat) -> List<&2, J.Json>:
  match n:
    case 0n: Nil{}
    case 1n+k: job("running", "PRIVATE open command") <> open_jobs(k)

def status() -> J.Json:
  J.Json.obj([J.KV{"running", J.Num{"8"}}, J.KV{"ambiguous", J.Num{"2"}}, J.KV{"unresolved", J.Num{"1"}}, J.KV{"orphans", J.Num{"1"}},
    J.KV{"today", obj("{\"cost_cs\":100,\"reserved_open_cs\":null}")}, J.KV{"month", obj("{\"cost_cs\":300}")}, J.KV{"budget", obj("{\"cs\":1000,\"source\":\"fixture\"}")},
    J.KV{"jobs", J.Json.arr(List.append(&2, J.Json, open_jobs(8n), [job("ended", String.repeat("PRIVATE closed ledger ", 60000n)),
      job("orphan", "PRIVATE warning command"), job("ambiguous", "PRIVATE warning command"), job("unresolved", "PRIVATE warning command")]))}])

def rows(rs: List<&2, Z.URow>) -> String:
  match rs:
    case Nil{}: ""
    case +r <> tail: Z.URow.label(r) ++ "|" ++ Z.URow.note(r) ++ "|" ++ Z.URow.act(r) ++ "\n" ++ rows(tail)

def state() -> M.State:
  M.State.replay([M.ThreadCreated{"t", "", "Fixture", M.Local{}, "", "", "codex", "", 1n}], M.State.empty())

def message(text: String) -> J.Json:
  J.Json.obj([J.KV{"t", J.Str{"info"}}, J.KV{"info", J.Json.obj([J.KV{"boat.usage", J.Str{text}}])}])

def target() -> J.Json:
  V.Target.json("thread", "t")

def actor() -> A.Actor:
  A.Actor{U.Ui.reselect("t", U.Ui.adopt(U.Ui.init(100n), state())), "view", target(), Map.set(&2, String, Map.new(&2, String), "9", "untouched scope"),
    [obj("{\"type\":\"send\",\"text\":\"synthetic queued command\"}")], False{}, Nil{}}

def received(+text: String) -> A.Actor:
  A.Actor.message(actor(), True{}, "fixture-hub", message(text))

def pending(a: A.Actor) -> String:
  match a:
    case A.Actor{_, _, _, p, _, _, _}: A.Actor.pending_scope(p, "9")

def request() -> J.Json:
  J.Json.obj([J.KV{"target", target()}, J.KV{"cid", J.Str{"fixture-device"}}, J.KV{"session", obj("{\"info\":{\"boat.usage\":\"CLIENT SPOOF\"},\"drafts\":{},\"outbox\":{}}") }])

def refreshed(+text: String) -> A.Actor:
  A.Actor.refresh([received(text)], True{}, state(), obj("{}"), obj("{}"), 101n, request(), "view")

def check(name: String, yes: Bool) -> IO(Unit):
  C.Check.str(name, Bool.pick(String, yes, "yes", "no"), "yes")

def main.source(+source: J.Json) -> IO(Unit):
  +text = Z.Bj.info("{}", J.Json.show(source))
  +projection = J.Json.get_obj(Z.Bj.parse(text), "status")
  do IO<Unit>:
    check("fixture closed ledger exceeds rejected-frame size", Nat.is_gt(J.Json.bytes(source), 1205527n))
    check("projected usage info fits well below the unchanged frame cap", Nat.is_lt(J.Json.bytes(message(text)), 8192n))
    C.Check.str("all displayed totals, warnings, runtime, unknown amounts and capped-open count unchanged",
      rows(Z.Bj.rows(state(), text, 100n)), rows(Z.Bj.rows.k(state(), obj("{}"), source, 100n)))
    C.Check.str("all eight open and all three warning jobs retained", Nat.show(List.length(&2, J.Json, J.Json.elems(J.Json.get_obj(projection, "jobs")))), "11")
    check("closed ledger and unused command bodies absent", Bool.not(String.contains(text, "PRIVATE")))
    check("thin info is folded rather than sent on raw wire", Bool.not(A.Message.on_wire(message(text))))
    check("ordinary correlated replies still pass", A.Message.on_wire(obj("{\"t\":\"reply\",\"id\":9,\"ok\":true}")))
    check("heartbeat routing remains available", A.Message.on_wire(obj("{\"t\":\"hb\"}")))
    C.Check.str("targeted info reaches private actor Ui", U.Ui.info(A.Actor.ui(received(text)), "boat.usage"), text)
    check("private info dirties bounded view", A.Actor.dirty(received(text)))
    C.Check.str("info does not settle pending request", pending(received(text)), "untouched scope")
    C.Check.str("info preserves queued commands without adding one", Nat.show(List.length(&2, J.Json, A.Actor.queued(received(text)))), "1")
    check("private usage cache is absent from serialized presentation session", Bool.not(String.contains(J.Json.show(V.Route.pack(A.Actor.ui(received(text)))), "boat.usage")))
    C.Check.str("refresh retains private info and rejects client spoof", U.Ui.info(A.Actor.ui(refreshed(text)), "boat.usage"), text)

# The hub's "desk" info (which threads the desks show) is drawn by no view:
# folding it must not mark the view dirty (each desk.look re-tells it to
# every client), but any other key with it still does.
def desk(+keys: List<&2, J.KV>) -> A.Actor:
  A.Actor.message(actor(), True{}, "fixture-hub", J.Json.obj([J.KV{"t", J.Str{"info"}}, J.KV{"info", J.Json.obj(keys)}]))

# An owned reply that changes nothing the view draws (asked again and
# answered the same) leaves it clean; one that changes the error line, a
# kept answer or the selection marks it dirty.
def replied(+a: A.Actor, +o: String) -> A.Actor:
  A.Actor.reply_if(True{}, a, True{}, "fixture-hub", obj(o))

def secrets() -> String:
  "{\"t\":\"reply\",\"id\":5,\"ok\":true,\"secrets\":\"[]\",\"vault\":\"{}\"}"

def fab_count(xs: List<&2, J.Json>, n: Nat) -> Nat:
  match xs:
    case Nil{}: n
    case +x <> rest: fab_count(rest, Nat.add(n, Bool.pick(Nat, String.contains(J.Json.get_str(x, "text", ""), "fab.look"), 1n, 0n)))

def project_actor() -> A.Actor:
  A.Actor{U.Ui.select.k(U.Ui.adopt(U.Ui.init(100n), M.State.replay([M.ProjectCreated{"p", "Project", "/fixture", 1n},
    M.ThreadCreated{"t", "p", "Fixture", M.Local{}, "", "", "codex", "", 1n}], M.State.empty())), "t"), "view", target(), Map.new(&2, String), Nil{}, False{}, Nil{}}

def fab_reads(+a: A.Actor) -> Nat:
  fab_count(A.Actor.queued(a), 0n)

def main.quiet() -> IO(Unit):
  +once = A.Actor.clean(replied(actor(), secrets()))
  do IO<Unit>:
    C.Check.str("desk info is kept in the Ui", U.Ui.info(A.Actor.ui(desk([J.KV{"desk", J.Str{"t"}}])), "desk"), "t")
    check("desk info alone leaves the view clean", Bool.not(A.Actor.dirty(desk([J.KV{"desk", J.Str{"t"}}]))))
    check("desk info with another key still dirties", A.Actor.dirty(desk([J.KV{"desk", J.Str{"t"}}, J.KV{"boat.usage", J.Str{"x"}}])))
    check("empty info dirties as before", A.Actor.dirty(desk(Nil{})))
    check("a bare ok reply (its empty request slots cleared) changes nothing drawn", Bool.not(A.Actor.dirty(replied(once, "{\"t\":\"reply\",\"id\":8,\"ok\":true}"))))
    check("an error reply dirties", A.Actor.dirty(replied(actor(), "{\"t\":\"reply\",\"id\":5,\"ok\":false,\"error\":\"boom\"}")))
    check("a first secret list dirties", A.Actor.dirty(replied(actor(), secrets())))
    C.Check.str("the secret list is kept", U.Ui.scratch(A.Actor.ui(once), "@secrets"), "[]")
    check("the same secret list again leaves the view clean", Bool.not(A.Actor.dirty(replied(once, secrets()))))
    check("a web view's greeting says the hub asks the card itself", J.Json.get_bool(TW.Wire.greeting("web", "1", "e"), "fabSelf", False{}))
    check("a phone's greeting does not", Bool.not(J.Json.get_bool(TW.Wire.greeting("mobile", "1", "e"), "fabSelf", False{})))
    C.Check.str("a shown thread's render asks for its order card itself", Nat.show(fab_reads(A.Actor.mail_ready(project_actor()))), "1")
    C.Check.str("and only once while the thread stays as it was", Nat.show(fab_reads(A.Actor.mail_ready(A.Actor.clean(A.Actor.mail_ready(project_actor()))))), "0")
    check("a changed secret list dirties", A.Actor.dirty(replied(once, "{\"t\":\"reply\",\"id\":6,\"ok\":true,\"secrets\":\"[1]\",\"vault\":\"{}\"}")))

def main() -> IO(Unit):
  do IO<Unit>:
    main.source(status())
    main.quiet()
