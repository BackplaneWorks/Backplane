import Base
import ./json.bend as J
import ./model.bend as M
import ./notice.bend as NO
import ./sign.bend as SG

# An outbound notice posted to push.webhook.url (server/push.bend). It
# follows Notify me for (notice.bend's Policy): a post goes out only for
# an alert record whose kind the policy would notify on. A settled turn
# or a failure (kind done or fail) is event "turn.ended" and keeps the
# end word TurnEnded reported. An ask, a merge, a reminder or a
# notify_person note uses the kind as its event. Off unless the URL is
# an http(s) address. The body is JSON. When a secret is configured the
# request carries the same signature a webhook in does (sign.bend):
#
#   X-Backplane-Timestamp: <unix seconds>
#   X-Backplane-Signature: sha256=<hex HMAC-SHA256(secret, ts + "." + body)>
#
# The secret is optional. With none, the notice goes out unsigned. A
# secret the caller asked for and could not read is mode 0: nothing is
# posted, so an unsigned notice is never sent in its place.

def Wn.url.key() -> String:
  "push.webhook.url"

def Wn.secret.key() -> String:
  "push.webhook.secret"

# the file read when push.webhook.secret is unset (0600, like other secrets)
def Wn.file(+home: String) -> String:
  home ++ "/secrets/push.webhook"

def Wn.asked(+set: String) -> Bool:
  Bool.not(String.is_empty(String.trim(set)))

def Wn.path.k(empty: Bool, +home: String, +set: String) -> String:
  match empty:
    case True{}:
      Wn.file(home)
    case False{}:
      set

# the secret's file: the setting when it names one, else the default
def Wn.path.go(+set: String, +home: String) -> String:
  Wn.path.k(String.is_empty(set), home, set)

def Wn.path(+home: String, +raw: String) -> String:
  Wn.path.go(String.trim(raw), home)

# 0: a configured secret is missing (do not post)
# 1: post with no signature
# 2: post signed
def Wn.mode.k(asked: Bool, empty: Bool) -> Nat:
  match asked empty:
    case True{} True{}:
      0n
    case True{} False{}:
      2n
    case False{} True{}:
      1n
    case False{} False{}:
      2n

def Wn.mode(asked: Bool, +secret: String) -> Nat:
  Wn.mode.k(asked, String.is_empty(secret))

def Wn.on(+url: String) -> Bool:
  Bool.not(String.is_empty(url))

# printable ASCII, no spaces or controls (a curl config value, not a file)
def Wn.urlc(u: String) -> Bool:
  match u:
    case SNil{}:
      True{}
    case SCon{Chr{+c}, t}:
      Bool.and(Bool.and(U32.is_gt(c, 32), U32.is_lt(c, 127)), Wn.urlc(t))

def Wn.url.ok(+u: String) -> Bool:
  Bool.and(Bool.or(String.starts_with(u, "http://"), String.starts_with(u, "https://")),
    Bool.and(Nat.is_le(String.length(u), 2000n), Wn.urlc(u)))

def Wn.send(+url: String) -> Bool:
  Bool.and(Wn.on(url), Wn.url.ok(url))

def Wn.live(+raw: String) -> Bool:
  Wn.send(String.trim(raw))

# the word TurnEnded uses: a stop is "stopped", not the codec's "interrupted"
def Wn.outcome(t: M.TurnState) -> String:
  match t:
    case M.Failed{}:
      "failed"
    case M.Interrupted{}:
      "stopped"
    case M.Completed{}:
      "completed"
    case M.Running{}:
      "running"
    case M.Idle{}:
      "idle"

def Wn.known(+s: String) -> Bool:
  Bool.or(Bool.or(String.eq(s, "completed"), String.eq(s, "failed")), String.eq(s, "stopped"))

# the end the hub reported, or the thread's turn when that word is absent
# (a queued message can start the next turn before the notice is posted)
def Wn.report.k(known: Bool, +given: String, t: M.TurnState) -> String:
  match known:
    case True{}:
      given
    case False{}:
      Wn.outcome(t)

def Wn.report(+given: String, t: M.TurnState) -> String:
  Wn.report.k(Wn.known(given), given, t)

# a settled completion or a failure: the turn-end notice, with the end word
def Wn.ended.or(done: Bool, fail: Bool) -> Bool:
  match done:
    case True{}:
      True{}
    case False{}:
      fail

def Wn.ended(+kind: String) -> Bool:
  Wn.ended.or(String.eq(kind, "done"), String.eq(kind, "fail"))

def Wn.event.k(ended: Bool, +kind: String) -> String:
  match ended:
    case True{}:
      "turn.ended"
    case False{}:
      kind

# "turn.ended" for a settle or a failure; otherwise the alert's own kind
def Wn.event(+kind: String) -> String:
  Wn.event.k(Wn.ended(kind), kind)

# the policy's own test, on a record of this kind (an empty kind is the
# quiet settle: Policy.said reads "_", so nothing is posted)
def Wn.fires(+kind: String) -> Bool:
  NO.Policy.loud([M.SettingSet{NO.Alert.key("t"), NO.Alert.line(kind, "k", "", "1.0", ""), 1n}])

def Wn.same(a: Bool, b: Bool) -> Bool:
  match a b:
    case True{} True{}:
      True{}
    case False{} False{}:
      True{}
    case True{} False{}:
      False{}
    case False{} True{}:
      False{}

# the webhook posts for a change list exactly when the policy would notify
def Wn.agrees(+cs: List<&2, M.Change>) -> Bool:
  Wn.same(Wn.fires(NO.Policy.kind(cs)), NO.Policy.loud(cs))

def Wn.route.e(ended: Bool) -> Nat:
  match ended:
    case True{}:
      1n
    case False{}:
      2n

def Wn.route.f(fires: Bool, ended: Bool) -> Nat:
  match fires:
    case False{}:
      0n
    case True{}:
      Wn.route.e(ended)

# 0 quiet, 1 the turn-end path (done, fail), 2 the alert path (ask, merged, reminder, note)
def Wn.route(+kind: String) -> Nat:
  Wn.route.f(Wn.fires(kind), Wn.ended(kind))

def Wn.turn(route: Nat) -> Bool:
  match route:
    case 1n:
      True{}
    case _:
      False{}

def Wn.other(route: Nat) -> Bool:
  match route:
    case 2n:
      True{}
    case _:
      False{}

# TurnEnded posts only a settle or a failure the policy raised
def Wn.due.turn(+kind: String) -> Bool:
  Wn.turn(Wn.route(kind))

# Alerted posts the other kinds (so a settle is not sent twice)
def Wn.due.alert(+kind: String) -> Bool:
  Wn.other(Wn.route(kind))

def Wn.body(+kind: String, +thread: String, +state: String, +title: String, +text: String) -> String:
  J.Json.show(J.Json.obj([
    J.KV{"event", J.Str{Wn.event(kind)}},
    J.KV{"kind", J.Str{kind}},
    J.KV{"thread", J.Str{thread}},
    J.KV{"state", J.Str{state}},
    J.KV{"title", J.Str{title}},
    J.KV{"text", J.Str{text}}]))

def Wn.signed(+secret: String) -> Bool:
  Bool.not(String.is_empty(secret))

def Wn.sig(+secret: String, +ts: Nat, +body: String) -> String:
  SG.Sign.header(secret, ts, body)

def Wn.headers.k(sign: Bool, +secret: String, +ts: Nat, +body: String) -> List<&2, String>:
  match sign:
    case False{}:
      ["Content-Type: application/json"]
    case True{}:
      ["Content-Type: application/json", "X-Backplane-Timestamp: " ++ Nat.show(ts),
        "X-Backplane-Signature: " ++ Wn.sig(secret, ts, body)]

def Wn.headers(+secret: String, +ts: Nat, +body: String) -> List<&2, String>:
  Wn.headers.k(Wn.signed(secret), secret, ts, body)

def Wn.ch(+c: U32) -> Bool:
  Bool.or(Bool.or(Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57)), Bool.and(U32.is_ge(c, 97), U32.is_le(c, 122))),
    Bool.or(Bool.and(U32.is_ge(c, 65), U32.is_le(c, 90)), Bool.or(U32.is_eq(c, 45), U32.is_eq(c, 95))))

def Wn.safe.put(ok: Bool, +c: U32, +acc: String) -> String:
  match ok:
    case True{}:
      SCon{Chr{c}, acc}
    case False{}:
      SCon{Chr{95}, acc}

def Wn.safe.go(s: String, +acc: String) -> String:
  match s:
    case SNil{}:
      String.reverse(acc)
    case SCon{Chr{+c}, t}:
      Wn.safe.go(t, Wn.safe.put(Wn.ch(c), c, acc))

# a thread id safe to put in a temp file name
def Wn.safe(+thread: String) -> String:
  String.take(Wn.safe.go(thread, ""), 80n)
