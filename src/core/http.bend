import Base
import ./utf8.bend as U
import ./sha1.bend as S
import ./json.bend as J
import ./cbor.bend as CB

# HTTP/1.1 requests and WebSocket (RFC 6455) frames over byte lists. Pure:
# the server feeds bytes in and gets requests, frames, and bytes out.

# Requests
# --------

type Header is Data:
  Header{name: String, value: String}

type Request is Data:
  Request{method: String, path: String, headers: List<&2, Header>}

# a request head (bytes before the blank line) and the bytes after it
type Split is Data:
  Split{head: List<&2, U32>, rest: List<&2, U32>}

# `seen` counts how much of \r\n\r\n the last bytes matched
def Http.split.want(+seen: Nat) -> U32:
  Bool.pick(U32, Nat.is_eq(Nat.mod(seen, 2n), 0n), 13, 10)

def Http.split.k(hit: Bool, +x: U32, +seen: Nat) -> Nat:
  match hit:
    case True{}:
      1n+seen
    case False{}:
      Bool.pick(Nat, U32.is_eq(x, 13), 1n, 0n)

def Http.split.go(bs: List<&2, U32>, seen: Nat, acc: List<&2, U32>) -> Maybe<&2, Split>:
  match bs seen:
    case _ 1n+1n+1n+1n+_:
      Some{Split{List.reverse(&2, U32, acc), bs}}
    case Nil{} _:
      None{}
    case +x <> t +s:
      Http.split.go(t, Http.split.k(U32.is_eq(x, Http.split.want(s)), x, s), x <> acc)

# the head and the rest once the headers are complete, else None
def Http.split(bs: List<&2, U32>) -> Maybe<&2, Split>:
  Http.split.go(bs, 0n, Nil{})

def Http.header.k(parts: List<&2, String>) -> Header:
  match parts:
    case Nil{}:
      Header{"", ""}
    case n <> vs:
      Header{String.to_lower(String.trim(n)), String.trim(String.join(vs, ":"))}

def Http.header(line: String) -> Header:
  Http.header.k(String.split(line, ':'))

def Http.headers(ls: List<&2, String>) -> List<&2, Header>:
  match ls:
    case Nil{}:
      Nil{}
    case l <> t:
      Http.header(String.trim_end(l)) <> Http.headers(t)

def Http.request.line(parts: List<&2, String>, hs: List<&2, String>) -> Request:
  match parts:
    case m <> p <> _:
      Request{m, p, Http.headers(hs)}
    case _:
      Request{"", "", Nil{}}

def Http.request.k(ls: List<&2, String>) -> Request:
  match ls:
    case Nil{}:
      Request{"", "", Nil{}}
    case l <> hs:
      Http.request.line(String.split(String.trim_end(l), ' '), hs)

def Http.request(head: List<&2, U32>) -> Request:
  Http.request.k(String.lines(U.Utf8.decode(head)))

def Http.get.hit(hs: List<&2, Header>, +name: String) -> Bool:
  match hs:
    case Header{n, _} <> _:
      String.eq(n, name)
    case Nil{}:
      False{}

def Http.get.go(hs: List<&2, Header>, hit: Bool, +name: String) -> String:
  match hs hit:
    case Header{_, v} <> _ True{}:
      v
    case _ <> +t False{}:
      Http.get.go(t, Http.get.hit(t, name), name)
    case _ _:
      ""

# a header's value by lower-case name, or ""
def Http.header_of(+hs: List<&2, Header>, +name: String) -> String:
  Http.get.go(hs, Http.get.hit(hs, name), name)

def Http.route.k(parts: List<&2, String>) -> String:
  match parts:
    case h <> _:
      h
    case Nil{}:
      "/"

# the path without its query string
def Http.param.go(ps: List<&2, String>, +name: String) -> String:
  match ps:
    case Nil{}:
      ""
    case +q <> t:
      Bool.pick(String, String.starts_with(q, name ++ "="), String.drop(q, 1n+String.length(name)), Http.param.go(t, name))

# a query parameter of a request path ("" when absent)
def Http.param(+p: String, +name: String) -> String:
  Http.param.go(String.split(Maybe.default(&2, String, List.head(&2, String, List.drop(&2, String, String.split(p, '?'), 1n)), ""), '&'), name)

def Http.route(p: String) -> String:
  Http.route.k(String.split(p, '?'))

# How many bytes, in a tail call: Base's List.length keeps a stack frame
# per cell, which the JS target (web, phones, the Bun hub) overflows on a
# body of a few hundred KB (laws http_count_go_succ, http_count_is_length)
def Http.count.go(bs: List<&2, U32>, +n: Nat) -> Nat:
  match bs:
    case Nil{}:
      n
    case _ <> t:
      Http.count.go(t, 1n+n)

def Http.count(bs: List<&2, U32>) -> Nat:
  Http.count.go(bs, 0n)

# a response with its own Cache-Control
def Http.response.c(status: String, ctype: String, body: List<&2, U32>, cache: String, extra: String) -> List<&2, U32>:
  +body2 = body
  List.append(&2, U32, U.Utf8.encode("HTTP/1.1 " ++ status ++ "\r\nContent-Type: " ++ ctype
    ++ "\r\nContent-Length: " ++ Nat.show(Http.count(body2))
    ++ "\r\nCache-Control: " ++ cache ++ "\r\nConnection: close\r\n" ++ extra ++ "\r\n"), body2)

# Responses
# ---------

def Http.response(status: String, ctype: String, body: List<&2, U32>, extra: String) -> List<&2, U32>:
  Http.response.c(status, ctype, body, "no-cache", extra)

def Http.text(status: String, body: String) -> List<&2, U32>:
  Http.response(status, "text/plain; charset=utf-8", U.Utf8.encode(body), "")

def Http.ctype.k(html: Bool, js: Bool, css: Bool, svg: Bool, json: Bool, png: Bool) -> String:
  match html js css svg json png:
    case True{} _ _ _ _ _:
      "text/html; charset=utf-8"
    case _ True{} _ _ _ _:
      "text/javascript; charset=utf-8"
    case _ _ True{} _ _ _:
      "text/css; charset=utf-8"
    case _ _ _ True{} _ _:
      "image/svg+xml"
    case _ _ _ _ True{} _:
      "application/json"
    case _ _ _ _ _ True{}:
      "image/png"
    case _ _ _ _ _ _:
      "application/octet-stream"

# the Content-Type for a path's extension
def Http.ctype(+p: String) -> String:
  Http.ctype.k(String.ends_with(p, ".html"), String.ends_with(p, ".js"),
    String.ends_with(p, ".css"), String.ends_with(p, ".svg"), String.ends_with(p, ".json"), String.ends_with(p, ".png"))

# the 101 response that accepts a WebSocket upgrade for this key
def Ws.accept(key: String) -> List<&2, U32>:
  U.Utf8.encode("HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: "
    ++ S.B64.encode(S.Sha1.hash(U.Utf8.encode(key ++ "258EAFA5-E914-47DA-95CA-C5AB0DC85B11"))) ++ "\r\n\r\n")

# WebSocket frames
# ----------------

type Frame is Data:
  Frame{fin: Bool, op: U32, payload: List<&2, U32>}

# a decoded frame and the bytes after it, or None until enough bytes arrived
type Got is Data:
  Got{frame: Frame, rest: List<&2, U32>}

def Ws.unmask.k(+x: U32, +key: List<&2, U32>, +i: Nat) -> U32:
  U32.xor(x, Maybe.default(&2, U32, List.get(&2, U32, key, Nat.mod(i, 4n)), 0))

def Ws.unmask(bs: List<&2, U32>, +key: List<&2, U32>, +i: Nat) -> List<&2, U32>:
  match bs:
    case Nil{}:
      Nil{}
    case x <> t:
      Ws.unmask.k(x, key, i) <> Ws.unmask(t, key, 1n+i)

# Ws.unmask in tail calls, onto acc reversed (laws ws_unmask_go,
# ws_unmask_take_go): a frame's payload is walked once, however long
def Ws.unmask.go(bs: List<&2, U32>, +key: List<&2, U32>, +i: Nat, acc: List<&2, U32>) -> List<&2, U32>:
  match bs:
    case Nil{}:
      acc
    case x <> t:
      Ws.unmask.go(t, key, 1n+i, Ws.unmask.k(x, key, i) <> acc)

# the first n bytes unmasked (Ws.unmask of List.take), reversed onto acc
def Ws.unmask.take.go(bs: List<&2, U32>, n: Nat, +key: List<&2, U32>, +i: Nat, acc: List<&2, U32>) -> List<&2, U32>:
  match bs n:
    case Nil{} _:
      acc
    case _ <> _ 0n:
      acc
    case x <> t 1n+m:
      Ws.unmask.take.go(t, m, key, 1n+i, Ws.unmask.k(x, key, i) <> acc)

def Ws.be(bs: List<&2, U32>, acc: Nat) -> Nat:
  match bs:
    case Nil{}:
      acc
    case x <> t:
      Ws.be(t, Nat.add(Nat.mul(acc, 256n), U32.to_nat(x)))

def Ws.body.fit(ok: Bool, fin: Bool, +op: U32, +key: List<&2, U32>, +len: Nat, +bs: List<&2, U32>) -> Maybe<&2, Got>:
  match ok:
    case True{}:
      Some{Got{Frame{fin, op, List.reverse(&2, U32, Ws.unmask.take.go(bs, len, key, 0n, Nil{}))}, List.drop(&2, U32, bs, len)}}
    case False{}:
      None{}

# whether bs holds at least n bytes, looking at no more than n of them (a
# buffer of many small frames is read in one pass, not one per frame)
def Ws.has(n: Nat, bs: List<&2, U32>) -> Bool:
  match n bs:
    case 0n _:
      True{}
    case 1n+k _ <> t:
      Ws.has(k, t)
    case 1n+k Nil{}:
      False{}

# after the length: the mask key (when masked) and the payload
def Ws.body(fin: Bool, +op: U32, masked: Bool, +len: Nat, +bs: List<&2, U32>) -> Maybe<&2, Got>:
  match masked:
    case True{}:
      Ws.body.fit(Ws.has(Nat.add(4n, len), bs), fin, op, List.take(&2, U32, bs, 4n), len, List.drop(&2, U32, bs, 4n))
    case False{}:
      Ws.body.fit(Ws.has(len, bs), fin, op, [0, 0, 0, 0], len, bs)

def Ws.len.k(short: Bool, mid: Bool, fin: Bool, +op: U32, masked: Bool, +n: U32, +bs: List<&2, U32>) -> Maybe<&2, Got>:
  match short mid:
    case True{} _:
      Ws.body(fin, op, masked, U32.to_nat(n), bs)
    case False{} True{}:
      Ws.body(fin, op, masked, Ws.be(List.take(&2, U32, bs, 2n), 0n), List.drop(&2, U32, bs, 2n))
    case False{} False{}:
      Ws.body(fin, op, masked, Ws.be(List.take(&2, U32, bs, 8n), 0n), List.drop(&2, U32, bs, 8n))

# one frame from the front of a byte stream, or None when incomplete
def Ws.decode(bs: List<&2, U32>) -> Maybe<&2, Got>:
  match bs:
    case +b0 <> +b1 <> rest:
      Ws.len.k(U32.is_lt(U32.and(b1, 127), 126), U32.is_eq(U32.and(b1, 127), 126),
        U32.is_ne(U32.and(b0, 128), 0), U32.and(b0, 15), U32.is_ne(U32.and(b1, 128), 0),
        U32.and(b1, 127), rest)
    case _:
      None{}

def Ws.len_bytes.k(short: Bool, mid: Bool, +n: Nat) -> List<&2, U32>:
  match short mid:
    case True{} _:
      [U32.from_nat(n)]
    case False{} True{}:
      [126, U32.from_nat(Nat.div(n, 256n)), U32.from_nat(Nat.mod(n, 256n))]
    case False{} False{}:
      [127, 0, 0, 0, 0,
        U32.from_nat(Nat.mod(Nat.div(n, 16777216n), 256n)),
        U32.from_nat(Nat.mod(Nat.div(n, 65536n), 256n)),
        U32.from_nat(Nat.mod(Nat.div(n, 256n), 256n)),
        U32.from_nat(Nat.mod(n, 256n))]

# an unmasked, final server frame
def Ws.encode(+op: U32, payload: List<&2, U32>) -> List<&2, U32>:
  +p = payload
  +n = Http.count(p)
  U32.or(128, op) <> List.append(&2, U32, Ws.len_bytes.k(Nat.is_lt(n, 126n), Nat.is_lt(n, 65536n), n), p)

def Ws.text(s: String) -> List<&2, U32>:
  Ws.encode(1, U.Utf8.encode(s))

# a message's JSON text as the CBOR the network carries
def Ws.cbor.bytes(t: String) -> List<&2, U32>:
  CB.Cbor.encode(Maybe.default(&2, J.Json, J.Json.parse(t), J.Null{}))

# a message as one binary CBOR frame: every frame the hub sends (law
# hub_frames_cbor)
def Ws.cbor(t: String) -> List<&2, U32>:
  Ws.encode(2, Ws.cbor.bytes(t))

# text as one binary frame of its UTF-8 bytes (the phones' plots)
def Ws.bin(s: String) -> List<&2, U32>:
  Ws.encode(2, U.Utf8.encode(s))

# Client side (a hub talking to another machine's hub)
# ----------------------------------------------------

# a client's frames are masked; the key only has to be there
def Ws.key() -> List<&2, U32>:
  [66, 80, 108, 110]

def Ws.mask_len(bs: List<&2, U32>) -> List<&2, U32>:
  match bs:
    case x <> t:
      U32.or(x, 128) <> t
    case Nil{}:
      Nil{}

# a masked, final client frame
def Ws.encode_masked(+op: U32, payload: List<&2, U32>) -> List<&2, U32>:
  +p = payload
  +n = Http.count(p)
  U32.or(128, op) <> List.append(&2, U32, Ws.mask_len(Ws.len_bytes.k(Nat.is_lt(n, 126n), Nat.is_lt(n, 65536n), n)),
    List.append(&2, U32, Ws.key(), List.reverse(&2, U32, Ws.unmask.go(p, Ws.key(), 0n, Nil{}))))

def Ws.client_text(s: String) -> List<&2, U32>:
  Ws.encode_masked(1, U.Utf8.encode(s))

# a message as one masked binary CBOR frame (a client's request)
def Ws.client_cbor(t: String) -> List<&2, U32>:
  Ws.encode_masked(2, Ws.cbor.bytes(t))

# the upgrade request for path on host:port
def Ws.request(+host: String, +port: U32, +path: String) -> String:
  "GET " ++ path ++ " HTTP/1.1\r\nHost: " ++ host ++ ":" ++ U32.show(port) ++ "\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: YmFja3BsYW5lLWxpbmsh\r\nSec-WebSocket-Version: 13\r\n\r\n"

def Ws.need.ext(ok: Bool, +hd: Nat, +len: Nat, +bs: List<&2, U32>) -> Maybe<&2, Nat>:
  match ok:
    case True{}:
      Some{Nat.add(Nat.add(2n, hd), Ws.be(List.take(&2, U32, List.drop(&2, U32, bs, 2n), hd), 0n))}
    case False{}:
      None{}

def Ws.need.k(short: Bool, mid: Bool, +len7: U32, +bs: List<&2, U32>) -> Maybe<&2, Nat>:
  match short mid:
    case True{} _:
      Some{Nat.add(2n, U32.to_nat(len7))}
    case False{} True{}:
      Ws.need.ext(Nat.is_ge(List.length(&2, U32, bs), 4n), 2n, 0n, bs)
    case False{} False{}:
      Ws.need.ext(Nat.is_ge(List.length(&2, U32, bs), 10n), 8n, 0n, bs)

# how many bytes an unmasked (server) frame takes, once its header is in
def Ws.need(+bs: List<&2, U32>) -> Maybe<&2, Nat>:
  match bs:
    case _ <> +b1 <> _:
      Ws.need.k(U32.is_lt(U32.and(b1, 127), 126), U32.is_eq(U32.and(b1, 127), 126), U32.and(b1, 127), List.take(&2, U32, bs, 10n))
    case _:
      None{}
