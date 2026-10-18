import Base
import ../src/core/i18n.bend as I
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/bot.bend as BT
import ../src/core/sched.bend as S
import ../src/core/tz.bend as Z

def hel() -> String:
  "EET-2EEST,M3.5.0/3,M10.5.0/4"

def base() -> List<&2, M.Change>:
  [M.SettingSet{"tz.rule.Europe/Helsinki", "EET-2EEST,M3.5.0/3,M10.5.0/4", 1n},
   M.SettingSet{"tz.rule.America/New_York", "EST5EDT,M3.2.0,M11.1.0", 1n},
   M.SettingSet{"tz.rule.Asia/Tokyo", "JST-9", 1n},
   M.SettingSet{"person.tz", "Europe/Helsinki", 1n}]

def world(cs: List<&2, M.Change>) -> M.State:
  M.State.replay(List.append(&2, M.Change, base(), cs), M.State.empty())

def bare(cs: List<&2, M.Change>) -> M.State:
  M.State.replay(cs, M.State.empty())

# a routine made at `created` with a plan
def mk(+id: String, +cron: String, +tz: String, +once: Nat, +kind: String, +created: Nat) -> List<&2, M.Change>:
  [M.RoutineSet{id, "b1", "R", cron, "p", True{}, created}, M.RoutinePlan{id, once, tz, kind, created}]

def rt(+st: M.State, +id: String) -> BT.Routine:
  BT.World.routine(M.State.bots(st), id)

def due.s(+st: M.State, +id: String, +now: Nat) -> String:
  Bool.show(S.Sched.due(st, rt(st, id), now, 0n, True{}))

def due(cs: List<&2, M.Change>, +id: String, +now: Nat) -> String:
  due.s(world(cs), id, now)

def fire.s(+st: M.State, cs: List<&2, M.Change>, +id: String, +now: Nat) -> List<&2, M.Change>:
  List.append(&2, M.Change, cs, [M.RoutineFired{id, now}, M.RoutineSlot{id, S.Sched.wall(st, rt(st, id), now), now}])

# the changes the tick logs when a routine fires at `now`
def fire(+cs: List<&2, M.Change>, +id: String, +now: Nat) -> List<&2, M.Change>:
  fire.s(world(cs), cs, id, now)

def next.s(+st: M.State, +id: String, +now: Nat) -> String:
  Nat.show(S.Sched.next(st, rt(st, id), now, 0n, True{}))

def next(cs: List<&2, M.Change>, +id: String, +now: Nat) -> String:
  next.s(world(cs), id, now)

def row.show(w: S.SRow) -> String:
  match w:
    case S.SRow{kind, label, when, next, rel, zone, done, dt}:
      kind ++ "|" ++ label ++ "|" ++ when ++ "|" ++ next ++ "|" ++ rel ++ "|" ++ zone ++ "|" ++ Bool.show(done) ++ "|" ++ dt

def row.s(+st: M.State, +id: String, +now: Nat) -> String:
  row.show(S.Sched.row(I.En{}, st, rt(st, id), now, 0n, True{}))

def row(cs: List<&2, M.Change>, +id: String, +now: Nat) -> String:
  row.s(world(cs), id, now)

def zone.str(+st: M.State) -> String:
  S.Zone.person(st) ++ "/" ++ S.Zone.source(st)

def hint(+t: String, +now: Nat) -> String:
  S.Sched.hint(I.En{}, world(Nil{}), t, now, 0n, True{})

def order.ids(rs: List<&2, BT.Routine>) -> String:
  match rs:
    case Nil{}:
      ""
    case r <> t:
      BT.Routine.id(r) ++ " " ++ order.ids(t)

def order(+st: M.State, +now: Nat) -> String:
  order.ids(S.Sched.order(st, BT.World.routines.all(M.State.bots(st)), now, 0n, True{}))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("zone: nothing is UTC", zone.str(bare(Nil{})), "UTC/utc")
    C.Check.str("zone: this machine", zone.str(bare([M.SettingSet{"tz.machine", "Europe/London", 1n}])), "Europe/London/machine")
    C.Check.str("zone: the phone beats the machine", zone.str(bare([M.SettingSet{"tz.machine", "Europe/London", 1n}, M.SettingSet{"person.tz.device", "Asia/Tokyo", 2n}])), "Asia/Tokyo/phone")
    C.Check.str("zone: set beats the phone", zone.str(bare([M.SettingSet{"person.tz.device", "Asia/Tokyo", 2n}, M.SettingSet{"person.tz", "Europe/Helsinki", 3n}])), "Europe/Helsinki/set")
    C.Check.str("zone: auto clears the set one", zone.str(bare([M.SettingSet{"person.tz.device", "Asia/Tokyo", 2n}, M.SettingSet{"person.tz", "Europe/Helsinki", 3n}, M.SettingSet{"person.tz", "", 4n}])), "Asia/Tokyo/phone")
    C.Check.str("zone: of person", S.Zone.of(world(Nil{}), "person"), "Europe/Helsinki")
    C.Check.str("zone: of a name", S.Zone.of(world(Nil{}), "Asia/Tokyo"), "Asia/Tokyo")
    C.Check.str("zone: UTC is built in", Bool.show(BT.Maybe.is(Z.TzRule, S.Zone.rule(bare(Nil{}), "UTC"))), "True")
    C.Check.str("zone: unknown has no rule", Bool.show(BT.Maybe.is(Z.TzRule, S.Zone.rule(bare(Nil{}), "Mars/Base"))), "False")
    C.Check.str("zone: resolved one has", Bool.show(BT.Maybe.is(Z.TzRule, S.Zone.rule(world(Nil{}), "Europe/Helsinki"))), "True")
    C.Check.str("summer 09:29:59 not due", due(mk("r1", "30 9 * * *", "person", 0n, "", 1791892800n), "r1", 1791959399n), "False")
    C.Check.str("summer 09:30 due", due(mk("r1", "30 9 * * *", "person", 0n, "", 1791892800n), "r1", 1791959400n), "True")
    C.Check.str("winter 09:29:59 not due", due(mk("r1", "30 9 * * *", "person", 0n, "", 1797163200n), "r1", 1797233399n), "False")
    C.Check.str("winter 09:30 due", due(mk("r1", "30 9 * * *", "person", 0n, "", 1797163200n), "r1", 1797233400n), "True")
    C.Check.str("named zone, not the person's", due(mk("r1", "30 9 * * *", "Asia/Tokyo", 0n, "", 1791939600n), "r1", 1791959400n), "False")
    C.Check.str("named zone at its own 09:30", due(mk("r1", "30 9 * * *", "Asia/Tokyo", 0n, "", 1791939600n), "r1", 1792024200n), "True")
    C.Check.str("a zone with no rule waits", due(mk("r1", "30 9 * * *", "Mars/Base", 0n, "", 1791892800n), "r1", 1791959400n), "False")
    C.Check.str("fired: not due a minute later", due(fire(mk("r1", "30 9 * * *", "person", 0n, "", 1791892800n), "r1", 1791959400n), "r1", 1791959460n), "False")
    C.Check.str("fired: not due the same slot hours later", due(fire(mk("r1", "30 9 * * *", "person", 0n, "", 1791892800n), "r1", 1791959400n), "r1", 1791980000n), "False")
    C.Check.str("fired: due next day", due(fire(mk("r1", "30 9 * * *", "person", 0n, "", 1791892800n), "r1", 1791959400n), "r1", 1792045800n), "True")
    C.Check.str("west: a move to New York does not refire", due(List.append(&2, M.Change, fire(mk("r1", "30 9 * * *", "person", 0n, "", 1791892800n), "r1", 1791959400n), [M.SettingSet{"person.tz", "America/New_York", 9n}]), "r1", 1791959460n), "False")
    C.Check.str("west: nor at 09:30 New York time that day", due(List.append(&2, M.Change, fire(mk("r1", "30 9 * * *", "person", 0n, "", 1791892800n), "r1", 1791959400n), [M.SettingSet{"person.tz", "America/New_York", 9n}]), "r1", 1791984600n), "False")
    C.Check.str("west: but the next day", due(List.append(&2, M.Change, fire(mk("r1", "30 9 * * *", "person", 0n, "", 1791892800n), "r1", 1791959400n), [M.SettingSet{"person.tz", "America/New_York", 9n}]), "r1", 1792071000n), "True")
    C.Check.str("east: one catch-up for an hourly routine", due(List.append(&2, M.Change, fire(mk("r1", "30 * * * *", "person", 0n, "", 1791892800n), "r1", 1791959400n), [M.SettingSet{"person.tz", "Asia/Tokyo", 9n}]), "r1", 1791959460n), "True")
    C.Check.str("fall back: 03:30 EEST due", due(mk("r1", "30 3 * * *", "person", 0n, "", 1792843200n), "r1", 1792888200n), "True")
    C.Check.str("fall back: 03:29:59 not due", due(mk("r1", "30 3 * * *", "person", 0n, "", 1792843200n), "r1", 1792888199n), "False")
    C.Check.str("fall back: 03:30 EET not due again", due(fire(mk("r1", "30 3 * * *", "person", 0n, "", 1792843200n), "r1", 1792888200n), "r1", 1792891800n), "False")
    C.Check.str("fall back: still not at 03:59 EET", due(fire(mk("r1", "30 3 * * *", "person", 0n, "", 1792843200n), "r1", 1792888200n), "r1", 1792893540n), "False")
    C.Check.str("fall back: next day", due(fire(mk("r1", "30 3 * * *", "person", 0n, "", 1792843200n), "r1", 1792888200n), "r1", 1792978200n), "True")
    C.Check.str("spring forward: 02:59 not due", due(mk("r1", "30 3 * * *", "person", 0n, "", 1774699200n), "r1", 1774745999n), "False")
    C.Check.str("spring forward: due at 04:00", due(mk("r1", "30 3 * * *", "person", 0n, "", 1774699200n), "r1", 1774746000n), "True")
    C.Check.str("spring forward: once", due(fire(mk("r1", "30 3 * * *", "person", 0n, "", 1774699200n), "r1", 1774746000n), "r1", 1774747800n), "False")
    C.Check.str("once: before", due(mk("r1", "", "person", 1791979200n, "", 1791892800n), "r1", 1791979199n), "False")
    C.Check.str("once: at the time", due(mk("r1", "", "person", 1791979200n, "", 1791892800n), "r1", 1791979200n), "True")
    C.Check.str("once: late after downtime", due(mk("r1", "", "person", 1791979200n, "", 1791892800n), "r1", 1792500000n), "True")
    C.Check.str("once: done is never due", due(List.append(&2, M.Change, mk("r1", "", "person", 1791979200n, "", 1791892800n), [M.RoutineFired{"r1", 1791979200n}, M.RoutineDone{"r1", 1791979200n}]), "r1", 1792500000n), "False")
    C.Check.str("once: set again with a new time is pending", due(List.append(&2, M.Change, mk("r1", "", "person", 1791979200n, "", 1791892800n), [M.RoutineDone{"r1", 1791979200n}, M.RoutinePlan{"r1", 1792500000n, "person", "", 1791979300n}]), "r1", 1792500000n), "True")
    C.Check.str("once: off is not due", due(List.append(&2, M.Change, mk("r1", "", "person", 1791979200n, "", 1791892800n), [M.RoutineSet{"r1", "b1", "R", "", "p", False{}, 1791900000n}]), "r1", 1791979200n), "False")
    C.Check.str("once: an edit without a plan keeps the plan", due(List.append(&2, M.Change, mk("r1", "", "person", 1791979200n, "", 1791892800n), [M.RoutineSet{"r1", "b1", "R2", "", "p", True{}, 1791900000n}]), "r1", 1791979200n), "True")
    C.Check.str("legacy: no plan fires on the machine offset", due([M.RoutineSet{"r1", "b1", "R", "30 9 * * *", "p", True{}, 1791892800n}], "r1", 1791959400n), "False")
    C.Check.str("legacy: at 09:30 UTC", due([M.RoutineSet{"r1", "b1", "R", "30 9 * * *", "p", True{}, 1791892800n}], "r1", 1791970200n), "True")
    C.Check.str("legacy: fired once", due([M.RoutineSet{"r1", "b1", "R", "30 9 * * *", "p", True{}, 1791892800n}, M.RoutineFired{"r1", 1791970200n}], "r1", 1791970260n), "False")
    C.Check.str("next: zoned daily", next(mk("r1", "30 9 * * *", "person", 0n, "", 1791892800n), "r1", 1791957600n), "1791959400")
    C.Check.str("next: once", next(mk("r1", "", "person", 1791979200n, "", 1791892800n), "r1", 1791957600n), "1791979200")
    C.Check.str("next: done is none", next(List.append(&2, M.Change, mk("r1", "", "person", 1791979200n, "", 1791892800n), [M.RoutineDone{"r1", 1791979200n}]), "r1", 1791957600n), "0")
    C.Check.str("next: legacy in UTC", next([M.RoutineSet{"r1", "b1", "R", "30 9 * * *", "p", True{}, 1791892800n}], "r1", 1791957600n), "1791970200")
    C.Check.str("next: after firing, tomorrow", next(fire(mk("r1", "30 9 * * *", "person", 0n, "", 1791892800n), "r1", 1791959400n), "r1", 1791959460n), "1792045800")
    C.Check.str("row: daily", row(mk("r1", "30 9 * * *", "person", 0n, "", 1791892800n), "r1", 1791957600n), "routine|Repeats|daily at 09:30|Wed 14 Oct 09:30 EEST|in 30m|your time|False|")
    C.Check.str("row: once", row(mk("r1", "", "person", 1791979200n, "", 1791892800n), "r1", 1791957600n), "once|Once|Wed 14 Oct 15:00 EEST · in 6h||in 6h|your time|False|")
    C.Check.str("row: reminder", row(mk("r1", "", "person", 1791979200n, "remind", 1791892800n), "r1", 1791957600n), "reminder|Reminder|Wed 14 Oct 15:00 EEST · in 6h||in 6h|your time|False|")
    C.Check.str("row: done", row(List.append(&2, M.Change, mk("r1", "", "person", 1791979200n, "remind", 1791892800n), [M.RoutineFired{"r1", 1791979200n}, M.RoutineDone{"r1", 1791979200n}]), "r1", 1792000000n), "reminder|Reminder|Wed 14 Oct 15:00 EEST|||your time|True|Done · Wed 14 Oct 15:00")
    C.Check.str("row: legacy", row([M.RoutineSet{"r1", "b1", "R", "every 15m", "p", True{}, 1791892800n}], "r1", 1791957600n), "routine|Repeats|every 15 minutes|Wed 14 Oct 09:15 EEST|in 15m|this machine's time|False|")
    C.Check.str("row: another zone", row(mk("r1", "30 9 * * *", "Asia/Tokyo", 0n, "", 1791892800n), "r1", 1791957600n), "routine|Repeats|daily at 09:30|Thu 15 Oct 03:30 EEST|in 18h 30m|Asia/Tokyo · 09:30|False|")
    C.Check.str("hint: daily", hint("daily 09:30", 1791957600n), "daily at 09:30 · next Wed 14 Oct 09:30 EEST")
    C.Check.str("hint: once", hint("tomorrow 9am", 1791957600n), "once · Thu 15 Oct 09:00 EEST")
    C.Check.str("hint: error", hint("whenever", 1791957600n), "I can read: 2026-10-14T09:00+03:00, 2026-10-14 09:00, in 20m, in 1h30m, in 3d, today 17:30, tomorrow 9am, monday 09:00, or just 9am.")
    C.Check.str("hint: blank", hint("  ", 1791957600n), "")
    C.Check.str("order: pending by next, off, then done newest first", order(world(List.append(&2, M.Change, List.append(&2, M.Change, List.append(&2, M.Change, List.append(&2, M.Change, mk("a", "30 9 * * *", "person", 0n, "", 1791892800n), mk("b", "", "person", 1791979200n, "", 1791892800n)), [M.RoutineSet{"c", "b1", "R", "daily 09:00", "p", False{}, 1791892800n}]), mk("d", "", "person", 1791900000n, "remind", 1791892800n)), [M.RoutineDone{"d", 1791900000n}, M.RoutineFired{"d", 1791900000n}])), 1791957600n), "a b c d ")
    C.Check.str("saved: once", S.Sched.saved(world(mk("r1", "", "person", 1791979200n, "", 1791892800n)), rt(world(mk("r1", "", "person", 1791979200n, "", 1791892800n)), "r1"), 1791957600n, 0n, True{}), "once, Wed 14 Oct 15:00 EEST (in 6h)")
    C.Check.str("saved: daily", S.Sched.saved(world(mk("r1", "30 9 * * *", "person", 0n, "", 1791892800n)), rt(world(mk("r1", "30 9 * * *", "person", 0n, "", 1791892800n)), "r1"), 1791957600n, 0n, True{}), "daily at 09:30, your time (Europe/Helsinki); next Wed 14 Oct 09:30 EEST")
