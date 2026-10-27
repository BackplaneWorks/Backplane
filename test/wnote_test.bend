import Base
import ./check.bend as C
import ../src/core/json.bend as J
import ../src/core/model.bend as M
import ../src/core/notice.bend as NO
import ../src/core/sign.bend as SG
import ../src/core/wnote.bend as WN

# The outbound webhook (core/wnote.bend): it posts only for an alert the
# notification policy would raise, the JSON carries that kind, and the
# HMAC-SHA256 header matches Python's hmac (a hex secret is the raw key,
# Sign.key).

def yn(b: Bool) -> String:
  match b:
    case True{}:
      "yes"
    case False{}:
      "no"

def field(+body: String, +key: String) -> String:
  J.Json.get_str(Maybe.default(&2, J.Json, J.Json.parse(body), J.Null{}), key, "")

def hdr(+secret: String, +ts: Nat, +body: String) -> String:
  String.join(WN.Wn.headers(secret, ts, body), "\n")

# a worker the private Mailroom triage (Bub, thread foh) launched, its turn failed
def managed() -> M.State:
  M.State.replay([M.ProjectCreated{"p", "P", "/tmp/p", 1n},
    M.ThreadCreated{"foh", "", "Bub", M.Local{}, "", "/tmp/b", "claude", "", 1n},
    M.ThreadCreated{"worker", "p", "Worker", M.Local{}, "", "/tmp/p", "claude", "", 1n},
    M.BotSet{"b", "Bub", "foh", 0n, "", 1n}, M.SettingSet{"uber.bot", "b", 1n},
    M.ThreadLaunched{"worker", "foh", True{}, 1n},
    M.MessagePosted{"worker", "m1", M.User{}, "check the board", 2n}, M.TurnChanged{"worker", M.Running{}, 2n},
    M.TurnChanged{"worker", M.Failed{}, 3n}], M.State.empty())

# newest first: an interrupted turn that logged no error, after an older
# turn that hit a usage limit
def older() -> List<&2, M.Entry>:
  [M.Msg{"a2", "t1", M.Assistant{}, "working on it", 21n}, M.Msg{"m2", "t1", M.User{}, "next", 20n},
   M.Act{"e1", "t1", "error", "result", "Claude AI usage limit reached|1792800000", 11n}, M.Msg{"m1", "t1", M.User{}, "do it", 10n}]

def current() -> List<&2, M.Entry>:
  [M.Act{"e2", "t1", "error", "result", "API Error: 500", 22n}, M.Msg{"m2", "t1", M.User{}, "next", 20n},
   M.Act{"e1", "t1", "error", "result", "Claude AI usage limit reached|1792800000", 11n}]

# a turn that ended before it replied, after an older turn's reply
def noreply() -> List<&2, M.Entry>:
  [M.Msg{"m5", "t1", M.User{}, "again", 50n}, M.Msg{"a2", "t1", M.Assistant{}, "an older reply", 21n}, M.Msg{"m2", "t1", M.User{}, "next", 20n}]

# a turn whose model call failed: Claude Code's API error, logged as the
# turn's own (hub.bend's Claude.api.err), then an interrupted end
def badmodel() -> List<&2, M.Entry>:
  [M.Act{"api1", "t1", "error", "api", "There's an issue with the selected model (no-such-model-webhook-test)", 41n},
   M.Msg{"a4", "t1", M.Assistant{}, "There's an issue with the selected model (no-such-model-webhook-test)", 41n}, M.Msg{"m4", "t1", M.User{}, "hi", 40n},
   M.Act{"e1", "t1", "error", "result", "older failure", 11n}]

# a turn whose reply reports a limit, with no error logged, after an older
# turn's reply
def limited() -> List<&2, M.Entry>:
  [M.Msg{"a3", "t1", M.Assistant{}, "Claude AI usage limit reached|1792800000", 31n}, M.Msg{"m3", "t1", M.User{}, "go on", 30n},
   M.Msg{"a1", "t1", M.Assistant{}, "an older reply", 12n}]

# a long history, newest first: n older entries of an info tone, after
# the oldest (last) entry
def long.go(n: Nat, acc: List<&2, M.Entry>) -> List<&2, M.Entry>:
  match n:
    case 0n:
      acc
    case 1n+p:
      long.go(p, M.Act{"o", "t1", "info", "tool", "older work", 5n} <> acc)

def long(n: Nat, +last: M.Entry) -> List<&2, M.Entry>:
  long.go(n, [last])

# a turn that failed (its error logged), with push.webhook.url and the
# turn's start mark as given
def hooked.m(+url: String, +mark: String) -> M.State:
  M.State.replay([M.ProjectCreated{"p", "P", "/tmp/p", 1n},
    M.ThreadCreated{"t1", "p", "Work", M.Local{}, "", "/tmp/p", "claude", "", 1n},
    M.SettingSet{"push.webhook.url", url, 1n}, M.SettingSet{WN.Wn.from.key("t1"), mark, 2n},
    M.MessagePosted{"t1", "m1", M.User{}, "go", 2n}, M.TurnChanged{"t1", M.Running{}, 2n},
    M.ActivityLogged{"t1", "e1", "error", "result", "API Error: 500", 3n}, M.TurnChanged{"t1", M.Failed{}, 3n}], M.State.empty())

def hooked(+url: String) -> M.State:
  hooked.m(url, "@m1")

def main() -> IO(Unit):
  do IO<Unit>:
    +body : String = WN.Wn.body("fail", "t1", "failed", "Board", "Failed")
    +ok : String = WN.Wn.body("done", "t1", "completed", "A", "done")
    +ask : String = WN.Wn.body("ask", "t1", "running", "Board", "Needs you")
    +done : List<&2, M.Change> = NO.Policy.end(NO.Policy.demo.s(NO.Policy.demo.ask()), 13n, "t1", "completed")
    +stop : List<&2, M.Change> = NO.Policy.end(NO.Policy.demo.s(NO.Policy.demo.cat([NO.Policy.demo.base(), NO.Policy.demo.run("t1", "m1", "do it", 10n), [M.TurnChanged{"t1", M.Interrupted{}, 12n}]])), 13n, "t1", "stopped")
    +asked : List<&2, M.Change> = NO.Policy.ask(NO.Policy.demo.s(NO.Policy.demo.ask()), 20n, "t1", "Bash", "ls")
    +s1 : M.State = NO.Policy.demo.s(NO.Policy.demo.ask())
    +s2 : M.State = NO.Policy.demo.fold(s1, NO.Policy.end(s1, 13n, "t1", "completed"), NO.Policy.demo.cat([NO.Policy.demo.run("t1", M.Link.id("t0", "r1"), "[Backplane] Launched thread \"w\" (t2) completed.", 14n), NO.Policy.demo.done("t1", 15n)]))
    +later : String = NO.Policy.kind(NO.Policy.end(s2, 16n, "t1", "completed"))
    C.Check.str("off when unset", yn(WN.Wn.on("")), "no")
    C.Check.str("blank is off", yn(WN.Wn.live("  ")), "no")
    C.Check.str("https is on", yn(WN.Wn.live(" https://example.com/hook ")), "yes")
    C.Check.str("http is on", yn(WN.Wn.url.ok("http://127.0.0.1:9/h")), "yes")
    C.Check.str("file url refused", yn(WN.Wn.url.ok("file:///etc/passwd")), "no")
    C.Check.str("ftp refused", yn(WN.Wn.url.ok("ftp://example.com/h")), "no")
    C.Check.str("space refused", yn(WN.Wn.url.ok("https://example.com/a b")), "no")
    C.Check.str("default secret file", WN.Wn.path("/h", ""), "/h/secrets/push.webhook")
    C.Check.str("blank secret setting", WN.Wn.path("/h", "  "), "/h/secrets/push.webhook")
    C.Check.str("secret path", WN.Wn.path("/h", " /tmp/s "), "/tmp/s")
    C.Check.str("unsigned without a secret", Nat.show(WN.Wn.mode(False{}, "")), "1")
    C.Check.str("default file signs", Nat.show(WN.Wn.mode(False{}, "s")), "2")
    C.Check.str("missing configured secret", Nat.show(WN.Wn.mode(True{}, "")), "0")
    C.Check.str("configured secret signs", Nat.show(WN.Wn.mode(True{}, "s")), "2")
    C.Check.str("failed", WN.Wn.outcome(M.Failed{}), "failed")
    C.Check.str("stopped", WN.Wn.outcome(M.Interrupted{}), "stopped")
    C.Check.str("completed", WN.Wn.outcome(M.Completed{}), "completed")
    C.Check.str("end kept over a follow-up", WN.Wn.report("completed", M.Running{}), "completed")
    C.Check.str("failure kept over a follow-up", WN.Wn.report("failed", M.Running{}), "failed")
    C.Check.str("stop kept", WN.Wn.report("stopped", M.Running{}), "stopped")
    C.Check.str("missing word falls back", WN.Wn.report("", M.Interrupted{}), "stopped")
    C.Check.str("body", body, "{\"event\":\"turn.ended\",\"kind\":\"fail\",\"thread\":\"t1\",\"state\":\"failed\",\"reason\":\"\",\"title\":\"Board\",\"text\":\"Failed\"}")
    C.Check.str("event", field(ok, "event"), "turn.ended")
    C.Check.str("kind", field(body, "kind"), "fail")
    C.Check.str("ask event", field(ask, "event"), "ask")
    C.Check.str("ask kind", field(ask, "kind"), "ask")
    C.Check.str("state", field(body, "state"), "failed")
    C.Check.str("title quote", field(WN.Wn.body("done", "t", "completed", "a\"b", "x"), "title"), "a\"b")
    C.Check.str("settle fires", yn(WN.Wn.fires(NO.Policy.kind(done))), "yes")
    C.Check.str("settle agrees", yn(WN.Wn.agrees(done)), "yes")
    C.Check.str("settle is the turn path", Nat.show(WN.Wn.route(NO.Policy.kind(done))), "1")
    C.Check.str("stop is quiet", yn(WN.Wn.fires(NO.Policy.kind(stop))), "no")
    C.Check.str("stop agrees", yn(WN.Wn.agrees(stop)), "yes")
    C.Check.str("stop posts nothing", Nat.show(WN.Wn.route(NO.Policy.kind(stop))), "0")
    C.Check.str("ask fires", yn(WN.Wn.fires(NO.Policy.kind(asked))), "yes")
    C.Check.str("ask agrees", yn(WN.Wn.agrees(asked)), "yes")
    C.Check.str("ask is the alert path", Nat.show(WN.Wn.route("ask")), "2")
    C.Check.str("merge is the alert path", Nat.show(WN.Wn.route("merged")), "2")
    C.Check.str("reminder is the alert path", Nat.show(WN.Wn.route("reminder")), "2")
    C.Check.str("note is the alert path", Nat.show(WN.Wn.route("note")), "2")
    C.Check.str("empty kind posts nothing", Nat.show(WN.Wn.route("")), "0")
    C.Check.str("failure is the turn path", Nat.show(WN.Wn.route("fail")), "1")
    C.Check.str("reason empty on a settle", field(ok, "reason"), "")
    C.Check.str("limit reason", field(WN.Wn.body.r("fail", "t1", "failed", WN.Wn.reason("failed", "Claude AI usage limit reached|1792800000"), "B", "x"), "reason"), "limit")
    C.Check.str("hit your limit", WN.Wn.reason("failed", "You've hit your limit · resets 8pm"), "limit")
    C.Check.str("error reason", WN.Wn.reason("failed", "API Error: 500"), "error")
    C.Check.str("interrupted reason", WN.Wn.reason("interrupted", ""), "interrupted")
    C.Check.str("interrupted by a limit", WN.Wn.reason("interrupted", "usage limit reached"), "limit")
    C.Check.str("no reason on a stop", WN.Wn.reason("stopped", "usage limit reached"), "")
    C.Check.str("quiet failure posts", yn(WN.Wn.due.end("", "failed", True{}, False{})), "yes")
    C.Check.str("quiet interrupt posts", yn(WN.Wn.due.end("", "interrupted", True{}, False{})), "yes")
    C.Check.str("stop still quiet", yn(WN.Wn.due.end("", "stopped", True{}, False{})), "no")
    C.Check.str("settled off: quiet failure", yn(WN.Wn.due.end("", "failed", False{}, False{})), "no")
    C.Check.str("broken turn is a fail", WN.Wn.end.kind("", "interrupted"), "fail")
    C.Check.str("policy kind kept", WN.Wn.end.kind("done", "completed"), "done")
    C.Check.str("interrupted word kept", WN.Wn.report("interrupted", M.Idle{}), "interrupted")
    C.Check.str("text secret", WN.Wn.sig("secret", 10n, body), "sha256=0df3b999b5907d0df7f87cb619ab755d4a9559f30e6b0491864301418b661666")
    C.Check.str("hex secret", WN.Wn.sig("00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff", 11n, body), "sha256=38a0e230e021a7f27dbeca05d42804424489cdcf40b9717ab6819a67345be218")
    C.Check.str("signature checks", yn(SG.Sign.check("secret", 10n, 10n, body, WN.Wn.sig("secret", 10n, body))), "yes")
    C.Check.str("tampered refused", yn(SG.Sign.check("secret", 10n, 10n, body, "sha256=00")), "no")
    C.Check.str("unsigned headers", hdr("", 10n, body), "Content-Type: application/json")
    C.Check.str("signed headers", hdr("secret", 10n, body), "Content-Type: application/json\nX-Backplane-Timestamp: 10\nX-Backplane-Signature: sha256=0df3b999b5907d0df7f87cb619ab755d4a9559f30e6b0491864301418b661666")
    C.Check.str("safe name", WN.Wn.safe("t1"), "t1")
    C.Check.str("safe slash", WN.Wn.safe("a/b c"), "a_b_c")
    C.Check.str("stale record is kept", NO.Alert.kind(NO.Alert.get(s2, "t1")), "done")
    C.Check.str("a quiet end writes no kind", later, "")
    C.Check.str("a quiet end posts nothing", yn(WN.Wn.due.end(later, "completed", True{}, False{})), "no")
    C.Check.str("a quiet end with settled off", yn(WN.Wn.due.end(later, "completed", False{}, False{})), "no")
    C.Check.str("triage worker is private", yn(WN.Wn.private(managed(), "worker")), "yes")
    C.Check.str("policy keeps it quiet", NO.Policy.kind(NO.Policy.end(managed(), 4n, "worker", "failed")), "")
    C.Check.str("private failure posts nothing", yn(WN.Wn.due.end(NO.Policy.kind(NO.Policy.end(managed(), 4n, "worker", "failed")), "failed", True{}, WN.Wn.private(managed(), "worker"))), "no")
    C.Check.str("private interrupt posts nothing", yn(WN.Wn.due.end("", "interrupted", True{}, True{})), "no")
    C.Check.str("the person's thread is not private", yn(WN.Wn.private(s1, "t1")), "no")
    C.Check.str("older turn's error not taken", WN.Wn.err(older(), "@m2", "t1"), "")
    C.Check.str("interrupt keeps its own reason", WN.Wn.reason("interrupted", WN.Wn.err(older(), "@m2", "t1")), "interrupted")
    C.Check.str("this turn's error taken", WN.Wn.err(current(), "@m2", "t1"), "API Error: 500")
    C.Check.str("this turn's error classified", WN.Wn.reason("failed", WN.Wn.err(current(), "@m2", "t1")), "error")
    C.Check.str("absent default is unsigned", Nat.show(WN.Wn.mode(WN.Wn.wants("", WN.Wn.seen.fail(2)), "")), "1")
    C.Check.str("unreadable default posts nothing", Nat.show(WN.Wn.mode(WN.Wn.wants("", WN.SeenFile{}), "")), "0")
    C.Check.str("readable default signs", Nat.show(WN.Wn.mode(WN.Wn.wants(" ", WN.SeenFile{}), "k")), "2")
    C.Check.str("named empty file posts nothing", Nat.show(WN.Wn.mode(WN.Wn.wants("/tmp/s", WN.SeenFile{}), "")), "0")
    C.Check.str("default stat EACCES posts nothing", Nat.show(WN.Wn.mode(WN.Wn.wants("", WN.Wn.seen.fail(13)), "")), "0")
    C.Check.str("default stat EIO posts nothing", Nat.show(WN.Wn.mode(WN.Wn.wants("", WN.Wn.seen.fail(5)), "")), "0")
    C.Check.str("only ENOENT is absent", Bool.show(WN.Wn.absent(WN.Wn.seen.fail(2))), "True")
    C.Check.str("EACCES is not absent", Bool.show(WN.Wn.absent(WN.Wn.seen.fail(13))), "False")
    C.Check.str("short hex is raw bytes", WN.Wn.sig("deadbeef", 10n, body), "sha256=068662f0b5260bd58dfcd9a4df964a0b956e7a36d24dfe5430ff2ffe6f74a351")
    C.Check.str("odd-length hex is text", WN.Wn.sig("deadbee", 10n, body), "sha256=b76febb25b10ab2e866cdf2b62c0a360a30ffa30286401d08f7f5ed7972de0b2")
    C.Check.str("reply is this turn's", WN.Wn.reply(limited(), "@m3", "t1"), "Claude AI usage limit reached|1792800000")
    C.Check.str("no reply in this turn", WN.Wn.reply(older(), "@m2", "t1"), "working on it")
    C.Check.str("limit kept from the reply", WN.Wn.end.reason("interrupted", WN.Wn.err(limited(), "@m3", "t1"), WN.Wn.reply(limited(), "@m3", "t1")), "limit")
    C.Check.str("failed with a limit reply", WN.Wn.end.reason("failed", "", "You've hit your limit · resets 8pm"), "limit")
    C.Check.str("ordinary reply is no error word", WN.Wn.end.reason("failed", "", "I could not finish"), "error")
    C.Check.str("ordinary reply on an interrupt", WN.Wn.end.reason("interrupted", "", "working on it"), "interrupted")
    C.Check.str("logged error wins over the reply", WN.Wn.end.reason("failed", "API Error: 500", "usage limit reached"), "error")
    C.Check.str("older turn's limit not taken", WN.Wn.end.reason("interrupted", WN.Wn.err(older(), "@m2", "t1"), WN.Wn.reply(older(), "@m2", "t1")), "interrupted")
    C.Check.str("model error is the turn's", WN.Wn.err(badmodel(), "@m4", "t1"), "There's an issue with the selected model (no-such-model-webhook-test)")
    C.Check.str("interrupt on a model error is an error", WN.Wn.end.reason("interrupted", WN.Wn.err(badmodel(), "@m4", "t1"), WN.Wn.reply(badmodel(), "@m4", "t1")), "error")
    C.Check.str("failed on a model error is an error", WN.Wn.end.reason("failed", WN.Wn.err(badmodel(), "@m4", "t1"), ""), "error")
    C.Check.str("interrupt with a limit error stays limit", WN.Wn.end.reason("interrupted", "usage limit reached", ""), "limit")
    C.Check.str("plain interrupt stays interrupted", WN.Wn.end.reason("interrupted", "", ""), "interrupted")
    C.Check.str("no reply of its own: not the older turn's", WN.Wn.end.text("interrupted", WN.Wn.reply(noreply(), "@m5", "t1")), "Failed")
    C.Check.str("interrupted with its own reply", WN.Wn.end.text("interrupted", WN.Wn.reply(older(), "@m2", "t1")), "working on it")
    C.Check.str("failed says Failed", WN.Wn.end.text("failed", "working on it"), "Failed")
    C.Check.str("settled with no reply", WN.Wn.end.text("completed", ""), "Finished")
    C.Check.str("settled with its reply", WN.Wn.end.text("completed", "done"), "done")
    C.Check.str("a stop has no reason", WN.Wn.end.reason("stopped", "", "usage limit reached"), "")
    C.Check.str("long history: stops at the turn's start", WN.Wn.err(M.Act{"e9", "t1", "error", "result", "now failing", 10n} <> (M.Msg{"m9", "t1", M.User{}, "go", 9n} <> long(20000n, M.Act{"e0", "t1", "error", "result", "oldest", 0n})), "@m9", "t1"), "now failing")
    C.Check.str("long history: an empty turn reads none", WN.Wn.err(M.Msg{"m9", "t1", M.User{}, "go", 9n} <> long(20000n, M.Act{"e0", "t1", "error", "result", "oldest", 0n}), "@m9", "t1"), "")
    C.Check.str("long history: walked to its end without the stack", WN.Wn.err(long(20000n, M.Act{"e0", "t1", "error", "result", "oldest", 0n}), "@none", "t1"), "oldest")
    C.Check.str("long history: reply found once both are", WN.Wn.reply(M.Msg{"a9", "t1", M.Assistant{}, "done", 11n} <> (M.Act{"e9", "t1", "error", "result", "now failing", 10n} <> long(20000n, M.Msg{"m0", "t1", M.User{}, "go", 0n})), "@m0", "t1"), "done")
    C.Check.str("webhook off: nothing read", WN.Wn.got.err(WN.Wn.cap(hooked(""), "t1")), "")
    C.Check.str("webhook blank: nothing read", WN.Wn.got.err(WN.Wn.cap(hooked("  "), "t1")), "")
    C.Check.str("webhook on: the turn's error", WN.Wn.got.err(WN.Wn.cap(hooked("https://example.com/h"), "t1")), "API Error: 500")
    C.Check.str("no start mark: nothing guessed", WN.Wn.got.err(WN.Wn.cap(hooked.m("https://example.com/h", ""), "t1")), "")
    C.Check.str("mark of an empty thread", WN.Wn.mark(Nil{}), "@")
    C.Check.str("mark is the newest entry", WN.Wn.mark(older()), "@a2")
