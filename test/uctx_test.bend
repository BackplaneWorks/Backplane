import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/codec.bend as K
import ../src/core/json.bend as J
import ../src/core/hub.bend as H
import ../src/core/mcp.bend as MC
import ../src/core/uber.bend as UB
import ../src/core/storedb.bend as SDB
import ../src/core/uctx.bend as U

# The uberbot's infinite context (core/uctx.bend, hub.bend's Uc.*): the
# tree, the view, compaction order, the step pass with the setting on and
# off, and CtxNoted in the log, on the wire and in the store.

# Kit (b1, thread t1) is the uberbot; Pip (b2, t2) is another bot
def demo(+on: Bool) -> M.State:
  +s = M.State.apply(UB.Uber.demo(), M.SettingSet{"uber.bot", "b1", 1n})
  Bool.pick(M.State, on, M.State.apply(s, M.SettingSet{"uber.ctx", "on", 2n}), s)

def refs(ns: List<&2, U.UNode>) -> String:
  String.join(U.Uctx.refs(ns), ",")

# every node of a full tree over n messages, as written
def all.lv(c: Nat, +l: Nat, +i: Nat, +m: Map<&2, Bool>) -> Map<&2, Bool>:
  match c:
    case 0n: m
    case 1n+k: all.lv(k, l, Nat.add(i, 1n), Map.set(&2, Bool, m, U.Uctx.key(l, i), True{}))

def all.go(+l: Nat, +n: Nat, +m: Map<&2, Bool>) -> Map<&2, Bool>:
  match l:
    case 0n: all.lv(n, 0n, 0n, m)
    case 1n+k: all.go(k, n, all.lv(Nat.div(n, U.Uctx.pow(1n+k)), 1n+k, 0n, m))

def all(+n: Nat) -> Map<&2, Bool>:
  all.go(10n, n, Map.new(&2, Bool))

def msg(+who: String, +size: Nat) -> U.UMsg:
  U.UMsg{who, size}

def out.name(o: H.Out) -> String:
  match o:
    case H.AgentStop{_}: "stop"
    case H.AgentStart{_, _, _, _, +se, _, _}: "start(" ++ se ++ ")"
    case H.AgentSend{_, _}: "send"
    case H.CodexTurn{_, _, _, +se, _, _}: "codex(" ++ se ++ ")"
    case H.Git{_, _, +job, _, _, _, _}: "git:" ++ job
    case _: "other"

def outs(os: List<&2, H.Out>) -> String:
  match os:
    case Nil{}: ""
    case o <> Nil{}: out.name(o)
    case o <> t: out.name(o) ++ "," ++ outs(t)

def step.outs(st: H.Step) -> String:
  match st:
    case H.Step{_, os}: outs(os)

def step.n(st: H.Step) -> Nat:
  List.length(&2, M.Change, H.Queue.changes(st))

def turn(+th: String) -> H.Step:
  H.Step{[M.TurnChanged{th, M.Completed{}, 50n}], [H.AgentStart{th, "/w", "claude", "", "sess", M.FullAccess{}, M.DefaultMode{}}, H.AgentSend{th, "hi"}]}

def round(c: M.Change) -> String:
  J.Json.show(K.Change.encode(Maybe.default(&2, M.Change, K.Change.decode(K.Change.encode(c)), M.QueueTaken{"x", 0n})))

def main() -> IO(Unit):
  do IO<Unit>:
    +off : M.State = demo(False{})
    +on : M.State = demo(True{})
    # the setting and the thread
    C.Check.str("off: no thread runs with it", Bool.show(U.Uctx.on(off, "t1")), "False")
    C.Check.str("on: the uberbot's thread", Bool.show(U.Uctx.on(on, "t1")), "True")
    C.Check.str("on: not another bot's", Bool.show(U.Uctx.on(on, "t2")), "False")
    C.Check.str("the uberbot's thread", U.Uctx.thread(on), "t1")
    # the tree
    C.Check.str("roots of 13", refs(U.Uctx.roots(13n)), "0+8,8+4,12+1")
    C.Check.str("roots of 0", refs(U.Uctx.roots(0n)), "")
    C.Check.str("roots of 64", refs(U.Uctx.roots(64n)), "0+64")
    C.Check.str("level of 8", Nat.show(U.Uctx.level(8n)), "3")
    C.Check.str("a node", Bool.show(U.Uctx.part.ok(8n, 4n, 13n)), "True")
    C.Check.str("not a power of two", Bool.show(U.Uctx.part.ok(0n, 3n, 13n)), "False")
    C.Check.str("not aligned", Bool.show(U.Uctx.part.ok(2n, 4n, 13n)), "False")
    C.Check.str("past the end", Bool.show(U.Uctx.part.ok(8n, 8n, 13n)), "False")
    # the view
    C.Check.str("nothing written: every message a line", Nat.show(List.length(&2, U.UNode, U.Uctx.view(20n, Map.new(&2, Bool)))), "20")
    C.Check.str("full tree of 32", refs(U.Uctx.view(32n, all(32n))),
      "0+2,2+2,4+2,6+2,8+2,10+2,12+2,14+2,16+1,17+1,18+1,19+1,20+1,21+1,22+1,23+1,24+1,25+1,26+1,27+1,28+1,29+1,30+1,31+1")
    C.Check.str("full tree of 64: 4 + 16 + 16 lines", Nat.show(List.length(&2, U.UNode, U.Uctx.view(64n, all(64n)))), "36")
    C.Check.str("full tree of 64 starts coarse", refs(List.take(&2, U.UNode, U.Uctx.view(64n, all(64n)), 5n)), "0+4,4+4,8+4,12+4,16+2")
    C.Check.str("within an epoch the older part stays", refs(List.take(&2, U.UNode, U.Uctx.view(70n, all(64n)), 36n)), refs(U.Uctx.view(64n, all(64n))))
    C.Check.str("the epoch's new messages are leaves", refs(List.drop(&2, U.UNode, U.Uctx.view(70n, all(64n)), 36n)), "64+1,65+1,66+1,67+1,68+1,69+1")
    C.Check.str("1000 messages: about per() lines a level", Nat.show(List.length(&2, U.UNode, U.Uctx.view(1000n, all(1000n)))), "104")
    # compaction order
    C.Check.str("due: leaves first, then their merge", refs(U.Uctx.due(4n, 8n, Map.new(&2, Bool))), "0+1,1+1,2+1,3+1")
    C.Check.str("due: merges once halves are written", refs(U.Uctx.due(4n, 8n, all.lv(4n, 0n, 0n, Map.new(&2, Bool)))), "0+2,2+2")
    C.Check.str("due: at most m", Nat.show(List.length(&2, U.UNode, U.Uctx.due(100n, 8n, Map.new(&2, Bool)))), "8")
    C.Check.str("due: nothing on a full tree", refs(U.Uctx.due(64n, 8n, all(64n))), "")
    C.Check.str("fits: short leaves are their own lines", Nat.show(List.length(&2, String, Map.keys(&2, Bool, U.Uctx.fit([msg("user", 10n), msg("you", 900n), msg("tool", 20n)], 0n, Map.new(&2, Bool))))), "2")
    # the turn's own messages
    C.Check.str("own: trailing person's messages", Nat.show(U.Uctx.own([msg("you", 1n), msg("user", 1n), msg("user", 1n)])), "2")
    C.Check.str("own: none after a reply", Nat.show(U.Uctx.own([msg("user", 1n), msg("you", 1n)])), "0")
    # rows read
    C.Check.str("msgs read", Nat.show(List.length(&2, U.UMsg, U.Uctx.msgs([["msg", "user", "12"], ["act", "tool", "40"], ["msg", "assistant", "3"]]))), "3")
    C.Check.str("who", U.Uctx.who("act", "tool") ++ U.Uctx.who("msg", "user") ++ U.Uctx.who("msg", "assistant"), "tooluseryou")
    # text
    C.Check.str("flat", U.Uctx.flat("a\nb\r\nc"), "a b  c")
    C.Check.str("said: short", U.Uctx.said("user", "hello\nthere"), "user: hello there")
    C.Check.str("said: long is cut", Nat.show(String.length(U.Uctx.said("user", String.repeat("x", 2000n)))), "510")
    C.Check.str("block", U.Uctx.block([U.UNode{1n, 0n}, U.UNode{0n, 2n}], Map.set(&2, String, Map.set(&2, String, Map.new(&2, String), "1/0", "user: a; you: b"), "0/2", "tool: ls"), 3n),
      U.Uctx.doc() ++ "\n<chat>\n0+2|user: a; you: b\n2+1|tool: ls\n</chat>")
    C.Check.str("a line not written yet", U.Uctx.got(Map.new(&2, String), "1/0"), "(not summarized yet: zoom it)")
    +big : List<&2, String> = String.lines(String.repeat(String.repeat("y", 999n) ++ "\n", 300n))
    C.Check.str("bound: oldest lines left out", Nat.show(List.length(&2, String, U.Uctx.bound(big, True{}, U.Uctx.size(big, 0n)))), "128")
    C.Check.str("fts", U.Uctx.fts("  zoom   the \"tree\" "), "\"zoom\" \"the\" \"tree\"")
    C.Check.str("fts empty", U.Uctx.fts("   "), "")
    C.Check.str("whole: a page", U.Uctx.whole(5n, "user", "Mon", "hello", 0n), "5+1|user (Mon): hello")
    C.Check.str("whole: more", Bool.show(String.contains(U.Uctx.whole(5n, "user", "Mon", String.repeat("z", 20000n), 0n), "at 16000")), "True")
    C.Check.str("halves keys", J.Json.show(J.Json.arr(U.Uctx.halves.keys(8n, 8n))), "[\"2/2\",\"2/3\"]")
    C.Check.str("task: merge", String.take(U.Uctx.task(2n, 1n, "x"), 32n), "Merge lines 4+2 and 6+2 into one")
    C.Check.str("line: cut to the limit", Nat.show(String.length(U.Uctx.line(String.repeat("w", 900n)))), "512")
    C.Check.str("short", Bool.show(U.Uctx.short("a line")) ++ Bool.show(U.Uctx.short("  ")), "TrueFalse")
    # the step pass: off is the identity
    C.Check.str("off: outs unchanged", step.outs(H.Uc.after(off, 60n, turn("t1"))), "start(sess),send")
    C.Check.str("off: changes unchanged", Nat.show(step.n(H.Uc.after(off, 60n, turn("t1")))), "1")
    # on: a fresh session, and a compaction run at the turn's end
    C.Check.str("on: fresh Claude process", step.outs(H.Uc.after(on, 60n, turn("t1"))), "stop,start(),send,git:uctx")
    C.Check.str("on: busy kept", Nat.show(step.n(H.Uc.after(on, 60n, turn("t1")))), "2")
    C.Check.str("on: other threads untouched", step.outs(H.Uc.after(on, 60n, turn("t2"))), "start(sess),send")
    C.Check.str("on: codex starts fresh", step.outs(H.Uc.after(on, 60n, H.Step{Nil{}, [H.CodexTurn{"t1", "/w", "", "sess", M.FullAccess{}, "hi"}]})), "codex()")
    +busy : M.State = M.State.apply(on, M.SettingSet{U.Uctx.busy.key("t1"), "50", 50n})
    C.Check.str("on: no second run while one is live", step.outs(H.Uc.after(busy, 60n, turn("t1"))), "stop,start(),send")
    C.Check.str("on: a stale run frees its place", step.outs(H.Uc.after(busy, 5000n, turn("t1"))), "stop,start(),send,git:uctx")
    # the compaction run's requests
    C.Check.str("node kept", Nat.show(step.n(H.Uc.node(on, 70n, 0, "1", J.Json.obj([J.KV{"thread", J.Str{"t1"}}, J.KV{"l", J.Num{"1"}}, J.KV{"i", J.Num{"0"}}, J.KV{"text", J.Str{"user: a"}}])))), "1")
    C.Check.str("node refused while off", Nat.show(step.n(H.Uc.node(off, 70n, 0, "1", J.Json.obj([J.KV{"thread", J.Str{"t1"}}, J.KV{"l", J.Num{"1"}}, J.KV{"i", J.Num{"0"}}, J.KV{"text", J.Str{"user: a"}}])))), "0")
    C.Check.str("start kept once", U.Uctx.start(M.State.replay(H.Queue.changes(H.Uc.start.set(on, 70n, 0, "1", J.Json.obj([J.KV{"thread", J.Str{"t1"}}, J.KV{"ord", J.Str{"42"}}]))), on), "t1"), "42")
    +started : M.State = M.State.apply(on, M.SettingSet{U.Uctx.start.key("t1"), "42", 3n})
    C.Check.str("start not moved", Nat.show(step.n(H.Uc.start.set(started, 70n, 0, "1", J.Json.obj([J.KV{"thread", J.Str{"t1"}}, J.KV{"ord", J.Str{"7"}}])))), "0")
    C.Check.str("done with more starts another", step.outs(H.Uc.run.done(on, 70n, 0, "1", J.Json.obj([J.KV{"thread", J.Str{"t1"}}, J.KV{"more", J.Str{"1"}}]))), "other,git:uctx")
    C.Check.str("done frees the place", Bool.show(U.Uctx.busy(M.State.replay(H.Queue.changes(H.Uc.run.done(busy, 70n, 0, "1", J.Json.obj([J.KV{"thread", J.Str{"t1"}}, J.KV{"more", J.Str{"0"}}]))), busy), "t1", 70n)), "False")
    # the tools
    C.Check.str("tools: none while off", Bool.show(String.contains(J.Json.show(MC.Mcp.tools.for(off, "t1")), "memory_zoom")), "False")
    C.Check.str("tools: zoom and search while on", Bool.show(Bool.and(String.contains(J.Json.show(MC.Mcp.tools.for(on, "t1")), "memory_zoom"), String.contains(J.Json.show(MC.Mcp.tools.for(on, "t1")), "memory_search"))), "True")
    C.Check.str("tools: not for another bot", Bool.show(String.contains(J.Json.show(MC.Mcp.tools.for(on, "t2")), "memory_zoom")), "False")
    # CtxNoted: the log, the wire, the store, the fold
    C.Check.str("CtxNoted round trip", round(M.CtxNoted{"t1", 2n, 3n, "user: a", 9n}), "{\"$\":\"CtxNoted\",\"thread\":\"t1\",\"l\":2,\"i\":3,\"text\":\"user: a\",\"at\":9}")
    C.Check.str("CtxNoted on the wire: no text", J.Json.show(K.Change.wire(M.CtxNoted{"t1", 2n, 3n, "secret", 9n})), "{\"$\":\"CtxNoted\",\"at\":9}")
    C.Check.str("CtxNoted commit line: no text", Bool.show(String.contains(H.Commit.line(M.CtxNoted{"t1", 2n, 3n, "secret", 9n}, "disk"), "secret")), "False")
    C.Check.str("CtxNoted is a store fact", String.join(SDB.Sql.of(Maybe.default(&2, SDB.Op, List.head(&2, SDB.Op, SDB.F.other(on, 3n, M.CtxNoted{"t1", 2n, 3n, "x", 9n})), SDB.Del{"", Nil{}})), "|"),
      "INSERT INTO ctx(k, thread, l, i, text, at) VALUES (substr(?, 2), substr(?, 2), substr(?, 2), substr(?, 2), substr(?, 2), substr(?, 2)) ON CONFLICT DO NOTHING|vt1/2/3|vt1|v2|v3|vx|v9")
