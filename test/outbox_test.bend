import Base
import ./check.bend as C
import ../src/core/client.bend as U
import ../src/core/model.bend as M
import ../src/core/json.bend as J

# Exercise the public client actions: the same test runs against the old
# client, without importing the new ordering helper.
type Batch is Data:
  Batch{ui: U.Ui, sent: List<&2, String>}

def json(s: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(s), J.Null{})

def at(u: U.Ui, n: Nat) -> U.Ui:
  match u:
    case U.Ui{st, sel, dr, live, op, on, now, ad, pa, v, _, w, er, note, nf, ob, tr}:
      U.Ui{st, sel, dr, live, op, on, now, ad, pa, v, n, w, er, note, nf, ob, tr}

def legacy(u: U.Ui, key: String) -> U.Ui:
  match u:
    case U.Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}:
      U.Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, Map.set(&2, String, ob, key, ""), tr}

def size(u: U.Ui) -> String:
  match u:
    case U.Ui{_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, ob, _}:
      Nat.show(Map.size(&2, String, ob))

def error(u: U.Ui) -> String:
  match u:
    case U.Ui{_, _, _, _, _, _, _, _, _, _, _, _, er, _, _, _, _}: er

def texts(cs: List<&2, U.Cmd>) -> List<&2, String>:
  match cs:
    case Nil{}: Nil{}
    case U.Send{x} <> t: x <> texts(t)
    case _ <> t: texts(t)

def field(xs: List<&2, String>, +key: String) -> List<&2, String>:
  match xs:
    case Nil{}: Nil{}
    case x <> t: J.Json.as_raw(J.Json.get(json(x), key), "") <> field(t, key)

def msgs(xs: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}: Nil{}
    case x <> t: J.Json.get_str(J.Json.get_obj(json(x), "p"), "msg", "") <> msgs(t)

def wire(cs: List<&2, U.Cmd>) -> String:
  String.join(texts(cs), "\n")

def ids(cs: List<&2, U.Cmd>) -> String:
  String.join(field(texts(cs), "id"), ",")

def batch_ui(b: Batch) -> U.Ui:
  match b:
    case Batch{u, _}: u

def original(b: Batch) -> String:
  match b:
    case Batch{_, sent}: String.join(sent, "\n")

def capture(a: U.Act, sent: List<&2, String>) -> Batch:
  match a:
    case U.Act{u, cs}: Batch{u, List.append(&2, String, sent, texts(cs))}

def add(b: Batch, thread: String, text: String, msg: String) -> Batch:
  match b:
    case Batch{u, sent}:
      capture(U.Ui.rpc.keep(u, "turn.start", [J.KV{"thread", J.Str{thread}}, J.KV{"text", J.Str{text}}, J.KV{"msg", J.Str{msg}}]), sent)

def start(n: Nat) -> Batch:
  Batch{at(U.Ui.set_cid(U.Ui.init(123n), "offline-test"), n), Nil{}}

def four(n: Nat) -> Batch:
  add(add(add(add(start(n), "t1", "first", "m-first"), "t1", "second", "m-second"), "t1", "third", "m-third"), "t1", "fourth", "m-fourth")

def skip(b: Batch) -> Batch:
  match b:
    case Batch{u, sent}: Batch{U.Act.ui(U.Ui.rpc(u, "status", Nil{})), sent}

def gapped() -> Batch:
  add(add(skip(add(start(8n), "t1", "first", "m-first")), "t1", "second", "m-second"), "t1", "third", "m-third")

def jump(b: Batch, n: Nat) -> Batch:
  match b:
    case Batch{u, sent}: Batch{at(u, n), sent}

# Nat holds 48 bits, but each source literal must fit in 32 bits.
def near_limit() -> Nat:
  Nat.add(Nat.mul(4294967295n, 65536n), 65533n)

def sparse() -> Batch:
  add(add(jump(add(skip(add(start(4294967294n), "t1", "first", "m-first")), "t1", "second", "m-second"), near_limit()), "t1", "third", "m-third"), "t1", "fourth", "m-fourth")

def reply(+id: String, ok: Bool) -> J.Json:
  J.Json.obj([J.KV{"t", J.Str{"reply"}}, J.KV{"id", J.Num{id}}, J.KV{"ok", J.Flag{ok}}, J.KV{"error", J.Str{"request failed"}}])

def ack(u: U.Ui, id: String) -> U.Ui:
  U.Ui.recv(u, reply(id, True{}))

def log() -> J.Json:
  json("{\"t\":\"log\",\"since\":0,\"origin\":\"test-hub\",\"info\":{},\"items\":[]}")

def stored(u: U.Ui) -> U.Ui:
  U.Ui.fold([M.ThreadCreated{"t1", "", "First", M.Local{}, "", "", "claude", "", 1n},
    M.ThreadCreated{"t2", "", "Other", M.Local{}, "", "", "claude", "", 1n},
    M.MessagePosted{"t1", "m-second", M.User{}, "second", 2n}], u)

def pending(u: U.Ui, thread: String) -> String:
  String.join(U.Ui.pending(u, thread), "|")

# Closed scenarios keep large client values out of C IO continuations.
def Cases.ten() -> Batch:
  four(8n)

def Cases.hundred() -> Batch:
  four(98n)

def Cases.gap() -> Batch:
  gapped()

def Cases.large() -> Batch:
  sparse()

def Cases.u() -> U.Ui:
  batch_ui(Cases.ten())

def Cases.middle() -> U.Ui:
  ack(Cases.u(), "10")

def Cases.removed() -> U.Ui:
  ack(Cases.middle(), "8")

def Cases.duplicate() -> U.Ui:
  ack(Cases.removed(), "10")

def Cases.unknown() -> U.Ui:
  ack(Cases.duplicate(), "500")

def Cases.finished() -> U.Ui:
  ack(ack(Cases.unknown(), "11"), "9")

def Cases.ordinary() -> U.Ui:
  ack(batch_ui(Cases.gap()), "9")

def Cases.failed() -> U.Ui:
  U.Ui.recv(Cases.u(), reply("9", False{}))

def Cases.old() -> U.Ui:
  legacy(legacy(Cases.u(), "7"), "1000")

def Cases.other() -> U.Ui:
  batch_ui(add(Cases.ten(), "t2", "elsewhere", "m-other"))

def Cases.joined() -> U.Act:
  U.Ui.recv_act(Cases.u(), log())

def Cases.again() -> U.Act:
  U.Ui.recv_act(U.Act.ui(Cases.joined()), log())

def Cases.answered() -> U.Act:
  U.Ui.recv_act(U.Act.ui(Cases.again()), reply("9", True{}))

def Cases.rejoined() -> U.Act:
  U.Ui.recv_act(U.Act.ui(Cases.answered()), log())

def Cases.escaped() -> Batch:
  add(add(start(9n), "t1", "first\n\"quoted\" π", "original-one"), "t1", "second\\end", "original-two")

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("empty outbox has no commands", ids(U.Ui.outbox(U.Ui.init(0n))), "")
    C.Check.str("empty pending has no texts", pending(U.Ui.init(0n), "t1"), "")
    C.Check.str("decimal transition 8 through 11 resends oldest first", ids(U.Ui.outbox(Cases.u())), "8,9,10,11")
    C.Check.str("decimal transition 98 through 101 resends oldest first", ids(U.Ui.outbox(batch_ui(Cases.hundred()))), "98,99,100,101")
    C.Check.str("decimal transition pending is oldest first", pending(Cases.u(), "t1"), "first|second|third|fourth")
    C.Check.str("three digit transition pending is oldest first", pending(batch_ui(Cases.hundred()), "t1"), "first|second|third|fourth")
    C.Check.str("resend preserves initially emitted request bytes", wire(U.Ui.outbox(Cases.u())), original(Cases.ten()))
    C.Check.str("resend preserves message identity", String.join(msgs(texts(U.Ui.outbox(Cases.u()))), ","), "m-first,m-second,m-third,m-fourth")
    C.Check.str("resend preserves escaped and unicode payloads", wire(U.Ui.outbox(batch_ui(Cases.escaped()))), original(Cases.escaped()))
    C.Check.str("ordinary RPC gaps do not change resend order", ids(U.Ui.outbox(batch_ui(Cases.gap()))), "8,10,11")
    C.Check.str("ordinary RPC gaps do not change pending order", pending(batch_ui(Cases.gap()), "t1"), "first|second|third")
    C.Check.str("sparse U32 and Nat boundary IDs keep numeric order", ids(U.Ui.outbox(batch_ui(Cases.large()))), "4294967294,4294967296,281474976710653,281474976710654")
    C.Check.str("sparse IDs retain exact original request bytes", wire(U.Ui.outbox(batch_ui(Cases.large()))), original(Cases.large()))
    C.Check.str("sparse IDs retain pending order", pending(batch_ui(Cases.large()), "t1"), "first|second|third|fourth")
    C.Check.str("middle reply removes only its request", ids(U.Ui.outbox(Cases.middle())), "8,9,11")
    C.Check.str("reply deletes its outbox entry", size(Cases.middle()), "3")
    C.Check.str("oldest reply leaves chronological survivors", ids(U.Ui.outbox(Cases.removed())), "9,11")
    C.Check.str("duplicate reply cannot recreate an entry", size(Cases.duplicate()), "2")
    C.Check.str("duplicate reply preserves outstanding bytes", wire(U.Ui.outbox(Cases.duplicate())), wire(U.Ui.outbox(Cases.removed())))
    C.Check.str("unknown reply cannot grow the outbox", size(Cases.unknown()), "2")
    C.Check.str("unknown reply preserves outstanding bytes", wire(U.Ui.outbox(Cases.unknown())), wire(U.Ui.outbox(Cases.removed())))
    C.Check.str("non-kept reply cannot grow the outbox", size(Cases.ordinary()), "3")
    C.Check.str("non-kept reply preserves kept requests", wire(U.Ui.outbox(Cases.ordinary())), original(Cases.gap()))
    C.Check.str("all replies leave no retained entries", size(Cases.finished()), "0")
    C.Check.str("all replies leave no resend commands", ids(U.Ui.outbox(Cases.finished())), "")
    C.Check.str("all replies leave no pending messages", pending(Cases.finished(), "t1"), "")
    C.Check.str("failed reply also removes its request", ids(U.Ui.outbox(Cases.failed())), "8,10,11")
    C.Check.str("failed reply still reports its error", error(Cases.failed()), "request failed")
    C.Check.str("legacy empty entries never resend", wire(U.Ui.outbox(Cases.old())), original(Cases.ten()))
    C.Check.str("legacy empty entries never appear pending", pending(Cases.old(), "t1"), "first|second|third|fourth")
    C.Check.str("only legacy empty entries have no commands", ids(U.Ui.outbox(legacy(legacy(U.Ui.init(0n), "9"), "10"))), "")
    C.Check.str("pending ignores other threads", pending(Cases.other(), "t1"), "first|second|third|fourth")
    C.Check.str("pending selects the other thread", pending(Cases.other(), "t2"), "elsewhere")
    C.Check.str("stored message is omitted without reordering pending", pending(stored(Cases.other()), "t1"), "first|third|fourth")
    C.Check.str("stored message does not remove unacknowledged resend", ids(U.Ui.outbox(stored(Cases.other()))), "8,9,10,11,12")
    C.Check.str("shared reconnect resends oldest first", ids(U.Act.cmds(Cases.joined())), "8,9,10,11")
    C.Check.str("shared reconnect preserves original request bytes", wire(U.Act.cmds(Cases.joined())), original(Cases.ten()))
    C.Check.str("repeated reconnect preserves original request bytes", wire(U.Act.cmds(Cases.again())), original(Cases.ten()))
    C.Check.str("shared reply does not resend requests", ids(U.Act.cmds(Cases.answered())), "")
    C.Check.str("shared reconnect excludes answered requests", ids(U.Act.cmds(Cases.rejoined())), "8,10,11")
    C.Check.str("changes frame does not resend requests", ids(U.Act.cmds(U.Ui.recv_act(Cases.u(), json("{\"t\":\"changes\",\"items\":[]}")))), "")
    C.Check.str("info frame does not resend requests", ids(U.Act.cmds(U.Ui.recv_act(Cases.u(), json("{\"t\":\"info\",\"info\":{}}")))), "")
