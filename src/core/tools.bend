import Base
import ./model.bend as M
import ./icons.bend as I
import ./i18n.bend as LC
import ./msgs.bend as MSG

# The thread header's buttons, one list for every client (the window, the
# web, the phones): each an icon (core/icons.bend), a word or two, what it
# does in a sentence (the tooltip), and whether it is on. They come in
# groups, left to right: stopping the turn; where the thread sits in the
# list (pin, settle, snooze, archive, delete); forking; what shows beside
# it (the terminal, the panel). `gap` starts a group.
type Tool is Data:
  Tool{icon: String, label: String, tip: String, action: String, value: String, on: Bool, danger: Bool, gap: Bool}

# what the header shows of a thread
type Head is Data:
  Head{running: Bool, settled: Bool, pinned: Bool, snoozed: Bool, archived: Bool, del: Bool, forked: Bool, term: Bool, panel: Bool}

# settled by hand, or by itself (an answer the person has seen)
def Tools.settled(o: M.Override, auto: Bool) -> Bool:
  match o auto:
    case M.ForcedSettled{} _:
      True{}
    case M.NoOverride{} True{}:
      True{}
    case _ _:
      False{}

# Each in the device language (.loc); the plain names are English, for
# the laws, the web and the phones until they pass a language
def Tools.stop.loc(+l: LC.Loc, running: Bool) -> List<&2, Tool>:
  match running:
    case True{}:
      [Tool{"stop", MSG.T.composer.stop(l), "Stop the running turn", "interrupt", "", False{}, True{}, False{}}]
    case False{}:
      Nil{}

def Tools.stop(running: Bool) -> List<&2, Tool>:
  Tools.stop.loc(LC.En{}, running)

def Tools.pin.loc(+l: LC.Loc, p: Bool) -> Tool:
  match p:
    case True{}:
      Tool{"pin", MSG.T.tools.pinned(l), "Unpin: it sorts with the rest again", "unpin", "", True{}, False{}, True{}}
    case False{}:
      Tool{"pin", MSG.T.menu.pin(l), "Keep it at the top of the list", "pin", "", False{}, False{}, True{}}

def Tools.pin(p: Bool) -> Tool:
  Tools.pin.loc(LC.En{}, p)

def Tools.settle.loc(+l: LC.Loc, s: Bool) -> Tool:
  match s:
    case True{}:
      Tool{"circle-check", MSG.T.notify.settled(l), "Bring it back to the active threads", "unsettle", "", True{}, False{}, False{}}
    case False{}:
      Tool{"circle-check", MSG.T.menu.settle(l), "Done for now: it leaves the active threads", "settle", "", False{}, False{}, False{}}

def Tools.settle(s: Bool) -> Tool:
  Tools.settle.loc(LC.En{}, s)

# snoozed: it wakes; else it offers the snooze choices, through the
# client's own action (a, v)
def Tools.snooze.loc(+l: LC.Loc, s: Bool, a: String, v: String) -> Tool:
  match s:
    case True{}:
      Tool{"clock", MSG.T.sidebar.snoozed(l), "Wake it now", "snooze", "off", True{}, False{}, False{}}
    case False{}:
      Tool{"clock", MSG.T.tools.snooze(l), "Hide it until later", a, v, False{}, False{}, False{}}

def Tools.snooze(s: Bool, a: String, v: String) -> Tool:
  Tools.snooze.loc(LC.En{}, s, a, v)

def Tools.archive.loc(+l: LC.Loc, a: Bool) -> Tool:
  match a:
    case True{}:
      Tool{"archive", MSG.T.sidebar.archived(l), "Bring it back from the archive", "unarchive", "", True{}, False{}, False{}}
    case False{}:
      Tool{"archive", MSG.T.menu.archive(l), "Put it away in the archive", "archive", "", False{}, False{}, False{}}

def Tools.archive(a: Bool) -> Tool:
  Tools.archive.loc(LC.En{}, a)

# Delete asks first: once armed it reads "Delete thread" beside Cancel
def Tools.del.loc(+l: LC.Loc, on: Bool) -> List<&2, Tool>:
  match on:
    case True{}:
      [Tool{"trash", MSG.T.palette.delete(l), "Delete it for good", "delete", "", False{}, True{}, False{}},
       Tool{"x", MSG.T.common.cancel(l), "Keep it", "delete-no", "", False{}, False{}, False{}}]
    case False{}:
      [Tool{"trash", MSG.T.common.delete(l), "Delete this thread (asks first)", "delete", "", False{}, False{}, False{}}]

def Tools.del(on: Bool) -> List<&2, Tool>:
  Tools.del.loc(LC.En{}, on)

def Tools.fork.loc(+l: LC.Loc) -> Tool:
  Tool{"git-fork", MSG.T.tools.forkthread(l), "A new thread that goes on from here", "fork", "", False{}, False{}, True{}}

def Tools.fork() -> Tool:
  Tools.fork.loc(LC.En{})

def Tools.back.loc(+l: LC.Loc, forked: Bool) -> List<&2, Tool>:
  match forked:
    case True{}:
      [Tool{"undo", MSG.T.menu.bringback(l), "Send what this fork did to the thread it came from, with its next message", "back", "", False{}, False{}, False{}}]
    case False{}:
      Nil{}

def Tools.back(forked: Bool) -> List<&2, Tool>:
  Tools.back.loc(LC.En{}, forked)

def Tools.term.loc(+l: LC.Loc, on: Bool, a: String, v: String) -> Tool:
  Tool{"terminal", MSG.T.palette.terminal(l), "A shell in the thread's folder (ctrl+j)", a, v, on, False{}, True{}}

def Tools.term(on: Bool, a: String, v: String) -> Tool:
  Tools.term.loc(LC.En{}, on, a, v)

def Tools.panel.loc(+l: LC.Loc, on: Bool) -> Tool:
  Tool{"panel-right", MSG.T.thread.viewer(l), "Board, schematic, 3D, Mech, files, diff and browser beside the thread", "panel", "", on, False{}, False{}}

def Tools.panel(on: Bool) -> Tool:
  Tools.panel.loc(LC.En{}, on)

# the header's buttons, left to right; (sa, sv) opens the client's snooze
# choices and (ta, tv) its terminal
def Tools.head.loc(+l: LC.Loc, h: Head, sa: String, sv: String, ta: String, tv: String) -> List<&2, Tool>:
  match h:
    case Head{running, settled, pinned, snoozed, archived, del, forked, term, panel}:
      List.concat(&2, Tool, [Tools.stop.loc(l, running),
        [Tools.pin.loc(l, pinned), Tools.settle.loc(l, settled), Tools.snooze.loc(l, snoozed, sa, sv), Tools.archive.loc(l, archived)], Tools.del.loc(l, del),
        [Tools.fork.loc(l)], Tools.back.loc(l, forked),
        [Tools.term.loc(l, term, ta, tv), Tools.panel.loc(l, panel)]])

def Tools.head(h: Head, sa: String, sv: String, ta: String, tv: String) -> List<&2, Tool>:
  Tools.head.loc(LC.En{}, h, sa, sv, ta, tv)

# a button's label, which the first-run tour names it by
def Tools.label(t: Tool) -> String:
  match t:
    case Tool{_, +l, _, _, _, _, _, _}:
      l

def Tools.icons(ts: List<&2, Tool>) -> List<&2, String>:
  match ts:
    case Nil{}:
      Nil{}
    case Tool{i, _, _, _, _, _, _, _} <> t:
      i <> Tools.icons(t)

# every header this could show: every flag both ways
def Tools.every() -> List<&2, Tool>:
  List.concat(&2, Tool, [
    Tools.head(Head{True{}, True{}, True{}, True{}, True{}, True{}, True{}, True{}, True{}}, "", "", "", ""),
    Tools.head(Head{False{}, False{}, False{}, False{}, False{}, False{}, False{}, False{}, False{}}, "", "", "", "")])

# every button has a label and a tooltip to go with its icon
def Tools.labelled(ts: List<&2, Tool>) -> Bool:
  match ts:
    case Nil{}:
      True{}
    case Tool{_, +l, +tip, _, _, _, _, _} <> t:
      Bool.and(Bool.and(Bool.not(String.is_empty(l)), Bool.not(String.is_empty(tip))), Tools.labelled(t))

# the icons every header uses, none missing from the set
def Tools.missing() -> List<&2, String>:
  I.Icon.missing(Tools.icons(Tools.every()))
