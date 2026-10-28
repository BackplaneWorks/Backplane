import Base
import ./i18n.bend as I
import ./msgs.bend as MSG
import ./model.bend as M
import ./bot.bend as BT
import ./cron.bend as CR
import ./tz.bend as TZ
import ./notice.bend as NO
import ./when.bend as WH

# When a routine fires, in whose time
# -----------------------------------
# A routine is one of: legacy (tz ""): a cron line in the machine's own
# offset, as it always was; zoned (tz "person" or an IANA name): a cron line
# read as the wall clock of that zone, with DST; one-time (once > 0): it fires
# once at a UTC instant and is then done. kind "remind" delivers an alert to
# the person and wakes no agent.
#
# A zoned routine fires once per wall-clock slot: it remembers the zone's
# wall clock at its last fire (`slot`), and is due when its cron line has a
# match after that slot at or before the wall clock now. A wall clock that
# goes back (a move west, DST ending) never repeats a slot; one that jumps
# ahead (a move east, DST starting) fires once.

# one routine as a client shows it
type SRow is Data:
  SRow{kind: String, label: String, when: String, next: String, rel: String, zone: String, done: Bool, done_text: String}

# Zones
# -----

def Zone.get(s: M.State, k: String) -> String:
  M.Setting.get(s, k, "")

# a, or b when a is empty
def Zone.first(+a: String, +b: String) -> String:
  Bool.pick(String, String.is_empty(a), b, a)

# the person's zone: the one they or a bot set, else the phone's, else the
# hub machine's own, else UTC
def Zone.person(+s: M.State) -> String:
  Zone.first(Zone.get(s, "person.tz"), Zone.first(Zone.get(s, "person.tz.device"), Zone.first(Zone.get(s, "tz.machine"), "UTC")))

# "set", "phone", "machine" or "utc": where Zone.person comes from
def Zone.source(+s: M.State) -> String:
  Bool.pick(String, String.is_empty(Zone.get(s, "person.tz")),
    Bool.pick(String, String.is_empty(Zone.get(s, "person.tz.device")),
      Bool.pick(String, String.is_empty(Zone.get(s, "tz.machine")), "utc", "machine"), "phone"), "set")

def Zone.utc(+name: String) -> Bool:
  Bool.or(String.eq(name, "UTC"), Bool.or(String.eq(name, "Etc/UTC"), Bool.or(String.eq(name, "Etc/UCT"), String.eq(name, "UCT"))))

def Zone.rule.k(utc: Bool, s: M.State, name: String) -> Maybe<&2, TZ.TzRule>:
  match utc:
    case True{}:
      Some{TZ.Tz.utc0()}
    case False{}:
      TZ.Tz.parse(Zone.get(s, "tz.rule." ++ name))

# the rule the hub resolved for a zone name (UTC is built in)
def Zone.rule(s: M.State, +name: String) -> Maybe<&2, TZ.TzRule>:
  Zone.rule.k(Zone.utc(name), s, name)

# a routine's tz field as a zone name
def Zone.of(+s: M.State, +tz: String) -> String:
  Bool.pick(String, String.eq(tz, "person"), Zone.person(s), tz)

def Zone.rule.of(+s: M.State, tz: String) -> Maybe<&2, TZ.TzRule>:
  Zone.rule(s, Zone.of(s, tz))

# a rule to show times in when the person's zone is not known yet: the
# machine's offset
def Sched.person.k(m: Maybe<&2, TZ.TzRule>, +mo: Nat, +me: Bool) -> TZ.TzRule:
  match m:
    case Some{r}:
      r
    case None{}:
      TZ.Tz.fixed(me, mo)

# the person's zone as a rule, else the machine's offset
def Sched.person(+s: M.State, +mo: Nat, +me: Bool) -> TZ.TzRule:
  Sched.person.k(Zone.rule(s, Zone.person(s)), mo, me)

def Zone.valid.char(+c: Char) -> Bool:
  Bool.or(Char.is_alpha(c), Bool.or(Char.is_digit(c), Bool.or(Char.is_eq(c, '_'), Bool.or(Char.is_eq(c, '+'), Char.is_eq(c, '-')))))

# A zone name the hub may look up: segments of letters, digits, _ + -
# joined by /, no leading /, at most 64 characters
def Zone.valid.chars(s: String) -> Bool:
  match s:
    case SNil{}:
      True{}
    case SCon{+c, t}:
      Bool.and(Zone.valid.char(c), Zone.valid.chars(t))

def Zone.valid.segs(ss: List<&2, String>) -> Bool:
  match ss:
    case Nil{}:
      True{}
    case +h <> t:
      Bool.and(Bool.and(Bool.not(String.is_empty(h)), Zone.valid.chars(h)), Zone.valid.segs(t))

def Zone.valid(+name: String) -> Bool:
  Bool.and(Bool.and(Nat.is_ge(String.length(name), 1n), Nat.is_le(String.length(name), 64n)), Zone.valid.segs(String.split(name, '/')))

def Zone.has(xs: List<&2, String>, +x: String) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case +h <> t:
      Bool.or(String.eq(h, x), Zone.has(t, x))

# x added to xs unless it is there, empty, UTC or not a name
def Zone.add.k(ok: Bool, xs: List<&2, String>, x: String) -> List<&2, String>:
  match ok:
    case True{}:
      x <> xs
    case False{}:
      xs

def Zone.add(+xs: List<&2, String>, +x: String) -> List<&2, String>:
  Zone.add.k(Bool.and(Zone.valid(x), Bool.and(Bool.not(Zone.utc(x)), Bool.not(Zone.has(xs, x)))), xs, x)

# a routine's zone by name ("person" is the person's zone, already listed)
def Zone.names.r(rs: List<&2, BT.Routine>, +s: M.State, acc: List<&2, String>) -> List<&2, String>:
  match rs:
    case Nil{}:
      acc
    case r <> t:
      Zone.names.r(t, s, Zone.add(acc, Zone.of(s, BT.Routine.tz(r))))

# every zone the hub needs a rule for: the person's, the phone's, the
# machine's, and the ones live routines name
def Zone.names(+s: M.State) -> List<&2, String>:
  Zone.names.r(BT.World.routines.all(M.State.bots(s)), s, Zone.add(Zone.add(Zone.add(Zone.add(Nil{}, Zone.person(s)), Zone.get(s, "person.tz")), Zone.get(s, "person.tz.device")), Zone.get(s, "tz.machine")))

# "name<tab>rule it has" for the zones to look up: those with no rule, or
# all of them (the daily check)
def Zone.todo.k(full: Bool, none: Bool, line: String) -> List<&2, String>:
  Bool.pick(List<&2, String>, Bool.or(full, none), [line], Nil{})

def Zone.todo.go(xs: List<&2, String>, +s: M.State, +full: Bool) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case +n <> t:
      List.append(&2, String, Zone.todo.k(full, String.is_empty(Zone.get(s, "tz.rule." ++ n)), n ++ "\t" ++ Zone.get(s, "tz.rule." ++ n)), Zone.todo.go(t, s, full))

def Zone.todo(+s: M.State, +full: Bool) -> List<&2, String>:
  Zone.todo.go(Zone.names(s), s, full)

def Zone.checked(+t: String) -> Nat:
  Maybe.default(&2, Nat, CR.Cron.num(t), 0n)

# the day's check is due: not yet made, or a day old
def Zone.stale(+s: M.State, +now: Nat) -> Bool:
  Bool.or(String.is_empty(Zone.get(s, "tz.checked")), Nat.is_ge(now, Nat.add(Zone.checked(Zone.get(s, "tz.checked")), 86400n)))

def Zone.fixed.k(sign: String, hh: String, +mm: String, z: String) -> String:
  "<" ++ z ++ ">" ++ Bool.pick(String, String.eq(sign, "-"), "", "-") ++ Nat.show(Zone.checked(hh)) ++ Bool.pick(String, Nat.is_eq(Zone.checked(mm), 0n), "", ":" ++ mm)

# "+0530" (date +%z) as a POSIX rule: <+0530>-5:30
def Zone.fixed.rule(+z: String) -> String:
  Zone.fixed.k(String.take(z, 1n), String.take(String.drop(z, 1n), 2n), String.take(String.drop(z, 3n), 2n), z)

# Due
# ---

def Sched.cron.due(m: Maybe<&2, CR.Cron>, +last: Nat, +now: Nat, +mo: Nat, +me: Bool) -> Bool:
  match m:
    case Some{c}:
      CR.Cron.due(c, last, now, mo, me)
    case None{}:
      False{}

# the wall clock the search starts from: the last slot, else the wall clock at the last fire
def Sched.base(rule: TZ.TzRule, +last: Nat, +slot: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_gt(slot, 0n), slot, TZ.Tz.wall(rule, last))

def Sched.due.w(c: CR.Cron, +rule: TZ.TzRule, +last: Nat, +now: Nat, +slot: Nat) -> Bool:
  CR.Cron.due(c, Sched.base(rule, last, slot), TZ.Tz.wall(rule, now), 0n, True{})

def Sched.due.z(m: Maybe<&2, CR.Cron>, zr: Maybe<&2, TZ.TzRule>, +last: Nat, +now: Nat, +slot: Nat) -> Bool:
  match m zr:
    case Some{c} Some{+rule}:
      Sched.due.w(c, rule, last, now, slot)
    case _ _:
      False{}

def Sched.due.leg(legacy: Bool, +s: M.State, cron: String, +last: Nat, +now: Nat, +mo: Nat, +me: Bool, tz: String, +slot: Nat) -> Bool:
  match legacy:
    case True{}:
      Sched.cron.due(CR.Cron.parse(cron), last, now, mo, me)
    case False{}:
      Sched.due.z(CR.Cron.parse(cron), Zone.rule.of(s, tz), last, now, slot)

def Sched.due.once(isonce: Bool, +s: M.State, cron: String, +last: Nat, +now: Nat, +mo: Nat, +me: Bool, +once: Nat, +tz: String, +slot: Nat) -> Bool:
  match isonce:
    case True{}:
      Nat.is_ge(now, once)
    case False{}:
      Sched.due.leg(String.is_empty(tz), s, cron, last, now, mo, me, tz, slot)

def Sched.due.live(live: Bool, +s: M.State, cron: String, +last: Nat, +now: Nat, +mo: Nat, +me: Bool, +once: Nat, tz: String, +slot: Nat) -> Bool:
  match live:
    case True{}:
      Sched.due.once(Nat.is_gt(once, 0n), s, cron, last, now, mo, me, once, tz, slot)
    case False{}:
      False{}

# whether a routine runs at `now`; mo/me the machine's offset (legacy routines)
def Sched.due(+s: M.State, r: BT.Routine, +now: Nat, +mo: Nat, +me: Bool) -> Bool:
  match r:
    case BT.Routine{_, _, _, cron, _, on, +last, _, deleted, +once, tz, _, done, +slot}:
      Sched.due.live(Bool.and(Bool.not(done), Bool.and(on, Bool.not(deleted))), s, cron, last, now, mo, me, once, tz, slot)

# Slots
# -----

def Sched.slot.cron(m: Maybe<&2, CR.Cron>, +after: Nat, +mo: Nat, +me: Bool) -> Nat:
  match m:
    case Some{c}:
      CR.Cron.next(c, after, mo, me)
    case None{}:
      0n

def Sched.slot.z(m: Maybe<&2, CR.Cron>, zr: Maybe<&2, TZ.TzRule>, +last: Nat, +slot: Nat) -> Nat:
  match m zr:
    case Some{c} Some{rule}:
      CR.Cron.next(c, Sched.base(rule, last, slot), 0n, True{})
    case _ _:
      0n

def Sched.slot.leg(legacy: Bool, +s: M.State, cron: String, +last: Nat, +mo: Nat, +me: Bool, tz: String, +slot: Nat) -> Nat:
  match legacy:
    case True{}:
      Sched.slot.cron(CR.Cron.parse(cron), last, mo, me)
    case False{}:
      Sched.slot.z(CR.Cron.parse(cron), Zone.rule.of(s, tz), last, slot)

def Sched.slot.once(isonce: Bool, +s: M.State, cron: String, +last: Nat, +mo: Nat, +me: Bool, +once: Nat, +tz: String, +slot: Nat) -> Nat:
  match isonce:
    case True{}:
      once
    case False{}:
      Sched.slot.leg(String.is_empty(tz), s, cron, last, mo, me, tz, slot)

# the time a routine is next scheduled for after its last run, as an
# uberbot's replicas name it: a UTC instant (legacy, once) or a wall clock
# (zoned). 0: never
def Sched.slot(+s: M.State, r: BT.Routine, +mo: Nat, +me: Bool) -> Nat:
  match r:
    case BT.Routine{_, _, _, cron, _, _, +last, _, _, +once, tz, _, _, +slot}:
      Sched.slot.once(Nat.is_gt(once, 0n), s, cron, last, mo, me, once, tz, slot)

# What it logs when it fires
# --------------------------

# the wall clock of a zoned routine's zone now (the slot to record), 0 when
# the routine is not zoned or its zone has no rule yet
def Sched.wall.z(zr: Maybe<&2, TZ.TzRule>, +now: Nat) -> Nat:
  match zr:
    case Some{rule}:
      TZ.Tz.wall(rule, now)
    case None{}:
      0n

def Sched.wall.k(legacy: Bool, once: Bool, +s: M.State, tz: String, +now: Nat) -> Nat:
  match legacy once:
    case False{} False{}:
      Sched.wall.z(Zone.rule.of(s, tz), now)
    case _ _:
      0n

def Sched.wall(+s: M.State, +r: BT.Routine, +now: Nat) -> Nat:
  Sched.wall.k(String.is_empty(BT.Routine.tz(r)), Nat.is_gt(BT.Routine.once(r), 0n), s, BT.Routine.tz(r), now)

# Next
# ----

def Sched.later(+a: Nat, +b: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_ge(a, b), a, b)

def Sched.at.z(rule: TZ.TzRule, +w: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_eq(w, 0n), 0n, TZ.Tz.at(rule, w))

def Sched.next.w(c: CR.Cron, +rule: TZ.TzRule, +last: Nat, +now: Nat, +slot: Nat) -> Nat:
  Sched.at.z(rule, CR.Cron.next(c, Sched.later(Sched.base(rule, last, slot), TZ.Tz.wall(rule, now)), 0n, True{}))

def Sched.next.z(m: Maybe<&2, CR.Cron>, zr: Maybe<&2, TZ.TzRule>, +last: Nat, +now: Nat, +slot: Nat) -> Nat:
  match m zr:
    case Some{c} Some{+rule}:
      Sched.next.w(c, rule, last, now, slot)
    case _ _:
      0n

# a legacy routine's next time: by the machine's zone when the hub has
# logged it (tz.machine; clients don't know the machine's offset), else by
# the offset given
def Sched.next.mach(zr: Maybe<&2, TZ.TzRule>, +cron: String, +last: Nat, +now: Nat, +mo: Nat, +me: Bool) -> Nat:
  match zr:
    case Some{+rule}:
      Sched.next.z(CR.Cron.parse(cron), Some{rule}, last, now, 0n)
    case None{}:
      Sched.slot.cron(CR.Cron.parse(cron), Sched.later(last, now), mo, me)

def Sched.next.leg(legacy: Bool, +s: M.State, cron: String, +last: Nat, +now: Nat, +mo: Nat, +me: Bool, tz: String, +slot: Nat) -> Nat:
  match legacy:
    case True{}:
      Sched.next.mach(Zone.rule(s, Zone.get(s, "tz.machine")), cron, last, now, mo, me)
    case False{}:
      Sched.next.z(CR.Cron.parse(cron), Zone.rule.of(s, tz), last, now, slot)

def Sched.next.once(isonce: Bool, +s: M.State, cron: String, +last: Nat, +now: Nat, +mo: Nat, +me: Bool, +once: Nat, +tz: String, +slot: Nat) -> Nat:
  match isonce:
    case True{}:
      once
    case False{}:
      Sched.next.leg(String.is_empty(tz), s, cron, last, now, mo, me, tz, slot)

def Sched.next.live(live: Bool, +s: M.State, cron: String, +last: Nat, +now: Nat, +mo: Nat, +me: Bool, +once: Nat, tz: String, +slot: Nat) -> Nat:
  match live:
    case True{}:
      Sched.next.once(Nat.is_gt(once, 0n), s, cron, last, now, mo, me, once, tz, slot)
    case False{}:
      0n

# the UTC instant of a routine's next fire, 0 when it is off, done or never
def Sched.next(+s: M.State, r: BT.Routine, +now: Nat, +mo: Nat, +me: Bool) -> Nat:
  match r:
    case BT.Routine{_, _, _, cron, _, on, +last, _, deleted, +once, tz, _, done, +slot}:
      Sched.next.live(Bool.and(Bool.not(done), Bool.and(on, Bool.not(deleted))), s, cron, last, now, mo, me, once, tz, slot)

def Sched.dur.pair(+a: Nat, ua: String, +b: Nat, ub: String) -> String:
  Nat.show(a) ++ ua ++ Bool.pick(String, Nat.is_eq(b, 0n), "", " " ++ Nat.show(b) ++ ub)

def Sched.dur.k(sec: Bool, hour: Bool, day: Bool, +d: Nat) -> String:
  match sec hour day:
    case True{} _ _:
      "under a minute"
    case False{} True{} _:
      Nat.show(Nat.div(d, 60n)) ++ "m"
    case False{} False{} True{}:
      Sched.dur.pair(Nat.div(d, 3600n), "h", Nat.div(Nat.mod(d, 3600n), 60n), "m")
    case False{} False{} False{}:
      Sched.dur.pair(Nat.div(d, 86400n), "d", Nat.div(Nat.mod(d, 86400n), 3600n), "h")

# Showing
# -------

# "3h 5m", "2d 4h": how long from now
def Sched.dur(+d: Nat) -> String:
  Sched.dur.k(Nat.is_lt(d, 60n), Nat.is_lt(d, 3600n), Nat.is_lt(d, 86400n), d)

def Sched.rel.k(none: Bool, past: Bool, +d: Nat) -> String:
  match none past:
    case True{} _:
      ""
    case False{} True{}:
      "due now"
    case False{} False{}:
      "in " ++ Sched.dur(d)

# "in 3h 5m", "due now"; "" for a time of 0
def Sched.rel(+now: Nat, +t: Nat) -> String:
  Sched.rel.k(Nat.is_eq(t, 0n), Nat.is_le(t, now), Nat.sub(t, now))

# the same, in the device language (the old helpers above stay for what
# is not a screen)
def Sched.rel.l.k(+l: I.Loc, none: Bool, past: Bool, +d: Nat) -> String:
  match none past:
    case True{} _:
      ""
    case False{} True{}:
      I.I18n.due(l)
    case False{} False{}:
      I.I18n.until(l, d)

def Sched.rel.l(+l: I.Loc, +now: Nat, +t: Nat) -> String:
  Sched.rel.l.k(l, Nat.is_eq(t, 0n), Nat.is_le(t, now), Nat.sub(t, now))

def Sched.next.text(rule: TZ.TzRule, +t: Nat) -> String:
  Bool.pick(String, Nat.is_eq(t, 0n), "", TZ.Tz.show(rule, t))

def Sched.zone.t.k(+there: String, mine: String) -> String:
  Bool.pick(String, String.eq(there, mine), "", " · " ++ there)

# its own wall time beside the name when it differs from the person's
def Sched.zone.t(wz: Maybe<&2, TZ.TzRule>, +nx: Nat, mine: String) -> String:
  match wz:
    case Some{z}:
      Sched.zone.t.k(TZ.Tz.show.time(z, nx), mine)
    case None{}:
      ""

def Sched.zone.k(+l: I.Loc, legacy: Bool, person: Bool, +tz: String, +nx: Nat, wz: Maybe<&2, TZ.TzRule>, mine: String) -> String:
  match legacy person:
    case True{} _:
      MSG.T.sched.zone.machine(l)
    case False{} True{}:
      MSG.T.sched.zone.you(l)
    case False{} False{}:
      Bool.pick(String, Nat.is_eq(nx, 0n), tz, tz ++ Sched.zone.t(wz, nx, mine))

def Sched.zone.name(+l: I.Loc, +s: M.State, +tz: String, +mo: Nat, +me: Bool, +nx: Nat, wz: Maybe<&2, TZ.TzRule>) -> String:
  Sched.zone.k(l, String.is_empty(tz), String.eq(tz, "person"), tz, nx, wz, TZ.Tz.show.time(Sched.person(s, mo, me), nx))

def Sched.kind(kind: String, +once: Nat) -> String:
  Bool.pick(String, String.eq(kind, "remind"), "reminder", Bool.pick(String, Nat.is_gt(once, 0n), "once", "routine"))

def Sched.label(+l: I.Loc, +k: String) -> String:
  Bool.pick(String, String.eq(k, "reminder"), MSG.T.sched.kind.reminder(l), Bool.pick(String, String.eq(k, "once"), MSG.T.sched.kind.once(l), MSG.T.sched.kind.repeat(l)))

# A schedule's words where a client shows them (cron.bend's Cron.describe,
# in a language; the tools and the agents keep Cron.describe's English)
def Sched.cron.each(+l: I.Loc, minute: Bool, +n: Nat) -> String:
  match minute:
    case True{}: Bool.pick(String, Nat.is_eq(n, 1n), MSG.T.sched.every.minute(l), MSG.T.sched.every.minutes(l, Nat.show(n)))
    case False{}: Bool.pick(String, Nat.is_eq(n, 1n), MSG.T.sched.every.hour(l), MSG.T.sched.every.hours(l, Nat.show(n)))

def Sched.cron.hourly(+l: I.Loc, +m: Nat) -> String:
  Bool.pick(String, Nat.is_eq(m, 0n), MSG.T.sched.hourly(l), MSG.T.sched.hourly.at(l, CR.Cron.pad2(m)))

def Sched.cron.parts(+l: I.Loc, c: CR.Cron) -> Maybe<&2, String>:
  match c:
    case CR.Cron{+mi, +ho, +dm, +mo, +dw, +da, +wa}:
      +all = Bool.and(Bool.and(da, wa), Nat.is_eq(List.length(&2, Nat, mo), 12n))
      +allh = Nat.is_eq(List.length(&2, Nat, ho), 24n)
      +n = CR.Cron.gap(mi)
      +k = CR.Cron.gap(ho)
      +m = CR.Cron.head(mi)
      +h = CR.Cron.head(ho)
      CR.Cron.or(CR.Cron.when(Bool.and(Bool.and(all, allh), Bool.and(Nat.is_eq(Nat.mod(60n, n), 0n), CR.Cron.same(mi, CR.Cron.vals([CR.CRng{0n, 59n, n}], 0n, 59n)))), Sched.cron.each(l, True{}, n)),
        CR.Cron.or(CR.Cron.when(Bool.and(Bool.and(all, allh), CR.Cron.one.of(mi)), Sched.cron.hourly(l, m)),
          CR.Cron.or(CR.Cron.when(Bool.and(Bool.and(all, CR.Cron.same(mi, [0n])), Bool.and(Nat.is_eq(Nat.mod(24n, k), 0n), CR.Cron.same(ho, CR.Cron.vals([CR.CRng{0n, 23n, k}], 0n, 23n)))), Sched.cron.each(l, False{}, k)),
            CR.Cron.or(CR.Cron.when(Bool.and(all, Bool.and(CR.Cron.one.of(mi), CR.Cron.one.of(ho))), MSG.T.sched.daily(l, CR.Cron.hm(h, m))),
              CR.Cron.when(Bool.and(Bool.and(Bool.and(da, Bool.not(wa)), Nat.is_eq(List.length(&2, Nat, mo), 12n)), Bool.and(CR.Cron.same(dw, [1n, 2n, 3n, 4n, 5n]), Bool.and(CR.Cron.one.of(mi), CR.Cron.one.of(ho)))), MSG.T.sched.weekdays(l, CR.Cron.hm(h, m)))))))

def Sched.cron.parsed(+l: I.Loc, m: Maybe<&2, CR.Cron>, line: String) -> String:
  match m:
    case None{}: ""
    case Some{c}: Maybe.default(&2, String, Sched.cron.parts(l, c), line)

# the words for a cron line, else the line itself; "" when invalid
def Sched.cron(+l: I.Loc, +text: String) -> String:
  Sched.cron.parsed(l, CR.Cron.parse(text), CR.Cron.text(text))

def Sched.when.cron(+l: I.Loc, +cron: String) -> String:
  Bool.pick(String, String.is_empty(Sched.cron(l, cron)), cron, Sched.cron(l, cron))

def Sched.when(+l: I.Loc, cron: String, +once: Nat, rule: TZ.TzRule) -> String:
  Bool.pick(String, Nat.is_gt(once, 0n), TZ.Tz.show(rule, once), Sched.when.cron(l, cron))

def Sched.done.text(+l: I.Loc, done: Bool, rule: TZ.TzRule, +last: Nat) -> String:
  Bool.pick(String, done, MSG.T.sched.done(l, TZ.Tz.show(rule, last)), "")

def Sched.row.k(+l: I.Loc, +kind: String, +once: Nat, cron: String, +tz: String, +done: Bool, +last: Nat, +nx: Nat, +now: Nat, +s: M.State, +mo: Nat, +me: Bool) -> SRow:
  SRow{Sched.kind(kind, once), Sched.label(l, Sched.kind(kind, once)),
    Sched.when(l, cron, once, Sched.person(s, mo, me)) ++ Bool.pick(String, Bool.or(done, Bool.or(Nat.is_eq(once, 0n), Nat.is_eq(nx, 0n))), "", " · " ++ Sched.rel.l(l, now, nx)),
    Bool.pick(String, Bool.or(done, Nat.is_gt(once, 0n)), "", Sched.next.text(Sched.person(s, mo, me), nx)), Bool.pick(String, done, "", Sched.rel.l(l, now, nx)),
    Sched.zone.name(l, s, tz, mo, me, nx, Zone.rule.of(s, tz)), done, Sched.done.text(l, done, Sched.person(s, mo, me), last)}

# a routine as a client's row (a one-time one's time is its `when`, with
# how long until it; `next` is only a repeating one's)
def Sched.row(+l: I.Loc, +s: M.State, r: BT.Routine, +now: Nat, +mo: Nat, +me: Bool) -> SRow:
  match r:
    case BT.Routine{_, _, _, +cron, _, _, +last, _, _, +once, +tz, +kind, +done, _}:
      Sched.row.k(l, kind, once, cron, tz, done, last, Sched.next(s, r, now, mo, me), now, s, mo, me)

# Order
# -----
# pending first by next fire, then off ones, then done ones, newest first

type SK is Data:
  SK{g: Nat, k: Nat, r: BT.Routine}

def Sched.key.g(on: Bool, done: Bool) -> Nat:
  Bool.pick(Nat, done, 2n, Bool.pick(Nat, on, 0n, 1n))

# a routine with nothing next sorts after those with a time
def Sched.key.k(g: Nat, +nx: Nat, +last: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_eq(g, 2n), last, Bool.pick(Nat, Nat.is_eq(nx, 0n), 4000000000n, nx))

def Sched.key.of(+g: Nat, +nx: Nat, +r: BT.Routine) -> SK:
  SK{g, Sched.key.k(g, nx, BT.Routine.last(r)), r}

def Sched.key(+s: M.State, +r: BT.Routine, +now: Nat, +mo: Nat, +me: Bool) -> SK:
  Sched.key.of(Sched.key.g(BT.Routine.on(r), BT.Routine.done(r)), Sched.next(s, r, now, mo, me), r)

# whether a goes before b
def Sched.before.k(same: Bool, +ga: Nat, +ka: Nat, +gb: Nat, +kb: Nat) -> Bool:
  match same:
    case False{}:
      Nat.is_lt(ga, gb)
    case True{}:
      Bool.pick(Bool, Nat.is_eq(ga, 2n), Nat.is_gt(ka, kb), Nat.is_lt(ka, kb))

def Sched.before(a: SK, b: SK) -> Bool:
  match a b:
    case SK{+ga, +ka, _} SK{+gb, +kb, _}:
      Sched.before.k(Nat.is_eq(ga, gb), ga, ka, gb, kb)

# whether x goes before the head of ys (an empty list: yes)
def Sched.first(ys: List<&2, SK>, +x: SK) -> Bool:
  match ys:
    case Nil{}:
      True{}
    case y <> _:
      Sched.before(x, y)

# x put in its place in the sorted list ys; `first`: it goes before ys's
# head (Sched.first), so the walk stops there
def Sched.ins(ys: List<&2, SK>, +x: SK, first: Bool) -> List<&2, SK>:
  match ys first:
    case Nil{} _:
      [x]
    case +y <> t True{}:
      x <> y <> t
    case +y <> +t False{}:
      y <> Sched.ins(t, x, Sched.first(t, x))

def Sched.sorted(xs: List<&2, SK>, +acc: List<&2, SK>) -> List<&2, SK>:
  match xs:
    case Nil{}:
      acc
    case +x <> t:
      Sched.sorted(t, Sched.ins(acc, x, Sched.first(acc, x)))

def Sched.keys(+s: M.State, rs: List<&2, BT.Routine>, +now: Nat, +mo: Nat, +me: Bool) -> List<&2, SK>:
  match rs:
    case Nil{}:
      Nil{}
    case +r <> t:
      Sched.key(s, r, now, mo, me) <> Sched.keys(s, t, now, mo, me)

def Sched.strip(xs: List<&2, SK>) -> List<&2, BT.Routine>:
  match xs:
    case Nil{}:
      Nil{}
    case SK{_, _, r} <> t:
      r <> Sched.strip(t)

# a bot's routines in the order its list shows them
def Sched.order(+s: M.State, rs: List<&2, BT.Routine>, +now: Nat, +mo: Nat, +me: Bool) -> List<&2, BT.Routine>:
  Sched.strip(Sched.sorted(Sched.keys(s, rs, now, mo, me), Nil{}))

# What a client says about a time
# -------------------------------

def Sched.hint.next(m: Maybe<&2, CR.Cron>, +rule: TZ.TzRule, +now: Nat) -> Nat:
  match m:
    case Some{c}:
      Sched.at.z(rule, CR.Cron.next(c, TZ.Tz.wall(rule, now), 0n, True{}))
    case None{}:
      0n

def Sched.hint.cron.k(+l: I.Loc, +text: String, +t: Nat, rule: TZ.TzRule) -> String:
  Bool.pick(String, Nat.is_eq(t, 0n), Sched.cron(l, text), MSG.T.sched.hint.next(l, Sched.cron(l, text), TZ.Tz.show(rule, t)))

def Sched.hint.cron(+l: I.Loc, +text: String, +rule: TZ.TzRule, +now: Nat) -> String:
  Sched.hint.cron.k(l, text, Sched.hint.next(CR.Cron.parse(text), rule, now), rule)

def Sched.hint.once(+l: I.Loc, m: Maybe<&2, Nat>, rule: TZ.TzRule, blank: Bool) -> String:
  match m:
    case Some{t}:
      MSG.T.sched.hint.once(l, TZ.Tz.show(rule, t))
    case None{}:
      Bool.pick(String, blank, "", MSG.T.sched.help.time(l))

def Sched.hint.k(+l: I.Loc, ok: Bool, +text: String, +rule: TZ.TzRule, +now: Nat) -> String:
  match ok:
    case True{}:
      Sched.hint.cron(l, text, rule, now)
    case False{}:
      Sched.hint.once(l, WH.When.parse(text, now, rule), rule, String.is_empty(String.trim(text)))

# what a schedule or a one-time phrase means, in the person's zone: the
# live hint under a routine form's When field
def Sched.hint(+l: I.Loc, +s: M.State, +text: String, +now: Nat, +mo: Nat, +me: Bool) -> String:
  Sched.hint.k(l, BT.Maybe.is(CR.Cron, CR.Cron.parse(text)), text, Sched.person(s, mo, me), now)

# whether the text is neither a schedule nor a time (the hint then says
# what it can read); blank is not
def Sched.hint.bad.k(cron: Bool, +text: String, +now: Nat, rule: TZ.TzRule) -> Bool:
  match cron:
    case True{}:
      False{}
    case False{}:
      Bool.and(Bool.not(String.is_empty(String.trim(text))), Bool.not(BT.Maybe.is(Nat, WH.When.parse(text, now, rule))))

def Sched.hint.bad(+s: M.State, +text: String, +now: Nat, +mo: Nat, +me: Bool) -> Bool:
  Sched.hint.bad.k(BT.Maybe.is(CR.Cron, CR.Cron.parse(text)), text, now, Sched.person(s, mo, me))

# the sentence a tool says after saving: "once, Tue 14 Oct 09:00 EEST (in 17h 56m)"
def Sched.zone.phrase(+s: M.State, +tz: String) -> String:
  Bool.pick(String, String.is_empty(tz), "this machine's time",
    Bool.pick(String, String.eq(tz, "person"), "your time (" ++ Zone.person(s) ++ ")", tz))

def Sched.saved.k(isonce: Bool, kind: String, when: String, +rel: String, zone: String, +next: String) -> String:
  match isonce:
    case True{}:
      "once, " ++ next ++ Bool.pick(String, String.is_empty(rel), "", " (" ++ rel ++ ")")
    case False{}:
      when ++ ", " ++ zone ++ Bool.pick(String, String.is_empty(next), "", "; next " ++ next)

def Sched.saved.row(w: SRow, +once: Nat, zone: String, onceat: String) -> String:
  match w:
    case SRow{kind, _, when, next, rel, _, _, _}:
      Sched.saved.k(Nat.is_gt(once, 0n), kind, when, rel, zone, Bool.pick(String, Nat.is_gt(once, 0n), onceat, next))

def Sched.saved(+s: M.State, +r: BT.Routine, +now: Nat, +mo: Nat, +me: Bool) -> String:
  Sched.saved.row(Sched.row(I.En{}, s, r, now, mo, me), BT.Routine.once(r), Sched.zone.phrase(s, BT.Routine.tz(r)), TZ.Tz.show(Sched.person(s, mo, me), BT.Routine.once(r)))

# The alert a delivered reminder raises: notice.bend's alert record for a
# reminder the person scheduled (key remind-<id>, never held by a desk;
# kind "" when the person turned reminders off). A routine that does not
# need the person never calls it.
def Sched.alert(+s: M.State, +now: Nat, +r: BT.Routine, +thread: String) -> List<&2, M.Change>:
  NO.Policy.reminder(s, now, thread, BT.Routine.id(r), BT.Routine.prompt(r))
