import Base
import ./check.bend as C
import ../src/server/db.bend as D
import ../src/server/hotboot.bend as HB
import ../src/core/storedb.bend as S
import ../src/core/store.bend as ST
import ../src/core/model.bend as M
import ../src/core/codec.bend as CO
import ../src/core/json.bend as J
import ../src/core/utf8.bend as U

# The hot boot end to end (S3): a home whose events.jsonl is the demo log
# with a torn fragment in the middle (a crash cut a line) and an empty
# line, and a store.db holding what S1's pure materializer makes of its
# first lines (facts, records, and meta as stobuild writes it).
# HB.Hot.load must answer the full replay's records (but the cold
# settings), retaining no wire lines, for cuts before and after the torn
# line and at the end; a store with another codec or of another log is refused.

def text.of(c: M.Change) -> String:
  J.Json.show(CO.Change.encode(c))

def lines.k(torn: Bool, empty: Bool, l: String, rest: List<&2, String>) -> List<&2, String>:
  match torn empty:
    case True{} _:
      ST.Store.torn.demo() <> l <> rest
    case False{} True{}:
      "" <> l <> rest
    case False{} False{}:
      l <> rest

# every demo change, a torn fragment before the 20th, an empty line before the 40th
def lines.go(cs: List<&2, M.Change>, +i: Nat) -> List<&2, String>:
  match cs:
    case Nil{}:
      Nil{}
    case c <> t:
      lines.k(Nat.is_eq(i, 20n), Nat.is_eq(i, 40n), text.of(c), lines.go(t, 1n+i))

def lines() -> List<&2, String>:
  lines.go(S.Demo.changes(), 0n)

def ll.k(m: Maybe<&2, J.Json>, +len: Nat) -> S.LL:
  match m:
    case Some{o}:
      S.LL{len, True{}, CO.Change.decode(o)}
    case None{}:
      S.LL{len, False{}, None{}}

def lls(ls: List<&2, String>) -> List<&2, S.LL>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      ll.k(J.Json.parse(l), List.length(&2, U32, U.Utf8.encode(l))) <> lls(t)

def head(+k: String) -> String:
  Maybe.default(&2, String, List.head(&2, String, String.split(k, '\u{1f}')), "")

def items(xs: List<&2, Sigma<&2, &2, String, _ => String>>) -> List<&2, List<&2, String>>:
  match xs:
    case Nil{}:
      Nil{}
    case (+k, +v) <> t:
      S.Sql.of(S.Put{head(k), String.split(v, '\u{1f}')}) <> items(t)

def meta(+k: String, v: String) -> List<&2, String>:
  ["INSERT OR REPLACE INTO meta(k, v) VALUES (?, ?)", k, v]

def store.items(m: S.Mat, +codec: String, +id: String) -> List<&2, List<&2, String>>:
  match m:
    case S.Mat{_, fs, rs, +seq, +off, _}:
      List.append(&2, List<&2, String>, items(Map.to_list(&2, String, fs)), List.append(&2, List<&2, String>, items(Map.to_list(&2, String, rs)),
        [meta("schema", "1"), meta("codec", codec), meta("applied_seq", Nat.show(seq)), meta("applied_off", Nat.show(off)), meta("log_id", id)]))

def warm.k(cold: Bool, m: Map<&2, String>, +k: String, +v: String) -> Map<&2, String>:
  match cold:
    case True{}:
      m
    case False{}:
      Map.set(&2, String, m, k, v)

# records but the cold settings (a hot boot holds those only when the tail
# set them, or for an open thread)
def warm.go(xs: List<&2, Sigma<&2, &2, String, _ => String>>, m: Map<&2, String>) -> Map<&2, String>:
  match xs:
    case Nil{}:
      m
    case (+k, +v) <> t:
      warm.go(t, warm.k(Bool.and(String.starts_with(k, "setting\u{1f}"), S.Sdb.cold(String.drop(k, 8n))), m, k, v))

def dump.rows(xs: List<&2, Sigma<&2, &2, String, _ => String>>) -> String:
  match xs:
    case Nil{}:
      ""
    case (k, v) <> t:
      k ++ " => " ++ v ++ "\n" ++ dump.rows(t)

def dump(+m: Map<&2, String>) -> String:
  dump.rows(Map.to_list(&2, String, m))

def full() -> ST.Loaded:
  ST.Store.load(lines(), ST.Loaded{M.State.empty(), Nil{}})

def want() -> String:
  dump(warm.go(Map.to_list(&2, String, S.Tab.dump(ST.Loaded.st(full()))), Map.new(&2, String)))

def dir.of(r: Result<&1, &1, U32 & String, U32 & (String & String)>) -> IO(String):
  match r:
    case Done{(_, (out, _))}:
      IO.pure(String, String.trim(out))
    case Fail{(_, e)}:
      IO.die(String, 1, "hotboot_test: mktemp failed: " ++ e)

def write.done(r: File & Result<&1, &1, U32 & String, Unit>) -> IO(Unit):
  (f, _) = r
  File.close(f)

def write.k(r: Result<&1, &1, U32 & String, File>, s: String) -> IO(Unit):
  match r:
    case Done{f}:
      IO.bind(File & Result<&1, &1, U32 & String, Unit>, Unit, File.write(f, s), write.done)
    case Fail{(_, e)}:
      IO.die(Unit, 1, "hotboot_test: cannot write the log: " ++ e)

def write(path: String, s: String) -> IO(Unit):
  IO.bind(Result<&1, &1, U32 & String, File>, Unit, File.open(path, "w"), r => write.k(r, s))

# a home with the log and the store of its first n lines
def home(+n: Nat, +codec: String, +id: String) -> IO(String):
  do IO<String>:
    made : Result<&1, &1, U32 & String, U32 & (String & String)> <- Process.run("mktemp", ["-d"], "", 4096, 10000)
    +dir : String <- dir.of(made)
    write(dir ++ "/events.jsonl", ST.Store.text(lines()))
    +o : String <- D.Db.open(dir ++ "/store.db")
    +h : String = String.drop(o, 3n)
    s : String <- D.Db.exec(h, S.Sdb.schema())
    C.Check.str("schema", s, "ok")
    w : String <- D.Db.script(h, store.items(S.Mat.replay(List.take(&2, S.LL, lls(lines()), n), S.Mat.empty()), codec, id))
    C.Check.str("store written (" ++ Nat.show(n) ++ ")", String.take(w, 2n), "ok")
    cl : String <- D.Db.close(h)
    return dir

def log.id() -> String:
  S.Sdb.log_id(U.Utf8.encode(ST.Store.text(lines())))

def hot.loads(+n: Nat) -> IO(Unit):
  do IO<Unit>:
    +dir : String <- home(n, Nat.show(S.Sdb.codec()), log.id())
    +hot : HB.Hot <- HB.Hot.load(dir)
    C.Check.str("cut " ++ Nat.show(n) ++ ": used", HB.Hot.why(hot), "")
    C.Check.str("cut " ++ Nat.show(n) ++ ": records", dump(warm.go(Map.to_list(&2, String, S.Tab.dump(HB.Hot.st(hot))), Map.new(&2, String))), want())
    C.Check.str("cut " ++ Nat.show(n) ++ ": no log lines held", Nat.show(List.length(&2, String, HB.Hot.ls(hot))), "0")
    C.Check.str("cut " ++ Nat.show(n) ++ ": seq", Nat.show(M.State.seq(HB.Hot.st(hot))), Nat.show(M.State.seq(ST.Loaded.st(full()))))
    rm : Result<&1, &1, U32 & String, U32 & (String & String)> <- Process.run("rm", ["-rf", dir], "", 4096, 10000)
    return Unit{}

def hot.refused(+what: String, +codec: String, +id: String) -> IO(Unit):
  do IO<Unit>:
    +dir : String <- home(30n, codec, id)
    +hot : HB.Hot <- HB.Hot.load(dir)
    C.Check.str(what ++ ": refused", Bool.show(HB.Hot.ok(hot)), "False")
    IO.print("  (" ++ HB.Hot.why(hot) ++ ")")
    rm : Result<&1, &1, U32 & String, U32 & (String & String)> <- Process.run("rm", ["-rf", dir], "", 4096, 10000)
    return Unit{}

def main() -> IO(Unit):
  do IO<Unit>:
    hot.loads(0n)
    hot.loads(10n)
    hot.loads(20n)
    hot.loads(21n)
    hot.loads(22n)
    hot.loads(45n)
    hot.loads(List.length(&2, String, lines()))
    hot.refused("another codec", "0", log.id())
    hot.refused("another log", Nat.show(S.Sdb.codec()), "sha1:00")
