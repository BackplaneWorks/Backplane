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
type HeaderTool is Data:
  HeaderTool{icon: String, label: String, tip: String, action: String, value: String, on: Bool, danger: Bool, gap: Bool}

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
def Tools.stop.loc(+l: LC.Loc, running: Bool) -> List<&2, HeaderTool>:
  match running:
    case True{}:
      [HeaderTool{"stop", MSG.T.composer.stop(l), MSG.T.tools.tip.stop(l), "interrupt", "", False{}, True{}, False{}}]
    case False{}:
      Nil{}

def Tools.stop(running: Bool) -> List<&2, HeaderTool>:
  Tools.stop.loc(LC.En{}, running)

# a turn that failed (an API error, a crash): Retry sends the person's
# last message again (client.bend's Ui.retry), where Stop sits while it runs
def Tools.retry.loc(+l: LC.Loc, failed: Bool) -> List<&2, HeaderTool>:
  match failed:
    case True{}:
      [HeaderTool{"repeat", MSG.T.tools.retry(l), MSG.T.tools.tip.retry(l), "retry", "", False{}, False{}, False{}}]
    case False{}:
      Nil{}

def Tools.retry(failed: Bool) -> List<&2, HeaderTool>:
  Tools.retry.loc(LC.En{}, failed)

# the turn's own button: Stop while it runs, Retry once it failed
def Tools.turn.loc(+l: LC.Loc, t: M.TurnState) -> List<&2, HeaderTool>:
  match t:
    case M.Running{}:
      Tools.stop.loc(l, True{})
    case M.Failed{}:
      Tools.retry.loc(l, True{})
    case _:
      Nil{}

def Tools.pin.loc(+l: LC.Loc, p: Bool) -> HeaderTool:
  match p:
    case True{}:
      HeaderTool{"pin", MSG.T.tools.pinned(l), MSG.T.tools.tip.unpin(l), "unpin", "", True{}, False{}, True{}}
    case False{}:
      HeaderTool{"pin", MSG.T.menu.pin(l), MSG.T.tools.tip.pin(l), "pin", "", False{}, False{}, True{}}

def Tools.pin(p: Bool) -> HeaderTool:
  Tools.pin.loc(LC.En{}, p)

def Tools.settle.loc(+l: LC.Loc, s: Bool) -> HeaderTool:
  match s:
    case True{}:
      HeaderTool{"circle-check", MSG.T.notify.settled(l), MSG.T.tools.tip.unsettle(l), "unsettle", "", True{}, False{}, False{}}
    case False{}:
      HeaderTool{"circle-check", MSG.T.menu.settle(l), MSG.T.tools.tip.settle(l), "settle", "", False{}, False{}, False{}}

def Tools.settle(s: Bool) -> HeaderTool:
  Tools.settle.loc(LC.En{}, s)

# snoozed: it wakes; else it offers the snooze choices, through the
# client's own action (a, v)
def Tools.snooze.loc(+l: LC.Loc, s: Bool, a: String, v: String) -> HeaderTool:
  match s:
    case True{}:
      HeaderTool{"clock", MSG.T.sidebar.snoozed(l), MSG.T.tools.tip.wake(l), "snooze", "off", True{}, False{}, False{}}
    case False{}:
      HeaderTool{"clock", MSG.T.tools.snooze(l), MSG.T.tools.tip.snooze(l), a, v, False{}, False{}, False{}}

def Tools.snooze(s: Bool, a: String, v: String) -> HeaderTool:
  Tools.snooze.loc(LC.En{}, s, a, v)

def Tools.archive.loc(+l: LC.Loc, a: Bool) -> HeaderTool:
  match a:
    case True{}:
      HeaderTool{"archive", MSG.T.sidebar.archived(l), MSG.T.tools.tip.unarchive(l), "unarchive", "", True{}, False{}, False{}}
    case False{}:
      HeaderTool{"archive", MSG.T.menu.archive(l), MSG.T.tools.tip.archive(l), "archive", "", False{}, False{}, False{}}

def Tools.archive(a: Bool) -> HeaderTool:
  Tools.archive.loc(LC.En{}, a)

# Delete asks first: once armed it reads "Delete thread" beside Cancel
def Tools.del.loc(+l: LC.Loc, on: Bool) -> List<&2, HeaderTool>:
  match on:
    case True{}:
      [HeaderTool{"trash", MSG.T.palette.delete(l), MSG.T.tools.tip.delete(l), "delete", "", False{}, True{}, False{}},
       HeaderTool{"x", MSG.T.common.cancel(l), MSG.T.tools.tip.keep(l), "delete-no", "", False{}, False{}, False{}}]
    case False{}:
      [HeaderTool{"trash", MSG.T.common.delete(l), MSG.T.tools.tip.askdelete(l), "delete", "", False{}, False{}, False{}}]

def Tools.del(on: Bool) -> List<&2, HeaderTool>:
  Tools.del.loc(LC.En{}, on)

def Tools.fork.loc(+l: LC.Loc) -> HeaderTool:
  HeaderTool{"git-fork", MSG.T.tools.forkthread(l), MSG.T.tools.tip.forkthread(l), "fork", "", False{}, False{}, True{}}

def Tools.fork() -> HeaderTool:
  Tools.fork.loc(LC.En{})

def Tools.back.loc(+l: LC.Loc, forked: Bool) -> List<&2, HeaderTool>:
  match forked:
    case True{}:
      [HeaderTool{"undo", MSG.T.menu.forkback(l), MSG.T.tools.tip.back(l), "back", "", False{}, False{}, False{}}]
    case False{}:
      Nil{}

def Tools.back(forked: Bool) -> List<&2, HeaderTool>:
  Tools.back.loc(LC.En{}, forked)

def Tools.term.loc(+l: LC.Loc, on: Bool, a: String, v: String) -> HeaderTool:
  HeaderTool{"terminal", MSG.T.palette.terminal(l), MSG.T.tools.tip.terminal(l), a, v, on, False{}, True{}}

def Tools.term(on: Bool, a: String, v: String) -> HeaderTool:
  Tools.term.loc(LC.En{}, on, a, v)

def Tools.panel.loc(+l: LC.Loc, on: Bool) -> HeaderTool:
  HeaderTool{"panel-right", MSG.T.thread.viewer(l), MSG.T.tools.tip.panel(l), "panel", "", on, False{}, False{}}

def Tools.panel(on: Bool) -> HeaderTool:
  Tools.panel.loc(LC.En{}, on)

# the header's buttons, left to right; (sa, sv) opens the client's snooze
# choices and (ta, tv) its terminal
def Tools.head.loc(+l: LC.Loc, h: Head, sa: String, sv: String, ta: String, tv: String) -> List<&2, HeaderTool>:
  match h:
    case Head{running, settled, pinned, snoozed, archived, del, forked, term, panel}:
      List.concat(&2, HeaderTool, [Tools.stop.loc(l, running),
        [Tools.pin.loc(l, pinned), Tools.settle.loc(l, settled), Tools.snooze.loc(l, snoozed, sa, sv), Tools.archive.loc(l, archived)], Tools.del.loc(l, del),
        [Tools.fork.loc(l)], Tools.back.loc(l, forked),
        [Tools.term.loc(l, term, ta, tv), Tools.panel.loc(l, panel)]])

def Tools.head(h: Head, sa: String, sv: String, ta: String, tv: String) -> List<&2, HeaderTool>:
  Tools.head.loc(LC.En{}, h, sa, sv, ta, tv)

# a button's label, which the first-run tour names it by
def Tools.label(t: HeaderTool) -> String:
  match t:
    case HeaderTool{_, +l, _, _, _, _, _, _}:
      l

def Tools.icons(ts: List<&2, HeaderTool>) -> List<&2, String>:
  match ts:
    case Nil{}:
      Nil{}
    case HeaderTool{i, _, _, _, _, _, _, _} <> t:
      i <> Tools.icons(t)

# every header this could show: every flag both ways
def Tools.every() -> List<&2, HeaderTool>:
  List.concat(&2, HeaderTool, [
    Tools.head(Head{True{}, True{}, True{}, True{}, True{}, True{}, True{}, True{}, True{}}, "", "", "", ""),
    Tools.head(Head{False{}, False{}, False{}, False{}, False{}, False{}, False{}, False{}, False{}}, "", "", "", ""), Tools.retry(True{})])

# every button has a label and a tooltip to go with its icon
def Tools.labelled(ts: List<&2, HeaderTool>) -> Bool:
  match ts:
    case Nil{}:
      True{}
    case HeaderTool{_, +l, +tip, _, _, _, _, _} <> t:
      Bool.and(Bool.and(Bool.not(String.is_empty(l)), Bool.not(String.is_empty(tip))), Tools.labelled(t))

# the icons every header uses, none missing from the set
def Tools.missing() -> List<&2, String>:
  I.Icon.missing(Tools.icons(Tools.every()))
