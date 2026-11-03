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

# what the palette offers for a typed query, its labels in order
def found(+q: String) -> String:
  String.join(labels(WS.WS.items(U.Palette.set(at([]), q, 0n))), ",")

def ui.of(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}: u

# running the first item found for q: Settings up, and its page
def ran(+q: String) -> String:
  +u = ui.of(WS.WS.palette.run(U.Palette.set(at([]), q, 0n), 0n))
  Bool.show(U.Ui.flag(u, "settings")) ++ ":" ++ U.Settings.page(u)

# with a bot "scout" (its chat thread b1)
def with.bot() -> U.Ui:
  at([M.ThreadCreated{"b1", "", "scout", M.Local{}, "", "", "claude", "", 2n}, M.BotSet{"bot1", "scout", "b1", 3n, "", 2n}])

def found.at(+u: U.Ui, +q: String) -> String:
  String.join(labels(WS.WS.items(U.Palette.set(u, q, 0n))), ",")

def ran.routines(+u: U.Ui) -> String:
  +r = ui.of(WS.WS.palette.run(U.Palette.set(u, "routines", 0n), 0n))
  U.Ui.scratch(r, "@bot") ++ ":" ++ U.Ui.scratch(r, "@btab")

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
    # a new person types what they want (found on the e2e first-week walk:
    # "release", "devices", "pair" found nothing)
    C.Check.str("release is found", found("release"), "Release: tag and publish this project")
    C.Check.str("pair finds Devices first", String.join(List.take(&2, String, String.split(found("pair"), ','), 1n), ""), "Settings: Devices · Pair a phone or another computer")
    C.Check.str("a Settings page by what it is for", found("theme"), "Settings: Appearance · Theme, text size, language and sidebar")
    C.Check.str("running it opens that page", ran("pair"), "True:devices")
    C.Check.str("no bot: routines says how to get one", found("routines"), "Routines: add a bot first (a bot runs routines on a schedule),Settings: Time zone · Local time for routines and reminders")
    C.Check.str("a bot: its routines", found.at(with.bot(), "routines"), "Routines: scout · what it runs and when,Settings: Time zone · Local time for routines and reminders")
    C.Check.str("running it opens the bot's Routines tab", ran.routines(with.bot()), "bot1:routines")
    C.Check.str("idle hides Stop", has(at([]), "Stop the running turn · Ctrl+."), "0")
    C.Check.str("running offers Stop once", has(at([M.TurnChanged{"t", M.Running{}, 2n}]), "Stop the running turn · Ctrl+."), "1")
    C.Check.str("completed hides Stop", has(at([M.TurnChanged{"t", M.Completed{}, 2n}]), "Stop the running turn · Ctrl+."), "0")
