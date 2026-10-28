import Base
import ./check.bend as C
import ../src/core/client.bend as U
import ../src/core/model.bend as M
import ../src/web/workspace.bend as WS

# The command palette lists only the thread commands that apply to the open
# thread, as its ▾ does (workspace.bend's WS.cmds): Pin or Unpin, Settle or
# Mark active, Archive or Unarchive, the snoozes or Wake.

def log() -> List<&2, M.Change>:
  [M.ProjectCreated{"p", "Project", "/fixture", 1n},
   M.ThreadCreated{"t", "p", "Thread", M.Local{}, "", "/fixture", "codex", "", 1n}]

# thread t open, after es, at 1000
def at(es: List<&2, M.Change>) -> U.Ui:
  U.Ui.select.k(U.Ui.fold(List.append(&2, M.Change, log(), es), U.Ui.init(1000n)), "t")

def labels(xs: List<&2, U.PItem>) -> List<&2, String>:
  match xs:
    case Nil{}: Nil{}
    case U.PItem{l, _, _} <> t: l <> labels(t)

def shelf(+u: U.Ui) -> String:
  String.join(labels(WS.WS.cmds.shelf(WS.WS.cmds(u))), ",")

def snooze(+u: U.Ui) -> String:
  String.join(labels(WS.WS.cmds.snooze(WS.WS.cmds(u))), ",")

# how many of the palette's commands read exactly l
def count(xs: List<&2, String>, +l: String) -> Nat:
  match xs:
    case Nil{}: 0n
    case +x <> t: Nat.add(Bool.pick(Nat, String.eq(x, l), 1n, 0n), count(t, l))

def has(+u: U.Ui, +l: String) -> String:
  Nat.show(count(labels(WS.WS.commands(u)), l))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("plain: pin, settle, archive", shelf(at([])), "Pin thread,Settle thread · Ctrl+Shift+S,Archive thread")
    C.Check.str("plain: the snoozes", snooze(at([])), "Snooze thread for 1 hour,Snooze thread until tomorrow")
    C.Check.str("pinned: unpin", shelf(at([M.ThreadPinned{"t", True{}, 2n}])), "Unpin thread,Settle thread · Ctrl+Shift+S,Archive thread")
    C.Check.str("settled: mark active", shelf(at([M.ThreadSettled{"t", True{}, 2n}])), "Pin thread,Mark thread active,Archive thread")
    C.Check.str("archived: unarchive", shelf(at([M.ThreadArchived{"t", True{}, 2n}])), "Pin thread,Settle thread · Ctrl+Shift+S,Unarchive thread")
    C.Check.str("snoozed: wake", snooze(at([M.ThreadSnoozed{"t", 5000n, 2n}])), "Wake thread")
    C.Check.str("snooze over: the snoozes", snooze(at([M.ThreadSnoozed{"t", 900n, 2n}])), "Snooze thread for 1 hour,Snooze thread until tomorrow")
    C.Check.str("listed once: pin", has(at([]), "Pin thread") ++ has(at([]), "Unpin thread"), "10")
    C.Check.str("listed once: archive", has(at([M.ThreadArchived{"t", True{}, 2n}]), "Archive thread") ++ has(at([M.ThreadArchived{"t", True{}, 2n}]), "Unarchive thread"), "01")
    C.Check.str("listed once: wake", has(at([M.ThreadSnoozed{"t", 5000n, 2n}]), "Wake thread") ++ has(at([M.ThreadSnoozed{"t", 5000n, 2n}]), "Snooze thread for 1 hour"), "10")
    C.Check.str("the rest stay", has(at([]), "Fork thread") ++ has(at([]), "Delete thread…") ++ has(at([]), "Settings"), "111")
