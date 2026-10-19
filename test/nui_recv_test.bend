import Base
import ./check.bend as C
import ../src/core/client.bend as U
import ../src/core/model.bend as M
import ../src/core/json.bend as J
import ../src/app/nui.bend as N

# The native window receives through the client's own receive
# (U.Ui.recv_act), as the web and the phones do: a desktop notification
# becomes a notify command, a reconnect's log sends the outbox again (and
# an answered request is not sent again), and the hub's own window, which
# adopts the read model without a socket, sends it too.

def json(s: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(s), J.Null{})

def cmds(s: N.Step) -> List<&2, N.NCmd>:
  match s:
    case N.Step{_, cs, _}:
      cs

def nui(s: N.Step) -> N.Nui:
  match s:
    case N.Step{n, _, _}:
      n

def sends(cs: List<&2, N.NCmd>) -> Nat:
  match cs:
    case Nil{}:
      0n
    case N.NSend{_} <> t:
      1n+sends(t)
    case _ <> t:
      sends(t)

def notifies(cs: List<&2, N.NCmd>) -> String:
  match cs:
    case Nil{}:
      ""
    case N.NNotify{th, ti, _} <> _:
      th ++ "|" ++ ti
    case _ <> t:
      notifies(t)

def log() -> J.Json:
  json("{\"t\":\"log\",\"since\":0,\"origin\":\"O\",\"info\":{},\"items\":[]}")

# a client with one request waiting for its answer (id 0)
def waiting() -> N.Nui:
  N.Nui.init(U.Act.ui(U.Ui.rpc.keep(U.Ui.init(0n), "turn.start", [J.KV{"thread", J.Str{"t1"}}, J.KV{"text", J.Str{"hi"}}, J.KV{"msg", J.Str{"c1"}}])), 800, 600)

def at(u: U.Ui, n: Nat) -> U.Ui:
  match u:
    case U.Ui{st, sel, dr, live, op, on, now, ad, pa, v, _, w, er, note, nf, ob, tr}:
      U.Ui{st, sel, dr, live, op, on, now, ad, pa, v, n, w, er, note, nf, ob, tr}

def keep(u: U.Ui, text: String, msg: String) -> U.Ui:
  U.Act.ui(U.Ui.rpc.keep(u, "turn.start", [J.KV{"thread", J.Str{"t1"}}, J.KV{"text", J.Str{text}}, J.KV{"msg", J.Str{msg}}]))

def ordered_waiting() -> N.Nui:
  N.Nui.init(keep(keep(at(U.Ui.init(0n), 9n), "first", "c9"), "second", "c10"), 800, 600)

def ids(cs: List<&2, N.NCmd>) -> String:
  match cs:
    case Nil{}: ""
    case N.NSend{x} <> t:
      J.Json.as_raw(J.Json.get(json(x), "id"), "") ++ "," ++ ids(t)
    case _ <> t: ids(t)

def messages(cs: List<&2, N.NCmd>) -> String:
  match cs:
    case Nil{}: ""
    case N.NSend{x} <> t:
      J.Json.get_str(J.Json.get_obj(json(x), "p"), "msg", "") ++ "," ++ messages(t)
    case _ <> t: messages(t)

def wire(cs: List<&2, N.NCmd>) -> String:
  match cs:
    case Nil{}: ""
    case N.NSend{x} <> t: x ++ "\n" ++ wire(t)
    case _ <> t: wire(t)

def main() -> IO(Unit):
  +note = N.Nui.hear(N.Nui.init(U.Ui.init(0n), 800, 600), json("{\"t\":\"notify\",\"thread\":\"t1\",\"title\":\"Board\",\"body\":\"Finished\",\"key\":\"k\"}"))
  +joined = N.Nui.hear(waiting(), log())
  +answered = N.Nui.hear(nui(joined), json("{\"t\":\"reply\",\"id\":0,\"ok\":true}"))
  +again = N.Nui.hear(nui(answered), log())
  +adopted = N.Nui.adopt.step(waiting(), M.State.empty(), log())
  +ordered = N.Nui.hear(ordered_waiting(), log())
  +repeated = N.Nui.hear(nui(ordered), log())
  +one_answered = N.Nui.hear(nui(repeated), json("{\"t\":\"reply\",\"id\":9,\"ok\":true}"))
  +remaining = N.Nui.hear(nui(one_answered), log())
  +ordered_adopted = N.Nui.adopt.step(ordered_waiting(), M.State.empty(), log())
  do IO<Unit>:
    C.Check.str("a notify message notifies", notifies(cmds(note)), "t1|Board")
    C.Check.str("a reconnect's log sends the outbox again", Nat.show(sends(cmds(joined))), "1")
    C.Check.str("an answered request is not sent again", Nat.show(sends(cmds(again))), "0")
    C.Check.str("the hub's own window sends the outbox on adopting", Nat.show(sends(cmds(adopted))), "1")
    C.Check.str("a changes message sends nothing", Nat.show(sends(cmds(N.Nui.hear(waiting(), json("{\"t\":\"changes\",\"items\":[]}"))))), "0")
    C.Check.str("native reconnect resends 9 before 10", ids(cmds(ordered)), "9,10,")
    C.Check.str("native repeated reconnect keeps request order", ids(cmds(repeated)), "9,10,")
    C.Check.str("native repeated reconnect keeps exact request bytes", wire(cmds(repeated)), wire(cmds(ordered)))
    C.Check.str("native reconnect retains message IDs", messages(cmds(ordered)), "c9,c10,")
    C.Check.str("native reply itself does not resend", ids(cmds(one_answered)), "")
    C.Check.str("native reconnect excludes the answered request", ids(cmds(remaining)), "10,")
    C.Check.str("native adoption resends 9 before 10", ids(cmds(ordered_adopted)), "9,10,")
    C.Check.str("native adoption preserves the same request bytes", wire(cmds(ordered_adopted)), wire(cmds(ordered)))
    C.Check.str("native adoption retains message IDs", messages(cmds(ordered_adopted)), "c9,c10,")
