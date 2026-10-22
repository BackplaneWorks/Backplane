import Base
import ./check.bend as C
import ../src/core/storedb.bend as S
import ../src/core/model.bend as M
import ../src/core/json.bend as J
import ../src/core/codec.bend as CD
import ../src/core/utf8.bend as U
import ../src/server/db.bend as D
import ../src/server/stobuild.bend as B

# The store's streamed first build (src/server/stobuild.bend), under JS:
# a log written to a fresh mktemp directory (the demo's changes, a torn
# line, a newer hub's line, a long line, a trailing fragment), built with
# a tiny chunk (most lines outgrow it) and again with a big one; the
# tables must equal the pure store: facts as SDB.Mat.replay files them,
# records as SDB.Tab.dump of the fold, the counters Switch.count.

# the log's lines (without newlines), oldest first
def Log.tail() -> List<&2, M.Change>:
  [M.MessagePosted{"t2", "m9", M.User{}, String.repeat("ü", 150n) ++ " long line", 30n}, M.ProviderSet{"t2", "codex", "gpt", 31n},
   M.ActivityLogged{"t2", "u9", "tool", "Bash", "make", 32n}, M.ProviderSet{"t2", "claude", "opus", 33n}]

def Log.encode(cs: List<&2, M.Change>) -> List<&2, String>:
  match cs:
    case Nil{}:
      Nil{}
    case c <> t:
      J.Json.show(CD.Change.encode(c)) <> Log.encode(t)

def Log.lines() -> List<&2, String>:
  List.append(&2, String, Log.encode(S.Demo.changes()), List.append(&2, String, ["{torn", "{\"$\":\"FromTheFuture\",\"at\":5}"], Log.encode(Log.tail())))

# what the pure store reads of a line
def Log.ll.k(m: Maybe<&2, J.Json>, +len: Nat) -> S.LL:
  match m:
    case Some{+o}:
      S.LL{len, True{}, CD.Change.decode(o)}
    case None{}:
      S.LL{len, False{}, None{}}

def Log.lls(ls: List<&2, String>) -> List<&2, S.LL>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      Log.ll.k(J.Json.parse(l), List.length(&2, U32, U.Utf8.encode(l))) <> Log.lls(t)

def Log.changes(ls: List<&2, S.LL>) -> List<&2, M.Change>:
  match ls:
    case Nil{}:
      Nil{}
    case S.LL{_, _, Some{c}} <> t:
      c <> Log.changes(t)
    case S.LL{_, _, None{}} <> t:
      Log.changes(t)

def Mat.ord(m: S.Mat) -> Nat:
  match m:
    case S.Mat{_, _, _, _, _, o}:
      o

def first() -> String:
  Maybe.default(&2, String, List.head(&2, String, Log.lines()), "")

def mat() -> S.Mat:
  S.Mat.replay(Log.lls(Log.lines()), S.Mat.empty())

def fold() -> M.State:
  M.State.replay(Log.changes(Log.lls(Log.lines())), M.State.empty())

# A pure table as the database answers it: "#<digits>" bound as an integer
# reads back as the digits
# ---------------------------------------------------------------------

def Norm.num.go(s: String, ok: Bool) -> Bool:
  match s:
    case SNil{}:
      ok
    case SCon{Chr{+x}, t}:
      Norm.num.go(t, Bool.and(ok, Bool.and(U32.is_ge(x, 48), U32.is_le(x, 57))))

def Norm.col(+s: String) -> String:
  Bool.pick(String, Bool.and(String.starts_with(s, "#"), Bool.and(Bool.not(String.is_empty(String.drop(s, 1n))), Norm.num.go(String.drop(s, 1n), True{}))), String.drop(s, 1n), s)

def Norm.cols(xs: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      Norm.col(x) <> Norm.cols(t)

def Norm.row(+v: String) -> String:
  String.join(Norm.cols(String.split(v, '\u{1f}')), "\u{1f}")

def Norm.map(xs: List<&2, Sigma<&2, &2, String, _ => String>>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case (+k, +v) <> t:
      (Norm.row(k) ++ " => " ++ Norm.row(v)) <> Norm.map(t)

def Pure.of(m: Map<&2, String>) -> String:
  String.join(Norm.map(Map.to_list(&2, String, m)), "\n")

# the database's rows of some tables, as the same map
def Db.put(rs: List<&2, List<&2, String>>, +t: String, m: Map<&2, String>) -> Map<&2, String>:
  match rs:
    case Nil{}:
      m
    case +r <> rest:
      Db.put(rest, t, Map.set(&2, String, m, S.Tab.key(t, List.take(&2, String, r, S.Tab.keyn(t))), String.join(r, "\u{1f}")))

def Db.cols(+t: String) -> String:
  String.join(S.Sdb.cols(t), ", ")

def Db.tables(ts: List<&2, String>, +h: String, +wh: String, m: Map<&2, String>) -> IO(Map<&2, String>):
  match ts:
    case Nil{}:
      IO.pure(Map<&2, String>, m)
    case +t <> rest:
      do IO<Map<&2, String>>:
        rs : List<&2, List<&2, String>> <- D.Db.query(h, "SELECT " ++ Db.cols(t) ++ " FROM " ++ t ++ Bool.pick(String, String.eq(t, "obj"), wh, ""), [], 1000000)
        Db.tables(rest, h, wh, Db.put(rs, t, m))

def Db.show(m: Map<&2, String>) -> String:
  String.join(Norm.map(Map.to_list(&2, String, m)), "\n")

def facts() -> List<&2, String>:
  ["line", "entry", "term", "post", "mail"]

def recs() -> List<&2, String>:
  ["project", "thread", "task", "ask", "queue", "setting", "bot", "room", "routine", "conv", "mark", "obj"]

def one(rs: List<&2, List<&2, String>>) -> String:
  match rs:
    case (x <> _) <> _:
      x
    case _:
      "?"

def dir.of(r: Result<&1, &1, U32 & String, U32 & (String & String)>) -> IO(String):
  match r:
    case Done{(_, (out, _))}:
      IO.pure(String, String.trim(out))
    case Fail{(_, e)}:
      IO.die(String, 1, "stobuild_test: mktemp failed: " ++ e)

def Wf.done(r: File & Result<&1, &1, U32 & String, Unit>) -> IO(Unit):
  (f, _) = r
  File.close(f)

def Wf.opened(+text: String, r: Result<&1, &1, U32 & String, File>) -> IO(Unit):
  match r:
    case Done{f}:
      IO.bind(File & Result<&1, &1, U32 & String, Unit>, Unit, File.write(f, text), Wf.done)
    case Fail{(_, e)}:
      IO.die(Unit, 1, "stobuild_test: write: " ++ e)

def Wf.put(+path: String, +text: String) -> IO(Unit):
  IO.bind(Result<&1, &1, U32 & String, File>, Unit, File.open(path, "w"), r => Wf.opened(text, r))

def built(b: B.Built) -> String:
  match b:
    case B.Built{s, o, n}:
      Nat.show(s) ++ " " ++ Nat.show(o) ++ " " ++ Nat.show(n)

def bytes(+s: String) -> Nat:
  List.length(&2, U32, U.Utf8.encode(s))

def check(+name: String, +h: String) -> IO(Unit):
  do IO<Unit>:
    fs : Map<&2, String> <- Db.tables(facts(), h, "", Map.new(&2, String))
    C.Check.str(name ++ ": facts equal the pure store's", Db.show(fs), Pure.of(S.Mat.facts(mat())))
    rs : Map<&2, String> <- Db.tables(recs(), h, " WHERE domain <> 'count'", Map.new(&2, String))
    C.Check.str(name ++ ": records equal the fold's dump", Db.show(rs), Pure.of(S.Tab.dump(fold())))
    c1 : List<&2, List<&2, String>> <- D.Db.query(h, "SELECT json FROM obj WHERE domain = 'count' AND key = 't1'", [], 1)
    c2 : List<&2, List<&2, String>> <- D.Db.query(h, "SELECT json FROM obj WHERE domain = 'count' AND key = 't2'", [], 1)
    C.Check.str(name ++ ": counters are Switch.count", one(c1) ++ " " ++ one(c2), Nat.show(M.Switch.count(fold(), "t1")) ++ " " ++ Nat.show(M.Switch.count(fold(), "t2")))
    mt : List<&2, List<&2, String>> <- D.Db.query(h, "SELECT group_concat(k || '=' || v, ' ') FROM (SELECT k, v FROM meta WHERE k <> 'log_id' ORDER BY k)", [], 1)
    C.Check.str(name ++ ": meta", one(mt), "applied_off=" ++ Nat.show(bytes(String.join(Log.lines(), "\n") ++ "\n")) ++ " applied_seq=" ++ Nat.show(Nat.sub(List.length(&2, String, Log.lines()), 1n)) ++ " codec=" ++ Nat.show(S.Sdb.codec()) ++ " ord=" ++ Nat.show(Mat.ord(mat())) ++ " origin=1 schema=1")
    id : List<&2, List<&2, String>> <- D.Db.query(h, "SELECT v FROM meta WHERE k = 'log_id'", [], 1)
    C.Check.str(name ++ ": log id (Sto.log_id's form: SHA-1 of the first line)", one(id), B.Stb.sha.k(U.Utf8.encode(first())))
    cl : String <- D.Db.close(h)
    return Unit{}

def main() -> IO(Unit):
  do IO<Unit>:
    made : Result<&1, &1, U32 & String, U32 & (String & String)> <- Process.run("mktemp", ["-d"], "", 4096, 10000)
    +dir : String <- dir.of(made)
    Wf.put(dir ++ "/events.jsonl", String.join(Log.lines(), "\n") ++ "\n{\"$\":\"MessagePosted\",\"th")
    +b1 : B.Built <- B.Stb.build(dir, 64)
    C.Check.str("tiny chunks: lines, offset, entries", built(b1), Nat.show(Nat.sub(List.length(&2, String, Log.lines()), 1n)) ++ " " ++ Nat.show(bytes(String.join(Log.lines(), "\n") ++ "\n")) ++ " " ++ Nat.show(Mat.ord(mat())))
    o1 : String <- D.Db.open(dir ++ "/store.db")
    check("tiny chunks", String.drop(o1, 3n))
    b2 : B.Built <- B.Stb.build(dir, 1048576)
    C.Check.str("rebuilt, one chunk", built(b2), built(b1))
    o2 : String <- D.Db.open(dir ++ "/store.db")
    check("one chunk", String.drop(o2, 3n))
    rm : Result<&1, &1, U32 & String, U32 & (String & String)> <- Process.run("rm", ["-rf", dir], "", 4096, 10000)
    return Unit{}
