import Base
import ../server/server.bend as S
import ../server/host.bend as H
import ../core/http.bend as W
import ../core/utf8.bend as U
import ../core/json.bend as J
import ../core/cbor.bend as CB
import ../core/desk.bend as DK
import ../core/winnote.bend as NT

# Resolution and TCP connect share a bounded deadline. The byte poll
# keeps any binary frame that arrives with the HTTP upgrade head intact.
def Link.connect(host: String, port: U32, ms: U32) -> IO(Result<&1, &1, U32 & String, Socket>):
  import "./effects/link.c"
  import "./effects/link.js"

def Link.poll(s: Socket, ms: U32) -> IO(Socket & Result<&1, &1, U32 & String, Maybe<&2, List<&2, U32>>>):
  import "./effects/link.c"
  import "./effects/link.js"

# A link to another machine's hub, in the shape of a hub: the window sends
# it S.Msg (Join once, then Rpc for each request, Leave to hang up) and
# reads S.Wire from the channel it joined with, just as with its own hub.
# Underneath is a WebSocket client (binary CBOR frames; the window's own
# messages stay JSON text in this process). When the connection
# drops, `out` closes; the window decides whether to try again.

def Link.fuel() -> Nat:
  U32.to_nat(4294967295)

def Link.after_scheme(+url: String) -> String:
  Bool.pick(String, String.starts_with(url, "http://"), String.drop(url, 7n), url)

def Link.hostport(+url: String) -> List<&2, String>:
  String.split(Maybe.default(&2, String, List.head(&2, String, String.split(Link.after_scheme(url), '/')), ""), ':')

def Link.host(+url: String) -> String:
  Maybe.default(&2, String, List.head(&2, String, Link.hostport(url)), "")

def Link.port(+url: String) -> U32:
  U32.from_nat(Maybe.default(&2, Nat, Nat.read(Maybe.default(&2, String, List.get(&2, String, Link.hostport(url), 1n), "")), 3787n))

def Link.say(+out: Chan(S.Wire), +text: String) -> IO(Unit):
  do IO<Unit>:
    ok : Result<&1, &1, S.Wire, Unit> <- Chan.send(S.Wire, out, S.WText{text})
    return Unit{}

# the link is down: say why (a note the window shows), then hang up
def Link.down(+out: Chan(S.Wire), +why: String) -> IO(Unit):
  do IO<Unit>:
    Link.say(out, J.Json.show(J.Json.obj([J.KV{"t", J.Str{"update"}}, J.KV{"state", J.Str{why}}, J.KV{"error", J.Str{why}}])))
    Chan.close(S.Wire, out)

# Writing: requests from the window become masked binary CBOR frames
# ------------------------------------------------------------------

def Link.wrote(r: Socket & Result<&1, &1, U32 & String, Unit>, k: Socket -> IO(Unit)) -> IO(Unit):
  (s, _) = r
  k(s)

def Link.write.one(m: Maybe<&1, S.Msg>, s: Socket, k: Socket -> IO(Unit)) -> IO(Unit):
  match m:
    case Some{S.Rpc{_, text}}:
      IO.bind(Socket & Result<&1, &1, U32 & String, Unit>, Unit, H.Sock.send_bytes(s, W.Ws.client_cbor(text)), r => Link.wrote(r, k))
    case Some{S.Leave{_}}:
      IO.bind(Socket, Unit, H.Sock.shutdown(s), s2 => Socket.close(s2))
    case Some{_}:
      k(s)
    case None{}:
      Socket.close(s)

def Link.writer(fuel: Nat, s: Socket, +inbox: Chan(S.Msg)) -> IO(Unit):
  match fuel:
    case 0n:
      Socket.close(s)
    case 1n+f:
      IO.bind(Maybe<&1, S.Msg>, Unit, Chan.recv(S.Msg, inbox), m => Link.write.one(m, s, s2 => Link.writer(f, s2, inbox)))

# requests while there is no connection are dropped (the client's outbox
# sends them again once a log arrives); Leave ends the link
def Link.drain.one(m: Maybe<&1, S.Msg>, k: Unit -> IO(Unit)) -> IO(Unit):
  match m:
    case Some{S.Leave{_}}:
      IO.pure(Unit, Unit{})
    case Some{_}:
      k(Unit{})
    case None{}:
      IO.pure(Unit, Unit{})

def Link.drain(fuel: Nat, +inbox: Chan(S.Msg)) -> IO(Unit):
  match fuel:
    case 0n:
      IO.pure(Unit, Unit{})
    case 1n+f:
      IO.bind(Maybe<&1, S.Msg>, Unit, Chan.recv(S.Msg, inbox), m => Link.drain.one(m, u => Link.drain(f, inbox)))

# Reading: frames from the hub
# ----------------------------
# Bytes arrive in chunks: `front` holds the start of the stream (enough to
# read a frame's length), `back` the later chunks newest first, `have` how
# many bytes there are in all. A frame is cut out once all of it is in, so
# a large log costs one pass, not one per chunk.

def Link.flat(back: List<&2, List<&2, U32>>, acc: List<&2, U32>) -> List<&2, U32>:
  match back:
    case Nil{}:
      acc
    case c <> t:
      Link.flat(t, List.append(&2, U32, c, acc))

# the front grown by the oldest chunk still in back (to read a header)
def Link.grow(+front: List<&2, U32>, back: List<&2, List<&2, U32>>) -> List<&2, U32>:
  List.append(&2, U32, front, Link.flat(back, Nil{}))

# a binary frame's CBOR as the JSON text the window reads
def Link.json(m: Maybe<&2, J.Json>) -> String:
  match m:
    case Some{j}:
      J.Json.show(j)
    case None{}:
      ""

def Link.frame.op(bin: Bool, close: Bool, p: List<&2, U32>, rest: List<&2, U32>, s: Socket, +out: Chan(S.Wire), k: Socket -> List<&2, U32> -> IO(Unit)) -> IO(Unit):
  match bin close:
    case True{} _:
      IO.bind(Unit, Unit, Link.say(out, Link.json(CB.Cbor.decode(p))), u => k(s, rest))
    case False{} True{}:
      do IO<Unit>:
        Socket.close(s)
        Link.down(out, "")
    case False{} False{}:
      k(s, rest)

def Link.frame(g: Maybe<&2, W.Got>, s: Socket, +out: Chan(S.Wire), k: Socket -> List<&2, U32> -> IO(Unit)) -> IO(Unit):
  match g:
    case Some{W.Got{W.Frame{_, +op, p}, rest}}:
      Link.frame.op(U32.is_eq(op, 2), U32.is_eq(op, 8), p, rest, s, out, k)
    case None{}:
      k(s, Nil{})

# the link is dead when nothing at all (a heartbeat included) arrives for
# Alive.limit() ms: the wait is bounded, and a timeout hangs up like a
# closed socket (the window decides whether to try again)
def Link.got(r: Socket & Result<&1, &1, U32 & String, Maybe<&2, List<&2, U32>>>, k: Socket -> List<&2, U32> -> IO(Unit), +out: Chan(S.Wire)) -> IO(Unit):
  (s, res) = r
  match res:
    case Done{Some{Nil{}}}:
      do IO<Unit>:
        Socket.close(s)
        Link.down(out, "")
    case Done{Some{bs}}:
      k(s, bs)
    case Done{None{}}:
      do IO<Unit>:
        Socket.close(s)
        Link.down(out, NT.Notice.make("silent", ""))
    case Fail{_}:
      do IO<Unit>:
        Socket.close(s)
        Link.down(out, "")

def Link.more(s: Socket, +out: Chan(S.Wire), k: Socket -> List<&2, U32> -> IO(Unit)) -> IO(Unit):
  IO.bind(Socket & Result<&1, &1, U32 & String, Maybe<&2, List<&2, U32>>>, Unit, Link.poll(s, U32.from_nat(DK.Alive.limit())), r => Link.got(r, k, out))

# with the whole frame in: cut it out and go on with what follows
def Link.cut(ready: Bool, s: Socket, +front: List<&2, U32>, back: List<&2, List<&2, U32>>, +have: Nat, +need: Nat, +out: Chan(S.Wire),
    k: Socket -> List<&2, U32> -> List<&2, List<&2, U32>> -> Nat -> IO(Unit)) -> IO(Unit):
  match ready:
    case True{}:
      Link.frame(W.Ws.decode(Link.grow(front, back)), s, out, s2 => rest => k(s2, rest, Nil{}, Nat.sub(have, need)))
    case False{}:
      Link.more(s, out, s2 => +bs => k(s2, front, bs <> back, Nat.add(have, List.length(&2, U32, bs))))

# the header is not all in the front yet: take the chunks in, or wait for more
def Link.need(more: Bool, s: Socket, +front: List<&2, U32>, back: List<&2, List<&2, U32>>, +have: Nat, +out: Chan(S.Wire),
    k: Socket -> List<&2, U32> -> List<&2, List<&2, U32>> -> Nat -> IO(Unit)) -> IO(Unit):
  match more:
    case True{}:
      k(s, Link.grow(front, back), Nil{}, have)
    case False{}:
      Link.more(s, out, s2 => +bs => k(s2, front, bs <> back, Nat.add(have, List.length(&2, U32, bs))))

def Link.step.k(m: Maybe<&2, Nat>, s: Socket, +front: List<&2, U32>, back: List<&2, List<&2, U32>>, +have: Nat, +out: Chan(S.Wire),
    k: Socket -> List<&2, U32> -> List<&2, List<&2, U32>> -> Nat -> IO(Unit)) -> IO(Unit):
  match m:
    case Some{+need}:
      Link.cut(Nat.is_ge(have, need), s, front, back, have, need, out, k)
    case None{}:
      Link.need(Nat.is_gt(have, List.length(&2, U32, front)), s, front, back, have, out, k)

def Link.reader(fuel: Nat, s: Socket, +front: List<&2, U32>, back: List<&2, List<&2, U32>>, +have: Nat, +out: Chan(S.Wire)) -> IO(Unit):
  match fuel:
    case 0n:
      Socket.close(s)
    case 1n+f:
      Link.step.k(W.Ws.need(front), s, front, back, have, out, s2 => fr => bk => hv => Link.reader(f, s2, fr, bk, hv, out))

def Link.duped(r: Socket & Result<&1, &1, U32 & String, Socket>, +rest: List<&2, U32>, +inbox: Chan(S.Msg), +out: Chan(S.Wire)) -> IO(Unit):
  (rd, res) = r
  match res:
    case Done{wr}:
      do IO<Unit>:
        IO.spawn(Unit, Link.writer(Link.fuel(), wr, inbox))
        Link.reader(Link.fuel(), rd, rest, Nil{}, List.length(&2, U32, rest), out)
    case Fail{_}:
      do IO<Unit>:
        Socket.close(rd)
        IO.spawn(Unit, Link.drain(Link.fuel(), inbox))
        Link.down(out, "")

# upgraded: any note of an earlier failure goes; one handle reads frames,
# a second writes requests
def Link.open(ok: Bool, s: Socket, +rest: List<&2, U32>, +inbox: Chan(S.Msg), +out: Chan(S.Wire)) -> IO(Unit):
  match ok:
    case True{}:
      IO.bind(Unit, Unit, Link.say(out, "{\"t\":\"update\",\"state\":\"\"}"),
        u => IO.bind(Socket & Result<&1, &1, U32 & String, Socket>, Unit, H.Sock.dup(s), r => Link.duped(r, rest, inbox, out)))
    case False{}:
      do IO<Unit>:
        Socket.close(s)
        IO.spawn(Unit, Link.drain(Link.fuel(), inbox))
        Link.down(out, NT.Notice.make("denied", ""))

# Connecting
# ----------

def Link.split.k(m: Maybe<&2, W.Split>, s: Socket, +inbox: Chan(S.Msg), +out: Chan(S.Wire), k: Socket -> IO(Unit)) -> IO(Unit):
  match m:
    case Some{W.Split{head, rest}}:
      Link.open(String.contains(U.Utf8.decode(head), " 101 "), s, rest, inbox, out)
    case None{}:
      k(s)

def Link.unreachable(s: Socket, +inbox: Chan(S.Msg), +out: Chan(S.Wire)) -> IO(Unit):
  do IO<Unit>:
    Socket.close(s)
    IO.spawn(Unit, Link.drain(Link.fuel(), inbox))
    Link.down(out, NT.Notice.make("nohandshake", ""))

def Link.head.got(r: Socket & Result<&1, &1, U32 & String, Maybe<&2, List<&2, U32>>>, +inbox: Chan(S.Msg), +out: Chan(S.Wire), k: Socket -> List<&2, U32> -> IO(Unit)) -> IO(Unit):
  (s, res) = r
  match res:
    case Done{Some{Nil{}}}:
      Link.unreachable(s, inbox, out)
    case Done{Some{bs}}:
      k(s, bs)
    case _:
      Link.unreachable(s, inbox, out)

def Link.head(fuel: Nat, s: Socket, +buf: List<&2, U32>, +deadline: Nat, +inbox: Chan(S.Msg), +out: Chan(S.Wire)) -> IO(Unit):
  match fuel:
    case 0n:
      Link.unreachable(s, inbox, out)
    case 1n+f:
      Link.split.k(W.Http.split(buf), s, inbox, out,
        s2 => IO.bind(Nat, Unit, IO.now(), now => IO.bind(Socket & Result<&1, &1, U32 & String, Maybe<&2, List<&2, U32>>>, Unit,
          Link.poll(s2, U32.from_nat(Nat.sub(deadline, now))), r => Link.head.got(r, inbox, out, s3 => bs => Link.head(f, s3, List.append(&2, U32, buf, bs), deadline, inbox, out)))))

def Link.sent(r: Socket & Result<&1, &1, U32 & String, Unit>, +inbox: Chan(S.Msg), +out: Chan(S.Wire)) -> IO(Unit):
  (s, res) = r
  match res:
    case Done{_}:
      IO.bind(Nat, Unit, IO.now(), now => Link.head(64n, s, Nil{}, Nat.add(now, 5000n), inbox, out))
    case Fail{_}:
      Link.unreachable(s, inbox, out)

def Link.path(since: Nat, origin: String) -> String:
  "/ws?since=" ++ Nat.show(since) ++ "&origin=" ++ origin ++ "&enc=cbor"

def Link.connected(r: Result<&1, &1, U32 & String, Socket>, +url: String, since: Nat, origin: String, +inbox: Chan(S.Msg), +out: Chan(S.Wire)) -> IO(Unit):
  match r:
    case Done{s}:
      IO.bind(Socket & Result<&1, &1, U32 & String, Unit>, Unit, H.Sock.send_text(s, W.Ws.request(Link.host(url), Link.port(url), Link.path(since, origin))),
        r2 => Link.sent(r2, inbox, out))
    case Fail{_}:
      do IO<Unit>:
        IO.spawn(Unit, Link.drain(Link.fuel(), inbox))
        Link.down(out, NT.Notice.make("nohost", Link.host(url)))

def Link.joined(m: Maybe<&1, S.Msg>, +url: String, +inbox: Chan(S.Msg)) -> IO(Unit):
  match m:
    case Some{S.Join{_, out, since, origin, _, _}}:
      IO.bind(Result<&1, &1, U32 & String, Socket>, Unit, Link.connect(Link.host(url), Link.port(url), 5000), r => Link.connected(r, url, since, origin, inbox, out))
    case _:
      IO.pure(Unit, Unit{})

# a link to the hub at url ("http://host:port"); join it like a hub
def Link.start(+url: String) -> IO(Chan(S.Msg)):
  do IO<Chan(S.Msg)>:
    +inbox : Chan(S.Msg) <- Chan.new(S.Msg, 1024)
    IO.spawn(Unit, IO.bind(Maybe<&1, S.Msg>, Unit, Chan.recv(S.Msg, inbox), m => Link.joined(m, url, inbox)))
    return inbox
