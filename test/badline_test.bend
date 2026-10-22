import Base
import ./check.bend as C
import ../src/server/db.bend as D
import ../src/server/hotboot.bend as HB
import ../src/server/storedb.bend as SO
import ../src/server/stobuild.bend as STB
import ../src/core/storedb.bend as S
import ../src/core/store.bend as ST
import ../src/core/model.bend as M
import ../src/core/codec.bend as CO
import ../src/core/json.bend as J

# A log line that parses but is not a JSON object (J.Json.parse is
# lenient: a bare word passes; a JSON array is valid but no change) must
# never reach a client as its own text: one such line broke every join
# from 0. It keeps its number (since/resume stay exact) and goes out as
# CO.Change.inert(), in the full replay's lines, in the store's line table
# (a join from 0 and a resume after it, SO.Sto.lines) and in the hot boot.

def text.of(c: M.Change) -> String:
  J.Json.show(CO.Change.encode(c))

def bad.at(+i: Nat) -> String:
  Bool.pick(String, Nat.is_eq(i, 10n), "not json at all", Bool.pick(String, Nat.is_eq(i, 25n), "[1, 2]", ""))

def lines.k(none: Bool, bad: String, l: String, rest: List<&2, String>) -> List<&2, String>:
  match none:
    case True{}: l <> rest
    case False{}: bad <> l <> rest

# every demo change, a bare word before the 10th, an array before the 25th
def lines.go(cs: List<&2, M.Change>, +i: Nat) -> List<&2, String>:
  match cs:
    case Nil{}: Nil{}
    case c <> t: lines.k(String.is_empty(bad.at(i)), bad.at(i), text.of(c), lines.go(t, 1n+i))

def lines() -> List<&2, String>:
  lines.go(S.Demo.changes(), 0n)

def full() -> ST.Loaded:
  ST.Store.load(lines(), ST.Loaded{M.State.empty(), Nil{}})

# the lines clients see, oldest first
def wire() -> List<&2, String>:
  List.reverse(&2, String, ST.Loaded.wire(full()))

def objs.k(m: Maybe<&2, J.Json>) -> Bool:
  match m:
    case Some{o}: CO.Change.is_obj(o)
    case None{}: False{}

def objs(ws: List<&2, String>, +n: Nat) -> Nat:
  match ws:
    case Nil{}: n
    case w <> t: objs(t, Nat.add(n, Bool.pick(Nat, objs.k(J.Json.parse(w)), 0n, 1n)))

def inert(ws: List<&2, String>, +n: Nat) -> Nat:
  match ws:
    case Nil{}: n
    case w <> t: inert(t, Nat.add(n, Bool.pick(Nat, String.eq(w, CO.Change.inert()), 1n, 0n)))

def dir.of(r: Result<&1, &1, U32 & String, U32 & (String & String)>) -> IO(String):
  match r:
    case Done{(_, (out, _))}: IO.pure(String, String.trim(out))
    case Fail{(_, e)}: IO.die(String, 1, "badline_test: mktemp failed: " ++ e)

def write.done(r: File & Result<&1, &1, U32 & String, Unit>) -> IO(Unit):
  (f, _) = r
  File.close(f)

def write.k(r: Result<&1, &1, U32 & String, File>, s: String) -> IO(Unit):
  match r:
    case Done{f}: IO.bind(File & Result<&1, &1, U32 & String, Unit>, Unit, File.write(f, s), write.done)
    case Fail{(_, e)}: IO.die(Unit, 1, "badline_test: cannot write the log: " ++ e)

def write(path: String, s: String) -> IO(Unit):
  IO.bind(Result<&1, &1, U32 & String, File>, Unit, File.open(path, "w"), r => write.k(r, s))

def built.seq(b: STB.Built) -> Nat:
  match b:
    case STB.Built{s, _, _}: s

def built.ord(b: STB.Built) -> Nat:
  match b:
    case STB.Built{_, _, o}: o

def main() -> IO(Unit):
  do IO<Unit>:
    +n : Nat = List.length(&2, String, wire())
    C.Check.str("both odd lines are numbered", Nat.show(n), Nat.show(Nat.add(List.length(&2, M.Change, S.Demo.changes()), 2n)))
    C.Check.str("full replay: every wire line is an object", Nat.show(objs(wire(), 0n)), "0")
    C.Check.str("full replay: the two odd lines go out inert", Nat.show(inert(wire(), 0n)), "2")
    made : Result<&1, &1, U32 & String, U32 & (String & String)> <- Process.run("mktemp", ["-d"], "", 4096, 10000)
    +dir : String <- dir.of(made)
    write(dir ++ "/events.jsonl", ST.Store.text(lines()))
    +b : STB.Built <- STB.Stb.build(dir, 512)
    C.Check.str("store build: every line numbered", Nat.show(built.seq(b)), Nat.show(n))
    +o : String <- D.Db.open(dir ++ "/store.db")
    +lg : SO.Lg = SO.Lg{String.drop(o, 3n), dir ++ "/events.jsonl", built.seq(b), built.ord(b), ""}
    +j0 : List<&2, String> <- SO.Sto.lines(lg, 0n, n)
    C.Check.str("join from 0: the full replay's lines", String.join(j0, "\n"), String.join(wire(), "\n"))
    C.Check.str("join from 0: every line an object", Nat.show(objs(j0, 0n)), "0")
    +r1 : List<&2, String> <- SO.Sto.lines(lg, 11n, n)
    C.Check.str("resume after the bare word", String.join(r1, "\n"), String.join(List.drop(&2, String, wire(), 11n), "\n"))
    +r2 : List<&2, String> <- SO.Sto.lines(lg, 10n, 3n)
    C.Check.str("resume at the bare word", String.join(r2, "\n"), String.join(List.take(&2, String, List.drop(&2, String, wire(), 10n), 3n), "\n"))
    cl : String <- D.Db.close(String.drop(o, 3n))
    +hot : HB.Hot <- HB.Hot.load(dir)
    C.Check.str("hot boot: used", HB.Hot.why(hot), "")
    C.Check.str("hot boot: seq", Nat.show(M.State.seq(HB.Hot.st(hot))), Nat.show(M.State.seq(ST.Loaded.st(full()))))
    rm : Result<&1, &1, U32 & String, U32 & (String & String)> <- Process.run("rm", ["-rf", dir], "", 4096, 10000)
    return Unit{}
