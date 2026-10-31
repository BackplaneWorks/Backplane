import Base
import ./model.bend as M
import ./json.bend as J
import ./live.bend as L

# The latest message anchors a checkpoint to the exact reply being streamed.
# A completed reply, a new question or a stopped turn makes an old file inert,
# even if the hub exited between committing the log and removing that file.
def Checkpoint.anchor(es: List<&2, M.Entry>) -> String:
  match es:
    case Nil{}: ""
    case M.Msg{id, _, _, _, _} <> _: id
    case _ <> tail: Checkpoint.anchor(tail)

def Checkpoint.record(+s: M.State, +b: L.Buf) -> J.Json:
  J.Json.obj([J.KV{"anchor", J.Str{Checkpoint.anchor(M.State.mine(s, L.Buf.thread(b)))}}, J.KV{"buffer", L.Buf.json(b)}])

def Checkpoint.pick(hit: Bool, +s: M.State, b: L.Buf, rest: J.Json) -> J.Json:
  match hit:
    case True{}: Checkpoint.record(s, b)
    case False{}: rest

def Checkpoint.one(bs: List<&2, L.Buf>, +s: M.State, +thread: String) -> J.Json:
  match bs:
    case Nil{}: J.Null{}
    case +b <> tail:
      Checkpoint.pick(L.Live.is(b, thread), s, b, Checkpoint.one(tail, s, thread))

def Checkpoint.records(bs: List<&2, L.Buf>, +s: M.State) -> List<&2, J.Json>:
  match bs:
    case Nil{}: Nil{}
    case L.Buf{_, Nil{}, _, _, _} <> tail: Checkpoint.records(tail, s)
    case b <> tail: Checkpoint.record(s, b) <> Checkpoint.records(tail, s)

def Checkpoint.running(t: M.TurnState) -> Bool:
  match t:
    case M.Running{}: True{}
    case _: False{}

def Checkpoint.restore.one(valid: Bool, +b: J.Json) -> List<&2, L.Buf>:
  match valid:
    case False{}: Nil{}
    case True{}:
      [L.Buf{J.Json.get_str(b, "thread", ""), [J.Json.get_str(b, "text", "")], String.length(J.Json.get_str(b, "text", "")), J.Json.get_bool(b, "trunc", False{}), J.Json.get_nat(b, "gen", 0n)}]

def Checkpoint.restore.record(+s: M.State, +record: J.Json) -> List<&2, L.Buf>:
  +b = J.Json.get_obj(record, "buffer")
  +th = J.Json.get_str(b, "thread", "")
  Checkpoint.restore.one(Bool.and(Bool.not(String.is_empty(th)), Bool.and(Checkpoint.running(M.Thread.turn(M.State.thread.get(s, th))),
    String.eq(J.Json.get_str(record, "anchor", ""), Checkpoint.anchor(M.State.mine(s, th))))), b)

def Checkpoint.restore.items(rs: List<&2, J.Json>, +s: M.State) -> List<&2, L.Buf>:
  match rs:
    case Nil{}: Nil{}
    case r <> tail: List.append(&2, L.Buf, Checkpoint.restore.record(s, r), Checkpoint.restore.items(tail, s))

def Checkpoint.restore(+s: M.State, text: String) -> List<&2, L.Buf>:
  Checkpoint.restore.items(J.Json.elems(Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{})), s)

# Ordinary durable messages and turn ends can consume or split a live buffer.
# Other changes do not rewrite checkpoints.
def Checkpoint.changed(cs: List<&2, M.Change>) -> Bool:
  match cs:
    case Nil{}: False{}
    case M.MessagePosted{_, _, _, _, _} <> _: True{}
    case M.TurnChanged{_, _, _} <> _: True{}
    case _ <> tail: Checkpoint.changed(tail)
