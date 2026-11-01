import Base
import ../core/json.bend as J
import ../core/model.bend as M
import ../core/codec.bend as C
import ../core/storedb.bend as SDB
import ../core/far.bend as F
import ../core/exe.bend as EX
import ./stobuild.bend as SB
import ./db.bend as D
import ./proc.bend as P
import ./host.bend as H
import ./perf.bend as PS

# far_line refilled from the log
# -----------------------------
# far_line (the lines that go to the owner's other machines, read by
# /far/log v3) is written by every commit and catch-up and, since the
# build writes it too, by a rebuild. A store a rebuild made before that has
# none for its history, and no meta "far_line". Once the hub serves, a
# child (`backplane --store-far HOME`, nice 19 and ionice idle) folds the log up to the
# store's applied_off as the build does, with no entries kept and no facts
# but these rows, numbering the lines as the store does, and adds every
# missing row in short transactions (a 64 KB chunk each: the hub's
# connection waits at most 5 s for a lock), each with meta "far_line.seq",
# the last line written: a refill a restart cut off folds from the start
# again (a row needs the state at its line) but writes only past it. Then
# meta "far_line" is set to 1.
# Rows the commits wrote are kept (ON CONFLICT DO NOTHING). Test:
# test/tools/store_far_line_e2e.ts.

def Fl.done() -> String:
  "1"

# the fold: its state (no entries, no turn inputs: Fl.input), the counters, the last
# line's number, the next line's offset, this chunk's rows, and the line up
# to which rows were already written (meta "far_line.seq": a refill cut off
# by a restart writes nothing twice)
type FAcc is Data:
  FAcc{st: M.State, cnt: Map<&2, Nat>, seq: Nat, off: Nat, rows: List<&2, List<&2, String>>, past: Nat}

def Fl.rows.k(write: Bool, +post: M.State, +c: M.Change, +seq: Nat, rows: List<&2, List<&2, String>>) -> List<&2, List<&2, String>>:
  match write:
    case True{}:
      SB.Stb.far(F.Fr.fact(post, c, seq), rows)
    case False{}:
      rows

# a turn's input (launch.input.*, fallback.input.*: a few hundred MB on a
# real log) is not folded: no fold step reads one, and none is shared
def Fl.input(c: M.Change) -> Bool:
  match c:
    case M.SettingSet{+k, _, _}:
      Bool.or(String.starts_with(k, "launch.input."), String.starts_with(k, "fallback.input."))
    case _:
      False{}

def Fl.apply.k(skip: Bool, +st: M.State, +cnt: Map<&2, Nat>, +c: M.Change) -> M.State:
  match skip:
    case True{}:
      st
    case False{}:
      SB.Stb.apply(st, cnt, c)

def Fl.change(m: Maybe<&2, M.Change>, +st: M.State, +cnt: Map<&2, Nat>, +seq: Nat, +off: Nat, +len: Nat, rows: List<&2, List<&2, String>>, +past: Nat) -> FAcc:
  match m:
    case Some{+c}:
      +post = Fl.apply.k(Fl.input(c), st, cnt, c)
      FAcc{post, SB.Stb.bump(SDB.F.posts(c), cnt), seq, Nat.add(off, 1n+len), Fl.rows.k(Nat.is_gt(seq, past), post, c, seq, rows), past}
    case None{}:
      FAcc{st, cnt, seq, Nat.add(off, 1n+len), rows, past}

# a line that is JSON takes the next number (SB.Stb.parsed), one that is not
# only its bytes
def Fl.parsed(m: Maybe<&2, J.Json>, st: M.State, cnt: Map<&2, Nat>, +seq: Nat, +off: Nat, +len: Nat, rows: List<&2, List<&2, String>>, +past: Nat) -> FAcc:
  match m:
    case Some{+o}:
      Fl.change(C.Change.decode(SB.Stb.flat(o)), st, cnt, 1n+seq, off, len, rows, past)
    case None{}:
      FAcc{st, cnt, seq, Nat.add(off, 1n+len), rows, past}

def Fl.line(a: FAcc, l: SB.Ln) -> FAcc:
  match a l:
    case FAcc{st, cnt, +seq, +off, rows, +past} SB.Ln{+len, +text}:
      Fl.parsed(J.Json.parse(text), st, cnt, seq, off, len, rows, past)

def Fl.lines(ls: List<&2, SB.Ln>, a: FAcc) -> FAcc:
  match ls:
    case Nil{}:
      a
    case l <> t:
      Fl.lines(t, Fl.line(a, l))

# the chunk's rows with the checkpoint, and the fold without them
def Fl.take(a: FAcc) -> List<&2, List<&2, String>> & FAcc:
  match a:
    case FAcc{st, cnt, +seq, off, rows, +past}:
      (List.append(&2, List<&2, String>, rows, [SB.Stb.meta.sql("far_line.seq", Nat.show(Bool.pick(Nat, Nat.is_gt(seq, past), seq, past)))]), FAcc{SB.Stb.strip(st), cnt, seq, off, Nil{}, past})

def Fl.off(a: FAcc) -> Nat:
  match a:
    case FAcc{_, _, _, off, _, _}:
      off

def Fl.seq(a: FAcc) -> Nat:
  match a:
    case FAcc{_, _, seq, _, _, _}:
      seq

def Fl.write.k(+h: String, r: List<&2, List<&2, String>> & FAcc) -> IO(FAcc):
  (rows, +a) = r
  do IO<FAcc>:
    +res : String <- D.Db.script(h, rows)
    SB.Stb.check("far_line ending at " ++ Nat.show(Fl.off(a)), res)
    return a

def Fl.write(+h: String, ls: List<&2, SB.Ln>, a: FAcc) -> IO(FAcc):
  Fl.write.k(h, Fl.take(Fl.lines(ls, a)))

# about this many bytes of lines a read (at least one whole line)
def Fl.budget() -> Nat:
  1048576n

# the complete lines from byte `from` (effects/storefar.js: the bytes stay
# in JavaScript), each "<bytes> <text>"
def Fl.lines.read(path: String, from: String, to: String, budget: String) -> IO(List<&2, String>):
  import "./effects/storefar.js"

# "<bytes> <text>": the digits before the first space, then the rest (one
# walk that stops at the space: the text can be megabytes)
def Fl.num(space: Bool, +c: U32, +n: Nat) -> Nat:
  match space:
    case True{}:
      n
    case False{}:
      Nat.add(Nat.mul(n, 10n), U32.to_nat((c - 48 : U32)))

def Fl.scan(s: String, done: Bool, +n: Nat) -> SB.Ln:
  match s done:
    case +r True{}:
      SB.Ln{n, r}
    case SNil{} False{}:
      SB.Ln{n, ""}
    case SCon{Chr{+c}, t} False{}:
      Fl.scan(t, U32.is_eq(c, 32), Fl.num(U32.is_eq(c, 32), c, n))

def Fl.ln.go(s: String, +n: Nat) -> SB.Ln:
  Fl.scan(s, False{}, n)

def Fl.lns(xs: List<&2, String>) -> List<&2, SB.Ln>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> t:
      Fl.ln.go(x, 0n) <> Fl.lns(t)

def Fl.got(none: Bool, +h: String, ls: List<&2, String>, +a: FAcc, k: FAcc -> IO(FAcc)) -> IO(FAcc):
  match none:
    case True{}:
      IO.pure(FAcc, a)
    case False{}:
      IO.bind(FAcc, FAcc, Fl.write(h, Fl.lns(ls), a), k)

def Fl.got.l(+ls: List<&2, String>, +h: String, +a: FAcc, k: FAcc -> IO(FAcc)) -> IO(FAcc):
  Fl.got(List.is_empty(&2, String, ls), h, ls, a, k)

def Fl.step(none: Bool, +h: String, +path: String, +to: Nat, +a: FAcc, k: FAcc -> IO(FAcc)) -> IO(FAcc):
  match none:
    case True{}:
      IO.pure(FAcc, a)
    case False{}:
      IO.bind(List<&2, String>, FAcc, Fl.lines.read(path, Nat.show(Fl.off(a)), Nat.show(to), Nat.show(Fl.budget())),
        ls => Fl.got.l(ls, h, a, k))

# reads until `to` (the store's applied_off: the commits number the rest)
def Fl.loop(fuel: Nat, +h: String, +path: String, +to: Nat, +a: FAcc) -> IO(FAcc):
  match fuel:
    case 0n:
      IO.pure(FAcc, a)
    case 1n+n:
      Fl.step(Nat.is_ge(Fl.off(a), to), h, path, to, a, a2 => Fl.loop(n, h, path, to, a2))

def Fl.cell.k(r: Maybe<&2, List<&2, String>>) -> String:
  match r:
    case Some{cs}:
      Maybe.default(&2, String, List.head(&2, String, cs), "")
    case None{}:
      ""

def Fl.one(+h: String, +sql: String) -> IO(String):
  IO.bind(List<&2, List<&2, String>>, String, D.Db.query(h, sql, Nil{}, 1), rows => IO.pure(String, Fl.cell.k(List.head(&2, List<&2, String>, rows))))

def Fl.fill.k(+a: FAcc, +h: String, +seq: Nat) -> IO(String):
  do IO<String>:
    m : String <- D.Db.script(h, [SB.Stb.meta.sql("far_line", Fl.done())])
    +n : String <- Fl.one(h, "SELECT count(*) FROM far_line")
    return Bool.pick(String, Nat.is_eq(Fl.seq(a), seq), "filled " ++ n ++ " rows to line " ++ Nat.show(Fl.seq(a)),
      "fail far_line numbered " ++ Nat.show(Fl.seq(a)) ++ " lines, the store " ++ Nat.show(seq))

# the log folded up to applied_off; its rows written; meta marked. The
# numbering must end where the store's does, or nothing is marked done.
def Fl.fill(+h: String, +home: String, +to: Nat, +seq: Nat) -> IO(String):
  do IO<String>:
    past : String <- Fl.one(h, "SELECT v FROM meta WHERE k = 'far_line.seq'")
    a : FAcc <- Fl.loop(Nat.add(to, 64n), h, home ++ "/events.jsonl", to, FAcc{M.State.empty(), Map.new(&2, Nat), 0n, 0n, Nil{}, M.Switch.num(past, 0n)})
    Fl.fill.k(a, h, seq)

def Fl.go(due: Bool, +h: String, +home: String) -> IO(String):
  match due:
    case False{}:
      IO.pure(String, "current")
    case True{}:
      do IO<String>:
        to : String <- Fl.one(h, "SELECT v FROM meta WHERE k = 'applied_off'")
        seq : String <- Fl.one(h, "SELECT v FROM meta WHERE k = 'applied_seq'")
        Fl.fill(h, home, M.Switch.num(to, 0n), M.Switch.num(seq, 0n))

def Fl.child.h(+h: String, +home: String) -> IO(Unit):
  do IO<Unit>:
    have : String <- Fl.one(h, "SELECT v FROM meta WHERE k = 'far_line'")
    r : String <- Fl.go(Bool.not(String.eq(have, Fl.done())), h, home)
    cl : String <- D.Db.close(h)
    IO.print(r)

def Fl.child.k(ok: Bool, +o: String, +home: String) -> IO(Unit):
  match ok:
    case True{}:
      Fl.child.h(String.drop(o, 3n), home)
    case False{}:
      IO.print(o)

# `backplane --store-far HOME`: "current", "filled N rows to line M" or "fail ..."
def Fl.opened(+o: String, +home: String) -> IO(Unit):
  Fl.child.k(String.starts_with(o, "ok "), o, home)

def Fl.child(+home: String) -> IO(Unit):
  IO.bind(String, Unit, D.Db.open(home ++ "/store.db"), o => Fl.opened(o, home))

def Fl.back(xs: List<&2, String>) -> List<String>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      x <> Fl.back(t)

def Fl.copy(xs: List<String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      x <> Fl.copy(t)

def Fl.main.k(xs: List<&2, String>, go: List<String> -> IO(Unit)) -> IO(Unit):
  match xs:
    case "--store-far" <> +home <> _:
      Fl.child(home)
    case x:
      go(Fl.back(x))

# the program's arguments: the child's, or passed on (go)
def Fl.main(xs: List<String>, go: List<String> -> IO(Unit)) -> IO(Unit):
  Fl.main.k(Fl.copy(xs), go)

# The hub's side
# --------------

def Fl.said(+r: String) -> IO(Unit):
  PS.Log.info("store", "far_line from the log: " ++ r)

def Fl.ran(r: P.Ran) -> IO(Unit):
  match r:
    case P.Ran{_, +out}:
      Fl.said(String.trim(out))

# the child at the lowest CPU and I/O priority there is (macOS has no
# ionice), its JS heap sized to 1 GB so it collects instead of growing
def Fl.low() -> String:
  "export BUN_JSC_forceRAMSize=1073741824; if command -v ionice >/dev/null 2>&1; then exec nice -n 19 ionice -c 3 \"$@\"; else exec nice -n 19 \"$@\"; fi"

def Fl.job.k(need: Bool, +home: String) -> IO(Unit):
  match need:
    case False{}:
      IO.pure(Unit, Unit{})
    case True{}:
      do IO<Unit>:
        Fl.said("its history is missing; refilling it in the background")
        exe : String <- H.Sys.exe_path()
        r : P.Ran <- P.Proc.run("sh", ["-c", Fl.low(), "sh", EX.Exe.child(exe), "--store-far", home], "/")
        Fl.ran(r)

# once the hub serves: only a store without meta "far_line" runs the child
def Fl.job(+home: String, +h: String) -> IO(Unit):
  do IO<Unit>:
    have : String <- Fl.one(h, "SELECT v FROM meta WHERE k = 'far_line'")
    Fl.job.k(Bool.not(String.eq(have, Fl.done())), home)

def Fl.start(+home: String, +h: String) -> IO(Unit):
  IO.spawn(Unit, Fl.job(home, h))
