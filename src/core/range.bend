import Base

# Byte ranges (HTTP Range, RFC 9110 14.1.2). A hub serves files from disk a
# piece at a time (server.bend's Img.*, host.js's Sock.send_path), so a
# viewer, a PDF reader or a video can ask for any part of a big file
# without the hub reading the rest. One range only: "bytes=a-b",
# "bytes=a-" (to the end) and "bytes=-n" (the last n); anything else is
# the whole file, and a range that starts past the end is unsatisfiable
# (416). Laws range_*.

# a decimal count; None for an empty text or any other character
def Rng.digit(+c: U32) -> Bool:
  Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57))

def Rng.num.go(s: String, acc: Maybe<&2, Nat>) -> Maybe<&2, Nat>:
  match s acc:
    case SNil{} _:
      acc
    case SCon{Chr{+c}, t} Some{+n}:
      Rng.num.go(t, Bool.pick(Maybe<&2, Nat>, Rng.digit(c), Some{Nat.add(Nat.mul(n, 10n), U32.to_nat((c - 48 : U32)))}, None{}))
    case SCon{Chr{+c}, t} None{}:
      None{}

def Rng.num.k(empty: Bool, +s: String) -> Maybe<&2, Nat>:
  match empty:
    case True{}:
      None{}
    case False{}:
      Rng.num.go(s, Some{0n})

def Rng.num(+s: String) -> Maybe<&2, Nat>:
  Rng.num.k(String.is_empty(s), String.trim(s))

# what to send: from start, len bytes; part says 206 (else 200), ok False
# is a 416
type Rng is Data:
  Rng{start: Nat, len: Nat, part: Bool, ok: Bool}

def Rng.whole(+size: Nat) -> Rng:
  Rng{0n, size, False{}, True{}}

def Rng.start(r: Rng) -> Nat:
  match r:
    case Rng{s, _, _, _}: s

def Rng.len(r: Rng) -> Nat:
  match r:
    case Rng{_, n, _, _}: n

def Rng.part(r: Rng) -> Bool:
  match r:
    case Rng{_, _, p, _}: p

def Rng.ok(r: Rng) -> Bool:
  match r:
    case Rng{_, _, _, o}: o

def Rng.span.to(back: Bool, +a: Nat, +b: Nat, +size: Nat) -> Rng:
  match back:
    case True{}:
      Rng.whole(size)
    case False{}:
      Rng{a, Nat.add(Nat.sub(b, a), 1n), True{}, True{}}

# a..b, both known (b clipped to the last byte)
def Rng.span.k(past: Bool, +a: Nat, +b: Nat, +size: Nat) -> Rng:
  match past:
    case True{}:
      Rng{0n, 0n, False{}, False{}}
    case False{}:
      Rng.span.to(Nat.is_lt(b, a), a, Bool.pick(Nat, Nat.is_ge(b, size), Nat.sub(size, 1n), b), size)

def Rng.tail.of(+n: Nat, +size: Nat) -> Rng:
  Rng{Nat.sub(size, n), n, True{}, True{}}


# the last n bytes (all of them when n is the size or more; none is a 416)
def Rng.tail.k(none: Bool, +n: Nat, +size: Nat) -> Rng:
  match none:
    case True{}:
      Rng{0n, 0n, False{}, False{}}
    case False{}:
      Rng.tail.of(Bool.pick(Nat, Nat.is_gt(n, size), size, n), size)

def Rng.pair(a: Maybe<&2, Nat>, b: Maybe<&2, Nat>, +size: Nat) -> Rng:
  match a b:
    case Some{+x} Some{+y}:
      Rng.span.k(Nat.is_ge(x, size), x, y, size)
    case Some{+x} None{}:
      Rng.span.k(Nat.is_ge(x, size), x, size, size)
    case None{} Some{+n}:
      Rng.tail.k(Bool.or(Nat.is_eq(n, 0n), Nat.is_eq(size, 0n)), n, size)
    case None{} None{}:
      Rng.whole(size)

def Rng.dash(ps: List<&2, String>, +size: Nat) -> Rng:
  match ps:
    case +a <> +b <> Nil{}:
      Rng.pair(Rng.num(a), Rng.num(b), size)
    case _:
      Rng.whole(size)

def Rng.bytes(one: Bool, +spec: String, +size: Nat) -> Rng:
  match one:
    case True{}:
      Rng.dash(String.split(spec, '-'), size)
    case False{}:
      Rng.whole(size)

def Rng.of.k(bytes: Bool, +h: String, +size: Nat) -> Rng:
  match bytes:
    case True{}:
      Rng.bytes(Bool.not(String.contains(h, ",")), String.trim(String.drop(h, 6n)), size)
    case False{}:
      Rng.whole(size)

# the range a Range header asks for in a file of size bytes
def Rng.of(+h: String, +size: Nat) -> Rng:
  Rng.of.k(String.starts_with(String.trim(h), "bytes="), String.trim(h), size)

# the head of the answer for it, up to the blank line (a 416 names the size)
def Rng.head(r: Rng, +size: Nat, +ctype: String, +cache: String) -> String:
  match r:
    case Rng{+a, +n, True{}, True{}}:
      "HTTP/1.1 206 Partial Content\r\nContent-Type: " ++ ctype ++ "\r\nContent-Length: " ++ Nat.show(n)
        ++ "\r\nContent-Range: bytes " ++ Nat.show(a) ++ "-" ++ Nat.show(Nat.sub(Nat.add(a, n), 1n)) ++ "/" ++ Nat.show(size)
        ++ "\r\nAccept-Ranges: bytes\r\nCache-Control: " ++ cache ++ "\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n"
    case Rng{_, +n, False{}, True{}}:
      "HTTP/1.1 200 OK\r\nContent-Type: " ++ ctype ++ "\r\nContent-Length: " ++ Nat.show(n)
        ++ "\r\nAccept-Ranges: bytes\r\nCache-Control: " ++ cache ++ "\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n"
    case Rng{_, _, _, False{}}:
      "HTTP/1.1 416 Range Not Satisfiable\r\nContent-Range: bytes */" ++ Nat.show(size) ++ "\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
