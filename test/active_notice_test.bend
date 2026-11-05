import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/thin-pace.bend as P

def touched(key: String) -> P.Touch:
  P.Touch.changes([M.SettingSet{key, "progress", 1n}], P.Touch.none())

def yes(b: Bool) -> String:
  Bool.pick(String, b, "yes", "no")

# Twenty producers stamp progress every 10 ms. The foreground is a quiet
# conversation: those changes can affect its sidebar, never its live row.
def burst(n: Nat, +at: Nat, pace: P.Pace, sent: Nat) -> Nat:
  match n:
    case 0n: sent
    case 1n+k:
      +p = P.Pace.step(pace, at, P.Touch.level(touched("liveness.other"), "visible"))
      burst(k, Nat.add(at, 10n), P.Paced.pace(p), Nat.add(sent, Bool.pick(Nat, P.Paced.send(p), 1n, 0n)))

# Real mixed commits include policy bookkeeping beside the actual message.
# Twenty thread IDs rotate through 1000 events; metadata must not promote
# the quiet view from its sidebar cadence into the urgent lane.
def mixed(n: Nat, +at: Nat, pace: P.Pace, sent: Nat) -> Nat:
  match n:
    case 0n: sent
    case 1n+k:
      +next = k
      +th = "active-" ++ Nat.show(Nat.mod(next, 20n))
      +touch = P.Touch.changes([M.MessagePosted{th, "output", M.Assistant{}, "done", at},
        M.SettingSet{"autosettle.quiet." ++ th, Nat.show(at), at},
        M.SettingSet{"autosettle.output." ++ th, "output", at}], P.Touch.none())
      +p = P.Pace.step(pace, at, P.Touch.level(touch, "visible"))
      mixed(next, Nat.add(at, 10n), P.Paced.pace(p), Nat.add(sent, Bool.pick(Nat, P.Paced.send(p), 1n, 0n)))

# a state with thread "bot" (and "owner"), its liveness as given
def live(stalled: Bool, +reason: String) -> M.State:
  M.State.apply(M.State.apply(M.State.apply(M.State.empty(), M.ThreadCreated{"bot", "p", "Bot", M.Local{}, "", "", "codex", "", 1n}),
      M.ThreadCreated{"owner", "p", "Owner", M.Local{}, "", "", "codex", "", 1n}),
    M.SettingSet{"liveness.bot", "{\"last\":5,\"nudged\":0,\"stalled\":" ++ Bool.pick(String, stalled, "true", "false") ++ ",\"reported\":false,\"reason\":\"" ++ reason ++ "\"}", 2n})

# a stamp from old to new (as one step) for a view of th
def stamp(+old: M.State, +new: M.State, +th: String) -> String:
  Nat.show(P.Touch.level(P.Touch.changes.at(old, new, [M.SettingSet{"liveness.bot", "{}", 3n}], P.Touch.none()), th))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("a progress stamp that changes no dot: its own view hears it", stamp(live(False{}, "w"), live(False{}, "w"), "bot"), "2")
    C.Check.str("a progress stamp that changes no dot: other views do not", stamp(live(False{}, "w"), live(False{}, "w"), "owner"), "0")
    C.Check.str("a reason change while not stalled shows nowhere", stamp(live(False{}, "w"), live(False{}, "x"), "owner"), "0")
    C.Check.str("a stall reaches other views' sidebars", stamp(live(False{}, "w"), live(True{}, "w"), "owner"), "1")
    C.Check.str("a recovery reaches other views' sidebars", stamp(live(True{}, "w"), live(False{}, "w"), "owner"), "1")
    C.Check.str("a shown reason change reaches other views", stamp(live(True{}, "w"), live(True{}, "x"), "owner"), "1")
    C.Check.str("a still stall: other views do not hear it", stamp(live(True{}, "w"), live(True{}, "w"), "owner"), "0")
    C.Check.str("other settings keep their touch", Nat.show(P.Touch.level(P.Touch.changes.at(live(False{}, "w"), live(False{}, "w"), [M.SettingSet{"new.setting", "x", 3n}], P.Touch.none()), "owner")), "2")
    C.Check.str("an empty liveness owner stays conservative", Nat.show(P.Touch.level(P.Touch.changes.at(live(False{}, "w"), live(False{}, "w"), [M.SettingSet{"liveness.", "x", 3n}], P.Touch.none()), "owner")), "2")
    C.Check.str("progress reaches its own view urgently", Nat.show(P.Touch.level(touched("liveness.owner"), "owner")), "2")
    C.Check.str("progress elsewhere uses sidebar cadence", Nat.show(P.Touch.level(touched("liveness.owner"), "visible")), "1")
    C.Check.str("unknown settings remain global", Nat.show(P.Touch.level(touched("new.setting"), "visible")), "2")
    C.Check.str("quiet receipt creates no view work", Nat.show(P.Touch.level(touched("autosettle.quiet.owner"), "owner")), "0")
    C.Check.str("output receipt creates no sidebar work", Nat.show(P.Touch.level(touched("autosettle.output.owner"), "visible")), "0")
    C.Check.str("read receipt creates no view work", Nat.show(P.Touch.level(touched("autosettle.read.owner"), "owner")), "0")
    C.Check.str("delivery receipt creates no view work", Nat.show(P.Touch.level(touched("autosettle.delivery.owner\tparent"), "parent")), "0")
    C.Check.str("report receipt creates no view work", Nat.show(P.Touch.level(touched("autosettle.report.owner"), "owner")), "0")
    C.Check.str("audience receipt creates no view work", Nat.show(P.Touch.level(touched("autosettle.audience.owner"), "owner")), "0")
    C.Check.str("delegation seed creates no view work", Nat.show(P.Touch.level(touched("autosettle.seed.owner"), "owner")), "0")
    C.Check.str("migration version creates no view work", Nat.show(P.Touch.level(touched("autosettle.version"), "visible")), "0")
    C.Check.str("empty metadata owner stays conservative", Nat.show(P.Touch.level(touched("autosettle.quiet."), "visible")), "2")
    C.Check.str("unknown autosettle key stays conservative", Nat.show(P.Touch.level(touched("autosettle.future.owner"), "visible")), "2")
    C.Check.str("version lookalike stays conservative", Nat.show(P.Touch.level(touched("autosettle.version.extra"), "visible")), "2")
    C.Check.str("settle policy remains global", Nat.show(P.Touch.level(touched("settle.idle.minutes"), "visible")), "2")
    C.Check.str("actual settlement reaches owner", Nat.show(P.Touch.level(P.Touch.changes([M.ThreadSettled{"owner", False{}, 1n}], P.Touch.none()), "owner")), "2")
    C.Check.str("actual settlement reaches sidebar", Nat.show(P.Touch.level(P.Touch.changes([M.ThreadSettled{"owner", False{}, 1n}], P.Touch.none()), "visible")), "1")
    C.Check.str("empty liveness owner stays conservative", Nat.show(P.Touch.level(touched("liveness."), "visible")), "2")
    C.Check.str("subagent owner is normalized", Nat.show(P.Touch.level(touched("liveness.@sub:owner"), "owner")), "2")
    C.Check.str("background stream has no sidebar effect", Nat.show(P.Touch.level(P.Touch.add(P.Touch.none(), P.Touch.live("other")), "visible")), "0")
    C.Check.str("1000 progress events yield only20 sidebar notices", Nat.show(burst(1000n, 1000n, P.Pace.fresh(), 0n)), "20")
    C.Check.str("20 mixed threads plus metadata yield only20 sidebar notices", Nat.show(mixed(1000n, 1000n, P.Pace.fresh(), 0n)), "20")
    C.Check.str("deferred progress is delivered at timer", yes(P.Paced.send(P.Pace.step(P.Paced.pace(P.Pace.step(P.Pace{1000n, 0n}, 1010n, 1n)), 1500n, 0n))), "yes")
