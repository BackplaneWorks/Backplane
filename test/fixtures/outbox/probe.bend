import Base
import ../../../src/core/client.bend as U
import ../../../src/core/model.bend as M
import ../../../src/core/json.bend as J

# The emitted JS uses the same client functions as the web and phones.
# Only the starting request counter is set directly, to exercise digit
# boundaries without manufacturing unrelated actions.
def counter(u: U.Ui, rid: Nat) -> U.Ui:
  match u:
    case U.Ui{st, sel, dr, live, op, on, now, ad, pa, v, _, w, er, note, nf, ob, tr}:
      U.Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}

def json(s: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(s), J.Null{})

def seed(+start: Nat, +thread: String) -> U.Ui:
  counter(U.Ui.select.k(U.Ui.set_cid(U.Ui.fold([
    M.ProjectCreated{"p-outbox", "Example", "/tmp/outbox-example", 1n},
    M.ThreadCreated{thread, "p-outbox", "Offline messages", M.Local{}, "", "", "claude", "", 2n}], U.Ui.init(10n)), "outbox-e2e"), thread), start)

def first(+start: Nat, +thread: String) -> U.Act:
  U.Ui.send.rpc(seed(start, thread), "first " ++ thread, "steer")

def second(+start: Nat, +thread: String) -> U.Act:
  U.Ui.send.rpc(U.Act.ui(first(start, thread)), "second " ++ thread, "steer")

def sends(cs: List<&2, U.Cmd>) -> List<&2, J.Json>:
  match cs:
    case Nil{}:
      Nil{}
    case U.Send{text} <> t:
      J.Str{text} <> sends(t)
    case _ <> t:
      sends(t)

def strings(xs: List<&2, String>) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      J.Str{x} <> strings(t)

def joined(u: U.Ui) -> U.Act:
  U.Ui.recv_act(u, json("{\"t\":\"log\",\"since\":0,\"origin\":\"outbox-origin\",\"info\":{},\"items\":[]}"))

def acknowledged(u: U.Ui, id: Nat) -> U.Ui:
  U.Ui.recv(u, J.Json.obj([J.KV{"t", J.Str{"reply"}}, J.KV{"id", J.Num{Nat.show(id)}}, J.KV{"ok", J.Flag{True{}}}]))

def scenario(+start: Nat, +thread: String) -> String:
  J.Json.show(J.Json.obj([
    J.KV{"original", J.Json.arr(List.append(&2, J.Json, sends(U.Act.cmds(first(start, thread))), sends(U.Act.cmds(second(start, thread)))))},
    J.KV{"resend", J.Json.arr(sends(U.Act.cmds(joined(U.Act.ui(second(start, thread))))))},
    J.KV{"again", J.Json.arr(sends(U.Act.cmds(joined(U.Act.ui(joined(U.Act.ui(second(start, thread))))))))},
    J.KV{"pending", J.Json.arr(strings(U.Ui.pending(U.Act.ui(second(start, thread)), thread)))},
    J.KV{"afterFirstAck", J.Json.arr(sends(U.Ui.outbox(acknowledged(U.Act.ui(second(start, thread)), start))))},
    J.KV{"afterSecondAck", J.Json.arr(sends(U.Ui.outbox(acknowledged(U.Act.ui(second(start, thread)), 1n+start))))},
    J.KV{"afterBoth", J.Json.arr(sends(U.Ui.outbox(acknowledged(acknowledged(U.Act.ui(second(start, thread)), 1n+start), start))))}]))
