import Base
import ./check.bend as C
import ../src/core/json.bend as J
import ../src/core/model.bend as M
import ../src/core/sign.bend as SG
import ../src/core/wnote.bend as WN

# The turn-end webhook (core/wnote.bend): when it posts, the JSON, and the
# HMAC-SHA256 header. The vectors are HMAC-SHA256(key, ts + "." + body)
# from Python's hmac (a hex secret is the raw key, Sign.key).

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
    +body : String = WN.Wn.body("t1", "failed", "Board", "Failed")
    +ok : String = WN.Wn.body("t1", "completed", "A", "done")
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
    C.Check.str("body", body, "{\"event\":\"turn.ended\",\"thread\":\"t1\",\"state\":\"failed\",\"title\":\"Board\",\"text\":\"Failed\"}")
    C.Check.str("event", field(ok, "event"), "turn.ended")
    C.Check.str("state", field(body, "state"), "failed")
    C.Check.str("title quote", field(WN.Wn.body("t", "completed", "a\"b", "x"), "title"), "a\"b")
    C.Check.str("text secret", WN.Wn.sig("secret", 10n, body), "sha256=6fa8a58774ca1f57e31ba691267838657bf5c12db0427ea900cd42ee8b9d910e")
    C.Check.str("hex secret", WN.Wn.sig("00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff", 11n, body), "sha256=fa5a570a7509b54ecbcb1b0813de5ed1922255eeff168c498164fec432cce6ef")
    C.Check.str("signature checks", yn(SG.Sign.check("secret", 10n, 10n, body, WN.Wn.sig("secret", 10n, body))), "yes")
    C.Check.str("tampered refused", yn(SG.Sign.check("secret", 10n, 10n, body, "sha256=00")), "no")
    C.Check.str("unsigned headers", hdr("", 10n, body), "Content-Type: application/json")
    C.Check.str("signed headers", hdr("secret", 10n, body), "Content-Type: application/json\nX-Backplane-Timestamp: 10\nX-Backplane-Signature: sha256=6fa8a58774ca1f57e31ba691267838657bf5c12db0427ea900cd42ee8b9d910e")
    C.Check.str("safe name", WN.Wn.safe("t1"), "t1")
    C.Check.str("safe slash", WN.Wn.safe("a/b c"), "a_b_c")
