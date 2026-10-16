import Base
import ../../src/core/client.bend as U
import ../../src/core/model.bend as M
import ../../src/core/deploy.bend as DP
import ../../src/app/nui.bend as N
import ../../src/app/layout.bend as L
import ../../src/app/bots.bend as BV
import ../../src/app/cats.bend as CA
import ../../src/app/theme.bend as T
import ../../src/gfx/raster.bend as R
import ../../src/gfx/draw.bend as D
import ../../src/server/argv.bend as AV

# The bots' screens with no window: a made-up log (a project, four bots,
# two rooms, posts, memory, routines, webhooks, a space), each screen as a
# PPM in DIR (default /tmp): bots-<name>.ppm. Also times a cat frame.
#   scripts/build-app.sh test/native/bots_snap.bend build/bots_snap
#   build/bots_snap [DIR]

# The uberbot: the sidebar's row and the Status tab (a made-up log with N
# threads in every state, one bot promoted), as PPMs and frame times.
#   scripts/build-app.sh test/native/uber_snap.bend build/uber_snap
#   build/uber_snap [DIR [THREADS]]

def at() -> Nat:
  1790000000n

def bot(+i: Nat, +name: String, +look: Nat, +persona: String) -> List<&2, M.Change>:
  [M.ThreadCreated{"bt" ++ Nat.show(i), "", name, M.Local{}, "", "", "claude", "", at()},
   M.BotSet{"b" ++ Nat.show(i), name, "bt" ++ Nat.show(i), look, persona, at()}]

def state(+id: String, +i: Nat) -> List<&2, M.Change>:
  match i:
    case 0n:
      [M.TurnChanged{id, M.Running{}, Nat.add(at(), 20n)}, M.AskOpened{"a" ++ id, id, "approval", "Bash", "make fab", "", Nat.add(at(), 21n)}]
    case 1n:
      [M.TurnChanged{id, M.Running{}, Nat.add(at(), 20n)}, M.AskOpened{"a" ++ id, id, "input", "", "Which stackup?", "", Nat.add(at(), 21n)}]
    case 2n:
      [M.TurnChanged{id, M.Failed{}, Nat.add(at(), 30n)}]
    case 3n:
      [M.TurnChanged{id, M.Running{}, Nat.add(at(), 40n)}]
    case 4n:
      [M.TurnChanged{id, M.Completed{}, Nat.add(at(), 50n)}]
    case _:
      Nil{}

def threads(+k: Nat) -> List<&2, M.Change>:
  match k:
    case 0n:
      Nil{}
    case 1n+j:
      +id = {"t" ++ Nat.show(j) : String}
      List.concat(&2, M.Change, [[M.ThreadCreated{id, Bool.pick(String, Nat.is_eq(Nat.mod(j, 2n), 0n), "p1", "p2"), "Thread " ++ Nat.show(j) ++ " " ++ Bool.pick(String, Nat.is_eq(Nat.mod(j, 3n), 0n), "regulator enable pin", "layout review"), M.Local{}, "", "", "claude", "", Nat.add(at(), Nat.mul(j, 2n))}],
        state(id, Nat.mod(j, 6n)), threads(j)])

def log(+k: Nat) -> List<&2, M.Change>:
  List.concat(&2, M.Change, [
    [M.ProjectCreated{"p1", "board", "/tmp/board", at()}, M.ProjectCreated{"p2", "sensor", "/tmp/sensor", at()}],
    threads(k),
    bot(0n, "Mochi", 3n, "Chief of staff."), bot(1n, "Biscuit", 20n, "Keeps the calendar."), bot(2n, "Tofu", 41n, "Reviews schematics."),
    [M.TurnChanged{"bt1", M.Running{}, Nat.add(at(), 60n)}, M.TurnChanged{"bt2", M.Running{}, Nat.add(at(), 61n)},
     M.RoomSet{"r0", "general", "b0\nb1\nb2", at()}, M.RoomSet{"r1", "boards", "b1\nb2", at()},
     M.RoomPosted{"p0", "r0", "b1", "Fab queue is clear, the power board ships Friday.", 0n, Nat.add(at(), 10n)},
     M.RoomPosted{"p1", "r1", "b2", "Rev C schematic reviewed.", 0n, Nat.add(at(), 40n)},
     M.RoutineSet{"o0", "b1", "Morning summary", "0 9 * * 1-5", "Summarize open PRs.", True{}, at()},
     M.RoutineSet{"o1", "b2", "Weekly cleanup", "@weekly", "Close stale branches.", True{}, at()},
     M.UberSet{"b0", "u1-x", at()}]])

# this machine (mA) has the uberbot and machine mB holds its conversation
def homelog() -> List<&2, M.Change>:
  [M.SettingSet{"machine.id", "mA", at()}, M.UberOp{"u1-x", "mB", 1n, "0000001790.000000.mB", "{\"k\":\"lease\",\"holder\":\"mB\",\"term\":2}", at()}]

# a deploy waiting (core/deploy.bend): the plane's "Restart pending" row
def deploylog() -> List<&2, M.Change>:
  [M.SettingSet{"deploy.wait", DP.Deploy.line("", "dev.sh install", "/tmp/stage", "0.1.0", at(), False{}, True{}, ""), at()}]

def act.k(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}:
      u

def ops(o: L.Out) -> List<&2, R.Op>:
  match o:
    case L.Out{os, _}:
      os

def base(+k: Nat) -> U.Ui:
  U.Ui.fold(log(k), U.Ui.init(Nat.add(at(), 100n)))

def sized(n: N.Nui, +w: U32, +h: U32) -> N.Nui:
  match n:
    case N.Nui{u, _, _, mx, my, sd, sc, ca, fo, pa, hi, ms, vw}:
      N.Nui{u, w, h, mx, my, sd, sc, ca, fo, pa, hi, ms, vw}

def write.k(r: File & Result<&1, &1, U32 & String, Unit>) -> IO(Unit):
  (f, _) = r
  File.close(f)

def write(+path: String, +w: U32, +h: U32, +img: Image) -> IO(Unit):
  do IO<Unit>:
    g : File <- IO.try(File, File.open(path, "w"))
    r : File & Result<&1, &1, U32 & String, Unit> <- File.write_bytes(g, D.Snap.ppm(img, w, h))
    write.k(r)

def frame(+ty: L.Ty, +n: N.Nui) -> L.Out:
  BV.Bv.with_cats(BV.Bv.frame.w(ty, L.Lay.memo(ty, n, L.TL.memo.none()), BV.Bv.warm(N.Nui.ui(n), CA.Cats.none(), 1234n), BV.Bv.wraps(ty, n, BV.Bv.wraps.none()), n), BV.Bv.warm(N.Nui.ui(n), CA.Cats.none(), 1234n), 1234n)

def shot(+ty: L.Ty, +dir: String, +name: String, +u: U.Ui, +w: U32, +h: U32) -> IO(Unit):
  +n = sized(N.Nui.init(u, w, h), w, h)
  +os = ops(frame(ty, n))
  do IO<Unit>:
    write(dir ++ "/win-" ++ name ++ ".ppm", w, h, R.Frame.render(w, h, R.Res.ops(os, L.Ty.tab(ty)), T.Th.bg()))
    IO.print(dir ++ "/win-" ++ name ++ ".ppm")

def arg(xs: List<&2, String>, +i: Nat, d: String) -> String:
  match xs i:
    case Nil{} _:
      d
    case h <> _ 0n:
      h
    case _ <> t 1n+k:
      arg(t, k, d)

def copy(xs: List<&1, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case h <> t:
      String.append(h, "") <> copy(t)

def timed(+k: Nat, +ty: L.Ty, +n: N.Nui, +acc: Nat) -> Nat:
  match k:
    case 0n:
      acc
    case 1n+j:
      timed(j, ty, N.Nui.with_ui(n, U.Ui.info_set(N.Nui.ui(n), "bench", Nat.show(j))), Nat.add(acc, List.length(&2, R.Op, ops(frame(ty, n)))))

def main() -> IO(Unit):
  do IO<Unit>:
    args : List<String> <- AV.Argv.args()
    +as : List<&2, String> = copy(args)
    +dir : String = arg(as, 0n, "/tmp")
    +k : Nat = Maybe.default(&2, Nat, Nat.read(arg(as, 1n, "40")), 40n)
    +ty : L.Ty <- L.Ty.load()
    +u : U.Ui = base(k)
    +us : U.Ui = act.k(U.Ui.act(u, "uber", ""))
    shot(ty, dir, "side", u, 1400, 900)
    shot(ty, dir, "status", us, 1400, 900)
    shot(ty, dir, "status-narrow", us, 900, 800)
    shot(ty, dir, "status-deploy", U.Ui.fold(deploylog(), us), 1400, 900)
    shot(ty, dir, "status-home", U.Ui.fold(homelog(), us), 1400, 900)
    +t0 : Nat <- IO.now()
    +c : Nat = timed(5n, ty, N.Nui.init(us, 1400, 900), 0n)
    +t1 : Nat <- IO.now()
    IO.print(Nat.show(k) ++ " threads: status frame (plane built each time) " ++ Nat.show(Nat.div(Nat.sub(t1, t0), 5n)) ++ " ms (" ++ Nat.show(c) ++ " ops)")
