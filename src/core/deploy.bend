import Base
import ./json.bend as J
import ./model.bend as M
import ./ship.bend as SH

# Safe deploy and restart (docs/deploy.md; the steps are hub.bend's Deploy.*).
# Restarting is fine: the hub resumes every cut turn. What must not be cut
# is a tool call in flight (a build, an edit) and a build under way (the
# machine-wide lock). A deploy waits as the private setting deploy.wait, one
# JSON line; just before the hub execs into the new build it becomes
# deploy.done, which the new process reads once at start.

def Deploy.wait.key() -> String:
  "deploy.wait"

def Deploy.done.key() -> String:
  "deploy.done"

# after this long the requester is told once what still blocks (seconds)
def Deploy.slow() -> Nat:
  1800n

# the line: who asked (a thread id, "" for the person or a script), the
# note, the staged package ("" when the files are in place), the version
# before, when asked, whether the requester was told it is slow, whether
# the build lock was held at the last look, (done) the threads cut, and the
# tool calls in flight at the last look (one thread id per call, comma-joined;
# the hub's in-memory list, Open.* below)
def Deploy.line.o(+by: String, +note: String, +stage: String, +from: String, +since: Nat, noted: Bool, locked: Bool, +cut: String, +open: String) -> String:
  J.Json.show(J.Json.obj([J.KV{"by", J.Str{by}}, J.KV{"note", J.Str{note}}, J.KV{"stage", J.Str{stage}}, J.KV{"from", J.Str{from}},
    J.KV{"since", J.Str{Nat.show(since)}}, J.KV{"noted", J.Flag{noted}}, J.KV{"locked", J.Flag{locked}}, J.KV{"cut", J.Str{cut}}, J.KV{"open", J.Str{open}}]))

def Deploy.line(+by: String, +note: String, +stage: String, +from: String, +since: Nat, noted: Bool, locked: Bool, +cut: String) -> String:
  Deploy.line.o(by, note, stage, from, since, noted, locked, cut, "")

def Deploy.obj(+line: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(line), J.Null{})

def Deploy.wait(+s: M.State) -> String:
  M.Setting.get(s, "deploy.wait", "")

def Deploy.done(+s: M.State) -> String:
  M.Setting.get(s, "deploy.done", "")

def Deploy.waiting(+s: M.State) -> Bool:
  Bool.not(String.is_empty(Deploy.wait(s)))

def Deploy.by(+line: String) -> String:
  J.Json.get_str(Deploy.obj(line), "by", "")

def Deploy.note(+line: String) -> String:
  J.Json.get_str(Deploy.obj(line), "note", "")

def Deploy.stage(+line: String) -> String:
  J.Json.get_str(Deploy.obj(line), "stage", "")

def Deploy.from(+line: String) -> String:
  J.Json.get_str(Deploy.obj(line), "from", "")

def Deploy.since(+line: String) -> Nat:
  M.Switch.num(J.Json.get_str(Deploy.obj(line), "since", "0"), 0n)

def Deploy.noted(+line: String) -> Bool:
  J.Json.get_bool(Deploy.obj(line), "noted", False{})

def Deploy.locked(+line: String) -> Bool:
  J.Json.get_bool(Deploy.obj(line), "locked", False{})

def Deploy.cut(+line: String) -> String:
  J.Json.get_str(Deploy.obj(line), "cut", "")

def Deploy.open(+line: String) -> String:
  J.Json.get_str(Deploy.obj(line), "open", "")

# whether a thread id is in a comma-joined list
def Deploy.has(ids: List<&2, String>, +id: String) -> Bool:
  match ids:
    case Nil{}:
      False{}
    case +x <> t:
      Bool.or(String.eq(x, id), Deploy.has(t, id))

def Deploy.in(+line: String, +id: String) -> Bool:
  Deploy.has(String.split(Deploy.cut(line), ','), id)

# Tool calls in flight
# --------------------
# The hub knows when a tool call starts and ends (hub.bend's ToolOpen and
# ToolShut, ToolsGone at a turn's end) and keeps the open ones in memory,
# never in the log: a list of "thread<TAB>call" entries (Open.*). A thread
# between two calls has none open, so a deploy goes then, however long its
# turn. The requester's own deploy_* call is never opened (Tools.opens,
# Open.mcp).

def Open.key(+th: String, +call: String) -> String:
  th ++ "\t" ++ call

def Open.has(xs: List<&2, String>, +key: String) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case +x <> t:
      Bool.or(String.eq(x, key), Open.has(t, key))

def Open.cons(has: Bool, +key: String, xs: List<&2, String>) -> List<&2, String>:
  match has:
    case True{}:
      xs
    case False{}:
      key <> xs

def Open.add.k(has: Bool, xs: List<&2, String>, +key: String) -> List<&2, String>:
  Open.cons(has, key, xs)

# a call opens (once: opening it again changes nothing)
def Open.add(+xs: List<&2, String>, +th: String, +call: String) -> List<&2, String>:
  Open.add.k(Open.has(xs, Open.key(th, call)), xs, Open.key(th, call))

def Open.keep(drop: Bool, +x: String, rest: List<&2, String>) -> List<&2, String>:
  match drop:
    case True{}:
      rest
    case False{}:
      x <> rest

# a call ends
def Open.shut(xs: List<&2, String>, +th: String, +call: String) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> t:
      Open.keep(String.eq(x, Open.key(th, call)), x, Open.shut(t, th, call))

# every call of a thread ends (its turn ended, its agent went); a request
# still on the MCP route stays open until its answer is written
def Open.gone(xs: List<&2, String>, +th: String) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> t:
      Open.keep(Bool.and(String.starts_with(x, th ++ "\t"), Bool.not(String.starts_with(x, th ++ "\tmcp:"))), x, Open.gone(t, th))

# Backplane's own MCP calls, every provider's: the hub sees each request on
# its MCP route from the moment it arrives until its answer is written
# (server.bend's Mcp.ask.raw), so it opens there and shuts there, under
# "mcp:<n>". The caller's own deploy_* call is never counted: a thread
# asking for a deploy, or how it goes, would wait for itself (laws
# deploy_mcp_*).
def Open.mcp.tool(+o: J.Json) -> Bool:
  Bool.and(String.eq(J.Json.get_str(o, "method", ""), "tools/call"), Bool.not(String.starts_with(J.Json.get_str(J.Json.get_obj(o, "params"), "name", ""), "deploy_")))

# whether an MCP request counts as a tool call in flight
def Open.mcp.counts(+body: String) -> Bool:
  Open.mcp.tool(Maybe.default(&2, J.Json, J.Json.parse(body), J.Null{}))

def Open.mcp.key(+call: String) -> String:
  "mcp:" ++ call

def Open.mcp.k(go: Bool, +xs: List<&2, String>, +th: String, +call: String) -> List<&2, String>:
  match go:
    case True{}:
      Open.add(xs, th, Open.mcp.key(call))
    case False{}:
      xs

# a request on the MCP route opens (when it counts)
def Open.mcp(+xs: List<&2, String>, +th: String, +call: String, +body: String) -> List<&2, String>:
  Open.mcp.k(Open.mcp.counts(body), xs, th, call)

# its answer was written
def Open.mcp.shut(xs: List<&2, String>, +th: String, +call: String) -> List<&2, String>:
  Open.shut(xs, th, Open.mcp.key(call))

def Open.count(xs: List<&2, String>) -> Nat:
  List.length(&2, String, xs)

def Open.threads.go(xs: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> t:
      Maybe.default(&2, String, List.head(&2, String, String.split(x, '\t')), "") <> Open.threads.go(t)

# the threads of the open calls, one id per call, comma-joined
def Open.threads(xs: List<&2, String>) -> String:
  String.join(Open.threads.go(xs), ",")

def Deploy.count.k(+open: String) -> Nat:
  Bool.pick(Nat, String.is_empty(open), 0n, List.length(&2, String, String.split(open, ',')))

# the calls the line recorded at the last look
def Deploy.count(+line: String) -> Nat:
  Deploy.count.k(Deploy.open(line))

def Deploy.mark(go: Bool, +id: String, rest: List<&2, String>) -> List<&2, String>:
  match go:
    case True{}:
      id <> rest
    case False{}:
      rest

# the live threads running a turn
def Deploy.ids(ts: List<&2, M.Thread>, +s: M.State) -> List<&2, String>:
  match ts:
    case Nil{}:
      Nil{}
    case M.Thread{+id, _, _, _, _, _, _, ar, _, _, _, _, _, _, _, _, _, _, _, _, de} <> t:
      Deploy.mark(Bool.and(Bool.not(Bool.or(ar, de)), M.State.running.k(M.State.thread.get(s, id))), id, Deploy.ids(t, s))

# the threads a restart now would cut
def Deploy.running(+s: M.State) -> String:
  match s:
    case M.State{_, +ts, _, _, _, _, _, _, _, _, _}:
      String.join(Deploy.ids(Map.values(&2, M.Thread, ts), s), ",")

# Words
# -----

def Deploy.calls(+n: Nat) -> String:
  Nat.show(n) ++ Bool.pick(String, Nat.is_eq(n, 1n), " tool call", " tool calls")

# what the restart waits for: n tool calls running, the build lock held
def Deploy.blocking(+n: Nat, +locked: Bool) -> String:
  Bool.pick(String, Nat.is_eq(n, 0n), Bool.pick(String, locked, "waiting for the build lock", "waiting for a safe point"),
    "waiting for " ++ Deploy.calls(n) ++ Bool.pick(String, locked, " and the build lock", ""))

def Deploy.blocking.line(+n: Nat, +locked: Bool) -> String:
  Deploy.blocking(n, locked)

def Deploy.row.k(+line: String) -> String:
  Bool.pick(String, String.is_empty(line), "", "Restart pending · " ++ Deploy.blocking.line(Deploy.count(line), Deploy.locked(line)))

# the plane's and Settings' line while a deploy waits ("" when none does)
def Deploy.row(+s: M.State) -> String:
  Deploy.row.k(Deploy.wait(s))

# who asked, for a note: " by <thread> (note)" or ""
def Deploy.who(+by: String, +note: String) -> String:
  Bool.pick(String, String.is_empty(by), "", " by " ++ by) ++ Bool.pick(String, String.is_empty(note), "", " (" ++ note ++ ")")

# said to the requester once, after 30 minutes
def Deploy.slow.text(+n: Nat, locked: Bool) -> String:
  "[Backplane] Your deploy_restart has waited a long time and still " ++ Deploy.blocking.line(n, locked) ++ ". It restarts as soon as that clears; deploy_cancel stops it."

# said after the restart
def Deploy.after.text(+line: String, +to: String) -> String:
  "Backplane restarted for a deploy" ++ Deploy.who(Deploy.by(line), Deploy.note(line)) ++ ": " ++ Deploy.from(line) ++ " → " ++ to ++ "."

def Deploy.suffix.k(+line: String, +to: String) -> String:
  Bool.pick(String, String.is_empty(line), "", "\n\n" ++ Deploy.after.text(line, to) ++ " Your turn was resumed.")

# appended to the continue message of a turn the restart cut ("" when
# there was no deploy)
def Deploy.suffix(+s: M.State, +to: String) -> String:
  Deploy.suffix.k(Deploy.done(s), to)

# the line again with the noted and locked flags set
def Deploy.with(+line: String, noted: Bool, locked: Bool, +open: String) -> String:
  Deploy.line.o(Deploy.by(line), Deploy.note(line), Deploy.stage(line), Deploy.from(line), Deploy.since(line), noted, locked, Deploy.cut(line), open)

def Deploy.status.k(+line: String, main: J.Json) -> J.Json:
  J.Json.obj([J.KV{"waiting", J.Flag{Bool.not(String.is_empty(line))}}, J.KV{"by", J.Str{Deploy.by(line)}}, J.KV{"note", J.Str{Deploy.note(line)}},
    J.KV{"since", J.Str{Nat.show(Deploy.since(line))}}, J.KV{"inflight", J.Num{Nat.show(Deploy.count(line))}},
    J.KV{"threads", J.Str{Deploy.open(line)}}, J.KV{"buildLock", J.Flag{Deploy.locked(line)}}, J.KV{"text", J.Str{Deploy.row.k(line)}}, J.KV{"main", main}])

# deploy_main's part (core/ship.bend)
def Deploy.main(+s: M.State) -> J.Json:
  SH.Ship.status(SH.Ship.read(M.Setting.get(s, "deploy.ship", "")), SH.Ship.follow.on(M.Setting.get(s, "deploy.follow", "")), M.Setting.get(s, "deploy.repo", ""))

# what deploy.status and the deploy_status tool answer
def Deploy.status(+s: M.State) -> J.Json:
  Deploy.status.k(Deploy.wait(s), Deploy.main(s))
