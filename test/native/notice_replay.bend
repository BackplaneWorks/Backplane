import Base
import ../../src/core/model.bend as M
import ../../src/core/json.bend as J
import ../../src/core/codec.bend as C
import ../../src/core/utf8.bend as U
import ../../src/core/notice.bend as NO
import ../../src/server/argv.bend as AV

# How many alerts would the person have had? Folds an event log one change
# at a time and counts, for the changes whose `at` is in a window:
#   before: at each turn end (a thread's turn going from running to
#           completed, failed or interrupted), what the older rule alerts
#           (Notice.of: kind done or fail)
#   after:  what the policy records alert (Policy.end at each turn end,
#           Policy.ask at each AskOpened, Policy.merged at ThreadMerged,
#           default settings), folding the policy's own records into the
#           state as it goes
# Desks are ignored in both counts, and days are UTC.
#   scripts/build-js.sh test/native/notice_replay.bend build/notice_replay
#   build/notice_replay EVENTS.jsonl [FROM_EPOCH TO_EPOCH]
# It only reads the file it is given: run it on a copy with the devices and
# tokens left out, never on the live log:
#   grep -v -E '"(DeviceRegistered|DeviceDropped)"|"push\.|webhook|token' ~/.backplane-bend/events.jsonl > /tmp/np-events.jsonl

# one day's counts
type Day is Data:
  Day{day: Nat, bdone: Nat, bfail: Nat, aask: Nat, adone: Nat, afail: Nat, amerged: Nat}

type Acc is Data:
  Acc{st: M.State, days: List<&2, Day>}

def Day.new(+day: Nat) -> Day:
  Day{day, 0n, 0n, 0n, 0n, 0n, 0n}

def Day.add.k(k: Nat, +day: Nat, +a: Nat, +b: Nat, +c: Nat, +e: Nat, +f: Nat, +g: Nat) -> Day:
  match k:
    case 0n:
      Day{day, 1n+a, b, c, e, f, g}
    case 1n:
      Day{day, a, 1n+b, c, e, f, g}
    case 2n:
      Day{day, a, b, 1n+c, e, f, g}
    case 3n:
      Day{day, a, b, c, 1n+e, f, g}
    case 4n:
      Day{day, a, b, c, e, 1n+f, g}
    case _:
      Day{day, a, b, c, e, f, 1n+g}

# add one to the count k of a day (0 before done, 1 before fail, 2 after
# ask, 3 after done, 4 after fail, 5 after merged)
def Day.add(+d: Day, +k: Nat) -> Day:
  match d:
    case Day{+day, +a, +b, +c, +e, +f, +g}:
      Day.add.k(k, day, a, b, c, e, f, g)

def Day.of(d: Day) -> Nat:
  match d:
    case Day{x, _, _, _, _, _, _}:
      x

def Days.pick(same: Bool, +d: Day, +k: Nat, t: List<&2, Day>, rest: List<&2, Day>) -> List<&2, Day>:
  match same:
    case True{}:
      Day.add(d, k) <> t
    case False{}:
      d <> rest

# the day's counts with one more at k (a new day goes last)
def Days.bump(ds: List<&2, Day>, +day: Nat, +k: Nat) -> List<&2, Day>:
  match ds:
    case Nil{}:
      [Day.add(Day.new(day), k)]
    case +d <> +t:
      Days.pick(Nat.is_eq(Day.of(d), day), d, k, t, Days.bump(t, day, k))

# a count for the kind a record or a turn end alerts with (n: 0 before, 1
# after), when the change is in the window
def Rep.kind(+n: Nat, +kind: String) -> Nat:
  Bool.pick(Nat, Nat.is_eq(n, 0n),
    Bool.pick(Nat, String.eq(kind, "done"), 0n, Bool.pick(Nat, String.eq(kind, "fail"), 1n, 9n)),
    Bool.pick(Nat, String.eq(kind, "ask"), 2n, Bool.pick(Nat, String.eq(kind, "done"), 3n, Bool.pick(Nat, String.eq(kind, "fail"), 4n, Bool.pick(Nat, String.eq(kind, "merged"), 5n, 9n)))))

def Rep.count.k(ok: Bool, +k: Nat, +at: Nat, ds: List<&2, Day>) -> List<&2, Day>:
  match ok:
    case True{}:
      Days.bump(ds, Nat.div(at, 86400n), k)
    case False{}:
      ds

def Rep.count(inside: Bool, +k: Nat, +at: Nat, ds: List<&2, Day>) -> List<&2, Day>:
  match inside:
    case True{}:
      Rep.count.k(Nat.is_lt(k, 9n), k, at, ds)
    case False{}:
      ds

def Rep.in(+at: Nat, +from: Nat, +to: Nat) -> Bool:
  Bool.and(Nat.is_ge(at, from), Nat.is_le(at, to))

def Rep.end.k(+cs: List<&2, M.Change>, +old: NO.Notice, +post: M.State, +at: Nat, +inside: Bool, ds: List<&2, Day>) -> Acc:
  Acc{M.State.replay(cs, post), Rep.count(inside, Rep.kind(1n, NO.Policy.kind(cs)), at, Rep.count(inside, Rep.kind(0n, NO.Notice.kind(old)), at, ds))}

# a turn end: the state after the change; the older rule's alert, and the
# policy's record folded in
def Rep.end(+post: M.State, +th: String, +ts: String, +at: Nat, inside: Bool, ds: List<&2, Day>) -> Acc:
  Rep.end.k(NO.Policy.end(post, at, th, ts), NO.Notice.of(post, th, String.eq(ts, "failed"), True{}), post, at, inside, ds)

def Rep.ends(ts: M.TurnState) -> Bool:
  match ts:
    case M.Completed{}:
      True{}
    case M.Failed{}:
      True{}
    case M.Interrupted{}:
      True{}
    case _:
      False{}

def Rep.name(ts: M.TurnState) -> String:
  match ts:
    case M.Completed{}:
      "completed"
    case M.Failed{}:
      "failed"
    case _:
      "interrupted"

def Rep.turn(ended: Bool, +th: String, +ts: String, +at: Nat, +post: M.State, inside: Bool, ds: List<&2, Day>) -> Acc:
  match ended:
    case True{}:
      Rep.end(post, th, ts, at, inside, ds)
    case False{}:
      Acc{post, ds}

# an ask or a merge: the record folded in and counted
def Rep.ask(+cs: List<&2, M.Change>, +post: M.State, +at: Nat, inside: Bool, ds: List<&2, Day>) -> Acc:
  Acc{M.State.replay(cs, post), Rep.count(inside, Rep.kind(1n, NO.Policy.kind(cs)), at, ds)}

def Rep.one(+c: M.Change, +s: M.State, +post: M.State, +from: Nat, +to: Nat, ds: List<&2, Day>) -> Acc:
  match c:
    case M.TurnChanged{+th, +ts, +at}:
      Rep.turn(Bool.and(M.State.running.k(M.State.thread.get(s, th)), Rep.ends(ts)), th, Rep.name(ts), at, post, Rep.in(at, from, to), ds)
    case M.AskOpened{_, +th, _, +tool, +d, _, +at}:
      Rep.ask(NO.Policy.ask(post, at, th, tool, d), post, at, Rep.in(at, from, to), ds)
    case M.ThreadMerged{+th, +url, +at}:
      Rep.ask(NO.Policy.merged(post, at, th, url), post, at, Rep.in(at, from, to), ds)
    case _:
      Acc{post, ds}

def Rep.step(m: Maybe<&2, M.Change>, a: Acc, +from: Nat, +to: Nat) -> Acc:
  match m a:
    case Some{+c} Acc{+s, ds}:
      Rep.one(c, s, M.State.apply(s, c), from, to, ds)
    case None{} _:
      a

def Rep.line(m: Maybe<&2, J.Json>, a: Acc, +from: Nat, +to: Nat) -> Acc:
  match m:
    case Some{+o}:
      Rep.step(C.Change.decode(o), a, from, to)
    case None{}:
      a

def Rep.go(ls: List<&2, String>, a: Acc, +from: Nat, +to: Nat) -> Acc:
  match ls:
    case Nil{}:
      a
    case +l <> t:
      Rep.go(t, Rep.line(J.Json.parse(l), a, from, to), from, to)

# Output
# ------

def Rep.days(ds: List<&2, Day>) -> List<&2, Day>:
  ds

def Rep.pick(k: Nat, +a: Nat, +b: Nat, +c: Nat, +e: Nat, +f: Nat, +g: Nat) -> Nat:
  match k:
    case 0n:
      a
    case 1n:
      b
    case 2n:
      c
    case 3n:
      e
    case 4n:
      f
    case _:
      g

def Rep.sum(ds: List<&2, Day>, +k: Nat) -> Nat:
  match ds:
    case Nil{}:
      0n
    case Day{_, +a, +b, +c, +e, +f, +g} <> t:
      Nat.add(Rep.pick(k, a, b, c, e, f, g), Rep.sum(t, k))

def Rep.zeros(+k: Nat) -> String:
  match k:
    case 0n:
      ""
    case 1n+j:
      "0" ++ Rep.zeros(j)

def Rep.pad(+n: Nat, +w: Nat) -> String:
  Rep.zeros(Nat.sub(w, String.length(Nat.show(n)))) ++ Nat.show(n)

def Rep.date.f(+y: Nat, +d: Nat, +m: Nat) -> String:
  Rep.pad(Bool.pick(Nat, Nat.is_le(m, 2n), Nat.add(y, 1n), y), 4n) ++ "-" ++ Rep.pad(m, 2n) ++ "-" ++ Rep.pad(d, 2n)

def Rep.date.m(+y: Nat, +doy: Nat, +mp: Nat) -> String:
  Rep.date.f(y, Nat.add(Nat.sub(doy, Nat.div(Nat.add(Nat.mul(153n, mp), 2n), 5n)), 1n), Bool.pick(Nat, Nat.is_lt(mp, 10n), Nat.add(mp, 3n), Nat.sub(mp, 9n)))

def Rep.date.d(+y: Nat, +doy: Nat) -> String:
  Rep.date.m(y, doy, Nat.div(Nat.add(Nat.mul(5n, doy), 2n), 153n))

def Rep.date.y(+era: Nat, +doe: Nat, +yoe: Nat) -> String:
  Rep.date.d(Nat.add(yoe, Nat.mul(era, 400n)), Nat.sub(doe, Nat.sub(Nat.add(Nat.mul(365n, yoe), Nat.div(yoe, 4n)), Nat.div(yoe, 100n))))

def Rep.date.e(+era: Nat, +doe: Nat) -> String:
  Rep.date.y(era, doe, Nat.div(Nat.add(Nat.sub(doe, Nat.div(doe, 1460n)), Nat.sub(Nat.div(doe, 36524n), Nat.div(doe, 146096n))), 365n))

def Rep.date.k(+z: Nat) -> String:
  Rep.date.e(Nat.div(z, 146097n), Nat.sub(z, Nat.mul(Nat.div(z, 146097n), 146097n)))

# the date of a day number (days since 1970-01-01), by Hinnant's civil_from_days
def Rep.date(+days: Nat) -> String:
  Rep.date.k(Nat.add(days, 719468n))

def Day.before(d: Day) -> Nat:
  match d:
    case Day{_, a, b, _, _, _, _}:
      Nat.add(a, b)

def Day.after(d: Day) -> Nat:
  match d:
    case Day{_, _, _, c, e, f, g}:
      Nat.add(Nat.add(c, e), Nat.add(f, g))

def Rep.lines(ds: List<&2, Day>) -> IO(Unit):
  match ds:
    case Nil{}:
      IO.pure(Unit, Unit{})
    case +d <> t:
      do IO<Unit>:
        IO.print(Rep.date(Day.of(d)) ++ "  before " ++ Nat.show(Day.before(d)) ++ "  after " ++ Nat.show(Day.after(d)))
        Rep.lines(t)

def Rep.best(more: Bool, +new: Nat, +old: Nat) -> Nat:
  Bool.pick(Nat, more, new, old)

# the day with the most alerts before
def Rep.busy(ds: List<&2, Day>, +best: Nat, +bn: Nat) -> String:
  match ds:
    case Nil{}:
      Rep.date(best) ++ " (before " ++ Nat.show(bn) ++ ")"
    case +d <> t:
      Rep.busy(t, Rep.best(Nat.is_gt(Day.before(d), bn), Day.of(d), best), Rep.best(Nat.is_gt(Day.before(d), bn), Day.before(d), bn))

def Nat.min.small(+a: Nat, +b: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_le(a, b), a, b)

def Rep.permille(+m: Nat) -> String:
  Nat.show(Nat.div(m, 10n)) ++ "." ++ Nat.show(Nat.mod(m, 10n)) ++ "%"

def Rep.percent(+before: Nat, +after: Nat) -> String:
  Bool.pick(String, Nat.is_eq(before, 0n), "n/a",
    Rep.permille(Nat.div(Nat.mul(Nat.sub(before, Nat.min.small(after, before)), 1000n), before)))

def Rep.num(s: String, +acc: Nat) -> Nat:
  match s:
    case SNil{}:
      acc
    case SCon{Chr{+c}, t}:
      Rep.num(t, Nat.add(Nat.mul(acc, 10n), U32.to_nat((c - 48 : U32))))

# the file and the window from the arguments
type Params is Data:
  Params{file: String, from: Nat, to: Nat}

def Rep.p2(f: String, +a: String, t: List<String>) -> Params:
  match t:
    case Nil{}:
      Params{f, Rep.num(a, 0n), 4000000000n}
    case +b <> _:
      Params{f, Rep.num(a, 0n), Rep.num(b, 0n)}

def Rep.p1(f: String, t: List<String>) -> Params:
  match t:
    case Nil{}:
      Params{f, 0n, 4000000000n}
    case +a <> t2:
      Rep.p2(f, a, t2)

def Rep.params(xs: List<String>) -> Params:
  match xs:
    case Nil{}:
      Params{"/dev/null", 0n, 4000000000n}
    case f <> t:
      Rep.p1(f, t)

def Rep.read.done(r: File & Result<&1, &1, U32 & String, List<&2, U32>>) -> IO(List<&2, U32>):
  (f, res) = r
  do IO<List<&2, U32>>:
    File.close(f)
    return Result.default(&1, &1, U32 & String, List<&2, U32>, res, Nil{})

def Rep.read.sized(r: File & Result<&1, &1, U32 & String, U32>) -> IO(List<&2, U32>):
  (f, res) = r
  IO.bind(File & Result<&1, &1, U32 & String, List<&2, U32>>, List<&2, U32>,
    File.read_bytes(f, U32.max(Result.default(&1, &1, U32 & String, U32, res, 0), 1)), Rep.read.done)

def Rep.read.opened(r: Result<&1, &1, U32 & String, File>) -> IO(List<&2, U32>):
  match r:
    case Done{f}:
      IO.bind(File & Result<&1, &1, U32 & String, U32>, List<&2, U32>, File.size(f), Rep.read.sized)
    case Fail{_}:
      IO.pure(List<&2, U32>, Nil{})

def Rep.read(path: String) -> IO(List<&2, U32>):
  IO.bind(Result<&1, &1, U32 & String, File>, List<&2, U32>, File.open(path, "r"), Rep.read.opened)

def Rep.lines.k(r: U.Lines & List<&2, String>) -> List<&2, String>:
  (_, ls) = r
  ls

def Rep.split(bs: List<&2, U32>) -> List<&2, String>:
  Rep.lines.k(U.Lines.feed(U.Lines.new(), bs))

def Rep.days.of(a: Acc) -> List<&2, Day>:
  match a:
    case Acc{_, ds}:
      ds

def Rep.report(+ds: List<&2, Day>) -> IO(Unit):
  do IO<Unit>:
    +bd : Nat = Rep.sum(ds, 0n)
    +bf : Nat = Rep.sum(ds, 1n)
    +aa : Nat = Rep.sum(ds, 2n)
    +ad : Nat = Rep.sum(ds, 3n)
    +af : Nat = Rep.sum(ds, 4n)
    +am : Nat = Rep.sum(ds, 5n)
    +before : Nat = Nat.add(bd, bf)
    +after : Nat = Nat.add(Nat.add(aa, ad), Nat.add(af, am))
    IO.print("desks ignored in both counts; days are UTC")
    IO.print("before total " ++ Nat.show(before) ++ " (done " ++ Nat.show(bd) ++ ", fail " ++ Nat.show(bf) ++ ")")
    IO.print("after total " ++ Nat.show(after) ++ " (ask " ++ Nat.show(aa) ++ ", done " ++ Nat.show(ad) ++ ", fail " ++ Nat.show(af) ++ ", merged " ++ Nat.show(am) ++ ")")
    IO.print("reduction " ++ Rep.percent(before, after))
    IO.print("busiest day " ++ Rep.busy(ds, 0n, 0n))
    Rep.lines(ds)

def Rep.run(p: Params) -> IO(Unit):
  match p:
    case Params{+file, +from, +to}:
      do IO<Unit>:
        +bs : List<&2, U32> <- Rep.read(file)
        +a : Acc = Rep.go(Rep.split(bs), Acc{M.State.empty(), Nil{}}, from, to)
        Rep.report(Rep.days.of(a))

def main() -> IO(Unit):
  do IO<Unit>:
    args : List<String> <- AV.Argv.args()
    Rep.run(Rep.params(args))
