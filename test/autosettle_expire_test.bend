import Base
import ./check.bend as C
import ../src/core/autosettle.bend as A
import ../src/core/model.bend as M

def fixture(+turn: M.TurnState, +kind: String, +target: String, +once: Bool, more: List<&2, M.Change>) -> M.State:
  M.State.replay(List.append(&2, M.Change, [M.ProjectCreated{"p", "Project", "/tmp/autosettle", 1n},
    M.ThreadCreated{"owner", "p", "Owner", M.Local{}, "", "", "codex", "", 2n},
    M.ThreadCreated{"target", "p", "Target", M.Local{}, "", "", "codex", "", 2n},
    M.TurnChanged{"owner", turn, 3n},
    M.WakeSet{"wake", "owner", kind, target, ["end"], once, "Wait", 100n}], more), M.State.empty())

def after(+s: M.State, now: Nat) -> M.State:
  M.State.replay(A.Auto.changes(s, now), s)

def subscriptions(s: M.State) -> String:
  M.Setting.get(s, "wakes.by.owner", "")

def main() -> IO(Unit):
  +running = fixture(M.Running{}, "work", "ended", True{}, Nil{})
  +done = after(running, 1000n)
  do IO<Unit>:
    C.Check.str("active subscriber retires ended work after worker grace", subscriptions(done), "")
    C.Check.str("retiring stale work does not settle or stop its running owner", Bool.show(A.Auto.expired.done(M.Shelf.of(M.State.thread.get(done, "owner")), M.Thread.turn(M.State.thread.get(done, "owner")))), "False")
    C.Check.str("registration race keeps new work subscription", subscriptions(after(running, 999n)), "wake")
    C.Check.str("live work subscription remains", subscriptions(after(fixture(M.Running{}, "work", "live", True{}, [M.WorkSet{"target", "watch", "live\twatching", 4n}]), 1000n)), "wake")
    C.Check.str("uncertain timed-out work remains", subscriptions(after(fixture(M.Running{}, "work", "live", True{}, [M.WorkSet{"target", "watch", "live\twatching (timed out, still running)", 4n}]), 1000n)), "wake")
    C.Check.str("active file subscription is preserved", subscriptions(after(fixture(M.Running{}, "file", "/tmp/receipt", True{}, Nil{}), 1000n)), "wake")
    C.Check.str("active PR subscription is preserved", subscriptions(after(fixture(M.Running{}, "pr", "owner/repo#1", True{}, Nil{}), 1000n)), "wake")
    C.Check.str("active thread subscription is preserved", subscriptions(after(fixture(M.Running{}, "thread", "target", True{}, Nil{}), 1000n)), "wake")
    C.Check.str("unknown subscriptions remain conservative", subscriptions(after(fixture(M.Running{}, "future-kind", "ended", True{}, Nil{}), 1000n)), "wake")
    C.Check.str("repeat subscription to ended immutable work also retires", subscriptions(after(fixture(M.Running{}, "work", "ended", False{}, Nil{}), 1000n)), "")
    C.Check.str("failed owner stays failed after stale dependency cleanup", Bool.show(A.Auto.allow(M.Thread.turn(M.State.thread.get(after(fixture(M.Failed{}, "work", "ended", True{}, Nil{}), 1000n), "owner")), False{}, False{}, False{}, True{}, True{}, M.Active{}, M.NoOverride{})), "False")
    C.Check.str("disabled worker timer preserves subscriptions", subscriptions(after(fixture(M.Running{}, "work", "ended", True{}, [M.SettingSet{"settle.idle.minutes", "0", 4n}]), 1000n)), "wake")
    C.Check.str("worker timer config controls orphan expiry", subscriptions(after(fixture(M.Running{}, "work", "ended", True{}, [M.SettingSet{"settle.idle.minutes", "1", 4n}]), 160n)), "")
