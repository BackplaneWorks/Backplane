import Base
import ./check.bend as C
import ../src/core/allowance.bend as A
import ../src/core/json.bend as J
import ../src/core/usage.bend as U

# These cases also run through the emitted JavaScript in allowance_test.mjs.
type Case is Data:
  Case{label: String, got: String, want: String}

def window(+name: String, +scope: String, used: J.Json, reset: J.Json) -> J.Json:
  J.Json.obj([J.KV{"name", J.Str{name}}, J.KV{"scope", J.Str{scope}},
    J.KV{"used_permille", used}, J.KV{"resets_at", reset},
    J.KV{"duration_minutes", J.Num{"300"}}, J.KV{"shared", J.Flag{String.eq(scope, "shared")}}])

def win(+scope: String, used: String) -> J.Json:
  window(scope ++ " window", scope, J.Num{used}, J.Num{"1792103600"})

def snapshot(+provider: String, state: String, at: J.Json, ws: List<&2, J.Json>, more: List<&2, J.KV>) -> String:
  J.Json.show(J.Json.obj(List.append(&2, J.KV,
    [J.KV{"provider", J.Str{provider}}, J.KV{"state", J.Str{state}}, J.KV{"at", at},
     J.KV{"auth", J.Str{"chatgpt"}}, J.KV{"source", J.Str{"provider fixture"}}, J.KV{"windows", J.Json.arr(ws)}], more)))

def good(provider: String, ws: List<&2, J.Json>) -> String:
  snapshot(provider, "ok", J.Num{"1792100000"}, ws, Nil{})

def chip(snapshot: String) -> String:
  A.Allowance.chip(snapshot, "codex", "gpt-6.1-sol", 1792100000n)

def text(rs: List<&2, U.URow>) -> String:
  match rs:
    case Nil{}: ""
    case +r <> t: U.URow.label(r) ++ " | " ++ U.URow.note(r) ++ " [" ++ U.URow.act(r) ++ "]\n" ++ text(t)

def rows(snapshot: String, provider: String, model: String) -> String:
  text(A.Allowance.rows(snapshot, provider, model, 1792100000n))

def has(label: String, s: String, part: String) -> Case:
  Case{label, Bool.show(String.contains(s, part)), "True"}

def lacks(label: String, s: String, part: String) -> Case:
  Case{label, Bool.show(String.contains(s, part)), "False"}

def chips() -> List<&2, Case>:
  +one = good("codex", [win("shared", "280")])
  +multi = good("codex", [win("account-a", "280"), win("account-b", "900")])
  [Case{"consumed 28 percent means 72 left", chip(one), "72% left · shared"},
   Case{"round remaining down", chip(good("codex", [win("shared", "281")])), "71% left · shared"},
   Case{"reported zero consumption is full remaining", chip(good("codex", [win("shared", "0")])), "100% left · shared"},
   Case{"reported exhaustion is zero remaining", chip(good("codex", [win("shared", "1000")])), "0% left · shared"},
   Case{"same pool uses limiting time window", chip(good("codex", [win("shared", "280"), win("shared", "900")])), "10% left · shared"},
   Case{"multiple pools never choose a bucket", chip(multi), "Shared limits"},
   Case{"reversing pools preserves summary", chip(good("codex", [win("account-b", "900"), win("account-a", "280")])), "Shared limits"},
   Case{"Codex does not map selected model", A.Allowance.chip(multi, "codex", "account-a", 1792100000n), "Shared limits"},
   Case{"unknown provider", A.Allowance.chip(one, "pi", "", 1792100000n), "Limits unknown"},
   Case{"provider mismatch", A.Allowance.chip(one, "claude", "opus", 1792100000n), "Limits unknown"},
   Case{"malformed snapshot", chip("not json"), "Limits unknown"},
   Case{"legacy snapshot", chip("{\"used_percent\":28}"), "Limits unknown"},
   Case{"null snapshot", chip("null"), "Limits unknown"},
   Case{"no pools", chip(good("codex", Nil{})), "Limits unknown"},
   Case{"unavailable", chip(snapshot("codex", "unavailable", J.Num{"1792100000"}, Nil{}, Nil{})), "Limits unknown"}]

def invalid() -> List<&2, Case>:
  [Case{"out of range", chip(good("codex", [win("shared", "1001")])), "Limits unknown"},
   Case{"negative consumed", chip(good("codex", [win("shared", "-1")])), "Limits unknown"},
   Case{"fractional consumed", chip(good("codex", [win("shared", "28.5")])), "Limits unknown"},
   Case{"exponent consumed", chip(good("codex", [win("shared", "2e2")])), "Limits unknown"},
   Case{"string consumed is not an integer", chip(good("codex", [window("five", "shared", J.Str{"280"}, J.Null{})])), "Limits unknown"},
   Case{"null consumed", chip(good("codex", [window("five", "shared", J.Null{}, J.Null{})])), "Limits unknown"},
   Case{"absent consumed", chip(good("codex", [J.Json.obj([J.KV{"scope", J.Str{"shared"}}])])), "Limits unknown"},
   Case{"bool consumed", chip(good("codex", [window("five", "shared", J.Flag{False{}}, J.Null{})])), "Limits unknown"},
   lacks("invalid rows never fabricate full remaining", rows(good("codex", [win("shared", "1001")]), "codex", ""), "100% left"),
   has("invalid rows explain the missing measurement", rows(good("codex", [window("five", "shared", J.Null{}, J.Null{})]), "codex", ""), "Remaining unknown"),
   Case{"missing pool scope is unknown", chip(good("codex", [win("", "280")])), "Limits unknown"}]

def freshness() -> List<&2, Case>:
  +one = good("codex", [win("shared", "280")])
  +failed = snapshot("codex", "error", J.Num{"1792100000"}, [win("shared", "280")], [J.KV{"attempted_at", J.Num{"1792100100"}}])
  +noat = {"{\"provider\":\"codex\",\"state\":\"ok\",\"windows\":[{\"scope\":\"shared\",\"used_permille\":280}]}" : String}
  [Case{"exactly fifteen minutes still fresh", A.Allowance.chip(one, "codex", "", 1792100900n), "72% left · shared"},
   Case{"over fifteen minutes stale", A.Allowance.chip(one, "codex", "", 1792100901n), "Limits stale"},
   Case{"reset reached exactly", A.Allowance.chip(good("codex", [window("five", "shared", J.Num{"280"}, J.Num{"1792100000"})]), "codex", "", 1792100000n), "Limits stale"},
   Case{"reset in past", chip(good("codex", [window("five", "shared", J.Num{"280"}, J.Num{"1792099999"})])), "Limits stale"},
   Case{"epoch reset is passed not unknown", chip(good("codex", [window("five", "shared", J.Num{"280"}, J.Num{"0"})])), "Limits stale"},
   Case{"one expired time window invalidates current pool", chip(good("codex", [win("shared", "280"), window("week", "shared", J.Num{"900"}, J.Num{"1792099999"})])), "Limits stale"},
   Case{"missing reset does not invent one", chip(good("codex", [window("five", "shared", J.Num{"280"}, J.Null{})])), "72% left · shared"},
   Case{"missing reading timestamp", chip(noat), "Limits stale"},
   Case{"zero reading timestamp", chip(snapshot("codex", "ok", J.Num{"0"}, [win("shared", "280")], Nil{})), "Limits stale"},
   Case{"future reading timestamp", chip(snapshot("codex", "ok", J.Num{"1792100001"}, [win("shared", "280")], Nil{})), "Limits stale"},
   Case{"timestamp string is not trusted", chip(snapshot("codex", "ok", J.Str{"1792100000"}, [win("shared", "280")], Nil{})), "Limits stale"},
   Case{"refresh error overrides recent at", chip(failed), "Limits stale"},
   has("failed refresh rows say error", rows(failed, "codex", ""), "Refresh error"),
   has("failed refresh retains only last report", rows(failed, "codex", ""), "Last reported 72% left · stale"),
   has("missing update is explicit", rows(noat, "codex", ""), "Update time unknown"),
   lacks("missing update does not invent epoch date", rows(noat, "codex", ""), "1970-01-01"),
   Case{"attempt time never freshens old reading", chip(snapshot("codex", "ok", J.Num{"1792098000"}, [win("shared", "280")], [J.KV{"attempted_at", J.Num{"1792100000"}}])), "Limits stale"}]

def claude() -> List<&2, Case>:
  +s = good("claude", [window("five_hour", "shared", J.Num{"280"}, J.Null{}),
    window("seven_day", "shared", J.Num{"500"}, J.Null{}),
    window("opus_week", "opus", J.Num{"900"}, J.Null{}),
    window("sonnet_week", "sonnet", J.Num{"100"}, J.Null{})])
  +opus = rows(s, "claude", "claude-opus-5-5")
  +sonnet = rows(s, "claude", "SONNET")
  +unknown = rows(s, "claude", "default")
  [has("Opus includes five hour shared", opus, "five_hour"),
   has("Opus includes seven day shared", opus, "seven_day"),
   has("Opus includes its explicit pool", opus, "opus_week"),
   lacks("Opus excludes Sonnet", opus, "sonnet_week"),
   has("Sonnet alias includes its explicit pool", sonnet, "sonnet_week"),
   lacks("Sonnet excludes Opus", sonnet, "opus_week"),
   has("unknown model keeps shared", unknown, "five_hour"),
   lacks("unknown model excludes specific pools", unknown, "opus_week"),
   lacks("unrelated substring is not Opus", rows(s, "claude", "octopus"), "opus_week"),
   lacks("ambiguous family is not inferred", rows(s, "claude", "opus-sonnet"), "opus_week"),
   Case{"shared Claude windows use limiting percentage", A.Allowance.chip(s, "claude", "haiku", 1792100000n), "50% left · shared"},
   Case{"explicit Claude model pool can be the only pool", A.Allowance.chip(good("claude", [win("opus", "280")]), "claude", "claude/OPUS_5.5", 1792100000n), "72% left · shared"}]

def detail() -> List<&2, Case>:
  +s = good("codex", [window("Provider label A", "limit-id-a", J.Num{"280"}, J.Num{"1792103600"}),
    window("Provider label B", "limit-id-b", J.Num{"900"}, J.Null{})])
  +r = rows(s, "codex", "gpt-6.1-sol")
  +missing = rows(good("codex", [J.Json.obj([J.KV{"scope", J.Str{"shared"}}, J.KV{"used_permille", J.Num{"280"}}])]), "codex", "")
  [has("selected provider and model", r, "Selected model | codex · gpt-6.1-sol"),
   has("every returned Codex pool A", r, "Provider label A | 72% left"),
   has("every returned Codex pool B", r, "Provider label B | 10% left"),
   has("Codex allocation explicitly unreported", r, "allocation per model not reported"),
   has("Codex scope is the reported limit id", r, "scope: limit-id-a"),
   has("Codex auth label", r, "ChatGPT account"),
   has("duration is reported minutes", r, "5 h window"),
   has("reset absolute UTC includes full date", r, "reset 2026-10-15T22:33:20Z UTC (in 1 h)"),
   has("unknown reset remains unknown", r, "reset unknown"),
   has("reading time is absolute and relative", r, "Last update | 2026-10-15T21:33:20Z UTC (0 s ago)"),
   has("source included", r, "Source | provider fixture"),
   has("missing reset key stays unknown", missing, "reset unknown"),
   has("missing duration stays unknown", missing, "duration unknown"),
   lacks("missing reset has no manufactured epoch", missing, "1970-01-01"),
   has("unavailable state explicit", rows(snapshot("codex", "unavailable", J.Num{"1792100000"}, Nil{}, Nil{}), "codex", ""), "Limits unavailable"),
   has("invalid snapshot explicit", rows("broken", "codex", ""), "invalid or legacy"),
   has("unknown auth is an honest label", rows("{\"provider\":\"codex\",\"state\":\"unavailable\",\"auth\":\"user@example.com\"}", "codex", ""), "Account auth unknown"),
   lacks("account email is not projected", rows("{\"provider\":\"codex\",\"auth\":\"user@example.com\",\"email\":\"user@example.com\"}", "codex", ""), "user@example.com"),
   has("API auth label", rows("{\"provider\":\"codex\",\"auth\":\"apikey\"}", "codex", ""), "API key account")]

def due(snapshot: String, now: Nat) -> String:
  Bool.show(A.Allowance.due(snapshot, now))

def passive() -> List<&2, Case>:
  +s = J.Json.show(J.Json.obj([J.KV{"provider", J.Str{"claude"}}, J.KV{"state", J.Str{"ok"}},
    J.KV{"auth", J.Str{"unknown"}}, J.KV{"at", J.Num{"1792100000"}}, J.KV{"source", J.Str{"Claude stream"}},
    J.KV{"windows", J.Json.arr([window("5 h", "shared", J.Num{"281"}, J.Null{}),
      window("7 d", "shared", J.Num{"500"}, J.Null{})])}]))
  +r = rows(s, "claude", "claude-sonnet-5-5")
  [Case{"passive Claude limits valid without account identity", A.Allowance.chip(s, "claude", "claude-sonnet-5-5", 1792100000n), "50% left · shared"},
   has("passive Claude account label", r, "Claude CLI account on this machine · identity not reported"),
   has("passive Claude includes shared five hour window", r, "5 h | 71% left"),
   has("passive Claude includes shared seven day window", r, "7 d | 50% left"),
   has("passive Claude states shared account scope", r, "scope: shared · shared account pool"),
   has("passive Claude preserves provider source", r, "Source | Claude stream"),
   lacks("passive Claude does not claim ChatGPT authentication", r, "ChatGPT account")]

def caching() -> List<&2, Case>:
  +s = good("codex", [win("shared", "280")])
  +failed = snapshot("codex", "error", J.Num{"1792090000"}, [win("shared", "280")], [J.KV{"attempted_at", J.Num{"1792100000"}}])
  [Case{"fallback at blocks immediate refresh", due(s, 1792100000n), "False"},
   Case{"299 seconds is too soon", due(s, 1792100299n), "False"},
   Case{"300 seconds is due", due(s, 1792100300n), "True"},
   Case{"failed attempt blocks retry despite old reading", due(failed, 1792100299n), "False"},
   Case{"failed attempt due at five minutes", due(failed, 1792100300n), "True"},
   Case{"future attempt cannot cause rapid refresh", due(failed, 1792099999n), "False"},
   Case{"missing snapshot is due", due("", 1792100000n), "True"},
   Case{"missing reading but known attempt is throttled", due("{\"attempted_at\":1792100000}", 1792100299n), "False"},
   Case{"safe parser invalid input", J.Json.show(A.Allowance.parse("not-json")), "null"}]

def cases() -> List<&2, Case>:
  List.append(&2, Case, chips(), List.append(&2, Case, invalid(), List.append(&2, Case,
    freshness(), List.append(&2, Case, claude(), List.append(&2, Case, detail(), List.append(&2, Case, passive(), caching()))))))

def run(cs: List<&2, Case>) -> IO(Unit):
  match cs:
    case Nil{}: IO.pure(Unit, Unit{})
    case Case{label, got, want} <> rest:
      do IO<Unit>:
        C.Check.str(label, got, want)
        run(rest)

def main() -> IO(Unit):
  run(cases())
