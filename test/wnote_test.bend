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

def main() -> IO(Unit):
  do IO<Unit>:
    +body : String = WN.Wn.body("fail", "t1", "failed", "Board", "Failed")
    +ok : String = WN.Wn.body("done", "t1", "completed", "A", "done")
    +ask : String = WN.Wn.body("ask", "t1", "running", "Board", "Needs you")
    +done : List<&2, M.Change> = NO.Policy.end(NO.Policy.demo.s(NO.Policy.demo.ask()), 13n, "t1", "completed")
    +stop : List<&2, M.Change> = NO.Policy.end(NO.Policy.demo.s(NO.Policy.demo.cat([NO.Policy.demo.base(), NO.Policy.demo.run("t1", "m1", "do it", 10n), [M.TurnChanged{"t1", M.Interrupted{}, 12n}]])), 13n, "t1", "stopped")
    +asked : List<&2, M.Change> = NO.Policy.ask(NO.Policy.demo.s(NO.Policy.demo.ask()), 20n, "t1", "Bash", "ls")
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
    C.Check.str("quiet failure posts", yn(WN.Wn.due.end("", "failed", True{})), "yes")
    C.Check.str("quiet interrupt posts", yn(WN.Wn.due.end("", "interrupted", True{})), "yes")
    C.Check.str("stop still quiet", yn(WN.Wn.due.end("", "stopped", True{})), "no")
    C.Check.str("settled off: quiet failure", yn(WN.Wn.due.end("", "failed", False{})), "no")
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
