import Base
import ../core/i18n.bend as I
import ../core/json.bend as J
import ../core/model.bend as M
import ../core/client.bend as U
import ../core/botui.bend as B
import ../core/bot.bend as BT
import ../core/cron.bend as CR
import ../core/sched.bend as SC
import ../core/plane.bend as PL
import ./plane.bend as MPL
import ./screen.bend as S

# ios-snappy: tabs. The phones draw a tab bar (Home, Threads, Bots,
# Routines, Settings); this is what Bend gives the bar, cheaply, from what
# the list already computed (never the whole plane): the badges, and every
# bot's routines as rows. One hub's `tabs` object is
#   {home: {badge}, bots: {badge}, routines: {badge, rows}}
# and hubs.bend merges them over the paired hubs.

# rooms with something unread
def MTab.rooms.unread(rs: List<&2, B.RRow>, +u: U.Ui, acc: Nat) -> Nat:
  match rs:
    case Nil{}:
      acc
    case B.RRow{+id, _, _, _} <> t:
      MTab.rooms.unread(t, u, Nat.add(acc, Bool.pick(Nat, Nat.is_gt(U.Rooms.unread(u, id), 0n), 1n, 0n)))

def MTab.bot.name(m: Maybe<&2, BT.Bot>) -> String:
  match m:
    case Some{b}:
      BT.Bot.name(b)
    case None{}:
      ""

# a routine as a row of the Routines tab: kind "routine" (what the plane's rows
# and the hubs' de-duplication call it), the words of the plane's routine row
# (title, sub = schedule and next run, as core's Sched.row makes them for
# recurring, one-time and time-zone routines alike), the bot it belongs to,
# when it is next due (`at`, 0 for none), how long ago it last ran, and the tap
# (act, value, nav) the plane's routine row sends, which opens its bot's
# Routines tab. "state" is idle, reminder (a remind_person routine), off, or
# done (a one-time one that has run); "sched" is Sched's kind (routine, once,
# remind...), with its label, zone, next-run text and the done line.
def MTab.state(+done: Bool, +on: Bool, +kind: String) -> String:
  Bool.pick(String, done, "done", Bool.pick(String, on, Bool.pick(String, String.eq(kind, "remind"), "reminder", "idle"), "off"))

def MTab.routine.w.loc(+lc: I.Loc, w: SC.SRow, +r: BT.Routine, +u: U.Ui, +now: Nat) -> J.Json:
  match w:
    case SC.SRow{+kind, +label, +when, +next, +rel, +zone, +done, +dtext}:
      J.Json.obj([J.KV{"kind", J.Json.str("routine")}, J.KV{"id", J.Json.str(BT.Routine.id(r))}, J.KV{"act", J.Json.str("uber-bot")},
        J.KV{"value", J.Json.str(BT.Routine.bot(r) ++ "|routines")}, J.KV{"title", J.Json.str(BT.Routine.name(r))},
        J.KV{"sub", J.Json.str(PL.Plane.routine.sub(SC.SRow{kind, label, when, next, rel, zone, done, dtext}))},
        J.KV{"sched", J.Json.str(kind)}, J.KV{"label", J.Json.str(label)}, J.KV{"zone", J.Json.str(zone)}, J.KV{"next", J.Json.str(next)},
        J.KV{"rel", J.Json.str(rel)}, J.KV{"doneText", J.Json.str(dtext)},
        J.KV{"bot", J.Json.str(MTab.bot.name(BT.World.find.id(BT.World.bot(M.State.bots(U.Ui.state(u)), BT.Routine.bot(r)))))},
        J.KV{"state", J.Json.str(MTab.state(done, BT.Routine.on(r), BT.Routine.kind(r)))},
        J.KV{"at", J.Json.nat(SC.Sched.next(U.Ui.state(u), r, now, 0n, True{}))},
        J.KV{"ago", J.Json.str(Bool.pick(String, Nat.is_eq(BT.Routine.last(r), 0n), "", I.I18n.ago(lc, now, BT.Routine.last(r))))},
        J.KV{"machine", J.Json.str(U.Ui.far.name(u, BT.Routine.bot(r)))}, J.KV{"away", J.Json.bool(U.Ui.far.away(u, BT.Routine.bot(r)))},
        J.KV{"nav", J.Json.str(MPL.MPlane.nav("routine", BT.Routine.id(r), BT.Routine.bot(r) ++ "|routines", U.Ui.state(u)))}])

def MTab.routine.w(w: SC.SRow, +r: BT.Routine, +u: U.Ui, +now: Nat) -> J.Json:
  MTab.routine.w.loc(I.En{}, w, r, u, now)

def MTab.routine(+lc: I.Loc, +r: BT.Routine, +u: U.Ui, +now: Nat) -> J.Json:
  MTab.routine.w.loc(lc, SC.Sched.row(lc, U.Ui.state(u), r, now, 0n, True{}), r, u, now)

def MTab.routines(+lc: I.Loc, rs: List<&2, BT.Routine>, +u: U.Ui, +now: Nat) -> List<&2, J.Json>:
  match rs:
    case Nil{}:
      Nil{}
    case +r <> t:
      MTab.routine(lc, r, u, now) <> MTab.routines(lc, t, u, now)

def MTab.badge(+n: Nat) -> J.Json:
  J.Json.obj([J.KV{"badge", J.Json.nat(n)}])

# one hub's tabs: the need-you count of the list's own tally (Plane.tally,
# the sidebar's count), rooms with unread, every bot's routines
def MTab.json(+lc: I.Loc, +u: U.Ui, +ix: U.SIx, +rows: List<&2, U.SRow>) -> J.Json:
  J.Json.obj([J.KV{"home", MTab.badge(PL.PTally.need(PL.Plane.tally(u, ix, rows)))},
    J.KV{"bots", MTab.badge(MTab.rooms.unread(B.BU.rooms(u), u, 0n))},
    J.KV{"routines", J.Json.obj([J.KV{"badge", J.Json.nat(0n)},
      J.KV{"rows", J.Json.arr(MTab.routines(lc, BT.World.routines.all(M.State.bots(U.Ui.state(u))), u, S.Mob.now(u)))}])}])
