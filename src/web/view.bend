import Base
import ../core/title.bend as TTL
import ../core/msgs.bend as MSG
import ../core/kexport.bend as KEX
import ../core/json.bend as J
import ../core/kihealth.bend as KH
import ../core/model.bend as M
import ../core/i18n.bend as I
import ./dom.bend as D
import ./icon.bend as WI
import ../core/tools.bend as TO
import ../core/client.bend as U
import ../core/usage.bend as UZ
import ../core/attach.bend as AT
import ../core/vt.bend as VT
import ../core/diff.bend as DF
import ../core/mech.bend as MC
import ../core/look.bend as LO
import ../core/board.bend as B
import ../core/layers.bend as LY
import ../core/schematic.bend as SC
import ../core/hier.bend as HI
import ../core/images.bend as IM
import ../core/fold.bend as FO
import ../core/subs.bend as SB
import ../core/badge.bend as BG
import ../core/todo.bend as TD
import ../core/bot.bend as BT
import ../core/botui.bend as BI
import ./bots.bend as WB
import ../core/voice.bend as VC
import ../core/hist.bend as HS
import ../core/review.bend as RVW
import ../core/emb.bend as EM
import ../core/qr.bend as QR
import ../core/onboard.bend as OB
import ../core/sched.bend as SCH
import ../core/tz.bend as TZ
import ../core/stamp.bend as STP
import ../core/order.bend as OR

# Ui -> Node. Every decision (order, shelf, status) comes from core defs
# with laws behind them; this file only lays them out.

# Shared display words for the web and phone subagent panels. Counts use raw
# decimal strings, preserving the English bytes even above a thousand.
def View.sub.count(+n: Nat) -> String:
  Bool.pick(String, Nat.is_eq(n, 0n), "none", Nat.show(n))

def View.sub.badge(+lc: I.Loc, +n: Nat) -> String:
  Bool.pick(String, Nat.is_eq(n, 0n), "", MSG.T.subagents.badge(lc, Nat.show(n), Bool.pick(String, Nat.is_eq(n, 1n), "one", "other")))

def View.sub.busy(+lc: I.Loc, +n: Nat) -> String:
  Bool.pick(String, Nat.is_eq(n, 0n), "", MSG.T.subagents.busy(lc, Nat.show(n)))

def View.sub.panel(+lc: I.Loc, +rs: List<&2, SB.SubRow>) -> String:
  MSG.T.subagents.panel(lc, View.sub.count(SB.Subs.waiting(rs)), View.sub.count(SB.Subs.live(rs)), Nat.show(SB.Subs.finished_n(rs)))

def View.sub.cancel.mode(+act: String) -> String:
  Bool.pick(String, String.eq(act, "lock-release"), "release", Bool.pick(String, Bool.or(String.eq(act, "lock-cancel"), String.eq(act, "wake-cancel")), "cancel", "stop"))

# The shared model keeps protocol phases; their displayed words follow the frame.
def View.phase.label(+lc: I.Loc, p: M.Phase) -> String:
  match p:
    case M.PhApproval{}: MSG.T.phase.approval(lc)
    case M.PhInput{}: MSG.T.phase.input(lc)
    case M.PhWorking{}: MSG.T.phase.working(lc)
    case M.PhMonitoring{+n}: MSG.T.phase.monitoring(lc, Nat.show(n), Bool.pick(String, Nat.is_eq(n, 1n), "one", "other"))
    case M.PhFailed{}: MSG.T.notify.failed(lc)
    case M.PhComplete{}: MSG.T.phase.complete(lc)
    case M.PhStopped{}: MSG.T.phase.stopped(lc)
    case M.PhIdle{}: ""

# Sidebar
# -------

def VLabel.choice(+lc: I.Loc, v: String) -> String:
  match v:
    case "": MSG.T.common.default(lc)
    case "on": MSG.T.option.on(lc)
    case "off": MSG.T.option.off(lc)
    case "dark": MSG.T.option.dark(lc)
    case "light": MSG.T.option.light(lc)
    case "projects": MSG.T.option.projects(lc)
    case "active": MSG.T.option.active(lc)
    case "stable": MSG.T.option.stable(lc)
    case "nightly": MSG.T.option.nightly(lc)
    case x: x

def VLabel.option.k(+lc: I.Loc, known: Bool, +v: String) -> String:
  match known:
    case True{}: VLabel.choice(lc, v)
    case False{}: Bool.pick(String, String.is_empty(v), MSG.T.common.default(lc), v)

def VLabel.option(+lc: I.Loc, +key: String, v: String) -> String:
  VLabel.option.k(lc, Bool.or(Bool.or(String.eq(key, "order.enabled"), String.eq(key, "order.estimates")), Bool.or(String.eq(key, "restart.continue"), Bool.or(String.eq(key, "ui.theme"), Bool.or(String.eq(key, "sidebar.view"), Bool.or(String.eq(key, "update.channel"), Bool.or(String.eq(key, "kicad.ipc"), Bool.or(String.starts_with(key, "notify."), String.starts_with(key, "sync.")))))))), v)

# Display labels only: metadata keys and submitted chip values stay unchanged.
def VLabel.effort(+lc: I.Loc, +e: String) -> String:
  match e:
    case "": MSG.T.model.level.auto(lc)
    case "auto": MSG.T.model.level.auto(lc)
    case "none": MSG.T.model.level.none(lc)
    case "minimal": MSG.T.model.level.minimal(lc)
    case "low": MSG.T.model.level.low(lc)
    case "medium": MSG.T.model.level.medium(lc)
    case "high": MSG.T.model.level.high(lc)
    case "xhigh": MSG.T.model.level.xhigh(lc)
    case "max": MSG.T.model.level.max(lc)
    case x: x

def VLabel.chip(+lc: I.Loc, +e: String) -> String:
  Bool.pick(String, String.starts_with(e, "@"), U.Chip.name(e), VLabel.effort(lc, e))

def VLabel.model(+lc: I.Loc, provider: String, +model: String) -> String:
  provider ++ " · " ++ Bool.pick(String, String.is_empty(model), MSG.T.common.default(lc), model)

def VLabel.effort.label(+lc: I.Loc, +label: String, +e: String) -> String:
  Bool.pick(String, String.is_empty(e), label, label ++ " · " ++ VLabel.effort(lc, e))

type PRow is Data:
  PRow{rank: Nat, project: M.Project, shelves: M.Shelves}

def PRow.before(a: PRow, b: PRow) -> Bool:
  match a b:
    case PRow{ra, _, _} PRow{rb, _, _}:
      Nat.is_ge(ra, rb)

def View.latest(ts: List<&2, M.Thread>, +acc: Nat) -> Nat:
  match ts:
    case Nil{}:
      acc
    case M.Thread{_, _, _, _, +a, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _} <> t:
      View.latest(t, M.Time.max(acc, a))

def View.rows.k(removed: Bool, p: M.Project, +mine: List<&2, M.Thread>, +c: Nat, rest: List<&2, PRow>) -> List<&2, PRow>:
  match removed:
    case True{}:
      rest
    case False{}:
      PRow{View.latest(mine, c), p, M.Sidebar.order(M.Sidebar.split(mine))} <> rest

# a project's place in the sidebar: its latest thread activity (or its
# creation), newest first, as T3Code's default "updated" order
def View.rows(ps: List<&2, M.Project>, +ts: List<&2, M.Thread>) -> List<&2, PRow>:
  match ps:
    case Nil{}:
      Nil{}
    case M.Project{+id, ti, r, +c, +rm} <> t:
      View.rows.k(rm, M.Project{id, ti, r, c, rm}, M.Thread.filter_project(ts, id), c, View.rows(t, ts))

# a sidebar row: its status dot, title and age (a snoozed row: the time
# left); rows of a shelf with manual order get move buttons on hover
# a row with subagents at work says how many where its age goes
def View.note(+badge: String, +other: String) -> D.Node:
  Bool.pick(D.Node, String.is_empty(badge), D.Dom.span("ago", other), D.Dom.span("ago agents", badge))

# A piece of the page left for the host to make: a "bp-memo" element keyed
# like the node it stands for, its attributes all it is made from ("fn"
# first, saying how), its one child a text. host.js makes it with
# app.bend's make (View.make) and keeps what it made while the element stays
# the same, attributes and text, so a piece is made once, not every frame
# (a message's markdown, links and images were most of one), and patch
# skips it whole. View.expand makes them all in Bend, keeping nothing (the
# tests).
def View.memo.loc(+lc: I.Loc, +key: String, as: List<&2, D.Attr>, +text: String) -> D.Node:
  D.El{"bp-memo", key, List.append(&2, D.Attr, as, [D.At{"lang", I.I18n.tag(lc)}]), [D.Txt{text}]}

def View.memo(+key: String, as: List<&2, D.Attr>, +text: String) -> D.Node:
  View.memo.loc(I.En{}, key, as, text)

def View.attr(as: List<&2, D.Attr>, +name: String) -> String:
  match as:
    case Nil{}:
      ""
    case D.At{+n, +v} <> t:
      Bool.pick(String, String.eq(n, name), v, View.attr(t, name))
    case _ <> t:
      View.attr(t, name)

def View.txt(ns: List<&2, D.Node>) -> String:
  match ns:
    case D.Txt{+x} <> _:
      x
    case _:
      ""

def View.row.k.loc(+lc: I.Loc, +id: String, +title: String, +ago: String, +badge: String, +dot: String, +sel: String, +movable: Bool) -> D.Node:
  D.El{"div", id, [D.At{"class", Bool.pick(String, String.eq(id, sel), "thread on", "thread")}, D.At{"data-value", id}, D.On{"click", "select"}, D.On{"contextmenu", "row-menu"}],
    D.Dom.cat([D.Dom.span("dot " ++ dot, ""), D.Dom.span("title", title)],
      D.Dom.cat(Bool.pick(List<&2, D.Node>, movable, [D.El{"span", "", [D.At{"class", "moves"}], [D.Dom.button("move", "move", id ++ "|up", MSG.T.sidebar.move.up(lc), "↑"), D.Dom.button("move", "move", id ++ "|down", MSG.T.sidebar.move.down(lc), "↓")]}], Nil{}),
        [View.note(badge, ago)]))}

# a sidebar row is made by the host from these alone (View.make)

def View.row.k(+id: String, +title: String, +ago: String, +badge: String, +dot: String, +sel: String, +movable: Bool) -> D.Node:
  View.row.k.loc(I.En{}, id, title, ago, badge, dot, sel, movable)

def View.row.m.loc(+lc: I.Loc, +id: String, +title: String, +ago: String, +badge: String, +dot: String, +sel: String, +movable: Bool) -> D.Node:
  View.memo.loc(lc, id, [D.At{"fn", "row"}, D.At{"ago", ago}, D.At{"badge", badge}, D.At{"dot", dot}, D.At{"on", Bool.pick(String, String.eq(id, sel), "1", "")}, D.At{"mov", Bool.pick(String, movable, "1", "")}], title)

def View.row.m(+id: String, +title: String, +ago: String, +badge: String, +dot: String, +sel: String, +movable: Bool) -> D.Node:
  View.row.m.loc(I.En{}, id, title, ago, badge, dot, sel, movable)

def View.row(+lc: I.Loc, +t: M.Thread, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat, +movable: Bool, +snoozed: Bool) -> D.Node:
  match t:
    case M.Thread{+id, _, +title, _, +a, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      View.row.m.loc(lc, id, U.Label.title(st, id, TTL.Title.shown(lc, title)), Bool.pick(String, snoozed, I.I18n.ago(lc, U.Side.until(st, id), now), I.I18n.ago(lc, now, a)), View.sub.badge(lc, SB.Subs.count.ix(ix, st, id)), M.Phase.name(U.Side.phase.ix(ix, st, t)), sel, movable)

def View.rows_in(+lc: I.Loc, ts: List<&2, M.Thread>, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat, +movable: Bool, +snoozed: Bool) -> List<&2, D.Node>:
  match ts:
    case Nil{}:
      Nil{}
    case +t <> rest:
      View.row(lc, t, ix, st, sel, now, movable, snoozed) <> View.rows_in(lc, rest, ix, st, sel, now, movable, snoozed)

def View.rows_of(+lc: I.Loc, ts: List<&2, M.Thread>, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat) -> List<&2, D.Node>:
  View.rows_in(lc, ts, ix, st, sel, now, False{}, False{})

def View.shelf.k(+lc: I.Loc, open: Bool, +key: String, +label: String, ts: List<&2, M.Thread>, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat) -> List<&2, D.Node>:
  match open:
    case True{}:
      WI.WIcon.button("shelf open", "toggle-settled", key, MSG.T.common.hide(lc), "chevron-down", 14, label) <> View.rows_of(lc, ts, ix, st, sel, now)
    case False{}:
      [WI.WIcon.button("shelf", "toggle-settled", key, MSG.T.common.show(lc), "chevron-right", 14, label)]

# a collapsible shelf (Settled, Archived): its label and count, then its
# rows when open; nothing when empty
def View.shelf(+lc: I.Loc, open: Bool, +key: String, +name: String, ts: List<&2, M.Thread>, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat) -> List<&2, D.Node>:
  match ts:
    case Nil{}:
      Nil{}
    case +t <> +rest:
      View.shelf.k(lc, open, key, MSG.T.sidebar.shelf.count(lc, name, Nat.show(List.length(&2, M.Thread, t <> rest))), t <> rest, ix, st, sel, now)

# snoozed threads, soonest to wake first
def View.snoozed(+lc: I.Loc, ts: List<&2, M.Thread>, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat) -> List<&2, D.Node>:
  match ts:
    case Nil{}:
      Nil{}
    case +t <> +rest:
      D.Dom.span("shelf-label", MSG.T.sidebar.snoozed(lc)) <> View.rows_in(lc, t <> rest, ix, st, sel, now, False{}, True{})

# a project's badge editor, open under its name: its tints, then its
# icons (a click sets one and leaves it open; a click anywhere else, on the
# shade behind it, closes it)
def View.swatches(cs: List<&2, String>, +cur: String, +id: String) -> List<&2, D.Node>:
  match cs:
    case Nil{}:
      Nil{}
    case +c <> t:
      D.El{"button", "", [D.At{"class", Bool.pick(String, String.eq(c, cur), "swatch on", "swatch")}, D.At{"style", "background:" ++ BG.Badge.css(c)}, D.At{"title", c},
        D.At{"data-value", BG.Badge.color_key(id) ++ "=" ++ c}, D.On{"click", "look-set"}], Nil{}} <> View.swatches(t, cur, id)

def View.glyphs(ns: List<&2, String>, +cur: String, +id: String) -> List<&2, D.Node>:
  match ns:
    case Nil{}:
      Nil{}
    case +n <> t:
      WI.WIcon.button(Bool.pick(String, String.eq(n, cur), "glyph on", "glyph"), "look-set", BG.Badge.icon_key(id) ++ "=" ++ n, n, n, 16, "") <> View.glyphs(t, cur, id)

# its last row removes the project (two clicks: client.bend's
# Ui.remove_project)
def View.look.loc(+lc: I.Loc, open: Bool, +st: M.State, +id: String, +rm: Bool) -> List<&2, D.Node>:
  match open:
    case True{}:
      [D.El{"div", "", [D.At{"class", "shade"}, D.At{"data-value", ""}, D.On{"click", "look"}], Nil{}},
       D.El{"div", "", [D.At{"class", "look"}], [
         D.El{"div", "", [D.At{"class", "swatches"}], View.swatches(BG.Badge.colors(), BG.Badge.color(st, id), id)},
         D.El{"div", "", [D.At{"class", "glyphs"}], View.glyphs(BG.Badge.icons(), BG.Badge.icon(st, id), id)},
         D.Dom.button("remove", "proj-remove", id, MSG.T.sidebar.remove.note(lc), Bool.pick(String, rm, MSG.T.menu.remove.confirm(lc), MSG.T.menu.project.remove(lc)))]}]
    case False{}:
      Nil{}

def View.look(open: Bool, +st: M.State, +id: String, +rm: Bool) -> List<&2, D.Node>:
  View.look.loc(I.En{}, open, st, id, rm)

def View.project(+lc: I.Loc, r: U.SRow, +quiet: Bool, +open: Map<&2, Bool>, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat, +look: String, +rm: String) -> D.Node:
  match r:
    case U.SRow{_, M.Project{+id, title, root, _, _}, M.Shelves{pinned, active, settled, hidden, snoozed}}:
      D.El{"section", id, [D.At{"class", Bool.pick(String, Bool.or(quiet, U.Label.syncing(st, id)), Bool.pick(String, U.Label.syncing(st, id), "project quiet syncing", "project quiet"), "project")}, D.At{"style", "--tint:" ++ BG.Badge.css_of(st, id)}, D.At{"data-drag-to", id}, D.At{"data-drag-act", "reorder-project"}],
        List.concat(&2, D.Node, [[D.El{"div", "", [D.At{"class", "project-head"}, D.At{"title", root}, D.At{"draggable", "true"}, D.At{"data-drag", id}, D.At{"data-drag-act", "reorder-project"}, D.At{"data-value", id}, D.On{"contextmenu", "proj-menu"}],
            [WI.WIcon.button("badge", "look", id, MSG.T.menu.look(lc), BG.Badge.icon(st, id), 14, ""), D.Dom.span("name", U.Label.title(st, id, title)), WI.WIcon.button("icon", "new-thread", id, MSG.T.thread.new(lc), "plus", 16, "")]}],
          View.look.loc(lc, String.eq(look, id), st, id, String.eq(rm, id)),
          View.rows_in(lc, pinned, ix, st, sel, now, True{}, False{}),
          View.rows_in(lc, active, ix, st, sel, now, True{}, False{}),
          View.snoozed(lc, snoozed, ix, st, sel, now),
          View.shelf(lc, U.Flag.get(open, id), id, MSG.T.notify.settled(lc), settled, ix, st, sel, now),
          View.shelf(lc, U.Flag.get(open, "arch:" ++ id), "arch:" ++ id, MSG.T.sidebar.archived(lc), M.Archive.of(hidden), ix, st, sel, now)])}

def View.projects(+lc: I.Loc, rs: List<&2, U.SRow>, +quiet: Bool, +open: Map<&2, Bool>, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat, +look: String, +rm: String) -> List<&2, D.Node>:
  match rs:
    case Nil{}:
      Nil{}
    case r <> t:
      View.project(lc, r, quiet, open, ix, st, sel, now, look, rm) <> View.projects(lc, t, quiet, open, ix, st, sel, now, look, rm)

def View.older.open(+lc: I.Loc, on: Bool, rs: List<&2, U.SRow>, +open: Map<&2, Bool>, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat, +look: String, +rm: String) -> List<&2, D.Node>:
  match on:
    case True{}:
      View.projects(lc, rs, True{}, open, ix, st, sel, now, look, rm)
    case False{}:
      Nil{}

# the older projects (quiet for Quiet.days) folded under one row; a search
# shows them in place
def View.older.k(+lc: I.Loc, searching: Bool, +n: Nat, rs: List<&2, U.SRow>, +open: Map<&2, Bool>, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat, +look: String, +rm: String) -> List<&2, D.Node>:
  match searching:
    case True{}:
      View.projects(lc, rs, True{}, open, ix, st, sel, now, look, rm)
    case False{}:
      WI.WIcon.button(Bool.pick(String, U.Flag.get(open, "old-projects"), "shelf older open", "shelf older"), "side-older", "", MSG.T.sidebar.quiet.note(lc),
          Bool.pick(String, U.Flag.get(open, "old-projects"), "chevron-down", "chevron-right"), 14, MSG.T.sidebar.older(lc, Nat.show(n)))
        <> View.older.open(lc, U.Flag.get(open, "old-projects"), rs, open, ix, st, sel, now, look, rm)

def View.older(+lc: I.Loc, rs: List<&2, U.SRow>, +searching: Bool, +open: Map<&2, Bool>, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat, +look: String, +rm: String) -> List<&2, D.Node>:
  match rs:
    case Nil{}:
      Nil{}
    case +r <> +t:
      View.older.k(lc, searching, List.length(&2, U.SRow, r <> t), r <> t, open, ix, st, sel, now, look, rm)

# the projects view: live, quiet (faded), older (folded): core's Side.groups
def View.groups(+lc: I.Loc, g: U.SGroups, +q: String, +open: Map<&2, Bool>, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat, +look: String, +rm: String) -> List<&2, D.Node>:
  match g:
    case U.SGroups{l, qs, o}:
      List.concat(&2, D.Node, [View.projects(lc, l, False{}, open, ix, st, sel, now, look, rm), View.projects(lc, qs, True{}, open, ix, st, sel, now, look, rm),
        View.older(lc, o, Bool.not(String.is_empty(q)), open, ix, st, sel, now, look, rm)])

# the active view: every live thread, what waits on you first, then the
# threads settled lately, faded (core's Active.rows); a row names its
# project where the other view shows its age
def View.arow(+lc: I.Loc, r: U.ARow, +ix: U.SIx, +st: M.State, +sel: String) -> D.Node:
  match r:
    case U.ARow{_, _, +t, +pid, +pt, +faded}:
      D.El{"div", M.Thread.id(t), [D.At{"class", Bool.pick(String, String.eq(M.Thread.id(t), sel), "thread on", Bool.pick(String, faded, "thread faded", "thread"))},
          D.At{"style", "--tint:" ++ BG.Badge.css_of(st, pid)}, D.At{"data-value", M.Thread.id(t)}, D.On{"click", "select"}, D.On{"contextmenu", "row-menu"}],
        [D.Dom.span("dot " ++ M.Phase.name(U.Side.phase.ix(ix, st, t)), ""), D.Dom.span("title", U.Label.title(st, M.Thread.id(t), TTL.Title.shown(lc, U.Active.title(t)))), View.note(View.sub.badge(lc, SB.Subs.count.ix(ix, st, M.Thread.id(t))), pt)]}

def View.arows(+lc: I.Loc, rs: List<&2, U.ARow>, +ix: U.SIx, +st: M.State, +sel: String) -> List<&2, D.Node>:
  match rs:
    case Nil{}:
      Nil{}
    case r <> t:
      View.arow(lc, r, ix, st, sel) <> View.arows(lc, t, ix, st, sel)

def View.afaded.open(+lc: I.Loc, on: Bool, rs: List<&2, U.ARow>, +ix: U.SIx, +st: M.State, +sel: String) -> List<&2, D.Node>:
  match on:
    case True{}:
      View.arows(lc, rs, ix, st, sel)
    case False{}:
      Nil{}

# the settled rows folded under one "Settled N" row, shown while it is
# open or a search is on (core's Active.open)
def View.afaded.k(+lc: I.Loc, searching: Bool, +open: Bool, +n: Nat, rs: List<&2, U.ARow>, +ix: U.SIx, +st: M.State, +sel: String) -> List<&2, D.Node>:
  match searching:
    case True{}:
      View.arows(lc, rs, ix, st, sel)
    case False{}:
      WI.WIcon.button(Bool.pick(String, open, "shelf older open", "shelf older"), "side-settled", "", MSG.T.sidebar.settled.note(lc),
          Bool.pick(String, open, "chevron-down", "chevron-right"), 14, MSG.T.sidebar.settled.count(lc, Nat.show(n)))
        <> View.afaded.open(lc, open, rs, ix, st, sel)

def View.afaded(+lc: I.Loc, rs: List<&2, U.ARow>, +q: String, +open: Map<&2, Bool>, +ix: U.SIx, +st: M.State, +sel: String) -> List<&2, D.Node>:
  match rs:
    case Nil{}:
      Nil{}
    case +r <> +t:
      View.afaded.k(lc, Bool.not(String.is_empty(q)), U.Flag.get(open, "active-settled"), List.length(&2, U.ARow, r <> t), r <> t, ix, st, sel)

def View.active.k(+lc: I.Loc, p: U.AParts, +q: String, +open: Map<&2, Bool>, +ix: U.SIx, +st: M.State, +sel: String) -> List<&2, D.Node>:
  match p:
    case U.AParts{l, f}:
      [D.El{"section", "", [D.At{"class", "project active"}], List.append(&2, D.Node, View.arows(lc, l, ix, st, sel), View.afaded(lc, f, q, open, ix, st, sel))}]

def View.active(+lc: I.Loc, rs: List<&2, U.ARow>, +q: String, +open: Map<&2, Bool>, +ix: U.SIx, +st: M.State, +sel: String) -> List<&2, D.Node>:
  match rs:
    case Nil{}:
      [D.Dom.span("shelf-label", MSG.T.sidebar.nothing(lc))]
    case +r <> +t:
      View.active.k(lc, U.Active.parts(r <> t), q, open, ix, st, sel)

# the list the "sidebar.view" setting picks (only that one is built)
def View.list.k(+lc: I.Loc, active: Bool, +q: String, +open: Map<&2, Bool>, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat, +look: String, +rm: String) -> List<&2, D.Node>:
  match active:
    case True{}:
      View.active(lc, U.Active.rows.ix(st, ix, now, q), q, open, ix, st, sel)
    case False{}:
      View.groups(lc, U.Side.groups(U.Side.shown.ix(st, ix, now, q), now), q, open, ix, st, sel, now, look, rm)

# (what every row asks of the whole state is gathered once: core's Side.ix)
def View.list(+lc: I.Loc, active: Bool, +q: String, +open: Map<&2, Bool>, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat, +look: String, +rm: String) -> List<&2, D.Node>:
  View.list.k(lc, active, q, open, ix, st, sel, now, look, rm)

# the search, always at the top of the list
def View.find.loc(+lc: I.Loc, +active: Bool, +q: String) -> List<&2, D.Node>:
  [D.El{"div", "", [D.At{"class", "find"}], [WB.WB.input("proj-find", "pq", q, Bool.pick(String, active, MSG.T.palette.search(lc), MSG.T.sidebar.search.projects(lc)), "proj-go")]}]

# a row's or project's menu (core's Menu), where the pointer was (host.js
# keeps it in --mx, --my); the shade under it shuts it

def View.find(+active: Bool, +q: String) -> List<&2, D.Node>:
  View.find.loc(I.En{}, active, q)

def View.mitems(xs: List<&2, U.MItem>) -> List<&2, D.Node>:
  match xs:
    case Nil{}:
      Nil{}
    case U.MItem{+l, a, v, +t} <> r:
      D.Dom.button(Bool.pick(String, String.eq(t, "danger"), "mi danger", "mi"), a, v, l, l) <> View.mitems(r)

def View.rmenu(xs: List<&2, U.MItem>) -> List<&2, D.Node>:
  match xs:
    case Nil{}:
      Nil{}
    case +i <> +r:
      [D.El{"div", "", [D.At{"class", "shade"}, D.At{"data-value", ""}, D.On{"click", "menu-close"}, D.On{"contextmenu", "menu-close"}], Nil{}},
       D.El{"div", "", [D.At{"class", "rowmenu"}], View.mitems(i <> r)}]

def View.add.loc(+lc: I.Loc, adding: Bool) -> List<&2, D.Node>:
  match adding:
    case True{}:
      [D.El{"div", "", [D.At{"class", "add"}], [
        D.El{"input", "project-path", [D.At{"id", "project-path"}, D.At{"placeholder", "/path/to/project"}, D.At{"data-enter", "add-project"}, D.At{"data-value", "#project-path"}], Nil{}},
        D.Dom.button("primary", "add-project", "#project-path", MSG.T.sidebar.add.project(lc), MSG.T.common.add(lc))]}]
    case False{}:
      Nil{}

# the owner's machines (only with more than one), at the foot of the
# sidebar beside Settings: Backplane is one app on each, and picking
# another opens that machine's page ("flag machines" opens the list)

def View.add(adding: Bool) -> List<&2, D.Node>:
  View.add.loc(I.En{}, adding)

def View.mrows(rs: List<&2, U.MRow>) -> List<&2, D.Node>:
  match rs:
    case Nil{}:
      Nil{}
    case U.MRow{+n, url, +on} <> t:
      D.Dom.button(Bool.pick(String, on, "machine on", "machine"), "machine", url, n, n) <> View.mrows(t)

def View.mlist(open: Bool, rs: List<&2, U.MRow>) -> List<&2, D.Node>:
  match open:
    case True{}:
      [D.El{"div", "", [D.At{"class", "machines"}], View.mrows(rs)}]
    case False{}:
      Nil{}

def View.machines.loc(+lc: I.Loc, many: Bool, +open: Bool, +rs: List<&2, U.MRow>) -> List<&2, D.Node>:
  match many:
    case True{}:
      List.append(&2, D.Node, View.mlist(open, rs), [WI.WIcon.button("machine-pick", "flag", "machines", MSG.T.sidebar.machine(lc), "chevron-up", 14, U.Machines.current(rs))])
    case False{}:
      Nil{}

def View.machines(many: Bool, +open: Bool, +rs: List<&2, U.MRow>) -> List<&2, D.Node>:
  View.machines.loc(I.En{}, many, open, rs)

def View.sidebar.k(+lc: I.Loc, +ms: List<&2, U.MRow>, +ix: U.SIx, +st: M.State, +sel: String, +open: Map<&2, Bool>, on: Bool, +now: Nat, adding: Bool, v: String, +look: String, +rm: String, +q: String, mi: List<&2, U.MItem>, bots: List<&2, D.Node>, uber: List<&2, D.Node>) -> D.Node:
  D.El{"aside", "sidebar", [D.At{"class", "sidebar"}],
        D.Dom.cat([D.El{"div", "", [D.At{"class", "brand"}], [
            D.Dom.span(Bool.pick(String, on, "conn on", "conn"), ""),
            D.Dom.span("name", "Backplane"), D.Dom.span("ver", v),
            WI.WIcon.button("icon", "proj-open", "", MSG.T.sidebar.add.project(lc), "plus", 16, "")]}],
          D.Dom.cat(uber, D.Dom.cat(View.add.loc(lc, adding), D.Dom.cat(View.find.loc(lc, String.eq(U.Side.view(st), "active"), q),
            D.Dom.cat(View.list(lc, String.eq(U.Side.view(st), "active"), q, open, ix, st, sel, now, look, rm), D.Dom.cat(bots, D.Dom.cat(View.rmenu(mi),
            [D.El{"div", "", [D.At{"class", "foot"}], WI.WIcon.button("settings-link", "flag", "settings", MSG.T.palette.settings(lc), "settings", 16, MSG.T.palette.settings(lc))
              <> View.machines.loc(lc, Nat.is_gt(List.length(&2, U.MRow, ms), 1n), U.Flag.get(open, "machines"), ms)}])))))))}

def View.sidebar.w(+lc: I.Loc, bots: List<&2, D.Node>, uber: List<&2, D.Node>, ms: List<&2, U.MRow>, mi: List<&2, U.MItem>, +ix: U.SIx, u: U.Ui) -> D.Node:
  match u:
    case U.Ui{+st, +sel, +d, _, +open, +on, +now, +adding, _, +v, _, _, _, _, _, _, _}:
      View.sidebar.k(lc, ms, ix, st, sel, open, on, now, adding, v, U.Str.get(d, "@look"), U.Str.get(d, "@rm"), U.Find.q(open, d), mi, bots, uber)

# the sidebar's index (Side.ix: every thread and ask of the state) is made
# once a frame, for the list and the uberbot's counts both
def View.sidebar.x(+lc: I.Loc, bots: List<&2, D.Node>, +u: U.Ui, +ix: U.SIx) -> D.Node:
  View.sidebar.w(lc, bots, WB.WB.uber(lc, u, ix), U.Machines.rows.loc(lc, u), U.Menu.items.loc(lc, u, True{}), ix, u)

def View.sidebar.u(+lc: I.Loc, bots: List<&2, D.Node>, +u: U.Ui) -> D.Node:
  View.sidebar.x(lc, bots, u, U.Side.ix(U.Ui.state(u)))

def View.sidebar(+lc: I.Loc, +u: U.Ui) -> D.Node:
  View.sidebar.u(lc, WB.WB.side(lc, u), u)

# Thread
# ------

def View.mine.k(same: Bool, e: M.Entry, rest: List<&2, M.Entry>) -> List<&2, M.Entry>:
  match same:
    case True{}:
      e <> rest
    case False{}:
      rest

def View.entry_thread(e: M.Entry) -> String:
  match e:
    case M.Msg{_, t, _, _, _}:
      t
    case M.Act{_, t, _, _, _, _}:
      t

def View.link(+cls: String, +label: String, +to: String) -> D.Node:
  D.El{"button", "", [D.At{"class", "link " ++ cls}, D.At{"data-value", to}, D.On{"click", "select"}], [D.Txt{label}]}

def View.from.k(none: Bool, +from: String, +st: M.State) -> List<&2, D.Node>:
  match none:
    case True{}:
      Nil{}
    case False{}:
      [View.link("in", "← from " ++ U.Tasks.title(st, from), from)]

def View.entry_id(e: M.Entry) -> String:
  match e:
    case M.Msg{i, _, _, _, _}:
      i
    case M.Act{i, _, _, _, _, _}:
      i

# a thread's entries, oldest first (the state keeps them newest first)
def View.mine(es: List<&2, M.Entry>, +th: String, acc: List<&2, M.Entry>) -> List<&2, M.Entry>:
  match es:
    case Nil{}:
      acc
    case +e <> t:
      View.mine(t, th, View.mine.k(Bool.or(String.eq(View.entry_thread(e), th), String.eq(M.Link.from(View.entry_id(e)), th)), e, acc))

def View.phase.k(idle: Bool, +label: String, +name: String, +key: String) -> List<&2, D.Node>:
  match idle:
    case True{}:
      Nil{}
    case False{}:
      [D.El{"div", key, [D.At{"class", "phase " ++ name}], [D.El{"span", "", [D.At{"class", "dot " ++ name}], Nil{}}, D.Txt{label}]}]

# the reply streaming in (made by the host from its text, View.make)
def View.live.n.loc(+lc: I.Loc, +key: String, +text: String) -> D.Node:
  D.El{"div", key, [D.At{"class", "msg assistant live"}], [D.El{"div", "", [D.At{"class", "body"}], D.Md.render.loc(lc, text)}]}

# the reply streaming in, else the thread's phase (keyed "live:<thread>":
# a delta redraws this row alone, app.bend's live)

def View.live.n(+key: String, +text: String) -> D.Node:
  View.live.n.loc(I.En{}, key, text)

def View.live.k.loc(+lc: I.Loc, empty: Bool, +ph: M.Phase, text: String, +key: String) -> List<&2, D.Node>:
  match empty:
    case False{}:
      [View.memo.loc(lc, key, [D.At{"fn", "live"}], text)]
    case True{}:
      View.phase.k(String.is_empty(View.phase.label(lc, ph)), View.phase.label(lc, ph), M.Phase.name(ph), key)

def View.live.k(empty: Bool, +ph: M.Phase, text: String, +key: String) -> List<&2, D.Node>:
  View.live.k.loc(I.En{}, empty, ph, text, key)

def View.live.of.loc(+lc: I.Loc, +st: M.State, +live: Map<&2, String>, +id: String) -> List<&2, D.Node>:
  View.live.k.loc(lc, String.is_empty(U.Str.get(live, id)), U.Side.phase(st, U.Ui.thread(st, id)), U.Str.get(live, id), "live:" ++ id)

def View.live.of(+st: M.State, +live: Map<&2, String>, +id: String) -> List<&2, D.Node>:
  View.live.of.loc(I.En{}, st, live, id)

def View.one(ns: List<&2, D.Node>) -> D.Node:
  match ns:
    case n <> _:
      n
    case Nil{}:
      D.Txt{""}

# the selected thread's live row alone (app.bend's live: host.js redraws
# it in place for a delta)
def View.live.u.loc(+lc: I.Loc, u: U.Ui) -> D.Node:
  match u:
    case U.Ui{+st, +sel, _, +live, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      View.one(View.live.of.loc(lc, st, live, sel))

def View.live.u(u: U.Ui) -> D.Node:
  View.live.u.loc(I.En{}, u)

def View.running(t: M.TurnState) -> Bool:
  match t:
    case M.Running{}:
      True{}
    case _:
      False{}

# a header tool (core/tools.bend): its icon and label, its sentence as the
# tooltip; a group starts with a line before it (class "gap")
def View.tool.cls(on: Bool, danger: Bool, gap: Bool) -> String:
  Bool.pick(String, on, "on", "") ++ Bool.pick(String, danger, " danger", "") ++ Bool.pick(String, gap, " gap", "")

def View.tool(t: TO.Tool) -> D.Node:
  match t:
    case TO.Tool{+i, +l, tip, a, v, on, dg, gap}:
      WI.WIcon.button("tool " ++ View.tool.cls(on, dg, gap), a, v, tip, i, 16, l)

def View.tools(ts: List<&2, TO.Tool>) -> List<&2, D.Node>:
  match ts:
    case Nil{}:
      Nil{}
    case t <> r:
      View.tool(t) <> View.tools(r)

# picking a snooze: its choices, then the button that closes them
def View.snoozes.loc(+lc: I.Loc, picking: Bool) -> List<&2, D.Node>:
  match picking:
    case True{}:
      [D.Dom.button("", "snooze", "1h", MSG.T.thread.snooze.hour.tip(lc), MSG.T.common.hour(lc)), D.Dom.button("", "snooze", "tomorrow", MSG.T.thread.snooze.tomorrow.tip(lc), MSG.T.common.tomorrow(lc)),
       D.Dom.button("", "snooze", "1w", MSG.T.thread.snooze.week.tip(lc), MSG.T.common.week(lc)), D.Dom.button("on", "flag", "snooze", MSG.T.common.cancel(lc), MSG.T.common.cancel(lc))]
    case False{}:
      Nil{}

# the title, or (being renamed: client.bend's Ren) a field for it; a click
# on the title renames

def View.snoozes(picking: Bool) -> List<&2, D.Node>:
  View.snoozes.loc(I.En{}, picking)

def View.title.k.loc(+lc: I.Loc, on: Bool, +id: String, +title: String, +text: String) -> List<&2, D.Node>:
  match on:
    case True{}:
      [WB.WB.input("thread-title", "title", text, MSG.T.thread.title.placeholder(lc), "rename-save"), D.Dom.button("primary", "rename-save", "", MSG.T.thread.title.keep(lc), MSG.T.common.save(lc)),
       D.Dom.button("", "rename-no", "", MSG.T.thread.title.cancel(lc), MSG.T.common.cancel(lc))]
    case False{}:
      [D.El{"span", "", [D.At{"class", "title rename"}, D.At{"title", MSG.T.thread.title.rename(lc)}, D.At{"data-value", id}, D.On{"click", "row-rename"}], [D.Txt{title}]}]

# a thread's Boat chip after its branch ("Boat · large · running"); a tap
# opens Settings, where Usage says more (core/usage.bend's Chip)

def View.title.k(on: Bool, +id: String, +title: String, +text: String) -> List<&2, D.Node>:
  View.title.k.loc(I.En{}, on, id, title, text)

def View.boat.chip.loc(+lc: I.Loc, +chip: String) -> List<&2, D.Node>:
  D.Dom.when(Bool.not(String.is_empty(chip)), D.Dom.button("boat-chip", "usage-open", "", MSG.T.thread.boat.settings(lc), chip))

def View.boat.chip(+chip: String) -> List<&2, D.Node>:
  View.boat.chip.loc(I.En{}, chip)

def View.header(+lc: I.Loc, t: M.Thread, +st: M.State, +chip: String, mini: List<&2, TO.Tool>, +panel: Bool, snoozed: Bool, picking: Bool, term: Bool, +diff: Bool, del: Bool, +ren: String, +rtext: String) -> D.Node:
  match t:
    case M.Thread{+id, +p, title, _, _, turn, _, archived, override, +sa, +ua, pinned, _, +branch, _, _, model, _, _, _, _}:
      D.El{"header", "", [D.At{"class", "thread-head"}, D.At{"style", "--tint:" ++ BG.Badge.css_of(st, p)}], List.concat(&2, D.Node, [[
        D.Dom.button("back", "select", "", MSG.T.tour.title.threads(lc), "‹"),
        D.El{"span", "", [D.At{"class", "badge"}], [WI.WIcon.svg(BG.Badge.icon(st, p), 16)]},
        D.El{"div", "", [D.At{"class", "title-box"}], View.title.k.loc(lc, String.eq(ren, id), id, U.Label.title(st, id, TTL.Title.shown(lc, title)), rtext)},
        D.Dom.span("meta", Bool.pick(String, String.is_empty(branch), "", branch))], View.boat.chip.loc(lc, chip), [
        D.El{"div", "", [D.At{"class", "tools"}], List.append(&2, D.Node,
          View.tools(List.append(&2, TO.Tool, mini, TO.Tools.head.loc(lc, TO.Head{View.running(turn), TO.Tools.settled(override, Nat.is_gt(sa, ua)), pinned, snoozed, archived, del,
            Bool.not(String.is_empty(M.Fork.source(st, id))), term, panel}, "flag", "snooze", "term-toggle", "@term-size"))),
          View.snoozes.loc(lc, picking))}]])}

# the model menu's rows: each model's name, then what its CLI calls it,
# in two columns (the names' as wide as the longest, --namew)
def View.models.loc(+lc: I.Loc, ms: List<&2, String>, +cur: String, +info: String, +prov: String) -> List<&2, D.Node>:
  match ms:
    case Nil{}:
      Nil{}
    case +m <> t:
      D.El{"button", "", [D.At{"class", Bool.pick(String, String.eq(m, cur), "row on", "row")}, D.At{"title", MSG.T.model.choose(lc, Bool.pick(String, String.is_empty(m), MSG.T.model.default.article(lc), m))}, D.At{"data-value", m}, D.On{"click", "model"}],
        [D.Dom.span("name", Bool.pick(String, String.is_empty(m), MSG.T.common.default(lc), m)), D.Dom.span("label", U.Models.label_of(info, prov, m))]} <> View.models.loc(lc, t, cur, info, prov)

def View.models(ms: List<&2, String>, +cur: String, +info: String, +prov: String) -> List<&2, D.Node>:
  View.models.loc(I.En{}, ms, cur, info, prov)

def View.namew(ms: List<&2, String>) -> Nat:
  match ms:
    case Nil{}:
      7n
    case +m <> t:
      Nat.max(String.length(m), View.namew(t))

# reasoning effort chips, the provider's default first
def View.efforts.loc(+lc: I.Loc, es: List<&2, String>, +cur: String, +prov: String) -> List<&2, D.Node>:
  match es:
    case Nil{}:
      Nil{}
    case +e <> t:
      D.Dom.button(Bool.pick(String, U.Chip.on(e, cur, prov), "on", ""), "effort", e,
        Bool.pick(String, String.starts_with(e, "@"), MSG.T.model.effort.next(lc, VLabel.chip(lc, e)), MSG.T.model.effort.tip(lc, VLabel.chip(lc, e))), VLabel.chip(lc, e)) <> View.efforts.loc(lc, t, cur, prov)

def View.efforts(es: List<&2, String>, +cur: String, +prov: String) -> List<&2, D.Node>:
  View.efforts.loc(I.En{}, es, cur, prov)

def View.effort_row.loc(+lc: I.Loc, +es: List<&2, String>, +cur: String, +prov: String) -> List<&2, D.Node>:
  Bool.pick(List<&2, D.Node>, List.is_empty(&2, String, es), Nil{}, [D.El{"div", "", [D.At{"class", "efforts"}], View.efforts.loc(lc, es, cur, prov)}])

# the model menu over the composer, and a shade behind it: a click
# anywhere off the menu lands on the shade and closes it

def View.effort_row(+es: List<&2, String>, +cur: String, +prov: String) -> List<&2, D.Node>:
  View.effort_row.loc(I.En{}, es, cur, prov)

def View.menu.loc(+lc: I.Loc, open: Bool, +ms: List<&2, String>, +cur: String, +info: String, +prov: String, +es: List<&2, String>, +effort: String) -> List<&2, D.Node>:
  match open:
    case True{}:
      [D.El{"div", "", [D.At{"class", "shade"}, D.At{"data-value", "models"}, D.On{"click", "flag"}], Nil{}},
       D.El{"div", "", [D.At{"class", "model-menu"}, D.At{"style", "--namew:" ++ Nat.show(View.namew(ms)) ++ "ch"}],
         D.Dom.cat(View.effort_row.loc(lc, es, effort, prov), [D.El{"div", "", [D.At{"class", "rows"}], View.models.loc(lc, ms, cur, info, prov)}])}]
    case False{}:
      Nil{}

# the skills a `$` being typed may complete to, or the bots, threads,
# projects and rooms a `@`, `>`, `%` or `#` may (the arrow keys move the
# highlight, Tab or Enter takes it)

def View.menu(open: Bool, +ms: List<&2, String>, +cur: String, +info: String, +prov: String, +es: List<&2, String>, +effort: String) -> List<&2, D.Node>:
  View.menu.loc(I.En{}, open, ms, cur, info, prov, es, effort)

def View.skills.go(rs: List<&2, U.SkillRow>) -> List<&2, D.Node>:
  match rs:
    case Nil{}:
      Nil{}
    case U.SkillRow{+n, +d, on} <> t:
      D.El{"button", "", [D.At{"class", Bool.pick(String, on, "skill on", "skill")}, D.At{"data-value", n}, D.At{"title", d}, D.On{"click", "skill"}], [D.Dom.span("name", n), D.Dom.span("desc", d)]} <> View.skills.go(t)

def View.skills(+rs: List<&2, U.SkillRow>) -> List<&2, D.Node>:
  Bool.pick(List<&2, D.Node>, List.is_empty(&2, U.SkillRow, rs), Nil{}, [D.El{"div", "", [D.At{"class", "skills"}], View.skills.go(rs)}])

# Subagents
# ---------


# the panel over the timeline (core/subs.bend has the rows), held at the
# top while the timeline scrolls; a task's row opens its thread, a
# subagent's opens or shuts its steps

def View.sub.step(s: SB.SubStep) -> D.Node:
  match s:
    case SB.SubStep{+kind, +text}:
      D.El{"div", "", [D.At{"class", "sub-step"}], [D.Dom.span("kind", SB.Subs.step.label(kind)), D.Dom.span("text", text)]}

def View.sub.steps(ss: List<&2, SB.SubStep>) -> List<&2, D.Node>:
  match ss:
    case Nil{}:
      Nil{}
    case s <> t:
      View.sub.step(s) <> View.sub.steps(t)

def View.sub.act(agent: Bool, +open: Bool, +key: String) -> List<&2, D.Attr>:
  match agent:
    case True{}:
      [D.At{"data-value", SB.Subs.ask(open, key)}, D.On{"click", "fold"}]
    case False{}:
      [D.At{"data-value", key}, D.On{"click", "select"}]

# a row's Stop (SB.Subs.cancel): beside the row, not inside its click
def View.sub.stop.loc(+lc: I.Loc, +act: String, +key: String) -> List<&2, D.Node>:
  D.Dom.when(Bool.not(String.is_empty(act)), D.Dom.button("sub-stop", act, key, Bool.pick(String, String.starts_with(act, "lock-"), MSG.T.subagents.lock.cancel(lc, View.sub.cancel.mode(act)), MSG.T.subagents.cancel.tip(lc)), MSG.T.subagents.cancel.word(lc, View.sub.cancel.mode(act))))

def View.sub.stop(+act: String, +key: String) -> List<&2, D.Node>:
  View.sub.stop.loc(I.En{}, act, key)

def View.sub.k(+lc: I.Loc, r: SB.SubRow, +act: String) -> D.Node:
  match r:
    case SB.SubRow{+key, +agent, +title, +who, +word, +doing, +live, +open, _, ss}:
      D.El{"div", key, [D.At{"class", "sub " ++ word ++ Bool.pick(String, live, " live", "")}],
        D.Dom.cat([D.El{"div", "", List.append(&2, D.Attr, [D.At{"class", "sub-row"}], View.sub.act(agent, open, key)),
          [D.Dom.span("state-dot", ""), D.Dom.span("who", Bool.pick(String, agent, Bool.pick(String, open, "▾", "▸"), who)), D.Dom.span("title", TTL.Title.shown(lc, title)), D.Dom.span("state", U.StateWord.label(lc, word))]}],
          D.Dom.cat(View.sub.stop.loc(lc, act, key), D.Dom.cat(D.Dom.when(Bool.not(String.is_empty(doing)), D.Dom.span("doing", doing)), View.sub.steps(ss))))}

def View.sub(+lc: I.Loc, +r: SB.SubRow) -> D.Node:
  View.sub.k(lc, r, SB.Subs.cancel(r))

def View.subs.go(+lc: I.Loc, rs: List<&2, SB.SubRow>) -> List<&2, D.Node>:
  match rs:
    case Nil{}:
      Nil{}
    case r <> rest:
      View.sub(lc, r) <> View.subs.go(lc, rest)

# the subagent viewer: what this thread delegated and the agent's own. Its
# head opens and shuts it; done, it folds to the head (subs.bend's panel)
def View.subs.k(+lc: I.Loc, +busy: String, +open: Bool, +th: String, +rs: List<&2, SB.SubRow>) -> D.Node:
  D.El{"div", "subs", [D.At{"class", "subs" ++ Bool.pick(String, String.is_empty(busy), "", " busy") ++ Bool.pick(String, open, "", " shut")}],
    WI.WIcon.after("subs-head", SB.Subs.panel.action(open), SB.Subs.panel.ask(open, th, rs), Bool.pick(String, open, MSG.T.subagents.hide(lc), MSG.T.subagents.show(lc)),
      MSG.T.subagents.heading(lc, View.sub.panel(lc, rs)), Bool.pick(String, open, "chevron-down", "chevron-right"), 14)
      <> View.subs.go(lc, SB.Subs.panel.rows(open, rs))}

def View.subs(+lc: I.Loc, +info: Map<&2, String>, +th: String, +rs: List<&2, SB.SubRow>) -> List<&2, D.Node>:
  Bool.pick(List<&2, D.Node>, List.is_empty(&2, SB.SubRow, rs), Nil{},
    [View.subs.k(lc, View.sub.busy(lc, SB.Subs.live(rs)), SB.Subs.panel.open(info, th, rs), th, rs)])

def View.parent(+st: M.State, +thread: String, +p: String) -> List<&2, D.Node>:
  Bool.pick(List<&2, D.Node>, String.is_empty(p), Nil{},
    [D.El{"button", "", [D.At{"class", "parent"}, D.At{"data-value", p}, D.On{"click", "select"}], [D.Txt{U.Tasks.up.label(st, thread, p)}]}])

# Asks: what the agent waits on the user for

def View.ask.opts(ls: List<&2, String>, +id: String) -> List<&2, D.Node>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      D.Dom.button("", "answer", id ++ "|" ++ l, l, l) <> View.ask.opts(t, id)

def View.ask.btns.loc(+lc: I.Loc, +k: String, +id: String, +o: String, +draft: String) -> List<&2, D.Node>:
  Bool.pick(List<&2, D.Node>, String.eq(k, "send"), [D.Dom.button("primary", "answer", id ++ "|accept", MSG.T.ask.send.tip(lc), MSG.T.composer.send(lc)), D.Dom.button("", "answer", id ++ "|decline", MSG.T.ask.send.none(lc), MSG.T.ask.notnow(lc))],
  Bool.pick(List<&2, D.Node>, String.eq(k, "plan"), [D.Dom.button("primary", "answer", id ++ "|implement", MSG.T.ask.implement.tip(lc), MSG.T.approval.implement(lc)), D.Dom.button("", "answer", id ++ "|dismiss", MSG.T.approval.dismiss(lc), MSG.T.approval.dismiss(lc))],
    Bool.pick(List<&2, D.Node>, String.eq(k, "input"),
      List.append(&2, D.Node, View.ask.opts(String.split(o, '\n'), id),
        Bool.pick(List<&2, D.Node>, String.is_empty(String.trim(draft)), [D.Dom.button("", "answer", id ++ "|", MSG.T.approval.dismiss(lc), MSG.T.approval.dismiss(lc))], [D.Dom.button("primary", "answer", id ++ "|" ++ String.trim(draft), MSG.T.ask.draft.tip(lc), MSG.T.approval.draft(lc))])),
      [D.Dom.button("primary", "answer", id ++ "|accept", MSG.T.ask.allow.once(lc), MSG.T.approval.allow(lc)), D.Dom.button("", "answer", id ++ "|session", MSG.T.ask.allow.session.tip(lc), MSG.T.approval.session(lc)), D.Dom.button("", "answer", id ++ "|decline", MSG.T.approval.deny(lc), MSG.T.approval.deny(lc))])))

def View.ask.btns(+k: String, +id: String, +o: String, +draft: String) -> List<&2, D.Node>:
  View.ask.btns.loc(I.En{}, k, id, o, draft)

def View.ask.std(+lc: I.Loc, a: M.Ask, +draft: String) -> D.Node:
  match a:
    case M.Ask{+id, _, +k, +tool, +d, +o, _, _}:
      D.El{"div", id, [D.At{"class", "ask " ++ k}], [
        D.Dom.span("ask-head", U.Ask.head.loc(lc, k, tool, d)),
        Bool.pick(D.Node, String.eq(k, "plan"), D.El{"div", "", [D.At{"class", "body"}], D.Md.render.loc(lc, d)}, D.El{"div", "", [D.At{"class", "ask-detail"}], [D.Txt{d}]}),
        D.El{"div", "", [D.At{"class", "ask-btns"}], View.ask.btns.loc(lc, U.Ask.bkind(k, tool), id, o, draft)}]}

# a browser task paused for help: its question, the screenshot, and the
# person's choices (client.bend's Teach.ask)
def View.ask.web.btns(bs: List<&2, U.TBtn>) -> List<&2, D.Node>:
  match bs:
    case Nil{}:
      Nil{}
    case U.TBtn{+label, +action, +value, +primary} <> t:
      D.Dom.button(Bool.pick(String, primary, "primary", ""), action, value, label, label) <> View.ask.web.btns(t)

def View.ask.web.loc(+lc: I.Loc, t: U.TAsk) -> D.Node:
  match t:
    case U.TAsk{+id, _, +slug, +n, _, +q, _, +shot, bs}:
      D.El{"div", id, [D.At{"class", "ask browser"}], D.Dom.cat([
        D.Dom.span("ask-head", MSG.T.teach.paused(lc, slug, Nat.show(n))),
        D.El{"div", "", [D.At{"class", "ask-detail"}], [D.Txt{q}]}],
        D.Dom.cat(D.Dom.when(Bool.not(String.is_empty(shot)), D.El{"a", "", [D.At{"class", "tk-shot"}, D.At{"href", shot}, D.At{"target", "_blank"}, D.At{"data-lightbox", shot}],
            [D.El{"img", "", [D.At{"src", shot}, D.At{"alt", MSG.T.ask.browser.stopped(lc)}], Nil{}}]}),
          [D.El{"div", "", [D.At{"class", "ask-btns"}], View.ask.web.btns(bs)}]))}

def View.ask.web(t: U.TAsk) -> D.Node:
  View.ask.web.loc(I.En{}, t)

def View.ask.k(+lc: I.Loc, web: Bool, a: M.Ask, +draft: String, nf: Map<&2, String>) -> D.Node:
  match web:
    case True{}:
      View.ask.web.loc(lc, U.Teach.ask.from.loc(lc, a, nf))
    case False{}:
      View.ask.std(lc, a, draft)

def View.ask(+lc: I.Loc, a: M.Ask, +draft: String, nf: Map<&2, String>) -> D.Node:
  match a:
    case M.Ask{_, _, +k, _, _, _, _, _}:
      View.ask.k(lc, String.eq(k, "browser"), a, draft, nf)

def View.asks(+lc: I.Loc, as: List<&2, M.Ask>, +draft: String, +nf: Map<&2, String>) -> List<&2, D.Node>:
  match as:
    case Nil{}:
      Nil{}
    case a <> t:
      View.ask(lc, a, draft, nf) <> View.asks(lc, t, draft, nf)

def View.qbtns(bs: List<&2, U.QBtn>) -> List<&2, D.Node>:
  match bs:
    case Nil{}:
      Nil{}
    case U.QBtn{+l, a, v} <> t:
      D.Dom.button("", a, v, l, l) <> View.qbtns(t)

def View.qrow(+lc: I.Loc, q: M.QItem, +first: Bool, +run: Bool) -> D.Node:
  match q:
    case M.QItem{+msg, text}:
      D.El{"div", "", [D.At{"class", "queued"}], [D.Dom.span("label", U.Qu.tag(lc, run, msg)), D.Dom.span("text", U.Qu.label(lc, msg, text)),
        D.El{"span", "", [D.At{"class", "q-acts"}], View.qbtns(U.Qu.row_buttons(lc, run, first, msg))}]}

def View.qrows(+lc: I.Loc, qs: List<&2, M.QItem>, +first: Bool, +run: Bool) -> List<&2, D.Node>:
  match qs:
    case Nil{}:
      Nil{}
    case q <> t:
      View.qrow(lc, q, first, run) <> View.qrows(lc, t, False{}, run)

def View.tdlines(rs: List<&2, TD.TRow>) -> List<&2, D.Node>:
  match rs:
    case Nil{}:
      Nil{}
    case +r <> rest:
      D.Dom.span("todo " ++ TD.TRow.status(r), TD.TRow.text(r)) <> View.tdlines(rest)

# the agent's todo list above the queue (U.Todos.shown)
# the todo card (client.bend's Todos): its head opens or shuts it, and a
# long list ends in a line that shows every step or the fitted few again
def View.todo.more(+text: String, +ask: String) -> List<&2, D.Node>:
  Bool.pick(List<&2, D.Node>, String.is_empty(text), Nil{}, [D.Dom.button("more", "fold", ask, text, text)])

def View.todos.k(+lc: I.Loc, +open: Bool, +all: Bool, +th: String, +ts: List<&2, TD.Todo>, +info: Map<&2, String>) -> List<&2, D.Node>:
  WI.WIcon.after("head", "fold", U.Todos.toggle(open, th), Bool.pick(String, open, MSG.T.todos.hide(lc), MSG.T.todos.show(lc)), TD.Todo.head(ts),
    Bool.pick(String, open, "chevron-down", "chevron-right"), 14)
    <> List.append(&2, D.Node, View.tdlines(U.Todos.rows(info, th, U.Todos.cap.all(), ts)),
      Bool.pick(List<&2, D.Node>, open, View.todo.more(U.Todos.more(lc, all, ts), U.Todos.more.ask(all, th)), Nil{}))

def View.todos(+lc: I.Loc, +info: Map<&2, String>, +th: String, +ts: List<&2, TD.Todo>) -> D.Node:
  D.El{"div", "", [D.At{"class", Bool.pick(String, U.Todos.all(info, th), "todos all", "todos")}], Bool.pick(List<&2, D.Node>, List.is_empty(&2, TD.Todo, ts), Nil{},
    View.todos.k(lc, U.Todos.open(info, th), U.Todos.all(info, th), th, ts, info))}

# context handed over to go with the next message (M.Queue.context), each
# with its buttons (U.Qu.row_buttons)
def View.queued(+lc: I.Loc, qs: List<&2, M.QItem>, +run: Bool) -> D.Node:
  D.El{"div", "", [D.At{"class", "queue"}], View.qrows(lc, qs, True{}, run)}

# Settings
# --------

def View.chips.loc(+lc: I.Loc, xs: List<&2, String>, +cur: String, +key: String) -> List<&2, D.Node>:
  match xs:
    case Nil{}:
      Nil{}
    case +v <> t:
      D.Dom.button(Bool.pick(String, String.eq(v, cur), "on", ""), "setting", key ++ "=" ++ v, key, VLabel.option(lc, key, v)) <> View.chips.loc(lc, t, cur, key)

def View.chips(xs: List<&2, String>, +cur: String, +key: String) -> List<&2, D.Node>:
  View.chips.loc(I.En{}, xs, cur, key)

def View.setrow(+label: String, +note: String, ctl: List<&2, D.Node>) -> D.Node:
  D.El{"div", "", [D.At{"class", "set-row"}], [D.El{"div", "", [D.At{"class", "set-text"}], [D.Dom.span("set-label", label), D.Dom.span("set-note", note)]},
    D.El{"div", "", [D.At{"class", "set-ctl"}], ctl}]}

def View.days.loc(+lc: I.Loc, +d: String) -> List<&2, D.Node>:
  +n = {Maybe.default(&2, Nat, Nat.read(d), 3n) : Nat}
  [D.Dom.button("", "setting", "settle.days=" ++ Nat.show(Bool.pick(Nat, Nat.is_gt(n, 1n), Nat.sub(n, 1n), 1n)), MSG.T.settings.days.fewer(lc), "−"),
   D.Dom.span("days", MSG.T.settings.settle.days(lc, Nat.show(n), Bool.pick(String, Nat.is_eq(n, 1n), "one", "other"))),
   D.Dom.button("", "setting", "settle.days=" ++ Nat.show(Bool.pick(Nat, Nat.is_lt(n, 90n), 1n+n, 90n)), MSG.T.settings.days.more(lc), "+")]

def View.days(+d: String) -> List<&2, D.Node>:
  View.days.loc(I.En{}, d)

def View.kn.acts(as: List<&2, KH.KAct>) -> List<&2, D.Node>:
  match as:
    case Nil{}: Nil{}
    case KH.KAct{+l, "link", v} <> t:
      D.El{"a", "", [D.At{"href", v}, D.At{"target", "_blank"}, D.At{"rel", "noopener"}], [D.Txt{l}]} <> View.kn.acts(t)
    case KH.KAct{+l, a, v} <> t: D.Dom.button("", a, v, l, l) <> View.kn.acts(t)

def View.kn.close.loc(+lc: I.Loc, on: Bool, +key: String) -> List<&2, D.Node>:
  match on:
    case True{}: [D.Dom.button("kn-close", "setting", "kicad.notice.hidden=" ++ key, MSG.T.settings.kicad.dismiss(lc), "×")]
    case False{}: Nil{}

def View.kn.close(on: Bool, +key: String) -> List<&2, D.Node>:
  View.kn.close.loc(I.En{}, on, key)

def View.kn.loc(+lc: I.Loc, n: Maybe<&2, KH.KNote>, dismiss: Bool) -> List<&2, D.Node>:
  match n:
    case None{}: Nil{}
    case Some{KH.KNote{key, title, body, acts}}:
      [D.El{"div", "kicad-notice", [D.At{"class", "kicad-notice"}], List.concat(&2, D.Node, [
        [D.El{"strong", "", Nil{}, [D.Txt{title}]}, D.El{"p", "", Nil{}, [D.Txt{body}]}, D.El{"div", "", [D.At{"class", "kn-actions"}], View.kn.acts(acts)}], View.kn.close.loc(lc, dismiss, key)])}]

def View.kn(n: Maybe<&2, KH.KNote>, dismiss: Bool) -> List<&2, D.Node>:
  View.kn.loc(I.En{}, n, dismiss)

def View.kx.lines(ls: List<&2, String>) -> List<&2, D.Node>:
  match ls:
    case Nil{}: Nil{}
    case l <> t: D.El{"li", "", Nil{}, [D.Txt{l}]} <> View.kx.lines(t)

def View.kx.note(n: Maybe<&2, KH.KNote>, +nf: Map<&2, String>, +th: String) -> List<&2, D.Node>:
  match n:
    case None{}: Nil{}
    case Some{KH.KNote{_, title, _, acts}}:
      +r = U.KX.result(nf, th)
      [D.El{"section", "kicad-export", [D.At{"class", "kicad-notice kicad-export"}], [
        D.El{"strong", "", Nil{}, [D.Txt{title}]}, D.El{"p", "", Nil{}, [D.Txt{J.Json.get_str(r, "text", "")}]},
        D.El{"ul", "", Nil{}, View.kx.lines(List.take(&2, String, List.drop(&2, String, KEX.KX.lines(KEX.KX.report(J.Json.get_obj(r, "report"))), U.KX.offset(nf, th)), 8n))},
        D.El{"div", "", [D.At{"class", "kn-actions"}], View.kn.acts(acts)}]}]

def View.kn.path.loc(+lc: I.Loc, +u: U.Ui) -> D.Node:
  D.El{"input", "kicad-path", [D.At{"id", "kicad-path"}, D.At{"value", M.Setting.get(U.Ui.state(u), "kicad.path", "")},
    D.At{"placeholder", MSG.T.settings.kicad.automatic(lc, U.Ui.info(u, "kicad.cli"))}, D.At{"data-enter", "setting"}, D.At{"data-pre", "kicad.path="}], Nil{}}

def View.kn.path(+u: U.Ui) -> D.Node:
  View.kn.path.loc(I.En{}, u)

def View.fork.loc(+lc: I.Loc, +fork: String) -> List<&2, D.Node>:
  Bool.pick(List<&2, D.Node>, String.is_empty(fork), [D.Dom.button("primary", "kicad-install", "", MSG.T.settings.kicad.install(lc), MSG.T.settings.kicad.install(lc))],
    [D.Dom.span("ok", "Backplane KiCad " ++ fork)])

# Voice: the OpenAI key (only ever shown masked), the microphone, the
# dictionary (× takes a term out), as in the native window

def View.fork(+fork: String) -> List<&2, D.Node>:
  View.fork.loc(I.En{}, fork)

def View.devs(ds: List<&2, U.VDev>, +cur: String) -> List<&2, D.Node>:
  match ds:
    case Nil{}:
      Nil{}
    case U.VDev{+name, +label} <> t:
      D.Dom.button(Bool.pick(String, String.eq(name, cur), "on", ""), "setting", "voice.device=" ++ name, name, label) <> View.devs(t, cur)

def View.terms.loc(+lc: I.Loc, ts: List<&2, String>) -> List<&2, D.Node>:
  match ts:
    case Nil{}:
      Nil{}
    case +t <> r:
      D.Dom.button("chip", "voice-unterm", t, MSG.T.settings.voice.term.remove(lc), t ++ " ×") <> View.terms.loc(lc, r)

def View.terms(ts: List<&2, String>) -> List<&2, D.Node>:
  View.terms.loc(I.En{}, ts)

def View.voice.set.loc(+lc: I.Loc, +u: U.Ui, +st: M.State) -> List<&2, D.Node>:
  [View.setrow(MSG.T.settings.voice.key(lc), Bool.pick(String, String.is_empty(U.Ui.scratch(u, "@voice.key")), MSG.T.settings.voice.key.note(lc),
      MSG.T.settings.voice.key.saved(lc, U.Ui.scratch(u, "@voice.key"))),
     [WB.WB.secret("voice-key", "vkey", U.Ui.scratch(u, "@f.vkey"), "sk-…"), D.Dom.button("primary", "voice-key", "", MSG.T.settings.key.keep(lc), MSG.T.common.save(lc)),
      D.Dom.button("", "voice-unkey", "", MSG.T.settings.key.forget(lc), MSG.T.common.remove(lc))]),
   View.setrow(MSG.T.settings.voice.microphone(lc), MSG.T.voice.device.note(lc, Bool.pick(String, String.is_empty(M.Setting.get(st, "voice.device", "")), MSG.T.voice.device.default(lc, U.Voice.label(u, U.Voice.default(u))), U.Voice.label(u, M.Setting.get(st, "voice.device", "")))),
     D.Dom.button(Bool.pick(String, String.is_empty(M.Setting.get(st, "voice.device", "")), "on", ""), "setting", "voice.device=", MSG.T.settings.voice.default.tip(lc), MSG.T.voice.default(lc)) <>
       View.devs(U.Voice.devices(u), M.Setting.get(st, "voice.device", ""))),
   View.setrow(MSG.T.settings.voice.dictionary(lc), MSG.T.web.settings.voice.dictionary.note(lc),
     List.append(&2, D.Node, View.terms.loc(lc, U.Voice.dict(u)), [WB.WB.input("voice-term", "vterm", U.Ui.scratch(u, "@f.vterm"), MSG.T.settings.voice.term(lc), "voice-term"),
       D.Dom.button("", "voice-term", "", MSG.T.settings.voice.term.add(lc), MSG.T.common.add(lc))]))]

# API keys (client.bend's ApiKeys): add one, view the names, delete one

def View.voice.set(+u: U.Ui, +st: M.State) -> List<&2, D.Node>:
  View.voice.set.loc(I.En{}, u, st)

def View.keys.rows.loc(+lc: I.Loc, ns: List<&2, String>) -> List<&2, D.Node>:
  match ns:
    case Nil{}:
      Nil{}
    case +n <> t:
      View.setrow(n, "", [D.Dom.button("danger", "key-del", n, MSG.T.settings.keys.forget(lc), MSG.T.common.delete(lc))]) <> View.keys.rows.loc(lc, t)

def View.keys.rows(ns: List<&2, String>) -> List<&2, D.Node>:
  View.keys.rows.loc(I.En{}, ns)

def View.keys.list.loc(+lc: I.Loc, open: Bool, +u: U.Ui) -> List<&2, D.Node>:
  match open:
    case True{}:
      Bool.pick(List<&2, D.Node>, List.is_empty(&2, String, U.ApiKeys.names(u)), [View.setrow(MSG.T.settings.keys.empty(lc), "", Nil{})], View.keys.rows.loc(lc, U.ApiKeys.names(u)))
    case False{}:
      Nil{}

def View.keys.list(open: Bool, +u: U.Ui) -> List<&2, D.Node>:
  View.keys.list.loc(I.En{}, open, u)

def View.keys.form(+lc: I.Loc, open: Bool, +u: U.Ui) -> List<&2, D.Node>:
  match open:
    case True{}:
      [View.setrow(MSG.T.settings.keys.new(lc), U.ApiKeys.form.note(lc),
        [WB.WB.input("kname", "kname", U.Ui.scratch(u, "@f.kname"), MSG.T.bots.name(lc), "key-add"), WB.WB.secret("kval", "kval", U.Ui.scratch(u, "@f.kval"), MSG.T.common.key(lc)),
         D.Dom.button("primary", "key-add", "", MSG.T.settings.key.keep(lc), MSG.T.common.save(lc))])]
    case False{}:
      Nil{}

# Antigravity (client.bend's Agv.rows): a row's fields and buttons; its
# "open-url" button is a link that opens in a new tab
def View.agy.btn(+b: U.AgBtn) -> D.Node:
  match b:
    case U.AgBtn{+l, +a, +v, on}:
      Bool.pick(D.Node, String.eq(a, "open-url"),
        D.El{"a", "", [D.At{"class", "btn primary"}, D.At{"href", v}, D.At{"target", "_blank"}, D.At{"rel", "noopener noreferrer"}], [D.Txt{l}]},
        D.Dom.button(Bool.pick(String, on, "on", ""), a, v, l, l))

def View.agy.btns(bs: List<&2, U.AgBtn>) -> List<&2, D.Node>:
  match bs:
    case Nil{}:
      Nil{}
    case +b <> t:
      View.agy.btn(b) <> View.agy.btns(t)

def View.agy.field(+f: U.AgField) -> D.Node:
  match f:
    case U.AgField{+n, +h, sec, +v, +e}:
      Bool.pick(D.Node, sec, WB.WB.secret("agy-" ++ n, n, v, h), WB.WB.input("agy-" ++ n, n, v, h, e))

def View.agy.fields(fs: List<&2, U.AgField>) -> List<&2, D.Node>:
  match fs:
    case Nil{}:
      Nil{}
    case +f <> t:
      View.agy.field(f) <> View.agy.fields(t)

def View.agy.rows(rs: List<&2, U.AgRow>) -> List<&2, D.Node>:
  match rs:
    case Nil{}:
      Nil{}
    case U.AgRow{+l, +n, fs, bs} <> t:
      View.setrow(l, n, List.append(&2, D.Node, View.agy.fields(fs), View.agy.btns(bs))) <> View.agy.rows(t)

# the time zone: where the person is (their phone reports it, or they set
# it); a field for a zone name, Set, and Automatic (on while one is set)
# Automatic: off (disabled) while the zone already follows the phone
def View.tz.auto.loc(+lc: I.Loc, on: Bool) -> D.Node:
  match on:
    case True{}:
      D.Dom.button("", "tz-auto", "", MSG.T.settings.tz.follow(lc), MSG.T.settings.lang.auto(lc))
    case False{}:
      D.El{"button", "", [D.At{"class", ""}, D.At{"title", MSG.T.settings.tz.following(lc)}, D.At{"disabled", "disabled"}], [D.Txt{MSG.T.settings.lang.auto(lc)}]}

def View.tz.auto(on: Bool) -> D.Node:
  View.tz.auto.loc(I.En{}, on)

def View.usage.btn.loc(+lc: I.Loc, none: Bool, +act: String) -> List<&2, D.Node>:
  match none:
    case True{}:
      Nil{}
    case False{}:
      [D.Dom.button("", act, "", MSG.T.settings.boat.refresh(lc), MSG.T.common.refresh(lc))]

# Settings > Usage (src/core/usage.bend), the first row titled "Usage: ...";
# a row with an action has its Refresh button

def View.usage.btn(none: Bool, +act: String) -> List<&2, D.Node>:
  View.usage.btn.loc(I.En{}, none, act)

def View.usage.row.loc(+lc: I.Loc, +label: String, +r: UZ.URow) -> D.Node:
  View.setrow(label, UZ.URow.note(r), View.usage.btn.loc(lc, String.is_empty(UZ.URow.act(r)), UZ.URow.act(r)))

def View.usage.row(+label: String, +r: UZ.URow) -> D.Node:
  View.usage.row.loc(I.En{}, label, r)

def View.usage.rows.go.loc(+lc: I.Loc, rs: List<&2, UZ.URow>) -> List<&2, D.Node>:
  match rs:
    case Nil{}:
      Nil{}
    case +r <> t:
      View.usage.row.loc(lc, UZ.URow.label(r), r) <> View.usage.rows.go.loc(lc, t)

def View.usage.rows.go(rs: List<&2, UZ.URow>) -> List<&2, D.Node>:
  View.usage.rows.go.loc(I.En{}, rs)

def View.usage.rows.loc(+lc: I.Loc, rs: List<&2, UZ.URow>) -> List<&2, D.Node>:
  match rs:
    case Nil{}:
      Nil{}
    case +r <> t:
      View.usage.row.loc(lc, MSG.T.settings.usage.label(lc, UZ.URow.label(r)), r) <> View.usage.rows.go.loc(lc, t)

def View.usage.rows(rs: List<&2, UZ.URow>) -> List<&2, D.Node>:
  View.usage.rows.loc(I.En{}, rs)

def View.tz.set(+lc: I.Loc, +u: U.Ui) -> List<&2, D.Node>:
  [View.setrow(MSG.T.settings.zone(lc), U.Tzs.line.loc(lc, u),
    [WB.WB.input("tzname", "tzname", U.Ui.scratch(u, "@f.tzname"), "Europe/Helsinki", "tz-set"), D.Dom.button("primary", "tz-set", "", MSG.T.settings.tz.choose(lc), MSG.T.common.set(lc)),
     View.tz.auto.loc(lc, U.Tzs.auto(u))]),
   View.setrow("", U.Tzs.err(u), Nil{})]

def View.keys.set(+lc: I.Loc, +u: U.Ui) -> List<&2, D.Node>:
  List.concat(&2, D.Node, [
    [View.setrow(MSG.T.settings.keys(lc), U.ApiKeys.note.loc(lc, u),
      [D.Dom.button(Bool.pick(String, U.Ui.flag(u, "keyform"), "on", ""), "flag", "keyform", MSG.T.settings.keys.add.tip(lc), MSG.T.settings.keys.add(lc)),
       D.Dom.button(Bool.pick(String, U.Ui.flag(u, "keys"), "on", ""), "flag", "keys", MSG.T.settings.keys.names(lc), MSG.T.settings.keys.view(lc))])],
    View.keys.form(lc, U.Ui.flag(u, "keyform"), u), View.keys.list.loc(lc, U.Ui.flag(u, "keys"), u)])

# the pairing code under its row while "pairqr" is on: the code on white
# with its quiet zone, a click puts it away
def View.pair.qr.svg(+lc: I.Loc, +rows: List<&2, String>) -> D.Node:
  +side = {U32.show((U32.from_nat(List.length(&2, String, rows)) + 8 : U32)) : String}
  D.El{"div", "", [D.At{"class", "pair-qr"}, D.At{"data-value", "pairqr"}, D.On{"click", "flag"}], [
    D.El{"svg", "", [D.At{"viewBox", "0 0 " ++ side ++ " " ++ side}, D.At{"shape-rendering", "crispEdges"}],
      [D.El{"path", "", [D.At{"d", QR.Qr.path(rows)}], Nil{}}]},
    D.Dom.span("set-note", U.Pair.qr.note(lc))]}

def View.pair.qr(+lc: I.Loc, on: Bool, +nf: Map<&2, String>) -> List<&2, D.Node>:
  match on:
    case True{}:
      [View.pair.qr.svg(lc, U.Pair.qr(nf))]
    case False{}:
      Nil{}

# the same settings as the native window
# the version row's button (U.Update.btn)
def View.update.btn.loc(+lc: I.Loc, i: Nat) -> List<&2, D.Node>:
  match i:
    case 1n:
      [D.Dom.button("primary", "update", "", MSG.T.settings.update.install.tip(lc), MSG.T.settings.update.install(lc))]
    case 2n:
      [D.Dom.button("", "update-check", "", MSG.T.settings.update.check.tip(lc), MSG.T.settings.update.check(lc))]
    case 3n:
      [D.Dom.button("primary", "update-now", "", MSG.T.settings.update.now.tip(lc), MSG.T.settings.update.now(lc))]
    case _:
      Nil{}

# Your machines (U.Mate.rows): each of the owner's other machines, how its
# sync goes, and On/Off ("sync.<name>")

def View.update.btn(i: Nat) -> List<&2, D.Node>:
  View.update.btn.loc(I.En{}, i)

def View.mates.go.loc(+lc: I.Loc, rs: List<&2, U.MateRow>) -> List<&2, D.Node>:
  match rs:
    case Nil{}:
      Nil{}
    case U.MateRow{+nm, +note, +on, +key} <> t:
      View.setrow(nm, note, View.chips.loc(lc, ["on", "off"], on, key)) <> View.mates.go.loc(lc, t)

def View.mates.go(rs: List<&2, U.MateRow>) -> List<&2, D.Node>:
  View.mates.go.loc(I.En{}, rs)

def View.mates(+lc: I.Loc, +u: U.Ui) -> List<&2, D.Node>:
  View.setrow(U.Mate.title(lc), U.Mate.lead.loc(lc, u), Nil{}) <> View.mates.go.loc(lc, U.Mate.rows.loc(lc, u))

# Notification policy rows, shared with the window and phones.
def View.notify.rows.loc(+lc: I.Loc, rs: List<&2, U.NotifyRow>) -> List<&2, D.Node>:
  match rs:
    case Nil{}:
      Nil{}
    case U.NotifyRow{nm, note, on, key} <> t:
      View.setrow(nm, note, View.chips.loc(lc, ["on", "off"], on, key)) <> View.notify.rows.loc(lc, t)

def View.notify.rows(rs: List<&2, U.NotifyRow>) -> List<&2, D.Node>:
  View.notify.rows.loc(I.En{}, rs)

def View.notify(+lc: I.Loc, st: M.State) -> List<&2, D.Node>:
  View.setrow(U.Notify.title(lc), U.Notify.lead(lc), Nil{}) <> View.notify.rows.loc(lc, U.Notify.rows.loc(lc, st))

def View.lang.chips(+lc: I.Loc, vs: List<&2, String>, +u: U.Ui) -> List<&2, D.Node>:
  match vs:
    case Nil{}: Nil{}
    case +v <> t:
      D.Dom.button(Bool.pick(String, String.eq(v, U.Lang.choice(u)), "on", ""), "lang", v, "", U.Lang.label(lc, v)) <> View.lang.chips(lc, t, u)

def View.lang.loc(+lc: I.Loc, +u: U.Ui) -> List<&2, D.Node>:
  [View.setrow(MSG.T.settings.lang(lc), "", View.lang.chips(lc, U.Lang.offered(u), u)),
    View.setrow(MSG.T.settings.agent.lang(lc), "", [D.Dom.button(Bool.pick(String, U.Lang.agent.on(u), "on", ""),
      "setting", "agent.lang=" ++ U.Lang.agent.value(Bool.not(U.Lang.agent.on(u)), u), "",
      Bool.pick(String, U.Lang.agent.on(u), MSG.T.settings.lang.on(lc), MSG.T.settings.lang.off(lc)))])]

def View.lang(+u: U.Ui) -> List<&2, D.Node>:
  View.lang.loc(U.Lang.loc(u), u)

def View.settings.loc(+lc: I.Loc, +u: U.Ui, +st: M.State, +nf: Map<&2, String>, +v: String, +note: String) -> D.Node:
  +prov = M.Setting.get(st, "agent.provider", "claude")
  +ipc = M.Setting.get(st, "kicad.ipc", "on")
  D.El{"main", "settings", [D.At{"class", "settings-view"}], [
    D.El{"header", "", [D.At{"class", "thread-head"}], [D.Dom.span("title", MSG.T.palette.settings(lc)), D.Dom.span("meta", ""), D.Dom.button("", "flag", "settings", MSG.T.settings.close(lc), MSG.T.common.close(lc))]},
    D.El{"div", "", [D.At{"class", "settings"}], List.concat(&2, D.Node, [[
      View.setrow(MSG.T.settings.settle(lc), MSG.T.settings.settle.note(lc), View.days.loc(lc, M.Setting.get(st, "settle.days", "3"))),
      View.setrow(MSG.T.settings.restart(lc), MSG.T.settings.restart.note(lc), View.chips.loc(lc, ["on", "off"], Bool.pick(String, M.Carry.on(st), "on", "off"), "restart.continue"))],
      View.notify(lc, st), View.agy.rows(U.Fallbackv.rows(u)),
      [View.setrow(MSG.T.settings.provider(lc), MSG.T.settings.agents.note(lc, Bool.pick(String, String.is_empty(U.Models.cli(U.Str.get(nf, "models"), prov)), MSG.T.settings.provider.missing(lc, prov), U.Models.cli(U.Str.get(nf, "models"), prov))), View.chips.loc(lc, ["claude", "codex", "grok", "muse", "antigravity"], prov, "agent.provider")),
      View.setrow(MSG.T.settings.model(lc), Bool.pick(String, String.is_empty(M.Setting.get(st, "agent.model", "")), MSG.T.common.default(lc), M.Setting.get(st, "agent.model", "")) ++ ": " ++ U.Models.label_of(U.Str.get(nf, "models"), prov, M.Setting.get(st, "agent.model", "")), View.chips.loc(lc, U.Models.menu(U.Str.get(nf, "models"), prov), M.Setting.get(st, "agent.model", ""), "agent.model")),
      View.setrow(MSG.T.settings.writing.provider(lc), MSG.T.web.settings.writing.note(lc), View.chips.loc(lc, ["claude", "codex", "grok", "muse"], U.TextGen.provider(st), "text.provider")),
      View.setrow(MSG.T.web.settings.writing.model(lc), Bool.pick(String, String.is_empty(U.TextGen.model(st)), MSG.T.common.default(lc), U.TextGen.model(st)) ++ ": " ++ U.Models.label_of(U.Str.get(nf, "models"), U.TextGen.provider(st), U.TextGen.model(st)),
        View.chips.loc(lc, U.Models.menu(U.Str.get(nf, "models"), U.TextGen.provider(st)), U.TextGen.model(st), "text.model")),
      View.setrow("KiCad", U.KN.summary(nf), View.kn.path.loc(lc, u) <> View.kn.loc(lc, U.KN.of(nf, st), False{})),
      View.setrow(U.FreeCAD.label.loc(lc, nf), U.FreeCAD.note(lc, nf), Nil{}),
      View.setrow(U.KiCad.ipc.label(lc, nf), U.KiCad.ipc.note(lc, nf), Bool.pick(List<&2, D.Node>, U.KiCad.ipc.ready(nf), View.chips.loc(lc, ["on", "off"], ipc, "kicad.ipc"), Nil{})),
      View.setrow(U.Fab.set.order.label(), U.Fab.set.order.note(), View.chips.loc(lc, ["on", "off"], U.Fab.set.enabled(st), "order.enabled")),
      View.setrow(U.Fab.set.label(), U.Fab.set.note.k(U.Fab.enabled(st)), Bool.pick(List<&2, D.Node>, U.Fab.enabled(st), View.chips.loc(lc, ["on", "off"], U.Fab.set.on(st), "order.estimates"), Nil{})),
      View.setrow(MSG.T.settings.theme(lc), MSG.T.settings.theme.note(lc), View.chips.loc(lc, ["dark", "light"], Bool.pick(String, String.eq(M.Setting.get(st, "ui.theme", ""), "light"), "light", "dark"), "ui.theme"))], View.lang.loc(lc, u), [
      View.setrow(MSG.T.settings.sidebar(lc), MSG.T.settings.sidebar.note(lc), View.chips.loc(lc, ["projects", "active"], U.Side.view(st), "sidebar.view")),

      View.setrow(U.Tail.label(lc, nf), U.Tail.note(lc, nf), Nil{})],
      View.mates(lc, u),
      [View.setrow(MSG.T.settings.pair(lc), U.Pair.note(lc, U.Pair.link(nf)), D.Dom.cat(D.Dom.when(U.Pair.qr.ready(nf), D.Dom.button(Bool.pick(String, U.Ui.flag(u, "pairqr"), "on", ""), "flag", "pairqr", MSG.T.settings.pair.qr.tip(lc), MSG.T.settings.pair.code(lc))),
        D.Dom.when(Bool.not(String.is_empty(U.Pair.link(nf))), D.Dom.copy.loc(lc, U.Pair.link(nf)))))],
      View.pair.qr(lc, U.Ui.flag(u, "pairqr"), nf),
      [View.setrow(MSG.T.settings.channel(lc), MSG.T.settings.channel.note(lc), View.chips.loc(lc, ["stable", "nightly"], M.Setting.get(st, "update.channel", "stable"), "update.channel")),
      View.setrow("Backplane " ++ v, U.Update.line.w(lc, st, nf, note), View.update.btn.loc(lc, U.Update.btn(st, nf, note)))], View.voice.set.loc(lc, u, st), View.keys.set(lc, u), View.agy.rows(U.Agv.rows.loc(lc, u)), View.tz.set(lc, u), View.usage.rows.loc(lc, U.Usage.rows(u)), [View.setrow(MSG.T.settings.tour(lc), MSG.T.settings.tour.note(lc), [D.Dom.button("", "ob-open", "", MSG.T.settings.tour.again(lc), MSG.T.settings.tour.show(lc))])]])}]}

def View.settings(+u: U.Ui, +st: M.State, +nf: Map<&2, String>, +v: String, +note: String) -> D.Node:
  View.settings.loc(U.Lang.loc(u), u, st, nf, v, note)

def View.empty.loc(+lc: I.Loc, n: Nat, nf: Map<&2, String>, st: M.State) -> D.Node:
  D.El{"main", "empty", [D.At{"class", "empty"}], D.Dom.cat([D.Txt{Bool.pick(String, Nat.is_eq(n, 0n), MSG.T.empty.add(lc), MSG.T.empty.thread(lc))}], View.kn.loc(lc, U.KN.visible(nf, st), True{}))}

def View.empty(n: Nat, nf: Map<&2, String>, st: M.State) -> D.Node:
  View.empty.loc(I.En{}, n, nf, st)

def View.error(e: String) -> List<&2, D.Node>:
  match e:
    case SNil{}:
      Nil{}
    case SCon{h, t}:
      [D.El{"div", "error", [D.At{"class", "toast"}, D.On{"click", "dismiss"}], [D.Txt{SCon{h, t}}]}]

# Project picker
# --------------

def View.prow.cls(+a: String) -> String:
  Bool.pick(String, Bool.or(String.eq(a, "proj-cd"), String.eq(a, "proj-pick")), "prow", Bool.pick(String, String.is_empty(a), "prow off", "prow act"))

# a row: its label, and a project's path at the right
def View.prows(+u: U.Ui, ps: List<&2, U.PItem>, +i: Nat, +sel: Nat) -> List<&2, D.Node>:
  match ps:
    case Nil{}:
      Nil{}
    case U.PItem{+l, +a, +v} <> t:
      D.El{"button", "", [D.At{"class", View.prow.cls(a) ++ Bool.pick(String, Nat.is_eq(i, sel), " sel", "")}, D.At{"title", l}, D.At{"data-value", v}, D.On{"click", a}],
        [D.El{"span", "", [D.At{"class", "pl"}], [D.Txt{l}]}, D.El{"span", "", [D.At{"class", "pd"}], [D.Txt{U.Proj.detail(u, U.PItem{l, a, v})}]}]} <> View.prows(u, t, 1n+i, sel)

# above the filter: "Projects", or the folder browsed and the way back
def View.phead.loc(+lc: I.Loc, dir: Bool, +u: U.Ui) -> D.Node:
  match dir:
    case True{}:
      D.El{"div", "", [D.At{"class", "dir"}], [D.Dom.button("pback", "proj-back", "", MSG.T.picker.back(lc), MSG.T.tour.title.projects(lc)), D.Txt{U.Proj.dir(u)}]}
    case False{}:
      D.El{"div", "", [D.At{"class", "dir"}], [D.Txt{MSG.T.tour.title.projects(lc)}]}

def View.phead(dir: Bool, +u: U.Ui) -> D.Node:
  View.phead.loc(I.En{}, dir, u)

def View.picker(+lc: I.Loc, open: Bool, +u: U.Ui) -> List<&2, D.Node>:
  match open:
    case True{}:
      [D.El{"div", "", [D.At{"class", "modal-back"}, D.On{"click", "proj-close"}], Nil{}},
        D.El{"div", "picker", [D.At{"class", "picker"}], D.Dom.cat([
          View.phead.loc(lc, U.Proj.browsing(u), u),
          D.El{"input", "proj", [D.At{"id", "proj"}, D.At{"placeholder", Bool.pick(String, U.Proj.browsing(u), MSG.T.picker.folder.placeholder(lc), MSG.T.picker.project.placeholder(lc))},
            D.At{"autocomplete", "off"}, D.At{"value", U.Proj.query(u)}, D.On{"input", "proj-q"}, D.At{"data-enter", "proj-enter"}], Nil{}}],
          D.Dom.cat(Bool.pick(List<&2, D.Node>, String.is_empty(U.Ui.info(u, "proj.err")), Nil{}, [D.El{"div", "", [D.At{"class", "perr"}], [D.Txt{U.Ui.info(u, "proj.err")}]}]),
            [D.El{"div", "", [D.At{"class", "prows"}], View.prows(u, U.Proj.items.loc(lc, u), 0n, U.Proj.index(u))}]))}]
    case False{}:
      Nil{}

# Attachments: a message's attachment lines drawn as chips
def View.chip(+path: String) -> D.Node:
  D.El{"span", "", [D.At{"class", Bool.pick(String, AT.Att.image(path), "chip image", "chip")}, D.At{"title", path}], [D.Txt{AT.Att.label(path)}]}

def View.chips.go(ps: List<&2, String>) -> List<&2, D.Node>:
  match ps:
    case Nil{}:
      Nil{}
    case +p <> t:
      View.chip(p) <> View.chips.go(t)

def View.chips.of(ps: List<&2, String>) -> List<&2, D.Node>:
  match ps:
    case Nil{}:
      Nil{}
    case +p <> +t:
      [D.El{"div", "", [D.At{"class", "chips"}], View.chips.go(p <> t)}]

# a user message: what was written, then its attachments
def View.user_body(+text: String) -> List<&2, D.Node>:
  D.Dom.cat(Bool.pick(List<&2, D.Node>, String.is_empty(AT.Att.body(text)), Nil{}, [D.El{"div", "", [D.At{"class", "body plain"}], D.Dom.linked(AT.Att.body(text))}]),
    View.chips.of(AT.Att.paths(text)))

# the images a reply names; a click opens one in the lightbox (data-lightbox,
# app.bend's lb_*), a middle click in a new tab
def View.images(ps: List<&2, String>) -> List<&2, D.Node>:
  match ps:
    case Nil{}:
      Nil{}
    case +p <> t:
      D.El{"a", "", [D.At{"class", "shot"}, D.At{"href", IM.Img.url(p)}, D.At{"target", "_blank"}, D.At{"title", p}, D.At{"data-lightbox", IM.Img.url(p)}],
        [D.El{"img", "", [D.At{"src", IM.Img.url(p)}, D.At{"alt", p}, D.At{"loading", "lazy"}], Nil{}}]} <> View.images(t)

# a run of tool calls, or a turn's work, folded into one row (FO.Fold
# decides the runs, their labels and which are open); a click opens or
# shuts it
def View.fold.row.loc(+lc: I.Loc, +id: String, +label: String, +open: Bool) -> D.Node:
  WI.WIcon.button("fold", "fold", FO.Fold.ask(open, id), MSG.T.fold.tooltip(lc, Bool.pick(String, open, "yes", "no"), Bool.pick(String, String.starts_with(id, "work:"), "work", "tools")),
    Bool.pick(String, open, "chevron-down", "chevron-right"), 14, label)

def View.fold.row(+id: String, +label: String, +open: Bool) -> D.Node:
  View.fold.row.loc(I.En{}, id, label, open)

def View.fold.shown(open: Bool, ns: List<&2, D.Node>) -> List<&2, D.Node>:
  match open:
    case True{}:
      ns
    case False{}:
      Nil{}

# an entry's nodes as its role says
def View.role.loc(+lc: I.Loc, r: FO.Role, ns: List<&2, D.Node>) -> List<&2, D.Node>:
  match r:
    case FO.RShow{}:
      ns
    case FO.RHead{+id, +label, +open}:
      View.fold.row.loc(lc, id, label, open) <> View.fold.shown(open, ns)
    case FO.RIn{open}:
      View.fold.shown(open, ns)
    case FO.RWork{+id, +label, open, inner}:
      match open:
        case True{}:
          View.fold.row.loc(lc, id, label, True{}) <> View.role.loc(lc, inner, ns)
        case False{}:
          [View.fold.row.loc(lc, id, label, False{})]

# the newest page of a thread's entries (core's Page), with a mark at
# the top when earlier ones are left out: scrolled near it, the page asks
# for them ("earlier", host.js), no button

def View.role(r: FO.Role, ns: List<&2, D.Node>) -> List<&2, D.Node>:
  View.role.loc(I.En{}, r, ns)

def View.earlier(+hidden: Nat, +id: String) -> List<&2, D.Node>:
  Bool.pick(List<&2, D.Node>, Nat.is_eq(hidden, 0n), Nil{},
    [D.Dom.el("div", [D.At{"class", "earlier"}, D.At{"data-earlier", id}], Nil{})])

def View.sending.loc(+lc: I.Loc, ps: List<&2, String>) -> List<&2, D.Node>:
  match ps:
    case Nil{}:
      Nil{}
    case p <> t:
      D.El{"div", "", [D.At{"class", "msg user sending"}], D.Dom.cat(View.user_body(p), [D.Dom.span("note", MSG.T.thread.sending(lc))])} <> View.sending.loc(lc, t)

# Terminal
# --------

def View.sending(ps: List<&2, String>) -> List<&2, D.Node>:
  View.sending.loc(I.En{}, ps)

def View.nib(+n: U32) -> Char:
  Chr{Bool.pick(U32, U32.is_lt(n, 10), (n + 48 : U32), (n + 87 : U32))}

def View.hex.go(k: Nat, +x: U32, acc: String) -> String:
  match k:
    case 0n:
      acc
    case 1n+j:
      View.hex.go(j, U32.shrn(x, 4n), SCon{View.nib(U32.and(x, 15)), acc})

# 0xRRGGBB as "#rrggbb"
def View.hex(+x: U32) -> String:
  "#" ++ View.hex.go(6n, x, "")

def View.run.style(+fg: U32, +bg: U32, bold: Bool, ul: Bool) -> String:
  String.concat([Bool.pick(String, U32.is_eq(fg, VT.Vt.fg()), "", "color:" ++ View.hex(fg) ++ ";"),
    Bool.pick(String, U32.is_eq(bg, VT.Vt.bg()), "", "background:" ++ View.hex(bg) ++ ";"),
    Bool.pick(String, bold, "font-weight:700;", ""), Bool.pick(String, ul, "text-decoration:underline;", "")])

def View.runs(rs: List<&2, VT.VRun>) -> List<&2, D.Node>:
  match rs:
    case Nil{}:
      Nil{}
    case VT.VRun{t, +fg, +bg, bo, ul} <> rest:
      D.El{"span", "", [D.At{"style", View.run.style(fg, bg, bo, ul)}], [D.Txt{t}]} <> View.runs(rest)

def View.tlines(ls: List<&2, VT.VLine>) -> List<&2, D.Node>:
  match ls:
    case Nil{}:
      Nil{}
    case VT.VLine{rs} <> t:
      D.El{"div", "", [D.At{"class", "tl"}], View.runs(rs)} <> View.tlines(t)

def View.cursor(c: VT.VCur) -> List<&2, D.Node>:
  match c:
    case VT.VCur{+x, +y, True{}}:
      [D.El{"div", "cur", [D.At{"class", "cur"}, D.At{"style", "left:" ++ U32.show(x) ++ "ch;top:" ++ U32.show((y * 17 : U32)) ++ "px"}], Nil{}}]
    case VT.VCur{_, _, False{}}:
      Nil{}

# a tool call or other activity; a design step (core/hist.bend) is a row
# that shows it in the viewer. data-id is what host.js scrolls to (Jump)
# Terminals in the chat (core/emb.bend): a live one is its screen, with a
# field over it that takes keys (data-for names it); an inactive one is a
# gray row a click starts again; a pinned one sits at the top of the
# thread (View.pins) and leaves a row here
def View.emb.screen.loc(+lc: I.Loc, +v: VT.Vt, +key: String, +cols: U32, +rows: U32) -> D.Node:
  D.El{"div", "", [D.At{"class", "screen emb-screen"}, D.At{"style", "background:" ++ View.hex(VT.Vt.bg()) ++ ";color:" ++ View.hex(VT.Vt.fg()) ++ ";width:calc(" ++ U32.show(cols) ++ "ch + 24px);height:" ++ U32.show((rows * 17 + 8 : U32)) ++ "px"}],
    D.Dom.cat(View.tlines(VT.Vt.lines(v, 0n)), D.Dom.cat(View.cursor(VT.Vt.cursor(v)),
      [D.El{"textarea", "emb:" ++ key, [D.At{"class", "term-in"}, D.At{"data-keys", "emb-key"}, D.At{"data-text", "emb-paste"}, D.At{"data-for", key},
        D.At{"autocapitalize", "off"}, D.At{"autocomplete", "off"}, D.At{"spellcheck", "false"}, D.At{"aria-label", MSG.T.terminal.input(lc)}], Nil{}}]))}

def View.emb.screen(+v: VT.Vt, +key: String, +cols: U32, +rows: U32) -> D.Node:
  View.emb.screen.loc(I.En{}, v, key, cols, rows)

def View.emb.live.loc(+lc: I.Loc, +ec: U.Ec, ev: EM.EV) -> D.Node:
  match ev:
    case EM.EV{+key, +id, +title, +cols, +rows, _, +pinned}:
      D.El{"div", "emb-" ++ id, [D.At{"class", "emb"}, D.At{"data-id", id}], [
        D.El{"div", "", [D.At{"class", "emb-head"}], [WI.WIcon.svg("terminal", 14), D.Dom.span("label", title ++ Bool.pick(String, String.is_empty(VT.Vt.title(U.Ec.vt(ec, key))), "", " · " ++ VT.Vt.title(U.Ec.vt(ec, key)))),
          WI.WIcon.button(Bool.pick(String, pinned, "icon on", "icon"), "emb-pin", key, Bool.pick(String, pinned, MSG.T.menu.unpin(lc), MSG.T.terminal.pin(lc)), "pin", 14, "")]},
        View.emb.screen.loc(lc, U.Ec.vt(ec, key), key, cols, rows)]}

def View.emb.live(+ec: U.Ec, ev: EM.EV) -> D.Node:
  View.emb.live.loc(I.En{}, ec, ev)

def View.emb.k.loc(+lc: I.Loc, live: Bool, pinned: Bool, +ec: U.Ec, +th: String, ev: EM.EV) -> D.Node:
  match live pinned ev:
    case True{} True{} EM.EV{+key, +id, +title, _, _, _, _}:
      D.El{"div", "emb-" ++ id, [D.At{"class", "emb pinned-here"}, D.At{"data-id", id}], [
        WI.WIcon.button("emb-row", "emb-pin", key, MSG.T.terminal.unpin(lc), "pin", 14, MSG.T.web.terminal.pinned(lc, title))]}
    case True{} False{} _:
      View.emb.live.loc(lc, ec, ev)
    case False{} _ EM.EV{_, +id, +title, _, _, _, _}:
      D.El{"div", "emb-" ++ id, [D.At{"class", "emb off"}, D.At{"data-id", id}], [
        WI.WIcon.button("emb-row", "emb-restart", th ++ "\t" ++ id, MSG.T.terminal.restart(lc), "play", 14, MSG.T.web.terminal.inactive(lc, title))]}

def View.emb.k(live: Bool, pinned: Bool, +ec: U.Ec, +th: String, ev: EM.EV) -> D.Node:
  View.emb.k.loc(I.En{}, live, pinned, ec, th, ev)

def View.emb.ev.loc(+lc: I.Loc, +ev: EM.EV, +ec: U.Ec, +th: String) -> D.Node:
  match ev:
    case EM.EV{k, i, t, c, r, +live, +pinned}:
      View.emb.k.loc(lc, live, pinned, ec, th, EM.EV{k, i, t, c, r, live, pinned})

def View.emb.ev(+ev: EM.EV, +ec: U.Ec, +th: String) -> D.Node:
  View.emb.ev.loc(I.En{}, ev, ec, th)

def View.emb.loc(+lc: I.Loc, +ec: U.Ec, +st: M.State, +e: M.Entry, +th: String) -> D.Node:
  View.emb.ev.loc(lc, U.Ec.view(ec, st, e), ec, th)

# a reminder: a bell, Reminder, the text and when, in the person's zone
# an act's two spans: a review comment's from its JSON (core/review.bend),
# parsed only when it is one (Review.label/say pick after parsing, and every
# tool call is an act)

def View.emb(+ec: U.Ec, +st: M.State, +e: M.Entry, +th: String) -> D.Node:
  View.emb.loc(I.En{}, ec, st, e, th)

def View.act.spans(review: Bool, +tone: String, +kind: String, +summary: String) -> List<&2, D.Node>:
  match review:
    case True{}:
      [D.Dom.span("kind", RVW.Review.label(tone, kind, summary)), D.Dom.span("summary", RVW.Review.say(tone, kind, summary))]
    case False{}:
      [D.Dom.span("kind", kind), D.Dom.span("summary", summary)]

# an act (a tool call, an error...): made by the host from these alone
# (View.make)
def View.act.n(+id: String, +tone: String, +kind: String, +summary: String) -> D.Node:
  D.El{"div", id, [D.At{"class", "act " ++ tone}, D.At{"data-id", id}], View.act.spans(String.eq(tone, RVW.Review.tone()), tone, kind, summary)}

def View.act.g.loc(+lc: I.Loc, rem: Bool, +id: String, +tone: String, +kind: String, +summary: String, +at: Nat, +st: M.State) -> D.Node:
  match rem:
    case True{}:
      D.El{"div", id, [D.At{"class", "act remind entry-remind"}, D.At{"data-id", id}],
        [WI.WIcon.svg("bell", 14), D.Dom.span("kind", MSG.T.sched.kind.reminder(lc)), D.Dom.span("summary", summary), D.Dom.span("when", TZ.Tz.show.nz(SCH.Sched.person(st, 0n, True{}), at))]}
    case False{}:
      View.memo.loc(lc, id, [D.At{"fn", "act"}, D.At{"tone", tone}, D.At{"kind", kind}], summary)

def View.act.g(rem: Bool, +id: String, +tone: String, +kind: String, +summary: String, +at: Nat, +st: M.State) -> D.Node:
  View.act.g.loc(I.En{}, rem, id, tone, kind, summary, at, st)

def View.act.t.loc(+lc: I.Loc, term: Bool, e: M.Entry, +st: M.State, +ec: U.Ec) -> D.Node:
  match term e:
    case True{} M.Act{+id, +th, tone, kind, sum, +at}:
      View.emb.loc(lc, ec, st, M.Act{id, th, tone, kind, sum, at}, th)
    case _ M.Act{+id, _, +tone, +kind, +summary, +at}:
      View.act.g.loc(lc, String.eq(tone, "remind"), id, tone, kind, summary, at, st)
    case _ M.Msg{+id, _, _, text, _}:
      D.El{"div", id, [D.At{"class", "act"}, D.At{"data-id", id}], [D.Dom.span("summary", text)]}

def View.act.t(term: Bool, e: M.Entry, +st: M.State, +ec: U.Ec) -> D.Node:
  View.act.t.loc(I.En{}, term, e, st, ec)

def View.pins.go.loc(+lc: I.Loc, vs: List<&2, EM.EV>, +ec: U.Ec) -> List<&2, D.Node>:
  match vs:
    case Nil{}:
      Nil{}
    case v <> t:
      View.emb.live.loc(lc, ec, v) <> View.pins.go.loc(lc, t, ec)

# the thread's pinned terminals, at the top with the subagents

def View.pins.go(vs: List<&2, EM.EV>, +ec: U.Ec) -> List<&2, D.Node>:
  View.pins.go.loc(I.En{}, vs, ec)

def View.pins.loc(+lc: I.Loc, vs: List<&2, EM.EV>, +ec: U.Ec) -> List<&2, D.Node>:
  match vs:
    case Nil{}:
      Nil{}
    case _ <> _:
      [D.El{"section", "emb-pins", [D.At{"class", "emb-pins"}], View.pins.go.loc(lc, vs, ec)}]

def View.pins(vs: List<&2, EM.EV>, +ec: U.Ec) -> List<&2, D.Node>:
  View.pins.loc(I.En{}, vs, ec)

def View.act.loc(+lc: I.Loc, step: Bool, e: M.Entry, +st: M.State, +ec: U.Ec) -> D.Node:
  match step e:
    case True{} M.Act{+id, _, _, _, _, _}:
      D.El{"div", id, [D.At{"class", "act step"}, D.At{"data-id", id}],
        [WI.WIcon.button("step", "hist-show", id, MSG.T.review.step.show(lc), "history", 14, HS.Hist.row(e))]}
    case _ M.Act{+id, +th, +tone, +kind, +summary, +at}:
      View.act.t.loc(lc, String.eq(tone, EM.Emb.tone()), M.Act{id, th, tone, kind, summary, at}, st, ec)
    case _ M.Msg{+id, _, _, text, _}:
      D.El{"div", id, [D.At{"class", "act"}, D.At{"data-id", id}], [D.Dom.span("summary", text)]}

# A message: a person's ("u") or anyone else's, made from its id, text and
# stamp alone

def View.act(step: Bool, e: M.Entry, +st: M.State, +ec: U.Ec) -> D.Node:
  View.act.loc(I.En{}, step, e, st, ec)

def View.msg.k.loc(+lc: I.Loc, user: Bool, +id: String, +text: String, +stamp: String) -> D.Node:
  match user:
    case True{}:
      D.El{"div", id, [D.At{"class", "msg user"}, D.At{"data-id", id}], D.Dom.cat(View.user_body(text), [D.Dom.span("stamp", stamp), D.Dom.copy.loc(lc, AT.Att.body(text))])}
    case False{}:
      D.El{"div", id, [D.At{"class", "msg assistant"}, D.At{"data-id", id}], D.Dom.cat([D.El{"div", "", [D.At{"class", "body"}], D.Md.render.loc(lc, text)}], D.Dom.cat(View.images(IM.Img.paths(text)), [D.Dom.span("stamp", stamp), D.Dom.copy.loc(lc, text)]))}

def View.msg.k(user: Bool, +id: String, +text: String, +stamp: String) -> D.Node:
  View.msg.k.loc(I.En{}, user, id, text, stamp)

def View.msg.loc(+lc: I.Loc, +id: String, +who: String, +text: String, +stamp: String) -> D.Node:
  View.msg.k.loc(lc, String.eq(who, "u"), id, text, stamp)

def View.msg(+id: String, +who: String, +text: String, +stamp: String) -> D.Node:
  View.msg.loc(I.En{}, id, who, text, stamp)

def View.make.r.loc(+lc: I.Loc, row: Bool, +k: String, +as: List<&2, D.Attr>, +text: String) -> D.Node:
  match row:
    case True{}:
      View.row.k.loc(lc, k, text, View.attr(as, "ago"), View.attr(as, "badge"), View.attr(as, "dot"), Bool.pick(String, String.eq(View.attr(as, "on"), "1"), k, ""), String.eq(View.attr(as, "mov"), "1"))
    case False{}:
      View.act.n(k, View.attr(as, "tone"), View.attr(as, "kind"), text)

def View.make.r(row: Bool, +k: String, +as: List<&2, D.Attr>, +text: String) -> D.Node:
  View.make.r.loc(I.En{}, row, k, as, text)

def View.make.l.loc(+lc: I.Loc, live: Bool, +fn: String, +k: String, +as: List<&2, D.Attr>, +text: String) -> D.Node:
  match live:
    case True{}:
      View.live.n.loc(lc, k, text)
    case False{}:
      View.make.r.loc(lc, String.eq(fn, "row"), k, as, text)

def View.make.l(live: Bool, +fn: String, +k: String, +as: List<&2, D.Attr>, +text: String) -> D.Node:
  View.make.l.loc(I.En{}, live, fn, k, as, text)

def View.make.f.loc(+lc: I.Loc, msg: Bool, +fn: String, +k: String, +as: List<&2, D.Attr>, +text: String) -> D.Node:
  match msg:
    case True{}:
      View.msg.loc(lc, k, View.attr(as, "who"), text, View.attr(as, "stamp"))
    case False{}:
      View.make.l.loc(lc, String.eq(fn, "live"), fn, k, as, text)

# the node a bp-memo element stands for

def View.make.f(msg: Bool, +fn: String, +k: String, +as: List<&2, D.Attr>, +text: String) -> D.Node:
  View.make.f.loc(I.En{}, msg, fn, k, as, text)

def View.make(n: D.Node) -> D.Node:
  match n:
    case D.El{_, +k, +as, +ks}:
      View.make.f.loc(I.I18n.of_tag(View.attr(as, "lang")), String.eq(View.attr(as, "fn"), "msg"), View.attr(as, "fn"), k, as, View.txt(ks))
    case D.Txt{t}:
      D.Txt{t}

def View.expand.one(memo: Bool, n: D.Node) -> D.Node:
  match memo:
    case True{}:
      View.make(n)
    case False{}:
      n

# the nodes with every bp-memo made
def View.expand(ns: List<&2, D.Node>) -> List<&2, D.Node>:
  match ns:
    case Nil{}:
      Nil{}
    case D.El{+t, +k, +as, ks} <> r:
      View.expand.one(String.eq(t, "bp-memo"), D.El{t, k, as, View.expand(ks)}) <> View.expand(r)
    case +n <> r:
      n <> View.expand(r)

def View.entry.s.loc(+lc: I.Loc, e: M.Entry, +st: M.State, +ec: U.Ec, +stamp: String) -> D.Node:
  match e:
    case M.Msg{+id, _, M.User{}, +text, _}:
      View.memo.loc(lc, id, [D.At{"fn", "msg"}, D.At{"who", "u"}, D.At{"stamp", stamp}], text)
    case M.Msg{+id, _, _, +text, _}:
      View.memo.loc(lc, id, [D.At{"fn", "msg"}, D.At{"who", "a"}, D.At{"stamp", stamp}], text)
    case M.Act{+id, +th, +tone, +kind, +summary, +at}:
      View.act.loc(lc, String.eq(tone, HS.Step.tone()), M.Act{id, th, tone, kind, summary, at}, st, ec)

# an entry of the thread itself, with the time under a message (core/stamp.bend)

def View.entry.s(e: M.Entry, +st: M.State, +ec: U.Ec, +stamp: String) -> D.Node:
  View.entry.s.loc(I.En{}, e, st, ec, stamp)

def View.entry.own(+lc: I.Loc, +e: M.Entry, +st: M.State, +ec: U.Ec, +sp: STP.Stamps) -> D.Node:
  View.entry.s.loc(lc, e, st, ec, STP.Stamp.text.loc(lc, sp, e))

# an entry of this thread (headed by the thread that sent it) or a message
# this thread sent to another (a link)
def View.entry.k(+lc: I.Loc, mine: Bool, e: M.Entry, +st: M.State, +ec: U.Ec, +sp: STP.Stamps) -> List<&2, D.Node>:
  match mine e:
    case True{} M.Msg{+id, t, r, x, at}:
      List.append(&2, D.Node, View.from.k(String.is_empty(M.Link.from(id)), M.Link.from(id), st), [View.entry.own(lc, M.Msg{id, t, r, x, at}, st, ec, sp)])
    case True{} _:
      [View.entry.own(lc, e, st, ec, sp)]
    case False{} M.Msg{_, +t, _, x, _}:
      [View.link("out", "→ " ++ U.Tasks.title(st, t) ++ ": " ++ DF.Diff.first(x), t)]
    case False{} _:
      Nil{}

def View.entry(+lc: I.Loc, +e: M.Entry, +st: M.State, +sel: String, +ec: U.Ec, +sp: STP.Stamps) -> List<&2, D.Node>:
  View.entry.k(lc, String.eq(View.entry_thread(e), sel), e, st, ec, sp)

def View.entries.r(+lc: I.Loc, es: List<&2, M.Entry>, rs: List<&2, FO.Role>, +st: M.State, +sel: String, +ec: U.Ec, +sp: STP.Stamps) -> List<&2, D.Node>:
  match es rs:
    case Nil{} _:
      Nil{}
    case e <> t r <> rt:
      List.append(&2, D.Node, View.role.loc(lc, r, View.entry(lc, e, st, sel, ec, sp)), View.entries.r(lc, t, rt, st, sel, ec, sp))
    case e <> t Nil{}:
      List.append(&2, D.Node, View.entry(lc, e, st, sel, ec, sp), View.entries.r(lc, t, Nil{}, st, sel, ec, sp))

# a thread's entries, its runs of tool calls folded (the last stays open
# while its turn runs)
def View.entries.e(+lc: I.Loc, +es: List<&2, M.Entry>, +st: M.State, +sel: String, +info: Map<&2, String>, +running: Bool, +ec: U.Ec) -> List<&2, D.Node>:
  View.entries.r(lc, es, FO.Fold.roles.loc(lc, es, info, running), st, sel, ec, STP.Stamps.of(es, st, FO.Fold.now(info), running))

def View.entries(+lc: I.Loc, +es: List<&2, M.Entry>, +st: M.State, +sel: String, +info: Map<&2, String>, +running: Bool) -> List<&2, D.Node>:
  View.entries.e(lc, es, st, sel, info, running, U.Ec.none())

def View.paged.w(+lc: I.Loc, w: U.Win, +st: M.State, +sel: String, +info: Map<&2, String>, +running: Bool, +ec: U.Ec) -> List<&2, D.Node>:
  match w:
    case U.Win{+h, es}:
      List.append(&2, D.Node, View.earlier(h, sel), View.entries.e(lc, es, st, sel, info, running, ec))

def View.paged(+lc: I.Loc, +drafts: Map<&2, String>, +es: List<&2, M.Entry>, +st: M.State, +sel: String, +info: Map<&2, String>, +running: Bool, +ec: U.Ec) -> List<&2, D.Node>:
  View.paged.w(lc, U.Page.newest.from(es, sel, U.Page.since(st, sel), U.Hold.page(es, U.Str.get(drafts, "@away." ++ sel), U.Page.count(Maybe.default(&2, Nat, Nat.read(U.Str.get(drafts, "@more." ++ sel)), 0n)))), st, sel, info, running, ec)

# the thread's shell: its screen, and over it a transparent field that
# takes keys (and a phone's keyboard)
def View.term.loc(+lc: I.Loc, +v: VT.Vt) -> D.Node:
  D.El{"section", "term-pane", [D.At{"class", "term-pane"}], [
    D.El{"div", "", [D.At{"class", "term-head"}], [D.Dom.span("label", MSG.T.palette.terminal(lc) ++ Bool.pick(String, String.is_empty(VT.Vt.title(v)), "", " · " ++ VT.Vt.title(v))),
      WI.WIcon.button("icon", "term-toggle", "", MSG.T.terminal.close(lc), "x", 16, "")]},
    D.El{"div", "", [D.At{"class", "screen"}, D.At{"style", "background:" ++ View.hex(VT.Vt.bg()) ++ ";color:" ++ View.hex(VT.Vt.fg())}],
      D.Dom.cat(View.tlines(VT.Vt.lines(v, 0n)), D.Dom.cat(View.cursor(VT.Vt.cursor(v)),
        [D.El{"textarea", "term", [D.At{"id", "term"}, D.At{"class", "term-in"}, D.At{"data-keys", "term-key"}, D.At{"data-text", "term-paste"},
          D.At{"autocapitalize", "off"}, D.At{"autocomplete", "off"}, D.At{"spellcheck", "false"}, D.At{"aria-label", MSG.T.terminal.input(lc)}], Nil{}}]))}]}

# Diff
# ----

def View.term(+v: VT.Vt) -> D.Node:
  View.term.loc(I.En{}, v)

def View.dl.cls(+k: U32) -> String:
  Bool.pick(String, U32.is_eq(k, 1), "dl add", Bool.pick(String, U32.is_eq(k, 2), "dl del", Bool.pick(String, U32.is_eq(k, 3), "dl meta", "dl")))

def View.dl.no(+n: Nat) -> String:
  Bool.pick(String, Nat.is_eq(n, 0n), "", Nat.show(n))

def View.dlines(ls: List<&2, DF.DLine>) -> List<&2, D.Node>:
  match ls:
    case Nil{}:
      Nil{}
    case DF.DLine{+k, t, +o, +n} <> rest:
      D.El{"div", "", [D.At{"class", View.dl.cls(k)}], [D.Dom.span("no", View.dl.no(o)), D.Dom.span("no", View.dl.no(n)),
        D.Dom.span("sign", Bool.pick(String, U32.is_eq(k, 1), "+", Bool.pick(String, U32.is_eq(k, 2), "-", " "))), D.Dom.span("code", t)]} <> View.dlines(rest)

def View.dhunks(hs: List<&2, DF.DHunk>) -> List<&2, D.Node>:
  match hs:
    case Nil{}:
      Nil{}
    case DF.DHunk{+hd, ls} <> t:
      D.Dom.cat(Bool.pick(List<&2, D.Node>, String.is_empty(hd), Nil{}, [D.Dom.span("dhunk", hd)]), D.Dom.cat(View.dlines(ls), View.dhunks(t)))

def View.dfiles(fs: List<&2, DF.DFile>) -> List<&2, D.Node>:
  match fs:
    case Nil{}:
      Nil{}
    case DF.DFile{_, +nw, st, hs} <> t:
      D.El{"div", nw, [D.At{"class", "dfile"}], D.El{"div", "", [D.At{"class", "dhead"}], [D.Dom.span("name", nw), D.Dom.span("status", st)]} <> View.dhunks(hs)} <> View.dfiles(t)

# what this thread changed, with the git actions (as the native Lay.diff)
def View.diff.loc(+lc: I.Loc, +text: String) -> List<&2, D.Node>:
  +fs = DF.Diff.parse(text)
  [D.El{"div", "", [D.At{"class", "diff-head"}], [
      D.Dom.span("summary", Bool.pick(String, List.is_empty(&2, DF.DFile, fs), MSG.T.viewer.diff.empty(lc), DF.Diff.summary(fs))),
      D.El{"div", "", [D.At{"class", "diff-btns"}], [
        D.Dom.button("", "revert", "0", MSG.T.diff.revert.tip(lc), MSG.T.palette.revert(lc)),
        D.Dom.button("", "git", "commit_push_pr", MSG.T.diff.pr.tip(lc), MSG.T.viewer.diff.pr(lc)),
        D.Dom.button("primary", "git", "commit_push", MSG.T.diff.commit.tip(lc), MSG.T.palette.commit(lc))]}]},
    D.El{"div", "", [D.At{"class", "diff-body"}], View.dfiles(fs)}]

def View.diff(+text: String) -> List<&2, D.Node>:
  View.diff.loc(I.En{}, text)

def View.pending.loc(+lc: I.Loc, ps: List<&2, String>) -> List<&2, D.Node>:
  match ps:
    case Nil{}:
      Nil{}
    case +p <> t:
      D.El{"button", "", [D.At{"class", Bool.pick(String, AT.Att.image(p), "chip image", "chip")}, D.At{"title", MSG.T.composer.detach(lc, p)}, D.At{"data-value", p}, D.On{"click", "detach"}],
        [D.Txt{AT.Att.label(p) ++ " ×"}]} <> View.pending.loc(lc, t)

# what the next message attaches (and what is still uploading)

def View.pending(ps: List<&2, String>) -> List<&2, D.Node>:
  View.pending.loc(I.En{}, ps)

def View.attaching.loc(+lc: I.Loc, +ps: List<&2, String>, +up: String) -> List<&2, D.Node>:
  Bool.pick(List<&2, D.Node>, Bool.and(List.is_empty(&2, String, ps), String.is_empty(up)), Nil{},
    [D.El{"div", "", [D.At{"class", "chips"}], D.Dom.cat(View.pending.loc(lc, ps),
      Bool.pick(List<&2, D.Node>, String.is_empty(up), Nil{}, [D.Dom.span("chip up", up ++ " …")]))}])

# Voice: dictation into the composer (client.bend's Voice). The mic is red
# while it records; above the composer, what is being heard, a note when
# nothing comes in or something failed, and the words to clarify, one at
# a time.

def View.attaching(+ps: List<&2, String>, +up: String) -> List<&2, D.Node>:
  View.attaching.loc(I.En{}, ps, up)

def View.mic.loc(+lc: I.Loc, +state: String) -> D.Node:
  WI.WIcon.button(Bool.pick(String, String.is_empty(state), "mic", "mic " ++ state), "voice", "",
    Bool.pick(String, String.is_empty(state), MSG.T.voice.dictate(lc), MSG.T.voice.stop(lc)), "mic", 16, "")

def View.mic(+state: String) -> D.Node:
  View.mic.loc(I.En{}, state)

def View.vhint.loc(+lc: I.Loc, +h: String, +dev: String) -> String:
  +d = Bool.pick(String, String.is_empty(dev), MSG.T.voice.microphone(lc), dev)
  Bool.pick(String, String.eq(h, "no-audio"), MSG.T.voice.noaudio(lc, d),
    Bool.pick(String, String.eq(h, "silent"), MSG.T.voice.silent(lc, d), h))

def View.vhint(+h: String, +dev: String) -> String:
  View.vhint.loc(I.En{}, h, dev)

def View.vstrip.loc(+lc: I.Loc, on: Bool, +u: U.Ui) -> List<&2, D.Node>:
  match on:
    case True{}:
      +state = U.Ui.scratch(u, "@voice")
      [D.El{"div", "", [D.At{"class", "voice-strip"}], [D.Dom.span("rec", ""),
        D.Dom.span("voice-head", Bool.pick(String, String.eq(state, "starting"), MSG.T.voice.starting(lc), Bool.pick(String, String.eq(state, "stopping"), MSG.T.voice.finishing(lc),
          MSG.T.web.voice.listening(lc) ++ Bool.pick(String, String.is_empty(U.Ui.scratch(u, "@voice.dev")), "", " · " ++ U.Ui.scratch(u, "@voice.dev"))))),
        D.El{"span", "", [D.At{"class", "meter"}], [D.El{"span", "", [D.At{"style", "width:" ++ Nat.show(Maybe.default(&2, Nat, Nat.read(U.Ui.scratch(u, "@voice.lv")), 0n)) ++ "%"}], Nil{}}]},
        D.Dom.span("voice-part", U.Ui.scratch(u, "@voice.part")),
        D.Dom.button("stop", "voice", "", MSG.T.voice.stop(lc), MSG.T.composer.stop(lc))]}]
    case False{}:
      Nil{}

def View.vstrip(on: Bool, +u: U.Ui) -> List<&2, D.Node>:
  View.vstrip.loc(I.En{}, on, u)

def View.vnote.loc(+lc: I.Loc, on: Bool, +u: U.Ui) -> List<&2, D.Node>:
  match on:
    case True{}:
      [D.El{"div", "", [D.At{"class", Bool.pick(String, String.is_empty(U.Ui.scratch(u, "@voice.err")), "voice-note warn", "voice-note bad")}], [
        D.Dom.span("", Bool.pick(String, String.is_empty(U.Ui.scratch(u, "@voice.err")), View.vhint.loc(lc, U.Ui.scratch(u, "@voice.hint"), U.Ui.scratch(u, "@voice.dev")),
          MSG.T.voice.error(lc, U.Ui.scratch(u, "@voice.err")))),
        WI.WIcon.button("", "voice-err", "", MSG.T.common.close(lc), "x", 14, "")]}]
    case False{}:
      Nil{}

def View.vnote(on: Bool, +u: U.Ui) -> List<&2, D.Node>:
  View.vnote.loc(I.En{}, on, u)

def View.vask.k.loc(+lc: I.Loc, x: VC.Unsure, +n: Nat) -> List<&2, D.Node>:
  match x:
    case VC.Unsure{+w, +g}:
      [D.El{"div", "", [D.At{"class", "voice-ask"}], List.append(&2, D.Node,
        [D.Dom.span("", MSG.T.web.voice.heard(lc, w, Bool.pick(String, Nat.is_gt(n, 1n), "many", "one"), Nat.show(n)))],
        List.append(&2, D.Node, Bool.pick(List<&2, D.Node>, String.is_empty(g), Nil{}, [D.Dom.button("primary", "voice-use", "0", MSG.T.voice.guess.tip(lc), MSG.T.voice.choose(lc, g))]),
          [D.Dom.button("", "voice-fix", "0", MSG.T.voice.fix.tip(lc), MSG.T.voice.fix(lc)), D.Dom.button("", "voice-add", "0", MSG.T.voice.dictionary.tip(lc), MSG.T.voice.dictionary.add(lc)),
           D.Dom.button("", "voice-skip", "0", MSG.T.voice.leave(lc), MSG.T.tour.skip(lc))]))}]

def View.vask.k(x: VC.Unsure, +n: Nat) -> List<&2, D.Node>:
  View.vask.k.loc(I.En{}, x, n)

def View.vask.go.loc(+lc: I.Loc, fixing: Bool, +u: U.Ui, +us: List<&2, VC.Unsure>) -> List<&2, D.Node>:
  match fixing:
    case True{}:
      [D.El{"div", "", [D.At{"class", "voice-ask"}], [WB.WB.input("voice-fix", "vfix", U.Ui.scratch(u, "@f.vfix"), MSG.T.voice.spelling(lc), "voice-fixed"),
        D.Dom.button("primary", "voice-fixed", "", MSG.T.voice.replace(lc), MSG.T.common.save(lc)), D.Dom.button("", "form-close", "@voice.fix", MSG.T.common.cancel(lc), MSG.T.common.cancel(lc))]}]
    case False{}:
      View.vask.k.loc(lc, Maybe.default(&2, VC.Unsure, List.head(&2, VC.Unsure, us), VC.Unsure{"", ""}), List.length(&2, VC.Unsure, us))

def View.vask.go(fixing: Bool, +u: U.Ui, +us: List<&2, VC.Unsure>) -> List<&2, D.Node>:
  View.vask.go.loc(I.En{}, fixing, u, us)

def View.vask.loc(+lc: I.Loc, on: Bool, +u: U.Ui) -> List<&2, D.Node>:
  match on:
    case True{}:
      View.vask.go.loc(lc, Bool.not(String.is_empty(U.Ui.scratch(u, "@voice.fix"))), u, U.Voice.unsure(u))
    case False{}:
      Nil{}

# what sits over the composer for dictation, and the mic's state

def View.vask(on: Bool, +u: U.Ui) -> List<&2, D.Node>:
  View.vask.loc(I.En{}, on, u)

def View.voice.loc(+lc: I.Loc, +u: U.Ui, +sel: String) -> List<&2, D.Node>:
  +here = Bool.or(String.eq(U.Ui.scratch(u, "@voice.th"), sel), String.is_empty(U.Ui.scratch(u, "@voice.th")))
  D.Dom.cat(View.vnote.loc(lc, Bool.and(here, Bool.or(Bool.not(String.is_empty(U.Ui.scratch(u, "@voice.err"))), Bool.and(U.Voice.on(u), Bool.not(String.is_empty(U.Ui.scratch(u, "@voice.hint")))))), u),
    D.Dom.cat(View.vstrip.loc(lc, Bool.and(here, U.Voice.on(u)), u),
      View.vask.loc(lc, Bool.and(String.eq(U.Ui.scratch(u, "@voice.uth"), sel), Bool.not(List.is_empty(&2, VC.Unsure, U.Voice.unsure(u)))), u)))

# a side question (/btw) and its answer, over the composer until closed

def View.voice(+u: U.Ui, +sel: String) -> List<&2, D.Node>:
  View.voice.loc(I.En{}, u, sel)

def View.btw(+lc: I.Loc, +u: U.Ui) -> List<&2, D.Node>:
  Bool.pick(List<&2, D.Node>, String.is_empty(U.Btw.question(u)), Nil{},
    [D.El{"div", "", [D.At{"class", "btw"}], [
      D.El{"div", "", [D.At{"class", "btw-head"}], [D.Dom.span("btw-q", "btw · " ++ U.Btw.question(u)),
        WI.WIcon.button("btw-close", "btw-close", "", MSG.T.common.close(lc), "x", 14, "")]},
      D.Dom.div("btw-a", [D.Txt{U.Btw.text(lc, U.Btw.reply.of(u))}])]}])

def View.mic.of(+u: U.Ui, +sel: String) -> String:
  Bool.pick(String, Bool.or(String.eq(U.Ui.scratch(u, "@voice.th"), sel), String.is_empty(U.Ui.scratch(u, "@voice.th"))), U.Ui.scratch(u, "@voice"), "")

# the send button, or Stop while a turn runs with nothing typed
def View.send.loc(+lc: I.Loc, +act: String, +send: String) -> D.Node:
  Bool.pick(D.Node, String.eq(act, "interrupt"), D.Dom.button("primary stop", act, "", MSG.T.composer.stop.tip(lc), send),
    D.Dom.button("primary", act, "", MSG.T.composer.enter.tip(lc, Bool.pick(String, String.eq(send, MSG.T.composer.queue(lc)), "steer", "queue")), send))

def View.composer.loc(+lc: I.Loc, +draft: String, +send: String, +act: String, +label: String, +ctx: String, +picking: Bool, +ms: List<&2, String>, +model: String, +info: String, +prov: String, +es: List<&2, String>, +effort: String, +rs: List<&2, U.SkillRow>,
    +atts: List<&2, String>, +up: String, vo: List<&2, D.Node>, +mic: String) -> D.Node:
  D.El{"div", "", [D.At{"class", "composer"}], D.Dom.cat(
    View.menu.loc(lc, picking, ms, model, info, prov, es, effort), D.Dom.cat(View.skills(rs), D.Dom.cat(vo, D.Dom.cat(View.attaching.loc(lc, atts, up), [
    D.El{"textarea", "composer", [D.At{"id", "composer"}, D.At{"rows", "3"}, D.At{"placeholder", MSG.T.composer.placeholder(lc)}, D.At{"value", draft}, D.On{"input", "draft"},
      D.At{"data-enter", Bool.pick(String, List.is_empty(&2, U.SkillRow, rs), "send", "skill")}, D.At{"data-enter-alt", "send-alt"}, D.At{"data-tab", Bool.pick(String, List.is_empty(&2, U.SkillRow, rs), "", "skill")}, D.At{"data-arrows", Bool.pick(String, List.is_empty(&2, U.SkillRow, rs), "", "skill-move")}, D.At{"data-esc", Bool.pick(String, List.is_empty(&2, U.SkillRow, rs), "", "skill-close")}, D.At{"data-paste", "attach"}], Nil{}},
    D.El{"div", "", [D.At{"class", "composer-foot"}], [
      WI.WIcon.after("model-chip", "flag", "models", MSG.T.composer.model.tip(lc), VLabel.effort.label(lc, label, effort) ++ ctx, "chevron-down", 14),
      D.El{"label", "", [D.At{"class", "attach"}, D.At{"title", MSG.T.composer.attach.tip(lc)}], [WI.WIcon.svg("paperclip", 16),
        D.El{"input", "attach-file", [D.At{"type", "file"}, D.At{"multiple", ""}, D.At{"data-attach", "attach"}], Nil{}}]},
      D.El{"span", "", [D.At{"class", "foot-end"}], [View.mic.loc(lc, mic),
        View.send.loc(lc, act, send)]}]}]))))}

# the thread's header, or the one given (a bot's)

def View.composer(+draft: String, +send: String, +act: String, +label: String, +ctx: String, +picking: Bool, +ms: List<&2, String>, +model: String, +info: String, +prov: String, +es: List<&2, String>, +effort: String, +rs: List<&2, U.SkillRow>,
    +atts: List<&2, String>, +up: String, vo: List<&2, D.Node>, +mic: String) -> D.Node:
  View.composer.loc(I.En{}, draft, send, act, label, ctx, picking, ms, model, info, prov, es, effort, rs, atts, up, vo, mic)

def View.send(+act: String, +send: String) -> D.Node:
  View.send.loc(I.En{}, act, send)

# the thread's header, or the one given (a bot's)
def View.top(+lc: I.Loc, hd: List<&2, D.Node>, t: M.Thread, +st: M.State, +chip: String, mini: List<&2, TO.Tool>, +panel: Bool, snoozed: Bool, picking: Bool, term: Bool, +diff: Bool, del: Bool, +ren: String, +rtext: String) -> List<&2, D.Node>:

  match hd:
    case Nil{}:
      [View.header(lc, t, st, chip, mini, panel, snoozed, picking, term, diff, del, ren, rtext)]
    case h <> r:
      h <> r

# The two parts of a thread's page that follow its draft: the composer and
# the asks (an answer with the draft). The page draws them with these, and
# host.js redraws only them for a keystroke (app.bend's composer and asks):
# rebuilding a long thread's page for every key took 280 ms a key.
def View.composer.t(+lc: I.Loc, +st: M.State, +sel: String, +drafts: Map<&2, String>, +op: Map<&2, Bool>, +nf: Map<&2, String>, +turn: M.TurnState, +pr: String, +mo: String,
    +atts: List<&2, String>, +up: String, vo: List<&2, D.Node>, +mic: String) -> D.Node:
  +stop = U.Follow.stop(View.running(turn), U.Str.get(drafts, sel), atts)
  View.composer.loc(lc, U.Str.get(drafts, sel), U.Follow.button(lc, st, sel, View.running(turn), stop, U.Str.get(drafts, sel)), U.Follow.act(stop), VLabel.model(lc, pr, mo), U.Usage.tail(st, sel), U.Flag.get(op, "models"), U.Models.menu(U.Str.get(nf, "models"), pr), mo, U.Str.get(nf, "models"), pr,
    U.Chip.all(U.Str.get(nf, "models"), pr, mo), U.Effort.of(st, sel), U.Skills.shown(st, sel, drafts, nf), atts, up, vo, mic)

def View.asks.box(+lc: I.Loc, +st: M.State, +id: String, +draft: String, +nf: Map<&2, String>) -> D.Node:
  D.El{"div", "asks", [D.At{"class", "asks"}, D.At{"id", "asks"}], View.asks(lc, U.Asks.open(st, id), draft, nf)}

# the same two from the Ui alone (app.bend's composer and asks)
def View.composer.k(+lc: I.Loc, u: U.Ui, t: M.Thread, +atts: List<&2, String>, +up: String, vo: List<&2, D.Node>, +mic: String) -> D.Node:
  match u t:
    case U.Ui{+st, +sel, +drafts, _, +op, _, _, _, _, _, _, _, _, _, +nf, _, _} M.Thread{_, _, _, _, _, +turn, _, _, _, _, _, _, _, _, _, +pr, +mo, _, _, _, _}:
      View.composer.t(lc, st, sel, drafts, op, nf, turn, pr, mo, atts, up, vo, mic)

def View.composer.l(+lc: I.Loc, +u: U.Ui) -> D.Node:
  View.composer.k(lc, u, U.Ui.thread(U.Ui.state(u), U.Ui.sel(u)), U.Ui.atts(u, U.Ui.sel(u)), U.Ui.uploading(u, U.Ui.sel(u)), D.Dom.cat(View.btw(lc, u), View.voice.loc(lc, u, U.Ui.sel(u))), View.mic.of(u, U.Ui.sel(u)))

def View.composer.u(+u: U.Ui) -> D.Node:
  View.composer.l(U.Lang.loc(u), u)

def View.asks.u(+lc: I.Loc, u: U.Ui) -> D.Node:
  match u:
    case U.Ui{+st, +sel, +drafts, _, _, _, _, _, _, _, _, _, _, _, +nf, _, _}:
      View.asks.box(lc, st, sel, U.Str.get(drafts, sel), nf)

# The project card (client.bend's Fab.*, core/order.bend): the intent's
# goal (a click opens the whole intent), Edit, and Buy it with its price
def View.fab.buy(+label: String) -> List<&2, D.Node>:
  Bool.pick(List<&2, D.Node>, String.is_empty(label), Nil{}, [D.Dom.button("fab-buy", "buy", "", "Order this project's parts in a thread that asks what's missing, exports the files and quotes them. The price is the bare parts at the smallest order; the quote adds assembly, components, shipping and duties.", label)])

def View.fab.body(open: Bool, +intent: String) -> List<&2, D.Node>:
  match open:
    case True{}:
      [D.Dom.div("fab-intent md", D.Md.render(intent))]
    case False{}:
      Nil{}

def View.fab.k(shown: Bool, +u: U.Ui) -> List<&2, D.Node>:
  match shown:
    case True{}:
      [D.El{"div", "fab", [D.At{"class", Bool.pick(String, U.Fab.open(u), "fab-card open", "fab-card")}], D.Dom.cat(
        [D.Dom.div("fab-row", [D.Dom.button("fab-goal", "fab-open", "", "The project's intent: what it is for and what it is made of", U.Fab.line(u)),
          D.Dom.button("fab-edit", "fab-edit", "", "Change the project's intent (it goes in the message box; Save sends it)", "Edit")])],
        View.fab.body(U.Fab.open(u), U.Fab.intent(u, U.Ui.sel(u))))}]
    case False{}:
      Nil{}

def View.fab(+u: U.Ui) -> List<&2, D.Node>:
  View.fab.k(U.Fab.shown(u), u)

# Ready to order: what there is to buy and Buy it, above the message box
# (U.Fab.ready), once the agent's turn is over; it glows as it comes
def View.ready.k(+line: String, +buy: String) -> List<&2, D.Node>:
  Bool.pick(List<&2, D.Node>, String.is_empty(line), Nil{},
    [D.El{"div", "ready", [D.At{"class", "fab-ready"}], D.Dom.cat([D.Dom.span("fab-ready-text", line)], List.append(&2, D.Node, View.fab.buy(buy),
      [D.Dom.button("fab-x", "fab-hide", "", "Put this away: Buy it stays in the thread's header", "×")]))}])

def View.ready(+u: U.Ui) -> List<&2, D.Node>:
  View.ready.k(U.Fab.ready(u), U.Fab.ready.btn(u))

# Away from the timeline's end with something new below (client.bend's
# Hold.pill): "3 new ↓" stuck to the timeline's foot, a click back down
def View.hold.pill(+label: String) -> List<&2, D.Node>:
  Bool.pick(List<&2, D.Node>, String.is_empty(label), Nil{},
    [D.El{"button", "hold-pill", [D.At{"class", "hold-pill"}, D.At{"data-value", ""}, D.On{"click", "hold-bottom"}], [D.Txt{label}]}])

def View.thread.p(+lc: I.Loc, u: U.Ui, t: M.Thread, pend: List<&2, String>, +atts: List<&2, String>, +up: String, +term: Bool, hd: List<&2, D.Node>, vo: List<&2, D.Node>, +mic: String, fab: List<&2, D.Node>, rd: List<&2, D.Node>, mini: List<&2, TO.Tool>, +pill: String) -> D.Node:
  match u t:
    case U.Ui{+st, +sel, +drafts, +live, +op, _, +now, _, panel, _, _, _, _, _, +nf, _, +tr} M.Thread{+id, p, ti, c, a, +turn, pe, ar, o, sa, ua, pi, en, br, wt, +pr, +mo, rm, im, se, de}:
      +stop = U.Follow.stop(View.running(turn), U.Str.get(drafts, sel), atts)
      D.El{"main", "thread", [D.At{"class", Bool.pick(String, term, "thread-view with-term", "thread-view") ++ Bool.pick(String, U.Flag.get(op, "drop"), " dropping", "")},
        D.At{"data-drop", "attach"}, D.At{"data-drop-hover", "drop-hover"}], D.Dom.cat(D.Dom.cat(
        View.top(lc, hd, M.Thread{id, p, ti, c, a, turn, pe, ar, o, sa, ua, pi, en, br, wt, pr, mo, rm, im, se, de}, st, UZ.Chip.of(st, id, now), mini, panel, M.Snooze.on(now, U.Side.until(st, id)), U.Flag.get(op, "snooze"), term, String.eq(U.Str.get(nf, "view.kind:" ++ id), "diff"), String.eq(U.Str.get(drafts, "@del"), id), U.Str.get(drafts, "@ren"), U.Str.get(drafts, "@f.title")), D.Dom.cat(D.Dom.cat(View.kx.note(U.KX.note(nf, sel), nf, sel), View.kn.loc(lc, U.KN.thread(nf, st, sel), True{})), D.Dom.cat([
        D.El{"div", "timeline", [D.At{"class", "timeline"}, D.At{"id", "timeline"}],
          D.Dom.cat(fab, D.Dom.cat(View.parent(st, id, U.Tasks.up(st, id)), D.Dom.cat([D.El{"div", "work", [D.At{"class", "work"}], D.Dom.cat(View.subs(lc, nf, id, SB.Subs.rows(st, nf, id, now)), View.pins.loc(lc, U.Ec.pins(U.Ec.make(U.MailvLocal.terms(tr), st, id, now), st, id), U.Ec.make(U.MailvLocal.terms(tr), st, id, now)))}], D.Dom.cat(Nil{},
          D.Dom.cat(View.paged(lc, drafts, M.State.mine(st, id), st, id, nf, View.running(turn), U.Ec.make(U.MailvLocal.terms(tr), st, id, now)), D.Dom.cat(View.sending.loc(lc, pend),
            D.Dom.cat(View.live.of.loc(lc, st, live, id), View.hold.pill(pill))))))))},
        View.asks.box(lc, st, id, U.Str.get(drafts, sel), nf),
        View.todos(lc, nf, id, U.Todos.shown(st, id, View.running(turn))),
        View.queued(lc, M.Queue.context(M.Queue.items(st, id)), View.running(turn))], D.Dom.cat(rd, [
        View.composer.t(lc, st, sel, drafts, op, nf, turn, pr, mo, atts, up, vo, mic)])))),
        D.Dom.when(term, View.term.loc(lc, U.Term.get(U.MailvLocal.terms(tr), id))))}

def View.thread.h(+lc: I.Loc, +u: U.Ui, +t: M.Thread, hd: List<&2, D.Node>) -> D.Node:
  View.thread.p(lc, u, t, U.Ui.pending(u, U.Ui.sel(u)), U.Ui.atts(u, U.Ui.sel(u)), U.Ui.uploading(u, U.Ui.sel(u)), U.Ui.flag(u, "term"), hd, D.Dom.cat(View.btw(lc, u), View.voice.loc(lc, u, U.Ui.sel(u))), View.mic.of(u, U.Ui.sel(u)), View.fab(u), View.ready(u), U.Fab.mini.tools(u), U.Hold.pill(u, M.Thread.id(t)))

def View.thread(+lc: I.Loc, +u: U.Ui, t: M.Thread) -> D.Node:
  View.thread.h(lc, u, t, Nil{})

# a bot, here or mirrored from a linked machine: its header and tabs
# (BU.head), then its thread (Chat) or another tab
def View.bot.k(+lc: I.Loc, chat: Bool, +u: U.Ui, +b: BT.Bot, +hd: BI.Head) -> D.Node:
  match chat:
    case True{}:
      View.thread.h(lc, u, U.Ui.thread(U.Ui.state(u), BT.Bot.thread(b)), [WB.WB.head.loc(lc, u, b, hd), WB.WB.tabs(lc, hd)])
    case False{}:
      D.El{"main", "bot", [D.At{"class", "thread-view bot-view"}], [WB.WB.head.loc(lc, u, b, hd), WB.WB.tabs(lc, hd), WB.WB.main(lc, u, b, BI.Head.tab(hd))]}

def View.bot.h(+lc: I.Loc, +u: U.Ui, +b: BT.Bot, +hd: BI.Head) -> D.Node:
  View.bot.k(lc, String.eq(BI.Head.tab(hd), "chat"), u, b, hd)

def View.bot(+lc: I.Loc, +u: U.Ui, +b: BT.Bot) -> D.Node:
  View.bot.h(lc, u, b, BI.BU.head.bot(u, b))

def View.main.s(+lc: I.Loc, settings: Bool, none: Bool, u: U.Ui, +sel: String, +n: Nat) -> D.Node:
  match settings none u:
    case True{} _ U.Ui{+st, s2, d, l, o, on, now, ad, pa, +v, r, w, e, +note, +nf, ob, tr}:
      View.settings.loc(lc, U.Ui{st, s2, d, l, o, on, now, ad, pa, v, r, w, e, note, nf, ob, tr}, st, nf, v, note)
    case False{} True{} U.Ui{st, _, _, _, _, _, _, _, _, _, _, _, _, _, nf, _, _}:
      View.empty.loc(lc, n, nf, st)
    case False{} False{} U.Ui{+st, s2, d, l, o, on, now, ad, pa, v, r, w, e, note, nf, ob, tr}:
      View.thread(lc, U.Ui{st, s2, d, l, o, on, now, ad, pa, v, r, w, e, note, nf, ob, tr}, U.Ui.thread(st, sel))

def View.main.k(+lc: I.Loc, none: Bool, u: U.Ui, +sel: String, +n: Nat) -> D.Node:
  match none u:
    case _ U.Ui{+st, _, _, _, +o, _, _, _, _, +v, _, _, _, +note, +nf, +ob, +tr}:
      View.main.s(lc, U.Flag.get(o, "settings"), none, u, sel, n)

# settings, else a bot, a room or a bot elsewhere, else the thread
def View.main.b(+lc: I.Loc, settings: Bool, bv: BI.View, +u: U.Ui, +sel: String, +n: Nat) -> D.Node:
  match settings bv:
    case False{} BI.VBot{b}:
      View.bot(lc, u, b)
    case False{} BI.VRoom{r}:
      WB.WB.room.loc(lc, u, r)
    case False{} BI.VRemote{id}:
      WB.WB.remote(lc, u, id)
    case _ _:
      View.main.k(lc, String.is_empty(sel), u, sel, n)

# the thread's own browser page (a bot's is the bot's), as a bot's
# Browser tab shows it
def View.browser.back.loc(+lc: I.Loc, on: Bool, +u: U.Ui) -> List<&2, D.Node>:
  match on:
    case True{}: [D.Dom.button("", "tk-handback", U.Ui.page(u), MSG.T.browser.handback.tip(lc), MSG.T.teach.handback(lc))]
    case False{}: Nil{}

def View.browser.back(on: Bool, +u: U.Ui) -> List<&2, D.Node>:
  View.browser.back.loc(I.En{}, on, u)

def View.browser.strip.loc(+lc: I.Loc, on: Bool, +u: U.Ui) -> List<&2, D.Node>:
  match on:
    case True{}:
      D.Dom.span("", U.CefUI.strip(u, U.Ui.page(u))) <> View.browser.back.loc(lc, Bool.not(String.is_empty(U.Teach.drive(u, U.Ui.page(u)))), u)
    case False{}: Nil{}

def View.browser.strip(on: Bool, +u: U.Ui) -> List<&2, D.Node>:
  View.browser.strip.loc(I.En{}, on, u)

def View.browser.loc(+lc: I.Loc, +u: U.Ui) -> List<&2, D.Node>:
  List.append(&2, D.Node, View.browser.strip.loc(lc, U.CefUI.strip_on(u, U.Ui.page(u)), u),
    [WB.WB.browser.k.loc(lc, String.is_empty(U.Ui.info(u, "shot." ++ U.Ui.page(u))), U.Ui.info(u, "shot." ++ U.Ui.page(u)), U.Ui.page(u), MSG.T.browser.thread(lc))])

# The Mechanical page (client.bend's Mpage): a chip per part, Open in 3D
# and Refresh, the part's renders (a click opens one in the lightbox), and
# where to get FreeCAD when the hub has none

def View.browser(+u: U.Ui) -> List<&2, D.Node>:
  View.browser.loc(I.En{}, u)

def View.mech.chips(ps: List<&2, MC.MPart>, +cur: String) -> List<&2, D.Node>:
  match ps:
    case Nil{}:
      Nil{}
    case MC.MPart{+p, +n, _} <> t:
      D.Dom.button(Bool.pick(String, String.eq(p, cur), "on", ""), "mech-part", p, p, n) <> View.mech.chips(t, cur)

def View.mech.shots(fs: List<&2, MC.MFile>, +root: String) -> List<&2, D.Node>:
  match fs:
    case Nil{}:
      Nil{}
    case +f <> t:
      D.El{"figure", "", [D.At{"class", "mech-shot"}], [D.Dom.span("mech-view", MC.Mech.view.of(f)),
        D.El{"a", "", [D.At{"href", MC.Mech.url(root, f)}, D.At{"target", "_blank"}, D.At{"data-lightbox", MC.Mech.url(root, f)}],
          [D.El{"img", "", [D.At{"src", MC.Mech.url(root, f)}, D.At{"alt", MC.Mech.view.of(f)}], Nil{}}]}]} <> View.mech.shots(t, root)

def View.mech.part(m: Maybe<&2, MC.MPart>, +root: String) -> List<&2, D.Node>:
  match m:
    case Some{MC.MPart{_, +n, +fs}}:
      Bool.pick(List<&2, D.Node>, List.is_empty(&2, MC.MFile, fs), [D.Dom.span("mech-say", MC.Mech.unrendered(n))],
        [D.El{"div", "", [D.At{"class", "mech-grid"}], View.mech.shots(fs, root)}])
    case None{}:
      Nil{}

def View.mech.k(+lc: I.Loc, state: Nat, +u: U.Ui) -> List<&2, D.Node>:
  match state:
    case 0n:
      [D.Dom.span("mech-say", MSG.T.viewer.mech.loading(lc))]
    case 1n:
      [D.Dom.span("mech-say", MC.Mech.none())]
    case _:
      D.El{"div", "", [D.At{"class", "mech-head"}], List.append(&2, D.Node, View.mech.chips(U.Mpage.parts(u), U.Mpage.cur.path(u)),
        [D.Dom.button("", "mech-3d", MC.Mech.abs(U.Mpage.root(u), U.Mpage.cur.path(u)), MSG.T.mech.turn(lc), "3D"),
         D.Dom.button(Bool.pick(String, String.is_empty(U.Mpage.busy(u)), "primary", "busy"), Bool.pick(String, String.is_empty(U.Mpage.busy(u)), "mech-render", ""),
           U.Mpage.cur.path(u), MSG.T.mech.render.tip(lc), U.Mpage.render.label.loc(lc, u)),
         D.Dom.button("", "mech-refresh", "", MSG.T.mech.refresh.tip(lc), MSG.T.common.refresh(lc))])}
      <> List.append(&2, D.Node, D.Dom.when(Bool.not(String.is_empty(U.Mpage.err(u))), D.Dom.span("mech-say", MSG.T.viewer.render.failed(lc, U.Mpage.err(u)))),
        View.mech.part(U.Mpage.cur(u), U.Mpage.root(u)))

def View.mech(+lc: I.Loc, +u: U.Ui) -> List<&2, D.Node>:
  [D.El{"div", "", [D.At{"class", "mech-page"}],
    List.append(&2, D.Node, View.mech.k(lc, U.Mpage.state(u), u), D.Dom.when(Bool.not(String.is_empty(U.Mpage.note.loc(lc, u))), D.Dom.span("mech-note", U.Mpage.note.loc(lc, u))))}]

# Open in KiCad: the project opens in KiCad on the hub's machine, as the
# window's button does
def View.kicad.loc(+lc: I.Loc) -> D.Node:
  D.El{"span", "", Nil{}, [D.Dom.button("", "kicad-open", "", MSG.T.viewer.kicad.tip(lc), MSG.T.palette.kicad(lc)), D.Dom.button("", "kicad-export10", "", MSG.T.viewer.kicad10.tip(lc), MSG.T.viewer.kicad10(lc))]}

# A fit button: host.js fits the board, schematic or model on show

def View.kicad() -> D.Node:
  View.kicad.loc(I.En{})

def View.fit.loc(+lc: I.Loc) -> D.Node:
  D.El{"button", "", [D.At{"class", "fit"}, D.At{"title", MSG.T.viewer.fit.tip(lc)}, D.At{"data-view", "fit"}], [D.Txt{MSG.T.viewer.fit(lc)}]}

def View.fit() -> D.Node:
  View.fit.loc(I.En{})

def View.bar.loc(+lc: I.Loc, kids: List<&2, D.Node>, +hint: String) -> D.Node:
  D.El{"div", "", [D.At{"class", "viewer-bar"}], List.append(&2, D.Node, kids, [D.Dom.span("hint", hint), View.fit.loc(lc)])}

# a hierarchical schematic's sheets as chips (the one on show lit; the
# root's value is "", the project's own file); none with one sheet

def View.bar(kids: List<&2, D.Node>, +hint: String) -> D.Node:
  View.bar.loc(I.En{}, kids, hint)

def View.sheet.chips.loc(+lc: I.Loc, rs: List<&2, HI.Row>, +i: Nat, +cur: String) -> List<&2, D.Node>:
  match rs:
    case Nil{}:
      Nil{}
    case HI.Row{+d, +n, +f, _} <> t:
      D.Dom.button(Bool.pick(String, String.eq(Bool.pick(String, Nat.is_eq(i, 0n), "", f), cur), "on", ""), "view-sheet", Bool.pick(String, Nat.is_eq(i, 0n), "", f), f,
        Bool.pick(String, Nat.is_eq(i, 0n), MSG.T.viewer.sheet.top(lc), String.repeat("› ", d) ++ n)) <> View.sheet.chips.loc(lc, t, 1n+i, cur)

# the project's boards as chips (the one on show lit) when it has several
# (U.Bd); what: the bar's words otherwise
def View.board.chips(fs: List<&2, String>, +cur: String) -> List<&2, D.Node>:
  match fs:
    case Nil{}:
      Nil{}
    case +f <> t:
      D.Dom.button(Bool.pick(String, U.Bd.on(cur, f), "on", ""), "board-pick", f, f, U.Bd.name(f)) <> View.board.chips(t, cur)

def View.boards(+u: U.Ui, +what: String) -> List<&2, D.Node>:
  Bool.pick(List<&2, D.Node>, U.Bd.many(u), View.board.chips(U.Bd.files(u), U.Bd.shown(u, U.Ui.view_get(u, "view.path"))), [D.Dom.span("what", what)])

def View.sheet.chips(rs: List<&2, HI.Row>, +i: Nat, +cur: String) -> List<&2, D.Node>:
  View.sheet.chips.loc(I.En{}, rs, i, cur)

def View.sheets.of(+lc: I.Loc, +rs: List<&2, HI.Row>, +u: U.Ui) -> List<&2, D.Node>:
  Bool.pick(List<&2, D.Node>, Nat.is_gt(List.length(&2, HI.Row, rs), 1n),
    View.sheet.chips.loc(lc, rs, 0n, U.Ui.view_get(u, "view.path")), [D.Dom.span("what", MSG.T.viewer.schematic.tip(lc))])

# the project's sheets, read once per render
def View.sheets.loc(+lc: I.Loc, +u: U.Ui) -> List<&2, D.Node>:
  View.sheets.of(lc, HI.Hier.read(U.Ui.info(u, "sheets:" ++ U.Ui.root(u))), u)

# the Layers menu (core/layers.bend): each layer shown by default, in
# KiCad's order (plot2d.js hides those the file has nothing on), a row of
# its colour (a click shows or hides it) and its name (a click makes it the
# active layer and dims the rest; a click on it again puts all back). Over
# a board's rows, how the layers other than the active one show: All, Dim
# or Only.

def View.sheets(+u: U.Ui) -> List<&2, D.Node>:
  View.sheets.loc(I.En{}, u)

def View.layer.alpha(l: U32 & U32) -> U32:
  (_, a) = l
  a

def View.layer.color(l: U32 & U32) -> U32:
  (c, _) = l
  c

def View.layer.row.loc(+lc: I.Loc, shown: Bool, +i: U32, +name: String, +color: U32, +off: U32, +act: Bool, rest: List<&2, D.Node>) -> List<&2, D.Node>:
  match shown:
    case True{}:
      D.El{"div", "", [D.At{"class", "layer-row" ++ Bool.pick(String, act, " active", "") ++ Bool.pick(String, LY.Lyr.has(off, i), " off", "")}, D.At{"data-layer", U32.show(i)}],
        [D.El{"button", "", [D.At{"class", "swatch"}, D.At{"title", MSG.T.viewer.layer.toggle(lc, name)}, D.At{"data-value", U32.show(i)},
            D.At{"style", "background:" ++ View.hex(color)}, D.On{"click", "view-layer"}], Nil{}},
         D.El{"button", "", [D.At{"class", "lname"}, D.At{"title", Bool.pick(String, act, MSG.T.viewer.layers.restore(lc), MSG.T.viewer.layer.focus(lc, name))}, D.At{"data-value", U32.show(i)},
            D.On{"click", "view-layer-pick"}], [D.Txt{name}]}]} <> rest
    case False{}:
      rest

def View.layer.row(shown: Bool, +i: U32, +name: String, +color: U32, +off: U32, +act: Bool, rest: List<&2, D.Node>) -> List<&2, D.Node>:
  View.layer.row.loc(I.En{}, shown, i, name, color, off, act, rest)

def View.layer.rows.loc(+lc: I.Loc, ls: List<&2, U32>, +k: U32, +off: U32, +f: LY.Focus) -> List<&2, D.Node>:
  match ls:
    case Nil{}:
      Nil{}
    case +i <> t:
      View.layer.row.loc(lc, U32.is_gt(View.layer.alpha(LO.Look.of.t(False{}, k, i)), 0), i, LY.Lyr.name(k, i), View.layer.color(LO.Look.of.t(False{}, k, i)), off,
        Bool.and(LY.Lyr.on(f), U32.is_eq(i, LY.Lyr.active(f))), View.layer.rows.loc(lc, t, k, off, f))

def View.layer.rows(ls: List<&2, U32>, +k: U32, +off: U32, +f: LY.Focus) -> List<&2, D.Node>:
  View.layer.rows.loc(I.En{}, ls, k, off, f)

def View.layer.mode(+m: U32, +cur: U32, +label: String, +tip: String) -> D.Node:
  D.El{"button", "", [D.At{"class", Bool.pick(String, U32.is_eq(m, cur), "lmode on", "lmode")}, D.At{"title", tip}, D.At{"data-value", U32.show(m)}, D.On{"click", "view-layer-mode"}], [D.Txt{label}]}

# KiCad's high contrast: the other layers as they are, dimmed, or hidden
def View.layer.modes.loc(+lc: I.Loc, sch: Bool, +f: LY.Focus) -> List<&2, D.Node>:
  match sch:
    case True{}:
      Nil{}
    case False{}:
      +cur = {Bool.pick(U32, LY.Lyr.on(f), LY.Lyr.mode(f), 0) : U32}
      [D.El{"div", "", [D.At{"class", "layer-modes"}], [View.layer.mode(0, cur, MSG.T.viewer.layers.all(lc), MSG.T.viewer.layers.all.tip(lc)),
        View.layer.mode(1, cur, MSG.T.viewer.layers.dim(lc), MSG.T.viewer.layers.dim.tip(lc)),
        View.layer.mode(2, cur, MSG.T.viewer.layers.only(lc), MSG.T.viewer.layers.only.tip(lc))]}]

# the menu's button names the active layer while one stands out

def View.layer.modes(sch: Bool, +f: LY.Focus) -> List<&2, D.Node>:
  View.layer.modes.loc(I.En{}, sch, f)

def View.layer.head.loc(+lc: I.Loc, +k: U32, +f: LY.Focus) -> String:
  Bool.pick(String, LY.Lyr.on(f), MSG.T.web.viewer.layers.active(lc, LY.Lyr.name(k, LY.Lyr.active(f))), MSG.T.viewer.layers(lc))

# the look kind of a plot: a board or schematic, a comparison's marks too

def View.layer.head(+k: U32, +f: LY.Focus) -> String:
  View.layer.head.loc(I.En{}, k, f)

def View.layer.kind(sch: Bool, cmp: Bool) -> U32:
  U32.or(Bool.pick(U32, sch, 1, 0), Bool.pick(U32, cmp, LO.Look.diff(), 0))

def View.layers.loc(+lc: I.Loc, +sch: Bool, +cmp: Bool, +off: U32, +f: LY.Focus) -> D.Node:
  +k = View.layer.kind(sch, cmp)
  D.El{"details", "", [D.At{"class", "layers"}], [D.El{"summary", "", [D.At{"title", MSG.T.viewer.layers.tip(lc)}], [D.Txt{View.layer.head.loc(lc, k, f)}]},
    D.El{"div", "", [D.At{"class", "layer-list"}], List.append(&2, D.Node, View.layer.modes.loc(lc, sch, f), View.layer.rows.loc(lc, LY.Lyr.order(k), k, off, f))}]}

# The design history bar (client.bend's Hs.bar): the arrows, play and
# the position, the changes, compare and live buttons; a scrubber with a
# tick a step; what the agent said before the step, and a way back to it.
# A comparison names its two sides (each opens the menu of versions) over
# a legend.

def View.layers(+sch: Bool, +cmp: Bool, +off: U32, +f: LY.Focus) -> D.Node:
  View.layers.loc(I.En{}, sch, cmp, off, f)

def View.hist.icon(+cls: String, +action: String, +title: String, +name: String) -> D.Node:
  WI.WIcon.button(cls, action, "", title, name, 16, "")

def View.hist.live.loc(+lc: I.Loc, at: Nat) -> List<&2, D.Node>:
  match at:
    case 0n:
      Nil{}
    case _:
      [D.Dom.button("hbtn live", "hist-live", "", MSG.T.history.live.tip(lc), MSG.T.history.live(lc))]

def View.hist.live(at: Nat) -> List<&2, D.Node>:
  View.hist.live.loc(I.En{}, at)

def View.hist.tick.loc(+lc: I.Loc, +id: String, +i: Nat, +n: Nat, +cur: Bool) -> D.Node:
  D.El{"button", "", [D.At{"class", Bool.pick(String, cur, "tick on", "tick")}, D.At{"title", Bool.pick(String, String.is_empty(id), MSG.T.history.file.tip(lc), MSG.T.history.step.label(lc, Nat.show(1n+i)))},
    D.At{"style", "left:calc(100% * " ++ Nat.show(i) ++ " / " ++ Nat.show(Bool.pick(Nat, Nat.is_eq(n, 0n), 1n, n)) ++ ")"}, D.At{"data-value", id}, D.On{"click", "hist-go"}], Nil{}}

def View.hist.tick(+id: String, +i: Nat, +n: Nat, +cur: Bool) -> D.Node:
  View.hist.tick.loc(I.En{}, id, i, n, cur)

def View.hist.ticks.loc(+lc: I.Loc, ss: List<&2, HS.Step>, +i: Nat, +n: Nat, +at: Nat) -> List<&2, D.Node>:
  match ss:
    case Nil{}:
      [View.hist.tick.loc(lc, "", i, n, Nat.is_eq(at, 0n))]
    case +s <> t:
      View.hist.tick.loc(lc, HS.Step.id(s), i, n, Nat.is_eq(at, 1n+i)) <> View.hist.ticks.loc(lc, t, 1n+i, n, at)

def View.hist.ticks(ss: List<&2, HS.Step>, +i: Nat, +n: Nat, +at: Nat) -> List<&2, D.Node>:
  View.hist.ticks.loc(I.En{}, ss, i, n, at)

def View.hist.said.loc(+lc: I.Loc, live: Bool, +text: String) -> List<&2, D.Node>:
  match live:
    case True{}:
      [D.Dom.span("said", text)]
    case False{}:
      [WI.WIcon.button("icon", "hist-jump", "", MSG.T.history.thread.point(lc), "message-square", 16, ""),
       D.El{"button", "", [D.At{"class", "said"}, D.At{"title", MSG.T.history.thread.point(lc)}, D.At{"data-value", ""}, D.On{"click", "hist-jump"}], [D.Txt{text}]}]

def View.hist.said(live: Bool, +text: String) -> List<&2, D.Node>:
  View.hist.said.loc(I.En{}, live, text)

def View.hist.walk(+lc: I.Loc, b: U.HBar, +u: U.Ui) -> D.Node:
  match b:
    case U.HBar{_, +n, +at, +title, _, +label, +said, +pl, +df, _, _, _, _}:
      D.El{"div", "", [D.At{"class", "hist"}], [
        D.El{"div", "", [D.At{"class", "hist-row"}], List.append(&2, D.Node, [
          View.hist.icon("icon", "hist-first", MSG.T.history.first(lc), "skip-back"),
          View.hist.icon("icon", "hist-prev", MSG.T.history.previous(lc), "chevron-left"),
          View.hist.icon("icon play", "hist-play", Bool.pick(String, pl, MSG.T.history.pause(lc), MSG.T.history.play(lc)), Bool.pick(String, pl, "pause", "play")),
          View.hist.icon("icon", "hist-next", MSG.T.history.next(lc), "chevron-right"),
          View.hist.icon("icon", "hist-last", MSG.T.history.last(lc), "skip-forward"),
          D.Dom.span("title", title), D.Dom.span("grow", ""),
          D.Dom.button(Bool.pick(String, df, "hbtn on", "hbtn"), "hist-diff", "", MSG.T.history.changes.tip(lc), MSG.T.viewer.history.changes(lc)),
          D.Dom.button("hbtn", "cmp", "a", MSG.T.history.compare.tip(lc), MSG.T.viewer.history.compare(lc))], View.hist.live.loc(lc, at))},
        D.El{"div", "", [D.At{"class", "hist-scrub"}], View.hist.ticks.loc(lc, U.Hs.track(u), 0n, n, at)},
        D.El{"div", "", [D.At{"class", "hist-said"}], View.hist.said.loc(lc, Nat.is_eq(at, 0n), Bool.pick(String, Nat.is_eq(at, 0n), U.Hs.live.said.loc(lc, n), Bool.pick(String, String.is_empty(said), label, said)))}]}

def View.hist.key(+layer: U32, +label: String) -> D.Node:
  D.El{"span", "", [D.At{"class", "key"}], [D.El{"i", "", [D.At{"style", "background:" ++ View.hex(LO.Look.mark.color(False{}, layer))}], Nil{}}, D.Txt{label}]}

# Old / Diff / New (client.bend's Rv.showing), the legend only under Diff
def View.hist.as.loc(+lc: I.Loc, +showing: String, +as: String, +label: String) -> D.Node:
  D.Dom.button(Bool.pick(String, String.eq(showing, as), "hbtn on", "hbtn"), "rv-as", as, MSG.T.history.version.show(lc, label), label)

def View.hist.as(+showing: String, +as: String, +label: String) -> D.Node:
  View.hist.as.loc(I.En{}, showing, as, label)

def View.hist.legend.loc(+lc: I.Loc, on: Bool) -> List<&2, D.Node>:
  match on:
    case True{}:
      [View.hist.key(26, MSG.T.viewer.history.removed(lc)), View.hist.key(27, MSG.T.viewer.history.changed(lc)), View.hist.key(28, MSG.T.viewer.history.added(lc))]
    case False{}:
      Nil{}

def View.hist.legend(on: Bool) -> List<&2, D.Node>:
  View.hist.legend.loc(I.En{}, on)

def View.hist.cmp.loc(+lc: I.Loc, b: U.HBar, +u: U.Ui) -> D.Node:
  match b:
    case U.HBar{_, _, _, _, _, _, _, _, _, _, +a, +bb, +menu}:
      +showing = {U.Rv.showing(u) : String}
      D.El{"div", "", [D.At{"class", "hist cmp"}], [
        D.El{"div", "", [D.At{"class", "hist-row"}], [
          D.Dom.button(Bool.pick(String, String.eq(menu, "a"), "hbtn side on", "hbtn side"), "cmp-side", "a", MSG.T.history.old.tip(lc), MSG.T.history.old.label(lc, a)),
          D.Dom.button(Bool.pick(String, String.eq(menu, "b"), "hbtn side on", "hbtn side"), "cmp-side", "b", MSG.T.history.new.tip(lc), MSG.T.history.new.label(lc, bb)),
          D.Dom.span("grow", ""), D.Dom.button("hbtn", "rv-bom", "", MSG.T.history.bom.tip(lc), MSG.T.review.bom(lc)), D.Dom.button("hbtn", "cmp-close", "", MSG.T.history.live.tip(lc), MSG.T.common.close(lc))]},
        D.El{"div", "", [D.At{"class", "hist-legend"}], List.append(&2, D.Node, View.hist.legend.loc(lc, String.eq(showing, "diff")),
          [D.Dom.span("grow", ""), View.hist.as.loc(lc, showing, "old", MSG.T.viewer.history.old(lc)), View.hist.as.loc(lc, showing, "diff", MSG.T.viewer.diff(lc)), View.hist.as.loc(lc, showing, "new", MSG.T.viewer.history.new(lc))])}]}

def View.hist.cmp(b: U.HBar, +u: U.Ui) -> D.Node:
  View.hist.cmp.loc(I.En{}, b, u)

def View.hist.k(+lc: I.Loc, on: Bool, cmp: Bool, b: U.HBar, +u: U.Ui) -> List<&2, D.Node>:
  match on cmp:
    case False{} _:
      Nil{}
    case True{} True{}:
      [View.hist.cmp.loc(lc, b, u)]
    case True{} False{}:
      [View.hist.walk(lc, b, u)]

def View.hist(+lc: I.Loc, +b: U.HBar, +u: U.Ui) -> List<&2, D.Node>:
  View.hist.k(lc, U.HBar.on(b), U.HBar.cmp(b), b, u)

# the menu of versions over the stage, a row each; a press outside shuts it
def View.hist.rows(rs: List<&2, U.HRow>) -> List<&2, D.Node>:
  match rs:
    case Nil{}:
      Nil{}
    case U.HRow{+l, +sub, +v} <> t:
      D.El{"button", "", [D.At{"class", "row"}, D.At{"data-value", v}, D.On{"click", "cmp-pick"}], [D.Dom.span("name", l), D.Dom.span("sub", sub)]} <> View.hist.rows(t)

# a ref typed in: Enter sends rv-ref "<side>\t<text>" (host.js's data-pre)
def View.hist.ref.loc(+lc: I.Loc, +side: String) -> D.Node:
  D.El{"input", "rvref", [D.At{"id", "rvref"}, D.At{"class", "field"}, D.At{"placeholder", MSG.T.viewer.history.ref(lc)}, D.At{"autocomplete", "off"}, D.At{"spellcheck", "false"},
    D.At{"data-enter", "rv-ref"}, D.At{"data-pre", Bool.pick(String, String.eq(side, "b"), "b\t", "a\t")}], Nil{}}

def View.hist.ref(+side: String) -> D.Node:
  View.hist.ref.loc(I.En{}, side)

def View.hist.menu.k(+lc: I.Loc, open: Bool, +u: U.Ui, +side: String) -> List<&2, D.Node>:
  match open:
    case True{}:
      [D.El{"div", "", [D.At{"class", "hist-scrim"}, D.At{"data-value", ""}, D.On{"click", "cmp-shut"}], Nil{}},
       D.El{"div", "", [D.At{"class", "hist-menu"}], D.Dom.span("head", Bool.pick(String, String.eq(side, "b"), MSG.T.viewer.history.compare.new(lc), MSG.T.viewer.history.compare.old(lc))) <> (View.hist.ref.loc(lc, side) <> View.hist.rows(U.Hs.menu.loc(lc, u, side)))}]
    case False{}:
      Nil{}

def View.hist.menu(+lc: I.Loc, +u: U.Ui) -> D.Node:
  D.El{"div", "", [D.At{"class", "hist-over"}], View.hist.menu.k(lc, Bool.not(String.is_empty(U.Ui.scratch(u, "@cmp.menu"))), u, U.Ui.scratch(u, "@cmp.menu"))}

# Review comments (client.bend's Rc), over the canvas: a pin at each place
# (plot2d.js puts them where the plot's own transform says: data-x, data-y in
# mm), the note being written and the open comment as cards. A right click
# on the plot starts a note (plot2d.js, "rv-note" with the place).
def View.rc.pin.loc(+lc: I.Loc, p: RVW.Pin, +sel: String) -> D.Node:
  match p:
    case RVW.Pin{+id, +x, +y, _, _, +res, +cnt}:
      D.El{"button", "rcp:" ++ id, [D.At{"class", "rc-pin" ++ Bool.pick(String, res, " res", "") ++ Bool.pick(String, String.eq(id, sel), " sel", "")}, D.At{"data-x", F32.show(x)}, D.At{"data-y", F32.show(y)},
        D.At{"data-value", id}, D.At{"title", MSG.T.review.comment.open(lc)}, D.On{"click", "rv-pin"}], Bool.pick(List<&2, D.Node>, Nat.is_gt(cnt, 0n), [D.Txt{Nat.show(cnt)}], Nil{})}

def View.rc.pin(p: RVW.Pin, +sel: String) -> D.Node:
  View.rc.pin.loc(I.En{}, p, sel)

def View.rc.pins.loc(+lc: I.Loc, ps: List<&2, RVW.Pin>, +sel: String) -> List<&2, D.Node>:
  match ps:
    case Nil{}:
      Nil{}
    case +p <> t:
      View.rc.pin.loc(lc, p, sel) <> View.rc.pins.loc(lc, t, sel)

def View.rc.pins(ps: List<&2, RVW.Pin>, +sel: String) -> List<&2, D.Node>:
  View.rc.pins.loc(I.En{}, ps, sel)

def View.rc.reps(rs: List<&2, String>) -> List<&2, D.Node>:
  match rs:
    case Nil{}:
      Nil{}
    case +r <> t:
      D.El{"div", "", [D.At{"class", "rc-reply"}], [D.Dom.span("by", Maybe.default(&2, String, List.get(&2, String, String.split(r, '\u{1f}'), 1n), "")),
        D.Txt{Maybe.default(&2, String, List.get(&2, String, String.split(r, '\u{1f}'), 0n), "")}]} <> View.rc.reps(t)

def View.rc.open.k.loc(+lc: I.Loc, o: U.RcOpen, +u: U.Ui) -> D.Node:
  match o:
    case U.RcOpen{+id, +text, +by, +place, +res, +reps}:
      D.El{"div", "rc-open", [D.At{"class", "rc-card rc-open"}],
        List.concat(&2, D.Node, [[D.El{"div", "", [D.At{"class", "rc-head"}], [D.Dom.span("what", by ++ " · " ++ place ++ Bool.pick(String, res, " · resolved", "")), D.Dom.button("rc-x", "rv-pin", "", MSG.T.common.close(lc), "×")]},
          D.El{"div", "", [D.At{"class", "rc-text"}], [D.Txt{text}]}],
          View.rc.reps(reps),
          [WB.WB.input("rcreply", "rcreply", U.Ui.scratch(u, "@f.rcreply"), MSG.T.review.reply(lc), "rv-reply-send"),
           D.El{"div", "", [D.At{"class", "rc-row"}], [D.Dom.button("primary", "rv-reply-send", "", MSG.T.review.reply.tip(lc), MSG.T.review.reply(lc)),
             D.Dom.button("", Bool.pick(String, res, "rv-reopen", "rv-resolve"), id, Bool.pick(String, res, MSG.T.review.reopen.tip(lc), MSG.T.review.resolve.tip(lc)), Bool.pick(String, res, MSG.T.review.reopen(lc), MSG.T.review.resolve(lc)))]}]])}

def View.rc.open.k(o: U.RcOpen, +u: U.Ui) -> D.Node:
  View.rc.open.k.loc(I.En{}, o, u)

def View.rc.open.loc(+lc: I.Loc, m: Maybe<&2, U.RcOpen>, +u: U.Ui) -> List<&2, D.Node>:
  match m:
    case Some{+o}:
      [View.rc.open.k.loc(lc, o, u)]
    case None{}:
      Nil{}

def View.rc.open(m: Maybe<&2, U.RcOpen>, +u: U.Ui) -> List<&2, D.Node>:
  View.rc.open.loc(I.En{}, m, u)

def View.rc.note.k.loc(+lc: I.Loc, +xs: List<&2, String>, +u: U.Ui) -> List<&2, D.Node>:
  [D.El{"span", "rc-draft", [D.At{"class", "rc-pin draft"}, D.At{"data-x", Maybe.default(&2, String, List.get(&2, String, xs, 0n), "0")}, D.At{"data-y", Maybe.default(&2, String, List.get(&2, String, xs, 1n), "0")}], Nil{}},
   D.El{"div", "rc-note", [D.At{"class", "rc-card rc-note"}, D.At{"data-x", Maybe.default(&2, String, List.get(&2, String, xs, 0n), "0")}, D.At{"data-y", Maybe.default(&2, String, List.get(&2, String, xs, 1n), "0")}],
    [D.El{"div", "", [D.At{"class", "rc-head"}], [D.Dom.span("what", MSG.T.review.here(lc))]},
     WB.WB.input("rcnote", "rcnote", U.Ui.scratch(u, "@f.rcnote"), MSG.T.review.note(lc), "rv-note-send"),
     D.El{"div", "", [D.At{"class", "rc-row"}], [D.Dom.button("primary", "rv-note-send", "", MSG.T.review.pin.tip(lc), MSG.T.composer.send(lc)), D.Dom.button("", "rv-note-shut", "", MSG.T.review.cancel.tip(lc), MSG.T.common.cancel(lc))]}]}]

def View.rc.note.k(+xs: List<&2, String>, +u: U.Ui) -> List<&2, D.Node>:
  View.rc.note.k.loc(I.En{}, xs, u)

def View.rc.note.if.loc(+lc: I.Loc, none: Bool, +at: String, +u: U.Ui) -> List<&2, D.Node>:
  match none:
    case True{}:
      Nil{}
    case False{}:
      View.rc.note.k.loc(lc, String.split(at, '\t'), u)

def View.rc.note.if(none: Bool, +at: String, +u: U.Ui) -> List<&2, D.Node>:
  View.rc.note.if.loc(I.En{}, none, at, u)

def View.rc.note.loc(+lc: I.Loc, +at: String, +u: U.Ui) -> List<&2, D.Node>:
  View.rc.note.if.loc(lc, String.is_empty(at), at, u)

def View.rc.note(+at: String, +u: U.Ui) -> List<&2, D.Node>:
  View.rc.note.loc(I.En{}, at, u)

def View.rc.loc(+lc: I.Loc, +u: U.Ui) -> List<&2, D.Node>:
  List.concat(&2, D.Node, [View.rc.pins.loc(lc, U.Rc.pins(u), U.Rc.sel(u)), View.rc.note.loc(lc, U.Rc.at(u), u), View.rc.open.loc(lc, U.Rc.open(u), u)])

# The Checks page: the gate line, then the checklist's boxes under their
# group's heading; a click ticks one ("rv-tick" and its index)

def View.rc(+u: U.Ui) -> List<&2, D.Node>:
  View.rc.loc(I.En{}, u)

def View.rc.tasks.loc(+lc: I.Loc, ts: List<&2, U.RpTask>, +prev: String) -> List<&2, D.Node>:
  match ts:
    case Nil{}:
      Nil{}
    case U.RpTask{+i, +text, +done, +group} <> t:
      List.append(&2, D.Node, Bool.pick(List<&2, D.Node>, Bool.and(Bool.not(String.is_empty(group)), Bool.not(String.eq(group, prev))), [D.Dom.span("rc-group", group)], Nil{}),
        D.El{"button", "rct:" ++ Nat.show(i), [D.At{"class", "rc-task" ++ Bool.pick(String, done, " done", "")}, D.At{"data-value", Nat.show(i)}, D.At{"title", Bool.pick(String, done, MSG.T.review.untick(lc), MSG.T.review.tick(lc))}, D.On{"click", "rv-tick"}],
          [D.El{"span", "", [D.At{"class", "rc-box"}], [D.Txt{Bool.pick(String, done, "✓", "")}]}, D.Txt{text}]} <> View.rc.tasks.loc(lc, t, group))

def View.rc.tasks(ts: List<&2, U.RpTask>, +prev: String) -> List<&2, D.Node>:
  View.rc.tasks.loc(I.En{}, ts, prev)

def View.rc.checks.k.loc(+lc: I.Loc, on: Bool, +u: U.Ui) -> List<&2, D.Node>:
  match on:
    case True{}:
      [D.El{"div", "", [D.At{"class", "rc-checks"}], D.El{"div", "", [D.At{"class", Bool.pick(String, String.is_empty(U.Rp.gate(u)), "rc-gate ok", "rc-gate bad")}],
        [D.Txt{Bool.pick(String, String.is_empty(U.Rp.gate(u)), MSG.T.review.passes(lc), MSG.T.review.fails(lc, U.Rp.gate(u)))}]} <> View.rc.tasks.loc(lc, U.Rp.tasks(u), "")}]
    case False{}:
      Nil{}

def View.rc.checks.k(on: Bool, +u: U.Ui) -> List<&2, D.Node>:
  View.rc.checks.k.loc(I.En{}, on, u)

def View.rc.checks.loc(+lc: I.Loc, +u: U.Ui) -> List<&2, D.Node>:
  View.rc.checks.k.loc(lc, Bool.and(String.eq(U.Rp.page(u), "checks"), Bool.not(String.is_empty(U.RpView.md(U.Rp.view.loc(lc, u))))), u)

# a board or schematic: host.js (plot2d.js) draws the hub's plot of it on
# the canvas whose data-key it carries, less the layers data-off names

def View.rc.checks(+u: U.Ui) -> List<&2, D.Node>:
  View.rc.checks.loc(I.En{}, u)

def View.plot(+lc: I.Loc, +u: U.Ui, +what: List<&2, D.Node>) -> List<&2, D.Node>:
  View.bar.loc(lc, List.append(&2, D.Node, what, [View.layers.loc(lc, String.eq(U.Solid.kind(u), "schematic"), Nat.is_eq(HS.Hist.kind(U.Ui.view_get(u, "view.path")), 2n), U.Solid.off(u), U.Lfocus.of(u))]), MSG.T.viewer.pan.tip(lc))
   <> D.El{"div", "", [D.At{"class", "viewer-stage"}], D.El{"canvas", "plot", [D.At{"id", "plot"}, D.At{"data-key", U.Solid.want(u)},
        D.At{"data-off", U32.show(LY.Masks.off(LY.Lyr.masks(U.Lfocus.kind(u), U.Solid.off(u), U.Lfocus.of(u))))},
        D.At{"data-dim", U32.show(LY.Masks.dim(LY.Lyr.masks(U.Lfocus.kind(u), U.Solid.off(u), U.Lfocus.of(u))))}, D.At{"data-lift", U32.show(LY.Lyr.top(U.Lfocus.of(u)))},
        D.At{"data-fade", Nat.show(U.Hs.fade())}, D.At{"data-fam", HS.Hist.key.family(U.Solid.want(u))}], Nil{}} <> (View.hist.menu(lc, u) <> View.rc.loc(lc, u))}
   <> View.hist(lc, U.Hs.bar.loc(lc, u), u)

# over a part in 3D: the thread's parts and Renders; the board: its name
# after the parts: Renders for one of them, or the name of another model
# an agent showed (its path on hover)
def View.solid.more.loc(+lc: I.Loc, listed: Bool, +u: U.Ui) -> List<&2, D.Node>:
  match listed:
    case True{}:
      [D.Dom.button("", "mech-renders", U.Ui.view_get(u, "view.path"), MSG.T.mech.renders.tip(lc), MSG.T.viewer.mech.renders(lc))]
    case False{}:
      [D.El{"span", "", [D.At{"class", "what on"}, D.At{"title", U.Ui.view_get(u, "view.path")}], [D.Txt{MC.Mech.last(U.Ui.view_get(u, "view.path"))}]}]

def View.solid.more(listed: Bool, +u: U.Ui) -> List<&2, D.Node>:
  View.solid.more.loc(I.En{}, listed, u)

def View.solid.what.loc(+lc: I.Loc, on: Bool, +u: U.Ui) -> List<&2, D.Node>:
  match on:
    case True{}:
      List.append(&2, D.Node, View.mech.chips(U.Mpage.parts(u), U.Mpage.shown(u)), View.solid.more.loc(lc, U.Mpage.listed(u), u))
    case False{}:
      View.boards(u, MSG.T.viewer.board3d.tip(lc))

# a model turned in 3D: host.js (solid.js) draws the hub's model (a "plot"
# with a mesh, as the phones get) on the canvas whose data-key it carries,
# and says in the note why there is none

def View.solid.what(on: Bool, +u: U.Ui) -> List<&2, D.Node>:
  View.solid.what.loc(I.En{}, on, u)

def View.solid.loc(+lc: I.Loc, +u: U.Ui) -> List<&2, D.Node>:
  View.bar.loc(lc, View.solid.what.loc(lc, U.Mpage.onpart(u), u), MSG.T.viewer.orbit.tip(lc))
    <> List.append(&2, D.Node, D.Dom.when(Bool.not(String.is_empty(U.Ui.info(u, "plotnote:" ++ U.Solid.want(u)))), D.Dom.span("viewer-say", U.Ui.info(u, "plotnote:" ++ U.Solid.want(u)))),
      [D.El{"div", "", [D.At{"class", "viewer-stage"}], [D.El{"canvas", "solid", [D.At{"id", "solid"}, D.At{"data-key", U.Solid.want(u)}], Nil{}},
        D.El{"div", "", [D.At{"class", "viewer-note"}], Nil{}}, D.El{"div", "", [D.At{"class", "viewer-stat"}], Nil{}}]}])

# The viewer pane: a tab for each thing it shows (the one on show lit),
# Close, then that thing

def View.solid(+u: U.Ui) -> List<&2, D.Node>:
  View.solid.loc(I.En{}, u)

def View.tab(+cur: String, +kind: String, +action: String, +value: String, +title: String, +label: String) -> D.Node:
  D.Dom.button(Bool.pick(String, String.eq(cur, kind), "tab on", "tab"), action, value, title, label)

def View.tabs.loc(+lc: I.Loc, +cur: String) -> D.Node:
  D.El{"nav", "", [D.At{"class", "viewer-tabs"}], [
    View.tab(cur, "board", "view-kind", "board", MSG.T.viewer.board.tip(lc), MSG.T.viewer.board(lc)),
    View.tab(cur, "schematic", "view-kind", "schematic", MSG.T.viewer.schematic.tip(lc), MSG.T.viewer.schematic(lc)),
    View.tab(cur, "3d", "view-kind", "3d", MSG.T.viewer.parts3d.tip(lc), "3D"),
    View.tab(cur, "mech", "mech", "", MSG.T.viewer.mech.tip(lc), MSG.T.viewer.mech(lc)),
    View.tab(cur, "files", "fm", "", MSG.T.viewer.files.tip(lc), MSG.T.viewer.files(lc)),
    View.tab(cur, "diff", "diff", "", MSG.T.viewer.diff.tip(lc), MSG.T.viewer.diff(lc)),
    View.tab(cur, "review", "rv-page", "bom", MSG.T.viewer.review.tip(lc), MSG.T.viewer.review(lc)),
    View.tab(cur, "browser", "view-kind", "browser", MSG.T.viewer.browser.tip(lc), MSG.T.bots.tab.browser(lc)),
    D.El{"span", "", [D.At{"class", "grow"}], Nil{}},
    WI.WIcon.button("icon", "panel", "", MSG.T.viewer.close(lc), "x", 16, "")]}

# The Files tab (client.bend's Fm): where the folder is, then its folders
# and files; a file the viewer can open is a button, any other is listed

def View.tabs(+cur: String) -> D.Node:
  View.tabs.loc(I.En{}, cur)

def View.fm.trail.loc(+lc: I.Loc, ps: List<&2, U.PItem>) -> List<&2, D.Node>:
  match ps:
    case Nil{}:
      Nil{}
    case U.PItem{+l, +a, +v} <> t:
      D.Dom.button("crumb", a, v, MSG.T.proj.goto(lc, l), l) <> View.fm.trail.loc(lc, t)

def View.fm.trail(ps: List<&2, U.PItem>) -> List<&2, D.Node>:
  View.fm.trail.loc(I.En{}, ps)

def View.fm.row.loc(+lc: I.Loc, r: U.FmRow) -> D.Node:
  match r:
    case U.FmRow{+l, +a, +v, +d, +k}:
      D.El{"button", "", List.append(&2, D.Attr, [D.At{"class", "fm-row " ++ Bool.pick(String, String.is_empty(a), "off", k)}, D.At{"title", Bool.pick(String, String.is_empty(a), MSG.T.files.preview.none(lc, l), l)}],
          Bool.pick(List<&2, D.Attr>, String.is_empty(a), [D.At{"disabled", ""}], [D.At{"data-value", v}, D.On{"click", a}])),
        [WI.WIcon.svg(U.Fm.icon(k), 16), D.Dom.span("fm-name", l), D.Dom.span("fm-size", d)]}

def View.fm.row(r: U.FmRow) -> D.Node:
  View.fm.row.loc(I.En{}, r)

def View.fm.rows.loc(+lc: I.Loc, rs: List<&2, U.FmRow>) -> List<&2, D.Node>:
  match rs:
    case Nil{}:
      Nil{}
    case r <> t:
      View.fm.row.loc(lc, r) <> View.fm.rows.loc(lc, t)

def View.fm.rows(rs: List<&2, U.FmRow>) -> List<&2, D.Node>:
  View.fm.rows.loc(I.En{}, rs)

def View.fm.body.loc(+lc: I.Loc, ready: Bool, +u: U.Ui) -> List<&2, D.Node>:
  match ready:
    case True{}:
      Bool.pick(List<&2, D.Node>, List.is_empty(&2, U.FmRow, U.Fm.rows(u)), [D.Dom.span("fm-note", MSG.T.viewer.files.empty(lc))], View.fm.rows.loc(lc, U.Fm.rows(u)))
    case False{}:
      [D.Dom.span("fm-note", MSG.T.viewer.files.loading(lc))]

def View.fm.body(ready: Bool, +u: U.Ui) -> List<&2, D.Node>:
  View.fm.body.loc(I.En{}, ready, u)

def View.files(+lc: I.Loc, +u: U.Ui) -> List<&2, D.Node>:
  [D.El{"div", "", [D.At{"class", "fm-bar"}], List.append(&2, D.Node, View.fm.trail.loc(lc, U.Fm.trail.loc(lc, u)),
     [D.El{"span", "", [D.At{"class", "grow"}], Nil{}}, D.Dom.button("", "fm-up", "", MSG.T.files.up.tip(lc), MSG.T.queue.up(lc))])},
   D.El{"div", "files-list", [D.At{"class", "fm-list"}], View.fm.body.loc(lc, U.Fm.ready(u), u)}]

# an image or a PDF opened from the Files tab: its name, back to the folder
def View.fm.head.loc(+lc: I.Loc, +u: U.Ui, more: List<&2, D.Node>) -> D.Node:
  D.El{"div", "", [D.At{"class", "fm-bar"}], List.append(&2, D.Node, [D.Dom.button("", "fm", "", MSG.T.files.back.tip(lc), MSG.T.viewer.files.back(lc)), D.Dom.span("fm-file", U.Fm.name(u)),
    D.El{"span", "", [D.At{"class", "grow"}], Nil{}}], more)}

def View.fm.head(+u: U.Ui, more: List<&2, D.Node>) -> D.Node:
  View.fm.head.loc(I.En{}, u, more)

def View.image.loc(+lc: I.Loc, +u: U.Ui) -> List<&2, D.Node>:
  [View.fm.head.loc(lc, u, [D.El{"a", "", [D.At{"class", "btn"}, D.At{"href", IM.Img.url(U.Ui.view_get(u, "view.path"))}, D.At{"target", "_blank"}], [D.Txt{MSG.T.menu.file.open(lc)}]}]),
   D.El{"div", "", [D.At{"class", "fm-image"}], [D.El{"a", "", [D.At{"href", IM.Img.url(U.Ui.view_get(u, "view.path"))}, D.At{"data-lightbox", IM.Img.url(U.Ui.view_get(u, "view.path"))}],
     [D.El{"img", "", [D.At{"src", IM.Img.url(U.Ui.view_get(u, "view.path"))}, D.At{"alt", U.Fm.name(u)}], Nil{}}]}]}]

def View.image(+u: U.Ui) -> List<&2, D.Node>:
  View.image.loc(I.En{}, u)

def View.pdf.loc(+lc: I.Loc, +u: U.Ui) -> List<&2, D.Node>:
  [View.fm.head.loc(lc, u, [D.El{"a", "", [D.At{"class", "btn"}, D.At{"href", IM.Img.url(U.Ui.view_get(u, "view.path"))}, D.At{"target", "_blank"}], [D.Txt{MSG.T.files.tab(lc)}]}]),
   D.El{"iframe", "", [D.At{"class", "fm-pdf"}, D.At{"src", IM.Img.url(U.Ui.view_get(u, "view.path"))}, D.At{"title", U.Fm.name(u)}], Nil{}}]

# The Review tab (client.bend's Rp): a button for each page, the title,
# Refresh, then the report's Markdown (or why there is none)

def View.pdf(+u: U.Ui) -> List<&2, D.Node>:
  View.pdf.loc(I.En{}, u)

def View.review.tabs(ts: List<&2, U.RpTab>) -> List<&2, D.Node>:
  match ts:
    case Nil{}:
      Nil{}
    case U.RpTab{+p, +l, +on} <> t:
      D.Dom.button(Bool.pick(String, on, "crumb on", "crumb"), "rv-page", p, l, l) <> View.review.tabs(t)

def View.review.body(+lc: I.Loc, has: Bool, +v: U.RpView, +extra: List<&2, D.Node>) -> D.Node:
  match has:
    case True{}:
      D.El{"div", "review-body", [D.At{"class", "review-body body"}], List.append(&2, D.Node, extra, D.Md.render.loc(lc, U.RpView.md(v)))}
    case False{}:
      D.El{"div", "review-body", [D.At{"class", "review-body"}], List.append(&2, D.Node, extra, [D.Dom.span("fm-note", Bool.pick(String, String.eq(U.RpView.page(v), "release"), Bool.pick(String, U.RpView.busy(v), "", Bool.pick(String, String.is_empty(U.RpView.err(v)), U.RpView.note.loc(lc, v), "")), U.RpView.note.loc(lc, v)))])}

def View.review.v(+lc: I.Loc, +v: U.RpView, +extra: List<&2, D.Node>) -> List<&2, D.Node>:
  [D.El{"div", "", [D.At{"class", "fm-bar review-pane"}], List.append(&2, D.Node, View.review.tabs(U.RpView.tabs(v)),
     [D.Dom.span("fm-file", U.RpView.title(v) ++ Bool.pick(String, U.RpView.busy(v), " · reading…", "")), D.El{"span", "", [D.At{"class", "grow"}], Nil{}},
      D.Dom.button("", "rv-run", "", MSG.T.review.refresh.tip(lc), MSG.T.common.refresh(lc))])},
   View.review.body(lc, Bool.not(String.is_empty(U.RpView.md(v))), v, extra)]

# The Release page (client.bend's Rl): the tag field (Enter makes the
# release; empty means the proposed tag), two toggles, the button, and why
# not (red) above the report
def View.release.toggle(+label: String, +on: Bool, +act: String) -> D.Node:
  D.El{"button", act, [D.At{"class", "rc-task" ++ Bool.pick(String, on, " done", "")}, D.At{"data-value", ""}, D.At{"aria-pressed", Bool.pick(String, on, "true", "false")}, D.On{"click", act}],
    [D.El{"span", "", [D.At{"class", "rc-box"}], [D.Txt{Bool.pick(String, on, "✓", "")}]}, D.Txt{label}]}

def View.release.k.loc(+lc: I.Loc, f: U.RlForm, +v: U.RpView, +u: U.Ui) -> List<&2, D.Node>:
  match f:
    case U.RlForm{+tag, +next, +publish, +force}:
      [D.El{"div", "", [D.At{"class", "rc-checks rl-form"}],
         [WB.WB.input("rltag", "rltag", U.Ui.scratch(u, "@f.rltag"), next, "rv-release"),
          View.release.toggle(MSG.T.review.publish(lc), publish, "rv-release-pub"),
          View.release.toggle(MSG.T.review.force(lc), force, "rv-release-force"),
          D.El{"button", "rv-release", List.append(&2, D.Attr, [D.At{"class", "primary"}, D.At{"data-value", ""}, D.On{"click", "rv-release"}], Bool.pick(List<&2, D.Attr>, U.RpView.busy(v), [D.At{"disabled", "true"}], Nil{})),
            [D.Txt{Bool.pick(String, U.RpView.busy(v), MSG.T.review.release.making(lc, tag), MSG.T.review.release.make(lc, tag))}]}]},
       D.El{"div", "", [D.At{"class", "rc-gate bad"}], Bool.pick(List<&2, D.Node>, String.is_empty(U.RpView.err(v)), Nil{}, [D.Txt{U.RpView.err(v)}])}]

def View.release.k(f: U.RlForm, +v: U.RpView, +u: U.Ui) -> List<&2, D.Node>:
  View.release.k.loc(I.En{}, f, v, u)

def View.release(+lc: I.Loc, +u: U.Ui) -> List<&2, D.Node>:
  View.release.k.loc(lc, U.Rl.form(u), U.Rp.view.loc(lc, u), u)

def View.review.x(+lc: I.Loc, rel: Bool, +u: U.Ui) -> List<&2, D.Node>:
  match rel:
    case True{}:
      View.release(lc, u)
    case False{}:
      View.rc.checks.loc(lc, u)

def View.review(+lc: I.Loc, +u: U.Ui) -> List<&2, D.Node>:
  View.review.v(lc, U.Rp.view.loc(lc, u), View.review.x(lc, String.eq(U.Rp.page(u), "release"), u))

def View.aside.k(+lc: I.Loc, i: Nat, +u: U.Ui) -> List<&2, D.Node>:
  match i:
    case 0n:
      View.diff.loc(lc, U.Ui.view_get(u, "diff"))
    case 1n:
      View.browser.loc(lc, u)
    case 2n:
      View.mech(lc, u)
    case 3n:
      View.solid.loc(lc, u)
    case 4n:
      View.plot(lc, u, List.append(&2, D.Node, Bool.pick(List<&2, D.Node>, U.Bd.many(u), View.board.chips(U.Bd.files(u), U.Bd.shown(u, U.Ui.view_get(u, "view.path"))), Nil{}), List.append(&2, D.Node, View.sheets.loc(lc, u), [View.kicad.loc(lc)])))

    case 6n:
      View.files(lc, u)
    case 7n:
      View.image.loc(lc, u)
    case 8n:
      View.pdf.loc(lc, u)
    case 9n:
      View.review(lc, u)
    case _:
      View.plot(lc, u, List.append(&2, D.Node, View.boards(u, MSG.T.viewer.board.tip(lc)), [View.kicad.loc(lc)]))

def View.aside.n(+kind: String) -> Nat:
  Bool.pick(Nat, String.eq(kind, "diff"), 0n, Bool.pick(Nat, String.eq(kind, "browser"), 1n, Bool.pick(Nat, String.eq(kind, "mech"), 2n,
    Bool.pick(Nat, String.eq(kind, "3d"), 3n, Bool.pick(Nat, String.eq(kind, "schematic"), 4n,
      Bool.pick(Nat, String.eq(kind, "files"), 6n, Bool.pick(Nat, String.eq(kind, "image"), 7n, Bool.pick(Nat, String.eq(kind, "pdf"), 8n, Bool.pick(Nat, String.eq(kind, "review"), 9n, 5n)))))))))

# the side pane of the selected thread: the tabs, then what they show. Its
# left edge is a handle that resizes it (host.js keeps the width per
# browser; a double-click gives the pane its share back)
def View.split.loc(+lc: I.Loc) -> D.Node:
  D.El{"div", "split", [D.At{"class", "viewer-split"}, D.At{"title", MSG.T.viewer.resize(lc)}], Nil{}}

def View.split() -> D.Node:
  View.split.loc(I.En{})

def View.aside(+lc: I.Loc, +kind: String, +u: U.Ui) -> D.Node:
  D.El{"aside", "viewer", [D.At{"class", "viewer " ++ Bool.pick(String, String.is_empty(kind), "board", kind)}, D.At{"id", "viewer"}],
    View.split.loc(lc) <> View.tabs.loc(lc, Bool.pick(String, U.Mpage.onpart(u), "mech", U.Fm.tab(U.Solid.kind(u)))) <> View.aside.k(lc, View.aside.n(kind), u)}

def View.aside.if(+lc: I.Loc, open: Bool, +kind: String, +u: U.Ui) -> List<&2, D.Node>:
  match open:
    case True{}:
      [View.aside(lc, kind, u)]
    case False{}:
      Nil{}

# The first-run tour (core/onboard.bend): a dimmed page and a dialog with
# the step, Skip, Back and Next. The page's class names what the step
# points at ("tour-sidebar", "tour-add", "tour-head", "tour-settings"; the
# head only with a thread open), and style.css lifts that part of the page
# over the dimming. host.js makes it modal: the page behind is inert, focus
# stays in the dialog, and the keys go through app.bend's tour_key (Enter
# and the right arrow next, the left arrow back, Esc skip).
# the uberbot step's choices (client.bend's Tour.choices): one button each
def View.tour.choices(cs: List<&2, U.TChoice>) -> List<&2, D.Node>:
  match cs:
    case Nil{}:
      Nil{}
    case U.TChoice{+l, a, v} <> t:
      D.Dom.button("", a, v, l, l) <> View.tour.choices(t)

def View.tour.back.loc(+lc: I.Loc, on: Bool) -> D.Node:
  match on:
    case True{}:
      D.Dom.button("", "ob-back", "", MSG.T.tour.back.tip(lc), MSG.T.tour.back(lc))
    case False{}:
      D.Dom.span("gap", "")

def View.tour.back(on: Bool) -> D.Node:
  View.tour.back.loc(I.En{}, on)

def View.tour.card(+lc: I.Loc, +i: Nat, +body: String, cs: List<&2, U.TChoice>, +counter: String, back: Bool) -> List<&2, D.Node>:
  [D.El{"div", "", [D.At{"class", "tour-back"}], Nil{}},
    D.El{"div", "tour", [D.At{"class", "tour"}, D.At{"role", "dialog"}, D.At{"aria-modal", "true"}, D.At{"aria-labelledby", "tour-title"}], [
      D.Dom.span("count", counter),
      D.El{"div", "tour-title", [D.At{"class", "tour-title"}, D.At{"id", "tour-title"}], [D.Txt{OB.Ob.title(lc, i)}]},
      D.El{"p", "", [D.At{"class", "tour-body"}], [D.Txt{body}]},
      D.El{"div", "", [D.At{"class", "tour-choices"}], View.tour.choices(cs)},
      D.El{"div", "", [D.At{"class", "tour-btns"}], [
        D.Dom.button("", "ob-skip", "", MSG.T.tour.end.tip(lc), MSG.T.tour.skip(lc)),
        D.Dom.span("gap", ""),
        View.tour.back.loc(lc, back),
        D.Dom.button("primary", "ob-next", "", MSG.T.tour.next.tip(lc), OB.Ob.forward(lc, i))]}]}]

def View.tour.k(+lc: I.Loc, on: Bool, +i: Nat, +u: U.Ui) -> List<&2, D.Node>:
  match on:
    case True{}:
      View.tour.card(lc, i, U.Tour.body.loc(lc, 1n, u), U.Tour.choices(u), U.Tour.counter.loc(lc, u), U.Tour.back(u))
    case False{}:
      Nil{}

def View.tour(+lc: I.Loc, +u: U.Ui) -> List<&2, D.Node>:
  View.tour.k(lc, U.Tour.on(u), U.Tour.at(u), u)

def View.tour.cls.k(on: Bool, +g: String) -> String:
  match on:
    case True{}:
      Bool.pick(String, String.is_empty(g), " tour-on", " tour-on tour-" ++ g)
    case False{}:
      ""

def View.tour.cls(+u: U.Ui) -> String:
  View.tour.cls.k(U.Tour.on(u), U.Tour.target(1n, u))

# the whole page
def View.page.b(+lc: I.Loc, +bv: BI.View, u: U.Ui) -> D.Node:
  match u:
    case U.Ui{+st, +sel, +d, +l, +o, +on, +now, +ad, +pa, +v, +r, +w, +e, +note, +nf, +ob, +tr}:
      D.El{"div", "app", [D.At{"class", Bool.pick(String, pa, "app panel", "app") ++ Bool.pick(String, Bool.or(Bool.or(Bool.not(String.is_empty(sel)), U.Flag.get(o, "settings")), WB.WB.shown(bv)), " picked", "")
        ++ Bool.pick(String, String.eq(M.Setting.get(st, "ui.theme", ""), "light"), " light", "") ++ View.tour.cls(U.Ui{st, sel, d, l, o, on, now, ad, pa, v, r, w, e, note, nf, ob, tr})}],
        D.Dom.cat([View.sidebar(lc, U.Ui{st, sel, d, l, o, on, now, ad, pa, v, r, w, e, note, nf, ob, tr}),
          View.main.b(lc, U.Flag.get(o, "settings"), bv, U.Ui{st, sel, d, l, o, on, now, ad, pa, v, r, w, e, note, nf, ob, tr}, sel, List.length(&2, M.Project, U.Ui.projects(st)))],
          D.Dom.cat(View.aside.if(lc, pa, U.Str.get(nf, "view.kind:" ++ sel), U.Ui{st, sel, d, l, o, on, now, ad, pa, v, r, w, e, note, nf, ob, tr}),
            D.Dom.cat(View.picker(lc, U.Flag.get(o, "proj"), U.Ui{st, sel, d, l, o, on, now, ad, pa, v, r, w, e, note, nf, ob, tr}), D.Dom.cat(View.error(e), View.tour(lc, U.Ui{st, sel, d, l, o, on, now, ad, pa, v, r, w, e, note, nf, ob, tr})))))}

# the page in the device's language, looked up once here and passed down
def View.page(+u: U.Ui) -> D.Node:
  View.page.b(U.Lang.loc(u), BI.BU.view(u), u)
