import Base
import ../../src/core/json.bend as J
import ../../src/core/model.bend as M
import ../../src/core/client.bend as U
import ../../src/core/autosettle.bend as A
import ../../src/core/codec.bend as C
import ../../src/core/watchdog.bend as W
import ../../src/core/hub.bend as B

# The autosettle probe (state lane): the same fold as digest.bend, then Auto.changes at a time, printed as a JSON array of events.
#   autosettle_probe NOW CHUNK1 [CHUNK2 ...]
# The differential oracle: events.jsonl through Backplane's own State.replay, printed as a
# canonical state JSON. Rust prints the same shape from its fold (see
# test/gpui/parity/oracle/DIGEST.md); the harness sorts both and compares SHA-256.
#   digest CHUNK1 [CHUNK2 ...]   (files of whole lines, folded in order)
# Base IO only.

# ---- reading ----

def Dg.item(m: Maybe<&2, J.Json>, acc: List<&2, J.Json>) -> List<&2, J.Json>:
  match m:
    case None{}: acc
    case Some{j}: j <> acc

def Dg.items(xs: List<&2, String>, acc: List<&2, J.Json>) -> List<&2, J.Json>:
  match xs:
    case Nil{}: List.reverse(&2, J.Json, acc)
    case x <> t: Dg.items(t, Dg.item(J.Json.parse(x), acc))

def Dg.apply.d(+d: U.Decoded, +st: M.State) -> M.State:
  match d:
    case U.Decoded{cs, _}: M.State.replay(List.reverse(&2, M.Change, cs), st)

def Dg.apply(+st: M.State, js: List<&2, J.Json>) -> M.State:
  Dg.apply.d(U.Ui.decode.all(js, U.Decoded{Nil{}, 0n}), st)

def Dg.read.k(r: File & Result<&1, &1, U32 & String, String>) -> IO(String):
  (f, res) = r
  do IO<String>:
    File.close(f)
    s : String <- IO.pass(String, res)
    return s

def Dg.read(+path: String) -> IO(String):
  do IO<String>:
    f : File <- IO.try(File, File.open(path, "r"))
    r : File & Result<&1, &1, U32 & String, String> <- File.read(f, 2000000000)
    Dg.read.k(r)

# the files folded in the order given (the harness cuts a big log on line boundaries)
def Dg.files(+paths: List<&2, String>, +st: M.State) -> IO(M.State):
  match paths:
    case Nil{}: IO.pure(M.State, st)
    case p <> t:
      do IO<M.State>:
        text : String <- Dg.read(p)
        Dg.files(t, Dg.apply(st, Dg.items(String.lines(text), Nil{})))

def Dg.copy(xs: List<&1, String>) -> List<&2, String>:
  match xs:
    case Nil{}: Nil{}
    case x <> t: x <> Dg.copy(t)

def Ap.enc(cs: List<&2, M.Change>) -> List<&2, J.Json>:
  match cs:
    case Nil{}: Nil{}
    case c <> t: C.Change.encode(c) <> Ap.enc(t)

def Ap.batch.d(+d: U.Decoded) -> List<&2, M.Change>:
  match d:
    case U.Decoded{cs, _}: List.reverse(&2, M.Change, cs)

# the batch file's changes (a file of event lines; "none" is no batch)
def Ap.batch.k(none: Bool, +path: String) -> IO(List<&2, M.Change>):
  match none:
    case True{}: IO.pure(List<&2, M.Change>, Nil{})
    case False{}:
      do IO<List<&2, M.Change>>:
        text : String <- Dg.read(path)
        IO.pure(List<&2, M.Change>, Ap.batch.d(U.Ui.decode.all(Dg.items(String.lines(text), Nil{}), U.Decoded{Nil{}, 0n})))

def Ap.batch(+path: String) -> IO(List<&2, M.Change>):
  Ap.batch.k(String.eq(path, "none"), path)

# Watchdog.after: the batch followed by the liveness progress it adds; only the added part is printed
def Ap.live.k(+out: List<&2, M.Change>, +n: Nat) -> J.Json:
  J.Json.arr(Ap.enc(List.drop(&2, M.Change, out, n)))

def Ap.live.step(r: B.Step, +n: Nat) -> J.Json:
  match r:
    case B.Step{out, _}: Ap.live.k(out, n)

def Ap.live(+st: M.State, +now: Nat, +cs: List<&2, M.Change>) -> J.Json:
  Ap.live.step(W.Watchdog.after(st, now, B.Step{cs, Nil{}}), List.length(&2, M.Change, cs))

# Ended.out then Alerts.changes (the production order): the summary clock, the alert record and the told marks a turn's end writes. The
# first TurnChanged's thread is the ended one; only the added part is printed
# the agents commit a failed end's mark (failed.<msg>) in the batch; Bend writes it itself after the alert (Stamp.fail.mark), so it is left out here
def Ap.nofail(cs: List<&2, M.Change>) -> List<&2, M.Change>:
  match cs:
    case Nil{}: Nil{}
    case M.SettingSet{+k, v, at} <> t: Bool.pick(List<&2, M.Change>, String.starts_with(k, "failed."), Ap.nofail(t), M.SettingSet{k, v, at} <> Ap.nofail(t))
    case c <> t: c <> Ap.nofail(t)

def Ap.end.th(cs: List<&2, M.Change>) -> String:
  match cs:
    case Nil{}: ""
    case M.TurnChanged{+th, _, _} <> _: th
    case _ <> t: Ap.end.th(t)

def Ap.end.k(r: B.Step, +st: M.State, +n: Nat) -> J.Json:
  match r:
    case B.Step{cs2, _}: Ap.live.k(B.Alerts.changes(st, cs2), n)

def Ap.end(+st: M.State, +now: Nat, +cs: List<&2, M.Change>) -> J.Json:
  Ap.end.k(B.Ended.out(st, now, Ap.end.th(cs), B.Step{cs, Nil{}}), st, List.length(&2, M.Change, cs))

def Ap.out(+st: M.State, +now: Nat, +cs: List<&2, M.Change>) -> J.Json:
  J.Json.obj([J.KV{"tick", J.Json.arr(Ap.enc(A.Auto.changes(st, now)))},
    J.KV{"meta", J.Json.arr(Ap.enc(A.Auto.messages.metadata(cs, st)))},
    J.KV{"quiet", J.Json.arr(Ap.enc(A.Auto.quiet.changes(Map.to_list(&2, Nat, A.Auto.quiet.collect(cs, st, Map.new(&2, Nat))), st)))},
    J.KV{"live", Ap.live(st, now, cs)},
    J.KV{"end", Ap.end(st, now, Ap.nofail(cs))},
    J.KV{"wakes", J.Json.arr(Ap.enc(B.Shelve.wakes(B.Shelve.tick.post(st, now, M.Work.kids.map(st)))))}])

def main() -> IO(Unit):
  do IO<Unit>:
    a : List<&1, String> <- IO.args()
    +xs : List<&2, String> = Dg.copy(a)
    +now : Nat = M.Switch.num(Maybe.default(&2, String, List.head(&2, String, List.drop(&2, String, xs, 1n)), "0"), 0n)
    cs : List<&2, M.Change> <- Ap.batch(Maybe.default(&2, String, List.head(&2, String, List.drop(&2, String, xs, 2n)), "none"))
    st : M.State <- Dg.files(List.drop(&2, String, xs, 3n), M.State.empty())
    IO.print(J.Json.show(Ap.out(st, now, cs)))
