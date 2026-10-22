import Base
import ../core/telemetry.bend as T
import ../core/sha256.bend as SH
import ../core/exe.bend as EX
import ../core/tz.bend as TZ
import ../core/model.bend as M
import ../core/semver.bend as SV
import ./google.bend as G
import ./host.bend as H
import ./proc.bend as P
import ./argv.bend as AV
import ../core/port.bend as PO

# A one-element list in Cfg avoids flattening its secrets into app state.
type TmCfg is Data:
  TmCfg{enabled: Bool, sandbox: Bool, url: String, kid: String, key: String, os: String, arch: String, day_secs: Nat, busy: Nat, id: String, channel: String, human: Nat}

def Tf.baked(name: String) -> IO(String):
  import "./effects/telemetry.c"
  import "./effects/telemetry.js"

def Tf.platform() -> IO(String):
  import "./effects/telemetry.c"
  import "./effects/telemetry.js"

def Tf.pow(id: String) -> IO(String):
  import "./effects/telemetry.c"
  import "./effects/telemetry.js"

def Tio.empty(sb: Bool) -> List<&2, TmCfg>:
  [TmCfg{False{}, sb, "", "", "", "", "", 0n, 0n, "", "", 0n}]

def Tio.sb(xs: List<&2, TmCfg>) -> Bool:
  match xs:
    case TmCfg{_, sb, _, _, _, _, _, _, _, _, +channel, _} <> _: sb
    case _: False{}

def Tio.env.k(r: Result<&1, &1, U32 & String, String>, d: String) -> String:
  match r:
    case Done{v}: v
    case Fail{_}: d

def Tio.env(+name: String) -> IO(String):
  do IO<String>:
    +test : String <- G.Gf.env("BACKPLANE_TELEMETRY_TEST", "")
    +b : String <- Tf.baked(name)
    r : Result<&1, &1, U32 & String, String> <- IO.get_env(name)
    return Bool.pick(String, String.eq(test, "1"), Tio.env.k(r, b), b)

def Tio.plat.cfg(p: String & String, enabled: Bool, sb: Bool, url: String, kid: String, key: String, day_secs: Nat, channel: String) -> List<&2, TmCfg>:
  (os, arch) = p
  [TmCfg{enabled, sb, url, kid, key, os, arch, day_secs, 0n, "", channel, 0n}]

def Tio.sandbox.args(xs: List<String>) -> Bool:
  match xs:
    case Nil{}: False{}
    case h <> t: Bool.or(String.eq(h, "--sandbox"), Tio.sandbox.args(t))

def Tio.config(+sb: Bool) -> IO(List<&2, TmCfg>):
  do IO<List<&2, TmCfg>>:
    +test : String <- G.Gf.env("BACKPLANE_TELEMETRY_TEST", "")
    +channel : String <- Tio.env("BACKPLANE_TELEMETRY_CHANNEL")
    +url : String <- Tio.env("BACKPLANE_TELEMETRY_URL")
    +kid : String <- Tio.env("BACKPLANE_TELEMETRY_KID")
    +key : String <- Tio.env("BACKPLANE_TELEMETRY_KEY")
    args : List<String> <- AV.Argv.args()
    se : String <- G.Gf.env("BACKPLANE_SANDBOX", "")
    tm : String <- G.Gf.env("BACKPLANE_TELEMETRY", "")
    dn : String <- G.Gf.env("DO_NOT_TRACK", "")
    ci : String <- G.Gf.env("CI", "")
    days : String <- G.Gf.env("BACKPLANE_TELEMETRY_DAY_SECS", "0")
    plat : String <- Tf.platform()
    return Tio.plat.cfg(T.Tm.plat(plat), Bool.and(T.Tm.config.ok(test, url, kid, key, channel), T.Tm.gate(T.TmEnv{Bool.or(sb, Tio.sandbox.args(args)), se, tm, dn, ci, url, kid, key})), sb, url, kid, key, Bool.pick(Nat, Bool.and(String.eq(test, "1"), T.Tm.loopback(url)), T.Tm.num(days), 0n), Bool.pick(String, Bool.and(String.eq(test, "1"), String.is_empty(channel)), "dev", channel))

def Tio.platform(xs: List<&2, TmCfg>, arch: Bool) -> String:
  match xs:
    case TmCfg{_, _, _, _, _, +os, +ar, _, _, _, +channel, _} <> _: Bool.pick(String, arch, ar, os)
    case _: ""

def Tio.channel(xs: List<&2, TmCfg>) -> String:
  match xs:
    case TmCfg{_, _, _, _, _, _, _, _, _, _, channel, _} <> _: channel
    case _: ""

def Tio.can(xs: List<&2, TmCfg>, +now: Nat) -> Bool:
  match xs:
    case TmCfg{on, _, _, _, _, _, _, _, +busy, _, +channel, _} <> _: Bool.and(on, Bool.or(Nat.is_eq(busy, 0n), Nat.is_ge(Nat.sub(now, busy), 600n)))
    case _: False{}

def Tio.owns(xs: List<&2, TmCfg>, gen: Nat) -> Bool:
  match xs:
    case TmCfg{on, _, _, _, _, _, _, _, busy, _, _, _} <> _:
      Bool.and(on, Nat.is_eq(busy, gen))
    case _: False{}

def Tio.busy(xs: List<&2, TmCfg>, at: Nat) -> List<&2, TmCfg>:
  match xs:
    case TmCfg{en, sb, url, kid, key, os, arch, d, _, +ident, +channel, human} <> _: [TmCfg{en, sb, url, kid, key, os, arch, d, at, ident, channel, human}]
    case _: Nil{}

def Tio.done(xs: List<&2, TmCfg>, +gen: Nat, +id: String) -> List<&2, TmCfg>:
  match xs:
    case TmCfg{en, sb, url, kid, key, os, arch, d, +b, +ident, +channel, human} <> _: [TmCfg{en, sb, url, kid, key, os, arch, d, Bool.pick(Nat, Nat.is_eq(b, gen), 0n, b), Bool.pick(String, Nat.is_eq(b, gen), id, ident), channel, human}]
    case _: Nil{}

def Tio.date(+now: Nat, +secs: Nat) -> String:
  # Test days alternate between today's UTC date and yesterday, both valid
  # for the real server's retention window. Only an env URL enables this.
  String.take(TZ.Tz.iso.at(Bool.pick(Nat, Bool.and(Nat.is_gt(secs, 0n), Nat.is_eq(Nat.mod(Nat.div(now, Bool.pick(Nat, Nat.is_eq(secs, 0n), 86400n, secs)), 2n), 1n)), Nat.sub(now, 86400n), now), "+00:00"), 10n)

def Tio.view(+st: M.State, +ver: String, +now: Nat, xs: List<&2, TmCfg>) -> T.TmView:
  match xs:
    case TmCfg{_, _, _, _, _, os, arch, days, _, _, +channel, _} <> _:
      T.TmView{T.Tm.share(st), T.Tm.asked(st), T.Tm.reset(st), ver, T.Tm.channel(ver, channel), os, arch, Tio.date(now, days), now}
    case _: T.TmView{False{}, False{}, "0", ver, "release", "linux", "x64", "", now}

# Server-owned evidence only: call after an authenticated, validated successful
# allowlisted human UI action, while sharing and consent are on. No payload is
# kept. This timestamp stays in memory and only its UTC day enters an event.
def Tio.human(xs: List<&2, TmCfg>, +now: Nat) -> List<&2, TmCfg>:
  match xs:
    case TmCfg{+on, sb, url, kid, key, os, arch, d, busy, id, channel, human} <> _:
      [TmCfg{on, sb, url, kid, key, os, arch, d, busy, id, channel, Bool.pick(Nat, on, now, human)}]
    case _: Nil{}

# Clear at any successful consent/reset change, including before an id exists.
# Opt-out must discard pending action evidence as well as queued activity.
def Tio.human.clear(xs: List<&2, TmCfg>) -> List<&2, TmCfg>:
  match xs:
    case TmCfg{on, sb, url, kid, key, os, arch, d, busy, id, channel, _} <> _:
      [TmCfg{on, sb, url, kid, key, os, arch, d, busy, id, channel, 0n}]
    case _: Nil{}

def Tio.human.day(xs: List<&2, TmCfg>) -> String:
  match xs:
    case TmCfg{on, _, _, _, _, _, _, days, _, _, _, +human} <> _:
      Bool.pick(String, Bool.and(on, Nat.is_gt(human, 0n)), Tio.date(human, days), "")
    case _: ""

def Tio.random() -> IO(String):
  do IO<String>:
    a : U32 <- IO.try(U32, IO.random_u32())
    b : U32 <- IO.try(U32, IO.random_u32())
    c : U32 <- IO.try(U32, IO.random_u32())
    d : U32 <- IO.try(U32, IO.random_u32())
    return SH.Hex.encode(SH.Sha256.be_bytes(a, SH.Sha256.be_bytes(b, SH.Sha256.be_bytes(c, SH.Sha256.be_bytes(d, Nil{})))))

def Tio.write(+home: String, st: T.TmState) -> IO(Unit):
  do IO<Unit>:
    G.Gf.write(home ++ "/telemetry/state.tmp", T.Tm.state.json(st))
    r : Result<&1, &1, U32 & String, Unit> <- H.FS.rename(home ++ "/telemetry/state.tmp", home ++ "/telemetry/state.json")
    return Unit{}

def Tio.pow.result(r: P.Ran) -> String:
  match r:
    case P.Ran{0, out}: String.trim(out)
    case _: ""

def Tio.pow.accept(+st: T.TmState, +proof: String) -> T.TmState:
  Bool.pick(T.TmState, Bool.and(Bool.not(String.is_empty(proof)), Bool.and(T.Tm.hex(proof), Bool.and(Nat.is_le(String.length(proof), 16n), T.Tm.pow.ok(T.TmState.id(st), proof, 18n)))), T.Tm.pow.put(st, proof), st)

def Tio.pow(needed: Bool, +st: T.TmState) -> IO(T.TmState):
  match needed:
    case False{}: IO.pure(T.TmState, st)
    case True{}:
      do IO<T.TmState>:
        exe : String <- H.Sys.exe_path()
        # through Proc.run, never Base's Process.run: on the JS runtime that
        # is a spawnSync that holds the hub's one loop for the child's life
        r : P.Ran <- P.Proc.run("sh", ["-c", PO.Port.tmo() ++ "tmo \"$@\" 2>/dev/null", "sh", "300", EX.Exe.child(exe), "--telemetry-pow", T.TmState.id(st)], "")
        return Tio.pow.accept(st, Tio.pow.result(r))

def Tio.plan(p: T.TmPlan) -> IO(T.TmState):
  match p:
    case T.TmPlan{st, need}: Tio.pow(need, st)

def Tio.retry.k(hit: Bool, s: String, rest: Nat) -> Nat:
  match hit:
    case True{}: T.Tm.num(String.trim(String.drop(s, 12n)))
    case False{}: rest

def Tio.retry.lines(xs: List<&2, String>) -> Nat:
  match xs:
    case Nil{}: 0n
    case +h <> t: Tio.retry.k(String.starts_with(String.to_lower(h), "retry-after:"), h, Tio.retry.lines(t))

def Tio.http.code(r: P.Ran) -> Nat:
  match r:
    case P.Ran{0, out}: T.Tm.num(String.trim(out))
    case _: 0n

def Tio.send(xs: List<&2, TmCfg>, +home: String, +st: T.TmState, +evs: List<&2, T.TmEv>, +now: Nat) -> IO(T.TmState):
  match xs:
    case TmCfg{_, _, +url, +kid, +key, _, _, _, _, _, +channel, _} <> _:
      do IO<T.TmState>:
        nonce : String <- Tio.random()
        +body : String = T.Tm.body(now, String.take(nonce, 16n), evs)
        +path : String = home ++ "/telemetry/request.curl"
        +hdr : String = home ++ "/telemetry/response.headers"
        G.Gf.write(path, "url = " ++ G.Gc.q(url) ++ "\nrequest = \"POST\"\nheader = \"Content-Type: application/json\"\n" ++ G.Gc.headers(T.Tm.headers(kid, key, now, body)) ++ "\ndata-binary = " ++ G.Gc.q(body) ++ "\noutput = \"/dev/null\"\ndump-header = " ++ G.Gc.q(hdr) ++ "\nwrite-out = \"%{http_code}\"\n")
        # --disable must be first: a user's .curlrc must not enable redirects,
        # tracing or other settings on a request holding telemetry credentials.
        r : P.Ran <- P.Proc.run("curl", ["--disable", "--silent", "--max-time", "10", "--max-filesize", "16384", "--proto", "=http,https", "-K", path], "")
        headers : String <- G.Gf.text(hdr)
        G.Gf.rm(path)
        G.Gf.rm(hdr)
        return T.Tm.sent(st, evs, Tio.http.code(r), Tio.retry.lines(String.split(headers, '\n')), now)
    case _: IO.pure(T.TmState, st)

def Tio.send.when(go: Bool, xs: List<&2, TmCfg>, home: String, st: T.TmState, evs: List<&2, T.TmEv>, now: Nat) -> IO(T.TmState):
  match go:
    case True{}: Tio.send(xs, home, st, evs, now)
    case False{}: IO.pure(T.TmState, st)

def Tio.finish(allowed: Bool, cfg: List<&2, TmCfg>, +home: String, +st: T.TmState, +evs: List<&2, T.TmEv>, +now: Nat) -> IO(String):
  match allowed:
    case False{}: IO.pure(String, "")
    case True{}:
      do IO<String>:
        +st2 : T.TmState <- Tio.send.when(Bool.and(Bool.not(List.is_empty(&2, T.TmEv, evs)), Nat.is_ge(now, T.TmState.next(st))), cfg, home, st, evs, now)
        Tio.write(home, st2)
        return T.TmState.id(st2)

def Tio.job(+cfg: List<&2, TmCfg>, +home: String, +view: T.TmView, +now: Nat, authorize: Unit -> IO(Bool)) -> IO(String):
  do IO<String>:
    mk : Result<&1, &1, U32 & String, Unit> <- H.FS.mkdirs(home ++ "/telemetry")
    md : Result<&1, &1, U32 & String, Unit> <- H.FS.chmod(home ++ "/telemetry", 448)
    raw : String <- G.Gf.text(home ++ "/telemetry/state.json")
    id : String <- Tio.random()
    +st : T.TmState <- Tio.plan(T.Tm.plan.activity(T.Tm.state.parse(raw), view, T.Tm.uuid(id), Tio.human.day(cfg)))
    +evs : List<&2, T.TmEv> = T.Tm.batch(T.Tm.ready(T.TmState.queue(st), now))
    allowed : Bool <- authorize(Unit{})
    Tio.finish(allowed, cfg, home, st, evs, now)

def Tio.args.copy(xs: List<String>) -> List<&2, String>:
  match xs:
    case Nil{}: Nil{}
    case h <> t: h <> Tio.args.copy(t)

def Tio.args.back(xs: List<&2, String>) -> List<String>:
  match xs:
    case Nil{}: Nil{}
    case h <> t: h <> Tio.args.back(t)

def Tio.main.pow(ok: Bool, id: String) -> IO(Unit):
  match ok:
    case False{}: IO.print("invalid UUIDv4")
    case True{}: IO.bind(String, Unit, Tf.pow(id), IO.print)

def Tio.main.k(xs: List<&2, String>, go: List<String> -> IO(Unit)) -> IO(Unit):
  match xs:
    case "--telemetry-pow" <> +id <> _:
      Tio.main.pow(T.Tm.uuid.ok(id), id)
    case x: go(Tio.args.back(x))

def Tio.main(xs: List<String>, go: List<String> -> IO(Unit)) -> IO(Unit):
  Tio.main.k(Tio.args.copy(xs), go)

def Tio.id(xs: List<&2, TmCfg>) -> String:
  match xs:
    case TmCfg{_, _, _, _, _, _, _, _, _, +id, +channel, _} <> _: Bool.pick(String, T.Tm.uuid.ok(id), id, "")
    case _: ""

def Tio.id.clear(xs: List<&2, TmCfg>) -> List<&2, TmCfg>:
  match xs:
    case TmCfg{en, sb, url, kid, key, os, arch, d, busy, _, +channel, _} <> _: [TmCfg{en, sb, url, kid, key, os, arch, d, busy, "", channel, 0n}]
    case _: Nil{}
