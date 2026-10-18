import Base
import ../server/server.bend as S
import ../server/proc.bend as P
import ../server/botnet.bend as BN
import ../server/farview.bend as FVS
import ../core/json.bend as J
import ../core/fview.bend as FV
import ../core/rview.bend as RV
import ../core/winnote.bend as NT

# The window on another machine's hub (core/rview.bend): the files its
# viewers draw come from that hub's /view/* (read only, let in as the
# window's own connection is) into <cache>/remote/<hub>, and the viewers
# read them there as they read a folder here. One round is one long poll:
# the hub answers when its list of the thread's files changes (or after a
# while), the files that differ come over four at a time, and the answer
# says whether the viewers should read again. Whether to go on is the
# window's to say (main.bend's Watch.follow), so nothing runs for a viewer
# that is shut.

type Got is Data:
  Got{ok: Bool, ver: String, reread: Bool, why: String}

# <cache>/remote/<hub> for the hub at url ("" for this machine's own)
def Rvw.pre(+url: String) -> IO(String):
  do IO<String>:
    +home : String <- S.Env.get("HOME", "/tmp")
    xdg : String <- S.Env.get("XDG_CACHE_HOME", home ++ "/.cache")
    return Bool.pick(String, String.is_empty(url), "", RV.Rview.pre(xdg ++ "/backplane-bend", url))

def Rvw.fail(+why: String) -> Got:
  Got{False{}, "", False{}, why}

# a GET of the hub's, no redirects; the body is written to `out` as it came
# when that is set (a file's bytes are not text), else it is Resp's
def Rvw.get.k(ok: Bool, +url: String, +path: String, +secs: Nat, +max: Nat, +out: String) -> IO(BN.Resp):
  match ok:
    case True{}:
      IO.bind(P.Ran, BN.Resp, BN.Bn.curl(List.concat(&2, String, [["-sS", "--proto", "=http,https", "-m", Nat.show(secs), "--max-filesize", Nat.show(max)], BN.Bn.out(out),
        ["-w", "\n%{http_code}", "--url", BN.Bn.url.base(url) ++ path]]), ""), r => IO.pure(BN.Resp, BN.Bn.resp(r)))
    case False{}:
      IO.pure(BN.Resp, BN.Resp{0n, "not an http address"})

def Rvw.get(+url: String, +path: String, +secs: Nat, +max: Nat, +out: String) -> IO(BN.Resp):
  Rvw.get.k(BN.Bn.url.ok(url), url, path, secs, max, out)

# a thread's own folder is the hub's to name; one it mirrors is the cache
# folder far.view told the client of (`root`)
def Rvw.q(+thread: String, +root: String, +rel: String) -> String:
  "?thread=" ++ RV.Rview.enc(thread) ++ "&root=" ++ RV.Rview.enc(root) ++ "&path=" ++ RV.Rview.enc(rel)

# one file into the cache's own temporary name; "rel<TAB>tmp" when it came
def Rvw.pull(+url: String, +thread: String, +root: String, +rel: String, +cache: String) -> IO(String):
  do IO<String>:
    +n : String <- BN.Bn.random(4n)
    +tmp : String = cache ++ "/.farpart-" ++ n
    r : BN.Resp <- Rvw.get(url, "/view/get" ++ Rvw.q(thread, root, rel), 120n, Nat.add(FV.Fv.cap(), 1048576n), tmp)
    FVS.Fvs.pull.kept(Nat.is_eq(FVS.Fvs.code(r), 200n), rel, tmp)

def Rvw.lane(ls: List<&2, String>, +url: String, +thread: String, +root: String, +cache: String) -> IO(List<&2, String>):
  match ls:
    case Nil{}:
      IO.pure(List<&2, String>, Nil{})
    case +l <> t:
      do IO<List<&2, String>>:
        +p : String <- Rvw.pull(url, thread, root, FVS.Fvs.rel(l), cache)
        rest : List<&2, String> <- Rvw.lane(t, url, thread, root, cache)
        return FVS.Fvs.pair.k(String.is_empty(p), p, rest)

def Rvw.lane.job(ls: List<&2, String>, +url: String, +thread: String, +root: String, +cache: String, +out: Chan(List<&2, String>)) -> IO(Unit):
  do IO<Unit>:
    +r : List<&2, String> <- Rvw.lane(ls, url, thread, root, cache)
    ok : Result<&1, &1, List<&2, String>, Unit> <- Chan.send(List<&2, String>, out, r)
    return Unit{}

# the files in four lanes at once; the lines that came
def Rvw.fetch(+todo: List<&2, String>, +url: String, +thread: String, +root: String, +cache: String) -> IO(List<&2, String>):
  do IO<List<&2, String>>:
    +out : Chan(List<&2, String>) <- Chan.new(List<&2, String>, 4)
    IO.spawn(Unit, Rvw.lane.job(FVS.Fvs.pick(todo, 1n, 0n), url, thread, root, cache, out))
    IO.spawn(Unit, Rvw.lane.job(FVS.Fvs.pick(todo, 2n, 0n), url, thread, root, cache, out))
    IO.spawn(Unit, Rvw.lane.job(FVS.Fvs.pick(todo, 3n, 0n), url, thread, root, cache, out))
    +a : List<&2, String> <- Rvw.lane(FVS.Fvs.pick(todo, 0n, 0n), url, thread, root, cache)
    b : Maybe<&1, List<&2, String>> <- Chan.recv(List<&2, String>, out)
    c : Maybe<&1, List<&2, String>> <- Chan.recv(List<&2, String>, out)
    d : Maybe<&1, List<&2, String>> <- Chan.recv(List<&2, String>, out)
    return List.append(&2, String, a, List.append(&2, String, FVS.Fvs.got(b), List.append(&2, String, FVS.Fvs.got(c), FVS.Fvs.got(d))))

# a list that differs from the copies: bring what changed, install it (one
# step at a time per folder, never rolling a file back), and say whether
# the viewers read again; a file that did not come asks for an answer with
# no digest, so the next round asks again
def Rvw.bring(+url: String, +thread: String, +root: String, +cache: String, +files: String, +ver: String, +have: String) -> IO(Got):
  do IO<Got>:
    +old : Map<&2, String> <- FVS.Fvs.idx(cache)
    +todo : List<&2, String> = FVS.Fvs.todo(FVS.Fvs.lines(files), old)
    +ticket : String <- FVS.Fvs.ticket(cache)
    +got : List<&2, String> <- Rvw.fetch(todo, url, thread, root, cache)
    now : String <- FVS.Fvs.install(cache, files, got, ticket)
    +failed : Nat = Nat.sub(List.length(&2, String, todo), List.length(&2, String, got))
    FVS.Fvs.sync.pause(failed)
    return Got{True{}, Bool.pick(String, Nat.is_eq(failed, 0n), ver, ""), Bool.not(String.eq(ver, have)), ""}

def Rvw.res.k(+cache: String, +ver: String, +files: String, +url: String, +thread: String, +root: String, +have: String) -> IO(Got):
  Bool.pick(IO(Got), String.is_empty(cache), IO.pure(Got, Rvw.fail(NT.Notice.make("keep", ""))),
    Bool.pick(IO(Got), Bool.and(String.eq(ver, have), Bool.not(String.is_empty(have))), IO.pure(Got, Got{True{}, ver, False{}, ""}),
      Rvw.bring(url, thread, root, cache, files, ver, have)))

def Rvw.res(+res: J.Json, +url: String, +pre: String, +thread: String, +root: String, +have: String) -> IO(Got):
  Rvw.res.k(RV.Rview.at(pre, J.Json.get_str(res, "root", "")), J.Json.get_str(res, "ver", ""), J.Json.get_str(res, "files", ""), url, thread, root, have)

def Rvw.body(m: Maybe<&2, J.Json>, +url: String, +pre: String, +thread: String, +root: String, +have: String) -> IO(Got):
  match m:
    case Some{+o}:
      Bool.pick(IO(Got), J.Json.get_bool(o, "ok", False{}), Rvw.res(J.Json.get_obj(o, "result"), url, pre, thread, root, have),
        IO.pure(Got, Rvw.fail(J.Json.get_str(o, "error", NT.Notice.make("notaken", "")))))
    case None{}:
      IO.pure(Got, Rvw.fail(NT.Notice.make("badreply", "")))

def Rvw.fail.of(+r: BN.Resp) -> Got:
  match r:
    case BN.Resp{+c, +b}:
      Rvw.fail(Bool.pick(String, Nat.is_eq(c, 0n), NT.Notice.make("away", ""), NT.Notice.make("refused", String.take(b, 120n))))

def Rvw.got(+r: BN.Resp, +url: String, +pre: String, +thread: String, +root: String, +have: String) -> IO(Got):
  Bool.pick(IO(Got), Nat.is_eq(FVS.Fvs.code(r), 200n), Rvw.body(J.Json.parse(FVS.Fvs.body(r)), url, pre, thread, root, have), IO.pure(Got, Rvw.fail.of(r)))

# One round: the hub's list of the thread's files, `want` the one more
# the viewer is on (from the thread's folder), `have` the digest of the
# last list held ("" for none). It answers at once when its digest is not
# `have`, else after the list changes or a while.
def Rvw.sync(+url: String, +pre: String, +thread: String, +root: String, +want: String, +have: String) -> IO(Got):
  IO.bind(BN.Resp, Got, Rvw.get(url, "/view/ver?thread=" ++ RV.Rview.enc(thread) ++ "&root=" ++ RV.Rview.enc(root) ++ "&want=" ++ RV.Rview.enc(want) ++ "&have=" ++ RV.Rview.enc(have), 40n, 1048576n, ""),
    r => Rvw.got(r, url, pre, thread, root, have))

# a file of a commit the viewers name (a history version): the bytes, [] when
# it cannot be read
def Rvw.git.done(+r: BN.Resp, +tmp: String) -> IO(List<&2, U32>):
  match r:
    case BN.Resp{+c, _}:
      do IO<List<&2, U32>>:
        +bs : List<&2, U32> <- Bool.pick(IO(List<&2, U32>), Nat.is_eq(c, 200n), BN.Bf.read(tmp), IO.pure(List<&2, U32>, Nil{}))
        BN.Bf.rm(tmp)
        return bs

def Rvw.git(+url: String, +pre: String, +thread: String, +root: String, +path: String) -> IO(List<&2, U32>):
  do IO<List<&2, U32>>:
    mk : P.Ran <- P.Proc.run("mkdir", ["-p", pre], "/")
    +n : String <- BN.Bn.random(4n)
    +tmp : String = pre ++ "/.git-" ++ n
    r : BN.Resp <- Rvw.get(url, "/view/git" ++ Rvw.q(thread, root, path), 60n, Nat.add(FV.Fv.cap(), 1048576n), tmp)
    Rvw.git.done(r, tmp)
