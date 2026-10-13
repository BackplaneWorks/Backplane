import Base
import ../core/model.bend as M
import ./dom.bend as D
import ./icon.bend as WI
import ../core/tools.bend as TO
import ../core/client.bend as U
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

# Ui -> Node. Every decision (order, shelf, status) comes from core defs
# with laws behind them; this file only lays them out.

# Sidebar
# -------

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

def View.row.k(+id: String, +title: String, +ago: String, +badge: String, +dot: String, +sel: String, +movable: Bool) -> D.Node:
  D.El{"div", id, [D.At{"class", Bool.pick(String, String.eq(id, sel), "thread on", "thread")}, D.At{"data-value", id}, D.On{"click", "select"}, D.On{"contextmenu", "row-menu"}],
    D.Dom.cat([D.Dom.span("dot " ++ dot, ""), D.Dom.span("title", title)],
      D.Dom.cat(Bool.pick(List<&2, D.Node>, movable, [D.El{"span", "", [D.At{"class", "moves"}], [D.Dom.button("move", "move", id ++ "|up", "Move up", "↑"), D.Dom.button("move", "move", id ++ "|down", "Move down", "↓")]}], Nil{}),
        [View.note(badge, ago)]))}

def View.row(+t: M.Thread, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat, +movable: Bool, +snoozed: Bool) -> D.Node:
  match t:
    case M.Thread{+id, _, +title, _, +a, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      View.row.k(id, U.Label.title(st, id, title), Bool.pick(String, snoozed, U.Time.ago(U.Side.until(st, id), now), U.Time.ago(now, a)), SB.Subs.badge(SB.Subs.count.ix(ix, st, id)), M.Phase.name(U.Side.phase.ix(ix, st, t)), sel, movable)

def View.rows_in(ts: List<&2, M.Thread>, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat, +movable: Bool, +snoozed: Bool) -> List<&2, D.Node>:
  match ts:
    case Nil{}:
      Nil{}
    case +t <> rest:
      View.row(t, ix, st, sel, now, movable, snoozed) <> View.rows_in(rest, ix, st, sel, now, movable, snoozed)

def View.rows_of(ts: List<&2, M.Thread>, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat) -> List<&2, D.Node>:
  View.rows_in(ts, ix, st, sel, now, False{}, False{})

def View.shelf.k(open: Bool, +key: String, +label: String, ts: List<&2, M.Thread>, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat) -> List<&2, D.Node>:
  match open:
    case True{}:
      WI.WIcon.button("shelf open", "toggle-settled", key, "Hide", "chevron-down", 14, label) <> View.rows_of(ts, ix, st, sel, now)
    case False{}:
      [WI.WIcon.button("shelf", "toggle-settled", key, "Show", "chevron-right", 14, label)]

# a collapsible shelf (Settled, Archived): its label and count, then its
# rows when open; nothing when empty
def View.shelf(open: Bool, +key: String, +name: String, ts: List<&2, M.Thread>, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat) -> List<&2, D.Node>:
  match ts:
    case Nil{}:
      Nil{}
    case +t <> +rest:
      View.shelf.k(open, key, name ++ " " ++ Nat.show(List.length(&2, M.Thread, t <> rest)), t <> rest, ix, st, sel, now)

# snoozed threads, soonest to wake first
def View.snoozed(ts: List<&2, M.Thread>, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat) -> List<&2, D.Node>:
  match ts:
    case Nil{}:
      Nil{}
    case +t <> +rest:
      D.Dom.span("shelf-label", "Snoozed") <> View.rows_in(t <> rest, ix, st, sel, now, False{}, True{})

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
def View.look(open: Bool, +st: M.State, +id: String, +rm: Bool) -> List<&2, D.Node>:
  match open:
    case True{}:
      [D.El{"div", "", [D.At{"class", "shade"}, D.At{"data-value", ""}, D.On{"click", "look"}], Nil{}},
       D.El{"div", "", [D.At{"class", "look"}], [
         D.El{"div", "", [D.At{"class", "swatches"}], View.swatches(BG.Badge.colors(), BG.Badge.color(st, id), id)},
         D.El{"div", "", [D.At{"class", "glyphs"}], View.glyphs(BG.Badge.icons(), BG.Badge.icon(st, id), id)},
         D.Dom.button("remove", "proj-remove", id, "Remove from the sidebar (adding its folder again brings it back)", Bool.pick(String, rm, "Click again to remove", "Remove project"))]}]
    case False{}:
      Nil{}

def View.project(r: U.SRow, +quiet: Bool, +open: Map<&2, Bool>, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat, +look: String, +rm: String) -> D.Node:
  match r:
    case U.SRow{_, M.Project{+id, title, root, _, _}, M.Shelves{pinned, active, settled, hidden, snoozed}}:
      D.El{"section", id, [D.At{"class", Bool.pick(String, Bool.or(quiet, U.Label.syncing(st, id)), Bool.pick(String, U.Label.syncing(st, id), "project quiet syncing", "project quiet"), "project")}, D.At{"style", "--tint:" ++ BG.Badge.css_of(st, id)}, D.At{"data-drag-to", id}, D.At{"data-drag-act", "reorder-project"}],
        List.concat(&2, D.Node, [[D.El{"div", "", [D.At{"class", "project-head"}, D.At{"title", root}, D.At{"draggable", "true"}, D.At{"data-drag", id}, D.At{"data-drag-act", "reorder-project"}, D.At{"data-value", id}, D.On{"contextmenu", "proj-menu"}],
            [WI.WIcon.button("badge", "look", id, "Colour and icon", BG.Badge.icon(st, id), 14, ""), D.Dom.span("name", U.Label.title(st, id, title)), WI.WIcon.button("icon", "new-thread", id, "New thread", "plus", 16, "")]}],
          View.look(String.eq(look, id), st, id, String.eq(rm, id)),
          View.rows_in(pinned, ix, st, sel, now, True{}, False{}),
          View.rows_in(active, ix, st, sel, now, True{}, False{}),
          View.snoozed(snoozed, ix, st, sel, now),
          View.shelf(U.Flag.get(open, id), id, "Settled", settled, ix, st, sel, now),
          View.shelf(U.Flag.get(open, "arch:" ++ id), "arch:" ++ id, "Archived", M.Archive.of(hidden), ix, st, sel, now)])}

def View.projects(rs: List<&2, U.SRow>, +quiet: Bool, +open: Map<&2, Bool>, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat, +look: String, +rm: String) -> List<&2, D.Node>:
  match rs:
    case Nil{}:
      Nil{}
    case r <> t:
      View.project(r, quiet, open, ix, st, sel, now, look, rm) <> View.projects(t, quiet, open, ix, st, sel, now, look, rm)

def View.older.open(on: Bool, rs: List<&2, U.SRow>, +open: Map<&2, Bool>, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat, +look: String, +rm: String) -> List<&2, D.Node>:
  match on:
    case True{}:
      View.projects(rs, True{}, open, ix, st, sel, now, look, rm)
    case False{}:
      Nil{}

# the older projects (quiet for Quiet.days) folded under one row; a search
# shows them in place
def View.older.k(searching: Bool, +n: Nat, rs: List<&2, U.SRow>, +open: Map<&2, Bool>, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat, +look: String, +rm: String) -> List<&2, D.Node>:
  match searching:
    case True{}:
      View.projects(rs, True{}, open, ix, st, sel, now, look, rm)
    case False{}:
      WI.WIcon.button(Bool.pick(String, U.Flag.get(open, "old-projects"), "shelf older open", "shelf older"), "side-older", "", "Projects quiet for three days",
          Bool.pick(String, U.Flag.get(open, "old-projects"), "chevron-down", "chevron-right"), 14, "Older " ++ Nat.show(n))
        <> View.older.open(U.Flag.get(open, "old-projects"), rs, open, ix, st, sel, now, look, rm)

def View.older(rs: List<&2, U.SRow>, +searching: Bool, +open: Map<&2, Bool>, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat, +look: String, +rm: String) -> List<&2, D.Node>:
  match rs:
    case Nil{}:
      Nil{}
    case +r <> +t:
      View.older.k(searching, List.length(&2, U.SRow, r <> t), r <> t, open, ix, st, sel, now, look, rm)

# the projects view: live, quiet (faded), older (folded): core's Side.groups
def View.groups(g: U.SGroups, +q: String, +open: Map<&2, Bool>, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat, +look: String, +rm: String) -> List<&2, D.Node>:
  match g:
    case U.SGroups{l, qs, o}:
      List.concat(&2, D.Node, [View.projects(l, False{}, open, ix, st, sel, now, look, rm), View.projects(qs, True{}, open, ix, st, sel, now, look, rm),
        View.older(o, Bool.not(String.is_empty(q)), open, ix, st, sel, now, look, rm)])

# the active view: every live thread, what waits on you first, then the
# threads settled lately, faded (core's Active.rows); a row names its
# project where the other view shows its age
def View.arow(r: U.ARow, +ix: U.SIx, +st: M.State, +sel: String) -> D.Node:
  match r:
    case U.ARow{_, _, +t, +pid, +pt, +faded}:
      D.El{"div", M.Thread.id(t), [D.At{"class", Bool.pick(String, String.eq(M.Thread.id(t), sel), "thread on", Bool.pick(String, faded, "thread faded", "thread"))},
          D.At{"style", "--tint:" ++ BG.Badge.css_of(st, pid)}, D.At{"data-value", M.Thread.id(t)}, D.On{"click", "select"}, D.On{"contextmenu", "row-menu"}],
        [D.Dom.span("dot " ++ M.Phase.name(U.Side.phase.ix(ix, st, t)), ""), D.Dom.span("title", U.Label.title(st, M.Thread.id(t), U.Active.title(t))), View.note(SB.Subs.badge(SB.Subs.count.ix(ix, st, M.Thread.id(t))), pt)]}

def View.arows(rs: List<&2, U.ARow>, +ix: U.SIx, +st: M.State, +sel: String) -> List<&2, D.Node>:
  match rs:
    case Nil{}:
      Nil{}
    case r <> t:
      View.arow(r, ix, st, sel) <> View.arows(t, ix, st, sel)

def View.afaded.open(on: Bool, rs: List<&2, U.ARow>, +ix: U.SIx, +st: M.State, +sel: String) -> List<&2, D.Node>:
  match on:
    case True{}:
      View.arows(rs, ix, st, sel)
    case False{}:
      Nil{}

# the settled rows folded under one "Settled N" row, shown while it is
# open or a search is on (core's Active.open)
def View.afaded.k(searching: Bool, +open: Bool, +n: Nat, rs: List<&2, U.ARow>, +ix: U.SIx, +st: M.State, +sel: String) -> List<&2, D.Node>:
  match searching:
    case True{}:
      View.arows(rs, ix, st, sel)
    case False{}:
      WI.WIcon.button(Bool.pick(String, open, "shelf older open", "shelf older"), "side-settled", "", "Threads settled in the last three days",
          Bool.pick(String, open, "chevron-down", "chevron-right"), 14, "Settled " ++ Nat.show(n))
        <> View.afaded.open(open, rs, ix, st, sel)

def View.afaded(rs: List<&2, U.ARow>, +q: String, +open: Map<&2, Bool>, +ix: U.SIx, +st: M.State, +sel: String) -> List<&2, D.Node>:
  match rs:
    case Nil{}:
      Nil{}
    case +r <> +t:
      View.afaded.k(Bool.not(String.is_empty(q)), U.Flag.get(open, "active-settled"), List.length(&2, U.ARow, r <> t), r <> t, ix, st, sel)

def View.active.k(p: U.AParts, +q: String, +open: Map<&2, Bool>, +ix: U.SIx, +st: M.State, +sel: String) -> List<&2, D.Node>:
  match p:
    case U.AParts{l, f}:
      [D.El{"section", "", [D.At{"class", "project active"}], List.append(&2, D.Node, View.arows(l, ix, st, sel), View.afaded(f, q, open, ix, st, sel))}]

def View.active(rs: List<&2, U.ARow>, +q: String, +open: Map<&2, Bool>, +ix: U.SIx, +st: M.State, +sel: String) -> List<&2, D.Node>:
  match rs:
    case Nil{}:
      [D.Dom.span("shelf-label", "Nothing active")]
    case +r <> +t:
      View.active.k(U.Active.parts(r <> t), q, open, ix, st, sel)

# the list the "sidebar.view" setting picks (only that one is built)
def View.list.k(active: Bool, +q: String, +open: Map<&2, Bool>, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat, +look: String, +rm: String) -> List<&2, D.Node>:
  match active:
    case True{}:
      View.active(U.Active.rows.ix(st, ix, now, q), q, open, ix, st, sel)
    case False{}:
      View.groups(U.Side.groups(U.Side.shown.ix(st, ix, now, q), now), q, open, ix, st, sel, now, look, rm)

# (what every row asks of the whole state is gathered once: core's Side.ix)
def View.list(active: Bool, +q: String, +open: Map<&2, Bool>, +st: M.State, +sel: String, +now: Nat, +look: String, +rm: String) -> List<&2, D.Node>:
  View.list.k(active, q, open, U.Side.ix(st), st, sel, now, look, rm)

# the search, always at the top of the list
def View.find(+active: Bool, +q: String) -> List<&2, D.Node>:
  [D.El{"div", "", [D.At{"class", "find"}], [WB.WB.input("proj-find", "pq", q, Bool.pick(String, active, "Search threads", "Search projects"), "proj-go")]}]

# a row's or project's menu (core's Menu), where the pointer was (host.js
# keeps it in --mx, --my); the shade under it shuts it
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

def View.add(adding: Bool) -> List<&2, D.Node>:
  match adding:
    case True{}:
      [D.El{"div", "", [D.At{"class", "add"}], [
        D.El{"input", "project-path", [D.At{"id", "project-path"}, D.At{"placeholder", "/path/to/project"}, D.At{"data-enter", "add-project"}, D.At{"data-value", "#project-path"}], Nil{}},
        D.Dom.button("primary", "add-project", "#project-path", "Add project", "Add")]}]
    case False{}:
      Nil{}

# the owner's machines (only with more than one), at the foot of the
# sidebar beside Settings: Backplane is one app on each, and picking
# another opens that machine's page ("flag machines" opens the list)
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

def View.machines(many: Bool, +open: Bool, +rs: List<&2, U.MRow>) -> List<&2, D.Node>:
  match many:
    case True{}:
      List.append(&2, D.Node, View.mlist(open, rs), [WI.WIcon.button("machine-pick", "flag", "machines", "Machine", "chevron-up", 14, U.Machines.current(rs))])
    case False{}:
      Nil{}

def View.sidebar.k(+ms: List<&2, U.MRow>, +st: M.State, +sel: String, +open: Map<&2, Bool>, on: Bool, +now: Nat, adding: Bool, v: String, +look: String, +rm: String, +q: String, mi: List<&2, U.MItem>, bots: List<&2, D.Node>, uber: List<&2, D.Node>) -> D.Node:
  D.El{"aside", "sidebar", [D.At{"class", "sidebar"}],
        D.Dom.cat([D.El{"div", "", [D.At{"class", "brand"}], [
            D.Dom.span(Bool.pick(String, on, "conn on", "conn"), ""),
            D.Dom.span("name", "Backplane"), D.Dom.span("ver", v),
            WI.WIcon.button("icon", "proj-open", "", "Add project", "plus", 16, "")]}],
          D.Dom.cat(uber, D.Dom.cat(View.add(adding), D.Dom.cat(View.find(String.eq(U.Side.view(st), "active"), q),
            D.Dom.cat(View.list(String.eq(U.Side.view(st), "active"), q, open, st, sel, now, look, rm), D.Dom.cat(bots, D.Dom.cat(View.rmenu(mi),
            [D.El{"div", "", [D.At{"class", "foot"}], WI.WIcon.button("settings-link", "flag", "settings", "Settings", "settings", 16, "Settings")
              <> View.machines(Nat.is_gt(List.length(&2, U.MRow, ms), 1n), U.Flag.get(open, "machines"), ms)}])))))))}

def View.sidebar.w(bots: List<&2, D.Node>, uber: List<&2, D.Node>, ms: List<&2, U.MRow>, mi: List<&2, U.MItem>, u: U.Ui) -> D.Node:
  match u:
    case U.Ui{+st, +sel, +d, _, +open, +on, +now, +adding, _, +v, _, _, _, _, _, _, _}:
      View.sidebar.k(ms, st, sel, open, on, now, adding, v, U.Str.get(d, "@look"), U.Str.get(d, "@rm"), U.Find.q(open, d), mi, bots, uber)

def View.sidebar.u(bots: List<&2, D.Node>, +u: U.Ui) -> D.Node:
  View.sidebar.w(bots, WB.WB.uber(u), U.Machines.rows(u), U.Menu.items(u, True{}), u)

def View.sidebar(+u: U.Ui) -> D.Node:
  View.sidebar.u(WB.WB.side(u), u)

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

def View.phase.k(idle: Bool, +label: String, +name: String) -> List<&2, D.Node>:
  match idle:
    case True{}:
      Nil{}
    case False{}:
      [D.El{"div", "live", [D.At{"class", "phase " ++ name}], [D.El{"span", "", [D.At{"class", "dot " ++ name}], Nil{}}, D.Txt{label}]}]

# the reply streaming in, else the thread's phase
def View.live.k(empty: Bool, +ph: M.Phase, text: String) -> List<&2, D.Node>:
  match empty:
    case False{}:
      [D.El{"div", "live", [D.At{"class", "msg assistant live"}], [D.El{"div", "", [D.At{"class", "body"}], D.Md.render(text)}]}]
    case True{}:
      View.phase.k(String.is_empty(M.Phase.label(ph)), M.Phase.label(ph), M.Phase.name(ph))

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
def View.snoozes(picking: Bool) -> List<&2, D.Node>:
  match picking:
    case True{}:
      [D.Dom.button("", "snooze", "1h", "Snooze for an hour", "1 hour"), D.Dom.button("", "snooze", "tomorrow", "Snooze until 9:00 tomorrow", "Tomorrow"),
       D.Dom.button("", "snooze", "1w", "Snooze for a week", "1 week"), D.Dom.button("on", "flag", "snooze", "Cancel", "Cancel")]
    case False{}:
      Nil{}

# the title, or (being renamed: client.bend's Ren) a field for it; a click
# on the title renames
def View.title.k(on: Bool, +id: String, +title: String, +text: String) -> List<&2, D.Node>:
  match on:
    case True{}:
      [WB.WB.input("thread-title", "title", text, "Thread title", "rename-save"), D.Dom.button("primary", "rename-save", "", "Keep this title", "Save"),
       D.Dom.button("", "rename-no", "", "Leave the title as it was", "Cancel")]
    case False{}:
      [D.El{"span", "", [D.At{"class", "title rename"}, D.At{"title", "Click to rename"}, D.At{"data-value", id}, D.On{"click", "row-rename"}], [D.Txt{title}]}]

def View.header(t: M.Thread, +st: M.State, +panel: Bool, snoozed: Bool, picking: Bool, term: Bool, +diff: Bool, del: Bool, +ren: String, +rtext: String) -> D.Node:
  match t:
    case M.Thread{+id, +p, title, _, _, turn, _, archived, override, +sa, +ua, pinned, _, +branch, _, _, model, _, _, _, _}:
      D.El{"header", "", [D.At{"class", "thread-head"}, D.At{"style", "--tint:" ++ BG.Badge.css_of(st, p)}], [
        D.Dom.button("back", "select", "", "Threads", "‹"),
        D.El{"span", "", [D.At{"class", "badge"}], [WI.WIcon.svg(BG.Badge.icon(st, p), 16)]},
        D.El{"div", "", [D.At{"class", "title-box"}], View.title.k(String.eq(ren, id), id, U.Label.title(st, id, title), rtext)},
        D.Dom.span("meta", Bool.pick(String, String.is_empty(branch), "", branch)),
        D.El{"div", "", [D.At{"class", "tools"}], List.append(&2, D.Node,
          View.tools(TO.Tools.head(TO.Head{View.running(turn), TO.Tools.settled(override, Nat.is_gt(sa, ua)), pinned, snoozed, archived, del,
            Bool.not(String.is_empty(M.Fork.source(st, id))), term, panel}, "flag", "snooze", "term-toggle", "@term-size")),
          View.snoozes(picking))}]}

# the model menu's rows: each model's name, then what its CLI calls it,
# in two columns (the names' as wide as the longest, --namew)
def View.models(ms: List<&2, String>, +cur: String, +info: String, +prov: String) -> List<&2, D.Node>:
  match ms:
    case Nil{}:
      Nil{}
    case +m <> t:
      D.El{"button", "", [D.At{"class", Bool.pick(String, String.eq(m, cur), "row on", "row")}, D.At{"title", "Use " ++ Bool.pick(String, String.is_empty(m), "the default", m)}, D.At{"data-value", m}, D.On{"click", "model"}],
        [D.Dom.span("name", Bool.pick(String, String.is_empty(m), "default", m)), D.Dom.span("label", U.Models.label_of(info, prov, m))]} <> View.models(t, cur, info, prov)

def View.namew(ms: List<&2, String>) -> Nat:
  match ms:
    case Nil{}:
      7n
    case +m <> t:
      Nat.max(String.length(m), View.namew(t))

# reasoning effort chips, the provider's default first
def View.efforts(es: List<&2, String>, +cur: String, +prov: String) -> List<&2, D.Node>:
  match es:
    case Nil{}:
      Nil{}
    case +e <> t:
      D.Dom.button(Bool.pick(String, U.Chip.on(e, cur, prov), "on", ""), "effort", e,
        Bool.pick(String, String.starts_with(e, "@"), "Go on with " ++ U.Chip.name(e) ++ " from the next turn", "Reasoning effort: " ++ U.Chip.name(e)), U.Chip.name(e)) <> View.efforts(t, cur, prov)

def View.effort_row(+es: List<&2, String>, +cur: String, +prov: String) -> List<&2, D.Node>:
  Bool.pick(List<&2, D.Node>, List.is_empty(&2, String, es), Nil{}, [D.El{"div", "", [D.At{"class", "efforts"}], View.efforts(es, cur, prov)}])

# the model menu over the composer, and a shade behind it: a click
# anywhere off the menu lands on the shade and closes it
def View.menu(open: Bool, +ms: List<&2, String>, +cur: String, +info: String, +prov: String, +es: List<&2, String>, +effort: String) -> List<&2, D.Node>:
  match open:
    case True{}:
      [D.El{"div", "", [D.At{"class", "shade"}, D.At{"data-value", "models"}, D.On{"click", "flag"}], Nil{}},
       D.El{"div", "", [D.At{"class", "model-menu"}, D.At{"style", "--namew:" ++ Nat.show(View.namew(ms)) ++ "ch"}],
         D.Dom.cat(View.effort_row(es, effort, prov), [D.El{"div", "", [D.At{"class", "rows"}], View.models(ms, cur, info, prov)}])}]
    case False{}:
      Nil{}

# the skills a `$` being typed may complete to, or the bots, threads,
# projects and rooms a `@`, `>`, `%` or `#` may (the arrow keys move the
# highlight, Tab or Enter takes it)
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
def View.sub.stop(+act: String, +key: String) -> List<&2, D.Node>:
  D.Dom.when(Bool.not(String.is_empty(act)), D.Dom.button("sub-stop", act, key, "Stop this work", "Stop"))

def View.sub.k(r: SB.SubRow, +act: String) -> D.Node:
  match r:
    case SB.SubRow{+key, +agent, +title, +who, +word, +doing, +live, +open, _, ss}:
      D.El{"div", key, [D.At{"class", "sub " ++ word ++ Bool.pick(String, live, " live", "")}],
        D.Dom.cat([D.El{"div", "", List.append(&2, D.Attr, [D.At{"class", "sub-row"}], View.sub.act(agent, open, key)),
          [D.Dom.span("state-dot", ""), D.Dom.span("who", Bool.pick(String, agent, Bool.pick(String, open, "▾", "▸"), who)), D.Dom.span("title", title), D.Dom.span("state", word)]}],
          D.Dom.cat(View.sub.stop(act, key), D.Dom.cat(D.Dom.when(Bool.not(String.is_empty(doing)), D.Dom.span("doing", doing)), View.sub.steps(ss))))}

def View.sub(+r: SB.SubRow) -> D.Node:
  View.sub.k(r, SB.Subs.cancel(r))

def View.subs.go(rs: List<&2, SB.SubRow>) -> List<&2, D.Node>:
  match rs:
    case Nil{}:
      Nil{}
    case r <> rest:
      View.sub(r) <> View.subs.go(rest)

# the subagent viewer: what this thread delegated and the agent's own. Its
# head opens and shuts it; done, it folds to the head (subs.bend's panel)
def View.subs.k(+busy: String, +open: Bool, +th: String, +rs: List<&2, SB.SubRow>) -> D.Node:
  D.El{"div", "subs", [D.At{"class", "subs" ++ Bool.pick(String, String.is_empty(busy), "", " busy") ++ Bool.pick(String, open, "", " shut")}],
    WI.WIcon.after("subs-head", "fold", SB.Subs.panel.ask(open, th), Bool.pick(String, open, "Hide the subagents", "Show the subagents"),
      "Subagents · " ++ SB.Subs.panel.word(rs), Bool.pick(String, open, "chevron-down", "chevron-right"), 14)
      <> View.subs.go(SB.Subs.panel.rows(open, rs))}

def View.subs(+info: Map<&2, String>, +th: String, +rs: List<&2, SB.SubRow>) -> List<&2, D.Node>:
  Bool.pick(List<&2, D.Node>, List.is_empty(&2, SB.SubRow, rs), Nil{},
    [View.subs.k(SB.Subs.busy(SB.Subs.live(rs)), SB.Subs.panel.open(info, th, rs), th, rs)])

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

def View.ask.btns(+k: String, +id: String, +o: String, +draft: String) -> List<&2, D.Node>:
  Bool.pick(List<&2, D.Node>, String.eq(k, "plan"), [D.Dom.button("primary", "answer", id ++ "|implement", "Implement the plan", "Implement"), D.Dom.button("", "answer", id ++ "|dismiss", "Dismiss", "Dismiss")],
    Bool.pick(List<&2, D.Node>, String.eq(k, "input"),
      List.append(&2, D.Node, View.ask.opts(String.split(o, '\n'), id),
        Bool.pick(List<&2, D.Node>, String.is_empty(String.trim(draft)), [D.Dom.button("", "answer", id ++ "|", "Dismiss", "Dismiss")], [D.Dom.button("primary", "answer", id ++ "|" ++ String.trim(draft), "Answer with your draft", "Answer with draft")])),
      [D.Dom.button("primary", "answer", id ++ "|accept", "Allow once", "Allow"), D.Dom.button("", "answer", id ++ "|session", "Allow this tool for the session", "Allow for session"), D.Dom.button("", "answer", id ++ "|decline", "Deny", "Deny")]))

def View.ask.std(a: M.Ask, +draft: String) -> D.Node:
  match a:
    case M.Ask{+id, _, +k, +tool, +d, +o, _, _}:
      D.El{"div", id, [D.At{"class", "ask " ++ k}], [
        D.Dom.span("ask-head", Bool.pick(String, String.eq(k, "plan"), "Proposed plan", Bool.pick(String, String.eq(k, "input"), "The agent asks", "Allow " ++ tool ++ "?"))),
        Bool.pick(D.Node, String.eq(k, "plan"), D.El{"div", "", [D.At{"class", "body"}], D.Md.render(d)}, D.El{"div", "", [D.At{"class", "ask-detail"}], [D.Txt{d}]}),
        D.El{"div", "", [D.At{"class", "ask-btns"}], View.ask.btns(k, id, o, draft)}]}

# a browser task paused for help: its question, the screenshot, and the
# person's choices (client.bend's Teach.ask)
def View.ask.web.btns(bs: List<&2, U.TBtn>) -> List<&2, D.Node>:
  match bs:
    case Nil{}:
      Nil{}
    case U.TBtn{+label, +action, +value, +primary} <> t:
      D.Dom.button(Bool.pick(String, primary, "primary", ""), action, value, label, label) <> View.ask.web.btns(t)

def View.ask.web(t: U.TAsk) -> D.Node:
  match t:
    case U.TAsk{+id, _, +slug, +n, _, +q, _, +shot, bs}:
      D.El{"div", id, [D.At{"class", "ask browser"}], D.Dom.cat([
        D.Dom.span("ask-head", "Task paused: " ++ slug ++ " (step " ++ Nat.show(n) ++ ")"),
        D.El{"div", "", [D.At{"class", "ask-detail"}], [D.Txt{q}]}],
        D.Dom.cat(D.Dom.when(Bool.not(String.is_empty(shot)), D.El{"a", "", [D.At{"class", "tk-shot"}, D.At{"href", shot}, D.At{"target", "_blank"}, D.At{"data-lightbox", shot}],
            [D.El{"img", "", [D.At{"src", shot}, D.At{"alt", "where the task stopped"}], Nil{}}]}),
          [D.El{"div", "", [D.At{"class", "ask-btns"}], View.ask.web.btns(bs)}]))}

def View.ask.k(web: Bool, a: M.Ask, +draft: String, nf: Map<&2, String>) -> D.Node:
  match web:
    case True{}:
      View.ask.web(U.Teach.ask.from(a, nf))
    case False{}:
      View.ask.std(a, draft)

def View.ask(a: M.Ask, +draft: String, nf: Map<&2, String>) -> D.Node:
  match a:
    case M.Ask{_, _, +k, _, _, _, _, _}:
      View.ask.k(String.eq(k, "browser"), a, draft, nf)

def View.asks(as: List<&2, M.Ask>, +draft: String, +nf: Map<&2, String>) -> List<&2, D.Node>:
  match as:
    case Nil{}:
      Nil{}
    case a <> t:
      View.ask(a, draft, nf) <> View.asks(t, draft, nf)

def View.qbtns(bs: List<&2, U.QBtn>) -> List<&2, D.Node>:
  match bs:
    case Nil{}:
      Nil{}
    case U.QBtn{+l, a, v} <> t:
      D.Dom.button("", a, v, l, l) <> View.qbtns(t)

def View.qrow(q: M.QItem, +first: Bool, +run: Bool) -> D.Node:
  match q:
    case M.QItem{+msg, text}:
      D.El{"div", "", [D.At{"class", "queued"}], [D.Dom.span("label", U.Qu.tag(run, msg)), D.Dom.span("text", U.Qu.label(msg, text)),
        D.El{"span", "", [D.At{"class", "q-acts"}], View.qbtns(U.Qu.row_buttons(run, first, msg))}]}

def View.qrows(qs: List<&2, M.QItem>, +first: Bool, +run: Bool) -> List<&2, D.Node>:
  match qs:
    case Nil{}:
      Nil{}
    case q <> t:
      View.qrow(q, first, run) <> View.qrows(t, False{}, run)

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

def View.todos.k(+open: Bool, +all: Bool, +th: String, +ts: List<&2, TD.Todo>, +info: Map<&2, String>) -> List<&2, D.Node>:
  WI.WIcon.after("head", "fold", U.Todos.toggle(open, th), Bool.pick(String, open, "Hide the steps", "Show the steps"), TD.Todo.head(ts),
    Bool.pick(String, open, "chevron-down", "chevron-right"), 14)
    <> List.append(&2, D.Node, View.tdlines(U.Todos.rows(info, th, U.Todos.cap.all(), ts)),
      Bool.pick(List<&2, D.Node>, open, View.todo.more(U.Todos.more(all, ts), U.Todos.more.ask(all, th)), Nil{}))

def View.todos(+info: Map<&2, String>, +th: String, +ts: List<&2, TD.Todo>) -> D.Node:
  D.El{"div", "", [D.At{"class", Bool.pick(String, U.Todos.all(info, th), "todos all", "todos")}], Bool.pick(List<&2, D.Node>, List.is_empty(&2, TD.Todo, ts), Nil{},
    View.todos.k(U.Todos.open(info, th), U.Todos.all(info, th), th, ts, info))}

# what waits for a later turn (stored on the server), next first, each
# with its buttons (U.Qu.buttons)
def View.queued(qs: List<&2, M.QItem>, +run: Bool) -> D.Node:
  D.El{"div", "", [D.At{"class", "queue"}], View.qrows(qs, True{}, run)}

# Settings
# --------

def View.chips(xs: List<&2, String>, +cur: String, +key: String) -> List<&2, D.Node>:
  match xs:
    case Nil{}:
      Nil{}
    case +v <> t:
      D.Dom.button(Bool.pick(String, String.eq(v, cur), "on", ""), "setting", key ++ "=" ++ v, key, Bool.pick(String, String.is_empty(v), "default", v)) <> View.chips(t, cur, key)

def View.setrow(+label: String, +note: String, ctl: List<&2, D.Node>) -> D.Node:
  D.El{"div", "", [D.At{"class", "set-row"}], [D.El{"div", "", [D.At{"class", "set-text"}], [D.Dom.span("set-label", label), D.Dom.span("set-note", note)]},
    D.El{"div", "", [D.At{"class", "set-ctl"}], ctl}]}

def View.days(+d: String) -> List<&2, D.Node>:
  +n = {Maybe.default(&2, Nat, Nat.read(d), 3n) : Nat}
  [D.Dom.button("", "setting", "settle.days=" ++ Nat.show(Bool.pick(Nat, Nat.is_gt(n, 1n), Nat.sub(n, 1n), 1n)), "Fewer days", "−"),
   D.Dom.span("days", Nat.show(n) ++ Bool.pick(String, Nat.is_eq(n, 1n), " day", " days")),
   D.Dom.button("", "setting", "settle.days=" ++ Nat.show(Bool.pick(Nat, Nat.is_lt(n, 90n), 1n+n, 90n)), "More days", "+")]

def View.fork(+fork: String) -> List<&2, D.Node>:
  Bool.pick(List<&2, D.Node>, String.is_empty(fork), [D.Dom.button("primary", "kicad-install", "", "Install Backplane KiCad", "Install Backplane KiCad")],
    [D.Dom.span("ok", "Backplane KiCad " ++ fork)])

# Voice: the OpenAI key (only ever shown masked), the microphone, the
# dictionary (× takes a term out), as in the native window
def View.devs(ds: List<&2, U.VDev>, +cur: String) -> List<&2, D.Node>:
  match ds:
    case Nil{}:
      Nil{}
    case U.VDev{+name, +label} <> t:
      D.Dom.button(Bool.pick(String, String.eq(name, cur), "on", ""), "setting", "voice.device=" ++ name, name, label) <> View.devs(t, cur)

def View.terms(ts: List<&2, String>) -> List<&2, D.Node>:
  match ts:
    case Nil{}:
      Nil{}
    case +t <> r:
      D.Dom.button("chip", "voice-unterm", t, "Take it out of the dictionary", t ++ " ×") <> View.terms(r)

def View.voice.set(+u: U.Ui, +st: M.State) -> List<&2, D.Node>:
  [View.setrow("OpenAI API key", Bool.pick(String, String.is_empty(U.Ui.scratch(u, "@voice.key")), "For GPT-Live-Transcribe. Kept in this machine's secrets; never shown again.",
      "Saved: " ++ U.Ui.scratch(u, "@voice.key") ++ ". Paste a new one to replace it."),
     [WB.WB.secret("voice-key", "vkey", U.Ui.scratch(u, "@f.vkey"), "sk-…"), D.Dom.button("primary", "voice-key", "", "Keep this key", "Save"),
      D.Dom.button("", "voice-unkey", "", "Forget the key", "Remove")]),
   View.setrow("Microphone", "The mic button in the composer records the hub machine's " ++
      Bool.pick(String, String.is_empty(M.Setting.get(st, "voice.device", "")), "default microphone, " ++ U.Voice.label(u, U.Voice.default(u)), U.Voice.label(u, M.Setting.get(st, "voice.device", ""))) ++ ".",
     D.Dom.button(Bool.pick(String, String.is_empty(M.Setting.get(st, "voice.device", "")), "on", ""), "setting", "voice.device=", "The default microphone", "Default") <>
       View.devs(U.Voice.devices(u), M.Setting.get(st, "voice.device", ""))),
   View.setrow("Dictionary", "Names and terms to expect. Words you clarify after dictating join it. Pebble ring: set its webhook to send the recording.",
     List.append(&2, D.Node, View.terms(U.Voice.dict(u)), [WB.WB.input("voice-term", "vterm", U.Ui.scratch(u, "@f.vterm"), "Add a term", "voice-term"),
       D.Dom.button("", "voice-term", "", "Add it to the dictionary", "Add")]))]

# API keys (client.bend's ApiKeys): add one, view the names, delete one
def View.keys.rows(ns: List<&2, String>) -> List<&2, D.Node>:
  match ns:
    case Nil{}:
      Nil{}
    case +n <> t:
      View.setrow(n, "", [D.Dom.button("danger", "key-del", n, "Forget this key", "Delete")]) <> View.keys.rows(t)

def View.keys.list(open: Bool, +u: U.Ui) -> List<&2, D.Node>:
  match open:
    case True{}:
      Bool.pick(List<&2, D.Node>, List.is_empty(&2, String, U.ApiKeys.names(u)), [View.setrow("No API keys yet", "", Nil{})], View.keys.rows(U.ApiKeys.names(u)))
    case False{}:
      Nil{}

def View.keys.form(open: Bool, +u: U.Ui) -> List<&2, D.Node>:
  match open:
    case True{}:
      [View.setrow("New API key", U.ApiKeys.form.note(),
        [WB.WB.input("kname", "kname", U.Ui.scratch(u, "@f.kname"), "Name", "key-add"), WB.WB.secret("kval", "kval", U.Ui.scratch(u, "@f.kval"), "Key"),
         D.Dom.button("primary", "key-add", "", "Keep this key", "Save")])]
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

def View.keys.set(+u: U.Ui) -> List<&2, D.Node>:
  List.concat(&2, D.Node, [
    [View.setrow("API keys", U.ApiKeys.note(u),
      [D.Dom.button(Bool.pick(String, U.Ui.flag(u, "keyform"), "on", ""), "flag", "keyform", "Add an API key", "Add API key"),
       D.Dom.button(Bool.pick(String, U.Ui.flag(u, "keys"), "on", ""), "flag", "keys", "The names kept", "View API keys")])],
    View.keys.form(U.Ui.flag(u, "keyform"), u), View.keys.list(U.Ui.flag(u, "keys"), u)])

# the pairing code under its row while "pairqr" is on: the code on white
# with its quiet zone, a click puts it away
def View.pair.qr.svg(+rows: List<&2, String>) -> D.Node:
  +side = {U32.show((U32.from_nat(List.length(&2, String, rows)) + 8 : U32)) : String}
  D.El{"div", "", [D.At{"class", "pair-qr"}, D.At{"data-value", "pairqr"}, D.On{"click", "flag"}], [
    D.El{"svg", "", [D.At{"viewBox", "0 0 " ++ side ++ " " ++ side}, D.At{"shape-rendering", "crispEdges"}],
      [D.El{"path", "", [D.At{"d", QR.Qr.path(rows)}], Nil{}}]},
    D.Dom.span("set-note", U.Pair.qr.note())]}

def View.pair.qr(on: Bool, +nf: Map<&2, String>) -> List<&2, D.Node>:
  match on:
    case True{}:
      [View.pair.qr.svg(U.Pair.qr(nf))]
    case False{}:
      Nil{}

# the same settings as the native window
# the version row's button (U.Update.btn)
def View.update.btn(i: Nat) -> List<&2, D.Node>:
  match i:
    case 1n:
      [D.Dom.button("primary", "update", "", "Install the update", "Install update")]
    case 2n:
      [D.Dom.button("", "update-check", "", "Look for an update now", "Check now")]
    case 3n:
      [D.Dom.button("primary", "update-now", "", "Update without waiting for the threads at work", "Update now")]
    case _:
      Nil{}

# Your machines (U.Mate.rows): each of the owner's other machines, how its
# sync goes, and On/Off ("sync.<name>")
def View.mates.go(rs: List<&2, U.MateRow>) -> List<&2, D.Node>:
  match rs:
    case Nil{}:
      Nil{}
    case U.MateRow{+nm, +note, +on, +key} <> t:
      View.setrow(nm, note, View.chips(["on", "off"], on, key)) <> View.mates.go(t)

def View.mates(+u: U.Ui) -> List<&2, D.Node>:
  View.setrow(U.Mate.title(), U.Mate.lead(u), Nil{}) <> View.mates.go(U.Mate.rows(u))

def View.settings(+u: U.Ui, +st: M.State, +nf: Map<&2, String>, +v: String, +note: String) -> D.Node:
  +prov = M.Setting.get(st, "agent.provider", "claude")
  +ipc = M.Setting.get(st, "kicad.ipc", "on")
  D.El{"main", "settings", [D.At{"class", "settings-view"}], [
    D.El{"header", "", [D.At{"class", "thread-head"}], [D.Dom.span("title", "Settings"), D.Dom.span("meta", ""), D.Dom.button("", "flag", "settings", "Close settings", "Close")]},
    D.El{"div", "", [D.At{"class", "settings"}], List.concat(&2, D.Node, [[
      View.setrow("Settle after", "Threads idle this long move to Settled. They stay saved.", View.days(M.Setting.get(st, "settle.days", "3"))),
      View.setrow("Follow-ups while a turn runs", "Queue waits for the turn to end; steer sends into it. Ctrl+Enter does the other. Default: Claude steers, Codex, Grok and Muse queue.",
        View.chips(["", "queue", "steer"], M.Setting.get(st, "followup", ""), "followup")),
      View.setrow("Continue after a restart", "Turns a restart cuts short go on by themselves.", View.chips(["on", "off"], Bool.pick(String, M.Carry.on(st), "on", "off"), "restart.continue")),
      View.setrow("Provider for new threads", Bool.pick(String, String.is_empty(U.Models.cli(U.Str.get(nf, "models"), prov)), prov ++ " is not installed", U.Models.cli(U.Str.get(nf, "models"), prov)) ++ ". Each thread keeps its own; change it from the composer.", View.chips(["claude", "codex", "grok", "muse", "antigravity"], prov, "agent.provider")),
      View.setrow("Model for new threads", Bool.pick(String, String.is_empty(M.Setting.get(st, "agent.model", "")), "default", M.Setting.get(st, "agent.model", "")) ++ ": " ++ U.Models.label_of(U.Str.get(nf, "models"), prov, M.Setting.get(st, "agent.model", "")), View.chips(U.Models.menu(U.Str.get(nf, "models"), prov), M.Setting.get(st, "agent.model", ""), "agent.model")),
      View.setrow("Writing: provider", "Titles threads, names branches and writes commit messages. A small model keeps this cheap.", View.chips(["claude", "codex", "grok", "muse"], U.TextGen.provider(st), "text.provider")),
      View.setrow("Writing: model", Bool.pick(String, String.is_empty(U.TextGen.model(st)), "default", U.TextGen.model(st)) ++ ": " ++ U.Models.label_of(U.Str.get(nf, "models"), U.TextGen.provider(st), U.TextGen.model(st)),
        View.chips(U.Models.menu(U.Str.get(nf, "models"), U.TextGen.provider(st)), U.TextGen.model(st), "text.model")),
      View.setrow(U.KiCad.cli.label(nf), U.KiCad.cli.note(nf), View.fork(U.Str.get(nf, "kicad.fork"))),
      View.setrow(U.FreeCAD.label(nf), U.FreeCAD.note(nf), Nil{}),
      View.setrow(U.KiCad.ipc.label(nf), U.KiCad.ipc.note(nf), Bool.pick(List<&2, D.Node>, U.KiCad.ipc.ready(nf), View.chips(["on", "off"], ipc, "kicad.ipc"), Nil{})),
      View.setrow("Theme", "The window and the web page (phones unset follow their own system).", View.chips(["dark", "light"], Bool.pick(String, String.eq(M.Setting.get(st, "ui.theme", ""), "light"), "light", "dark"), "ui.theme")),
      View.setrow("Thread list", "Projects groups threads by project; active lists what is going on, what needs you first.", View.chips(["projects", "active"], U.Side.view(st), "sidebar.view")),
      View.setrow(U.Tail.label(nf), U.Tail.note(nf), Nil{})],
      View.mates(u),
      [View.setrow("Pairing link", U.Pair.note(U.Pair.link(nf)), D.Dom.cat(D.Dom.when(U.Pair.qr.ready(nf), D.Dom.button(Bool.pick(String, U.Ui.flag(u, "pairqr"), "on", ""), "flag", "pairqr", "Show the link as a code to scan", "QR code")),
        D.Dom.when(Bool.not(String.is_empty(U.Pair.link(nf))), D.Dom.copy(U.Pair.link(nf)))))],
      View.pair.qr(U.Ui.flag(u, "pairqr"), nf),
      [View.setrow("Update channel", "Stable takes releases; nightly also takes the daily builds.", View.chips(["stable", "nightly"], M.Setting.get(st, "update.channel", "stable"), "update.channel")),
      View.setrow("Backplane " ++ v, U.Update.line.w(st, nf, note), View.update.btn(U.Update.btn(st, nf, note)))], View.voice.set(u, st), View.keys.set(u), View.agy.rows(U.Agv.rows(u)), [View.setrow("Tour", "The short walk through projects, threads, bots, your agents and your machines.", [D.Dom.button("", "ob-open", "", "Show the tour again", "Show the tour")])]])}]}

def View.empty(n: Nat) -> D.Node:
  D.El{"main", "empty", [D.At{"class", "empty"}], [D.Txt{Bool.pick(String, Nat.is_eq(n, 0n), "Add a project with + to start.", "Pick a thread, or start one with + next to a project.")}]}

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
def View.phead(dir: Bool, +u: U.Ui) -> D.Node:
  match dir:
    case True{}:
      D.El{"div", "", [D.At{"class", "dir"}], [D.Dom.button("pback", "proj-back", "", "Back to projects", "Projects"), D.Txt{U.Proj.dir(u)}]}
    case False{}:
      D.El{"div", "", [D.At{"class", "dir"}], [D.Txt{"Projects"}]}

def View.picker(open: Bool, +u: U.Ui) -> List<&2, D.Node>:
  match open:
    case True{}:
      [D.El{"div", "", [D.At{"class", "modal-back"}, D.On{"click", "proj-close"}], Nil{}},
        D.El{"div", "picker", [D.At{"class", "picker"}], D.Dom.cat([
          View.phead(U.Proj.browsing(u), u),
          D.El{"input", "proj", [D.At{"id", "proj"}, D.At{"placeholder", Bool.pick(String, U.Proj.browsing(u), "Filter, name a new folder, or type a path", "Find a project, or type a path")},
            D.At{"autocomplete", "off"}, D.At{"value", U.Proj.query(u)}, D.On{"input", "proj-q"}, D.At{"data-enter", "proj-enter"}], Nil{}}],
          D.Dom.cat(Bool.pick(List<&2, D.Node>, String.is_empty(U.Ui.info(u, "proj.err")), Nil{}, [D.El{"div", "", [D.At{"class", "perr"}], [D.Txt{U.Ui.info(u, "proj.err")}]}]),
            [D.El{"div", "", [D.At{"class", "prows"}], View.prows(u, U.Proj.items(u), 0n, U.Proj.index(u))}]))}]
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
def View.fold.row(+id: String, +label: String, +open: Bool) -> D.Node:
  WI.WIcon.button("fold", "fold", FO.Fold.ask(open, id), Bool.pick(String, open, "Hide ", "Show ") ++ Bool.pick(String, String.starts_with(id, "work:"), "what the agent did", "these tool calls"),
    Bool.pick(String, open, "chevron-down", "chevron-right"), 14, label)

def View.fold.shown(open: Bool, ns: List<&2, D.Node>) -> List<&2, D.Node>:
  match open:
    case True{}:
      ns
    case False{}:
      Nil{}

# an entry's nodes as its role says
def View.role(r: FO.Role, ns: List<&2, D.Node>) -> List<&2, D.Node>:
  match r:
    case FO.RShow{}:
      ns
    case FO.RHead{+id, +label, +open}:
      View.fold.row(id, label, open) <> View.fold.shown(open, ns)
    case FO.RIn{open}:
      View.fold.shown(open, ns)
    case FO.RWork{+id, +label, open, inner}:
      match open:
        case True{}:
          View.fold.row(id, label, True{}) <> View.role(inner, ns)
        case False{}:
          [View.fold.row(id, label, False{})]

# the newest page of a thread's entries (core's Page), with a mark at
# the top when earlier ones are left out: scrolled near it, the page asks
# for them ("earlier", host.js), no button
def View.earlier(+hidden: Nat, +id: String) -> List<&2, D.Node>:
  Bool.pick(List<&2, D.Node>, Nat.is_eq(hidden, 0n), Nil{},
    [D.Dom.el("div", [D.At{"class", "earlier"}, D.At{"data-earlier", id}], Nil{})])

def View.sending(ps: List<&2, String>) -> List<&2, D.Node>:
  match ps:
    case Nil{}:
      Nil{}
    case p <> t:
      D.El{"div", "", [D.At{"class", "msg user sending"}], D.Dom.cat(View.user_body(p), [D.Dom.span("note", "sending…")])} <> View.sending(t)

# Terminal
# --------

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
def View.emb.screen(+v: VT.Vt, +key: String, +cols: U32, +rows: U32) -> D.Node:
  D.El{"div", "", [D.At{"class", "screen emb-screen"}, D.At{"style", "background:" ++ View.hex(VT.Vt.bg()) ++ ";color:" ++ View.hex(VT.Vt.fg()) ++ ";width:calc(" ++ U32.show(cols) ++ "ch + 24px);height:" ++ U32.show((rows * 17 + 8 : U32)) ++ "px"}],
    D.Dom.cat(View.tlines(VT.Vt.lines(v, 0n)), D.Dom.cat(View.cursor(VT.Vt.cursor(v)),
      [D.El{"textarea", "emb:" ++ key, [D.At{"class", "term-in"}, D.At{"data-keys", "emb-key"}, D.At{"data-text", "emb-paste"}, D.At{"data-for", key},
        D.At{"autocapitalize", "off"}, D.At{"autocomplete", "off"}, D.At{"spellcheck", "false"}, D.At{"aria-label", "Terminal input"}], Nil{}}]))}

def View.emb.live(+ec: U.Ec, ev: EM.EV) -> D.Node:
  match ev:
    case EM.EV{+key, +id, +title, +cols, +rows, _, +pinned}:
      D.El{"div", "emb-" ++ id, [D.At{"class", "emb"}, D.At{"data-id", id}], [
        D.El{"div", "", [D.At{"class", "emb-head"}], [WI.WIcon.svg("terminal", 14), D.Dom.span("label", title ++ Bool.pick(String, String.is_empty(VT.Vt.title(U.Ec.vt(ec, key))), "", " · " ++ VT.Vt.title(U.Ec.vt(ec, key)))),
          WI.WIcon.button(Bool.pick(String, pinned, "icon on", "icon"), "emb-pin", key, Bool.pick(String, pinned, "Unpin", "Pin to the top"), "pin", 14, "")]},
        View.emb.screen(U.Ec.vt(ec, key), key, cols, rows)]}

def View.emb.k(live: Bool, pinned: Bool, +ec: U.Ec, +th: String, ev: EM.EV) -> D.Node:
  match live pinned ev:
    case True{} True{} EM.EV{+key, +id, +title, _, _, _, _}:
      D.El{"div", "emb-" ++ id, [D.At{"class", "emb pinned-here"}, D.At{"data-id", id}], [
        WI.WIcon.button("emb-row", "emb-pin", key, "Unpin it back into the chat", "pin", 14, title ++ " · pinned at the top")]}
    case True{} False{} _:
      View.emb.live(ec, ev)
    case False{} _ EM.EV{_, +id, +title, _, _, _, _}:
      D.El{"div", "emb-" ++ id, [D.At{"class", "emb off"}, D.At{"data-id", id}], [
        WI.WIcon.button("emb-row", "emb-restart", th ++ "\t" ++ id, "Start it again", "play", 14, title ++ " · inactive, click to restart")]}

def View.emb.ev(+ev: EM.EV, +ec: U.Ec, +th: String) -> D.Node:
  match ev:
    case EM.EV{k, i, t, c, r, +live, +pinned}:
      View.emb.k(live, pinned, ec, th, EM.EV{k, i, t, c, r, live, pinned})

def View.emb(+ec: U.Ec, +st: M.State, +e: M.Entry, +th: String) -> D.Node:
  View.emb.ev(U.Ec.view(ec, st, e), ec, th)

def View.act.t(term: Bool, e: M.Entry, +st: M.State, +ec: U.Ec) -> D.Node:
  match term e:
    case True{} M.Act{+id, +th, tone, kind, sum, +at}:
      View.emb(ec, st, M.Act{id, th, tone, kind, sum, at}, th)
    case _ M.Act{+id, _, +tone, +kind, +summary, _}:
      D.El{"div", id, [D.At{"class", "act " ++ tone}, D.At{"data-id", id}], [D.Dom.span("kind", RVW.Review.label(tone, kind, summary)), D.Dom.span("summary", RVW.Review.say(tone, kind, summary))]}
    case _ M.Msg{+id, _, _, text, _}:
      D.El{"div", id, [D.At{"class", "act"}, D.At{"data-id", id}], [D.Dom.span("summary", text)]}

def View.pins.go(vs: List<&2, EM.EV>, +ec: U.Ec) -> List<&2, D.Node>:
  match vs:
    case Nil{}:
      Nil{}
    case v <> t:
      View.emb.live(ec, v) <> View.pins.go(t, ec)

# the thread's pinned terminals, at the top with the subagents
def View.pins(vs: List<&2, EM.EV>, +ec: U.Ec) -> List<&2, D.Node>:
  match vs:
    case Nil{}:
      Nil{}
    case _ <> _:
      [D.El{"section", "emb-pins", [D.At{"class", "emb-pins"}], View.pins.go(vs, ec)}]

def View.act(step: Bool, e: M.Entry, +st: M.State, +ec: U.Ec) -> D.Node:
  match step e:
    case True{} M.Act{+id, _, _, _, _, _}:
      D.El{"div", id, [D.At{"class", "act step"}, D.At{"data-id", id}],
        [WI.WIcon.button("step", "hist-show", id, "Show this change in the viewer", "history", 14, HS.Hist.row(e))]}
    case _ M.Act{+id, +th, +tone, +kind, +summary, +at}:
      View.act.t(String.eq(tone, EM.Emb.tone()), M.Act{id, th, tone, kind, summary, at}, st, ec)
    case _ M.Msg{+id, _, _, text, _}:
      D.El{"div", id, [D.At{"class", "act"}, D.At{"data-id", id}], [D.Dom.span("summary", text)]}

def View.entry.own(e: M.Entry, +st: M.State, +ec: U.Ec) -> D.Node:
  match e:
    case M.Msg{+id, _, M.User{}, +text, _}:
      D.El{"div", id, [D.At{"class", "msg user"}, D.At{"data-id", id}], D.Dom.cat(View.user_body(text), [D.Dom.copy(AT.Att.body(text))])}
    case M.Msg{+id, _, _, +text, _}:
      D.El{"div", id, [D.At{"class", "msg assistant"}, D.At{"data-id", id}], D.Dom.cat([D.El{"div", "", [D.At{"class", "body"}], D.Md.render(text)}], D.Dom.cat(View.images(IM.Img.paths(text)), [D.Dom.copy(text)]))}
    case M.Act{+id, +th, +tone, +kind, +summary, +at}:
      View.act(String.eq(tone, HS.Step.tone()), M.Act{id, th, tone, kind, summary, at}, st, ec)

# an entry of this thread (headed by the thread that sent it) or a message
# this thread sent to another (a link)
def View.entry.k(mine: Bool, e: M.Entry, +st: M.State, +ec: U.Ec) -> List<&2, D.Node>:
  match mine e:
    case True{} M.Msg{+id, t, r, x, at}:
      List.append(&2, D.Node, View.from.k(String.is_empty(M.Link.from(id)), M.Link.from(id), st), [View.entry.own(M.Msg{id, t, r, x, at}, st, ec)])
    case True{} _:
      [View.entry.own(e, st, ec)]
    case False{} M.Msg{_, +t, _, x, _}:
      [View.link("out", "→ " ++ U.Tasks.title(st, t) ++ ": " ++ DF.Diff.first(x), t)]
    case False{} _:
      Nil{}

def View.entry(+e: M.Entry, +st: M.State, +sel: String, +ec: U.Ec) -> List<&2, D.Node>:
  View.entry.k(String.eq(View.entry_thread(e), sel), e, st, ec)

def View.entries.r(es: List<&2, M.Entry>, rs: List<&2, FO.Role>, +st: M.State, +sel: String, +ec: U.Ec) -> List<&2, D.Node>:
  match es rs:
    case Nil{} _:
      Nil{}
    case e <> t r <> rt:
      List.append(&2, D.Node, View.role(r, View.entry(e, st, sel, ec)), View.entries.r(t, rt, st, sel, ec))
    case e <> t Nil{}:
      List.append(&2, D.Node, View.entry(e, st, sel, ec), View.entries.r(t, Nil{}, st, sel, ec))

# a thread's entries, its runs of tool calls folded (the last stays open
# while its turn runs)
def View.entries.e(+es: List<&2, M.Entry>, +st: M.State, +sel: String, +info: Map<&2, String>, +running: Bool, +ec: U.Ec) -> List<&2, D.Node>:
  View.entries.r(es, FO.Fold.roles(es, info, running), st, sel, ec)

def View.entries(+es: List<&2, M.Entry>, +st: M.State, +sel: String, +info: Map<&2, String>, +running: Bool) -> List<&2, D.Node>:
  View.entries.e(es, st, sel, info, running, U.Ec.none())

def View.paged.w(w: U.Win, +st: M.State, +sel: String, +info: Map<&2, String>, +running: Bool, +ec: U.Ec) -> List<&2, D.Node>:
  match w:
    case U.Win{+h, es}:
      List.append(&2, D.Node, View.earlier(h, sel), View.entries.e(es, st, sel, info, running, ec))

def View.paged(+drafts: Map<&2, String>, +es: List<&2, M.Entry>, +st: M.State, +sel: String, +info: Map<&2, String>, +running: Bool, +ec: U.Ec) -> List<&2, D.Node>:
  View.paged.w(U.Page.newest.from(es, sel, U.Page.since(st, sel), U.Page.count(Maybe.default(&2, Nat, Nat.read(U.Str.get(drafts, "@more." ++ sel)), 0n))), st, sel, info, running, ec)

# the thread's shell: its screen, and over it a transparent field that
# takes keys (and a phone's keyboard)
def View.term(+v: VT.Vt) -> D.Node:
  D.El{"section", "term-pane", [D.At{"class", "term-pane"}], [
    D.El{"div", "", [D.At{"class", "term-head"}], [D.Dom.span("label", "Terminal" ++ Bool.pick(String, String.is_empty(VT.Vt.title(v)), "", " · " ++ VT.Vt.title(v))),
      WI.WIcon.button("icon", "term-toggle", "", "Close (ctrl+j)", "x", 16, "")]},
    D.El{"div", "", [D.At{"class", "screen"}, D.At{"style", "background:" ++ View.hex(VT.Vt.bg()) ++ ";color:" ++ View.hex(VT.Vt.fg())}],
      D.Dom.cat(View.tlines(VT.Vt.lines(v, 0n)), D.Dom.cat(View.cursor(VT.Vt.cursor(v)),
        [D.El{"textarea", "term", [D.At{"id", "term"}, D.At{"class", "term-in"}, D.At{"data-keys", "term-key"}, D.At{"data-text", "term-paste"},
          D.At{"autocapitalize", "off"}, D.At{"autocomplete", "off"}, D.At{"spellcheck", "false"}, D.At{"aria-label", "Terminal input"}], Nil{}}]))}]}

# Diff
# ----

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
def View.diff(+text: String) -> List<&2, D.Node>:
  +fs = DF.Diff.parse(text)
  [D.El{"div", "", [D.At{"class", "diff-head"}], [
      D.Dom.span("summary", Bool.pick(String, List.is_empty(&2, DF.DFile, fs), "No changes since this thread began.", DF.Diff.summary(fs))),
      D.El{"div", "", [D.At{"class", "diff-btns"}], [
        D.Dom.button("", "revert", "0", "Put the files back as they were when this thread began", "Revert thread"),
        D.Dom.button("", "git", "commit_push_pr", "Commit, push and open a pull request", "Open PR"),
        D.Dom.button("primary", "git", "commit_push", "Commit and push", "Commit & push")]}]},
    D.El{"div", "", [D.At{"class", "diff-body"}], View.dfiles(fs)}]

def View.pending(ps: List<&2, String>) -> List<&2, D.Node>:
  match ps:
    case Nil{}:
      Nil{}
    case +p <> t:
      D.El{"button", "", [D.At{"class", Bool.pick(String, AT.Att.image(p), "chip image", "chip")}, D.At{"title", "Remove " ++ p}, D.At{"data-value", p}, D.On{"click", "detach"}],
        [D.Txt{AT.Att.label(p) ++ " ×"}]} <> View.pending(t)

# what the next message attaches (and what is still uploading)
def View.attaching(+ps: List<&2, String>, +up: String) -> List<&2, D.Node>:
  Bool.pick(List<&2, D.Node>, Bool.and(List.is_empty(&2, String, ps), String.is_empty(up)), Nil{},
    [D.El{"div", "", [D.At{"class", "chips"}], D.Dom.cat(View.pending(ps),
      Bool.pick(List<&2, D.Node>, String.is_empty(up), Nil{}, [D.Dom.span("chip up", up ++ " …")]))}])

# Voice: dictation into the composer (client.bend's Voice). The mic is red
# while it records; above the composer, what is being heard, a note when
# nothing comes in or something failed, and the words to clarify, one at
# a time.
def View.mic(+state: String) -> D.Node:
  WI.WIcon.button(Bool.pick(String, String.is_empty(state), "mic", "mic " ++ state), "voice", "",
    Bool.pick(String, String.is_empty(state), "Dictate with the hub machine's microphone", "Stop dictating"), "mic", 16, "")

def View.vhint(+h: String, +dev: String) -> String:
  +d = Bool.pick(String, String.is_empty(dev), "the microphone", dev)
  Bool.pick(String, String.eq(h, "no-audio"), "Nothing received from " ++ d ++ " yet. Is it plugged in? Pick another in Settings, Voice.",
    Bool.pick(String, String.eq(h, "silent"), "Only silence from " ++ d ++ " so far. Is it muted, or the wrong one?", h))

def View.vstrip(on: Bool, +u: U.Ui) -> List<&2, D.Node>:
  match on:
    case True{}:
      +state = U.Ui.scratch(u, "@voice")
      [D.El{"div", "", [D.At{"class", "voice-strip"}], [D.Dom.span("rec", ""),
        D.Dom.span("voice-head", Bool.pick(String, String.eq(state, "starting"), "Starting the microphone…", Bool.pick(String, String.eq(state, "stopping"), "Finishing…",
          "Listening" ++ Bool.pick(String, String.is_empty(U.Ui.scratch(u, "@voice.dev")), "", " · " ++ U.Ui.scratch(u, "@voice.dev"))))),
        D.El{"span", "", [D.At{"class", "meter"}], [D.El{"span", "", [D.At{"style", "width:" ++ Nat.show(Maybe.default(&2, Nat, Nat.read(U.Ui.scratch(u, "@voice.lv")), 0n)) ++ "%"}], Nil{}}]},
        D.Dom.span("voice-part", U.Ui.scratch(u, "@voice.part")),
        D.Dom.button("stop", "voice", "", "Stop dictating", "Stop")]}]
    case False{}:
      Nil{}

def View.vnote(on: Bool, +u: U.Ui) -> List<&2, D.Node>:
  match on:
    case True{}:
      [D.El{"div", "", [D.At{"class", Bool.pick(String, String.is_empty(U.Ui.scratch(u, "@voice.err")), "voice-note warn", "voice-note bad")}], [
        D.Dom.span("", Bool.pick(String, String.is_empty(U.Ui.scratch(u, "@voice.err")), View.vhint(U.Ui.scratch(u, "@voice.hint"), U.Ui.scratch(u, "@voice.dev")),
          "Dictation: " ++ U.Ui.scratch(u, "@voice.err"))),
        WI.WIcon.button("", "voice-err", "", "Close", "x", 14, "")]}]
    case False{}:
      Nil{}

def View.vask.k(x: VC.Unsure, +n: Nat) -> List<&2, D.Node>:
  match x:
    case VC.Unsure{+w, +g}:
      [D.El{"div", "", [D.At{"class", "voice-ask"}], List.append(&2, D.Node,
        [D.Dom.span("", "Heard \u{201c}" ++ w ++ "\u{201d}" ++ Bool.pick(String, Nat.is_gt(n, 1n), " (1 of " ++ Nat.show(n) ++ ")", ""))],
        List.append(&2, D.Node, Bool.pick(List<&2, D.Node>, String.is_empty(g), Nil{}, [D.Dom.button("primary", "voice-use", "0", "Use the guess", "Use " ++ g)]),
          [D.Dom.button("", "voice-fix", "0", "Type the right spelling", "Fix…"), D.Dom.button("", "voice-add", "0", "Add it to the dictionary as heard", "Add to dictionary"),
           D.Dom.button("", "voice-skip", "0", "Leave it", "Skip")]))}]

def View.vask.go(fixing: Bool, +u: U.Ui, +us: List<&2, VC.Unsure>) -> List<&2, D.Node>:
  match fixing:
    case True{}:
      [D.El{"div", "", [D.At{"class", "voice-ask"}], [WB.WB.input("voice-fix", "vfix", U.Ui.scratch(u, "@f.vfix"), "The right spelling (it joins the dictionary)", "voice-fixed"),
        D.Dom.button("primary", "voice-fixed", "", "Replace the word and add it to the dictionary", "Save"), D.Dom.button("", "form-close", "@voice.fix", "Cancel", "Cancel")]}]
    case False{}:
      View.vask.k(Maybe.default(&2, VC.Unsure, List.head(&2, VC.Unsure, us), VC.Unsure{"", ""}), List.length(&2, VC.Unsure, us))

def View.vask(on: Bool, +u: U.Ui) -> List<&2, D.Node>:
  match on:
    case True{}:
      View.vask.go(Bool.not(String.is_empty(U.Ui.scratch(u, "@voice.fix"))), u, U.Voice.unsure(u))
    case False{}:
      Nil{}

# what sits over the composer for dictation, and the mic's state
def View.voice(+u: U.Ui, +sel: String) -> List<&2, D.Node>:
  +here = Bool.or(String.eq(U.Ui.scratch(u, "@voice.th"), sel), String.is_empty(U.Ui.scratch(u, "@voice.th")))
  D.Dom.cat(View.vnote(Bool.and(here, Bool.or(Bool.not(String.is_empty(U.Ui.scratch(u, "@voice.err"))), Bool.and(U.Voice.on(u), Bool.not(String.is_empty(U.Ui.scratch(u, "@voice.hint")))))), u),
    D.Dom.cat(View.vstrip(Bool.and(here, U.Voice.on(u)), u),
      View.vask(Bool.and(String.eq(U.Ui.scratch(u, "@voice.uth"), sel), Bool.not(List.is_empty(&2, VC.Unsure, U.Voice.unsure(u)))), u)))

# a side question (/btw) and its answer, over the composer until closed
def View.btw(+u: U.Ui) -> List<&2, D.Node>:
  Bool.pick(List<&2, D.Node>, String.is_empty(U.Btw.question(u)), Nil{},
    [D.El{"div", "", [D.At{"class", "btw"}], [
      D.El{"div", "", [D.At{"class", "btw-head"}], [D.Dom.span("btw-q", "btw · " ++ U.Btw.question(u)),
        WI.WIcon.button("btw-close", "btw-close", "", "Close", "x", 14, "")]},
      D.Dom.div("btw-a", [D.Txt{U.Btw.text(U.Btw.reply.of(u))}])]}])

def View.mic.of(+u: U.Ui, +sel: String) -> String:
  Bool.pick(String, Bool.or(String.eq(U.Ui.scratch(u, "@voice.th"), sel), String.is_empty(U.Ui.scratch(u, "@voice.th"))), U.Ui.scratch(u, "@voice"), "")

# the send button, or Stop while a turn runs with nothing typed
def View.send(+act: String, +send: String) -> D.Node:
  Bool.pick(D.Node, String.eq(act, "interrupt"), D.Dom.button("primary stop", act, "", "Stop the running turn", send),
    D.Dom.button("primary", act, "", "Enter; ctrl+Enter to " ++ Bool.pick(String, String.eq(send, "Queue"), "steer", "queue") ++ " instead", send))

def View.composer(+draft: String, +send: String, +act: String, +label: String, +ctx: String, +picking: Bool, +ms: List<&2, String>, +model: String, +info: String, +prov: String, +es: List<&2, String>, +effort: String, +rs: List<&2, U.SkillRow>,
    +atts: List<&2, String>, +up: String, vo: List<&2, D.Node>, +mic: String) -> D.Node:
  D.El{"div", "", [D.At{"class", "composer"}], D.Dom.cat(
    View.menu(picking, ms, model, info, prov, es, effort), D.Dom.cat(View.skills(rs), D.Dom.cat(vo, D.Dom.cat(View.attaching(atts, up), [
    D.El{"textarea", "composer", [D.At{"id", "composer"}, D.At{"rows", "3"}, D.At{"placeholder", "Ask anything: @bot, >thread, %project, #room, $skill, /btw"}, D.At{"value", draft}, D.On{"input", "draft"},
      D.At{"data-enter", Bool.pick(String, List.is_empty(&2, U.SkillRow, rs), "send", "skill")}, D.At{"data-enter-alt", "send-alt"}, D.At{"data-tab", Bool.pick(String, List.is_empty(&2, U.SkillRow, rs), "", "skill")}, D.At{"data-arrows", Bool.pick(String, List.is_empty(&2, U.SkillRow, rs), "", "skill-move")}, D.At{"data-esc", Bool.pick(String, List.is_empty(&2, U.SkillRow, rs), "", "skill-close")}, D.At{"data-paste", "attach"}], Nil{}},
    D.El{"div", "", [D.At{"class", "composer-foot"}], [
      WI.WIcon.after("model-chip", "flag", "models", "Choose the model and effort", U.Effort.label(label, effort) ++ ctx, "chevron-down", 14),
      D.El{"label", "", [D.At{"class", "attach"}, D.At{"title", "Attach files or images (or paste an image)"}], [WI.WIcon.svg("paperclip", 16),
        D.El{"input", "attach-file", [D.At{"type", "file"}, D.At{"multiple", ""}, D.At{"data-attach", "attach"}], Nil{}}]},
      D.El{"span", "", [D.At{"class", "foot-end"}], [View.mic(mic),
        View.send(act, send)]}]}]))))}

# the thread's header, or the one given (a bot's)
def View.top(hd: List<&2, D.Node>, t: M.Thread, +st: M.State, +panel: Bool, snoozed: Bool, picking: Bool, term: Bool, +diff: Bool, del: Bool, +ren: String, +rtext: String) -> List<&2, D.Node>:
  match hd:
    case Nil{}:
      [View.header(t, st, panel, snoozed, picking, term, diff, del, ren, rtext)]
    case h <> r:
      h <> r

# The two parts of a thread's page that follow its draft: the composer and
# the asks (an answer with the draft). The page draws them with these, and
# host.js redraws only them for a keystroke (app.bend's composer and asks):
# rebuilding a long thread's page for every key took 280 ms a key.
def View.composer.t(+st: M.State, +sel: String, +drafts: Map<&2, String>, +op: Map<&2, Bool>, +nf: Map<&2, String>, +turn: M.TurnState, +pr: String, +mo: String,
    +atts: List<&2, String>, +up: String, vo: List<&2, D.Node>, +mic: String) -> D.Node:
  +stop = U.Follow.stop(View.running(turn), U.Str.get(drafts, sel), atts)
  View.composer(U.Str.get(drafts, sel), U.Follow.button(st, sel, View.running(turn), stop, U.Str.get(drafts, sel)), U.Follow.act(stop), U.Models.label(pr, mo), U.Usage.tail(st, sel), U.Flag.get(op, "models"), U.Models.menu(U.Str.get(nf, "models"), pr), mo, U.Str.get(nf, "models"), pr,
    U.Chip.all(U.Str.get(nf, "models"), pr, mo), U.Effort.of(st, sel), U.Skills.shown(st, sel, drafts, nf), atts, up, vo, mic)

def View.asks.box(+st: M.State, +id: String, +draft: String, +nf: Map<&2, String>) -> D.Node:
  D.El{"div", "asks", [D.At{"class", "asks"}, D.At{"id", "asks"}], View.asks(U.Asks.open(st, id), draft, nf)}

# the same two from the Ui alone (app.bend's composer and asks)
def View.composer.k(u: U.Ui, t: M.Thread, +atts: List<&2, String>, +up: String, vo: List<&2, D.Node>, +mic: String) -> D.Node:
  match u t:
    case U.Ui{+st, +sel, +drafts, _, +op, _, _, _, _, _, _, _, _, _, +nf, _, _} M.Thread{_, _, _, _, _, +turn, _, _, _, _, _, _, _, _, _, +pr, +mo, _, _, _, _}:
      View.composer.t(st, sel, drafts, op, nf, turn, pr, mo, atts, up, vo, mic)

def View.composer.u(+u: U.Ui) -> D.Node:
  View.composer.k(u, U.Ui.thread(U.Ui.state(u), U.Ui.sel(u)), U.Ui.atts(u, U.Ui.sel(u)), U.Ui.uploading(u, U.Ui.sel(u)), D.Dom.cat(View.btw(u), View.voice(u, U.Ui.sel(u))), View.mic.of(u, U.Ui.sel(u)))

def View.asks.u(u: U.Ui) -> D.Node:
  match u:
    case U.Ui{+st, +sel, +drafts, _, _, _, _, _, _, _, _, _, _, _, +nf, _, _}:
      View.asks.box(st, sel, U.Str.get(drafts, sel), nf)

def View.thread.p(u: U.Ui, t: M.Thread, pend: List<&2, String>, +atts: List<&2, String>, +up: String, +term: Bool, hd: List<&2, D.Node>, vo: List<&2, D.Node>, +mic: String) -> D.Node:
  match u t:
    case U.Ui{+st, +sel, +drafts, +live, +op, _, +now, _, panel, _, _, _, _, _, +nf, _, +tr} M.Thread{+id, p, ti, c, a, +turn, pe, ar, o, sa, ua, pi, en, br, wt, +pr, +mo, rm, im, se, de}:
      +stop = U.Follow.stop(View.running(turn), U.Str.get(drafts, sel), atts)
      D.El{"main", "thread", [D.At{"class", Bool.pick(String, term, "thread-view with-term", "thread-view") ++ Bool.pick(String, U.Flag.get(op, "drop"), " dropping", "")},
        D.At{"data-drop", "attach"}, D.At{"data-drop-hover", "drop-hover"}], D.Dom.cat(D.Dom.cat(
        View.top(hd, M.Thread{id, p, ti, c, a, turn, pe, ar, o, sa, ua, pi, en, br, wt, pr, mo, rm, im, se, de}, st, panel, M.Snooze.on(now, U.Side.until(st, id)), U.Flag.get(op, "snooze"), term, String.eq(U.Str.get(nf, "view.kind:" ++ id), "diff"), String.eq(U.Str.get(drafts, "@del"), id), U.Str.get(drafts, "@ren"), U.Str.get(drafts, "@f.title")), [
        D.El{"div", "timeline", [D.At{"class", "timeline"}, D.At{"id", "timeline"}],
          D.Dom.cat(View.parent(st, id, U.Tasks.up(st, id)), D.Dom.cat([D.El{"div", "work", [D.At{"class", "work"}], D.Dom.cat(View.subs(nf, id, SB.Subs.rows(st, nf, id)), View.pins(U.Ec.pins(U.Ec.make(tr, st, id, now), st, id), U.Ec.make(tr, st, id, now)))}], D.Dom.cat(Nil{},
          D.Dom.cat(View.paged(drafts, M.State.mine(st, id), st, id, nf, View.running(turn), U.Ec.make(tr, st, id, now)), D.Dom.cat(View.sending(pend),
            View.live.k(String.is_empty(U.Str.get(live, id)), U.Side.phase(st, U.Ui.thread(st, id)), U.Str.get(live, id)))))))},
        View.asks.box(st, id, U.Str.get(drafts, sel), nf),
        View.todos(nf, id, U.Todos.shown(st, id, View.running(turn))),
        View.queued(M.Queue.items(st, id), View.running(turn)),
        View.composer.t(st, sel, drafts, op, nf, turn, pr, mo, atts, up, vo, mic)]),
        D.Dom.when(term, View.term(U.Term.get(tr, id))))}

def View.thread.h(+u: U.Ui, t: M.Thread, hd: List<&2, D.Node>) -> D.Node:
  View.thread.p(u, t, U.Ui.pending(u, U.Ui.sel(u)), U.Ui.atts(u, U.Ui.sel(u)), U.Ui.uploading(u, U.Ui.sel(u)), U.Ui.flag(u, "term"), hd, D.Dom.cat(View.btw(u), View.voice(u, U.Ui.sel(u))), View.mic.of(u, U.Ui.sel(u)))

def View.thread(+u: U.Ui, t: M.Thread) -> D.Node:
  View.thread.h(u, t, Nil{})

# a bot, here or mirrored from a linked machine: its header and tabs
# (BU.head), then its thread (Chat) or another tab
def View.bot.k(chat: Bool, +u: U.Ui, +b: BT.Bot, +hd: BI.Head) -> D.Node:
  match chat:
    case True{}:
      View.thread.h(u, U.Ui.thread(U.Ui.state(u), BT.Bot.thread(b)), [WB.WB.head(u, b, hd), WB.WB.tabs(hd)])
    case False{}:
      D.El{"main", "bot", [D.At{"class", "thread-view bot-view"}], [WB.WB.head(u, b, hd), WB.WB.tabs(hd), WB.WB.main(u, b, BI.Head.tab(hd))]}

def View.bot.h(+u: U.Ui, +b: BT.Bot, +hd: BI.Head) -> D.Node:
  View.bot.k(String.eq(BI.Head.tab(hd), "chat"), u, b, hd)

def View.bot(+u: U.Ui, +b: BT.Bot) -> D.Node:
  View.bot.h(u, b, BI.BU.head.bot(u, b))

def View.main.s(settings: Bool, none: Bool, u: U.Ui, +sel: String, +n: Nat) -> D.Node:
  match settings none u:
    case True{} _ U.Ui{+st, s2, d, l, o, on, now, ad, pa, +v, r, w, e, +note, +nf, ob, tr}:
      View.settings(U.Ui{st, s2, d, l, o, on, now, ad, pa, v, r, w, e, note, nf, ob, tr}, st, nf, v, note)
    case False{} True{} _:
      View.empty(n)
    case False{} False{} U.Ui{+st, s2, d, l, o, on, now, ad, pa, v, r, w, e, note, nf, ob, tr}:
      View.thread(U.Ui{st, s2, d, l, o, on, now, ad, pa, v, r, w, e, note, nf, ob, tr}, U.Ui.thread(st, sel))

def View.main.k(none: Bool, u: U.Ui, +sel: String, +n: Nat) -> D.Node:
  match none u:
    case _ U.Ui{+st, _, _, _, +o, _, _, _, _, +v, _, _, _, +note, +nf, +ob, +tr}:
      View.main.s(U.Flag.get(o, "settings"), none, u, sel, n)

# settings, else a bot, a room or a bot elsewhere, else the thread
def View.main.b(settings: Bool, bv: BI.View, +u: U.Ui, +sel: String, +n: Nat) -> D.Node:
  match settings bv:
    case False{} BI.VBot{b}:
      View.bot(u, b)
    case False{} BI.VRoom{r}:
      WB.WB.room(u, r)
    case False{} BI.VRemote{id}:
      WB.WB.remote(u, id)
    case _ _:
      View.main.k(String.is_empty(sel), u, sel, n)

# the thread's own browser page (a bot's is the bot's), as a bot's
# Browser tab shows it
def View.browser(+u: U.Ui) -> List<&2, D.Node>:
  [WB.WB.browser.k(String.is_empty(U.Ui.info(u, "shot." ++ U.Ui.page(u))), U.Ui.info(u, "shot." ++ U.Ui.page(u)), U.Ui.page(u), "This thread")]

# The Mechanical page (client.bend's Mpage): a chip per part, Open in 3D
# and Refresh, the part's renders (a click opens one in the lightbox), and
# where to get FreeCAD when the hub has none
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

def View.mech.k(state: Nat, +u: U.Ui) -> List<&2, D.Node>:
  match state:
    case 0n:
      [D.Dom.span("mech-say", "Loading the parts…")]
    case 1n:
      [D.Dom.span("mech-say", MC.Mech.none())]
    case _:
      D.El{"div", "", [D.At{"class", "mech-head"}], List.append(&2, D.Node, View.mech.chips(U.Mpage.parts(u), U.Mpage.cur.path(u)),
        [D.Dom.button("", "mech-3d", MC.Mech.abs(U.Mpage.root(u), U.Mpage.cur.path(u)), "Turn the part in the 3D viewer", "3D"),
         D.Dom.button(Bool.pick(String, String.is_empty(U.Mpage.busy(u)), "primary", "busy"), Bool.pick(String, String.is_empty(U.Mpage.busy(u)), "mech-render", ""),
           U.Mpage.cur.path(u), "Draw the part's iso, front, top and right views on the hub", U.Mpage.render.label(u)),
         D.Dom.button("", "mech-refresh", "", "Look for parts and renders again", "Refresh")])}
      <> List.append(&2, D.Node, D.Dom.when(Bool.not(String.is_empty(U.Mpage.err(u))), D.Dom.span("mech-say", "Rendering failed: " ++ U.Mpage.err(u))),
        View.mech.part(U.Mpage.cur(u), U.Mpage.root(u)))

def View.mech(+u: U.Ui) -> List<&2, D.Node>:
  [D.El{"div", "", [D.At{"class", "mech-page"}],
    List.append(&2, D.Node, View.mech.k(U.Mpage.state(u), u), D.Dom.when(Bool.not(String.is_empty(U.Mpage.note(u))), D.Dom.span("mech-note", U.Mpage.note(u))))}]

# Open in KiCad: the project opens in KiCad on the hub's machine, as the
# window's button does
def View.kicad() -> D.Node:
  D.Dom.button("", "kicad-open", "", "Open this project in KiCad on the hub's machine", "Open in KiCad")

# A fit button: host.js fits the board, schematic or model on show
def View.fit() -> D.Node:
  D.El{"button", "", [D.At{"class", "fit"}, D.At{"title", "Show the whole thing (or double-click it)"}, D.At{"data-view", "fit"}], [D.Txt{"Fit"}]}

def View.bar(kids: List<&2, D.Node>, +hint: String) -> D.Node:
  D.El{"div", "", [D.At{"class", "viewer-bar"}], List.append(&2, D.Node, kids, [D.Dom.span("hint", hint), View.fit()])}

# a hierarchical schematic's sheets as chips (the one on show lit; the
# root's value is "", the project's own file); none with one sheet
def View.sheet.chips(rs: List<&2, HI.Row>, +i: Nat, +cur: String) -> List<&2, D.Node>:
  match rs:
    case Nil{}:
      Nil{}
    case HI.Row{+d, +n, +f, _} <> t:
      D.Dom.button(Bool.pick(String, String.eq(Bool.pick(String, Nat.is_eq(i, 0n), "", f), cur), "on", ""), "view-sheet", Bool.pick(String, Nat.is_eq(i, 0n), "", f), f,
        Bool.pick(String, Nat.is_eq(i, 0n), "Top sheet", String.repeat("› ", d) ++ n)) <> View.sheet.chips(t, 1n+i, cur)

def View.sheets(+u: U.Ui) -> List<&2, D.Node>:
  Bool.pick(List<&2, D.Node>, Nat.is_gt(List.length(&2, HI.Row, HI.Hier.read(U.Ui.info(u, "sheets:" ++ U.Ui.root(u)))), 1n),
    View.sheet.chips(HI.Hier.read(U.Ui.info(u, "sheets:" ++ U.Ui.root(u))), 0n, U.Ui.view_get(u, "view.path")), [D.Dom.span("what", "The project's schematic")])

# the Layers menu (core/layers.bend): each layer shown by default, in
# KiCad's order (plot2d.js hides those the file has nothing on), a row of
# its colour (a click shows or hides it) and its name (a click makes it the
# active layer and dims the rest; a click on it again puts all back). Over
# a board's rows, how the layers other than the active one show: All, Dim
# or Only.
def View.layer.alpha(l: U32 & U32) -> U32:
  (_, a) = l
  a

def View.layer.color(l: U32 & U32) -> U32:
  (c, _) = l
  c

def View.layer.row(shown: Bool, +i: U32, +name: String, +color: U32, +off: U32, +act: Bool, rest: List<&2, D.Node>) -> List<&2, D.Node>:
  match shown:
    case True{}:
      D.El{"div", "", [D.At{"class", "layer-row" ++ Bool.pick(String, act, " active", "") ++ Bool.pick(String, LY.Lyr.has(off, i), " off", "")}, D.At{"data-layer", U32.show(i)}],
        [D.El{"button", "", [D.At{"class", "swatch"}, D.At{"title", "Show or hide " ++ name}, D.At{"data-value", U32.show(i)},
            D.At{"style", "background:" ++ View.hex(color)}, D.On{"click", "view-layer"}], Nil{}},
         D.El{"button", "", [D.At{"class", "lname"}, D.At{"title", Bool.pick(String, act, "Show every layer again", name ++ " stands out")}, D.At{"data-value", U32.show(i)},
            D.On{"click", "view-layer-pick"}], [D.Txt{name}]}]} <> rest
    case False{}:
      rest

def View.layer.rows(ls: List<&2, U32>, +k: U32, +off: U32, +f: LY.Focus) -> List<&2, D.Node>:
  match ls:
    case Nil{}:
      Nil{}
    case +i <> t:
      View.layer.row(U32.is_gt(View.layer.alpha(LO.Look.of.t(False{}, k, i)), 0), i, LY.Lyr.name(k, i), View.layer.color(LO.Look.of.t(False{}, k, i)), off,
        Bool.and(LY.Lyr.on(f), U32.is_eq(i, LY.Lyr.active(f))), View.layer.rows(t, k, off, f))

def View.layer.mode(+m: U32, +cur: U32, +label: String, +tip: String) -> D.Node:
  D.El{"button", "", [D.At{"class", Bool.pick(String, U32.is_eq(m, cur), "lmode on", "lmode")}, D.At{"title", tip}, D.At{"data-value", U32.show(m)}, D.On{"click", "view-layer-mode"}], [D.Txt{label}]}

# KiCad's high contrast: the other layers as they are, dimmed, or hidden
def View.layer.modes(sch: Bool, +f: LY.Focus) -> List<&2, D.Node>:
  match sch:
    case True{}:
      Nil{}
    case False{}:
      +cur = {Bool.pick(U32, LY.Lyr.on(f), LY.Lyr.mode(f), 0) : U32}
      [D.El{"div", "", [D.At{"class", "layer-modes"}], [View.layer.mode(0, cur, "All", "Every layer as it is"),
        View.layer.mode(1, cur, "Dim", "The active layer stands out, the rest dimmed"),
        View.layer.mode(2, cur, "Only", "Only the active layer, with the outline (and a copper layer's pads and vias)")]}]

# the menu's button names the active layer while one stands out
def View.layer.head(+k: U32, +f: LY.Focus) -> String:
  Bool.pick(String, LY.Lyr.on(f), "Layers · " ++ LY.Lyr.name(k, LY.Lyr.active(f)), "Layers")

# the look kind of a plot: a board or schematic, a comparison's marks too
def View.layer.kind(sch: Bool, cmp: Bool) -> U32:
  U32.or(Bool.pick(U32, sch, 1, 0), Bool.pick(U32, cmp, LO.Look.diff(), 0))

def View.layers(+sch: Bool, +cmp: Bool, +off: U32, +f: LY.Focus) -> D.Node:
  +k = View.layer.kind(sch, cmp)
  D.El{"details", "", [D.At{"class", "layers"}], [D.El{"summary", "", [D.At{"title", "Layers: show, hide or single one out"}], [D.Txt{View.layer.head(k, f)}]},
    D.El{"div", "", [D.At{"class", "layer-list"}], List.append(&2, D.Node, View.layer.modes(sch, f), View.layer.rows(LY.Lyr.order(k), k, off, f))}]}

# The design history bar (client.bend's Hs.bar): the arrows, play and
# the position, the changes, compare and live buttons; a scrubber with a
# tick a step; what the agent said before the step, and a way back to it.
# A comparison names its two sides (each opens the menu of versions) over
# a legend.

def View.hist.icon(+cls: String, +action: String, +title: String, +name: String) -> D.Node:
  WI.WIcon.button(cls, action, "", title, name, 16, "")

def View.hist.live(at: Nat) -> List<&2, D.Node>:
  match at:
    case 0n:
      Nil{}
    case _:
      [D.Dom.button("hbtn live", "hist-live", "", "Back to the file as it is now", "Live")]

def View.hist.tick(+id: String, +i: Nat, +n: Nat, +cur: Bool) -> D.Node:
  D.El{"button", "", [D.At{"class", Bool.pick(String, cur, "tick on", "tick")}, D.At{"title", Bool.pick(String, String.is_empty(id), "The live file", "Step " ++ Nat.show(1n+i))},
    D.At{"style", "left:calc(100% * " ++ Nat.show(i) ++ " / " ++ Nat.show(Bool.pick(Nat, Nat.is_eq(n, 0n), 1n, n)) ++ ")"}, D.At{"data-value", id}, D.On{"click", "hist-go"}], Nil{}}

def View.hist.ticks(ss: List<&2, HS.Step>, +i: Nat, +n: Nat, +at: Nat) -> List<&2, D.Node>:
  match ss:
    case Nil{}:
      [View.hist.tick("", i, n, Nat.is_eq(at, 0n))]
    case +s <> t:
      View.hist.tick(HS.Step.id(s), i, n, Nat.is_eq(at, 1n+i)) <> View.hist.ticks(t, 1n+i, n, at)

def View.hist.said(live: Bool, +text: String) -> List<&2, D.Node>:
  match live:
    case True{}:
      [D.Dom.span("said", text)]
    case False{}:
      [WI.WIcon.button("icon", "hist-jump", "", "Go to this point of the thread", "message-square", 16, ""),
       D.El{"button", "", [D.At{"class", "said"}, D.At{"title", "Go to this point of the thread"}, D.At{"data-value", ""}, D.On{"click", "hist-jump"}], [D.Txt{text}]}]

def View.hist.walk(b: U.HBar, +u: U.Ui) -> D.Node:
  match b:
    case U.HBar{_, +n, +at, +title, _, +label, +said, +pl, +df, _, _, _, _}:
      D.El{"div", "", [D.At{"class", "hist"}], [
        D.El{"div", "", [D.At{"class", "hist-row"}], List.append(&2, D.Node, [
          View.hist.icon("icon", "hist-first", "The first step", "skip-back"),
          View.hist.icon("icon", "hist-prev", "The step before", "chevron-left"),
          View.hist.icon("icon play", "hist-play", Bool.pick(String, pl, "Pause", "Play the steps"), Bool.pick(String, pl, "pause", "play")),
          View.hist.icon("icon", "hist-next", "The step after", "chevron-right"),
          View.hist.icon("icon", "hist-last", "The last step", "skip-forward"),
          D.Dom.span("title", title), D.Dom.span("grow", ""),
          D.Dom.button(Bool.pick(String, df, "hbtn on", "hbtn"), "hist-diff", "", "Mark what each step changed", "Changes"),
          D.Dom.button("hbtn", "cmp", "a", "Compare two versions of the file", "Compare")], View.hist.live(at))},
        D.El{"div", "", [D.At{"class", "hist-scrub"}], View.hist.ticks(U.Hs.track(u), 0n, n, at)},
        D.El{"div", "", [D.At{"class", "hist-said"}], View.hist.said(Nat.is_eq(at, 0n), Bool.pick(String, Nat.is_eq(at, 0n), U.Hs.live.said(n), Bool.pick(String, String.is_empty(said), label, said)))}]}

def View.hist.key(+layer: U32, +label: String) -> D.Node:
  D.El{"span", "", [D.At{"class", "key"}], [D.El{"i", "", [D.At{"style", "background:" ++ View.hex(LO.Look.mark.color(False{}, layer))}], Nil{}}, D.Txt{label}]}

# Old / Diff / New (client.bend's Rv.showing), the legend only under Diff
def View.hist.as(+showing: String, +as: String, +label: String) -> D.Node:
  D.Dom.button(Bool.pick(String, String.eq(showing, as), "hbtn on", "hbtn"), "rv-as", as, "Show the " ++ label ++ " version", label)

def View.hist.legend(on: Bool) -> List<&2, D.Node>:
  match on:
    case True{}:
      [View.hist.key(26, "Removed"), View.hist.key(27, "Changed"), View.hist.key(28, "Added")]
    case False{}:
      Nil{}

def View.hist.cmp(b: U.HBar, +u: U.Ui) -> D.Node:
  match b:
    case U.HBar{_, _, _, _, _, _, _, _, _, _, +a, +bb, +menu}:
      +showing = {U.Rv.showing(u) : String}
      D.El{"div", "", [D.At{"class", "hist cmp"}], [
        D.El{"div", "", [D.At{"class", "hist-row"}], [
          D.Dom.button(Bool.pick(String, String.eq(menu, "a"), "hbtn side on", "hbtn side"), "cmp-side", "a", "Pick the old version", "Old: " ++ a),
          D.Dom.button(Bool.pick(String, String.eq(menu, "b"), "hbtn side on", "hbtn side"), "cmp-side", "b", "Pick the new version", "New: " ++ bb),
          D.Dom.span("grow", ""), D.Dom.button("hbtn", "rv-bom", "", "The parts that changed between the two versions", "BOM"), D.Dom.button("hbtn", "cmp-close", "", "Back to the file as it is now", "Close")]},
        D.El{"div", "", [D.At{"class", "hist-legend"}], List.append(&2, D.Node, View.hist.legend(String.eq(showing, "diff")),
          [D.Dom.span("grow", ""), View.hist.as(showing, "old", "Old"), View.hist.as(showing, "diff", "Diff"), View.hist.as(showing, "new", "New")])}]}

def View.hist.k(on: Bool, cmp: Bool, b: U.HBar, +u: U.Ui) -> List<&2, D.Node>:
  match on cmp:
    case False{} _:
      Nil{}
    case True{} True{}:
      [View.hist.cmp(b, u)]
    case True{} False{}:
      [View.hist.walk(b, u)]

def View.hist(+b: U.HBar, +u: U.Ui) -> List<&2, D.Node>:
  View.hist.k(U.HBar.on(b), U.HBar.cmp(b), b, u)

# the menu of versions over the stage, a row each; a press outside shuts it
def View.hist.rows(rs: List<&2, U.HRow>) -> List<&2, D.Node>:
  match rs:
    case Nil{}:
      Nil{}
    case U.HRow{+l, +sub, +v} <> t:
      D.El{"button", "", [D.At{"class", "row"}, D.At{"data-value", v}, D.On{"click", "cmp-pick"}], [D.Dom.span("name", l), D.Dom.span("sub", sub)]} <> View.hist.rows(t)

# a ref typed in: Enter sends rv-ref "<side>\t<text>" (host.js's data-pre)
def View.hist.ref(+side: String) -> D.Node:
  D.El{"input", "rvref", [D.At{"id", "rvref"}, D.At{"class", "field"}, D.At{"placeholder", "Commit, branch or tag"}, D.At{"autocomplete", "off"}, D.At{"spellcheck", "false"},
    D.At{"data-enter", "rv-ref"}, D.At{"data-pre", Bool.pick(String, String.eq(side, "b"), "b\t", "a\t")}], Nil{}}

def View.hist.menu.k(open: Bool, +u: U.Ui, +side: String) -> List<&2, D.Node>:
  match open:
    case True{}:
      [D.El{"div", "", [D.At{"class", "hist-scrim"}, D.At{"data-value", ""}, D.On{"click", "cmp-shut"}], Nil{}},
       D.El{"div", "", [D.At{"class", "hist-menu"}], D.Dom.span("head", Bool.pick(String, String.eq(side, "b"), "Compare with (new)", "Compare from (old)")) <> (View.hist.ref(side) <> View.hist.rows(U.Hs.menu(u, side)))}]
    case False{}:
      Nil{}

def View.hist.menu(+u: U.Ui) -> D.Node:
  D.El{"div", "", [D.At{"class", "hist-over"}], View.hist.menu.k(Bool.not(String.is_empty(U.Ui.scratch(u, "@cmp.menu"))), u, U.Ui.scratch(u, "@cmp.menu"))}

# Review comments (client.bend's Rc), over the canvas: a pin at each place
# (plot2d.js puts them where the plot's own transform says: data-x, data-y in
# mm), the note being written and the open comment as cards. A right click
# on the plot starts a note (plot2d.js, "rv-note" with the place).
def View.rc.pin(p: RVW.Pin, +sel: String) -> D.Node:
  match p:
    case RVW.Pin{+id, +x, +y, _, _, +res, +cnt}:
      D.El{"button", "rcp:" ++ id, [D.At{"class", "rc-pin" ++ Bool.pick(String, res, " res", "") ++ Bool.pick(String, String.eq(id, sel), " sel", "")}, D.At{"data-x", F32.show(x)}, D.At{"data-y", F32.show(y)},
        D.At{"data-value", id}, D.At{"title", "Open the comment"}, D.On{"click", "rv-pin"}], Bool.pick(List<&2, D.Node>, Nat.is_gt(cnt, 0n), [D.Txt{Nat.show(cnt)}], Nil{})}

def View.rc.pins(ps: List<&2, RVW.Pin>, +sel: String) -> List<&2, D.Node>:
  match ps:
    case Nil{}:
      Nil{}
    case +p <> t:
      View.rc.pin(p, sel) <> View.rc.pins(t, sel)

def View.rc.reps(rs: List<&2, String>) -> List<&2, D.Node>:
  match rs:
    case Nil{}:
      Nil{}
    case +r <> t:
      D.El{"div", "", [D.At{"class", "rc-reply"}], [D.Dom.span("by", Maybe.default(&2, String, List.get(&2, String, String.split(r, '\u{1f}'), 1n), "")),
        D.Txt{Maybe.default(&2, String, List.get(&2, String, String.split(r, '\u{1f}'), 0n), "")}]} <> View.rc.reps(t)

def View.rc.open.k(o: U.RcOpen, +u: U.Ui) -> D.Node:
  match o:
    case U.RcOpen{+id, +text, +by, +place, +res, +reps}:
      D.El{"div", "rc-open", [D.At{"class", "rc-card rc-open"}],
        List.concat(&2, D.Node, [[D.El{"div", "", [D.At{"class", "rc-head"}], [D.Dom.span("what", by ++ " · " ++ place ++ Bool.pick(String, res, " · resolved", "")), D.Dom.button("rc-x", "rv-pin", "", "Close", "×")]},
          D.El{"div", "", [D.At{"class", "rc-text"}], [D.Txt{text}]}],
          View.rc.reps(reps),
          [WB.WB.input("rcreply", "rcreply", U.Ui.scratch(u, "@f.rcreply"), "Reply", "rv-reply-send"),
           D.El{"div", "", [D.At{"class", "rc-row"}], [D.Dom.button("primary", "rv-reply-send", "", "Send the reply", "Reply"),
             D.Dom.button("", Bool.pick(String, res, "rv-reopen", "rv-resolve"), id, Bool.pick(String, res, "Open it again", "Mark it done"), Bool.pick(String, res, "Reopen", "Resolve"))]}]])}

def View.rc.open(m: Maybe<&2, U.RcOpen>, +u: U.Ui) -> List<&2, D.Node>:
  match m:
    case Some{+o}:
      [View.rc.open.k(o, u)]
    case None{}:
      Nil{}

def View.rc.note.k(+xs: List<&2, String>, +u: U.Ui) -> List<&2, D.Node>:
  [D.El{"span", "rc-draft", [D.At{"class", "rc-pin draft"}, D.At{"data-x", Maybe.default(&2, String, List.get(&2, String, xs, 0n), "0")}, D.At{"data-y", Maybe.default(&2, String, List.get(&2, String, xs, 1n), "0")}], Nil{}},
   D.El{"div", "rc-note", [D.At{"class", "rc-card rc-note"}, D.At{"data-x", Maybe.default(&2, String, List.get(&2, String, xs, 0n), "0")}, D.At{"data-y", Maybe.default(&2, String, List.get(&2, String, xs, 1n), "0")}],
    [D.El{"div", "", [D.At{"class", "rc-head"}], [D.Dom.span("what", "Comment here")]},
     WB.WB.input("rcnote", "rcnote", U.Ui.scratch(u, "@f.rcnote"), "Note", "rv-note-send"),
     D.El{"div", "", [D.At{"class", "rc-row"}], [D.Dom.button("primary", "rv-note-send", "", "Pin the comment here", "Send"), D.Dom.button("", "rv-note-shut", "", "Do not comment", "Cancel")]}]}]

def View.rc.note.if(none: Bool, +at: String, +u: U.Ui) -> List<&2, D.Node>:
  match none:
    case True{}:
      Nil{}
    case False{}:
      View.rc.note.k(String.split(at, '\t'), u)

def View.rc.note(+at: String, +u: U.Ui) -> List<&2, D.Node>:
  View.rc.note.if(String.is_empty(at), at, u)

def View.rc(+u: U.Ui) -> List<&2, D.Node>:
  List.concat(&2, D.Node, [View.rc.pins(U.Rc.pins(u), U.Rc.sel(u)), View.rc.note(U.Rc.at(u), u), View.rc.open(U.Rc.open(u), u)])

# The Checks page: the gate line, then the checklist's boxes under their
# group's heading; a click ticks one ("rv-tick" and its index)
def View.rc.tasks(ts: List<&2, U.RpTask>, +prev: String) -> List<&2, D.Node>:
  match ts:
    case Nil{}:
      Nil{}
    case U.RpTask{+i, +text, +done, +group} <> t:
      List.append(&2, D.Node, Bool.pick(List<&2, D.Node>, Bool.and(Bool.not(String.is_empty(group)), Bool.not(String.eq(group, prev))), [D.Dom.span("rc-group", group)], Nil{}),
        D.El{"button", "rct:" ++ Nat.show(i), [D.At{"class", "rc-task" ++ Bool.pick(String, done, " done", "")}, D.At{"data-value", Nat.show(i)}, D.At{"title", Bool.pick(String, done, "Untick", "Tick")}, D.On{"click", "rv-tick"}],
          [D.El{"span", "", [D.At{"class", "rc-box"}], [D.Txt{Bool.pick(String, done, "✓", "")}]}, D.Txt{text}]} <> View.rc.tasks(t, group))

def View.rc.checks.k(on: Bool, +u: U.Ui) -> List<&2, D.Node>:
  match on:
    case True{}:
      [D.El{"div", "", [D.At{"class", "rc-checks"}], D.El{"div", "", [D.At{"class", Bool.pick(String, String.is_empty(U.Rp.gate(u)), "rc-gate ok", "rc-gate bad")}],
        [D.Txt{Bool.pick(String, String.is_empty(U.Rp.gate(u)), "Passes", "Fails: " ++ U.Rp.gate(u))}]} <> View.rc.tasks(U.Rp.tasks(u), "")}]
    case False{}:
      Nil{}

def View.rc.checks(+u: U.Ui) -> List<&2, D.Node>:
  View.rc.checks.k(Bool.and(String.eq(U.Rp.page(u), "checks"), Bool.not(String.is_empty(U.RpView.md(U.Rp.view(u))))), u)

# a board or schematic: host.js (plot2d.js) draws the hub's plot of it on
# the canvas whose data-key it carries, less the layers data-off names
def View.plot(+u: U.Ui, +what: List<&2, D.Node>) -> List<&2, D.Node>:
  View.bar(List.append(&2, D.Node, what, [View.layers(String.eq(U.Solid.kind(u), "schematic"), Nat.is_eq(HS.Hist.kind(U.Ui.view_get(u, "view.path")), 2n), U.Solid.off(u), U.Lfocus.of(u))]), "drag or two fingers to pan · wheel or pinch to zoom · double-click to fit")
   <> D.El{"div", "", [D.At{"class", "viewer-stage"}], D.El{"canvas", "plot", [D.At{"id", "plot"}, D.At{"data-key", U.Solid.want(u)},
        D.At{"data-off", U32.show(LY.Masks.off(LY.Lyr.masks(U.Lfocus.kind(u), U.Solid.off(u), U.Lfocus.of(u))))},
        D.At{"data-dim", U32.show(LY.Masks.dim(LY.Lyr.masks(U.Lfocus.kind(u), U.Solid.off(u), U.Lfocus.of(u))))}, D.At{"data-lift", U32.show(LY.Lyr.top(U.Lfocus.of(u)))},
        D.At{"data-fade", Nat.show(U.Hs.fade())}, D.At{"data-fam", HS.Hist.key.family(U.Solid.want(u))}], Nil{}} <> (View.hist.menu(u) <> View.rc(u))}
   <> View.hist(U.Hs.bar(u), u)

# over a part in 3D: the thread's parts and Renders; the board: its name
# after the parts: Renders for one of them, or the name of another model
# an agent showed (its path on hover)
def View.solid.more(listed: Bool, +u: U.Ui) -> List<&2, D.Node>:
  match listed:
    case True{}:
      [D.Dom.button("", "mech-renders", U.Ui.view_get(u, "view.path"), "The part's pictures, and drawing them", "Renders")]
    case False{}:
      [D.El{"span", "", [D.At{"class", "what on"}, D.At{"title", U.Ui.view_get(u, "view.path")}], [D.Txt{MC.Mech.last(U.Ui.view_get(u, "view.path"))}]}]

def View.solid.what(on: Bool, +u: U.Ui) -> List<&2, D.Node>:
  match on:
    case True{}:
      List.append(&2, D.Node, View.mech.chips(U.Mpage.parts(u), U.Mpage.shown(u)), View.solid.more(U.Mpage.listed(u), u))
    case False{}:
      [D.Dom.span("what", "The board in 3D")]

# a model turned in 3D: host.js (solid.js) draws the hub's model (a "plot"
# with a mesh, as the phones get) on the canvas whose data-key it carries,
# and says in the note why there is none
def View.solid(+u: U.Ui) -> List<&2, D.Node>:
  View.bar(View.solid.what(U.Mpage.onpart(u), u), "drag to turn · right-drag or two fingers to move · wheel or pinch to zoom · double-click to fit")
    <> List.append(&2, D.Node, D.Dom.when(Bool.not(String.is_empty(U.Ui.info(u, "plotnote:" ++ U.Solid.want(u)))), D.Dom.span("viewer-say", U.Ui.info(u, "plotnote:" ++ U.Solid.want(u)))),
      [D.El{"div", "", [D.At{"class", "viewer-stage"}], [D.El{"canvas", "solid", [D.At{"id", "solid"}, D.At{"data-key", U.Solid.want(u)}], Nil{}},
        D.El{"div", "", [D.At{"class", "viewer-note"}], Nil{}}, D.El{"div", "", [D.At{"class", "viewer-stat"}], Nil{}}]}])

# The viewer pane: a tab for each thing it shows (the one on show lit),
# Close, then that thing
def View.tab(+cur: String, +kind: String, +action: String, +value: String, +title: String, +label: String) -> D.Node:
  D.Dom.button(Bool.pick(String, String.eq(cur, kind), "tab on", "tab"), action, value, title, label)

def View.tabs(+cur: String) -> D.Node:
  D.El{"nav", "", [D.At{"class", "viewer-tabs"}], [
    View.tab(cur, "board", "view-kind", "board", "The project's board", "Board"),
    View.tab(cur, "schematic", "view-kind", "schematic", "The project's schematic", "Schematic"),
    View.tab(cur, "3d", "view-kind", "3d", "The board in 3D, with its parts", "3D"),
    View.tab(cur, "mech", "mech", "", "Mechanical parts: their renders, and each in 3D", "Mech"),
    View.tab(cur, "files", "fm", "", "The project's files: open a board, a model, an image or a PDF", "Files"),
    View.tab(cur, "diff", "diff", "", "What this thread changed (ctrl+d)", "Diff"),
    View.tab(cur, "review", "rv-page", "bom", "Review the design: its parts, and what changed", "Review"),
    View.tab(cur, "browser", "view-kind", "browser", "What this thread's browser shows", "Browser"),
    D.El{"span", "", [D.At{"class", "grow"}], Nil{}},
    WI.WIcon.button("icon", "panel", "", "Close the viewer", "x", 16, "")]}

# The Files tab (client.bend's Fm): where the folder is, then its folders
# and files; a file the viewer can open is a button, any other is listed
def View.fm.trail(ps: List<&2, U.PItem>) -> List<&2, D.Node>:
  match ps:
    case Nil{}:
      Nil{}
    case U.PItem{+l, +a, +v} <> t:
      D.Dom.button("crumb", a, v, "Go to " ++ l, l) <> View.fm.trail(t)

def View.fm.row(r: U.FmRow) -> D.Node:
  match r:
    case U.FmRow{+l, +a, +v, +d, +k}:
      D.El{"button", "", List.append(&2, D.Attr, [D.At{"class", "fm-row " ++ Bool.pick(String, String.is_empty(a), "off", k)}, D.At{"title", Bool.pick(String, String.is_empty(a), l ++ ": no preview here", l)}],
          Bool.pick(List<&2, D.Attr>, String.is_empty(a), [D.At{"disabled", ""}], [D.At{"data-value", v}, D.On{"click", a}])),
        [WI.WIcon.svg(U.Fm.icon(k), 16), D.Dom.span("fm-name", l), D.Dom.span("fm-size", d)]}

def View.fm.rows(rs: List<&2, U.FmRow>) -> List<&2, D.Node>:
  match rs:
    case Nil{}:
      Nil{}
    case r <> t:
      View.fm.row(r) <> View.fm.rows(t)

def View.fm.body(ready: Bool, +u: U.Ui) -> List<&2, D.Node>:
  match ready:
    case True{}:
      Bool.pick(List<&2, D.Node>, List.is_empty(&2, U.FmRow, U.Fm.rows(u)), [D.Dom.span("fm-note", "This folder is empty")], View.fm.rows(U.Fm.rows(u)))
    case False{}:
      [D.Dom.span("fm-note", "Listing the folder…")]

def View.files(+u: U.Ui) -> List<&2, D.Node>:
  [D.El{"div", "", [D.At{"class", "fm-bar"}], List.append(&2, D.Node, View.fm.trail(U.Fm.trail(u)),
     [D.El{"span", "", [D.At{"class", "grow"}], Nil{}}, D.Dom.button("", "fm-up", "", "The folder above", "Up")])},
   D.El{"div", "files-list", [D.At{"class", "fm-list"}], View.fm.body(U.Fm.ready(u), u)}]

# an image or a PDF opened from the Files tab: its name, back to the folder
def View.fm.head(+u: U.Ui, more: List<&2, D.Node>) -> D.Node:
  D.El{"div", "", [D.At{"class", "fm-bar"}], List.append(&2, D.Node, [D.Dom.button("", "fm", "", "Back to the folder", "‹ Files"), D.Dom.span("fm-file", U.Fm.name(u)),
    D.El{"span", "", [D.At{"class", "grow"}], Nil{}}], more)}

def View.image(+u: U.Ui) -> List<&2, D.Node>:
  [View.fm.head(u, [D.El{"a", "", [D.At{"class", "btn"}, D.At{"href", IM.Img.url(U.Ui.view_get(u, "view.path"))}, D.At{"target", "_blank"}], [D.Txt{"Open"}]}]),
   D.El{"div", "", [D.At{"class", "fm-image"}], [D.El{"a", "", [D.At{"href", IM.Img.url(U.Ui.view_get(u, "view.path"))}, D.At{"data-lightbox", IM.Img.url(U.Ui.view_get(u, "view.path"))}],
     [D.El{"img", "", [D.At{"src", IM.Img.url(U.Ui.view_get(u, "view.path"))}, D.At{"alt", U.Fm.name(u)}], Nil{}}]}]}]

def View.pdf(+u: U.Ui) -> List<&2, D.Node>:
  [View.fm.head(u, [D.El{"a", "", [D.At{"class", "btn"}, D.At{"href", IM.Img.url(U.Ui.view_get(u, "view.path"))}, D.At{"target", "_blank"}], [D.Txt{"Open in a tab"}]}]),
   D.El{"iframe", "", [D.At{"class", "fm-pdf"}, D.At{"src", IM.Img.url(U.Ui.view_get(u, "view.path"))}, D.At{"title", U.Fm.name(u)}], Nil{}}]

# The Review tab (client.bend's Rp): a button for each page, the title,
# Refresh, then the report's Markdown (or why there is none)
def View.review.tabs(ts: List<&2, U.RpTab>) -> List<&2, D.Node>:
  match ts:
    case Nil{}:
      Nil{}
    case U.RpTab{+p, +l, +on} <> t:
      D.Dom.button(Bool.pick(String, on, "crumb on", "crumb"), "rv-page", p, l, l) <> View.review.tabs(t)

def View.review.body(has: Bool, +v: U.RpView, +extra: List<&2, D.Node>) -> D.Node:
  match has:
    case True{}:
      D.El{"div", "review-body", [D.At{"class", "review-body body"}], List.append(&2, D.Node, extra, D.Md.render(U.RpView.md(v)))}
    case False{}:
      D.El{"div", "review-body", [D.At{"class", "review-body"}], List.append(&2, D.Node, extra, [D.Dom.span("fm-note", Bool.pick(String, String.eq(U.RpView.page(v), "release"), Bool.pick(String, U.RpView.busy(v), "", Bool.pick(String, String.is_empty(U.RpView.err(v)), U.RpView.note(v), "")), U.RpView.note(v)))])}

def View.review.v(+v: U.RpView, +extra: List<&2, D.Node>) -> List<&2, D.Node>:
  [D.El{"div", "", [D.At{"class", "fm-bar review-pane"}], List.append(&2, D.Node, View.review.tabs(U.RpView.tabs(v)),
     [D.Dom.span("fm-file", U.RpView.title(v) ++ Bool.pick(String, U.RpView.busy(v), " · reading…", "")), D.El{"span", "", [D.At{"class", "grow"}], Nil{}},
      D.Dom.button("", "rv-run", "", "Read the design again", "Refresh")])},
   View.review.body(Bool.not(String.is_empty(U.RpView.md(v))), v, extra)]

# The Release page (client.bend's Rl): the tag field (Enter makes the
# release; empty means the proposed tag), two toggles, the button, and why
# not (red) above the report
def View.release.toggle(+label: String, +on: Bool, +act: String) -> D.Node:
  D.El{"button", act, [D.At{"class", "rc-task" ++ Bool.pick(String, on, " done", "")}, D.At{"data-value", ""}, D.At{"aria-pressed", Bool.pick(String, on, "true", "false")}, D.On{"click", act}],
    [D.El{"span", "", [D.At{"class", "rc-box"}], [D.Txt{Bool.pick(String, on, "✓", "")}]}, D.Txt{label}]}

def View.release.k(f: U.RlForm, +v: U.RpView, +u: U.Ui) -> List<&2, D.Node>:
  match f:
    case U.RlForm{+tag, +next, +publish, +force}:
      [D.El{"div", "", [D.At{"class", "rc-checks rl-form"}],
         [WB.WB.input("rltag", "rltag", U.Ui.scratch(u, "@f.rltag"), next, "rv-release"),
          View.release.toggle("Publish on GitHub", publish, "rv-release-pub"),
          View.release.toggle("Release even if checks fail", force, "rv-release-force"),
          D.El{"button", "rv-release", List.append(&2, D.Attr, [D.At{"class", "primary"}, D.At{"data-value", ""}, D.On{"click", "rv-release"}], Bool.pick(List<&2, D.Attr>, U.RpView.busy(v), [D.At{"disabled", "true"}], Nil{})),
            [D.Txt{Bool.pick(String, U.RpView.busy(v), "Making release " ++ tag ++ "…", "Make release " ++ tag)}]}]},
       D.El{"div", "", [D.At{"class", "rc-gate bad"}], Bool.pick(List<&2, D.Node>, String.is_empty(U.RpView.err(v)), Nil{}, [D.Txt{U.RpView.err(v)}])}]

def View.release(+u: U.Ui) -> List<&2, D.Node>:
  View.release.k(U.Rl.form(u), U.Rp.view(u), u)

def View.review.x(rel: Bool, +u: U.Ui) -> List<&2, D.Node>:
  match rel:
    case True{}:
      View.release(u)
    case False{}:
      View.rc.checks(u)

def View.review(+u: U.Ui) -> List<&2, D.Node>:
  View.review.v(U.Rp.view(u), View.review.x(String.eq(U.Rp.page(u), "release"), u))

def View.aside.k(i: Nat, +u: U.Ui) -> List<&2, D.Node>:
  match i:
    case 0n:
      View.diff(U.Ui.view_get(u, "diff"))
    case 1n:
      View.browser(u)
    case 2n:
      View.mech(u)
    case 3n:
      View.solid(u)
    case 4n:
      View.plot(u, List.append(&2, D.Node, View.sheets(u), [View.kicad()]))
    case 6n:
      View.files(u)
    case 7n:
      View.image(u)
    case 8n:
      View.pdf(u)
    case 9n:
      View.review(u)
    case _:
      View.plot(u, [D.Dom.span("what", "The project's board"), View.kicad()])

def View.aside.n(+kind: String) -> Nat:
  Bool.pick(Nat, String.eq(kind, "diff"), 0n, Bool.pick(Nat, String.eq(kind, "browser"), 1n, Bool.pick(Nat, String.eq(kind, "mech"), 2n,
    Bool.pick(Nat, String.eq(kind, "3d"), 3n, Bool.pick(Nat, String.eq(kind, "schematic"), 4n,
      Bool.pick(Nat, String.eq(kind, "files"), 6n, Bool.pick(Nat, String.eq(kind, "image"), 7n, Bool.pick(Nat, String.eq(kind, "pdf"), 8n, Bool.pick(Nat, String.eq(kind, "review"), 9n, 5n)))))))))

# the side pane of the selected thread: the tabs, then what they show. Its
# left edge is a handle that resizes it (host.js keeps the width per
# browser; a double-click gives the pane its share back)
def View.split() -> D.Node:
  D.El{"div", "split", [D.At{"class", "viewer-split"}, D.At{"title", "Drag to resize the viewer; double-click to reset"}], Nil{}}

def View.aside(+kind: String, +u: U.Ui) -> D.Node:
  D.El{"aside", "viewer", [D.At{"class", "viewer " ++ Bool.pick(String, String.is_empty(kind), "board", kind)}, D.At{"id", "viewer"}],
    View.split() <> View.tabs(Bool.pick(String, U.Mpage.onpart(u), "mech", U.Fm.tab(U.Solid.kind(u)))) <> View.aside.k(View.aside.n(kind), u)}

def View.aside.if(open: Bool, +kind: String, +u: U.Ui) -> List<&2, D.Node>:
  match open:
    case True{}:
      [View.aside(kind, u)]
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

def View.tour.back(on: Bool) -> D.Node:
  match on:
    case True{}:
      D.Dom.button("", "ob-back", "", "The step before (left arrow)", "Back")
    case False{}:
      D.Dom.span("gap", "")

def View.tour.card(+i: Nat, +body: String, cs: List<&2, U.TChoice>, +counter: String, back: Bool) -> List<&2, D.Node>:
  [D.El{"div", "", [D.At{"class", "tour-back"}], Nil{}},
    D.El{"div", "tour", [D.At{"class", "tour"}, D.At{"role", "dialog"}, D.At{"aria-modal", "true"}, D.At{"aria-labelledby", "tour-title"}], [
      D.Dom.span("count", counter),
      D.El{"div", "tour-title", [D.At{"class", "tour-title"}, D.At{"id", "tour-title"}], [D.Txt{OB.Ob.title(i)}]},
      D.El{"p", "", [D.At{"class", "tour-body"}], [D.Txt{body}]},
      D.El{"div", "", [D.At{"class", "tour-choices"}], View.tour.choices(cs)},
      D.El{"div", "", [D.At{"class", "tour-btns"}], [
        D.Dom.button("", "ob-skip", "", "End the tour (Esc)", "Skip"),
        D.Dom.span("gap", ""),
        View.tour.back(back),
        D.Dom.button("primary", "ob-next", "", "Next step (Enter or right arrow)", OB.Ob.forward(i))]}]}]

def View.tour.k(on: Bool, +i: Nat, +u: U.Ui) -> List<&2, D.Node>:
  match on:
    case True{}:
      View.tour.card(i, U.Tour.body(1n, u), U.Tour.choices(u), U.Tour.counter(u), U.Tour.back(u))
    case False{}:
      Nil{}

def View.tour(+u: U.Ui) -> List<&2, D.Node>:
  View.tour.k(U.Tour.on(u), U.Tour.at(u), u)

def View.tour.cls.k(on: Bool, +g: String) -> String:
  match on:
    case True{}:
      Bool.pick(String, String.is_empty(g), " tour-on", " tour-on tour-" ++ g)
    case False{}:
      ""

def View.tour.cls(+u: U.Ui) -> String:
  View.tour.cls.k(U.Tour.on(u), U.Tour.target(1n, u))

# the whole page
def View.page.b(+bv: BI.View, u: U.Ui) -> D.Node:
  match u:
    case U.Ui{+st, +sel, +d, +l, +o, +on, +now, +ad, +pa, +v, +r, +w, +e, +note, +nf, +ob, +tr}:
      D.El{"div", "app", [D.At{"class", Bool.pick(String, pa, "app panel", "app") ++ Bool.pick(String, Bool.or(Bool.or(Bool.not(String.is_empty(sel)), U.Flag.get(o, "settings")), WB.WB.shown(bv)), " picked", "")
        ++ Bool.pick(String, String.eq(M.Setting.get(st, "ui.theme", ""), "light"), " light", "") ++ View.tour.cls(U.Ui{st, sel, d, l, o, on, now, ad, pa, v, r, w, e, note, nf, ob, tr})}],
        D.Dom.cat([View.sidebar(U.Ui{st, sel, d, l, o, on, now, ad, pa, v, r, w, e, note, nf, ob, tr}),
          View.main.b(U.Flag.get(o, "settings"), bv, U.Ui{st, sel, d, l, o, on, now, ad, pa, v, r, w, e, note, nf, ob, tr}, sel, List.length(&2, M.Project, U.Ui.projects(st)))],
          D.Dom.cat(View.aside.if(pa, U.Str.get(nf, "view.kind:" ++ sel), U.Ui{st, sel, d, l, o, on, now, ad, pa, v, r, w, e, note, nf, ob, tr}),
            D.Dom.cat(View.picker(U.Flag.get(o, "proj"), U.Ui{st, sel, d, l, o, on, now, ad, pa, v, r, w, e, note, nf, ob, tr}), D.Dom.cat(View.error(e), View.tour(U.Ui{st, sel, d, l, o, on, now, ad, pa, v, r, w, e, note, nf, ob, tr})))))}

def View.page(+u: U.Ui) -> D.Node:
  View.page.b(BI.BU.view(u), u)
