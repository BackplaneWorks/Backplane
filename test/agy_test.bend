import Base
import ./check.bend as C
import ../src/core/acp.bend as A
import ../src/core/agy.bend as G
import ../src/core/json.bend as J
import ./replay.bend as R

# Google Antigravity (core/agy.bend): turns, the sign-in and the models
# replayed from what agy_acp_server 1.3.0 wrote (test/fixtures/antigravity:
# a fresh turn, a resumed one, a Google sign-in, its address redacted),
# the way back from Google, the probe, the runner's settings.

def turn(+sid: String, +model: String) -> G.AgyTurn:
  G.AgyTurn{A.AcpTurn{"/tmp/p", "http://127.0.0.1:1/mcp/t?token=x", "RULES", "hello", sid, True{}}, "gemini-api-key", "default", model}

def feed(r: R.Run, +t: G.AgyTurn, +l: String) -> R.Run:
  match r:
    case R.Run{a, +ss, +es}:
      R.Run.add(G.Agy.line(a, t, l), ss, es)

def replay(ls: List<&2, String>, r: R.Run, +t: G.AgyTurn) -> R.Run:
  match ls:
    case Nil{}:
      r
    case +l <> rest:
      replay(rest, feed(r, t, l), t)

def sends(r: R.Run) -> List<&2, String>:
  R.sends(r)

def emits(r: R.Run) -> String:
  R.emits(r)

def done(r: R.Run) -> String:
  R.done(r)

def obj(+line: String) -> J.Json:
  R.obj(line)

def method(+line: String) -> String:
  R.method(line)

def methods(xs: List<&2, String>) -> List<&2, String>:
  R.methods(xs)

def params(+line: String) -> J.Json:
  R.params(line)

def nth(xs: List<&2, String>, n: Nat) -> String:
  R.nth(xs, n)

def yes(b: Bool) -> String:
  R.yes(b)

def start() -> R.Run:
  R.Run.new()

# the sign-in, line by line
type SRun is Data:
  SRun{s: G.AgySign, sends: List<&2, String>, says: List<&2, String>}

def SRun.add(o: G.AgySignOut, +ss: List<&2, String>, +ys: List<&2, String>) -> SRun:
  match o:
    case G.AgySignOut{s, a, b}:
      SRun{s, List.append(&2, String, ss, a), List.append(&2, String, ys, b)}

def SRun.step(r: SRun, +l: String) -> SRun:
  match r:
    case SRun{s, +ss, +ys}:
      SRun.add(G.AgySign.line(s, "oauth-personal", "/home/me/.backplane", l), ss, ys)

def sreplay(ls: List<&2, String>, r: SRun) -> SRun:
  match ls:
    case Nil{}:
      r
    case +l <> rest:
      sreplay(rest, SRun.step(r, l))

def ssends(r: SRun) -> List<&2, String>:
  match r:
    case SRun{_, s, _}:
      s

def says(r: SRun) -> List<&2, String>:
  match r:
    case SRun{_, _, y}:
      y

def sdone(r: SRun) -> String:
  match r:
    case SRun{s, _, _}:
      yes(G.AgySign.done(s))

def file(path: String) -> IO(List<&2, String>):
  R.file(path)

def asks(o: A.AcpOut) -> String:
  match o:
    case A.AcpOut{_, _, _, ks}:
      method(String.join(ks, ""))

def perm() -> String:
  "{\"jsonrpc\":\"2.0\",\"id\":9,\"method\":\"session/request_permission\",\"params\":{\"sessionId\":\"S\",\"toolCall\":{\"toolCallId\":\"T\",\"title\":\"Run ls\",\"kind\":\"execute\"},\"options\":[{\"optionId\":\"a\",\"kind\":\"allow_once\",\"name\":\"Allow\"},{\"optionId\":\"r\",\"kind\":\"reject_once\",\"name\":\"Deny\"}]}}"

def url() -> String:
  "https://accounts.google.com/o/oauth2/v2/auth?response_type=code&client_id=CLIENT.apps.googleusercontent.com&redirect_uri=http%3A%2F%2F127.0.0.1%3A38117%2F&scope=https%3A%2F%2Fwww.googleapis.com%2Fauth%2Fcloud-platform&state=STATE123&code_challenge=C&code_challenge_method=S256"

def main() -> IO(Unit):
  do IO<Unit>:
    +fresh : List<&2, String> <- file("test/fixtures/antigravity/turn.jsonl")
    +res : List<&2, String> <- file("test/fixtures/antigravity/resume.jsonl")
    +sign : List<&2, String> <- file("test/fixtures/antigravity/signin.jsonl")
    +r1 : R.Run = replay(fresh, start(), turn("", "gemini-3.1-pro-low"))
    +r2 : R.Run = replay(res, start(), turn("b2edf953-0194-476f-8e17-e65a3bc9c466", ""))
    +rs : SRun = sreplay(sign, SRun{G.AgySign.new(), Nil{}, Nil{}})
    C.Check.str("fixtures read", Nat.show(List.length(&2, String, fresh)) ++ "," ++ Nat.show(List.length(&2, String, res)) ++ "," ++ Nat.show(List.length(&2, String, sign)), "8,5,4")
    # a fresh turn: authenticate, session/new with Backplane's MCP, the mode, the model, the prompt
    C.Check.str("agy init", method(G.Agy.init()), "initialize")
    C.Check.str("agy fresh sends", String.join(methods(sends(r1)), ","), "authenticate,session/new,session/set_mode,session/set_config_option,session/prompt")
    C.Check.str("agy auth method", J.Json.get_str(params(nth(sends(r1), 0n)), "methodId", ""), "gemini-api-key")
    C.Check.str("agy mcp", J.Json.show(J.Json.get_obj(params(nth(sends(r1), 1n)), "mcpServers")), "[{\"type\":\"http\",\"name\":\"backplane\",\"url\":\"http://127.0.0.1:1/mcp/t?token=x\",\"headers\":[]}]")
    C.Check.str("agy mode", J.Json.get_str(params(nth(sends(r1), 2n)), "modeId", ""), "default")
    C.Check.str("agy session id", J.Json.get_str(params(nth(sends(r1), 2n)), "sessionId", ""), "b2edf953-0194-476f-8e17-e65a3bc9c466")
    C.Check.str("agy model", J.Json.get_str(params(nth(sends(r1), 3n)), "value", ""), "gemini-3.1-pro-low")
    C.Check.str("agy rules first", yes(String.contains(nth(sends(r1), 4n), "<backplane_instructions>\\nRULES")), "yes")
    C.Check.str("agy fresh events", emits(r1), "{\"type\":\"thread.started\",\"thread_id\":\"b2edf953-0194-476f-8e17-e65a3bc9c466\"} | {\"type\":\"item.delta\",\"text\":\"API key not valid. Please pass a valid API key.\"} | {\"type\":\"item.completed\",\"item\":{\"type\":\"agent_message\",\"text\":\"API key not valid. Please pass a valid API key.\"}} | {\"type\":\"turn.completed\"}")
    C.Check.str("agy fresh done", done(r1), "done")
    # a resumed one: session/resume, no model, a plain prompt
    C.Check.str("agy resume sends", String.join(methods(sends(r2)), ","), "authenticate,session/resume,session/set_mode,session/prompt")
    C.Check.str("agy resume plain", yes(Bool.and(String.contains(nth(sends(r2), 3n), "\"text\":\"hello\""), Bool.not(String.contains(nth(sends(r2), 3n), "backplane_instructions")))), "yes")
    C.Check.str("agy resume started", nth(String.split(emits(r2), '|'), 0n), "{\"type\":\"thread.started\",\"thread_id\":\"b2edf953-0194-476f-8e17-e65a3bc9c466\"} ")
    # a lost session opens afresh, and the prompt carries the rules again
    C.Check.str("agy lost session", method(nth(sends(replay(["{\"jsonrpc\":\"2.0\",\"id\":2,\"error\":{\"code\":-32603,\"message\":\"session not found\"}}"], start(), turn("OLD", ""))), 0n)), "session/new")
    C.Check.str("agy lost session id", J.Json.as_raw(J.Json.get(obj(nth(sends(replay(["{\"jsonrpc\":\"2.0\",\"id\":2,\"error\":{\"code\":-32603,\"message\":\"x\"}}"], start(), turn("OLD", ""))), 0n)), "id"), ""), "7")
    C.Check.str("agy renewed mode", J.Json.as_raw(J.Json.get(obj(nth(sends(replay(["{\"jsonrpc\":\"2.0\",\"id\":7,\"result\":{\"sessionId\":\"NEW\"}}"], start(), turn("OLD", ""))), 0n)), "id"), ""), "5")
    C.Check.str("agy renewed rules", yes(String.contains(nth(sends(replay(["{\"jsonrpc\":\"2.0\",\"id\":7,\"result\":{\"sessionId\":\"NEW\"}}", "{\"jsonrpc\":\"2.0\",\"id\":5,\"result\":{}}"], start(), turn("OLD", ""))), 1n), "backplane_instructions")), "yes")
    # failures
    C.Check.str("agy no key", emits(replay(["{\"jsonrpc\":\"2.0\",\"id\":2,\"error\":{\"code\":-32000,\"message\":\"auth.type is set to 'gemini-api-key' in settings.json but the GEMINI_API_KEY environment variable is not set.\",\"data\":null}}"], start(), turn("", ""))),
      "{\"type\":\"turn.failed\",\"message\":\"auth.type is set to 'gemini-api-key' in settings.json but the GEMINI_API_KEY environment variable is not set. (Settings › Antigravity: add a key or sign in)\"}")
    C.Check.str("agy bad model", emits(replay(["{\"jsonrpc\":\"2.0\",\"id\":6,\"error\":{\"code\":-32602,\"message\":\"bad\"}}"], start(), turn("", "gemini-9"))), "{\"type\":\"turn.failed\",\"message\":\"Antigravity has no model gemini-9 for this sign-in: pick another in the composer.\"}")
    C.Check.str("agy mode refused goes on", method(nth(sends(replay(["{\"jsonrpc\":\"2.0\",\"id\":5,\"error\":{\"code\":-32601,\"message\":\"no\"}}"], start(), turn("", ""))), 0n)), "session/prompt")
    C.Check.str("agy wants a sign-in", emits(replay(["BACKPLANE-SIGNIN " ++ url()], start(), turn("", ""))), "{\"type\":\"turn.failed\",\"message\":\"Antigravity is not signed in: sign in under Settings › Antigravity first.\"}")
    C.Check.str("agy missing", emits(replay(["BACKPLANE-MISSING"], start(), turn("", ""))), "{\"type\":\"turn.failed\",\"message\":\"Antigravity is not installed: install it under Settings › Antigravity.\"}")
    C.Check.str("agy noise", emits(replay(["E1013 12:26:32 some log"], start(), turn("", ""))), "")
    C.Check.str("agy permission parks", asks(G.Agy.line(A.Acp.new(), turn("", ""), perm())), "session/request_permission")
    C.Check.str("agy permission yolo", (asks(G.Agy.line(A.Acp.new(), G.AgyTurn{A.AcpTurn{"/tmp/p", "u", "R", "h", "", False{}}, "gemini-api-key", "yolo", ""}, perm()))), "")
    # the modes
    C.Check.str("agy modes", G.Agy.mode.k(0n) ++ "," ++ G.Agy.mode.k(1n) ++ "," ++ G.Agy.mode.k(2n) ++ "," ++ G.Agy.mode.k(3n), "default,auto_edit,default,yolo")
    # models from a real session/new
    C.Check.str("agy models first", String.take(G.Agy.cache(J.Json.get_obj(obj(nth(fresh, 2n)), "result")), 100n), "{\"models\":[{\"id\":\"gemini-3.8-flash-high\",\"name\":\"Gemini 3.8 Flash (High)\"},{\"id\":\"gemini-3.8-flash-m")
    C.Check.str("agy models count", Nat.show(List.length(&2, G.AgyModel, G.Agy.models(J.Json.get_obj(obj(nth(fresh, 2n)), "result")))), "14")
    C.Check.str("agy models default", G.Agy.current(J.Json.get_obj(obj(nth(fresh, 2n)), "result")), "gemini-3.8-flash-high")
    C.Check.str("agy models fallback", G.Agy.cache(obj("{\"models\":{\"availableModels\":[{\"modelId\":\"m1\",\"name\":\"One\"}],\"currentModelId\":\"m1\"}}")), "{\"models\":[{\"id\":\"m1\",\"name\":\"One\"}],\"default\":\"m1\"}")
    C.Check.str("agy models groups", G.Agy.cache(obj("{\"configOptions\":[{\"id\":\"model\",\"currentValue\":\"b\",\"options\":[{\"name\":\"G\",\"options\":[{\"value\":\"a\",\"name\":\"A\"},{\"value\":\"b\"}]}]}]}")), "{\"models\":[{\"id\":\"a\",\"name\":\"A\"},{\"id\":\"b\",\"name\":\"b\"}],\"default\":\"b\"}")
    # the sign-in: initialize, authenticate, the address, then a session that lists the models
    C.Check.str("sign sends", String.join(methods(ssends(rs)), ","), "authenticate,session/new")
    C.Check.str("sign no mcp", J.Json.show(J.Json.get_obj(params(nth(ssends(rs), 1n)), "mcpServers")), "[]")
    C.Check.str("sign url", String.take(nth(says(rs), 0n), 60n), "url\thttps://accounts.google.com/o/oauth2/v2/auth?response_ty")
    C.Check.str("sign ok", String.take(nth(says(rs), 1n), 40n), "ok\t{\"models\":[{\"id\":\"gemini-3.8-flash-hi")
    C.Check.str("sign done", sdone(rs), "yes")
    C.Check.str("sign foreign url", nth(says(sreplay(["BACKPLANE-SIGNIN https://evil.example/o/oauth2/v2/auth?redirect_uri=http%3A%2F%2F127.0.0.1%3A1%2F&state=x"], SRun{G.AgySign.new(), Nil{}, Nil{}})), 0n), "fail\tAntigravity gave a sign-in address that is not Google's")
    C.Check.str("sign subscription", nth(says(sreplay(["{\"jsonrpc\":\"2.0\",\"id\":4,\"error\":{\"code\":-32000,\"message\":\"SUBSCRIPTION_REQUIRED: x\"}}"], SRun{G.AgySign.new(), Nil{}, Nil{}})), 0n), "fail\tGoogle needs an eligible Antigravity plan for this account.")
    # the way back from Google
    C.Check.str("cb ok", yes(G.Agy.callback.ok(url(), "http://127.0.0.1:38117/?state=STATE123&code=4%2F0Ab&scope=x")), "yes")
    C.Check.str("cb error ok", yes(G.Agy.callback.ok(url(), "http://127.0.0.1:38117/?state=STATE123&error=access_denied")), "yes")
    C.Check.str("cb other port", yes(G.Agy.callback.ok(url(), "http://127.0.0.1:38118/?state=STATE123&code=a")), "no")
    C.Check.str("cb other host", yes(G.Agy.callback.ok(url(), "http://evil.example:38117/?state=STATE123&code=a")), "no")
    C.Check.str("cb other state", yes(G.Agy.callback.ok(url(), "http://127.0.0.1:38117/?state=OTHER&code=a")), "no")
    C.Check.str("cb two states", yes(G.Agy.callback.ok(url(), "http://127.0.0.1:38117/?state=STATE123&state=STATE123&code=a")), "no")
    C.Check.str("cb no code", yes(G.Agy.callback.ok(url(), "http://127.0.0.1:38117/?state=STATE123")), "no")
    C.Check.str("cb code and error", yes(G.Agy.callback.ok(url(), "http://127.0.0.1:38117/?state=STATE123&code=a&error=b")), "no")
    C.Check.str("cb fragment", yes(G.Agy.callback.ok(url(), "http://127.0.0.1:38117/?state=STATE123&code=a#x")), "no")
    C.Check.str("cb no sign-in", yes(G.Agy.callback.ok("", "http://127.0.0.1:38117/?state=STATE123&code=a")), "no")
    C.Check.str("unpct", G.Agy.unpct("http%3A%2F%2F127.0.0.1%3A38117%2F+a"), "http://127.0.0.1:38117/ a")
    # methods, accounts, settings
    C.Check.str("method default", G.Agy.method(""), "gemini-api-key")
    C.Check.str("method bogus", G.Agy.method("rm -rf"), "gemini-api-key")
    C.Check.str("method google", G.Agy.method("oauth-personal"), "oauth-personal")
    C.Check.str("method keys", G.Agy.key("gemini-api-key") ++ "," ++ G.Agy.key("agent-platform") ++ "," ++ G.Agy.key("oauth-personal"), "GEMINI_API_KEY,GOOGLE_API_KEY,")
    C.Check.str("method warns", yes(String.contains(G.Agy.note("oauth-personal"), "suspended")), "yes")
    C.Check.str("acct", G.Agy.acct(" Work Account!/../x "), "workaccountx")
    C.Check.str("acct empty", G.Agy.acct("../"), "default")
    C.Check.str("settings", G.Agy.settings("oauth-business", " proj ", "us-central1"), "{\"auth\":{\"type\":\"oauth-business\"},\"gcp\":{\"project\":\"proj\",\"location\":\"us-central1\"}}")
    C.Check.str("settings bare", G.Agy.settings("", "", ""), "{\"auth\":{\"type\":\"gemini-api-key\"}}")
    C.Check.str("args", String.join(G.Agy.args("/h/antigravity", "/h/secrets/api.env", "oauth-personal", "Work", "", ""), " "),
      "/h/antigravity/agy.sh /h/antigravity/1.3.0 /h/antigravity/profiles/work oauth-personal /h/secrets/api.env {\"auth\":{\"type\":\"oauth-personal\"}}")
    # the probe and what clients are told
    C.Check.str("info", J.Json.show(G.Agy.info.json(G.Agy.info("have 1.3.0\nold 1.2.1\nold 1.3.0\nplat Linux-x86_64\nacct 1 default\nacct 0 work\nmodels {\"models\":[{\"id\":\"m1\",\"name\":\"One\"}],\"default\":\"m1\"}\n"))),
      "{\"pin\":\"1.3.0\",\"have\":\"1.3.0\",\"old\":\"1.2.1\",\"plat\":true,\"accts\":[\"default\",\"work\"],\"signed\":[\"default\"]}")
    C.Check.str("info models", String.join(G.Agy.info.models(G.Agy.info("have 1.3.0\nmodels {\"models\":[{\"id\":\"m1\",\"name\":\"One\"},{\"id\":\"m2\",\"name\":\"Two\"}],\"default\":\"m1\"}")), ","), "m1,m2")
    C.Check.str("info none installed", String.join(G.Agy.info.models(G.Agy.info("models {\"models\":[{\"id\":\"m1\",\"name\":\"One\"}]}")), ","), "")
    C.Check.str("info about", J.Json.show(G.Agy.info.about(G.Agy.info("have 1.3.0\nmodels {\"models\":[{\"id\":\"m1\",\"name\":\"One\"}],\"default\":\"m1\"}"))),
      "{\"cli\":\"Antigravity 1.3.0\",\"labels\":{\"default\":\"Antigravity's own default\",\"m1\":\"One · Antigravity's default\"}}")
    C.Check.str("info no build", J.Json.show(J.Json.get_obj(G.Agy.info.json(G.Agy.info("plat Plan9-mips")), "plat")), "false")
    C.Check.str("install ok", G.Agy.install.err("downloading\nok\n"), "")
    C.Check.str("install fail", G.Agy.install.err("the download's SHA-256 is not the one Backplane pins\n"), "the download's SHA-256 is not the one Backplane pins")
    C.Check.str("install pins", yes(Bool.and(String.contains(G.Agy.install.sh(), "9fb60956af0a9d76220a4db91ca9ac88e2a2372ad68f985ab5fceace6b825b96"), String.contains(G.Agy.install.sh(), "Linux-x86_64)"))), "yes")
