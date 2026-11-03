import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/hub.bend as H
import ../src/core/cmds.bend as CM
import ../src/core/json.bend as J
import ../src/core/stamp.bend as STP
import ../src/core/i18n.bend as I

# Messages not read yet (cmds.bend's Cmds.unread, hub.bend's Wait.*,
# stamp.bend's Stamp.unread): a message sent while the agent works waits
# in the CLI behind the command it works on. The hub keeps the waiting
# messages in the setting wait.<thread>; while the thread runs their time
# reads "not read yet · 06:13", and they read as any other once started.

def log() -> List<&2, M.Change>:
  [M.ProjectCreated{"p1", "p", "/p", 1n}, M.ThreadCreated{"t", "p1", "one", M.Local{}, "", "", "claude", "", 1n},
    M.MessagePosted{"t", "u1", M.User{}, "the fan-out", 2n}, M.TurnChanged{"t", M.Running{}, 2n},
    M.MessagePosted{"t", "u2", M.User{}, "keep the SWD header", 5n}, M.MessagePosted{"t", "u3", M.User{}, "0.2 mm vias", 6n},
    M.MessagePosted{"o", "x1", M.User{}, "another thread", 6n}]

def state() -> M.State:
  M.State.replay(log(), M.State.empty())

def with(cs: List<&2, M.Change>) -> M.State:
  M.State.replay(List.append(&2, M.Change, log(), cs), M.State.empty())

# the uuid the hub gave a message's command (Hub.post.go)
def uuid(+th: String, +msg: String, +at: Nat) -> String:
  H.Hub.uuid(at, th ++ "/" ++ msg)

def sets(cs: List<&2, M.Change>) -> String:
  match cs:
    case Nil{}:
      ""
    case M.SettingSet{k, v, _} <> rest:
      k ++ "=" ++ v ++ ";" ++ sets(rest)
    case _ <> rest:
      "other;" ++ sets(rest)

def lc(+u: String, +state: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse("{\"type\":\"command_lifecycle\",\"command_uuid\":\"" ++ u ++ "\",\"state\":\"" ++ state ++ "\"}"), J.Null{})

def result() -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse("{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"result\":\"ok\"}"), J.Null{})

def text() -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse("{\"type\":\"assistant\",\"message\":{\"content\":[]}}"), J.Null{})

def waits(c: CM.Cmds) -> String:
  String.join(CM.Cmds.unread(c), " ")

# the CLI's order (test/fixtures/claude/stop-steer.jsonl): a is written and
# starts; b is written while a works, queued, and starts after a ends
def s1() -> CM.Cmds:
  CM.Cmds.sent(CM.Cmds.accept(CM.Cmds.new(), "a"), "a")

def s2() -> CM.Cmds:
  CM.Cmds.see(CM.Cmds.see(s1(), lc("a", "queued")), lc("a", "started"))

def s3() -> CM.Cmds:
  CM.Cmds.see(CM.Cmds.sent(CM.Cmds.accept(s2(), "b"), "b"), lc("b", "queued"))

def s4() -> CM.Cmds:
  CM.Cmds.see(s3(), result())

def s5() -> CM.Cmds:
  CM.Cmds.see(CM.Cmds.see(s4(), lc("a", "completed")), lc("b", "started"))

def unread(+s: M.State, +id: String, +at: Nat) -> String:
  Bool.pick(String, STP.Stamp.unread(s, M.Msg{id, "t", M.User{}, "x", at}), "unread", "read")

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("the first command, written and not started yet: nothing waits behind it", waits(s1()), "")
    C.Check.str("it started", waits(s2()), "")
    C.Check.str("one sent while it works waits", waits(s3()), "b")
    C.Check.str("still after the first's result", waits(s4()), "b")
    C.Check.str("read once it starts", waits(s5()), "")
    C.Check.str("behind a turn the CLI started by itself", waits(CM.Cmds{[CM.Cmd{"w", 1n, False{}, ""}], 1n, True{}}), "w")
    C.Check.str("a stop's tombs wait for nothing", waits(CM.Cmds.stop(s3(), Nil{})), "")
    C.Check.str("lifecycles and results move the ledger", Bool.pick(String, Bool.and(CM.Cmds.moves(lc("a", "queued")), CM.Cmds.moves(result())), "yes", "no"), "yes")
    C.Check.str("a streamed line does not", Bool.pick(String, CM.Cmds.moves(text()), "yes", "no"), "no")
    C.Check.str("the waiting messages are kept, oldest first", sets(H.Wait.set(state(), "t", [uuid("t", "u3", 6n), uuid("t", "u2", 5n)], 9n)), "wait.t=u2 u3;")
    C.Check.str("the same again: nothing", sets(H.Wait.set(with([M.SettingSet{"wait.t", "u2 u3", 8n}]), "t", [uuid("t", "u3", 6n), uuid("t", "u2", 5n)], 9n)), "")
    C.Check.str("the first picked up", sets(H.Wait.set(with([M.SettingSet{"wait.t", "u2 u3", 8n}]), "t", [uuid("t", "u3", 6n)], 9n)), "wait.t=u3;")
    C.Check.str("all picked up", sets(H.Wait.set(with([M.SettingSet{"wait.t", "u3", 8n}]), "t", Nil{}, 9n)), "wait.t=;")
    C.Check.str("none and none kept: nothing", sets(H.Wait.set(state(), "t", Nil{}, 9n)), "")
    C.Check.str("another thread's uuid is not this thread's message", sets(H.Wait.set(state(), "t", [uuid("o", "x1", 6n)], 9n)), "")
    C.Check.str("a line that moves nothing changes nothing", sets(H.Wait.line(False{}, state(), "t", [uuid("t", "u2", 5n)], 9n)), "")
    C.Check.str("a waiting message reads unread while the thread runs", unread(with([M.SettingSet{"wait.t", "u2 u3", 8n}]), "u2", 5n), "unread")
    C.Check.str("so does the next", unread(with([M.SettingSet{"wait.t", "u2 u3", 8n}]), "u3", 6n), "unread")
    C.Check.str("the one the agent works on does not", unread(with([M.SettingSet{"wait.t", "u2 u3", 8n}]), "u1", 2n), "read")
    C.Check.str("no prefix match", unread(with([M.SettingSet{"wait.t", "u22", 8n}]), "u2", 5n), "read")
    C.Check.str("picked up: read", unread(with([M.SettingSet{"wait.t", "u2 u3", 8n}, M.SettingSet{"wait.t", "u3", 9n}]), "u2", 5n), "read")
    C.Check.str("once the turn is over nothing reads unread", unread(with([M.SettingSet{"wait.t", "u2 u3", 8n}, M.TurnChanged{"t", M.Completed{}, 9n}]), "u2", 5n), "read")
    C.Check.str("its time says so", STP.Stamp.unread.text(True{}, I.En{}, "06:13"), "not read yet · 06:13")
    C.Check.str("a read one's is its time", STP.Stamp.unread.text(False{}, I.En{}, "06:13"), "06:13")
