import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/codec.bend as K
import ../src/core/json.bend as J
import ../src/core/hub.bend as H
import ../src/core/mcp.bend as MC

# thread_send's text: `message` is taken when `text` is blank (agents copy
# Claude Code's SendMessage), and a blank text is refused instead of being
# posted as an empty message that answers "sent".

def base() -> M.State:
  M.State.replay([M.ProjectCreated{"p1", "p", "/p", 1n}, M.ThreadCreated{"t0", "p1", "from", M.Local{}, "", "", "claude", "", 1n},
    M.ThreadCreated{"t1", "p1", "to", M.Local{}, "", "", "claude", "", 1n}], M.State.empty())

def send(+args: String) -> MC.Res:
  MC.Mcp.call.k(K.Codec.pos(MC.Mcp.names(), "thread_send"), base(), 20n, "t0", J.Null{}, Maybe.default(&2, J.Json, J.Json.parse(args), J.Null{}))

def shown(cs: List<&2, M.Change>) -> String:
  match cs:
    case Nil{}:
      ""
    case c <> t:
      J.Json.show(K.Change.encode(c)) ++ "\n" ++ shown(t)

def changes(r: MC.Res) -> String:
  match r:
    case MC.Res{_, _, H.Step{c, _}}:
      shown(c)

def ok(r: MC.Res) -> String:
  match r:
    case MC.Res{o, _, _}:
      Bool.show(o)

def body(r: MC.Res) -> String:
  match r:
    case MC.Res{_, b, _}:
      J.Json.show(b)

def main() -> IO(Unit):
  do IO<Unit>:
    +text : MC.Res = send("{\"threadId\":\"t1\",\"text\":\"route the clock\"}")
    C.Check.str("text is sent", ok(text), "True")
    C.Check.str("text reaches the thread", Bool.show(String.contains(changes(text), "route the clock")), "True")
    +alias : MC.Res = send("{\"threadId\":\"t1\",\"message\":\"route the clock\"}")
    C.Check.str("message alone is sent", ok(alias), "True")
    C.Check.str("message reaches the thread as its text", Bool.show(String.contains(changes(alias), "route the clock")), "True")
    +both : MC.Res = send("{\"threadId\":\"t1\",\"text\":\"  \",\"message\":\"use me\"}")
    C.Check.str("a blank text falls back to message", Bool.show(String.contains(changes(both), "use me")), "True")
    +none : MC.Res = send("{\"threadId\":\"t1\"}")
    C.Check.str("no text is refused", ok(none), "False")
    C.Check.str("the refusal names text", Bool.show(String.contains(body(none), "non-empty `text`")), "True")
    C.Check.str("a refusal posts nothing", changes(none), "")
    +empty : MC.Res = send("{\"threadId\":\"t1\",\"text\":\"\"}")
    C.Check.str("empty text is refused", ok(empty), "False")
    C.Check.str("empty text posts nothing", changes(empty), "")
    +ws : MC.Res = send("{\"threadId\":\"t1\",\"text\":\" \\n\\t \\r\"}")
    C.Check.str("whitespace-only text is refused", ok(ws), "False")
    C.Check.str("whitespace-only text posts nothing", changes(ws), "")
    +wsm : MC.Res = send("{\"threadId\":\"t1\",\"message\":\"   \"}")
    C.Check.str("whitespace-only message is refused", ok(wsm), "False")
    C.Check.str("blank check walks a long text", Bool.show(MC.Mcp.blank(String.repeat(" ", 50000n) ++ "x")), "False")
    C.Check.str("a long blank is blank", Bool.show(MC.Mcp.blank(String.repeat(" ", 50000n))), "True")
