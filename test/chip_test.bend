import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/usage.bend as UZ

# A thread's Boat chip from the Boat runner's status.json "threads" (the
# shape agreed in bp-coord/usage/DATA-CONTRACT.md): logged only when it
# changes, against the state; stale when a running job's runner stopped
# telling for 3 minutes; a warm sandbox past its idle time is gone.

def log() -> List<&2, M.Change>:
  [M.ProjectCreated{"p1", "demo", "/tmp/demo", 1n},
   M.ThreadCreated{"t1", "p1", "One", M.Local{}, "", "", "claude", "", 1n},
   M.ThreadCreated{"t2", "p1", "Two", M.Local{}, "", "", "claude", "", 1n},
   M.ThreadCreated{"t3", "p1", "Three", M.Local{}, "", "", "claude", "", 1n},
   M.SettingSet{"boat.thread.t3", "running/default/1/0", 2n}]

def status(+gen: String) -> String:
  "{\"generated_at\":" ++ gen ++ ",\"threads\":{\"t1\":{\"state\":\"running\",\"size\":\"large\",\"jobs\":2,\"idle_until\":null,\"since\":900}," ++
    "\"t2\":{\"state\":\"warm\",\"size\":\"default\",\"jobs\":0,\"idle_until\":1720,\"since\":950}}}"

def show(cs: List<&2, M.Change>) -> String:
  match cs:
    case Nil{}:
      ""
    case M.SettingSet{k, v, _} <> t:
      k ++ "=" ++ v ++ "\n" ++ show(t)
    case _ <> t:
      "?\n" ++ show(t)

def main() -> IO(Unit):
  do IO<Unit>:
    +st : M.State = M.State.replay(log(), M.State.empty())
    +cs : List<&2, M.Change> = UZ.Chip.changes(st, status("990"), 1000n)
    C.Check.str("changes: t1 and t2 set, t3 cleared", Nat.show(List.length(&2, M.Change, cs)), "3")
    +s1 : M.State = M.State.replay(cs, st)
    C.Check.str("t1 running", UZ.Chip.of(s1, "t1", 1000n), "Boat · large · 2 running")
    C.Check.str("t2 warm", UZ.Chip.of(s1, "t2", 1000n), "Boat · warm 12m")
    C.Check.str("t3 gone", UZ.Chip.of(s1, "t3", 1000n), "")
    C.Check.str("same status again: nothing logged", Nat.show(List.length(&2, M.Change, UZ.Chip.changes(s1, status("990"), 1010n))), "0")
    C.Check.str("warm past its idle time: gone, never 0m", UZ.Chip.of(s1, "t2", 1720n), "")
    +s2 : M.State = M.State.replay(UZ.Chip.changes(s1, status("990"), 1200n), s1)
    C.Check.str("running, runner quiet over 3 min: stale", UZ.Chip.of(s2, "t1", 1200n), "Boat · ? (stale)")
    C.Check.str("warm is never stale", UZ.Chip.of(s2, "t2", 1200n), "Boat · warm 9m")
    C.Check.str("an unreadable status changes nothing", Nat.show(List.length(&2, M.Change, UZ.Chip.changes(s1, "", 1200n))), "0")
