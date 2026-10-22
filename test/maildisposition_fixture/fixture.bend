import Base
import ../maildisposition_test.bend as T
import ../../src/core/model.bend as M
import ../../src/core/hub.bend as H
import ../../src/core/mcp.bend as MCP
import ../../src/core/codec.bend as C
import ../../src/core/json.bend as J
import ../../src/core/mail.bend as ML
import ../../src/core/maildisposition.bend as D

# Test adapter only. Production MCP supplies all tool definitions,
# validation and dispositions. The host fsyncs these exact events/body
# before accepting the returned state. There is no real hub or provider IO.
type Prepared is Data:
  Prepared{before: M.State, reply: String, changes: List<&2, M.Change>, body: String}

def initial() -> M.State: T.base()
def run_id() -> String: T.run()
def first_input() -> String: T.input()
def current(+s: M.State) -> String:
  H.Run.input(s, ML.Run{T.run(), "root", T.conv(), T.input(), 1792519184n, True{}})

def encoded(cs: List<&2, M.Change>) -> List<&2, J.Json>:
  match cs:
    case Nil{}: Nil{}
    case c <> tail: C.Change.encode(c) <> encoded(tail)

def append_changes(os: List<&2, H.Out>) -> List<&2, M.Change>:
  match os:
    case H.MailAppend{_, _, cs} <> Nil{}: cs
    case _: Nil{}

def append_body(os: List<&2, H.Out>) -> String:
  match os:
    case H.MailAppend{line, _, _} <> Nil{}: line
    case _: ""

def prepared(+before: M.State, o: MCP.McpOut) -> Prepared:
  match o:
    case MCP.McpOut{reply, H.Step{cs, +os}, _}:
      Prepared{before, reply, List.append(&2, M.Change, cs, append_changes(os)), append_body(os)}

def prepare(+s: M.State, text: String) -> Prepared:
  prepared(s, MCP.Mcp.text(s, 1792519190n, T.run(), J.Null{}, "fixture", text))

def reply(p: Prepared) -> String:
  match p:
    case Prepared{_, r, _, _}: r

def events(p: Prepared) -> String:
  match p:
    case Prepared{_, _, cs, _}: J.Json.show(J.Json.arr(encoded(cs)))

def body(p: Prepared) -> String:
  match p:
    case Prepared{_, _, _, b}: b

def commit(p: Prepared) -> M.State:
  match p:
    case Prepared{s, _, cs, _}: M.State.replay(cs, s)

def advance_event(input: String) -> String:
  J.Json.show(C.Change.encode(T.incoming(input)))

def advance(s: M.State, input: String) -> M.State:
  M.State.apply(s, T.incoming(input))

def replay_one(c: Maybe<&2, M.Change>, s: M.State) -> M.State:
  match c:
    case None{}: s
    case Some{c}: M.State.apply(s, c)

def replay_go(xs: List<&2, J.Json>, s: M.State) -> M.State:
  match xs:
    case Nil{}: s
    case x <> tail: replay_go(tail, replay_one(C.Change.decode(x), s))

def restore(text: String) -> M.State:
  replay_go(J.Json.elems(Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{})), initial())

def complete(+s: M.State) -> Prepared:
  +st = T.fallback(s)
  Prepared{s, "", H.Queue.changes(st), ""}

def handled(+s: M.State, input: String) -> Bool:
  Bool.not(String.is_empty(M.Setting.get(s, D.Disposition.key(T.run(), input), "")))

def summary(+s: M.State) -> String:
  J.Json.show(J.Json.obj([
    J.KV{"done", J.Flag{handled(s, T.input())}},
    J.KV{"reply", J.Flag{handled(s, "fixture-m2")}},
    J.KV{"defer", J.Flag{handled(s, "fixture-m3")}},
    J.KV{"current", J.Str{current(s)}},
    J.KV{"hold_enabled", J.Str{M.Setting.get(s, "hold.enabled", "")}},
    J.KV{"forwarded", J.Str{M.Setting.get(s, "run.forwarded." ++ T.run(), "")}},
    J.KV{"fallback_posts", J.Json.nat(T.owes(s))},
    J.KV{"next_fallback_posts", J.Json.nat(T.owes(M.State.apply(s, T.incoming("fixture-m4"))))}]))
