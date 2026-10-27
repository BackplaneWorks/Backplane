import Base
import ./json.bend as J
import ./model.bend as M
import ./notice.bend as NO
import ./sign.bend as SG
import ./fallback.bend as FB
import ./mailtriage.bend as TRI

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
# The secret is optional. With none (no setting, no default file), the
# notice goes out unsigned. A secret the setting names, or a default file
# that is there, that cannot be read or is empty is mode 0: nothing is
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

# a secret is wanted when the setting names a file, or when the default
# file is there (present): only a default that is absent leaves the
# notice unsigned. One there that cannot be read, or holds no key (a
# rotation half done), is mode 0, not a quiet downgrade to unsigned
def Wn.wants(+set: String, present: Bool) -> Bool:
  Bool.or(Wn.asked(set), present)

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
  Bool.or(Bool.or(String.eq(s, "completed"), String.eq(s, "failed")), Bool.or(String.eq(s, "stopped"), String.eq(s, "interrupted")))

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

# a turn that broke: it failed (an error, a usage limit) or the agent cut
# it off. A Stop the person asked for ("stopped") is not one.
def Wn.broke(+state: String) -> Bool:
  Bool.or(String.eq(state, "failed"), String.eq(state, "interrupted"))

# a thread whose ends the policy never alerts on (notice.bend's
# Policy.end.managed): a run of the private Mailroom triage, another bot
# while triage is on, a worker triage launched. Its failures reach the
# person only through the coordinator's own alert path.
def Wn.private(+s: M.State, +thread: String) -> Bool:
  TRI.Triage.managed(s, thread)

# A broken turn is posted as a failure whenever Notify me for would tell
# of a failure (settled: notify.settled), even when the policy kept it
# quiet: a turn no fresh message started (a bot's, a queued one) or an
# interrupted one. A limit or an error is never only in the log. A
# private thread (Wn.private) keeps the policy's guard: only a record the
# policy wrote for it posts.
def Wn.due.end(+kind: String, +state: String, settled: Bool, private: Bool) -> Bool:
  Bool.or(Wn.due.turn(kind), Bool.and(Bool.and(settled, Bool.not(private)), Wn.broke(state)))

# the kind a turn end posts under: the policy's, or "fail" for a broken
# turn the policy kept quiet
def Wn.end.kind(+kind: String, +state: String) -> String:
  Bool.pick(String, Wn.due.turn(kind), kind, Bool.pick(String, Wn.broke(state), "fail", kind))

# why a turn ended: "limit" (a usage, rate or quota limit, by the words
# fallback.bend's Error.words knows), "error" (any other failure),
# "interrupted", or "" (a settle, a stop, or not a turn end)
def Wn.reason.k(+limit: Bool, +state: String) -> String:
  Bool.pick(String, String.eq(state, "failed"), Bool.pick(String, limit, "limit", "error"),
    Bool.pick(String, String.eq(state, "interrupted"), Bool.pick(String, limit, "limit", "interrupted"), ""))

def Wn.reason(+state: String, +err: String) -> String:
  Wn.reason.k(FB.Error.words(String.to_lower(err)), state)

# The ending turn's error and reply, read by the hub when the turn ends
# (hub.bend's Ended.out.p) and carried in TurnEnded, so the webhook never
# searches history. The entries are newest first. A turn's identity is
# its start, not a message from the user: a wake starts a turn with none,
# and a message queued while a turn runs is logged inside it. So when a
# turn starts (any TurnChanged to Running, hub.bend's Turn.marks) the hub
# records the newest entry the thread had then, as turn.from.<thread> =
# "@<id>" ("@" when it had none). A scan stops at that entry and never
# takes an older turn's error or reply.
#
# One scan finds both, and stops at the boundary or as soon as both are
# found: each step's stop is decided before the next call, and the call is
# in tail position, so a long history is neither walked to its end nor
# held on the stack.

# what a turn's scan found: its newest error (an Act of tone "error": a
# failed result's text, an exit, an API error) and its newest reply, ""
# when none
type Got is Data:
  Got{err: String, reply: String}

def Wn.got.err(g: Got) -> String:
  match g:
    case Got{+err, _}:
      err

def Wn.got.reply(g: Got) -> String:
  match g:
    case Got{_, +reply}:
      reply

def Wn.got.none() -> Got:
  Got{"", ""}

def Wn.got.done(g: Got) -> Bool:
  match g:
    case Got{+err, +reply}:
      Bool.and(Bool.not(String.is_empty(err)), Bool.not(String.is_empty(reply)))

# the first of each kind wins (the scan runs newest first)
def Wn.got.keep(take: Bool, +found: String, +had: String) -> String:
  Bool.pick(String, Bool.and(take, String.is_empty(had)), found, had)

def Wn.got(e: M.Entry, +thread: String, +g: Got) -> Got:
  match e:
    case M.Act{_, +th, +tone, _, +summary, _}:
      Got{Wn.got.keep(Bool.and(String.eq(th, thread), String.eq(tone, "error")), summary, Wn.got.err(g)), Wn.got.reply(g)}
    case M.Msg{_, +th, M.Assistant{}, +text, _}:
      Got{Wn.got.err(g), Wn.got.keep(String.eq(th, thread), text, Wn.got.reply(g))}
    case _:
      g

def Wn.id(e: M.Entry) -> String:
  match e:
    case M.Msg{+id, _, _, _, _}:
      id
    case M.Act{+id, _, _, _, _, _}:
      id

# the setting a turn's start is kept under
def Wn.from.key(+thread: String) -> String:
  "turn.from." ++ thread

# a turn's start mark from the thread's entries as it starts (newest first)
def Wn.mark(es: List<&2, M.Entry>) -> String:
  match es:
    case Nil{}:
      "@"
    case e <> _:
      "@" ++ Wn.id(e)

# whether the scan stops before es: at the entry that was newest when the
# turn started (mark), at the end, or with both found
def Wn.stop(es: List<&2, M.Entry>, +mark: String, g: Got) -> Bool:
  match es:
    case e <> _:
      Bool.or(String.eq("@" ++ Wn.id(e), mark), Wn.got.done(g))
    case Nil{}:
      True{}

def Wn.scan(es: List<&2, M.Entry>, stop: Bool, +mark: String, +thread: String, +g: Got) -> Got:
  match es stop:
    case +e <> +t False{}:
      Wn.scan(t, Wn.stop(t, mark, Wn.got(e, thread, g)), mark, thread, Wn.got(e, thread, g))
    case _ _:
      g

# the error and reply of the turn that started at mark
def Wn.span(+es: List<&2, M.Entry>, +mark: String, +thread: String) -> Got:
  Wn.scan(es, Wn.stop(es, mark, Wn.got.none()), mark, thread, Wn.got.none())

def Wn.err(+es: List<&2, M.Entry>, +mark: String, +thread: String) -> String:
  Wn.got.err(Wn.span(es, mark, thread))

def Wn.reply(+es: List<&2, M.Entry>, +mark: String, +thread: String) -> String:
  Wn.got.reply(Wn.span(es, mark, thread))

# a turn with no start mark (one that started before marks were kept) is
# not guessed at: nothing is carried, and the notice says its state
def Wn.cap.m(known: Bool, +mark: String, +s: M.State, +thread: String) -> Got:
  match known:
    case True{}:
      Wn.span(M.State.mine(s, thread), mark, thread)
    case False{}:
      Wn.got.none()

def Wn.cap.mk(+mark: String, +s: M.State, +thread: String) -> Got:
  Wn.cap.m(Bool.not(String.is_empty(mark)), mark, s, thread)

def Wn.cap.k(on: Bool, +s: M.State, +thread: String) -> Got:
  match on:
    case True{}:
      Wn.cap.mk(M.Setting.get(s, Wn.from.key(thread), ""), s, thread)
    case False{}:
      Wn.got.none()

# what TurnEnded carries: the scan runs once, and only when a webhook is
# set (push.webhook.url); with none, nothing is read and nothing carried
def Wn.cap(+s: M.State, +thread: String) -> Got:
  Wn.cap.k(Wn.on(String.trim(M.Setting.get(s, Wn.url.key(), ""))), s, thread)

# a turn end's text from its own reply's first line: "Failed" for a
# failed turn (its error takes over when it logged one) or a broken one
# that gave no reply, "Finished" for a settle with none, else the line
def Wn.end.text(+state: String, +line: String) -> String:
  Bool.pick(String, Bool.or(String.eq(state, "failed"), Bool.and(Wn.broke(state), String.is_empty(line))), "Failed",
    Bool.pick(String, String.is_empty(line), "Finished", line))

# why the turn ended (Wn.reason): from its logged error; with none, its own
# reply is read, and only a limit's words (fallback.bend's Error.words)
# there change anything, so a reply never makes up an error. An
# interrupted turn that logged an error of its own (a model or API error,
# hub.bend's Claude.api.err) ended on that error: "error", not
# "interrupted". "" for a settle or a stop.
def Wn.end.reason.k(said: Bool, +r: String) -> String:
  Bool.pick(String, Bool.and(said, String.eq(r, "interrupted")), "error", r)

def Wn.end.reason(+state: String, +err: String, +reply: String) -> String:
  Wn.end.reason.k(Bool.not(String.is_empty(err)), Wn.reason(state, Bool.pick(String, String.is_empty(err), reply, err)))

# Alerted posts the other kinds (so a settle is not sent twice)
def Wn.due.alert(+kind: String) -> Bool:
  Wn.other(Wn.route(kind))

# reason: Wn.reason's word ("" when the notice is not a broken turn)
def Wn.body.r(+kind: String, +thread: String, +state: String, +reason: String, +title: String, +text: String) -> String:
  J.Json.show(J.Json.obj([
    J.KV{"event", J.Str{Wn.event(kind)}},
    J.KV{"kind", J.Str{kind}},
    J.KV{"thread", J.Str{thread}},
    J.KV{"state", J.Str{state}},
    J.KV{"reason", J.Str{reason}},
    J.KV{"title", J.Str{title}},
    J.KV{"text", J.Str{text}}]))

def Wn.body(+kind: String, +thread: String, +state: String, +title: String, +text: String) -> String:
  Wn.body.r(kind, thread, state, "", title, text)

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
