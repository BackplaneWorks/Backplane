import Base
import ../../src/core/model.bend as M
import ../../src/core/json.bend as J
import ../../src/core/codec.bend as C
import ../../src/core/utf8.bend as U
import ../../src/core/hub.bend as H
import ../../src/core/client.bend as UI
import ../../src/mobile/notify.bend as Nt
import ../../src/server/argv.bend as AV

# What does the phones' island list on a real log, before and after a hub
# restart (Hub.recover, then Hub.carry)? Folds EVENTS.jsonl, prints the
# island's lines and, for each thread on it, the work it waits on. It only
# reads the file it is given: run it on a copy with the devices and tokens
# left out, never on the live log:
#   grep -v -E '"(DeviceRegistered|DeviceDropped)"|"push\\.|webhook|token' ~/.backplane-bend/events.jsonl > /tmp/lw-events.jsonl
#   scripts/build-js.sh test/native/lostwork_replay.bend build/lostwork_replay
#   build/lostwork_replay /tmp/lw-events.jsonl

def Lw.fold(m: Maybe<&2, M.Change>, +s: M.State) -> M.State:
  match m:
    case Some{+c}:
      M.State.apply(s, c)
    case None{}:
      s

def Lw.line(m: Maybe<&2, J.Json>, +s: M.State) -> M.State:
  match m:
    case Some{+o}:
      Lw.fold(C.Change.decode(o), s)
    case None{}:
      s

def Lw.go(ls: List<&2, String>, +s: M.State) -> M.State:
  match ls:
    case Nil{}:
      s
    case +l <> t:
      Lw.go(t, Lw.line(J.Json.parse(l), s))

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

def Lw.island(+s: M.State) -> List<&2, J.Json>:
  Nt.Note.lines.w(UI.Ui.threads(s), M.Work.kids.map(s), s)

def Lw.rows(ls: List<&2, String>, +tag: String) -> String:
  match ls:
    case Nil{}:
      ""
    case +l <> t:
      "\n      " ++ tag ++ " " ++ M.Bg.field(l, 0n) ++ " [" ++ M.Bg.kind.of(l) ++ "] " ++ M.Work.row.title(l)
        ++ Bool.pick(String, M.Bg.ended(l), " (ended: " ++ M.Bg.status(l) ++ ")", "") ++ Lw.rows(t, tag)

def Lw.show.k(+s: M.State, +th: String, +title: String, +doing: String) -> String:
  th ++ " | " ++ title ++ " | " ++ doing ++ " | kids " ++ Nat.show(M.Work.kids(s, th)) ++ Lw.rows(M.Bg.of(s, th), "bg") ++ Lw.rows(M.Work.watch(s, th), "watch")

def Lw.show(+s: M.State, +o: J.Json) -> String:
  Lw.show.k(s, J.Json.get_str(o, "thread", ""), J.Json.get_str(o, "title", ""), J.Json.get_str(o, "doing", ""))

def Lw.shows(os: List<&2, J.Json>, +s: M.State) -> IO(Unit):
  match os:
    case Nil{}:
      IO.pure(Unit, Unit{})
    case +o <> t:
      do IO<Unit>:
        IO.print("  " ++ Lw.show(s, o))
        Lw.shows(t, s)

def Lw.report(+tag: String, +s: M.State) -> IO(Unit):
  do IO<Unit>:
    IO.print(tag ++ ": island lists " ++ Nat.show(List.length(&2, J.Json, Lw.island(s))))
    Lw.shows(Lw.island(s), s)

def Lw.num(s: String, +acc: Nat) -> Nat:
  match s:
    case SNil{}:
      acc
    case SCon{Chr{+c}, t}:
      Lw.num(t, Nat.add(Nat.mul(acc, 10n), U32.to_nat((c - 48 : U32))))

def Lw.now.k(xs: List<String>) -> Nat:
  match xs:
    case Nil{}:
      1791990000n
    case a <> _:
      Lw.num(a, 0n)

def Lw.file(xs: List<String>) -> String:
  match xs:
    case Nil{}:
      "/dev/null"
    case f <> _:
      f

def Lw.run(+file: String, +now: Nat) -> IO(Unit):
  do IO<Unit>:
    +bs : List<&2, U32> <- Rep.read(file)
    +s : M.State = Lw.go(Rep.split(bs), M.State.empty())
    IO.print("restart.continue: " ++ Bool.show(M.Carry.on(s)))
    Lw.report("before", s)
    +after : M.State = H.Carry.after(s, now)
    Lw.report("after recover", after)

def main() -> IO(Unit):
  do IO<Unit>:
    args : List<String> <- AV.Argv.args()
    Lw.run(Lw.file(args), 1791990000n)
