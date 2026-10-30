import Base
import ./check.bend as C
import ../src/core/watchdog.bend as W
import ../src/core/liveness.bend as L
import ../src/core/model.bend as M
import ../src/core/hub.bend as B

# A dormant project (model.bend's Dormant.*): the watchdog, the restart's
# carry and the tick's wake leave its threads alone; the same threads in a
# project that is not dormant are handled as always.

def shown(b: Bool) -> String:
  Bool.pick(String, b, "true", "false")

def base.k(pre: List<&2, M.Change>) -> M.State:
  M.State.replay(List.append(&2, M.Change, pre, [M.ProjectCreated{"p", "project", "/tmp/project", 1n},
    M.ThreadCreated{"worker", "p", "worker", M.Local{}, "", "", "codex", "", 1n},
    M.ThreadCreated{"launcher", "p", "launcher", M.Local{}, "", "", "codex", "", 1n},
    M.ThreadLaunched{"worker", "launcher", True{}, 1n},
    M.MessagePosted{"worker", "m1", M.User{}, "go", 10n},
    M.TurnChanged{"worker", M.Running{}, 10n},
    M.SettingSet{L.Liveness.key("worker"), L.Liveness.write(L.Live{10n, 0n, False{}, False{}, "blocking shell"}), 10n}]), M.State.empty())

def dormant() -> List<&2, M.Change>:
  [M.SettingSet{M.Dormant.key("p"), "on", 1n}]

def apply(+s: M.State, step: B.Step) -> M.State:
  M.State.replay(B.Queue.changes(step), s)

def tick(+s: M.State, +now: Nat) -> M.State:
  apply(s, W.Watchdog.after(s, now, W.Watchdog.tick(s, now)))

def nudge(+s: M.State) -> String:
  Nat.show(L.Liveness.nudged(W.Watchdog.live(tick(s, 1210n), "worker")))

def stalled(+s: M.State) -> String:
  shown(L.Liveness.stalled(W.Watchdog.live(tick(tick(s, 1210n), 1270n), "worker")))

def wakes(+s: M.State) -> String:
  Nat.show(List.length(&2, M.Change, B.Shelve.tick(B.Shelve.threads(s), M.State.navs(s), M.Work.kids.map(s), s, 100n)))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("the key", M.Dormant.key("p"), "project.dormant.p")
    C.Check.str("marked", shown(M.Dormant.thread(base.k(dormant()), "worker")), "true")
    C.Check.str("not marked", shown(M.Dormant.thread(base.k(Nil{}), "worker")), "false")
    C.Check.str("off is not dormant", shown(M.Dormant.thread(base.k([M.SettingSet{M.Dormant.key("p"), "off", 1n}]), "worker")), "false")
    C.Check.str("watchdog nudges a quiet thread", nudge(base.k(Nil{})), "1210")
    C.Check.str("dormant: no nudge", nudge(base.k(dormant())), "0")
    C.Check.str("watchdog stalls it after the grace", stalled(base.k(Nil{})), "true")
    C.Check.str("dormant: never stalled", stalled(base.k(dormant())), "false")
    C.Check.str("a restart carries a running turn on", shown(B.Carry.will(base.k(Nil{}), "worker")), "true")
    C.Check.str("dormant: no carry", shown(B.Carry.will(base.k(dormant()), "worker")), "false")
    C.Check.str("dormant: no deferred carry", shown(B.Carry.deferred(base.k(dormant()), "worker")), "false")
    C.Check.str("the tick wakes a self-settled thread that works", wakes(B.Shelve.demo([M.ThreadSettled{"t1", False{}, 3n}])), "1")
    C.Check.str("dormant: the tick reopens nothing", wakes(B.Shelve.demo([M.SettingSet{M.Dormant.key("p1"), "on", 1n}, M.ThreadSettled{"t1", False{}, 3n}])), "0")
