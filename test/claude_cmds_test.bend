import Base
import ./check.bend as C
import ../src/core/json.bend as J
import ../src/core/utf8.bend as U
import ../src/core/model.bend as M
import ../src/core/hub.bend as B
import ../src/core/cmds.bend as CM
import ../src/core/codec.bend as C2
import ../src/core/wnote.bend as WN
import ../src/core/secret.bend as SEC

# Claude's turns by the command ledger (core/cmds.bend): both recorded
# Claude Code 2.1.286 streams (test/fixtures/claude/) replayed line by line
# through CM.Cmds and B.Hub.agent_line.vo on a thread, as the server does,
# checking the verdict and the thread's turn after every line; then the
# sequences the recordings lack (the live repro, Stop before a command
# starts or while its images are read, a wake beside a queued message, a
# stopped command the CLI starts after all, an old CLI).

type Ev is Data:
  # the next line: its verdict and the turn after it (R C I F -)
  Line{v: Nat, st: String}
  # a line given here rather than read from the recording
  Raw{text: String, v: Nat, st: String}
  # the hub writes a command (the thread runs)
  Send{u: String}
  # the hub takes a command whose images it still reads
  Prep{u: String}
  # the user's Stop
  Stop{}

# (tr: after each line, the thread's background rows, "all/at work", last
# first)
type Sim is Data:
  Sim{s: M.State, c: CM.Cmds, n: Nat, wakes: Nat, ends: Nat, tr: List<&2, String>}

def turn.k(t: M.TurnState) -> String:
  match t:
    case M.Idle{}:
      "-"
    case M.Running{}:
      "R"
    case M.Completed{}:
      "C"
    case M.Interrupted{}:
      "I"
    case M.Failed{}:
      "F"

def turn(+s: M.State) -> String:
  turn.k(M.Thread.turn(B.Hub.thread(s, "t1")))

def wakes(cs: List<&2, M.Change>) -> Nat:
  match cs:
    case Nil{}:
      0n
    case M.ActivityLogged{_, _, _, +kind, _, _} <> t:
      Nat.add(Bool.pick(Nat, String.eq(kind, "wake"), 1n, 0n), wakes(t))
    case _ <> t:
      wakes(t)

def ends(os: List<&2, B.Out>) -> Nat:
  match os:
    case Nil{}:
      0n
    case B.TurnEnded{_, _, _, _, _, _} <> t:
      1n+ends(t)
    case _ <> t:
      ends(t)

def start() -> M.State:
  M.State.replay([M.ProjectCreated{"p1", "p", "/p", 1n}, M.ThreadCreated{"t1", "p1", "x", M.Local{}, "", "", "claude", "", 1n}], M.State.empty())

def sim0() -> Sim:
  Sim{start(), CM.Cmds.new(), 0n, 0n, 0n, Nil{}}

def rows(+s: M.State) -> String:
  Nat.show(M.Bg.live(M.Bg.of(s, "t1"))) ++ "/" ++ Nat.show(M.Bg.running(M.Bg.of(s, "t1")))

def trace.k(+s: M.State, tr: List<&2, String>) -> List<&2, String>:
  rows(s) <> tr

# one line as the server takes it: judged, seen, then stepped
def feed.k(st: B.Step, +v: Nat, +o: J.Json, sim: Sim) -> Sim & Nat:
  match st sim:
    case B.Step{+cs, os} Sim{s, c, n, w, e, tr}:
      +s2 = M.State.replay(cs, s)
      (Sim{s2, CM.Cmds.see(c, o), 1n+n, Nat.add(w, wakes(cs)), Nat.add(e, ends(os)), trace.k(s2, tr)}, v)

def feed.v(+v: Nat, +line: String, +o: J.Json, +sim: Sim) -> Sim & Nat:
  match sim:
    case Sim{+s, +c, _, _, _, _}:
      feed.k(B.Hub.agent_line.vo(v, s, 9n, "t1", line, o, CM.Cmds.text(c, o)), v, o, sim)

def feed.o(+line: String, +o: J.Json, +sim: Sim) -> Sim & Nat:
  match sim:
    case Sim{+s, +c, _, _, _, _}:
      feed.v(CM.Cmds.verdict(c, B.Hub.running(B.Hub.thread(s, "t1")), o), line, o, sim)

def feed(+line: String, +sim: Sim) -> Sim & Nat:
  feed.o(line, Maybe.default(&2, J.Json, J.Json.parse(line), J.Null{}), sim)

def send.k(run: Bool, +s: M.State) -> M.State:
  match run:
    case True{}:
      s
    case False{}:
      M.State.replay([M.TurnChanged{"t1", M.Running{}, 9n}], s)

def send(+u: String, sim: Sim) -> Sim:
  match sim:
    case Sim{+s, c, n, w, e, tr}:
      Sim{send.k(B.Hub.running(B.Hub.thread(s, "t1")), s), CM.Cmds.sent(CM.Cmds.accept(c, u), u), n, w, e, tr}

def prep(+u: String, sim: Sim) -> Sim:
  match sim:
    case Sim{+s, c, n, w, e, tr}:
      Sim{send.k(B.Hub.running(B.Hub.thread(s, "t1")), s), CM.Cmds.accept(c, u), n, w, e, tr}

def stop(sim: Sim) -> Sim:
  match sim:
    case Sim{+s, c, n, w, e, tr}:
      Sim{M.State.replay(B.Queue.changes(B.Hub.interrupt(s, 9n, 0, "r", "t1")), s), CM.Cmds.stop(c, Nil{}), n, w, e, tr}

def sim.s(sim: Sim) -> M.State:
  match sim:
    case Sim{s, _, _, _, _, _}:
      s

def sim.n(sim: Sim) -> Nat:
  match sim:
    case Sim{_, _, n, _, _, _}:
      n

def sim.wakes(sim: Sim) -> Nat:
  match sim:
    case Sim{_, _, _, w, _, _}:
      w

def sim.ends(sim: Sim) -> Nat:
  match sim:
    case Sim{_, _, _, _, e, _}:
      e

def sim.tr(sim: Sim) -> List<&2, String>:
  match sim:
    case Sim{_, _, _, _, _, tr}:
      List.reverse(&2, String, tr)

# the rows after lines a..b (from 1)
def rows.at(+sim: Sim, +a: Nat, +b: Nat) -> String:
  String.join(List.take(&2, String, List.drop(&2, String, sim.tr(sim), Nat.sub(a, 1n)), Nat.add(Nat.sub(b, a), 1n)), ",")

def show(+v: Nat, +s: M.State) -> String:
  Nat.show(v) ++ "/" ++ turn(s)

def line.check(r: Sim & Nat, +name: String, +want: String, k: Sim -> IO(Unit)) -> IO(Unit):
  (+sim, +v) = r
  do IO<Unit>:
    C.Check.str(name ++ " l." ++ Nat.show(sim.n(sim)), show(v, sim.s(sim)), want)
    k(sim)

def head(ls: List<&2, String>) -> String:
  Maybe.default(&2, String, List.head(&2, String, ls), "")

def tail(ls: List<&2, String>) -> List<&2, String>:
  List.drop(&2, String, ls, 1n)

def run(evs: List<&2, Ev>, +ls: List<&2, String>, sim: Sim, +name: String, k: Sim -> IO(Unit)) -> IO(Unit):
  match evs:
    case Nil{}:
      k(sim)
    case Line{+v, +st} <> t:
      line.check(feed(head(ls), sim), name, Nat.show(v) ++ "/" ++ st, s2 => run(t, tail(ls), s2, name, k))
    case Raw{+text, +v, +st} <> t:
      line.check(feed(text, sim), name, Nat.show(v) ++ "/" ++ st, s2 => run(t, ls, s2, name, k))
    case Send{+u} <> t:
      run(t, ls, send(u, sim), name, k)
    case Prep{+u} <> t:
      run(t, ls, prep(u, sim), name, k)
    case Stop{} <> t:
      run(t, ls, stop(sim), name, k)

def rep(n: Nat, +v: Nat, +st: String) -> List<&2, Ev>:
  match n:
    case 0n:
      Nil{}
    case 1n+p:
      Line{v, st} <> rep(p, v, st)

def cat(xs: List<&2, List<&2, Ev>>) -> List<&2, Ev>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      List.append(&2, Ev, x, cat(t))

# fixture 1: a background shell ends after its turn, and Claude wakes by
# itself; then a second message
def bg_wake() -> List<&2, Ev>:
  cat([[Send{"11111111-1111-4111-8111-111111111111"}], rep(14n, 2n, "R"),
    [Line{1n, "R"}, Line{3n, "C"}, Line{1n, "C"}, Line{1n, "C"}, Line{6n, "R"}], rep(3n, 2n, "R"), [Line{3n, "C"}],
    [Send{"22222222-2222-4222-8222-222222222222"}], rep(6n, 2n, "R"), [Line{1n, "R"}, Line{3n, "C"}]])

def u(+n: String) -> String:
  "aaaaaaaa-0000-4000-8000-00000000000" ++ n

# fixture 2: Stop with the next message sent at once; two messages back
# to back; a background shell that outlives two turns
def stop_steer() -> List<&2, Ev>:
  cat([[Send{u("1")}], rep(6n, 2n, "R"), [Stop{}, Send{u("2")}],
    [Line{1n, "R"}, Line{1n, "R"}, Line{1n, "R"}, Line{1n, "R"}, Line{1n, "R"}],
    rep(6n, 2n, "R"), [Line{1n, "R"}, Line{3n, "C"}],
    [Send{u("3")}], rep(2n, 2n, "R"), [Send{u("4")}], rep(4n, 2n, "R"), [Line{1n, "R"}, Line{1n, "R"}], rep(4n, 2n, "R"), [Line{1n, "R"}, Line{3n, "C"}],
    [Send{u("5")}], rep(12n, 2n, "R"), [Line{1n, "R"}, Line{3n, "C"}],
    [Send{u("6")}], rep(9n, 2n, "R"), [Line{1n, "R"}, Line{3n, "C"}],
    [Line{1n, "C"}, Line{1n, "C"}, Line{6n, "R"}], rep(5n, 2n, "R"), [Line{3n, "C"}]])

def lc(+n: String, +state: String) -> String:
  "{\"type\":\"command_lifecycle\",\"command_uuid\":\"" ++ u(n) ++ "\",\"state\":\"" ++ state ++ "\"}"

def init() -> String:
  "{\"type\":\"system\",\"subtype\":\"init\",\"session_id\":\"s1\",\"claude_code_version\":\"2.1.286\"}"

def init.old() -> String:
  "{\"type\":\"system\",\"subtype\":\"init\",\"session_id\":\"s1\",\"claude_code_version\":\"2.0.0\"}"

def note() -> String:
  "{\"type\":\"system\",\"subtype\":\"task_notification\",\"task_id\":\"b1\",\"status\":\"completed\",\"summary\":\"Background command done\"}"

def say(+text: String) -> String:
  "{\"type\":\"assistant\",\"message\":{\"id\":\"m-" ++ text ++ "\",\"role\":\"assistant\",\"content\":[{\"type\":\"text\",\"text\":\"" ++ text ++ "\"}]}}"

def result(+text: String) -> String:
  "{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"result\":\"" ++ text ++ "\"}"

def failed() -> String:
  "{\"type\":\"result\",\"subtype\":\"error_during_execution\",\"is_error\":true}"

# the live repro: the resumed process first runs a stale notification
# turn while the user's command waits in its queue
def stale() -> List<&2, Ev>:
  [Send{u("1")}, Raw{note(), 0n, "R"}, Raw{init(), 2n, "R"}, Raw{say("stale"), 2n, "R"}, Raw{result("stale"), 1n, "R"},
   Raw{lc("1", "queued"), 2n, "R"}, Raw{lc("1", "started"), 2n, "R"}, Raw{init(), 2n, "R"}, Raw{say("real"), 2n, "R"}, Raw{result("real"), 1n, "R"},
   Raw{lc("1", "completed"), 3n, "C"}]

# Stop before the command started: it never runs
def stop_early() -> List<&2, Ev>:
  [Raw{init(), 6n, "R"}, Raw{result("x"), 3n, "C"}, Send{u("1")}, Stop{}, Raw{lc("1", "queued"), 2n, "I"}, Raw{failed(), 1n, "I"}, Raw{lc("1", "cancelled"), 1n, "I"}]

# a stopped command the CLI starts after all shows as running again, and
# ends with its lifecycle
def tomb_started() -> List<&2, Ev>:
  [Raw{init(), 6n, "R"}, Raw{result("x"), 3n, "C"}, Send{u("1")}, Stop{}, Raw{lc("1", "started"), 6n, "R"}, Raw{say("late"), 2n, "R"}, Raw{result("late"), 1n, "R"}, Raw{lc("1", "completed"), 3n, "C"}]

# a message whose images are still read counts as outstanding: a wake's
# result beside it never ends the thread
def prep_wake() -> List<&2, Ev>:
  [Raw{init(), 6n, "R"}, Raw{result("x"), 3n, "C"}, Prep{u("1")}, Raw{note(), 1n, "R"}, Raw{say("woke"), 2n, "R"}, Raw{result("woke"), 1n, "R"}]

# a wake beside a queued (sent, not started) message
def wake_queued() -> List<&2, Ev>:
  [Raw{init(), 6n, "R"}, Raw{result("x"), 3n, "C"}, Send{u("1")}, Raw{note(), 1n, "R"}, Raw{result("w"), 1n, "R"},
   Raw{lc("1", "started"), 2n, "R"}, Raw{result("y"), 1n, "R"}, Raw{lc("1", "completed"), 3n, "C"}]

# an old CLI: no lifecycles, the old rule (a running turn's result ends it)
def old() -> List<&2, Ev>:
  [Send{u("1")}, Raw{init.old(), 0n, "R"}, Raw{say("a"), 0n, "R"}, Raw{result("a"), 0n, "C"}]

# a failed command fails the thread when its lifecycle completes
def fails() -> List<&2, Ev>:
  [Send{u("1")}, Raw{lc("1", "started"), 2n, "R"}, Raw{init(), 2n, "R"}, Raw{"{\"type\":\"result\",\"is_error\":true,\"result\":\"API error\"}", 1n, "R"}, Raw{lc("1", "completed"), 4n, "F"}]

def lines.r(r: U.Lines & List<&2, String>) -> List<&2, String>:
  (_, ls) = r
  ls

# (read as the server reads its log: a big file overflows String.split)
def lines.k(bs: List<&2, U32>) -> List<&2, String>:
  lines.r(U.Lines.feed(U.Lines.new(), bs))

def file.done(r: File & Result<&1, &1, U32 & String, List<&2, U32>>) -> IO(List<&2, String>):
  (f, res) = r
  do IO<List<&2, String>>:
    File.close(f)
    return lines.k(Result.default(&1, &1, U32 & String, List<&2, U32>, res, Nil{}))

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

def totals(+name: String, +wakes: Nat, +ends: Nat, +sim: Sim) -> IO(Unit):
  do IO<Unit>:
    C.Check.str(name ++ ": wake Acts", Nat.show(sim.wakes(sim)), Nat.show(wakes))
    C.Check.str(name ++ ": turns ended", Nat.show(sim.ends(sim)), Nat.show(ends))

# the background shell's row: at work, ended when Claude's open list drops
# it (still owed), gone at its notification
def totals.bg(+name: String, +wakes: Nat, +ends: Nat, +a: Nat, +b: Nat, +want: String, +sim: Sim) -> IO(Unit):
  do IO<Unit>:
    totals(name, wakes, ends, sim)
    C.Check.str(name ++ ": rows l." ++ Nat.show(a) ++ "-" ++ Nat.show(b), rows.at(sim, a, b), want)

def store.back(m: Maybe<&2, M.Change>) -> Bool:
  match m:
    case Some{M.WorkSet{+t, +k, +r, +a}}:
      Bool.and(Bool.and(String.eq(t, "t1"), String.eq(k, "bg")), Bool.and(String.eq(r, "a\tx"), Nat.is_eq(a, 7n)))
    case _:
      False{}

# the id of the first message in a step's changes
def mid(cs: List<&2, M.Change>) -> String:
  match cs:
    case M.MessagePosted{_, +i, _, _, _} <> _:
      i
    case _ <> t:
      mid(t)
    case Nil{}:
      ""

def tb() -> J.Json:
  J.Json.obj([J.KV{"type", J.Str{"text"}}, J.KV{"text", J.Str{"x"}}])

def held() -> IO(Unit):
  do IO<Unit>:
    +c : CM.Cmds = CM.Cmds.accept(CM.Cmds.accept(CM.Cmds.new(), "a"), "b")
    +hs : List<&2, CM.Held> = CM.Held.push(CM.Held.push(Nil{}, "a", "", False{}), "b", "B", True{})
    C.Check.str("held: waits behind a message still being read", String.join(CM.Flush.out(CM.Held.flush(hs, c)), ","), "")
    C.Check.str("held: then both go, in order", String.join(CM.Flush.out(CM.Held.flush(CM.Held.ready(hs, "a", "A"), c)), ","), "A,B")
    C.Check.str("held: a stopped one is never written", String.join(CM.Flush.out(CM.Held.flush(CM.Held.ready(hs, "a", "A"), CM.Cmds.stop(c, Nil{}))), ","), "")
    C.Check.str("held: still queued is kept", String.join(CM.Flush.out(CM.Held.flush(CM.Held.ready(hs, "a", "A"), CM.Cmds.still(CM.Cmds.stop(c, Nil{}), ["b"]))), ","), "B")
    C.Check.str("uuid: v4 shaped", Nat.show(String.length(B.Hub.uuid(1790922525n, "t1/m1"))), "36")
    C.Check.str("uuid: version and variant", String.take(String.drop(B.Hub.uuid(1790922525n, "t1/m1"), 14n), 1n) ++ String.take(String.drop(B.Hub.uuid(1790922525n, "t1/m1"), 19n), 1n), "48")
    C.Check.str("uuid: differs by message", Bool.show(String.eq(B.Hub.uuid(1790922525n, "t1/m1"), B.Hub.uuid(1790922525n, "t1/m2"))), "False")
    C.Check.str("uuid: kept by the line", B.Claude.uuid.of(B.Claude.user.u("hi", "u-1")), "u-1")
    C.Check.str("version: 2.1.286 has lifecycles", Bool.show(CM.Cmds.lifecycle("2.1.286")), "True")
    C.Check.str("version: 2.2.0 has", Bool.show(CM.Cmds.lifecycle("2.2.0")), "True")
    C.Check.str("version: 2.1.200 has not", Bool.show(CM.Cmds.lifecycle("2.1.200")), "False")
    C.Check.str("ids: two text lines of one message differ", mid(B.Claude.block.k(0n, "t1", "m", 0n, 1n, 0n, "aaaaaaaa-1111", tb(), False{})) ++ "|" ++ mid(B.Claude.block.k(0n, "t1", "m", 0n, 1n, 0n, "bbbbbbbb-2222", tb(), False{})), "m:0:aaaaaaaa|m:0:bbbbbbbb")
    C.Check.str("ids: a line with no uuid keeps the old id", mid(B.Claude.block.k(0n, "t1", "m", 0n, 1n, 0n, "", tb(), False{})), "m:0")
    C.Check.str("WorkSet stored and read back", Bool.show(store.back(C2.Change.decode(C2.Change.encode(M.WorkSet{"t1", "bg", "a\tx", 7n})))), "True")

def api.show(cs: List<&2, M.Change>) -> String:
  match cs:
    case M.ActivityLogged{_, _, +tone, +kind, +summary, _} <> _:
      tone ++ "|" ++ kind ++ "|" ++ summary
    case _:
      ""

def api.line(+s: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(s), J.Null{})

# a Claude Code line for a model it cannot use
def api.bad() -> J.Json:
  api.line("{\"type\":\"assistant\",\"error\":\"invalid_request\",\"message\":{\"model\":\"<synthetic>\",\"content\":[{\"type\":\"text\",\"text\":\"There's an issue with the selected model (no-such-model-webhook-test)\"}]}}")

def api_errs() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("api error: logged as the turn's", api.show(B.Claude.api.err(True{}, "t1", 5n, 0n, api.bad())), "error|api|There's an issue with the selected model (no-such-model-webhook-test)")
    C.Check.str("api error: none once the turn is over", api.show(B.Claude.api.err(False{}, "t1", 5n, 0n, api.bad())), "")
    C.Check.str("api error: an ordinary reply logs none", api.show(B.Claude.api.err(True{}, "t1", 5n, 0n, api.line("{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"hi\"}]}}"))), "")
    C.Check.str("api error: no text says the code", api.show(B.Claude.api.err(True{}, "t1", 5n, 0n, api.line("{\"type\":\"assistant\",\"error\":\"server_error\",\"message\":{\"content\":[]}}"))), "error|api|server_error")

# Turn identity (hub.bend's Turn.marks, wnote.bend's Wn.cap): a turn is
# known by where it started, whatever user messages it holds

# a step stored as the server stores it (Hub.commit.w marks turn starts)
def marked(+s: M.State, +cs: List<&2, M.Change>) -> M.State:
  M.State.replay(B.Turn.marks(s, cs), s)

def ended.show(o: B.Out) -> String:
  match o:
    case B.TurnEnded{_, +st, _, +e, +r, +x}:
      st ++ "|" ++ e ++ "|" ++ r ++ "|" ++ x
    case _:
      ""

def ended.of(+s: M.State, +state: String) -> String:
  ended.show(B.Ended.turn("t1", state, "", WN.Wn.cap(s, "t1")))

# a turn a person started hit a usage limit and was cut off
def turns.first() -> List<&2, M.Change>:
  [M.ProjectCreated{"p1", "p", "/p", 1n}, M.ThreadCreated{"t1", "p1", "x", M.Local{}, "", "", "claude", "", 1n},
   M.SettingSet{"push.webhook.url", "https://example.com/h", 1n},
   M.MessagePosted{"t1", "m1", M.User{}, "go", 2n}, M.TurnChanged{"t1", M.Running{}, 2n},
   M.MessagePosted{"t1", "a1", M.Assistant{}, "an old reply", 3n},
   M.ActivityLogged{"t1", "e1", "error", "result", "Claude AI usage limit reached|1792800000", 3n}, M.TurnChanged{"t1", M.Interrupted{}, 3n}]

# then a wake (Wake.start: no message from the user) resumed it, and it
# was cut off again with no error and no reply of its own
def turns.wake() -> List<&2, M.Change>:
  [M.TurnChanged{"t1", M.Running{}, 5n}, M.ActivityLogged{"t1", "k1", "info", "wake", "woke", 5n}, M.TurnChanged{"t1", M.Interrupted{}, 6n}]

def turns.woke() -> M.State:
  marked(marked(M.State.empty(), turns.first()), turns.wake())

# a turn where the person queued a message while it ran (Hub.post.go logs
# it at once), and which then failed on a model error
def turns.queued() -> List<&2, M.Change>:
  [M.ProjectCreated{"p1", "p", "/p", 1n}, M.ThreadCreated{"t1", "p1", "x", M.Local{}, "", "", "claude", "", 1n},
   M.SettingSet{"push.webhook.url", "https://example.com/h", 1n},
   M.MessagePosted{"t1", "m0", M.User{}, "earlier", 1n}, M.TurnChanged{"t1", M.Running{}, 1n},
   M.ActivityLogged{"t1", "e0", "error", "result", "API Error: 500", 1n}, M.TurnChanged{"t1", M.Failed{}, 1n},
   M.MessagePosted{"t1", "m1", M.User{}, "go", 2n}, M.TurnChanged{"t1", M.Running{}, 2n},
   M.MessagePosted{"t1", "a1", M.Assistant{}, "There's an issue with the selected model (no-such-model)", 3n},
   M.MessagePosted{"t1", "m2", M.User{}, "and then this", 4n},
   M.ActivityLogged{"t1", "api1", "error", "api", "There's an issue with the selected model (no-such-model)", 5n}, M.TurnChanged{"t1", M.Interrupted{}, 5n}]

def turns.mark(+s: M.State) -> String:
  M.Setting.get(s, WN.Wn.from.key("t1"), "")

def turns() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("turns: a start is marked", turns.mark(marked(M.State.empty(), turns.first())), "@m1")
    C.Check.str("turns: no start, no mark", Nat.show(List.length(&2, M.Change, B.Turn.marks(M.State.empty(), [M.TurnChanged{"t1", M.Failed{}, 1n}]))), "1")
    C.Check.str("turns: the first turn's own end", ended.of(marked(M.State.empty(), turns.first()), "interrupted"), "interrupted|Claude AI usage limit reached|1792800000|limit|an old reply")
    C.Check.str("turns: a wake is marked at its start", turns.mark(turns.woke()), "@e1")
    C.Check.str("turns: a resumed wake is not the old limit", ended.of(turns.woke(), "interrupted"), "interrupted||interrupted|")
    C.Check.str("turns: a queued message keeps the model error", ended.of(marked(M.State.empty(), turns.queued()), "interrupted"), "interrupted|There's an issue with the selected model (no-such-model)|error|There's an issue with the selected model (no-such-model)")
    C.Check.str("turns: a queued message does not start a turn", turns.mark(marked(M.State.empty(), turns.queued())), "@m1")

# The vault (hub.bend's Secret.changes, Secret.outs): a turn that ends on
# an error holding a stored value carries it neither where it is stored
# nor out to the server (the webhook)

def vault.value() -> String:
  "sk-synthetic-7Qx9vWm2LpR4"

def vault.v() -> SEC.Vault:
  SEC.Sec.vault([vault.value()])

def vault.running() -> M.State:
  marked(M.State.empty(), [M.ProjectCreated{"p1", "p", "/p", 1n}, M.ThreadCreated{"t1", "p1", "x", M.Local{}, "", "", "claude", "", 1n},
    M.SettingSet{"push.webhook.url", "https://example.com/h", 1n},
    M.MessagePosted{"t1", "m1", M.User{}, "go", 2n}, M.TurnChanged{"t1", M.Running{}, 2n},
    M.MessagePosted{"t1", "a1", M.Assistant{}, "using " ++ vault.value(), 3n}])

def vault.end() -> B.Step:
  B.Ended.out(vault.running(), 4n, "t1", B.Hub.agent_gone.k(True{}, "t1", 4n, 1, "boom " ++ vault.value()))

def vault.cs.text(cs: List<&2, M.Change>) -> String:
  match cs:
    case Nil{}:
      ""
    case M.ActivityLogged{_, _, _, _, +summary, _} <> t:
      summary ++ "\n" ++ vault.cs.text(t)
    case _ <> t:
      vault.cs.text(t)

def vault.os.text(os: List<&2, B.Out>) -> String:
  match os:
    case Nil{}:
      ""
    case B.TurnEnded{_, _, _, +e, _, +x} <> t:
      e ++ "\n" ++ x ++ "\n" ++ vault.os.text(t)
    case _ <> t:
      vault.os.text(t)

def vault.stored.k(st: B.Step) -> String:
  match st:
    case B.Step{+cs, _}:
      vault.cs.text(B.Secret.changes(vault.v(), cs))

def vault.stored() -> String:
  vault.stored.k(vault.end())

def vault.raw.k(st: B.Step) -> String:
  match st:
    case B.Step{_, +os}:
      vault.os.text(os)

def vault.raw() -> String:
  vault.raw.k(vault.end())

def vault.sent.k(st: B.Step) -> String:
  match st:
    case B.Step{_, +os}:
      vault.os.text(B.Secret.outs(vault.v(), os))

def vault.sent() -> String:
  vault.sent.k(vault.end())

def vault() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("vault: the end read the value (the case covered)", Bool.show(String.contains(vault.raw(), vault.value())), "True")
    C.Check.str("vault: not stored", Bool.show(String.contains(vault.stored(), vault.value())), "False")
    C.Check.str("vault: not sent", Bool.show(String.contains(vault.sent(), vault.value())), "False")
    C.Check.str("vault: the rest of the error kept", Bool.show(String.contains(vault.sent(), "boom ")), "True")
    C.Check.str("vault: an empty vault leaves outs as they are", vault.os.text(B.Secret.outs(SEC.Sec.vault(Nil{}), [B.TurnEnded{"t1", "failed", "", "e", "error", "x"}])), "e\nx\n")

def main() -> IO(Unit):
  do IO<Unit>:
    +f1 : List<&2, String> <- file("test/fixtures/claude/bg-wake.jsonl")
    +f2 : List<&2, String> <- file("test/fixtures/claude/stop-steer.jsonl")
    C.Check.str("fixtures read", Nat.show(List.length(&2, String, f1)) ++ "," ++ Nat.show(List.length(&2, String, f2)), "31,67")
    run(bg_wake(), f1, sim0(), "bg-wake", s => totals.bg("bg-wake", 1n, 3n, 10n, 23n, "0/0,1/1,1/1,1/1,1/1,1/1,1/1,1/0,1/0,0/0,0/0,0/0,0/0,0/0", s))
    run(stop_steer(), f2, sim0(), "stop-steer", s => totals.bg("stop-steer", 1n, 5n, 41n, 67n,
      "0/0,1/1,1/1,1/1,1/1,1/1,1/1,1/1,1/1,1/1,1/1,1/1,1/1,1/1,1/1,1/1,1/1,1/1,1/0,1/0,0/0,0/0,0/0,0/0,0/0,0/0,0/0", s))
    run(stale(), Nil{}, sim0(), "live repro", s => IO.pure(Unit, Unit{}))
    run(stop_early(), Nil{}, sim0(), "stop before start", s => IO.pure(Unit, Unit{}))
    run(tomb_started(), Nil{}, sim0(), "stopped then started", s => totals("stopped then started", 2n, 2n, s))
    run(prep_wake(), Nil{}, sim0(), "prep and a wake", s => IO.pure(Unit, Unit{}))
    run(wake_queued(), Nil{}, sim0(), "wake and a queued message", s => IO.pure(Unit, Unit{}))
    run(old(), Nil{}, sim0(), "old CLI", s => IO.pure(Unit, Unit{}))
    run(fails(), Nil{}, sim0(), "a failed command", s => IO.pure(Unit, Unit{}))
    held()
    api_errs()
    turns()
    vault()
