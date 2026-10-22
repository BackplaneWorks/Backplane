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

# the file the store builder writes its progress to
def Early.build.path(+home: String) -> String:
  home ++ "/store.build"
