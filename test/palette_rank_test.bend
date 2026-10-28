import Base
import ./check.bend as C
import ../src/core/client.bend as U

# The command palette's order for a query (client.bend's Palette.filter and
# Palette.rank): the rows whose name is the query first, then names that
# start with it, then rows where every word starts a word, then the rest;
# each rank keeps the list's own order.

def item(+l: String) -> U.PItem:
  U.PItem{l, "a", ""}

def items(ls: List<&2, String>) -> List<&2, U.PItem>:
  match ls:
    case Nil{}: Nil{}
    case +l <> t: item(l) <> items(t)

def label(p: U.PItem) -> String:
  match p:
    case U.PItem{l, _, _}: l

def names(ps: List<&2, U.PItem>) -> List<&2, String>:
  match ps:
    case Nil{}: Nil{}
    case p <> t: label(p) <> names(t)

def labels(ps: List<&2, U.PItem>) -> String:
  String.join(names(ps), " | ")

def ranked(ls: List<&2, String>, +q: String) -> String:
  labels(U.Palette.filter(items(ls), q))

def cmds() -> List<&2, String>:
  ["New subthread of current thread", "Uberbot chat · Ctrl+Shift+U", "Previous thread · Ctrl+Shift+[", "New thread · Ctrl+N",
   "Pin thread", "Settle thread · Ctrl+Shift+S", "Snooze thread for 1 hour", "Settings", "New thread", "New thread"]

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("new thread: New thread first, the subthread after the threads named so",
      ranked(cmds(), "new thread"), "New thread · Ctrl+N | New thread | New thread | New subthread of current thread")
    C.Check.str("any case and spaces around", ranked(cmds(), "  NEW Thread "), "New thread · Ctrl+N | New thread | New thread | New subthread of current thread")
    C.Check.str("a name that starts with the query before word starts", ranked(cmds(), "sett"), "Settle thread · Ctrl+Shift+S | Settings")
    C.Check.str("the exact name before a longer one", ranked(["Settings viewer", "Settings"], "settings"), "Settings | Settings viewer")
    C.Check.str("word starts before words inside", ranked(["Rethreading", "New thread · Ctrl+N", "Thread list"], "thread"), "Thread list | New thread · Ctrl+N | Rethreading")
    C.Check.str("a word starts after a slash or a dash", ranked(["busbar", "fix/usb-c", "usb hub"], "usb"), "usb hub | fix/usb-c | busbar")
    C.Check.str("the keys are not the name", ranked(["Diff · Ctrl+D", "Ctrl"], "ctrl"), "Ctrl | Diff · Ctrl+D")
    C.Check.str("rows the words miss go", ranked(cmds(), "snooze hour"), "Snooze thread for 1 hour")
    C.Check.str("nothing matches", ranked(cmds(), "zzz"), "")
    C.Check.str("no query: every row in order", ranked(["b", "a", "c"], ""), "b | a | c")
    C.Check.str("a blank query too", ranked(["b", "a"], "   "), "b | a")
    C.Check.str("rank: the name", Nat.show(U.Palette.rank("new thread", "New thread · Ctrl+N")), "0")
    C.Check.str("rank: a name's start", Nat.show(U.Palette.rank("new", "New thread · Ctrl+N")), "1")
    C.Check.str("rank: word starts", Nat.show(U.Palette.rank("new thread", "New subthread of current thread")), "2")
    C.Check.str("rank: inside words", Nat.show(U.Palette.rank("hread", "New thread")), "3")
    return Unit{}
