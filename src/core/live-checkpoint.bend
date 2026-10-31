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

# The thread a message or turn change belongs to ("" for any other change):
# only those consume, split or re-anchor a live buffer.
def Checkpoint.touch(c: M.Change) -> String:
  match c:
    case M.MessagePosted{t, _, _, _, _}: t
    case M.TurnChanged{t, _, _}: t
    case _: ""

def Checkpoint.seen(ts: List<&2, String>, +t: String) -> Bool:
  match ts:
    case Nil{}: False{}
    case +h <> tail: Bool.or(String.eq(h, t), Checkpoint.seen(tail, t))

def Checkpoint.touched.k(skip: Bool, +t: String, acc: List<&2, String>) -> List<&2, String>:
  match skip:
    case True{}: acc
    case False{}: t <> acc

def Checkpoint.touched.c(+t: String, +acc: List<&2, String>) -> List<&2, String>:
  Checkpoint.touched.k(Bool.or(String.is_empty(t), Checkpoint.seen(acc, t)), t, acc)

def Checkpoint.touched.go(cs: List<&2, M.Change>, +acc: List<&2, String>) -> List<&2, String>:
  match cs:
    case Nil{}: acc
    case c <> tail: Checkpoint.touched.go(tail, Checkpoint.touched.c(Checkpoint.touch(c), acc))

# the threads a step's changes touched, each once, in order
def Checkpoint.touched(cs: List<&2, M.Change>) -> List<&2, String>:
  List.reverse(&2, String, Checkpoint.touched.go(cs, Nil{}))

# a thread with no buffer: its record says remove its file
def Checkpoint.gone(+t: String) -> J.Json:
  J.Json.obj([J.KV{"anchor", J.Str{""}}, J.KV{"buffer", J.Json.obj([J.KV{"thread", J.Str{t}}, J.KV{"text", J.Str{""}}])}])

def Checkpoint.of.k(has: Bool, +bs: List<&2, L.Buf>, +s: M.State, +t: String) -> J.Json:
  match has:
    case True{}: Checkpoint.one(bs, s, t)
    case False{}: Checkpoint.gone(t)

def Checkpoint.of(+bs: List<&2, L.Buf>, +s: M.State, +t: String) -> J.Json:
  Checkpoint.of.k(L.Live.has(bs, t), bs, s, t)

def Checkpoint.dirty.go(ts: List<&2, String>, +bs: List<&2, L.Buf>, +s: M.State) -> List<&2, J.Json>:
  match ts:
    case Nil{}: Nil{}
    case +t <> tail: Checkpoint.of(bs, s, t) <> Checkpoint.dirty.go(tail, bs, s)

# What a committed step writes: only the threads its messages and turn
# changes touched, each buffer whole (a new anchor, a steered remainder) or
# its file removed (an empty buffer, none at all). A stream in another
# thread is left to go on appending its deltas; nothing is rewritten for it.
def Checkpoint.dirty(cs: List<&2, M.Change>, +bs: List<&2, L.Buf>, +s: M.State) -> List<&2, J.Json>:
  Checkpoint.dirty.go(Checkpoint.touched(cs), bs, s)

# the threads of the restored buffers (their journals are kept at start)
def Checkpoint.threads(bs: List<&2, L.Buf>) -> List<&2, J.Json>:
  match bs:
    case Nil{}: Nil{}
    case b <> tail: J.Str{L.Buf.thread(b)} <> Checkpoint.threads(tail)
