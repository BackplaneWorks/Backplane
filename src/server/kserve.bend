import Base
import ./host.bend as H
import ./proc.bend as P
import ./browser.bend as BR
import ./kicad.bend as K
import ../core/json.bend as J
import ../core/utf8.bend as U
import ../core/kserve.bend as KS
import ../core/kihealth.bend as KH

# The hub's headless KiCad servers (core/kserve.bend is the policy). One
# actor owns a record per project and one backplane-kipc helper
# (tools/kipc), spoken to in JSON lines like the browser's. The helper
# starts each `kicad-cli api-server` (inside a systemd scope capped at
# 1.5 GB where there is one), reports its stdout lines and its exit, and
# carries the IPC calls: the actor never kills anything. Servers stop by
# themselves: --parent-pid when the hub goes, --idle-exit when unused,
# kiapi.backplane.Shutdown when a third project needs the room.
#
# A request for a project's server (KAsk) is answered by the helper's
# reply line; when the server is not ready yet it waits in `wait` and goes
# out when the server answers Ping and GetVersion. A failure the tool
# can't use is {"ok":false,"error":<why and the fallback>,"fallback":true}.

# what the runtime is: the kicad-cli, its root, the capabilities its
# manifest names, and the manifest's version
type KEnv is Data:
  KEnv{cli: String, root: String, caps: List<&2, String>, version: String}

# a request waiting for its project's server
type Wt is Data:
  Wt{project: String, ty: String, msg: J.Json, env: KEnv, reply: Chan(String)}

type KMsg is Data:
  # a tool's command for a project's server (started if needed)
  KAsk{project: String, env: KEnv, ty: String, msg: J.Json, reply: Chan(String)}
  # a command for a socket, no policy (the actor's own tasks)
  KRaw{sock: String, env: KEnv, ty: String, msg: J.Json, reply: Chan(String)}
  KInfo{reply: Chan(String)}
  KLine{text: String}
  KDead{}
  # the server answered Ping and GetVersion as the build the manifest names
  KUp{project: String, version: String}
  KBad{project: String, why: String}
  # 30 s after a start: it never said it was listening
  KLate{project: String, born: Nat}
  # the backoff after a crash is over
  KGo{project: String}
  # the server that made room has stopped (or would not)
  KEvicted{victim: String, project: String, ok: Bool, why: String}
  # Retry: failed servers are forgotten
  KClear{}
  # the command line a project's server is started with
  KArgv{project: String, reply: Chan(String)}

# the helper's input (None until it runs), the requests waiting for a reply,
# the next id and the helper's path
type Hp is Data:
  Hp{input: Maybe<&2, Chan(String)>, pend: List<&2, BR.Pend>, next: Nat, exe: String}

type Ks is Data:
  Ks{hp: Hp, srvs: List<&2, KS.KSrv>, wait: List<&2, Wt>, shown: String, hub: U32, home: String, rt: String, env: KEnv, q: Chan(KMsg), ev: Chan(String)}

def KServe.forever() -> Nat:
  4000000000n

def KEnv.cli(e: KEnv) -> String:
  match e:
    case KEnv{x, _, _, _}: x

def KEnv.root(e: KEnv) -> String:
  match e:
    case KEnv{_, x, _, _}: x

def KEnv.caps(e: KEnv) -> List<&2, String>:
  match e:
    case KEnv{_, _, x, _}: x

def KEnv.version(e: KEnv) -> String:
  match e:
    case KEnv{_, _, _, x}: x

def KEnv.none() -> KEnv:
  KEnv{"", "", Nil{}, ""}

def Wt.project(w: Wt) -> String:
  match w:
    case Wt{x, _, _, _, _}: x

def Ks.hp(k: Ks) -> Hp:
  match k:
    case Ks{x, _, _, _, _, _, _, _, _, _}: x

def Ks.srvs(k: Ks) -> List<&2, KS.KSrv>:
  match k:
    case Ks{_, x, _, _, _, _, _, _, _, _}: x

def Ks.wait(k: Ks) -> List<&2, Wt>:
  match k:
    case Ks{_, _, x, _, _, _, _, _, _, _}: x

def Ks.shown(k: Ks) -> String:
  match k:
    case Ks{_, _, _, x, _, _, _, _, _, _}: x

def Ks.hub(k: Ks) -> U32:
  match k:
    case Ks{_, _, _, _, x, _, _, _, _, _}: x

def Ks.home(k: Ks) -> String:
  match k:
    case Ks{_, _, _, _, _, x, _, _, _, _}: x

def Ks.rt(k: Ks) -> String:
  match k:
    case Ks{_, _, _, _, _, _, x, _, _, _}: x

def Ks.env(k: Ks) -> KEnv:
  match k:
    case Ks{_, _, _, _, _, _, _, x, _, _}: x

def Ks.q(k: Ks) -> Chan(KMsg):
  match k:
    case Ks{_, _, _, _, _, _, _, _, x, _}: x

def Ks.ev(k: Ks) -> Chan(String):
  match k:
    case Ks{_, _, _, _, _, _, _, _, _, x}: x

def Ks.with_hp(k: Ks, +x: Hp) -> Ks:
  match k:
    case Ks{_, b, c, d, e, f, g, h, i, j}: Ks{x, b, c, d, e, f, g, h, i, j}

def Ks.with_srvs(k: Ks, +x: List<&2, KS.KSrv>) -> Ks:
  match k:
    case Ks{a, _, c, d, e, f, g, h, i, j}: Ks{a, x, c, d, e, f, g, h, i, j}

def Ks.with_wait(k: Ks, +x: List<&2, Wt>) -> Ks:
  match k:
    case Ks{a, b, _, d, e, f, g, h, i, j}: Ks{a, b, x, d, e, f, g, h, i, j}

def Ks.with_shown(k: Ks, +x: String) -> Ks:
  match k:
    case Ks{a, b, c, _, e, f, g, h, i, j}: Ks{a, b, c, x, e, f, g, h, i, j}

def Ks.with_env(k: Ks, +x: KEnv) -> Ks:
  match k:
    case Ks{a, b, c, d, e, f, g, _, i, j}: Ks{a, b, c, d, e, f, g, x, i, j}

# Strings
# -------

def KServe.failed(+text: String) -> String:
  J.Json.show(J.Json.obj([J.KV{"ok", J.Flag{False{}}}, J.KV{"error", J.Str{text}}, J.KV{"why", J.Str{text}}, J.KV{"fallback", J.Flag{True{}}}]))

def KServe.strs(xs: List<&2, String>) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      J.Str{x} <> KServe.strs(t)

def KServe.now() -> IO(Nat):
  do IO<Nat>:
    t : U32 <- H.Clock.epoch()
    return U32.to_nat(t)

def KServe.parse(+line: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(line), J.Null{})

# the helper beside the hub's binary, else on PATH
def KServe.exe() -> IO(String):
  do IO<String>:
    +d : String <- H.Sys.exe_dir()
    ok : Bool <- K.Io.is_file(d ++ "/backplane-kipc")
    return Bool.pick(String, ok, d ++ "/backplane-kipc", "backplane-kipc")

# Reading and writing the helper
# ------------------------------

def KServe.lines(ls: List<&2, String>, +q: Chan(KMsg)) -> IO(Unit):
  match ls:
    case Nil{}:
      IO.pure(Unit, Unit{})
    case l <> t:
      do IO<Unit>:
        ok : Result<&1, &1, KMsg, Unit> <- Chan.send(KMsg, q, KLine{l})
        KServe.lines(t, q)

def KServe.end(s: Socket, +pid: U32, +q: Chan(KMsg)) -> IO(Unit):
  do IO<Unit>:
    Socket.close(s)
    code : U32 <- H.Proc.wait(pid)
    ok : Result<&1, &1, KMsg, Unit> <- Chan.send(KMsg, q, KDead{})
    return Unit{}

def KServe.fed(r: U.Lines & List<&2, String>, s: Socket, +q: Chan(KMsg), k: Socket -> U.Lines -> IO(Unit)) -> IO(Unit):
  (carry, ls) = r
  do IO<Unit>:
    KServe.lines(ls, q)
    k(s, carry)

def KServe.got(r: Socket & Result<&1, &1, U32 & String, List<&2, U32>>, carry: U.Lines, +pid: U32, +q: Chan(KMsg), k: Socket -> U.Lines -> IO(Unit)) -> IO(Unit):
  (s, res) = r
  match res:
    case Done{Nil{}}:
      KServe.end(s, pid, q)
    case Done{bs}:
      KServe.fed(U.Lines.feed(carry, bs), s, q, k)
    case Fail{_}:
      KServe.end(s, pid, q)

def KServe.reader(fuel: Nat, s: Socket, carry: U.Lines, +pid: U32, +q: Chan(KMsg)) -> IO(Unit):
  match fuel:
    case 0n:
      KServe.end(s, pid, q)
    case 1n+f:
      IO.bind(Socket & Result<&1, &1, U32 & String, List<&2, U32>>, Unit, H.Sock.recv_bytes(s, 65536),
        r => KServe.got(r, carry, pid, q, s2 => c2 => KServe.reader(f, s2, c2, pid, q)))

def KServe.wrote(r: Socket & Result<&1, &1, U32 & String, Unit>, k: Socket -> IO(Unit)) -> IO(Unit):
  (s, _) = r
  k(s)

def KServe.write(m: Maybe<&1, String>, s: Socket, k: Socket -> IO(Unit)) -> IO(Unit):
  match m:
    case Some{line}:
      IO.bind(Socket & Result<&1, &1, U32 & String, Unit>, Unit, H.Sock.send_text(s, line ++ "\n"), r => KServe.wrote(r, k))
    case None{}:
      Socket.close(s)

def KServe.writer(fuel: Nat, s: Socket, +input: Chan(String)) -> IO(Unit):
  match fuel:
    case 0n:
      Socket.close(s)
    case 1n+f:
      IO.bind(Maybe<&1, String>, Unit, Chan.recv(String, input), m => KServe.write(m, s, s2 => KServe.writer(f, s2, input)))

def KServe.duped(r: Socket & Result<&1, &1, U32 & String, Socket>, +pid: U32, +q: Chan(KMsg)) -> IO(Maybe<&2, Chan(String)>):
  (rd, res) = r
  match res:
    case Done{wr}:
      do IO<Maybe<&2, Chan(String)>>:
        +input : Chan(String) <- Chan.new(String, 256)
        IO.spawn(Unit, KServe.writer(KServe.forever(), wr, input))
        IO.spawn(Unit, KServe.reader(KServe.forever(), rd, U.Lines.new(), pid, q))
        return Some{input}
    case Fail{_}:
      do IO<Maybe<&2, Chan(String)>>:
        Socket.close(rd)
        return None{}

def KServe.spawned.ok(started: U32 & Socket, +q: Chan(KMsg)) -> IO(Maybe<&2, Chan(String)>):
  (+pid, s) = started
  IO.bind(Socket & Result<&1, &1, U32 & String, Socket>, Maybe<&2, Chan(String)>, H.Sock.dup(s), r => KServe.duped(r, pid, q))

def KServe.spawned(r: Result<&1, &1, U32 & String, U32 & Socket>, +q: Chan(KMsg)) -> IO(Maybe<&2, Chan(String)>):
  match r:
    case Done{started}:
      KServe.spawned.ok(started, q)
    case Fail{_}:
      IO.pure(Maybe<&2, Chan(String)>, None{})

def KServe.start(+exe: String, +q: Chan(KMsg)) -> IO(Maybe<&2, Chan(String)>):
  IO.bind(Result<&1, &1, U32 & String, U32 & Socket>, Maybe<&2, Chan(String)>, H.Proc.spawn(exe, Nil{}, "/", 0), r => KServe.spawned(r, q))

# the running helper's input, starting it if needed
def KServe.ensure(m: Maybe<&2, Chan(String)>, +exe: String, +q: Chan(KMsg)) -> IO(Maybe<&2, Chan(String)>):
  match m:
    case Some{c}:
      IO.pure(Maybe<&2, Chan(String)>, Some{c})
    case None{}:
      KServe.start(exe, q)

# a request line (the id first, then the fields), registered for its reply
def KServe.sent(m: Maybe<&2, Chan(String)>, +fields: List<&2, J.KV>, +id: Nat, +reply: Chan(String), hp: Hp) -> IO(Hp):
  match m hp:
    case Some{+c} Hp{_, ps, n, exe}:
      do IO<Hp>:
        ok : Result<&1, &1, String, Unit> <- Chan.send(String, c, J.Json.show(J.Json.obj(J.KV{"id", J.Num{Nat.show(id)}} <> fields)))
        return Hp{Some{c}, BR.Pend{Nat.show(id), reply} <> ps, n, exe}
    case None{} Hp{_, ps, n, exe}:
      do IO<Hp>:
        BR.Browser.say(reply, KServe.failed("the KiCad IPC helper could not start (is backplane-kipc installed next to Backplane?)"))
        return Hp{None{}, ps, n, exe}

def KServe.call(hp: Hp, +q: Chan(KMsg), +fields: List<&2, J.KV>, +reply: Chan(String)) -> IO(Hp):
  match hp:
    case Hp{m, ps, +n, +exe}:
      IO.bind(Maybe<&2, Chan(String)>, Hp, KServe.ensure(m, exe, q),
        m2 => KServe.sent(m2, fields, n, reply, Hp{None{}, ps, 1n+n, exe}))

def KServe.fail.pend(ps: List<&2, BR.Pend>) -> IO(Unit):
  match ps:
    case Nil{}:
      IO.pure(Unit, Unit{})
    case BR.Pend{_, reply} <> t:
      do IO<Unit>:
        BR.Browser.say(reply, KServe.failed("the KiCad IPC helper exited"))
        KServe.fail.pend(t)

# the helper went away: what waits for it fails, and the next request starts it again
def KServe.hp.dead(hp: Hp) -> Hp:
  match hp:
    case Hp{_, _, n, exe}: Hp{None{}, Nil{}, n, exe}

def KServe.hp.pend(hp: Hp) -> List<&2, BR.Pend>:
  match hp:
    case Hp{_, ps, _, _}: ps

def KServe.hp.with_pend(hp: Hp, ps: List<&2, BR.Pend>) -> Hp:
  match hp:
    case Hp{m, _, n, exe}: Hp{m, ps, n, exe}

def KServe.later(+q: Chan(KMsg), ms: U32, m: KMsg) -> IO(Unit):
  do IO<Unit>:
    IO.sleep(ms)
    ok : Result<&1, &1, KMsg, Unit> <- Chan.send(KMsg, q, m)
    return Unit{}

# The records
# -----------

def KServe.has(xs: List<&2, KS.KSrv>, +project: String) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case s <> t:
      Bool.or(String.eq(KS.KS.project(s), project), KServe.has(t, project))

def KServe.rep.k(hit: Bool, s: KS.KSrv, h: KS.KSrv, rest: List<&2, KS.KSrv>) -> List<&2, KS.KSrv>:
  match hit:
    case True{}:
      s <> rest
    case False{}:
      h <> rest

def KServe.rep(xs: List<&2, KS.KSrv>, +s: KS.KSrv) -> List<&2, KS.KSrv>:
  match xs:
    case Nil{}:
      Nil{}
    case +h <> t:
      KServe.rep.k(String.eq(KS.KS.project(h), KS.KS.project(s)), s, h, KServe.rep(t, s))

def KServe.put.k(has: Bool, +xs: List<&2, KS.KSrv>, +s: KS.KSrv) -> List<&2, KS.KSrv>:
  match has:
    case True{}:
      KServe.rep(xs, s)
    case False{}:
      List.append(&2, KS.KSrv, xs, [s])

# the record, replaced or added
def KServe.put(+xs: List<&2, KS.KSrv>, +s: KS.KSrv) -> List<&2, KS.KSrv>:
  KServe.put.k(KServe.has(xs, KS.KS.project(s)), xs, s)

def KServe.get.k(hit: Bool, s: KS.KSrv, rest: KS.KSrv) -> KS.KSrv:
  match hit:
    case True{}:
      s
    case False{}:
      rest

# a project's record (a fresh stopped one when there is none)
def KServe.get(xs: List<&2, KS.KSrv>, +project: String) -> KS.KSrv:
  match xs:
    case Nil{}:
      KS.KS.set(KS.KS.new(project, 0n), "stopped", "")
    case +s <> t:
      KServe.get.k(String.eq(KS.KS.project(s), project), s, KServe.get(t, project))

def KServe.stopped.k(active: Bool, s: KS.KSrv) -> KS.KSrv:
  match active:
    case True{}:
      KS.KS.set(s, "stopped", "")
    case False{}:
      s

def KServe.stopped(xs: List<&2, KS.KSrv>) -> List<&2, KS.KSrv>:
  match xs:
    case Nil{}:
      Nil{}
    case +s <> t:
      KServe.stopped.k(KS.KS.active(KS.KS.state(s)), s) <> KServe.stopped(t)

def KServe.cleared.k(failed: Bool, s: KS.KSrv) -> KS.KSrv:
  match failed:
    case True{}:
      KS.KS.cleared(s)
    case False{}:
      s

def KServe.cleared(xs: List<&2, KS.KSrv>) -> List<&2, KS.KSrv>:
  match xs:
    case Nil{}:
      Nil{}
    case +s <> t:
      KServe.cleared.k(String.eq(KS.KS.state(s), "failed"), s) <> KServe.cleared(t)

def KServe.mine.k(hit: Bool, w: Wt, rest: List<&2, Wt>) -> List<&2, Wt>:
  match hit:
    case True{}:
      w <> rest
    case False{}:
      rest

# the waiting requests of a project, and the others
def KServe.mine(ws: List<&2, Wt>, +project: String) -> List<&2, Wt>:
  match ws:
    case Nil{}:
      Nil{}
    case +w <> t:
      KServe.mine.k(String.eq(Wt.project(w), project), w, KServe.mine(t, project))

def KServe.rest.k(hit: Bool, w: Wt, rest: List<&2, Wt>) -> List<&2, Wt>:
  match hit:
    case True{}:
      rest
    case False{}:
      w <> rest

def KServe.rest(ws: List<&2, Wt>, +project: String) -> List<&2, Wt>:
  match ws:
    case Nil{}:
      Nil{}
    case +w <> t:
      KServe.rest.k(String.eq(Wt.project(w), project), w, KServe.rest(t, project))

# The state, as the person is told it
# ------------------------------------

def KServe.emit.k(same: Bool, +k: Ks) -> IO(Ks):
  match same:
    case True{}:
      IO.pure(Ks, k)
    case False{}:
      do IO<Ks>:
        +text : String = KS.KS.lines(Ks.srvs(k))
        IO.spawn(Unit, BR.Browser.say(Ks.ev(k), text))
        return Ks.with_shown(k, text)

# the servers' lines go out when they changed
def KServe.emit(+k: Ks) -> IO(Ks):
  KServe.emit.k(String.eq(KS.KS.lines(Ks.srvs(k)), Ks.shown(k)), k)

# Waiting requests
# ----------------

def KServe.call.fields(+sock: String, env: KEnv, +ty: String, msg: J.Json) -> List<&2, J.KV>:
  match env:
    case KEnv{_, +root, caps, _}:
      [J.KV{"sock", J.Str{sock}}, J.KV{"type", J.Str{ty}}, J.KV{"message", msg}, J.KV{"desc", J.Str{KS.KS.desc(root, caps)}},
       J.KV{"lib", J.Str{root ++ "/lib"}}, J.KV{"timeout", J.Num{"30000"}}]

# a request goes to the project's ready server
def KServe.forward(+k: Ks, +project: String, +env: KEnv, +ty: String, msg: J.Json, +reply: Chan(String)) -> IO(Ks):
  do IO<Ks>:
    now : Nat <- KServe.now()
    +srv : KS.KSrv = KS.KS.touch(KServe.get(Ks.srvs(k), project), now)
    hp : Hp <- KServe.call(Ks.hp(k), Ks.q(k), KServe.call.fields(KS.KS.sock(srv), env, ty, msg), reply)
    return Ks.with_srvs(Ks.with_hp(k, hp), KServe.put(Ks.srvs(k), srv))

def KServe.fail.all(ws: List<&2, Wt>, +text: String) -> IO(Unit):
  match ws:
    case Nil{}:
      IO.pure(Unit, Unit{})
    case Wt{_, _, _, env, reply} <> t:
      do IO<Unit>:
        BR.Browser.say(reply, KServe.failed(KS.KS.unavailable(text, KEnv.cli(env))))
        KServe.fail.all(t, text)

# a project's waiting requests are answered with why not
def KServe.fail.project(+k: Ks, +project: String, +text: String) -> IO(Ks):
  do IO<Ks>:
    KServe.fail.all(KServe.mine(Ks.wait(k), project), text)
    return Ks.with_wait(k, KServe.rest(Ks.wait(k), project))

def KServe.flush(ws: List<&2, Wt>, +k: Ks) -> IO(Ks):
  match ws:
    case Nil{}:
      IO.pure(Ks, k)
    case Wt{project, ty, msg, env, reply} <> t:
      IO.bind(Ks, Ks, KServe.forward(k, project, env, ty, msg, reply), k2 => KServe.flush(t, k2))

# Starting and stopping
# ---------------------

def KServe.mkdir(+dir: String) -> IO(Unit):
  IO.bind(P.Ran, Unit, P.Proc.run("sh", ["-c", "umask 077; mkdir -p \"$1\" && chmod 700 \"$1\"", "sh", dir], "/"), r => IO.pure(Unit, Unit{}))

def KServe.spawn.fields(+project: String, +argv: List<&2, String>) -> List<&2, J.KV>:
  [J.KV{"op", J.Str{"spawn"}}, J.KV{"key", J.Str{project}}, J.KV{"argv", J.Json.arr(KServe.strs(argv))}, J.KV{"memory", J.Str{"1536M"}}]

# the project's server is started (its record says "starting" until it
# is listening and has answered), and 30 s are given to say so
def KServe.launch(+k: Ks, +project: String) -> IO(Ks):
  do IO<Ks>:
    +now : Nat <- KServe.now()
    +env : KEnv = Ks.env(k)
    +dir : String = KS.KS.dir(Ks.rt(k), Ks.home(k), project)
    KServe.mkdir(dir)
    +sink : Chan(String) <- Chan.new(String, 1)
    hp : Hp <- KServe.call(Ks.hp(k), Ks.q(k), KServe.spawn.fields(project, KS.KS.argv(KEnv.cli(env), project, dir, Ks.hub(k), KEnv.caps(env))), sink)
    IO.spawn(Unit, KServe.later(Ks.q(k), 30000, KLate{project, now}))
    KServe.emit(Ks.with_srvs(Ks.with_hp(k, hp), KServe.put(Ks.srvs(k), KS.KS.starting(KServe.get(Ks.srvs(k), project), now))))

def KServe.close.k(empty: Bool, +k: Ks, +sock: String) -> IO(Ks):
  match empty:
    case True{}:
      IO.pure(Ks, k)
    case False{}:
      do IO<Ks>:
        +sink : Chan(String) <- Chan.new(String, 1)
        hp : Hp <- KServe.call(Ks.hp(k), Ks.q(k), [J.KV{"op", J.Str{"close"}}, J.KV{"sock", J.Str{sock}}], sink)
        return Ks.with_hp(k, hp)

# the helper lets go of a socket whose server is gone
def KServe.close(+k: Ks, +sock: String) -> IO(Ks):
  KServe.close.k(String.is_empty(sock), k, sock)

# a command for a socket, answered with the helper's reply line
def KServe.raw(+q: Chan(KMsg), +sock: String, env: KEnv, +ty: String, msg: J.Json) -> IO(String):
  do IO<String>:
    +reply : Chan(String) <- Chan.new(String, 1)
    ok : Result<&1, &1, KMsg, Unit> <- Chan.send(KMsg, q, KRaw{sock, env, ty, msg, reply})
    m : Maybe<&1, String> <- Chan.recv(String, reply)
    return Maybe.default(&1, String, m, KServe.failed("no reply"))

def KServe.hand.why(+err: String, +full: String, +want: String) -> String:
  Bool.pick(String, String.is_empty(err), "this KiCad is " ++ full ++ ", not the " ++ want ++ " Backplane installed", "it did not answer GetVersion: " ++ err)

def KServe.hand.msg(ok: Bool, +project: String, +full: String, +want: String, +err: String) -> KMsg:
  match ok:
    case True{}:
      KUp{project, full}
    case False{}:
      KBad{project, KServe.hand.why(err, full, want)}

# a server that is listening answers Ping and GetVersion, and is the build
# the manifest names, before any request goes to it
def KServe.hand.version(line: String, +q: Chan(KMsg), +project: String, +want: String) -> IO(Unit):
  do IO<Unit>:
    +o : J.Json = KServe.parse(line)
    +full : String = J.Json.get_str(J.Json.get_obj(J.Json.get_obj(o, "message"), "version"), "full_version", "")
    ok : Result<&1, &1, KMsg, Unit> <- Chan.send(KMsg, q, KServe.hand.msg(Bool.and(J.Json.get_bool(o, "ok", False{}), KS.KS.version_ok(full, want)), project, full, want, J.Json.get_str(o, "error", "")))
    return Unit{}

def KServe.hand.ping(ok: Bool, +q: Chan(KMsg), +project: String, +env: KEnv, +sock: String, +err: String) -> IO(Unit):
  match ok:
    case True{}:
      IO.bind(String, Unit, KServe.raw(q, sock, env, "kiapi.common.commands.GetVersion", J.Json.obj(Nil{})), line => KServe.hand.version(line, q, project, KEnv.version(env)))
    case False{}:
      do IO<Unit>:
        ok2 : Result<&1, &1, KMsg, Unit> <- Chan.send(KMsg, q, KBad{project, "it did not answer Ping: " ++ err})
        return Unit{}

def KServe.hand(+q: Chan(KMsg), +project: String, +env: KEnv, +sock: String) -> IO(Unit):
  do IO<Unit>:
    +line : String <- KServe.raw(q, sock, env, "kiapi.common.commands.Ping", J.Json.obj(Nil{}))
    KServe.hand.ping(J.Json.get_bool(KServe.parse(line), "ok", False{}), q, project, env, sock, J.Json.get_str(KServe.parse(line), "error", ""))

# What the actor does
# -------------------

def KServe.ask.fail(k: Ks, w: Wt, +text: String) -> IO(Ks):
  match w:
    case Wt{_, _, _, env, reply}:
      do IO<Ks>:
        BR.Browser.say(reply, KServe.failed(KS.KS.unavailable(text, KEnv.cli(env))))
        return k

def KServe.ask.ready(+k: Ks, w: Wt) -> IO(Ks):
  match w:
    case Wt{project, ty, msg, env, reply}:
      KServe.forward(k, project, env, ty, msg, reply)

# the request waits and the project's server is started
def KServe.start.w(+k: Ks, +w: Wt) -> IO(Ks):
  KServe.launch(Ks.with_wait(k, w <> Ks.wait(k)), Wt.project(w))

def KServe.evicting(+q: Chan(KMsg), +victim: String, +project: String, env: KEnv, +sock: String) -> IO(Unit):
  do IO<Unit>:
    line : String <- KServe.raw(q, sock, env, "kiapi.backplane.Shutdown", J.Json.obj(Nil{}))
    +o : J.Json = KServe.parse(line)
    ok : Result<&1, &1, KMsg, Unit> <- Chan.send(KMsg, q, KEvicted{victim, project, J.Json.get_bool(o, "ok", False{}), J.Json.get_str(o, "error", "")})
    return Unit{}

# a third project: the least recently used server is asked to stop (it
# does so itself); the new one starts when it has
def KServe.evict(+victim: String, +k: Ks, +w: Wt) -> IO(Ks):
  do IO<Ks>:
    now : Nat <- KServe.now()
    +project : String = Wt.project(w)
    IO.spawn(Unit, KServe.evicting(Ks.q(k), victim, project, Ks.env(k), KS.KS.sock(KServe.get(Ks.srvs(k), victim))))
    KServe.emit(Ks.with_wait(Ks.with_srvs(k, KServe.put(Ks.srvs(k), KS.KS.new(project, now))), w <> Ks.wait(k)))

def KServe.ask.k(+plan: String, +victim: String, +k: Ks, +w: Wt) -> IO(Ks):
  match plan:
    case "ready":
      KServe.ask.ready(k, w)
    case "wait":
      IO.pure(Ks, Ks.with_wait(k, w <> Ks.wait(k)))
    case "failed":
      KServe.ask.fail(k, w, "the headless KiCad for " ++ KS.KS.name(Wt.project(w)) ++ " stopped: " ++ KS.KS.why.of(KServe.get(Ks.srvs(k), Wt.project(w))))
    case "busy":
      KServe.ask.fail(k, w, KS.KS.busy())
    case "start":
      KServe.start.w(k, w)
    case _:
      KServe.evict(victim, k, w)

def KServe.ask.plan(+plan: String, +k: Ks, +w: Wt) -> IO(Ks):
  KServe.ask.k(plan, String.drop(plan, 6n), k, w)

def KServe.ask(+k: Ks, +w: Wt) -> IO(Ks):
  KServe.ask.plan(KS.KS.plan(Ks.srvs(k), Wt.project(w), KEnv.caps(Ks.env(k))), k, w)

def KServe.refused.k(none: Bool, +k: Ks, +other: String, +victim: String, +project: String, +why: String) -> IO(Ks):
  match none:
    case True{}:
      do IO<Ks>:
        k2 : Ks <- KServe.fail.project(Ks.with_srvs(k, KServe.put(Ks.srvs(k), KS.KS.set(KServe.get(Ks.srvs(k), project), "stopped", ""))), project, KS.KS.busy() ++ " (" ++ KS.KS.name(victim) ++ " would not stop: " ++ why ++ ")")
        KServe.emit(k2)
    case False{}:
      do IO<Ks>:
        IO.spawn(Unit, KServe.evicting(Ks.q(k), other, project, Ks.env(k), KS.KS.sock(KServe.get(Ks.srvs(k), other))))
        return k

# Shutdown was refused (a document has unsaved changes): that server is
# held, never forced; the least recently used other one is asked instead,
# and only when none is left is the request answered "busy"
def KServe.refused(+k: Ks, +victim: String, +project: String, +why: String) -> IO(Ks):
  KServe.refused.k(String.is_empty(KS.KS.evict(Ks.srvs(k))), k, KS.KS.evict(Ks.srvs(k)), victim, project, why)

def KServe.evicted(ok: Bool, +k: Ks, +victim: String, +project: String, +why: String) -> IO(Ks):
  match ok:
    case True{}:
      KServe.launch(Ks.with_srvs(k, KServe.put(Ks.srvs(k), KS.KS.set(KServe.get(Ks.srvs(k), victim), "stopped", ""))), project)
    case False{}:
      KServe.refused(Ks.with_srvs(k, KServe.put(Ks.srvs(k), KS.KS.hold(KServe.get(Ks.srvs(k), victim)))), victim, project, why)

def KServe.listening.k(first: Bool, +k: Ks, +project: String, +path: String) -> IO(Ks):
  match first:
    case True{}:
      do IO<Ks>:
        IO.spawn(Unit, KServe.hand(Ks.q(k), project, Ks.env(k), path))
        return Ks.with_srvs(k, KServe.put(Ks.srvs(k), KS.KS.listening(KServe.get(Ks.srvs(k), project), path)))
    case False{}:
      IO.pure(Ks, k)

# a server said where it listens: it is asked Ping and GetVersion
def KServe.listening(+k: Ks, +project: String, +path: String) -> IO(Ks):
  KServe.listening.k(Bool.and(String.eq(KS.KS.state(KServe.get(Ks.srvs(k), project)), "starting"), String.is_empty(KS.KS.sock(KServe.get(Ks.srvs(k), project)))), k, project, path)

def KServe.out.k(m: Maybe<&2, String>, +k: Ks, +project: String) -> IO(Ks):
  match m:
    case Some{path}:
      KServe.listening(k, project, path)
    case None{}:
      IO.pure(Ks, k)

def KServe.go.in(m: Maybe<&2, Nat>, +q: Chan(KMsg), +project: String) -> IO(Unit):
  match m:
    case Some{s}:
      KServe.later(q, U32.from_nat(Nat.mul(s, 1000n)), KGo{project})
    case None{}:
      IO.pure(Unit, Unit{})

def KServe.after.state(+st: String, +k: Ks, +srv: KS.KSrv, +project: String, +now: Nat) -> IO(Ks):
  match st:
    case "starting":
      do IO<Ks>:
        IO.spawn(Unit, KServe.go.in(KS.KS.retry(KS.KS.fails.of(srv), now), Ks.q(k), project))
        return k
    case "failed":
      KServe.fail.project(k, project, "the headless KiCad for " ++ KS.KS.name(project) ++ " stopped: " ++ KS.KS.why.of(srv))
    case _:
      KServe.fail.project(k, project, "the headless KiCad for " ++ KS.KS.name(project) ++ " exited")

# the server's process ended: the record says what that means
def KServe.after(+srv: KS.KSrv, +k: Ks, +project: String, +now: Nat) -> IO(Ks):
  do IO<Ks>:
    k2 : Ks <- KServe.close(Ks.with_srvs(k, KServe.put(Ks.srvs(k), srv)), KS.KS.sock(srv))
    k3 : Ks <- KServe.after.state(KS.KS.state(srv), k2, srv, project, now)
    KServe.emit(k3)

def KServe.exited.k(skip: Bool, +k: Ks, +srv: KS.KSrv, +project: String, code: U32, +tail: String, +now: Nat) -> IO(Ks):
  match skip:
    case True{}:
      KServe.close(k, KS.KS.sock(srv))
    case False{}:
      KServe.after(KS.KS.exited(srv, code, now, tail), k, project, now)

def KServe.exited(+k: Ks, +project: String, code: U32, +tail: String) -> IO(Ks):
  do IO<Ks>:
    now : Nat <- KServe.now()
    +srv : KS.KSrv = KServe.get(Ks.srvs(k), project)
    KServe.exited.k(String.eq(KS.KS.state(srv), "stopped"), k, srv, project, code, tail, now)

def KServe.reply(+k: Ks, +o: J.Json, +text: String) -> IO(Ks):
  do IO<Ks>:
    ps : List<&2, BR.Pend> <- BR.Browser.deliver(KServe.hp.pend(Ks.hp(k)), J.Json.as_raw(J.Json.get(o, "id"), ""), text)
    return Ks.with_hp(k, KServe.hp.with_pend(Ks.hp(k), ps))

# a line from the helper: a server's stdout line or exit, else a reply (by id)
def KServe.line.ev(+event: String, +k: Ks, +o: J.Json, +text: String) -> IO(Ks):
  match event:
    case "line":
      KServe.out.k(KS.KS.ready(J.Json.get_str(o, "text", "")), k, J.Json.get_str(o, "key", ""))
    case "exit":
      KServe.exited(k, J.Json.get_str(o, "key", ""), U32.from_nat(J.Json.get_nat(o, "code", 1n)), J.Json.get_str(o, "tail", ""))
    case _:
      KServe.reply(k, o, text)

def KServe.line.o(+k: Ks, +text: String, +o: J.Json) -> IO(Ks):
  KServe.line.ev(J.Json.get_str(o, "event", ""), k, o, text)

def KServe.line(+k: Ks, +text: String) -> IO(Ks):
  KServe.line.o(k, text, KServe.parse(text))

def KServe.up.k(starting: Bool, +k: Ks, +project: String, +version: String) -> IO(Ks):
  match starting:
    case True{}:
      do IO<Ks>:
        now : Nat <- KServe.now()
        +srv : KS.KSrv = KS.KS.up(KServe.get(Ks.srvs(k), project), KS.KS.sock(KServe.get(Ks.srvs(k), project)), version, now)
        +k2 : Ks = Ks.with_srvs(k, KServe.put(Ks.srvs(k), srv))
        k3 : Ks <- KServe.flush(KServe.mine(Ks.wait(k2), project), Ks.with_wait(k2, KServe.rest(Ks.wait(k2), project)))
        KServe.emit(k3)
    case False{}:
      IO.pure(Ks, k)

# it answered Ping and GetVersion as the right build: ready, and what waited goes out
def KServe.up(+k: Ks, +project: String, +version: String) -> IO(Ks):
  KServe.up.k(String.eq(KS.KS.state(KServe.get(Ks.srvs(k), project)), "starting"), k, project, version)

def KServe.bad.k(starting: Bool, +k: Ks, +project: String, +why: String) -> IO(Ks):
  match starting:
    case True{}:
      do IO<Ks>:
        k2 : Ks <- KServe.fail.project(Ks.with_srvs(k, KServe.put(Ks.srvs(k), KS.KS.set(KServe.get(Ks.srvs(k), project), "failed", why))), project, "the headless KiCad for " ++ KS.KS.name(project) ++ " " ++ why)
        KServe.emit(k2)
    case False{}:
      IO.pure(Ks, k)

# it failed the handshake or never listened
def KServe.bad(+k: Ks, +project: String, +why: String) -> IO(Ks):
  KServe.bad.k(String.eq(KS.KS.state(KServe.get(Ks.srvs(k), project)), "starting"), k, project, why)

def KServe.late.srv(+srv: KS.KSrv, born: Nat) -> Bool:
  Bool.and(String.eq(KS.KS.state(srv), "starting"), Bool.and(Nat.is_eq(KS.KS.started.of(srv), born), String.is_empty(KS.KS.sock(srv))))

def KServe.late.k(late: Bool, +k: Ks, +project: String) -> IO(Ks):
  match late:
    case True{}:
      KServe.bad(k, project, "did not say it was listening within 30 s")
    case False{}:
      IO.pure(Ks, k)

def KServe.late(+k: Ks, +project: String, born: Nat) -> IO(Ks):
  KServe.late.k(KServe.late.srv(KServe.get(Ks.srvs(k), project), born), k, project)

def KServe.go.k(starting: Bool, +k: Ks, +project: String) -> IO(Ks):
  match starting:
    case True{}:
      KServe.launch(k, project)
    case False{}:
      IO.pure(Ks, k)

# the wait after a crash is over: start it again
def KServe.go(+k: Ks, +project: String) -> IO(Ks):
  KServe.go.k(String.eq(KS.KS.state(KServe.get(Ks.srvs(k), project)), "starting"), k, project)

def KServe.clear(+k: Ks) -> IO(Ks):
  KServe.emit(Ks.with_srvs(k, KServe.cleared(Ks.srvs(k))))

def KServe.dead(+k: Ks) -> IO(Ks):
  do IO<Ks>:
    KServe.fail.pend(KServe.hp.pend(Ks.hp(k)))
    KServe.fail.all(Ks.wait(k), "the KiCad IPC helper exited")
    KServe.emit(Ks.with_wait(Ks.with_srvs(Ks.with_hp(k, KServe.hp.dead(Ks.hp(k))), KServe.stopped(Ks.srvs(k))), Nil{}))

def KServe.info(+k: Ks, +reply: Chan(String)) -> IO(Ks):
  do IO<Ks>:
    BR.Browser.say(reply, KS.KS.lines(Ks.srvs(k)))
    return k

# the words of the command line a project's server is started with
def KServe.argv(+k: Ks, +project: String, +reply: Chan(String)) -> IO(Ks):
  do IO<Ks>:
    +dir : String = KS.KS.dir(Ks.rt(k), Ks.home(k), project)
    BR.Browser.say(reply, String.join(KS.KS.argv(KEnv.cli(Ks.env(k)), project, dir, Ks.hub(k), KEnv.caps(Ks.env(k))), " "))
    return k

def KServe.rawcall(+k: Ks, +sock: String, env: KEnv, +ty: String, msg: J.Json, +reply: Chan(String)) -> IO(Ks):
  do IO<Ks>:
    hp : Hp <- KServe.call(Ks.hp(k), Ks.q(k), KServe.call.fields(sock, env, ty, msg), reply)
    return Ks.with_hp(k, hp)

def KServe.step(m: Maybe<&1, KMsg>, +k: Ks, kont: Ks -> IO(Unit)) -> IO(Unit):
  match m:
    case Some{KAsk{project, +env, ty, msg, reply}}:
      IO.bind(Ks, Unit, KServe.ask(Ks.with_env(k, env), Wt{project, ty, msg, env, reply}), kont)
    case Some{KRaw{sock, env, ty, msg, reply}}:
      IO.bind(Ks, Unit, KServe.rawcall(k, sock, env, ty, msg, reply), kont)
    case Some{KInfo{reply}}:
      IO.bind(Ks, Unit, KServe.info(k, reply), kont)
    case Some{KLine{t}}:
      IO.bind(Ks, Unit, KServe.line(k, t), kont)
    case Some{KDead{}}:
      IO.bind(Ks, Unit, KServe.dead(k), kont)
    case Some{KUp{project, version}}:
      IO.bind(Ks, Unit, KServe.up(k, project, version), kont)
    case Some{KBad{project, why}}:
      IO.bind(Ks, Unit, KServe.bad(k, project, why), kont)
    case Some{KLate{project, born}}:
      IO.bind(Ks, Unit, KServe.late(k, project, born), kont)
    case Some{KGo{project}}:
      IO.bind(Ks, Unit, KServe.go(k, project), kont)
    case Some{KEvicted{victim, project, ok, why}}:
      IO.bind(Ks, Unit, KServe.evicted(ok, k, victim, project, why), kont)
    case Some{KClear{}}:
      IO.bind(Ks, Unit, KServe.clear(k), kont)
    case Some{KArgv{project, reply}}:
      IO.bind(Ks, Unit, KServe.argv(k, project, reply), kont)
    case None{}:
      IO.pure(Unit, Unit{})

def KServe.loop(fuel: Nat, +q: Chan(KMsg), k: Ks) -> IO(Unit):
  match fuel:
    case 0n:
      IO.pure(Unit, Unit{})
    case 1n+f:
      IO.bind(Maybe<&1, KMsg>, Unit, Chan.recv(KMsg, q), m => KServe.step(m, k, k2 => KServe.loop(f, q, k2)))

def KServe.pid(r: P.Ran) -> U32:
  match r:
    case P.Ran{0, +out}:
      U32.from_nat(KH.KH.num(String.trim(out), 0n))
    case P.Ran{_, _}:
      0

def KServe.boot(pid: U32, +rt: String, +exe: String, +home: String, +q: Chan(KMsg), +ev: Chan(String)) -> Ks:
  Ks{Hp{None{}, Nil{}, 1n, exe}, Nil{}, Nil{}, "", pid, home, rt, KEnv.none(), q, ev}

# the KiCad actor: requests on q; the servers' lines go to ev whenever
# they change. Native children watch the hub's native OS process ID.
def KServe.run(+q: Chan(KMsg), +ev: Chan(String), +exe: String, +home: String) -> IO(Unit):
  do IO<Unit>:
    pid : U32 <- H.Sys.native_pid()
    rt : String <- K.Rt.env("XDG_RUNTIME_DIR")
    KServe.loop(KServe.forever(), q, KServe.boot(pid, rt, exe, home, q, ev))

# What the hub asks
# -----------------

# a command for a project's server; answers the helper's reply line (or
# {"ok":false,"error":...,"fallback":true})
def KServe.ask_on(+q: Chan(KMsg), +project: String, env: KEnv, +ty: String, msg: J.Json) -> IO(String):
  do IO<String>:
    +reply : Chan(String) <- Chan.new(String, 1)
    ok : Result<&1, &1, KMsg, Unit> <- Chan.send(KMsg, q, KAsk{project, env, ty, msg, reply})
    m : Maybe<&1, String> <- Chan.recv(String, reply)
    return Maybe.default(&1, String, m, KServe.failed("no reply"))

# the servers' lines now ("project|state|why", one per line)
def KServe.info_on(+q: Chan(KMsg)) -> IO(String):
  do IO<String>:
    +reply : Chan(String) <- Chan.new(String, 1)
    ok : Result<&1, &1, KMsg, Unit> <- Chan.send(KMsg, q, KInfo{reply})
    m : Maybe<&1, String> <- Chan.recv(String, reply)
    return Maybe.default(&1, String, m, "")

# the command line the project's server is started with
def KServe.argv_on(+q: Chan(KMsg), +project: String) -> IO(String):
  do IO<String>:
    +reply : Chan(String) <- Chan.new(String, 1)
    ok : Result<&1, &1, KMsg, Unit> <- Chan.send(KMsg, q, KArgv{project, reply})
    m : Maybe<&1, String> <- Chan.recv(String, reply)
    return Maybe.default(&1, String, m, "")

# Retry: failed servers are forgotten
def KServe.clear_on(+q: Chan(KMsg)) -> IO(Unit):
  do IO<Unit>:
    ok : Result<&1, &1, KMsg, Unit> <- Chan.send(KMsg, q, KClear{})
    return Unit{}
