import Base
import ../../src/core/model.bend as M
import ../../src/core/thin-pace.bend as P
import ./kcommon.bend as C

# Broadcast kernel: the real Touch.change / Touch.adds / Touch.level.for /
# Pace.step of src/core/thin-pace.bend over a generated event stream, four
# views. Mirrors test/gpui/bend/ts/pace.ts.

def Ev.thread(+r: Nat) -> String:
  C.K.id("t", C.K.mod(C.K.div(r, 1024n), 64n))

def Ev.other(+r: Nat) -> String:
  C.K.id("t", C.K.mod(C.K.div(r, 65536n), 64n))

def Ev.msg(+r: Nat, +i: Nat) -> String:
  Bool.pick(String, C.K.is(C.K.mod(r, 5n), 0n), "t:" ++ Ev.other(r) ++ ":" ++ Nat.show(i), C.K.id("m", i))

def Ev.setting(+r: Nat, +i: Nat, +k: Nat) -> M.Change:
  match k:
    case 0n: M.SettingSet{"liveness." ++ Ev.thread(r), "x", i}
    case 1n: M.SettingSet{"autosettle.quiet." ++ Ev.thread(r), "x", i}
    case 2n: M.SettingSet{C.K.id("launch.", i), "x", i}
    case 1n+1n+1n+_: M.SettingSet{"ui.theme", "dark", i}

def Ev.of.k(+k: Nat, +r: Nat, +i: Nat) -> M.Change:
  match k:
    case 0n: M.MessagePosted{Ev.thread(r), Ev.msg(r, i), M.User{}, "hello", i}
    case 1n: M.TurnChanged{Ev.thread(r), M.Running{}, i}
    case 2n: M.ActivityLogged{Ev.thread(r), C.K.id("a", i), "tool", "bash", "ls", i}
    case 3n: Ev.setting(r, i, 0n)
    case 4n: Ev.setting(r, i, 1n)
    case 5n: Ev.setting(r, i, 2n)
    case 6n: Ev.setting(r, i, 3n)
    case 7n: M.TaskDelegated{C.K.id("k", i), Ev.thread(r), Ev.other(r), 1n, "claude", "", i}
    case 8n: M.PendingChanged{Ev.thread(r), 1n, i}
    case 9n: M.ThreadForked{Ev.thread(r), Ev.other(r), i}
    case 10n: M.ThreadSettled{Ev.thread(r), False{}, i}
    case 11n+_: M.RoomPosted{C.K.id("p", i), "room", "bot", "hi", 0n, i}

def Ev.of(+i: Nat) -> M.Change:
  Ev.of.k(C.K.mod(C.K.div(C.K.h(i), 16n), 12n), C.K.h(i), i)

# events i .. i+n
def Ev.gen(n: Nat, +i: Nat, acc: List<&2, M.Change>) -> List<&2, M.Change>:
  match n:
    case 0n: List.reverse(&2, M.Change, acc)
    case 1n+k: Ev.gen(k, Nat.add(i, 1n), Ev.of(i) <> acc)

type Fold is Data:
  Fold{acc: Nat, pace: P.Pace}

def Fold.new() -> Fold:
  Fold{0n, P.Pace.fresh()}

def Fold.last(p: P.Pace, +acc: Nat, +level: Nat, +s: Nat, +w: Nat) -> Fold:
  match p:
    case P.Pace{+last, +owed}:
      Fold{C.K.mix(acc, Nat.add(Nat.add(Nat.add(level, Nat.mul(3n, s)), Nat.add(Nat.mul(7n, w), Nat.mul(11n, last))), Nat.mul(13n, owed))), P.Pace{last, owed}}

def Fold.of(pc: P.Paced, +acc: Nat, +level: Nat) -> Fold:
  match pc:
    case P.Paced{s, p, +w}: Fold.last(p, acc, level, C.K.b(s), w)

def Fold.pace(f: Fold, +now: Nat, +level: Nat) -> Fold:
  match f:
    case Fold{+acc, p}: Fold.of(P.Pace.step(p, now, level), acc, level)

def Fold.acc(f: Fold) -> Nat:
  match f:
    case Fold{a, _}: a

type Four is Data:
  Four{a: Fold, b: Fold, c: Fold, d: Fold}

def View.th(+v: Nat) -> String:
  C.K.id("t", v)

def View.cid(+v: Nat) -> String:
  Nat.show(Nat.add(v, 1n))

def Step.one(+t: P.Touch, +v: Nat, +now: Nat, f: Fold) -> Fold:
  Fold.pace(f, now, P.Touch.level.for(t, View.th(v), View.cid(v)))

def Step.all(+t: P.Touch, +now: Nat, f: Four) -> Four:
  match f:
    case Four{a, b, c, d}:
      Four{Step.one(t, 0n, now, a), Step.one(t, 1n, now, b), Step.one(t, 2n, now, c), Step.one(t, 3n, now, d)}

# the events as the hub sees them: touch, then each view's pace
def Run.loop(evs: List<&2, M.Change>, +i: Nat, f: Four) -> Four:
  match evs:
    case Nil{}: f
    case e <> rest:
      Run.loop(rest, Nat.add(i, 1n), Step.all(P.Touch.adds(P.Touch.change(e), P.Touch.none()), Nat.mul(i, 13n), f))

def Run.show(f: Four) -> String:
  match f:
    case Four{a, b, c, d}:
      Nat.show(Fold.acc(a)) ++ " " ++ Nat.show(Fold.acc(b)) ++ " " ++ Nat.show(Fold.acc(c)) ++ " " ++ Nat.show(Fold.acc(d))

def Run.reps(k: Nat, +evs: List<&2, M.Change>, +n: Nat) -> IO(Unit):
  match k:
    case 0n: IO.pure(Unit, Unit{})
    case 1n+rest:
      do IO<Unit>:
        t0 : Nat <- IO.now()
        +out : String = Run.show(Run.loop(evs, 0n, Four{Fold.new(), Fold.new(), Fold.new(), Fold.new()}))
        IO.print("digest " ++ out)
        t1 : Nat <- IO.now()
        IO.print("ms " ++ Nat.show(Nat.sub(t1, t0)))
        Run.reps(rest, evs, n)

def main() -> IO(Unit):
  do IO<Unit>:
    +a : C.Args <- C.K.args()
    +n : Nat = C.K.Args.a(a)
    +r : Nat = C.K.Args.b(a)
    t0 : Nat <- IO.now()
    +evs : List<&2, M.Change> = Ev.gen(n, 0n, Nil{})
    +len : Nat = List.length(&2, M.Change, evs)
    t1 : Nat <- IO.now()
    IO.print("events " ++ Nat.show(len))
    IO.print("gen_ms " ++ Nat.show(Nat.sub(t1, t0)))
    Run.reps(r, evs, n)
