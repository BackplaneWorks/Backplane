import Base
import ../core/cad-install.bend as CIP
import ../core/message-context.bend as CTXT
import ./project-home.bend as PHV
import ../core/project-home.bend as PH
import ./composer-bar.bend as CBAR
import ../core/chat-pending.bend as CP
import ./terminal-cells.bend as TCELL
import ../core/view-page.bend as VP
import ../core/board-catalog.bend as BC
import ./list-windows.bend as LW
import ../core/accounts.bend as ACCT
import ../core/bub-move.bend as HOP
import ../core/subthreads.bend as ST
import ../core/textscale.bend as TS
import ../core/codec.bend as C
import ../core/title.bend as TTL
import ../core/msgs.bend as MSG
import ../core/kexport.bend as KEX
import ../core/json.bend as J
import ../core/kihealth.bend as KH
import ../core/model.bend as M
import ../core/roots.bend as ROOTS
import ../core/far.bend as FR
import ../core/sidebar-read.bend as SR
import ../core/signal.bend as SG
import ../core/i18n.bend as I
import ./dom.bend as D
import ./sidebar-slot.bend as SS
import ./usage-page.bend as UV
import ./allowance.bend as AW
import ../core/allowance.bend as ALQ
import ./settings-menu.bend as SM
import ./icon.bend as WI
import ./phone.bend as PHN
import ../core/plane.bend as PL
import ../core/tools.bend as TO
import ../core/client.bend as U
import ../core/refs.bend as RF
import ../core/usage.bend as UZ
import ../core/usage_page.bend as UP
import ../core/attach.bend as AT
import ../core/vt.bend as VT
import ../core/diff.bend as DF
import ../core/cad.bend as CAD
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
import ../core/ubersync.bend as US
import ./bots.bend as WB
import ../core/voice.bend as VC
import ../core/vdict.bend as VDV
import ../core/hist.bend as HS
import ../core/review.bend as RVW
import ../core/emb.bend as EM
import ../core/qr.bend as QR
import ../core/reply.bend as RTO
import ../core/onboard.bend as OB
import ../core/sched.bend as SCH
import ../core/tz.bend as TZ
import ../core/stamp.bend as STP
import ../core/order.bend as OR
import ../core/modelpick.bend as MPK
import ../core/model-catalog.bend as MCat
import ../core/draftbound.bend as DB
import ../core/fm.bend as FM

# Each web connection explicitly opts into the rail's DOM/CSS contract.
# An old loaded host must keep the flat layout its stylesheet understands.
def View.rail(+u: U.Ui) -> Bool:
  String.eq(U.Ui.info(u, "local.workspace.layout"), "rail-v1")

# The top bar is a second opt-in on top of the rail ("top-v1", host.js):
# its page draws the window's chrome in one bar (View.chrome.bar) instead
# of the sidebar's brand and Settings rows and the thread's own header.
def View.chrome(+u: U.Ui) -> Bool:
  Bool.and(View.rail(u), String.eq(U.Ui.info(u, "local.workspace.chrome"), "top-v1"))

def View.compact(+u: U.Ui) -> Bool:
  String.eq(U.Ui.info(u, "local.view.compact"), "1")

def View.compact.text(compact: Bool, +text: String) -> String:
  match compact:
    case False{}: text
    case True{}: String.take(text, 512n) ++ Bool.pick(String, String.is_empty(String.drop(text, 512n)), "", "…")

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

# VLabel.choice matches a copy of v: a string pattern splits bit by bit and a
# row that mentions its scrutinee gets it rebuilt at every split (lane B).
def VLabel.choice.by(key: String, +lc: I.Loc, v: String) -> String:
  match key:
    case "": MSG.T.common.default(lc)
    case "on": MSG.T.option.on(lc)
    case "off": MSG.T.option.off(lc)
    case "dark": MSG.T.option.dark(lc)
    case "light": MSG.T.option.light(lc)
    case "projects": MSG.T.option.projects(lc)
    case "active": MSG.T.option.active(lc)
    case "stable": MSG.T.option.stable(lc)
    case "nightly": MSG.T.option.nightly(lc)
    case _: v

def VLabel.choice(+lc: I.Loc, +v: String) -> String:
  VLabel.choice.by(v, lc, v)

def VLabel.option.k(+lc: I.Loc, known: Bool, +v: String) -> String:
  match known:
    case True{}: VLabel.choice(lc, v)
    case False{}: Bool.pick(String, String.is_empty(v), MSG.T.common.default(lc), v)

def VLabel.option(+lc: I.Loc, +key: String, v: String) -> String:
  VLabel.option.k(lc, Bool.or(Bool.or(Bool.or(String.eq(key, "order.enabled"), String.eq(key, "order.estimates")), Bool.or(String.eq(key, "ui.tokstats"), String.eq(key, "agent.thinking"))), Bool.or(String.eq(key, "restart.continue"), Bool.or(String.eq(key, "ui.theme"), Bool.or(String.eq(key, "sidebar.view"), Bool.or(String.eq(key, "update.channel"), Bool.or(String.eq(key, "kicad.ipc"), Bool.or(String.starts_with(key, "notify."), String.starts_with(key, "sync.")))))))), v)

# Display labels only: metadata keys and submitted chip values stay unchanged.
# VLabel.effort matches a copy of e: a string pattern splits bit by bit and a
# row that mentions its scrutinee gets it rebuilt at every split (lane B).
def VLabel.effort.by(key: String, +lc: I.Loc, +e: String) -> String:
  match key:
    case "": MSG.T.model.level.auto(lc)
    case "auto": MSG.T.model.level.auto(lc)
    case "none": MSG.T.model.level.none(lc)
    case "minimal": MSG.T.model.level.minimal(lc)
    case "low": MSG.T.model.level.low(lc)
    case "medium": MSG.T.model.level.medium(lc)
    case "high": MSG.T.model.level.high(lc)
    case "xhigh": MSG.T.model.level.xhigh(lc)
    case "max": MSG.T.model.level.max(lc)
    case _: e

def VLabel.effort(+lc: I.Loc, +e: String) -> String:
  VLabel.effort.by(e, lc, e)

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
# left); rows of a shelf with manual order get move buttons on hover, in
# the age's place; the title's tooltip is the whole title, however short
# the row cuts it
# a row with subagents at work says how many where its age goes
def View.note(+badge: String, +other: String) -> D.Node:
  Bool.pick(D.Node, String.is_empty(badge), D.Dom.span("ago", other), D.El{"span", "", [D.At{"class", "ago agents"}, D.At{"title", badge}], [D.Txt{badge}]})

# a red row says what it wants there instead (U.Side.want: approval,
# question, failed), its age kept in the tooltip
def View.note.want(+want: String, +badge: String, +other: String) -> D.Node:
  Bool.pick(D.Node, String.is_empty(want), View.note(badge, other),
    D.El{"span", "", [D.At{"class", "ago want"}, D.At{"title", Bool.pick(String, String.is_empty(other), want, want ++ " · " ++ other)}], [D.Txt{want}]})

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

# A shape and a textual status keep activity accessible without colour.
def View.dot.tip(+name: String, +tip: String) -> D.Node:
  D.El{"span", "", [D.At{"class", "dot " ++ name}, D.At{"role", "img"},
    D.At{"aria-label", tip}, D.At{"title", tip}], Nil{}}

def View.dot(+lc: I.Loc, +name: String) -> D.Node:
  View.dot.tip(name, SG.Signal.label.loc(lc, name))

# a pinned row's mark, small and dim beside its age
def View.pin(+lc: I.Loc, pin: Bool) -> List<&2, D.Node>:
  match pin:
    case True{}:
      [D.El{"span", "", [D.At{"class", "pin"}, D.At{"role", "img"}, D.At{"title", MSG.T.tools.pinned(lc)}, D.At{"aria-label", MSG.T.tools.pinned(lc)}], [WI.WIcon.svg("pin", 11)]}]
    case False{}:
      Nil{}

def View.row.p.loc(+lc: I.Loc, pin: Bool, +id: String, +title: String, +ago: String, +badge: String, +want: String, +dot: String, +sel: String, +movable: Bool) -> D.Node:
  D.El{"div", id, [D.At{"class", Bool.pick(String, String.eq(id, sel), "thread on", "thread")}, D.At{"role", "button"}, D.At{"tabindex", "0"},
    D.At{"aria-current", Bool.pick(String, String.eq(id, sel), "page", "false")}, D.At{"data-value", id}, D.On{"click", "select"}, D.On{"contextmenu", "row-menu"}],
    D.Dom.cat([View.dot(lc, dot), D.El{"span", "", [D.At{"class", "title"}, D.At{"title", title}], [D.Txt{title}]}],
      D.Dom.cat(View.pin(lc, pin), D.Dom.cat(Bool.pick(List<&2, D.Node>, movable, [D.El{"span", "", [D.At{"class", "moves"}], [D.Dom.button("move", "move", id ++ "|up", MSG.T.sidebar.move.up(lc), "↑"), D.Dom.button("move", "move", id ++ "|down", MSG.T.sidebar.move.down(lc), "↓")]}], Nil{}),
        [View.note.want(want, badge, ago)])))}

def View.row.k.loc(+lc: I.Loc, +id: String, +title: String, +ago: String, +badge: String, +dot: String, +sel: String, +movable: Bool) -> D.Node:
  View.row.p.loc(lc, False{}, id, title, ago, badge, "", dot, sel, movable)

# a sidebar row is made by the host from these alone (View.make)

def View.row.k(+id: String, +title: String, +ago: String, +badge: String, +dot: String, +sel: String, +movable: Bool) -> D.Node:
  View.row.k.loc(I.En{}, id, title, ago, badge, dot, sel, movable)

def View.row.m.p.loc(+lc: I.Loc, pin: Bool, +id: String, +title: String, +ago: String, +badge: String, +want: String, +dot: String, +sel: String, +movable: Bool) -> D.Node:
  View.memo.loc(lc, id, [D.At{"fn", "row"}, D.At{"ago", ago}, D.At{"badge", badge}, D.At{"want", want}, D.At{"dot", dot}, D.At{"on", Bool.pick(String, String.eq(id, sel), "1", "")}, D.At{"mov", Bool.pick(String, movable, "1", "")},
    D.At{"pin", Bool.pick(String, pin, "1", "")}], title)

def View.row.m.loc(+lc: I.Loc, +id: String, +title: String, +ago: String, +badge: String, +dot: String, +sel: String, +movable: Bool) -> D.Node:
  View.row.m.p.loc(lc, False{}, id, title, ago, badge, "", dot, sel, movable)

def View.row.m(+id: String, +title: String, +ago: String, +badge: String, +dot: String, +sel: String, +movable: Bool) -> D.Node:
  View.row.m.loc(I.En{}, id, title, ago, badge, dot, sel, movable)

# (settled: a settled thread among the live rows, U.Side.note)
def View.row.s(+lc: I.Loc, +t: M.Thread, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat, +movable: Bool, snoozed: Bool, settled: Bool) -> D.Node:
  match t:
    case M.Thread{+id, _, +title, _, +a, _, _, _, _, _, _, pinned, _, _, _, _, _, _, _, _, _}:
      +dot = SG.Signal.dot.thread(SG.Signal.display.name(U.Side.signal.ix(ix, t), st, M.Thread.id(t)), t)
      View.row.m.p.loc(lc, pinned, id, U.Label.title(st, id, TTL.Title.shown(lc, title)), U.Side.note(lc, snoozed, settled, U.Side.until(st, id), now, a), SG.Signal.display.note(lc, st, id, View.sub.badge(lc, SB.Subs.count.ix(ix, st, id))), U.Side.want(lc, dot, U.SIx.open(ix, M.Thread.id(t)), t), dot, sel, movable)

def View.row(+lc: I.Loc, +t: M.Thread, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat, +movable: Bool, +snoozed: Bool) -> D.Node:
  View.row.s(lc, t, ix, st, sel, now, movable, snoozed, False{})

# Expanders are separate buttons beside the selection target: keyboard
# activation never opens a thread accidentally, and every row has one.
# (data-count: a row with no subthreads shows no chevron, style.css)
def View.tree.expander(+id: String, +expanded: Bool, +count: Nat) -> D.Node:
  D.El{"button", "tree-toggle:" ++ id, [D.At{"class", "thread-expander"}, D.At{"type", "button"}, D.At{"data-count", Nat.show(count)},
    D.At{"aria-expanded", Bool.pick(String, expanded, "true", "false")}, D.At{"aria-label", Bool.pick(String, expanded, "Collapse subthreads", "Expand subthreads") ++ " (" ++ Nat.show(count) ++ ")"},
    D.At{"title", Bool.pick(String, expanded, "Collapse subthreads", "Expand subthreads") ++ " · " ++ Nat.show(count)}, D.At{"data-value", id}, D.On{"click", "thread-tree"}],
    [WI.WIcon.tree(expanded)]}

def View.tree.create(show: Bool, +id: String, +depth: Nat) -> List<&2, D.Node>:
  match show:
    case True{}:
      [D.El{"div", "tree-create:" ++ id, [D.At{"class", "thread-create"}, D.At{"style", "--depth:" ++ Nat.show(depth)}],
        [D.Dom.button("", "thread-child", id, "Create a conversation beneath this thread", "+ New subthread")]}]
    case False{}: Nil{}

# (live: the rows of a live shelf, pinned or active, where a settled
# thread's row says so)
def View.tree.rows.raw(+lc: I.Loc, rs: List<&2, ST.TRow>, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat, +movable: Bool, +live: Bool) -> List<&2, D.Node>:
  match rs:
    case Nil{}: Nil{}
    case ST.TRow{+t, +depth, +expanded, +children} <> rest:
      +id = M.Thread.id(t)
      +indent = Bool.pick(Nat, Nat.is_gt(depth, 4n), 4n, depth)
      List.concat(&2, D.Node, [[D.El{"div", "tree:" ++ id, [D.At{"class", "thread-tree-row"}, D.At{"style", "--depth:" ++ Nat.show(indent)}],
        [View.tree.expander(id, expanded, children), View.row.s(lc, t, ix, st, sel, now, Bool.and(movable, Nat.is_eq(depth, 0n)), M.Snooze.on(now, U.Side.until(st, id)), Bool.and(live, U.Side.settled(t)))]}],
        View.tree.create(Bool.and(expanded, U.ThreadTree.can_create(st, id)), id, indent), View.tree.rows.raw(lc, rest, ix, st, sel, now, movable, live)])

def View.tree.rows(+lc: I.Loc, +rs: List<&2, ST.TRow>, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat, +movable: Bool, +live: Bool) -> List<&2, D.Node>:
  +p = VP.Page.of(st, Bool.pick(String, live, "live", "threads"), ST.Tree.row_ids(rs), sel)
  List.append(&2, D.Node, View.tree.rows.raw(lc, LW.Lists.tree(rs, VP.Page.ids(p)), ix, st, sel, now, movable, live), LW.Lists.pager(p))

def View.rows_in(+lc: I.Loc, ts: List<&2, M.Thread>, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat, +movable: Bool, +snoozed: Bool) -> List<&2, D.Node>:
  View.tree.rows(lc, ST.Tree.rows(st, ts, sel), ix, st, sel, now, movable, movable)

def View.rows_of(+lc: I.Loc, ts: List<&2, M.Thread>, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat) -> List<&2, D.Node>:
  View.rows_in(lc, ts, ix, st, sel, now, False{}, False{})

# a shut shelf, with the open thread's row if it is on it (U.Shelf.words)
def View.shelf.shut(+lc: I.Loc, +key: String, +label: String, +shown: List<&2, M.Thread>, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat) -> List<&2, D.Node>:
  WI.WIcon.button("shelf", "toggle-settled", key, MSG.T.common.show(lc), "chevron-right", 14, U.Shelf.words(lc, label, List.length(&2, M.Thread, shown))) <> View.rows_of(lc, shown, ix, st, sel, now)

def View.shelf.k(+lc: I.Loc, open: Bool, +key: String, +label: String, ts: List<&2, M.Thread>, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat) -> List<&2, D.Node>:
  match open:
    case True{}:
      WI.WIcon.button("shelf open", "toggle-settled", key, MSG.T.common.hide(lc), "chevron-down", 14, label) <> View.rows_of(lc, ts, ix, st, sel, now)
    case False{}:
      View.shelf.shut(lc, key, label, U.Side.sel(ts, sel), ix, st, sel, now)

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
         D.Dom.button("", "look", "", "Close icon picker", "Close"),
         D.Dom.button("remove", "proj-remove", id, MSG.T.sidebar.remove.note(lc), Bool.pick(String, rm, MSG.T.menu.remove.confirm(lc), MSG.T.menu.project.remove(lc)))]}]
    case False{}:
      Nil{}

def View.look(open: Bool, +st: M.State, +id: String, +rm: Bool) -> List<&2, D.Node>:
  View.look.loc(I.En{}, open, st, id, rm)

# A project whose folder is gone (core/roots.bend): its sidebar head says so
# (the hover names the path), and its threads say what to do above the
# composer, before a message fails with "its folder ... is gone"
def View.gone.title(missing: Bool, +root: String) -> String:
  Bool.pick(String, missing, "Folder missing: " ++ root ++ " (moved, renamed or deleted)", root)

def View.gone.chip(missing: Bool) -> List<&2, D.Node>:
  match missing:
    case True{}: [D.Dom.span("missing-note", "folder missing")]
    case False{}: Nil{}

def View.gone.k(missing: Bool, +lc: I.Loc, +root: String, +pid: String, +armed: Bool, +typed: String) -> List<&2, D.Node>:
  match missing:
    case True{}:
      [D.El{"div", "gone-note", [D.At{"class", "gone-note"}, D.At{"role", "status"}], [
        D.El{"strong", "", Nil{}, [D.Txt{"This project's folder is missing"}]},
        D.El{"p", "", Nil{}, [D.Txt{"Backplane can't find " ++ root ++ ": it was moved, renamed or deleted, and agents can't start here until it is back at that path. If it moved, type where it is now; its threads come with it."}]},
        D.El{"div", "", [D.At{"class", "gone-move"}], [
          D.El{"input", "gone-path", [D.At{"id", "gone-path"}, D.At{"class", "field"}, D.At{"type", "text"}, D.At{"autocomplete", "off"}, D.At{"spellcheck", "false"},
            D.At{"aria-label", "The folder's new place"}, D.At{"placeholder", "/full/path/to/the/folder"}, D.At{"value", typed},
            D.At{"data-bfield", "mv"}, D.On{"input", "bfield"}, D.At{"data-enter", "ws-proj-move"}, D.At{"data-value", pid}], Nil{}},
          D.Dom.button("primary", "ws-proj-move", pid, "Point this project at that folder", "Use this folder")]},
        D.Dom.button("", "proj-remove", pid, MSG.T.sidebar.remove.note(lc), Bool.pick(String, armed, MSG.T.menu.remove.confirm(lc), MSG.T.menu.project.remove(lc)))]}]
    case False{}: Nil{}

# (a thread elsewhere is its own hub's to judge: none)
def View.gone(+lc: I.Loc, +st: M.State, +id: String, +dr: Map<&2, String>) -> List<&2, D.Node>:
  +pid = U.Ui.thread_project(U.Ui.thread(st, id))
  View.gone.k(Bool.and(Bool.not(FR.Fr.is(id)), ROOTS.Roots.missing(st, pid)), lc, U.Ui.project_root(st, pid), pid, String.eq(U.Str.get(dr, "@rm"), pid), U.Str.get(dr, "@f.mv"))

def View.project(+lc: I.Loc, r: U.SRow, +quiet: Bool, +open: Map<&2, Bool>, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat, +look: String, +rm: String) -> D.Node:
  match r:
    case U.SRow{_, M.Project{+id, title, root, _, _}, M.Shelves{pinned, active, settled, hidden, snoozed}}:
      D.El{"section", id, [D.At{"class", Bool.pick(String, Bool.or(quiet, U.Label.syncing(st, id)), Bool.pick(String, U.Label.syncing(st, id), "project quiet syncing", "project quiet"), "project") ++ Bool.pick(String, ROOTS.Roots.missing(st, id), " missing", "")}, D.At{"style", "--tint:" ++ BG.Badge.css_of(st, id)}, D.At{"data-drag-to", id}, D.At{"data-drag-act", "reorder-project"}],
        List.concat(&2, D.Node, [[D.El{"div", "", [D.At{"class", "project-head" ++ Bool.pick(String, String.eq(sel, PH.Home.owner(id)), " on", "")}, D.At{"title", View.gone.title(ROOTS.Roots.missing(st, id), root)}, D.At{"draggable", "true"}, D.At{"data-drag", id}, D.At{"data-drag-act", "reorder-project"}, D.At{"data-value", id}, D.On{"contextmenu", "proj-menu"}],
            List.concat(&2, D.Node, [[WI.WIcon.button("icon project-disclosure", "toggle-settled", "project-closed:" ++ id, "Expand or collapse project threads", Bool.pick(String, U.Flag.get(open, "project-closed:" ++ id), "chevron-right", "chevron-down"), 14, ""), WI.WIcon.button("badge", "look", id, MSG.T.menu.look(lc), BG.Badge.icon(st, id), 14, ""), D.Dom.button("name project-home-link", "proj-home", id, "Open project home", U.Label.title(st, id, title))], View.gone.chip(ROOTS.Roots.missing(st, id)), [WI.WIcon.button("icon", "new-thread", id, MSG.T.thread.new(lc), "plus", 16, "")]])}],
          View.look.loc(lc, Bool.and(String.eq(look, id), Bool.not(PH.Home.is(sel))), st, id, String.eq(rm, id)),
          [D.El{"div", "", [D.At{"class", "project-threads"}, D.At{"style", Bool.pick(String, U.Flag.get(open, "project-closed:" ++ id), "display:none", "display:contents")}], List.concat(&2, D.Node, [View.rows_in(lc, pinned, ix, st, sel, now, True{}, False{}),
          View.rows_in(lc, active, ix, st, sel, now, True{}, False{}),
          View.snoozed(lc, snoozed, ix, st, sel, now),
          View.shelf(lc, U.Flag.get(open, id), id, MSG.T.notify.settled(lc), settled, ix, st, sel, now),
          View.shelf(lc, U.Flag.get(open, "arch:" ++ id), "arch:" ++ id, MSG.T.sidebar.archived(lc), M.Archive.of(hidden), ix, st, sel, now)])}]])}

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
      View.projects(lc, U.Side.older.sel(rs, U.Side.cur.of(st, sel)), True{}, open, ix, st, sel, now, look, rm)

# the older projects (quiet for Quiet.days) folded under one row; a search
# shows them in place. Shut, it still shows the open thread's project, and
# says so ("Older 7 · 1 shown": U.Side.older.sel, U.Shelf.words)
def View.older.k(+lc: I.Loc, searching: Bool, +n: Nat, +rs: List<&2, U.SRow>, +open: Map<&2, Bool>, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat, +look: String, +rm: String) -> List<&2, D.Node>:
  match searching:
    case True{}:
      View.projects(lc, rs, True{}, open, ix, st, sel, now, look, rm)
    case False{}:
      WI.WIcon.button(Bool.pick(String, U.Flag.get(open, "old-projects"), "shelf older open", "shelf older"), "side-older", "", MSG.T.sidebar.quiet.note(lc),
          Bool.pick(String, U.Flag.get(open, "old-projects"), "chevron-down", "chevron-right"), 14,
          Bool.pick(String, U.Flag.get(open, "old-projects"), MSG.T.sidebar.older(lc, Nat.show(n)),
            U.Shelf.words(lc, MSG.T.sidebar.older(lc, Nat.show(n)), List.length(&2, U.SRow, U.Side.older.sel(rs, U.Side.cur.of(st, sel))))))
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
      +tt = U.Label.title(st, M.Thread.id(t), TTL.Title.shown(lc, U.Active.title(t)))
      +dot = SG.Signal.dot.thread(SG.Signal.display.name(U.Side.signal.ix(ix, t), st, M.Thread.id(t)), t)
      D.El{"div", M.Thread.id(t), [D.At{"class", Bool.pick(String, String.eq(M.Thread.id(t), sel), "thread on", Bool.pick(String, faded, "thread faded", "thread"))},
          D.At{"role", "button"}, D.At{"tabindex", "0"}, D.At{"aria-current", Bool.pick(String, String.eq(M.Thread.id(t), sel), "page", "false")},
          D.At{"style", "--tint:" ++ BG.Badge.css_of(st, pid)}, D.At{"data-shortcut-project", pid}, D.At{"data-value", M.Thread.id(t)}, D.On{"click", "select"}, D.On{"contextmenu", "row-menu"}],
        List.concat(&2, D.Node, [[View.dot(lc, dot), D.El{"span", "", [D.At{"class", "title"}, D.At{"title", tt}], [D.Txt{tt}]}], View.pin(lc, U.Active.pinned(t)),
          [View.note.want(U.Side.want(lc, dot, U.SIx.open(ix, M.Thread.id(t)), t), SG.Signal.display.note(lc, st, M.Thread.id(t), View.sub.badge(lc, SB.Subs.count.ix(ix, st, M.Thread.id(t)))), pt)]])}

def View.atree.step(root: Bool, anchors: List<&2, U.ARow>, fallback: U.ARow) -> List<&2, U.ARow> & U.ARow:
  match root anchors:
    case True{} a <> rest: (rest, a)
    case _ xs: (xs, fallback)

def View.atree.card(+lc: I.Loc, +t: M.Thread, anchor: U.ARow, +ix: U.SIx, +st: M.State, +sel: String) -> D.Node:
  match anchor:
    case U.ARow{_, _, _, pid, pt, faded}: View.arow(lc, U.ARow{0n, 0n, t, pid, pt, faded}, ix, st, sel)

type ATreeStep is Data:
  ATreeStep{next: List<&2, U.ARow>, anchor: U.ARow}

def View.atree.data(pair: List<&2, U.ARow> & U.ARow) -> ATreeStep:
  (next, anchor) = pair
  ATreeStep{next, anchor}

def View.atree.next(step: ATreeStep) -> List<&2, U.ARow>:
  match step:
    case ATreeStep{next, _}: next

def View.atree.anchor(step: ATreeStep) -> U.ARow:
  match step:
    case ATreeStep{_, anchor}: anchor

def View.atree.row(show: Bool, +lc: I.Loc, +t: M.Thread, +depth: Nat, +expanded: Bool, +children: Nat, +at: U.ARow, +ix: U.SIx, +st: M.State, +sel: String, tail: List<&2, D.Node>) -> List<&2, D.Node>:
  match show:
    case False{}: tail
    case True{}:
      +id = M.Thread.id(t)
      +indent = Bool.pick(Nat, Nat.is_gt(depth, 4n), 4n, depth)
      List.concat(&2, D.Node, [[D.El{"div", "tree:" ++ id, [D.At{"class", "thread-tree-row"}, D.At{"style", "--depth:" ++ Nat.show(indent)}],
        [View.tree.expander(id, expanded, children), View.atree.card(lc, t, at, ix, st, sel)]}],
        View.tree.create(Bool.and(expanded, U.ThreadTree.can_create(st, id)), id, indent), tail])

def View.atree.rows.raw(rs: List<&2, ST.TRow>, +lc: I.Loc, anchors: List<&2, U.ARow>, anchor: U.ARow, +ix: U.SIx, +st: M.State, +sel: String, +ids: List<&2, String>) -> List<&2, D.Node>:
  match rs:
    case Nil{}: Nil{}
    case ST.TRow{+t, +depth, +expanded, +children} <> rest:
      +step = View.atree.data(View.atree.step(Nat.is_eq(depth, 0n), anchors, anchor))
      +at = View.atree.anchor(step)
      View.atree.row(VP.Page.has(ids, M.Thread.id(t)), lc, t, depth, expanded, children, at, ix, st, sel,
        View.atree.rows.raw(rest, lc, View.atree.next(step), at, ix, st, sel, ids))

def View.atree.rows(+rs: List<&2, ST.TRow>, +lc: I.Loc, anchors: List<&2, U.ARow>, anchor: U.ARow, +ix: U.SIx, +st: M.State, +sel: String, +kind: String) -> List<&2, D.Node>:
  +p = VP.Page.of(st, kind, ST.Tree.row_ids(rs), sel)
  List.append(&2, D.Node, View.atree.rows.raw(rs, lc, anchors, anchor, ix, st, sel, VP.Page.ids(p)), LW.Lists.pager(p))

def View.arows(+lc: I.Loc, rs: List<&2, U.ARow>, +ix: U.SIx, +st: M.State, +sel: String, +q: String, +kind: String) -> List<&2, D.Node>:
  match rs:
    case Nil{}: Nil{}
    case +r <> +rest:
      View.atree.rows(ST.Tree.rows_search(st, U.Active.root_threads(r <> rest), sel, q), lc, r <> rest, r, ix, st, sel, kind)

def View.afaded.open(+lc: I.Loc, on: Bool, rs: List<&2, U.ARow>, +ix: U.SIx, +st: M.State, +sel: String, +q: String) -> List<&2, D.Node>:
  match on:
    case True{}:
      View.arows(lc, rs, ix, st, sel, q, "active")
    case False{}:
      View.arows(lc, U.Active.sel(rs, sel), ix, st, sel, q, "active")

# the settled rows folded under one "Settled N" row, shown while it is
# open or a search is on (core's Active.open)
def View.afaded.k(+lc: I.Loc, searching: Bool, +open: Bool, +n: Nat, +rs: List<&2, U.ARow>, +ix: U.SIx, +st: M.State, +sel: String) -> List<&2, D.Node>:
  match searching:
    case True{}:
      View.arows(lc, rs, ix, st, sel, "search", "active")
    case False{}:
      WI.WIcon.button(Bool.pick(String, open, "shelf older open", "shelf older"), "side-settled", "", MSG.T.sidebar.settled.note(lc),
          Bool.pick(String, open, "chevron-down", "chevron-right"), 14, Bool.pick(String, open, MSG.T.sidebar.settled.count(lc, Nat.show(n)), U.Shelf.words(lc, MSG.T.sidebar.settled.count(lc, Nat.show(n)), List.length(&2, U.ARow, U.Active.sel(rs, sel)))))
        <> View.afaded.open(lc, open, rs, ix, st, sel, "")

def View.afaded(+lc: I.Loc, rs: List<&2, U.ARow>, +q: String, +open: Map<&2, Bool>, +ix: U.SIx, +st: M.State, +sel: String) -> List<&2, D.Node>:
  match rs:
    case Nil{}:
      Nil{}
    case +r <> +t:
      View.afaded.k(lc, Bool.not(String.is_empty(q)), U.Flag.get(open, "active-settled"), List.length(&2, U.ARow, r <> t), r <> t, ix, st, sel)

def View.active.k(+lc: I.Loc, p: U.AParts, +q: String, +open: Map<&2, Bool>, +ix: U.SIx, +st: M.State, +sel: String) -> List<&2, D.Node>:
  match p:
    case U.AParts{l, f}:
      [D.El{"section", "", [D.At{"class", "project active"}], List.append(&2, D.Node, View.arows(lc, l, ix, st, sel, q, "live"), View.afaded(lc, f, q, open, ix, st, sel))}]

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
      View.groups(lc, U.Side.groups.at(U.Find.rows(U.Side.rows.ix.at(st, ix, now, U.Side.cur.held(st, sel, now)), q), now, U.Side.cur.held(st, sel, now)), q, open, ix, st, sel, now, look, rm)

# (what every row asks of the whole state is gathered once: core's Side.ix)
def View.history.rows(+lc: I.Loc, ts: List<&2, M.Thread>, +q: String, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat) -> List<&2, D.Node>:
  match ts:
    case Nil{}: Nil{}
    case +t <> rest:
      D.Dom.cat(D.Dom.when(Bool.or(String.is_empty(q), U.Palette.hit(q, U.Active.title(t))), View.row(lc, t, ix, st, sel, now, False{}, False{})), View.history.rows(lc, rest, q, ix, st, sel, now))

def View.history.open(show: Bool, +lc: I.Loc, ts: List<&2, M.Thread>, q: String, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat) -> List<&2, D.Node>:
  match show:
    case True{}: View.history.rows(lc, ts, q, ix, st, sel, now)
    case False{}: Nil{}

def View.history.k(ts: List<&2, M.Thread>, +lc: I.Loc, +q: String, +open: Map<&2, Bool>, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat) -> List<&2, D.Node>:
  match ts:
    case Nil{}: Nil{}
    case +t <> +rest:
      [D.El{"section", "conversation-history", [D.At{"class", "conversation-history"}],
        WI.WIcon.button("shelf", "thread-history", "", "Previous conversations", Bool.pick(String, U.Flag.get(open, "conversation-history"), "chevron-down", "chevron-right"), 14, "Conversation history") <>
          View.history.open(Bool.or(U.Flag.get(open, "conversation-history"), Bool.not(String.is_empty(q))), lc, t <> rest, q, ix, st, sel, now)}]

def View.history.live(ts: List<&2, M.Thread>, +lc: I.Loc, +q: String, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat) -> List<&2, D.Node>:
  match ts:
    case Nil{}: Nil{}
    case t <> rest:
      [D.El{"section", "legacy-live", [D.At{"class", "conversation-history legacy-live"}],
        D.Dom.span("shelf-label", "Conversations in progress") <> View.history.rows(lc, t <> rest, q, ix, st, sel, now)}]

def View.history(+lc: I.Loc, +q: String, +open: Map<&2, Bool>, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat) -> List<&2, D.Node>:
  List.append(&2, D.Node, View.history.live(U.Side.legacy.filtered(ST.Legacy.live(st, sel), q), lc, q, ix, st, sel, now),
    View.history.k(ST.Legacy.history(st, sel), lc, q, open, ix, st, sel, now))

def View.list(+lc: I.Loc, active: Bool, +q: String, +open: Map<&2, Bool>, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat, +look: String, +rm: String) -> List<&2, D.Node>:
  List.append(&2, D.Node, View.list.k(lc, active, q, open, ix, st, sel, now, look, rm), View.history(lc, q, open, ix, st, sel, now))

# the search's field: with a query, Esc clears it (client.bend's
# Find.toggle); empty, Esc is the page's
def View.find.esc(+q: String, n: D.Node) -> D.Node:
  match n:
    case D.El{t, k, as, ks}:
      D.El{t, k, List.append(&2, D.Attr, as, [D.At{"data-esc", Bool.pick(String, String.is_empty(q), "", "proj-find")}]), ks}
    case other:
      other

def View.find.field(+lc: I.Loc, +active: Bool, +q: String) -> D.Node:
  View.find.esc(q, WB.WB.input("proj-find", "pq", q, Bool.pick(String, active, MSG.T.palette.search(lc), MSG.T.sidebar.search.projects(lc)), "proj-go"))

# the search, always at the top of the list
def View.find.loc(+lc: I.Loc, +active: Bool, +q: String) -> List<&2, D.Node>:
  [D.El{"div", "", [D.At{"class", "find"}], [View.find.field(lc, active, q)]}]

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
      List.append(&2, D.Node, View.mlist(open, rs), [WI.WIcon.button("machine-pick", "flag", "machines", MSG.T.sidebar.machine(lc), "chevron-down", 14, U.Machines.current(rs))])
    case False{}:
      Nil{}

def View.machines(many: Bool, +open: Bool, +rs: List<&2, U.MRow>) -> List<&2, D.Node>:
  View.machines.loc(I.En{}, many, open, rs)

# the head's last button folds the sidebar to a strip (host.js, kept in
# this browser; Ctrl+B too)
def View.sidebar.head(top: Bool, +lc: I.Loc, +ms: List<&2, U.MRow>, +open: Map<&2, Bool>, +on: Bool, v: String) -> List<&2, D.Node>:
  match top:
    case True{}:
      D.Dom.when(Nat.is_gt(List.length(&2, U.MRow, ms), 1n), D.El{"div", "", [D.At{"class", "sidebar-tools"}], View.machines.loc(lc, True{}, U.Flag.get(open, "machines"), ms)})
    case False{}:
      [D.El{"div", "", [D.At{"class", "brand"}], [
            D.El{"span", "", [D.At{"class", "brand-mark"}], [D.El{"span", "", [D.At{"class", Bool.pick(String, on, "conn on", "conn")},
              D.At{"role", "img"}, D.At{"title", MSG.T.native.hub.state(lc, Bool.pick(String, on, "online", "offline"))},
              D.At{"aria-label", MSG.T.native.hub.state(lc, Bool.pick(String, on, "online", "offline"))}], Nil{}}]},
            D.Dom.span("name", "Backplane"), D.Dom.span("ver", v),
            WI.WIcon.button("icon", "thin-search-open", "", "Search conversations", "search", 16, ""),
            WI.WIcon.button("icon", "proj-open", "", MSG.T.sidebar.add.project(lc), "plus", 16, ""),
            WI.WIcon.button("icon side-toggle", "side-toggle", "", "Hide or show the sidebar · Ctrl+B", "panel-right", 16, "")]},
          D.El{"div", "", [D.At{"class", "sidebar-tools"}], WI.WIcon.button("settings-link", "flag", "settings", MSG.T.palette.settings(lc), "settings", 16, MSG.T.palette.settings(lc))
              <> View.machines.loc(lc, Nat.is_gt(List.length(&2, U.MRow, ms), 1n), U.Flag.get(open, "machines"), ms)}]

# Under the top bar the sidebar starts with its own fold, a chevron like
# the rail's (host.js's side-toggle, as the strip's chevron and Ctrl+B
# do), then its filter and the add-project + beside it. The fold comes
# first so it stays where it is when the sidebar folds: the strip's
# unfold chevron sits on the same spot (style.css); else the filter
# follows the brand, Settings and the uberbot
def View.sidebar.find(top: Bool, +lc: I.Loc, +active: Bool, +q: String) -> List<&2, D.Node> & List<&2, D.Node>:
  match top:
    case True{}:
      ([D.El{"div", "", [D.At{"class", "find with-add"}], [WI.WIcon.button("icon side-fold", "side-toggle", "", "Fold the sidebar · Ctrl+B", "chevron-left", 16, ""),
        View.find.field(lc, active, q),
        WI.WIcon.button("icon add-project", "proj-open", "", MSG.T.sidebar.add.project(lc), "plus", 16, "")]}], Nil{})
    case False{}:
      (Nil{}, View.find.loc(lc, active, q))

def View.sidebar.top(found: List<&2, D.Node> & List<&2, D.Node>, head: List<&2, D.Node>, uber: List<&2, D.Node>, add: List<&2, D.Node>, rest: List<&2, D.Node>) -> List<&2, D.Node>:
  (first, later) = found
  D.Dom.cat(first, D.Dom.cat(head, D.Dom.cat(uber, D.Dom.cat(add, D.Dom.cat(later, rest)))))

# The sidebar's foot under the top bar: the keys to move between threads
# and "/" back to the composer, shown while Ctrl is held (R05,
# style.css); the last row opens the command palette, which lists them all
def View.legend.word(lc: I.Loc, +k: String) -> String:
  match lc:
    case I.Ja{}:
      Bool.pick(String, String.eq(k, "move"), "前／次のスレッド", Bool.pick(String, String.eq(k, "new"), "ここに新しいスレッド", Bool.pick(String, String.eq(k, "jump"), "ここのスレッドへ移動", Bool.pick(String, String.eq(k, "all"), "すべてのショートカット", Bool.pick(String, String.eq(k, "type"), "入力欄へ", "キーボードショートカット")))))
    case _:
      Bool.pick(String, String.eq(k, "move"), "prev / next thread", Bool.pick(String, String.eq(k, "new"), "new thread here", Bool.pick(String, String.eq(k, "jump"), "jump to a thread here", Bool.pick(String, String.eq(k, "all"), "all shortcuts", Bool.pick(String, String.eq(k, "type"), "write a message", "Keyboard shortcuts")))))

def View.legend.keys(ks: List<&2, String>) -> List<&2, D.Node>:
  match ks:
    case Nil{}: Nil{}
    case k <> t: D.El{"kbd", "", Nil{}, [D.Txt{k}]} <> View.legend.keys(t)

def View.legend.row(ks: List<&2, String>, +what: String) -> List<&2, D.Node>:
  [D.El{"span", "", [D.At{"class", "keys"}], View.legend.keys(ks)}, D.Dom.span("what", what)]

def View.sidebar.legend(top: Bool, +lc: I.Loc) -> List<&2, D.Node>:
  match top:
    case True{}:
      [D.El{"div", "keys-legend", [D.At{"class", "keys-legend"}, D.At{"role", "note"}, D.At{"aria-label", View.legend.word(lc, "")}], List.concat(&2, D.Node, [
        View.legend.row(["Ctrl ⇧", "[", "]"], View.legend.word(lc, "move")),
        View.legend.row(["Ctrl", "N"], View.legend.word(lc, "new")),
        View.legend.row(["Ctrl", "1–9"], View.legend.word(lc, "jump")),
        View.legend.row(["/"], View.legend.word(lc, "type")),
        [D.El{"button", "", [D.At{"class", "legend-all"}, D.At{"type", "button"}, D.At{"title", "Command palette · Ctrl+K"}, D.At{"data-value", ""}, D.On{"click", "ws-palette-open"}],
          View.legend.row(["Ctrl", "K"], View.legend.word(lc, "all"))}]])}]
    case False{}:
      Nil{}

def View.sidebar.no_matches(+lc: I.Loc, +q: String, empty: Bool) -> List<&2, D.Node>:
  match empty:
    case False{}: Nil{}
    case True{}:
      D.Dom.when(Bool.not(String.is_empty(String.trim(q))), D.El{"div", "", [D.At{"class", "project-no-matches"}],
        [D.Dom.span("dim", MSG.T.sidebar.search.empty(lc)), D.Dom.button("", "proj-find", "off", MSG.T.sidebar.search.clear(lc), MSG.T.sidebar.search.clear(lc))]})

def View.sidebar.body.k(+top: Bool, +lc: I.Loc, +ms: List<&2, U.MRow>, +ix: U.SIx, +st: M.State, +sel: String, +open: Map<&2, Bool>, +on: Bool, +now: Nat, adding: Bool, v: String, +look: String, +rm: String, +q: String, mi: List<&2, U.MItem>, bots: List<&2, D.Node>, uber: List<&2, D.Node>, items: List<&2, D.Node>) -> D.Node:
  D.El{"aside", "sidebar", [D.At{"class", "sidebar"}],
    View.sidebar.top(View.sidebar.find(top, lc, String.eq(U.Side.view(st), "active"), q), View.sidebar.head(top, lc, ms, open, on, v), uber, View.add.loc(lc, adding),
      D.Dom.cat(items, D.Dom.cat(bots, D.Dom.cat(View.sidebar.legend(top, lc), View.rmenu(mi)))))}

def View.sidebar.body(+lc: I.Loc, +ms: List<&2, U.MRow>, +ix: U.SIx, +st: M.State, +sel: String, +open: Map<&2, Bool>, +on: Bool, +now: Nat, adding: Bool, v: String, +look: String, +rm: String, +q: String, mi: List<&2, U.MItem>, bots: List<&2, D.Node>, uber: List<&2, D.Node>, items: List<&2, D.Node>) -> D.Node:
  View.sidebar.body.k(False{}, lc, ms, ix, st, sel, open, on, now, adding, v, look, rm, q, mi, bots, uber, items)

def View.sidebar.k(+lc: I.Loc, +ms: List<&2, U.MRow>, +ix: U.SIx, +st: M.State, +sel: String, +open: Map<&2, Bool>, +on: Bool, +now: Nat, adding: Bool, v: String, +look: String, +rm: String, +q: String, mi: List<&2, U.MItem>, bots: List<&2, D.Node>, uber: List<&2, D.Node>) -> D.Node:
  View.sidebar.body(lc, ms, ix, st, sel, open, on, now, adding, v, look, rm, q, mi, bots, uber,
    View.list(lc, String.eq(U.Side.view(st), "active"), q, open, ix, st, sel, now, look, rm))

def View.sidebar.k.top(+top: Bool, +lc: I.Loc, +ms: List<&2, U.MRow>, +ix: U.SIx, +st: M.State, +sel: String, +open: Map<&2, Bool>, +on: Bool, +now: Nat, adding: Bool, v: String, +look: String, +rm: String, +q: String, mi: List<&2, U.MItem>, bots: List<&2, D.Node>, uber: List<&2, D.Node>) -> D.Node:
  View.sidebar.body.k(top, lc, ms, ix, st, sel, open, on, now, adding, v, look, rm, q, mi, bots, uber,
    View.list(lc, String.eq(U.Side.view(st), "active"), q, open, ix, st, sel, now, look, rm))

def View.sidebar.w(+lc: I.Loc, bots: List<&2, D.Node>, uber: List<&2, D.Node>, ms: List<&2, U.MRow>, mi: List<&2, U.MItem>, +ix: U.SIx, +u: U.Ui) -> D.Node:
  match u:
    case U.Ui{+st, +sel, +d, _, +open, +on, +now, +adding, _, +v, _, _, _, _, _, _, _}:
      View.sidebar.k.top(View.chrome(u), lc, ms, ix, st, sel, open, on, now, adding, v, U.Str.get(d, "@look"), U.Str.get(d, "@rm"), U.Find.q(open, d), mi, bots, uber)

# the sidebar's index (Side.ix: every thread and ask of the state) is made
# once a frame, for the list and the uberbot's counts both
# Export the exact digit destinations; the host only reveals their numbers.
# Use the same Side lists as WS.key.digit.valid, including all current folds.
def View.sidebar.keys(n: D.Node, projects: List<&2, String>, threads: List<&2, String>) -> D.Node:
  match n:
    case D.El{tag, key, attrs, kids}:
      D.El{tag, key, List.append(&2, D.Attr, attrs,
        [D.At{"data-shortcut-projects", J.Json.show(J.Json.arr(U.Side.strs(List.take(&2, String, projects, 9n))))},
         D.At{"data-shortcut-threads", J.Json.show(J.Json.arr(U.Side.strs(List.take(&2, String, threads, 9n))))}]), kids}
    case D.Txt{text}: D.Txt{text}

# Share fresh, unfiltered project/shelf rows within this frame, including
# Active mode. Filtering and phase decoration remain separate for display
# and shortcut destinations. The old helpers above remain the oracle.
def View.list.projected(+lc: I.Loc, active: Bool, +rows: List<&2, U.SRow>, raw: List<&2, U.SRow>, +q: String, +open: Map<&2, Bool>, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat, +look: String, +rm: String) -> List<&2, D.Node>:
  match active:
    case True{}:
      List.append(&2, D.Node, View.active(lc, U.Active.rows.given(st, ix, now, q, raw), q, open, ix, st, sel), View.history(lc, q, open, ix, st, sel, now))
    case False{}:
      List.append(&2, D.Node, View.groups(lc, U.Side.groups.at(rows, now, U.Side.cur.held(st, sel, now)), q, open, ix, st, sel, now, look, rm), View.history(lc, q, open, ix, st, sel, now))

# the sidebar's Active list (setting sidebar.view): a phone's page keeps the
# projects in its Projects tab, its Now tab being the Active list
def View.side.active(+st: M.State, +u: U.Ui) -> Bool:
  Bool.and(String.eq(U.Side.view(st), "active"), Bool.not(PHN.Phone.on(u)))

def View.sidebar.projected(+lc: I.Loc, bots: List<&2, D.Node>, uber: List<&2, D.Node>, ms: List<&2, U.MRow>, mi: List<&2, U.MItem>, +ix: U.SIx, +u: U.Ui, +rows: List<&2, U.SRow>, raw: List<&2, U.SRow>) -> D.Node:
  match u:
    case U.Ui{+st, +sel, +d, _, +open, +on, +now, +adding, _, +v, _, _, _, _, _, _, _}:
      View.sidebar.body.k(View.chrome(u), lc, ms, ix, st, sel, open, on, now, adding, v, U.Str.get(d, "@look"), U.Str.get(d, "@rm"), U.Find.q(open, d), mi, bots, uber,
        D.Dom.cat(View.sidebar.no_matches(lc, U.Find.q(open, d), Bool.and(Bool.not(View.side.active(st, u)), List.is_empty(&2, U.SRow, rows))), View.list.projected(lc, View.side.active(st, u), rows, raw, U.Find.q(open, d), open, ix, st, sel, now, U.Str.get(d, "@look"), U.Str.get(d, "@rm"))))

def View.sidebar.threads.kind(active: Bool, +u: U.Ui, ix: U.SIx, rows: List<&2, U.SRow>, raw: List<&2, U.SRow>) -> List<&2, String>:
  match active:
    case True{}:
      +q = U.Ui.scratch(u, "@f.pq")
      ST.Tree.row_ids(ST.Tree.rows_search(U.Ui.state(u), U.Active.root_threads(U.Side.active.project(
        U.Active.folded(U.Active.open(U.Side.open(u), q), U.Active.parts(U.Active.rows.given(U.Ui.state(u), ix, U.Side.now(u), q, raw))), U.Side.cur(u))), U.Ui.sel(u), q))
    case False{}: U.Side.visible.tree(U.Side.only(rows, U.Side.cur(u)), U.Side.open(u), U.Ui.state(u), U.Ui.sel(u))

def View.sidebar.threads(+u: U.Ui, ix: U.SIx, rows: List<&2, U.SRow>, raw: List<&2, U.SRow>) -> List<&2, String>:
  List.append(&2, String, View.sidebar.threads.kind(View.side.active(U.Ui.state(u), u), u, ix, rows, raw),
    U.Side.in_project(U.Side.legacy.shown(u), U.Ui.state(u), U.Side.cur(u)))

# Folded (Ctrl+B or the sidebar's own chevron: host.js
# keeps it, style.css shows it), the sidebar is a 44 px strip: the chevron
# that unfolds it, pointing out, then each project's badge in its colour,
# which opens the project (its thread, as Alt+digit does); the open
# thread's project is marked
# a project's face in the strip: the icon picked for it, else its initials
# (core's Badge.initials), so projects left on the folder differ by more
# than their colour
def View.strip.face(picked: Bool, +st: M.State, +p: String) -> D.Node:
  match picked:
    case True{}: WI.WIcon.svg(BG.Badge.icon(st, p), 16)
    case False{}: D.Dom.span("ini", BG.Badge.initials(U.Ui.project_title(st, p)))

def View.strip.projects(ps: List<&2, String>, +st: M.State, +cur: String) -> List<&2, D.Node>:
  match ps:
    case Nil{}:
      Nil{}
    case +p <> t:
      D.El{"button", "strip:" ++ p, [D.At{"class", Bool.pick(String, String.eq(p, cur), "strip-project on", "strip-project")}, D.At{"type", "button"},
        D.At{"style", "--tint:" ++ BG.Badge.css_of(st, p)}, D.At{"title", U.Label.title(st, p, U.Ui.project_title(st, p))}, D.At{"aria-label", U.Label.title(st, p, U.Ui.project_title(st, p))},
        D.At{"data-value", p}, D.On{"click", "ws-project"}], [View.strip.face(BG.Badge.picked(st, p), st, p)]} <> View.strip.projects(t, st, cur)

def View.strip(+lc: I.Loc, ps: List<&2, String>, +st: M.State, +cur: String) -> D.Node:
  D.El{"nav", "side-strip", [D.At{"class", "side-strip"}, D.At{"aria-label", MSG.T.tour.title.projects(lc)}],
    WI.WIcon.button("icon side-toggle", "side-toggle", "", "Unfold the sidebar · Ctrl+B", "chevron-right", 16, "") <> View.strip.projects(ps, st, cur)}

def View.sidebar.strip(n: D.Node, strip: D.Node) -> D.Node:
  match n:
    case D.El{tag, key, attrs, kids}:
      D.El{tag, key, attrs, strip <> kids}
    case D.Txt{text}:
      D.Txt{text}

def View.sidebar.frame.rows(+lc: I.Loc, bots: List<&2, D.Node>, +u: U.Ui, +plain: U.SIx, +signals: U.SIx, +rows: List<&2, U.SRow>, +unfolded: List<&2, U.SRow>, +raw: List<&2, U.SRow>) -> D.Node:
  View.sidebar.strip(View.sidebar.keys(View.sidebar.projected(lc, bots, WB.WB.uber.rows(lc, u, signals, raw), U.Machines.rows.loc(lc, u), U.Menu.items.loc(lc, u, True{}), signals, u, rows, raw),
    U.Side.pids(unfolded), View.sidebar.threads(u, plain, unfolded, raw)), View.strip(lc, U.Side.pids(unfolded), U.Ui.state(u), U.Side.cur(u)))

def View.sidebar.frame.shown.raw(+lc: I.Loc, bots: List<&2, D.Node>, +u: U.Ui, +plain: U.SIx, signals: U.SIx, +rows: List<&2, U.SRow>, raw: List<&2, U.SRow>) -> D.Node:
  View.sidebar.frame.rows(lc, bots, u, plain, signals, rows, U.Side.unfolded.at(rows, U.Side.open(u), U.Ui.scratch(u, "@f.pq"), U.Side.now(u), U.Side.cur.held(U.Ui.state(u), U.Ui.sel(u), U.Side.now(u)), U.Side.cur(u)), raw)

def View.sidebar.frame.project_page(node: D.Node, p: VP.Page) -> D.Node:
  match node:
    case D.El{tag, key, attrs, kids}: D.El{tag, key, attrs, List.append(&2, D.Node, kids, LW.Lists.pager(p))}
    case other: other

def View.sidebar.frame.shown(+lc: I.Loc, bots: List<&2, D.Node>, +u: U.Ui, +plain: U.SIx, signals: U.SIx, +rows: List<&2, U.SRow>, raw: List<&2, U.SRow>) -> D.Node:
  +p = VP.Page.of(U.Ui.state(u), "projects", U.Side.pids(rows), U.Side.cur(u))
  View.sidebar.frame.project_page(View.sidebar.frame.shown.raw(lc, bots, u, plain, signals, LW.Lists.projects(rows, VP.Page.ids(p)), raw), p)

def View.sidebar.frame.raw(+lc: I.Loc, bots: List<&2, D.Node>, +u: U.Ui, +plain: U.SIx, signals: U.SIx, +raw: List<&2, U.SRow>) -> D.Node:
  View.sidebar.frame.shown(lc, bots, u, plain, signals, U.Find.rows(raw, U.Ui.scratch(u, "@f.pq")), raw)

def View.sidebar.frame(+lc: I.Loc, bots: List<&2, D.Node>, +u: U.Ui, +plain: U.SIx, signals: U.SIx) -> D.Node:
  View.sidebar.frame.raw(lc, bots, u, plain, signals, U.Side.rows.ix.at(U.Ui.state(u), plain, U.Side.now(u), U.Side.cur.held(U.Ui.state(u), U.Ui.sel(u), U.Side.now(u))))

def View.sidebar.x(+lc: I.Loc, bots: List<&2, D.Node>, +u: U.Ui, +ix: U.SIx) -> D.Node:
  View.sidebar.frame(lc, bots, u, ix, U.Side.signals.ix(ix, u))

def View.sidebar.indexed(+lc: I.Loc, +u: U.Ui, +ix: U.SIx, signals: Map<&2, Nat>) -> D.Node:
  View.sidebar.frame(lc, WB.WB.side(lc, u), u, ix, U.Side.signals.ix.given(ix, u, signals))

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

def View.pending.node(+lc: I.Loc, +key: String) -> D.Node:
  D.El{"div", key, [D.At{"class", "msg assistant pending"}, D.At{"role", "status"}, D.At{"aria-live", "polite"}],
    [D.El{"div", "", [D.At{"class", "body"}], [D.Txt{CP.Pending.label(lc)}]}]}

def View.pending.rows(+lc: I.Loc, show: Bool, +key: String) -> List<&2, D.Node>:
  match show:
    case True{}: [View.pending.node(lc, key)]
    case False{}: Nil{}

# the reply streaming in (made by the host from its text, View.make)
def View.live.n.loc(+lc: I.Loc, +key: String, +text: String) -> D.Node:
  View.pending.node(lc, key)

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

def View.signal.live.loc(+lc: I.Loc, empty: Bool, +signal: Nat, text: String, +key: String) -> List<&2, D.Node>:
  match empty:
    case False{}:
      [View.memo.loc(lc, key, [D.At{"fn", "live"}], text)]
    case True{}:
      [D.El{"div", key, [D.At{"class", "phase " ++ SG.Signal.name(signal)}],
        [View.dot(lc, SG.Signal.name(signal)), D.Txt{SG.Signal.label(lc, signal)}]}]

def View.signal.of.loc(+lc: I.Loc, +info: Map<&2, String>, +live: Map<&2, String>, +id: String) -> List<&2, D.Node>:
  View.signal.live.loc(lc, String.is_empty(U.Str.get(live, id)), SG.Signal.cached(info, id), U.Str.get(live, id), "live:" ++ id)

# A pending reply is a static bubble. Thin views already have a synthetic
# row once deltas arrive; avoid a second bubble below that row.
def View.thread.live.hide.loc(+lc: I.Loc, hide: Bool, +info: Map<&2, String>, +live: Map<&2, String>, +id: String) -> List<&2, D.Node>:
  match hide:
    case True{}: Nil{}
    case False{}: View.signal.of.loc(lc, info, live, id)

def View.thread.live.loc(+lc: I.Loc, +st: M.State, +info: Map<&2, String>, +live: Map<&2, String>, +id: String) -> List<&2, D.Node>:
  View.pending.rows(lc, Bool.and(CP.Pending.visible(SG.Signal.cached(info, id)), Bool.not(CP.Pending.has(M.State.mine(st, id)))), "live:" ++ id)

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
def View.live.signal.u.loc(+lc: I.Loc, u: U.Ui) -> D.Node:
  match u:
    case U.Ui{+st, +sel, _, +live, _, _, _, _, _, _, _, _, _, _, +nf, _, _}:
      View.one(View.thread.live.loc(lc, st, nf, live, sel))

# Text deltas keep the existing incremental memo path. Derive activity only
# when the live row actually needs to display a status.
def View.live.u.chosen(+lc: I.Loc, empty: Bool, u: U.Ui, +sel: String, +live: Map<&2, String>) -> D.Node:
  match empty:
    case False{}:
      View.memo.loc(lc, "live:" ++ sel, [D.At{"fn", "live"}], U.Str.get(live, sel))
    case True{}:
      View.live.signal.u.loc(lc, U.Side.with_signals(u))

def View.live.u.loc(+lc: I.Loc, +u: U.Ui) -> D.Node:
  match u:
    case U.Ui{_, +sel, _, +live, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      View.live.signal.u.loc(lc, U.Side.with_signals(u))

def View.live.u(u: U.Ui) -> D.Node:
  View.live.u.loc(I.En{}, u)

def View.running(t: M.TurnState) -> Bool:
  match t:
    case M.Running{}:
      True{}
    case _:
      False{}

def View.failed(t: M.TurnState) -> Bool:
  match t:
    case M.Failed{}:
      True{}
    case _:
      False{}

# a header tool (core/tools.bend): its icon and label, its sentence as the
# tooltip; a group starts with a line before it (class "gap")
def View.tool.cls(on: Bool, danger: Bool, gap: Bool) -> String:
  Bool.pick(String, on, "on", "") ++ Bool.pick(String, danger, " danger", "") ++ Bool.pick(String, gap, " gap", "")

def View.tool(t: TO.HeaderTool) -> D.Node:
  match t:
    case TO.HeaderTool{+i, +l, tip, a, v, on, dg, gap}:
      WI.WIcon.button("tool " ++ View.tool.cls(on, dg, gap), a, v, tip, i, 16, l)

def View.tools(ts: List<&2, TO.HeaderTool>) -> List<&2, D.Node>:
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

# Thread management stays in a contextual menu. Actions and confirmation
# states are the same shared tools; Stop stays beside the current work state.
def View.header.tool.label(lc: I.Loc, +action: String, label: String) -> String:
  match lc:
    case I.En{}:
      Bool.pick(String, String.eq(action, "settle"), "Mark settled", Bool.pick(String, String.eq(action, "unsettle"), "Mark active", Bool.pick(String, String.eq(action, "snooze"), "Wake thread", label)))
    case I.Ja{}:
      Bool.pick(String, String.eq(action, "settle"), "完了にする", Bool.pick(String, String.eq(action, "unsettle"), "アクティブにする", Bool.pick(String, String.eq(action, "snooze"), "スレッドを再開", label)))

def View.header.tool(+lc: I.Loc, t: TO.HeaderTool) -> D.Node:
  match t:
    case TO.HeaderTool{i, l, tip, +a, v, on, dg, gap}:
      View.tool(TO.HeaderTool{i, View.header.tool.label(lc, a, l), tip, a, v, on, dg, gap})

def View.header.tools(+lc: I.Loc, ts: List<&2, TO.HeaderTool>) -> List<&2, D.Node>:
  match ts:
    case Nil{}: Nil{}
    case +t <> rest:
      match t:
        case TO.HeaderTool{_, _, _, +a, _, _, _, _}:
          D.Dom.cat(D.Dom.when(Bool.not(Bool.or(String.eq(a, "panel"), Bool.or(String.eq(a, "term-toggle"), String.eq(a, "flag")))), View.header.tool(lc, t)), View.header.tools(lc, rest))

def View.group.label(lc: I.Loc, snooze: Bool) -> String:
  match lc:
    case I.En{}: Bool.pick(String, snooze, "Snooze thread", "Thread")
    case I.Ja{}: Bool.pick(String, snooze, "スヌーズ", "スレッド")

# the thread's menu (the header's ⋯, the top bar's ▾): where it sits in the
# list, fork, a subthread, the snooze choices; open while a delete is armed
def View.thread.actions(+lc: I.Loc, summary: List<&2, D.Node>, +st: M.State, +id: String, settled: Bool, pinned: Bool, snoozed: Bool, archived: Bool, +del: Bool, term: Bool, +panel: Bool) -> D.Node:
  D.El{"details", "thread-actions:" ++ id, List.append(&2, D.Attr, [D.At{"class", "local-menu thread-actions"}], Bool.pick(List<&2, D.Attr>, del, [D.At{"open", ""}], Nil{})), [
    D.El{"summary", "", [D.At{"aria-label", "Thread actions"}, D.At{"title", "Thread actions"}], summary},
    D.El{"div", "", [D.At{"class", "local-menu-body"}], List.concat(&2, D.Node, [
      [D.Dom.button("thread-find", "thread-find", id, "Find what was said in this thread", "Search in thread")],
      View.header.tools(lc, TO.Tools.head.loc(lc, TO.Head{False{}, settled, pinned, snoozed, archived, del,
        Bool.not(String.is_empty(M.Fork.source(st, id))), term, panel}, "flag", "snooze", "term-toggle", "@term-size")),
      D.Dom.when(U.ThreadTree.can_route(st, id), D.Dom.button("", "thread-child", id, "Create a conversation beneath this thread", "+ New subthread")),
      [D.Dom.span("control-label", View.group.label(lc, True{}))], List.take(&2, D.Node, View.snoozes.loc(lc, True{}), 3n)])}]}

def View.header(+lc: I.Loc, t: M.Thread, +st: M.State, +chip: String, mini: List<&2, TO.HeaderTool>, +panel: Bool, snoozed: Bool, picking: Bool, term: Bool, +diff: Bool, +del: Bool, +ren: String, +rtext: String) -> D.Node:
  match t:
    case M.Thread{+id, +p, title, _, _, turn, _, archived, override, +sa, +ua, pinned, _, +branch, _, _, model, _, _, _, _}:
      D.El{"header", "", [D.At{"class", "thread-head"}, D.At{"style", "--tint:" ++ BG.Badge.css_of(st, p)}], List.concat(&2, D.Node, [[
        D.Dom.button("back", "select", "", MSG.T.tour.title.threads(lc), "‹"),
        D.El{"span", "", [D.At{"class", "badge"}], [WI.WIcon.svg(BG.Badge.icon(st, p), 16)]},
        D.El{"div", "", [D.At{"class", "title-box"}], View.title.k.loc(lc, String.eq(ren, id), id, U.Label.title(st, id, TTL.Title.shown(lc, title)), rtext)},
        D.Dom.span("meta", Bool.pick(String, String.is_empty(branch), "", branch))], View.boat.chip.loc(lc, chip), [
        D.El{"div", "", [D.At{"class", "tools thread-controls"}], [
          View.thread.actions(lc, [D.Dom.span("menu-dots", "⋯")], st, id, TO.Tools.settled(override, Nat.is_gt(sa, ua)), pinned, snoozed, archived, del, term, panel)]}]])}

def View.header.legacy(+lc: I.Loc, t: M.Thread, +st: M.State, +chip: String, mini: List<&2, TO.HeaderTool>, +panel: Bool, snoozed: Bool, picking: Bool, term: Bool, +diff: Bool, del: Bool, +ren: String, +rtext: String) -> D.Node:
  match t:
    case M.Thread{+id, +p, title, _, _, turn, _, archived, override, +sa, +ua, pinned, _, +branch, _, _, model, _, _, _, _}:
      D.El{"header", "", [D.At{"class", "thread-head"}, D.At{"style", "--tint:" ++ BG.Badge.css_of(st, p)}], List.concat(&2, D.Node, [[
        D.Dom.button("back", "select", "", MSG.T.tour.title.threads(lc), "‹"),
        D.El{"span", "", [D.At{"class", "badge"}], [WI.WIcon.svg(BG.Badge.icon(st, p), 16)]},
        D.El{"div", "", [D.At{"class", "title-box"}], View.title.k.loc(lc, String.eq(ren, id), id, U.Label.title(st, id, TTL.Title.shown(lc, title)), rtext)},
        D.Dom.span("meta", Bool.pick(String, String.is_empty(branch), "", branch))], View.boat.chip.loc(lc, chip), [
        D.El{"div", "", [D.At{"class", "tools thread-controls"}], D.Dom.span("control-label", View.group.label(lc, False{})) <> List.append(&2, D.Node,
          View.header.tools(lc, List.append(&2, TO.HeaderTool, mini, TO.Tools.head.loc(lc, TO.Head{False{}, TO.Tools.settled(override, Nat.is_gt(sa, ua)), pinned, snoozed, archived, del,
            Bool.not(String.is_empty(M.Fork.source(st, id))), term, panel}, "flag", "snooze", "term-toggle", "@term-size"))),
          D.Dom.span("control-label", View.group.label(lc, True{})) <> List.take(&2, D.Node, View.snoozes.loc(lc, True{}), 3n))}]])}

# The model picker (core/modelpick.bend, client.bend's Mpk.*): three
# columns, agent | model | effort, over the composer. The keys' column
# has its head lit; in each column the cursor's row has the accent edge
# and the thread's own one a faint edge. A click applies at once and the
# picker stays open; the keys move the cursor, Enter applies it.

def View.mp.cls(on: Bool, own: Bool) -> String:
  "row" ++ Bool.pick(String, on, " on", "") ++ Bool.pick(String, own, " own", "")

# the agent column: each agent, what it is dim beside it
def View.mp.agents(+lc: I.Loc, ags: List<&2, String>, +on: String, +own: String) -> List<&2, D.Node>:
  match ags:
    case Nil{}:
      Nil{}
    case +a <> t:
      D.El{"button", "", [D.At{"class", View.mp.cls(String.eq(a, on), String.eq(a, own))}, D.At{"title", MSG.T.model.effort.next(lc, a)}, D.At{"data-value", a}, D.On{"click", "mp-agent"}],
        [D.Dom.span("name", a), D.Dom.span("label", MPK.ModelPick.product(a))]} <> View.mp.agents(lc, t, on, own)

# the model column's rows: each model's name, then what its CLI calls it,
# in two columns (the names' as wide as the longest, --namew)
def View.models.labels.loc(+lc: I.Loc, ms: List<&2, String>, +cur: String, +own: String, +labels: J.Json) -> List<&2, D.Node>:
  match ms:
    case Nil{}:
      Nil{}
    case +m <> t:
      D.El{"button", "", [D.At{"class", View.mp.cls(String.eq(m, cur), String.eq(m, own))}, D.At{"title", MSG.T.model.choose(lc, Bool.pick(String, String.is_empty(m), MSG.T.model.default.article(lc), m))}, D.At{"data-value", m}, D.On{"click", "mp-model"}],
        [D.Dom.span("name", Bool.pick(String, String.is_empty(m), MSG.T.common.default(lc), m)), D.Dom.span("label", U.Models.label.given(labels, m))]} <> View.models.labels.loc(lc, t, cur, own, labels)

# Resolve this render's provider metadata once for the bounded row list.
# The hidden menu remains ready; no catalog or picture is retained here.
def View.models.loc(lc: I.Loc, ms: List<&2, String>, cur: String, own: String, info: String, prov: String) -> List<&2, D.Node>:
  match ms:
    case Nil{}: Nil{}
    case m <> tail: View.models.labels.loc(lc, m <> tail, cur, own, U.Models.labels(info, prov))

def View.namew(ms: List<&2, String>) -> Nat:
  match ms:
    case Nil{}:
      7n
    case +m <> t:
      Nat.max(String.length(m), View.namew(t))

# a step's word under its bar (short: the column is narrow)
def VLabel.step(+lc: I.Loc, +e: String) -> String:
  match e:
    case "medium": MSG.T.model.step.medium(lc)
    case "minimal": MSG.T.model.step.minimal(lc)
    case x: VLabel.effort(lc, x)

def View.mp.press(on: Bool) -> List<&2, D.Attr>:
  match on:
    case True{}: [D.On{"click", "mp-effort"}]
    case False{}: [D.At{"disabled", "disabled"}]

# the effort meter: a bar a step, higher to the right, the chosen one lit
# and those below it filled; a step the agent's model does not take is
# drawn disabled
def View.mp.bars(+lc: I.Loc, bs: List<&2, MPK.MpBar>) -> List<&2, D.Node>:
  match bs:
    case Nil{}:
      Nil{}
    case MPK.MpBar{+l, on, lit, fill, i} <> t:
      D.El{"button", "", List.append(&2, D.Attr, [D.At{"class", "step" ++ Bool.pick(String, lit, " on", "") ++ Bool.pick(String, fill, " fill", "")},
        D.At{"title", MSG.T.model.effort.tip(lc, VLabel.effort(lc, l))}, D.At{"data-value", l}, D.At{"style", "--h:" ++ Nat.show(Nat.add(6n, Nat.mul(5n, i))) ++ "px"}], View.mp.press(on)),
        [D.El{"i", "", Nil{}, Nil{}}, D.Dom.span("lv", VLabel.step(lc, l))]} <> View.mp.bars(lc, t)

# a column's head: its name, and the keys that work in it as keycaps
def View.mp.head(+label: String, +keys: String, on: Bool) -> D.Node:
  D.El{"div", "", [D.At{"class", Bool.pick(String, on, "mp-h on", "mp-h")}], [D.Dom.span("", label), D.El{"kbd", "", Nil{}, [D.Txt{keys}]}]}

def View.mp.note(+lc: I.Loc, none: Bool, +agent: String, +own: String) -> String:
  match none:
    case True{}: MSG.T.model.effort.none(lc, agent)
    case False{}: MSG.T.model.effort.note(lc, VLabel.effort(lc, own))

# the picker's search field, once the agent lists many models
# (client.bend's Models.search)
def View.menu.find(+lc: I.Loc, srch: Bool, +q: String) -> List<&2, D.Node>:
  match srch:
    case True{}:
      [D.El{"div", "", [D.At{"class", "find"}], [WB.WB.input("model-q", "mq", q, MSG.T.common.search(lc), "")]}]
    case False{}:
      Nil{}

# a phone's picker: the CLI's first View.mp.few() models (its own picks:
# Claude Code lists default, opus, fable, sonnet, haiku first), the rest
# folded under "More models", unfolded while the cursor's or the thread's
# model is one of them; a search lists all it finds, unfolded
def View.mp.few() -> Nat:
  5n

def View.mp.more.at(open: Bool) -> List<&2, D.Attr>:
  match open:
    case True{}: [D.At{"class", "mp-more"}, D.At{"open", ""}]
    case False{}: [D.At{"class", "mp-more"}]

def View.mp.more.label(+lc: I.Loc, +n: Nat) -> String:
  SM.Menu.text(lc, "More models (" ++ Nat.show(n) ++ ")", "その他のモデル（" ++ Nat.show(n) ++ "）")

def View.mp.rows.labels(fold: Bool, +lc: I.Loc, +ms: List<&2, String>, +cur: String, +own: String, +labels: J.Json) -> List<&2, D.Node>:
  match fold:
    case False{}:
      [D.El{"div", "", [D.At{"class", "rows"}], View.models.labels.loc(lc, ms, cur, own, labels)}]
    case True{}:
      +rest = List.drop(&2, String, ms, View.mp.few())
      [D.El{"div", "", [D.At{"class", "rows"}], View.models.labels.loc(lc, List.take(&2, String, ms, View.mp.few()), cur, own, labels)},
       D.El{"details", "mp-more", View.mp.more.at(Bool.or(List.contains(~String, ~String.eq, rest, cur), List.contains(~String, ~String.eq, rest, own))),
         [D.El{"summary", "", Nil{}, [D.Txt{View.mp.more.label(lc, List.length(&2, String, rest))}]},
          D.El{"div", "", [D.At{"class", "rows"}], View.models.labels.loc(lc, rest, cur, own, labels)}]}]

# Compatibility callers supply serialized metadata; resolve it only once.
def View.mp.rows(fold: Bool, lc: I.Loc, ms: List<&2, String>, cur: String, own: String, mi: String, prov: String) -> List<&2, D.Node>:
  View.mp.rows.labels(fold, lc, ms, cur, own, U.Models.labels(mi, prov))

# the three columns: agent | model | effort (fold: View.mp.rows)
def View.mp.cols.labels(+lc: I.Loc, +ms: List<&2, String>, +labels: J.Json, +c: MPK.MpCur, +now: MPK.MpCur, +ss: List<&2, MPK.MpStep>, +q: String, srch: Bool, fold: Bool) -> List<&2, D.Node>:
  [D.El{"div", "", [D.At{"class", "mp-col mp-agents"}], [View.mp.head(MSG.T.model.col.agent(lc), MPK.ModelPick.cap(0n, MPK.MpCur.col(c)), Nat.is_eq(MPK.MpCur.col(c), 0n)),
     D.El{"div", "", [D.At{"class", "rows"}], View.mp.agents(lc, U.Mpk.agents(), MPK.MpCur.agent(c), MPK.MpCur.agent(now))}]},
   D.El{"div", "", [D.At{"class", "mp-col mp-models"}, D.At{"style", "--namew:" ++ Nat.show(View.namew(ms)) ++ "ch"}], D.Dom.cat(
     [View.mp.head(MSG.T.model.col.model(lc), MPK.ModelPick.cap(1n, MPK.MpCur.col(c)), Nat.is_eq(MPK.MpCur.col(c), 1n))], D.Dom.cat(View.menu.find(lc, srch, q),
     View.mp.rows.labels(fold, lc, ms, MPK.MpCur.model(c), Bool.pick(String, String.eq(MPK.MpCur.agent(c), MPK.MpCur.agent(now)), MPK.MpCur.model(now), "\u{1f}"), labels)))},
   D.El{"div", "", [D.At{"class", "mp-col mp-effort"}], [View.mp.head(MSG.T.model.col.effort(lc), MPK.ModelPick.cap(2n, MPK.MpCur.col(c)), Nat.is_eq(MPK.MpCur.col(c), 2n)),
     D.El{"div", "", [D.At{"class", "meter"}], View.mp.bars(lc, MPK.ModelPick.bars(ss, MPK.MpCur.effort(c)))},
     D.Dom.span("note", View.mp.note(lc, List.is_empty(&2, String, MPK.ModelPick.levels(ss)), MPK.MpCur.agent(c), MPK.MpCur.effort(now)))]}]

def View.mp.cols(lc: I.Loc, ms: List<&2, String>, mi: String, +c: MPK.MpCur, now: MPK.MpCur, ss: List<&2, MPK.MpStep>, q: String, srch: Bool, fold: Bool) -> List<&2, D.Node>:
  View.mp.cols.labels(lc, ms, U.Models.labels(mi, MPK.MpCur.agent(c)), c, now, ss, q, srch, fold)

# a phone's picker is a sheet: a head saying what it is with a close (the
# shade's flag), then the columns stacked in a body that scrolls under it
# (style.css's "phone: the model picker")
def View.mp.top(+lc: I.Loc) -> D.Node:
  D.El{"div", "", [D.At{"class", "mp-top"}], [D.Dom.span("mp-title", MSG.T.composer.model.tip(lc)),
    D.El{"button", "", [D.At{"type", "button"}, D.At{"class", "mp-x"}, D.At{"aria-label", MSG.T.common.close(lc)}, D.At{"title", MSG.T.common.close(lc)},
      D.At{"data-value", "models"}, D.On{"click", "flag"}], [WI.WIcon.svg("x", 22)]}]}

def View.mp.body(phone: Bool, +lc: I.Loc, cols: List<&2, D.Node>) -> List<&2, D.Node>:
  match phone:
    case True{}: [View.mp.top(lc), D.El{"div", "", [D.At{"class", "mp-body"}], cols}]
    case False{}: cols

# the picker open over the composer, and a shade behind it: a click
# anywhere off the picker lands on the shade and shuts it. ms: the models
# of the cursor's agent the search finds (Mpk.models), mi the models info
# cut to them, so each row's label is read from a few KB (Models.view);
# ss its meter (Mpk.steps); phone: a phone's sheet (View.mp.top, .rows)
def View.mp.open.labels(+lc: I.Loc, +ms: List<&2, String>, +labels: J.Json, +c: MPK.MpCur, +now: MPK.MpCur, +ss: List<&2, MPK.MpStep>, +q: String, srch: Bool, +phone: Bool) -> List<&2, D.Node>:
  [D.El{"div", "", [D.At{"class", "shade"}, D.At{"data-value", "models"}, D.On{"click", "flag"}], Nil{}},
   D.El{"div", "", [D.At{"class", Bool.pick(String, phone, "model-menu mp mp-phone", "model-menu mp")}, D.At{"role", "dialog"}, D.At{"aria-label", MSG.T.composer.model.tip(lc)}, D.At{"title", MSG.T.model.keys(lc)}],
     View.mp.body(phone, lc, View.mp.cols.labels(lc, ms, labels, c, now, ss, q, srch,
       Bool.and(phone, Bool.and(String.is_empty(String.trim(q)), Nat.is_gt(List.length(&2, String, ms), Nat.add(View.mp.few(), 1n))))))}]

# Keep the phone sheet and folded model groups when reusing parsed labels.
def View.mp.open(lc: I.Loc, ms: List<&2, String>, mi: String, +c: MPK.MpCur, now: MPK.MpCur, ss: List<&2, MPK.MpStep>, q: String, srch: Bool, phone: Bool) -> List<&2, D.Node>:
  View.mp.open.labels(lc, ms, U.Models.labels(mi, MPK.MpCur.agent(c)), c, now, ss, q, srch, phone)

# Parse the chosen provider's catalog once per render. Labels stay as Json;
# serializing a trimmed catalog and parsing it again cost another full pass.
# Efforts retain their existing source (models, even for opencode).
def View.mp.catalog(+lc: I.Loc, +catalog: J.Json, +efforts: J.Json, +c: MPK.MpCur, +now: MPK.MpCur, +q: String, phone: Bool) -> List<&2, D.Node>:
  +agent = MPK.MpCur.agent(c)
  +prepared = MCat.Catalog.prepare.given(catalog, agent, MPK.MpCur.model(now), String.eq(agent, MPK.MpCur.agent(now)), q)
  View.mp.open.labels(lc, MCat.ModelCatalog.models(prepared), MCat.ModelCatalog.labels(prepared), c, now,
    MPK.ModelPick.steps(U.Effort.chips.about(U.Models.about.given(efforts, agent), agent, MPK.MpCur.model(c))), q, MCat.ModelCatalog.search(prepared), phone)

def View.mp.catalog.of(oc: Bool, lc: I.Loc, +nf: Map<&2, String>, +catalog: J.Json, c: MPK.MpCur, now: MPK.MpCur, q: String, phone: Bool) -> List<&2, D.Node>:
  match oc:
    case False{}: View.mp.catalog(lc, catalog, catalog, c, now, q, phone)
    case True{}: View.mp.catalog(lc, catalog, U.Models.parse(U.Str.get(nf, "models")), c, now, q, phone)

def View.mp.shown(+lc: I.Loc, +nf: Map<&2, String>, +c: MPK.MpCur, +now: MPK.MpCur, +q: String) -> List<&2, D.Node>:
  View.mp.catalog.of(String.eq(MPK.MpCur.agent(c), "opencode"), lc, nf, U.Models.parse(U.Models.of(nf, MPK.MpCur.agent(c))), c, now, q, String.eq(U.Str.get(nf, "local.workspace.form"), "phone"))

# the picker, open or not, with cursor c over the thread's own (now); its
# layer carries data-draft while a cursor is kept, so the web shuts and
# opens it through the hub (host.js's modelMenuReady), never over a
# picture of a cursor dropped since
def View.mp.loc(+lc: I.Loc, +open: Bool, +nf: Map<&2, String>, +c: MPK.MpCur, +now: MPK.MpCur, +q: String, kept: Bool) -> List<&2, D.Node>:
  # Keep only the existing bounded model rows ready in the authenticated tree.
  # Opening can paint immediately; choosing still takes the ordinary hub path.
  +query = Bool.pick(String, open, q, "")
  [D.El{"div", "model-layer", List.append(&2, D.Attr, [D.At{"class", "model-layer"}, D.At{"data-local-models", "1"},
    D.At{"style", Bool.pick(String, open, "display:contents", "display:none")}, D.At{"aria-hidden", Bool.pick(String, open, "false", "true")}],
    Bool.pick(List<&2, D.Attr>, kept, [D.At{"data-draft", "1"}], Nil{})),
    # Prewarm model rows, but a closed picker has no modal backdrop. Retained
    # hosts use backdrop presence to defer their quiet deployment reload.
    List.drop(&2, D.Node, View.mp.shown(lc, nf, c, now, query), Bool.pick(Nat, open, 0n, 1n))}]

# the same from one provider's info (tests and the older callers): the
# thread's own provider, model and effort, no cursor kept
def View.menu.loc(+lc: I.Loc, +open: Bool, ms: List<&2, String>, +cur: String, +info: String, +prov: String, +es: List<&2, String>, +effort: String, +q: String, srch: Bool) -> List<&2, D.Node>:
  View.mp.loc(lc, open, Map.set(&2, String, Map.set(&2, String, Map.new(&2, String), "models", info), "models.oc", info),
    MPK.MpCur{1n, prov, cur, effort}, MPK.MpCur{1n, prov, cur, effort}, q, False{})

# the chip under the composer that opens the picker: agent / model /
# effort in a box with its key (Ctrl+M, workspace-keys.js), then how much
# of the model's window the thread holds (ctx)
# (tail: the effort's " / high", its own span, which a narrow composer
# puts away with the key, style.css, so agent and model still read whole)
def View.mp.chip.k(+lc: I.Loc, +text: String, +tail: String, +ctx: String) -> List<&2, D.Node>:
  D.El{"button", "", [D.At{"class", "model-chip"}, D.At{"title", text ++ tail ++ " · " ++ MSG.T.composer.model.tip(lc) ++ " · Ctrl+M"}, D.At{"data-value", "models"}, D.On{"click", "flag"}],
    [D.El{"span", "", [D.At{"class", "mp-sel"}], D.Dom.cat([D.Txt{text}], Bool.pick(List<&2, D.Node>, String.is_empty(tail), Nil{}, [D.Dom.span("mp-eff", tail)]))},
     D.El{"kbd", "", Nil{}, [D.Txt{"Ctrl M"}]}]} <> Bool.pick(List<&2, D.Node>, String.is_empty(String.trim(ctx)), Nil{}, [D.Dom.span("model-ctx", String.trim(String.drop(String.trim(ctx), Bool.pick(Nat, String.starts_with(String.trim(ctx), "·"), 1n, 0n))))])

def View.mp.chip(+lc: I.Loc, +text: String, +ctx: String) -> List<&2, D.Node>:
  View.mp.chip.k(lc, text, "", ctx)

# the chip's words: "claude / opus / high" (the effort left out when the
# agent's model takes none); View.mp.am the agent and model, View.mp.tail
# the effort
def View.mp.am(+lc: I.Loc, +agent: String, +model: String) -> String:
  agent ++ " / " ++ Bool.pick(String, String.is_empty(model), MSG.T.common.default(lc), model)

def View.mp.tail(+lc: I.Loc, +effort: String, efforts: Bool) -> String:
  Bool.pick(String, efforts, " / " ++ VLabel.effort(lc, effort), "")

def View.mp.text(+lc: I.Loc, +agent: String, +model: String, +effort: String, efforts: Bool) -> String:
  View.mp.am(lc, agent, model) ++ View.mp.tail(lc, effort, efforts)

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


# A dock below the thread header, outside the conversation's scroll area.
# A task opens its thread; an agent opens its recent steps in this dock.

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
      [D.At{"aria-expanded", Bool.pick(String, open, "true", "false")}, D.At{"data-value", SB.Subs.ask(open, key)}, D.On{"click", "fold"}]
    case False{}:
      [D.At{"data-value", key}, D.On{"click", "select"}]

# a row's Stop (SB.Subs.cancel): beside the row, not inside its click
def View.sub.stop.loc(+lc: I.Loc, +act: String, +key: String) -> List<&2, D.Node>:
  D.Dom.when(Bool.not(String.is_empty(act)), D.Dom.button("sub-stop", act, key, Bool.pick(String, String.starts_with(act, "lock-"), MSG.T.subagents.lock.cancel(lc, View.sub.cancel.mode(act)), MSG.T.subagents.cancel.tip(lc)), MSG.T.subagents.cancel.word(lc, View.sub.cancel.mode(act))))

def View.sub.stop(+act: String, +key: String) -> List<&2, D.Node>:
  View.sub.stop.loc(I.En{}, act, key)

def View.sub.empty.loc(lc: I.Loc) -> String:
  match lc:
    case I.En{}: "No steps reported yet."
    case I.Ja{}: "まだ手順が報告されていません。"

# what it was asked (its prompt, as its Agent call gave it), open only
def View.sub.ask.loc(lc: I.Loc) -> String:
  match lc:
    case I.En{}: "Asked"
    case I.Ja{}: "依頼"

def View.sub.ask(+lc: I.Loc, +ask: String) -> List<&2, D.Node>:
  D.Dom.when(Bool.not(String.is_empty(ask)), D.El{"div", "", [D.At{"class", "sub-ask"}], [D.Dom.span("label", View.sub.ask.loc(lc)), D.Dom.span("text", ask)]})

def View.sub.detail.open(+lc: I.Loc, +agent: Bool, +open: Bool, +key: String, +ss: List<&2, SB.SubStep>, +ask: String) -> List<&2, D.Node>:
  D.Dom.cat(View.sub.ask(lc, ask), Bool.pick(List<&2, D.Node>, Bool.and(agent, Bool.and(open, Bool.and(Bool.not(String.starts_with(key, "subs.done:")), List.is_empty(&2, SB.SubStep, ss)))),
    [D.Dom.span("sub-empty", View.sub.empty.loc(lc))], View.sub.steps(ss)))

# what a subagent of the agent's own shows open, drawn shut too, hidden
# (style.css's .sub:not(.open) > .sub-detail), so a click on its row shows
# it at once (subs-feedback.js, presentation.bend's Node.rows); its steps
# from the walk (SB.SubInfo's shown) when the row has none, shut
def View.sub.detail(+lc: I.Loc, +agent: Bool, +open: Bool, +key: String, +ss: List<&2, SB.SubStep>, +ask: String) -> List<&2, D.Node>:
  match agent:
    case False{}: Nil{}
    case True{}: [D.El{"div", "", [D.At{"class", "sub-detail"}], View.sub.detail.open(lc, agent, True{}, key, ss, ask)}]

# the line under a row's name: where it runs, its model, how many steps it
# took and what it used (subs.bend's SubInfo; a part not known is left out)
def View.sub.meta.go(ps: List<&2, String>) -> List<&2, D.Node>:
  match ps:
    case Nil{}: Nil{}
    case +p <> t: List.append(&2, D.Node, D.Dom.when(Bool.not(String.is_empty(p)), D.Dom.span("part", p)), View.sub.meta.go(t))

def View.sub.meta(+ps: List<&2, String>) -> List<&2, D.Node>:
  D.Dom.when(Bool.not(List.is_empty(&2, D.Node, View.sub.meta.go(ps))), D.El{"div", "", [D.At{"class", "sub-meta"}], View.sub.meta.go(ps)})

# what it does now (at work: Claude's word for it), or what it reported
def View.sub.now(+live: Bool, +doing: String, +title: String, +outcome: String) -> List<&2, D.Node>:
  Bool.pick(List<&2, D.Node>, Bool.and(Bool.not(String.is_empty(doing)), Bool.not(String.eq(doing, title))), [D.Dom.span("doing", doing)],
    D.Dom.when(Bool.and(Bool.not(live), Bool.not(String.is_empty(outcome))), D.Dom.span("sub-outcome", outcome)))

# its newest step, shown while its steps are not (style.css's .sub.open > .sub-last)
def View.sub.last(s: SB.SubStep) -> List<&2, D.Node>:
  match s:
    case SB.SubStep{+kind, +text}:
      D.Dom.when(Bool.not(String.is_empty(text)), D.El{"div", "", [D.At{"class", "sub-step sub-last"}], [D.Dom.span("kind", SB.Subs.step.label(kind)), D.Dom.span("text", text)]})

# "running · 3m", "done · 4m": its state, and how long it has been at work
# or took
def View.sub.state(+lc: I.Loc, +word: String, +took: String) -> String:
  U.StateWord.label(lc, word) ++ Bool.pick(String, String.is_empty(took), "", " · " ++ took)

def View.sub.signal.k(+lc: I.Loc, r: SB.SubRow, +act: String, +signal: String, i: SB.SubInfo, +now: Nat) -> D.Node:
  match r i:
    case SB.SubRow{+key, +agent, +title, +who, +word, +doing, +live, +open, _, +ss} SB.SubInfo{+runs, +model, +since, +until, +steps, +tokens, last, +ask, +outcome, +shown}:
      D.El{"div", key, [D.At{"class", "sub sub-signal-" ++ signal ++ " " ++ word ++ Bool.pick(String, String.eq(signal, "active"), " live", "") ++ Bool.pick(String, open, " open", "")}],
        D.Dom.cat([D.El{"button", "", List.append(&2, D.Attr, [D.At{"class", "sub-row"}, D.At{"type", "button"}], View.sub.act(agent, open, key)),
          [View.dot(lc, signal), D.Dom.span(Bool.pick(String, agent, "who chev", "who"), Bool.pick(String, agent, "▸", "↗")), D.Dom.span("title", TTL.Title.shown(lc, title)), D.Dom.span("state", View.sub.state(lc, word, SB.Subs.took(live, since, until, now)))]}],
          D.Dom.cat(View.sub.stop.loc(lc, act, key), D.Dom.cat(View.sub.meta([SB.Subs.or(runs, who), model, SB.Subs.steps.word(steps), SB.Subs.tokens(tokens)]),
            D.Dom.cat(View.sub.now(live, doing, title, outcome), D.Dom.cat(View.sub.last(last), View.sub.detail(lc, agent, open, key, Bool.pick(List<&2, SB.SubStep>, List.is_empty(&2, SB.SubStep, ss), shown, ss), ask))))))}

def View.sub.k(+lc: I.Loc, +r: SB.SubRow, +act: String) -> D.Node:
  View.sub.signal.k(lc, r, act, SB.Subs.signal(r), SB.SubInfo.none(), 0n)

def View.sub.signals.row(+lc: I.Loc, +r: SB.SubRow, +info: Map<&2, String>, +st: M.State, +th: String, +now: Nat) -> D.Node:
  View.sub.signal.k(lc, r, SB.Subs.cancel(r), SB.Subs.signal.of(info, r), SB.Subs.info(st, th, r), now)

def View.sub.signals.k(heading: Bool, +lc: I.Loc, +r: SB.SubRow, +info: Map<&2, String>, +st: M.State, +th: String, +now: Nat) -> D.Node:
  match heading:
    case True{}: D.El{"h3", SB.Subs.key(r), [D.At{"class", "sub-section"}], [D.Txt{SB.Subs.display.label(lc, r)}]}
    case False{}: View.sub.signals.row(lc, r, info, st, th, now)

def View.sub.signals(+lc: I.Loc, +r: SB.SubRow, +info: Map<&2, String>, +st: M.State, +th: String, +now: Nat) -> D.Node:
  View.sub.signals.k(SB.Subs.display.heading(r), lc, r, info, st, th, now)

def View.subs.signals.raw(+lc: I.Loc, rs: List<&2, SB.SubRow>, +info: Map<&2, String>, +st: M.State, +th: String, +now: Nat) -> List<&2, D.Node>:
  match rs:
    case Nil{}: Nil{}
    case r <> rest: View.sub.signals(lc, r, info, st, th, now) <> View.subs.signals.raw(lc, rest, info, st, th, now)

# the rows a page shows, each with more about it (SB.Subs.info: only these)
def View.subs.signals.at(+lc: I.Loc, +rs: List<&2, SB.SubRow>, +info: Map<&2, String>, +st: M.State, +th: String, +now: Nat) -> List<&2, D.Node>:
  +p = VP.Page.info(info, "subagents", SB.Subs.row_keys(rs), "")
  List.append(&2, D.Node, View.subs.signals.raw(lc, SB.Subs.display(LW.Lists.subs(rs, VP.Page.ids(p))), info, st, th, now), LW.Lists.pager(p))

def View.subs.signals.go(+lc: I.Loc, +rs: List<&2, SB.SubRow>, +info: Map<&2, String>) -> List<&2, D.Node>:
  View.subs.signals.at(lc, rs, info, M.State.empty(), "", 0n)

def View.sub(+lc: I.Loc, +r: SB.SubRow) -> D.Node:
  View.sub.k(lc, r, SB.Subs.cancel(r))

def View.subs.go(+lc: I.Loc, rs: List<&2, SB.SubRow>) -> List<&2, D.Node>:
  match rs:
    case Nil{}:
      Nil{}
    case r <> rest:
      View.sub(lc, r) <> View.subs.go(lc, rest)

# Shared head and scroll body keep the dock's controls available as rows grow.
# The rows are drawn shut too, hidden (style.css's .subs.shut), and the head
# carries what a shut keeps either way, so the page shows or hides them at
# the click (subs-feedback.js, presentation.bend's Node.subs) while the hub
# answers; its chevron turns by CSS.
def View.subs.head(+lc: I.Loc, +open: Bool, +th: String, +rs: List<&2, SB.SubRow>, word: String) -> D.Node:
  D.El{"button", "subs-head", [D.At{"class", "subs-head"}, D.At{"type", "button"},
    D.At{"aria-expanded", Bool.pick(String, open, "true", "false")}, D.At{"aria-controls", "subagent-rows"},
    D.At{"data-value", SB.Subs.panel.ask(open, th, rs)}, D.On{"click", SB.Subs.panel.action(open)},
    D.At{"title", Bool.pick(String, open, MSG.T.subagents.hide(lc), MSG.T.subagents.show(lc))}],
    [D.Dom.span("subs-label", MSG.T.thread.subagents(lc)), D.Dom.span("subs-summary", word), WI.WIcon.svg("chevron-down", 14)]}

def View.subs.body(open: Bool, rows: List<&2, D.Node>) -> D.Node:
  D.El{"div", "subagent-rows", [D.At{"id", "subagent-rows"}, D.At{"class", "subs-body"}], rows}

# The head opens and shuts the dock; done, it folds to the head.
def View.subs.k(+lc: I.Loc, +busy: String, +open: Bool, +th: String, +rs: List<&2, SB.SubRow>) -> D.Node:
  D.El{"section", "subs", [D.At{"aria-label", MSG.T.thread.subagents(lc)}, D.At{"class", "subs" ++ Bool.pick(String, String.is_empty(busy), "", " busy") ++ Bool.pick(String, open, "", " shut")}],
    [View.subs.head(lc, open, th, rs, View.sub.panel(lc, rs)), View.subs.body(open, View.subs.go(lc, rs))]}

# the rows that finished and are reporting to the agent (SB.Subs.reporting):
# not at work, so the cell counts them done rather than say "0 done"
def View.sub.reporting(rs: List<&2, SB.SubRow>) -> Nat:
  match rs:
    case Nil{}: 0n
    case +r <> t: Nat.add(Bool.pick(Nat, String.eq(SB.Subs.word(r), "reporting"), 1n, 0n), View.sub.reporting(t))

def View.sub.signal.panel(+lc: I.Loc, +rs: List<&2, SB.SubRow>, +info: Map<&2, String>) -> String:
  MSG.T.subagents.panel(lc, View.sub.count(SB.Subs.waiting(rs)), View.sub.count(SB.Subs.active(rs, info)), Nat.show(Nat.add(SB.Subs.finished_n(rs), View.sub.reporting(rs))))

def View.subs.signal.k(+lc: I.Loc, +busy: String, +open: Bool, +th: String, +rs: List<&2, SB.SubRow>, +info: Map<&2, String>, +st: M.State, +now: Nat) -> D.Node:
  D.El{"section", "subs", [D.At{"aria-label", MSG.T.thread.subagents(lc)}, D.At{"class", "subs" ++ Bool.pick(String, String.is_empty(busy), "", " busy") ++ Bool.pick(String, open, "", " shut")}],
    [View.subs.head(lc, open, th, rs, View.sub.signal.panel(lc, rs, info)), View.subs.body(open, View.subs.signals.at(lc, rs, info, st, th, now))]}

def View.subs.at(+lc: I.Loc, +info: Map<&2, String>, +th: String, +rs: List<&2, SB.SubRow>, +st: M.State, +now: Nat) -> List<&2, D.Node>:
  Bool.pick(List<&2, D.Node>, List.is_empty(&2, SB.SubRow, rs), Nil{},
    [View.subs.signal.k(lc, View.sub.busy(lc, SB.Subs.active(rs, info)), SB.Subs.panel.open(info, th, rs), th, rs, info, st, now)])

def View.subs(+lc: I.Loc, +info: Map<&2, String>, +th: String, +rs: List<&2, SB.SubRow>) -> List<&2, D.Node>:
  View.subs.at(lc, info, th, rs, M.State.empty(), 0n)

def View.subthreads.if(show: Bool, id: String) -> List<&2, D.Node>:
  match show:
    case True{}: [D.El{"div", "subthreads-control", [D.At{"class", "subthreads-control"}], [D.Dom.button("", "thread-child", id, "Create a visible conversation beneath this thread", "+ New subthread")]}]
    case False{}: Nil{}

# The workspace's per-thread creation control, separate from Subagents.
def View.subthreads(+lc: I.Loc, +st: M.State, +id: String) -> List<&2, D.Node>:
  View.subthreads.if(U.ThreadTree.can_route(st, id), id)

def View.parent.loc(+lc: I.Loc, +st: M.State, +thread: String, +p: String) -> List<&2, D.Node>:
  Bool.pick(List<&2, D.Node>, String.is_empty(p), Nil{},
    [D.El{"button", "", [D.At{"class", "parent"}, D.At{"data-value", p}, D.On{"click", "select"}], [D.Txt{U.Tasks.up.label.loc(lc, st, thread, p)}]}])

def View.parent(st: M.State, thread: String, p: String) -> List<&2, D.Node>:
  View.parent.loc(I.En{}, st, thread, p)

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
        D.El{"div", "", [D.At{"class", "ask-btns"}], Bool.pick(List<&2, D.Node>, String.eq(tool, "settings_proposal"),
          [D.Dom.button("primary", "answer", id ++ "|accept", "Apply this proposal", "Approve"), D.Dom.button("", "answer", id ++ "|decline", "Reject this proposal", "Reject")],
          View.ask.btns.loc(lc, U.Ask.bkind(k, tool), id, o, draft))}]}

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

# The agent's todo list (U.Todos.shown) at the top of the thread, beside its
# subagents and in their style: one line, "To-do 3/7", shut until the
# person opens it (client.bend's Todos.open: nothing else ever opens it).
# Open, its steps show under it, and a long list ends in a line that shows
# every step or the fitted few again. Under the top bar the bar's To-do cell
# is the head (View.chrome.todos) and the dock only the steps
# (View.todos.dock); without the bar it is a section with its own head.
def View.todo.more(+text: String, +ask: String) -> List<&2, D.Node>:
  Bool.pick(List<&2, D.Node>, String.is_empty(text), Nil{}, [D.Dom.button("more", "fold", ask, text, text)])

def View.todo.more.if(open: Bool, +lc: I.Loc, +all: Bool, +th: String, +ts: List<&2, TD.Todo>) -> List<&2, D.Node>:
  match open:
    case True{}: View.todo.more(U.Todos.more(lc, all, ts), U.Todos.more.ask(all, th))
    case False{}: Nil{}

# the steps (none shut) and the last line: a done step's box holds a ✓,
# the others are empty squares that style.css draws (the step in progress,
# class in_progress, in the accent); ↑ ↓ × show on the hovered or focused row
def View.todo.item(+th: String, +t: TD.Todo, +n: Nat) -> D.Node:
  D.El{"div", "todo:" ++ TD.Todo.id(t), [D.At{"class", "todo " ++ TD.Todo.status(t)}], [
    D.Dom.button("todo-check", "todo-edit", TD.Todo.check(th, t), "Check or uncheck item", Bool.pick(String, String.eq(TD.Todo.status(t), "completed"), "✓", "")),
    D.El{"input", "text", [D.At{"type", "text"}, D.At{"value", TD.Todo.text(t)}, D.At{"aria-label", "To-do text"}, D.At{"maxlength", "2000"},
      D.At{"data-pre", th ++ "\u{1f}" ++ TD.Todo.id(t) ++ "\u{1f}"}, D.On{"change", "todo-text"}], Nil{}},
    D.Dom.button("todo-up", "todo-edit", TD.Todo.ask(th, "move", TD.Todo.id(t), "", "", Nat.sub(n, 1n)), "Move item up", "↑"),
    D.Dom.button("todo-down", "todo-edit", TD.Todo.ask(th, "move", TD.Todo.id(t), "", "", 1n+n), "Move item down", "↓"),
    D.Dom.button("todo-delete", "todo-edit", TD.Todo.ask(th, "delete", TD.Todo.id(t), "", "", 0n), "Delete item", "×")]}

def View.todo.items(ts: List<&2, TD.Todo>, +th: String, +n: Nat) -> List<&2, D.Node>:
  match ts:
    case Nil{}: Nil{}
    case +t <> rest: View.todo.item(th, t, n) <> View.todo.items(rest, th, 1n+n)

def View.todo.add(+th: String) -> D.Node:
  D.El{"div", "todo-add", [D.At{"class", "todo-add"}], [
    D.El{"input", "new", [D.At{"id", "todo-new:" ++ th}, D.At{"type", "text"}, D.At{"aria-label", "New to-do"}, D.At{"placeholder", "Add an item"}, D.At{"maxlength", "2000"}], Nil{}},
    D.El{"button", "add", [D.At{"type", "button"}, D.At{"data-field", "todo-new:" ++ th}, D.At{"data-space", th ++ "\u{1f}"}, D.On{"click", "todo-add"}], [D.Txt{"Add"}]}]}

def View.todos.editable(open: Bool, +th: String, ts: List<&2, TD.Todo>) -> List<&2, D.Node>:
  match open:
    case False{}: Nil{}
    case True{}: List.append(&2, D.Node, View.todo.items(ts, th, 0n), [View.todo.add(th)])

def View.todos.rows(+lc: I.Loc, +open: Bool, +all: Bool, +th: String, +ts: List<&2, TD.Todo>, +info: Map<&2, String>) -> List<&2, D.Node>:
  View.todos.editable(open, th, ts)

def View.todos.body(rows: List<&2, D.Node>) -> D.Node:
  D.El{"div", "todo-rows", [D.At{"id", "todo-rows"}, D.At{"class", "todos-body"}], rows}

def View.todos.head(+lc: I.Loc, +open: Bool, +th: String, +ts: List<&2, TD.Todo>) -> D.Node:
  D.El{"button", "todos-head", [D.At{"class", "todos-head"}, D.At{"type", "button"},
    D.At{"aria-expanded", Bool.pick(String, open, "true", "false")}, D.At{"aria-controls", "todo-rows"},
    D.At{"data-value", U.Todos.toggle(open, th)}, D.On{"click", "fold"},
    D.At{"title", Bool.pick(String, open, MSG.T.todos.hide(lc), MSG.T.todos.show(lc))}],
    [D.Dom.span("todos-label", MSG.T.thread.todos(lc)), D.Dom.span("todos-summary", TD.Todo.count(ts)), WI.WIcon.svg(Bool.pick(String, open, "chevron-down", "chevron-right"), 14)]}

def View.todos.k(+lc: I.Loc, +open: Bool, +all: Bool, +th: String, +ts: List<&2, TD.Todo>, +info: Map<&2, String>) -> D.Node:
  D.El{"section", "todos", [D.At{"aria-label", MSG.T.thread.todos(lc)}, D.At{"class", "todos" ++ Bool.pick(String, open, "", " shut")}],
    [View.todos.head(lc, open, th, ts), View.todos.body(View.todos.rows(lc, open, all, th, ts, info))]}

# without the top bar: the section, its head and (open) its steps
def View.todos(+lc: I.Loc, +info: Map<&2, String>, +st: M.State, +th: String, +ts: List<&2, TD.Todo>) -> D.Node:
  View.todos.k(lc, U.Todos.open(info, st, th), U.Todos.all(info, th), th, ts, info)

# under the top bar: only the steps, while the bar's To-do cell has them open
def View.todos.dock.k(open: Bool, +lc: I.Loc, +info: Map<&2, String>, +th: String, +ts: List<&2, TD.Todo>) -> List<&2, D.Node>:
  match open:
    case True{}:
      [D.El{"section", "todos", [D.At{"aria-label", MSG.T.thread.todos(lc)}, D.At{"class", "todos dock"}], [View.todos.body(View.todos.rows(lc, True{}, U.Todos.all(info, th), th, ts, info))]}]
    case False{}:
      Nil{}

def View.todos.dock(+lc: I.Loc, +info: Map<&2, String>, +st: M.State, +th: String, +ts: List<&2, TD.Todo>) -> List<&2, D.Node>:
  View.todos.dock.k(Bool.and(Bool.not(List.is_empty(&2, TD.Todo, ts)), U.Todos.open(info, st, th)), lc, info, th, ts)

def View.todos.box(none: Bool, +lc: I.Loc, +info: Map<&2, String>, +st: M.State, +th: String, +ts: List<&2, TD.Todo>) -> List<&2, D.Node>:
  match none:
    case True{}: Nil{}
    case False{}: [View.todos(lc, info, st, th, ts)]

# what the thread's page draws of it, right under its subagents
def View.thread.todos.own(own: Bool, +lc: I.Loc, +info: Map<&2, String>, +st: M.State, +th: String, +ts: List<&2, TD.Todo>) -> List<&2, D.Node>:
  match own:
    case True{}: View.todos.dock(lc, info, st, th, ts)
    case False{}: View.todos.box(List.is_empty(&2, TD.Todo, ts), lc, info, st, th, ts)

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

# Informational rows have no control panel. Keep them visually distinct
# throughout Settings, including empty collections and unavailable features.
def View.setrow.k(+info: Bool, +label: String, +note: String, ctl: List<&2, D.Node>) -> D.Node:
  D.El{"div", "", [D.At{"class", Bool.pick(String, info, "set-row set-info", "set-row")},
    D.At{"data-setting-kind", U.Settings.row.kind(Bool.not(info))}],
    [D.El{"div", "", [D.At{"class", "set-text"}], [D.Dom.span("set-label", label), D.Dom.span("set-note", note)]},
      D.El{"div", "", [D.At{"class", "set-ctl"}], ctl}]}

def View.setrow(+label: String, +note: String, +ctl: List<&2, D.Node>) -> D.Node:
  View.setrow.k(List.is_empty(&2, D.Node, ctl), label, note, ctl)

def View.days.loc(+lc: I.Loc, +d: String) -> List<&2, D.Node>:
  +n = {Maybe.default(&2, Nat, Nat.read(d), 3n) : Nat}
  [D.Dom.button("", "setting", "settle.days=" ++ Nat.show(Bool.pick(Nat, Nat.is_gt(n, 1n), Nat.sub(n, 1n), 1n)), MSG.T.settings.days.fewer(lc), "−"),
   D.Dom.span("days", MSG.T.settings.settle.days(lc, Nat.show(n), Bool.pick(String, Nat.is_eq(n, 1n), "one", "other"))),
   D.Dom.button("", "setting", "settle.days=" ++ Nat.show(Bool.pick(Nat, Nat.is_lt(n, 90n), 1n+n, 90n)), MSG.T.settings.days.more(lc), "+")]

def View.days(+d: String) -> List<&2, D.Node>:
  View.days.loc(I.En{}, d)

# Same HTML progress element and styling as the local voice model download.
def View.cad.meter(+p: J.Json) -> List<&2, D.Node>:
  Bool.pick(List<&2, D.Node>, CIP.Cad.busy(CIP.Cad.phase(J.Json.get_str(p, "phase", "idle"))),
    [D.El{"progress", "", List.append(&2, D.Attr, [D.At{"class", "voice-progress cad-progress"}, D.At{"aria-label", "Installation progress"}],
      Bool.pick(List<&2, D.Attr>, CIP.Cad.known(p), [D.At{"max", "1000"}, D.At{"value", Nat.show(CIP.Cad.fraction(p))}], Nil{})), Nil{}}], Nil{})

def View.kn.acts(as: List<&2, KH.KAct>) -> List<&2, D.Node>:
  match as:
    case Nil{}: Nil{}
    case KH.KAct{+l, "cad-cancel", +v} <> t:
      List.append(&2, D.Node, View.cad.meter(Maybe.default(&2, J.Json, J.Json.parse(v), J.Null{})), D.Dom.button("", "cad-cancel", v, l, l) <> View.kn.acts(t))
    case KH.KAct{+l, "link", v} <> t:
      D.El{"a", "", [D.At{"href", v}, D.At{"target", "_blank"}, D.At{"rel", "noopener"}], [D.Txt{l}]} <> View.kn.acts(t)
    case KH.KAct{+l, "kicad-installing", _} <> t:
      D.El{"button", "", [D.At{"class", "kn-busy"}, D.At{"disabled", "disabled"}, D.At{"aria-busy", "true"}, D.At{"aria-live", "polite"}], [D.Txt{l}]} <> View.kn.acts(t)
    case KH.KAct{+l, "kicad-installed", _} <> t:
      D.El{"button", "", [D.At{"class", "kn-done"}, D.At{"disabled", "disabled"}, D.At{"aria-live", "polite"}], [D.Txt{"✓ " ++ l}]} <> View.kn.acts(t)
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

# the dictionary's entries as written (core/vdict.bend: settings voice.e.*)
def View.ventries.go(ks: List<&2, String>, +st: M.State, acc: List<&2, String>) -> List<&2, String>:
  match ks:
    case Nil{}:
      List.reverse(&2, String, acc)
    case +k <> t:
      View.ventries.go(t, st, M.Setting.get(st, k, "") <> acc)

def View.ventries(+st: M.State) -> List<&2, String>:
  View.ventries.go(M.Setting.keys.pre(st, VDV.Vdc.e.pre()), st, Nil{})

def View.voice.set.base(+lc: I.Loc, +u: U.Ui, +st: M.State) -> List<&2, D.Node>:
  [View.setrow(MSG.T.settings.voice.key(lc), U.Sealed.say(u, "vkey", Bool.pick(String, String.is_empty(U.Ui.scratch(u, "@voice.key")), MSG.T.settings.voice.key.note(lc),
      MSG.T.settings.voice.key.saved(lc, U.Ui.scratch(u, "@voice.key")))),
     [WB.WB.sealed("voice-key", "vkey", "password", "sk-…"), D.Dom.button("primary", "sealed", "vkey", MSG.T.settings.key.keep(lc), MSG.T.common.save(lc)),
      D.Dom.button("", "voice-unkey", "", MSG.T.settings.key.forget(lc), MSG.T.common.remove(lc))]),
   View.setrow(MSG.T.settings.voice.microphone(lc), MSG.T.voice.device.note(lc, Bool.pick(String, String.is_empty(M.Setting.get(st, "voice.device", "")), MSG.T.voice.device.default(lc, U.Voice.label(u, U.Voice.default(u))), U.Voice.label(u, M.Setting.get(st, "voice.device", "")))),
     D.Dom.button(Bool.pick(String, String.is_empty(M.Setting.get(st, "voice.device", "")), "on", ""), "setting", "voice.device=", MSG.T.settings.voice.default.tip(lc), MSG.T.voice.default(lc)) <>
       View.devs(U.Voice.devices(u), M.Setting.get(st, "voice.device", ""))),
   View.setrow(MSG.T.settings.voice.dictionary(lc),
     Bool.pick(String, List.is_empty(&2, String, List.append(&2, String, U.Voice.dict(u), View.ventries(st))), MSG.T.settings.voice.dictionary.empty(lc) ++ " " ++ MSG.T.settings.voice.entries.note(lc), MSG.T.settings.voice.entries.note(lc)),
     List.append(&2, D.Node, View.terms.loc(lc, List.append(&2, String, U.Voice.dict(u), View.ventries(st))), [WB.WB.area("voice-term", "vterm", U.Ui.scratch(u, "@f.vterm"), MSG.T.settings.voice.term(lc)),
       D.Dom.button("", "voice-term", "", MSG.T.settings.voice.term.add(lc), MSG.T.common.add(lc))]))]

# (Delete asks first: the name pressed once, rm, reads "Click again to
# delete", client.bend's ApiKeys.del)
def View.keys.rows.loc(+lc: I.Loc, ns: List<&2, String>, +rm: String) -> List<&2, D.Node>:
  match ns:
    case Nil{}:
      Nil{}
    case +n <> t:
      View.setrow(n, "", [D.Dom.button(Bool.pick(String, String.eq(n, rm), "danger on", "danger"), "key-del", n, MSG.T.settings.keys.forget(lc),
        Bool.pick(String, String.eq(n, rm), MSG.T.menu.delete.confirm(lc), MSG.T.common.delete(lc)))]) <> View.keys.rows.loc(lc, t, rm)

def View.keys.rows(ns: List<&2, String>) -> List<&2, D.Node>:
  View.keys.rows.loc(I.En{}, ns, "")

def View.keys.list.loc(+lc: I.Loc, open: Bool, +u: U.Ui) -> List<&2, D.Node>:
  match open:
    case True{}:
      Bool.pick(List<&2, D.Node>, List.is_empty(&2, String, U.ApiKeys.names(u)), [View.setrow(MSG.T.settings.keys.empty(lc), "", Nil{})], View.keys.rows.loc(lc, U.ApiKeys.names(u), U.Ui.scratch(u, "@keys.rm")))
    case False{}:
      Nil{}

def View.keys.list(open: Bool, +u: U.Ui) -> List<&2, D.Node>:
  View.keys.list.loc(I.En{}, open, u)

# (the form is sealed, client.bend's Sealed: the key's value never reaches
# the client's state, a view action or a draft; host.js's sealedSave reads
# it and the name once, at Save or Enter, into the request U.ApiKeys.req
# builds)
def View.keys.form(+lc: I.Loc, open: Bool, +u: U.Ui) -> List<&2, D.Node>:
  match open:
    case True{}:
      [View.setrow(MSG.T.settings.keys.new(lc), U.ApiKeys.form.say(lc, u),
        [WB.WB.sealed("kname", "key", "text", MSG.T.bots.name(lc)), WB.WB.sealed("kval", "key", "password", MSG.T.common.key(lc)),
         D.Dom.button("primary", "sealed", "key", MSG.T.settings.key.keep(lc), MSG.T.common.save(lc))])]
    case False{}:
      Nil{}

# Antigravity (client.bend's Agv.rows): a row's fields and buttons; its
# "open-url" button is a link that opens in a new tab. The key and the
# sign-in's way back are sealed (U.Sealed.agy names their form): their
# Save is "sealed" and their fields have no binding.
def View.agy.btn.k(+l: String, +a: String, +v: String, +cls: String, +form: String) -> D.Node:
  Bool.pick(D.Node, String.eq(a, "open-url"),
    D.El{"a", "", [D.At{"class", Bool.pick(String, String.eq(cls, "on"), "btn primary", "btn")}, D.At{"href", v}, D.At{"target", "_blank"}, D.At{"rel", "noopener noreferrer"}], [D.Txt{l}]},
    Bool.pick(D.Node, String.is_empty(form), D.Dom.button(cls, a, v, l, l), D.Dom.button(cls, "sealed", form, l, l)))

def View.agy.btn(+u: U.Ui, +b: U.AgBtn) -> D.Node:
  match b:
    case U.AgBtn{+l, +a, +v, on}:
      View.agy.btn.k(l, a, v, Bool.pick(String, on, "on", ""), U.Sealed.agy(u, a))

def View.agy.btns(+u: U.Ui, bs: List<&2, U.AgBtn>) -> List<&2, D.Node>:
  match bs:
    case Nil{}:
      Nil{}
    case +b <> t:
      View.agy.btn(u, b) <> View.agy.btns(u, t)

# (a secret field with no form of its own is sealed too: its Save says
# there is nothing to save rather than its value going anywhere)
def View.agy.field.k(sealed: Bool, +n: String, +h: String, +kind: String, +v: String, +e: String, +form: String) -> D.Node:
  Bool.pick(D.Node, sealed, WB.WB.sealed("agy-" ++ n, form, kind, h), WB.WB.input("agy-" ++ n, n, v, h, e))

def View.agy.field(+u: U.Ui, +f: U.AgField) -> D.Node:
  match f:
    case U.AgField{+n, +h, +sec, +v, +e}:
      View.agy.field.k(Bool.or(sec, Bool.not(String.is_empty(U.Sealed.agy(u, e)))), n, h, Bool.pick(String, sec, "password", "text"), v, e, U.Sealed.agy(u, e))

def View.agy.fields(+u: U.Ui, fs: List<&2, U.AgField>) -> List<&2, D.Node>:
  match fs:
    case Nil{}:
      Nil{}
    case +f <> t:
      View.agy.field(u, f) <> View.agy.fields(u, t)

def View.agy.rows(+u: U.Ui, rs: List<&2, U.AgRow>) -> List<&2, D.Node>:
  match rs:
    case Nil{}:
      Nil{}
    case U.AgRow{+l, +n, fs, bs} <> t:
      View.setrow(l, n, List.append(&2, D.Node, View.agy.fields(u, fs), View.agy.btns(u, bs))) <> View.agy.rows(u, t)

def View.cad.row(+u: U.Ui, +tool: String) -> D.Node:
  D.El{"div", "cad-" ++ tool, [D.At{"class", "cad-install"}, D.At{"role", "status"}, D.At{"aria-live", "polite"}],
    List.append(&2, D.Node, View.agy.rows(u, [U.Cadview.row(u, tool)]), View.cad.meter(U.Cadview.at(U.Ui.infos(u), tool)))}

def View.voice.engine(+lc: I.Loc, +u: U.Ui, +st: M.State) -> List<&2, D.Node>:
  View.agy.rows(u, U.Vlocal.rows(lc, u))

# (the key's field is sealed, client.bend's Sealed: its value never
# reaches the client's state, a view action or a draft)
def View.vlocal.meter(on: Bool, +u: U.Ui) -> List<&2, D.Node>:
  match on:
    case False{}: Nil{}
    case True{}:
      +p = Maybe.default(&2, J.Json, J.Json.parse(U.Ui.scratch(u, "@voice.progress")), J.Null{})
      +total = J.Json.get_nat(p, "total", 0n)
      +known = Bool.and(Nat.is_gt(total, 0n), String.eq(J.Json.get_str(p, "phase", "download"), "download"))
      [D.El{"progress", "voice-model-progress", List.append(&2, D.Attr,
        [D.At{"class", "voice-progress"}, D.At{"aria-label", MSG.T.settings.voice.model(U.Lang.loc(u))}],
        Bool.pick(List<&2, D.Attr>, known, [D.At{"max", Nat.show(total)}, D.At{"value", Nat.show(Nat.min(total, J.Json.get_nat(p, "done", 0n)))}], Nil{})), Nil{}}]

def View.vlocal.row(+lc: I.Loc, +u: U.Ui) -> D.Node:
  D.El{"div", "voice-model", [D.At{"class", Bool.pick(String, U.Vlocal.busy(u), "set-row set-busy", "set-row")},
    D.At{"aria-busy", Bool.pick(String, U.Vlocal.busy(u), "true", "false")}], [
    D.El{"div", "", [D.At{"class", "set-text"}], [D.Dom.span("set-label", MSG.T.settings.voice.model(lc)),
      D.El{"span", "", [D.At{"class", Bool.pick(String, String.is_empty(U.Vlocal.error(u)), "set-note", "set-note bad")}, D.At{"role", "status"}, D.At{"aria-live", "polite"}], [D.Txt{U.Vlocal.note(lc, u)}]}]},
    D.El{"div", "", [D.At{"class", "set-ctl"}], List.append(&2, D.Node, View.agy.btns(u, U.Vlocal.btns.k(U.Vlocal.busy(u), lc, u)), View.vlocal.meter(U.Vlocal.busy(u), u))}]}

def View.voice.set.loc(+lc: I.Loc, +u: U.Ui, +st: M.State) -> List<&2, D.Node>:
  List.append(&2, D.Node, List.append(&2, D.Node, View.agy.rows(u, List.take(&2, U.AgRow, U.Vlocal.rows(lc, u), 1n)), [View.vlocal.row(lc, u)]), List.append(&2, D.Node, View.voice.set.base(lc, u, st), View.agy.rows(u, U.Vlearned.rows(lc, st))))

# API keys (client.bend's ApiKeys): add one, view the names, delete one

def View.voice.set(+u: U.Ui, +st: M.State) -> List<&2, D.Node>:
  View.voice.set.loc(I.En{}, u, st)

# the time zone: where the person is (their phone reports it, or they set
# it); a field for a zone name, Set, and Automatic: on (like a chosen chip)
# while no zone is set, its words saying what it follows (the phone, else
# this machine: client.bend's Tzs.auto.tip); a click goes back to it once
# one is
def View.tz.auto.loc(+lc: I.Loc, +set: Bool, +tip: String) -> D.Node:
  D.El{"button", "", [D.At{"class", Bool.pick(String, set, "", "on")}, D.At{"type", "button"}, D.At{"title", tip},
    D.At{"aria-pressed", Bool.pick(String, set, "false", "true")}, D.At{"data-value", ""}, D.On{"click", "tz-auto"}], [D.Txt{MSG.T.settings.lang.auto(lc)}]}

# the hub's answer to a zone it could not set, under the row; no row without one
def View.tz.err(+err: String) -> List<&2, D.Node>:
  Bool.pick(List<&2, D.Node>, String.is_empty(err), Nil{}, [View.setrow("", err, Nil{})])

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

def View.usage.controls(cs: List<&2, UP.UpControl>) -> List<&2, D.Node>:
  match cs:
    case Nil{}: Nil{}
    case UP.UpControl{+label, action, value, on} <> rest:
      D.Dom.button(Bool.pick(String, on, "on", ""), action, value, label, label) <> View.usage.controls(rest)

def View.usage.pagers(cs: List<&2, UP.UpControl>) -> List<&2, D.Node>:
  match cs:
    case Nil{}: Nil{}
    case +h <> +rest: [View.setrow("Breakdown pages", "", View.usage.controls(h <> rest))]

def View.usage.page(+lc: I.Loc, +u: U.Ui) -> List<&2, D.Node>:
  UV.Uv.page(lc, u)

# the opt-in for the composer's token chip (usage.bend's Ts)
def View.tok.set(+lc: I.Loc, +st: M.State) -> D.Node:
  View.setrow(UZ.Ts.label(), UZ.Ts.blurb(), View.chips.loc(lc, ["on", "off"], Bool.pick(String, UZ.Ts.on(st), "on", "off"), UZ.Ts.key()))

def View.tz.set(+lc: I.Loc, +u: U.Ui) -> List<&2, D.Node>:
  View.setrow(MSG.T.settings.zone(lc), U.Tzs.line.loc(lc, u),
    [WB.WB.input("tzname", "tzname", U.Ui.scratch(u, "@f.tzname"), "Europe/Helsinki", "tz-set"), D.Dom.button("primary", "tz-set", "", MSG.T.settings.tz.choose(lc), MSG.T.common.set(lc)),
     View.tz.auto.loc(lc, U.Tzs.auto(u), U.Tzs.auto.tip(lc, u))]) <> View.tz.err(U.Tzs.err(u))

def View.keys.set(+lc: I.Loc, +u: U.Ui) -> List<&2, D.Node>:
  List.concat(&2, D.Node, [
    [View.setrow(MSG.T.settings.keys(lc), U.ApiKeys.note.loc(lc, u),
      [D.Dom.button(Bool.pick(String, U.Ui.flag(u, "keyform"), "on", ""), "flag", "keyform", MSG.T.settings.keys.add.tip(lc), MSG.T.settings.keys.add(lc)),
       D.Dom.button(Bool.pick(String, U.Ui.flag(u, "keys"), "on", ""), "flag", "keys", MSG.T.settings.keys.names(lc), MSG.T.settings.keys.view(lc))])],
    View.keys.form(lc, U.Ui.flag(u, "keyform"), u), View.keys.list.loc(lc, U.Ui.flag(u, "keys"), u)])

def View.sbox.fields(+lc: I.Loc, fresh: Bool, +n: String, +p: String) -> List<&2, D.Node>:
  match fresh:
    case True{}:
      [WB.WB.input("sname", "sname", n, MSG.T.secret.name.ph(lc), ""), WB.WB.input("spurpose", "spurpose", p, MSG.T.secret.purpose.ph(lc), "")]
    case False{}:
      Bool.pick(List<&2, D.Node>, String.is_empty(p), Nil{}, [D.Dom.span("sbox-purpose", p)])

# The secret box (client.bend's Sbox.*): a masked field with no binding
# (the value is never in the client's state); Save carries the name,
# purpose and thread in data-space and host.js reads the field once, at
# that click (secretSave), into the request Sbox.req builds
def View.sbox.k(+lc: I.Loc, b: U.SBox) -> D.Node:
  match b:
    case U.SBox{+n, +p, +who, +th, fresh, +err}:
      D.El{"div", "sbox", [D.At{"class", "sbox"}], List.concat(&2, D.Node, [
        [D.Dom.span("sbox-head", Bool.pick(String, String.is_empty(who), MSG.T.secret.add(lc), MSG.T.secret.asks(lc, who, n)))],
        View.sbox.fields(lc, fresh, n, p),
        [D.El{"input", "sbox-val", [D.At{"id", "sbox-val"}, D.At{"class", "field sbox-val"}, D.At{"type", "password"}, D.At{"autocomplete", "new-password"}, D.At{"spellcheck", "false"},
          D.At{"placeholder", MSG.T.secret.value.ph(lc)}, D.At{"data-enter", "secret-save"}, D.At{"data-space", n ++ "\u{1f}" ++ p ++ "\u{1f}" ++ th}, D.At{"data-field", "sbox-val"}], Nil{}},
         D.Dom.span("sbox-note", Bool.pick(String, String.is_empty(err), MSG.T.secret.note(lc), err)),
         D.El{"div", "", [D.At{"class", "sbox-btns"}], [
           D.El{"button", "", [D.At{"class", "primary"}, D.At{"data-space", n ++ "\u{1f}" ++ p ++ "\u{1f}" ++ th}, D.At{"data-field", "sbox-val"}, D.On{"click", "secret-save"}], [D.Txt{MSG.T.secret.save(lc)}]},
           Bool.pick(D.Node, String.is_empty(who), D.Dom.button("", "secret-shut", "", MSG.T.secret.notnow(lc), MSG.T.secret.notnow(lc)), D.Dom.button("", "secret-dismiss", n, MSG.T.secret.notnow(lc), MSG.T.secret.notnow(lc)))]}]])}

def View.sbox(+lc: I.Loc, m: Maybe<&2, U.SBox>) -> List<&2, D.Node>:
  match m:
    case Some{b}:
      [View.sbox.k(lc, b)]
    case None{}:
      Nil{}

def View.secrets.asks(xs: List<&2, U.SAsk>, +lc: I.Loc, +st: M.State) -> List<&2, D.Node>:
  match xs:
    case Nil{}:
      Nil{}
    case +a <> t:
      View.setrow(MSG.T.secret.requested(lc) ++ ": " ++ U.SAsk.name(a), U.Sbox.ask.note(st, a),
        [D.Dom.button("primary", "secret-open", U.SAsk.name(a), MSG.T.secret.save(lc), MSG.T.common.set(lc)), D.Dom.button("", "secret-dismiss", U.SAsk.name(a), MSG.T.secret.notnow(lc), MSG.T.secret.notnow(lc))])
        <> View.secrets.asks(t, lc, st)

def View.secrets.row(+lc: I.Loc, r: U.SRowV) -> D.Node:
  match r:
    case U.SRowV{+n, +note, +rm}:
      View.setrow(n, note, [D.Dom.button("", "secret-open", n, MSG.T.secret.replace(lc), MSG.T.secret.replace(lc)),
        D.Dom.button(Bool.pick(String, rm, "danger on", "danger"), "secret-del", n, MSG.T.common.delete(lc), Bool.pick(String, rm, MSG.T.secret.delete.ask(lc, n), MSG.T.common.delete(lc)))])

def View.secrets.rows(rs: List<&2, U.SRowV>, +lc: I.Loc) -> List<&2, D.Node>:
  match rs:
    case Nil{}:
      Nil{}
    case r <> t:
      View.secrets.row(lc, r) <> View.secrets.rows(t, lc)

# Settings > Secrets (beside API keys): the vault, an open box, the
# requests and the secrets kept (names, purposes, last use; never a value)
def View.secrets.set(+lc: I.Loc, +u: U.Ui) -> List<&2, D.Node>:
  List.concat(&2, D.Node, [
    [View.setrow(MSG.T.secret.title(lc), U.Sbox.note(lc, u), [D.Dom.button("", "secret-open", "", MSG.T.secret.open.tip(lc), MSG.T.secret.add(lc))])],
    View.sbox(lc, U.Sbox.box(u, "")),
    View.secrets.asks(U.Sbox.asks(U.Ui.state.of(u)), lc, U.Ui.state.of(u)),
    View.secrets.rows(U.Sbox.rows(lc, u), lc)])

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

# (a link nothing verified asks Mine / Not mine instead: "peer-own")
def View.mates.one(ask: Bool, +lc: I.Loc, +nm: String, +note: String, +on: String, +key: String, +link: String) -> D.Node:
  match ask:
    case True{}:
      View.setrow(nm, note, [D.Dom.button("", "peer-own", link ++ "=1", MSG.T.machines.mine(lc), MSG.T.machines.mine(lc)),
        D.Dom.button("", "peer-own", link ++ "=0", MSG.T.machines.notmine(lc), MSG.T.machines.notmine(lc))])
    case False{}:
      View.setrow(nm, note, View.chips.loc(lc, ["on", "off"], on, key))

def View.mates.go.loc(+lc: I.Loc, rs: List<&2, U.MateRow>) -> List<&2, D.Node>:
  match rs:
    case Nil{}:
      Nil{}
    case U.MateRow{+nm, +note, +on, +key, +ask} <> t:
      View.mates.one(Bool.not(String.is_empty(ask)), lc, nm, note, on, key, ask) <> View.mates.go.loc(lc, t)

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

def View.settings.scale(vs: List<&2, String>, +cur: U32) -> List<&2, D.Node>:
  match vs:
    case Nil{}: Nil{}
    case +v <> t:
      D.Dom.button(Bool.pick(String, U32.is_eq(TS.Scale.read(v), cur), "on", ""), "setting", "ui.text_scale=" ++ v, TS.Scale.label(v), TS.Scale.label(v)) <> View.settings.scale(t, cur)

# a phone's way out of the phone layout (phone-other.js's
# [data-ph-layout]: kept for this browser, as ?phone=0 is); a phone on the
# computer layout gets a "Phone layout" button back
def View.settings.phone.layout(phone: Bool, +lc: I.Loc) -> List<&2, D.Node>:
  match phone:
    case True{}:
      [View.setrow(SM.Menu.text(lc, "Layout on this phone", "このスマートフォンのレイアウト"),
        SM.Menu.text(lc, "The phone layout shows one screen at a time. The computer layout shows the sidebar, thread and viewer together, small; a Phone layout button brings this one back.",
          "スマートフォン用レイアウトは一度に一画面を表示します。パソコン用レイアウトはサイドバー、スレッド、ビューアーを小さくまとめて表示し、Phone layout ボタンでこちらに戻れます。"),
        [D.El{"button", "", [D.At{"type", "button"}, D.At{"data-ph-layout", "0"}], [D.Txt{SM.Menu.text(lc, "Use the computer layout", "パソコン用レイアウトにする")}]}])]
    case False{}: Nil{}

# a phone's own notifications: the browser asks only on a tap on Allow
# (phone-other.js's [data-ph-notify]), and html's ph-notify-* class says
# which of the words shows (granted, denied, unavailable here, or not
# over plain http: browsers keep notifications to HTTPS)
def View.settings.phone.notify(phone: Bool, +lc: I.Loc) -> List<&2, D.Node>:
  match phone:
    case True{}:
      [View.setrow(SM.Menu.text(lc, "Notifications on this phone", "このスマートフォンの通知"),
        SM.Menu.text(lc, "An alert when a thread needs you or finishes while this phone was the device you used last. On an iPhone, add Backplane to the Home Screen first (Share, Add to Home Screen) and allow them there.",
          "最後に使った端末がこのスマートフォンのとき、スレッドがあなたを必要としたり終わったりすると知らせます。iPhone では先に Backplane をホーム画面に追加し（共有、ホーム画面に追加）、そこで許可してください。"),
        [D.El{"button", "", [D.At{"type", "button"}, D.At{"class", "ph-notify-ask"}, D.At{"data-ph-notify", ""}], [D.Txt{SM.Menu.text(lc, "Allow notifications", "通知を許可")}]},
         D.Dom.span("ph-notify-on", SM.Menu.text(lc, "On for this phone", "このスマートフォンでオン")),
         D.Dom.span("ph-notify-off", SM.Menu.text(lc, "Blocked: allow notifications for this site in the browser's settings", "ブロック中：ブラウザの設定でこのサイトの通知を許可してください")),
         D.Dom.span("ph-notify-none", SM.Menu.text(lc, "This browser can't show notifications here", "このブラウザではここに通知を表示できません")),
         D.Dom.span("ph-notify-http", SM.Menu.text(lc, "Not over this http:// address: a phone's browser allows notifications only over HTTPS. Start the hub with --tailscale-https and open its https:// address here.",
           "この http:// アドレスでは使えません：スマートフォンのブラウザは HTTPS でのみ通知を許可します。ハブを --tailscale-https で起動し、その https:// アドレスをここで開いてください。"))])]
    case False{}: Nil{}

# inside the phone apps (docs/phone-app-shell.md; app-shell.js's
# [data-ph-app], html.ph-in-app shows it): the app's own settings, its
# hubs, notifications and Classic screens
def View.settings.phone.app(phone: Bool, +lc: I.Loc) -> List<&2, D.Node>:
  match phone:
    case True{}:
      [D.El{"div", "", [D.At{"class", "ph-app-row"}], [View.setrow(SM.Menu.text(lc, "This phone app", "このスマートフォンアプリ"),
        SM.Menu.text(lc, "The hubs this phone is paired with, its notifications, and the app's Classic screens.", "ペアリングしたハブ、通知、アプリのクラシック画面。"),
        [D.El{"button", "", [D.At{"type", "button"}, D.At{"data-ph-app", "settings"}], [D.Txt{SM.Menu.text(lc, "App settings", "アプリの設定")}]}])]}]
    case False{}: Nil{}

# a phone's way to Improvements (improvements.js opens it from any
# [data-improvements]; the desktop's is in the top bar, which a phone has not)
def View.settings.improve(phone: Bool, +lc: I.Loc) -> List<&2, D.Node>:
  match phone:
    case True{}:
      [View.setrow(SM.Menu.text(lc, "Improvements", "改善の提案"), SM.Menu.text(lc, "Tell us what to fix or add, with a screenshot of what you see.", "直してほしいことや欲しい機能を、見ている画面のスクリーンショットと一緒に送れます。"),
        [D.El{"button", "", [D.At{"type", "button"}, D.At{"data-improvements", ""}], [D.Txt{SM.Menu.text(lc, "Send an improvement", "改善を送る")}]}])]
    case False{}: Nil{}

def View.settings.rows.k(i: Nat, +lc: I.Loc, +u: U.Ui, +st: M.State, +nf: Map<&2, String>, +v: String, +note: String, +prov: String, +ipc: String) -> List<&2, D.Node>:
  match i:
    case 0n: List.concat(&2, D.Node, [[View.setrow(MSG.T.settings.theme(lc), MSG.T.settings.theme.note(lc), View.chips.loc(lc, ["dark", "light"], Bool.pick(String, String.eq(M.Setting.get(st, "ui.theme", ""), "light"), "light", "dark"), "ui.theme"))],
      View.lang.loc(lc, u),
      [View.setrow(MSG.T.settings.sidebar(lc), MSG.T.settings.sidebar.note(lc), View.chips.loc(lc, ["projects", "active"], U.Side.view(st), "sidebar.view"))],
      [View.setrow(MSG.T.settings.textsize(lc), MSG.T.settings.textsize.note(lc), View.settings.scale(TS.Scale.options(), TS.Scale.of(st)))],
      View.settings.phone.layout(PHN.Phone.on(u), lc), View.settings.phone.app(PHN.Phone.on(u), lc)])
    case 1n: List.concat(&2, D.Node, [[View.setrow(MSG.T.settings.settle(lc), U.Settings.settle.note(lc, st), View.chips.loc(lc, ["0", "5", "10", "15", "30", "60"], M.Setting.get(st, "settle.idle.minutes", "15"), "settle.idle.minutes"))],
      [View.setrow(MSG.T.settings.settle.personal(lc), MSG.T.settings.settle.personal.note(lc), View.chips.loc(lc, ["0", "60", "120", "240", "480"], M.Setting.get(st, "settle.personal.minutes", "240"), "settle.personal.minutes"))],
      [View.setrow(MSG.T.settings.settle.done(lc), MSG.T.settings.settle.done.note(lc), View.chips.loc(lc, ["on", "off"], Bool.pick(String, M.Lifecycle.on(st), "on", "off"), M.Lifecycle.key()))],
      [View.setrow(MSG.T.settings.restart(lc), MSG.T.settings.restart.note(lc), View.chips.loc(lc, ["on", "off"], Bool.pick(String, M.Carry.on(st), "on", "off"), "restart.continue"))],
      [View.setrow(U.Files.set.label(), U.Files.set.note(st), View.chips.loc(lc, AT.Att.sizes(), U.Files.set.on(st), AT.Att.key()))],
      View.notify(lc, st),
      View.settings.phone.notify(PHN.Phone.on(u), lc), View.settings.phone.app(PHN.Phone.on(u), lc),
      View.agy.rows(u, U.Hookv.rows.loc(lc, u))])
    case 2n: List.append(&2, D.Node, Bool.pick(List<&2, D.Node>, U.Psv.active(u), View.agy.rows(u, U.Psv.rows(1n, u)), List.concat(&2, D.Node, [View.agy.rows(u, U.Psv.rows(1n, u)), [View.setrow(MSG.T.settings.provider(lc), MSG.T.settings.agents.note(lc, Bool.pick(String, String.is_empty(U.Models.cli(U.Str.get(nf, "models"), prov)), MSG.T.settings.provider.missing(lc, prov), U.Models.cli(U.Str.get(nf, "models"), prov))), View.chips.loc(lc, ["claude", "codex", "grok", "muse", "antigravity", "opencode", "pi", "omp"], prov, "agent.provider"))],
      [View.setrow(MSG.T.settings.model(lc), Bool.pick(String, String.is_empty(M.Setting.get(st, "agent.model", "")), MSG.T.common.default(lc), M.Setting.get(st, "agent.model", "")) ++ ": " ++ U.Models.label_of(U.Models.of(nf, prov), prov, M.Setting.get(st, "agent.model", "")),
        D.Dom.cat(View.menu.find(lc, U.Models.search(U.Models.menu(U.Models.of(nf, prov), prov)), U.Ui.scratch(u, "@f.mq")),
          View.chips.loc(lc, U.Models.settings(U.Models.of(nf, prov), prov, U.Ui.scratch(u, "@f.mq"), M.Setting.get(st, "agent.model", "")), M.Setting.get(st, "agent.model", ""), "agent.model")))],
      View.agy.rows(u, U.Fallbackv.rows(u)),
      Bool.pick(List<&2, D.Node>, U.Psv.active(u), Nil{}, View.agy.rows(u, U.Acv.rows(u))),
      [View.setrow(U.Think.label(), U.Think.note(), View.chips.loc(lc, ["on", "off"], Bool.pick(String, M.Think.on(st), "on", "off"), M.Think.key()))]])), View.agy.rows(u, U.Agv.rows.loc(lc, u)))
    case 3n: List.concat(&2, D.Node, [[View.setrow(MSG.T.settings.writing.provider(lc), MSG.T.web.settings.writing.note(lc), View.chips.loc(lc, ["claude", "codex", "grok", "muse"], U.TextGen.provider(st), "text.provider"))],
      [View.setrow(MSG.T.web.settings.writing.model(lc), Bool.pick(String, String.is_empty(U.TextGen.model(st)), MSG.T.common.default(lc), U.TextGen.model(st)) ++ ": " ++ U.Models.label_of(U.Str.get(nf, "models"), U.TextGen.provider(st), U.TextGen.model(st)),
        View.chips.loc(lc, U.Models.menu(U.Str.get(nf, "models"), U.TextGen.provider(st)), U.TextGen.model(st), "text.model"))]])
    case 4n: List.concat(&2, D.Node, [[View.setrow("KiCad", U.KN.summary(nf), View.kn.path.loc(lc, u) <> View.kn.loc(lc, U.KN.of(nf, st), False{}))],
      [View.cad.row(u, "kicad"), View.cad.row(u, "freecad")],
      [View.setrow(U.KiCad.ipc.label(lc, nf), U.KiCad.ipc.note(lc, nf), Bool.pick(List<&2, D.Node>, U.KiCad.ipc.ready(nf), View.chips.loc(lc, ["on", "off"], ipc, "kicad.ipc"), Nil{}))]])
    case 5n: List.concat(&2, D.Node, [[View.setrow(U.Tail.label(lc, nf), U.Tail.note(lc, nf), Nil{})],
      View.mates(lc, u),
      [View.setrow("Devices", "Connect another computer or phone with a one-time invitation.", [D.Dom.button("", "devices-open", "", "", "Add a device")])],
      [View.setrow(MSG.T.settings.channel(lc), MSG.T.settings.channel.note(lc), View.chips.loc(lc, ["stable", "nightly"], M.Setting.get(st, "update.channel", "stable"), "update.channel"))],
      [View.setrow("Backplane " ++ v, U.Update.line.w(lc, st, nf, note), View.update.btn.loc(lc, U.Update.btn(st, nf, note)))]])
    case 6n: List.concat(&2, D.Node, [View.voice.set.loc(lc, u, st)])
    case 7n: List.concat(&2, D.Node, [View.keys.set(lc, u), View.secrets.set(lc, u)])
    case 8n: List.concat(&2, D.Node, [View.agy.rows(u, U.Agv.rows.loc(lc, u))])
    case 9n: List.concat(&2, D.Node, [View.tz.set(lc, u)])
    case 10n: View.tok.set(lc, st) <> View.usage.page(lc, u)
    case 11n: List.concat(&2, D.Node, [[View.setrow(MSG.T.settings.tour(lc), MSG.T.settings.tour.note(lc), [D.Dom.button("", "ob-open", "", MSG.T.settings.tour.again(lc), MSG.T.settings.tour.show(lc))])], View.settings.improve(PHN.Phone.on(u), lc)])
    case 12n: View.agy.rows(u, U.Share.rows(u))
    case 14n: [View.setrow("Devices", "Create a one-time invitation, see last activity, or revoke a device.", [D.Dom.button("primary", "devices-open", "", "", "Manage devices")])]
    case 13n: List.concat(&2, D.Node, [[D.El{"p", "", [D.At{"class", "set-about"}], [D.Txt{U.Fab.set.about()}]}],
      [View.setrow(U.Fab.set.order.label(), U.Fab.set.order.note(), View.chips.loc(lc, ["on", "off"], U.Fab.set.enabled(st), "order.enabled"))],
      [View.setrow(U.Fab.set.label(), U.Fab.set.note.k(U.Fab.enabled(st)), Bool.pick(List<&2, D.Node>, U.Fab.enabled(st), View.chips.loc(lc, ["on", "off"], U.Fab.set.on(st), "order.estimates"), Nil{}))]])
    case _: Nil{}

def View.settings.rows(i: Nat, +lc: I.Loc, +u: U.Ui, +st: M.State, +nf: Map<&2, String>, +v: String, +note: String) -> List<&2, D.Node>:
  +prov = M.Setting.get(st, "agent.provider", "claude")
  +ipc = M.Setting.get(st, "kicad.ipc", "on")
  View.settings.rows.k(i, lc, u, st, nf, v, note, prov, ipc)

def View.settings.menu(ps: List<&2, String>, +lc: I.Loc) -> List<&2, D.Node>:
  match ps:
    case Nil{}: Nil{}
    case +p <> t:
      SM.Menu.card(lc, p, "") <> View.settings.menu(t, lc)

# the menu with Settings' search over it (client.bend's Settings.found): the
# pages it finds, each with the rows that matched; Enter opens the first
def View.settings.find.btn(+lc: I.Loc, +p: String, +hint: String) -> D.Node:
  SM.Menu.card(lc, p, hint)

def View.settings.found(ps: List<&2, String>, +lc: I.Loc, +q: String) -> List<&2, D.Node>:
  match ps:
    case Nil{}: Nil{}
    case +p <> t: View.settings.find.btn(lc, p, U.Settings.hint(lc, p, q)) <> View.settings.found(t, lc, q)

def View.settings.none(none: Bool, +lc: I.Loc) -> List<&2, D.Node>:
  match none:
    case True{}: [D.El{"p", "", [D.At{"class", "set-about"}], [D.Txt{MSG.T.settings.search.none(lc)}]}]
    case False{}: Nil{}

def View.settings.root(+lc: I.Loc, +u: U.Ui) -> List<&2, D.Node>:
  SM.Menu.root(lc, u)

def View.settings.body(root: Bool, +lc: I.Loc, +u: U.Ui, +st: M.State, +nf: Map<&2, String>, +v: String, +note: String) -> List<&2, D.Node>:
  match root:
    case True{}: View.settings.root(lc, u)
    case False{}: List.append(&2, D.Node, SM.Menu.context(lc, U.Settings.page(u)), View.settings.rows(C.Codec.pos(U.Settings.pages(), U.Settings.page(u)), lc, u, st, nf, v, note))

# Settings' own head (its name, Close); under the top bar, whose crumb names
# it and whose gear closes it, there is none
def View.settings.head(top: Bool, +lc: I.Loc) -> List<&2, D.Node>:
  match top:
    case True{}: Nil{}
    case False{}:
      [D.El{"header", "", [D.At{"class", "thread-head settings-head"}], [
        D.Dom.span("title", MSG.T.palette.settings(lc)), D.Dom.span("meta", ""),
        D.Dom.button("", "flag", "settings", MSG.T.settings.close(lc), MSG.T.common.close(lc))]}]

# Settings: its path, then the menu, or the open page beside the categories
# (settings-menu.bend's Menu.side, shown on a wide page only)
def View.settings.loc(+lc: I.Loc, +u: U.Ui, +st: M.State, +nf: Map<&2, String>, +v: String, +note: String) -> D.Node:
  +page = U.Settings.page(u)
  D.El{"main", "settings", [D.At{"class", "settings-view"}], List.append(&2, D.Node, View.settings.head(View.chrome(u), lc), [
    D.El{"nav", "settings-path", [D.At{"class", "settings-path"}, D.At{"aria-label", MSG.T.palette.settings(lc)}],
      D.Dom.cat(D.Dom.when(Bool.not(String.is_empty(page)), D.Dom.button("", "settings-page", "", MSG.T.palette.settings(lc), "‹ " ++ MSG.T.palette.settings(lc))),
        [D.El{"h2", "settings-title", [D.At{"id", "settings-title"}, D.At{"tabindex", "-1"}], [D.Txt{U.Settings.title(lc, page)}]}])},
    D.El{"div", "settings-split", [D.At{"class", "settings-split"}], D.Dom.cat(D.Dom.when(Bool.not(String.is_empty(page)), SM.Menu.side(lc, page)), [
      D.El{"div", "settings-content", [D.At{"id", "settings-content"}, D.At{"class", "settings"}, D.At{"data-settings-page", page}], View.settings.body(String.is_empty(page), lc, u, st, nf, v, note)}])}])}

def View.settings(+u: U.Ui, +st: M.State, +nf: Map<&2, String>, +v: String, +note: String) -> D.Node:
  View.settings.loc(U.Lang.loc(u), u, st, nf, v, note)

# with no project yet, its first step is a button too (the folder picker,
# as the sidebar's +), ahead of the agent setup
def View.empty.add(none: Bool, +lc: I.Loc) -> List<&2, D.Node>:
  match none:
    case True{}: [D.Dom.button("primary", "proj-open", "", MSG.T.sidebar.add.project(lc), MSG.T.sidebar.add.project(lc))]
    case False{}: Nil{}

# (tour: the first-run tour is up, and the KiCad notice waits for it: U.Tour.kn)
def View.empty.t.loc(+lc: I.Loc, tour: Bool, +n: Nat, nf: Map<&2, String>, st: M.State) -> D.Node:
  D.El{"main", "empty", [D.At{"class", "empty"}], List.concat(&2, D.Node, [[D.Txt{Bool.pick(String, Nat.is_eq(n, 0n), MSG.T.empty.add(lc), MSG.T.empty.thread(lc))}], View.empty.add(Nat.is_eq(n, 0n), lc),
    [D.Dom.button("", "provider-setup-open", "", "Choose and connect an agent", "Set up an agent")], View.kn.loc(lc, U.Tour.kn.k(tour, U.KN.visible(nf, st)), True{})])}

def View.empty.loc(+lc: I.Loc, n: Nat, nf: Map<&2, String>, st: M.State) -> D.Node:
  View.empty.t.loc(lc, False{}, n, nf, st)

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

# a completion's label: the part typed ("sens") bright, the rest dim
# (client.bend's Proj.comps)
def View.prow.label(comp: Bool, +l: String, +typed: Nat) -> List<&2, D.Node>:
  match comp:
    case True{}:
      [D.Dom.span("pt", String.take(l, typed)), D.Dom.span("pr", String.drop(l, typed))]
    case False{}:
      [D.Txt{l}]

# at the right: a project's path, or Tab for the completion it writes
def View.prow.detail(tab: Bool, +u: U.Ui, it: U.PItem) -> String:
  match tab:
    case True{}:
      "Tab"
    case False{}:
      U.Proj.detail(u, it)

# a row: its label, and a project's path at the right (the first `comps`
# rows a typed path's completions, `typed` characters of each typed)
def View.prows(+u: U.Ui, ps: List<&2, U.PItem>, +i: Nat, +sel: Nat, +comps: Nat, +typed: Nat) -> List<&2, D.Node>:
  match ps:
    case Nil{}:
      Nil{}
    case U.PItem{+l, +a, +v} <> t:
      D.El{"button", "", [D.At{"class", View.prow.cls(a) ++ Bool.pick(String, Nat.is_eq(i, sel), " sel", "")}, D.At{"title", l}, D.At{"data-value", v}, D.On{"click", a}],
        [D.El{"span", "", [D.At{"class", "pl"}], View.prow.label(Nat.is_lt(i, comps), l, typed)}, D.El{"span", "", [D.At{"class", "pd"}], [D.Txt{View.prow.detail(Bool.and(Nat.is_eq(i, sel), Nat.is_lt(i, comps)), u, U.PItem{l, a, v})}]}]} <> View.prows(u, t, 1n+i, sel, comps, typed)

# Tab writes the selected completion into the field, as typing would
# (a form field's edit: thin-host keeps it), when there is one
def View.ptab(+tab: String) -> List<&2, D.Attr>:
  Bool.pick(List<&2, D.Attr>, String.is_empty(tab), Nil{}, [D.At{"data-tab", "bfield"}, D.At{"data-value", "proj\u{1f}" ++ tab}])

# above the filter: "Projects", or the folder browsed and the way back
def View.phead.loc(+lc: I.Loc, dir: Bool, +u: U.Ui) -> D.Node:
  match dir:
    case True{}:
      D.El{"div", "", [D.At{"class", "dir"}], [D.Dom.button("pback", "proj-back", "", MSG.T.picker.back(lc), MSG.T.tour.title.projects(lc)), D.Txt{U.Proj.dir(u)}]}
    case False{}:
      D.El{"div", "", [D.At{"class", "dir"}], [D.Txt{MSG.T.tour.title.projects(lc)}]}

def View.phead(dir: Bool, +u: U.Ui) -> D.Node:
  View.phead.loc(I.En{}, dir, u)

# a phone's picker fills the screen, so its backdrop cannot be tapped:
# it gets a × of its own (proj-close, as the backdrop and Esc)
def View.picker.close(phone: Bool, +lc: I.Loc) -> List<&2, D.Node>:
  match phone:
    case True{}: [WI.WIcon.button("picker-close", "proj-close", "", MSG.T.common.close(lc), "x", 22, "")]
    case False{}: Nil{}

def View.picker(+lc: I.Loc, open: Bool, +u: U.Ui) -> List<&2, D.Node>:
  match open:
    case True{}:
      [D.El{"div", "", [D.At{"class", "modal-back"}, D.On{"click", "proj-close"}], Nil{}},
        D.El{"div", "picker", [D.At{"class", "picker"}], D.Dom.cat(List.append(&2, D.Node, View.picker.close(PHN.Phone.on(u), lc), [
          View.phead.loc(lc, U.Proj.browsing(u), u),
          D.Dom.div("subthread-route-note", D.Dom.when(U.ThreadTree.routing(u), D.Dom.span("", U.ThreadTree.route.note()))),
          D.Dom.div("desktop-folder", D.Dom.when(Bool.and(String.eq(U.Ui.scratch(u, "@desktop"), "1"), Bool.not(U.ThreadTree.routing(u))), D.Dom.button("", "desktop-folder", "", "Choose a local folder", "Choose folder…"))),
          D.El{"input", "proj", List.append(&2, D.Attr, [D.At{"id", "proj"}, D.At{"placeholder", Bool.pick(String, U.Proj.browsing(u), MSG.T.picker.folder.placeholder(lc), MSG.T.picker.project.placeholder(lc))},
            D.At{"autocomplete", "off"}, D.At{"value", U.Proj.query(u)}, D.At{"data-bfield", "proj"}, D.On{"input", "bfield"}, D.At{"data-enter", "proj-enter"}], View.ptab(U.Proj.tab(u))), Nil{}}]),
          D.Dom.cat(Bool.pick(List<&2, D.Node>, String.is_empty(U.Ui.info(u, "proj.err")), Nil{}, [D.El{"div", "", [D.At{"class", "perr"}], [D.Txt{U.Ui.info(u, "proj.err")}]}]),
            [D.El{"div", "", [D.At{"class", "prows"}], View.prows(u, U.Proj.items.loc(lc, u), 0n, U.Proj.index(u), U.Proj.comps.n(u), U.Proj.typed.n(u))}]))}]
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

def View.message.images(+entry: String, ps: List<&2, String>) -> List<&2, D.Node>:
  match ps:
    case Nil{}:
      Nil{}
    case +p <> t:
      D.El{"a", "", [D.At{"class", "shot"}, D.At{"href", IM.Img.message_url(entry, p)}, D.At{"target", "_blank"}, D.At{"title", p}, D.At{"data-lightbox", IM.Img.message_url(entry, p)}],
        [D.El{"img", "", [D.At{"src", IM.Img.message_url(entry, p)}, D.At{"alt", p}, D.At{"loading", "lazy"}, D.At{"decoding", "async"}, D.At{"data-message-image", "1"}], Nil{}}, D.Dom.span("message-image-unavailable", "Image unavailable")]} <> View.message.images(entry, t)

# a run of tool calls, or a turn's work, folded into one row: a drawer's
# head, one line (FO.Fold decides the runs, their labels and which are
# open: only the person's click opens one); a click opens or shuts it. The
# live run's head ("live": the last thing while its turn runs) says what
# runs now and pulses (style.css)
def View.fold.cls(+open: Bool, +live: Bool) -> String:
  "fold" ++ Bool.pick(String, open, " open", "") ++ Bool.pick(String, live, " live", "")

def View.fold.accessible(open: Bool, node: D.Node) -> D.Node:
  match node:
    case D.El{tag, key, attrs, kids}: D.El{tag, key, D.At{"aria-expanded", Bool.pick(String, open, "true", "false")} <> attrs, kids}
    case other: other

def View.fold.row.live(+lc: I.Loc, +id: String, +label: String, +open: Bool, +live: Bool) -> D.Node:
  View.fold.accessible(open, WI.WIcon.button(View.fold.cls(open, live), "fold", FO.Fold.ask(open, id), MSG.T.fold.tooltip(lc, Bool.pick(String, open, "yes", "no"), Bool.pick(String, String.starts_with(id, "work:"), "work", "tools")),
    Bool.pick(String, open, "chevron-down", "chevron-right"), 14, label))

def View.fold.row.loc(+lc: I.Loc, +id: String, +label: String, +open: Bool) -> D.Node:
  View.fold.row.live(lc, id, label, open, False{})

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
    case FO.RHead{+id, +label, +open, +live}:
      View.fold.row.live(lc, id, label, open, live) <> View.fold.shown(open, ns)
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
      D.El{"span", "", [D.At{"style", View.run.style(fg, bg, bo, ul)}], TCELL.Cells.of(t)} <> View.runs(rest)

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

# An open run's rows say which run they are in (data-run, its first
# entry's id; on a memo, View.make passes it on): the drawer's body in
# style.css, and in the thin view the timeline's history drops them once
# their run is shut (timeline-history.bend's H.shut)
def View.run.el(none: Bool, +run: String, +t: String, +k: String, as: List<&2, D.Attr>, ks: List<&2, D.Node>) -> D.Node:
  match none:
    case True{}:
      D.El{t, k, as, ks}
    case False{}:
      D.El{t, k, List.append(&2, D.Attr, as, [D.At{"data-run", run}]), ks}

def View.act.now(now: Bool) -> String:
  match now:
    case True{}:
      " now"
    case False{}:
      ""

def View.act.now.note(+lc: I.Loc, now: Bool) -> List<&2, D.Node>:
  match now:
    case True{}:
      [D.Dom.span("now", MSG.T.fold.now(lc))]
    case False{}:
      Nil{}

# the call a live run is on: its last entry (the live run is the last thing
# of its thread), "data-now" for View.make
def View.now.mark(now: Bool, ns: List<&2, D.Node>) -> List<&2, D.Node>:
  match now ns:
    case True{} D.El{t, k, as, ks} <> r:
      D.El{t, k, List.append(&2, D.Attr, as, [D.At{"data-now", "1"}]), ks} <> r
    case _ other:
      other

def View.run.mark(+run: String, ns: List<&2, D.Node>) -> List<&2, D.Node>:
  match ns:
    case Nil{}:
      Nil{}
    case D.El{t, k, as, ks} <> r:
      View.run.el(String.is_empty(run), run, t, k, as, ks) <> View.run.mark(run, r)
    case n <> r:
      n <> View.run.mark(run, r)

# a tool's name as the row shows it (FO.Tool.shown: "viewer_show" for
# "mcp__backplane__viewer_show", "To-do" for TodoWrite), the full name in
# its tooltip when they differ
def View.act.kind.k(same: Bool, +shown: String, +kind: String) -> D.Node:
  match same:
    case True{}:
      D.Dom.span("kind", shown)
    case False{}:
      D.El{"span", "", [D.At{"class", "kind"}, D.At{"title", kind}], [D.Txt{shown}]}

def View.act.kind(+lc: I.Loc, +kind: String) -> D.Node:
  +shown = FO.Tool.shown(lc, kind)
  View.act.kind.k(String.eq(shown, kind), shown, kind)

def View.act.spans(+lc: I.Loc, review: Bool, +tone: String, +kind: String, +summary: String) -> List<&2, D.Node>:
  match review:
    case True{}:
      [D.Dom.span("kind", RVW.Review.label(tone, kind, summary)), D.Dom.span("summary", RVW.Review.say(tone, kind, summary))]
    case False{}:
      [View.act.kind(lc, kind), D.Dom.span("summary", FO.Tool.text(kind, summary))]

# an act (a tool call, an error...): made by the host from these alone
# (View.make)
def View.act.n(+id: String, +tone: String, +kind: String, +summary: String) -> D.Node:
  D.El{"div", id, [D.At{"class", "act " ++ tone}, D.At{"data-id", id}], View.act.spans(I.En{}, String.eq(tone, RVW.Review.tone()), tone, kind, summary)}

# the same inside an open run's drawer (View.run.mark: its run's id); now:
# the call a live run is on (View.now.mark), marked like the live fold's row
def View.act.in(+lc: I.Loc, +id: String, +tone: String, +kind: String, +summary: String, +run: String, +now: Bool) -> D.Node:
  View.run.el(String.is_empty(run), run, "div", id, [D.At{"class", "act " ++ tone ++ View.act.now(now)}, D.At{"data-id", id}],
    List.append(&2, D.Node, View.act.spans(lc, String.eq(tone, RVW.Review.tone()), tone, kind, summary), View.act.now.note(lc, now)))

def View.act.g.loc(+lc: I.Loc, rem: Bool, +id: String, +tone: String, +kind: String, +summary: String, +at: Nat, +st: M.State) -> D.Node:
  match rem:
    case True{}:
      D.El{"div", id, [D.At{"class", "act remind entry-remind"}, D.At{"data-id", id}],
        [WI.WIcon.svg("bell", 14), D.Dom.span("kind", MSG.T.sched.kind.reminder(lc)), D.Dom.span("summary", summary), D.Dom.span("when", TZ.Tz.show.nz(SCH.Sched.person(st, 0n, True{}), at))]}
    case False{}:
      View.memo.loc(lc, id, [D.At{"fn", "act"}, D.At{"tone", FO.Tool.tone(tone, kind)}, D.At{"kind", kind}], summary)

def View.act.g(rem: Bool, +id: String, +tone: String, +kind: String, +summary: String, +at: Nat, +st: M.State) -> D.Node:
  View.act.g.loc(I.En{}, rem, id, tone, kind, summary, at, st)

# A thought (model.bend's Think): folded under "Thinking" and its first
# line, the whole of it (Markdown) when opened; while the setting is off it
# is not shown at all
def View.think.clean(+l: String) -> String:
  String.take(String.trim(String.join(String.split(String.join(String.split(l, '*'), ""), '#'), "")), 160n)

def View.think.head(ls: List<&2, String>, +acc: String) -> String:
  match ls:
    case Nil{}:
      acc
    case +l <> t:
      View.think.head(t, Bool.pick(String, String.is_empty(acc), View.think.clean(l), acc))

def View.think.loc(+lc: I.Loc, on: Bool, +id: String, +text: String) -> D.Node:
  match on:
    case True{}:
      D.El{"details", id, [D.At{"class", "act thinking"}, D.At{"data-id", id}], [
        D.El{"summary", "", Nil{}, [D.Dom.span("kind", "Thinking"), D.Dom.span("summary", View.think.head(String.split(text, '\n'), ""))]},
        D.El{"div", "", [D.At{"class", "body"}], D.Md.render.loc(lc, text)}]}
    case False{}:
      D.El{"div", id, [D.At{"class", "act thinking"}, D.At{"data-id", id}, D.At{"hidden", ""}], Nil{}}

def View.act.t.loc(+lc: I.Loc, term: Bool, e: M.Entry, +st: M.State, +ec: U.Ec) -> D.Node:
  match term e:
    case True{} M.Act{+id, +th, tone, kind, sum, +at}:
      View.emb.loc(lc, ec, st, M.Act{id, th, tone, kind, sum, at}, th)
    case _ M.Act{+id, _, +tone, +kind, +summary, +at}:
      Bool.pick(D.Node, String.eq(tone, M.Think.tone()), View.think.loc(lc, M.Think.on(st), id, summary),
        View.act.g.loc(lc, String.eq(tone, "remind"), id, tone, kind, summary, at, st))
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

# Bot chats (core/reply.bend): a message's Reply button ("reply-to"), the
# "In reply to" line over a person's reply (its link goes to the message,
# "hist-jump", which brings it into view and flashes it) and the header
# over a bot's message naming the thread or project it links most
# ("ref-go"; "+N" the others). All come from the memo's attributes:
# rpl "1", rto/rtoq, ctx/ctxt/ctxn (View.entry.s.loc).
def View.rpl.button(+lc: I.Loc, on: Bool, +id: String) -> List<&2, D.Node>:
  match on:
    case True{}: [D.El{"button", "", [D.At{"class", "reply"}, D.At{"title", MSG.T.reply.action(lc)}, D.At{"data-value", id}, D.On{"click", "reply-to"}], [D.Txt{MSG.T.reply.action(lc)}]}]
    case False{}: Nil{}

def View.rto.line(+lc: I.Loc, +to: String, +q: String) -> List<&2, D.Node>:
  match to:
    case SNil{}: Nil{}
    case SCon{+h, +t}:
      [D.El{"div", "", [D.At{"class", "msg-reply"}], [D.Dom.span("label", MSG.T.reply.to(lc)),
        D.El{"a", "", [D.At{"class", "rto"}, D.At{"href", "#"}, D.At{"data-to", SCon{h, t}}, D.At{"data-value", SCon{h, t}}, D.On{"click", "hist-jump"}], [D.Txt{Bool.pick(String, String.is_empty(q), SCon{h, t}, q)}]}]}]

def View.ctx.more(none: Bool, +n: String) -> List<&2, D.Node>:
  match none:
    case True{}: Nil{}
    case False{}: [D.Dom.span("more", "+" ++ n)]

def View.ctx.head(+uri: String, +title: String, +n: String) -> List<&2, D.Node>:
  match uri:
    case SNil{}: Nil{}
    case SCon{+h, +t}:
      [D.El{"div", "", [D.At{"class", "msg-ctx"}], D.Dom.cat([D.El{"a", "", [D.At{"class", "ref ctx"}, D.At{"href", SCon{h, t}}, D.At{"data-ref", SCon{h, t}}, D.At{"data-bare", "0"}, D.At{"data-value", SCon{h, t}}, D.On{"click", "ref-go"}], [D.Txt{title}]}], View.ctx.more(Bool.or(String.is_empty(n), String.eq(n, "0")), n))}]

# a reply's body: its markdown, or for a provider's API error (the memo's
# "err": core/fold.bend's ApiErr.say) one red sentence with the raw text
# under "› details" (its Copy still copies the raw text)
def View.msg.body(+lc: I.Loc, +err: String, +id: String, +text: String) -> List<&2, D.Node>:
  match err:
    case SNil{}:
      [D.El{"div", "", [D.At{"class", "body"}], D.Md.render.loc(lc, text)}]
    case SCon{+h, +t}:
      [D.El{"div", "", [D.At{"class", "body apierr"}], [D.Dom.span("say", SCon{h, t}),
        D.El{"details", "apierr:" ++ id, [D.At{"class", "apierr-raw"}], [D.El{"summary", "", Nil{}, [D.Txt{MSG.T.apierr.details(lc)}]}, D.El{"pre", "", Nil{}, [D.Txt{text}]}]}]}]

def View.msg.x.loc(+lc: I.Loc, user: Bool, +id: String, +text: String, +stamp: String, +as: List<&2, D.Attr>) -> D.Node:
  match user:
    case True{}:
      D.El{"div", id, [D.At{"class", Bool.pick(String, String.eq(View.attr(as, "unread"), "1"), "msg user unread", "msg user")}, D.At{"data-id", id}], D.Dom.cat(View.rto.line(lc, View.attr(as, "rto"), View.attr(as, "rtoq")), D.Dom.cat(View.user_body(text), D.Dom.cat(View.images(AT.Att.images(text)), D.Dom.cat([D.Dom.span("stamp", stamp), D.Dom.copy.loc(lc, AT.Att.body(text))], View.rpl.button(lc, String.eq(View.attr(as, "rpl"), "1"), id)))))}
    case False{}:
      D.El{"div", id, [D.At{"class", "msg assistant"}, D.At{"data-id", id}], D.Dom.cat(View.ctx.head(View.attr(as, "ctx"), View.attr(as, "ctxt"), View.attr(as, "ctxn")), D.Dom.cat(View.msg.body(lc, View.attr(as, "err"), id, text), D.Dom.cat(View.message.images(id, IM.Img.paths(text)), D.Dom.cat([D.Dom.span(Bool.pick(String, String.eq(View.attr(as, "mid"), "1"), "stamp mid", "stamp"), stamp), D.Dom.copy.loc(lc, text)], View.rpl.button(lc, String.eq(View.attr(as, "rpl"), "1"), id)))))}

def View.msg.k.loc(+lc: I.Loc, user: Bool, +id: String, +text: String, +stamp: String) -> D.Node:
  View.msg.x.loc(lc, user, id, text, stamp, Nil{})

def View.msg.k(user: Bool, +id: String, +text: String, +stamp: String) -> D.Node:
  View.msg.k.loc(I.En{}, user, id, text, stamp)

def View.msg.loc(+lc: I.Loc, +id: String, +who: String, +text: String, +stamp: String) -> D.Node:
  View.msg.k.loc(lc, String.eq(who, "u"), id, text, stamp)

def View.msg(+id: String, +who: String, +text: String, +stamp: String) -> D.Node:
  View.msg.loc(I.En{}, id, who, text, stamp)

def View.make.r.loc(+lc: I.Loc, row: Bool, +k: String, +as: List<&2, D.Attr>, +text: String) -> D.Node:
  match row:
    case True{}:
      View.row.p.loc(lc, String.eq(View.attr(as, "pin"), "1"), k, text, View.attr(as, "ago"), View.attr(as, "badge"), View.attr(as, "want"), View.attr(as, "dot"), Bool.pick(String, String.eq(View.attr(as, "on"), "1"), k, ""), String.eq(View.attr(as, "mov"), "1"))
    case False{}:
      View.act.in(lc, k, View.attr(as, "tone"), View.attr(as, "kind"), text, View.attr(as, "data-run"), String.eq(View.attr(as, "data-now"), "1"))

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
      View.msg.x.loc(lc, String.eq(View.attr(as, "who"), "u"), k, text, View.attr(as, "stamp"), as)
    case False{}:
      View.make.l.loc(lc, String.eq(fn, "live"), fn, k, as, text)

# the node a bp-memo element stands for

def View.make.f(msg: Bool, +fn: String, +k: String, +as: List<&2, D.Attr>, +text: String) -> D.Node:
  View.make.f.loc(I.En{}, msg, fn, k, as, text)

def View.make.icon(icon: Bool, +k: String, +as: List<&2, D.Attr>, ks: List<&2, D.Node>) -> D.Node:
  match icon:
    case True{}: WI.WIcon.tree.make(k)
    case False{}:
      View.make.f.loc(I.I18n.of_tag(View.attr(as, "lang")), String.eq(View.attr(as, "fn"), "msg"), View.attr(as, "fn"), k, as, View.txt(ks))

def View.make(n: D.Node) -> D.Node:
  match n:
    case D.El{_, +k, +as, +ks}:
      View.make.icon(String.eq(View.attr(as, "fn"), "tree-icon"), k, as, ks)
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

# a bot chat's memo attributes: Reply, and a person's "In reply to"
def View.rto.attrs(chat: Bool, m: Maybe<&2, RTO.RtoBlock>) -> List<&2, D.Attr>:
  match chat m:
    case False{} _: Nil{}
    case True{} Some{RTO.RtoBlock{+to, +q, _}}: [D.At{"rpl", "1"}, D.At{"rto", to}, D.At{"rtoq", RTO.Rto.snip(q)}]
    case True{} None{}: [D.At{"rpl", "1"}]

def View.ctx.kv(m: Maybe<&2, RTO.MsgCtx>) -> List<&2, D.Attr>:
  match m:
    case Some{RTO.MsgCtx{+u, +t, +n}}: [D.At{"ctx", u}, D.At{"ctxt", t}, D.At{"ctxn", Nat.show(n)}]
    case None{}: Nil{}

# a bot chat's memo attributes for a bot's message: Reply, and its header
def View.ctx.attrs(chat: Bool, +st: M.State, +text: String) -> List<&2, D.Attr>:
  match chat:
    case False{}: Nil{}
    case True{}: D.At{"rpl", "1"} <> View.ctx.kv(RTO.Ctx.of(st, text))

# a reply that is a provider's API error: its sentence (FO.ApiErr.say),
# made only for one
def View.err.attrs(+lc: I.Loc, api: Bool, +text: String) -> List<&2, D.Attr>:
  match api:
    case True{}: [D.At{"err", FO.ApiErr.say(lc, text)}]
    case False{}: Nil{}

# Backplane's continue message after a restart is not the person's: it is
# the "Continuing after the restart" row, what it said under
# "› what it was told" (FO.Restart.told; the hub's own row after it is
# left out, FO.Fold.shown)
def View.carry.row(+lc: I.Loc, +id: String, +text: String) -> D.Node:
  D.El{"div", id, [D.At{"class", "act info restart"}, D.At{"data-id", id}], [View.act.kind(lc, "restart"), D.El{"span", "", [D.At{"class", "summary"}], [D.Txt{MSG.T.restart.continuing(lc)},
    D.El{"details", "told:" ++ id, [D.At{"class", "carry-told"}], [D.El{"summary", "", Nil{}, [D.Txt{MSG.T.restart.told(lc)}]}, D.El{"div", "", [D.At{"class", "told"}], [D.Txt{text}]}]}]}]}

def View.entry.user(carry: Bool, +lc: I.Loc, +id: String, +th: String, +text: String, +st: M.State, +stamp: String, +unread: Bool) -> D.Node:
  match carry:
    case True{}:
      View.carry.row(lc, id, text)
    case False{}:
      View.memo.loc(lc, id, List.append(&2, D.Attr, [D.At{"fn", "msg"}, D.At{"who", "u"}, D.At{"stamp", STP.Stamp.unread.text(unread, lc, stamp)}, D.At{"unread", Bool.pick(String, unread, "1", "")}], View.rto.attrs(RTO.Rto.chat(st, th), RTO.Rto.of(text))), RTO.Rto.text(text))

# (mid: a reply inside a turn, its time shown on hover: core/stamp.bend's
# Stamp.mid)
# (unread: a person's message the agent has not read yet, core/stamp.bend's
# Stamp.unread: its time says so and it is a step dimmer)
def View.entry.u.loc(+lc: I.Loc, e: M.Entry, +st: M.State, +ec: U.Ec, +stamp: String, mid: Bool, +unread: Bool) -> D.Node:
  match e:
    case M.Msg{+id, +th, M.User{}, +text, _}:
      View.entry.user(FO.Restart.told(text), lc, id, th, text, st, stamp, unread)
    case M.Msg{+id, +th, _, +text, _}:
      View.memo.loc(lc, id, List.append(&2, D.Attr, [D.At{"fn", Bool.pick(String, CP.Pending.synthetic(id), "live", "msg")}, D.At{"who", "a"}, D.At{"stamp", stamp}, D.At{"mid", Bool.pick(String, mid, "1", "")}], List.append(&2, D.Attr, View.err.attrs(lc, FO.ApiErr.is(text), text), View.ctx.attrs(RTO.Rto.chat(st, th), st, CP.Pending.text(lc, id, text)))), CP.Pending.text(lc, id, text))
    case M.Act{+id, +th, +tone, +kind, +summary, +at}:
      View.act.loc(lc, String.eq(tone, HS.Step.tone()), M.Act{id, th, tone, kind, summary, at}, st, ec)

def View.entry.m.loc(+lc: I.Loc, e: M.Entry, +st: M.State, +ec: U.Ec, +stamp: String, mid: Bool) -> D.Node:
  View.entry.u.loc(lc, e, st, ec, stamp, mid, False{})

def View.entry.s.loc(+lc: I.Loc, e: M.Entry, +st: M.State, +ec: U.Ec, +stamp: String) -> D.Node:
  View.entry.m.loc(lc, e, st, ec, stamp, False{})

# an entry of the thread itself, with the time under a message (core/stamp.bend)

def View.entry.s(e: M.Entry, +st: M.State, +ec: U.Ec, +stamp: String) -> D.Node:
  View.entry.s.loc(I.En{}, e, st, ec, stamp)

def View.entry.own(+lc: I.Loc, +e: M.Entry, +st: M.State, +ec: U.Ec, +sp: STP.Stamps) -> D.Node:
  View.entry.u.loc(lc, e, st, ec, STP.Stamp.text.loc(lc, sp, e), STP.Stamp.mid(sp, e), STP.Stamp.unread(st, e))

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

# Match before constructing an entry. A folded body must not parse Markdown,
# build viewers or materialize a tool's detail only to discard it afterwards.
def View.role.body(r: FO.Role) -> Bool:
  match r:
    case FO.RShow{}: True{}
    case FO.RHead{_, _, open, _}: open
    case FO.RIn{open}: open
    case FO.RWork{_, _, open, inner}:
      match open:
        case True{}: View.role.body(inner)
        case False{}: False{}

def View.entry.role(show: Bool, +lc: I.Loc, +e: M.Entry, +st: M.State, +sel: String, +ec: U.Ec, +sp: STP.Stamps) -> List<&2, D.Node>:
  match show:
    case True{}: View.entry(lc, e, st, sel, ec, sp)
    case False{}: Nil{}

# the run an entry's role puts it in: a head's own, a later entry's the
# one before it, "" out of any run
def View.run.of(r: FO.Role, +run: String) -> String:
  match r:
    case FO.RHead{+id, _, _, _}: id
    case FO.RIn{_}: run
    case _: ""

# whether an entry's role puts it in the live run (live: the one before)
def View.run.live(r: FO.Role, live: Bool) -> Bool:
  match r:
    case FO.RHead{_, _, _, l}: l
    case FO.RIn{_}: live
    case _: False{}

def View.entries.run(+lc: I.Loc, es: List<&2, M.Entry>, rs: List<&2, FO.Role>, +st: M.State, +sel: String, +ec: U.Ec, +sp: STP.Stamps, +run: String, +live: Bool) -> List<&2, D.Node>:
  match es rs:
    case Nil{} _:
      Nil{}
    case e <> +t +r <> rt:
      +at = View.run.of(r, run)
      +on = View.run.live(r, live)
      List.append(&2, D.Node, View.role.loc(lc, r, View.now.mark(Bool.and(on, List.is_empty(&2, M.Entry, t)), View.run.mark(at, View.entry.role(View.role.body(r), lc, e, st, sel, ec, sp)))), View.entries.run(lc, t, rt, st, sel, ec, sp, at, on))
    case e <> t Nil{}:
      List.append(&2, D.Node, View.entry(lc, e, st, sel, ec, sp), View.entries.run(lc, t, Nil{}, st, sel, ec, sp, "", False{}))

# the tool calls around a page, without the design as it was before
def View.around.k(c: FO.Around) -> FO.Around:
  match c:
    case FO.Around{b, a}:
      FO.Around{HS.Hist.seen(b), HS.Hist.seen(a)}

def View.around(+st: M.State, +sel: String, +es: List<&2, M.Entry>) -> FO.Around:
  View.around.k(FO.Fold.context.of(st, sel, es))

def View.entries.r(+lc: I.Loc, es: List<&2, M.Entry>, rs: List<&2, FO.Role>, +st: M.State, +sel: String, +ec: U.Ec, +sp: STP.Stamps) -> List<&2, D.Node>:
  View.entries.run(lc, es, rs, st, sel, ec, sp, "", False{})

# a thread's entries, its runs of tool calls folded (the last stays open
# while its turn runs)
def View.entries.folded(+lc: I.Loc, +es: List<&2, M.Entry>, +st: M.State, +sel: String, +info: Map<&2, String>, +running: Bool, +ec: U.Ec) -> List<&2, D.Node>:
  View.entries.r(lc, es, CTXT.Context.roles(es, FO.Fold.roles.ctx(lc, View.around(st, sel, es), es, info, running), st, sel, info), st, sel, ec, STP.Stamps.of.w(es, st, FO.Fold.now(info), running, STP.Stamp.end(U.Ui.thread(st, sel)), SG.Signal.waiting(info, st, sel, running)))

# blank messages left out, and a run cut by the page's edge folded whole
# (FO.Fold.roles.ctx)
def View.entries.seen(+lc: I.Loc, es: List<&2, M.Entry>, +st: M.State, +sel: String, +info: Map<&2, String>, +running: Bool, +ec: U.Ec) -> List<&2, D.Node>:
  View.entries.folded(lc, FO.Fold.shown(es), st, sel, info, running, ec)

# (the design as it was before the edits is no change: HS.Hist.seen)
def View.entries.shown(+lc: I.Loc, es: List<&2, M.Entry>, +st: M.State, +sel: String, +info: Map<&2, String>, +running: Bool, +ec: U.Ec) -> List<&2, D.Node>:
  View.entries.seen(lc, HS.Hist.seen(es), st, sel, info, running, ec)

# Project both loaded and newly received activity at render time; no empty
# DOM row or timestamp survives an internal delivery receipt.
def View.entries.e(+lc: I.Loc, +es: List<&2, M.Entry>, +st: M.State, +sel: String, +info: Map<&2, String>, +running: Bool, +ec: U.Ec) -> List<&2, D.Node>:
  View.entries.shown(lc, BI.BU.chat.entries(st, sel, es), st, sel, info, running, ec)

def View.entries(+lc: I.Loc, +es: List<&2, M.Entry>, +st: M.State, +sel: String, +info: Map<&2, String>, +running: Bool) -> List<&2, D.Node>:
  View.entries.e(lc, es, st, sel, info, running, U.Ec.none())

def View.paged.w(+lc: I.Loc, w: U.Win, +st: M.State, +sel: String, +info: Map<&2, String>, +running: Bool, +ec: U.Ec) -> List<&2, D.Node>:
  match w:
    case U.Win{+h, es}:
      List.append(&2, D.Node, View.earlier(h, sel), View.entries.shown(lc, es, st, sel, info, running, ec))

def View.paged(+lc: I.Loc, +drafts: Map<&2, String>, +es: List<&2, M.Entry>, +st: M.State, +sel: String, +info: Map<&2, String>, +running: Bool, +ec: U.Ec) -> List<&2, D.Node>:
  View.paged.w(lc, U.Page.newest.visible.from(Bool.not(BI.BU.chat.summary(st, sel)), es, sel, U.Page.since(st, sel), U.Hold.page(es, U.Str.get(drafts, "@away." ++ sel), U.Page.count(Maybe.default(&2, Nat, Nat.read(U.Str.get(drafts, "@more." ++ sel)), 0n)))), st, sel, info, running, ec)

# the thread's shell: its screen, and over it a transparent field that
# takes keys (and a phone's keyboard)
def View.term.scroll.loc(+lc: I.Loc, +v: VT.Vt, +scroll: Nat) -> D.Node:
  D.El{"section", "term-pane", [D.At{"class", "term-pane"}], [
    D.El{"div", "", [D.At{"class", "term-head"}], [D.Dom.span("label", MSG.T.palette.terminal(lc) ++ Bool.pick(String, String.is_empty(VT.Vt.title(v)), "", " · " ++ VT.Vt.title(v))),
      WI.WIcon.button("icon", "term-toggle", "", MSG.T.terminal.close(lc), "x", 16, "")]},
    D.El{"div", "", [D.At{"class", "screen"}, D.At{"data-wheel", "ws-term-scroll"}, D.At{"style", "background:" ++ View.hex(VT.Vt.bg()) ++ ";color:" ++ View.hex(VT.Vt.fg())}],
      D.Dom.cat(View.tlines(VT.Vt.lines(v, scroll)), D.Dom.cat(Bool.pick(List<&2, D.Node>, Nat.is_eq(scroll, 0n), View.cursor(VT.Vt.cursor(v)), Nil{}),
        [D.El{"textarea", "term", [D.At{"id", "term"}, D.At{"class", "term-in"}, D.At{"data-keys", "term-key"}, D.At{"data-text", "term-paste"},
          D.At{"autocapitalize", "off"}, D.At{"autocomplete", "off"}, D.At{"spellcheck", "false"}, D.At{"aria-label", MSG.T.terminal.input(lc)}], Nil{}}]))}]}

def View.term.loc(lc: I.Loc, v: VT.Vt) -> D.Node:
  View.term.scroll.loc(lc, v, 0n)

def View.term(+v: VT.Vt) -> D.Node:
  View.term.loc(I.En{}, v)

# Diff
# ----

def View.dl.cls(+k: U32) -> String:
  Bool.pick(String, U32.is_eq(k, 1), "dl add", Bool.pick(String, U32.is_eq(k, 2), "dl del", Bool.pick(String, U32.is_eq(k, 3), "dl meta", "dl")))

def View.dl.no(+n: Nat) -> String:
  Bool.pick(String, Nat.is_eq(n, 0n), "", Nat.show(n))

def View.dline(l: DF.DLine) -> D.Node:
  match l:
    case DF.DLine{+k, t, +o, +n}:
      D.El{"div", "", [D.At{"class", View.dl.cls(k)}], [D.Dom.span("no", View.dl.no(o)), D.Dom.span("no", View.dl.no(n)),
        D.Dom.span("sign", Bool.pick(String, U32.is_eq(k, 1), "+", Bool.pick(String, U32.is_eq(k, 2), "-", " "))), D.Dom.span("code", t)]}

# The diff's lines are shown up to View.diff.lines() in all: a view page is
# at most 256 KB, and a diff of a few MB (a thread that regenerated its STEP
# models) showed nothing at all. Every file keeps its head; one cut short
# says how many of its lines are not shown.
def View.diff.lines() -> Nat:
  120n

# and up to View.diff.files() files (1788 heads alone passed the page; a
# viewer of 150 files and 600 lines was 215 KB of a 229 KB page budget,
# and one of 60 and 250 still left the page 235 KB)
def View.diff.files() -> Nat:
  30n

# the first n lines
def View.dlines.b(ls: List<&2, DF.DLine>, n: Nat) -> List<&2, D.Node>:
  match ls n:
    case +_ 0n:
      Nil{}
    case Nil{} 1n+_:
      Nil{}
    case +l <> rest 1n+k:
      View.dline(l) <> View.dlines.b(rest, k)

def View.dhunks.count(hs: List<&2, DF.DHunk>, acc: Nat) -> Nat:
  match hs:
    case Nil{}:
      acc
    case DF.DHunk{_, +ls} <> t:
      View.dhunks.count(t, Nat.add(acc, List.length(&2, DF.DLine, ls)))

# hunks with left lines to show between them (no head once none is left)
def View.dhunks.b(hs: List<&2, DF.DHunk>, +left: Nat) -> List<&2, D.Node>:
  match hs:
    case Nil{}:
      Nil{}
    case DF.DHunk{+hd, +ls} <> t:
      D.Dom.cat(Bool.pick(List<&2, D.Node>, Bool.or(String.is_empty(hd), Nat.is_eq(left, 0n)), Nil{}, [D.Dom.span("dhunk", hd)]),
        D.Dom.cat(View.dlines.b(ls, left), View.dhunks.b(t, Nat.sub(left, List.length(&2, DF.DLine, ls)))))

def View.dfile.more(+lc: I.Loc, +n: Nat) -> List<&2, D.Node>:
  Bool.pick(List<&2, D.Node>, Nat.is_eq(n, 0n), Nil{}, [D.El{"div", "", [D.At{"class", "dl meta"}], [D.Dom.span("code", MSG.T.viewer.diff.more(lc, n))]}])

def View.dfiles.rest(+lc: I.Loc, +n: Nat) -> List<&2, D.Node>:
  Bool.pick(List<&2, D.Node>, Nat.is_eq(n, 0n), Nil{}, [D.El{"div", "", [D.At{"class", "dfile"}], [D.El{"div", "", [D.At{"class", "dhead"}], [D.Dom.span("name", MSG.T.viewer.diff.files.more(lc, n))]}]}])

# files with left lines and room files more to show
def View.dfiles.b(fs: List<&2, DF.DFile>, +lc: I.Loc, +left: Nat, room: Nat) -> List<&2, D.Node>:
  match fs room:
    case +rest 0n:
      View.dfiles.rest(lc, List.length(&2, DF.DFile, rest))
    case Nil{} 1n+_:
      Nil{}
    case DF.DFile{_, +nw, st, +hs} <> t 1n+k:
      +n = View.dhunks.count(hs, 0n)
      D.El{"div", nw, [D.At{"class", "dfile"}], D.El{"div", "", [D.At{"class", "dhead"}], [D.Dom.span("name", nw), D.Dom.span("status", st)]}
        <> List.append(&2, D.Node, View.dhunks.b(hs, left), View.dfile.more(lc, Nat.sub(n, left)))} <> View.dfiles.b(t, lc, Nat.sub(left, n), k)

def View.dfiles(fs: List<&2, DF.DFile>) -> List<&2, D.Node>:
  View.dfiles.b(fs, I.En{}, View.diff.lines(), View.diff.files())

# a git action of the diff's head; with no changes there is nothing to
# commit, push or put back, and it is shown disabled (no accent)
def View.diff.btn.off(+none: Bool, +off: Bool, +cls: String, +action: String, +value: String, +tip: String, +label: String) -> D.Node:
  D.El{"button", "", List.append(&2, D.Attr, [D.At{"class", Bool.pick(String, none, "", cls)}, D.At{"title", tip}, D.At{"data-value", value}, D.On{"click", action}],
    Bool.pick(List<&2, D.Attr>, Bool.or(none, off), [D.At{"disabled", "true"}], Nil{})), [D.Txt{label}]}

def View.diff.btn(+none: Bool, +cls: String, +action: String, +value: String, +tip: String, +label: String) -> D.Node:
  View.diff.btn.off(none, False{}, cls, action, value, tip, label)

# what this thread changed, with the git actions (as the native Lay.diff);
# off: the diff kept from before, drawn while git runs again (View.diff.at),
# its actions off until git's answer replaces it
def View.diff.loc.off(+lc: I.Loc, +text: String, +off: Bool) -> List<&2, D.Node>:
  +fs = DF.Diff.parse(text)
  +none = List.is_empty(&2, DF.DFile, fs)
  [D.El{"div", "", [D.At{"class", "diff-head"}], [
      D.Dom.span("summary", Bool.pick(String, none, MSG.T.viewer.diff.empty(lc), DF.Diff.summary(fs))),
      D.El{"div", "", [D.At{"class", "diff-btns"}], [
        View.diff.btn.off(none, off, "", "revert", "ask", MSG.T.diff.revert.tip(lc), MSG.T.palette.revert(lc)),
        View.diff.btn.off(none, off, "", "git", "commit_push_pr", MSG.T.diff.pr.tip(lc), MSG.T.viewer.diff.pr(lc)),
        View.diff.btn.off(none, off, "primary", "git", "commit_push", MSG.T.diff.commit.tip(lc), MSG.T.palette.commit(lc))]}]},
    D.El{"div", "", [D.At{"class", "diff-body"}], View.dfiles.b(fs, lc, View.diff.lines(), View.diff.files())}]

def View.diff.loc(+lc: I.Loc, +text: String) -> List<&2, D.Node>:
  View.diff.loc.off(lc, text, False{})

def View.diff(+text: String) -> List<&2, D.Node>:
  View.diff.loc(I.En{}, text)

# a thread whose folder is no git repository has nothing to compare with:
# the pane says so plainly and how to make it one, with no git actions
def View.diff.nogit(+lc: I.Loc, +path: String) -> List<&2, D.Node>:
  [D.El{"div", "", [D.At{"class", "diff-head"}], [D.Dom.span("summary", MSG.T.viewer.diff.nogit(lc))]},
   D.El{"div", "", [D.At{"class", "diff-body diff-none"}], [
     D.El{"p", "", Nil{}, [D.Txt{MSG.T.viewer.diff.nogit.why(lc, path)}]},
     D.El{"p", "", Nil{}, [D.Txt{MSG.T.viewer.diff.nogit.how(lc)}]},
     D.El{"pre", "", Nil{}, [D.Txt{"cd " ++ CAD.Cad.quote(path) ++ "\ngit init\ngit add -A\ngit commit -m start"}]},
     D.Dom.button("primary", "git", "init", MSG.T.viewer.diff.nogit.init.tip(lc), MSG.T.viewer.diff.nogit.init(lc))]}]

def View.diff.at.k(git: Bool, +lc: I.Loc, +text: String, +none: String) -> List<&2, D.Node>:
  match git:
    case True{}:
      View.diff.loc(lc, text)
    case False{}:
      View.diff.nogit(lc, none)

# a Diff asked for and not answered yet (client.bend's Dv.open): the
# pane's own head, its buttons off, "Loading…" where the summary goes
def View.diff.wait(+lc: I.Loc) -> List<&2, D.Node>:
  [D.El{"div", "", [D.At{"class", "diff-head"}], [
      D.Dom.span("summary", MSG.T.viewer.loading(lc)),
      D.El{"div", "", [D.At{"class", "diff-btns"}], [
        View.diff.btn(True{}, "", "revert", "ask", MSG.T.diff.revert.tip(lc), MSG.T.palette.revert(lc)),
        View.diff.btn(True{}, "", "git", "commit_push_pr", MSG.T.diff.pr.tip(lc), MSG.T.viewer.diff.pr(lc)),
        View.diff.btn(True{}, "primary", "git", "commit_push", MSG.T.diff.commit.tip(lc), MSG.T.palette.commit(lc))]}]},
    D.El{"div", "", [D.At{"class", "diff-body"}], Nil{}}]

# a Diff asked for, drawn with the thread's diff kept from before (kept:
# {diff, nogit} as this page or the hub had it last, client.bend's Dv.text,
# "diff.kept:t") until git answers, else "Loading…"
def View.diff.kept.k(git: Bool, +lc: I.Loc, +text: String, +none: String) -> List<&2, D.Node>:
  match git:
    case True{}: View.diff.loc.off(lc, text, True{})
    case False{}: View.diff.nogit(lc, none)

def View.diff.kept(known: Bool, +lc: I.Loc, +kept: J.Json) -> List<&2, D.Node>:
  match known:
    case True{}: View.diff.kept.k(String.is_empty(J.Json.get_str(kept, "nogit", "")), lc, J.Json.get_str(kept, "diff", ""), J.Json.get_str(kept, "nogit", ""))
    case False{}: View.diff.wait(lc)

def View.diff.at.w(wait: Bool, +lc: I.Loc, +text: String, +none: String, +kept: String) -> List<&2, D.Node>:
  match wait:
    case True{}: View.diff.kept(Bool.not(String.is_empty(kept)), lc, Maybe.default(&2, J.Json, J.Json.parse(kept), J.Null{}))
    case False{}: View.diff.at.k(String.is_empty(none), lc, text, none)

# (none: the folder the hub found no repository in, "diff.none:t"; wait:
# the asked Diff not answered yet, "diff.wait:t"; kept: its diff from before)
def View.diff.at(+lc: I.Loc, +text: String, +none: String, +wait: String, +kept: String) -> List<&2, D.Node>:
  View.diff.at.w(Bool.not(String.is_empty(wait)), lc, text, none, kept)

# The diff stays visible while the person decides what to discard.
def View.revert.loc(on: Bool, +lc: I.Loc) -> List<&2, D.Node>:
  match on:
    case False{}: Nil{}
    case True{}:
      [D.El{"div", "revert-confirm", [D.At{"class", "diff-head"}, D.At{"role", "alert"}, D.At{"data-esc", "revert-no"}], [
        D.Dom.span("summary", MSG.T.diff.revert.confirm(lc)),
        D.El{"div", "", [D.At{"class", "diff-btns"}], [
          D.Dom.button("danger", "revert", "confirm", MSG.T.diff.revert.tip(lc), MSG.T.palette.revert(lc)),
          D.Dom.button("", "revert-no", "", MSG.T.common.cancel(lc), MSG.T.common.cancel(lc))]}]}]

def View.pending.image.loc(+lc: I.Loc, image: Bool, +p: String) -> List<&2, D.Node>:
  match image:
    case True{}:
      [D.El{"img", "", [D.At{"class", "attachment-preview"}, D.At{"src", IM.Img.url(p)}, D.At{"alt", AT.Att.label(p)}, D.At{"width", "96"}, D.At{"height", "72"}], Nil{}}, D.Dom.span("attachment-error", MSG.T.composer.preview.unavailable(lc))]
    case False{}:
      Nil{}

def View.pending.loc(+lc: I.Loc, ps: List<&2, String>) -> List<&2, D.Node>:
  match ps:
    case Nil{}:
      Nil{}
    case +p <> t:
      D.El{"button", "", [D.At{"class", Bool.pick(String, AT.Att.image(p), "chip image", "chip")}, D.At{"title", MSG.T.composer.detach(lc, p)}, D.At{"data-value", p}, D.On{"click", "detach"}],
        D.Dom.cat(View.pending.image.loc(lc, AT.Att.image(p), p), [D.El{"span", "", [D.At{"class", "attachment-label"}], [D.Dom.span("attachment-name", AT.Att.label(p)), D.El{"span", "", [D.At{"class", "attachment-remove"}, D.At{"aria-hidden", "true"}], [D.Txt{"×"}]}]}])} <> View.pending.loc(lc, t)

# the draw board's button, beside Attach: host.js opens the board (draw.js)
# with these words, and its picture is attached as a picked file is
def View.draw.loc(+lc: I.Loc) -> D.Node:
  D.El{"button", "", [D.At{"class", "attach draw"}, D.At{"type", "button"}, D.At{"title", MSG.T.composer.draw.tip(lc)}, D.At{"data-draw", "attach"},
    D.At{"data-draw-title", MSG.T.draw.title(lc)}, D.At{"data-draw-hint", MSG.T.draw.hint(lc)}, D.At{"data-draw-save", MSG.T.draw.save(lc)},
    D.At{"data-draw-cancel", MSG.T.draw.cancel(lc)}, D.At{"data-draw-length", MSG.T.draw.length(lc)}], [WI.WIcon.svg("square-pen", 16)]}

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

# one quiet word for where dictation is: starting, loading the local model,
# listening, writing (words are settling into the draft), finishing. The
# words themselves are only ever in the draft, never repeated here.
def View.vstate(+lc: I.Loc, +state: String, +u: U.Ui) -> String:
  U.Voice.status(lc, u)

def View.vstrip.loc(+lc: I.Loc, on: Bool, +u: U.Ui) -> List<&2, D.Node>:
  match on:
    case True{}:
      +state = U.Ui.scratch(u, "@voice")
      [D.El{"div", "", [D.At{"class", "voice-strip"}], [D.Dom.span("rec", ""),
        D.Dom.span("voice-head", View.vstate(lc, state, u)),
        D.El{"span", "", [D.At{"class", "meter"}], [D.El{"span", "", [D.At{"style", "width:" ++ Nat.show(Maybe.default(&2, Nat, Nat.read(U.Ui.scratch(u, "@voice.lv")), 0n)) ++ "%"}], Nil{}}]},
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

# the last segment's fixes, each a chip whose click undoes it
def View.vfix.go(+lc: I.Loc, fs: List<&2, VDV.VFx>, +n: Nat) -> List<&2, D.Node>:
  match fs:
    case Nil{}:
      Nil{}
    case VDV.VFx{+h, +o} <> t:
      D.Dom.button("chip", "voice-unterm", "#" ++ Nat.show(n), MSG.T.voice.undo.tip(lc, h),
        Bool.pick(String, String.is_empty(o), MSG.T.voice.dropped(lc, h), MSG.T.voice.fixed(lc, o, h)) ++ " ↶") <> View.vfix.go(lc, t, 1n+n)

def View.vfix.loc(+lc: I.Loc, on: Bool, +u: U.Ui) -> List<&2, D.Node>:
  match on:
    case True{}:
      [D.El{"div", "", [D.At{"class", "voice-fixes"}], View.vfix.go(lc, U.Voice.fixes(u), 0n)}]
    case False{}:
      Nil{}

def View.voice.loc(+lc: I.Loc, +u: U.Ui, +sel: String) -> List<&2, D.Node>:
  +here = Bool.or(String.eq(U.Ui.scratch(u, "@voice.th"), sel), String.is_empty(U.Ui.scratch(u, "@voice.th")))
  D.Dom.cat(View.vnote.loc(lc, Bool.and(here, Bool.or(Bool.not(String.is_empty(U.Ui.scratch(u, "@voice.err"))), Bool.and(U.Voice.on(u), Bool.not(String.is_empty(U.Ui.scratch(u, "@voice.hint")))))), u),
    D.Dom.cat(View.vstrip.loc(lc, Bool.and(here, U.Voice.on(u)), u),
      D.Dom.cat(View.vfix.loc(lc, Bool.and(String.eq(U.Ui.scratch(u, "@voice.uth"), sel), Bool.not(List.is_empty(&2, VDV.VFx, U.Voice.fixes(u)))), u),
        View.vask.loc(lc, Bool.and(String.eq(U.Ui.scratch(u, "@voice.uth"), sel), Bool.not(List.is_empty(&2, VC.Unsure, U.Voice.unsure(u)))), u))))

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
# (Save, an intent's: Enter is a new line there, U.Draft.enter)
def View.send.loc(+lc: I.Loc, +act: String, +send: String) -> D.Node:
  Bool.pick(D.Node, String.eq(act, "interrupt"), D.Dom.button("primary stop", act, "", MSG.T.composer.stop.tip(lc), send),
    D.Dom.button("primary", act, "", Bool.pick(String, String.eq(send, MSG.T.common.save(lc)), MSG.T.intent.save.tip(lc),
      MSG.T.composer.enter.tip(lc)), send))

# The composer as typed (app.bend's presentation_overlay, in the browser):
# the hub drew it for the draft it last had, which lags what is typed while
# a turn streams (the hub is busy). Until it catches up, the send button
# (Stop, Steer, Ask...) and the slash commands follow the draft at once, as
# the hub would draw them; the root's data-run, data-atts, data-claude and
# data-lang say what the draft alone cannot. The hub's own render wins
# once it is for this draft (its textarea holds the same text).
type Local is Data:
  Local{draft: String, same: Bool, run: Bool, atts: Nat, claude: Bool, lc: I.Loc}

def View.local.attr(as: List<&2, D.Attr>, +k: String) -> String:
  match as:
    case Nil{}:
      ""
    case D.At{+n, +v} <> t:
      Bool.pick(String, String.eq(n, k), v, View.local.attr(t, k))
    case _ <> t:
      View.local.attr(t, k)

def View.local.class(+as: List<&2, D.Attr>, +c: String) -> Bool:
  String.eq(View.local.attr(as, "class"), c)

# the draft the hub drew the composer for: its textarea's value
def View.local.drawn(ks: List<&2, D.Node>) -> String:
  match ks:
    case Nil{}:
      ""
    case D.El{_, _, +as, _} <> t:
      Bool.pick(String, String.eq(View.local.attr(as, "id"), "composer"), View.local.attr(as, "value"), View.local.drawn(t))
    case _ <> t:
      View.local.drawn(t)

def View.local.skilled(ks: List<&2, D.Node>) -> Bool:
  match ks:
    case Nil{}:
      False{}
    case D.El{_, _, +as, _} <> t:
      Bool.or(View.local.class(as, "skills"), View.local.skilled(t))
    case _ <> t:
      View.local.skilled(t)

def Local.draft(l: Local) -> String:
  match l:
    case Local{d, _, _, _, _, _}: d

def Local.same(l: Local) -> Bool:
  match l:
    case Local{_, s, _, _, _, _}: s

def Local.lc(l: Local) -> I.Loc:
  match l:
    case Local{_, _, _, _, _, c}: c

def Local.slash(+l: Local) -> List<&2, D.Node>:
  match l:
    case Local{+d, _, _, _, +c, _}: Bool.pick(List<&2, D.Node>, U.Slash.typing.d(d), View.skills(U.Slash.local(c, d)), Nil{})

def Local.send(+l: Local) -> D.Node:
  match l:
    case Local{+d, _, +r, +n, _, +lc}:
      +stop = Bool.and(r, Bool.and(RF.Refs.blank(d), Nat.is_eq(n, 0n)))
      View.send.loc(lc, U.Follow.act(stop), U.Follow.button(lc, M.State.empty(), "", r, stop, d))

# the textarea's keys, as View.composer.loc sets them, for rows shown or not
# and draft d (U.Draft.enter)
# View.local.key matches a copy of the attribute: a string pattern splits
# bit by bit and a row that mentions its scrutinee gets it rebuilt at every
# split (lane B); the rows are View.local.keys' former ones.
def View.local.key(key: D.Attr, h: D.Attr, +rows: Bool, +d: String) -> D.Attr:
  match key:
    case D.At{"data-enter", _}:
      D.At{"data-enter", U.Draft.enter(rows, d)}
    case D.At{"data-tab", _}:
      D.At{"data-tab", Bool.pick(String, rows, "skill", "")}
    case D.At{"data-arrows", _}:
      D.At{"data-arrows", Bool.pick(String, rows, "skill-move", "")}
    case D.At{"data-esc", _}:
      D.At{"data-esc", Bool.pick(String, rows, "skill-close", "")}
    case _:
      h

def View.local.keys(as: List<&2, D.Attr>, +rows: Bool, +d: String) -> List<&2, D.Attr>:
  match as:
    case Nil{}:
      Nil{}
    case +h <> t:
      View.local.key(h, h, rows, d) <> View.local.keys(t, rows, d)

# the foot's send button (span.foot-end's last child) drawn for the draft
def View.local.end(ks: List<&2, D.Node>, +l: Local) -> List<&2, D.Node>:
  match ks:
    case Nil{}:
      Nil{}
    case D.El{+tag, +key, +as, +kids} <> Nil{}:
      Bool.pick(List<&2, D.Node>, String.eq(tag, "button"), [Local.send(l)], [D.El{tag, key, as, kids}])
    case +k <> t:
      k <> View.local.end(t, l)

def View.local.foot(ks: List<&2, D.Node>, +l: Local) -> List<&2, D.Node>:
  match ks:
    case Nil{}:
      Nil{}
    case D.El{+tag, +key, +as, +kids} <> t:
      Bool.pick(D.Node, View.local.class(as, "foot-end"), D.El{tag, key, as, View.local.end(kids, l)}, D.El{tag, key, as, kids}) <> View.local.foot(t, l)
    case +k <> t:
      k <> View.local.foot(t, l)

# the composer's children: the hub's skills kept while its render is for
# this draft, else the draft's slash commands (none once it is not one);
# its $skills and references stay as the hub drew them
def View.local.kids(ks: List<&2, D.Node>, +l: Local, +had: Bool, +drawn: String) -> List<&2, D.Node>:
  match ks:
    case Nil{}:
      Nil{}
    case D.El{+tag, +key, +as, +kids} <> t:
      +skills = View.local.class(as, "skills")
      +area = String.eq(View.local.attr(as, "id"), "composer")
      +mine = Bool.or(Local.same(l), Bool.not(Bool.or(U.Slash.typing.d(Local.draft(l)), U.Slash.typing.d(drawn))))
      +rows = Bool.pick(Bool, mine, had, Bool.not(List.is_empty(&2, D.Node, Local.slash(l))))
      List.concat(&2, D.Node, [
        Bool.pick(List<&2, D.Node>, Bool.and(area, Bool.and(Bool.not(mine), Bool.not(had))), Local.slash(l), Nil{}),
        Bool.pick(List<&2, D.Node>, skills, Bool.pick(List<&2, D.Node>, mine, [D.El{tag, key, as, kids}], Local.slash(l)),
          [D.El{tag, key, Bool.pick(List<&2, D.Attr>, area, View.local.keys(as, rows, Local.draft(l)), as),
            Bool.pick(List<&2, D.Node>, View.local.class(as, "composer-foot"), View.local.foot(kids, l), kids)}]),
        View.local.kids(t, l, had, drawn)])
    case +k <> t:
      k <> View.local.kids(t, l, had, drawn)

def View.local.go(nodes: List<&2, D.Node>, +draft: String, acc: List<&2, D.Node>) -> List<&2, D.Node>:
  match nodes:
    case Nil{}:
      acc
    case D.El{+tag, +key, +as, +kids} <> t:
      +mine = Bool.and(View.local.class(as, "composer"), Bool.not(String.is_empty(View.local.attr(as, "data-lang"))))
      View.local.go(t, draft, Bool.pick(D.Node, mine,
        D.El{tag, key, as, View.local.kids(kids, Local{draft, String.eq(View.local.drawn(kids), draft), String.eq(View.local.attr(as, "data-run"), "1"),
          Maybe.default(&2, Nat, Nat.read(View.local.attr(as, "data-atts")), 0n), String.eq(View.local.attr(as, "data-claude"), "1"), I.I18n.of_tag(View.local.attr(as, "data-lang"))},
          View.local.skilled(kids), View.local.drawn(kids))},
        D.El{tag, key, as, List.reverse(&2, D.Node, View.local.go(kids, draft, Nil{}))}) <> acc)
    case +k <> t:
      View.local.go(t, draft, k <> acc)

def View.local(node: D.Node, +draft: String) -> List<&2, D.Node>:
  View.local.go([node], draft, Nil{})

# The token chip (on unless turned off, usage.bend's Ts): the thread's token use by the
# send button; a click opens the panel with all its provider told, and a
# click outside or on the chip again closes it (the flag "tokstats")
def View.tok.chip(on: Bool, +text: String, +open: Bool) -> List<&2, D.Node>:
  match on:
    case True{}:
      [D.El{"button", "", [D.At{"class", Bool.pick(String, open, "tok-chip on", "tok-chip")}, D.At{"title", UZ.Ts.tip()}, D.At{"data-value", "tokstats"}, D.On{"click", "flag"}],
        WI.WIcon.svg("gauge", 14) <> Bool.pick(List<&2, D.Node>, String.is_empty(text), Nil{}, [D.Dom.span("tok-n", text)])}]
    case False{}:
      Nil{}

def View.tok.rows(rs: List<&2, UZ.TsRow>) -> List<&2, D.Node>:
  match rs:
    case Nil{}:
      Nil{}
    case +r <> t:
      D.El{"div", "", [D.At{"class", "tok-row"}], [D.Dom.span("tok-l", UZ.TsRow.label(r)), D.Dom.span("tok-v", UZ.TsRow.value(r))]} <> View.tok.rows(t)

def View.tok.secs(ss: List<&2, UZ.TsSec>) -> List<&2, D.Node>:
  match ss:
    case Nil{}:
      Nil{}
    case +x <> t:
      D.El{"div", "", [D.At{"class", "tok-sec"}], D.Dom.cat(Bool.pick(List<&2, D.Node>, String.is_empty(UZ.TsSec.title(x)), Nil{}, [D.Dom.span("tok-h", UZ.TsSec.title(x))]), View.tok.rows(UZ.TsSec.rows(x)))} <> View.tok.secs(t)

def View.tok.pop(open: Bool, +st: M.State, +th: String, +prov: String) -> List<&2, D.Node>:
  match open:
    case True{}:
      [D.El{"div", "", [D.At{"class", "shade"}, D.At{"data-value", "tokstats"}, D.On{"click", "flag"}], Nil{}},
       D.El{"div", "", [D.At{"class", "tok-pop"}], D.Dom.span("tok-title", UZ.Ts.label()) <> View.tok.secs(UZ.Ts.sections(st, th, prov))}]
    case False{}:
      Nil{}

def View.composer.p(+lc: I.Loc, +draft: String, +send: String, +act: String, +prov: String, menu: List<&2, D.Node>, chip: List<&2, D.Node>, +rs: List<&2, U.SkillRow>,
    +atts: List<&2, String>, +up: String, vo: List<&2, D.Node>, +mic: String, tk: List<&2, D.Node>, more: List<&2, D.Node>, tp: List<&2, D.Node>, run: Bool) -> D.Node:
  D.El{"div", "", [D.At{"class", "composer"}, D.At{"data-run", Bool.pick(String, run, "1", "")}, D.At{"data-atts", Nat.show(List.length(&2, String, atts))},
    D.At{"data-claude", Bool.pick(String, String.eq(prov, "claude"), "1", "")}, D.At{"data-lang", I.I18n.tag(lc)}], D.Dom.cat(
    menu, D.Dom.cat(tp, D.Dom.cat(View.skills(rs), D.Dom.cat(vo, D.Dom.cat(View.attaching.loc(lc, atts, up), [
    D.El{"textarea", "composer", [D.At{"id", "composer"}, D.At{"rows", "3"}, D.At{"placeholder", MSG.T.composer.placeholder(lc)}, D.At{"data-placeholder-full", MSG.T.composer.placeholder(lc)}, D.At{"data-placeholder-short", MSG.T.composer.ask(lc) ++ "…"}, D.At{"title", MSG.T.composer.placeholder(lc)}, D.At{"value", draft}, D.On{"input", "draft"},
      D.At{"data-enter", U.Draft.enter(Bool.not(List.is_empty(&2, U.SkillRow, rs)), draft)}, D.At{"data-enter-alt", "send-alt"}, D.At{"data-tab", Bool.pick(String, List.is_empty(&2, U.SkillRow, rs), "", "skill")}, D.At{"data-arrows", Bool.pick(String, List.is_empty(&2, U.SkillRow, rs), "", "skill-move")}, D.At{"data-esc", Bool.pick(String, List.is_empty(&2, U.SkillRow, rs), "", "skill-close")}, D.At{"data-paste", "attach"}], Nil{}},
    CBAR.ComposerBar.node(List.append(&2, D.Node, chip, [
      CBAR.ComposerBar.more(List.append(&2, D.Node, more, View.draw.loc(lc) <> tk)),
      D.El{"label", "", [D.At{"class", "attach"}, D.At{"title", MSG.T.composer.attach.tip(lc)}], [WI.WIcon.svg("paperclip", 16),
        D.El{"input", "attach-file", [D.At{"type", "file"}, D.At{"multiple", ""}, D.At{"aria-label", MSG.T.composer.attach.tip(lc)}, D.At{"data-attach", "attach"}], Nil{}}]},
      D.El{"span", "", [D.At{"class", "foot-end"}], [View.mic.loc(lc, mic),
        View.send.loc(lc, act, send)]}]))])))))}

def View.composer.loc(+lc: I.Loc, +draft: String, +send: String, +act: String, +label: String, +ctx: String, +picking: Bool, +ms: List<&2, String>, +model: String, +info: String, +prov: String, +es: List<&2, String>, +effort: String, +rs: List<&2, U.SkillRow>,
    +atts: List<&2, String>, +up: String, vo: List<&2, D.Node>, +mic: String, +q: String, srch: Bool, tk: List<&2, D.Node>, tp: List<&2, D.Node>, run: Bool) -> D.Node:
  View.composer.p(lc, draft, send, act, prov, View.menu.loc(lc, picking, ms, model, info, prov, es, effort, q, srch), View.mp.chip(lc, VLabel.effort.label(lc, label, effort), ctx),
    rs, atts, up, vo, mic, tk, Nil{}, tp, run)

# the thread's header, or the one given (a bot's)

def View.composer(+draft: String, +send: String, +act: String, +label: String, +ctx: String, +picking: Bool, +ms: List<&2, String>, +model: String, +info: String, +prov: String, +es: List<&2, String>, +effort: String, +rs: List<&2, U.SkillRow>,
    +atts: List<&2, String>, +up: String, vo: List<&2, D.Node>, +mic: String) -> D.Node:
  View.composer.loc(I.En{}, draft, send, act, label, ctx, picking, ms, model, info, prov, es, effort, rs, atts, up, vo, mic, "", False{}, Nil{}, Nil{}, False{})

def View.send(+act: String, +send: String) -> D.Node:
  View.send.loc(I.En{}, act, send)

# the thread's header, or the one given (a bot's)
def View.top.header(rail: Bool, +lc: I.Loc, +t: M.Thread, +st: M.State, +chip: String, +mini: List<&2, TO.HeaderTool>, +panel: Bool, +snoozed: Bool, +picking: Bool, +term: Bool, +diff: Bool, +del: Bool, +ren: String, +rtext: String) -> List<&2, D.Node>:
  match rail:
    case True{}: [View.header(lc, t, st, chip, mini, panel, snoozed, picking, term, diff, del, ren, rtext)]
    case False{}: [View.header.legacy(lc, t, st, chip, mini, panel, snoozed, picking, term, diff, del, ren, rtext)]

def View.top(rail: Bool, +lc: I.Loc, hd: List<&2, D.Node>, t: M.Thread, +st: M.State, +chip: String, mini: List<&2, TO.HeaderTool>, +panel: Bool, snoozed: Bool, picking: Bool, term: Bool, +diff: Bool, +del: Bool, +ren: String, +rtext: String) -> List<&2, D.Node>:

  match hd:
    case Nil{}:
      View.top.header(rail, lc, t, st, chip, mini, panel, snoozed, picking, term, diff, del, ren, rtext)
    case h <> r:
      h <> r

# The two parts of a thread's page that follow its draft: the composer and
# the asks (an answer with the draft). The page draws them with these, and
# host.js redraws only them for a keystroke (app.bend's composer and asks):
# rebuilding a long thread's page for every key took 280 ms a key.
def View.accounts.wait(+state: String) -> String:
  match state:
    case "unchecked": "Checking saved sign-in…"
    case "connecting": "Finish sign-in in Settings → Agents"
    case "disconnected": "Connect in Settings → Agents"
    case _: "Refresh status in Settings → Agents"

def View.accounts.option(+p: J.Json) -> D.Node:
  +label = J.Json.get_str(p, "provider", "") ++ " · " ++ J.Json.get_str(p, "label", "Account")
  +state = J.Json.get_str(p, "state", "unchecked")
  Bool.pick(D.Node, String.eq(state, "connected"),
    D.Dom.button("", "acct-select", J.Json.get_str(p, "id", ""), U.Acv.note(p), label ++ " · connected"),
    D.El{"button", "", [D.At{"disabled", "disabled"}, D.At{"aria-disabled", "true"}, D.At{"title", U.Acv.note(p)}], [D.Txt{label ++ " · " ++ View.accounts.wait(state)}]})

def View.accounts.options(xs: List<&2, J.Json>, +provider: String) -> List<&2, D.Node>:
  match xs:
    case Nil{}: Nil{}
    case +p <> t:
      View.accounts.option(p) <> View.accounts.options(t, provider)

def View.accounts.pop(open: Bool, +st: M.State, +provider: String) -> List<&2, D.Node>:
  match open:
    case False{}: Nil{}
    case True{}:
      [D.El{"div", "account-picker", [D.At{"class", "account-picker"}],
        D.Txt{"Choose an account between turns. Usage limits automatically try another connected account of the same agent, then your fallback hierarchy. Your choice returns after its reported reset, between turns."} <>
        D.Dom.button("", "acct-select", "", "Use existing CLI authentication", "Default " ++ provider ++ " account") <>
        View.accounts.options(ACCT.Accounts.profiles(st), provider)}]

def View.accounts.chip(+st: M.State, +sel: String, +provider: String) -> List<&2, D.Node>:
  Bool.pick(List<&2, D.Node>, ACCT.Accounts.supported(provider),
    D.El{"button", "", [D.At{"class", "account-chip"}, D.At{"title", "Account: " ++ ACCT.Accounts.label(st, sel, provider) ++ " · Manage logins in Settings → Providers → Accounts"}, D.At{"aria-label", "Account: " ++ ACCT.Accounts.label(st, sel, provider)}, D.At{"data-value", ""}, D.On{"click", "acct-open"}],
      [WI.WIcon.svg("user", 16), D.Dom.span("acct-v", ACCT.Accounts.short(ACCT.Accounts.label(st, sel, provider)))]} <> Nil{}, Nil{})

def View.accounts(+st: M.State, +sel: String, +provider: String, +open: Bool) -> List<&2, D.Node>:
  List.append(&2, D.Node, View.accounts.chip(st, sel, provider), View.accounts.pop(open, st, provider))

# a draft over a message's bound says so over the composer (draftbound.bend)
def View.draft.limit.k(+text: String) -> List<&2, D.Node>:
  match text:
    case SNil{}: Nil{}
    case SCon{+h, +t}: [D.El{"div", "draft-limit", [D.At{"class", "draft-limit"}, D.At{"role", "status"}], [D.Txt{SCon{h, t}}]}]

def View.composer.t(+lc: I.Loc, +st: M.State, +sel: String, +drafts: Map<&2, String>, +op: Map<&2, Bool>, +nf: Map<&2, String>, +turn: M.TurnState, +pr: String, +mo: String,
    +atts: List<&2, String>, +up: String, vo: List<&2, D.Node>, +mic: String) -> D.Node:
  +stop = U.Follow.stop(View.running(turn), U.Str.get(drafts, sel), atts)
  +tok = UZ.Ts.on(st)
  +quota = ALQ.Allowance.selected(st, sel, pr)
  +clock = M.Switch.num(U.Str.get(nf, "clock"), 0n)
  View.composer.p(lc, U.Str.get(drafts, sel), U.Follow.button(lc, st, sel, View.running(turn), stop, U.Str.get(drafts, sel)), U.Follow.act(stop), pr,
    View.mp.loc(lc, U.Flag.get(op, "models"), ACCT.Accounts.nf(nf, st, sel), U.Mpk.cur.of(st, sel, drafts), U.Mpk.now(st, sel), U.Str.get(drafts, "@f.mq"), MPK.ModelPick.kept(U.Str.get(drafts, "@mp"), sel)),
    [D.El{"div", "composer-model", [D.At{"class", "composer-model"}], List.concat(&2, D.Node, [
      View.mp.chip.k(lc, View.mp.am(lc, pr, mo), View.mp.tail(lc, U.Effort.of(st, sel), Bool.not(List.is_empty(&2, String, U.Effort.chips(U.Str.get(nf, "models"), pr, mo)))), Bool.pick(String, tok, "", U.Usage.tail(st, sel)))])}],
    U.Skills.shown(st, sel, drafts, nf), atts, up, D.Dom.cat(View.draft.limit.k(DB.Bound.say(lc, U.Str.get(drafts, "@draft.bound"))), vo), mic,
    View.tok.chip(tok, UZ.Ts.chip(st, sel), U.Flag.get(op, "tokstats")), List.append(&2, D.Node, View.accounts.chip(st, sel, pr), AW.AllowanceWeb.chip(quota, pr, mo, clock, U.Flag.get(op, "allowance"))), List.concat(&2, D.Node, [View.accounts.pop(U.Flag.get(op, "accounts"), st, pr), View.tok.pop(Bool.and(tok, U.Flag.get(op, "tokstats")), st, sel, pr), AW.AllowanceWeb.pop(U.Flag.get(op, "allowance"), quota, pr, mo, clock)]), View.running(turn))

# A buy thread's asks: a "Backplane needs" card each, above the composer
# (client.bend's Fab.order). An order question's choices are buttons (the
# earlier order's answer marked "· last time", sent without it:
# OR.Order.ask.value) and a box takes a typed answer, Enter or Continue
# sending it; a typed question (where it ships) leads with its box. The
# files' approval keeps its Send / Not now.
def View.needs.chips(ls: List<&2, String>, +id: String, +first: Bool) -> List<&2, D.Node>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      D.Dom.button(Bool.pick(String, first, "needs-chip primary", "needs-chip"), "answer", id ++ "|" ++ OR.Order.ask.value(l), l, l) <> View.needs.chips(t, id, False{})

def View.needs.box(+id: String, +typed: String, +lead: Bool) -> D.Node:
  D.El{"div", "", [D.At{"class", "needs-box"}], [
    D.El{"input", "needs-in", [D.At{"id", "needs:" ++ id}, D.At{"type", "text"}, D.At{"autocomplete", "off"}, D.At{"aria-label", "Your answer"},
      D.At{"placeholder", Bool.pick(String, lead, "Type it here", "Or type an answer")}, D.At{"value", typed},
      D.At{"data-bfield", U.Fab.typed.key(id)}, D.On{"input", "bfield"}, D.At{"data-pre", id ++ "|"}, D.At{"data-enter", "answer"}], Nil{}},
    D.El{"button", "needs-go", List.append(&2, D.Attr, [D.At{"type", "button"}, D.At{"class", Bool.pick(String, lead, "primary", "")}, D.At{"data-value", id ++ "|" ++ String.trim(typed)}, D.On{"click", "answer"}],
      Bool.pick(List<&2, D.Attr>, String.is_empty(String.trim(typed)), [D.At{"disabled", "disabled"}], Nil{})), [D.Txt{"Continue"}]}]}

def View.needs.ask(+lc: I.Loc, +typed: Bool, +id: String, +o: String, +field: String) -> List<&2, D.Node>:
  match typed:
    case True{}:
      [View.needs.box(id, field, True{}), D.El{"div", "", [D.At{"class", "needs-chips"}], View.needs.chips(OR.Order.ask.but(String.split(o, '\n'), ""), id, False{})}]
    case False{}:
      [D.El{"div", "", [D.At{"class", "needs-chips"}], View.needs.chips(OR.Order.ask.but(String.split(o, '\n'), ""), id, True{})}, View.needs.box(id, field, False{})]

def View.needs.k(+lc: I.Loc, question: Bool, a: M.Ask, +dr: Map<&2, String>) -> D.Node:
  match question a:
    case True{} M.Ask{+id, _, _, +tool, +d, +o, _, _}:
      D.El{"div", id, [D.At{"class", "ask needs"}, D.At{"role", "group"}, D.At{"aria-label", "Backplane needs"}], List.append(&2, D.Node, [
        D.El{"div", "", [D.At{"class", "needs-head"}], [D.Dom.span("needs-k", "Backplane needs"),
          D.Dom.button("needs-skip", "answer", id ++ "|", "Skip this question: the agent is told you skipped it", "Skip")]},
        D.El{"div", "", [D.At{"class", "needs-q"}], [D.Txt{d}]}],
        View.needs.ask(lc, OR.Order.ask.typed(tool), id, o, U.Str.get(dr, "@f." ++ U.Fab.typed.key(id))))}
    case False{} M.Ask{+id, _, +k, +tool, +d, +o, _, _}:
      D.El{"div", id, [D.At{"class", "ask needs " ++ k}, D.At{"role", "group"}, D.At{"aria-label", "Backplane needs"}], [
        D.El{"div", "", [D.At{"class", "needs-head"}], [D.Dom.span("needs-k", "Backplane needs"), D.Dom.span("needs-what", U.Ask.head.loc(lc, k, tool, d))]},
        D.El{"div", "", [D.At{"class", "ask-detail"}], [D.Txt{d}]},
        D.El{"div", "", [D.At{"class", "ask-btns"}], View.ask.btns.loc(lc, U.Ask.bkind(k, tool), id, o, "")}]}

# (any other ask in a buy thread, a tool's approval, a plan, a paused
# browser task, keeps its own card: View.ask)
def View.needs.or(order: Bool, +lc: I.Loc, a: M.Ask, +draft: String, +nf: Map<&2, String>, +dr: Map<&2, String>) -> D.Node:
  match order a:
    case True{} M.Ask{i, th, k, +tool, d, o, op, an}:
      View.needs.k(lc, OR.Order.ask.is(tool), M.Ask{i, th, k, tool, d, o, op, an}, dr)
    case False{} _:
      View.ask(lc, a, draft, nf)

def View.needs(+lc: I.Loc, a: M.Ask, +draft: String, +nf: Map<&2, String>, +dr: Map<&2, String>) -> D.Node:
  match a:
    case M.Ask{i, th, k, +tool, d, o, op, an}:
      View.needs.or(Bool.or(OR.Order.ask.is(tool), String.eq(tool, "order_quote")), lc, M.Ask{i, th, k, tool, d, o, op, an}, draft, nf, dr)

def View.needs.all(+lc: I.Loc, as: List<&2, M.Ask>, +draft: String, +nf: Map<&2, String>, +dr: Map<&2, String>) -> List<&2, D.Node>:
  match as:
    case Nil{}:
      Nil{}
    case a <> t:
      View.needs(lc, a, draft, nf, dr) <> View.needs.all(lc, t, draft, nf, dr)

def View.asks.of(+lc: I.Loc, buying: Bool, as: List<&2, M.Ask>, +draft: String, +nf: Map<&2, String>, +dr: Map<&2, String>) -> List<&2, D.Node>:
  match buying:
    case True{}:
      View.needs.all(lc, as, draft, nf, dr)
    case False{}:
      View.asks(lc, as, draft, nf)

def View.asks.box(+lc: I.Loc, +st: M.State, +id: String, +draft: String, +nf: Map<&2, String>, +dr: Map<&2, String>) -> D.Node:
  D.El{"div", "asks", [D.At{"class", "asks"}, D.At{"id", "asks"}], List.concat(&2, D.Node, [View.gone(lc, st, id, dr), View.sbox(lc, U.Sbox.box.d(st, dr, id)), View.asks.of(lc, U.Fab.buying(st, id), U.Asks.open(st, id), draft, nf, dr)])}

# the same two from the Ui alone (app.bend's composer and asks)
def View.composer.k(+lc: I.Loc, u: U.Ui, t: M.Thread, +atts: List<&2, String>, +up: String, vo: List<&2, D.Node>, +mic: String) -> D.Node:
  match u t:
    case U.Ui{+st, +sel, +drafts, _, +op, _, _, _, _, _, _, _, _, _, +nf, _, _} M.Thread{_, _, _, _, _, +turn, _, _, _, _, _, _, _, _, _, +pr, +mo, _, _, _, _}:
      View.composer.t(lc, st, sel, drafts, op, nf, turn, pr, mo, atts, up, vo, mic)

# over the composer while it replies to a message: "Replying to: <its
# first line>", the line going to the message, × forgetting it
def View.rpl.chip.k(+lc: I.Loc, +to: String, +q: String) -> List<&2, D.Node>:
  match to:
    case SNil{}: Nil{}
    case SCon{+h, +t}:
      [D.El{"div", "reply-chip", [D.At{"class", "reply-chip"}], [D.Dom.span("label", MSG.T.reply.replying(lc)),
        D.El{"a", "", [D.At{"class", "rto"}, D.At{"href", "#"}, D.At{"data-value", SCon{h, t}}, D.On{"click", "hist-jump"}], [D.Txt{q}]},
        D.El{"button", "", [D.At{"class", "reply-drop"}, D.At{"title", MSG.T.reply.cancel(lc)}, D.At{"aria-label", MSG.T.reply.cancel(lc)}, D.On{"click", "reply-drop"}], [D.Txt{"×"}]}]}]

def View.rpl.chip(+lc: I.Loc, +u: U.Ui) -> List<&2, D.Node>:
  View.rpl.chip.k(lc, U.Rpl.of(u), U.Rpl.label(u))

def View.composer.l(+lc: I.Loc, +u: U.Ui) -> D.Node:
  View.composer.k(lc, u, U.Ui.thread(U.Ui.state(u), U.Ui.sel(u)), U.Ui.atts(u, U.Ui.sel(u)), U.Ui.uploading(u, U.Ui.sel(u)), D.Dom.cat(View.rpl.chip(lc, u), D.Dom.cat(View.btw(lc, u), View.voice.loc(lc, u, U.Ui.sel(u)))), View.mic.of(u, U.Ui.sel(u)))

def View.composer.u(+u: U.Ui) -> D.Node:
  View.composer.l(U.Lang.loc(u), u)

def View.asks.u(+lc: I.Loc, u: U.Ui) -> D.Node:
  match u:
    case U.Ui{+st, +sel, +drafts, _, _, _, _, _, _, _, _, _, _, _, +nf, _, _}:
      View.asks.box(lc, st, sel, U.Str.get(drafts, sel), nf, drafts)

# Buy it with its price (client.bend's Fab.*, core/order.bend)
def View.fab.buy.tip() -> String:
  "Order this project's parts in a thread that asks what's missing, exports the files and quotes them. The price is the bare parts at the smallest order; the quote adds assembly, components, shipping and duties. The estimate is from the last check: use Refresh estimate after changing the fab files."

def View.fab.buy(+label: String) -> List<&2, D.Node>:
  Bool.pick(List<&2, D.Node>, String.is_empty(label), Nil{}, [D.Dom.button("fab-buy", "buy", "", View.fab.buy.tip(), label)])

# The project's intent (client.bend's Intent.*), opened from its menu's
# Intent…: what the project is for, as Markdown, over the page, with Edit
# (the composer gets it) and Close; a click beside it or Esc closes it too
def View.intent.body(+lc: I.Loc, got: Bool, +text: String) -> List<&2, D.Node>:
  match got:
    case False{}:
      [D.Dom.span("intent-note", MSG.T.intent.loading(lc))]
    case True{}:
      Bool.pick(List<&2, D.Node>, String.is_empty(String.trim(text)), [D.Dom.span("intent-note", MSG.T.intent.none(lc))], D.Md.render.loc(lc, text))

def View.intent.k(+lc: I.Loc, on: Bool, +u: U.Ui) -> List<&2, D.Node>:
  match on:
    case True{}:
      [D.El{"div", "", [D.At{"class", "modal-back"}, D.On{"click", "intent-close"}], Nil{}},
       D.El{"section", "intent", [D.At{"class", "intent"}, D.At{"id", "intent"}, D.At{"tabindex", "-1"}, D.At{"role", "dialog"}, D.At{"aria-modal", "true"},
           D.At{"aria-labelledby", "intent-title"}, D.At{"data-esc", "intent-close"}], [
         D.El{"header", "", [D.At{"class", "intent-head"}], [
           D.El{"span", "intent-title", [D.At{"class", "intent-title"}, D.At{"id", "intent-title"}],
             [D.Txt{MSG.T.intent.title(lc) ++ " · " ++ U.Ui.project_title(U.Ui.state(u), U.Ui.thread_project(U.Ui.thread(U.Ui.state(u), U.Ui.sel(u))))}]},
           D.Dom.button("intent-edit", "fab-edit", U.Ui.sel(u), MSG.T.intent.edit.tip(lc), MSG.T.intent.edit(lc)),
           WI.WIcon.button("icon", "intent-close", U.Ui.sel(u), MSG.T.common.close(lc), "x", 16, "")]},
         D.El{"div", "", [D.At{"class", "intent-body body"}], View.intent.body(lc, U.Intent.got(u), U.Intent.text(u))}]}]
    case False{}:
      Nil{}

# "Intent saved" for a few seconds once the hub has written it (client.bend's
# Intent.saved); a click puts it away
def View.intent.saved(+lc: I.Loc, saved: Bool) -> List<&2, D.Node>:
  match saved:
    case True{}:
      [D.El{"div", "intent-saved", [D.At{"class", "toast ok"}, D.At{"role", "status"}, D.At{"data-value", ""}, D.On{"click", "intent-saved-off"}], [D.Txt{MSG.T.intent.saved(lc)}]}]
    case False{}:
      Nil{}

def View.intent(+lc: I.Loc, +u: U.Ui) -> List<&2, D.Node>:
  List.append(&2, D.Node, View.intent.k(lc, U.Intent.on(u), u), View.intent.saved(lc, U.Intent.saved(u)))

# Ready to order: what there is to buy and Buy it, above the message box
# (U.Fab.ready), once the agent's turn is over; it glows as it comes
def View.ready.r(+line: String, +buy: String, rail: Bool) -> List<&2, D.Node>:
  Bool.pick(List<&2, D.Node>, String.is_empty(line), Nil{},
    [D.El{"div", "ready", [D.At{"class", "fab-ready"}], D.Dom.cat([D.Dom.span("fab-ready-text", line)], List.append(&2, D.Node, List.append(&2, D.Node, View.fab.buy(buy), D.Dom.when(Bool.or(String.starts_with(line, "Estimate unavailable:"), Bool.or(String.contains(buy, "last estimate:"), String.contains(buy, "estimate incomplete"))), D.Dom.button("", "fab-refresh", "", "Check the current fab files and refresh the estimate", Bool.pick(String, String.starts_with(line, "Estimate unavailable:"), "Retry", "Refresh estimate")))),
      [D.Dom.button("fab-x", "fab-hide", "", Bool.pick(String, rail, "Put this away: Buy it stays in the tool rail", "Put this away: Buy it stays in the thread's header"), "×")]))}])

def View.ready.k(+line: String, +buy: String) -> List<&2, D.Node>:
  View.ready.r(line, buy, False{})

def View.ready(+u: U.Ui) -> List<&2, D.Node>:
  View.ready.r(U.Fab.ready(u), U.Fab.ready.btn(u), View.rail(u))

# A buy thread's order card, pinned under its top bar (client.bend's
# Fab.order, core/order.bend's Order.card): "Order · <project>", the steps
# with the one at work lit, what happens now, and once quoted the total,
# what it covers about customs and Buy now
def View.order.steps(ss: List<&2, String>, +i: Nat, +k: Nat) -> List<&2, D.Node>:
  match ss:
    case Nil{}:
      Nil{}
    case +x <> t:
      D.El{"li", "", [D.At{"class", "oc-step " ++ OR.Order.card.mark(i, k)}], [D.Txt{Bool.pick(String, Nat.is_lt(k, i), "✓ " ++ x, x)}]} <> View.order.steps(t, i, 1n+k)

def View.order.quote(+total: String, +customs: String, +pay: String, +label: String) -> List<&2, D.Node>:
  D.Dom.when(Bool.not(String.is_empty(total)), D.El{"div", "oc-quote", [D.At{"class", "oc-quote"}], List.append(&2, D.Node,
    [D.Dom.span("oc-total", total), D.Dom.span("oc-customs", customs)],
    D.Dom.when(Bool.not(String.is_empty(pay)), D.El{"a", "oc-buy", [D.At{"class", "oc-buy"}, D.At{"href", pay}, D.At{"target", "_blank"}, D.At{"rel", "noopener noreferrer"}, D.At{"title", "Pay on Stripe's checkout page"}], [D.Txt{label}]}))})

def View.order.card(+lc: I.Loc, +st: M.State, +id: String, +c: OR.OrderCard) -> D.Node:
  D.El{"section", "order-card", [D.At{"class", "order-card" ++ Bool.pick(String, Nat.is_eq(OR.Order.card.step(c), 2n), " waiting", "")}, D.At{"aria-label", "Order"}], List.append(&2, D.Node, [
    D.El{"div", "", [D.At{"class", "oc-head"}], [WI.WIcon.svg("shopping-cart", 13), D.Dom.span("oc-k", "Order"), D.Dom.span("oc-name", U.Ui.project_title(st, U.Ui.thread_project(U.Ui.thread(st, id))))]},
    D.El{"ol", "", [D.At{"class", "oc-steps"}], View.order.steps(OR.Order.steps(), OR.Order.card.step(c), 0n)},
    D.El{"div", "oc-line", [D.At{"class", "oc-line"}, D.At{"role", "status"}], [D.Txt{OR.Order.card.text(c)}]}],
    View.order.quote(OR.Order.card.total(c), OR.Order.card.customs(c), OR.Order.card.pay(c), OR.Order.card.label(c)))}

def View.order(+lc: I.Loc, buying: Bool, +st: M.State, +id: String, running: Bool) -> List<&2, D.Node>:
  match buying:
    case True{}:
      [View.order.card(lc, st, id, U.Fab.order(st, id, running))]
    case False{}:
      Nil{}

# Away from the timeline's end with something new below (client.bend's
# Hold.pill): "3 new ↓" stuck to the timeline's foot, a click back down
def View.hold.pill(+label: String) -> List<&2, D.Node>:
  Bool.pick(List<&2, D.Node>, String.is_empty(label), Nil{},
    [D.El{"button", "hold-pill", [D.At{"class", "hold-pill"}, D.At{"data-value", ""}, D.On{"click", "hold-bottom"}], [D.Txt{label}]}])

# The Uberbot's conversation stays a conversation. Its operational overview
# remains available in Status; ordinary thread subagent panels are unchanged.
def View.thread.subs(+lc: I.Loc, +st: M.State, +nf: Map<&2, String>, +id: String, +now: Nat) -> List<&2, D.Node>:
  Bool.pick(List<&2, D.Node>, US.Ub.chat.is(st, id), Nil{}, View.subs.at(lc, nf, id, SB.Subs.rows(st, nf, id, now), st, now))

# Under the top bar its Subagents cell is the dock's head: the dock is only
# its rows, shown while that cell has them open and drawn hidden while shut
# (so the page shows them at the click: View.subs.head)
def View.subs.dock.k(some: Bool, +open: Bool, +lc: I.Loc, +info: Map<&2, String>, +rs: List<&2, SB.SubRow>, +st: M.State, +th: String, +now: Nat) -> List<&2, D.Node>:
  match some:
    case True{}:
      [D.El{"section", "subs", [D.At{"aria-label", MSG.T.thread.subagents(lc)}, D.At{"class", "subs dock" ++ Bool.pick(String, open, "", " shut")}], [View.subs.body(open, View.subs.signals.at(lc, rs, info, st, th, now))]}]
    case False{}:
      Nil{}

def View.subs.dock.at(+lc: I.Loc, +st: M.State, +info: Map<&2, String>, +th: String, +rs: List<&2, SB.SubRow>, +now: Nat) -> List<&2, D.Node>:
  View.subs.dock.k(Bool.and(Bool.not(List.is_empty(&2, SB.SubRow, rs)), Bool.not(US.Ub.chat.is(st, th))), SB.Subs.panel.open(info, th, rs), lc, info, rs, st, th, now)

def View.subs.dock(+lc: I.Loc, +st: M.State, +info: Map<&2, String>, +th: String, +rs: List<&2, SB.SubRow>) -> List<&2, D.Node>:
  View.subs.dock.at(lc, st, info, th, rs, 0n)

def View.tab(+cur: String, +kind: String, +action: String, +value: String, +title: String, +label: String) -> D.Node:
  D.El{"button", "", [D.At{"class", Bool.pick(String, String.eq(cur, kind), "tab on", "tab")}, D.At{"type", "button"},
    D.At{"aria-current", Bool.pick(String, String.eq(cur, kind), "page", "false")}, D.At{"data-value", value}, D.At{"title", title}, D.On{"click", action}], [D.Txt{label}]}

def View.destinations(+lc: I.Loc, +cur: String) -> List<&2, D.Node>:
  [
    View.tab(cur, "board", "view-kind", "board", MSG.T.viewer.board.tip(lc), MSG.T.viewer.board(lc)),
    View.tab(cur, "schematic", "view-kind", "schematic", MSG.T.viewer.schematic.tip(lc), MSG.T.viewer.schematic(lc)),
    View.tab(cur, "3d", "view-kind", "3d", MSG.T.viewer.parts3d.tip(lc), "3D"),
    View.tab(cur, "split", "view-kind", "split", "Schematic, board and 3D side by side", "Split"),
    View.tab(cur, "mech", "mech", "", MSG.T.viewer.mech.tip(lc), MSG.T.viewer.mech(lc)),
    View.tab(cur, "files", "fm", "", MSG.T.viewer.files.tip(lc), MSG.T.viewer.files(lc)),
    View.tab(cur, "diff", "diff", "", MSG.T.viewer.diff.tip(lc), MSG.T.viewer.diff(lc)),
    View.tab(cur, "review", "rv-page", "bom", MSG.T.viewer.review.tip(lc), MSG.T.viewer.review(lc)),
    View.tab(cur, "browser", "view-kind", "browser", MSG.T.viewer.browser.tip(lc), MSG.T.bots.tab.browser(lc))]

def View.commands.label(lc: I.Loc) -> String:
  match lc:
    case I.En{}: "Commands"
    case I.Ja{}: "コマンド"

# Labels and actions stay in Bend; local details only holds disclosure/focus.
def View.workspace.icon(kind: String) -> String:
  match kind:
    case "board": "circuit-board"
    case "schematic": "cpu"
    case "3d": "box"
    case "split": "layout-panel-left"
    case "mech": "layers"
    case "files": "folder"
    case "diff": "diff"
    case "review": "circle-check"
    case "browser": "globe"
    case "commands": "search"
    case _: "terminal"

def View.workspace.label(+lc: I.Loc, kind: String) -> String:
  match kind:
    case "board": MSG.T.viewer.board(lc)
    case "schematic": MSG.T.viewer.schematic(lc)
    case "3d": "3D"
    case "split": "Split"
    case "mech": MSG.T.viewer.mech(lc)
    case "files": MSG.T.viewer.files(lc)
    case "diff": MSG.T.viewer.diff(lc)
    case "review": MSG.T.viewer.review(lc)
    case "browser": MSG.T.bots.tab.browser(lc)
    case "commands": View.commands.label(lc)
    case _: MSG.T.palette.terminal(lc)

# the key that presses a rail button (R05's keycaps): a bare letter or
# digit outside any field (workspace-keys.js presses the button showing
# it), the terminal's backtick its own; "" for none
def View.workspace.key(kind: String) -> String:
  match kind:
    case "board": "b"
    case "schematic": "s"
    case "3d": "3"
    case "split": "v"
    case "mech": "m"
    case "files": "f"
    case "diff": "d"
    case "review": "r"
    case "browser": "w"
    case "terminal": "`"
    case _: ""

def View.workspace.keycap(+key: String) -> List<&2, D.Node>:
  D.Dom.when(Bool.not(String.is_empty(key)), D.El{"kbd", "", [D.At{"class", "keycap"}, D.At{"aria-hidden", "true"}], [D.Txt{String.to_upper(key)}]})

def View.workspace.key.attrs(+key: String) -> List<&2, D.Attr>:
  Bool.pick(List<&2, D.Attr>, String.is_empty(key), Nil{}, [D.At{"data-keycap", key}, D.At{"aria-keyshortcuts", String.to_upper(key)}])

# (tog: the rail's own buttons toggle, client.bend's Rail.*: the view on show
# shuts the viewer; the menu's pick a view)
def View.workspace.button(+tog: Bool, +lc: I.Loc, +cur: String, +kind: String, +action: String, +value: String) -> D.Node:
  +key = View.workspace.key(kind)
  D.El{"button", kind, List.append(&2, D.Attr, [D.At{"type", "button"}, D.At{"class", Bool.pick(String, String.eq(cur, kind), "tab on", "tab")},
    D.At{"aria-label", View.workspace.label(lc, kind)}, D.At{"aria-current", Bool.pick(String, String.eq(cur, kind), "page", "false")},
    D.At{"aria-pressed", Bool.pick(String, Bool.and(tog, String.eq(cur, kind)), "true", "false")},
    D.At{"title", View.workspace.label(lc, kind) ++ Bool.pick(String, String.is_empty(key), "", " · " ++ String.to_upper(key))}, D.At{"data-value", Bool.pick(String, tog, U.Rail.value(cur, kind, value), value)}, D.On{"click", Bool.pick(String, tog, U.Rail.act(cur, kind, action), action)}], View.workspace.key.attrs(key)),
    List.append(&2, D.Node, [WI.WIcon.svg(View.workspace.icon(kind), 18), D.Dom.span("workspace-label", View.workspace.label(lc, kind))], View.workspace.keycap(key))}

def View.workspace.choices(+tog: Bool, +lc: I.Loc, +cur: String, term: Bool, +palette: Bool) -> List<&2, D.Node>:
  [D.El{"div", "", [D.At{"class", "workspace-group"}, D.At{"role", "group"}, D.At{"aria-label", "Tools"}], [
    View.workspace.button(False{}, lc, Bool.pick(String, term, "terminal", ""), "terminal", "term-toggle", "@term-size"),
    View.workspace.button(False{}, lc, Bool.pick(String, palette, "commands", ""), "commands", U.Rail.palette(palette), "") ]},
   D.El{"div", "", [D.At{"class", "workspace-group"}, D.At{"role", "group"}, D.At{"aria-label", "Design"}], [
    View.workspace.button(tog, lc, cur, "board", "view-kind", "board"), View.workspace.button(tog, lc, cur, "schematic", "view-kind", "schematic"),
    View.workspace.button(tog, lc, cur, "3d", "view-kind", "3d"), View.workspace.button(tog, lc, cur, "split", "view-kind", "split"), View.workspace.button(tog, lc, cur, "mech", "mech", "")]},
   D.El{"div", "", [D.At{"class", "workspace-group"}, D.At{"role", "group"}, D.At{"aria-label", "Inspect"}], [
    View.workspace.button(tog, lc, cur, "files", "fm", ""), View.workspace.button(tog, lc, cur, "diff", "diff", ""),
    View.workspace.button(tog, lc, cur, "review", "rv-page", "bom"), View.workspace.button(tog, lc, cur, "browser", "view-kind", "browser")]}]

def View.workspace.menu(+lc: I.Loc, +cur: String, term: Bool, palette: Bool) -> D.Node:
  D.El{"details", "view-menu", [D.At{"class", "local-menu workspace-menu"}], [
    D.El{"summary", "", [D.At{"aria-label", "Workspace views and tools"}, D.At{"title", "Workspace views and tools"}], [
      WI.WIcon.svg(Bool.pick(String, String.is_empty(cur), "message-square", View.workspace.icon(cur)), 16),
      D.Dom.span("workspace-current", Bool.pick(String, String.is_empty(cur), MSG.T.bots.tab.chat(lc), View.workspace.label(lc, cur))), WI.WIcon.svg("chevron-down", 14)]},
    D.El{"div", "", [D.At{"class", "local-menu-body"}], View.workspace.choices(False{}, lc, cur, term, palette)}]}

# a phone's viewer screen: every view as a chip in one row that scrolls
# sideways (the menu's buttons, so a tap picks the view as the menu does),
# the one on show marked and inert (its action would close the viewer)
def View.viewer.chip.k(on: Bool, +lc: I.Loc, +kind: String, +action: String, +value: String) -> D.Node:
  match on:
    case True{}:
      D.El{"button", kind, [D.At{"type", "button"}, D.At{"class", "tab on"}, D.At{"aria-label", View.workspace.label(lc, kind)}, D.At{"aria-current", "page"}],
        [WI.WIcon.svg(View.workspace.icon(kind), 18), D.Dom.span("workspace-label", View.workspace.label(lc, kind))]}
    case False{}:
      View.workspace.button(False{}, lc, "", kind, action, value)

def View.viewer.chip(+lc: I.Loc, +cur: String, +kind: String, +action: String, +value: String) -> D.Node:
  View.viewer.chip.k(String.eq(cur, kind), lc, kind, action, value)

def View.viewer.chips(+lc: I.Loc, +cur: String) -> D.Node:
  D.El{"nav", "ph-views", [D.At{"class", "ph-chips ph-views"}, D.At{"aria-label", MSG.T.thread.viewer(lc)}], [
    View.viewer.chip(lc, cur, "board", "view-kind", "board"), View.viewer.chip(lc, cur, "schematic", "view-kind", "schematic"),
    View.viewer.chip(lc, cur, "3d", "view-kind", "3d"), View.viewer.chip(lc, cur, "split", "view-kind", "split"), View.viewer.chip(lc, cur, "mech", "mech", ""),
    View.viewer.chip(lc, cur, "files", "fm", ""), View.viewer.chip(lc, cur, "diff", "diff", ""),
    View.viewer.chip(lc, cur, "review", "rv-page", "bom"), View.viewer.chip(lc, cur, "browser", "view-kind", "browser")]}

# the rail's foot: a toggle host.js answers, its buttons icons only (their
# names as tooltips), kept in this browser
def View.workspace(+lc: I.Loc, +cur: String, +panel: Bool, +term: Bool, +palette: Bool) -> D.Node:
  D.El{"nav", "workspace", [D.At{"class", "workspace-bar"}, D.At{"data-loading", MSG.T.viewer.loading(lc)}, D.At{"aria-label", MSG.T.thread.viewer(lc)}], [
    D.El{"div", "", [D.At{"class", "workspace-rail"}], View.workspace.choices(True{}, lc, Bool.pick(String, panel, cur, ""), term, palette)},
    View.workspace.menu(lc, Bool.pick(String, panel, cur, ""), term, palette),
    D.El{"div", "", [D.At{"class", "workspace-close"}], D.Dom.when(panel, WI.WIcon.button("viewer-close", "panel", "", MSG.T.viewer.close(lc), "panel-right", 18, MSG.T.viewer.close(lc)))},
    WI.WIcon.button("rail-toggle", "rail-toggle", "", "Compact or full rail", "chevron-left", 14, ""),
    D.El{"span", "viewer-feedback", [D.At{"class", "workspace-feedback"}, D.At{"role", "status"}], Nil{}}]}

# Buy it at the rail's foot (client.bend's Fab.rail, ordering on), apart
# from the views: a cart, the words and the price under them while the
# rail is wide; the whole label is its tooltip. It does what the Ready to
# order bar's Buy it does.
def View.workspace.buy(+label: String) -> List<&2, D.Node>:
  D.Dom.when(Bool.not(String.is_empty(label)), D.El{"div", "", [D.At{"class", "workspace-buy"}], [
    D.El{"button", "buy", [D.At{"type", "button"}, D.At{"class", "buy"}, D.At{"aria-label", label}, D.At{"title", label ++ ". " ++ View.fab.buy.tip()}, D.At{"data-value", ""}, D.On{"click", "buy"}],
      List.append(&2, D.Node, [WI.WIcon.svg("shopping-cart", 18), D.Dom.span("workspace-label", U.Fab.rail.word(label))],
        D.Dom.when(Bool.not(String.is_empty(U.Fab.rail.price(label))), D.Dom.span("price", U.Fab.rail.price(label))))}]})

# The top bar replaces the thread header on a desktop, so its restore
# action belongs beside Buy it in the rail as well.
def View.workspace.parts(+u: U.Ui) -> List<&2, D.Node>:
  D.Dom.when(U.Fab.mini.k(U.Fab.min.of(U.Ui.state.of(u), U.Ui.sel(u)), U.Fab.ready.all(u)),
    D.El{"div", "", [D.At{"class", "workspace-buy"}], [D.Dom.button("parts", "fab-show", "", "Show the Ready to order bar again", "Show parts")]})

# the rail with what goes at its foot (Buy it); a flat bar gets nothing
def View.workspace.foot(rail: Bool, n: D.Node, foot: List<&2, D.Node>) -> D.Node:
  match rail n:
    case True{} D.El{tag, key, attrs, kids}: D.El{tag, key, attrs, List.append(&2, D.Node, kids, foot)}
    case False{} D.El{tag, key, attrs, kids}: D.El{tag, key, attrs, kids}
    case _ D.Txt{text}: D.Txt{text}

def View.tabs.loc(+lc: I.Loc, +cur: String) -> D.Node:
  D.El{"nav", "", [D.At{"class", "viewer-tabs"}, D.At{"aria-label", MSG.T.thread.viewer(lc)}], [
    View.workspace.menu(lc, cur, False{}, False{}), WI.WIcon.button("icon", "panel", "", MSG.T.viewer.close(lc), "x", 18, MSG.T.viewer.close(lc))]}

def View.workspace.legacy(+lc: I.Loc, +cur: String, +panel: Bool, term: Bool) -> D.Node:
  D.El{"nav", "workspace", [D.At{"class", "workspace-bar"}, D.At{"data-loading", MSG.T.viewer.loading(lc)}, D.At{"aria-label", MSG.T.thread.viewer(lc)}],
    View.tool(TO.Tools.term.loc(lc, term, "term-toggle", "@term-size")) <>
    D.Dom.button("commands", "ws-palette-open", "", "Command palette · Ctrl+Shift+P / Cmd+Shift+P", View.commands.label(lc)) <>
    List.append(&2, D.Node, View.destinations(lc, Bool.pick(String, panel, cur, "")),
      List.append(&2, D.Node, D.Dom.when(panel, D.Dom.button("viewer-close", "panel", "", MSG.T.viewer.close(lc), MSG.T.viewer.close(lc))), [D.El{"span", "viewer-feedback", [D.At{"class", "workspace-feedback"}, D.At{"role", "status"}], Nil{}}]))}

def View.workspace.for(rail: Bool, +lc: I.Loc, +cur: String, +panel: Bool, +term: Bool, +palette: Bool) -> D.Node:
  match rail:
    case True{}: View.workspace(lc, cur, panel, term, palette)
    case False{}: View.workspace.legacy(lc, cur, panel, term)

def View.tabs.for(rail: Bool, +lc: I.Loc, +cur: String) -> D.Node:
  match rail:
    case True{}: View.tabs.loc(lc, cur)
    case False{}:
      D.El{"nav", "", [D.At{"class", "viewer-tabs"}, D.At{"aria-label", MSG.T.thread.viewer(lc)}],
        List.append(&2, D.Node, View.destinations(lc, cur), [WI.WIcon.button("icon", "panel", "", MSG.T.viewer.close(lc), "x", 16, "")])}

def View.work.label(lc: I.Loc) -> String:
  match lc:
    case I.En{}: "Work"
    case I.Ja{}: "作業"

def View.workbar(+lc: I.Loc, +st: M.State, +info: Map<&2, String>, +id: String, +turn: M.TurnState, +pending: Nat, running: Bool) -> D.Node:
  +signal = SG.Signal.cached(info, id)
  +shown = SG.Signal.display.shown(signal, turn, pending, st, id)
  D.El{"div", "workbar", [D.At{"class", "thread-workbar"}, D.At{"role", "group"}, D.At{"aria-label", View.work.label(lc)}],
    List.append(&2, D.Node, [View.dot.tip(SG.Signal.dot.name(SG.Signal.display.name(signal, st, id), turn, pending), SG.Signal.display.label(lc, signal, shown, st, id)), D.Dom.span("work-state", SG.Signal.display.label(lc, signal, shown, st, id))], View.tools(List.append(&2, TO.HeaderTool, TO.Tools.stop.loc(lc, running), TO.Tools.retry.loc(lc, View.failed(turn)))))}

# Keep a running Stop control, but never reserve a status strip for an
# idle bot conversation. Ordinary thread workbars retain their summaries.
def View.thread.workbar(+lc: I.Loc, summary: Bool, +st: M.State, +info: Map<&2, String>, +id: String, +turn: M.TurnState, +pending: Nat, running: Bool) -> List<&2, D.Node>:
  match summary running:
    case True{} _:
      [View.workbar(lc, st, info, id, turn, pending, running)]
    case False{} True{}:
      [D.El{"div", "workbar", [D.At{"class", "thread-workbar"}, D.At{"role", "group"}, D.At{"aria-label", View.work.label(lc)}], View.tools(TO.Tools.stop.loc(lc, True{}))}]
    case False{} False{}:
      Nil{}

def View.tabs(+cur: String) -> D.Node:
  View.tabs.loc(I.En{}, cur)

# what the thread's page draws of its own head: nothing a top bar shows
# (own: the bar shows this thread), else its header, work line and dock
def View.top.own(own: Bool, rail: Bool, +lc: I.Loc, hd: List<&2, D.Node>, t: M.Thread, +st: M.State, +chip: String, mini: List<&2, TO.HeaderTool>, +panel: Bool, snoozed: Bool, picking: Bool, term: Bool, +diff: Bool, +del: Bool, +ren: String, +rtext: String) -> List<&2, D.Node>:
  match own:
    case True{}: Nil{}
    case False{}: View.top(rail, lc, hd, t, st, chip, mini, panel, snoozed, picking, term, diff, del, ren, rtext)

def View.thread.workbar.own(own: Bool, +lc: I.Loc, summary: Bool, +st: M.State, +info: Map<&2, String>, +id: String, +turn: M.TurnState, +pending: Nat, running: Bool) -> List<&2, D.Node>:
  match own:
    case True{}: Nil{}
    case False{}: View.thread.workbar(lc, summary, st, info, id, turn, pending, running)

def View.thread.subs.own(own: Bool, +lc: I.Loc, +st: M.State, +nf: Map<&2, String>, +id: String, +now: Nat) -> List<&2, D.Node>:
  match own:
    case True{}: View.subs.dock.at(lc, st, nf, id, SB.Subs.rows(st, nf, id, now), now)
    case False{}: View.thread.subs(lc, st, nf, id, now)

def View.thread.frame(rail: Bool, +attrs: List<&2, D.Attr>, +nav: D.Node, +content: List<&2, D.Node>) -> D.Node:
  match rail:
    case True{}:
      D.El{"main", "thread", attrs, [nav, D.El{"div", "thread-content", [D.At{"class", "thread-content"}], content}]}
    case False{}:
      # Retained hosts append their hidden font probe after the page. Anchor
      # its absolute box so measuring terminal cells cannot extend the page.
      D.El{"main", "thread", attrs, D.El{"style", "workspace-compat", Nil{}, [D.Txt{".term-probe { top: 0; left: 0; }"}]} <> nav <> content}

# The thread's timeline (#timeline): everything of the page that its rows
# make, so the hub can draw it alone for another page of rows (thin-view
# .bend's Page.splice) and put it into the page it drew before
def View.thread.timeline(+lc: I.Loc, +st: M.State, +id: String, +drafts: Map<&2, String>, +nf: Map<&2, String>, +turn: M.TurnState, +tr: U.MailvLocal, +now: Nat, +live: Map<&2, String>, pend: List<&2, String>, fab: List<&2, D.Node>, +pill: String) -> D.Node:
  D.El{"div", "timeline", [D.At{"class", "timeline"}, D.At{"id", "timeline"}, D.At{"data-reading-seen", U.Hold.source(M.State.mine(st, id))}], List.concat(&2, D.Node, [
    fab, View.parent.loc(lc, st, id, U.Tasks.up(st, id)),
    [D.El{"div", "work", [D.At{"class", "work"}], View.pins.loc(lc, U.Ec.pins(U.Ec.make(U.MailvLocal.terms(tr), st, id, now), st, id), U.Ec.make(U.MailvLocal.terms(tr), st, id, now))}],
    View.paged(lc, drafts, M.State.mine(st, id), st, id, nf, View.running(turn), U.Ec.make(U.MailvLocal.terms(tr), st, id, now)),
    View.sending.loc(lc, pend), View.thread.live.loc(lc, st, nf, live, id), View.hold.pill(pill)])}

def View.thread.p(+lc: I.Loc, +u: U.Ui, t: M.Thread, pend: List<&2, String>, +atts: List<&2, String>, +up: String, +term: Bool, +hd: List<&2, D.Node>, vo: List<&2, D.Node>, +mic: String, fab: List<&2, D.Node>, rd: List<&2, D.Node>, mini: List<&2, TO.HeaderTool>, +pill: String) -> D.Node:
  match u t:
    case U.Ui{+st, +sel, +drafts, +live, +op, _, +now, _, +panel, _, _, _, _, _, +nf, _, +tr} M.Thread{+id, +p, ti, c, a, +turn, +pe, ar, o, sa, ua, pi, en, br, wt, +pr, +mo, rm, im, se, de}:
      +own = Bool.and(View.chrome(u), List.is_empty(&2, D.Node, hd))
      View.thread.frame(View.rail(u), [D.At{"class", Bool.pick(String, View.rail(u), Bool.pick(String, term, "thread-view workspace-thread with-term", "thread-view workspace-thread"), Bool.pick(String, term, "thread-view with-term", "thread-view")) ++ Bool.pick(String, U.Flag.get(op, "drop"), " dropping", "")},
        D.At{"data-thread", id}, D.At{"data-project", p}, D.At{"data-drop", "attach"}, D.At{"data-drop-hover", "drop-hover"}],
        View.workspace.foot(View.rail(u), View.workspace.for(View.rail(u), lc, U.Rail.tab.of(panel, U.Str.get(nf, "view.kind:" ++ id), U.Str.get(nf, "view.path:" ++ id)), panel, term, U.Flag.get(op, "palette")), List.append(&2, D.Node, View.workspace.buy(U.Fab.rail(u)), View.workspace.parts(u))), List.concat(&2, D.Node, [
        View.top.own(own, View.rail(u), lc, hd, M.Thread{id, p, ti, c, a, turn, pe, ar, o, sa, ua, pi, en, br, wt, pr, mo, rm, im, se, de}, st, UZ.Chip.of(st, id, now), mini, panel, M.Snooze.on(now, U.Side.until(st, id)), U.Flag.get(op, "snooze"), term, String.eq(U.Str.get(nf, "view.kind:" ++ id), "diff"), String.eq(U.Str.get(drafts, "@del"), id), U.Str.get(drafts, "@ren"), U.Str.get(drafts, "@f.title")),
        View.kx.note(U.KX.note(nf, sel), nf, sel), View.kn.loc(lc, U.Tour.kn(u, U.KN.thread(nf, st, sel)), True{}),
        View.thread.workbar.own(own, lc, BI.BU.chat.summary(st, id), st, nf, id, turn, pe, View.running(turn)),
        View.thread.subs.own(own, lc, st, nf, id, now),
        View.thread.todos.own(own, lc, nf, st, id, U.Todos.shown(st, id, View.running(turn))),
        View.order(lc, U.Fab.buying(st, id), st, id, View.running(turn)),
        [View.thread.timeline(lc, st, id, drafts, nf, turn, tr, now, live, pend, fab, pill),
         View.asks.box(lc, st, id, U.Str.get(drafts, sel), nf, drafts),
         View.queued(lc, M.Queue.context(M.Queue.items(st, id)), View.running(turn))],
        rd, [View.composer.t(lc, st, sel, drafts, op, nf, turn, pr, mo, atts, up, vo, mic)]]))

def View.thread.h(+lc: I.Loc, +u: U.Ui, +t: M.Thread, hd: List<&2, D.Node>) -> D.Node:
  View.thread.p(lc, u, t, U.Ui.pending(u, U.Ui.sel(u)), U.Ui.atts(u, U.Ui.sel(u)), U.Ui.uploading(u, U.Ui.sel(u)), U.Ui.flag(u, "term"), hd, D.Dom.cat(View.rpl.chip(lc, u), D.Dom.cat(View.btw(lc, u), View.voice.loc(lc, u, U.Ui.sel(u)))), View.mic.of(u, U.Ui.sel(u)), Nil{}, View.ready(u), U.Fab.mini.tools(u), U.Hold.pill(u, M.Thread.id(t)))

def View.thread.timeline.k(+lc: I.Loc, u: U.Ui, t: M.Thread, pend: List<&2, String>, +pill: String) -> D.Node:
  match u t:
    case U.Ui{+st, _, +drafts, +live, _, _, +now, _, _, _, _, _, _, _, +nf, _, +tr} M.Thread{+id, _, _, _, _, +turn, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      View.thread.timeline(lc, st, id, drafts, nf, turn, tr, now, live, pend, Nil{}, pill)

# the timeline as View.thread.h draws it (no fab rows above it)
def View.thread.timeline.u(+lc: I.Loc, +u: U.Ui, +t: M.Thread) -> D.Node:
  View.thread.timeline.k(lc, u, t, U.Ui.pending(u, U.Ui.sel(u)), U.Hold.pill(u, M.Thread.id(t)))

def View.thread(+lc: I.Loc, +u: U.Ui, t: M.Thread) -> D.Node:
  View.thread.h(lc, u, t, Nil{})

# a bot's chat with nothing in it yet says what to do there
def View.bot.hint(+lc: I.Loc, empty: Bool, +name: String) -> List<&2, D.Node>:
  match empty:
    case True{}: [D.El{"p", "", [D.At{"class", "bot-hint"}], [D.Txt{MSG.T.bots.chat.hint(lc, name)}]}]
    case False{}: Nil{}

def View.bot.chat(+lc: I.Loc, +u: U.Ui, +b: BT.Bot, +hd: BI.Head, +id: String) -> D.Node:
  View.thread.h(lc, u, U.Ui.thread(U.Ui.state(u), id), List.append(&2, D.Node, [WB.WB.head.loc(lc, u, b, hd), WB.WB.tabs(lc, hd)],
    View.bot.hint(lc, List.is_empty(&2, M.Entry, M.State.mine(U.Ui.state(u), id)), BT.Bot.name(b))))

# a bot, here or mirrored from a linked machine: its header and tabs
# (BU.head), then its thread (Chat) or another tab
def View.bot.k(+lc: I.Loc, chat: Bool, +u: U.Ui, +b: BT.Bot, +hd: BI.Head) -> D.Node:
  match chat:
    case True{}:
      View.bot.chat(lc, u, b, hd, U.Bots.chat.of(U.Ui.state(u), BT.Bot.id(b), BT.Bot.thread(b)))
    case False{}:
      D.El{"main", "bot", [D.At{"class", "thread-view bot-view"}], [WB.WB.head.loc(lc, u, b, hd), WB.WB.tabs(lc, hd), WB.WB.main(lc, u, b, BI.Head.tab(hd))]}

def View.bot.h(+lc: I.Loc, +u: U.Ui, +b: BT.Bot, +hd: BI.Head) -> D.Node:
  View.bot.k(lc, String.eq(BI.Head.tab(hd), "chat"), u, b, hd)

def View.bot(+lc: I.Loc, +u: U.Ui, +b: BT.Bot) -> D.Node:
  View.bot.h(lc, u, b, BI.BU.head.bot(u, b))

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
  List.append(&2, D.Node, List.append(&2, D.Node,
    Bool.pick(List<&2, D.Node>, WB.WB.browser.direct(u, U.Ui.page(u)), [WB.WB.browser.drive.loc(lc, u, U.Ui.page(u))], View.browser.strip.loc(lc, U.CefUI.strip_on(u, U.Ui.page(u)), u)), WB.WB.browser.controls(lc, u, U.Ui.page(u))),
    [WB.WB.browser.surface.loc(lc, u, U.Ui.page(u), MSG.T.browser.thread(lc))])

# The Mechanical page (client.bend's Mpage): a chip per part, Open in 3D
# and Refresh, the part's renders (a click opens one in the lightbox), and
# where to get FreeCAD when the hub has none

def View.browser(+u: U.Ui) -> List<&2, D.Node>:
  View.browser.loc(I.En{}, u)

def View.mech.chips.named(+u: U.Ui, ps: List<&2, MC.MPart>, +cur: String) -> List<&2, D.Node>:
  match ps:
    case Nil{}:
      Nil{}
    case MC.MPart{+p, +n, _} <> t:
      D.Dom.button(Bool.pick(String, String.eq(p, cur), "on", ""), "mech-part", p, p, U.Asset.name(u, MC.Mech.abs(U.Mpage.root(u), p), n)) <> View.mech.chips.named(u, t, cur)

def View.mech.chips(ps: List<&2, MC.MPart>, cur: String) -> List<&2, D.Node>:
  View.mech.chips.named(U.Ui.init(0n), ps, cur)

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

def View.mech.error(+lc: I.Loc, +u: U.Ui) -> List<&2, D.Node>:
  D.Dom.when(Bool.not(String.is_empty(U.Mpage.err(u))), D.Dom.span("mech-say", MSG.T.viewer.render.failed(lc, U.Mpage.err(u))))

def View.mech.k(+lc: I.Loc, state: Nat, +u: U.Ui) -> List<&2, D.Node>:
  match state:
    case 0n:
      D.Dom.span("mech-say", MSG.T.viewer.mech.loading(lc)) <> View.mech.error(lc, u)
    case 1n:
      D.Dom.span("mech-say", MC.Mech.none()) <> View.mech.error(lc, u)
    case _:
      D.El{"div", "", [D.At{"class", "mech-head"}], List.append(&2, D.Node, View.mech.chips.named(u, U.Mpage.parts(u), U.Mpage.cur.path(u)),
        [D.Dom.button("", "mech-3d", MC.Mech.abs(U.Mpage.root(u), U.Mpage.cur.path(u)), MSG.T.mech.turn(lc), "3D"),
         D.Dom.button(Bool.pick(String, String.is_empty(U.Mpage.busy(u)), "primary", "busy"), Bool.pick(String, String.is_empty(U.Mpage.busy(u)), "mech-render", ""),
           U.Mpage.cur.path(u), MSG.T.mech.render.tip(lc), U.Mpage.render.label.loc(lc, u)),
         D.Dom.button("", "mech-refresh", "", MSG.T.mech.refresh.tip(lc), MSG.T.common.refresh(lc))])}
      <> List.append(&2, D.Node, View.mech.error(lc, u),
        View.mech.part(U.Mpage.cur(u), U.Mpage.root(u)))

def View.mech(+lc: I.Loc, +u: U.Ui) -> List<&2, D.Node>:
  [D.El{"div", "", [D.At{"class", "mech-page"}],
    List.append(&2, D.Node, View.mech.k(lc, U.Mpage.state(u), u), D.Dom.when(Bool.not(String.is_empty(U.Mpage.note.loc(lc, u))), D.Dom.span("mech-note", U.Mpage.note.loc(lc, u))))}]

# Open in KiCad: the project opens in KiCad on the hub's machine, as the
# window's button does
def View.kicad.loc(+lc: I.Loc) -> D.Node:
  D.El{"span", "", Nil{}, [D.Dom.button("", "kicad-open", "", MSG.T.viewer.kicad.tip(lc), MSG.T.palette.kicad(lc)), D.Dom.button("", "kicad-export10", "", MSG.T.viewer.kicad10.tip(lc), MSG.T.viewer.kicad10(lc))]}

# KiCad actions are secondary to the name of the board on show.
def View.plot.kicad.loc(+lc: I.Loc) -> D.Node:
  D.El{"details", "", [D.At{"class", "local-menu plot-kicad"}], [
    D.El{"summary", "", Nil{}, [D.Txt{"KiCad"}]},
    D.El{"div", "", [D.At{"class", "local-menu-body"}], [View.kicad.loc(lc)]}]}

def View.kicad() -> D.Node:
  View.kicad.loc(I.En{})

def View.fit.loc(+lc: I.Loc) -> D.Node:
  D.El{"button", "", [D.At{"class", "fit"}, D.At{"title", MSG.T.viewer.fit.tip(lc)}, D.At{"data-view", "fit"}], [D.Txt{MSG.T.viewer.fit(lc)}]}

def View.fit() -> D.Node:
  View.fit.loc(I.En{})

def View.bar.loc(+lc: I.Loc, kids: List<&2, D.Node>, +hint: String) -> D.Node:
  D.El{"div", "", [D.At{"class", "viewer-bar"}], List.append(&2, D.Node, kids, [
    D.El{"details", "viewer-help", [D.At{"class", "local-menu viewer-help"}], [
      D.El{"summary", "", [D.At{"aria-label", "Viewer help"}, D.At{"title", "Viewer help"}], [D.Txt{"?"}]},
      D.El{"div", "", [D.At{"class", "local-menu-body"}], [D.Txt{hint}]}]}, View.fit.loc(lc)])}

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
# Board identity is the server's root-relative file path, never its short
# label. Only exact repeated references collapse; no file is auto-selected.
def View.board.has(fs: List<&2, String>, +f: String) -> Bool:
  match fs:
    case Nil{}: False{}
    case h <> t: Bool.or(String.eq(h, f), View.board.has(t, f))

def View.board.on(+path: String, +f: String) -> Bool:
  Bool.or(String.eq(path, f), String.eq(path, U.Bd.path("schematic", f)))

def View.board.relative(+path: String, +root: String) -> String:
  Bool.pick(String, String.starts_with(path, root ++ "/"), String.drop(path, 1n+String.length(root)), path)

# (a sheet of a board's schematic in its folder is that board's)
def View.board.near(+path: String, +f: String) -> Bool:
  Bool.or(View.board.on(path, f), Bool.and(Bool.not(String.is_empty(MC.Mech.parent(f))), U.Bd.on("/" ++ path, f)))

# (else the board in the file's own folder: a sub-sheet beside a board at
# the project's top is that board's, not a board named after the sheet)
def View.board.same(fs: List<&2, String>, +path: String) -> String:
  match fs:
    case Nil{}: path
    case +f <> t: Bool.pick(String, String.eq(MC.Mech.parent(f), MC.Mech.parent(path)), f, View.board.same(t, path))

def View.board.current.go(fs: List<&2, String>, +path: String, +all: List<&2, String>) -> String:
  match fs:
    case Nil{}: View.board.same(all, path)
    case +f <> t: Bool.pick(String, View.board.near(path, f), f, View.board.current.go(t, path, all))

def View.board.current(+fs: List<&2, String>, +path: String) -> String:
  +exact = U.Bd.exact(fs, path)
  Bool.pick(String, String.is_empty(exact), View.board.current.go(fs, path, fs), exact)

def View.board.rows(fs: List<&2, String>, +cur: String, +own: String, +seen: List<&2, String>) -> List<&2, D.Node>:
  match fs:
    case Nil{}: Nil{}
    case +f <> t:
      List.append(&2, D.Node, D.Dom.when(Bool.not(View.board.has(seen, f)),
        D.El{"button", f, [D.At{"type", "button"}, D.At{"class", Bool.pick(String, String.eq(cur, f), "board-option on", "board-option")},
          D.At{"aria-current", Bool.pick(String, String.eq(cur, f), "true", "false")}, D.On{"click", "board-pick"}, D.At{"data-value", f}, D.At{"data-board-path", f}, D.At{"title", f}], [
          D.Dom.span("board-name", MC.Mech.stem(f)), D.El{"small", "", Nil{}, [D.Txt{f}]},
          D.Dom.span("board-default", Bool.pick(String, String.eq(own, f), "Default board", ""))]}),
      View.board.rows(t, cur, own, f <> seen))

def View.board.rows.named(+u: U.Ui, fs: List<&2, String>, +cur: String, +own: String, +seen: List<&2, String>) -> List<&2, D.Node>:
  match fs:
    case Nil{}: Nil{}
    case +f <> t:
      List.append(&2, D.Node, D.Dom.when(Bool.not(View.board.has(seen, f)),
        D.El{"button", f, [D.At{"type", "button"}, D.At{"class", Bool.pick(String, String.eq(cur, f), "board-option on", "board-option")},
          D.At{"aria-current", Bool.pick(String, String.eq(cur, f), "true", "false")}, D.On{"click", "board-pick"}, D.At{"data-value", f}, D.At{"data-board-path", f}, D.At{"title", f}], [
          D.Dom.span("board-name", U.Asset.name(u, MC.Mech.abs(U.Ui.root(u), f), MC.Mech.stem(f))), D.El{"small", "", Nil{}, [D.Txt{f}]},
          D.Dom.span("board-default", Bool.pick(String, String.eq(own, f), "Default board", ""))]}),
      View.board.rows.named(u, t, cur, own, f <> seen))

def View.board.chips(+fs: List<&2, String>, +cur: String) -> List<&2, D.Node>:
  View.board.rows(fs, View.board.current(fs, cur), "", Nil{})

# (a design-history step or a comparison names its board by the live file
# it is a version of, HS.Hist.family, never by its raw version path). Each
# time it opens it reads the project's boards again (host.js runs its
# data-on-open, "board-list": client.bend's Bd.look); the rows are keyed by
# their paths, so a list that came back the same leaves them as they are
def View.boards.selector.named(+u: U.Ui, fs: List<&2, String>, +path: String, +own: String, +root: String) -> List<&2, D.Node>:
  match fs:
    case Nil{}: [D.Dom.span("what", MC.Mech.last(HS.Hist.family(path)))]
    case +f <> +t:
      +cur = View.board.current(f <> t, View.board.relative(HS.Hist.family(path), root))
      [D.El{"details", "board-selector", [D.At{"class", "local-menu board-selector"}, D.On{"open", "board-list"}], [
        D.El{"summary", "", [D.At{"aria-label", "Select board"}, D.At{"title", cur}], [WI.WIcon.svg("circuit-board", 16),
          D.El{"span", "", [D.At{"class", "board-current"}], [D.Dom.span("", Bool.pick(String, String.is_empty(cur), "Select board", MC.Mech.stem(cur))), D.El{"small", "", Nil{}, [D.Txt{MC.Mech.parent(cur)}]}]}, WI.WIcon.svg("chevron-down", 14)]},
        D.El{"div", "", [D.At{"class", "local-menu-body"}], [
          D.El{"label", "", Nil{}, [D.Txt{"Find a board"}, D.El{"input", "board-search", [D.At{"type", "search"}, D.At{"data-board-filter", ""}, D.At{"aria-label", "Find a board by name or path"}, D.At{"placeholder", "Name or path"}], Nil{}}]},
          D.Dom.span("board-origin", "Folder: " ++ root),
          D.El{"div", "", [D.At{"class", "board-options"}], View.board.rows.named(u, f <> t, cur, own, Nil{})},
          D.El{"p", "", [D.At{"class", "board-no-match"}, D.At{"hidden", ""}], [D.Txt{"No matching boards"}]}]}]}]

# Unlike native <details> hiding, the shut catalog never builds option
# nodes. Search and continuation operate on metadata at the owning actor.
# Compatibility projection for callers without a client/settings context.
def View.boards.selector(fs: List<&2, String>, path: String, own: String, root: String) -> List<&2, D.Node>:
  View.boards.selector.named(U.Ui.init(0n), fs, path, own, root)

def View.boards.catalog.button(show: Bool, source: String, query: String, offset: Nat, +label: String) -> List<&2, D.Node>:
  match show:
    case False{}: Nil{}
    case True{}: [D.Dom.button("shelf", "board-catalog", BC.Catalog.command(source, True{}, query, offset), label, label)]

def View.boards.catalog.page(+u: U.Ui, page: VP.Page, +cur: String, own: String, +source: String, +query: String) -> List<&2, D.Node>:
  match page:
    case VP.Page{_, +total, +start, +limit, ids}:
      [D.El{"div", "", [D.At{"class", "local-menu-body"}], List.append(&2, D.Node, [
        D.El{"label", "", Nil{}, [D.Txt{"Find a board"}, D.El{"input", "board-search", [D.At{"type", "search"}, D.At{"data-board-query", ""}, D.At{"data-render-query", query}, D.At{"value", query}, D.At{"maxlength", "160"}, D.At{"aria-label", "Find a board by name or path"}, D.At{"placeholder", "Name or path"}], Nil{}}]},
        D.El{"div", "", [D.At{"class", "board-options"}], View.board.rows.named(u, ids, cur, own, Nil{})},
        D.Dom.span("board-count", Nat.show(total) ++ " boards · selected board kept visible")],
        List.append(&2, D.Node, View.boards.catalog.button(Nat.is_gt(start, 0n), source, query, Nat.sub(start, limit), "Previous boards"),
          View.boards.catalog.button(Nat.is_lt(Nat.add(start, limit), total), source, query, Nat.add(start, limit), "Next boards")))}]

def View.boards.catalog.options(open: Bool, +u: U.Ui, +ids: List<&2, String>, +cur: String, +source: String, +state: J.Json) -> List<&2, D.Node>:
  match open:
    case False{}: Nil{}
    case True{}:
      +query = BC.Catalog.query(state, source)
      +page = BC.Catalog.page(ids, cur, source, state, VP.Page.limit("boards", U.Ui.info(u, "local.view.budget")))
      View.boards.catalog.page(u, page, cur, U.Ui.info(u, "fab.board:" ++ U.Ui.sel(u)), source, query)

def View.boards.catalog(+u: U.Ui) -> List<&2, D.Node>:
  +ids = BC.Catalog.unique(U.Bd.files(u))
  +cur = View.board.current(ids, View.board.relative(HS.Hist.family(U.Bd.or(U.Bd.id(u), U.Bd.shown(u, U.Ui.view_get(u, "view.path")))), U.Ui.root(u)))
  +source = BC.Catalog.source(U.Ui.sel(u), U.Ui.root(u), ids)
  +state = BC.Catalog.state(U.Ui.info(u, "local.board.catalog"))
  +open = BC.Catalog.open(state, source)
  [D.El{"details", "board-selector", List.append(&2, D.Attr,
      [D.At{"class", "local-menu board-selector"}, D.At{"data-board-catalog", source}, D.At{"data-scope", U.Ui.sel(u)}, D.At{"data-render-open", Bool.pick(String, open, "1", "0")}],
      Bool.pick(List<&2, D.Attr>, open, [D.At{"open", ""}], Nil{})),
    D.El{"summary", "", [D.At{"aria-label", "Select board"}, D.At{"title", cur}], [WI.WIcon.svg("circuit-board", 16),
      D.El{"span", "", [D.At{"class", "board-current"}], [D.Dom.span("", Bool.pick(String, String.is_empty(cur), "Select board", MC.Mech.stem(cur))), D.El{"small", "", Nil{}, [D.Txt{MC.Mech.parent(cur)}]}]}, WI.WIcon.svg("chevron-down", 14)]}
      <> View.boards.catalog.options(open, u, ids, cur, source, state)}]

def View.board.legacy(fs: List<&2, String>, +cur: String) -> List<&2, D.Node>:
  match fs:
    case Nil{}: Nil{}
    case +f <> t:
      D.Dom.button(Bool.pick(String, U.Bd.on(cur, f), "on", ""), "board-pick", f, f, U.Bd.name(f)) <> View.board.legacy(t, cur)

def View.boards.contract(current: Bool, +u: U.Ui) -> List<&2, D.Node>:
  match current:
    case True{}: View.boards.catalog(u)
    case False{}: View.boards.selector.named(u, U.Bd.files(u), U.Bd.or(U.Bd.id(u), U.Bd.shown(u, U.Ui.view_get(u, "view.path"))), U.Ui.info(u, "fab.board:" ++ U.Ui.sel(u)), U.Ui.root(u))

def View.boards.for(rail: Bool, +u: U.Ui, +what: String) -> List<&2, D.Node>:
  match rail:
    case True{}: View.boards.contract(String.eq(U.Ui.info(u, "local.board.catalog.version"), "1"), u)
    case False{}:
      Bool.pick(List<&2, D.Node>, U.Bd.many(u), View.board.legacy(U.Bd.files(u), U.Bd.shown(u, U.Ui.view_get(u, "view.path"))), [D.Dom.span("what", what)])

def View.boards(+u: U.Ui, +what: String) -> List<&2, D.Node>:
  View.boards.for(View.rail(u), u, what)

def View.sheet.chips(rs: List<&2, HI.Row>, +i: Nat, +cur: String) -> List<&2, D.Node>:
  View.sheet.chips.loc(I.En{}, rs, i, cur)

# The schematic's sheet navigation (client.bend's Sch), by instance: in
# the bar Back, Forward, Up and Top, the hierarchy's toggle, the trail from
# the top sheet to the one on show and where it sits; under the bar why a
# sheet asked for is not there, the hierarchy (a branch folds; the path to
# the sheet on show stays open) and the sheets inside the one on show.
# Every control is a button (keyboard and touch alike, nothing on hover
# only); a click on a sheet symbol on the canvas opens it too (plot2d.js,
# the canvas's data-sheets).
def View.sch.label(+lc: I.Loc, +i: Nat, +name: String) -> String:
  Bool.pick(String, Nat.is_eq(i, 0n), MSG.T.viewer.sheet.top(lc), name)

def View.sch.off(on: Bool) -> List<&2, D.Attr>:
  match on:
    case True{}:
      Nil{}
    case False{}:
      [D.At{"disabled", "disabled"}, D.At{"aria-disabled", "true"}]

def View.sch.btn(+key: String, on: Bool, +title: String, +label: String) -> D.Node:
  D.El{"button", key, List.append(&2, D.Attr, [D.At{"class", "sch-btn"}, D.At{"title", title}, D.At{"aria-label", label}, D.At{"data-value", ""}, D.On{"click", key}], View.sch.off(on)), [D.Txt{label}]}

def View.sch.toggle(+lc: I.Loc, +on: Bool) -> D.Node:
  D.El{"button", "sch-tree", [D.At{"class", "sch-btn" ++ Bool.pick(String, on, " on", "")}, D.At{"title", MSG.T.viewer.sch.tree.tip(lc)},
    D.At{"aria-pressed", Bool.pick(String, on, "true", "false")}, D.At{"aria-controls", "sch-tree"}, D.At{"data-value", ""}, D.On{"click", "sch-tree"}], [D.Txt{MSG.T.viewer.sch.tree(lc)}]}

def View.sch.current(+st: Nat) -> String:
  Bool.pick(String, Nat.is_eq(st, 2n), "page", "false")

def View.sch.crumbs(+lc: I.Loc, rs: List<&2, U.SchRow>) -> List<&2, D.Node>:
  match rs:
    case Nil{}:
      Nil{}
    case U.SchRow{+i, _, +n, +v, +st, _, _, _} <> t:
      D.El{"button", "", [D.At{"class", "sch-crumb" ++ Bool.pick(String, Nat.is_eq(st, 2n), " on", "")}, D.At{"title", HS.Hist.field(v, 1n)},
        D.At{"aria-current", View.sch.current(st)}, D.At{"data-value", v}, D.On{"click", "sch-go"}], [D.Txt{View.sch.label(lc, i, n)}]}
        <> View.sch.crumbs(lc, t)

# (KiCad's page number when the files give it, else the row's place in the tree)
def View.sch.pos(+lc: I.Loc, out: Bool, +at: Nat, +count: Nat, +page: String) -> String:
  match out:
    case True{}:
      MSG.T.viewer.sch.outside(lc)
    case False{}:
      MSG.T.viewer.sch.position(lc, Bool.pick(String, String.is_empty(page), Nat.show(1n+at), page), Nat.show(count))

def View.sch.bar(+lc: I.Loc, v: U.SchView) -> List<&2, D.Node>:
  match v:
    case U.SchView{_, trail, _, +at, +count, _, back, fwd, out, tree, _, _, page}:
      [View.sch.btn("sch-back", back, MSG.T.viewer.sch.back.tip(lc), "‹ " ++ MSG.T.viewer.sch.back(lc)),
       View.sch.btn("sch-fwd", fwd, MSG.T.viewer.sch.forward.tip(lc), MSG.T.viewer.sch.forward(lc) ++ " ›"),
       View.sch.btn("sch-up", Bool.not(Nat.is_eq(at, 0n)), MSG.T.viewer.sch.up.tip(lc), "↑ " ++ MSG.T.viewer.sch.up(lc)),
       View.sch.toggle(lc, tree),
       D.El{"nav", "", [D.At{"class", "sch-trail"}, D.At{"aria-label", MSG.T.viewer.sch.trail(lc)}], View.sch.crumbs(lc, trail)},
       D.El{"span", "", [D.At{"class", "sch-pos"}, D.At{"role", "status"}], [D.Txt{Bool.pick(String, Nat.is_eq(count, 0n), MSG.T.viewer.schematic.tip(lc), View.sch.pos(lc, out, at, count, page))}]}]

def View.sch.cls(+st: Nat) -> String:
  Bool.pick(String, Nat.is_eq(st, 2n), " on", Bool.pick(String, Nat.is_eq(st, 1n), " above", ""))

def View.sch.fold(kids: Bool, +lc: I.Loc, +open: Bool, +name: String, +place: String) -> D.Node:
  match kids:
    case True{}:
      D.El{"button", "", [D.At{"class", "sch-fold"}, D.At{"title", Bool.pick(String, open, MSG.T.viewer.sch.fold(lc, name), MSG.T.viewer.sch.unfold(lc, name))},
        D.At{"aria-label", Bool.pick(String, open, MSG.T.viewer.sch.fold(lc, name), MSG.T.viewer.sch.unfold(lc, name))}, D.At{"aria-expanded", Bool.pick(String, open, "true", "false")},
        D.At{"data-value", place}, D.On{"click", "sch-fold"}], [D.Txt{Bool.pick(String, open, "▾", "▸")}]}
    case False{}:
      D.El{"span", "", [D.At{"class", "sch-fold none"}, D.At{"aria-hidden", "true"}], Nil{}}

def View.sch.rows(+lc: I.Loc, rs: List<&2, U.SchRow>) -> List<&2, D.Node>:
  match rs:
    case Nil{}:
      Nil{}
    case U.SchRow{+i, +d, +n, +v, +st, kids, open, l} <> t:
      D.El{"div", "", [D.At{"class", "sch-row" ++ View.sch.cls(st)}, D.At{"style", "--d:" ++ Nat.show(d)}],
        [View.sch.fold(kids, lc, open, View.sch.label(lc, i, n), U.Sch.vkey(v)),
         D.El{"button", "", [D.At{"class", "sch-name"}, D.At{"title", HS.Hist.field(v, 1n)}, D.At{"aria-current", View.sch.current(st)}, D.At{"data-value", v}, D.On{"click", "sch-go"}],
           [D.Txt{MSG.T.viewer.sheet.title(lc, View.sch.label(lc, i, n), Bool.pick(String, l, "loop", ""))}]}]}
        <> View.sch.rows(lc, t)

def View.sch.here(+lc: I.Loc, rs: List<&2, U.SchRow>) -> List<&2, D.Node>:
  match rs:
    case Nil{}:
      Nil{}
    case U.SchRow{_, _, +n, +v, _, _, _, l} <> t:
      D.El{"button", "", [D.At{"class", "sch-kid"}, D.At{"title", HS.Hist.field(v, 1n)}, D.At{"data-value", v}, D.On{"click", "sch-go"}],
        [D.Txt{MSG.T.viewer.sheet.title(lc, n, Bool.pick(String, l, "loop", ""))}]} <> View.sch.here(lc, t)

def View.sch.say(+lc: I.Loc, +out: Bool, +note: String) -> List<&2, D.Node>:
  Bool.pick(List<&2, D.Node>, String.eq(note, "gone"), [D.El{"div", "", [D.At{"class", "sch-say"}, D.At{"role", "status"}], [D.Txt{MSG.T.viewer.sch.gone(lc)}]}],
    D.Dom.when(out, D.El{"div", "", [D.At{"class", "sch-say"}, D.At{"role", "status"}], [D.Txt{MSG.T.viewer.sch.outside(lc)}]}))

def View.sch.tree(+lc: I.Loc, on: Bool, rows: List<&2, U.SchRow>, +here: List<&2, U.SchRow>) -> List<&2, D.Node>:
  match on:
    case True{}:
      [D.El{"div", "sch-panel", [D.At{"class", "sch-panel"}, D.At{"id", "sch-tree"}],
        D.El{"div", "", [D.At{"class", "sch-tree"}, D.At{"aria-label", MSG.T.viewer.sch.tree(lc)}], View.sch.rows(lc, rows)}
        <> Nil{}}]
    case False{}:
      Nil{}

def View.sch.panel(+lc: I.Loc, v: U.SchView) -> List<&2, D.Node>:
  match v:
    case U.SchView{rows, _, here, _, +count, _, _, _, +out, tree, _, note, _}:
      List.append(&2, D.Node, Bool.pick(List<&2, D.Node>, Nat.is_eq(count, 0n), Nil{}, View.sch.say(lc, out, note)), View.sch.tree(lc, tree, rows, here))

def View.sch.below.k(many: Bool, +lc: I.Loc, +u: U.Ui) -> List<&2, D.Node>:
  match many:
    case True{}:
      D.El{"div", "", [D.At{"class", "sch-navigation"}], View.sch.bar(lc, U.Sch.view(u))}
        <> View.sch.panel(lc, U.Sch.view(u))
    case False{}:
      Nil{}

# what goes under the bar of a schematic with several sheets
def View.sch.below(+lc: I.Loc, +u: U.Ui) -> List<&2, D.Node>:
  View.sch.below.k(U.Sch.many(u), lc, u)

# nodes put right after the first (the viewer's bar)
def View.sch.after(nodes: List<&2, D.Node>, extra: List<&2, D.Node>) -> List<&2, D.Node>:
  match nodes:
    case Nil{}:
      extra
    case h <> t:
      h <> List.append(&2, D.Node, extra, t)

def View.sheets.k(many: Bool, +lc: I.Loc, +u: U.Ui) -> List<&2, D.Node>:
  match many:
    case True{}:
      View.sch.bar(lc, U.Sch.view(u))
    case False{}:
      [D.Dom.span("what", MSG.T.viewer.schematic.tip(lc))]

def View.sheets.loc(+lc: I.Loc, +u: U.Ui) -> List<&2, D.Node>:
  View.sheets.k(U.Sch.many(u), lc, u)

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

def View.layer.rows.loc(+lc: I.Loc, ls: List<&2, U32>, +k: U32, +off: U32, +f: LY.LayerFocus) -> List<&2, D.Node>:
  match ls:
    case Nil{}:
      Nil{}
    case +i <> t:
      View.layer.row.loc(lc, U32.is_gt(View.layer.alpha(LO.Look.of.t(False{}, k, i)), 0), i, LY.Lyr.name(k, i), View.layer.color(LO.Look.of.t(False{}, k, i)), off,
        Bool.and(LY.Lyr.on(f), U32.is_eq(i, LY.Lyr.active(f))), View.layer.rows.loc(lc, t, k, off, f))

def View.layer.rows(ls: List<&2, U32>, +k: U32, +off: U32, +f: LY.LayerFocus) -> List<&2, D.Node>:
  View.layer.rows.loc(I.En{}, ls, k, off, f)

def View.layer.mode(+m: U32, +cur: U32, +label: String, +tip: String) -> D.Node:
  D.El{"button", "", [D.At{"class", Bool.pick(String, U32.is_eq(m, cur), "lmode on", "lmode")}, D.At{"title", tip}, D.At{"data-value", U32.show(m)}, D.On{"click", "view-layer-mode"}], [D.Txt{label}]}

# KiCad's high contrast: the other layers as they are, dimmed, or hidden
def View.layer.modes.loc(+lc: I.Loc, sch: Bool, +f: LY.LayerFocus) -> List<&2, D.Node>:
  match sch:
    case True{}:
      Nil{}
    case False{}:
      +cur = {Bool.pick(U32, LY.Lyr.on(f), LY.Lyr.mode(f), 0) : U32}
      [D.El{"div", "", [D.At{"class", "layer-modes"}], [View.layer.mode(0, cur, MSG.T.viewer.layers.all(lc), MSG.T.viewer.layers.all.tip(lc)),
        View.layer.mode(1, cur, MSG.T.viewer.layers.dim(lc), MSG.T.viewer.layers.dim.tip(lc)),
        View.layer.mode(2, cur, MSG.T.viewer.layers.only(lc), MSG.T.viewer.layers.only.tip(lc))]}]

# the menu's button names the active layer while one stands out

def View.layer.modes(sch: Bool, +f: LY.LayerFocus) -> List<&2, D.Node>:
  View.layer.modes.loc(I.En{}, sch, f)

def View.layer.head.loc(+lc: I.Loc, +k: U32, +f: LY.LayerFocus) -> String:
  Bool.pick(String, LY.Lyr.on(f), MSG.T.web.viewer.layers.active(lc, LY.Lyr.name(k, LY.Lyr.active(f))), MSG.T.viewer.layers(lc))

# the look kind of a plot: a board or schematic, a comparison's marks too

def View.layer.head(+k: U32, +f: LY.LayerFocus) -> String:
  View.layer.head.loc(I.En{}, k, f)

def View.layer.kind(sch: Bool, cmp: Bool) -> U32:
  U32.or(Bool.pick(U32, sch, 1, 0), Bool.pick(U32, cmp, LO.Look.diff(), 0))

# the menu's button reads "Layers", or the active layer's name alone (lit)
# while one is singled out, its title saying both: a long head ("Layers ·
# User.Comments") made the viewer's bar wrap and the board jump
def View.layer.button.loc(+lc: I.Loc, +k: U32, +f: LY.LayerFocus) -> D.Node:
  Bool.pick(D.Node, LY.Lyr.on(f),
    D.El{"summary", "", [D.At{"class", "on"}, D.At{"title", View.layer.head.loc(lc, k, f)}], [D.Txt{LY.Lyr.name(k, LY.Lyr.active(f))}]},
    D.El{"summary", "", [D.At{"title", MSG.T.viewer.layers.tip(lc)}], [D.Txt{MSG.T.viewer.layers(lc)}]})

def View.layers.loc(+lc: I.Loc, +sch: Bool, +cmp: Bool, +off: U32, +f: LY.LayerFocus) -> D.Node:
  +k = View.layer.kind(sch, cmp)
  D.El{"details", "", [D.At{"class", "layers"}], [View.layer.button.loc(lc, k, f),
    D.El{"div", "", [D.At{"class", "layer-list"}], List.append(&2, D.Node, View.layer.modes.loc(lc, sch, f), View.layer.rows.loc(lc, LY.Lyr.order(k), k, off, f))}]}

# The design history bar (client.bend's Hs.bar): the arrows, play and
# the position, the changes, compare and live buttons; a scrubber with a
# tick a step; what the agent said before the step, and a way back to it.
# A comparison names its two sides (each opens the menu of versions) over
# a legend.

def View.layers(+sch: Bool, +cmp: Bool, +off: U32, +f: LY.LayerFocus) -> D.Node:
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

def View.hist.tick.named(+id: String, +i: Nat, +n: Nat, +cur: Bool, +title: String) -> D.Node:
  D.El{"button", "", [D.At{"class", Bool.pick(String, cur, "tick on", "tick")}, D.At{"title", title}, D.At{"aria-label", title}, D.At{"aria-current", Bool.pick(String, cur, "step", "false")},
    D.At{"style", "left:calc(100% * " ++ Nat.show(i) ++ " / " ++ Nat.show(Bool.pick(Nat, Nat.is_eq(n, 0n), 1n, n)) ++ ");width:min(10px,calc(100% / " ++ Nat.show(Bool.pick(Nat, Nat.is_eq(n, 0n), 1n, n)) ++ "))"}, D.At{"data-value", id}, D.On{"click", "hist-go"}], Nil{}}

def View.hist.tick.loc(+lc: I.Loc, +id: String, +i: Nat, +n: Nat, +cur: Bool) -> D.Node:
  View.hist.tick.named(id, i, n, cur, Bool.pick(String, String.is_empty(id), MSG.T.history.file.tip(lc), MSG.T.history.step.label(lc, Nat.show(1n+i))))

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

# The timestamp is cheap per tick; source labels are read only for the
# selected step and the compare menu, never once per tick over the chat.
def View.hist.ticks.when(+lc: I.Loc, ss: List<&2, HS.Step>, +i: Nat, +n: Nat, +at: Nat, +zone: TZ.TzRule) -> List<&2, D.Node>:
  match ss:
    case Nil{}:
      [View.hist.tick.loc(lc, "", i, n, Nat.is_eq(at, 0n))]
    case +s <> t:
      View.hist.tick.named(HS.Step.id(s), i, n, Nat.is_eq(at, 1n+i), MSG.T.history.step.label(lc, Nat.show(1n+i)) ++ " · " ++ Bool.pick(String, Nat.is_eq(HS.Step.at(s), 0n), MSG.T.history.time.unknown(lc), U.Hs.when.zone(zone, HS.Step.at(s))))
        <> View.hist.ticks.when(lc, t, 1n+i, n, at, zone)

# Compact scrubbing keeps the first, selected and live endpoints at their
# true positions; the ordinary first/previous/next/last/jump controls remain.
def View.hist.ticks.compact(+lc: I.Loc, ss: List<&2, HS.Step>, +i: Nat, +n: Nat, +at: Nat) -> List<&2, D.Node>:
  match ss:
    case Nil{}: [View.hist.tick.loc(lc, "", i, n, Nat.is_eq(at, 0n))]
    case +s <> rest:
      List.append(&2, D.Node, D.Dom.when(Bool.or(Nat.is_eq(i, 0n), Nat.is_eq(at, 1n+i)), View.hist.tick.loc(lc, HS.Step.id(s), i, n, Nat.is_eq(at, 1n+i))), View.hist.ticks.compact(lc, rest, 1n+i, n, at))

def View.hist.ticks.for(compact: Bool, +lc: I.Loc, ss: List<&2, HS.Step>, n: Nat, at: Nat, u: U.Ui) -> List<&2, D.Node>:
  match compact:
    case False{}: View.hist.ticks.when(lc, ss, 0n, n, at, SCH.Sched.person(U.Ui.state(u), 0n, True{}))
    case True{}: View.hist.ticks.compact(lc, ss, 0n, n, at)

def View.hist.said.loc(+lc: I.Loc, live: Bool, +text: String) -> List<&2, D.Node>:
  match live:
    case True{}:
      [D.El{"span", "", [D.At{"class", "said"}, D.At{"title", text}], [D.Txt{text}]}]
    case False{}:
      [WI.WIcon.button("icon", "hist-jump", "", MSG.T.history.thread.point(lc), "message-square", 16, ""),
       D.El{"button", "", [D.At{"class", "said"}, D.At{"title", text ++ "\n" ++ MSG.T.history.thread.point(lc)}, D.At{"data-value", ""}, D.On{"click", "hist-jump"}], [D.Txt{text}]}]

def View.hist.said(live: Bool, +text: String) -> List<&2, D.Node>:
  View.hist.said.loc(I.En{}, live, text)

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

def View.hist.disabled(on: Bool) -> List<&2, D.Attr>:
  match on:
    case True{}: [D.At{"disabled", "disabled"}]
    case False{}: Nil{}

def View.hist.changes(+lc: I.Loc, +on: Bool, n: Nat) -> D.Node:
  D.El{"button", "", List.append(&2, D.Attr, [D.At{"class", Bool.pick(String, on, "hbtn on", "hbtn")}, D.At{"title", MSG.T.history.changes.tip(lc)},
    D.At{"aria-pressed", Bool.pick(String, on, "true", "false")}, D.At{"data-value", ""}, D.On{"click", "hist-diff"}], View.hist.disabled(Nat.is_eq(n, 0n))), [D.Txt{MSG.T.viewer.history.changes(lc)}]}

def View.hist.walk(+lc: I.Loc, b: U.HBar, +tr: List<&2, HS.Step>, +u: U.Ui) -> D.Node:
  match b:
    case U.HBar{_, +n, +at, +title, +entry, +label, +said, +pl, +df, _, _, _, _}:
      D.El{"div", "", [D.At{"class", "hist"}], [
        D.El{"div", "", [D.At{"class", "hist-row"}], List.append(&2, D.Node, [
          View.hist.icon("icon", "hist-first", MSG.T.history.first(lc), "skip-back"),
          View.hist.icon("icon", "hist-prev", MSG.T.history.previous(lc), "chevron-left"),
          View.hist.icon("icon play", "hist-play", Bool.pick(String, pl, MSG.T.history.pause(lc), MSG.T.history.play(lc)), Bool.pick(String, pl, "pause", "play")),
          View.hist.icon("icon", "hist-next", MSG.T.history.next(lc), "chevron-right"),
          View.hist.icon("icon", "hist-last", MSG.T.history.last(lc), "skip-forward"),
          D.Dom.span("title", title), D.Dom.span("grow", ""),
          View.hist.changes(lc, Bool.and(df, Nat.is_gt(at, 0n)), n),
          D.Dom.button("hbtn", "cmp", "a", MSG.T.history.compare.tip(lc), MSG.T.viewer.history.compare(lc))], View.hist.live.loc(lc, at))},
        D.El{"div", "", [D.At{"class", "hist-scrub"}], View.hist.ticks.for(View.compact(u), lc, tr, n, at, u)},
        D.El{"div", "", [D.At{"class", "hist-said"}], View.hist.said.loc(lc, Bool.or(Nat.is_eq(at, 0n), Bool.not(U.Hs.jump.available(u, entry))), Bool.pick(String, Nat.is_eq(at, 0n), U.Hs.live.said.loc(lc, n), Bool.pick(String, String.is_empty(said), label, said)))},
        D.El{"div", "", [D.At{"class", "hist-legend"}], View.hist.legend.loc(lc, Bool.and(df, Nat.is_gt(at, 0n)))}]}

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

def View.hist.k(+lc: I.Loc, on: Bool, cmp: Bool, b: U.HBar, +tr: List<&2, HS.Step>, +u: U.Ui) -> List<&2, D.Node>:
  match on cmp:
    case False{} _:
      Nil{}
    case True{} True{}:
      [View.hist.cmp.loc(lc, b, u)]
    case True{} False{}:
      [View.hist.walk(lc, b, tr, u)]

def View.hist.t(+lc: I.Loc, +b: U.HBar, +tr: List<&2, HS.Step>, +u: U.Ui) -> List<&2, D.Node>:
  View.hist.k(lc, U.HBar.on(b), U.HBar.cmp(b), b, tr, u)

# (the bar and its track made once: each U.Hs.track parses every step)
def View.hist.bt(+lc: I.Loc, bt: U.HBarT, +u: U.Ui) -> List<&2, D.Node>:
  match bt:
    case U.HBarT{+b, +tr}: View.hist.t(lc, b, tr, u)

def View.hist(+lc: I.Loc, +b: U.HBar, +u: U.Ui) -> List<&2, D.Node>:
  View.hist.t(lc, b, U.Hs.track(u), u)

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

# Cross-probing (client.bend's Probe, the web's probe.js): the probed part
# or net as a chip over the plot (in the combined view's bar), its name
# bringing it into view and its x clearing it (Esc too); a part on another
# sheet is said with its sheet over the schematic, which a click opens.
# Each plot's canvas carries the probe, its side, its sheet's place and its
# board's source.

def View.probe.chip.k(none: Bool, +v: String) -> List<&2, D.Node>:
  match none:
    case True{}:
      Nil{}
    case False{}:
      [D.El{"span", "probe-chip", [D.At{"class", "probe-chip"}, D.At{"role", "group"}, D.At{"aria-label", "Probed"}], [
        D.El{"button", "", [D.At{"type", "button"}, D.At{"class", "probe-name"}, D.At{"data-view", "probe"}, D.At{"title", "Bring it into view"}], [D.Txt{U.Probe.label(v)}]},
        D.El{"button", "", [D.At{"type", "button"}, D.At{"class", "icon probe-clear"}, D.At{"title", "Clear (Esc)"}, D.At{"aria-label", "Clear the probe"},
          D.At{"data-value", ""}, D.On{"click", "probe"}], [WI.WIcon.svg("x", 14)]}]}]

def View.probe.chip(+u: U.Ui) -> List<&2, D.Node>:
  View.probe.chip.k(String.is_empty(U.Probe.get(u)), U.Probe.get(u))

def View.probe.away.k(a: U.ProbeAway) -> List<&2, D.Node>:
  match a:
    case U.ProbeAway{+ref, +sheet, +value}:
      D.Dom.when(Bool.not(String.is_empty(value)), D.El{"div", "probe-away", [D.At{"class", "probe-away"}, D.At{"role", "status"}], [
        D.Txt{ref ++ " is on the sheet " ++ sheet}, D.El{"button", "", [D.At{"type", "button"}, D.At{"data-value", value}, D.On{"click", "sch-go"}], [D.Txt{"Open it"}]}]})

def View.probe.away.if(on: Bool, +u: U.Ui) -> List<&2, D.Node>:
  match on:
    case True{}:
      View.probe.away.k(U.Probe.note(u))
    case False{}:
      Nil{}

# (only a probed part, on a schematic of several sheets, is looked for)
def View.probe.away(+u: U.Ui) -> List<&2, D.Node>:
  View.probe.away.if(Bool.and(Bool.not(String.is_empty(U.Probe.ref(U.Probe.get(u)))), U.Sch.many(u)), u)

def View.probe.place.k(v: U.SchView) -> String:
  match v:
    case U.SchView{_, _, _, _, _, +place, _, _, _, _, _, _, _}:
      Bool.pick(String, String.is_empty(place), "/", place)

def View.probe.place(many: Bool, +u: U.Ui) -> String:
  match many:
    case True{}:
      View.probe.place.k(U.Sch.view(u))
    case False{}:
      "/"

# what a plot's canvas tells probe.js: the probe, which side it is, the
# sheet's place, and the board a schematic's nets are found on
def View.probe.attrs.k(sch: Bool, +u: U.Ui) -> List<&2, D.Attr>:
  match sch:
    case True{}:
      [D.At{"data-probe", U.Probe.get(u)}, D.At{"data-side", "schematic"}, D.At{"data-place", View.probe.place(U.Sch.many(u), u)}, D.At{"data-board", U.Probe.board.src(u)}]
    case False{}:
      [D.At{"data-probe", U.Probe.get(u)}, D.At{"data-side", "board"}]

def View.probe.attrs(+u: U.Ui, sch: Bool) -> List<&2, D.Attr>:
  View.probe.attrs.k(sch, u)

# a board or schematic: host.js (plot2d.js) draws the hub's plot of it on
# the canvas whose data-key it carries, less the layers data-off names

def View.rc.checks(+u: U.Ui) -> List<&2, D.Node>:
  View.rc.checks.loc(I.En{}, u)

# The 2D bar keeps its Layers button steady and uses the local menu keys.
# Other viewers keep their own disclosure and active-layer label.
def View.plot.layer.button(+lc: I.Loc, +k: U32, +f: LY.LayerFocus) -> D.Node:
  D.El{"summary", "", [D.At{"class", Bool.pick(String, LY.Lyr.on(f), "on", "")},
    D.At{"title", Bool.pick(String, LY.Lyr.on(f), View.layer.head.loc(lc, k, f), MSG.T.viewer.layers.tip(lc))}], [D.Txt{MSG.T.viewer.layers(lc)}]}

def View.plot.layers(+lc: I.Loc, +sch: Bool, +cmp: Bool, +off: U32, +f: LY.LayerFocus) -> D.Node:
  +k = View.layer.kind(sch, cmp)
  D.El{"details", "", [D.At{"class", "layers local-menu"}], [View.plot.layer.button(lc, k, f),
    D.El{"div", "", [D.At{"class", "layer-list"}], List.append(&2, D.Node, View.layer.modes.loc(lc, sch, f), View.layer.rows.loc(lc, LY.Lyr.order(k), k, off, f))}]}

# Wait for the hierarchy before the first drawing, so its controls cannot
# move a diagram that is already on screen. Boards do not need this reply.
def View.plot.wait.tree(sch: Bool, +u: U.Ui) -> String:
  match sch:
    case True{}:
      Bool.pick(String, List.is_empty(&2, HI.Row, U.Sch.rows(u)), "1", "")
    case False{}:
      ""

# the board's or schematic's help (its "?"): the mouse's and keys' ways,
# a phone's touch ones
def View.plot.hint(phone: Bool, +lc: I.Loc, +u: U.Ui) -> String:
  match phone:
    case True{}: MSG.T.viewer.pan.touch(lc)
    case False{}:
      MSG.T.viewer.pan.tip(lc) ++ Bool.pick(String, String.eq(U.Solid.kind(u), "schematic"),
        " · Alt+←/→: " ++ MSG.T.viewer.sch.back(lc) ++ "/" ++ MSG.T.viewer.sch.forward(lc) ++ " · Alt+↑/Backspace: " ++ MSG.T.viewer.sch.up(lc), "")

def View.plot(+lc: I.Loc, +u: U.Ui, +what: List<&2, D.Node>) -> List<&2, D.Node>:
  View.bar.loc(lc, List.append(&2, D.Node, what, [View.plot.layers(lc, String.eq(U.Solid.kind(u), "schematic"), Nat.is_eq(HS.Hist.kind(U.Ui.view_get(u, "view.path")), 2n), U.Solid.off(u), U.Lfocus.of(u))]), View.plot.hint(PHN.Phone.on(u), lc, u))
   <> D.El{"div", "", [D.At{"class", "viewer-stage"}], D.El{"canvas", "plot", List.append(&2, D.Attr, [D.At{"id", "plot"}, D.At{"class", "plot2d"}, D.At{"tabindex", "0"}, D.At{"aria-label", Bool.pick(String, String.eq(U.Solid.kind(u), "schematic"), MSG.T.viewer.schematic.tip(lc), MSG.T.viewer.board.tip(lc))}, D.At{"data-key", U.Solid.want(u)},
        D.At{"data-scope", U.Ui.sel(u)}, D.At{"data-off", U32.show(LY.Masks.off(LY.Lyr.masks(U.Lfocus.kind(u), U.Solid.off(u), U.Lfocus.of(u))))},
        D.At{"data-dim", U32.show(LY.Masks.dim(LY.Lyr.masks(U.Lfocus.kind(u), U.Solid.off(u), U.Lfocus.of(u))))}, D.At{"data-lift", U32.show(LY.Lyr.top(U.Lfocus.of(u)))},
        D.At{"data-wait-tree", View.plot.wait.tree(String.eq(U.Solid.kind(u), "schematic"), u)}, D.At{"data-fade", Nat.show(U.Hs.fade())}, D.At{"data-fam", U.Solid.fam(u, U.Solid.want(u))}, D.At{"data-sheets", U.Sch.boxes(u)}], View.probe.attrs(u, String.eq(U.Solid.kind(u), "schematic"))), Nil{}}
        <> List.append(&2, D.Node, View.probe.chip(u), View.hist.menu(lc, u) <> View.rc.loc(lc, u))}
   <> View.hist.bt(lc, U.Hs.bar.track(lc, u), u)

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

def View.solid.parts(compact: Bool, +lc: I.Loc, +u: U.Ui) -> List<&2, D.Node>:
  match compact:
    case False{}: List.append(&2, D.Node, View.mech.chips.named(u, U.Mpage.parts(u), U.Mpage.shown(u)), View.solid.more.loc(lc, U.Mpage.listed(u), u))
    case True{}: List.append(&2, D.Node, [D.Dom.span("what", MC.Mech.last(U.Ui.view_get(u, "view.path")))], [D.Dom.button("", "mech", "", "Browse mechanical parts", "Parts")])

def View.solid.what.loc(+lc: I.Loc, on: Bool, +u: U.Ui) -> List<&2, D.Node>:
  match on:
    case True{}:
      View.solid.parts(View.compact(u), lc, u)
    case False{}:
      View.boards(u, MSG.T.viewer.board3d.tip(lc))

# a model turned in 3D: host.js (solid.js) draws the hub's model (a "plot"
# with a mesh, as the phones get) on the canvas whose data-key it carries,
# and says in the note why there is none

def View.solid.what(on: Bool, +u: U.Ui) -> List<&2, D.Node>:
  View.solid.what.loc(I.En{}, on, u)

def View.solid.boardkey(+u: U.Ui) -> String:
  Bool.pick(String, Bool.or(String.is_empty(U.Ui.view_get(u, "view.path")), String.ends_with(U.Ui.view_get(u, "view.path"), ".kicad_pcb")),
    "0|" ++ U.Ui.root(u) ++ "|" ++ U.Ui.view_get(u, "view.path"), "")

# One original-model renderer for standalone, project-home and Split views.
def View.solid.workbench(+lc: I.Loc, +u: U.Ui, +key: String, +boardkey: String) -> D.Node:
  D.El{"div", "solid-workbench", [D.At{"id", "solid-workbench"}, D.At{"class", "solid-workbench"}, D.At{"data-key", key}, D.At{"data-scope", U.Ui.sel(u)}, D.At{"data-board-key", boardkey}],
        [D.El{"div", "solid-toolbar", [D.At{"class", "solid-toolbar"}], Nil{}},
         D.El{"div", "", [D.At{"class", "solid-layout"}],
          [D.El{"div", "", [D.At{"class", "viewer-stage"}], List.append(&2, D.Node, [D.El{"canvas", "solid", [D.At{"id", "solid"}, D.At{"data-key", key}, D.At{"data-probe", U.Probe.get(u)}, D.At{"data-scope", U.Ui.sel(u)}, D.At{"tabindex", "0"}, D.At{"aria-label", "3D model. Drag or arrow keys to orbit; F fits the model."}], Nil{}},
            D.El{"div", "", [D.At{"class", "viewer-note"}, D.At{"role", "status"}], Nil{}}, D.El{"div", "", [D.At{"class", "viewer-stat"}], Nil{}}], View.probe.chip(u))},
           D.El{"section", "solid-pcb", [D.At{"class", "solid-pcb viewer-stage"}, D.At{"aria-label", "PCB beside assembly"}],
             [D.El{"div", "solid-pcb-controls", [D.At{"class", "solid-pcb-controls"}, D.At{"aria-label", "PCB layer controls"}], [View.layers.loc(lc, False{}, False{}, U.Solid.off(u), U.Lfocus.of(u))]},
              D.El{"canvas", "solid-pcb-plot", List.append(&2, D.Attr, View.probe.attrs(u, False{}), [D.At{"id", "plot"}, D.At{"class", "plot2d"}, D.At{"data-key", boardkey}, D.At{"data-off", U32.show(LY.Masks.off(LY.Lyr.masks(0, U.Solid.off(u), U.Lfocus.of(u))))},
                D.At{"data-scope", U.Ui.sel(u)}, D.At{"data-dim", U32.show(LY.Masks.dim(LY.Lyr.masks(0, U.Solid.off(u), U.Lfocus.of(u))))}, D.At{"data-lift", U32.show(LY.Lyr.top(U.Lfocus.of(u)))},
                D.At{"data-fade", Nat.show(U.Hs.fade())}, D.At{"tabindex", "0"}, D.At{"aria-label", "Saved PCB. Drag to pan, wheel or pinch to zoom."}]), Nil{}},
              D.El{"div", "", [D.At{"class", "viewer-note"}, D.At{"role", "status"}], Nil{}}, D.El{"div", "", [D.At{"class", "viewer-stat"}], Nil{}}]},
           D.El{"aside", "solid-inspector", [D.At{"id", "solid-inspector"}, D.At{"class", "solid-inspector"}], Nil{}}]}]}

def View.solid.loc(+lc: I.Loc, +u: U.Ui) -> List<&2, D.Node>:
  View.bar.loc(lc, View.solid.what.loc(lc, U.Mpage.onpart(u), u), Bool.pick(String, PHN.Phone.on(u), MSG.T.viewer.orbit.touch(lc), MSG.T.viewer.orbit.tip(lc)))
    <> List.append(&2, D.Node, D.Dom.when(Bool.not(String.is_empty(U.Ui.info(u, "plotnote:" ++ U.Solid.want(u)))), D.Dom.span("viewer-say", View.compact.text(View.compact(u), U.Ui.info(u, "plotnote:" ++ U.Solid.want(u))))),
      [View.solid.workbench(lc, u, U.Solid.want(u), View.solid.boardkey(u))])

# The combined view (client.bend's Split): the schematic beside the board
# and its 3D model (on a phone one at a time: the bar's Board and 3D swap them), each
# pane under a head that names it, with a button that puts it away (the
# bar's toggles bring it back). The panes draw as on their own tabs
# (plot2d.js, solid.js), with the layers those show; the schematic's head
# navigates its sheets; the edge between the two panes resizes them
# (project-splitters.js).

def View.combo.hide(+pane: String, +name: String) -> D.Node:
  D.El{"button", "", [D.At{"type", "button"}, D.At{"class", "icon combo-hide"}, D.At{"title", "Hide the " ++ name}, D.At{"aria-label", "Hide the " ++ name},
    D.At{"data-value", pane}, D.On{"click", "split-pane"}], [WI.WIcon.svg("x", 14)]}

def View.combo.head(+pane: String, +name: String, more: List<&2, D.Node>) -> D.Node:
  D.El{"div", "", [D.At{"class", "combo-head"}], List.append(&2, D.Node, D.Dom.span("combo-name", name) <> more, [View.combo.hide(pane, name)])}

# (on a phone the board and the 3D take each other's place)
def View.combo.toggle.tip(+u: U.Ui, +pane: String, +name: String) -> String:
  Bool.pick(String, U.Split.on(u, pane), "Hide the " ++ name,
    Bool.pick(String, Bool.and(U.Split.one(u), Bool.and(String.eq(pane, "3d"), U.Split.on(u, "pcb"))), "Show the 3D in place of the board",
      Bool.pick(String, Bool.and(U.Split.one(u), Bool.and(String.eq(pane, "pcb"), U.Split.on(u, "3d"))), "Show the board in place of the 3D", "Show the " ++ name)))

def View.combo.toggle(+u: U.Ui, +pane: String, +name: String) -> D.Node:
  D.El{"button", "", [D.At{"type", "button"}, D.At{"class", Bool.pick(String, U.Split.on(u, pane), "combo-pick on", "combo-pick")},
    D.At{"aria-pressed", Bool.pick(String, U.Split.on(u, pane), "true", "false")}, D.At{"title", View.combo.toggle.tip(u, pane, name)},
    D.At{"data-value", pane}, D.On{"click", "split-pane"}], [D.Txt{name}]}

def View.combo.separator(+group: String, +first: String, +second: String, controls: String) -> D.Node:
  D.El{"div", "combo-resize-" ++ group, [D.At{"id", "combo-resize-" ++ group}, D.At{"class", "combo-resize"},
    D.At{"data-project-split", group}, D.At{"data-first-label", first}, D.At{"data-second-label", second},
    D.At{"role", "separator"}, D.At{"tabindex", "0"}, D.At{"aria-controls", controls},
    D.At{"aria-label", "Resize " ++ first ++ " and " ++ second},
    D.At{"title", "Drag or use arrow keys to resize. Shift moves faster; Home/End go to the limits; Enter or double-click resets."}], Nil{}}

# a plot pane's canvas: its source, and the layers its kind shows (k 0 a
# board, 1 a schematic, as the single tabs keep them)
def View.combo.canvas(+lc: I.Loc, +id: String, +key: String, +kind: String, +k: U32, +u: U.Ui, +sheets: String) -> D.Node:
  D.El{"canvas", id, List.append(&2, D.Attr, View.probe.attrs(u, U32.is_eq(k, 1)), [D.At{"id", id}, D.At{"class", "plot2d"}, D.At{"tabindex", "0"}, D.At{"aria-label", Bool.pick(String, U32.is_eq(k, 1), MSG.T.viewer.schematic.tip(lc), MSG.T.viewer.board.tip(lc))}, D.At{"data-key", key},
    D.At{"data-scope", U.Ui.sel(u)}, D.At{"data-off", U32.show(LY.Masks.off(LY.Lyr.masks(k, U.Solid.off.of(u, kind), U.Lfocus.of.at(u, kind))))},
    D.At{"data-dim", U32.show(LY.Masks.dim(LY.Lyr.masks(k, U.Solid.off.of(u, kind), U.Lfocus.of.at(u, kind))))}, D.At{"data-lift", U32.show(LY.Lyr.top(U.Lfocus.of.at(u, kind)))},
    D.At{"data-fade", Nat.show(U.Hs.fade())}, D.At{"data-fam", HS.Hist.key.family(key)}, D.At{"data-sheets", sheets}]), Nil{}}

def View.combo.trail(+lc: I.Loc, v: U.SchView) -> List<&2, D.Node>:
  match v:
    case U.SchView{rows, trail, _, +at, _, _, _, _, _, _, _, _, _}:
      [View.sch.btn("sch-up", Bool.not(Nat.is_eq(at, 0n)), MSG.T.viewer.sch.up.tip(lc), "↑"),
       D.El{"nav", "", [D.At{"class", "sch-trail"}, D.At{"aria-label", MSG.T.viewer.sch.trail(lc)}], View.sch.crumbs(lc, trail)},
       D.El{"details", "combo-sheets", [D.At{"class", "local-menu combo-sheets"}], [
         D.El{"summary", "", [D.At{"title", MSG.T.viewer.sch.tree.tip(lc)}], [D.Txt{MSG.T.viewer.sch.tree(lc)}]},
         D.El{"div", "", [D.At{"class", "local-menu-body"}], [D.El{"div", "", [D.At{"class", "sch-tree"}], View.sch.rows(lc, rows)}]}]}]

def View.combo.nav(many: Bool, +lc: I.Loc, +u: U.Ui) -> List<&2, D.Node>:
  match many:
    case True{}:
      View.combo.trail(lc, U.Sch.view(u))
    case False{}:
      Nil{}

def View.combo.boxes(many: Bool, +u: U.Ui) -> String:
  match many:
    case True{}:
      U.Sch.boxes.v(U.Sch.view(u))
    case False{}:
      ""

def View.combo.sch(+lc: I.Loc, +u: U.Ui) -> D.Node:
  D.El{"section", "combo-sch", [D.At{"id", "combo-sch"}, D.At{"class", "combo-pane combo-main"}, D.At{"aria-label", MSG.T.viewer.schematic(lc)}],
    View.combo.head("sch", MSG.T.viewer.schematic(lc), View.combo.nav(U.Sch.many(u), lc, u)) <> List.append(&2, D.Node, View.probe.away(u),
    [D.El{"div", "", [D.At{"class", "viewer-stage"}], [View.combo.canvas(lc, "plot-sch", U.Split.src.sch(u), "schematic", 1, u, View.combo.boxes(U.Sch.many(u), u))]}])}

def View.combo.pcb(+lc: I.Loc, +u: U.Ui) -> D.Node:
  D.El{"section", "combo-pcb", [D.At{"id", "combo-pcb"}, D.At{"class", "combo-pane"}, D.At{"aria-label", MSG.T.viewer.board(lc)}], [
    View.combo.head("pcb", MSG.T.viewer.board(lc), Nil{}),
    D.El{"div", "", [D.At{"class", "viewer-stage"}], [View.combo.canvas(lc, "plot-pcb", U.Split.src.pcb(u), "board", 0, u, "")]}]}

def View.combo.solid(+lc: I.Loc, +u: U.Ui) -> D.Node:
  D.El{"section", "combo-3d", [D.At{"id", "combo-3d"}, D.At{"class", "combo-pane"}, D.At{"aria-label", "3D"}], [
    View.combo.head("3d", "3D", Nil{}),
    View.solid.workbench(lc, u, U.Split.src.solid(u), U.Split.src.pcb(u))]}

def View.combo.pane(i: Nat, +lc: I.Loc, +u: U.Ui) -> D.Node:
  match i:
    case 0n:
      View.combo.sch(lc, u)
    case 1n:
      View.combo.pcb(lc, u)
    case _:
      View.combo.solid(lc, u)

# a pane shown, or nothing (matched, so a pane put away is never laid out)
def View.combo.if(on: Bool, +lc: I.Loc, +u: U.Ui, +i: Nat) -> List<&2, D.Node>:
  match on:
    case True{}:
      [View.combo.pane(i, lc, u)]
    case False{}:
      Nil{}

def View.combo.side.label(+lc: I.Loc, +u: U.Ui) -> String:
  Bool.pick(String, U.Split.on(u, "pcb"), Bool.pick(String, U.Split.on(u, "3d"), MSG.T.viewer.board(lc) ++ " and 3D", MSG.T.viewer.board(lc)), "3D")

def View.combo(+lc: I.Loc, +u: U.Ui) -> List<&2, D.Node>:
  [View.bar.loc(lc, List.append(&2, D.Node, View.boards(u, "Split"), D.El{"span", "", [D.At{"class", "combo-picks"}, D.At{"role", "group"}, D.At{"aria-label", "Panes"}], [
      View.combo.toggle(u, "sch", MSG.T.viewer.schematic(lc)), View.combo.toggle(u, "pcb", MSG.T.viewer.board(lc)), View.combo.toggle(u, "3d", "3D")]} <> View.probe.chip(u)),
     MSG.T.viewer.pan.tip(lc)),
   D.El{"div", "combo", [D.At{"class", "combo"}, D.At{"data-split-project", U.Asset.project(u)}], [D.El{"div", "", [D.At{"class", "combo-in"}],
     List.concat(&2, D.Node, [View.combo.if(U.Split.on(u, "sch"), lc, u, 0n),
       D.Dom.when(Bool.and(U.Split.on(u, "sch"), Bool.or(U.Split.on(u, "pcb"), U.Split.on(u, "3d"))), View.combo.separator("primary", MSG.T.viewer.schematic(lc), View.combo.side.label(lc, u), "combo-sch combo-side")),
       [D.El{"div", "combo-side", [D.At{"id", "combo-side"}, D.At{"class", "combo-side"}], List.concat(&2, D.Node, [View.combo.if(U.Split.on(u, "pcb"), lc, u, 1n),
         D.Dom.when(Bool.and(U.Split.on(u, "pcb"), U.Split.on(u, "3d")), View.combo.separator("secondary", MSG.T.viewer.board(lc), "3D", "combo-pcb combo-3d")),
         View.combo.if(U.Split.on(u, "3d"), lc, u, 2n)])}]])}]}]

# The viewer pane: a tab for each thing it shows (the one on show lit),
# Close, then that thing

def View.solid(+u: U.Ui) -> List<&2, D.Node>:
  View.solid.loc(I.En{}, u)

# The Files tab (client.bend's Fm): where the folder is, then its folders
# and files; a file the viewer can open is a button, any other is listed


def View.fm.trail.loc(+lc: I.Loc, ps: List<&2, U.PItem>) -> List<&2, D.Node>:
  match ps:
    case Nil{}:
      Nil{}
    case U.PItem{+l, +a, +v} <> t:
      D.Dom.button("crumb", a, v, MSG.T.proj.goto(lc, l), l) <> View.fm.trail.loc(lc, t)

def View.fm.trail(ps: List<&2, U.PItem>) -> List<&2, D.Node>:
  View.fm.trail.loc(I.En{}, ps)

# a download (client.bend's Fm.down: none when url is ""): a plain link
# the browser saves, with the page's own sign-in; the hub's answer names
# the file (core/fm.bend's Fm.disposition)
def View.fm.dl.k(none: Bool, +cls: String, +url: String, +tip: String, +icon: String, label: List<&2, D.Node>) -> List<&2, D.Node>:
  match none:
    case True{}:
      Nil{}
    case False{}:
      [D.El{"a", "", [D.At{"class", cls}, D.At{"href", url}, D.At{"download", ""}, D.At{"title", tip}, D.At{"aria-label", tip}], WI.WIcon.svg(icon, 16) <> label}]

def View.fm.dl(+cls: String, +url: String, +tip: String, +icon: String, label: List<&2, D.Node>) -> List<&2, D.Node>:
  View.fm.dl.k(String.is_empty(url), cls, url, tip, icon, label)

def View.fm.row.dl(+lc: I.Loc, +u: U.Ui, +folder: Bool, +l: String, +v: String) -> List<&2, D.Node>:
  View.fm.dl("fm-dl", U.Fm.down(u, folder, v), Bool.pick(String, folder, MSG.T.files.download.folder(lc, l), MSG.T.files.download.file(lc, l)), Bool.pick(String, folder, "folder-down", "download"), Nil{})

def View.fm.row.loc(+lc: I.Loc, +u: U.Ui, r: U.FmRow) -> D.Node:
  match r:
    case U.FmRow{+l, +a, +v, +d, +k}:
      D.El{"div", "", [D.At{"class", "fm-item"}], D.El{"button", "", List.append(&2, D.Attr, [D.At{"class", "fm-row " ++ Bool.pick(String, String.is_empty(a), "off", k)}, D.At{"title", Bool.pick(String, String.is_empty(a), MSG.T.files.preview.none(lc, l), l)}],
          Bool.pick(List<&2, D.Attr>, String.is_empty(a), [D.At{"disabled", "disabled"}], [D.At{"data-value", v}, D.On{"click", a}])),
        [WI.WIcon.svg(U.Fm.icon(k), 16), D.Dom.span("fm-name", l), D.Dom.span("fm-size", d)]} <> View.fm.row.dl(lc, u, String.eq(k, "folder"), l, v)}

def View.fm.row(+u: U.Ui, r: U.FmRow) -> D.Node:
  View.fm.row.loc(I.En{}, u, r)

def View.fm.rows.loc(+lc: I.Loc, +u: U.Ui, rs: List<&2, U.FmRow>) -> List<&2, D.Node>:
  match rs:
    case Nil{}:
      Nil{}
    case r <> t:
      View.fm.row.loc(lc, u, r) <> View.fm.rows.loc(lc, u, t)

def View.fm.rows(+u: U.Ui, rs: List<&2, U.FmRow>) -> List<&2, D.Node>:
  View.fm.rows.loc(I.En{}, u, rs)

def View.fm.body.loc(+lc: I.Loc, ready: Bool, +u: U.Ui) -> List<&2, D.Node>:
  match ready:
    case True{}:
      Bool.pick(List<&2, D.Node>, String.is_empty(U.Ui.info(u, "fm.err:" ++ U.Ui.sel(u))),
        Bool.pick(List<&2, D.Node>, List.is_empty(&2, U.FmRow, U.Fm.rows(u)), [D.Dom.span("fm-note", MSG.T.viewer.files.empty(lc))], View.fm.rows.loc(lc, u, U.Fm.rows(u))),
        [D.Dom.span("fm-note", U.Ui.info(u, "fm.err:" ++ U.Ui.sel(u)))])
    case False{}:
      [D.Dom.span("fm-note", MSG.T.viewer.files.loading(lc))]

def View.fm.body(ready: Bool, +u: U.Ui) -> List<&2, D.Node>:
  View.fm.body.loc(I.En{}, ready, u)

def View.files(+lc: I.Loc, +u: U.Ui) -> List<&2, D.Node>:
  [D.El{"div", "", [D.At{"class", "fm-bar"}], List.append(&2, D.Node, View.fm.trail.loc(lc, U.Fm.trail.loc(lc, u)),
     D.El{"span", "", [D.At{"class", "grow"}], Nil{}} <> List.append(&2, D.Node, View.fm.dl("btn fm-dl-bar", U.Fm.down(u, True{}, U.Fm.dir(u)), MSG.T.files.download.tip(lc), "folder-down", [D.Txt{MSG.T.files.download(lc)}]),
       [D.Dom.button("", "fm-up", "", MSG.T.files.up.tip(lc), MSG.T.queue.up(lc))]))},
   D.El{"div", "files-list", [D.At{"class", "fm-list"}], View.fm.body.loc(lc, U.Fm.shown(u), u)}]

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

# Text is loaded as textContent by the host, never interpreted as markup.
def View.text.pre.loc(+lc: I.Loc, +u: U.Ui) -> D.Node:
  D.El{"pre", "file-text:" ++ U.Ui.view_get(u, "view.path"), [D.At{"class", "fm-text"}, D.At{"data-text-src", IM.Img.url(U.Ui.view_get(u, "view.path"))}, D.At{"data-loading", MSG.T.viewer.loading(lc)}, D.At{"data-error", MSG.T.viewer.unread(lc)}], Nil{}}

# A Markdown note is drawn as the chat draws a message: the host reads the
# file as text and makes its nodes with app.bend's md (Md.render, its paths
# from the note's folder in the project: note.bend), so it never becomes
# HTML either.
def View.md.loc(+lc: I.Loc, +u: U.Ui) -> D.Node:
  D.El{"div", "file-md:" ++ U.Ui.view_get(u, "view.path"), [D.At{"class", "fm-md body"}, D.At{"data-md-src", IM.Img.url(U.Ui.view_get(u, "view.path"))},
    D.At{"data-md-root", U.Ui.root(u)}, D.At{"data-md-path", U.Ui.view_get(u, "view.path")}, D.At{"data-loading", MSG.T.viewer.loading(lc)}, D.At{"data-error", MSG.T.viewer.unread(lc)}], Nil{}}

def View.text.loc(+lc: I.Loc, +u: U.Ui) -> List<&2, D.Node>:
  [View.fm.head.loc(lc, u, Nil{}),
   Bool.pick(D.Node, FM.Fm.md(U.Ui.view_get(u, "view.path")), View.md.loc(lc, u), View.text.pre.loc(lc, u))]

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

def View.review.body(+lc: I.Loc, has: Bool, +v: U.RpView, +extra: List<&2, D.Node>, +u: U.Ui) -> D.Node:
  match has:
    case True{}:
      D.El{"div", "review-body", [D.At{"class", "review-body body rp-" ++ U.RpView.page(v)}, D.At{"data-review-thread", U.Ui.sel(u)}, D.At{"data-review-place", U.Rp.place(u)}, D.At{"data-review-ready", "1"}], List.append(&2, D.Node, extra, D.Md.render.loc(lc, U.RpView.md(v)))}
    case False{}:
      D.El{"div", "review-body", [D.At{"class", "review-body"}, D.At{"data-review-thread", U.Ui.sel(u)}, D.At{"data-review-place", U.Rp.place(u)}, D.At{"data-review-ready", "0"}], List.append(&2, D.Node, extra, [D.Dom.span("fm-note", Bool.pick(String, String.eq(U.RpView.page(v), "release"), Bool.pick(String, U.RpView.busy(v), "", Bool.pick(String, String.is_empty(U.RpView.err(v)), U.RpView.note.loc(lc, v), "")), U.RpView.note.loc(lc, v)))])}

def View.review.v(+lc: I.Loc, +v: U.RpView, +extra: List<&2, D.Node>, +u: U.Ui) -> List<&2, D.Node>:
  [D.El{"div", "", [D.At{"class", "fm-bar review-pane"}], List.append(&2, D.Node, View.review.tabs(U.RpView.tabs(v)),
     [D.Dom.span("fm-file", U.RpView.title(v) ++ Bool.pick(String, U.RpView.busy(v), " · reading…", "")), D.El{"span", "", [D.At{"class", "grow"}], Nil{}},
      D.Dom.button("", "rv-run", "", MSG.T.review.refresh.tip(lc), MSG.T.common.refresh(lc))])},
   View.review.body(lc, Bool.not(String.is_empty(U.RpView.md(v))), v, extra, u)]

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
  View.review.v(lc, U.Rp.view.loc(lc, u), View.review.x(lc, String.eq(U.Rp.page(u), "release"), u), u)

def View.aside.k(+lc: I.Loc, i: Nat, +u: U.Ui) -> List<&2, D.Node>:
  match i:
    case 0n:
      List.append(&2, D.Node, View.revert.loc(U.Ui.reverting(u), lc), View.diff.at(lc, U.Ui.view_get(u, "diff"), U.Ui.view_get(u, "diff.none"), U.Ui.view_get(u, "diff.wait"), U.Ui.view_get(u, "diff.kept")))
    case 1n:
      View.browser.loc(lc, u)
    case 2n:
      View.mech(lc, u)
    case 3n:
      View.solid.loc(lc, u)
    case 4n:
      View.sch.after(View.plot(lc, u, List.append(&2, D.Node, View.boards(u, MSG.T.viewer.schematic.tip(lc)), [View.plot.kicad.loc(lc)])),
        List.append(&2, D.Node, View.sch.below(lc, u), View.probe.away(u)))

    case 6n:
      View.files(lc, u)
    case 7n:
      View.image.loc(lc, u)
    case 8n:
      View.pdf.loc(lc, u)
    case 9n:
      View.review(lc, u)
    case 11n:
      View.text.loc(lc, u)
    case 10n:
      View.combo(lc, u)
    case _:
      View.plot(lc, u, List.append(&2, D.Node, View.boards(u, MSG.T.viewer.board.tip(lc)), [View.plot.kicad.loc(lc)]))

def View.aside.n(+kind: String) -> Nat:
  Bool.pick(Nat, String.eq(kind, "diff"), 0n, Bool.pick(Nat, String.eq(kind, "browser"), 1n, Bool.pick(Nat, String.eq(kind, "mech"), 2n,
    Bool.pick(Nat, String.eq(kind, "3d"), 3n, Bool.pick(Nat, String.eq(kind, "schematic"), 4n,
      Bool.pick(Nat, String.eq(kind, "files"), 6n, Bool.pick(Nat, String.eq(kind, "image"), 7n, Bool.pick(Nat, String.eq(kind, "pdf"), 8n, Bool.pick(Nat, String.eq(kind, "review"), 9n,
        Bool.pick(Nat, String.eq(kind, "split"), 10n, Bool.pick(Nat, String.eq(kind, "text"), 11n, 5n)))))))))))

# the side pane of the selected thread: the tabs, then what they show. Its
# left edge is a handle that resizes it (host.js keeps the width per
# browser; a double-click gives the pane its share back)
def View.split.loc(+lc: I.Loc) -> D.Node:
  D.El{"div", "split", [D.At{"class", "viewer-split"}, D.At{"tabindex", "0"}, D.At{"role", "separator"}, D.At{"aria-orientation", "vertical"},
    D.At{"aria-label", MSG.T.viewer.resize(lc)}, D.At{"title", MSG.T.viewer.resize(lc)}], Nil{}}

def View.split() -> D.Node:
  View.split.loc(I.En{})

def View.home.tabs(home: Bool, +lc: I.Loc, +kind: String, +u: U.Ui) -> D.Node:
  match home:
    case True{}: PHV.HomeView.viewer.tabs(u)
    case False{}: View.tabs.for(View.rail(u), lc, Bool.pick(String, U.Mpage.onpart(u), "mech", U.Fm.tab(U.Solid.kind(u))))

# a phone's viewer screen puts its views in a row of chips over the pane
# (the header has ‹ back); a project's home keeps its own tabs
def View.aside.tabs.phone(home: Bool, +lc: I.Loc, +kind: String, +u: U.Ui) -> D.Node:
  match home:
    case True{}: View.home.tabs(True{}, lc, kind, u)
    case False{}: View.viewer.chips(lc, Bool.pick(String, U.Mpage.onpart(u), "mech", U.Fm.tab(U.Solid.kind(u))))

def View.aside.tabs(phone: Bool, +lc: I.Loc, +kind: String, +u: U.Ui) -> D.Node:
  match phone:
    case True{}: View.aside.tabs.phone(PH.Home.is(U.Ui.sel(u)), lc, kind, u)
    case False{}: View.home.tabs(PH.Home.is(U.Ui.sel(u)), lc, kind, u)

def View.aside(+lc: I.Loc, +kind: String, +u: U.Ui) -> D.Node:
  D.El{"aside", "viewer", [D.At{"class", "viewer " ++ Bool.pick(String, String.is_empty(kind), "board", kind)}, D.At{"id", "viewer"}, D.At{"data-split-project", U.Asset.project(u)}],
    View.split.loc(lc) <> View.aside.tabs(PHN.Phone.on(u), lc, kind, u) <> View.aside.k(lc, View.aside.n(kind), u)}

def View.aside.if(+lc: I.Loc, open: Bool, +kind: String, +u: U.Ui) -> List<&2, D.Node>:
  match open:
    case True{}:
      [View.aside(lc, kind, u)]
    case False{}:
      Nil{}

def View.main.s(+lc: I.Loc, settings: Bool, none: Bool, tour: Bool, u: U.Ui, +sel: String, +n: Nat) -> D.Node:
  match settings none u:
    case True{} _ U.Ui{+st, s2, d, l, o, on, now, ad, pa, +v, r, w, e, +note, +nf, ob, tr}:
      View.settings.loc(lc, U.Ui{st, s2, d, l, o, on, now, ad, pa, v, r, w, e, note, nf, ob, tr}, st, nf, v, note)
    case False{} True{} U.Ui{st, _, _, _, _, _, _, _, _, _, _, _, _, _, nf, _, _}:
      View.empty.t.loc(lc, tour, n, nf, st)
    case False{} False{} U.Ui{+st, s2, d, l, o, on, now, ad, pa, v, r, w, e, note, nf, ob, tr}:
      View.thread(lc, U.Ui{st, s2, d, l, o, on, now, ad, pa, v, r, w, e, note, nf, ob, tr}, U.Ui.thread(st, sel))

def View.main.thread(+lc: I.Loc, none: Bool, +u: U.Ui, +sel: String, +n: Nat) -> D.Node:
  match none u:
    case _ U.Ui{+st, _, _, _, +o, _, _, _, _, +v, _, _, _, +note, +nf, +ob, +tr}:
      View.main.s(lc, U.Flag.get(o, "settings"), none, U.Tour.on(u), u, sel, n)

def View.main.home(home: Bool, +lc: I.Loc, none: Bool, +u: U.Ui, +sel: String, n: Nat) -> D.Node:
  match home:
    case True{}:
      PHV.HomeView.node.picker(u, View.aside.if(lc, U.Home.viewer.allowed(u), U.Ui.view_get(u, "view.kind"), u), View.look.loc(lc, String.eq(U.Ui.scratch(u, "@look"), PH.Home.project(sel)), U.Ui.state(u), PH.Home.project(sel), String.eq(U.Ui.scratch(u, "@rm"), PH.Home.project(sel))))
    case False{}: View.main.thread(lc, none, u, sel, n)

def View.main.k(+lc: I.Loc, none: Bool, +u: U.Ui, +sel: String, n: Nat) -> D.Node:
  View.main.home(Bool.and(PH.Home.is(sel), Bool.not(U.Ui.flag(u, "settings"))), lc, none, u, sel, n)

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

# The first-run tour (core/onboard.bend): a dimmed page and a dialog with
# the step, Skip, Back and Next. The page's class names what the step
# points at ("tour-sidebar", "tour-add", "tour-head", "tour-settings"; the
# head only with a thread open), and style.css lifts that part of the page
# over the dimming. host.js makes it modal: the page behind is inert, focus
# stays in the dialog, and the keys go through app.bend's tour_key (Enter
# and the right arrow next, the left arrow back, Esc skip).
# the uberbot step's choices (client.bend's Tour.choices): one button each
# a choice is a button, or (the policy, the issue form) a link to a new tab
def View.tour.choice(+l: String, +a: String, +v: String) -> D.Node:
  Bool.pick(D.Node, String.eq(a, "open-url"),
    D.El{"a", "", [D.At{"class", "btn"}, D.At{"href", v}, D.At{"target", "_blank"}, D.At{"rel", "noopener noreferrer"}], [D.Txt{l}]},
    D.Dom.button("", a, v, l, l))

def View.tour.choices(cs: List<&2, U.TChoice>) -> List<&2, D.Node>:
  match cs:
    case Nil{}:
      Nil{}
    case U.TChoice{+l, +a, +v} <> t:
      View.tour.choice(l, a, v) <> View.tour.choices(t)

def View.tour.back.loc(+lc: I.Loc, on: Bool) -> D.Node:
  match on:
    case True{}:
      D.Dom.button("", "ob-back", "", MSG.T.tour.back.tip(lc), MSG.T.tour.back(lc))
    case False{}:
      D.Dom.span("gap", "")

def View.tour.back(on: Bool) -> D.Node:
  View.tour.back.loc(I.En{}, on)

# The compact header keeps these tools inside a menu. Show their labels
# in the card so the step also makes sense before there is a thread.
def View.tour.example.tool(t: TO.HeaderTool) -> D.Node:
  D.El{"span", "", Nil{}, [D.Txt{TO.Tools.label(t)}]}

def View.tour.example(show: Bool, +lc: I.Loc) -> List<&2, D.Node>:
  match show:
    case False{}: Nil{}
    case True{}:
      [D.El{"span", "", [D.At{"class", "tour-example"}, D.At{"aria-label", OB.Ob.title(lc, 3n)}],
        [View.tour.example.tool(TO.Tools.settle.loc(lc, False{})), View.tour.example.tool(TO.Tools.snooze.loc(lc, False{}, "", "")),
         View.tour.example.tool(TO.Tools.fork.loc(lc)), View.tour.example.tool(TO.Tools.panel.loc(lc, False{}))]}]

def View.tour.footer(telemetry: Bool, +lc: I.Loc, back: Bool, fwd: String) -> List<&2, D.Node>:
  match telemetry:
    case True{}:
      [D.Dom.button("telemetry-off", "share", "off", "Disable every telemetry category", "Turn off all telemetry"),
        D.Dom.button("primary", "ob-next", "", "Save the displayed telemetry choice", fwd),
        D.Dom.button("telemetry-later", "ob-skip", "", "Leave undecided; send nothing", "Decide later")]
    case False{}:
      [D.Dom.button("", "ob-skip", "", MSG.T.tour.end.tip(lc), MSG.T.tour.skip(lc)), D.Dom.span("gap", ""),
        View.tour.back.loc(lc, back), D.Dom.button("primary", "ob-next", "", MSG.T.tour.next.tip(lc), fwd)]

def View.tour.card(+lc: I.Loc, +i: Nat, +body: String, example: List<&2, D.Node>, cs: List<&2, U.TChoice>, +counter: String, back: Bool, fwd: String) -> List<&2, D.Node>:
  [D.El{"div", "", [D.At{"class", "tour-back"}], Nil{}},
    D.El{"div", "tour", [D.At{"class", Bool.pick(String, Nat.is_eq(i, OB.Ob.share()), "tour telemetry-tour", "tour")}, D.At{"role", "dialog"}, D.At{"aria-modal", "true"}, D.At{"aria-labelledby", "tour-title"}], [
      D.Dom.span("count", counter),
      D.El{"div", "tour-title", [D.At{"class", "tour-title"}, D.At{"id", "tour-title"}], [D.Txt{OB.Ob.title(lc, i)}]},
      D.El{"p", "", [D.At{"class", "tour-body"}], D.Txt{body} <> example},
      D.El{"div", "", [D.At{"class", "tour-choices"}], View.tour.choices(cs)},
      D.El{"div", "", [D.At{"class", "tour-btns"}], View.tour.footer(Nat.is_eq(i, OB.Ob.share()), lc, back, fwd)}]}]

# which client's words and targets the tour has (onboard.bend's k): the
# web page's, or on a phone the phone's (no keys to press, nothing in a
# sidebar to point at)
def View.tour.kind(+u: U.Ui) -> Nat:
  Bool.pick(Nat, PHN.Phone.on(u), 2n, 1n)

def View.tour.k(+lc: I.Loc, on: Bool, +i: Nat, +u: U.Ui) -> List<&2, D.Node>:
  match on:
    case True{}:
      View.tour.card(lc, i, U.Tour.body.loc(lc, View.tour.kind(u), u), View.tour.example(U.Tour.example(i), lc), U.Tour.choices(u), U.Tour.counter.loc(lc, u), U.Tour.back(u), U.Tour.forward(u))
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
  View.tour.cls.k(U.Tour.on(u), U.Tour.target(View.tour.kind(u), u))

# The window's top bar (View.chrome): the mark with the hub's dot, where you
# are as a breadcrumb (a thread: its project, its title, which a click
# renames, and ▾ its menu; else Settings, a bot, a room or the projects),
# the shown thread's state as key/value cells, the command field, then the
# sidebar fold, search and Settings. On a phone-sized page a ‹ before the
# crumbs goes back to the list.
def View.chrome.state.k(lc: I.Loc) -> String:
  match lc:
    case I.Ja{}: "状態"
    case _: "State"

# the state's word in its cell; the whole sentence is the cell's tooltip
def View.chrome.state.word(lc: I.Loc, +name: String) -> String:
  match lc:
    case I.Ja{}:
      Bool.pick(String, String.eq(name, "stalled"), "進捗停止", Bool.pick(String, String.eq(name, "blocked"), "対応待ち", Bool.pick(String, String.eq(name, "active"), "作業中", Bool.pick(String, String.eq(name, "settled"), "完了",
        Bool.pick(String, String.eq(name, "waiting"), "完了待ち", Bool.pick(String, String.eq(name, "finished"), "作業完了", Bool.pick(String, String.eq(name, "stopped"), "停止", Bool.pick(String, String.eq(name, "failed"), "失敗", "待機"))))))))
    case _:
      Bool.pick(String, String.eq(name, "stalled"), "stalled", Bool.pick(String, String.eq(name, "blocked"), "needs you", Bool.pick(String, String.eq(name, "active"), "working", Bool.pick(String, String.eq(name, "settled"), "settled",
        Bool.pick(String, String.eq(name, "waiting"), "waiting", Bool.pick(String, String.eq(name, "finished"), "finished", Bool.pick(String, String.eq(name, "stopped"), "stopped", Bool.pick(String, String.eq(name, "failed"), "failed", "idle"))))))))

def View.chrome.jump(lc: I.Loc) -> String:
  match lc:
    case I.Ja{}: "移動・コマンド実行"
    case _: "Jump or run a command"

# the field's words where the whole sentence does not fit
def View.chrome.jump.short(lc: I.Loc) -> String:
  match lc:
    case I.Ja{}: "コマンド"
    case _: "Commands"

def View.chrome.jump.tip(lc: I.Loc) -> String:
  match lc:
    case I.Ja{}: "コマンドパレット: スレッド、プロジェクト、コマンド · Ctrl+K"
    case _: "Command palette: threads, projects and commands · Ctrl+K"

def View.chrome.where(lc: I.Loc) -> String:
  match lc:
    case I.Ja{}: "現在地"
    case _: "Where you are"

def View.chrome.sep() -> D.Node:
  D.El{"span", "", [D.At{"class", "sep"}, D.At{"aria-hidden", "true"}], [D.Txt{"/"}]}

# crumbs that only name a place
def View.chrome.crumbs(ns: List<&2, String>) -> List<&2, D.Node>:
  match ns:
    case Nil{}: Nil{}
    case n <> t: View.chrome.sep() <> D.Dom.span("crumb", n) <> View.chrome.crumbs(t)

def View.chrome.nav(+lc: I.Loc, kids: List<&2, D.Node>) -> D.Node:
  D.El{"nav", "crumbs", [D.At{"class", "crumbs"}, D.At{"aria-label", View.chrome.where(lc)}], kids}

def View.chrome.back(+lc: I.Loc, +action: String, +value: String) -> D.Node:
  D.Dom.button("back", action, value, MSG.T.tour.back(lc), "‹")

# a key/value cell: the key small and faint, then what it says
def View.chrome.kv(+cls: String, +title: String, +k: String, vs: List<&2, D.Node>) -> D.Node:
  D.El{"span", "", [D.At{"class", "kv " ++ cls}, D.At{"title", title}], D.Dom.span("k", k) <> vs}

# the thread's state (the work line's signal, or how its last turn ended:
# SG.Signal.shown), Stop beside it while a turn runs
def View.chrome.state(+lc: I.Loc, +st: M.State, +info: Map<&2, String>, +id: String, +turn: M.TurnState, +pending: Nat) -> D.Node:
  +signal = SG.Signal.cached(info, id)
  +shown = SG.Signal.display.shown(signal, turn, pending, st, id)
  D.El{"span", "work", [D.At{"class", "kv state"}, D.At{"role", "group"}, D.At{"aria-label", View.work.label(lc)},
    D.At{"title", SG.Signal.display.label(lc, signal, shown, st, id)}],
    List.append(&2, D.Node, [D.Dom.span("k", View.chrome.state.k(lc)), View.dot.tip(SG.Signal.dot.name(SG.Signal.display.name(signal, st, id), turn, pending), SG.Signal.display.label(lc, signal, shown, st, id)), D.Dom.span("v work-state", View.chrome.state.word(lc, shown))],
      View.tools(TO.Tools.turn.loc(lc, turn)))}

# the subagents: a click opens or shuts their rows under the bar (View.subs.dock)
def View.chrome.subs.k(+lc: I.Loc, +open: Bool, +th: String, +rs: List<&2, SB.SubRow>, +info: Map<&2, String>) -> D.Node:
  D.El{"button", "subs-head", [D.At{"class", "kv subs-head" ++ Bool.pick(String, String.is_empty(View.sub.busy(lc, SB.Subs.active(rs, info))), "", " busy")}, D.At{"type", "button"},
    D.At{"aria-expanded", Bool.pick(String, open, "true", "false")}, D.At{"aria-controls", "subagent-rows"},
    D.At{"data-value", SB.Subs.panel.ask(open, th, rs)}, D.On{"click", SB.Subs.panel.action(open)},
    D.At{"title", Bool.pick(String, open, MSG.T.subagents.hide(lc), MSG.T.subagents.show(lc))}],
    [D.Dom.span("k", MSG.T.thread.subagents(lc)), D.Dom.span("v", View.sub.signal.panel(lc, rs, info)), WI.WIcon.svg("chevron-down", 12)]}

def View.chrome.subs(+lc: I.Loc, +st: M.State, +info: Map<&2, String>, +th: String, +rs: List<&2, SB.SubRow>) -> List<&2, D.Node>:
  D.Dom.when(Bool.and(Bool.not(List.is_empty(&2, SB.SubRow, rs)), Bool.not(US.Ub.chat.is(st, th))), View.chrome.subs.k(lc, SB.Subs.panel.open(info, th, rs), th, rs, info))

# the to-do list beside them: "To-do 3/7", a click opens or shuts its steps
# under the bar (View.todos.dock); shut until the person opens it
def View.chrome.todos.k(+lc: I.Loc, +open: Bool, +th: String, +ts: List<&2, TD.Todo>) -> D.Node:
  D.El{"button", "todos-head", [D.At{"class", "kv todos-head"}, D.At{"type", "button"},
    D.At{"aria-expanded", Bool.pick(String, open, "true", "false")}, D.At{"aria-controls", "todo-rows"},
    D.At{"data-value", U.Todos.toggle(open, th)}, D.On{"click", "fold"},
    D.At{"title", Bool.pick(String, open, MSG.T.todos.hide(lc), MSG.T.todos.show(lc))}],
    [D.Dom.span("k", MSG.T.thread.todos(lc)), D.Dom.span("v", TD.Todo.count(ts)), WI.WIcon.svg("chevron-down", 12)]}

def View.chrome.todos.if(none: Bool, +lc: I.Loc, +info: Map<&2, String>, +st: M.State, +th: String, +ts: List<&2, TD.Todo>) -> List<&2, D.Node>:
  match none:
    case True{}: Nil{}
    case False{}: [View.chrome.todos.k(lc, U.Todos.open(info, st, th), th, ts)]

def View.chrome.todos(+lc: I.Loc, +info: Map<&2, String>, +st: M.State, +th: String, +ts: List<&2, TD.Todo>) -> List<&2, D.Node>:
  View.chrome.todos.if(List.is_empty(&2, TD.Todo, ts), lc, info, st, th, ts)

def View.chrome.branch(+b: String) -> List<&2, D.Node>:
  D.Dom.when(Bool.not(String.is_empty(b)), D.El{"span", "", [D.At{"class", "kv branch"}, D.At{"title", b}], [WI.WIcon.svg("git-branch", 12), D.Dom.span("v", b)]})

# a thread's cells: its state, subagents, to-do list, branch and chip;
# none for a thread this client has not got yet (gone: U.Ui.thread's
# blank, as on the page the browser draws while the hub's is on its way),
# so the State cell never says "idle" for a thread it knows nothing of
def View.chrome.cells(gone: Bool, +lc: I.Loc, +st: M.State, +nf: Map<&2, String>, +id: String, +turn: M.TurnState, +pending: Nat, +branch: String, +now: Nat) -> List<&2, D.Node>:
  match gone:
    case True{}:
      Nil{}
    case False{}:
      List.concat(&2, D.Node, [
        [View.chrome.state(lc, st, nf, id, turn, pending)],
        View.chrome.subs(lc, st, nf, id, SB.Subs.rows(st, nf, id, now)),
        View.chrome.todos(lc, nf, st, id, U.Todos.shown(st, id, View.running(turn))),
        View.chrome.branch(branch),
        View.boat.chip.loc(lc, UZ.Chip.of(st, id, now))])

# a buy thread's title is marked an Order (the cart, as Buy it's)
def View.chrome.order(buying: Bool) -> List<&2, D.Node>:
  match buying:
    case True{}:
      [D.El{"span", "order-mark", [D.At{"class", "order-mark"}, D.At{"title", "This thread orders the project's parts"}], [WI.WIcon.svg("shopping-cart", 12), D.Dom.span("", "Order")]}]
    case False{}:
      Nil{}

# a phone's one top bar also holds Buy it and the rail's views (its menu,
# as the rail shows it, in the corner); style.css shows them only where
# the rail is hidden
def View.chrome.views(+lc: I.Loc, +u: U.Ui, +id: String) -> D.Node:
  D.El{"div", "top-views", [D.At{"class", "top-views"}],
    List.append(&2, D.Node, View.workspace.buy(U.Fab.rail(u)),
      [View.workspace.menu(lc, Bool.pick(String, U.Ui.panel(u), U.Rail.tab.of(U.Ui.panel(u), U.Ui.info(u, "view.kind:" ++ id), U.Ui.info(u, "view.path:" ++ id)), ""), U.Ui.flag(u, "term"), U.Ui.flag(u, "palette"))])}

# a thread: its project, its title (a click renames it) and ▾ its menu, then
# its cells; a project name cut short ends in "…" and its tooltip is whole
def View.chrome.thread(+lc: I.Loc, +u: U.Ui, t: M.Thread) -> List<&2, D.Node>:
  match u t:
    case U.Ui{+st, _, +drafts, _, _, _, +now, _, +panel, _, _, _, _, _, +nf, _, _} M.Thread{+id, +p, title, _, _, turn, pending, archived, override, +sa, +ua, pinned, _, +branch, _, _, _, _, _, _, gone}:
      +pn = U.Label.title(st, p, U.Ui.project_title(st, p))
      [View.chrome.nav(lc, [
        View.chrome.sep(),
        D.El{"span", "", [D.At{"class", "crumb proj"}, D.At{"style", "--tint:" ++ BG.Badge.css_of(st, p)}, D.At{"title", pn}], [D.El{"i", "", [D.At{"class", "sw"}], Nil{}}, D.Dom.span("pn", pn)]},
        View.chrome.sep(),
        D.El{"div", "", [D.At{"class", "title-box"}], List.append(&2, D.Node, View.chrome.order(U.Fab.buying(st, id)), View.title.k.loc(lc, String.eq(U.Str.get(drafts, "@ren"), id), id, U.Label.title(st, id, TTL.Title.shown(lc, title)), U.Str.get(drafts, "@f.title")))},
        View.thread.actions(lc, [WI.WIcon.svg("chevron-down", 14)], st, id, TO.Tools.settled(override, Nat.is_gt(sa, ua)), pinned, M.Snooze.on(now, U.Side.until(st, id)), archived,
          String.eq(U.Str.get(drafts, "@del"), id), U.Ui.flag(u, "term"), panel)]),
       D.El{"div", "kvs", [D.At{"class", "kvs"}], View.chrome.cells(gone, lc, st, nf, id, turn, pending, branch, now)},
       View.chrome.views(lc, u, id)]

def View.chrome.selected(none: Bool, +lc: I.Loc, +u: U.Ui) -> List<&2, D.Node>:
  match none:
    case True{}: [View.chrome.nav(lc, View.chrome.crumbs([MSG.T.tour.title.projects(lc)]))]
    case False{}: View.chrome.back(lc, "select", "") <> View.chrome.thread(lc, u, U.Ui.thread(U.Ui.state(u), U.Ui.sel(u)))

# Settings, and the page open in it (its name goes back to the menu)
def View.chrome.settings.k(root: Bool, +lc: I.Loc, +page: String) -> List<&2, D.Node>:
  match root:
    case True{}: View.chrome.crumbs([MSG.T.palette.settings(lc)])
    case False{}: View.chrome.sep() <> D.Dom.button("crumb link", "settings-page", "", MSG.T.palette.settings(lc), MSG.T.palette.settings(lc)) <> View.chrome.crumbs([U.Settings.title(lc, page)])

def View.chrome.settings(+lc: I.Loc, +page: String) -> List<&2, D.Node>:
  View.chrome.settings.k(String.is_empty(page), lc, page)

# where you are: Settings over all, then what the main area shows
def View.chrome.place(+lc: I.Loc, settings: Bool, bv: BI.View, +u: U.Ui) -> List<&2, D.Node>:
  match settings bv:
    case True{} _:
      [View.chrome.back(lc, "flag", "settings"), View.chrome.nav(lc, View.chrome.settings(lc, U.Settings.page(u)))]
    case False{} BI.VBot{b}:
      [View.chrome.nav(lc, View.chrome.crumbs([MSG.T.tour.title.bots(lc), BI.Head.title(BI.BU.head.bot(u, b))]))]
    case False{} BI.VRoom{r}:
      [View.chrome.nav(lc, View.chrome.crumbs([MSG.T.bots.rooms(lc), "# " ++ BT.Room.name(r)]))]
    case False{} BI.VRemote{id}:
      [View.chrome.nav(lc, View.chrome.crumbs([BI.Head.title(BI.BU.head(u, BI.VRemote{id}))]))]
    case False{} BI.VThread{}:
      View.chrome.selected(String.is_empty(U.Ui.sel(u)), lc, u)

def View.chrome.brand(+lc: I.Loc, +on: Bool, +v: String) -> List<&2, D.Node>:
  [D.El{"span", "", [D.At{"class", "brand-mark"}], [D.El{"span", "", [D.At{"class", Bool.pick(String, on, "conn on", "conn")},
     D.At{"role", "img"}, D.At{"title", MSG.T.native.hub.state(lc, Bool.pick(String, on, "online", "offline"))},
     D.At{"aria-label", MSG.T.native.hub.state(lc, Bool.pick(String, on, "online", "offline"))}, D.At{"data-down", MSG.T.native.hub.state(lc, "offline")}], Nil{}}]},
   D.El{"span", "", [D.At{"class", "name"}, D.At{"title", "Backplane " ++ v}], [D.Txt{"Backplane"}]}]

# the command palette's field (WS.palette, Ctrl+K)
def View.chrome.cmd(+lc: I.Loc) -> D.Node:
  D.El{"button", "", [D.At{"class", "cmd"}, D.At{"type", "button"}, D.At{"title", View.chrome.jump.tip(lc)}, D.At{"data-value", ""}, D.On{"click", "ws-palette-open"}],
    [WI.WIcon.svg("chevron-right", 13), D.El{"span", "", [D.At{"class", "cmd-text"}], [D.Dom.span("cmd-long", View.chrome.jump(lc)), D.Dom.span("cmd-short", View.chrome.jump.short(lc))]},
     D.El{"kbd", "", Nil{}, [D.Txt{"Ctrl K"}]}]}

# search (Ctrl+Shift+F) and Settings, on while open (the sidebar folds
# from its own chevron and the strip's, and Ctrl+B)
def View.chrome.tools(+lc: I.Loc, settings: Bool) -> D.Node:
  D.El{"div", "", [D.At{"class", "tb"}], [
    D.El{"button", "", [D.At{"type", "button"}, D.At{"data-improvements", ""}, D.At{"title", "Improvements"}, D.At{"aria-label", "Improvements"}], [WI.WIcon.svg("message-square", 15), D.Dom.span("improvements-label", "Improvements")]},
    WI.WIcon.button("find-open", "thin-search-open", "", "Search conversations · Ctrl+Shift+F", "search", 15, ""),
    WI.WIcon.button(Bool.pick(String, settings, "gear on", "gear"), "flag", "settings", MSG.T.palette.settings(lc), "settings", 15, "")]}

def View.chrome.bar(+lc: I.Loc, +settings: Bool, bv: BI.View, +u: U.Ui) -> D.Node:
  match u:
    case U.Ui{_, _, _, _, _, +on, _, _, _, +v, _, _, _, _, _, _, _}:
      D.El{"header", "topbar", [D.At{"class", "topbar"}], List.concat(&2, D.Node, [
        View.chrome.brand(lc, on, v), View.chrome.place(lc, settings, bv, u),
        [D.Dom.span("grow", ""), View.chrome.cmd(lc), View.chrome.tools(lc, settings)]])}

def View.chrome.if(top: Bool, +lc: I.Loc, +settings: Bool, bv: BI.View, +u: U.Ui) -> List<&2, D.Node>:
  match top:
    case True{}: [View.chrome.bar(lc, settings, bv, u)]
    case False{}: Nil{}

# An optional suggestion in its own layout row, leaving the conversation
# and composer usable. Keep the offer while a tour, setup or dialog is up.
def View.star.visible(+u: U.Ui) -> Bool:
  U.Star.visible(u) && Bool.not(U.CefUI.covered(u)) && Bool.not(U.Ui.flag(u, "settings")) && Bool.not(U.Psv.active(u)) &&
    Bool.not(U.Intent.on(u)) && String.is_empty(U.Ui.row_deleting(u)) && String.is_empty(U.Ren.on(u)) && String.is_empty(U.Ui.scratch(u, "@rm"))

def View.star.k(on: Bool, +token: String) -> List<&2, D.Node>:
  match on:
    case False{}: Nil{}
    case True{}:
      [D.El{"section", "star-nudge", [D.At{"class", "star-nudge"}, D.At{"role", "status"}, D.At{"data-star-offer", token}], [
        D.El{"p", "", [D.At{"class", "star-nudge-text"}], [D.El{"span", "", [D.At{"class", "star-nudge-icon"}, D.At{"aria-hidden", "true"}], [D.Txt{"★"}]}, D.Dom.span("", U.Star.body())]},
        D.El{"a", "", [D.At{"class", "star-nudge-open"}, D.At{"aria-label", "Star on GitHub (opens in a new tab)"}, D.At{"href", U.Star.url()}, D.At{"target", "_blank"}, D.At{"rel", "noopener noreferrer"},
          D.At{"data-value", token}, D.On{"click", "star-open"}], [D.El{"span", "", [D.At{"aria-hidden", "true"}], [D.Txt{"★"}]}, D.Txt{"Star on GitHub"}]},
        D.El{"button", "", [D.At{"class", "star-nudge-dismiss"}, D.At{"type", "button"}, D.At{"aria-label", "Dismiss star suggestion"}, D.At{"title", "Dismiss star suggestion"},
          D.At{"data-value", token}, D.On{"click", "star-dismiss"}], [WI.WIcon.svg("x", 16)]}]}]

def View.star(+u: U.Ui) -> List<&2, D.Node>:
  View.star.k(View.star.visible(u), U.Star.token(u))

# The phone arrangement (phone.bend; scratch DESIGN.md): the same pieces as
# the page below, one screen at a time. The screen is worked out from what
# this browser has open: Settings (its root or a page), a bot or room, a
# project's home, a thread, or its viewer over it; with nothing open, the
# root tab (@ph.tab). Level 0 screens show the tab bar, the others a ‹.
def View.phone(+u: U.Ui) -> Bool:
  PHN.Phone.on(u)

def View.phone.screen.sel(none: Bool, +u: U.Ui) -> String:
  match none:
    case True{}: PHN.Phone.tab(u)
    case False{}: Bool.pick(String, PH.Home.is(U.Ui.sel(u)), "project", Bool.pick(String, U.Ui.panel(u), "viewer", "thread"))

def View.phone.screen(settings: Bool, bv: BI.View, +u: U.Ui) -> String:
  match settings bv:
    case True{} _: Bool.pick(String, String.is_empty(U.Settings.page(u)), "settings", "settings-page")
    case False{} BI.VBot{_}: "bot"
    case False{} BI.VRoom{_}: "bot"
    case False{} BI.VRemote{_}: "bot"
    case False{} BI.VThread{}: View.phone.screen.sel(String.is_empty(U.Ui.sel(u)), u)

def View.phone.level(+s: String) -> String:
  match s:
    case "now": "0"
    case "projects": "0"
    case "bots": "0"
    case "settings": "0"
    case "viewer": "2"
    case _: "1"

# a root screen's tools: search every conversation, the command palette
# the sidebar's rows over every project, as the sidebar makes them
def View.phone.raw(+u: U.Ui, +ix: U.SIx) -> List<&2, U.SRow>:
  U.Side.rows.ix.at(U.Ui.state(u), ix, U.Side.now(u), U.Side.cur.held(U.Ui.state(u), U.Ui.sel(u), U.Side.now(u)))

# a new thread in the project at the top of the sidebar's order (live
# first, the one worked in last), the thumb's quickest way to start; none
# before there is a project
def View.phone.new.k(none: Bool, +lc: I.Loc, +p: String, +st: M.State) -> List<&2, D.Node>:
  match none:
    case True{}: Nil{}
    case False{}: [PHN.Phone.tool("new-thread", p, MSG.T.thread.new(lc) ++ " · " ++ U.Ui.project_title(st, p), "square-pen")]

def View.phone.new(+lc: I.Loc, +u: U.Ui) -> List<&2, D.Node>:
  +p = Maybe.default(&2, String, List.head(&2, String, U.Side.pids(View.phone.raw(u, U.Side.ix(U.Ui.state(u))))), "")
  View.phone.new.k(String.is_empty(p), lc, p, U.Ui.state(u))

def View.phone.tools(+lc: I.Loc) -> List<&2, D.Node>:
  [PHN.Phone.tool("thin-search-open", "", "Search conversations", "search"), PHN.Phone.tool("ws-palette-open", "", View.chrome.jump.short(lc), "zap")]

def View.phone.root(+lc: I.Loc, +on: Bool, +v: String, +name: String, tools: List<&2, D.Node>) -> D.Node:
  PHN.Phone.head(Nil{}, List.append(&2, D.Node, View.chrome.brand(lc, on, v), PHN.Phone.title(name, "")), tools)

def View.phone.back(+lc: I.Loc) -> List<&2, D.Node>:
  [PHN.Phone.back("select", "", MSG.T.tour.back(lc))]

# the views a thread can show (the rail's, as its menu); C's to shape
# its summary on a phone: the open view's icon, or "layers" for the
# thread's own chat (the chat bubble read as a second message button)
def View.phone.views.menu(+lc: I.Loc, +cur: String, term: Bool, palette: Bool) -> D.Node:
  D.El{"details", "view-menu", [D.At{"class", "local-menu workspace-menu"}], [
    D.El{"summary", "", [D.At{"aria-label", "Views"}, D.At{"title", "Views: board, schematic, 3D, files, diff, review, browser, terminal"}], [
      WI.WIcon.svg(Bool.pick(String, String.is_empty(cur), "layers", View.workspace.icon(cur)), 20), D.Dom.span("workspace-current", "")]},
    D.El{"div", "", [D.At{"class", "local-menu-body"}], View.workspace.choices(False{}, lc, cur, term, palette)}]}

def View.phone.views(+lc: I.Loc, +u: U.Ui, +id: String) -> D.Node:
  D.El{"div", "top-views", [D.At{"class", "top-views"}],
    List.append(&2, D.Node, View.workspace.buy(U.Fab.rail(u)),
      [View.phone.views.menu(lc, Bool.pick(String, U.Ui.panel(u), U.Rail.tab.of(U.Ui.panel(u), U.Ui.info(u, "view.kind:" ++ id), U.Ui.info(u, "view.path:" ++ id)), ""), U.Ui.flag(u, "term"), U.Ui.flag(u, "palette"))])}

# a thread: ‹, its title (a tap renames it) over its project, ▾ its menu,
# Buy it and its views (in the corner); its state's cells (B's) ride in a row under the header
def View.phone.head.thread(+lc: I.Loc, +back: List<&2, D.Node>, +u: U.Ui, t: M.Thread) -> List<&2, D.Node>:
  match u t:
    case U.Ui{+st, _, +drafts, _, _, _, +now, _, +panel, _, _, _, _, _, +nf, _, _} M.Thread{+id, +p, title, _, _, turn, pending, archived, override, +sa, +ua, pinned, _, +branch, _, _, _, _, _, _, gone}:
      [PHN.Phone.head(back,
        [D.El{"div", "", [D.At{"class", "title-box"}], List.append(&2, D.Node, View.chrome.order(U.Fab.buying(st, id)), View.title.k.loc(lc, String.eq(U.Str.get(drafts, "@ren"), id), id, U.Label.title(st, id, TTL.Title.shown(lc, title)), U.Str.get(drafts, "@f.title")))},
         D.Dom.span("ph-sub", U.Label.title(st, p, U.Ui.project_title(st, p)))],
        [View.thread.actions(lc, [WI.WIcon.svg("chevron-down", 18)], st, id, TO.Tools.settled(override, Nat.is_gt(sa, ua)), pinned, M.Snooze.on(now, U.Side.until(st, id)), archived,
           String.eq(U.Str.get(drafts, "@del"), id), U.Ui.flag(u, "term"), panel), View.phone.views(lc, u, id)]),
       D.El{"div", "ph-cells", [D.At{"class", "kvs ph-cells"}], View.chrome.cells(gone, lc, st, nf, id, turn, pending, branch, now)}]

def View.phone.head.bot(+lc: I.Loc, bv: BI.View, +u: U.Ui) -> List<&2, D.Node>:
  match bv:
    case BI.VBot{b}: [PHN.Phone.head(View.phone.back(lc), PHN.Phone.title(BI.Head.title(BI.BU.head.bot(u, b)), MSG.T.tour.title.bots(lc)), Nil{})]
    case BI.VRoom{r}: [PHN.Phone.head(View.phone.back(lc), PHN.Phone.title("# " ++ BT.Room.name(r), MSG.T.bots.rooms(lc)), Nil{})]
    case BI.VRemote{id}: [PHN.Phone.head(View.phone.back(lc), PHN.Phone.title(BI.Head.title(BI.BU.head(u, BI.VRemote{id})), ""), Nil{})]
    case BI.VThread{}: Nil{}

def View.phone.head(+lc: I.Loc, +screen: String, bv: BI.View, +u: U.Ui, +on: Bool, +v: String) -> List<&2, D.Node>:
  match screen:
    case "thread": View.phone.head.thread(lc, View.phone.back(lc), u, U.Ui.thread(U.Ui.state(u), U.Ui.sel(u)))
    case "viewer": View.phone.head.thread(lc, [PHN.Phone.back("panel", "", MSG.T.viewer.close(lc))], u, U.Ui.thread(U.Ui.state(u), U.Ui.sel(u)))
    case "project": [PHN.Phone.head(View.phone.back(lc), PHN.Phone.title(U.Ui.project_title(U.Ui.state(u), PH.Home.project(U.Ui.sel(u))), MSG.T.tour.title.projects(lc)), Nil{})]
    case "bot": View.phone.head.bot(lc, bv, u)
    case "settings": [View.phone.root(lc, on, v, MSG.T.palette.settings(lc), Nil{})]
    case "settings-page": [PHN.Phone.head([PHN.Phone.back("settings-page", "", MSG.T.palette.settings(lc))], PHN.Phone.title(U.Settings.title(lc, U.Settings.page(u)), MSG.T.palette.settings(lc)), Nil{})]
    case "bots": [View.phone.root(lc, on, v, MSG.T.tour.title.bots(lc), View.phone.tools(lc))]
    case "now": [View.phone.root(lc, on, v, PHN.Phone.now.label(lc), List.append(&2, D.Node, View.phone.new(lc, u), View.phone.tools(lc)))]
    case _: [View.phone.root(lc, on, v, MSG.T.tour.title.projects(lc), List.append(&2, D.Node, View.phone.new(lc, u), View.phone.tools(lc)))]

# threads needing the person (the sidebar's own count, Plane.tally)
def View.phone.need(+u: U.Ui) -> Nat:
  +ix = U.Side.ix(U.Ui.state(u))
  PL.PTally.need(PL.Plane.tally(u, ix, View.phone.raw(u, ix)))

# rooms with something unread (the phone apps' Bots badge, MTab.rooms.unread)
def View.phone.unread(rs: List<&2, BI.RRow>, +u: U.Ui, acc: Nat) -> Nat:
  match rs:
    case Nil{}: acc
    case BI.RRow{+id, _, _, _} <> t: View.phone.unread(t, u, Nat.add(acc, Bool.pick(Nat, Nat.is_gt(U.Rooms.unread(u, id), 0n), 1n, 0n)))

def View.phone.tabs(+lc: I.Loc, root: Bool, +screen: String, +u: U.Ui) -> List<&2, D.Node>:
  match root:
    case True{}: [PHN.Phone.tabs(lc, screen, View.phone.need(u), View.phone.unread(BI.BU.rooms(u), u, 0n))]
    case False{}: Nil{}

# the Now tab: what needs the person first, then what runs, over every
# project (the sidebar's Active rows, core's Active.rows.given, whatever
# the sidebar.view setting), drawn only while the tab is open
# the rows whose urgency key is in [lo, hi), in their order
def View.phone.band.k(keep: Bool, +r: U.ARow, rest: List<&2, U.ARow>) -> List<&2, U.ARow>:
  match keep:
    case True{}: r <> rest
    case False{}: rest

def View.phone.band(rs: List<&2, U.ARow>, +lo: Nat, +hi: Nat) -> List<&2, U.ARow>:
  match rs:
    case Nil{}: Nil{}
    case U.ARow{+urg, +at, +th, +p, +ti, +f} <> rest:
      View.phone.band.k(Bool.and(Nat.is_ge(urg, lo), Nat.is_lt(urg, hi)), U.ARow{urg, at, th, p, ti, f}, View.phone.band(rest, lo, hi))

def View.phone.group(+lc: I.Loc, +label: String, rs: List<&2, U.ARow>, +ix: U.SIx, +st: M.State, +sel: String) -> List<&2, D.Node>:
  match rs:
    case Nil{}: Nil{}
    case +r <> +t: D.El{"div", "", [D.At{"class", "ph-group"}, D.At{"role", "heading"}], [D.Txt{label ++ " · " ++ Nat.show(Nat.add(1n, List.length(&2, U.ARow, t)))}]} <> View.arows(lc, r <> t, ix, st, sel, "", "live")

# the Active rows in bands: Needs you, Finished, At work, Idle, then the
# settled fold
def View.phone.now.parts(+lc: I.Loc, p: U.AParts, +u: U.Ui, +ix: U.SIx) -> List<&2, D.Node>:
  match p:
    case U.AParts{+l, f}:
      [D.El{"section", "", [D.At{"class", "project active"}], List.concat(&2, D.Node, [
        View.phone.group(lc, PHN.Phone.now.band(lc, 0n), View.phone.band(l, 12n, 100n), ix, U.Ui.state(u), U.Ui.sel(u)),
        View.phone.group(lc, PHN.Phone.now.band(lc, 1n), View.phone.band(l, 8n, 12n), ix, U.Ui.state(u), U.Ui.sel(u)),
        View.phone.group(lc, PHN.Phone.now.band(lc, 2n), View.phone.band(l, 4n, 8n), ix, U.Ui.state(u), U.Ui.sel(u)),
        View.phone.group(lc, PHN.Phone.now.band(lc, 3n), View.phone.band(l, 0n, 4n), ix, U.Ui.state(u), U.Ui.sel(u)),
        View.afaded(lc, f, "", U.Side.open(u), ix, U.Ui.state(u), U.Ui.sel(u))])}]

def View.phone.now.rows(+lc: I.Loc, rs: List<&2, U.ARow>, +u: U.Ui, +ix: U.SIx) -> List<&2, D.Node>:
  match rs:
    case Nil{}: [PHN.Phone.empty("circle-check", PHN.Phone.now.none(lc), PHN.Phone.now.hint(lc), [D.Dom.button("primary", "ph-tab", "projects", MSG.T.tour.title.projects(lc), MSG.T.tour.title.projects(lc))])]
    case +r <> +t: View.phone.now.parts(lc, U.Active.parts(r <> t), u, ix)

def View.phone.now.k(+lc: I.Loc, +u: U.Ui, +ix: U.SIx) -> D.Node:
  D.El{"section", "ph-now", [D.At{"class", "ph-now"}], View.phone.now.rows(lc, U.Active.rows.given(U.Ui.state(u), ix, U.Side.now(u), "", View.phone.raw(u, ix)), u, ix)}

def View.phone.now(on: Bool, +lc: I.Loc, +u: U.Ui) -> List<&2, D.Node>:
  match on:
    case True{}: [View.phone.now.k(lc, u, U.Side.signals.ix(U.Side.ix(U.Ui.state(u)), u))]
    case False{}: Nil{}

# a rename asked from a list's row menu (row-rename): the phones' dialog, as
# a sheet with the header's own field (the open thread renames in place)
def View.phone.rename.k(on: Bool, +lc: I.Loc, +u: U.Ui) -> List<&2, D.Node>:
  match on:
    case True{}: [PHN.Phone.sheet.on("rename", MSG.T.menu.rename(lc), "rename-no",
      [D.El{"div", "", [D.At{"class", "title-box ph-rename"}], View.title.k.loc(lc, True{}, U.Ui.scratch(u, "@ren"), "", U.Ui.scratch(u, "@f.title"))}])]
    case False{}: Nil{}

def View.phone.rename(+lc: I.Loc, +u: U.Ui) -> List<&2, D.Node>:
  View.phone.rename.k(Bool.and(Bool.not(String.is_empty(U.Ui.scratch(u, "@ren"))), Bool.not(String.eq(U.Ui.scratch(u, "@ren"), U.Ui.sel(u)))), lc, u)

def View.phone.cls(+screen: String, +level: String) -> String:
  "app top phone ph-" ++ screen ++ " ph-l" ++ level

# the whole phone page: the header, the list (the sidebar, which CSS shows
# on the Projects and Bots tabs), the main screen, the viewer while open,
# then the layers over them, and the tab bar on a root screen
def View.phone.page.k(+lc: I.Loc, +bv: BI.View, +u: U.Ui, sidebar: D.Node, +on: Bool, +v: String, +e: String) -> D.Node:
  +settings = U.Ui.flag(u, "settings")
  +screen = View.phone.screen(settings, bv, u)
  +sel = U.Ui.sel(u)
  +st = U.Ui.state(u)
  D.El{"div", "app", [D.At{"data-bub-home", J.Json.show(HOP.Hop.snapshot(u))}, D.At{"style", "--text-scale:" ++ U32.show(TS.Scale.of(st))}, D.At{"data-ph-screen", screen},
      D.At{"class", View.phone.cls(screen, View.phone.level(screen)) ++ Bool.pick(String, U.Ui.panel(u), " panel", "") ++ Bool.pick(String, Bool.or(Bool.or(Bool.not(String.is_empty(sel)), settings), WB.WB.shown(bv)), " picked", "")
        ++ Bool.pick(String, String.eq(M.Setting.get(st, "ui.theme", ""), "light"), " light", "") ++ View.tour.cls(u)}],
    List.concat(&2, D.Node, [View.phone.head(lc, screen, bv, u, on, v), [sidebar], View.phone.now(String.eq(screen, "now"), lc, u), [View.main.b(lc, settings, bv, u, sel, List.length(&2, M.Project, U.Ui.projects(st)))],
      View.aside.if(lc, Bool.and(U.Ui.panel(u), Bool.not(PH.Home.is(sel))), U.Ui.info(u, "view.kind:" ++ sel), u),
      View.picker(lc, U.Ui.flag(u, "proj"), u), View.phone.rename(lc, u), View.intent(lc, u), View.error(U.Ui.error.shown(lc, e)), View.tour(lc, u), View.star(u),
      View.phone.tabs(lc, String.eq(View.phone.level(screen), "0"), screen, u)])}

def View.phone.page(+lc: I.Loc, +bv: BI.View, u: U.Ui, sidebar: D.Node) -> D.Node:
  match u:
    case U.Ui{+st, +sel, +d, +l, +o, +on, +now, +ad, +pa, +v, +r, +w, +e, +note, +nf, +ob, +tr}:
      View.phone.page.k(lc, bv, U.Ui{st, sel, d, l, o, on, now, ad, pa, v, r, w, e, note, nf, ob, tr}, sidebar, on, v, e)

# the whole page (the desktop's)
def View.page.b.desk(+lc: I.Loc, +bv: BI.View, u: U.Ui, sidebar: D.Node) -> D.Node:
  match u:
    case U.Ui{+st, +sel, +d, +l, +o, +on, +now, +ad, +pa, +v, +r, +w, +e, +note, +nf, +ob, +tr}:
      D.El{"div", "app", [D.At{"data-bub-home", J.Json.show(HOP.Hop.snapshot(U.Ui{st, sel, d, l, o, on, now, ad, pa, v, r, w, e, note, nf, ob, tr}))}, D.At{"style", "--text-scale:" ++ U32.show(TS.Scale.of(st))}, D.At{"class", Bool.pick(String, pa, "app panel", "app") ++ Bool.pick(String, Bool.or(Bool.or(Bool.not(String.is_empty(sel)), U.Flag.get(o, "settings")), WB.WB.shown(bv)), " picked", "")
        ++ Bool.pick(String, String.eq(M.Setting.get(st, "ui.theme", ""), "light"), " light", "") ++ Bool.pick(String, View.chrome(U.Ui{st, sel, d, l, o, on, now, ad, pa, v, r, w, e, note, nf, ob, tr}), " top", "") ++ View.tour.cls(U.Ui{st, sel, d, l, o, on, now, ad, pa, v, r, w, e, note, nf, ob, tr})}],
        D.Dom.cat(View.chrome.if(View.chrome(U.Ui{st, sel, d, l, o, on, now, ad, pa, v, r, w, e, note, nf, ob, tr}), lc, U.Flag.get(o, "settings"), bv, U.Ui{st, sel, d, l, o, on, now, ad, pa, v, r, w, e, note, nf, ob, tr}), D.Dom.cat([sidebar,
          View.main.b(lc, U.Flag.get(o, "settings"), bv, U.Ui{st, sel, d, l, o, on, now, ad, pa, v, r, w, e, note, nf, ob, tr}, sel, List.length(&2, M.Project, U.Ui.projects(st)))],
          D.Dom.cat(View.aside.if(lc, Bool.and(pa, Bool.not(PH.Home.is(sel))), U.Str.get(nf, "view.kind:" ++ sel), U.Ui{st, sel, d, l, o, on, now, ad, pa, v, r, w, e, note, nf, ob, tr}),
            D.Dom.cat(View.picker(lc, U.Flag.get(o, "proj"), U.Ui{st, sel, d, l, o, on, now, ad, pa, v, r, w, e, note, nf, ob, tr}), D.Dom.cat(View.intent(lc, U.Ui{st, sel, d, l, o, on, now, ad, pa, v, r, w, e, note, nf, ob, tr}),
              D.Dom.cat(View.error(U.Ui.error.shown(lc, e)), D.Dom.cat(View.tour(lc, U.Ui{st, sel, d, l, o, on, now, ad, pa, v, r, w, e, note, nf, ob, tr}),
                View.star(U.Ui{st, sel, d, l, o, on, now, ad, pa, v, r, w, e, note, nf, ob, tr}))))))))}

# a phone's page is View.phone.page; every other page is exactly as before
def View.page.b.form(phone: Bool, +lc: I.Loc, +bv: BI.View, +u: U.Ui, sidebar: D.Node) -> D.Node:
  match phone:
    case True{}: View.phone.page(lc, bv, u, sidebar)
    case False{}: View.page.b.desk(lc, bv, u, sidebar)

def View.page.b.sidebar(+lc: I.Loc, +bv: BI.View, +u: U.Ui, sidebar: D.Node) -> D.Node:
  View.page.b.form(View.phone(u), lc, bv, u, sidebar)

def View.page.sidebar.node(reuse: Bool, +lc: I.Loc, u: U.Ui) -> D.Node:
  match reuse:
    case True{}: SS.Slot.placeholder()
    case False{}: View.sidebar(lc, u)

def View.page.b(+lc: I.Loc, +bv: BI.View, +u: U.Ui) -> D.Node:
  View.page.b.sidebar(lc, bv, u, View.sidebar(lc, u))

def View.page.signals.sidebar(+u: U.Ui, reuse: Bool) -> D.Node:
  View.page.b.sidebar(U.Lang.loc(u), BI.BU.view(u), u, View.page.sidebar.node(reuse, U.Lang.loc(u), u))

def View.page.sidebar.node.indexed(reuse: Bool, +lc: I.Loc, +u: U.Ui, signals: Map<&2, Nat>) -> D.Node:
  match reuse:
    case True{}: SS.Slot.placeholder()
    case False{}: View.sidebar.indexed(lc, u, U.Side.ix(U.Ui.state(u)), signals)

def View.page.signals.sidebar.indexed(+u: U.Ui, reuse: Bool, signals: Map<&2, Nat>) -> D.Node:
  View.page.b.sidebar(U.Lang.loc(u), BI.BU.view(u), u, View.page.sidebar.node.indexed(reuse, U.Lang.loc(u), u, signals))

# Server-only plain metadata projection; online/recipient signal decoration
# remains fresh, including reserved-info compatibility handling.
def View.page.sidebar.node.metadata(reuse: Bool, raw: Bool, +lc: I.Loc, +u: U.Ui, +plain: U.SIx, signals: Map<&2, Nat>) -> D.Node:
  match reuse raw:
    case True{} _: SS.Slot.placeholder()
    case False{} True{}: View.sidebar.indexed(lc, SR.Input.ui(u), plain, signals)
    case False{} False{}: View.sidebar.x(lc, WB.WB.side(lc, SR.Input.ui(u)), SR.Input.ui(u), plain)

def View.page.signals.sidebar.metadata(+u: U.Ui, reuse: Bool, raw: Bool, plain: U.SIx, signals: Map<&2, Nat>) -> D.Node:
  View.page.b.sidebar(U.Lang.loc(u), BI.BU.view(u), u, View.page.sidebar.node.metadata(reuse, raw, U.Lang.loc(u), u, plain, signals))

def View.page.sidebar(u: U.Ui, reuse: Bool) -> D.Node:
  View.page.signals.sidebar(U.Side.with_signals(u), reuse)

# the page in the device's language, looked up once here and passed down
def View.page.signals(+u: U.Ui) -> D.Node:
  View.page.b(U.Lang.loc(u), BI.BU.view(u), u)

def View.page(u: U.Ui) -> D.Node:
  View.page.signals(U.Side.with_signals(u))
