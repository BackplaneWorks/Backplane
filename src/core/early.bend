import Base
import ./json.bend as J
import ./utf8.bend as U
import ./http.bend as W

# The hub while it starts (pure). The hub binds its port before it loads
# its state (server.bend's Early.*), which can take minutes when it first
# builds store.db from the event log; meanwhile every request gets this
# 503 with Retry-After, so the clients wait instead of failing. The note
# says how far the build is from <home>/store.build ("<bytes done> <bytes
# total>", written by the store builder); no such file: "starting".
# Laws early_*; test test/early_test.bend.

def Early.digit(+c: U32) -> Bool:
  Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57))

# a word in decimal, its digits checked as it goes (a tail call)
def Early.dec(s: String, +acc: Nat, +ok: Bool) -> Maybe<&2, Nat>:
  match s:
    case SNil{}:
      Bool.pick(Maybe<&2, Nat>, ok, Some{acc}, None{})
    case SCon{Chr{+c}, t}:
      Early.dec(t, Nat.add(Nat.mul(acc, 10n), U32.to_nat((c - 48 : U32))), Bool.and(ok, Early.digit(c)))

def Early.word(+s: String) -> Maybe<&2, Nat>:
  Early.dec(s, 0n, Bool.not(String.is_empty(s)))

def Early.pct.k(zero: Bool, +done: Nat, +total: Nat) -> Nat:
  match zero:
    case True{}:
      0n
    case False{}:
      Bool.pick(Nat, Nat.is_ge(done, total), 100n, Nat.div(Nat.mul(done, 100n), total))

# done of total as a whole percent, at most 100 (no total: 0)
def Early.pct(+done: Nat, +total: Nat) -> Nat:
  Early.pct.k(Nat.is_eq(total, 0n), done, total)

def Early.pair(a: Maybe<&2, Nat>, b: Maybe<&2, Nat>) -> String:
  match a b:
    case Some{+d} Some{+t}:
      "building store " ++ Nat.show(Early.pct(d, t)) ++ "%"
    case _ _:
      "starting"

def Early.words(ws: List<&2, String>) -> String:
  match ws:
    case +a <> +b <> Nil{}:
      Early.pair(Early.word(a), Early.word(b))
    case _:
      "starting"

def Early.line(ls: List<&2, String>) -> String:
  match ls:
    case +l <> _:
      Early.words(String.split(l, ' '))
    case Nil{}:
      "starting"

# the note from store.build's text ("" when there is none)
def Early.note(+build: String) -> String:
  Early.line(String.split(build, '\n'))

def Early.body(+version: String, +build: String) -> String:
  J.Json.show(J.Json.obj([
    J.KV{"status", J.Str{"starting"}},
    J.KV{"note", J.Str{Early.note(build)}},
    J.KV{"app", J.Str{"backplane"}},
    J.KV{"version", J.Str{version}}]))

# every request's answer while the hub starts, a WebSocket upgrade's too
def Early.response(+version: String, +build: String) -> List<&2, U32>:
  W.Http.response.c("503 Service Unavailable", "application/json", U.Utf8.encode(Early.body(version, build)), "no-store", "Retry-After: 2\r\n")

# A browser's first visit (no service worker yet) gets a page, not JSON:
# the same 503 and Retry-After, the progress in words, and a meta refresh
# with no URL, so the browser reloads the very URL it asked for (query and
# pairing token included). Nothing from the request goes into the page.
def Early.html.note.k(building: Bool, +note: String) -> String:
  match building:
    case True{}:
      "Building the database\u{2026} " ++ String.drop(note, 15n)
    case False{}:
      "Starting\u{2026}"

def Early.html.note(+build: String) -> String:
  Early.html.note.k(String.starts_with(Early.note(build), "building store "), Early.note(build))

def Early.page(+build: String) -> String:
  "<!doctype html><html><head><meta charset=\"utf-8\"><meta http-equiv=\"refresh\" content=\"2\">"
    ++ "<meta name=\"viewport\" content=\"width=device-width\"><title>Backplane</title></head>"
    ++ "<body style=\"font-family:sans-serif;margin:3em\"><p>" ++ Early.html.note(build)
    ++ "</p><p>Upgrading your local database may take a few minutes on first startup. Please keep Backplane open.</p></body></html>"

def Early.html.response(+build: String) -> List<&2, U32>:
  W.Http.response.c("503 Service Unavailable", "text/html; charset=utf-8", U.Utf8.encode(Early.page(build)), "no-store", "Retry-After: 2\r\n")

# a page for a browser asking for HTML; anything else (an API call, a
# WebSocket upgrade) the JSON
def Early.html.wanted(+accept: String, +upgrade: String) -> Bool:
  Bool.and(String.contains(String.to_lower(accept), "text/html"), String.is_empty(upgrade))

def Early.for.k(html: Bool, +version: String, +build: String) -> List<&2, U32>:
  match html:
    case True{}:
      Early.html.response(build)
    case False{}:
      Early.response(version, build)

def Early.for.req(r: W.Request, +version: String, +build: String) -> List<&2, U32>:
  match r:
    case W.Request{_, _, +hs}:
      Early.for.k(Early.html.wanted(W.Http.header_of(hs, "accept"), W.Http.header_of(hs, "upgrade")), version, build)

# the answer to a request whose head is these bytes (none read: the JSON)
def Early.for(head: List<&2, U32>, +version: String, +build: String) -> List<&2, U32>:
  Early.for.req(W.Http.request(head), version, build)

# the file the store builder writes its progress to
def Early.build.path(+home: String) -> String:
  home ++ "/store.build"

# BACKPLANE_BOOT_DELAY_MS (tests only: a boot made slow on purpose, so the
# early listener can be seen): milliseconds, anything else 0
def Early.delay.k(m: Maybe<&2, Nat>) -> U32:
  match m:
    case Some{+n}:
      Bool.pick(U32, Nat.is_ge(n, 3600000n), 3600000, U32.from_nat(n))
    case None{}:
      0

def Early.delay(+s: String) -> U32:
  Early.delay.k(Early.word(s))
