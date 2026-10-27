import Base
import ../core/model.bend as M
import ../core/json.bend as J
import ../core/hub.bend as B
import ../core/sha1.bend as S
import ../core/utf8.bend as U
import ../core/notice.bend as NO
import ../core/desk.bend as DK
import ../core/wnote.bend as WN
import ./host.bend as H
import ./proc.bend as P

# Phone notifications (APNs alerts and Live Activity updates). The hub
# calls Push.turn_ended once for every turn that ends, off its own thread
# (the server spawns it): the island's update only. An alert is a push of
# its own (Push.note.dead), raised when a thread's alert record is written
# (core/notice.bend, the server's Alerted), unless a desk in use is
# showing the thread (core/desk.bend).
#
# Nothing happens until the settings push.apns.key (the path of the .p8
# key; the key itself is never stored), push.apns.keyid and push.apns.team
# are set and the key file exists. Each push signs an ES256 token with
# `openssl` and sends it with `curl`, both host tools. Device tokens and
# the signed token go to curl in a config file (-K), never on its command
# line, and never into a log.
#
# push.webhook.url, when set to an http(s) address, also POSTs a JSON
# notice for each alert Notify me for would raise (core/wnote.bend):
# a settle or a failure from TurnEnded, and an ask, a merge, a reminder
# or a note from Alerted. A desk holding the phone push does not hold
# the webhook. Unset, that does nothing. An optional secret
# (push.webhook.secret, a path, or <home>/secrets/push.webhook)
# signs it; a POST that fails is logged and does not change what this
# returns.

# Files
# -----

def Push.read.done(r: File & Result<&1, &1, U32 & String, List<&2, U32>>) -> IO(List<&2, U32>):
  (f, res) = r
  do IO<List<&2, U32>>:
    File.close(f)
    return Result.default(&1, &1, U32 & String, List<&2, U32>, res, Nil{})

def Push.read.sized(r: File & Result<&1, &1, U32 & String, U32>) -> IO(List<&2, U32>):
  (f, res) = r
  IO.bind(File & Result<&1, &1, U32 & String, List<&2, U32>>, List<&2, U32>,
    File.read_bytes(f, U32.max(Result.default(&1, &1, U32 & String, U32, res, 0), 1)), Push.read.done)

def Push.read.opened(r: Result<&1, &1, U32 & String, File>) -> IO(List<&2, U32>):
  match r:
    case Done{f}:
      IO.bind(File & Result<&1, &1, U32 & String, U32>, List<&2, U32>, File.size(f), Push.read.sized)
    case Fail{_}:
      IO.pure(List<&2, U32>, Nil{})

# a whole file's bytes, or [] when it cannot be read
def Push.read(path: String) -> IO(List<&2, U32>):
  IO.bind(Result<&1, &1, U32 & String, File>, List<&2, U32>, File.open(path, "r"), Push.read.opened)

def Push.write.done(r: File & Result<&1, &1, U32 & String, Unit>) -> IO(Unit):
  (f, _) = r
  File.close(f)

def Push.write.opened(r: Result<&1, &1, U32 & String, File>, bs: List<&2, U32>) -> IO(Unit):
  match r:
    case Done{f}:
      IO.bind(File & Result<&1, &1, U32 & String, Unit>, Unit, File.write_bytes(f, bs), Push.write.done)
    case Fail{_}:
      IO.pure(Unit, Unit{})

# a file (mode 0600) holding text as UTF-8
def Push.write(+path: String, text: String) -> IO(Unit):
  do IO<Unit>:
    r : Result<&1, &1, U32 & String, File> <- File.open(path, "w")
    m : Result<&1, &1, U32 & String, Unit> <- H.FS.chmod(path, 384)
    Push.write.opened(r, U.Utf8.encode(text))

def Push.rm(path: String) -> IO(Unit):
  do IO<Unit>:
    r : Result<&1, &1, U32 & String, Unit> <- H.FS.remove(path)
    return Unit{}

def Push.exists.k(r: Result<&1, &1, U32 & String, U32 & U32 & U32>) -> Bool:
  match r:
    case Done{_}:
      True{}
    case Fail{_}:
      False{}

def Push.exists(path: String) -> IO(Bool):
  do IO<Bool>:
    r : Result<&1, &1, U32 & String, U32 & U32 & U32> <- H.FS.stat(path)
    return Push.exists.k(r)

def Push.env.k(r: Result<&1, &1, U32 & String, String>) -> String:
  match r:
    case Done{v}:
      v
    case Fail{_}:
      ""

def Push.env(name: String) -> IO(String):
  do IO<String>:
    r : Result<&1, &1, U32 & String, String> <- IO.get_env(name)
    return Push.env.k(r)

# JWT (ES256)
# -----------

def B64u.chr(plus: Bool, slash: Bool, pad: Bool, +c: U32, rest: String) -> String:
  match plus slash pad:
    case True{} _ _:
      SCon{Chr{45}, rest}
    case False{} True{} _:
      SCon{Chr{95}, rest}
    case False{} False{} True{}:
      rest
    case False{} False{} False{}:
      SCon{Chr{c}, rest}

def B64u.fix(s: String) -> String:
  match s:
    case SNil{}:
      SNil{}
    case SCon{Chr{+c}, t}:
      B64u.chr(U32.is_eq(c, 43), U32.is_eq(c, 47), U32.is_eq(c, 61), c, B64u.fix(t))

# base64url without padding (RFC 7515)
def B64u.encode(bs: List<&2, U32>) -> String:
  B64u.fix(S.B64.encode(bs))

# a DER INTEGER's bytes as exactly 32: leading zeros stripped or added
def Der.pad(+xs: List<&2, U32>) -> List<&2, U32>:
  List.append(&2, U32, List.replicate(U32, Nat.sub(32n, List.length(&2, U32, xs)), 0),
    List.drop(&2, U32, xs, Nat.sub(List.length(&2, U32, xs), 32n)))

def Der.two(r: List<&2, U32>, rest: List<&2, U32>) -> List<&2, U32>:
  match rest:
    case _ <> +sl <> t:
      List.append(&2, U32, Der.pad(r), Der.pad(List.take(&2, U32, t, U32.to_nat(sl))))
    case _:
      Nil{}

def Der.one(+rn: Nat, +rest: List<&2, U32>) -> List<&2, U32>:
  Der.two(List.take(&2, U32, rest, rn), List.drop(&2, U32, rest, rn))

# openssl's DER signature, SEQUENCE { INTEGER r, INTEGER s }, as the raw
# r || s (64 bytes) that JWS wants; [] when it is not one. A P-256
# signature is at most 72 bytes, so every length is one byte.
def Der.raw(bs: List<&2, U32>) -> List<&2, U32>:
  match bs:
    case _ <> _ <> _ <> +rl <> rest:
      Der.one(U32.to_nat(rl), rest)
    case _:
      Nil{}

def Jwt.part(+text: String) -> String:
  B64u.encode(U.Utf8.encode(text))

# header.claims, the part the signature covers
def Jwt.input(+kid: String, +team: String, +now: Nat) -> String:
  Jwt.part(J.Json.show(J.Json.obj([J.KV{"alg", J.Str{"ES256"}}, J.KV{"kid", J.Str{kid}}]))) ++ "."
    ++ Jwt.part(J.Json.show(J.Json.obj([J.KV{"iss", J.Str{team}}, J.KV{"iat", J.Num{Nat.show(now)}}])))

def Jwt.done(+input: String, +raw: List<&2, U32>) -> String:
  Bool.pick(String, Nat.is_eq(List.length(&2, U32, raw), 64n), input ++ "." ++ B64u.encode(raw), "")

# the provider token, or "" when openssl could not sign
def Jwt.sign(+key: String, +dir: String, +name: String, +input: String) -> IO(String):
  do IO<String>:
    +inp : String = dir ++ "/" ++ name ++ ".jwt"
    +sig : String = dir ++ "/" ++ name ++ ".sig"
    Push.write(inp, input)
    r : P.Ran <- P.Proc.run("openssl", ["dgst", "-sha256", "-sign", key, "-out", sig, inp], dir)
    bs : List<&2, U32> <- Push.read(sig)
    Push.rm(inp)
    Push.rm(sig)
    return Jwt.done(input, Der.raw(bs))

# Payloads
# --------

def Push.title(t: M.Thread) -> String:
  match t:
    case M.Thread{_, _, +title, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      Bool.pick(String, String.is_empty(title), "Backplane", title)

# the first line of the last reply, at most 180 characters
def Push.first(+text: String) -> String:
  String.take(String.trim(Maybe.default(&2, String, List.head(&2, String, String.lines(String.trim(text))), "")), 180n)

def Push.body(failed: Bool, +line: String) -> String:
  match failed:
    case True{}:
      "Failed"
    case False{}:
      Bool.pick(String, String.is_empty(line), "Finished", line)

def Push.alert.body.t(own: Bool, +said: String, failed: Bool, +st: M.State, +thread: String) -> String:
  match own:
    case True{}:
      said
    case False{}:
      Push.body(failed, Push.first(B.Task.last(st, thread)))

def Push.alert.body.k(+kind: String, +said: String, +st: M.State, +thread: String) -> String:
  Push.alert.body.t(Bool.or(Bool.or(String.eq(kind, "merged"), String.eq(kind, "ask")), Bool.not(String.is_empty(said))), said, String.eq(kind, "fail"), st, thread)

# an alert record's body: its own text (a note, an ask, a merge), else the
# last reply's first line
def Push.alert.body(+st: M.State, +thread: String, +rec: NO.AlertRec) -> String:
  Push.alert.body.k(NO.Alert.kind(rec), NO.Alert.body(rec), st, thread)

# thread-id groups by the notice's key, so a room's alerts stack together
def Push.alert(+thread: String, +key: String, title: String, body: String) -> String:
  J.Json.show(J.Json.obj([
    J.KV{"aps", J.Json.obj([
      J.KV{"alert", J.Json.obj([J.KV{"title", J.Str{title}}, J.KV{"body", J.Str{body}}])},
      J.KV{"sound", J.Str{"default"}},
      J.KV{"thread-id", J.Str{key}}])},
    J.KV{"thread", J.Str{thread}}, J.KV{"key", J.Str{key}}]))

# an alert that carries a picture (a task pause's screenshot): the hub URL
# of the image rides at the top level and the payload is mutable, for a
# service extension on the phone to fetch it
def Push.alert.image(+thread: String, +key: String, +title: String, +body: String, +image: String) -> String:
  Bool.pick(String, String.is_empty(image), Push.alert(thread, key, title, body),
    J.Json.show(J.Json.obj([
      J.KV{"aps", J.Json.obj([
        J.KV{"alert", J.Json.obj([J.KV{"title", J.Str{title}}, J.KV{"body", J.Str{body}}])},
        J.KV{"sound", J.Str{"default"}},
        J.KV{"mutable-content", J.Json.nat(1n)},
        J.KV{"thread-id", J.Str{key}}])},
      J.KV{"thread", J.Str{thread}}, J.KV{"key", J.Str{key}}, J.KV{"image", J.Str{image}}])))

# the island: every thread still running once this one has ended
def Island.keep(+thread: String, t: M.Thread) -> Bool:
  match t:
    case M.Thread{+id, _, _, _, _, M.Running{}, _, ar, _, _, _, _, _, _, _, _, _, _, _, _, de}:
      Bool.and(Bool.not(String.eq(id, thread)), Bool.not(Bool.or(ar, de)))
    case _:
      False{}

def Island.pick(keep: Bool, t: M.Thread, rest: List<&2, M.Thread>) -> List<&2, M.Thread>:
  match keep:
    case True{}:
      t <> rest
    case False{}:
      rest

def Island.running(ts: List<&2, M.Thread>, +thread: String) -> List<&2, M.Thread>:
  match ts:
    case Nil{}:
      Nil{}
    case +t <> rest:
      Island.pick(Island.keep(thread, t), t, Island.running(rest, thread))

def Island.others(s: M.State, +thread: String) -> List<&2, M.Thread>:
  match s:
    case M.State{_, ts, _, _, _, _, _, _, _, _, _}:
      Island.running(Map.values(&2, M.Thread, ts), thread)

# Production hub status uses the same compact record as the phone.
def Island.content(+s: M.State) -> NO.Compact:
  NO.Island.compact.of(NO.Island.compact.threads(s), s)

def Island.line(t: M.Thread) -> J.Json:
  match t:
    case M.Thread{id, _, title, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      J.Json.obj([J.KV{"thread", J.Str{id}}, J.KV{"title", J.Str{title}}, J.KV{"doing", J.Str{"Working"}}])

def Island.lines(ts: List<&2, M.Thread>) -> List<&2, J.Json>:
  match ts:
    case Nil{}:
      Nil{}
    case t <> rest:
      Island.line(t) <> Island.lines(rest)

def Island.first(ts: List<&2, M.Thread>) -> String:
  match ts:
    case t <> _:
      Push.title(t)
    case Nil{}:
      ""

def Island.headline(n: Nat, +title: String) -> String:
  match n:
    case 0n:
      "All agents idle"
    case 1n:
      title
    case 1n+1n+p:
      Nat.show(2n+p) ++ " agents working"

# Legacy ActivityAttributes.ContentState: {running, headline, lines}, every thread
# counted, the first NO.Notice.island.max() listed
def Island.state(+ts: List<&2, M.Thread>) -> J.Json:
  J.Json.obj([J.KV{"running", J.Num{Nat.show(List.length(&2, M.Thread, ts))}},
    J.KV{"headline", J.Str{Island.headline(List.length(&2, M.Thread, ts), Island.first(ts))}},
    J.KV{"lines", J.Json.arr(Island.lines(List.take(&2, M.Thread, ts, NO.Notice.island.max())))}])

# an ended island goes at once: one left on the lock screen read "All
# agents idle" beside the next one started
def Island.end(ends: Bool, +now: Nat) -> List<&2, J.KV>:
  match ends:
    case True{}:
      [J.KV{"event", J.Str{"end"}}, J.KV{"dismissal-date", J.Num{Nat.show(now)}}]
    case False{}:
      [J.KV{"event", J.Str{"update"}}]

# how long an island holds without word from its hub (seconds): a busy hub
# pushes it at least hourly (Push.island.beat), so one that went quiet
# longer is down, and iOS marks its Live Activity stale (the widget dims it)
def Island.fresh() -> Nat:
  10800n

def Island.payload(ends: Bool, +now: Nat, cs: J.Json) -> String:
  J.Json.show(J.Json.obj([J.KV{"aps", J.Json.obj(List.concat(&2, J.KV, [
    [J.KV{"timestamp", J.Num{Nat.show(now)}}, J.KV{"stale-date", J.Num{Nat.show(Nat.add(now, Island.fresh()))}}],
    Island.end(ends, now),
    [J.KV{"content-state", cs}]]))}]))

# Sending
# -------

# everything one turn's pushes share; key is the alert's collapse id, ""
# when the turn raises no alert (core/notice.bend), only the island's update
type Job is Data:
  Job{dir: String, base: String, jwt: String, now: Nat, thread: String, alert: String, island: J.Json, idle: Bool, key: String}

# a value for a curl config file: quoted, \ and " escaped, controls dropped
def Curl.q.k(ctl: Bool, esc: Bool, +c: U32, rest: String) -> String:
  match ctl esc:
    case True{} _:
      rest
    case False{} True{}:
      SCon{Chr{92}, SCon{Chr{c}, rest}}
    case False{} False{}:
      SCon{Chr{c}, rest}

def Curl.q.go(s: String) -> String:
  match s:
    case SNil{}:
      "\""
    case SCon{Chr{+c}, t}:
      Curl.q.k(U32.is_lt(c, 32), Bool.or(U32.is_eq(c, 34), U32.is_eq(c, 92)), c, Curl.q.go(t))

def Curl.q(s: String) -> String:
  "\"" ++ Curl.q.go(s)

def Curl.headers(hs: List<&2, String>) -> String:
  match hs:
    case Nil{}:
      ""
    case h <> t:
      "header = " ++ Curl.q(h) ++ "\n" ++ Curl.headers(t)

def Curl.config(+url: String, hs: List<&2, String>, +body: String, +out: String) -> String:
  "url = " ++ Curl.q(url) ++ "\n" ++ Curl.headers(hs) ++ "data-binary = " ++ Curl.q("@" ++ body) ++ "\n"
    ++ "output = " ++ Curl.q(out) ++ "\n" ++ "write-out = \"%{http_code}\"\n"

# the status curl wrote last (its errors come first, on the same stream)
def Curl.code(+out: String) -> String:
  String.drop(String.trim(out), Nat.sub(String.length(String.trim(out)), 3n))

# APNs says the token is gone: 410, or 400 BadDeviceToken
def Push.dead(+code: String, +reason: String) -> Bool:
  Bool.or(String.eq(code, "410"), Bool.and(String.eq(code, "400"), Bool.or(String.eq(reason, "BadDeviceToken"), String.eq(reason, "Unregistered"))))

def Push.reason(bs: List<&2, U32>) -> String:
  J.Json.get_str(Maybe.default(&2, J.Json, J.Json.parse(U.Utf8.decode(bs)), J.Null{}), "reason", "")

def Push.ran(r: P.Ran) -> String:
  match r:
    case P.Ran{_, out}:
      out

# one request to APNs; True when the token is dead
def Push.post(+dir: String, +name: String, +url: String, hs: List<&2, String>, +payload: String, +what: String) -> IO(Bool):
  do IO<Bool>:
    +body : String = dir ++ "/" ++ name ++ ".json"
    +out : String = dir ++ "/" ++ name ++ ".out"
    +cfg : String = dir ++ "/" ++ name ++ ".curl"
    Push.write(body, payload)
    Push.write(cfg, Curl.config(url, hs, body, out))
    r : P.Ran <- P.Proc.run("curl", ["--http2", "-sS", "--max-time", "15", "-K", cfg], dir)
    +code : String = Curl.code(Push.ran(r))
    resp : List<&2, U32> <- Push.read(out)
    Push.rm(cfg)
    Push.rm(body)
    Push.rm(out)
    +reason : String = Push.reason(resp)
    IO.print_err("backplane: push " ++ what ++ " -> " ++ code ++ Bool.pick(String, String.is_empty(reason), "", " " ++ reason))
    return Push.dead(code, reason)

def Push.url(+base: String, +env: String, +token: String) -> String:
  Bool.pick(String, String.is_empty(base),
    Bool.pick(String, String.eq(env, "sandbox"), "https://api.sandbox.push.apple.com", "https://api.push.apple.com"), base)
    ++ "/3/device/" ++ token

def Push.kind(+platform: String, +kind: String) -> Nat:
  Bool.pick(Nat, String.eq(platform, "ios"), Bool.pick(Nat, String.eq(kind, "alert"), 1n, Bool.pick(Nat, String.eq(kind, "activity"), 2n, 0n)), 0n)

def Push.one.alert(quiet: Bool, j: Job, +name: String, +token: String, +env: String, +bundle: String) -> IO(Bool):
  match quiet j:
    case True{} _:
      IO.pure(Bool, False{})
    case False{} Job{+dir, +base, +jwt, _, _, +alert, _, _, +key}:
      Push.post(dir, name, Push.url(base, env, token),
        ["authorization: bearer " ++ jwt, "apns-topic: " ++ bundle, "apns-push-type: alert", "apns-priority: 10", "apns-collapse-id: " ++ key],
        alert, "alert")

def Job.key(j: Job) -> String:
  match j:
    case Job{_, _, _, _, _, _, _, _, k}:
      k

def Push.one.k(i: Nat, +j: Job, +name: String, +token: String, +dthread: String, +env: String, +bundle: String) -> IO(Bool):
  match i j:
    case 1n _:
      Push.one.alert(String.is_empty(Job.key(j)), j, name, token, env, bundle)
    case 2n Job{+dir, +base, +jwt, +now, +thread, _, +island, +idle, _}:
      Push.post(dir, name, Push.url(base, env, token),
        ["authorization: bearer " ++ jwt, "apns-topic: " ++ bundle ++ ".push-type.liveactivity", "apns-push-type: liveactivity", "apns-priority: 10"],
        Island.payload(Bool.or(idle, String.eq(dthread, thread)), now, island), "activity")
    case _ _:
      IO.pure(Bool, False{})

def Push.one(d: M.Device, +j: Job, +name: String) -> IO(Bool):
  match d:
    case M.Device{+token, +platform, +kind, +dthread, +env, +bundle, _}:
      Push.one.k(Push.kind(platform, kind), j, name, token, dthread, env, bundle)

def Push.keep(dead: Bool, +token: String, rest: List<&2, String>) -> List<&2, String>:
  match dead:
    case True{}:
      token <> rest
    case False{}:
      rest

def Device.token(d: M.Device) -> String:
  match d:
    case M.Device{t, _, _, _, _, _, _}:
      t

def Push.all.k(dead: Bool, +token: String, rest: IO(List<&2, String>)) -> IO(List<&2, String>):
  IO.bind(List<&2, String>, List<&2, String>, rest, r => IO.pure(List<&2, String>, Push.keep(dead, token, r)))

# every device, one after another; answers the dead tokens
def Push.all(ds: List<&2, M.Device>, +j: Job, +prefix: String, +i: Nat) -> IO(List<&2, String>):
  match ds:
    case Nil{}:
      IO.pure(List<&2, String>, Nil{})
    case +d <> t:
      IO.bind(Bool, List<&2, String>, Push.one(d, j, prefix ++ "-" ++ Nat.show(i)),
        dead => Push.all.k(dead, Device.token(d), Push.all(t, j, prefix, 1n+i)))

def Push.devices(s: M.State) -> List<&2, M.Device>:
  Map.values(&2, M.Device, M.State.devices(s))

# a held alert says so in the log
def Push.held.k(none: Bool, +thread: String) -> IO(Unit):
  match none:
    case True{}:
      IO.print_err("backplane: push alert held for " ++ thread ++ " (a desk in use is showing it)")
    case False{}:
      IO.pure(Unit, Unit{})

def Push.held(held: Bool, +thread: String) -> IO(Unit):
  Push.held.k(held, thread)

def Push.gate(no: Bool, ds: List<&2, M.Device>, +j: Job, +prefix: String) -> IO(List<&2, String>):
  match no:
    case True{}:
      do IO<List<&2, String>>:
        IO.print_err("backplane: push: could not sign with push.apns.key")
        return Nil{}
    case False{}:
      Push.all(ds, j, prefix, 0n)

# the island update when a turn ended: no alert (an alert is its own push,
# Push.note.*, raised by the thread's record)
def Push.send(+home: String, +st: M.State, +thread: String, +key: String, +kid: String, +team: String) -> IO(List<&2, String>):
  do IO<List<&2, String>>:
    +dir : String = home ++ "/tmp"
    mk : Result<&1, &1, U32 & String, Unit> <- H.FS.mkdirs(dir)
    m : Result<&1, &1, U32 & String, Unit> <- H.FS.chmod(dir, 448)
    t : U32 <- H.Clock.epoch()
    +now : Nat = U32.to_nat(t)
    +prefix : String = "push-" ++ Nat.show(now) ++ "-" ++ thread
    +jwt : String <- Jwt.sign(key, dir, prefix, Jwt.input(kid, team, now))
    base : String <- Push.env("BACKPLANE_APNS_URL")
    +compact : NO.Compact = Island.content(st)
    +job : Job = Job{dir, base, jwt, now, thread, "", NO.Island.compact.json(compact, st), Nat.is_eq(NO.Island.compact.count(compact), 0n), ""}
    Push.gate(String.is_empty(jwt), Push.devices(st), job, prefix)

# phones are registered but the hub cannot push to them: say so, or a
# phone asleep never hears of a turn and nothing shows why
def Push.skip(phones: Bool) -> IO(List<&2, String>):
  match phones:
    case True{}:
      do IO<List<&2, String>>:
        IO.print_err("backplane: push: phones are registered but push.apns.key, push.apns.keyid and push.apns.team are not all set, or the key file is missing")
        return Nil{}
    case False{}:
      IO.pure(List<&2, String>, Nil{})

def Push.ready(ok: Bool, phones: Bool, +home: String, +st: M.State, +thread: String, +key: String, +kid: String, +team: String) -> IO(List<&2, String>):
  match ok:
    case True{}:
      Push.send(home, st, thread, key, kid, team)
    case False{}:
      Push.skip(phones)

def Push.any(ds: List<&2, M.Device>) -> Bool:
  Bool.not(List.is_empty(&2, M.Device, ds))

def Push.start(+home: String, +st: M.State, +thread: String, +key: String, +kid: String, +team: String) -> IO(List<&2, String>):
  do IO<List<&2, String>>:
    has : Bool <- Push.exists(key)
    +phones : Bool = Push.any(Push.devices(st))
    Push.ready(Bool.and(has, Bool.and(phones, Bool.not(Bool.or(String.is_empty(key), Bool.or(String.is_empty(kid), String.is_empty(team)))))),
      phones, home, st, thread, key, kid, team)

# The island alone (hub.bend's IslandSync, and the hourly beat while any
# thread works): every running thread, no alert. "@island" is no thread
# (no id has '@'), so none is left out and no device's thread matches it
def Push.island.send(+home: String, +st: M.State, +key: String, +kid: String, +team: String) -> IO(List<&2, String>):
  do IO<List<&2, String>>:
    +dir : String = home ++ "/tmp"
    mk : Result<&1, &1, U32 & String, Unit> <- H.FS.mkdirs(dir)
    m : Result<&1, &1, U32 & String, Unit> <- H.FS.chmod(dir, 448)
    t : U32 <- H.Clock.epoch()
    +now : Nat = U32.to_nat(t)
    +prefix : String = "push-" ++ Nat.show(now) ++ "-island"
    +jwt : String <- Jwt.sign(key, dir, prefix, Jwt.input(kid, team, now))
    base : String <- Push.env("BACKPLANE_APNS_URL")
    +compact : NO.Compact = Island.content(st)
    +job : Job = Job{dir, base, jwt, now, "@island", "", NO.Island.compact.json(compact, st), Nat.is_eq(NO.Island.compact.count(compact), 0n), ""}
    Push.gate(String.is_empty(jwt), Push.devices(st), job, prefix)

# only where an island can be: a configured key and a phone's Live
# Activity registered (nothing is said otherwise: a turn's end says it)
def Push.island.ready(ok: Bool, +home: String, +st: M.State, +key: String, +kid: String, +team: String) -> IO(List<&2, String>):
  match ok:
    case True{}:
      Push.island.send(home, st, key, kid, team)
    case False{}:
      IO.pure(List<&2, String>, Nil{})

def Push.activity(d: M.Device) -> Bool:
  match d:
    case M.Device{_, +platform, +kind, _, _, _, _}:
      Nat.is_eq(Push.kind(platform, kind), 2n)

def Push.activities(ds: List<&2, M.Device>) -> Bool:
  match ds:
    case Nil{}:
      False{}
    case d <> t:
      Bool.or(Push.activity(d), Push.activities(t))

def Push.island.start(+home: String, +st: M.State, +key: String, +kid: String, +team: String) -> IO(List<&2, String>):
  do IO<List<&2, String>>:
    has : Bool <- Push.exists(key)
    Push.island.ready(Bool.and(has, Bool.and(Push.activities(Push.devices(st)), Bool.not(Bool.or(String.is_empty(key), Bool.or(String.is_empty(kid), String.is_empty(team)))))),
      home, st, key, kid, team)

# the island pushed alone; answers the tokens APNs reported dead
def Push.island.dead(+home: String, +st: M.State) -> IO(List<&2, String>):
  Push.island.start(home, st, M.Setting.get(st, "push.apns.key", ""), M.Setting.get(st, "push.apns.keyid", ""), M.Setting.get(st, "push.apns.team", ""))

# whether the hourly beat is due (the hub's minute tick, in the first 90
# seconds of an hour: a tick late by a few seconds still lands in one) and
# any thread works
def Push.island.beat.k(due: Bool, st: M.State) -> Bool:
  match due:
    case False{}:
      False{}
    case True{}:
      Nat.is_gt(NO.Island.compact.count(Island.content(st)), 0n)

def Push.island.beat(+st: M.State, +now: Nat) -> Bool:
  Push.island.beat.k(Nat.is_lt(Nat.mod(now, 3600n), 90n), st)

# Webhook
# -------
# push.webhook.url posts one JSON notice per alert the notification
# policy raises. The URL, the body and the signature stay in a 0600 curl
# config, as the APNs push does. A missing curl, a refused address or a
# dead endpoint is logged; the dead-token list is still returned.

def Push.hook.kind(+st: M.State, +thread: String) -> String:
  NO.Alert.kind(NO.Alert.get(st, thread))

def Push.hook.word(+st: M.State, +thread: String, +state: String) -> String:
  WN.Wn.report(state, M.Thread.turn(B.Hub.thread(st, thread)))

def Push.hook.text(+st: M.State, +thread: String, +state: String) -> String:
  Push.body(String.eq(state, "failed"), Push.first(B.Task.last(st, thread)))

# a settle or a failure uses the last reply; any other alert uses the record's text
def Push.hook.line(ended: Bool, +st: M.State, +thread: String, +state: String) -> String:
  match ended:
    case True{}:
      Push.hook.text(st, thread, state)
    case False{}:
      Push.alert.body(st, thread, NO.Alert.get(st, thread))

# the newest error the thread logged (an Act of tone "error": a failed
# result's text, an exit), "" when none
def Push.hook.err(es: List<&2, M.Entry>, +thread: String) -> String:
  match es:
    case Nil{}:
      ""
    case M.Act{_, +th, +tone, _, +summary, _} <> t:
      Bool.pick(String, Bool.and(String.eq(th, thread), String.eq(tone, "error")), summary, Push.hook.err(t, thread))
    case _ <> t:
      Push.hook.err(t, thread)

# a broken turn (failed, interrupted) says its error when it logged one
def Push.hook.said(broke: Bool, +err: String, +line: String) -> String:
  Bool.pick(String, Bool.and(broke, Bool.not(String.is_empty(err))), Push.first(err), line)

def Push.hook.note.e(+err: String, +kind: String, +st: M.State, +thread: String, +state: String) -> String:
  WN.Wn.body.r(kind, thread, state, WN.Wn.reason(state, err ++ "\n" ++ B.Task.last(st, thread)), Push.title(B.Hub.thread(st, thread)),
    Push.hook.said(WN.Wn.broke(state), err, Push.hook.line(WN.Wn.ended(kind), st, thread, state)))

def Push.hook.note(+kind: String, +st: M.State, +thread: String, +state: String) -> String:
  Push.hook.note.e(Push.hook.err(M.State.mine(st, thread), thread), kind, st, thread, state)

def Push.hook.read(+path: String) -> IO(String):
  do IO<String>:
    bs : List<&2, U32> <- Push.read(path)
    return String.trim(U.Utf8.decode(bs))

def Push.hook.post(+home: String, +st: M.State, +thread: String, +kind: String, +state: String, +url: String, +secret: String) -> IO(Unit):
  do IO<Unit>:
    +dir : String = home ++ "/tmp"
    mk : Result<&1, &1, U32 & String, Unit> <- H.FS.mkdirs(dir)
    m : Result<&1, &1, U32 & String, Unit> <- H.FS.chmod(dir, 448)
    t : U32 <- H.Clock.epoch()
    +now : Nat = U32.to_nat(t)
    +word : String = Push.hook.word(st, thread, state)
    +body : String = Push.hook.note(kind, st, thread, word)
    +name : String = "hook-" ++ Nat.show(now) ++ "-" ++ WN.Wn.safe(thread)
    +file : String = dir ++ "/" ++ name ++ ".json"
    +out : String = dir ++ "/" ++ name ++ ".out"
    +cfg : String = dir ++ "/" ++ name ++ ".curl"
    Push.write(file, body)
    Push.write(cfg, Curl.config(url, WN.Wn.headers(secret, now, body), file, out))
    r : P.Ran <- P.Proc.run("curl", ["-sS", "--proto", "=http,https", "--max-redirs", "0", "--max-time", "15", "-X", "POST", "-K", cfg], dir)
    +code : String = Curl.code(Push.ran(r))
    Push.rm(cfg)
    Push.rm(file)
    Push.rm(out)
    IO.print_err("backplane: push webhook -> " ++ code)
    return Unit{}

def Push.hook.act(mode: Nat, +home: String, +st: M.State, +thread: String, +kind: String, +state: String, +url: String, +secret: String) -> IO(Unit):
  match mode:
    case 0n:
      IO.print_err("backplane: push webhook: push.webhook.secret could not be read")
    case _:
      Push.hook.post(home, st, thread, kind, state, url, secret)

def Push.hook.send(+home: String, +st: M.State, +thread: String, +kind: String, +state: String, +url: String) -> IO(Unit):
  do IO<Unit>:
    +set : String = M.Setting.get(st, WN.Wn.secret.key(), "")
    +text : String <- Push.hook.read(WN.Wn.path(home, set))
    Push.hook.act(WN.Wn.mode(WN.Wn.asked(set), text), home, st, thread, kind, state, url, text)

def Push.hook.bad() -> IO(Unit):
  IO.print_err("backplane: push webhook: push.webhook.url is not an http address")

def Push.hook.skip() -> IO(Unit):
  IO.pure(Unit, Unit{})

def Push.hook.go(ok: Bool, +home: String, +st: M.State, +thread: String, +kind: String, +state: String, +url: String) -> IO(Unit):
  match ok:
    case True{}:
      Push.hook.send(home, st, thread, kind, state, url)
    case False{}:
      Push.hook.bad()

def Push.hook.k(set: Bool, +home: String, +st: M.State, +thread: String, +kind: String, +state: String, +url: String) -> IO(Unit):
  match set:
    case False{}:
      Push.hook.skip()
    case True{}:
      Push.hook.go(WN.Wn.url.ok(url), home, st, thread, kind, state, url)

# `kind` is the alert the notice is for; `state` is the turn's end:
# "completed", "failed", "interrupted" or "stopped", or the thread's turn
# when the notice is not a turn end
def Push.hook(+home: String, +st: M.State, +thread: String, +kind: String, +state: String) -> IO(Unit):
  do IO<Unit>:
    +url : String = String.trim(M.Setting.get(st, WN.Wn.url.key(), ""))
    Push.hook.k(WN.Wn.on(url), home, st, thread, kind, state, url)

def Push.hook.when(go: Bool, +home: String, +st: M.State, +thread: String, +kind: String, +state: String) -> IO(Unit):
  match go:
    case True{}:
      Push.hook(home, st, thread, kind, state)
    case False{}:
      Push.hook.skip()

# a settle or a failure, from TurnEnded, when the policy raised one; and a
# turn that failed (an error, a limit) or was cut off, as a failure,
# whenever notify.settled would tell of one (wnote.bend's Wn.due.end)
def Push.hook.turn.k(+kind: String, +home: String, +st: M.State, +thread: String, +state: String) -> IO(Unit):
  Push.hook.when(WN.Wn.due.end(kind, state, NO.Policy.on(st, "settled")), home, st, thread, WN.Wn.end.kind(kind, state), state)

# alert: the kind of the record this end wrote (TurnEnded's), never the
# thread's persisted alert.<thread>, which a quiet end leaves as it was
def Push.hook.turn(+home: String, +st: M.State, +thread: String, +state: String, +alert: String) -> IO(Unit):
  Push.hook.turn.k(alert, home, st, thread, state)

# an ask, a merge, a reminder or a note, from Alerted (a settle is left to TurnEnded)
def Push.hook.alert(+home: String, +st: M.State, +thread: String) -> IO(Unit):
  Push.hook.when(WN.Wn.due.alert(Push.hook.kind(st, thread)), home, st, thread, Push.hook.kind(st, thread), WN.Wn.outcome(M.Thread.turn(B.Hub.thread(st, thread))))

# the island push for a turn that ended; answers the tokens APNs reported dead.
# the webhook runs after it and cannot drop the tokens it found
def Push.turn_ended.dead(+home: String, +st: M.State, +thread: String, +state: String, +alert: String) -> IO(List<&2, String>):
  do IO<List<&2, String>>:
    dead : List<&2, String> <- Push.start(home, st, thread, M.Setting.get(st, "push.apns.key", ""), M.Setting.get(st, "push.apns.keyid", ""), M.Setting.get(st, "push.apns.team", ""))
    Push.hook.turn(home, st, thread, state, alert)
    return dead

# the request that forgets a dead token (for the hub's inbox, as a client
# request would; the reply goes to no one)
def Push.drop_request(token: String) -> String:
  J.Json.show(J.Json.obj([J.KV{"id", J.Num{"0"}}, J.KV{"m", J.Str{"device.drop"}}, J.KV{"p", J.Json.obj([J.KV{"token", J.Str{token}}])}]))

def Push.turn_ended.done(dead: List<&2, String>) -> IO(Unit):
  IO.pure(Unit, Unit{})

# without the hub's inbox: dead tokens are found but not dropped (the hub
# calls Push.turn_ended.dead and posts Push.drop_request for each)
def Push.turn_ended(+home: String, st: M.State, +thread: String, +state: String, +alert: String) -> IO(Unit):
  IO.bind(List<&2, String>, Unit, Push.turn_ended.dead(home, st, thread, state, alert), Push.turn_ended.done)

# (a far turn's end is an alert only: each hub's Live Activity is its own
# threads', so this hub never ends or rewrites it for another's)
def Push.alerts.keep(alert: Bool, d: M.Device, rest: List<&2, M.Device>) -> List<&2, M.Device>:
  match alert:
    case True{}:
      d <> rest
    case False{}:
      rest

def Push.alerts.only(ds: List<&2, M.Device>) -> List<&2, M.Device>:
  match ds:
    case Nil{}:
      Nil{}
    case M.Device{+t, +p, +k, th, e, b, x} <> r:
      Push.alerts.keep(Nat.is_eq(Push.kind(p, k), 1n), M.Device{t, p, k, th, e, b, x}, Push.alerts.only(r))

# A turn that ended on another of the owner's machines (core/farlive.bend):
# that machine decided it is news and under which key; this hub pushes it
# to its own phones like any alert (same key: one collapse id, so a phone
# paired with both machines shows it once)
def Push.note.send(+home: String, +st: M.State, +thread: String, +title: String, +body: String, +akey: String, +image: String, +pkey: String, +kid: String, +team: String) -> IO(List<&2, String>):
  do IO<List<&2, String>>:
    +dir : String = home ++ "/tmp"
    mk : Result<&1, &1, U32 & String, Unit> <- H.FS.mkdirs(dir)
    m : Result<&1, &1, U32 & String, Unit> <- H.FS.chmod(dir, 448)
    t : U32 <- H.Clock.epoch()
    +now : Nat = U32.to_nat(t)
    +prefix : String = "push-" ++ Nat.show(now) ++ "-" ++ thread
    +jwt : String <- Jwt.sign(pkey, dir, prefix, Jwt.input(kid, team, now))
    base : String <- Push.env("BACKPLANE_APNS_URL")
    +compact : NO.Compact = Island.content(st)
    +job : Job = Job{dir, base, jwt, now, thread, Push.alert.image(thread, akey, title, body, image), NO.Island.compact.json(compact, st), Nat.is_eq(NO.Island.compact.count(compact), 0n), akey}
    Push.gate(String.is_empty(jwt), Push.alerts.only(Push.devices(st)), job, prefix)

def Push.note.go(ok: Bool, phones: Bool, +home: String, +st: M.State, +thread: String, +title: String, +body: String, +akey: String, +image: String, +pkey: String, +kid: String, +team: String) -> IO(List<&2, String>):
  match ok:
    case True{}:
      Push.note.send(home, st, thread, title, body, akey, image, pkey, kid, team)
    case False{}:
      Push.skip(phones)

# the push for a far turn's end (akey "" when a desk watches it: none goes)
def Push.note.dead(+home: String, +st: M.State, +thread: String, +title: String, +body: String, +akey: String, +image: String) -> IO(List<&2, String>):
  do IO<List<&2, String>>:
    +pkey : String = M.Setting.get(st, "push.apns.key", "")
    +kid : String = M.Setting.get(st, "push.apns.keyid", "")
    +team : String = M.Setting.get(st, "push.apns.team", "")
    has : Bool <- Push.exists(pkey)
    +phones : Bool = Push.any(Push.devices(st))
    Push.note.go(Bool.and(has, Bool.and(phones, Bool.not(Bool.or(String.is_empty(akey), Bool.or(String.is_empty(kid), String.is_empty(team)))))),
      phones, home, st, thread, title, body, akey, image, pkey, kid, team)
