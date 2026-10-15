import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/omp.bend as OMP
import ../src/core/acp.bend as AC
import ../src/core/msp.bend as MS
import ../src/core/json.bend as J
import ../src/core/utf8.bend as U

# oh-my-pi (core/omp.bend): the mapping, the install and auth lines, the
# models, and the turn replayed from test/fixtures/omp/turn.jsonl (a fresh
# session, the config steps, the prompt), a turn with no config steps, a
# lost session, a sign-in failure and the runner's install line.

def turn(+session: String, +model: String, +effort: String) -> OMP.OmpTurn:
  OMP.OmpTurn{AC.AcpTurn{"/tmp/p", "http://127.0.0.1:1/mcp/t?token=x", "RULES", "hello", session, True{}}, model, effort}

type Run is Data:
  Run{a: AC.Acp, sends: List<&2, String>, emits: List<&2, String>}

def Run.add(o: AC.AcpOut, +ss: List<&2, String>, +es: List<&2, String>) -> Run:
  match o:
    case AC.AcpOut{a, s, e, _}:
      Run{a, List.append(&2, String, ss, s), List.append(&2, String, es, e)}

def Run.step(r: Run, +t: OMP.OmpTurn, +l: String) -> Run:
  match r:
    case Run{a, +ss, +es}:
      Run.add(OMP.Omp.line(a, t, l), ss, es)

def replay(ls: List<&2, String>, r: Run, +t: OMP.OmpTurn) -> Run:
  match ls:
    case Nil{}:
      r
    case +l <> rest:
      replay(rest, Run.step(r, t, l), t)

def sends(r: Run) -> List<&2, String>:
  match r:
    case Run{_, s, _}:
      s

def emits(r: Run) -> String:
  match r:
    case Run{_, _, e}:
      String.join(e, " | ")

def done(r: Run) -> String:
  match r:
    case Run{AC.Acp{_, _, True{}}, _, _}:
      "done"
    case Run{_, _, _}:
      "open"

def obj(+line: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(line), J.Null{})

def method(+line: String) -> String:
  J.Json.get_str(obj(line), "method", "")

def methods(xs: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> t:
      method(x) <> methods(t)

def params(+line: String) -> J.Json:
  J.Json.get_obj(obj(line), "params")

def nth(xs: List<&2, String>, n: Nat) -> String:
  Maybe.default(&2, String, List.head(&2, String, List.drop(&2, String, xs, n)), "")

def yes(b: Bool) -> String:
  Bool.pick(String, b, "yes", "no")

def start() -> Run:
  Run{AC.Acp.new(), Nil{}, Nil{}}

def lines.r(r: U.Lines & List<&2, String>) -> List<&2, String>:
  (_, ls) = r
  ls

def file.done(r: File & Result<&1, &1, U32 & String, List<&2, U32>>) -> IO(List<&2, String>):
  (f, res) = r
  do IO<List<&2, String>>:
    File.close(f)
    return lines.r(U.Lines.feed(U.Lines.new(), Result.default(&1, &1, U32 & String, List<&2, U32>, res, Nil{})))

def file.sized(r: File & Result<&1, &1, U32 & String, U32>) -> IO(List<&2, String>):
  (f, res) = r
  IO.bind(File & Result<&1, &1, U32 & String, List<&2, U32>>, List<&2, String>, File.read_bytes(f, U32.max(Result.default(&1, &1, U32 & String, U32, res, 0), 1)), file.done)

def file.opened(r: Result<&1, &1, U32 & String, File>) -> IO(List<&2, String>):
  match r:
    case Done{f}:
      IO.bind(File & Result<&1, &1, U32 & String, U32>, List<&2, String>, File.size(f), file.sized)
    case Fail{_}:
      IO.pure(List<&2, String>, Nil{})

def file(path: String) -> IO(List<&2, String>):
  IO.bind(Result<&1, &1, U32 & String, File>, List<&2, String>, File.open(path, "r"), file.opened)

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("omp approval supervised", OMP.Omp.approval(M.Supervised{}), "always-ask")
    C.Check.str("omp approval accept edits", OMP.Omp.approval(M.AcceptEdits{}), "write")
    C.Check.str("omp approval auto", OMP.Omp.approval(M.AutoMode{}), "write")
    C.Check.str("omp approval full access", OMP.Omp.approval(M.FullAccess{}), "yolo")
    C.Check.str("omp args", String.join(OMP.Omp.args(M.Supervised{}), " "), "acp --approval-mode always-ask")
    C.Check.str("omp levels", String.join(OMP.Omp.levels(), ","), "off,auto,low,medium,high,xhigh,max")
    C.Check.str("omp models keeps chat only", String.join(OMP.Omp.models("{\"models\":[{\"kind\":\"chat\",\"selector\":\"a/b\"},{\"kind\":\"embedding\",\"selector\":\"c/d\"}]}"), ","), "a/b")
    C.Check.str("omp models bad json", String.join(OMP.Omp.models("nope"), ","), "")
    C.Check.str("omp hint auth", OMP.Omp.hint("Not authenticated"), "Not authenticated (sign in once with `omp login`)")
    C.Check.str("omp hint login", OMP.Omp.hint("login required"), "login required (sign in once with `omp login`)")
    C.Check.str("omp hint plain", OMP.Omp.hint("tool failed"), "tool failed")
    C.Check.str("omp install", OMP.Omp.install(), "Install oh-my-pi with `curl -fsSL https://omp.sh/install | sh` or `bun install -g @oh-my-pi/pi-coding-agent`, then sign in with `omp login`.")
    # the runner's Proto: the command, its name, its install line and its handshake
    C.Check.str("omp proto cmd", MS.Proto.cmd(MS.POmp{AC.AcpTurn{"/tmp/p", "u", "P", "t", "", True{}}, "", ""}), "omp")
    C.Check.str("omp proto name", MS.Proto.name(MS.POmp{AC.AcpTurn{"/tmp/p", "u", "P", "t", "", True{}}, "", ""}), "omp")
    C.Check.str("omp proto install", MS.Proto.install(MS.POmp{AC.AcpTurn{"/tmp/p", "u", "P", "t", "", True{}}, "", ""}), OMP.Omp.install())
    C.Check.str("omp proto gone", MS.Proto.gone(MS.POmp{AC.AcpTurn{"/tmp/p", "u", "P", "t", "", True{}}, "", ""}), "omp exited before the turn ended (is it signed in? run `omp login`)")
    C.Check.str("omp proto init", MS.Proto.init(MS.POmp{AC.AcpTurn{"/tmp/p", "u", "P", "t", "", True{}}, "", ""}), AC.Acp.init.form())
    +fx : List<&2, String> <- file("test/fixtures/omp/turn.jsonl")
    # the fixture's session/new reply (line 2) is followed by the model and thinking config, each answered, then the prompt
    +cfg4 : String = "{\"jsonrpc\":\"2.0\",\"id\":4,\"result\":{}}"
    +cfg5 : String = "{\"jsonrpc\":\"2.0\",\"id\":5,\"result\":{}}"
    +spliced : List<&2, String> = List.append(&2, String, [nth(fx, 0n), nth(fx, 1n), cfg4, cfg5], List.drop(&2, String, fx, 2n))
    +r1 : Run = replay(spliced, start(), turn("", "anthropic/claude-haiku-5-5", "high"))
    C.Check.str("omp turn order", String.join(methods(sends(r1)), ","), "session/new,session/set_config_option,session/set_config_option,session/prompt")
    C.Check.str("omp turn model", J.Json.get_str(params(nth(sends(r1), 1n)), "configId", "") ++ "=" ++ J.Json.get_str(params(nth(sends(r1), 1n)), "value", ""), "model=anthropic/claude-haiku-5-5")
    C.Check.str("omp turn thinking", J.Json.get_str(params(nth(sends(r1), 2n)), "configId", "") ++ "=" ++ J.Json.get_str(params(nth(sends(r1), 2n)), "value", ""), "thinking=high")
    C.Check.str("omp turn session", J.Json.get_str(params(nth(sends(r1), 1n)), "sessionId", ""), "01a14112-89a7-7544-b120-9a13b2948ace")
    C.Check.str("omp turn rules first", yes(String.contains(nth(sends(r1), 3n), "<backplane_instructions>\\nRULES")), "yes")
    C.Check.str("omp turn started", yes(String.contains(emits(r1), "{\"type\":\"thread.started\",\"thread_id\":\"01a14112-89a7-7544-b120-9a13b2948ace\"}")), "yes")
    C.Check.str("omp turn finished", done(r1), "done")
    # no model and no thinking: the prompt straight after session/new
    +r2 : Run = replay(List.append(&2, String, [nth(fx, 0n), nth(fx, 1n)], Nil{}), start(), turn("", "", ""))
    C.Check.str("omp plain order", String.join(methods(sends(r2)), ","), "session/new,session/prompt")
    C.Check.str("omp plain prompt", yes(String.contains(nth(sends(r2), 1n), "<backplane_instructions>")), "yes")
    # a session/load the agent cannot answer: a fresh session/new (id 7), with a note, then the prompt
    +init : String = nth(fx, 0n)
    +lost : Run = replay(["{\"jsonrpc\":\"2.0\",\"id\":2,\"error\":{\"code\":-32603,\"message\":\"session not found\"}}", "{\"jsonrpc\":\"2.0\",\"id\":7,\"result\":{\"sessionId\":\"NEW\"}}"],
      replay([init], start(), turn("OLD", "", "")), turn("OLD", "", ""))
    C.Check.str("omp lost order", String.join(methods(sends(lost)), ","), "session/load,session/new,session/prompt")
    C.Check.str("omp lost id", J.Json.as_raw(J.Json.get(obj(nth(sends(lost), 1n)), "id"), ""), "7")
    C.Check.str("omp lost note", yes(String.contains(emits(lost), "could not be resumed")), "yes")
    C.Check.str("omp lost rules", yes(String.contains(nth(sends(lost), 2n), "<backplane_instructions>")), "yes")
    # a session/new that names auth gets the omp login hint
    C.Check.str("omp auth hint", emits(replay(["{\"jsonrpc\":\"2.0\",\"id\":2,\"error\":{\"code\":-32000,\"message\":\"Not authenticated\"}}"], replay([init], start(), turn("", "", "")), turn("", "", ""))),
      "{\"type\":\"turn.failed\",\"message\":\"Not authenticated (sign in once with `omp login`)\"}")
