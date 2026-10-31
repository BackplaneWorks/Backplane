import Base
import ./host.bend as H
import ./pty.bend as PT
import ../core/utf8.bend as U
import ../core/vt.bend as VT
import ../core/emb.bend as E

# Terminals: one shell per thread in a pty, started in the thread's
# directory. One actor owns them all; what a shell prints goes out through
# `out` (the server broadcasts it); input and resizes come in as messages.
#
# Terminals in the chat (core/emb.bend) are keyed "x:thread:id" and run a
# command an agent gave. Their output passes through the actor, which keeps
# each one's screen (`ETerm`) to answer the agent (TRead, TNote), a client's
# snapshot (TSnap) and the emulator's own queries (cursor reports), and
# writes the last screen to <home>/terms when the program exits.

type TMsg is Data:
  TOpen{thread: String, cwd: String, cols: U32, rows: U32}
  TIn{thread: String, data: List<&2, U32>}
  TSize{thread: String, cols: U32, rows: U32}
  TGone{thread: String, pid: U32, code: U32}
  # again: a restart (the running one is hung up first)
  TRun{key: String, title: String, cwd: String, cmd: String, cols: U32, rows: U32, again: Bool}
  TFed{key: String, pid: U32, data: List<&2, U32>}
  TSnap{client: U32, key: String}
  # the reply: the state ("running", "exited N", "gone") on the first line,
  # then the terminal as the agent reads it (E.Emb.report)
  TRead{key: String, reply: Chan(String)}
  TNote{thread: String, reply: Chan(String)}
  TKill{key: String}

# what a shell printed (the server forwards it to the hub); TTo is a
# snapshot for one client (a blank terminal of that size, then the bytes;
# client Et.all() is every client)
type TOut is Data:
  TOut{thread: String, pid: U32, data: List<&2, U32>}
  TTo{client: U32, key: String, cols: U32, rows: U32, data: List<&2, U32>}

# Every reader passes through the owning actor. It rejects output from a
# retired process before bytes can reach a replacement PTY of the same owner.
def Term.emit(+q: Chan(TMsg), +out: Chan(TOut), +thread: String, +pid: U32, bs: List<&2, U32>) -> IO(Unit):
  do IO<Unit>:
    ok : Result<&1, &1, TMsg, Unit> <- Chan.send(TMsg, q, TFed{thread, pid, bs})
    return Unit{}

type Term is Data:
  Term{thread: String, input: Chan(List<&2, U32>), pid: U32, cols: U32, rows: U32}

def Term.fuel() -> Nat:
  U32.to_nat(4294967295)

def Term.end(s: Socket, +thread: String, +pid: U32, +q: Chan(TMsg), +out: Chan(TOut)) -> IO(Unit):
  do IO<Unit>:
    Socket.close(s)
    +code : U32 <- H.Proc.wait(pid)
    Term.emit(q, out, thread, pid, U.Utf8.encode("\r\n[process exited " ++ U32.show(code) ++ "]\r\n"))
    ok : Result<&1, &1, TMsg, Unit> <- Chan.send(TMsg, q, TGone{thread, pid, code})
    return Unit{}

# Reading a shell
# ---------------

def Term.got(r: Socket & Result<&1, &1, U32 & String, List<&2, U32>>, +thread: String, +pid: U32, +q: Chan(TMsg),
    +out: Chan(TOut), k: Socket -> IO(Unit)) -> IO(Unit):
  (s, res) = r
  match res:
    case Done{Nil{}}:
      Term.end(s, thread, pid, q, out)
    case Done{bs}:
      IO.bind(Unit, Unit, Term.emit(q, out, thread, pid, bs), u => k(s))
    case Fail{_}:
      Term.end(s, thread, pid, q, out)

def Term.reader(fuel: Nat, s: Socket, +thread: String, +pid: U32, +q: Chan(TMsg), +out: Chan(TOut)) -> IO(Unit):
  match fuel:
    case 0n:
      Term.end(s, thread, pid, q, out)
    case 1n+f:
      IO.bind(Socket & Result<&1, &1, U32 & String, List<&2, U32>>, Unit, H.Sock.recv_bytes(s, 65536),
        r => Term.got(r, thread, pid, q, out, s2 => Term.reader(f, s2, thread, pid, q, out)))

# Writing to a shell (keys, pastes) and resizing it
# -------------------------------------------------

type TW is Data:
  TWrite{data: List<&2, U32>}
  TResize{cols: U32, rows: U32}

def Term.w.wrote(r: Socket & Result<&1, &1, U32 & String, Unit>, k: Socket -> IO(Unit)) -> IO(Unit):
  (s, _) = r
  k(s)

# bytes from the actor; a resize travels as [0xFF 0xFE cols rows] framing
# (never a real keystroke sequence: 0xFF 0xFE is not valid UTF-8)
def Term.w.data(bs: List<&2, U32>, s: Socket, k: Socket -> IO(Unit)) -> IO(Unit):
  match bs:
    case 255 <> 254 <> c <> r <> Nil{}:
      IO.bind(Socket, Unit, PT.Pty.resize(s, c, r), k)
    case _:
      IO.bind(Socket & Result<&1, &1, U32 & String, Unit>, Unit, H.Sock.send_bytes(s, bs), r => Term.w.wrote(r, k))

def Term.w.one(m: Maybe<&1, List<&2, U32>>, s: Socket, k: Socket -> IO(Unit)) -> IO(Unit):
  match m:
    case Some{bs}:
      Term.w.data(bs, s, k)
    case None{}:
      Socket.close(s)

def Term.writer(fuel: Nat, s: Socket, +input: Chan(List<&2, U32>)) -> IO(Unit):
  match fuel:
    case 0n:
      Socket.close(s)
    case 1n+f:
      IO.bind(Maybe<&1, List<&2, U32>>, Unit, Chan.recv(List<&2, U32>, input), m => Term.w.one(m, s, s2 => Term.writer(f, s2, input)))

# Starting
# --------

def Term.duped(r: Socket & Result<&1, &1, U32 & String, Socket>, +thread: String, +pid: U32, +cols: U32, +rows: U32, +q: Chan(TMsg),
    +out: Chan(TOut)) -> IO(Maybe<&2, Term>):
  (rd, res) = r
  match res:
    case Done{wr}:
      do IO<Maybe<&2, Term>>:
        +input : Chan(List<&2, U32>) <- Chan.new(List<&2, U32>, 256)
        IO.spawn(Unit, Term.writer(Term.fuel(), wr, input))
        IO.spawn(Unit, Term.reader(Term.fuel(), rd, thread, pid, q, out))
        return Some{Term{thread, input, pid, cols, rows}}
    case Fail{_}:
      do IO<Maybe<&2, Term>>:
        Socket.close(rd)
        return None{}

def Term.spawned.fail(e: U32 & String, +thread: String, +q: Chan(TMsg), +out: Chan(TOut)) -> IO(Maybe<&2, Term>):
  (_, msg) = e
  IO.bind(Unit, Maybe<&2, Term>, Term.emit(q, out, thread, 0, U.Utf8.encode("[could not start a shell: " ++ msg ++ "]\r\n")), u => IO.pure(Maybe<&2, Term>, None{}))

def Term.spawned.ok(started: U32 & Socket, +thread: String, +cols: U32, +rows: U32, +q: Chan(TMsg), +out: Chan(TOut)) -> IO(Maybe<&2, Term>):
  (+pid, s) = started
  IO.bind(Socket & Result<&1, &1, U32 & String, Socket>, Maybe<&2, Term>, H.Sock.dup(s), r => Term.duped(r, thread, pid, cols, rows, q, out))

def Term.spawned(r: Result<&1, &1, U32 & String, U32 & Socket>, +thread: String, +cols: U32, +rows: U32, +q: Chan(TMsg),
    +out: Chan(TOut)) -> IO(Maybe<&2, Term>):
  match r:
    case Done{started}:
      Term.spawned.ok(started, thread, cols, rows, q, out)
    case Fail{e}:
      Term.spawned.fail(e, thread, q, out)

# The actor
# ---------

def Term.has(ts: List<&2, Term>, +thread: String) -> Bool:
  match ts:
    case Nil{}:
      False{}
    case Term{t, _, _, _, _} <> rest:
      Bool.or(String.eq(t, thread), Term.has(rest, thread))

def Term.hit(ts: List<&2, Term>, +thread: String) -> Bool:
  match ts:
    case Term{t, _, _, _, _} <> _:
      String.eq(t, thread)
    case Nil{}:
      False{}

def Term.send.go(ts: List<&2, Term>, hit: Bool, +thread: String, bs: List<&2, U32>) -> IO(Unit):
  match ts hit:
    case Term{_, i, _, _, _} <> _ True{}:
      do IO<Unit>:
        ok : Result<&1, &1, List<&2, U32>, Unit> <- Chan.send(List<&2, U32>, i, bs)
        return Unit{}
    case _ <> +rest False{}:
      Term.send.go(rest, Term.hit(rest, thread), thread, bs)
    case Nil{} _:
      IO.pure(Unit, Unit{})

# bytes to a thread's shell (nothing when it has none)
def Term.send(+ts: List<&2, Term>, +thread: String, bs: List<&2, U32>) -> IO(Unit):
  Term.send.go(ts, Term.hit(ts, thread), thread, bs)

def Term.drop.k(gone: Bool, t: Term, rest: List<&2, Term>) -> List<&2, Term>:
  match gone:
    case True{}:
      rest
    case False{}:
      t <> rest

def Term.drop(ts: List<&2, Term>, +thread: String, +pid: U32) -> List<&2, Term>:
  match ts:
    case Nil{}:
      Nil{}
    case Term{+t, i, +p, c, r} <> rest:
      Term.drop.k(Bool.and(String.eq(t, thread), U32.is_eq(p, pid)), Term{t, i, p, c, r}, Term.drop(rest, thread, pid))

def Term.add(m: Maybe<&2, Term>, ts: List<&2, Term>) -> List<&2, Term>:
  match m:
    case Some{t}:
      t <> ts
    case None{}:
      ts

# a new shell unless the thread has one (then it only takes the new size)
def Term.opened(r: Result<&1, &1, U32 & String, U32 & Socket>, +thread: String, +cols: U32, +rows: U32, +shell: String, +cwd: String,
    +q: Chan(TMsg), +out: Chan(TOut)) -> IO(Maybe<&2, Term>):
  match r:
    case Done{started}: Term.spawned.ok(started, thread, cols, rows, q, out)
    case Fail{(code, msg)}:
      Term.spawned.fail((code, shell ++ " in " ++ cwd ++ ": " ++ msg ++ ". Check SHELL and that the thread folder exists."), thread, q, out)

def Term.open(has: Bool, +ts: List<&2, Term>, +thread: String, +cwd: String, +cols: U32, +rows: U32, +shell: String, +q: Chan(TMsg),
    +out: Chan(TOut)) -> IO(List<&2, Term>):
  match has:
    case True{}:
      IO.bind(Unit, List<&2, Term>, Term.send(ts, thread, [255, 254, cols, rows]), u => IO.pure(List<&2, Term>, ts))
    case False{}:
      IO.bind(Result<&1, &1, U32 & String, U32 & Socket>, List<&2, Term>, PT.Pty.spawn(shell, ["-l"], cwd, cols, rows),
        r => IO.bind(Maybe<&2, Term>, List<&2, Term>, Term.opened(r, thread, cols, rows, shell, cwd, q, out), m => IO.pure(List<&2, Term>, Term.add(m, ts))))

# Terminals in the chat
# ---------------------

# a chat terminal's screen and whether the agent has seen it as it is now
type ETerm is Data:
  ETerm{key: String, title: String, vt: VT.Vt, seen: Bool, state: String, pid: U32}

def Et.key(+e: ETerm) -> String:
  match e:
    case ETerm{k, _, _, _, _, _}:
      k

def Et.hit(es: List<&2, ETerm>, +key: String) -> Bool:
  match es:
    case ETerm{k, _, _, _, _, _} <> _:
      String.eq(k, key)
    case Nil{}:
      False{}

def Et.find.go(es: List<&2, ETerm>, hit: Bool, +key: String) -> Maybe<&2, ETerm>:
  match es hit:
    case e <> _ True{}:
      Some{e}
    case _ <> +rest False{}:
      Et.find.go(rest, Et.hit(rest, key), key)
    case Nil{} _:
      None{}

def Et.find(+es: List<&2, ETerm>, +key: String) -> Maybe<&2, ETerm>:
  Et.find.go(es, Et.hit(es, key), key)

def Et.drop.k(gone: Bool, e: ETerm, rest: List<&2, ETerm>) -> List<&2, ETerm>:
  match gone:
    case True{}:
      rest
    case False{}:
      e <> rest

def Et.drop(es: List<&2, ETerm>, +key: String) -> List<&2, ETerm>:
  match es:
    case Nil{}:
      Nil{}
    case ETerm{+k, t, v, s, st, p} <> rest:
      Et.drop.k(String.eq(k, key), ETerm{k, t, v, s, st, p}, Et.drop(rest, key))

def Et.put(es: List<&2, ETerm>, +e: ETerm) -> List<&2, ETerm>:
  e <> Et.drop(es, Et.key(e))

# the id the agent knows a terminal by: the key's last part
def Et.id.go(ps: List<&2, String>, +acc: String) -> String:
  match ps:
    case Nil{}:
      acc
    case +p <> t:
      Et.id.go(t, p)

def Et.id(+key: String) -> String:
  Et.id.go(String.split(key, ':'), "")

def Et.report(+e: ETerm) -> String:
  match e:
    case ETerm{+k, +ti, +v, _, +st, _}:
      E.Emb.report(Et.id(k), ti, st, E.Emb.text(v))

# The terminals' files: "COLSxROWS<tab>state" on the first line, then a
# snapshot of the screen
def Et.path(+home: String, +key: String) -> String:
  home ++ "/terms/" ++ key

def Et.read.done(r: File & Result<&1, &1, U32 & String, List<&2, U32>>) -> IO(List<&2, U32>):
  (f, res) = r
  do IO<List<&2, U32>>:
    File.close(f)
    return Result.default(&1, &1, U32 & String, List<&2, U32>, res, Nil{})

def Et.read.sized(r: File & Result<&1, &1, U32 & String, U32>) -> IO(List<&2, U32>):
  (f, res) = r
  IO.bind(File & Result<&1, &1, U32 & String, List<&2, U32>>, List<&2, U32>,
    File.read_bytes(f, U32.max(Result.default(&1, &1, U32 & String, U32, res, 0), 1)), Et.read.done)

def Et.read.opened(r: Result<&1, &1, U32 & String, File>) -> IO(List<&2, U32>):
  match r:
    case Done{f}:
      IO.bind(File & Result<&1, &1, U32 & String, U32>, List<&2, U32>, File.size(f), Et.read.sized)
    case Fail{_}:
      IO.pure(List<&2, U32>, Nil{})

def Et.read(path: String) -> IO(List<&2, U32>):
  IO.bind(Result<&1, &1, U32 & String, File>, List<&2, U32>, File.open(path, "r"), Et.read.opened)

def Et.write.done(r: File & Result<&1, &1, U32 & String, Unit>) -> IO(Unit):
  (f, _) = r
  File.close(f)

def Et.write.opened(r: Result<&1, &1, U32 & String, File>, bs: List<&2, U32>) -> IO(Unit):
  match r:
    case Done{f}:
      IO.bind(File & Result<&1, &1, U32 & String, Unit>, Unit, File.write_bytes(f, bs), Et.write.done)
    case Fail{_}:
      IO.pure(Unit, Unit{})

def Et.write(+path: String, bs: List<&2, U32>) -> IO(Unit):
  IO.bind(Result<&1, &1, U32 & String, File>, Unit, File.open(path, "w"), r => Et.write.opened(r, bs))

def Et.save(+home: String, +e: ETerm) -> IO(Unit):
  match e:
    case ETerm{+k, _, +v, _, +st, _}:
      Et.write(Et.path(home, k), List.append(&2, U32, U.Utf8.encode(U32.show(VT.Vt.cols(v)) ++ "x" ++ U32.show(VT.Vt.rows(v)) ++ "\t" ++ st ++ "\n"), E.Emb.snap(v)))

def Et.head(bs: List<&2, U32>) -> List<&2, U32>:
  match bs:
    case Nil{}:
      Nil{}
    case 10 <> _:
      Nil{}
    case b <> t:
      b <> Et.head(t)

def Et.body(bs: List<&2, U32>) -> List<&2, U32>:
  match bs:
    case Nil{}:
      Nil{}
    case 10 <> t:
      t
    case _ <> t:
      Et.body(t)

def Et.field(+s: String, +sep: Char, +i: Nat) -> String:
  Maybe.default(&2, String, List.get(&2, String, String.split(s, sep), i), "")

def Et.from.k(+key: String, +head: String, body: List<&2, U32>) -> ETerm:
  +c = E.Emb.cols(E.Emb.num(Et.field(Et.field(head, '\t', 0n), 'x', 0n)))
  +r = E.Emb.rows(E.Emb.num(Et.field(Et.field(head, '\t', 0n), 'x', 1n)))
  ETerm{key, "", VT.Vt.feed(VT.Vt.new(c, r), body), True{}, Et.field(head, '\t', 1n), 0}

def Et.from.go(empty: Bool, +key: String, +bs: List<&2, U32>, es: List<&2, ETerm>) -> List<&2, ETerm>:
  match empty:
    case True{}:
      es
    case False{}:
      Et.put(es, Et.from.k(key, U.Utf8.decode(Et.head(bs)), Et.body(bs)))

def Et.loaded(+bs: List<&2, U32>, +key: String, es: List<&2, ETerm>) -> List<&2, ETerm>:
  Et.from.go(List.is_empty(&2, U32, bs), key, bs, es)

# the terminal from its file when the actor does not hold it (the hub
# started again since it ran)
def Et.load.k(has: Bool, es: List<&2, ETerm>, +home: String, +key: String) -> IO(List<&2, ETerm>):
  match has:
    case True{}:
      IO.pure(List<&2, ETerm>, es)
    case False{}:
      IO.bind(List<&2, U32>, List<&2, ETerm>, Et.read(Et.path(home, key)), bs => IO.pure(List<&2, ETerm>, Et.loaded(bs, key, es)))

def Et.hit.any(es: List<&2, ETerm>, +key: String) -> Bool:
  match es:
    case Nil{}:
      False{}
    case ETerm{k, _, _, _, _, _} <> rest:
      Bool.or(String.eq(k, key), Et.hit.any(rest, key))

def Et.load(+es: List<&2, ETerm>, +home: String, +key: String) -> IO(List<&2, ETerm>):
  Et.load.k(Et.hit.any(es, key), es, home, key)

# Each of a thread's terminals the agent has not seen as they are now
def Et.mine(+e: ETerm, +pre: String) -> Bool:
  match e:
    case ETerm{k, _, _, s, _, _}:
      Bool.and(String.starts_with(k, pre), Bool.not(s))

def Et.reports.k(new: Bool, +e: ETerm, rest: List<&2, String>) -> List<&2, String>:
  match new:
    case True{}:
      Et.report(e) <> rest
    case False{}:
      rest

def Et.reports(es: List<&2, ETerm>, +pre: String) -> List<&2, String>:
  match es:
    case Nil{}:
      Nil{}
    case +e <> t:
      Et.reports.k(Et.mine(e, pre), e, Et.reports(t, pre))

def Et.saw(+e: ETerm, +pre: String) -> ETerm:
  match e:
    case ETerm{k, t, v, s, st, p}:
      ETerm{k, t, v, Bool.or(s, String.starts_with(k, pre)), st, p}

def Et.mark(es: List<&2, ETerm>, +pre: String) -> List<&2, ETerm>:
  match es:
    case Nil{}:
      Nil{}
    case +e <> t:
      Et.saw(e, pre) <> Et.mark(t, pre)

def Term.pid.go(ts: List<&2, Term>, hit: Bool, +thread: String) -> U32:
  match ts hit:
    case Term{_, _, p, _, _} <> _ True{}:
      p
    case _ <> +rest False{}:
      Term.pid.go(rest, Term.hit(rest, thread), thread)
    case Nil{} _:
      0

def Term.pid(+ts: List<&2, Term>, +thread: String) -> U32:
  Term.pid.go(ts, Term.hit(ts, thread), thread)

# The actor's state: the shells, and the chat terminals' screens
type TS is Data:
  TS{ts: List<&2, Term>, es: List<&2, ETerm>}

def TS.new() -> TS:
  TS{Nil{}, Nil{}}

def Term.reply(+ts: List<&2, Term>, +key: String, bs: List<&2, U32>) -> IO(Unit):
  match bs:
    case Nil{}:
      IO.pure(Unit, Unit{})
    case _ <> _:
      Term.send(ts, key, bs)

# everyone's copy of the screen starts over (a restart)
def Et.all() -> U32:
  4294967295

def Et.ran(m: Maybe<&2, Term>, +ts: List<&2, Term>, +es: List<&2, ETerm>, +key: String, +ti: String, +c: U32, +r: U32) -> TS:
  match m:
    case Some{Term{+th, i, +pid, tc, tr}}:
      TS{Term{th, i, pid, tc, tr} <> ts, Et.put(es, ETerm{key, ti, VT.Vt.new(c, r), False{}, "running", pid})}
    case None{}:
      TS{ts, Et.put(es, ETerm{key, ti, VT.Vt.new(c, r), False{}, "failed to start", 0})}

def Et.spawn(+ts: List<&2, Term>, +es: List<&2, ETerm>, +key: String, +ti: String, +cwd: String, +cmd: String, +c: U32, +r: U32, +shell: String,
    +q: Chan(TMsg), +out: Chan(TOut), k: TS -> IO(Unit)) -> IO(Unit):
  do IO<Unit>:
    ok : Result<&1, &1, TOut, Unit> <- Chan.send(TOut, out, TTo{Et.all(), key, c, r, Nil{}})
    res : Result<&1, &1, U32 & String, U32 & Socket> <- PT.Pty.spawn(shell, ["-lc", E.Emb.script(cmd, shell)], cwd, c, r)
    mt : Maybe<&2, Term> <- Term.spawned(res, key, c, r, q, out)
    k(Et.ran(mt, ts, es, key, ti, c, r))

def Et.gone.e(same: Bool, +e: ETerm, es: List<&2, ETerm>, +home: String) -> IO(List<&2, ETerm>):
  match same:
    case True{}:
      do IO<List<&2, ETerm>>:
        Et.save(home, e)
        return Et.put(es, e)
    case False{}:
      IO.pure(List<&2, ETerm>, es)

# what a stopped program prints as it goes, and its end, are no news to
# the agent (it stopped because the agent was told, or asked)
def Et.stays(+seen: Bool, +st: String) -> Bool:
  Bool.and(seen, String.eq(st, "stopped"))

# a chat terminal printed: its screen takes the bytes (and answers what
# the program asked of the terminal), then every client gets them; bytes
# from a process a restart replaced are dropped
def Et.fed.k(same: Bool, +ts: List<&2, Term>, +es: List<&2, ETerm>, +key: String, +ti: String, v: VT.Vt, +seen: Bool, +st: String, +pid: U32, +bs: List<&2, U32>, +out: Chan(TOut), k: TS -> IO(Unit)) -> IO(Unit):
  match same:
    case True{}:
      +v2 = VT.Vt.feed(v, bs)
      do IO<Unit>:
        Term.reply(ts, key, VT.Vt.replies(v2))
        ok : Result<&1, &1, TOut, Unit> <- Chan.send(TOut, out, TOut{key, pid, bs})
        k(TS{ts, Et.put(es, ETerm{key, ti, VT.Vt.drain(v2), Et.stays(seen, st), E.Emb.after(st, bs), pid})})
    case False{}:
      k(TS{ts, es})

def Et.fed(m: Maybe<&2, ETerm>, +ts: List<&2, Term>, +es: List<&2, ETerm>, +key: String, +pid: U32, +bs: List<&2, U32>, +out: Chan(TOut), k: TS -> IO(Unit)) -> IO(Unit):
  match m:
    case Some{ETerm{_, +ti, v, +seen, +st, +p}}:
      Et.fed.k(U32.is_eq(p, pid), ts, es, key, ti, v, seen, st, p, bs, out, k)
    case None{}:
      k(TS{ts, es})

def Et.gone.k(m: Maybe<&2, ETerm>, +es: List<&2, ETerm>, +key: String, +pid: U32, +code: U32, +home: String) -> IO(List<&2, ETerm>):
  match m:
    case Some{ETerm{_, ti, v, +seen, +st, +p}}:
      Et.gone.e(U32.is_eq(p, pid), {ETerm{key, ti, v, Et.stays(seen, st), "exited " ++ U32.show(code), p} : ETerm}, es, home)
    case None{}:
      IO.pure(List<&2, ETerm>, es)

# a chat terminal's program exited: its state says so and its last screen
# is kept on disk
def Et.gone(emb: Bool, +es: List<&2, ETerm>, +key: String, +pid: U32, +code: U32, +home: String) -> IO(List<&2, ETerm>):
  match emb:
    case True{}:
      Et.gone.k(Et.find(es, key), es, key, pid, code, home)
    case False{}:
      IO.pure(List<&2, ETerm>, es)

def Et.stopping(m: Maybe<&2, ETerm>, es: List<&2, ETerm>) -> List<&2, ETerm>:
  match m:
    case Some{ETerm{+k, t, v, s, +st, p}}:
      Et.put(es, ETerm{k, t, v, s, Bool.pick(String, String.eq(st, "running"), "stopped", st), p})
    case None{}:
      es

def Et.snap.k(m: Maybe<&2, ETerm>, +client: U32, +key: String, +out: Chan(TOut)) -> IO(Unit):
  match m:
    case Some{ETerm{_, _, +v, _, _, _}}:
      do IO<Unit>:
        ok : Result<&1, &1, TOut, Unit> <- Chan.send(TOut, out, TTo{client, key, VT.Vt.cols(v), VT.Vt.rows(v), E.Emb.snap(v)})
        return Unit{}
    case None{}:
      do IO<Unit>:
        ok : Result<&1, &1, TOut, Unit> <- Chan.send(TOut, out, TTo{client, key, E.Emb.cols(0), E.Emb.rows(0), Nil{}})
        return Unit{}

def Et.state(+e: ETerm) -> String:
  match e:
    case ETerm{_, _, _, _, st, _}:
      st

def Et.read.k(m: Maybe<&2, ETerm>, +reply: Chan(String)) -> IO(Unit):
  match m:
    case Some{+e}:
      do IO<Unit>:
        ok : Result<&1, &1, String, Unit> <- Chan.send(String, reply, Et.state(e) ++ "\n" ++ Et.report(e))
        return Unit{}
    case None{}:
      do IO<Unit>:
        ok : Result<&1, &1, String, Unit> <- Chan.send(String, reply, "gone\n")
        return Unit{}

def Et.seen(m: Maybe<&2, ETerm>, es: List<&2, ETerm>) -> List<&2, ETerm>:
  match m:
    case Some{ETerm{+k, t, v, _, st, p}}:
      Et.put(es, ETerm{k, t, v, True{}, st, p})
    case None{}:
      es

# a hang-up for the terminal's whole process group (its leader's pid
# negated, as kill(2) takes it): the command the shell runs goes too
def Term.kill(+pid: U32) -> IO(Unit):
  Bool.pick(IO(Unit), U32.is_eq(pid, 0), IO.pure(Unit, Unit{}), H.Proc.kill(((4294967295 - pid : U32) + 1 : U32), 1))

# an agent's command on a pty of its own; again (a restart) hangs up the
# one running first
def Et.run(again: Bool, has: Bool, +ts: List<&2, Term>, +es: List<&2, ETerm>, +key: String, +ti: String, +cwd: String, +cmd: String, +c: U32, +r: U32, +shell: String,
    +q: Chan(TMsg), +out: Chan(TOut), k: TS -> IO(Unit)) -> IO(Unit):
  match again has:
    case False{} True{}:
      k(TS{ts, es})
    case True{} True{}:
      IO.bind(Unit, Unit, Term.kill(Term.pid(ts, key)), u => Et.spawn(Term.drop(ts, key, Term.pid(ts, key)), es, key, ti, cwd, cmd, c, r, shell, q, out, k))
    case _ False{}:
      Et.spawn(ts, es, key, ti, cwd, cmd, c, r, shell, q, out, k)

def Et.snapped(+es: List<&2, ETerm>, ts: List<&2, Term>, +client: U32, +key: String, +out: Chan(TOut), k: TS -> IO(Unit)) -> IO(Unit):
  IO.bind(Unit, Unit, Et.snap.k(Et.find(es, key), client, key, out), u => k(TS{ts, es}))

def Et.readed(+es: List<&2, ETerm>, ts: List<&2, Term>, +key: String, +reply: Chan(String), k: TS -> IO(Unit)) -> IO(Unit):
  IO.bind(Unit, Unit, Et.read.k(Et.find(es, key), reply), u => k(TS{ts, Et.seen(Et.find(es, key), es)}))

# Ordinary shells keep a screen too, for a renderer that reconnects to
# the same live PTY. Its clients still own protocol replies as before.
def Et.shell.keep(same: Bool, +key: String, +c: U32, +r: U32, +pid: U32, v: VT.Vt) -> ETerm:
  match same:
    case True{}: ETerm{key, "", VT.Vt.resize(v, c, r), True{}, "running", pid}
    case False{}: ETerm{key, "", VT.Vt.new(c, r), True{}, "running", pid}

def Et.shell.open(m: Maybe<&2, ETerm>, +es: List<&2, ETerm>, +key: String, +c: U32, +r: U32, +pid: U32) -> List<&2, ETerm>:
  match m:
    case Some{ETerm{_, _, v, _, _, p}}: Et.put(es, Et.shell.keep(U32.is_eq(p, pid), key, c, r, pid, v))
    case None{}: Et.put(es, ETerm{key, "", VT.Vt.new(c, r), True{}, "running", pid})

def Et.shell.resize(m: Maybe<&2, ETerm>, +es: List<&2, ETerm>, +key: String, c: U32, r: U32) -> List<&2, ETerm>:
  match m:
    case Some{ETerm{k, ti, v, seen, st, pid}}: Et.put(es, ETerm{k, ti, VT.Vt.resize(v, c, r), seen, st, pid})
    case None{}: es

def Et.shell.fed(m: Maybe<&2, ETerm>, +es: List<&2, ETerm>, +key: String, +pid: U32, bs: List<&2, U32>) -> List<&2, ETerm>:
  match m:
    case Some{ETerm{k, ti, v, seen, st, +p}}:
      Bool.pick(List<&2, ETerm>, U32.is_eq(p, pid), Et.put(es, ETerm{k, ti, VT.Vt.drain(VT.Vt.feed(v, bs)), seen, st, p}), es)
    case None{}: es

def Term.forward(same: Bool, +ts: List<&2, Term>, +es: List<&2, ETerm>, +key: String, +pid: U32, bs: List<&2, U32>, +out: Chan(TOut), k: TS -> IO(Unit)) -> IO(Unit):
  match same:
    case False{}: k(TS{ts, es})
    case True{}:
      do IO<Unit>:
        ok : Result<&1, &1, TOut, Unit> <- Chan.send(TOut, out, TOut{key, pid, bs})
        k(TS{ts, es})

def Term.fed(emb: Bool, +ts: List<&2, Term>, +es: List<&2, ETerm>, +key: String, +pid: U32, +bs: List<&2, U32>, +out: Chan(TOut), k: TS -> IO(Unit)) -> IO(Unit):
  match emb:
    case True{}: Et.fed(Et.find(es, key), ts, es, key, pid, bs, out, k)
    case False{}: Term.forward(U32.is_eq(Term.pid(ts, key), pid), ts, Et.shell.fed(Et.find(es, key), es, key, pid, bs), key, pid, bs, out, k)

def Term.step(m: Maybe<&1, TMsg>, st: TS, +shell: String, +home: String, +q: Chan(TMsg), +out: Chan(TOut), k: TS -> IO(Unit)) -> IO(Unit):
  match m st:
    case Some{TOpen{+th, cwd, +c, +r}} TS{+ts, +es}:
      IO.bind(List<&2, Term>, Unit, Term.open(Term.has(ts, th), ts, th, cwd, c, r, shell, q, out), +t2 => k(TS{t2, Et.shell.open(Et.find(es, th), es, th, c, r, Term.pid(t2, th))}))
    case Some{TIn{+th, bs}} TS{+ts, es}:
      IO.bind(Unit, Unit, Term.send(ts, th, bs), u => k(TS{ts, es}))
    case Some{TSize{+th, +c, +r}} TS{+ts, +es}:
      IO.bind(Unit, Unit, Term.send(ts, th, [255, 254, c, r]), u => k(TS{ts, Et.shell.resize(Et.find(es, th), es, th, c, r)}))
    case Some{TGone{+th, +pid, +code}} TS{ts, +es}:
      IO.bind(List<&2, ETerm>, Unit, Et.gone(E.Emb.is_key(th), es, th, pid, code, home), e2 => k(TS{Term.drop(ts, th, pid), e2}))
    case Some{TRun{+key, +ti, +cwd, +cmd, +c, +r, again}} TS{+ts, +es}:
      Et.run(again, Bool.or(Term.has(ts, key), Et.hit.any(es, key)), ts, es, key, ti, cwd, cmd, E.Emb.cols(c), E.Emb.rows(r), shell, q, out, k)
    case Some{TFed{+key, +pid, +bs}} TS{+ts, +es}:
      Term.fed(E.Emb.is_key(key), ts, es, key, pid, bs, out, k)
    case Some{TSnap{+client, +key}} TS{ts, +es}:
      IO.bind(List<&2, ETerm>, Unit, Et.load(es, home, key), e2 => Et.snapped(e2, ts, client, key, out, k))
    case Some{TRead{+key, +reply}} TS{ts, +es}:
      IO.bind(List<&2, ETerm>, Unit, Et.load(es, home, key), e2 => Et.readed(e2, ts, key, reply, k))
    case Some{TNote{+th, +reply}} TS{ts, +es}:
      do IO<Unit>:
        ok : Result<&1, &1, String, Unit> <- Chan.send(String, reply, E.Emb.note(Et.reports(es, "x:" ++ th ++ ":")))
        k(TS{ts, Et.mark(es, "x:" ++ th ++ ":")})
    case Some{TKill{+key}} TS{+ts, +es}:
      IO.bind(Unit, Unit, Term.kill(Term.pid(ts, key)), u => k(TS{ts, Et.stopping(Et.find(es, key), es)}))
    case None{} _:
      IO.pure(Unit, Unit{})

def Term.loop(fuel: Nat, +q: Chan(TMsg), st: TS, +shell: String, +home: String, +out: Chan(TOut)) -> IO(Unit):
  match fuel:
    case 0n:
      IO.pure(Unit, Unit{})
    case 1n+f:
      IO.bind(Maybe<&1, TMsg>, Unit, Chan.recv(TMsg, q), m => Term.step(m, st, shell, home, q, out, s2 => Term.loop(f, q, s2, shell, home, out)))

# the actor, its terminals' folder made first
def Term.start(+q: Chan(TMsg), +shell: String, +home: String, +out: Chan(TOut)) -> IO(Unit):
  do IO<Unit>:
    r : Result<&1, &1, U32 & String, Unit> <- H.FS.mkdirs(home ++ "/terms")
    Term.loop(U32.to_nat(4294967295), q, TS.new(), shell, home, out)
