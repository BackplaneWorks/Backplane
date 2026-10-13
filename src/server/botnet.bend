import Base
import ../core/port.bend as PT
import ./host.bend as H
import ./proc.bend as P
import ../core/json.bend as J
import ../core/utf8.bend as U
import ../core/sign.bend as SG
import ../core/model.bend as M
import ../core/bot.bend as BO
import ../core/bothub.bend as BH
import ../core/far.bend as FR
import ../core/live.bend as LV
import ../core/farlive.bend as FL
import ../core/teach.bend as TE

# Bots' IO that needs secrets (docs/bots.md): webhook and peer secrets,
# signed requests to linked machines, verifying signed requests to us, and
# the directory of the bots on linked machines. server.bend wires these
# into the hub; nothing here talks to the hub.
#
# Secrets are files under <home>/secrets/ (directories 0700, files 0600).
# They never go into argv, the log, the event log or a broadcast: curl
# reads the signed headers on stdin and the body from a 0600 file.

# what the server keeps for bots: the linked machines' bots (a JSON array,
# as clients read it), the signatures accepted within the window, and the
# threads elsewhere (far.bend: mirrors, pushes, this hub's feed), and each
# thread's streamed text since its last message (core/live.bend)
type Bn is Data:
  Bn{dir: String, seen: SG.Seen, work: BH.DirWork, far: FR.Fh, live: List<&2, LV.Buf>, broken: String, fl: List<&2, FL.LQ>, tch: TE.TeachSt}

def Bn.empty() -> Bn:
  Bn{"[]", SG.Seen.empty(), BH.DirWork.empty(), FR.Fh.empty(), Nil{}, "", Nil{}, TE.Teach.empty()}

def Bn.dir(b: Bn) -> String:
  match b:
    case Bn{d, _, _, _, lv, _, _, _}:
      d

def Bn.far(b: Bn) -> FR.Fh:
  match b:
    case Bn{_, _, _, f, lv, _, _, _}:
      f

def Bn.live(b: Bn) -> List<&2, LV.Buf>:
  match b:
    case Bn{_, _, _, _, lv, _, _, _}:
      lv

def Bn.fl(b: Bn) -> List<&2, FL.LQ>:
  match b:
    case Bn{_, _, _, _, _, _, q, _}:
      q

def Bn.with_fl(b: Bn, q: List<&2, FL.LQ>) -> Bn:
  match b:
    case Bn{d, s, w, f, lv, bk, _, tc}:
      Bn{d, s, w, f, lv, bk, q, tc}

def Bn.with_live(b: Bn, lv: List<&2, LV.Buf>) -> Bn:
  match b:
    case Bn{d, s, w, f, _, bk, fl, tc}:
      Bn{d, s, w, f, lv, bk, fl, tc}

# why the log can no longer be appended to ("" while it can): a failed
# write could not be taken back, so nothing more is committed until it is
def Bn.broken(b: Bn) -> String:
  match b:
    case Bn{_, _, _, _, _, bk, _, _}:
      bk

def Bn.with_broken(b: Bn, +bk: String) -> Bn:
  match b:
    case Bn{d, s, w, f, lv, _, fl, tc}:
      Bn{d, s, w, f, lv, bk, fl, tc}

def Bn.with_far(b: Bn, f: FR.Fh) -> Bn:
  match b:
    case Bn{d, s, w, _, lv, bk, fl, tc}:
      Bn{d, s, w, f, lv, bk, fl, tc}

# Files
# -----

def Bf.read.done(r: File & Result<&1, &1, U32 & String, List<&2, U32>>) -> IO(List<&2, U32>):
  (f, res) = r
  do IO<List<&2, U32>>:
    File.close(f)
    return Result.default(&1, &1, U32 & String, List<&2, U32>, res, Nil{})

def Bf.read.sized(r: File & Result<&1, &1, U32 & String, U32>) -> IO(List<&2, U32>):
  (f, res) = r
  IO.bind(File & Result<&1, &1, U32 & String, List<&2, U32>>, List<&2, U32>,
    File.read_bytes(f, U32.max(Result.default(&1, &1, U32 & String, U32, res, 0), 1)), Bf.read.done)

def Bf.read.opened(r: Result<&1, &1, U32 & String, File>) -> IO(List<&2, U32>):
  match r:
    case Done{f}:
      IO.bind(File & Result<&1, &1, U32 & String, U32>, List<&2, U32>, File.size(f), Bf.read.sized)
    case Fail{_}:
      IO.pure(List<&2, U32>, Nil{})

# a whole file's bytes, [] when it cannot be read
def Bf.read(path: String) -> IO(List<&2, U32>):
  IO.bind(Result<&1, &1, U32 & String, File>, List<&2, U32>, File.open(path, "r"), Bf.read.opened)

def Bf.text(path: String) -> IO(String):
  do IO<String>:
    bs : List<&2, U32> <- Bf.read(path)
    return U.Utf8.decode(bs)

def Bf.done(r: Result<&1, &1, U32 & String, Unit>) -> Bool:
  match r:
    case Done{_}:
      True{}
    case Fail{_}:
      False{}

def Bf.wrote(r: File & Result<&1, &1, U32 & String, Unit>) -> IO(Bool):
  (f, res) = r
  do IO<Bool>:
    File.close(f)
    return Bf.done(res)

def Bf.write.opened(r: Result<&1, &1, U32 & String, File>, text: String) -> IO(Bool):
  match r:
    case Done{f}:
      IO.bind(File & Result<&1, &1, U32 & String, Unit>, Bool, File.write(f, text), Bf.wrote)
    case Fail{_}:
      IO.pure(Bool, False{})

# a file only its owner reads (0600, set before anything is written)
def Bf.secret(+path: String, text: String) -> IO(Bool):
  do IO<Bool>:
    f : Result<&1, &1, U32 & String, File> <- File.open(path, "w")
    m : Result<&1, &1, U32 & String, Unit> <- H.FS.chmod(path, 384)
    ok : Bool <- Bf.write.opened(f, text)
    return Bool.and(ok, Bf.done(m))

def Bf.dir(+path: String) -> IO(Unit):
  do IO<Unit>:
    mk : Result<&1, &1, U32 & String, Unit> <- H.FS.mkdirs(path)
    m : Result<&1, &1, U32 & String, Unit> <- H.FS.chmod(path, 448)
    return Unit{}

# <home>/secrets and one directory in it, both 0700
def Bf.secrets(+home: String, +sub: String) -> IO(Unit):
  do IO<Unit>:
    Bf.dir(home ++ "/secrets")
    Bf.dir(home ++ "/secrets/" ++ sub)

def Bf.rm(path: String) -> IO(Unit):
  do IO<Unit>:
    m : Result<&1, &1, U32 & String, Unit> <- H.FS.remove(path)
    return Unit{}

def Bf.env.or(r: Result<&1, &1, U32 & String, String>) -> String:
  match r:
    case Done{v}:
      v
    case Fail{_}:
      ""

def Bf.env(name: String) -> IO(String):
  do IO<String>:
    r : Result<&1, &1, U32 & String, String> <- IO.get_env(name)
    return Bf.env.or(r)

# Names and ids
# -------------

def Bn.idc(+c: U32) -> Bool:
  Bool.or(Bool.or(Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57)), Bool.and(U32.is_ge(c, 97), U32.is_le(c, 122))),
    Bool.or(Bool.and(U32.is_ge(c, 65), U32.is_le(c, 90)), Bool.or(U32.is_eq(c, 45), U32.is_eq(c, 95))))

def Bn.idchars(s: String) -> Bool:
  match s:
    case SNil{}:
      True{}
    case SCon{Chr{+c}, t}:
      Bool.and(Bn.idc(c), Bn.idchars(t))

# an id that may name a file: letters, digits, - and _ (no dots, no
# slashes), 1 to 80 of them
def Bn.safe(+id: String) -> Bool:
  Bool.and(Bool.and(Bool.not(String.is_empty(id)), Nat.is_le(String.length(id), 80n)), Bn.idchars(id))

def Bn.namec(+c: U32) -> Bool:
  Bool.or(Bn.idc(c), U32.is_eq(c, 46))

def Bn.clean.go(s: String, +acc: String) -> String:
  match s:
    case SNil{}:
      String.reverse(acc)
    case SCon{Chr{+c}, t}:
      Bn.clean.go(t, Bool.pick(String, Bn.namec(c), SCon{Chr{c}, acc}, acc))

# a name another machine told us, kept to letters, digits, . - _ (so it
# can never hide an @ or a path), at most 40
def Bn.clean(s: String) -> String:
  String.take(Bn.clean.go(s, ""), 40n)

def Bn.urlc(u: String) -> Bool:
  match u:
    case SNil{}:
      True{}
    case SCon{Chr{+c}, t}:
      Bool.and(Bool.and(U32.is_gt(c, 32), U32.is_lt(c, 127)), Bn.urlc(t))

# an address we may send to: http(s), printable, no spaces, not too long
def Bn.url.ok(+u: String) -> Bool:
  Bool.and(Bool.or(String.starts_with(u, "http://"), String.starts_with(u, "https://")),
    Bool.and(Nat.is_le(String.length(u), 300n), Bn.urlc(u)))

def Bn.url.base(+u: String) -> String:
  Bool.pick(String, String.ends_with(u, "/"), String.take(u, Nat.sub(String.length(u), 1n)), u)

# Secrets
# -------

def Bn.digit(+d: U32) -> Char:
  Chr{U32.add(d, Bool.pick(U32, U32.is_lt(d, 10), 48, 87))}

def Bn.hex.go(n: Nat, +x: U32, acc: String) -> String:
  match n:
    case 0n:
      acc
    case 1n+m:
      Bn.hex.go(m, U32.shrn(x, 4n), SCon{Bn.digit(U32.and(x, 15)), acc})

def Bn.hex(+x: U32) -> String:
  Bn.hex.go(8n, x, "")

# n random words from the host's entropy (getrandom), as 8n hex digits
def Bn.random(n: Nat) -> IO(String):
  match n:
    case 0n:
      IO.pure(String, "")
    case 1n+m:
      do IO<String>:
        +a : U32 <- IO.try(U32, IO.random_u32())
        rest : String <- Bn.random(m)
        return Bn.hex(a) ++ rest

# a secret: 32 random bytes as 64 hex digits
def Bn.secret.fresh() -> IO(String):
  Bn.random(8n)

def Bn.hook.path(+home: String, +id: String) -> String:
  home ++ "/secrets/hooks/" ++ id

def Bn.peer.path(+home: String, +id: String) -> String:
  home ++ "/secrets/peers/" ++ id

def Bn.made(ok: Bool, secret: String) -> String:
  match ok:
    case True{}:
      secret
    case False{}:
      ""

def Bn.hook.make.k(safe: Bool, +home: String, +id: String) -> IO(String):
  match safe:
    case True{}:
      do IO<String>:
        Bf.secrets(home, "hooks")
        +secret : String <- Bn.secret.fresh()
        ok : Bool <- Bf.secret(Bn.hook.path(home, id), secret)
        return Bn.made(ok, secret)
    case False{}:
      IO.pure(String, "")

# a webhook's new secret, kept in its file; "" when it could not be kept
def Bn.hook.make(+home: String, +id: String) -> IO(String):
  Bn.hook.make.k(Bn.safe(id), home, id)

def Bn.peer.save.k(safe: Bool, +home: String, +id: String, +secret: String) -> IO(Bool):
  match safe:
    case True{}:
      do IO<Bool>:
        Bf.secrets(home, "peers")
        Bf.secret(Bn.peer.path(home, id), J.Json.show(J.Json.obj([J.KV{"secret", J.Str{secret}}])))
    case False{}:
      IO.pure(Bool, False{})

# a linked machine's secret, kept as {"secret": hex}
def Bn.peer.save(+home: String, +id: String, +secret: String) -> IO(Bool):
  Bn.peer.save.k(Bn.safe(id), home, id, secret)

# a webhook's bearer token sits beside its secret: "<id>.token" (a plain
# id has no dot, so it never names another hook's file)
def Bn.token.path(+home: String, +id: String) -> String:
  Bn.hook.path(home, id) ++ ".token"

def Bn.hook.forget.k(+home: String, +id: String) -> IO(Unit):
  do IO<Unit>:
    Bf.rm(Bn.hook.path(home, id))
    Bf.rm(Bn.token.path(home, id))

# a revoked hook's secret and token both go
def Bn.hook.forget(+home: String, +id: String) -> IO(Unit):
  Bool.pick(IO(Unit), Bn.safe(id), Bn.hook.forget.k(home, id), IO.pure(Unit, Unit{}))

def Bn.token.k(safe: Bool, +home: String, +id: String) -> IO(String):
  match safe:
    case True{}:
      IO.bind(String, String, Bf.text(Bn.token.path(home, id)), t => IO.pure(String, String.trim(t)))
    case False{}:
      IO.pure(String, "")

# a webhook's bearer token, "" when it has none
def Bn.token.of(+home: String, +id: String) -> IO(String):
  Bn.token.k(Bn.safe(id), home, id)

def Bn.token.make.k(safe: Bool, +home: String, +id: String) -> IO(String):
  match safe:
    case True{}:
      do IO<String>:
        Bf.secrets(home, "hooks")
        +token : String <- Bn.random(4n)
        ok : Bool <- Bf.secret(Bn.token.path(home, id), token)
        return Bn.made(ok, token)
    case False{}:
      IO.pure(String, "")

# a webhook's new bearer token (16 random bytes, 32 hex digits: short
# enough to type into a phone), replacing any other; "" when it could not
# be kept
def Bn.token.make(+home: String, +id: String) -> IO(String):
  Bn.token.make.k(Bn.safe(id), home, id)

def Bn.token.forget(+home: String, +id: String) -> IO(Unit):
  Bool.pick(IO(Unit), Bn.safe(id), Bf.rm(Bn.token.path(home, id)), IO.pure(Unit, Unit{}))

def Bn.peer.forget(+home: String, +id: String) -> IO(Unit):
  Bool.pick(IO(Unit), Bn.safe(id), Bf.rm(Bn.peer.path(home, id)), IO.pure(Unit, Unit{}))

def Bn.secret.k(safe: Bool, hook: Bool, +home: String, +id: String) -> IO(String):
  match safe hook:
    case False{} _:
      IO.pure(String, "")
    case True{} True{}:
      IO.bind(String, String, Bf.text(Bn.hook.path(home, id)), t => IO.pure(String, String.trim(t)))
    case True{} False{}:
      IO.bind(String, String, Bf.text(Bn.peer.path(home, id)),
        t => IO.pure(String, J.Json.get_str(Maybe.default(&2, J.Json, J.Json.parse(t), J.Null{}), "secret", "")))

# the secret of a webhook (hook) or a linked machine; "" when there is none
def Bn.secret.of(hook: Bool, +home: String, +id: String) -> IO(String):
  Bn.secret.k(Bn.safe(id), hook, home, id)

def Bn.peer.secret(+home: String, +id: String) -> IO(String):
  Bn.secret.of(False{}, home, id)

# Verifying
# ---------

def Bn.check.gh(ok: Bool, +key: String, +delivery: String) -> String:
  match ok:
    case True{}:
      Bool.pick(String, String.is_empty(delivery), "", key ++ ":g:" ++ delivery)
    case False{}:
      ""

def Bn.check.ours(m: Maybe<&2, Nat>, +secret: String, +now: Nat, +body: String, +sig: String, +key: String) -> String:
  match m:
    case Some{+ts}:
      Bool.pick(String, SG.Sign.check(secret, now, ts, body, sig), key ++ ":s:" ++ sig, "")
    case None{}:
      ""

def Bn.check.ring.k(ok: Bool, m: Maybe<&2, Nat>, +secret: String, +now: Nat, +dl: String, +tr: String, test: Bool, +body: String, +sig: String, +key: String) -> String:
  match ok m:
    case True{} Some{+ts}:
      Bool.pick(String, SG.Sign.index(secret, now, ts, dl, tr, test, body, sig), key ++ ":i:" ++ dl, "")
    case _ _:
      ""

# the Pebble Index ring (version 1): its headers came packed as
# "index\n<version>\n<ts>\n<delivery>\n<trigger>\n<test>\n<sig>"; the
# delivery id is the replay key, tagged ":i:" so the hub knows the ring
def Bn.check.ring.f(fs: List<&2, String>, +secret: String, +now: Nat, +body: String, +key: String) -> String:
  match fs:
    case _ <> +v <> +ts <> +dl <> +tr <> +te <> +sig <> _:
      Bn.check.ring.k(Bool.and(String.eq(v, "1"), Bool.not(String.is_empty(dl))), Nat.read(ts), secret, now, dl, tr, String.eq(te, "true"), body, sig, key)
    case _:
      ""

def Bn.check.gh.or(ring: Bool, +secret: String, +now: Nat, +body: String, +gh: String, +delivery: String, +key: String) -> String:
  match ring:
    case True{}:
      Bn.check.ring.f(String.split(gh, '\n'), secret, now, body, key)
    case False{}:
      Bn.check.gh(SG.Sign.github(secret, body, gh), key, delivery)

def Bn.check.raw.k(ok: Bool, m: Maybe<&2, Nat>, +secret: String, +now: Nat, +dl: String, +tr: String, test: Bool, raw: List<&2, U32>, +sig: String, +key: String) -> String:
  match ok m:
    case True{} Some{+ts}:
      Bool.pick(String, SG.Sign.index.raw(secret, now, ts, dl, tr, test, raw, sig), key ++ ":i:" ++ dl, "")
    case _ _:
      ""

def Bn.check.raw.f(fs: List<&2, String>, +secret: String, +now: Nat, raw: List<&2, U32>, +key: String) -> String:
  match fs:
    case _ <> +v <> +ts <> +dl <> +tr <> +te <> +sig <> _:
      Bn.check.raw.k(Bool.and(String.eq(v, "1"), Bool.not(String.is_empty(dl))), Nat.read(ts), secret, now, dl, tr, String.eq(te, "true"), raw, sig, key)
    case _:
      ""

def Bn.check.raw.go(ring: Bool, +secret: String, +now: Nat, +key: String, +gh: String, raw: List<&2, U32>) -> String:
  match ring:
    case True{}:
      Bn.check.raw.f(String.split(gh, '\n'), secret, now, raw, key)
    case False{}:
      ""

# a Pebble ring's form with its recording: the Index signature over the
# body's bytes (only the ring sends one; "" for anything else)
def Bn.check.raw(+secret: String, +now: Nat, +key: String, +gh: String, raw: List<&2, U32>) -> String:
  Bn.check.raw.go(Bool.and(Bool.not(String.is_empty(secret)), String.starts_with(gh, "index\n")), secret, now, key, gh, raw)

def Bn.check.k(empty: Bool, ours: Bool, github: Bool, +secret: String, +now: Nat, +ts: String, +body: String, +sig: String, +gh: String,
    +delivery: String, +key: String) -> String:
  match empty ours github:
    case True{} _ _:
      ""
    case False{} True{} _:
      Bn.check.ours(Nat.read(ts), secret, now, body, sig, key)
    case False{} False{} True{}:
      Bn.check.gh.or(String.starts_with(gh, "index\n"), secret, now, body, gh, delivery, key)
    case False{} False{} False{}:
      ""

# the replay key of a genuine request, "" when it is not genuine: our
# signature (fresh, matching, compared in constant time) or, for a
# webhook, GitHub's with its delivery id. `key` names what was signed for.
def Bn.check(+hook: Bool, +secret: String, +now: Nat, +key: String, +ts: String, +sig: String, +gh: String, +delivery: String, +body: String) -> String:
  Bn.check.k(String.is_empty(secret), Bool.not(String.is_empty(sig)), Bool.and(hook, Bool.not(String.is_empty(gh))), secret, now, ts, body, sig, gh, delivery, key)

# a webhook called with Authorization: Bearer <token>: its replay key
# (tagged ":b:" with a fresh nonce, so each call is new; a static token
# cannot prove freshness anyway), else why it is refused, after a "!"
def Bn.bearer(+kept: String, +given: String, +key: String, +nonce: String) -> String:
  Bool.pick(String, String.is_empty(kept), "!notoken",
    Bool.pick(String, SG.Sign.bearer(kept, given), key ++ ":b:" ++ nonce, "!bearer"))

# a refused request: why, for the 401's text
def Bn.denied(+key: String) -> String:
  Bool.pick(String, String.eq(key, "!none"), "no credentials: send Authorization: Bearer <the webhook's token>, or sign the request",
    Bool.pick(String, String.eq(key, "!notoken"), "this webhook has no bearer token: make one in its bot's Webhooks tab",
      Bool.pick(String, String.eq(key, "!bearer"), "wrong bearer token", "bad or stale signature")))

# a key that lets nothing in: none, or a refusal
def Bn.refused(+key: String) -> Bool:
  Bool.or(String.is_empty(key), String.starts_with(key, "!"))

# a key accepted once: None when it was seen within the window, else the
# cache that now holds it
def Bn.seen.k(had: Bool, s: SG.Seen, +key: String, +now: Nat) -> Maybe<&2, SG.Seen>:
  match had:
    case True{}:
      None{}
    case False{}:
      Some{SG.Seen.add(s, key, now)}

def Bn.accept(+b: Bn, +key: String, +now: Nat) -> Maybe<&2, SG.Seen>:
  match b:
    case Bn{_, +s, _, _, lv, _, _, _}:
      Bn.seen.k(SG.Seen.has(s, key), s, key, now)

def Bn.with_seen(b: Bn, s: SG.Seen) -> Bn:
  match b:
    case Bn{d, _, w, f, lv, bk, fl, tc}:
      Bn{d, s, w, f, lv, bk, fl, tc}

def Bn.with_dir(b: Bn, d: String) -> Bn:
  match b:
    case Bn{_, s, w, f, lv, bk, fl, tc}:
      Bn{d, s, w, f, lv, bk, fl, tc}

# Signed requests out
# -------------------

type Resp is Data:
  Resp{code: Nat, body: String}

def Bn.resp.k(rev: List<&2, String>) -> Resp:
  match rev:
    case c <> t:
      Resp{Maybe.default(&2, Nat, Nat.read(String.trim(c)), 0n), String.join(List.reverse(&2, String, t), "\n")}
    case Nil{}:
      Resp{0n, ""}

# curl printed the body, then a line with the status code
def Bn.resp(r: P.Ran) -> Resp:
  match r:
    case P.Ran{0, out}:
      Bn.resp.k(List.reverse(&2, String, String.lines(out)))
    case P.Ran{_, _}:
      Resp{0n, ""}

def Bn.call.done(r: Socket & Result<&1, &1, U32 & String, Unit>, +pid: U32) -> IO(P.Ran):
  (s, _) = r
  do IO<P.Ran>:
    s2 : Socket <- H.Sock.shutdown(s)
    d : Socket & List<&2, U32> <- P.Proc.drain(100000n, s2, P.Chunks{Nil{}})
    P.Proc.finish(pid, d)

def Bn.call.started(started: U32 & Socket, +input: String) -> IO(P.Ran):
  (+pid, s) = started
  IO.bind(Socket & Result<&1, &1, U32 & String, Unit>, P.Ran, H.Sock.send_text(s, input), r => Bn.call.done(r, pid))

def Bn.call.spawned(r: Result<&1, &1, U32 & String, U32 & Socket>, +input: String) -> IO(P.Ran):
  match r:
    case Done{started}:
      Bn.call.started(started, input)
    case Fail{_}:
      IO.pure(P.Ran, P.Ran{127, ""})

# curl with its headers on stdin (-H @-); stderr dropped
def Bn.curl(+args: List<&2, String>, +headers: String) -> IO(P.Ran):
  IO.bind(Result<&1, &1, U32 & String, U32 & Socket>, P.Ran, H.Proc.spawn("curl", args, "/", 0), r => Bn.call.spawned(r, headers))

def Bn.headers(+peer: String, +now: Nat, +sig: String) -> String:
  "X-Backplane-Peer: " ++ peer ++ "\nX-Backplane-Timestamp: " ++ Nat.show(now) ++ "\nX-Backplane-Signature: " ++ sig ++ "\nContent-Type: application/json\n"

def Bn.data(post: Bool, +file: String) -> List<&2, String>:
  match post:
    case True{}:
      ["--data-binary", "@" ++ file]
    case False{}:
      Nil{}

# no redirects, http(s) only, bounded time and size
def Bn.args(+post: Bool, +file: String, +url: String) -> List<&2, String>:
  List.concat(&2, String, [["-sS", "--proto", "=http,https", "-m", "15", "--max-filesize", "1048576", "-X", Bool.pick(String, post, "POST", "GET"), "-H", "@-"],
    Bn.data(post, file), ["-w", "\n%{http_code}", "--url", url]])

def Bn.sent(r: P.Ran, +file: String) -> IO(Resp):
  do IO<Resp>:
    Bf.rm(file)
    return Bn.resp(r)

def Bn.signed.k(ok: Bool, +home: String, +url: String, +peer: String, +secret: String, +now: Nat, +path: String, +post: Bool, +body: String) -> IO(Resp):
  match ok:
    case True{}:
      do IO<Resp>:
        Bf.secrets(home, "tmp")
        +name : String <- Bn.random(4n)
        +file : String = home ++ "/secrets/tmp/" ++ name
        w : Bool <- Bf.secret(file, body)
        r : P.Ran <- Bn.curl(Bn.args(post, file, Bn.url.base(url) ++ path), Bn.headers(peer, now, SG.Sign.header(secret, now, body)))
        Bn.sent(r, file)
    case False{}:
      IO.pure(Resp, Resp{0n, "no secret, or not an http address"})

# a request to a linked machine, signed with its secret (POST with body,
# or GET over the empty body); code 0 when it could not be made
def Bn.signed(+home: String, +url: String, +peer: String, +secret: String, +now: Nat, +path: String, +post: Bool, +body: String) -> IO(Resp):
  Bn.signed.k(Bool.and(Bn.url.ok(url), Bool.not(String.is_empty(secret))), home, url, peer, secret, now, path, post, body)

# a signed request to a peer whose secret is in its file
def Bn.to_peer(+home: String, +url: String, +peer: String, +now: Nat, +path: String, +post: Bool, +body: String) -> IO(Resp):
  IO.bind(String, Resp, Bn.peer.secret(home, peer), secret => Bn.signed(home, url, peer, secret, now, path, post, body))

# The same for a request that waits or brings a file (the viewers of a
# thread elsewhere, core/fview.bend): a longer time limit, a bigger body,
# and the body written to `out` as it came when that is set (a file's bytes
# are not text); Resp's body is then "".
def Bn.out(+out: String) -> List<&2, String>:
  Bool.pick(List<&2, String>, String.is_empty(out), Nil{}, ["-o", out])

def Bn.args.long(+file: String, +url: String, +out: String, +secs: Nat, +max: Nat) -> List<&2, String>:
  List.concat(&2, String, [["-sS", "--proto", "=http,https", "-m", Nat.show(secs), "--max-filesize", Nat.show(max), "-X", "POST", "-H", "@-", "--data-binary", "@" ++ file],
    Bn.out(out), ["-w", "\n%{http_code}", "--url", url]])

def Bn.signed.long.k(ok: Bool, +home: String, +url: String, +peer: String, +secret: String, +now: Nat, +path: String, +body: String, +out: String, +secs: Nat, +max: Nat) -> IO(Resp):
  match ok:
    case True{}:
      do IO<Resp>:
        Bf.secrets(home, "tmp")
        +name : String <- Bn.random(4n)
        +file : String = home ++ "/secrets/tmp/" ++ name
        w : Bool <- Bf.secret(file, body)
        r : P.Ran <- Bn.curl(Bn.args.long(file, Bn.url.base(url) ++ path, out, secs, max), Bn.headers(peer, now, SG.Sign.header(secret, now, body)))
        Bn.sent(r, file)
    case False{}:
      IO.pure(Resp, Resp{0n, "no secret, or not an http address"})

def Bn.signed.long(+home: String, +url: String, +peer: String, +secret: String, +now: Nat, +path: String, +body: String, +out: String, +secs: Nat, +max: Nat) -> IO(Resp):
  Bn.signed.long.k(Bool.and(Bn.url.ok(url), Bool.not(String.is_empty(secret))), home, url, peer, secret, now, path, body, out, secs, max)

def Bn.to_peer.long(+home: String, +url: String, +peer: String, +now: Nat, +path: String, +body: String, +out: String, +secs: Nat, +max: Nat) -> IO(Resp):
  IO.bind(String, Resp, Bn.peer.secret(home, peer), secret => Bn.signed.long(home, url, peer, secret, now, path, body, out, secs, max))

# an unsigned POST with no body, to one of the owner's own machines (it
# trusts this one because Tailscale says whose it is: POST /bots/pair)
def Bn.plain.k(ok: Bool, +url: String) -> IO(Resp):
  match ok:
    case True{}:
      IO.bind(P.Ran, Resp, Bn.curl(["-sS", "--proto", "=http,https", "-m", "15", "--max-filesize", "65536", "-X", "POST", "-H", "@-", "-w", "\n%{http_code}", "--url", url], "Content-Length: 0\n"),
        r => IO.pure(Resp, Bn.resp(r)))
    case False{}:
      IO.pure(Resp, Resp{0n, "not an http address"})

def Bn.plain(+url: String) -> IO(Resp):
  Bn.plain.k(Bn.url.ok(url), url)

# Who we are
# ----------

def Bn.first(r: P.Ran) -> String:
  match r:
    case P.Ran{0, out}:
      String.trim(Maybe.default(&2, String, List.head(&2, String, String.lines(out)), ""))
    case P.Ran{_, _}:
      ""

def Bn.dns.k(+name: String) -> String:
  Bool.pick(String, String.ends_with(name, "."), String.take(name, Nat.sub(String.length(name), 1n)), name)

def Bn.dns(r: P.Ran) -> String:
  match r:
    case P.Ran{0, out}:
      Bn.dns.k(J.Json.get_str(J.Json.get_obj(Maybe.default(&2, J.Json, J.Json.parse(out), J.Null{}), "Self"), "DNSName", ""))
    case P.Ran{_, _}:
      ""

def Bn.at(+host: String, +port: U32) -> String:
  "http://" ++ host ++ ":" ++ U32.show(port)

# where other machines reach this hub: its MagicDNS name on the tailnet
# (the hub listens there on the same port), else its listen address
def Bn.url.here.k(tailnet: Bool, +host: String, +port: U32) -> IO(String):
  match tailnet:
    case True{}:
      do IO<String>:
        r : P.Ran <- P.Proc.run("sh", ["-c", (PT.Port.tmo() ++ "command -v tailscale >/dev/null && tmo 5 tailscale status --json")], "/")
        +name : String = Bn.dns(r)
        return Bool.pick(String, String.is_empty(name), Bn.at(host, port), Bn.at(name, port))
    case False{}:
      IO.pure(String, Bn.at(host, port))

# this hub's address for other hubs: BACKPLANE_URL when set (a hub bound to
# 0.0.0.0 off the tailnet names an address others reach), else as above
def Bn.url.here(tailnet: Bool, +host: String, +port: U32) -> IO(String):
  do IO<String>:
    +env : String <- Bf.env("BACKPLANE_URL")
    Bool.pick(IO(String), String.is_empty(String.trim(env)), Bn.url.here.k(tailnet, host, port), IO.pure(String, String.trim(env)))

# this machine's name for others: BACKPLANE_NAME, else its host name
def Bn.name() -> IO(String):
  do IO<String>:
    +env : String <- Bf.env("BACKPLANE_NAME")
    r : P.Ran <- P.Proc.run("hostname", Nil{}, "/")
    return Bn.clean(Bool.pick(String, String.is_empty(String.trim(env)), Bn.first(r), String.trim(env)))

# The directory
# -------------

def Bn.mine.one(+s: M.State, b: BO.Bot) -> J.Json:
  match b:
    case BO.Bot{_, n, +th, l, _, _, _}:
      J.Json.obj([J.KV{"name", J.Str{n}}, J.KV{"look", J.Json.nat(l)}, J.KV{"state", J.Str{M.Status.name(BH.Bots.status(s, th))}}])

def Bn.mine.go(bs: List<&2, BO.Bot>, +s: M.State) -> List<&2, J.Json>:
  match bs:
    case Nil{}:
      Nil{}
    case b <> t:
      Bn.mine.one(s, b) <> Bn.mine.go(t, s)

# our bots for a linked machine: [{"name", "look", "state"}]
def Bn.mine(+s: M.State) -> String:
  J.Json.show(J.Json.arr(Bn.mine.go(BO.World.bots(M.State.bots(s)), s)))

def Bn.item.keep(blank: Bool, it: J.Json, rest: List<&2, J.Json>) -> List<&2, J.Json>:
  match blank:
    case True{}:
      rest
    case False{}:
      it <> rest

def Bn.items(xs: List<&2, J.Json>, +peer: String, +pname: String, +now: Nat) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case +o <> t:
      Bn.item.keep(String.is_empty(Bn.clean(J.Json.get_str(o, "name", ""))),
        BH.Dir.item(peer, pname, J.Json.get_nat(o, "look", 0n), Bn.clean(J.Json.get_str(o, "name", "")), Bn.clean(J.Json.get_str(o, "state", "ready")), now),
        Bn.items(t, peer, pname, now))

# Only directory requests use the short deadline. Delivery keeps Bn.args
# and its existing 15-second timeout; signed headers still go over stdin.
def Bn.dir.args(url: String) -> List<&2, String>:
  ["-sS", "--proto", "=http,https", "--connect-timeout", "2", "-m", "4", "--max-filesize", "1048576",
    "-X", "GET", "-H", "@-", "-w", "\n%{http_code}", "--url", url]

def Bn.dir.signed(ok: Bool, +url: String, +peer: String, +secret: String, +now: Nat) -> IO(Resp):
  match ok:
    case True{}:
      do IO<Resp>:
        r : P.Ran <- Bn.curl(Bn.dir.args(Bn.url.base(url) ++ "/bots/dir"), Bn.headers(peer, now, SG.Sign.header(secret, now, "")))
        return Bn.resp(r)
    case False{}:
      IO.pure(Resp, Resp{0n, ""})

def Bn.dir.fetch(+home: String, +url: String, +peer: String, +now: Nat) -> IO(Resp):
  do IO<Resp>:
    +secret : String <- Bn.peer.secret(home, peer)
    Bn.dir.signed(Bool.and(Bn.url.ok(url), Bool.not(String.is_empty(secret))), url, peer, secret, now)

# Malformed JSON, including a valid object rather than an array, fails.
# An empty array is a successful replacement (the peer removed its bots).
def Bn.fetched.j(m: Maybe<&2, J.Json>, +peer: String, +pname: String, +now: Nat) -> Maybe<&2, List<&2, J.Json>>:
  match m:
    case Some{J.Arr{xs}}:
      Some{Bn.items(List.take(&2, J.Json, J.Json.elems(J.Arr{xs}), 200n), peer, pname, now)}
    case _:
      None{}

def Bn.fetched(r: Resp, +peer: String, +pname: String, +now: Nat) -> Maybe<&2, List<&2, J.Json>>:
  match r:
    case Resp{200n, body}:
      Bn.fetched.j(J.Json.parse(body), peer, pname, now)
    case _:
      None{}

def Bn.work(b: Bn) -> BH.DirWork:
  match b:
    case Bn{_, _, w, _, lv, _, _, _}:
      w

def Bn.with_work(b: Bn, w: BH.DirWork) -> Bn:
  match b:
    case Bn{d, s, _, f, lv, bk, fl, tc}:
      Bn{d, s, w, f, lv, bk, fl, tc}

# the teach state of the bots' browsers (core/teach.bend)
def Bn.tch(b: Bn) -> TE.TeachSt:
  match b:
    case Bn{_, _, _, _, _, _, _, t}:
      t

def Bn.with_tch(b: Bn, t: TE.TeachSt) -> Bn:
  match b:
    case Bn{d, s, w, f, lv, bk, fl, _}:
      Bn{d, s, w, f, lv, bk, fl, t}

def Bn.cache(home: String) -> String:
  home ++ "/bots.cache"

# One snapshot holds the last directory per peer, with original `seen`.
# The hub serializes these small local writes; rename keeps a restart
# from reading a partial file. No event-log entries or secrets here.
def Bn.cache.saved(ok: Bool, +home: String) -> IO(Unit):
  match ok:
    case True{}:
      do IO<Unit>:
        r : Result<&1, &1, U32 & String, Unit> <- H.FS.rename(Bn.cache(home) ++ ".tmp", Bn.cache(home))
        return Unit{}
    case False{}:
      IO.pure(Unit, Unit{})

def Bn.cache.save(+home: String, text: String) -> IO(Unit):
  IO.bind(Bool, Unit, Bf.secret(Bn.cache(home) ++ ".tmp", text), ok => Bn.cache.saved(ok, home))

def Bn.cache.load(text: String, ps: List<&2, BO.Peer>) -> String:
  J.Json.show(J.Json.arr(BH.Dir.cached(J.Json.elems(Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{})), ps)))

# A mirror batch appended to its file (made when missing).
def Bf.append(+path: String, text: String) -> IO(Bool):
  IO.bind(Result<&1, &1, U32 & String, File>, Bool, File.open(path, "a"), r => Bf.write.opened(r, text))
