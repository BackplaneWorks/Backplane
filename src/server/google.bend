import Base
import ../core/google.bend as G
import ../core/json.bend as J
import ../core/utf8.bend as U
import ./host.bend as H
import ./proc.bend as P

# Gmail and Google Calendar for bots: the IO half (src/core/google.bend is
# the pure one). Secrets live in <home>/secrets/google.json (directory
# 0700, file 0600) and never leave it: requests go through `curl`, whose
# URL, headers (the bearer token) and bodies (the client secret, codes,
# refresh tokens) come from a config file (-K) and data files in that
# directory, never its command line. Base URLs can be moved for tests with
# BACKPLANE_GOOGLE_OAUTH (token and revoke endpoints) and
# BACKPLANE_GOOGLE_API (Gmail and Calendar), and the shared client (see
# G.Google.shared) with BACKPLANE_GOOGLE_CLIENT_ID and _SECRET, at run
# time or baked in at build time (Gf.baked).

# Files
# -----

def Gf.read.done(r: File & Result<&1, &1, U32 & String, List<&2, U32>>) -> IO(List<&2, U32>):
  (f, res) = r
  do IO<List<&2, U32>>:
    File.close(f)
    return Result.default(&1, &1, U32 & String, List<&2, U32>, res, Nil{})

def Gf.read.sized(r: File & Result<&1, &1, U32 & String, U32>) -> IO(List<&2, U32>):
  (f, res) = r
  IO.bind(File & Result<&1, &1, U32 & String, List<&2, U32>>, List<&2, U32>,
    File.read_bytes(f, U32.max(Result.default(&1, &1, U32 & String, U32, res, 0), 1)), Gf.read.done)

def Gf.read.opened(r: Result<&1, &1, U32 & String, File>) -> IO(List<&2, U32>):
  match r:
    case Done{f}:
      IO.bind(File & Result<&1, &1, U32 & String, U32>, List<&2, U32>, File.size(f), Gf.read.sized)
    case Fail{_}:
      IO.pure(List<&2, U32>, Nil{})

# a whole file as UTF-8 text, "" when it cannot be read
def Gf.text(path: String) -> IO(String):
  do IO<String>:
    r : Result<&1, &1, U32 & String, File> <- File.open(path, "r")
    bs : List<&2, U32> <- Gf.read.opened(r)
    return U.Utf8.decode(bs)

def Gf.write.done(r: File & Result<&1, &1, U32 & String, Unit>) -> IO(Unit):
  (f, _) = r
  File.close(f)

def Gf.write.opened(r: Result<&1, &1, U32 & String, File>, bs: List<&2, U32>) -> IO(Unit):
  match r:
    case Done{f}:
      IO.bind(File & Result<&1, &1, U32 & String, Unit>, Unit, File.write_bytes(f, bs), Gf.write.done)
    case Fail{_}:
      IO.pure(Unit, Unit{})

# a file, mode 0600 before anything is in it, holding text as UTF-8
def Gf.write(+path: String, text: String) -> IO(Unit):
  do IO<Unit>:
    r : Result<&1, &1, U32 & String, File> <- File.open(path, "w")
    m : Result<&1, &1, U32 & String, Unit> <- H.FS.chmod(path, 384)
    Gf.write.opened(r, U.Utf8.encode(text))

def Gf.rm(path: String) -> IO(Unit):
  do IO<Unit>:
    r : Result<&1, &1, U32 & String, Unit> <- H.FS.remove(path)
    return Unit{}

def Gf.env.k(r: Result<&1, &1, U32 & String, String>) -> String:
  match r:
    case Done{v}:
      v
    case Fail{_}:
      ""

def Gf.env(name: String, +d: String) -> IO(String):
  do IO<String>:
    r : Result<&1, &1, U32 & String, String> <- IO.get_env(name)
    return G.Str.or(String.trim(Gf.env.k(r)), d)

def Gf.bytes4(+x: U32, acc: List<&2, U32>) -> List<&2, U32>:
  U32.shrn(x, 24n) <> U32.and(U32.shrn(x, 16n), 255) <> U32.and(U32.shrn(x, 8n), 255) <> U32.and(x, 255) <> acc

# 4n random bytes
def Gf.rand(n: Nat, acc: List<&2, U32>) -> IO(List<&2, U32>):
  match n:
    case 0n:
      IO.pure(List<&2, U32>, acc)
    case 1n+m:
      IO.bind(U32, List<&2, U32>, IO.try(U32, IO.random_u32()), x => Gf.rand(m, Gf.bytes4(x, acc)))

# random text from 4n bytes, base64url (RFC 7636's unreserved characters)
def Gf.nonce(n: Nat) -> IO(String):
  IO.bind(List<&2, U32>, String, Gf.rand(n, Nil{}), bs => IO.pure(String, G.Gb64.encode_url(bs)))

# The secrets file
# ----------------

# the client, a sign-in in flight (its PKCE verifier, state and redirect),
# and one account's tokens (G.Acct; the file keeps every account in
# "accounts"); expiry in epoch seconds
type GSec is Data:
  GSec{id: String, secret: String, refresh: String, access: String, expiry: Nat, email: String, verifier: String, state: String, redirect: String}

def Gs.dir(+home: String) -> String:
  home ++ "/secrets"

def Gs.path(+home: String) -> String:
  Gs.dir(home) ++ "/google.json"

# the client and the sign-in in flight, no account
def Gs.of(+j: J.Json) -> GSec:
  GSec{J.Json.get_str(j, "client_id", ""), J.Json.get_str(j, "client_secret", ""), "", "", 0n, "",
    J.Json.get_str(j, "pending_verifier", ""), J.Json.get_str(j, "pending_state", ""), J.Json.get_str(j, "redirect", "")}

def Gs.show(s: GSec, accts: List<&2, G.Acct>) -> String:
  match s:
    case GSec{id, secret, _, _, _, _, verifier, state, redirect}:
      J.Json.show(J.Json.obj([J.KV{"client_id", J.Str{id}}, J.KV{"client_secret", J.Str{secret}},
        J.KV{"pending_verifier", J.Str{verifier}}, J.KV{"pending_state", J.Str{state}},
        J.KV{"redirect", J.Str{redirect}}, J.KV{"accounts", G.Accts.json(accts)}]))

def Gs.file(+home: String) -> IO(J.Json):
  do IO<J.Json>:
    t : String <- Gf.text(Gs.path(home))
    return Maybe.default(&2, J.Json, J.Json.parse(t), J.Null{})

def Gs.load(+home: String) -> IO(GSec):
  IO.bind(J.Json, GSec, Gs.file(home), j => IO.pure(GSec, Gs.of(j)))

def Gs.accts(+home: String) -> IO(List<&2, G.Acct>):
  IO.bind(J.Json, List<&2, G.Acct>, Gs.file(home), j => IO.pure(List<&2, G.Acct>, G.Accts.of(j)))

# the client, the sign-in and the accounts, written whole to a 0600 file
# beside it, then renamed over it
def Gs.write(+home: String, s: GSec, accts: List<&2, G.Acct>) -> IO(Unit):
  do IO<Unit>:
    mk : Result<&1, &1, U32 & String, Unit> <- H.FS.mkdirs(Gs.dir(home))
    m : Result<&1, &1, U32 & String, Unit> <- H.FS.chmod(Gs.dir(home), 448)
    Gf.write(Gs.path(home) ++ ".new", Gs.show(s, accts))
    r : Result<&1, &1, U32 & String, Unit> <- H.FS.rename(Gs.path(home) ++ ".new", Gs.path(home))
    return Unit{}

def Gs.acct(s: GSec) -> G.Acct:
  match s:
    case GSec{_, _, refresh, access, expiry, email, _, _, _}:
      G.Acct{email, refresh, access, expiry}

def Gs.linked(s: GSec) -> Bool:
  match s:
    case GSec{_, _, refresh, _, _, _, _, _, _}:
      Bool.not(String.is_empty(refresh))

def Gs.put(linked: Bool, +accts: List<&2, G.Acct>, +s: GSec) -> List<&2, G.Acct>:
  match linked:
    case True{}:
      G.Accts.put(accts, Gs.acct(s))
    case False{}:
      accts

# the client and the sign-in, and s's account when it has one (put by its
# address among the others)
def Gs.save(+home: String, +s: GSec) -> IO(Unit):
  do IO<Unit>:
    +accts : List<&2, G.Acct> <- Gs.accts(home)
    Gs.write(home, s, Gs.put(Gs.linked(s), accts, s))

# s with one account's tokens (none: empty)
def Gs.with.acct(s: GSec, m: Maybe<&2, G.Acct>) -> GSec:
  match s m:
    case GSec{id, secret, _, _, _, _, v, st, r} Some{G.Acct{email, refresh, access, expiry}}:
      GSec{id, secret, refresh, access, expiry, email, v, st, r}
    case GSec{id, secret, _, _, _, _, v, st, r} None{}:
      GSec{id, secret, "", "", 0n, "", v, st, r}

# what the UI may know: {configured, connected, email (every address, the
# default first), accounts, pending, client} (Google.say turns it into a line)
def Gs.status(s: GSec, +accts: List<&2, G.Acct>) -> String:
  match s:
    case GSec{+id, _, _, _, _, _, _, state, _}:
      J.Json.show(J.Json.obj([J.KV{"configured", J.Flag{Bool.not(String.is_empty(id))}},
        J.KV{"connected", J.Flag{Bool.not(List.is_empty(&2, G.Acct, accts))}},
        J.KV{"email", J.Str{String.join(G.Accts.emails(accts), ", ")}},
        J.KV{"accounts", J.Json.arr(G.Tool.strs(G.Accts.emails(accts)))},
        J.KV{"pending", J.Flag{Bool.not(String.is_empty(state))}}, J.KV{"client", J.Str{id}}]))

def Google.status(+home: String) -> IO(String):
  IO.bind(J.Json, String, Gs.file(home), +j => IO.pure(String, Gs.status(Gs.of(j), G.Accts.of(j))))

def Gs.id(s: GSec) -> String:
  match s:
    case GSec{id, _, _, _, _, _, _, _, _}:
      id

def Gs.client(s: GSec) -> G.Client:
  match s:
    case GSec{id, secret, _, _, _, _, _, _, _}:
      G.Client{id, secret}

# a value scripts/build-js.sh baked in from .env, or "" (effects/google.js)
def Gf.baked(name: String) -> IO(String):
  import "./effects/google.js"

# the environment's value, else the baked one, else d
def Gf.env.baked(+name: String, +d: String) -> IO(String):
  do IO<String>:
    b : String <- Gf.baked(name)
    v : String <- Gf.env(name, G.Str.or(b, d))
    return v

# the client every install shares: the environment's, the one baked into
# this build, or G.Google.shared
def Gs.shared() -> IO(G.Client):
  do IO<G.Client>:
    i : String <- Gf.env.baked("BACKPLANE_GOOGLE_CLIENT_ID", G.Client.id(G.Google.shared()))
    x : String <- Gf.env.baked("BACKPLANE_GOOGLE_CLIENT_SECRET", G.Client.secret(G.Google.shared()))
    return G.Client{i, x}

# HTTP through curl
# -----------------

# a value for a curl config file: quoted, \ and " escaped, controls dropped
def Gc.q.k(ctl: Bool, esc: Bool, +c: U32, rest: String) -> String:
  match ctl esc:
    case True{} _:
      rest
    case False{} True{}:
      SCon{Chr{92}, SCon{Chr{c}, rest}}
    case False{} False{}:
      SCon{Chr{c}, rest}

def Gc.q.go(s: String) -> String:
  match s:
    case SNil{}:
      "\""
    case SCon{Chr{+c}, t}:
      Gc.q.k(U32.is_lt(c, 32), Bool.or(U32.is_eq(c, 34), U32.is_eq(c, 92)), c, Gc.q.go(t))

def Gc.q(s: String) -> String:
  "\"" ++ Gc.q.go(s)

def Gc.headers(hs: List<&2, String>) -> String:
  match hs:
    case Nil{}:
      ""
    case h <> t:
      "header = " ++ Gc.q(h) ++ "\n" ++ Gc.headers(t)

def Gc.data(has: Bool, +file: String) -> String:
  match has:
    case True{}:
      "data-binary = " ++ Gc.q("@" ++ file) ++ "\n"
    case False{}:
      ""

# an explicit method (PATCH, DELETE) as a curl config line
def Gc.method(+m: String) -> String:
  Bool.pick(String, String.is_empty(m), "", "request = " ++ Gc.q(m) ++ "\n")

# one request: its URL, headers, and a body file when there is a body
def Gc.config(+url: String, hs: List<&2, String>, has: Bool, +body: String, +out: String, +method: String) -> String:
  "url = " ++ Gc.q(url) ++ "\n" ++ Gc.method(method) ++ Gc.headers(hs) ++ Gc.data(has, body)
    ++ "output = " ++ Gc.q(out) ++ "\n" ++ "write-out = \"%{http_code}\"\n"

# the status curl printed last, and what it said before it (its errors)
type GResp is Data:
  GResp{code: String, body: String, err: String}

def Gc.code(+out: String) -> String:
  String.drop(String.trim(out), Nat.sub(String.length(String.trim(out)), 3n))

def Gc.said(+out: String) -> String:
  String.trim(String.take(String.trim(out), Nat.sub(String.length(String.trim(out)), 3n)))

def Gc.ran(r: P.Ran) -> String:
  match r:
    case P.Ran{_, out}:
      out

def Gc.body.put(has: Bool, +file: String, body: String) -> IO(Unit):
  match has:
    case True{}:
      Gf.write(file, body)
    case False{}:
      IO.pure(Unit, Unit{})

# the same with an explicit method ("PATCH", "DELETE"; "" lets curl choose)
def Gc.reqm(+dir: String, +url: String, hs: List<&2, String>, +body: String, +method: String) -> IO(GResp):
  do IO<GResp>:
    +name : String <- Gf.nonce(3n)
    +cfg : String = dir ++ "/g-" ++ name ++ ".curl"
    +inp : String = dir ++ "/g-" ++ name ++ ".in"
    +out : String = dir ++ "/g-" ++ name ++ ".out"
    +has : Bool = Bool.not(String.is_empty(body))
    Gc.body.put(has, inp, body)
    Gf.write(cfg, Gc.config(url, hs, has, inp, out, method))
    r : P.Ran <- P.Proc.run("curl", ["-sS", "--max-time", "30", "-K", cfg], dir)
    +said : String = Gc.ran(r)
    text : String <- Gf.text(out)
    Gf.rm(cfg)
    Gf.rm(inp)
    Gf.rm(out)
    return GResp{Gc.code(said), text, Gc.said(said)}

# a request; the body (form or JSON, as the headers say) when not ""
def Gc.req(+dir: String, +url: String, hs: List<&2, String>, +body: String) -> IO(GResp):
  Gc.reqm(dir, url, hs, body, "")

def Gc.ok(+code: String) -> Bool:
  String.starts_with(code, "2")

def Gc.why(+code: String) -> String:
  Bool.pick(String, String.eq(code, "401"), " (Google access expired or was revoked: connect Google again in Settings)", "")

# an unsuccessful answer as text for the agent
def Gc.fail(r: GResp) -> String:
  match r:
    case GResp{+code, +body, +err}:
      Bool.pick(String, Bool.or(String.eq(code, "000"), String.is_empty(body)),
        "error: could not reach Google: " ++ G.Str.or(err, "no answer"),
        "error: HTTP " ++ code ++ ": " ++ G.Google.api_error(body) ++ Gc.why(code))

def Gc.json(+s: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(s), J.Null{})

def Gc.answer.k(ok: Bool, r: GResp, f: J.Json -> String) -> String:
  match ok r:
    case True{} GResp{_, +body, _}:
      f(Gc.json(body))
    case False{} r2:
      Gc.fail(r2)

# a successful answer through f, else the error
def Gc.answer(+r: GResp, f: J.Json -> String) -> String:
  match r:
    case GResp{+code, _, _}:
      Gc.answer.k(Gc.ok(code), r, f)

# several GETs in one curl, one output file each

def Gc.outs(xs: List<&2, String>, +dir: String, +name: String, +i: Nat) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case _ <> t:
      (dir ++ "/g-" ++ name ++ "-" ++ Nat.show(i) ++ ".out") <> Gc.outs(t, dir, name, 1n+i)

def Gc.pairs(urls: List<&2, String>, outs: List<&2, String>) -> String:
  match urls outs:
    case u <> t o <> t2:
      "url = " ++ Gc.q(u) ++ "\noutput = " ++ Gc.q(o) ++ "\n" ++ Gc.pairs(t, t2)
    case _ _:
      ""

def Gc.read_all(ps: List<&2, String>, acc: List<&2, String>) -> IO(List<&2, String>):
  match ps:
    case Nil{}:
      IO.pure(List<&2, String>, List.reverse(&2, String, acc))
    case +p <> t:
      IO.bind(String, List<&2, String>, Gf.text(p), s => IO.bind(Unit, List<&2, String>, Gf.rm(p), u => Gc.read_all(t, s <> acc)))

# the bodies of GETs to each URL, in order ("" for one that failed)
def Gc.many(+dir: String, +urls: List<&2, String>, hs: List<&2, String>) -> IO(List<&2, String>):
  do IO<List<&2, String>>:
    +name : String <- Gf.nonce(3n)
    +cfg : String = dir ++ "/g-" ++ name ++ ".curl"
    +outs : List<&2, String> = Gc.outs(urls, dir, name, 0n)
    Gf.write(cfg, Gc.headers(hs) ++ Gc.pairs(urls, outs))
    r : P.Ran <- P.Proc.run("curl", ["-sS", "--max-time", "60", "-K", cfg], dir)
    Gf.rm(cfg)
    Gc.read_all(outs, Nil{})

# Endpoints
# ---------

type GEnds is Data:
  GEnds{oauth: String, api: String}

def Ge.get() -> IO(GEnds):
  do IO<GEnds>:
    o : String <- Gf.env("BACKPLANE_GOOGLE_OAUTH", "https://oauth2.googleapis.com")
    a : String <- Gf.env("BACKPLANE_GOOGLE_API", "https://www.googleapis.com")
    return GEnds{o, a}

def Ge.token(e: GEnds) -> String:
  match e:
    case GEnds{o, _}:
      o ++ "/token"

def Ge.revoke(e: GEnds) -> String:
  match e:
    case GEnds{o, _}:
      o ++ "/revoke"

def Ge.api(e: GEnds, +path: String) -> String:
  match e:
    case GEnds{_, a}:
      a ++ path

def Gh.form() -> List<&2, String>:
  ["Content-Type: application/x-www-form-urlencoded"]

def Gh.auth(+token: String) -> List<&2, String>:
  ["Authorization: Bearer " ++ token]

def Gh.json(+token: String) -> List<&2, String>:
  ["Authorization: Bearer " ++ token, "Content-Type: application/json; charset=UTF-8"]

# Setup
# -----

def Gs.config.k(same: Bool, s: GSec, +id: String, +secret: String) -> GSec:
  match same s:
    case True{} GSec{_, _, refresh, access, expiry, email, verifier, state, redirect}:
      GSec{id, secret, refresh, access, expiry, email, verifier, state, redirect}
    case False{} _:
      GSec{id, secret, "", "", 0n, "", "", "", ""}

# the same client keeps its accounts; another forgets them (their tokens
# were the old client's)
def Gs.configure.w(same: Bool, +home: String, s2: GSec) -> IO(Unit):
  match same:
    case True{}:
      Gs.save(home, s2)
    case False{}:
      Gs.write(home, s2, Nil{})

def Gs.configure.k(empty: Bool, +home: String, s: GSec, +id: String, +secret: String) -> IO(String):
  match empty s:
    case True{} _:
      IO.pure(String, "error: a Google OAuth client id and secret are both needed")
    case False{} GSec{+old, a, b, c, d, e, f, g, h}:
      do IO<String>:
        +same : Bool = String.eq(old, id)
        Gs.configure.w(same, home, Gs.config.k(same, GSec{old, a, b, c, d, e, f, g, h}, id, secret))
        st : String <- Google.status(home)
        return st

# the OAuth client (a Google Cloud "Desktop app" client); a different
# client forgets the tokens of the old one
def Google.configure(+home: String, client_id: String, client_secret: String) -> IO(String):
  do IO<String>:
    s : GSec <- Gs.load(home)
    +id : String = String.trim(client_id)
    +secret : String = String.trim(client_secret)
    Gs.configure.k(Bool.or(String.is_empty(id), String.is_empty(secret)), home, s, id, secret)

def Gs.challenge(+dir: String, +verifier: String) -> IO(String):
  do IO<String>:
    r : P.Ran <- P.Proc.run("sh", ["-c", "printf '%s' \"$1\" | openssl dgst -sha256 -binary | openssl base64 -A | tr '+/' '-_' | tr -d '='", "sh", verifier], dir)
    return String.trim(Gc.ran(r))

def Gs.begin.go(ok: Bool, +home: String, s: GSec, +verifier: String, +state: String, +challenge: String, +redirect: String, own: Bool) -> IO(String):
  match ok s:
    case False{} _:
      IO.pure(String, "error: could not make the PKCE challenge (is openssl installed?)")
    case True{} GSec{+id, secret, refresh, access, expiry, email, _, _, _}:
      do IO<String>:
        Gs.save(home, GSec{id, secret, refresh, access, expiry, email, verifier, state, redirect})
        return G.Google.auth_url(id, redirect, challenge, state, G.Google.scopes_for(own))

def Gs.begin.k(ok: Bool, +home: String, s: GSec, +redirect: String, +here: String, own: Bool) -> IO(String):
  match ok:
    case False{}:
      IO.pure(String, "error: this build has no shared Google client; add your own OAuth client (Desktop app) id and secret first")
    case True{}:
      do IO<String>:
        mk : Result<&1, &1, U32 & String, Unit> <- H.FS.mkdirs(Gs.dir(home))
        m : Result<&1, &1, U32 & String, Unit> <- H.FS.chmod(Gs.dir(home), 448)
        +verifier : String <- Gf.nonce(12n)
        +nonce : String <- Gf.nonce(4n)
        +state : String = G.Google.state(nonce, here)
        +challenge : String <- Gs.challenge(Gs.dir(home), verifier)
        Gs.begin.go(Nat.is_eq(String.length(challenge), 43n), home, s, verifier, state, challenge, redirect, own)

def Gs.pstate(s: GSec) -> String:
  match s:
    case GSec{_, _, _, _, _, _, _, state, _}:
      state

def Gs.configured(s: GSec) -> Bool:
  match s:
    case GSec{id, secret, _, _, _, _, _, _, _}:
      Bool.not(Bool.or(String.is_empty(id), String.is_empty(secret)))

# the secrets with the client a sign-in uses
def Gs.with(s: GSec, c: G.Client) -> GSec:
  match s c:
    case GSec{_, _, refresh, access, expiry, email, verifier, state, redirect} G.Client{id, secret}:
      GSec{id, secret, refresh, access, expiry, email, verifier, state, redirect}

def Gs.begin.with(+home: String, +s: GSec, +sh: String, +redirect: String, +here: String) -> IO(String):
  Gs.begin.k(Gs.configured(s), home, s, redirect, here, Bool.not(G.Google.is_shared(Gs.id(s), sh)))

# starts a sign-in: keeps a fresh PKCE verifier and state, and answers the
# URL the user opens (or "error: ..."). redirect is where Google sends the
# browser back: http://127.0.0.1:<port>/oauth/google. The user's own
# client when there is one, else the shared one (Calendar only). here is
# where other machines reach this hub, named in the state (G.Google.state)
# so a browser sent back to another machine's hub finds its way here.
def Google.begin(+home: String, +redirect: String, +here: String) -> IO(String):
  do IO<String>:
    +s : GSec <- Gs.load(home)
    +sh : G.Client <- Gs.shared()
    Gs.begin.with(home, Gs.with(s, G.Google.client(Gs.client(s), sh)), G.Client.id(sh), redirect, here)

def Gs.email.k(ok: Bool, +body: String) -> String:
  match ok:
    case True{}:
      J.Json.get_str(Gc.json(body), "emailAddress", "")
    case False{}:
      ""

def Gs.email.of(r: GResp) -> String:
  match r:
    case GResp{+code, +body, _}:
      Gs.email.k(Gc.ok(code), body)

# the account's address: the id_token's, else Gmail's profile's ("" when neither)
def Gs.email(have: Bool, +dir: String, +e: GEnds, +email: String, +access: String) -> IO(String):
  match have:
    case True{}:
      IO.pure(String, email)
    case False{}:
      do IO<String>:
        r : GResp <- Gc.req(dir, Ge.api(e, G.Google.gmail_profile_path()), Gh.auth(access), "")
        return Gs.email.of(r)

def Gc.bodyof(r: GResp) -> String:
  match r:
    case GResp{_, body, _}:
      body

def Gs.finish.done(has: Bool, +home: String, s2: GSec) -> IO(String):
  match has:
    case True{}:
      do IO<String>:
        Gs.save(home, s2)
        st : String <- Google.status(home)
        return st
    case False{}:
      IO.pure(String, "error: Google sent no refresh token; remove Backplane's access at https://myaccount.google.com/permissions and connect again")

def Gs.finish.keep(s: GSec, +access: String, +expiry: Nat, +refresh: String, +email: String) -> GSec:
  match s:
    case GSec{id, secret, oldr, _, _, olde, _, _, redirect}:
      GSec{id, secret, G.Str.or(refresh, oldr), access, expiry, G.Str.or(email, olde), "", "", redirect}

def Gs.finish.tok(m: Maybe<&2, G.Tok>, +home: String, s: GSec, r: GResp, +e: GEnds) -> IO(String):
  match m:
    case None{}:
      IO.pure(String, Gc.fail(r))
    case Some{G.Tok{+access, expiry, refresh, +email}}:
      do IO<String>:
        em : String <- Gs.email(Bool.not(String.is_empty(email)), Gs.dir(home), e, email, access)
        +s2 : GSec = Gs.finish.keep(s, access, expiry, refresh, em)
        Gs.finish.done(Gs.linked(s2), home, s2)

def Gs.finish.x(ok: Bool, +home: String, s: GSec, +code: String, +verifier: String, +redirect: String) -> IO(String):
  match ok s:
    case False{} _:
      IO.pure(String, "error: this sign-in was not started here or has expired; start again, or paste this page's address into Google's settings on the machine where you pressed Connect")
    case True{} GSec{+id, +secret, a, b, c, d, f, g, h}:
      do IO<String>:
        +e : GEnds <- Ge.get()
        +r : GResp <- Gc.req(Gs.dir(home), Ge.token(e), Gh.form(), G.Google.code_body(id, secret, code, verifier, redirect))
        t : U32 <- H.Clock.epoch()
        Gs.finish.tok(G.Google.tokens(Gc.bodyof(r), U32.to_nat(t)), home, GSec{id, secret, a, b, c, d, f, g, h}, r, e)

def Gs.finish.k(m: Maybe<&2, G.CodeState>, +err: String, +home: String, s: GSec, +redirect: String) -> IO(String):
  match m s:
    case None{} _:
      IO.pure(String, Bool.pick(String, String.is_empty(err), "error: no authorization code in that text", "error: Google said " ++ err))
    case Some{G.CodeState{+code, +state, bare}} GSec{a, b, c, d, f, g, +verifier, +pstate, +predir}:
      Gs.finish.x(Bool.and(Bool.not(String.is_empty(verifier)), Bool.or(bare, String.eq(state, pstate))), home,
        GSec{a, b, c, d, f, g, verifier, pstate, predir}, code, verifier, G.Str.or(predir, redirect))

# ends a sign-in: text is the redirected URL (or its path and query, as
# the hub's /oauth/google route sees it, or the bare code pasted). Checks
# the state, trades the code for tokens, stores them, and answers the
# status JSON, or "error: ...".
def Google.finish(+home: String, +text: String, +redirect: String) -> IO(String):
  do IO<String>:
    s : GSec <- Gs.load(home)
    Gs.finish.k(G.Google.code_of(text), G.Google.error_of(text), home, s, redirect)

# Access tokens
# -------------

type GAccess is Data:
  GOk{token: String}
  GErr{msg: String}

def Gs.fresh(s: GSec, +now: Nat) -> Bool:
  match s:
    case GSec{_, _, _, access, expiry, _, _, _, _}:
      Bool.and(Bool.not(String.is_empty(access)), G.Google.fresh(now, expiry))

def Gs.access(s: GSec) -> String:
  match s:
    case GSec{_, _, _, access, _, _, _, _, _}:
      access

def Gs.refreshed(m: Maybe<&2, G.Tok>, +home: String, s: GSec, r: GResp) -> IO(GAccess):
  match m:
    case None{}:
      IO.pure(GAccess, GErr{Gc.fail(r) ++ " (could not refresh Google access; connect Google again in Settings if this persists)"})
    case Some{G.Tok{+access, expiry, refresh, email}}:
      do IO<GAccess>:
        Gs.save(home, Gs.finish.keep(s, access, expiry, refresh, email))
        return GOk{access}

def Gs.refresh(+home: String, s: GSec, +now: Nat) -> IO(GAccess):
  match s:
    case GSec{+id, +secret, +refresh, a, b, c, d, f, g}:
      do IO<GAccess>:
        e : GEnds <- Ge.get()
        +r : GResp <- Gc.req(Gs.dir(home), Ge.token(e), Gh.form(), G.Google.refresh_body(id, secret, refresh))
        Gs.refreshed(G.Google.tokens(Gc.bodyof(r), now), home, GSec{id, secret, refresh, a, b, c, d, f, g}, r)

# why no account answered: none connected, or none with that address
def Gs.missing(+email: String, +accts: List<&2, G.Acct>) -> String:
  Bool.pick(String, List.is_empty(&2, G.Acct, accts), "error: Google is not connected (connect it in Settings)",
    "error: no connected Google account " ++ email ++ "; connected: " ++ String.join(G.Accts.emails(accts), ", "))

def Gs.token.k(fresh: Bool, linked: Bool, +home: String, s: GSec, +now: Nat, +why: String) -> IO(GAccess):
  match fresh linked:
    case True{} _:
      IO.pure(GAccess, GOk{Gs.access(s)})
    case False{} False{}:
      IO.pure(GAccess, GErr{why})
    case False{} True{}:
      Gs.refresh(home, s, now)

# an access token for the account with address email ("" is the first),
# good for a minute more, refreshed (and stored) when the one kept is not
def Google.token(+home: String, +email: String) -> IO(GAccess):
  do IO<GAccess>:
    +j : J.Json <- Gs.file(home)
    +accts : List<&2, G.Acct> = G.Accts.of(j)
    +s : GSec = Gs.with.acct(Gs.of(j), G.Accts.pick(accts, email))
    t : U32 <- H.Clock.epoch()
    +now : Nat = U32.to_nat(t)
    Gs.token.k(Gs.fresh(s, now), Gs.linked(s), home, s, now, Gs.missing(email, accts))

def Gs.revoke(none: Bool, +home: String, +e: GEnds, +refresh: String) -> IO(Unit):
  match none:
    case True{}:
      IO.pure(Unit, Unit{})
    case False{}:
      do IO<Unit>:
        r : GResp <- Gc.req(Gs.dir(home), Ge.revoke(e), Gh.form(), G.Url.pair("token", refresh))
        return Unit{}

def Gs.revoke.acct(a: G.Acct, +home: String, +e: GEnds) -> IO(Unit):
  match a:
    case G.Acct{_, +refresh, _, _}:
      Gs.revoke(String.is_empty(refresh), home, e, refresh)

def Gs.revoke.all(xs: List<&2, G.Acct>, +home: String, +e: GEnds) -> IO(Unit):
  match xs:
    case Nil{}:
      IO.pure(Unit, Unit{})
    case x <> t:
      IO.bind(Unit, Unit, Gs.revoke.acct(x, home, e), u => Gs.revoke.all(t, home, e))

def Gs.opt(m: Maybe<&2, G.Acct>) -> List<&2, G.Acct>:
  match m:
    case None{}:
      Nil{}
    case Some{a}:
      [a]

# the accounts a sign-out ends: every one for "", else the one named
def Gs.ending(all: Bool, +accts: List<&2, G.Acct>, +email: String) -> List<&2, G.Acct>:
  match all:
    case True{}:
      accts
    case False{}:
      Gs.opt(G.Accts.pick(accts, email))

def Gs.left(all: Bool, +accts: List<&2, G.Acct>, +email: String) -> List<&2, G.Acct>:
  match all:
    case True{}:
      Nil{}
    case False{}:
      G.Accts.drop(accts, email)

# signs out the account with address email, or every account for "":
# revokes each refresh token (best effort) and forgets its tokens; the
# client id and secret stay
def Google.disconnect(+home: String, +email: String) -> IO(String):
  do IO<String>:
    +j : J.Json <- Gs.file(home)
    +accts : List<&2, G.Acct> = G.Accts.of(j)
    +all : Bool = String.is_empty(email)
    e : GEnds <- Ge.get()
    Gs.revoke.all(Gs.ending(all, accts, email), home, e)
    Gs.write(home, Gs.of(j), Gs.left(all, accts, email))
    st : String <- Google.status(home)
    return st

# Tools
# -----

def Gt.clamp(+n: Nat, +hi: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_eq(n, 0n), 1n, Bool.pick(Nat, Nat.is_gt(n, hi), hi, n))

def Gt.urls(ids: List<&2, String>, +e: GEnds) -> List<&2, String>:
  match ids:
    case Nil{}:
      Nil{}
    case id <> t:
      Ge.api(e, G.Google.gmail_get_path(id, False{})) <> Gt.urls(t, e)

def Gt.parsed.put(m: Maybe<&2, J.Json>, +acc: List<&2, J.Json>) -> List<&2, J.Json>:
  match m:
    case Some{+j}:
      Bool.pick(List<&2, J.Json>, String.is_empty(J.Json.get_str(j, "id", "")), acc, j <> acc)
    case None{}:
      acc

def Gt.parsed(bs: List<&2, String>, acc: List<&2, J.Json>) -> List<&2, J.Json>:
  match bs:
    case Nil{}:
      List.reverse(&2, J.Json, acc)
    case b <> t:
      Gt.parsed(t, Gt.parsed.put(J.Json.parse(b), acc))

def Gt.metas(none: Bool, +ids: List<&2, String>, +tok: String, +dir: String, +e: GEnds) -> IO(String):
  match none:
    case True{}:
      IO.pure(String, "No messages.")
    case False{}:
      do IO<String>:
        bs : List<&2, String> <- Gc.many(dir, Gt.urls(ids, e), Gh.auth(tok))
        return G.Google.msg_list(Gt.parsed(bs, Nil{}))

def Gt.metas.ids(+ids: List<&2, String>, +tok: String, +dir: String, +e: GEnds) -> IO(String):
  Gt.metas(List.is_empty(&2, String, ids), ids, tok, dir, e)

def Gt.search.k(ok: Bool, r: GResp, +tok: String, +dir: String, +e: GEnds) -> IO(String):
  match ok r:
    case False{} r2:
      IO.pure(String, Gc.fail(r2))
    case True{} GResp{_, body, _}:
      Gt.metas.ids(G.Google.msg_ids(Gc.json(body)), tok, dir, e)

def Gc.okr(r: GResp) -> Bool:
  match r:
    case GResp{code, _, _}:
      Gc.ok(code)

# gmail_search {query, max}: the list, then each message's headers (one curl)
def Gt.search(+args: J.Json, +tok: String, +dir: String, +e: GEnds) -> IO(String):
  do IO<String>:
    +r : GResp <- Gc.req(dir, Ge.api(e, G.Google.gmail_search_path(J.Json.get_str(args, "query", ""), Gt.clamp(J.Json.get_nat(args, "max", 10n), 25n))), Gh.auth(tok), "")
    Gt.search.k(Gc.okr(r), r, tok, dir, e)

def Gt.need(missing: Bool, +what: String, act: IO(String)) -> IO(String):
  match missing:
    case True{}:
      IO.pure(String, "error: " ++ what ++ " is required")
    case False{}:
      act

# gmail_read {id}
def Gt.read(+args: J.Json, +tok: String, +dir: String, +e: GEnds) -> IO(String):
  +id = J.Json.get_str(args, "id", "")
  Gt.need(String.is_empty(String.trim(id)), "id",
    IO.bind(GResp, String, Gc.req(dir, Ge.api(e, G.Google.gmail_get_path(String.trim(id), True{})), Gh.auth(tok), ""),
      r => IO.pure(String, Gc.answer(r, j => G.Google.msg_full(j)))))

def Gt.sent(+j: J.Json) -> String:
  "Sent (message id " ++ J.Json.get_str(j, "id", "") ++ ", thread " ++ J.Json.get_str(j, "threadId", "") ++ ")."

# default window: from now (or the from given) for a week
def Gt.until(+from: String, +to: String, +now: Nat) -> String:
  Bool.pick(String, String.is_empty(to),
    Bool.pick(String, String.is_empty(from), G.Date.iso(Nat.add(now, 604800n)), G.Date.time(G.Date.plus_days(from, 7n))), to)

# calendar_events {from?, to?, max?, query?}
def Gt.events(+args: J.Json, +tok: String, +dir: String, +e: GEnds, +now: Nat) -> IO(String):
  +from = String.trim(J.Json.get_str(args, "from", ""))
  +to = String.trim(J.Json.get_str(args, "to", ""))
  IO.bind(GResp, String,
    Gc.req(dir, Ge.api(e, G.Google.cal_list_path(G.Str.or(from, G.Date.iso(now)), Gt.until(from, to, now),
      Gt.clamp(J.Json.get_nat(args, "max", 20n), 50n), J.Json.get_str(args, "query", ""))), Gh.auth(tok), ""),
    r => IO.pure(String, Gc.answer(r, j => G.Google.events(j))))

def Gt.strs(xs: List<&2, J.Json>, acc: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      List.reverse(&2, String, acc)
    case x <> t:
      Gt.strs(t, String.trim(J.Json.as_str(Some{x}, "")) <> acc)

def Gt.nonempty(xs: List<&2, String>, +acc: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      List.reverse(&2, String, acc)
    case +x <> t:
      Gt.nonempty(t, Bool.pick(List<&2, String>, String.is_empty(String.trim(x)), acc, String.trim(x) <> acc))

# attendees as a JSON array of emails, or one comma-separated string
def Gt.people(m: Maybe<&2, J.Json>) -> List<&2, String>:
  match m:
    case Some{J.Arr{xs}}:
      Gt.nonempty(Gt.strs(J.Json.to_list(xs), Nil{}), Nil{})
    case Some{J.Str{s}}:
      Gt.nonempty(String.split(s, ','), Nil{})
    case _:
      Nil{}

# calendar_create {summary, start, end, description?, attendees?, all_day?, timeZone?, location?, reminders?}
def Gt.create(+args: J.Json, +tok: String, +dir: String, +e: GEnds) -> IO(String):
  +start = String.trim(J.Json.get_str(args, "start", ""))
  +end = String.trim(J.Json.get_str(args, "end", ""))
  +all = J.Json.get_bool(args, "all_day", False{})
  +who = Gt.people(J.Json.get(args, "attendees"))
  Gt.need(Bool.or(String.is_empty(start), Bool.and(String.is_empty(end), Bool.not(all))), "start and end",
    IO.bind(GResp, String,
      Gc.req(dir, Ge.api(e, G.Google.cal_insert_path(Bool.not(List.is_empty(&2, String, who)))), Gh.json(tok),
        G.Google.event_create(G.Str.or(J.Json.get_str(args, "summary", ""), "(no title)"), args, start, end, who, all)),
      r => IO.pure(String, Gc.answer(r, j => G.Google.created(j)))))

# Calendar: update and delete (the hub has fetched the event, shown the
# person the change, and passes the arguments as shown: G.Cal.resolve)

# a missing or deleted event is a plain error, else the usual answer
def Gt.cal.ans(ok: Bool, gone: Bool, +id: String, r: GResp, f: J.Json -> String) -> String:
  match ok gone:
    case True{} _:
      Gc.answer(r, f)
    case False{} True{}:
      G.Cal.nosuch(id)
    case False{} False{}:
      Gc.fail(r)

def Gt.cal.done(+r: GResp, +id: String, f: J.Json -> String) -> String:
  match r:
    case GResp{+code, _, _}:
      Gt.cal.ans(Gc.ok(code), G.Cal.gone(code), id, r, f)

def Gt.cal.id.k(ok: Bool, +id: String, act: String -> IO(String)) -> IO(String):
  match ok:
    case True{}:
      act(id)
    case False{}:
      IO.pure(String, "error: invalid event id")

# the id as given: required, with no '/' or whitespace
def Gt.cal.id(+args: J.Json, act: String -> IO(String)) -> IO(String):
  +id = String.trim(J.Json.get_str(args, "id", ""))
  Gt.need(String.is_empty(id), "id", Gt.cal.id.k(G.Cal.id_ok(id), id, act))

def Gt.update.k(+id: String, +args: J.Json, +tok: String, +dir: String, +e: GEnds) -> IO(String):
  IO.bind(GResp, String,
    Gc.reqm(dir, Ge.api(e, G.Google.cal_patch_path(id, G.Cal.told(args))), Gh.json(tok), J.Json.show(G.Google.event_patch(args)), "PATCH"),
    r => IO.pure(String, Gt.cal.done(r, id, j => G.Google.updated(j))))

# calendar_update {id, ...} (args as G.Cal.resolve made them)
def Gt.update(+args: J.Json, +tok: String, +dir: String, +e: GEnds) -> IO(String):
  Gt.cal.id(args, id => Gt.update.k(id, args, tok, dir, e))

def Gt.cal.said(+s: String) -> String:
  G.Str.or(s, "Deleted.")

def Gt.delete.ans(ok: Bool, gone: Bool, +id: String, r: GResp, +said: String) -> String:
  match ok gone:
    case True{} _:
      Gt.cal.said(said)
    case False{} True{}:
      G.Cal.nosuch(id)
    case False{} False{}:
      Gc.fail(r)

def Gt.delete.done(+r: GResp, +id: String, +said: String) -> String:
  match r:
    case GResp{+code, _, _}:
      Gt.delete.ans(Gc.ok(code), G.Cal.gone(code), id, r, said)

def Gt.delete.k(+id: String, +args: J.Json, +tok: String, +dir: String, +e: GEnds) -> IO(String):
  IO.bind(GResp, String,
    Gc.reqm(dir, Ge.api(e, G.Google.cal_delete_path(id, G.Cal.told(args))), Gh.auth(tok), "", "DELETE"),
    r => IO.pure(String, Gt.delete.done(r, id, J.Json.get_str(args, "_done", ""))))

# calendar_delete {id} (args as G.Cal.resolve.del made them)
def Gt.delete(+args: J.Json, +tok: String, +dir: String, +e: GEnds) -> IO(String):
  Gt.cal.id(args, id => Gt.delete.k(id, args, tok, dir, e))

# What the hub does before it asks the person: fetch the event and make
# the arguments and the words to approve (a JSON object {args, detail}),
# or an "error: ..." for the agent
def Gt.prep.ok(update: Bool, +old: J.Json, +args: J.Json) -> String:
  match update:
    case True{}:
      +res = G.Cal.resolve(old, args)
      J.Json.show(J.Json.obj([J.KV{"args", res}, J.KV{"detail", J.Str{G.Cal.acct(args) ++ G.Cal.change.detail(old, res)}}]))
    case False{}:
      J.Json.show(J.Json.obj([J.KV{"args", G.Cal.resolve.del(old, args)}, J.KV{"detail", J.Str{G.Cal.acct(args) ++ G.Cal.delete.detail(old)}}]))

def Gt.prep.k(ok: Bool, gone: Bool, update: Bool, r: GResp, +id: String, +args: J.Json) -> String:
  match ok gone:
    case True{} _:
      match r:
        case GResp{_, +body, _}:
          Gt.prep.ok(update, Gc.json(body), args)
    case False{} True{}:
      G.Cal.nosuch(id)
    case False{} False{}:
      Gc.fail(r)

def Gt.prep.gone(r: GResp) -> Bool:
  match r:
    case GResp{+code, _, _}:
      G.Cal.gone(code)

def Gt.prep.code(r: GResp, update: Bool, +id: String, +args: J.Json) -> String:
  match r:
    case GResp{+code, _, _}:
      Gt.prep.k(Gc.ok(code), G.Cal.gone(code), update, r, id, args)

def Gt.prep.get(update: Bool, +id: String, +args: J.Json, +tok: String, +dir: String, +e: GEnds) -> IO(String):
  IO.bind(GResp, String, Gc.req(dir, Ge.api(e, G.Google.cal_event_path(id)), Gh.auth(tok), ""),
    r => IO.pure(String, Gt.prep.code(r, update, id, args)))

def Gt.prep.chk(none: Bool, update: Bool, +id: String, +args: J.Json, +tok: String, +dir: String, +e: GEnds) -> IO(String):
  match none:
    case True{}:
      IO.pure(String, "error: nothing to change")
    case False{}:
      Gt.prep.get(update, id, args, tok, dir, e)

def Gt.prep(+update: Bool, +args: J.Json, +tok: String, +dir: String, +e: GEnds) -> IO(String):
  Gt.cal.id(args, id => Gt.prep.chk(Bool.and(update, G.Cal.nothing(G.Google.event_patch(args))), update, id, args, tok, dir, e))

# Mail we send
# ------------
# gmail_send and gmail_draft are prepared off the hub's loop (Gm.run): the
# account is resolved, the reply's original or the draft read, the
# attachments probed by a shell step (their bytes never enter Bend), the
# whole message written to a folder under secrets (0700) and shown to the
# person as text. After they approve, Gm.send uploads exactly that file.

type GPrep is Data:
  GFail{msg: String}
  GAsk{detail: String, plan: String}
  GDone{msg: String}

# the account, the token for it, the tool and its arguments
type GJob is Data:
  GJob{email: String, tok: String, home: String, tool: String, args: J.Json}

# the message as the person sees it: to, cc, bcc, subject, html, the
# signed body, In-Reply-To, References, the thread it joins, the line for
# "In reply to", the attachments' paths
type GMsg is Data:
  GMsg{to: String, cc: String, bcc: String, subject: String, html: Bool, body: String, irt: String, refs: String, thread: String, inreply: String, paths: List<&2, String>}

# the original a reply answers
type GOrigR is Data:
  GNoReply{}
  GReply{o: G.Gorig}
  GOrigErr{msg: String}

def GJob.email(j: GJob) -> String:
  match j:
    case GJob{e, _, _, _, _}:
      e

def GJob.tok(j: GJob) -> String:
  match j:
    case GJob{_, t, _, _, _}:
      t

def GJob.home(j: GJob) -> String:
  match j:
    case GJob{_, _, h, _, _}:
      h

def GJob.tool(j: GJob) -> String:
  match j:
    case GJob{_, _, _, t, _}:
      t

def GJob.args(j: GJob) -> J.Json:
  match j:
    case GJob{_, _, _, _, a}:
      a

def GMsg.to(m: GMsg) -> String:
  match m:
    case GMsg{x, _, _, _, _, _, _, _, _, _, _}:
      x

def GMsg.cc(m: GMsg) -> String:
  match m:
    case GMsg{_, x, _, _, _, _, _, _, _, _, _}:
      x

def GMsg.bcc(m: GMsg) -> String:
  match m:
    case GMsg{_, _, x, _, _, _, _, _, _, _, _}:
      x

def GMsg.subject(m: GMsg) -> String:
  match m:
    case GMsg{_, _, _, x, _, _, _, _, _, _, _}:
      x

def GMsg.html(m: GMsg) -> Bool:
  match m:
    case GMsg{_, _, _, _, x, _, _, _, _, _, _}:
      x

def GMsg.body(m: GMsg) -> String:
  match m:
    case GMsg{_, _, _, _, _, x, _, _, _, _, _}:
      x

def GMsg.irt(m: GMsg) -> String:
  match m:
    case GMsg{_, _, _, _, _, _, x, _, _, _, _}:
      x

def GMsg.refs(m: GMsg) -> String:
  match m:
    case GMsg{_, _, _, _, _, _, _, x, _, _, _}:
      x

def GMsg.thread(m: GMsg) -> String:
  match m:
    case GMsg{_, _, _, _, _, _, _, _, x, _, _}:
      x

def GMsg.inreply(m: GMsg) -> String:
  match m:
    case GMsg{_, _, _, _, _, _, _, _, _, x, _}:
      x

def GMsg.paths(m: GMsg) -> List<&2, String>:
  match m:
    case GMsg{_, _, _, _, _, _, _, _, _, _, x}:
      x

def Gm.probe.sh() -> String:
  "for f in \"$@\"; do if [ -f \"$f\" ]; then if [ -r \"$f\" ]; then printf 'ok %s\\n' \"$(wc -c < \"$f\" | tr -d ' ')\"; else echo unreadable; fi; else echo missing; fi; done"

# $1: the file to make; then pairs: t FILE (a text piece) or f FILE (a file's base64, lines of 76, CRLF)
def Gm.assemble.sh() -> String:
  "out=$1; shift; : > \"$out\" || exit 1; while [ $# -ge 2 ]; do case $1 in t) cat \"$2\" >> \"$out\" || exit 1;; f) base64 < \"$2\" | tr -d '\\n' | fold -w 76 | awk '{printf \"%s\\r\\n\", $0}' >> \"$out\" || exit 1;; esac; shift 2; done"

def Gm.dir(+home: String, +nonce: String) -> String:
  Gs.dir(home) ++ "/gm-" ++ nonce

def Gm.clean.k(ok: Bool, +dir: String) -> IO(Unit):
  match ok:
    case True{}:
      IO.bind(P.Ran, Unit, P.Proc.run("rm", ["-rf", dir], ""), r => IO.pure(Unit, Unit{}))
    case False{}:
      IO.pure(Unit, Unit{})

# a prepared message's folder removed (only one of ours)
def Gm.clean(+home: String, +dir: String) -> IO(Unit):
  Gm.clean.k(String.starts_with(dir, Gs.dir(home) ++ "/gm-"), dir)

# the text before each attachment written to a file; the arguments for the
# assembling script, reversed
def Gm.att_files(hs: List<&2, String>, ps: List<&2, String>, +dir: String, +i: Nat, acc: List<&2, String>) -> IO(List<&2, String>):
  match hs ps:
    case +h <> ht +p <> pt:
      do IO<List<&2, String>>:
        +f : String = dir ++ "/a" ++ Nat.show(i)
        Gf.write(f, h)
        Gm.att_files(ht, pt, dir, Nat.add(i, 1n), p <> "f" <> f <> "t" <> acc)
    case _ _:
      IO.pure(List<&2, String>, acc)

def Gm.built(r: P.Ran) -> Bool:
  match r:
    case P.Ran{0, _}:
      True{}
    case P.Ran{_, _}:
      False{}

# the upload body (a multipart/related request: the JSON part, then the
# message) written to out
def Gm.build(+dir: String, +out: String, +parts: G.GParts, +json: String, +bnd: String, ps: List<&2, String>) -> IO(Bool):
  do IO<Bool>:
    Gf.write(dir ++ "/p0", G.Gmail.upload_head(bnd, json) ++ G.GParts.head(parts))
    Gf.write(dir ++ "/pz", G.GParts.tail(parts) ++ G.Gmail.upload_tail(bnd))
    acc : List<&2, String> <- Gm.att_files(G.GParts.atts(parts), ps, dir, 0n, Nil{})
    +args : List<&2, String> = List.append(&2, String, ["-c", Gm.assemble.sh(), "sh", out, "t", dir ++ "/p0"], List.append(&2, String, List.reverse(&2, String, acc), ["t", dir ++ "/pz"]))
    r : P.Ran <- P.Proc.run("sh", args, dir)
    return Gm.built(r)

# a request whose body is a file (and so may be big): the method, a longer deadline
def Gc.configf(+url: String, hs: List<&2, String>, +method: String, +body: String, +out: String, +secs: Nat) -> String:
  "url = " ++ Gc.q(url) ++ "\n" ++ Gc.headers(hs) ++ "request = " ++ Gc.q(method) ++ "\n" ++ "data-binary = " ++ Gc.q("@" ++ body) ++ "\n"
    ++ "max-time = " ++ Nat.show(secs) ++ "\n" ++ "output = " ++ Gc.q(out) ++ "\n" ++ "write-out = \"%{http_code}\"\n"

def Gc.reqfile(+dir: String, +url: String, hs: List<&2, String>, +method: String, +body: String, +secs: Nat) -> IO(GResp):
  do IO<GResp>:
    +name : String <- Gf.nonce(3n)
    +cfg : String = dir ++ "/g-" ++ name ++ ".curl"
    +out : String = dir ++ "/g-" ++ name ++ ".out"
    Gf.write(cfg, Gc.configf(url, hs, method, body, out, secs))
    r : P.Ran <- P.Proc.run("curl", ["-sS", "-K", cfg], dir)
    +said : String = Gc.ran(r)
    text : String <- Gf.text(out)
    Gf.rm(cfg)
    Gf.rm(out)
    return GResp{Gc.code(said), text, Gc.said(said)}

def Gm.argn(+args: J.Json, +k: String) -> String:
  String.trim(J.Json.get_str(args, k, ""))

# any argument that makes a message
def Gm.content(+args: J.Json) -> Bool:
  Bool.or(Bool.or(Bool.or(Bool.not(String.is_empty(Gm.argn(args, "to"))), Bool.not(String.is_empty(Gm.argn(args, "cc")))),
    Bool.or(Bool.not(String.is_empty(Gm.argn(args, "bcc"))), Bool.not(String.is_empty(Gm.argn(args, "subject"))))),
    Bool.or(Bool.or(Bool.not(String.is_empty(J.Json.get_str(args, "body", ""))), J.Json.get_bool(args, "html", False{})),
      Bool.or(Bool.or(Bool.not(String.is_empty(Gm.argn(args, "reply_to"))), Bool.not(String.is_empty(Gm.argn(args, "thread")))),
        Bool.not(List.is_empty(&2, String, G.Gmail.paths(J.Json.get(args, "attachments")))))))

def Gm.plan(+kind: String, +account: String, +dir: String, +bnd: String, +url: String, +draft: String) -> String:
  J.Json.show(J.Json.obj([J.KV{"kind", J.Str{kind}}, J.KV{"account", J.Str{account}}, J.KV{"dir", J.Str{dir}}, J.KV{"bnd", J.Str{bnd}},
    J.KV{"url", J.Str{url}}, J.KV{"draft", J.Str{draft}}]))

def Gm.upload.method(+draft: String) -> String:
  Bool.pick(String, String.is_empty(draft), "POST", "PUT")

def Gm.upload.path(+draft: String) -> String:
  Bool.pick(String, String.is_empty(draft), G.Gmail.upload_drafts(), G.Gmail.upload_draft(draft))

# the message saved as a draft: the upload at once, nobody is asked
def Gm.save(+job: GJob, +dir: String, +bnd: String, +draft: String) -> IO(GPrep):
  do IO<GPrep>:
    e : GEnds <- Ge.get()
    +r : GResp <- Gc.reqfile(Gs.dir(GJob.home(job)), Ge.api(e, Gm.upload.path(draft)), [("Authorization: Bearer " ++ GJob.tok(job)), G.Gmail.upload_type(bnd)], Gm.upload.method(draft), dir ++ "/upload", 120n)
    Gm.clean(GJob.home(job), dir)
    return GDone{Gc.answer(r, j => G.Gmail.draft_saved(j))}

def Gm.show(+job: GJob, +msg: GMsg, sizes: List<&2, Nat>) -> String:
  G.Gmail.review("", GJob.email(job), GJob.email(job), G.Mail.clean(GMsg.to(msg)), G.Mail.clean(GMsg.cc(msg)), G.Mail.clean(GMsg.bcc(msg)), G.Mail.clean(GMsg.subject(msg)),
    GMsg.inreply(msg), G.Gmail.att_line(G.Gmail.names(GMsg.paths(msg)), sizes), GMsg.html(msg), GMsg.body(msg))

def Gm.assemble.done(draft_tool: Bool, +job: GJob, +msg: GMsg, sizes: List<&2, Nat>, +dir: String, +bnd: String) -> IO(GPrep):
  match draft_tool:
    case True{}:
      Gm.save(job, dir, bnd, Gm.argn(GJob.args(job), "draft"))
    case False{}:
      IO.pure(GPrep, GAsk{Gm.show(job, msg, sizes), Gm.plan("send", GJob.email(job), dir, bnd, G.Gmail.upload_send(), "")})

def Gm.assemble.k(ok: Bool, +job: GJob, +msg: GMsg, sizes: List<&2, Nat>, +dir: String, +bnd: String) -> IO(GPrep):
  match ok:
    case False{}:
      IO.bind(Unit, GPrep, Gm.clean(GJob.home(job), dir), u => IO.pure(GPrep, GFail{"error: could not build the message (is `base64` available?)"}))
    case True{}:
      Gm.assemble.done(String.eq(GJob.tool(job), "gmail_draft"), job, msg, sizes, dir, bnd)

# the message on disk, a folder of its own
def Gm.assemble(+job: GJob, +msg: GMsg, sizes: List<&2, Nat>) -> IO(GPrep):
  do IO<GPrep>:
    +nonce : String <- Gf.nonce(5n)
    +dir : String = Gm.dir(GJob.home(job), nonce)
    mk : Result<&1, &1, U32 & String, Unit> <- H.FS.mkdirs(dir)
    cm : Result<&1, &1, U32 & String, Unit> <- H.FS.chmod(dir, 448)
    +bnd : String = "=_bpu_" ++ nonce
    +json : String = Bool.pick(String, String.eq(GJob.tool(job), "gmail_draft"), G.Gmail.json_draft(Gm.argn(GJob.args(job), "draft"), GMsg.thread(msg)), G.Gmail.json_send(GMsg.thread(msg)))
    ok : Bool <- Gm.build(dir, dir ++ "/upload", G.Gmail.compose(GJob.email(job), GMsg.to(msg), GMsg.cc(msg), GMsg.bcc(msg), GMsg.subject(msg), GMsg.irt(msg), GMsg.refs(msg), GMsg.html(msg), GMsg.body(msg), GMsg.paths(msg), nonce), json, bnd, GMsg.paths(msg))
    Gm.assemble.k(ok, job, msg, sizes, dir, bnd)

def Gm.probe.k(err: String, +job: GJob, +msg: GMsg, sizes: List<&2, Nat>) -> IO(GPrep):
  match err:
    case SNil{}:
      Gm.assemble(job, msg, sizes)
    case _:
      IO.pure(GPrep, GFail{"error: attachment: " ++ err})

def Gm.probe.go(none: Bool, job: GJob, msg: GMsg, +paths: List<&2, String>) -> IO(GPrep):
  match none:
    case True{}:
      Gm.assemble(job, msg, Nil{})
    case False{}:
      do IO<GPrep>:
        r : P.Ran <- P.Proc.run("sh", List.append(&2, String, ["-c", Gm.probe.sh(), "sh"], paths), "")
        +ls : List<&2, String> = String.lines(String.trim(Gc.ran(r)))
        Gm.probe.k(G.Gmail.att_err(paths, ls), job, msg, G.Gmail.att_sizes(ls))

# the attachments checked by the shell (they exist, can be read, fit), then the message
def Gm.probe(+job: GJob, +msg: GMsg) -> IO(GPrep):
  Gm.probe.go(List.is_empty(&2, String, GMsg.paths(msg)), job, msg, GMsg.paths(msg))

# what is wrong with the message before any file is looked at ("" for nothing)
def Gm.problem(+to: String, +body: String, +paths: List<&2, String>) -> String:
  Bool.pick(String, String.is_empty(to), "error: to is required (or reply_to / thread to reply)",
    Bool.pick(String, Bool.and(String.is_empty(body), List.is_empty(&2, String, paths)), "error: body is required", ""))

def Gm.check(err: String, job: GJob, msg: GMsg) -> IO(GPrep):
  match err:
    case SNil{}:
      Gm.probe(job, msg)
    case _:
      IO.pure(GPrep, GFail{err})

def Gm.msg.k(+args: J.Json, +reply: Bool, +o: G.Gorig, +sig: String) -> GMsg:
  +to = Bool.pick(String, String.is_empty(Gm.argn(args, "to")), Bool.pick(String, reply, G.Gmail.reply_to_addr(G.Gorig.replyto(o), G.Gorig.from(o)), ""), Gm.argn(args, "to"))
  +html = J.Json.get_bool(args, "html", False{})
  GMsg{to, Gm.argn(args, "cc"), Gm.argn(args, "bcc"),
    Bool.pick(String, reply, G.Gmail.reply_subject(J.Json.get_str(args, "subject", ""), G.Gorig.subject(o)), J.Json.get_str(args, "subject", "")),
    html, G.Gmail.signed(html, J.Json.get_str(args, "body", ""), sig),
    Bool.pick(String, reply, G.Gorig.msgid(o), ""), Bool.pick(String, reply, G.Gmail.references(G.Gorig.refs(o), G.Gorig.msgid(o)), ""),
    Bool.pick(String, reply, G.Gorig.thread(o), ""),
    Bool.pick(String, reply, G.Gmail.inreply(G.Gorig.subject(o), G.Gorig.from(o)), ""),
    G.Gmail.paths(J.Json.get(args, "attachments"))}

def Gm.msg(+job: GJob, reply: Bool, +o: G.Gorig, +sig: String) -> GMsg:
  Gm.msg.k(GJob.args(job), reply, o, sig)

def Gm.compose.chk(+job: GJob, +msg: GMsg) -> IO(GPrep):
  Gm.check(Gm.problem(GMsg.to(msg), J.Json.get_str(GJob.args(job), "body", ""), GMsg.paths(msg)), job, msg)

def Gm.compose.go(+job: GJob, reply: Bool, +o: G.Gorig, +sig: String) -> IO(GPrep):
  Gm.compose.chk(job, Gm.msg(job, reply, o, sig))

def Gm.compose.r(r: GOrigR, +job: GJob, +sig: String) -> IO(GPrep):
  match r:
    case GNoReply{}:
      Gm.compose.go(job, False{}, G.Gorig{"", "", "", "", "", "", ""}, sig)
    case GReply{+o}:
      Gm.compose.go(job, True{}, o, sig)
    case GOrigErr{m}:
      IO.pure(GPrep, GFail{m})

def Gm.orig.of(+o: G.Gorig) -> GOrigR:
  Bool.pick(GOrigR, String.is_empty(G.Gorig.msgid(o)), GOrigErr{"error: the message to reply to has no Message-ID"}, GReply{o})

def Gm.orig.kk(ok: Bool, r: GResp, thread: Bool) -> GOrigR:
  match ok r:
    case False{} r2:
      GOrigErr{"error: could not read the message to reply to: " ++ Gc.fail(r2)}
    case True{} GResp{_, +body, _}:
      Gm.orig.of(Bool.pick(G.Gorig, thread, G.Gmail.orig_thread(Gc.json(body)), G.Gmail.orig(Gc.json(body))))

def Gm.orig.k(+r: GResp, thread: Bool) -> GOrigR:
  Gm.orig.kk(Gc.okr(r), r, thread)

def Gm.orig.get(no_msg: Bool, no_thread: Bool, +tok: String, +dir: String, +e: GEnds, +id: String, +th: String) -> IO(GOrigR):
  match no_msg no_thread:
    case True{} True{}:
      IO.pure(GOrigR, GNoReply{})
    case True{} False{}:
      IO.bind(GResp, GOrigR, Gc.req(dir, Ge.api(e, G.Gmail.thread_path(th)), Gh.auth(tok), ""), r => IO.pure(GOrigR, Gm.orig.k(r, True{})))
    case False{} _:
      IO.bind(GResp, GOrigR, Gc.req(dir, Ge.api(e, G.Gmail.msg_path(id)), Gh.auth(tok), ""), r => IO.pure(GOrigR, Gm.orig.k(r, False{})))

# the message (reply_to) or conversation (thread) a reply answers
def Gm.orig(+job: GJob, +e: GEnds) -> IO(GOrigR):
  Gm.orig.get(String.is_empty(Gm.argn(GJob.args(job), "reply_to")), String.is_empty(Gm.argn(GJob.args(job), "thread")), GJob.tok(job), Gs.dir(GJob.home(job)), e,
    Gm.argn(GJob.args(job), "reply_to"), Gm.argn(GJob.args(job), "thread"))

def Gm.compose(+job: GJob, +sigs: J.Json) -> IO(GPrep):
  do IO<GPrep>:
    e : GEnds <- Ge.get()
    r : GOrigR <- Gm.orig(job, e)
    Gm.compose.r(r, job, J.Json.get_str(sigs, "gmail.sig." ++ GJob.email(job), ""))

def Gm.draft_send.k(ok: Bool, r: GResp, +email: String, +draft: String) -> IO(GPrep):
  match ok r:
    case False{} r2:
      IO.pure(GPrep, GFail{"error: could not read the draft: " ++ Gc.fail(r2)})
    case True{} GResp{_, +body, _}:
      IO.pure(GPrep, GAsk{G.Gmail.draft_review(email, Gc.json(body)), Gm.plan("draft_send", email, "", "", J.Json.get_str(J.Json.get_obj(Gc.json(body), "message"), "id", ""), draft)})

# gmail_send {draft}: the draft read, to be shown
def Gm.draft_send(+job: GJob) -> IO(GPrep):
  match job:
    case GJob{+email, +tok, +home, _, +args}:
      do IO<GPrep>:
        e : GEnds <- Ge.get()
        +r : GResp <- Gc.req(Gs.dir(home), Ge.api(e, G.Gmail.draft_path(Gm.argn(args, "draft"))), Gh.auth(tok), "")
        Gm.draft_send.k(Gc.okr(r), r, email, Gm.argn(args, "draft"))

def Gm.run.go(a: GAccess, draft_send: Bool, +email: String, +home: String, +tool: String, +args: J.Json, +sigs: J.Json) -> IO(GPrep):
  match a draft_send:
    case GErr{m} _:
      IO.pure(GPrep, GFail{m})
    case GOk{+tok} True{}:
      Gm.draft_send(GJob{email, tok, home, tool, args})
    case GOk{+tok} False{}:
      Gm.compose(GJob{email, tok, home, tool, args}, sigs)

def Gm.run.acct(m: Maybe<&2, G.Acct>, +accts: List<&2, G.Acct>, +home: String, +tool: String, +args: J.Json, +sigs: J.Json) -> IO(GPrep):
  match m:
    case None{}:
      IO.pure(GPrep, GFail{Gs.missing(Gm.argn(args, "account"), accts)})
    case Some{+a}:
      do IO<GPrep>:
        +email : String = G.Acct.email(a)
        t : GAccess <- Google.token(home, email)
        Gm.run.go(t, Bool.and(String.eq(tool, "gmail_send"), Bool.not(String.is_empty(Gm.argn(args, "draft")))), email, home, tool, args, sigs)

def Gm.run.k(clash: Bool, +home: String, +tool: String, +args: J.Json, +sigs: J.Json) -> IO(GPrep):
  match clash:
    case True{}:
      IO.pure(GPrep, GFail{"error: send a draft as it is, or update it with gmail_draft first"})
    case False{}:
      do IO<GPrep>:
        +accts : List<&2, G.Acct> <- Gs.accts(home)
        Gm.run.acct(G.Accts.pick(accts, Gm.argn(args, "account")), accts, home, tool, args, sigs)

# gmail_send / gmail_draft prepared. sigs: the hub's settings gmail.sig.<address> as a JSON object
def Gm.run(+home: String, +tool: String, +args: J.Json, +sigs: J.Json) -> IO(GPrep):
  Gm.run.k(Bool.and(Bool.and(String.eq(tool, "gmail_send"), Bool.not(String.is_empty(Gm.argn(args, "draft")))), Gm.content(args)), home, tool, args, sigs)

def Gm.send.req(draft_send: Bool, +sdir: String, +e: GEnds, +tok: String, +dir: String, +bnd: String, +url: String, +draft: String) -> IO(GResp):
  match draft_send:
    case True{}:
      Gc.req(sdir, Ge.api(e, G.Gmail.draft_send_path()), Gh.json(tok), J.Json.show(J.Json.obj([J.KV{"id", J.Str{draft}}])))
    case False{}:
      Gc.reqfile(sdir, Ge.api(e, url), [("Authorization: Bearer " ++ tok), G.Gmail.upload_type(bnd)], "POST", dir ++ "/upload", 120n)

def Gm.send.go(same: Bool, draft_send: Bool, +home: String, +e: GEnds, +tok: String, +dir: String, +bnd: String, +url: String, +draft: String) -> IO(String):
  match same:
    case False{}:
      IO.pure(String, "error: the draft changed after the person reviewed it; ask to send it again")
    case True{}:
      IO.bind(GResp, String, Gm.send.req(draft_send, Gs.dir(home), e, tok, dir, bnd, url, draft), r => IO.pure(String, Gc.answer(r, j => Gt.sent(j))))

# the draft's message id now ("" when it cannot be read)
def Gm.draft.mid(draft_send: Bool, +home: String, +e: GEnds, +tok: String, +draft: String) -> IO(String):
  match draft_send:
    case False{}:
      IO.pure(String, "")
    case True{}:
      IO.bind(GResp, String, Gc.req(Gs.dir(home), Ge.api(e, G.Gmail.draft_path(draft)), Gh.auth(tok), ""),
        r => IO.pure(String, J.Json.get_str(J.Json.get_obj(Gc.json(Gc.bodyof(r)), "message"), "id", "")))

# a draft is sent only as it was reviewed: updating a draft gives it a new
# message id, which the plan (url) kept
def Gm.send.tok(a: GAccess, +home: String, +kind: String, +dir: String, +bnd: String, +url: String, +draft: String) -> IO(String):
  match a:
    case GErr{m}:
      IO.pure(String, m)
    case GOk{+tok}:
      do IO<String>:
        +e : GEnds <- Ge.get()
        +ds : Bool = String.eq(kind, "draft_send")
        +mid : String <- Gm.draft.mid(ds, home, e, tok, draft)
        Gm.send.go(Bool.or(Bool.not(ds), String.eq(mid, url)), ds, home, e, tok, dir, bnd, url, draft)

# the approved message sent (the plan Gm.run made); its folder removed either way
def Gm.send(+home: String, +plan: String) -> IO(String):
  +j = Gc.json(plan)
  do IO<String>:
    a : GAccess <- Google.token(home, J.Json.get_str(j, "account", ""))
    +r : String <- Gm.send.tok(a, home, J.Json.get_str(j, "kind", ""), J.Json.get_str(j, "dir", ""), J.Json.get_str(j, "bnd", ""), J.Json.get_str(j, "url", ""), J.Json.get_str(j, "draft", ""))
    Gm.clean(home, J.Json.get_str(j, "dir", ""))
    return r

# a message the person declined: its folder removed
def Gm.drop(+home: String, +plan: String) -> IO(Unit):
  Gm.clean(home, J.Json.get_str(Gc.json(plan), "dir", ""))

def Gt.id(+tool: String) -> Nat:
  Bool.pick(Nat, String.eq(tool, "gmail_search"), 1n,
    Bool.pick(Nat, String.eq(tool, "gmail_read"), 2n,
      Bool.pick(Nat, String.eq(tool, "gmail_send"), 3n,
        Bool.pick(Nat, String.eq(tool, "calendar_events"), 4n,
          Bool.pick(Nat, String.eq(tool, "calendar_create"), 5n,
            Bool.pick(Nat, String.eq(tool, "calendar_update"), 6n, Bool.pick(Nat, String.eq(tool, "calendar_delete"), 7n, 0n)))))))

def Gt.run(n: Nat, +tool: String, +args: J.Json, +tok: String, +dir: String, +e: GEnds, +now: Nat) -> IO(String):
  match n:
    case 1n:
      Gt.search(args, tok, dir, e)
    case 2n:
      Gt.read(args, tok, dir, e)
    case 4n:
      Gt.events(args, tok, dir, e, now)
    case 5n:
      Gt.create(args, tok, dir, e)
    case 6n:
      Gt.update(args, tok, dir, e)
    case 7n:
      Gt.delete(args, tok, dir, e)
    case _:
      IO.pure(String, "error: no Google tool named " ++ tool)

def Gs.call.k(a: GAccess, +home: String, +tool: String, +args: J.Json) -> IO(String):
  match a:
    case GErr{m}:
      IO.pure(String, m)
    case GOk{+tok}:
      do IO<String>:
        +e : GEnds <- Ge.get()
        t : U32 <- H.Clock.epoch()
        Gt.run(Gt.id(tool), tool, args, tok, Gs.dir(home), e, U32.to_nat(t))

def Google.call.k(list: Bool, +home: String, +tool: String, +args: J.Json) -> IO(String):
  match list:
    case True{}:
      IO.bind(List<&2, G.Acct>, String, Gs.accts(home), xs => IO.pure(String, G.Accts.text(G.Accts.emails(xs))))
    case False{}:
      do IO<String>:
        a : GAccess <- Google.token(home, String.trim(J.Json.get_str(args, "account", "")))
        Gs.call.k(a, home, tool, args)

def Gs.prep.k(a: GAccess, +home: String, update: Bool, +args: J.Json) -> IO(String):
  match a:
    case GErr{m}:
      IO.pure(String, m)
    case GOk{+tok}:
      do IO<String>:
        +e : GEnds <- Ge.get()
        Gt.prep(update, args, tok, Gs.dir(home), e)

# calendar_update / calendar_delete before the person is asked: the event
# fetched, the arguments settled (G.Cal.resolve) and the words to approve,
# as JSON {args, detail}; or "error: ..." for the agent
def Google.prepare(+home: String, update: Bool, +args: J.Json) -> IO(String):
  do IO<String>:
    a : GAccess <- Google.token(home, String.trim(J.Json.get_str(args, "account", "")))
    Gs.prep.k(a, home, update, args)

# one MCP tool call (gmail_search, gmail_read, gmail_send, calendar_events,
# calendar_create, google_accounts) as the account its "account" names
# (default the first): compact text for the agent, or "error: ...". The
# hub asks the user before gmail_send and calendar_create (G.Google.asks).
def Google.call(+home: String, +tool: String, +args: J.Json) -> IO(String):
  Google.call.k(String.eq(tool, "google_accounts"), home, tool, args)

def Gs.say.k(obj: Bool, +text: String, +sh: G.Client) -> String:
  match obj:
    case False{}:
      text
    case True{}:
      +j = Gc.json(text)
      G.Google.note(J.Json.get_bool(j, "connected", False{}), J.Json.get_bool(j, "pending", False{}),
        Bool.or(J.Json.get_bool(j, "configured", False{}), G.Client.has(sh)), J.Json.get_str(j, "email", ""),
        G.Google.is_shared(J.Json.get_str(j, "client", ""), G.Client.id(sh)))

# a status answer as the line the Settings screens show; anything else
# (a sign-in URL, an error) as it is
def Google.say(+text: String) -> IO(String):
  IO.bind(G.Client, String, Gs.shared(), sh => IO.pure(String, Gs.say.k(String.starts_with(text, "{"), text, sh)))

# the MCP tool definitions, a JSON array shaped as tools/list's
def Google.tools() -> J.Json:
  J.Json.arr(G.Google.tool_defs())

# the state of the sign-in in flight here ("" for none)
def Google.pending(+home: String) -> IO(String):
  IO.bind(GSec, String, Gs.load(home), s => IO.pure(String, Gs.pstate(s)))
