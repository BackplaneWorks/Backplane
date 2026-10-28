import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/json.bend as J
import ../src/core/hub.bend as H
import ../src/core/usage.bend as UZ

# Token stats (usage.bend's Ts): Claude's result lines as measured
# 2026-10-24 (claude 2.1.286, two turns in one process): duration_api_ms
# and modelUsage run on across the session's turns, duration_ms and usage
# are the turn's own. A turn's speed is its share of output over its share
# of API time; a session already running when first seen is not told.

def js(+s: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(s), J.Null{})

def result(+key: String, +cost: String, +wall: String, +api: String, +o: String, +mo: String) -> J.Json:
  js("{\"type\":\"result\",\"uuid\":\"" ++ key ++ "\",\"duration_ms\":" ++ wall ++ ",\"duration_api_ms\":" ++ api ++ ",\"num_turns\":1,\"total_cost_usd\":" ++ cost ++
    ",\"usage\":{\"input_tokens\":10,\"cache_creation_input_tokens\":0,\"cache_read_input_tokens\":13803,\"output_tokens\":" ++ o ++ "}" ++
    ",\"modelUsage\":{\"claude-haiku-4-5-20251001\":{\"inputTokens\":910,\"outputTokens\":" ++ mo ++ ",\"cacheReadInputTokens\":13803,\"cacheCreationInputTokens\":0,\"webSearchRequests\":0,\"costUSD\":" ++ cost ++ ",\"contextWindow\":200000}}}")

def log() -> List<&2, M.Change>:
  [M.ProjectCreated{"p1", "demo", "/tmp/demo", 1n},
   M.ThreadCreated{"t1", "p1", "Claude work", M.Local{}, "", "", "claude", "claude-haiku-4-5-20251001", 1n},
   M.ThreadCreated{"t2", "p1", "Codex work", M.Local{}, "", "", "codex", "gpt-6.1-sol", 1n},
   M.SessionSet{"t2", "s1", 2n}]

def then(+st: M.State, cs: List<&2, M.Change>) -> M.State:
  M.State.replay(cs, st)

def cl(+st: M.State, +o: J.Json) -> M.State:
  then(st, H.Usage.result(st, "t1", 1792100000n, o))

def rows(rs: List<&2, UZ.TsRow>) -> String:
  match rs:
    case Nil{}:
      ""
    case +r <> t:
      "  " ++ UZ.TsRow.label(r) ++ ": " ++ UZ.TsRow.value(r) ++ "\n" ++ rows(t)

def show(ss: List<&2, UZ.TsSec>) -> String:
  match ss:
    case Nil{}:
      ""
    case +s <> t:
      UZ.TsSec.title(s) ++ "\n" ++ rows(UZ.TsSec.rows(s)) ++ show(t)

def main() -> IO(Unit):
  do IO<Unit>:
    +s0 : M.State = M.State.replay(log(), M.State.empty())
    C.Check.str("off unless turned on", Bool.show(UZ.Ts.on(s0)), "False")
    C.Check.str("on", Bool.show(UZ.Ts.on(then(s0, [M.SettingSet{"ui.tokstats", "on", 3n}]))), "True")
    +s1 : M.State = cl(then(s0, List.append(&2, M.Change, H.Turn.claude.start(False{}, "t1", "sess1", 2n), H.Speed.start(s0, False{}, "t1", "sess1", 2n))),
      result("r1", "0.0180473", "1276", "1955", "52", "67"))
    C.Check.str("first turn of a session seen to start: told from 0", M.Setting.get(s1, "turn.speed.t1", ""), "1955/67/1955/67/1276/1955/67/1")
    +s2 : M.State = cl(s1, result("r2", "0.0225717", "1383", "3321", "133", "200"))
    C.Check.str("second turn: the rise", M.Setting.get(s2, "turn.speed.t1", ""), "3321/200/1366/133/1383/3321/200/1")
    C.Check.str("a line again changes nothing", Nat.show(List.length(&2, M.Change, H.Usage.result(s2, "t1", 9n, result("r2", "0.0225717", "1383", "3321", "133", "200")))), "0")
    C.Check.str("last turn", M.Setting.get(s2, "turn.last.t1", ""), "10/13803/0/133/0/4524400/1792100000/irwoc/claude|claude-haiku-4-5-20251001")
    C.Check.str("models", M.Setting.get(s2, "turn.models.t1", ""), "claude-haiku-4-5-20251001|910|200|13803|0|22571700|0|200000")
    +n1 : M.State = then(s2, H.Speed.start(s2, False{}, "t1", "sess2", 4n))
    C.Check.str("a new session: totals from 0, the last turn kept", M.Setting.get(n1, "turn.speed.t1", ""), "0/0/1366/133/1383/3321/200/1")
    C.Check.str("same session: nothing", Nat.show(List.length(&2, M.Change, H.Speed.start(s2, True{}, "t1", "sess1", 4n))), "0")
    +u0 : M.State = cl(s0, result("u1", "0.5", "9000", "50000", "40", "4000"))
    C.Check.str("a session already running: not told", M.Setting.get(u0, "turn.speed.t1", ""), "50000/4000/0/0/9000/0/0/1")
    C.Check.str("a default model's turn is the widest model's, not a helper listed first",
      H.Turn.main(H.Usage.fields(J.Json.get_obj(js("{\"modelUsage\":{\"claude-haiku-4-5\":{\"contextWindow\":200000},\"claude-opus-5-5[1m]\":{\"contextWindow\":1000000}}}"), "modelUsage")), "", 0n), "claude-opus-5-5[1m]")
    C.Check.str("numbers", String.join([UZ.Ts.num(0n), UZ.Ts.num(999n), UZ.Ts.num(1000n), UZ.Ts.num(1234567n), UZ.Ts.num(1000000000n)], " "), "0 999 1,000 1,234,567 1,000,000,000")
    C.Check.str("rate", UZ.Ts.rate(133n, 1366n), "97.3 tok/s")
    C.Check.str("a turn not told shows no speed", show([UZ.Ts.last(M.Setting.get(u0, "turn.last.t1", ""), M.Setting.get(u0, "turn.speed.t1", ""))]),
      "Last turn\n  Model: claude · claude-haiku-4-5-20251001\n  Input: 10\n  Cache read: 13,803\n  Cache write: 0\n  Output: 40\n  Cost: not reported\n  Turn time: 9.0 s\n")
    C.Check.str("secs", UZ.Ts.secs(1366n) ++ " " ++ UZ.Ts.secs(125000n), "1.3 s 2m 05s")
    +c2 : M.State = then(s2, [M.UsageSet{"t1", 62000n, 200000n, 5n}])
    C.Check.str("chip: the thread's tokens, their cost and the context held", UZ.Ts.chip(c2, "t1"), "28k tok · $0.0225 · 31%")
    C.Check.str("chip: before a context reading, the thread's tokens and cost", UZ.Ts.chip(s2, "t1"), "28k tok · $0.0225")
    C.Check.str("chip: nothing yet", UZ.Ts.chip(s0, "t2"), "")
    # counts round to the nearest, a tenth while short (5,950 read "5k")
    C.Check.str("tokens rounded", String.join([UZ.Use.tokens(950n), UZ.Use.tokens(1000n), UZ.Use.tokens(5950n), UZ.Use.tokens(5949n), UZ.Use.tokens(9949n), UZ.Use.tokens(9950n),
      UZ.Use.tokens(62400n), UZ.Use.tokens(999499n), UZ.Use.tokens(999500n), UZ.Use.tokens(1250000n), UZ.Use.tokens(9950000n), UZ.Use.tokens(34400000n)], " "),
      "950 1k 6k 5.9k 9.9k 10k 62k 999k 1M 1.3M 10M 34M")
    C.Check.str("panel, claude", show(UZ.Ts.sections(c2, "t1", "claude")),
      "Context\n  Held now: 62,000 of 200,000 (31%)\n  Room left: 138,000\n" ++
      "Last turn\n  Model: claude · claude-haiku-4-5-20251001\n  Input: 10\n  Cache read: 13,803\n  Cache write: 0\n  Output: 133\n  Cost: $0.0045\n" ++
      "  Output speed: 97.3 tok/s\n  Model time: 1.3 s over 1 step\n  Turn time: 1.3 s\n" ++
      "This thread\n  Turns: 2\n  Input: 20\n  Cache read: 27,606\n  Cache write: 0\n  Output: 185\n  Cache hits: 99% of input\n  Cost: $0.0225\n  Average speed: 60.2 tok/s\n" ++
      "This session by model\n  claude-haiku-4-5-20251001: $0.0225\n  : 910 in · 200 out · 14k cache read · 0 cache write\n" ++
      "\n  : Speed is output tokens over the time Claude's model calls took, as Claude reports it: the wait for the first token is in it (so short replies read slow), tool runs are not.\n")
    C.Check.str("panel, a provider that reports nothing yet", show(UZ.Ts.sections(s0, "t2", "codex")),
      "Context\n  Held now: not reported by codex\nLast turn\n  : no turn counted yet\nThis thread\n  : no turns counted yet\n")
