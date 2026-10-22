import Base
import ../../src/core/client.bend as U
import ../../src/core/model.bend as M
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
#   scripts/build-js.sh test/native/bots_snap.bend build/bots_snap
#   build/bots_snap [DIR]

def at() -> Nat:
  1790000000n

def bot(+i: Nat, +name: String, +look: Nat, +persona: String) -> List<&2, M.Change>:
  [M.ThreadCreated{"bt" ++ Nat.show(i), "", name, M.Local{}, "", "", "claude", "", at()},
   M.BotSet{"b" ++ Nat.show(i), name, "bt" ++ Nat.show(i), look, persona, at()}]

def space() -> String:
  "{\"title\":\"Mochi's desk\",\"blocks\":[{\"type\":\"heading\",\"text\":\"Today\"},{\"type\":\"row\",\"blocks\":[{\"type\":\"stat\",\"label\":\"Open PRs\",\"value\":\"3\",\"hint\":\"2 need review\"},{\"type\":\"stat\",\"label\":\"Build\",\"value\":\"green\"},{\"type\":\"stat\",\"label\":\"Boards\",\"value\":\"12\",\"hint\":\"+2 this week\"}]},{\"type\":\"progress\",\"label\":\"Release 0.9\",\"value\":0.64},{\"type\":\"text\",\"text\":\"I watch the repo and the fab queue. Ask me for a summary any time.\"},{\"type\":\"list\",\"items\":[\"Review power board rev C\",\"Order stencils\",\"Reply to JLC about the panel\"]},{\"type\":\"kv\",\"rows\":[[\"Fab\",\"JLCPCB\"],[\"Next check\",\"09:00\"]]},{\"type\":\"table\",\"head\":[\"Board\",\"Rev\",\"State\"],\"rows\":[[\"power\",\"C\",\"fab\"],[\"sensor\",\"B\",\"layout\"]]},{\"type\":\"divider\"},{\"type\":\"row\",\"blocks\":[{\"type\":\"button\",\"label\":\"Summarize\",\"action\":\"summary\"},{\"type\":\"button\",\"label\":\"Check fab\",\"action\":\"fab\"}]},{\"type\":\"input\",\"label\":\"Ask Mochi\",\"action\":\"ask\",\"placeholder\":\"What should I look at?\"},{\"type\":\"badge\",\"text\":\"all good\",\"tone\":\"ok\"},{\"type\":\"code\",\"text\":\"make fab\\nmake check\"}]}"

def task() -> String:
  "{\"v\":1,\"slug\":\"jlcpcb-status\",\"name\":\"Check JLCPCB order status\",\"description\":\"Look up where an order is in production.\",\"origin\":\"show\",\"start\":\"https://jlcpcb.com\",\"params\":[{\"name\":\"order\",\"label\":\"Order number\",\"kind\":\"text\",\"example\":\"W2026\"}],\"steps\":[],\"urls\":[],\"created\":1,\"updated\":2}"

def run(+status: String) -> String:
  "{\"slug\":\"jlcpcb-status\",\"run\":\"r1\",\"status\":\"" ++ status ++ "\",\"n\":5,\"summary\":\"Order W2026 is in production, ships Thursday.\",\"shots\":[\"/bots/b0/tasks/shot?run=r1&n=2\",\"/bots/b0/tasks/shot?run=r1&n=5\"]}"

def pause() -> String:
  "{\"bot\":\"b0\",\"run\":\"r2\",\"slug\":\"jlcpcb-status\",\"n\":3,\"kind\":\"login\",\"question\":\"JLCPCB wants me to sign in. Sign in yourself, or tell me to skip.\",\"risky\":false,\"shot\":\"/bots/b0/tasks/shot?run=r2&n=3\"}"

def draft() -> String:
  "[{\"n\":1,\"do\":\"navigate\",\"url\":\"https://jlcpcb.com/user-center\"},{\"n\":2,\"do\":\"type\",\"target\":{\"role\":\"textbox\",\"label\":\"Order number\"},\"value\":\"W2026\"},{\"n\":3,\"do\":\"type\",\"target\":{\"label\":\"Password\"},\"value\":{\"secret\":\"Password\"}},{\"n\":4,\"do\":\"click\",\"target\":{\"role\":\"button\",\"name\":\"Search\"}},{\"n\":5,\"do\":\"note\",\"note\":\"use the second tab\"}]"

def log() -> List<&2, M.Change>:
  List.concat(&2, M.Change, [
    [M.ProjectCreated{"p1", "board", "/tmp/board", at()}, M.ThreadCreated{"t0", "p1", "Power sequencing", M.Local{}, "", "", "claude", "", at()},
     M.MessagePosted{"t0", "m0", M.User{}, "Check the regulator's enable pin.", at()}],
    bot(0n, "Mochi", 3n, "Watches the repo and the fab queue."), bot(1n, "Biscuit", 20n, "Keeps the calendar."),
    bot(2n, "Tofu", 41n, "Reviews schematics."), bot(3n, "Pixel", 58n, "Draws renders."),
    [M.RoomSet{"r0", "general", "b0\nb1\nb2", at()}, M.RoomSet{"r1", "boards", "b1\nb3", at()},
     M.RoomPosted{"p0", "r0", "b0", "Fab queue is clear, the power board ships Friday.", 0n, Nat.add(at(), 10n)},
     M.RoomPosted{"p1", "r0", "b1", "Noted. I moved the design review to Thursday 10:00 so it lands before the ship date.", 1n, Nat.add(at(), 40n)},
     M.RoomPosted{"p2", "r0", "you", "Thanks both.", 0n, Nat.add(at(), 60n)},
     M.MemorySaved{"b0", "fab", "fact", "Boards go to JLCPCB; stencils from OSH Stencils.", "fab,vendors", at()},
     M.MemorySaved{"b0", "reviews", "pref", "Wants a summary of open PRs every morning, short, no emoji.", "", at()},
     M.RoutineSet{"o0", "b0", "Morning summary", "0 9 * * 1-5", "Summarize open PRs and the fab queue.", True{}, at()},
     M.RoutineSet{"o1", "b0", "Weekly cleanup", "@weekly", "Close stale branches.", False{}, at()},
     M.SettingSet{"tz.rule.Europe/Helsinki", "EET-2EEST,M3.5.0/3,M10.5.0/4", at()}, M.SettingSet{"person.tz", "Europe/Helsinki", at()},
     M.RoutinePlan{"o0", 0n, "person", "", at()},
     M.RoutineSet{"o2", "b0", "Check the stencil order", "", "Ask OSH Stencils for the tracking number.", True{}, at()}, M.RoutinePlan{"o2", Nat.add(at(), 90000n), "person", "", at()},
     M.RoutineSet{"o3", "b0", "Call the fab about the panel", "", "Call the fab about the panel", True{}, at()}, M.RoutinePlan{"o3", Nat.add(at(), 7200n), "person", "remind", at()},
     M.RoutineSet{"o4", "b0", "Stand up", "", "Stand up", True{}, at()}, M.RoutinePlan{"o4", Nat.add(at(), 60n), "person", "remind", at()},
     M.RoutineFired{"o4", Nat.add(at(), 60n)}, M.RoutineDone{"o4", Nat.add(at(), 60n)},
     M.HookSet{"h0", "b0", "GitHub pushes", False{}, at()},
     M.SpaceSet{"b0", space(), at()},
     M.TaskSet{"b0", "jlcpcb-status", task(), at()},
     M.ActivityLogged{"bt0", "r1:a", "task", "run", run("running"), Nat.add(at(), 20n)},
     M.ActivityLogged{"bt0", "r1:b", "task", "run", run("ok"), Nat.add(at(), 30n)},
     M.AskOpened{"k1", "bt0", "browser", "task_pause", pause(), "continue\nskip\nstop", Nat.add(at(), 50n)}]])

def act.k(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}:
      u

# "!key=value" sets the hub's info (drive.b0, teach.b0, ...)
def step.i(info: Bool, +u: U.Ui, +k: String, +v: String) -> U.Ui:
  Bool.pick(U.Ui, info, U.Ui.info_set(u, String.drop(k, 1n), v), act.k(U.Ui.act(u, k, v)))

# a step: "action=value", or "@key=value" to set the scratch (what a hub's
# answer would, e.g. the Google accounts); "|" in a value is a line break
def step.k(scratch: Bool, +p: String, +u: U.Ui) -> U.Ui:
  +k = String.join(List.take(&2, String, String.split(p, '='), 1n), "")
  +v = String.join(String.split(String.join(List.drop(&2, String, String.split(p, '='), 1n), "="), '|'), "\n")
  Bool.pick(U.Ui, scratch, U.Ui.scratch_set(u, k, v), step.i(String.starts_with(p, "!"), u, k, v))

def acts(ps: List<&2, String>, u: U.Ui) -> U.Ui:
  match ps:
    case Nil{}:
      u
    case +p <> t:
      acts(t, step.k(String.starts_with(p, "@"), p, u))

def ops(o: L.Out) -> List<&2, R.Op>:
  match o:
    case L.Out{os, _}:
      os

def with_ms(n: N.Nui, +ms: Nat) -> N.Nui:
  match n:
    case N.Nui{u, w, h, mx, my, sd, sc, ca, fo, pa, hi, _, vw}:
      N.Nui{u, w, h, mx, my, sd, sc, ca, fo, pa, hi, ms, vw}

def with_focus(n: N.Nui, fo: String) -> N.Nui:
  match n:
    case N.Nui{u, w, h, mx, my, sd, sc, ca, _, pa, hi, ms, vw}:
      N.Nui{u, w, h, mx, my, sd, sc, ca, fo, pa, hi, ms, vw}

def info(u: U.Ui) -> U.Ui:
  U.Ui.info_set(U.Ui.info_set(U.Ui.info_set(U.Ui.info_set(U.Ui.info_set(U.Ui.info_set(u, "shot.ok.b0", "shot:b0:3"), "img:shot:b0:3", "1280x800"), "listen", "127.0.0.1:3787"),
    "img:tshot:3787:/bots/b0/tasks/shot?run=r1&n=2", "1280x800"), "img:tshot:3787:/bots/b0/tasks/shot?run=r1&n=5", "1280x800"), "img:tshot:3787:/bots/b0/tasks/shot?run=r2&n=3", "1280x800")

def base() -> U.Ui:
  info(act.k(U.Ui.act(U.Ui.fold(log(), U.Ui.init(Nat.add(at(), 100n))), "select", "t0")))

def write.k(r: File & Result<&1, &1, U32 & String, Unit>) -> IO(Unit):
  (f, _) = r
  File.close(f)

def write(+path: String, +img: Image) -> IO(Unit):
  do IO<Unit>:
    g : File <- IO.try(File, File.open(path, "w"))
    w : File & Result<&1, &1, U32 & String, Unit> <- File.write_bytes(g, D.Snap.ppm(img, 1400, 900))
    write.k(w)

# a screen after the steps ("action=value" apart by ;), with a field focused
def shot(+ty: L.Ty, +dir: String, +name: String, +steps: String, +focus: String) -> IO(Unit):
  +u = acts(String.split(steps, ';'), base())
  +n = with_focus(with_ms(N.Nui.init(u, 1400, 900), 1234n), focus)
  +os = ops(BV.Bv.with_cats(BV.Bv.frame(ty, L.Lay.memo(ty, n, L.TL.memo.none()), BV.Bv.warm(u, CA.Cats.none(), 1234n), n), BV.Bv.warm(u, CA.Cats.none(), 1234n), 1234n))
  do IO<Unit>:
    write(dir ++ "/bots-" ++ name ++ ".ppm", R.Frame.render(1400, 900, os, T.Th.bg()))
    IO.print(dir ++ "/bots-" ++ name ++ ".ppm")

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

def sep() -> String:
  SCon{Chr{31}, SNil{}}

# frames of one cat, timed: pose and ops, then the frame's layout
def bench(+k: Nat, +ty: L.Ty, +cats: Map<&2, CA.CatF>, +n: N.Nui, +acc: Nat) -> Nat:
  match k:
    case 0n:
      acc
    case 1n+j:
      bench(j, ty, cats, n, Nat.add(acc, List.length(&2, R.Op, ops(BV.Bv.frame(ty, L.TL.memo.none(), cats, with_ms(n, Nat.mul(j, 33n)))))))

def cats1(+k: Nat, +cats: Map<&2, CA.CatF>, +acc: Nat) -> Nat:
  match k:
    case 0n:
      acc
    case 1n+j:
      cats1(j, cats, Nat.add(acc, List.length(&2, R.Op, CA.Cats.draw(cats, 3n, "idle", Nat.mul(j, 33n), D.Clip.all(), 14, 40, 28))))

def main() -> IO(Unit):
  do IO<Unit>:
    args : List<String> <- AV.Argv.args()
    +as : List<&2, String> = copy(args)
    +dir : String = arg(as, 0n, "/tmp")
    +ty : L.Ty <- L.Ty.load()
    shot(ty, dir, "side", "", "composer")
    shot(ty, dir, "bform", "bot-new=;bfield=name" ++ sep() ++ "Nori;bfield=persona" ++ sep() ++ "Keeps an eye on the fab queue and the parts on order.", N.Fld.id("name", "bot-create", ""))
    shot(ty, dir, "chat", "bot=b0", "composer")
    shot(ty, dir, "space", "bot=b0;bot-tab=space", "")
    shot(ty, dir, "browser", "bot=b0;bot-tab=browser", "")
    shot(ty, dir, "teach-idle", "bot=b0;bot-tab=browser", "")
    shot(ty, dir, "teach-rec", "bot=b0;bot-tab=browser;!drive.b0=teach;!teach.b0=rec " ++ Nat.show(Nat.sub(at(), 85n)) ++ " 5;!teach.draft.b0=" ++ draft(), "")
    shot(ty, dir, "teach-review", "bot=b0;bot-tab=browser;!drive.b0=teach;!teach.b0=review;!teach.draft.b0=" ++ draft() ++ ";tk-mark=2;bfield=tkname" ++ sep() ++ "Check JLCPCB order status", "")
    shot(ty, dir, "teach-takeover", "bot=b0;bot-tab=browser;!drive.b0=takeover", "")
    shot(ty, dir, "tasks", "bot=b0;bot-tab=tasks", "")
    shot(ty, dir, "tasks-open", "bot=b0;bot-tab=tasks;tk-open=jlcpcb-status", "")
    shot(ty, dir, "memory", "bot=b0;bot-tab=memory", "")
    shot(ty, dir, "routines", "bot=b0;bot-tab=routines", "")
    shot(ty, dir, "routine-form", "bot=b0;bot-tab=routines;routine-edit=;@f.ocron=tomorrow 9am", "")
    shot(ty, dir, "hooks", "bot=b0;bot-tab=hooks", "")
    shot(ty, dir, "settings", "bot=b0;bot-tab=settings;bfield=persona" ++ sep() ++ "Watches the repo and the fab queue.", "")
    shot(ty, dir, "google", "bot=b0;bot-tab=settings;@google=Connected as cat@example.com, dog@example.com (Gmail and Calendar).;@google.accts=cat@example.com|dog@example.com", "")
    shot(ty, dir, "room", "room=r0", N.Fld.id("rpost", "room-post", ""))
    shot(ty, dir, "rform", "room-new=;bfield=rname" ++ sep() ++ "night shift;room-pick=b0;room-pick=b2", "")
    +u : U.Ui = base()
    +cats : Map<&2, CA.CatF> = BV.Bv.warm(u, CA.Cats.none(), 1234n)
    +n : N.Nui = N.Nui.init(u, 1400, 900)
    +t0 : Nat <- IO.now()
    +c : Nat = cats1(100n, cats, 0n)
    +t1 : Nat <- IO.now()
    +k : Nat = bench(30n, ty, cats, n, 0n)
    +t2 : Nat <- IO.now()
    IO.print("one 28 px cat posed and drawn: " ++ Nat.show(Nat.div(Nat.mul(Nat.sub(t1, t0), 10n), 100n)) ++ "/10 ms (" ++ Nat.show(c) ++ " ops); frame layout with 4 cats: " ++ Nat.show(Nat.div(Nat.sub(t2, t1), 30n)) ++ " ms (" ++ Nat.show(k) ++ ")")
