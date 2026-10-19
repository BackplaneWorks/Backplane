import Base
import ../core/json.bend as J
import ../core/i18n.bend as I
import ../core/model.bend as M
import ../core/client.bend as U
import ../core/botui.bend as B
import ../core/plane.bend as PL
import ./bots.bend as MB
import ../core/bot.bend as BT

# The status plane on the phones (docs/uber.md): the uberbot's card and
# the plane under it as JSON, built from core's Plane once a screen and
# only while a bot is the uberbot. The apps draw it (Plane.swift,
# Plane.kt) and send each row's act and value back; "id" and "value" are
# the keys hubs.bend qualifies with the hub, so a merged row is routed to
# the hub it came from.

# a row of the plane: what it is (kind: thread, bot, room, routine), what a
# tap sends (act, value), its words (title, sub), its state (thread: approval,
# question, failed, working, queued, finished, idle, away; bot: its mood;
# room: unread; routine: working, idle), how long ago, and the machine
# when it is not this one ("machine" is left out for this one)
def MPlane.machine(+m: String) -> List<&2, J.KV>:
  Bool.pick(List<&2, J.KV>, String.is_empty(m), Nil{}, [J.KV{"machine", J.Json.str(m)}])

# a routine's `at` is when it is next due, so it has no age
def MPlane.ago.loc(+lc: I.Loc, +kind: String, +now: Nat, +at: Nat) -> String:
  MB.MBots.ago.loc(lc, Bool.or(String.eq(kind, "routine"), Nat.is_eq(at, 0n)), now, at)

# the uberbot's own row has no value; its id routes the tap to its hub

def MPlane.ago(+kind: String, +now: Nat, +at: Nat) -> String:
  MPlane.ago.loc(I.En{}, kind, now, at)

def MPlane.value(+act: String, +val: String, +id: String) -> String:
  Bool.pick(String, Bool.and(String.eq(act, "uber"), String.is_empty(val)), id, val)

# Resolve a plane bot/routine through the same bot thread as its row.
def MPlane.nav.bot(m: Maybe<&2, BT.Bot>, +id: String) -> String:
  match m:
    case Some{+b}:
      MB.MBots.nav.k(String.is_empty(BT.Bot.thread(b)), id, BT.Bot.thread(b))
    case None{}:
      "@remote:" ++ id

def MPlane.nav.id(+kind: String, +id: String, +value: String) -> String:
  Bool.pick(String, String.eq(kind, "routine"), Maybe.default(&2, String, List.head(&2, String, String.split(value, '|')), id), id)

def MPlane.nav.k(room: Bool, +kind: String, +id: String, +value: String, +st: M.State) -> String:
  match room:
    case True{}:
      MB.MBots.nav.room(id)
    case False{}:
      Bool.pick(String, Bool.or(String.eq(kind, "thread"), String.eq(kind, "lock")), value, MPlane.nav.bot(BT.World.find.id(BT.World.bot(M.State.bots(st), MPlane.nav.id(kind, id, value))), MPlane.nav.id(kind, id, value)))

def MPlane.nav(+kind: String, +id: String, +value: String, +st: M.State) -> String:
  MPlane.nav.k(String.eq(kind, "room"), kind, id, value, st)

def MPlane.row(+lc: I.Loc, r: PL.PRow, +now: Nat, +st: M.State) -> J.Json:
  match r:
    case PL.PRow{+kind, +id, +act, +val, +title, +sub, +machine, +state, +at, +away}:
      J.Json.obj(List.append(&2, J.KV,
        [J.KV{"kind", J.Json.str(kind)}, J.KV{"id", J.Json.str(id)}, J.KV{"act", J.Json.str(act)}, J.KV{"value", J.Json.str(MPlane.value(act, val, id))},
         J.KV{"title", J.Json.str(PL.Plane.title.shown(lc, kind, title))}, J.KV{"sub", J.Json.str(sub)}, J.KV{"state", J.Json.str(state)},
         J.KV{"ago", J.Json.str(MPlane.ago.loc(lc, kind, now, at))}, J.KV{"at", J.Json.nat(at)}, J.KV{"away", J.Json.bool(away)}, J.KV{"nav", J.Json.str(MPlane.nav(kind, id, val, st))}],
        MPlane.machine(machine)))

def MPlane.rows(+lc: I.Loc, rs: List<&2, PL.PRow>, +now: Nat, +st: M.State) -> List<&2, J.Json>:
  match rs:
    case Nil{}:
      Nil{}
    case +r <> t:
      MPlane.row(lc, r, now, st) <> MPlane.rows(lc, t, now, st)

def MPlane.mach(ms: List<&2, PL.PMach>) -> List<&2, J.Json>:
  match ms:
    case Nil{}:
      Nil{}
    case PL.PMach{+n, +s} <> t:
      J.Json.obj([J.KV{"name", J.Json.str(n)}, J.KV{"state", J.Json.str(s)}]) <> MPlane.mach(t)

def MPlane.json.loc(+lc: I.Loc, +p: PL.Plane, +now: Nat, +st: M.State) -> J.Json:
  J.Json.obj([J.KV{"need", J.Json.arr(MPlane.rows(lc, PL.Plane.need(p), now, st))}, J.KV{"run", J.Json.arr(MPlane.rows(lc, PL.Plane.run(p), now, st))},
    J.KV{"idle", J.Json.arr(MPlane.rows(lc, PL.Plane.idle(p), now, st))}, J.KV{"idleN", J.Json.nat(PL.Plane.idle_n(p))},
    J.KV{"working", J.Json.nat(PL.Plane.working(p))}, J.KV{"bots", J.Json.arr(MPlane.rows(lc, PL.Plane.bots(p), now, st))}, J.KV{"rooms", J.Json.arr(MPlane.rows(lc, PL.Plane.rooms(p), now, st))},
    J.KV{"locks", J.Json.arr(MPlane.rows(lc, PL.Plane.locks(p), now, st))}, J.KV{"routines", J.Json.arr(MPlane.rows(lc, PL.Plane.routines(p), now, st))}, J.KV{"machines", J.Json.arr(MPlane.mach(PL.Plane.machines(p)))}])

def MPlane.json(p: PL.Plane, now: Nat, st: M.State) -> J.Json:
  MPlane.json.loc(I.En{}, p, now, st)

# where the conversation is held (core's Plane.home): null with no uberbot
# here; "button" says whether to offer "Make this the home" (a tap sends
# act with value, the uberbot's id, which routes it to its hub)
def MPlane.home.k(+lc: I.Loc, +h: PL.PHome, +id: String) -> J.Json:
  Bool.pick(J.Json, PL.PHome.on(h),
    J.Json.obj([J.KV{"name", J.Json.str(PL.PHome.name(h))}, J.KV{"here", J.Json.bool(PL.PHome.here(h))}, J.KV{"line", J.Json.str(PL.Plane.home.line(lc, h))},
      J.KV{"button", J.Json.bool(PL.Plane.home.button(h))}, J.KV{"label", J.Json.str(PL.Plane.home.label(lc))}, J.KV{"act", J.Json.str("uber-home")}, J.KV{"value", J.Json.str(id)}]),
    J.Null{})

def MPlane.home.m(+lc: I.Loc, m: Maybe<&2, B.BRow>, +u: U.Ui) -> J.Json:
  match m:
    case Some{B.BRow{+i, _, _, _, _, _, _, _, _, _}}:
      MPlane.home.k(lc, PL.Plane.home(u), i)
    case None{}:
      J.Null{}

def MPlane.home(+lc: I.Loc, +u: U.Ui) -> J.Json:
  MPlane.home.m(lc, B.BU.uber(u), u)

def MPlane.with.home(j: J.Json, +h: J.Json) -> J.Json:
  match j:
    case J.Obj{fs}:
      J.Obj{J.Field{"home", h, fs}}
    case x:
      x

# the uberbot's card: a bot's row (MBots.row) with the plane's counts
# the card with no counts (while the plane is not built)
def MPlane.bare(+lc: I.Loc, r: B.BRow) -> J.Json:
  MB.MBots.row(lc, r)

def MPlane.card.k(j: J.Json, +counts: String) -> J.Json:
  match j:
    case J.Obj{fs}:
      J.Obj{J.Field{"counts", J.Str{counts}, fs}}
    case x:
      x

def MPlane.card(+lc: I.Loc, r: B.BRow, +p: PL.Plane) -> J.Json:
  MPlane.card.k(MB.MBots.row(lc, r), PL.Plane.counts(lc, p))

type MUber is Data:
  MUber{uber: J.Json, plane: J.Json}

def MUber.uber(m: MUber) -> J.Json:
  match m:
    case MUber{u, _}:
      u

def MUber.plane(m: MUber) -> J.Json:
  match m:
    case MUber{_, p}:
      p
