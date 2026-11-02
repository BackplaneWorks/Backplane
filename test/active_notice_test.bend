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

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("progress reaches its own view urgently", Nat.show(P.Touch.level(touched("liveness.owner"), "owner")), "2")
    C.Check.str("progress elsewhere uses sidebar cadence", Nat.show(P.Touch.level(touched("liveness.owner"), "visible")), "1")
    C.Check.str("unknown settings remain global", Nat.show(P.Touch.level(touched("new.setting"), "visible")), "2")
    C.Check.str("empty liveness owner stays conservative", Nat.show(P.Touch.level(touched("liveness."), "visible")), "2")
    C.Check.str("subagent owner is normalized", Nat.show(P.Touch.level(touched("liveness.@sub:owner"), "owner")), "2")
    C.Check.str("background stream has no sidebar effect", Nat.show(P.Touch.level(P.Touch.add(P.Touch.none(), P.Touch.live("other")), "visible")), "0")
    C.Check.str("1000 progress events yield only20 sidebar notices", Nat.show(burst(1000n, 1000n, P.Pace.fresh(), 0n)), "20")
    C.Check.str("deferred progress is delivered at timer", yes(P.Paced.send(P.Pace.step(P.Paced.pace(P.Pace.step(P.Pace{1000n, 0n}, 1010n, 1n)), 1500n, 0n))), "yes")
