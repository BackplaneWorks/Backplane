import Base
import ./check.bend as C
import ../src/core/storedb.bend as S
import ../src/core/model.bend as M
import ../src/core/json.bend as J
import ../src/core/codec.bend as CD
import ../src/core/utf8.bend as U
import ../src/server/db.bend as D
import ../src/server/stobuild.bend as B
import ../src/core/far.bend as FR
import ../src/core/mail.bend as ML
import ../src/core/bot.bend as BT

# The store's streamed first build (src/server/stobuild.bend), under JS:
# a log written to a fresh mktemp directory (the demo's changes, a torn
# line, a newer hub's line, a long line, a trailing fragment), built with
# a tiny chunk (most lines outgrow it) and again with a big one; the
# tables must equal the pure store: facts as SDB.Mat.replay files them,
# records as SDB.Tab.dump of the fold (no counter rows), the in-memory
# counters Switch.count.

# the log's lines (without newlines), oldest first
def Log.tail() -> List<&2, M.Change>:
  [M.MessagePosted{"t2", "m9", M.User{}, String.repeat("ü", 150n) ++ " long line", 30n}, M.ProviderSet{"t2", "codex", "gpt", 31n},
   M.ActivityLogged{"t2", "u9", "tool", "Bash", "make", 32n}, M.ProviderSet{"t2", "claude", "opus", 33n},
   # Cold settings bypass the fold, but their SQL rows still equal its
   # final dump: repeated keys, unicode, #digits, empty values and the
   # WorkSet spelling of the same setting. Tiny chunks split the writes.
   M.SettingSet{"launch.input.run1", "replacement ü", 34n},
   M.SettingSet{"fallback.input.run2", "first", 35n},
   M.SettingSet{"launch.input.run1", "#42", 36n},
   M.SettingSet{"fallback.input.run2", "", 37n},
   M.SettingSet{"launch.input.rum", "hot neighbour", 38n},
   M.WorkSet{"run-work", "launch.input", "first work value", 39n},
   M.SettingSet{"launch.input.run-work", "second work value", 40n},
   M.WorkSet{"run-work", "launch.input", "", 41n},
   # A mail header older than the newest is filed but not retained. A
   # duplicate of that dropped header still does not change the records;
   # a timestamp tie selects the last arrival just as Headers.add does.
   M.MailPosted{"ml-new", "c1", "person", "bot:b1", "note", "", "act", "new", 3n, 45n},
   M.MailPosted{"ml-old", "c1", "person", "bot:b1", "note", "", "fyi", "old", 3n, 21n},
   M.MailPosted{"ml-old", "c1", "person", "bot:b1", "note", "", "act", "duplicate", 9n, 46n},
   M.MailPosted{"ml-tie", "c1", "person", "bot:b1", "note", "", "fyi", "tie", 3n, 45n},
   M.RoomPosted{"rp-new", "r1", "b1", "new room text", 0n, 47n},
   M.RoomPosted{"rp-old", "r1", "b1", "older room text", 0n, 16n},
   M.RoomPosted{"rp-old", "r1", "b1", "duplicate room text", 0n, 48n}]

# Cross the record-count flush boundary in a single domain, then the
# payload budget with one oversized unicode record. The same full
# SQL-vs-fold comparison checks all resulting batches, not just the last.
def Log.batch(+n: Nat) -> List<&2, M.Change>:
  match n:
    case 0n:
      [M.SettingSet{"batch.oversized", String.repeat("ü", 17000n), 50n}]
    case 1n+r:
      M.SettingSet{"batch." ++ Nat.show(r), "value", 50n} <> Log.batch(r)

def Log.encode(cs: List<&2, M.Change>) -> List<&2, String>:
  match cs:
    case Nil{}:
      Nil{}
    case c <> t:
      J.Json.show(CD.Change.encode(c)) <> Log.encode(t)

def Log.lines() -> List<&2, String>:
  List.append(&2, String, Log.encode(S.Demo.changes()), List.append(&2, String, ["{torn", "{\"$\":\"FromTheFuture\",\"at\":5}"], Log.encode(List.append(&2, M.Change, Log.tail(), Log.batch(140n)))))

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

def built.n(b: B.Built) -> String:
  match b:
    case B.Built{s, _, _}:
      Nat.show(s)

def built(b: B.Built) -> String:
  match b:
    case B.Built{s, o, n}:
      Nat.show(s) ++ " " ++ Nat.show(o) ++ " " ++ Nat.show(n)

def bytes(+s: String) -> Nat:
  List.length(&2, U32, U.Utf8.encode(s))

# the build's lines, as it cuts them
def Lns.of(ls: List<&2, String>) -> List<&2, B.Ln>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      B.Ln{bytes(l), l} <> Lns.of(t)

# the counters after every line: Switch.count of the fold, thread by thread
def counters() -> String:
  +c : Map<&2, Nat> = B.Acc.cnt(B.Stb.lines(Lns.of(Log.lines()), B.Acc.new("")))
  Nat.show(B.Stb.count(c, "t1")) ++ " " ++ Nat.show(B.Stb.count(c, "t2"))

# No cold setting key survives even before Acc.take strips the entries.
def Cold.count(xs: List<&2, String>, n: Nat) -> Nat:
  match xs:
    case Nil{}:
      n
    case +k <> t:
      Cold.count(t, Nat.add(n, Bool.pick(Nat, S.Sdb.cold(k), 1n, 0n)))

def Cold.kept() -> Nat:
  Cold.count(M.Setting.keys(B.Acc.st(B.Stb.lines(Lns.of(Log.lines()), B.Acc.new("")))), 0n)

# The chunk checkpoint drops full room texts and keeps one mail header
# per conversation; the unchanged SQL-vs-fold checks cover every fact and
# record, including wanted, counts and duplicate suppression.
def Retained.histories() -> String:
  +st = B.Stb.strip(B.Acc.st(B.Stb.lines(Lns.of(Log.lines()), B.Acc.new(""))))
  Nat.show(List.length(&2, ML.Mail, ML.Box.headers(M.State.mail(st), "c1"))) ++ " " ++
    Nat.show(List.length(&2, ML.Mail, ML.Box.headers(M.State.mail(st), "room:r1"))) ++ " " ++
    Nat.show(List.length(&2, BT.Post, S.W.posts(M.State.world.of(st))))

def Far.rows(fs: List<&2, FR.Fact>, acc: List<&2, String>) -> List<&2, String>:
  match fs:
    case Nil{}:
      acc
    case FR.Fact{+q, +sc} <> t:
      Far.rows(t, (Nat.show(q) ++ " " ++ sc) <> acc)

# far_line as the commit path files it: FR.Fr.fact for each line that
# decodes (its number among the lines that parse), judged by the fold (a
# change's far row depends only on mappings fixed once made: F1)
def Far.want.go(ls: List<&2, S.LL>, +n: Nat, +st: M.State, acc: List<&2, String>) -> List<&2, String>:
  match ls:
    case Nil{}:
      acc
    case S.LL{_, False{}, _} <> t:
      Far.want.go(t, n, st, acc)
    case S.LL{_, True{}, None{}} <> t:
      Far.want.go(t, 1n+n, st, acc)
    case S.LL{_, True{}, Some{+c}} <> t:
      Far.want.go(t, 1n+n, st, Far.rows(FR.Fr.fact(st, c, 1n+n), acc))

def Far.got(rs: List<&2, List<&2, String>>, acc: List<&2, String>) -> List<&2, String>:
  match rs:
    case (+q <> +sc <> _) <> t:
      Far.got(t, (q ++ " " ++ sc) <> acc)
    case _ <> t:
      Far.got(t, acc)
    case Nil{}:
      acc

def Far.set(xs: List<&2, String>, m: Map<&2, Bool>) -> Map<&2, Bool>:
  match xs:
    case Nil{}:
      m
    case +x <> t:
      Far.set(t, Map.set(&2, Bool, m, x, True{}))

def Far.sort(xs: List<&2, String>) -> String:
  String.join(Map.keys(&2, Bool, Far.set(xs, Map.new(&2, Bool))), "|")

# the files a build leaves beside the log (none but store.db)
def Files.left(r: Result<&1, &1, U32 & String, U32 & (String & String)>) -> String:
  match r:
    case Done{(_, (out, _))}:
      String.trim(out)
    case Fail{_}:
      "?"

def check(+name: String, +h: String) -> IO(Unit):
  do IO<Unit>:
    fs : Map<&2, String> <- Db.tables(facts(), h, "", Map.new(&2, String))
    C.Check.str(name ++ ": facts equal the pure store's", Db.show(fs), Pure.of(S.Mat.facts(mat())))
    rs : Map<&2, String> <- Db.tables(recs(), h, "", Map.new(&2, String))
    C.Check.str(name ++ ": records equal the fold's dump", Db.show(rs), Pure.of(S.Tab.dump(fold())))
    c0 : List<&2, List<&2, String>> <- D.Db.query(h, "SELECT count(*) FROM obj WHERE domain = 'count'", [], 1)
    C.Check.str(name ++ ": no counter rows", one(c0), "0")
    mt : List<&2, List<&2, String>> <- D.Db.query(h, "SELECT group_concat(k || '=' || v, ' ') FROM (SELECT k, v FROM meta WHERE k <> 'log_id' ORDER BY k)", [], 1)
    C.Check.str(name ++ ": meta", one(mt), "applied_off=" ++ Nat.show(bytes(String.join(Log.lines(), "\n") ++ "\n")) ++ " applied_seq=" ++ Nat.show(Nat.sub(List.length(&2, String, Log.lines()), 1n)) ++ " codec=" ++ Nat.show(S.Sdb.codec()) ++ " far_line=filled ord=" ++ Nat.show(Mat.ord(mat())) ++ " origin=1 schema=1")
    id : List<&2, List<&2, String>> <- D.Db.query(h, "SELECT v FROM meta WHERE k = 'log_id'", [], 1)
    C.Check.str(name ++ ": log id", one(id), S.Sdb.log_id(U.Utf8.encode(String.join(Log.lines(), "\n") ++ "\n{\"$\":\"MessagePosted\",\"th")))
    fl : List<&2, List<&2, String>> <- D.Db.query(h, "SELECT seq, scope FROM far_line", [], 100000)
    C.Check.str(name ++ ": far_line as the commit path files it", Far.sort(Far.got(fl, Nil{})), Far.sort(Far.want.go(Log.lls(Log.lines()), 0n, fold(), Nil{})))
    cl : String <- D.Db.close(h)
    return Unit{}

def main() -> IO(Unit):
  do IO<Unit>:
    made : Result<&1, &1, U32 & String, U32 & (String & String)> <- Process.run("mktemp", ["-d"], "", 4096, 10000)
    +dir : String <- dir.of(made)
    Wf.put(dir ++ "/events.jsonl", String.join(Log.lines(), "\n") ++ "\n{\"$\":\"MessagePosted\",\"th")
    C.Check.str("counters are Switch.count", counters(), Nat.show(M.Switch.count(fold(), "t1")) ++ " " ++ Nat.show(M.Switch.count(fold(), "t2")))
    C.Check.str("cold settings never enter the build fold", Nat.show(Cold.kept()), "0")
    C.Check.str("retained mail headers c1/room:r1 and room texts", Retained.histories(), "1 1 0")
    +b1 : B.Built <- B.Stb.build(dir, 64)
    C.Check.str("tiny chunks: lines, offset, entries", built(b1), Nat.show(Nat.sub(List.length(&2, String, Log.lines()), 1n)) ++ " " ++ Nat.show(bytes(String.join(Log.lines(), "\n") ++ "\n")) ++ " " ++ Nat.show(Mat.ord(mat())))
    o1 : String <- D.Db.open(dir ++ "/store.db")
    check("tiny chunks", String.drop(o1, 3n))
    # an earlier build cut short left its temp file: a build starts over
    Wf.put(dir ++ "/store.db.tmp", "half a store")
    b2 : B.Built <- B.Stb.build(dir, 1048576)
    ls : Result<&1, &1, U32 & String, U32 & (String & String)> <- Process.run("sh", ["-c", "cd " ++ dir ++ " && ls store.db* store.build* 2>/dev/null | tr '\\n' ' '"], "", 4096, 10000)
    C.Check.str("a build leaves store.db alone (no temp, no progress file)", Files.left(ls), "store.db")
    C.Check.str("rebuilt, one chunk", built(b2), built(b1))
    o2 : String <- D.Db.open(dir ++ "/store.db")
    check("one chunk", String.drop(o2, 3n))
    # a line that parses but is no object ("Unreadable"): numbered, its wire
    # the inert placeholder (C.Change.wire_over), the records as before
    Wf.put(dir ++ "/events.jsonl", String.join(Log.lines(), "\n") ++ "\n\"Unreadable\"\n")
    b3 : B.Built <- B.Stb.build(dir, 1048576)
    C.Check.str("a non-object line is numbered", built.n(b3), Nat.show(List.length(&2, String, Log.lines())))
    +o3 : String <- D.Db.open(dir ++ "/store.db")
    w3 : List<&2, List<&2, String>> <- D.Db.query(String.drop(o3, 3n), "SELECT wire FROM line ORDER BY seq DESC LIMIT 1", [], 1)
    C.Check.str("its wire is the inert placeholder", one(w3), CD.Change.inert())
    r3 : Map<&2, String> <- Db.tables(recs(), String.drop(o3, 3n), "", Map.new(&2, String))
    C.Check.str("records unchanged by it", Db.show(r3), Pure.of(S.Tab.dump(fold())))
    c3 : String <- D.Db.close(String.drop(o3, 3n))
    rm : Result<&1, &1, U32 & String, U32 & (String & String)> <- Process.run("rm", ["-rf", dir], "", 4096, 10000)
    return Unit{}
