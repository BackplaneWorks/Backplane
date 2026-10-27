import Base
import ../core/bub-move.bend as HOP
import ../core/title.bend as TTL
import ../core/msgs.bend as MSG
import ../core/kexport.bend as KEX
import ../core/kihealth.bend as KH
import ../core/i18n.bend as I
import ../core/json.bend as J
import ../core/model.bend as M
import ../core/signal.bend as SG
import ../core/client.bend as U
import ../core/usage.bend as UZ
import ../core/usage_page.bend as UP
import ../core/usage_graph.bend as UG
import ../web/dom.bend as D
import ../web/view.bend as V
import ./notify.bend as N
import ../core/notice.bend as NO
import ./view.bend as VW
import ./picker.bend as PK
import ../core/todo.bend as TD
import ../core/attach.bend as AT
import ../core/images.bend as IM
import ../core/links.bend as LK
import ../core/fold.bend as FO
import ../core/subs.bend as SB
import ../core/diff.bend as DF
import ../core/vt.bend as VT
import ../core/codec.bend as C
import ../core/onboard.bend as OB
import ../core/botui.bend as B
import ./bots.bend as MB
import ../core/hist.bend as HS
import ../core/review.bend as RVW
import ../core/tools.bend as TO
import ../core/emb.bend as EM
import ../core/sched.bend as SCH
import ../core/tz.bend as TZ
import ../core/stamp.bend as STP
import ./permissions.bend as PM
import ../core/subthreads.bend as ST

# Ui -> the screen the phone apps draw, as JSON. SwiftUI and Compose map
# it onto native lists, toolbars and text fields; every decision (order,
# shelf, status, labels, markdown blocks) is made here or in core.

def Mob.st(u: U.Ui) -> M.State:
  match u:
    case U.Ui{st, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      st

def Mob.state(t: M.TurnState) -> String:
  match t:
    case M.Running{}:
      "run"
    case M.Failed{}:
      "fail"
    case M.Interrupted{}:
      "stop"
    case _:
      "idle"

# a swipe button: what it says, the action it sends with its value, its
# tone ("danger", "settle", ""), and choices (a snooze) whose values it
# sends instead
def Mob.swipe(label: String, action: String, value: String, tone: String, opts: List<&2, J.Json>) -> J.Json:
  J.Json.obj([J.KV{"label", J.Json.str(label)}, J.KV{"action", J.Json.str(action)}, J.KV{"value", J.Json.str(value)},
    J.KV{"tone", J.Json.str(tone)}, J.KV{"options", J.Json.arr(opts)}])

def Mob.opt(label: String, value: String) -> J.Json:
  J.Json.obj([J.KV{"label", J.Json.str(label)}, J.KV{"value", J.Json.str(value)}])

def Mob.snoozes.loc(+lc: I.Loc, +id: String) -> List<&2, J.Json>:
  [Mob.opt(MSG.T.common.hour(lc), id ++ "|1h"), Mob.opt(MSG.T.common.tomorrow(lc), id ++ "|tomorrow"), Mob.opt(MSG.T.common.week(lc), id ++ "|1w")]

# the swipe from the leading edge: settle an open thread, bring back a
# settled one, wake a snoozed one (shelf 0 open, 1 settled, 2 snoozed,
# 3 archived: none)

def Mob.snoozes(+id: String) -> List<&2, J.Json>:
  Mob.snoozes.loc(I.En{}, id)

def Mob.lead.loc(+lc: I.Loc, shelf: Nat, +id: String) -> List<&2, J.Json>:
  match shelf:
    case 0n:
      [Mob.swipe(MSG.T.menu.settle(lc), "row-settle", id, "settle", Nil{})]
    case 1n:
      [Mob.swipe(MSG.T.phone.unsettle(lc), "row-unsettle", id, "settle", Nil{})]
    case 2n:
      [Mob.swipe(MSG.T.menu.wake(lc), "row-snooze", id ++ "|off", "settle", Nil{})]
    case _:
      Nil{}

# the swipe from the trailing edge: delete (asks first), snooze

def Mob.lead(shelf: Nat, +id: String) -> List<&2, J.Json>:
  Mob.lead.loc(I.En{}, shelf, id)

def Mob.trail.loc(+lc: I.Loc, +id: String) -> List<&2, J.Json>:
  [Mob.swipe(MSG.T.common.delete(lc), "row-delete", id, "danger", Nil{}), Mob.swipe(MSG.T.tools.snooze(lc), "row-snooze", "", "", Mob.snoozes.loc(lc, id))]

# a row's time: since its last activity, or (snoozed) until it wakes

def Mob.trail(+id: String) -> List<&2, J.Json>:
  Mob.trail.loc(I.En{}, id)

def Mob.when.loc(+lc: I.Loc, shelf: Nat, +st: M.State, +id: String, +a: Nat, +now: Nat) -> String:
  match shelf:
    case 2n:
      I.I18n.ago(lc, U.Side.until(st, id), now)
    case _:
      I.I18n.ago(lc, now, a)

def Mob.when(shelf: Nat, +st: M.State, +id: String, +a: Nat, +now: Nat) -> String:
  Mob.when.loc(I.En{}, shelf, st, id, a, now)

def Mob.row(+lc: I.Loc, t: M.Thread, +status: String, +ix: U.SIx, +st: M.State, +now: Nat, +shelf: Nat) -> J.Json:
  match t:
    case M.Thread{+id, _, title, _, +a, turn, _, _, _, _, _, pinned, _, _, _, _, _, _, _, _, _}:
      J.Json.obj([J.KV{"id", J.Json.str(id)}, J.KV{"title", J.Json.str(U.Label.title(st, id, TTL.Title.shown(lc, title)))}, J.KV{"state", J.Json.str(Mob.state(turn))},
        J.KV{"ago", J.Json.str(Mob.when.loc(lc, shelf, st, id, a, now))}, J.KV{"agents", J.Json.str(V.View.sub.badge(lc, SB.Subs.count.ix(ix, st, id)))}, J.KV{"pinned", J.Json.bool(pinned)},
        J.KV{"status", J.Json.str(status)}, J.KV{"canChild", J.Json.bool(U.ThreadTree.can_route(st, id))}, J.KV{"lead", J.Json.arr(Mob.lead.loc(lc, shelf, id))}, J.KV{"trail", J.Json.arr(Mob.trail.loc(lc, id))}])

# Keep each thread's own swipe/age semantics even when its subtree is
# anchored on a different shelf.
def Mob.shelf(sh: M.Shelf) -> Nat:
  match sh:
    case M.Settled{}: 1n
    case M.Snoozed{}: 2n
    case M.Hidden{}: 3n
    case _: 0n

def Mob.tree.meta(j: J.Json, depth: Nat, expanded: Bool, children: Nat) -> J.Json:
  match j:
    case J.Obj{fs}:
      J.Obj{J.Field{"depth", J.Json.nat(depth), J.Field{"expanded", J.Json.bool(expanded), J.Field{"children", J.Json.nat(children), fs}}}}
    case x: x

def Mob.trow(+lc: I.Loc, r: ST.TRow, +ix: U.SIx, +st: M.State, +now: Nat) -> J.Json:
  match r:
    case ST.TRow{+t, d, e, c}:
      Mob.tree.meta(Mob.row(lc, t, SG.Signal.name(U.Side.signal.ix(ix, t)), ix, st, now,
        Mob.shelf(M.Shelf.at(t, U.Side.until(st, M.Thread.id(t)), now))), d, e, c)

def Mob.trows(rs: List<&2, ST.TRow>, +lc: I.Loc, +ix: U.SIx, +st: M.State, +now: Nat) -> List<&2, J.Json>:
  match rs:
    case Nil{}: Nil{}
    case r <> rest:
      Mob.trow(lc, r, ix, st, now) <> Mob.trows(rest, lc, ix, st, now)

def Mob.rows.selected(+lc: I.Loc, ts: List<&2, M.Thread>, +ix: U.SIx, +st: M.State, +now: Nat, selected: String) -> List<&2, J.Json>:
  Mob.trows(ST.Tree.rows(st, ts, selected), lc, ix, st, now)

# Compatibility for callers without a selected thread.
def Mob.rows(+lc: I.Loc, ts: List<&2, M.Thread>, +ix: U.SIx, +st: M.State, +now: Nat, shelf: Nat) -> List<&2, J.Json>:
  Mob.rows.selected(lc, ts, ix, st, now, "")

# History stays a flat list: Legacy.threads already includes all original
# conversations, including archived children. No synthetic project is made.
def Mob.legacy(ts: List<&2, M.Thread>, +lc: I.Loc, +ix: U.SIx, +st: M.State, +now: Nat) -> List<&2, J.Json>:
  match ts:
    case Nil{}: Nil{}
    case +t <> rest:
      Mob.tree.meta(Mob.row(lc, t, SG.Signal.name(U.Side.signal.ix(ix, t)), ix, st, now,
        Mob.shelf(M.Shelf.at(t, U.Side.until(st, M.Thread.id(t)), now))), 0n, False{}, 0n) <> Mob.legacy(rest, lc, ix, st, now)

# snoozed threads sit on their own shelf, soonest to wake first
def Mob.project(+lc: I.Loc, r: U.SRow, +open: Map<&2, Bool>, +ix: U.SIx, +st: M.State, +now: Nat, +selected: String) -> J.Json:
  match r:
    case U.SRow{_, M.Project{+id, title, root, _, _}, M.Shelves{pinned, active, +settled, hidden, snoozed}}:
      J.Json.obj([J.KV{"id", J.Json.str(id)}, J.KV{"title", J.Json.str(U.Label.title(st, id, title))}, J.KV{"root", J.Json.str(root)},
        J.KV{"open", J.Json.bool(U.Flag.get(open, id))},
        J.KV{"threads", J.Json.arr(Mob.rows.selected(lc, List.append(&2, M.Thread, pinned, active), ix, st, now, selected))},
        J.KV{"snoozedShelf", J.Json.str(MSG.T.sidebar.snoozed(lc))},
        J.KV{"snoozed", J.Json.arr(Mob.rows.selected(lc, snoozed, ix, st, now, selected))},
        J.KV{"shelf", J.Json.str(MSG.T.sidebar.settled.count(lc, Nat.show(List.length(&2, M.Thread, settled))))},
        J.KV{"settled", J.Json.arr(Mob.rows.selected(lc, settled, ix, st, now, selected))},
        J.KV{"value", J.Json.str("arch:" ++ id)}, J.KV{"archOpen", J.Json.bool(U.Flag.get(open, "arch:" ++ id))},
        J.KV{"archived", J.Json.arr(Mob.rows.selected(lc, M.Archive.of(hidden), ix, st, now, selected))}])

def Mob.quiet(p: J.Json, q: Bool) -> J.Json:
  match p:
    case J.Obj{fs}:
      J.Obj{J.Field{"quiet", J.Json.bool(q), fs}}
    case x:
      x

# a project elsewhere still catching up shows faded, as a quiet one does
def Mob.syncing(r: U.SRow, +st: M.State) -> Bool:
  match r:
    case U.SRow{_, M.Project{+id, _, _, _, _}, _}:
      U.Label.syncing(st, id)

def Mob.projects(+lc: I.Loc, rs: List<&2, U.SRow>, +open: Map<&2, Bool>, +ix: U.SIx, +st: M.State, +now: Nat, +q: Bool, +selected: String) -> List<&2, J.Json>:
  match rs:
    case Nil{}:
      Nil{}
    case +r <> t:
      Mob.quiet(Mob.project(lc, r, open, ix, st, now, selected), Bool.or(q, Mob.syncing(r, st))) <> Mob.projects(lc, t, open, ix, st, now, q, selected)

# the projects as the sidebar groups them (core's Side.groups): live ones,
# then the quiet ones ("quiet": shown faded), then the older ones, only
# while their shelf is open or a search is on
def Mob.older.k(+lc: I.Loc, show: Bool, rs: List<&2, U.SRow>, +open: Map<&2, Bool>, +ix: U.SIx, +st: M.State, +now: Nat, +selected: String) -> List<&2, J.Json>:
  match show:
    case True{}:
      Mob.projects(lc, rs, open, ix, st, now, True{}, selected)
    case False{}:
      Nil{}

def Mob.grouped(+lc: I.Loc, g: U.SGroups, +open: Map<&2, Bool>, +ix: U.SIx, +st: M.State, +now: Nat, +q: String, +selected: String) -> List<&2, J.Json>:
  match g:
    case U.SGroups{l, qs, o}:
      List.concat(&2, J.Json, [Mob.projects(lc, l, open, ix, st, now, False{}, selected), Mob.projects(lc, qs, open, ix, st, now, True{}, selected),
        Mob.older.k(lc, Bool.or(U.Flag.get(open, "old-projects"), Bool.not(String.is_empty(q))), o, open, ix, st, now, selected)])

# the folded shelf of older projects: how many and whether it is open
# ("side-older" flips it); null while there are none or a search shows them
def Mob.older.j(+n: Nat, +open: Bool, +q: String) -> J.Json:
  Bool.pick(J.Json, Bool.or(Nat.is_eq(n, 0n), Bool.not(String.is_empty(q))), J.Null{},
    J.Json.obj([J.KV{"count", J.Json.nat(n)}, J.KV{"open", J.Json.bool(open)}]))

def Mob.older(g: U.SGroups, +open: Map<&2, Bool>, +q: String) -> J.Json:
  match g:
    case U.SGroups{_, _, o}:
      Mob.older.j(List.length(&2, U.SRow, o), U.Flag.get(open, "old-projects"), q)

# the active view's rows (core's Active.rows): a thread row with its
# project's name, "faded" for one settled lately, and "urg"/"at", which
# order rows merged from several hubs the same way
def Mob.arow.k(j: J.Json, +urg: Nat, +at: Nat, +pt: String, +faded: Bool) -> J.Json:
  match j:
    case J.Obj{fs}:
      J.Obj{J.Field{"project", J.Json.str(pt), J.Field{"faded", J.Json.bool(faded), J.Field{"urg", J.Json.nat(urg), J.Field{"at", J.Json.nat(at), fs}}}}}
    case x:
      x

def Mob.arow(+lc: I.Loc, r: U.ARow, +ix: U.SIx, +st: M.State, +now: Nat) -> J.Json:
  match r:
    case U.ARow{+urg, +at, +t, _, +pt, +faded}:
      Mob.arow.k(Mob.row(lc, t, SG.Signal.name(U.Side.signal.ix(ix, t)), ix, st, now, Bool.pick(Nat, faded, 1n, 0n)), urg, at, pt, faded)

def Mob.aroots(rs: List<&2, U.ARow>) -> List<&2, M.Thread>:
  match rs:
    case Nil{}: Nil{}
    case U.ARow{_, _, t, _, _, _} <> rest:
      t <> Mob.aroots(rest)

def Mob.atrow(+lc: I.Loc, r: ST.TRow, anchor: U.ARow, +ix: U.SIx, +st: M.State, +now: Nat) -> J.Json:
  match anchor:
    case U.ARow{urg, at, _, _, pt, faded}:
      Mob.arow.k(Mob.trow(lc, r, ix, st, now), urg, at, pt, faded)

def Mob.trow.root(r: ST.TRow) -> Bool:
  match r:
    case ST.TRow{_, d, _, _}: Nat.is_eq(d, 0n)

# Tree.rows keeps the input roots in order. Advance the anchor only at a
# root, retaining it for every descendant until the next root arrives.
def Mob.anchor.step(root: Bool, anchors: List<&2, U.ARow>, fallback: U.ARow) -> List<&2, U.ARow> & U.ARow:
  match root anchors:
    case True{} a <> rest: (rest, a)
    case _ xs: (xs, fallback)

type MAnchor is Data:
  MAnchor{next: List<&2, U.ARow>, current: U.ARow}

def Mob.anchor.data(pair: List<&2, U.ARow> & U.ARow) -> MAnchor:
  (next, current) = pair
  MAnchor{next, current}

def Mob.anchor.next(step: MAnchor) -> List<&2, U.ARow>:
  match step:
    case MAnchor{next, _}: next

def Mob.anchor.current(step: MAnchor) -> U.ARow:
  match step:
    case MAnchor{_, current}: current

def Mob.atrows(rs: List<&2, ST.TRow>, +lc: I.Loc, anchors: List<&2, U.ARow>, anchor: U.ARow, +ix: U.SIx, +st: M.State, +now: Nat) -> List<&2, J.Json>:
  match rs:
    case Nil{}: Nil{}
    case +r <> rest:
      +step = Mob.anchor.data(Mob.anchor.step(Mob.trow.root(r), anchors, anchor))
      next = Mob.anchor.next(step)
      +a = Mob.anchor.current(step)
      Mob.atrow(lc, r, a, ix, st, now) <> Mob.atrows(rest, lc, next, a, ix, st, now)

# Active search reveals matching descendants without changing saved expansion.
def Mob.arows.search(+lc: I.Loc, rs: List<&2, U.ARow>, +ix: U.SIx, +st: M.State, +now: Nat, selected: String, q: String) -> List<&2, J.Json>:
  match rs:
    case Nil{}: Nil{}
    case +r <> +rest:
      Mob.atrows(ST.Tree.rows_search(st, Mob.aroots(r <> rest), selected, q), lc, r <> rest, r, ix, st, now)

def Mob.arows.selected(+lc: I.Loc, rs: List<&2, U.ARow>, +ix: U.SIx, +st: M.State, +now: Nat, selected: String) -> List<&2, J.Json>:
  Mob.arows.search(lc, rs, ix, st, now, selected, "")

def Mob.arows(+lc: I.Loc, rs: List<&2, U.ARow>, +ix: U.SIx, +st: M.State, +now: Nat) -> List<&2, J.Json>:
  Mob.arows.selected(lc, rs, ix, st, now, "")

# the settled rows only while their fold is open or a search is on
def Mob.active(+lc: I.Loc, on: Bool, +st: M.State, +now: Nat, +q: String, +o: Map<&2, Bool>) -> List<&2, J.Json>:
  match on:
    case True{}:
      Mob.arows.search(lc, U.Active.folded(U.Active.open(o, q), U.Active.parts(U.Active.rows(st, now, q))), U.Side.ix(st), st, now, "", q)
    case False{}:
      Nil{}

# the active view's fold of settled rows: how many and whether it is open
# ("side-settled" flips it); null while there are none or a search shows them
def Mob.settled(+st: M.State, +now: Nat, +q: String, +o: Map<&2, Bool>) -> J.Json:
  Mob.older.j(U.Active.faded_n(U.Active.parts(U.Active.rows(st, now, q))), U.Flag.get(o, "active-settled"), q)

# (only worked out while the active view is on)
def Mob.settled.if(on: Bool, +st: M.State, +now: Nat, +q: String, +o: Map<&2, Bool>) -> J.Json:
  match on:
    case True{}:
      Mob.settled(st, now, q, o)
    case False{}:
      J.Null{}

# the active view's rows and its fold, from the rows split once (a phone
# screen built them twice); nothing is worked out while it is off
type MSide is Data:
  MSide{rows: List<&2, J.Json>, settled: J.Json}

def Mob.side.k(+lc: I.Loc, +p: U.AParts, +ix: U.SIx, +st: M.State, +now: Nat, +q: String, +o: Map<&2, Bool>, +selected: String) -> MSide:
  MSide{Mob.arows.search(lc, U.Active.folded(U.Active.open(o, q), p), ix, st, now, selected, q), Mob.older.j(U.Active.faded_n(p), U.Flag.get(o, "active-settled"), q)}

def Mob.side(+lc: I.Loc, on: Bool, +ix: U.SIx, +st: M.State, +now: Nat, +q: String, +o: Map<&2, Bool>, selected: String) -> MSide:
  match on:
    case True{}:
      Mob.side.k(lc, U.Active.parts(U.Active.rows.ix(st, ix, now, q)), ix, st, now, q, o, selected)
    case False{}:
      MSide{Nil{}, J.Null{}}

def MSide.rows(m: MSide) -> List<&2, J.Json>:
  match m:
    case MSide{r, _}:
      r

def MSide.settled(m: MSide) -> J.Json:
  match m:
    case MSide{_, s}:
      s

# a row's or project's menu (core's Menu): its title and items, each
# sending its action with its value; null while none is open, or while a
# delete or remove it armed asks through "deleting"/"removing"
def Mob.mitems(xs: List<&2, U.MItem>) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case U.MItem{l, a, v, t} <> r:
      J.Json.obj([J.KV{"label", J.Json.str(l)}, J.KV{"action", J.Json.str(a)}, J.KV{"value", J.Json.str(v)}, J.KV{"tone", J.Json.str(t)}]) <> Mob.mitems(r)

def Mob.rmenu.k(+lc: I.Loc, none: Bool, +u: U.Ui) -> J.Json:
  match none:
    case True{}:
      J.Null{}
    case False{}:
      J.Json.obj([J.KV{"title", J.Json.str(U.Menu.title.loc(lc, u))}, J.KV{"kind", J.Json.str(U.Menu.kind(U.Menu.open(u)))}, J.KV{"items", J.Json.arr(Mob.mitems(U.Menu.items.loc(lc, u, False{})))}])

def Mob.kx.items(ls: List<&2, String>, +th: String) -> List<&2, J.Json>:
  match ls:
    case Nil{}: Nil{}
    case l <> t: J.Json.obj([J.KV{"label", J.Str{l}}, J.KV{"action", J.Str{"kicad-export10-read"}}, J.KV{"value", J.Str{th}}, J.KV{"tone", J.Str{""}}]) <> Mob.kx.items(t, th)

def Mob.kx.actions(as: List<&2, KH.KAct>) -> List<&2, J.Json>:
  match as:
    case Nil{}: Nil{}
    case KH.KAct{l, a, v} <> t: J.Json.obj([J.KV{"label", J.Str{l}}, J.KV{"action", J.Str{a}}, J.KV{"value", J.Str{v}}, J.KV{"tone", J.Str{""}}]) <> Mob.kx.actions(t)

def Mob.kx.menu(n: Maybe<&2, KH.KNote>) -> J.Json:
  match n:
    case None{}: J.Null{}
    case Some{KH.KNote{_, title, body, acts}}:
      J.Json.obj([J.KV{"title", J.Str{title}}, J.KV{"kind", J.Str{"kx"}}, J.KV{"items", J.Json.arr(List.append(&2, J.Json, Mob.kx.items(String.lines(body), ""), Mob.kx.actions(acts)))}])

def Mob.rmenu.export(pending: Bool, +lc: I.Loc, +u: U.Ui) -> J.Json:
  match pending:
    case True{}: Mob.kx.menu(U.KX.note(U.Ui.infos(u), U.Ui.sel(u)))
    case False{}: Mob.rmenu.k(lc, Bool.or(String.is_empty(U.Menu.open(u)), Bool.not(String.is_empty(U.Ui.scratch(u, "@del") ++ U.Ui.scratch(u, "@rm")))), u)

def Mob.rmenu(+lc: I.Loc, +u: U.Ui) -> J.Json:
  Mob.rmenu.export(U.KX.pending(U.Ui.infos(u), U.Ui.sel(u)), lc, u)

# markdown blocks from the same Md.render the web page uses; built onto a
# reversed accumulator so a long reply's many blocks cost no stack (only
# nesting does)
# a link's target, the attribute a phone needs, and whether it is a
# file's (a tap opens the file's menu: "file-menu" with href)
# an ordered item's number as written ("n")
def Mob.num(n: Bool, +v: String, rest: List<&2, J.KV>) -> List<&2, J.KV>:
  match n:
    case True{}:
      J.KV{"n", J.Json.str(v)} <> rest
    case False{}:
      rest

def Mob.href.attrs(as: List<&2, D.Attr>) -> List<&2, J.KV>:
  match as:
    case Nil{}:
      Nil{}
    case D.At{+k, +v} <> t:
      Bool.pick(List<&2, J.KV>, String.eq(k, "href"), [J.KV{"href", J.Json.str(v)}, J.KV{"file", J.Json.bool(Bool.not(LK.Link.is_web(v)))}], Mob.num(String.eq(k, "value"), v, Mob.href.attrs(t)))
    case _ <> t:
      Mob.href.attrs(t)

# An anchor without href already falls through to plain text on both phones.
def Mob.ref.uri(xs: List<&2, D.Attr>) -> String:
  match xs:
    case Nil{}: ""
    case D.At{"data-ref", v} <> _: v
    case _ <> t: Mob.ref.uri(t)

def Mob.ref.bare(xs: List<&2, D.Attr>) -> Bool:
  match xs:
    case Nil{}: False{}
    case D.At{"data-bare", v} <> _: String.eq(v, "1")
    case _ <> t: Mob.ref.bare(t)

def Mob.href.ref(ref: String, +attrs: List<&2, D.Attr>) -> List<&2, J.KV>:
  match ref:
    case SNil{}: Mob.href.attrs(attrs)
    case SCon{h, t}: [J.KV{"ref", J.Json.str(SCon{h, t})}, J.KV{"bare", J.Json.bool(Mob.ref.bare(attrs))}]

def Mob.href(+attrs: List<&2, D.Attr>) -> List<&2, J.KV>:
  Mob.href.ref(Mob.ref.uri(attrs), attrs)

def Mob.nodes.go(ns: List<&2, D.Node>, acc: List<&2, J.Json>) -> List<&2, J.Json>:
  match ns:
    case Nil{}:
      acc
    case D.El{tag, _, as, kids} <> t:
      Mob.nodes.go(t, J.Json.obj(J.KV{"tag", J.Json.str(tag)} <> J.KV{"kids", J.Json.arr(List.reverse(&2, J.Json, Mob.nodes.go(kids, Nil{})))} <> Mob.href(as)) <> acc)
    case D.Txt{text} <> t:
      Mob.nodes.go(t, J.Json.obj([J.KV{"text", J.Json.str(text)}]) <> acc)

def Mob.nodes(ns: List<&2, D.Node>) -> List<&2, J.Json>:
  List.reverse(&2, J.Json, Mob.nodes.go(ns, Nil{}))

# the Review tab's report as nodes, only while that tab is open
def Mob.review.loc(+lc: I.Loc, on: Bool, +u: U.Ui) -> J.Json:
  match on:
    case True{}:
      J.Json.arr(Mob.nodes(D.Md.render.loc(lc, U.RpView.md(U.Rp.view.loc(lc, u)))))
    case False{}:
      J.Null{}

# a chip for an attachment line: its name, path, whether it is an image,
# and where the hub serves it

def Mob.review(on: Bool, +u: U.Ui) -> J.Json:
  Mob.review.loc(I.En{}, on, u)

def Mob.chip(+p: String) -> J.Json:
  J.Json.obj([J.KV{"label", J.Json.str(AT.Att.label(p))}, J.KV{"path", J.Json.str(p)}, J.KV{"image", J.Json.bool(AT.Att.image(p))},
    J.KV{"url", J.Json.str(IM.Img.url(p))}])

def Mob.chips(ps: List<&2, String>) -> List<&2, J.Json>:
  match ps:
    case Nil{}:
      Nil{}
    case +p <> t:
      Mob.chip(p) <> Mob.chips(t)

def Mob.urls(ps: List<&2, String>) -> List<&2, J.Json>:
  match ps:
    case Nil{}:
      Nil{}
    case +p <> t:
      J.Json.obj([J.KV{"path", J.Json.str(p)}, J.KV{"url", J.Json.str(IM.Img.url(p))}]) <> Mob.urls(t)

# The images a reply names, found once when it arrives and kept in the
# scratch ("@img:<id>": "=" then the paths, a line each; the scratch, unlike
# the info map, outlives a reconnect's new log): finding them
# walks the text char by char, which a phone's QuickJS did for every reply
# on every screen, most of a thread screen's cost
def Mob.imgs.key(+id: String) -> String:
  "@img:" ++ id

def Mob.imgs.found(may: Bool, +text: String) -> List<&2, String>:
  match may:
    case True{}:
      IM.Img.paths(text)
    case False{}:
      Nil{}

# how many of the newest replies have their images found
def Mob.imgs.max() -> Nat:
  30n

def Mob.imgs.put(+id: String, +text: String, u: U.Ui) -> U.Ui:
  U.Ui.scratch_set(u, Mob.imgs.key(id), "=" ++ String.join(Mob.imgs.found(IM.Img.may(text), text), "\n"))

# the walk over entries, newest first: how many replies may still be
# looked at, and whether it is done (a reply looked at before: every older
# one was too, or is past the newest Mob.imgs.max())
type ImgW is Data:
  ImgW{k: Nat, done: Bool, u: U.Ui}

def Mob.imgs.reply(fresh: Bool, +k: Nat, +id: String, +text: String, u: U.Ui) -> ImgW:
  match fresh k:
    case True{} 1n+j:
      ImgW{j, False{}, Mob.imgs.put(id, text, u)}
    case _ _:
      ImgW{k, True{}, u}

def Mob.imgs.step(e: M.Entry, w: ImgW) -> ImgW:
  match e w:
    case M.Msg{+id, _, M.Assistant{}, +text, _} ImgW{+k, False{}, +u}:
      Mob.imgs.reply(String.is_empty(U.Ui.scratch(u, Mob.imgs.key(id))), k, id, text, u)
    case _ _:
      w

# (the walk stops once done: every frame asks, and a log holds thousands
# of entries)
def Mob.imgs.seen.go(es: List<&2, M.Entry>, w: ImgW) -> ImgW:
  match es w:
    case e <> t ImgW{+k, False{}, +u}:
      Mob.imgs.seen.go(t, Mob.imgs.step(e, ImgW{k, False{}, u}))
    case _ _:
      w

def Mob.imgs.ui(w: ImgW) -> U.Ui:
  match w:
    case ImgW{_, _, u}:
      u

# the newest replies the client holds have their images found, each once
# (entries come newest first)
def Mob.imgs.seen(+u: U.Ui) -> U.Ui:
  Mob.imgs.ui(Mob.imgs.seen.go(U.Ui.entries(U.Ui.state.of(u)), ImgW{Mob.imgs.max(), False{}, u}))

def Mob.imgs.list(+s: String) -> List<&2, String>:
  Bool.pick(List<&2, String>, String.is_empty(s), Nil{}, String.split(s, '\n'))

def Mob.imgs(+dr: Map<&2, String>, +id: String) -> List<&2, J.Json>:
  Mob.urls(Mob.imgs.list(String.drop(U.Str.get(dr, Mob.imgs.key(id)), 1n)))

# a terminal's row or any other activity's: matched, not Bool.pick (which
# built both, so every tool call's summary was cut as a terminal's title)
def Mob.act.k(term: Bool, +id: String, +th: String, +tone: String, +kind: String, +summary: String) -> J.Json:
  match term:
    case True{}:
      J.Json.obj([J.KV{"id", J.Json.str(id)}, J.KV{"kind", J.Json.str("emb")}, J.KV{"key", J.Json.str(EM.Emb.key(th, id))}, J.KV{"text", J.Json.str(EM.Emb.title(summary))}])
    case False{}:
      J.Json.obj([J.KV{"id", J.Json.str(id)}, J.KV{"kind", J.Json.str("act")}, J.KV{"tone", J.Json.str(tone)},
        J.KV{"label", J.Json.str(RVW.Review.label(tone, kind, summary))}, J.KV{"text", J.Json.str(RVW.Review.say(tone, kind, summary))}])

# a tool call or other activity; a design step (core/hist.bend) is a row
# that shows it in the viewer ("hist-show" with its value)
def Mob.act(step: Bool, e: M.Entry) -> J.Json:
  match step e:
    case True{} M.Act{+id, _, _, _, _, _}:
      J.Json.obj([J.KV{"id", J.Json.str(id)}, J.KV{"kind", J.Json.str("step")}, J.KV{"text", J.Json.str(HS.Hist.row(e))}, J.KV{"value", J.Json.str(id)}])
    case _ M.Act{+id, +th, +tone, +kind, +summary, _}:
      Mob.act.k(String.eq(tone, EM.Emb.tone()), id, th, tone, kind, summary)
    case _ M.Msg{+id, _, _, text, _}:
      J.Json.obj([J.KV{"id", J.Json.str(id)}, J.KV{"kind", J.Json.str("act")}, J.KV{"tone", J.Json.str("")}, J.KV{"label", J.Json.str("")}, J.KV{"text", J.Json.str(text)}])

# an entry of the thread itself: a user message is what was written and
# its attachments; a reply its markdown and the images it names
def Mob.own.s.loc(+lc: I.Loc, e: M.Entry, +dr: Map<&2, String>, +stamp: String) -> J.Json:
  match e:
    case M.Msg{id, _, M.User{}, +text, _}:
      J.Json.obj([J.KV{"id", J.Json.str(id)}, J.KV{"kind", J.Json.str("user")}, J.KV{"text", J.Json.str(AT.Att.body(text))},
        J.KV{"attachments", J.Json.arr(Mob.chips(AT.Att.paths(text)))}, J.KV{"stamp", J.Json.str(stamp)}])
    case M.Msg{+id, _, _, +text, _}:
      J.Json.obj([J.KV{"id", J.Json.str(id)}, J.KV{"kind", J.Json.str("assistant")}, J.KV{"text", J.Json.str(text)},
        J.KV{"blocks", J.Json.arr(Mob.nodes(D.Md.render.loc(lc, text)))}, J.KV{"images", J.Json.arr(Mob.imgs(dr, id))}, J.KV{"stamp", J.Json.str(stamp)}])
    case M.Act{+id, +th, +tone, +kind, +summary, +at}:
      Mob.act(String.eq(tone, HS.Step.tone()), M.Act{id, th, tone, kind, summary, at})

# with the time under a message (core/stamp.bend; "stamp", empty for the
# rest)

def Mob.own.s(e: M.Entry, +dr: Map<&2, String>, +stamp: String) -> J.Json:
  Mob.own.s.loc(I.En{}, e, dr, stamp)

def Mob.own(+lc: I.Loc, +e: M.Entry, +dr: Map<&2, String>, +sp: STP.Stamps) -> J.Json:
  Mob.own.s.loc(lc, e, dr, STP.Stamp.text.loc(lc, sp, e))

# a row that opens another thread (value: its id)
def Mob.link(+id: String, +text: String, +to: String) -> J.Json:
  J.Json.obj([J.KV{"id", J.Json.str(id)}, J.KV{"kind", J.Json.str("link")}, J.KV{"text", J.Json.str(text)}, J.KV{"value", J.Json.str(to)}])

def Mob.from(none: Bool, +id: String, +from: String, +st: M.State) -> List<&2, J.Json>:
  match none:
    case True{}:
      Nil{}
    case False{}:
      [Mob.link(id ++ ":from", "← from " ++ U.Tasks.title(st, from), from)]

# a reminder (a Reminded in the log): kind "reminder", its text and when,
# in the person's zone
def Mob.remind.is(e: M.Entry) -> Bool:
  match e:
    case M.Act{_, _, +tone, _, _, _}:
      String.eq(tone, "remind")
    case _:
      False{}

def Mob.remind(e: M.Entry, +st: M.State) -> J.Json:
  match e:
    case M.Act{+id, _, _, _, +text, +at}:
      J.Json.obj([J.KV{"id", J.Json.str(id)}, J.KV{"kind", J.Json.str("reminder")}, J.KV{"text", J.Json.str(text)},
        J.KV{"when", J.Json.str(TZ.Tz.show.nz(SCH.Sched.person(st, 0n, True{}), at))}])
    case _:
      J.Json.obj([J.KV{"kind", J.Json.str("act")}])

def Mob.entry.r(+lc: I.Loc, rem: Bool, +e: M.Entry, +st: M.State, +dr: Map<&2, String>, +sp: STP.Stamps) -> List<&2, J.Json>:
  match rem:
    case True{}:
      [Mob.remind(e, st)]
    case False{}:
      [Mob.own(lc, e, dr, sp)]

# a thought (model.bend's Think) shows only while the setting is on
def Mob.think.hide(e: M.Entry, +st: M.State) -> Bool:
  match e:
    case M.Act{_, _, +tone, _, _, _}:
      Bool.and(String.eq(tone, M.Think.tone()), Bool.not(M.Think.on(st)))
    case _:
      False{}

# what an entry shows as, as the web's View.entry.k: this thread's own
# (headed by the thread that sent it), or a message it sent to another
def Mob.entry.k(+lc: I.Loc, mine: Bool, +e: M.Entry, +st: M.State, +dr: Map<&2, String>, +sp: STP.Stamps) -> List<&2, J.Json>:
  match mine e:
    case True{} M.Msg{+id, t, r, x, at}:
      List.append(&2, J.Json, Mob.from(String.is_empty(M.Link.from(id)), id, M.Link.from(id), st), [Mob.own(lc, M.Msg{id, t, r, x, at}, dr, sp)])
    case True{} _:
      Bool.pick(List<&2, J.Json>, Mob.think.hide(e, st), Nil{}, Mob.entry.r(lc, Mob.remind.is(e), e, st, dr, sp))
    case False{} M.Msg{+id, +t, _, x, _}:
      [Mob.link(id, "→ " ++ U.Tasks.title(st, t) ++ ": " ++ DF.Diff.first(x), t)]
    case False{} _:
      Nil{}

def Mob.entry(+lc: I.Loc, +e: M.Entry, +st: M.State, +sel: String, +dr: Map<&2, String>, +sp: STP.Stamps) -> List<&2, J.Json>:
  Mob.entry.k(lc, String.eq(V.View.entry_thread(e), sel), e, st, dr, sp)

# a run of tool calls as one row; "fold" with its value opens or shuts it
def Mob.fold(+id: String, +label: String, +open: Bool) -> J.Json:
  J.Json.obj([J.KV{"id", J.Json.str("fold:" ++ id)}, J.KV{"kind", J.Json.str("fold")}, J.KV{"text", J.Json.str(label)},
    J.KV{"open", J.Json.bool(open)}, J.KV{"value", J.Json.str(FO.Fold.ask(open, id))}])

# xs pushed onto a reversed accumulator
def Mob.onto(xs: List<&2, J.Json>, acc: List<&2, J.Json>) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      acc
    case x <> t:
      Mob.onto(t, x <> acc)

def Mob.shown(open: Bool, xs: List<&2, J.Json>, acc: List<&2, J.Json>) -> List<&2, J.Json>:
  match open:
    case True{}:
      Mob.onto(xs, acc)
    case False{}:
      acc

# an entry's rows as its fold role says
def Mob.role(r: FO.Role, xs: List<&2, J.Json>, acc: List<&2, J.Json>) -> List<&2, J.Json>:
  match r:
    case FO.RShow{}:
      Mob.onto(xs, acc)
    case FO.RHead{+id, +label, +open}:
      Mob.shown(open, xs, Mob.fold(id, label, open) <> acc)
    case FO.RIn{open}:
      Mob.shown(open, xs, acc)
    case FO.RWork{+id, +label, open, inner}:
      match open:
        case True{}:
          Mob.role(inner, xs, Mob.fold(id, label, True{}) <> acc)
        case False{}:
          Mob.fold(id, label, False{}) <> acc

# a thread's entries, tail-recursive (a thread can hold thousands)
def Mob.entries.go(+lc: I.Loc, es: List<&2, M.Entry>, rs: List<&2, FO.Role>, +st: M.State, +sel: String, +dr: Map<&2, String>, +sp: STP.Stamps, acc: List<&2, J.Json>) -> List<&2, J.Json>:
  match es rs:
    case Nil{} _:
      acc
    case e <> t r <> rt:
      Mob.entries.go(lc, t, rt, st, sel, dr, sp, Mob.role(r, Mob.entry(lc, e, st, sel, dr, sp), acc))
    case e <> t Nil{}:
      Mob.entries.go(lc, t, Nil{}, st, sel, dr, sp, Mob.onto(Mob.entry(lc, e, st, sel, dr, sp), acc))

# oldest first, runs of tool calls folded (the last stays open while its
# turn runs); a reply's images are the ones found when it came
# (Mob.imgs.seen)
def Mob.entries(+lc: I.Loc, +es: List<&2, M.Entry>, +st: M.State, +sel: String, +info: Map<&2, String>, +dr: Map<&2, String>, +running: Bool) -> List<&2, J.Json>:
  List.reverse(&2, J.Json, Mob.entries.go(lc, es, FO.Fold.roles.loc(lc, es, info, running), st, sel, dr, STP.Stamps.of.s(es, st, FO.Fold.now(info), running, STP.Stamp.stopped(U.Ui.thread(st, sel))), Nil{}))

# a toolbar button (core/tools.bend): its icon, what it says, what it does
# (tip) and the action it sends
def Mob.tool(t: TO.Tool) -> J.Json:
  match t:
    case TO.Tool{i, l, tip, a, v, on, dg, _}:
      J.Json.obj([J.KV{"icon", J.Json.str(i)}, J.KV{"label", J.Json.str(l)}, J.KV{"tip", J.Json.str(tip)}, J.KV{"action", J.Json.str(a)},
        J.KV{"value", J.Json.str(v)}, J.KV{"on", J.Json.bool(on)}, J.KV{"danger", J.Json.bool(dg)}])

def Mob.tools(ts: List<&2, TO.Tool>) -> List<&2, J.Json>:
  match ts:
    case Nil{}:
      Nil{}
    case t <> r:
      Mob.tool(t) <> Mob.tools(r)

# the toolbar's: stop, then under the menu where it sits and forking
def Mob.head.loc(+lc: I.Loc, running: Bool, pinned: Bool, settled: Bool, archived: Bool, forked: Bool) -> List<&2, J.Json>:
  Mob.tools(List.concat(&2, TO.Tool, [TO.Tools.stop.loc(lc, running), [TO.Tools.pin.loc(lc, pinned), TO.Tools.settle.loc(lc, settled), TO.Tools.archive.loc(lc, archived), TO.Tools.fork.loc(lc)], TO.Tools.back.loc(lc, forked)]))

def Mob.head(running: Bool, pinned: Bool, settled: Bool, archived: Bool, forked: Bool) -> List<&2, J.Json>:
  Mob.head.loc(I.En{}, running, pinned, settled, archived, forked)

def Mob.live.loc(+lc: I.Loc, empty: Bool, running: Bool) -> String:
  match empty running:
    case True{} True{}:
      MSG.T.phone.working(lc)
    case _ _:
      ""

def Mob.live(empty: Bool, running: Bool) -> String:
  Mob.live.loc(I.En{}, empty, running)

def Mob.strs(xs: List<&2, String>) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      J.Json.str(x) <> Mob.strs(t)

# a choice of a menu: what it says, the value it sends, whether it is on
def Mob.choice(label: String, value: String, on: Bool) -> J.Json:
  J.Json.obj([J.KV{"label", J.Json.str(label)}, J.KV{"value", J.Json.str(value)}, J.KV{"on", J.Json.bool(on)}])

# a model's name in the menu: the CLI's label for it ("Opus 5.5 ·
# claude-opus-5-5"), else its name. The picker reads the models info
# parsed once (parsing its text for each model, twice, was most of what
# a thread's screen cost on a phone's JS engine)
def Mob.label.k.loc(+lc: I.Loc, +l: String, +m: String) -> String:
  Bool.pick(String, String.is_empty(l), Bool.pick(String, String.is_empty(m), MSG.T.common.default(lc), m), l)

def Mob.label.k(+l: String, +m: String) -> String:
  Mob.label.k.loc(I.En{}, l, m)

def Mob.model_name.loc(+lc: I.Loc, +about: J.Json, +m: String) -> String:
  Mob.label.k.loc(lc, J.Json.get_str(J.Json.get_obj(about, "labels"), Bool.pick(String, String.is_empty(m), "default", m), ""), m)

def Mob.model_name(+about: J.Json, +m: String) -> String:
  Mob.model_name.loc(I.En{}, about, m)

def Mob.models.loc(+lc: I.Loc, ms: List<&2, String>, +about: J.Json, +cur: String) -> List<&2, J.Json>:
  match ms:
    case Nil{}:
      Nil{}
    case +m <> t:
      Mob.choice(Mob.model_name.loc(lc, about, m), m, String.eq(m, cur)) <> Mob.models.loc(lc, t, about, cur)

def Mob.models(ms: List<&2, String>, +about: J.Json, +cur: String) -> List<&2, J.Json>:
  Mob.models.loc(I.En{}, ms, about, cur)

def Mob.efforts.loc(+lc: I.Loc, es: List<&2, String>, +cur: String, +prov: String) -> List<&2, J.Json>:
  match es:
    case Nil{}:
      Nil{}
    case +e <> t:
      Mob.choice(V.VLabel.chip(lc, e), e, U.Chip.on(e, cur, prov)) <> Mob.efforts.loc(lc, t, cur, prov)

def Mob.efforts(es: List<&2, String>, +cur: String, +prov: String) -> List<&2, J.Json>:
  Mob.efforts.loc(I.En{}, es, cur, prov)

# the composer's model chip and its menu: the thread's provider's models
# (its default first; "model" sends one) and the efforts that model takes
# ("effort")
# (j: the models info, parsed; about: its part for the provider)
def Mob.picker.j.loc(+lc: I.Loc, +j: J.Json, +about: J.Json, +provider: String, +model: String, +effort: String, +ctx: String) -> J.Json:
  J.Json.obj([J.KV{"label", J.Json.str(V.VLabel.effort.label(lc, V.VLabel.model(lc, provider, model), effort) ++ ctx)},
    J.KV{"models", J.Json.arr(Mob.models.loc(lc, "" <> U.Models.strs(J.Json.elems(J.Json.get_obj(j, provider))), about, model))},
    J.KV{"efforts", J.Json.arr(Mob.efforts.loc(lc, U.Effort.chips.of(U.Effort.known(J.Json.get(J.Json.get_obj(about, "efforts"), Bool.pick(String, String.is_empty(model), "default", model)), provider)), effort, provider))},
    J.KV{"providers", J.Json.arr(Mob.efforts.loc(lc, U.Chip.providers(), effort, provider))}])

def Mob.picker.j(+j: J.Json, +about: J.Json, +provider: String, +model: String, +effort: String, +ctx: String) -> J.Json:
  Mob.picker.j.loc(I.En{}, j, about, provider, model, effort, ctx)

def Mob.picker.p.loc(+lc: I.Loc, +j: J.Json, +provider: String, +model: String, +effort: String, +ctx: String) -> J.Json:
  Mob.picker.j.loc(lc, j, J.Json.get_obj(J.Json.get_obj(j, "about"), provider), provider, model, effort, ctx)

def Mob.picker.p(+j: J.Json, +provider: String, +model: String, +effort: String, +ctx: String) -> J.Json:
  Mob.picker.p.loc(I.En{}, j, provider, model, effort, ctx)

def Mob.picker.ctx.loc(+lc: I.Loc, +info: String, +provider: String, +model: String, +effort: String, +ctx: String) -> J.Json:
  Mob.picker.p.loc(lc, Maybe.default(&2, J.Json, J.Json.parse(info), J.Null{}), provider, model, effort, ctx)

def Mob.picker.ctx(+info: String, +provider: String, +model: String, +effort: String, +ctx: String) -> J.Json:
  Mob.picker.ctx.loc(I.En{}, info, provider, model, effort, ctx)

def Mob.picker.loc(+lc: I.Loc, +info: String, +provider: String, +model: String, +effort: String) -> J.Json:
  Mob.picker.ctx.loc(lc, info, provider, model, effort, "")

def Mob.picker(+info: String, +provider: String, +model: String, +effort: String) -> J.Json:
  Mob.picker.loc(I.En{}, info, provider, model, effort)

def Mob.qbtns(bs: List<&2, U.QBtn>) -> List<&2, J.Json>:
  match bs:
    case Nil{}:
      Nil{}
    case U.QBtn{l, a, v} <> t:
      J.Json.obj([J.KV{"label", J.Json.str(l)}, J.KV{"action", J.Json.str(a)}, J.KV{"value", J.Json.str(v)}]) <> Mob.qbtns(t)

# context handed over, above the composer (M.Queue.context), with its
# buttons (U.Qu.row_buttons)
def Mob.queue(+lc: I.Loc, qs: List<&2, M.QItem>, +first: Bool, +run: Bool) -> List<&2, J.Json>:
  match qs:
    case Nil{}:
      Nil{}
    case M.QItem{+msg, +text} <> t:
      J.Json.obj([J.KV{"msg", J.Json.str(msg)}, J.KV{"text", J.Json.str(U.Qu.label(lc, msg, text))}, J.KV{"tag", J.Json.str(U.Qu.tag(lc, run, msg))},
        J.KV{"buttons", J.Json.arr(Mob.qbtns(U.Qu.row_buttons(lc, run, first, msg)))}]) <> Mob.queue(lc, t, False{}, run)

def Mob.tdlines(rs: List<&2, TD.TRow>) -> List<&2, J.Json>:
  match rs:
    case Nil{}:
      Nil{}
    case +r <> rest:
      J.Json.obj([J.KV{"text", J.Json.str(TD.TRow.text(r))}, J.KV{"status", J.Json.str(TD.TRow.status(r))}]) <> Mob.tdlines(rest)

# the agent's todo list (U.Todos.shown), or null while hidden: its head,
# whether it is open and what a tap on the head sends (fold, with toggle),
# its lines (every step once shown all: the app scrolls them), and a long
# list's last line ("Show all 24", "Show fewer"; "" when it fits) and its value
def Mob.todos.k(+lc: I.Loc, +open: Bool, +all: Bool, +th: String, +ts: List<&2, TD.Todo>, +info: Map<&2, String>) -> J.Json:
  J.Json.obj([J.KV{"head", J.Json.str(TD.Todo.head(ts))}, J.KV{"open", J.Json.bool(open)}, J.KV{"toggle", J.Json.str(U.Todos.toggle(open, th))},
    J.KV{"lines", J.Json.arr(Mob.tdlines(U.Todos.rows(info, th, U.Todos.cap.all(), ts)))},
    J.KV{"more", J.Json.str(Bool.pick(String, open, U.Todos.more(lc, all, ts), ""))}, J.KV{"moreValue", J.Json.str(U.Todos.more.ask(all, th))}])

def Mob.todos(+lc: I.Loc, +info: Map<&2, String>, +th: String, +ts: List<&2, TD.Todo>) -> J.Json:
  Bool.pick(J.Json, List.is_empty(&2, TD.Todo, ts), J.Null{}, Mob.todos.k(lc, U.Todos.open(info, th), U.Todos.all(info, th), th, ts, info))

# Asks: what the agent waits on the user for. Each button sends "answer"
# with its value; "primary" marks the one to lead with
def Mob.btn(+label: String, +value: String, primary: Bool) -> J.Json:
  J.Json.obj([J.KV{"label", J.Json.str(label)}, J.KV{"value", J.Json.str(value)}, J.KV{"primary", J.Json.bool(primary)}])

def Mob.ask.opts(ls: List<&2, String>, +id: String) -> List<&2, J.Json>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      Mob.btn(l, id ++ "|" ++ l, False{}) <> Mob.ask.opts(t, id)

def Mob.ask.draft.loc(+lc: I.Loc, empty: Bool, +id: String, +draft: String) -> J.Json:
  match empty:
    case True{}:
      Mob.btn(MSG.T.approval.dismiss(lc), id ++ "|", False{})
    case False{}:
      Mob.btn(MSG.T.approval.draft(lc), id ++ "|" ++ String.trim(draft), True{})

def Mob.ask.draft(empty: Bool, +id: String, +draft: String) -> J.Json:
  Mob.ask.draft.loc(I.En{}, empty, id, draft)

def Mob.ask.btns.loc(+lc: I.Loc, i: Nat, +id: String, +o: String, +draft: String) -> List<&2, J.Json>:
  match i:
    case 0n:
      [Mob.btn(MSG.T.approval.implement(lc), id ++ "|implement", True{}), Mob.btn(MSG.T.approval.dismiss(lc), id ++ "|dismiss", False{})]
    case 1n:
      List.append(&2, J.Json, Mob.ask.opts(String.split(o, '\n'), id), [Mob.ask.draft.loc(lc, String.is_empty(String.trim(draft)), id, draft)])
    case 2n:
      [Mob.btn(MSG.T.composer.send(lc), id ++ "|accept", True{}), Mob.btn(MSG.T.ask.notnow(lc), id ++ "|decline", False{})]

    case _:
      [Mob.btn(MSG.T.approval.allow(lc), id ++ "|accept", True{}), Mob.btn(MSG.T.approval.session(lc), id ++ "|session", False{}), Mob.btn(MSG.T.approval.deny(lc), id ++ "|decline", False{})]

def Mob.ask.btns(i: Nat, +id: String, +o: String, +draft: String) -> List<&2, J.Json>:
  Mob.ask.btns.loc(I.En{}, i, id, o, draft)

def Mob.ask.body.loc(+lc: I.Loc, plan: Bool, +d: String) -> List<&2, J.Json>:
  match plan:
    case True{}:
      Mob.nodes(D.Md.render.loc(lc, d))
    case False{}:
      Nil{}

def Mob.ask.body(plan: Bool, +d: String) -> List<&2, J.Json>:
  Mob.ask.body.loc(I.En{}, plan, d)

def Mob.ask.std.loc(+lc: I.Loc, a: M.Ask, +draft: String) -> J.Json:
  match a:
    case M.Ask{+id, _, +k, +tool, +d, +o, _, _}:
      J.Json.obj([J.KV{"id", J.Json.str(id)}, J.KV{"kind", J.Json.str(k)},
        J.KV{"head", J.Json.str(U.Ask.head.loc(lc, k, tool, d))},
        J.KV{"detail", J.Json.str(Bool.pick(String, String.eq(k, "plan"), "", d))},
        J.KV{"blocks", J.Json.arr(Mob.ask.body.loc(lc, String.eq(k, "plan"), d))},
        J.KV{"buttons", J.Json.arr(Mob.ask.btns.loc(lc, C.Codec.pos(["plan", "input", "send"], U.Ask.bkind(k, tool)), id, o, draft))}])

def Mob.ask.std(a: M.Ask, draft: String) -> J.Json:
  Mob.ask.std.loc(I.En{}, a, draft)

def Mob.ask.k.loc(+lc: I.Loc, web: Bool, a: M.Ask, +draft: String, nf: Map<&2, String>) -> J.Json:
  match web:
    case True{}:
      MB.MBots.ask.loc(lc, U.Teach.ask.from.loc(lc, a, nf))
    case False{}:
      Mob.ask.std.loc(lc, a, draft)

def Mob.ask.k(web: Bool, a: M.Ask, draft: String, nf: Map<&2, String>) -> J.Json:
  Mob.ask.k.loc(I.En{}, web, a, draft, nf)

def Mob.ask(+lc: I.Loc, +a: M.Ask, +draft: String, nf: Map<&2, String>) -> J.Json:
  Mob.ask.k.loc(lc, String.eq(MB.MBots.ask.kind(a), "browser"), a, draft, nf)

def Mob.asks(+lc: I.Loc, as: List<&2, M.Ask>, +draft: String, +nf: Map<&2, String>) -> List<&2, J.Json>:
  match as:
    case Nil{}:
      Nil{}
    case a <> t:
      Mob.ask(lc, a, draft, nf) <> Mob.asks(lc, t, draft, nf)

# Subagents (core/subs.bend): what this thread delegated ("select" with
# the row's key opens its thread) and the agent's own ("fold" with its
# value opens or shuts its steps), and the thread that delegated this one
def Mob.sub.step(s: SB.SubStep) -> J.Json:
  match s:
    case SB.SubStep{+kind, +text}:
      J.Json.obj([J.KV{"kind", J.Json.str(SB.Subs.step.label(kind))}, J.KV{"text", J.Json.str(text)}])

def Mob.sub.steps(ss: List<&2, SB.SubStep>) -> List<&2, J.Json>:
  match ss:
    case Nil{}:
      Nil{}
    case s <> t:
      Mob.sub.step(s) <> Mob.sub.steps(t)

# a row's state as shown, beside its token "state" (English adds nothing:
# the apps show the token when there is no label)
def Mob.sub.label(lc: I.Loc, +word: String) -> List<&2, J.KV>:
  match lc:
    case I.En{}: Nil{}
    case I.Ja{}: [J.KV{"stateLabel", J.Json.str(U.StateWord.label(I.Ja{}, word))}]

def Mob.sub.loc(+lc: I.Loc, r: SB.SubRow) -> J.Json:
  match r:
    case SB.SubRow{+key, +agent, +title, +who, +word, +doing, +live, +open, n, ss}:
      J.Json.obj(List.append(&2, J.KV, Mob.sub.label(lc, word), [J.KV{"id", J.Json.str(key)}, J.KV{"agent", J.Json.bool(agent)}, J.KV{"title", J.Json.str(TTL.Title.shown(lc, title))}, J.KV{"who", J.Json.str(who)},
        J.KV{"state", J.Json.str(word)}, J.KV{"doing", J.Json.str(doing)}, J.KV{"live", J.Json.bool(live)}, J.KV{"open", J.Json.bool(open)},
        J.KV{"n", J.Json.nat(n)}, J.KV{"steps", J.Json.arr(Mob.sub.steps(ss))},
        J.KV{"act", J.Json.str(Bool.pick(String, agent, "fold", "select"))}, J.KV{"value", J.Json.str(Bool.pick(String, agent, SB.Subs.ask(open, key), key))},
        J.KV{"cancel", J.Json.str(SB.Subs.cancel(SB.SubRow{key, agent, title, who, word, doing, live, open, 0n, Nil{}}))}]))

def Mob.sub(r: SB.SubRow) -> J.Json:
  Mob.sub.loc(I.En{}, r)

def Mob.subs.go(+lc: I.Loc, rs: List<&2, SB.SubRow>) -> List<&2, J.Json>:
  match rs:
    case Nil{}:
      Nil{}
    case r <> rest:
      Mob.sub.loc(lc, r) <> Mob.subs.go(lc, rest)

# the panel: how many are at work ("" with none), its word ("3 at work",
# "3 done"), whether it is open, the head's tap (the "fold" action with
# value), and its rows, none while shut (subs.bend's panel: done, it folds)
def Mob.subs.k(+lc: I.Loc, +open: Bool, +th: String, +rs: List<&2, SB.SubRow>) -> J.Json:
  J.Json.obj([J.KV{"busy", J.Json.str(V.View.sub.busy(lc, SB.Subs.live(rs)))}, J.KV{"word", J.Json.str(V.View.sub.panel(lc, rs))},
    J.KV{"open", J.Json.bool(open)}, J.KV{"act", J.Json.str(SB.Subs.panel.action(open))}, J.KV{"value", J.Json.str(SB.Subs.panel.ask(open, th, rs))},
    J.KV{"rows", J.Json.arr(Mob.subs.go(lc, SB.Subs.panel.rows(open, rs)))}])

def Mob.sub.signal.fields(fs: J.Json, +signal: String) -> J.Json:
  match fs:
    case J.Field{"stateLabel", _, rest}: Mob.sub.signal.fields(rest, signal)
    case J.Field{"state", _, rest}: J.Field{"state", J.Json.str(signal), Mob.sub.signal.fields(rest, signal)}
    case J.Field{k, v, rest}: J.Field{k, v, Mob.sub.signal.fields(rest, signal)}
    case x: x

def Mob.sub.signal.obj(j: J.Json, label: String, signal: String) -> J.Json:
  match j:
    case J.Obj{fs}: J.Obj{J.Field{"stateLabel", J.Json.str(label), Mob.sub.signal.fields(fs, signal)}}
    case x: x

def Mob.sub.signals.loc(+lc: I.Loc, +r: SB.SubRow, +info: Map<&2, String>) -> J.Json:
  match r:
    case SB.SubRow{_, _, _, _, word, _, _, _, _, _}:
      Mob.sub.signal.obj(Mob.sub.loc(lc, r), U.StateWord.label(lc, word), SB.Subs.signal.of(info, r))

def Mob.subs.signals.go(+lc: I.Loc, rs: List<&2, SB.SubRow>, +info: Map<&2, String>) -> List<&2, J.Json>:
  match rs:
    case Nil{}: Nil{}
    case r <> rest: Mob.sub.signals.loc(lc, r, info) <> Mob.subs.signals.go(lc, rest, info)

def Mob.subs.signal.k(+lc: I.Loc, +open: Bool, +th: String, +rs: List<&2, SB.SubRow>, +info: Map<&2, String>) -> J.Json:
  J.Json.obj([J.KV{"busy", J.Json.str(V.View.sub.busy(lc, SB.Subs.active(rs, info)))}, J.KV{"word", J.Json.str(V.View.sub.signal.panel(lc, rs, info))},
    J.KV{"open", J.Json.bool(open)}, J.KV{"act", J.Json.str(SB.Subs.panel.action(open))}, J.KV{"value", J.Json.str(SB.Subs.panel.ask(open, th, rs))},
    J.KV{"rows", J.Json.arr(Mob.subs.signals.go(lc, SB.Subs.panel.rows(open, rs), info))}])

def Mob.subs.loc(+lc: I.Loc, +info: Map<&2, String>, +th: String, +rs: List<&2, SB.SubRow>) -> J.Json:
  Bool.pick(J.Json, List.is_empty(&2, SB.SubRow, rs), J.Null{}, Mob.subs.signal.k(lc, SB.Subs.panel.open(info, th, rs), th, rs, info))

def Mob.subs(info: Map<&2, String>, th: String, rs: List<&2, SB.SubRow>) -> J.Json:
  Mob.subs.loc(I.En{}, info, th, rs)

def Mob.parent(none: Bool, +st: M.State, +thread: String, +p: String) -> J.Json:
  match none:
    case True{}:
      J.Null{}
    case False{}:
      Mob.link(p, U.Tasks.up.label(st, thread, p), p)

# the skills a `$` being typed may complete to ("skill" with the name)
def Mob.skills(rs: List<&2, U.SkillRow>) -> List<&2, J.Json>:
  match rs:
    case Nil{}:
      Nil{}
    case U.SkillRow{n, d, on} <> t:
      J.Json.obj([J.KV{"name", J.Json.str(n)}, J.KV{"desc", J.Json.str(d)}, J.KV{"on", J.Json.bool(on)}]) <> Mob.skills(t)

# Diff: what the thread changed, file by file, lines on a reversed
# accumulator (a diff can run to thousands of lines)
def Mob.dline(l: DF.DLine) -> J.Json:
  match l:
    case DF.DLine{+k, t, +o, +n}:
      J.Json.obj([J.KV{"k", J.Json.u32(k)}, J.KV{"t", J.Json.str(t)}, J.KV{"o", J.Json.str(V.View.dl.no(o))}, J.KV{"n", J.Json.str(V.View.dl.no(n))}])

def Mob.dlines(ls: List<&2, DF.DLine>, acc: List<&2, J.Json>) -> List<&2, J.Json>:
  match ls:
    case Nil{}:
      acc
    case l <> t:
      Mob.dlines(t, Mob.dline(l) <> acc)

def Mob.dhead(empty: Bool, +hd: String, acc: List<&2, J.Json>) -> List<&2, J.Json>:
  match empty:
    case True{}:
      acc
    case False{}:
      J.Json.obj([J.KV{"k", J.Json.u32(4)}, J.KV{"t", J.Json.str(hd)}, J.KV{"o", J.Json.str("")}, J.KV{"n", J.Json.str("")}]) <> acc

def Mob.dhunks(hs: List<&2, DF.DHunk>, acc: List<&2, J.Json>) -> List<&2, J.Json>:
  match hs:
    case Nil{}:
      acc
    case DF.DHunk{+hd, ls} <> t:
      Mob.dhunks(t, Mob.dlines(ls, Mob.dhead(String.is_empty(hd), hd, acc)))

def Mob.dfiles(fs: List<&2, DF.DFile>) -> List<&2, J.Json>:
  match fs:
    case Nil{}:
      Nil{}
    case DF.DFile{_, +nw, st, hs} <> t:
      J.Json.obj([J.KV{"name", J.Json.str(nw)}, J.KV{"status", J.Json.str(st)},
        J.KV{"lines", J.Json.arr(List.reverse(&2, J.Json, Mob.dhunks(hs, Nil{})))}]) <> Mob.dfiles(t)

def Mob.diff.of.loc(+lc: I.Loc, +fs: List<&2, DF.DFile>) -> J.Json:
  J.Json.obj([J.KV{"summary", J.Json.str(Bool.pick(String, List.is_empty(&2, DF.DFile, fs), MSG.T.viewer.diff.empty(lc), DF.Diff.summary(fs)))},
    J.KV{"files", J.Json.arr(Mob.dfiles(fs))}])

# shown once the hub has answered "diff", until "diff-close"

def Mob.diff.of(+fs: List<&2, DF.DFile>) -> J.Json:
  Mob.diff.of.loc(I.En{}, fs)

def Mob.diff.loc(+lc: I.Loc, open: Bool, +text: String) -> J.Json:
  match open:
    case True{}:
      Mob.diff.of.loc(lc, DF.Diff.parse(text))
    case False{}:
      J.Null{}

# Terminal: the thread's shell as rows of styled runs (colours 0xRRGGBB)

def Mob.diff(open: Bool, +text: String) -> J.Json:
  Mob.diff.loc(I.En{}, open, text)

def Mob.runs(rs: List<&2, VT.VRun>) -> List<&2, J.Json>:
  match rs:
    case Nil{}:
      Nil{}
    case VT.VRun{t, +fg, +bg, bo, ul} <> rest:
      J.Json.obj([J.KV{"t", J.Json.str(t)}, J.KV{"fg", J.Json.u32(fg)}, J.KV{"bg", J.Json.u32(bg)}, J.KV{"b", J.Json.bool(bo)}, J.KV{"u", J.Json.bool(ul)}]) <> Mob.runs(rest)

def Mob.tlines(ls: List<&2, VT.VLine>) -> List<&2, J.Json>:
  match ls:
    case Nil{}:
      Nil{}
    case VT.VLine{rs} <> t:
      J.Json.arr(Mob.runs(rs)) <> Mob.tlines(t)

def Mob.cursor(c: VT.VCur) -> J.Json:
  match c:
    case VT.VCur{x, y, on}:
      J.Json.obj([J.KV{"x", J.Json.u32(x)}, J.KV{"y", J.Json.u32(y)}, J.KV{"on", J.Json.bool(on)}])

def Mob.term.of(+v: VT.Vt) -> J.Json:
  J.Json.obj([J.KV{"title", J.Json.str(VT.Vt.title(v))}, J.KV{"fg", J.Json.u32(VT.Vt.fg())}, J.KV{"bg", J.Json.u32(VT.Vt.bg())},
    J.KV{"lines", J.Json.arr(Mob.tlines(VT.Vt.lines(v, 0n)))}, J.KV{"cursor", Mob.cursor(VT.Vt.cursor(v))}])

# Terminals in the chat (core/emb.bend): each of the thread's, by key, its
# title, size, whether it is live or pinned, the value that restarts it,
# and its screen while live. An entry row (kind "emb") names its key; a
# live one is drawn there, a pinned one at the top, an inactive one as a
# gray row whose tap sends "emb-restart" with "restart". Keys go as
# "emb-key" with "key<tab>DOM key<tab>mods", text as "emb-paste" with
# "key<tab>text", the pin as "emb-pin" with the key.
def Mob.emb.k(live: Bool, +u: U.Ui, +key: String) -> J.Json:
  match live:
    case True{}:
      Mob.term.of(U.Ui.term(u, key))
    case False{}:
      J.Null{}

def Mob.embs(vs: List<&2, EM.EV>, +u: U.Ui, +th: String) -> List<&2, J.Json>:
  match vs:
    case Nil{}:
      Nil{}
    case EM.EV{+key, +id, +title, +cols, +rows, +live, +pinned} <> t:
      J.Json.obj([J.KV{"key", J.Json.str(key)}, J.KV{"id", J.Json.str(id)}, J.KV{"title", J.Json.str(title)}, J.KV{"cols", J.Json.u32(cols)}, J.KV{"rows", J.Json.u32(rows)},
        J.KV{"live", J.Json.bool(live)}, J.KV{"pinned", J.Json.bool(pinned)}, J.KV{"restart", J.Json.str(th ++ "\t" ++ id)}, J.KV{"term", Mob.emb.k(live, u, key)}]) <> Mob.embs(t, u, th)

# open while "term-toggle" has it open
def Mob.term(open: Bool, +u: U.Ui, +id: String) -> J.Json:
  match open:
    case True{}:
      Mob.term.of(U.Ui.term(u, id))
    case False{}:
      J.Null{}

# The thread's menu: each item sends its action with its value, or (a
# snooze) offers choices whose values it sends instead; "on" checks it,
# "danger" asks first
def Mob.item(+icon: String, +label: String, +action: String, +value: String, on: Bool, danger: Bool, opts: List<&2, J.Json>) -> J.Json:
  J.Json.obj([J.KV{"icon", J.Json.str(icon)}, J.KV{"label", J.Json.str(label)}, J.KV{"action", J.Json.str(action)}, J.KV{"value", J.Json.str(value)},
    J.KV{"on", J.Json.bool(on)}, J.KV{"danger", J.Json.bool(danger)}, J.KV{"options", J.Json.arr(opts)}])

def Mob.snooze.loc(+lc: I.Loc, snoozed: Bool) -> J.Json:
  match snoozed:
    case True{}:
      Mob.item("clock", MSG.T.sidebar.snoozed(lc), "snooze", "off", True{}, False{}, Nil{})
    case False{}:
      Mob.item("clock", MSG.T.tools.snooze(lc), "snooze", "", False{}, False{}, [Mob.opt(MSG.T.common.hour(lc), "1h"), Mob.opt(MSG.T.common.tomorrow(lc), "tomorrow"), Mob.opt(MSG.T.common.week(lc), "1w")])

# the put-away Ready row's Buy it (client.bend's Fab.mini.tools), first
def Mob.menu.tools(ts: List<&2, TO.Tool>) -> List<&2, J.Json>:
  match ts:
    case Nil{}:
      Nil{}
    case TO.Tool{+icon, +label, _, +action, +value, _, _, _} <> r:
      Mob.item(icon, label, action, value, False{}, False{}, Nil{}) <> Mob.menu.tools(r)

def Mob.snooze(snoozed: Bool) -> J.Json:
  Mob.snooze.loc(I.En{}, snoozed)

def Mob.menu.loc(+lc: I.Loc, +id: String, snoozed: Bool, mini: List<&2, TO.Tool>) -> List<&2, J.Json>:
  List.append(&2, J.Json, Mob.menu.tools(mini), [Mob.item("diff", MSG.T.viewer.diff(lc), "diff", "", False{}, False{}, Nil{}), Mob.item("terminal", MSG.T.palette.terminal(lc), "term-toggle", "", False{}, False{}, Nil{}),
   Mob.item("search", MSG.T.palette.findfile(lc), "find-open", "files", False{}, False{}, Nil{}), Mob.snooze.loc(lc, snoozed),
   Mob.item("trash", MSG.T.common.delete(lc), "row-delete", id, False{}, True{}, Nil{})])

# a phone opens a thread on its newest 20 entries (a screen shows about
# ten, and each reply's markdown is laid out in the phone's JS engine,
# the most of what opening a thread costs there); "earlier" adds pages
# of core's size (U.Page.size)

def Mob.menu(+id: String, snoozed: Bool, mini: List<&2, TO.Tool>) -> List<&2, J.Json>:
  Mob.menu.loc(I.En{}, id, snoozed, mini)

def Mob.count(+more: Nat) -> Nat:
  Nat.add(20n, Nat.mul(U.Page.size(), more))

# the newest page of a thread's entries (core's Page): "earlier" says how
# many are left out, and the action "earlier" shows another page
def Mob.page.from(+es: List<&2, M.Entry>, +drafts: Map<&2, String>, +id: String, +st: M.State) -> U.Win:
  U.Page.newest.from(es, id, U.Page.since(st, id), U.Hold.page(es, U.Str.get(drafts, "@away." ++ id), Mob.count(Maybe.default(&2, Nat, Nat.read(U.Str.get(drafts, "@more." ++ id)), 0n))))

def Mob.page(+drafts: Map<&2, String>, +id: String, +st: M.State) -> U.Win:
  Mob.page.from(M.State.mine(st, id), drafts, id, st)

def Mob.paged.es(+lc: I.Loc, w: U.Win, +st: M.State, +sel: String, +info: Map<&2, String>, +dr: Map<&2, String>, +running: Bool) -> List<&2, J.Json>:
  Mob.entries(lc, U.Win.shown(w), st, sel, info, dr, running)

# a side question (/btw) and its answer, over the composer until closed
def Mob.btw(+lc: I.Loc, +u: U.Ui) -> J.Json:
  Bool.pick(J.Json, String.is_empty(U.Btw.question(u)), J.Null{},
    J.Json.obj([J.KV{"q", J.Json.str(U.Btw.question(u))}, J.KV{"a", J.Json.str(U.Btw.text(lc, U.Btw.reply.of(u)))}]))

# a streamed reply as the thread shows it (markdown nodes)
def Mob.live.nodes.loc(+lc: I.Loc, +text: String) -> J.Json:
  J.Json.arr(Mob.nodes(D.Md.render.loc(lc, text)))

# (the phones' bridge leaves it out, render False, and puts in its own:
# the reply's settled paragraphs it rendered before, and the rest now;
# bridge.js's liveNodes, hubs.bend's Hubs.opened)

def Mob.live.nodes(+text: String) -> J.Json:
  Mob.live.nodes.loc(I.En{}, text)

def Mob.live.k.loc(+lc: I.Loc, render: Bool, +text: String) -> J.Json:
  match render:
    case True{}:
      Mob.live.nodes.loc(lc, text)
    case False{}:
      J.Json.arr(Nil{})

def Mob.live.k(render: Bool, +text: String) -> J.Json:
  Mob.live.k.loc(I.En{}, render, text)

# The project card (client.bend's Fab.*), as the first row above the
# composer: the goal, Edit and Buy it (a row of the queue's kind, which
# both apps draw with its buttons)
def Mob.fab.buy(+buy: String, +id: String) -> List<&2, J.Json>:
  Bool.pick(List<&2, J.Json>, String.is_empty(buy), Nil{}, [J.Json.obj([J.KV{"label", J.Json.str(buy)}, J.KV{"action", J.Json.str("buy")}, J.KV{"value", J.Json.str(id)}]),
    # its ×: put away (client.bend's Fab.min), Buy it stays in the menu
    J.Json.obj([J.KV{"label", J.Json.str("×")}, J.KV{"action", J.Json.str("fab-hide")}, J.KV{"value", J.Json.str(id)}])])

# the project's intent while it is asked for (its menu's Intent…,
# client.bend's Intent.*: Goal, Edit, ×) and, once the turn is over and
# there is something to buy, Ready to order with Buy it (U.Fab.ready), as
# queue rows above the message box
# (the row's tag says Ready, or Ordered once something is paid for: the
# text is what there is to order, or how the order stands)
def Mob.fab.ready.text(+line: String) -> String:
  Bool.pick(String, String.starts_with(line, "Ready to order · "), String.drop(line, 17n),
    Bool.pick(String, String.starts_with(line, "Ordered · "), String.drop(line, 10n), line))

def Mob.fab.ready.tag(+line: String) -> String:
  Bool.pick(String, String.starts_with(line, "Ordered"), "Ordered", "Ready")

def Mob.fab.ready(+line: String, +buy: String, +id: String) -> List<&2, J.Json>:
  Bool.pick(List<&2, J.Json>, String.is_empty(line), Nil{},
    [J.Json.obj([J.KV{"msg", J.Json.str("ready:" ++ id)}, J.KV{"text", J.Json.str(Mob.fab.ready.text(line))}, J.KV{"tag", J.Json.str(Mob.fab.ready.tag(line))}, J.KV{"buttons", J.Json.arr(Mob.fab.buy(buy, id))}])])

def Mob.fab.goal(asked: Bool, +lc: I.Loc, +nf: Map<&2, String>, +id: String) -> List<&2, J.Json>:
  match asked:
    case True{}:
      [J.Json.obj([J.KV{"msg", J.Json.str("fab:" ++ id)}, J.KV{"text", J.Json.str(Bool.pick(String, String.is_empty(U.Str.get(nf, "fab.line:" ++ id)), MSG.T.intent.loading(lc), U.Str.get(nf, "fab.line:" ++ id)))},
        J.KV{"tag", J.Json.str(MSG.T.intent.title(lc))},
        J.KV{"buttons", J.Json.arr([J.Json.obj([J.KV{"label", J.Json.str(MSG.T.intent.edit(lc))}, J.KV{"action", J.Json.str("fab-edit")}, J.KV{"value", J.Json.str(id)}]),
          J.Json.obj([J.KV{"label", J.Json.str("×")}, J.KV{"action", J.Json.str("intent-close")}, J.KV{"value", J.Json.str(id)}])])}])]
    case False{}:
      Nil{}

def Mob.fab.k(shown: Bool, +st: M.State, +nf: Map<&2, String>, +id: String, run: Bool) -> List<&2, J.Json>:
  match shown:
    case True{}:
      Mob.fab.ready(U.Fab.ready.shown.k(U.Fab.min.of(st, id), U.Fab.ready.k(U.Fab.buy.of(st, nf, id), U.Str.get(nf, "fab.ready:" ++ id), run)), U.Fab.ready.btn.of(st, nf, id), id)
    case False{}:
      Nil{}

# (asked: the intent is on show for this thread, U.Intent.on)
def Mob.fab(+lc: I.Loc, asked: Bool, +st: M.State, +nf: Map<&2, String>, +id: String, run: Bool) -> List<&2, J.Json>:
  List.append(&2, J.Json, Mob.fab.goal(asked, lc, nf, id), Mob.fab.k(String.eq(U.Str.get(nf, "fab.shown:" ++ id), "1"), st, nf, id, run))

# a thread's request, a queue row above the message box (Set opens the box
# in Settings)
def Mob.sbox.q(+lc: I.Loc, +st: M.State, m: Maybe<&2, U.SAsk>) -> List<&2, J.Json>:
  match m:
    case Some{+a}:
      [J.Json.obj([J.KV{"msg", J.Json.str("sbox:" ++ U.SAsk.name(a))}, J.KV{"text", J.Json.str(U.SAsk.name(a) ++ ": " ++ U.Sbox.ask.note(st, a))}, J.KV{"tag", J.Json.str(MSG.T.secret.title(lc))},
        J.KV{"buttons", J.Json.arr([J.Json.obj([J.KV{"label", J.Json.str(MSG.T.common.set(lc))}, J.KV{"action", J.Json.str("secret-open-set")}, J.KV{"value", J.Json.str(U.SAsk.name(a))}]),
          J.Json.obj([J.KV{"label", J.Json.str(MSG.T.secret.notnow(lc))}, J.KV{"action", J.Json.str("secret-dismiss")}, J.KV{"value", J.Json.str(U.SAsk.name(a))}])])}])]
    case None{}:
      Nil{}

def Mob.thread(+lc: I.Loc, render: Bool, u: U.Ui, t: M.Thread, pend: List<&2, String>, vw: J.Json, +atts: List<&2, String>, +up: String, term: J.Json, skills: List<&2, J.Json>, +ph: Nat, btw: J.Json, embs: J.Json) -> J.Json:
  match u t:
    case U.Ui{+st, +sel, +drafts, +live, _, _, +now, _, pa, _, _, _, _, _, +nf, _, _} M.Thread{+id, _, title, _, _, +turn, _, archived, o, +sa, +ua, pinned, _, branch, _, +pr, +mo, rt, _, _, _}:
      +pg = Mob.page(drafts, id, st)
      J.Json.obj([J.KV{"id", J.Json.str(id)}, J.KV{"title", J.Json.str(U.Label.title(st, id, TTL.Title.shown(lc, title)))}, J.KV{"branch", J.Json.str(branch)},
        J.KV{"boat", J.Json.str(UZ.Chip.of(st, id, now))},
        J.KV{"permissions", PM.Permissions.json(lc, rt)},
        J.KV{"readingSeen", J.Json.str(U.Hold.newest(M.State.mine(st, id)))},
        J.KV{"canChild", J.Json.bool(U.ThreadTree.can_route(st, id))},
        J.KV{"state", J.Json.str(Mob.state(turn))},
        J.KV{"tools", J.Json.arr(Mob.head.loc(lc, V.View.running(turn), pinned, TO.Tools.settled(o, Nat.is_gt(sa, ua)), archived, Bool.not(String.is_empty(M.Fork.source(st, id)))))},
        J.KV{"menu", J.Json.arr(Mob.menu.loc(lc, id, M.Snooze.on(now, U.Side.until(st, id)), U.Fab.mini.tools.of(st, nf, id, V.View.running(turn))))},

        J.KV{"parent", Mob.parent(String.is_empty(U.Tasks.up(st, id)), st, id, U.Tasks.up(st, id))},
        J.KV{"subs", Mob.subs.loc(lc, nf, id, SB.Subs.rows(st, nf, id, now))},
        J.KV{"entries", J.Json.arr(Mob.paged.es(lc, pg, st, id, nf, drafts, V.View.running(turn)))},
        J.KV{"earlier", J.Json.nat(U.Win.hidden(pg))},
        J.KV{"sending", J.Json.arr(Mob.strs(pend))},
        J.KV{"asks", J.Json.arr(Mob.asks(lc, U.Asks.open(st, id), U.Str.get(drafts, sel), nf))},
        J.KV{"queued", J.Json.str("")},
        J.KV{"queue", J.Json.arr(List.append(&2, J.Json, Mob.sbox.q(lc, st, U.Sbox.ask.in(U.Sbox.asks(st), id)), List.append(&2, J.Json, Mob.fab(lc, String.eq(U.Str.get(drafts, "@intent"), id), st, nf, id, V.View.running(turn)), Mob.queue(lc, M.Queue.context(M.Queue.items(st, id)), True{}, V.View.running(turn)))))},
        J.KV{"todos", Mob.todos(lc, nf, id, U.Todos.shown(st, id, V.View.running(turn)))},
        J.KV{"live", Mob.live.k.loc(lc, render, U.Str.get(live, id))},
        J.KV{"working", J.Json.str(Bool.pick(String, String.is_empty(U.Str.get(live, id)), SG.Signal.label(lc, ph), ""))},
        J.KV{"phase", J.Json.str(SG.Signal.name(ph))},
        J.KV{"draft", J.Json.str(U.Str.get(drafts, sel))},
        J.KV{"send", J.Json.str(U.Follow.button(lc, st, sel, V.View.running(turn), U.Follow.stop(V.View.running(turn), U.Str.get(drafts, sel), atts), U.Str.get(drafts, sel)))},
        J.KV{"sendAct", J.Json.str(U.Follow.act(U.Follow.stop(V.View.running(turn), U.Str.get(drafts, sel), atts)))},
        J.KV{"picker", Mob.picker.ctx.loc(lc, U.Models.phone(nf, pr, U.Str.get(drafts, "@f.mq")), pr, mo, U.Effort.of(st, sel), U.Usage.tail(st, sel))},
        J.KV{"skills", J.Json.arr(skills)}, J.KV{"btw", btw},
        J.KV{"attaching", J.Json.arr(Mob.chips(atts))}, J.KV{"uploading", J.Json.str(up)},
        J.KV{"chunk", J.Json.nat(AT.Att.chunk())},
        J.KV{"cap", J.Json.nat(AT.Att.cap())},
        J.KV{"diff", Mob.diff.loc(lc, Bool.and(pa, String.eq(U.Str.get(nf, "view.kind:" ++ id), "diff")), U.Str.get(nf, "diff:" ++ id))},
        J.KV{"term", term},
        J.KV{"embs", embs},
        J.KV{"viewer", vw}])

def Mob.main.r(+lc: I.Loc, none: Bool, +u: U.Ui, +sel: String, render: Bool) -> J.Json:
  match none:
    case True{}:
      J.Null{}
    case False{}:
      Mob.thread(lc, render, u, U.Ui.thread(Mob.st(u), sel), U.Ui.pending(u, sel), VW.View.json(lc, u, Mob.review.loc(lc, String.eq(VW.View.kind(u), "review"), u)), U.Ui.atts(u, sel), U.Ui.uploading(u, sel),
        Mob.term(U.Ui.flag(u, "term"), u, sel), Mob.skills(U.Ui.skills(u)), U.Side.signal(u, U.Ui.thread(Mob.st(u), sel)), Mob.btw(lc, u), J.Json.arr(Mob.embs(U.Emb.views(u, sel), u, sel)))

def Mob.main(+lc: I.Loc, none: Bool, +u: U.Ui, +sel: String) -> J.Json:
  Mob.main.r(lc, none, u, sel, True{})

def Mob.empty.loc(+lc: I.Loc, n: Nat) -> String:
  Bool.pick(String, Nat.is_eq(n, 0n), MSG.T.home.phone.start(lc), MSG.T.home.phone.pick(lc))

def Mob.empty(n: Nat) -> String:
  Mob.empty.loc(I.En{}, n)

def Mob.title(t: M.Thread) -> String:
  match t:
    case M.Thread{_, _, title, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      title

# a delete a row asked for, to confirm: its thread, what to ask, the
# buttons (the yes button sends "row-delete id" again, the no "delete-no")
def Mob.deleting.k(none: Bool, +lc: I.Loc, +st: M.State, +id: String) -> J.Json:
  match none:
    case True{}:
      J.Null{}
    case False{}:
      J.Json.obj([J.KV{"id", J.Json.str(id)}, J.KV{"title", J.Json.str(MSG.T.sheet.delete(lc))},
        J.KV{"body", J.Json.str(TTL.Title.shown(lc, Mob.title(U.Ui.thread(st, id))))}, J.KV{"yes", J.Json.str(MSG.T.common.delete(lc))}, J.KV{"no", J.Json.str(MSG.T.common.cancel(lc))}])

# a thread being renamed (client.bend's Ren): its id, what to ask, the title
# so far ("text": a phone makes every "value" an id of its hub); the yes
# button sends "rename-save" with "id\u001ftitle", the no "rename-no"
def Mob.renaming.k(none: Bool, +lc: I.Loc, +id: String, +value: String) -> J.Json:
  match none:
    case True{}:
      J.Null{}
    case False{}:
      J.Json.obj([J.KV{"id", J.Json.str(id)}, J.KV{"title", J.Json.str(MSG.T.sheet.rename(lc))}, J.KV{"text", J.Json.str(value)},
        J.KV{"yes", J.Json.str(MSG.T.menu.rename(lc))}, J.KV{"no", J.Json.str(MSG.T.common.cancel(lc))}])

def Mob.renaming(+lc: I.Loc, +u: U.Ui) -> J.Json:
  Mob.renaming.k(String.is_empty(U.Ren.on(u)), lc, U.Ren.on(u), U.Ui.scratch(u, "@f.title"))

# the first-run tour (core/onboard.bend), while a step is on screen: its
# number, count and counter, its title and text, and the buttons (actions
# "ob-back", "ob-next", "ob-skip", sent with no value to the hub in focus),
# and the choices of the uberbot step (label, action, value: sent as they are)
def Mob.tour.choices(cs: List<&2, U.TChoice>) -> List<&2, J.Json>:
  match cs:
    case Nil{}:
      Nil{}
    case U.TChoice{l, a, v} <> t:
      J.Json.obj([J.KV{"label", J.Json.str(l)}, J.KV{"action", J.Json.str(a)}, J.KV{"value", J.Json.str(v)}]) <> Mob.tour.choices(t)

def Mob.tour.k(+lc: I.Loc, +i: Nat, +u: U.Ui) -> J.Json:
  J.Json.obj([J.KV{"step", J.Json.nat(i)}, J.KV{"count", J.Json.nat(OB.Ob.count())}, J.KV{"counter", J.Json.str(U.Tour.counter.loc(lc, u))},
    J.KV{"title", J.Json.str(OB.Ob.title(lc, i))}, J.KV{"body", J.Json.str(U.Tour.body.loc(lc, 2n, u))}, J.KV{"first", J.Json.bool(Bool.not(U.Tour.back(u)))},
    J.KV{"next", J.Json.str(U.Tour.forward(u))}, J.KV{"back", J.Json.str(MSG.T.tour.back(lc))}, J.KV{"skip", J.Json.str(MSG.T.tour.skip(lc))},
    J.KV{"choices", J.Json.arr(Mob.tour.choices(U.Tour.choices(u)))}])

def Mob.tour.at(+lc: I.Loc, live: Bool, +i: Nat, +u: U.Ui) -> J.Json:
  match live:
    case True{}:
      Mob.tour.k(lc, i, u)
    case False{}:
      J.Null{}

def Mob.tour(+lc: I.Loc, +u: U.Ui) -> J.Json:
  Mob.tour.at(lc, U.Tour.on(u), U.Tour.at(u), u)

def Mob.deleting(+lc: I.Loc, +u: U.Ui) -> J.Json:
  Mob.deleting.k(String.is_empty(U.Ui.row_deleting(u)), lc, Mob.st(u), U.Ui.row_deleting(u))

# a project remove to confirm ("proj-remove" armed it; "proj-remove" with
# id removes it, "proj-keep" cancels): the phones' second click
def Mob.removing.k(none: Bool, +lc: I.Loc, +st: M.State, +id: String) -> J.Json:
  match none:
    case True{}:
      J.Null{}
    case False{}:
      J.Json.obj([J.KV{"id", J.Json.str(id)}, J.KV{"title", J.Json.str(MSG.T.sheet.remove(lc, U.Ui.project_title(st, id)))},
        J.KV{"body", J.Json.str(MSG.T.sheet.remove.note(lc))}, J.KV{"yes", J.Json.str(MSG.T.common.remove(lc))}, J.KV{"no", J.Json.str(MSG.T.common.cancel(lc))}])

def Mob.removing(+lc: I.Loc, +u: U.Ui) -> J.Json:
  Mob.removing.k(String.is_empty(U.Ui.scratch(u, "@rm")), lc, Mob.st(u), U.Ui.scratch(u, "@rm"))

# the list's search: always shown at the top of the list (open stays true
# for older apps), its query ("proj-find-q"); it finds projects, or in the
# active view threads by their title or project
def Mob.search.loc(+lc: I.Loc, +u: U.Ui) -> J.Json:
  J.Json.obj([J.KV{"open", J.Json.bool(True{})}, J.KV{"query", J.Json.str(U.Ui.scratch(u, "@f.pq"))}, J.KV{"hint", J.Json.str(MSG.T.common.search(lc))}])

# Settings, as the web's: rows of a label, a note and buttons, each sending
# its action with its value ("on" marks the current choice)

def Mob.search(+u: U.Ui) -> J.Json:
  Mob.search.loc(I.En{}, u)

def Mob.set.btn(+label: String, +action: String, +value: String, on: Bool) -> J.Json:
  J.Json.obj([J.KV{"label", J.Json.str(label)}, J.KV{"action", J.Json.str(action)}, J.KV{"value", J.Json.str(value)}, J.KV{"on", J.Json.bool(on)}])

def Mob.set.chips.loc(+lc: I.Loc, xs: List<&2, String>, +cur: String, +key: String) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case +v <> t:
      Mob.set.btn(V.VLabel.option(lc, key, v), "setting", key ++ "=" ++ v, String.eq(v, cur)) <> Mob.set.chips.loc(lc, t, cur, key)

def Mob.set.chips(xs: List<&2, String>, +cur: String, +key: String) -> List<&2, J.Json>:
  Mob.set.chips.loc(I.En{}, xs, cur, key)

def Mob.set.row(+label: String, +note: String, btns: List<&2, J.Json>) -> J.Json:
  J.Json.obj([J.KV{"label", J.Json.str(label)}, J.KV{"note", J.Json.str(note)}, J.KV{"buttons", J.Json.arr(btns)}])

# A row about this device, not the hub (ios-snappy: the apps keep these in
# App settings and skip the row in the hub's page): `local` marks it
# A hub setting the apps draw in App settings' Notifications section instead of the
# hub's page (notif-policy's "Notify me for"): `notify` marks it
def Mob.set.row.n(+label: String, +note: String, btns: List<&2, J.Json>) -> J.Json:
  J.Json.obj([J.KV{"label", J.Json.str(label)}, J.KV{"note", J.Json.str(note)}, J.KV{"buttons", J.Json.arr(btns)}, J.KV{"notify", J.Json.bool(True{})}])

def Mob.set.row.l(+label: String, +note: String, btns: List<&2, J.Json>) -> J.Json:
  J.Json.obj([J.KV{"label", J.Json.str(label)}, J.KV{"note", J.Json.str(note)}, J.KV{"buttons", J.Json.arr(btns)}, J.KV{"local", J.Json.bool(True{})}])

# a row with fields to type in: each sends "bfield" (name, text) as it is
# typed; a secret one shows dots
def Mob.set.field(+name: String, +hint: String, secret: Bool, +value: String) -> J.Json:
  J.Json.obj([J.KV{"name", J.Json.str(name)}, J.KV{"hint", J.Json.str(hint)}, J.KV{"secret", J.Json.bool(secret)}, J.KV{"value", J.Json.str(value)}])

def Mob.set.row.f(+label: String, +note: String, fs: List<&2, J.Json>, btns: List<&2, J.Json>) -> J.Json:
  J.Json.obj([J.KV{"label", J.Json.str(label)}, J.KV{"note", J.Json.str(note)}, J.KV{"fields", J.Json.arr(fs)}, J.KV{"buttons", J.Json.arr(btns)}])

# Settings' model search (the picker's "mq"; it narrows the composer's
# picker too), once the provider lists many models
def Mob.set.find.k(many: Bool, +lc: I.Loc, +q: String) -> List<&2, J.Json>:
  match many:
    case True{}:
      [Mob.set.row.f(MSG.T.common.search(lc), "", [Mob.set.field("mq", MSG.T.common.search(lc), False{}, q)], Nil{})]
    case False{}:
      Nil{}

def Mob.set.find(+lc: I.Loc, u: U.Ui, +nf: Map<&2, String>, +prov: String) -> List<&2, J.Json>:
  Mob.set.find.k(U.Models.search(U.Models.menu(U.Models.of(nf, prov), prov)), lc, U.Ui.scratch(u, "@f.mq"))

# API keys (client.bend's ApiKeys), as on the web: Add and View, the form,
# the names each with Delete
def Mob.keys.rows.loc(+lc: I.Loc, ns: List<&2, String>) -> List<&2, J.Json>:
  match ns:
    case Nil{}:
      Nil{}
    case +n <> t:
      Mob.set.row(n, "", [Mob.set.btn(MSG.T.common.delete(lc), "key-del", n, False{})]) <> Mob.keys.rows.loc(lc, t)

def Mob.keys.rows(ns: List<&2, String>) -> List<&2, J.Json>:
  Mob.keys.rows.loc(I.En{}, ns)

def Mob.keys.list.loc(+lc: I.Loc, open: Bool, +u: U.Ui) -> List<&2, J.Json>:
  match open:
    case True{}:
      Bool.pick(List<&2, J.Json>, List.is_empty(&2, String, U.ApiKeys.names(u)), [Mob.set.row(MSG.T.settings.keys.empty(lc), "", Nil{})], Mob.keys.rows.loc(lc, U.ApiKeys.names(u)))
    case False{}:
      Nil{}

def Mob.keys.list(open: Bool, +u: U.Ui) -> List<&2, J.Json>:
  Mob.keys.list.loc(I.En{}, open, u)

def Mob.keys.form(+lc: I.Loc, open: Bool, +u: U.Ui) -> List<&2, J.Json>:
  match open:
    case True{}:
      [Mob.set.row.f(MSG.T.settings.keys.new(lc), U.ApiKeys.form.note(lc), [Mob.set.field("kname", MSG.T.bots.name(lc), False{}, U.Ui.scratch(u, "@f.kname")), Mob.set.field("kval", MSG.T.common.key(lc), True{}, "")],
        [Mob.set.btn(MSG.T.common.save(lc), "key-add", "", True{})])]
    case False{}:
      Nil{}

# the time zone (client.bend's Tzs): the row shows it; a field and Set; Automatic
# while one is set; an error from the hub under it
def Mob.tz(+lc: I.Loc, +u: U.Ui) -> List<&2, J.Json>:
  [Mob.set.row.f(MSG.T.settings.zone(lc), U.Tzs.line.loc(lc, u), [Mob.set.field("tzname", "Europe/Helsinki", False{}, U.Ui.scratch(u, "@f.tzname"))],
     List.append(&2, J.Json, [Mob.set.btn(MSG.T.common.set(lc), "tz-set", "", True{})], Bool.pick(List<&2, J.Json>, U.Tzs.auto(u), [Mob.set.btn(MSG.T.settings.lang.auto(lc), "tz-auto", "", False{})], Nil{}))),
   Mob.set.row("", U.Tzs.err(u), Nil{})]

# The secret box (client.bend's Sbox.*), as on the web. Settings shows the
# vault, an open box (its value field is a "vault" field: the app keeps
# what is typed and hands it to Backplane.secret at Save, never to the
# client as a form field), the requests and the secrets kept. A thread's
# request is a queue row whose Set opens that box.
def Mob.sbox.fields(+lc: I.Loc, fresh: Bool, +n: String, +p: String) -> List<&2, J.Json>:
  List.append(&2, J.Json, Bool.pick(List<&2, J.Json>, fresh, [Mob.set.field("sname", MSG.T.secret.name.ph(lc), False{}, n), Mob.set.field("spurpose", MSG.T.secret.purpose.ph(lc), False{}, p)], Nil{}),
    [J.Json.obj([J.KV{"name", J.Json.str("sval")}, J.KV{"hint", J.Json.str(MSG.T.secret.value.ph(lc))}, J.KV{"secret", J.Json.bool(True{})}, J.KV{"vault", J.Json.bool(True{})}, J.KV{"value", J.Json.str("")}])])

def Mob.sbox.k(+lc: I.Loc, b: U.SBox) -> J.Json:
  match b:
    case U.SBox{+n, +p, +who, +th, +fresh, +err}:
      Mob.set.row.f(Bool.pick(String, String.is_empty(who), MSG.T.secret.add(lc), MSG.T.secret.asks(lc, who, n)),
        Bool.pick(String, String.is_empty(err), Bool.pick(String, Bool.or(fresh, String.is_empty(p)), "", p ++ ". ") ++ MSG.T.secret.note(lc), err),
        Mob.sbox.fields(lc, fresh, n, p),
        [Mob.set.btn(MSG.T.secret.save(lc), "secret-save", n ++ "\u{1f}" ++ p ++ "\u{1f}" ++ th, True{}),
         Mob.set.btn(MSG.T.secret.notnow(lc), Bool.pick(String, String.is_empty(who), "secret-shut", "secret-dismiss"), n, False{})])

def Mob.sbox(+lc: I.Loc, m: Maybe<&2, U.SBox>) -> List<&2, J.Json>:
  match m:
    case Some{b}:
      [Mob.sbox.k(lc, b)]
    case None{}:
      Nil{}

def Mob.secrets.asks(xs: List<&2, U.SAsk>, +lc: I.Loc, +st: M.State) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case +a <> t:
      Mob.set.row(MSG.T.secret.requested(lc) ++ ": " ++ U.SAsk.name(a), U.Sbox.ask.note(st, a),
        [Mob.set.btn(MSG.T.common.set(lc), "secret-open", U.SAsk.name(a), True{}), Mob.set.btn(MSG.T.secret.notnow(lc), "secret-dismiss", U.SAsk.name(a), False{})])
        <> Mob.secrets.asks(t, lc, st)

def Mob.secrets.row(+lc: I.Loc, r: U.SRowV) -> J.Json:
  match r:
    case U.SRowV{+n, +note, +rm}:
      Mob.set.row(n, note, [Mob.set.btn(MSG.T.secret.replace(lc), "secret-open", n, False{}),
        Mob.set.btn(Bool.pick(String, rm, MSG.T.secret.delete.ask(lc, n), MSG.T.common.delete(lc)), "secret-del", n, rm)])

def Mob.secrets.rows(rs: List<&2, U.SRowV>, +lc: I.Loc) -> List<&2, J.Json>:
  match rs:
    case Nil{}:
      Nil{}
    case r <> t:
      Mob.secrets.row(lc, r) <> Mob.secrets.rows(t, lc)

def Mob.secrets(+lc: I.Loc, +u: U.Ui) -> List<&2, J.Json>:
  List.concat(&2, J.Json, [
    [Mob.set.row(MSG.T.secret.title(lc), U.Sbox.note(lc, u), [Mob.set.btn(MSG.T.secret.add(lc), "secret-open", "", False{})])],
    Mob.sbox(lc, U.Sbox.box(u, "")),
    Mob.secrets.asks(U.Sbox.asks(U.Ui.state.of(u)), lc, U.Ui.state.of(u)),
    Mob.secrets.rows(U.Sbox.rows(lc, u), lc)])

def Mob.keys(+lc: I.Loc, +u: U.Ui) -> List<&2, J.Json>:
  List.concat(&2, J.Json, [[Mob.set.row(MSG.T.settings.keys(lc), U.ApiKeys.note.loc(lc, u), [Mob.set.btn(MSG.T.settings.keys.add(lc), "flag", "keyform", U.Ui.flag(u, "keyform")), Mob.set.btn(MSG.T.settings.keys.view(lc), "flag", "keys", U.Ui.flag(u, "keys"))])],
    Mob.keys.form(lc, U.Ui.flag(u, "keyform"), u), Mob.keys.list.loc(lc, U.Ui.flag(u, "keys"), u)])

def Mob.set.days(+d: String) -> List<&2, J.Json>:
  +n = {Maybe.default(&2, Nat, Nat.read(d), 3n) : Nat}
  [Mob.set.btn("−", "setting", "settle.days=" ++ Nat.show(Bool.pick(Nat, Nat.is_gt(n, 1n), Nat.sub(n, 1n), 1n)), False{}),
   Mob.set.btn("+", "setting", "settle.days=" ++ Nat.show(Bool.pick(Nat, Nat.is_lt(n, 90n), 1n+n, 90n)), False{})]

def Mob.set.days.label.loc(+lc: I.Loc, +d: String) -> String:
  +n = {Maybe.default(&2, Nat, Nat.read(d), 3n) : Nat}
  MSG.T.settings.days.label(lc, Nat.show(n), Bool.pick(String, Nat.is_eq(n, 1n), "one", "other"))

# the version row's button (U.Update.btn)

def Mob.set.days.label(+d: String) -> String:
  Mob.set.days.label.loc(I.En{}, d)

def Mob.update.btn.loc(+lc: I.Loc, i: Nat) -> List<&2, J.Json>:
  match i:
    case 1n:
      [Mob.set.btn(MSG.T.settings.update.install(lc), "update", "", False{})]
    case 2n:
      [Mob.set.btn(MSG.T.settings.update.check(lc), "update-check", "", False{})]
    case 3n:
      [Mob.set.btn(MSG.T.settings.update.now(lc), "update-now", "", False{})]
    case _:
      Nil{}

def Mob.update.btn(i: Nat) -> List<&2, J.Json>:
  Mob.update.btn.loc(I.En{}, i)

def Mob.set.strs(xs: List<&2, String>) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      J.Json.str(x) <> Mob.set.strs(t)

def Mob.set.pair.k(+lc: I.Loc, on: Bool, +btns: List<&2, J.Json>, +nf: Map<&2, String>) -> J.Json:
  match on:
    case True{}:
      J.Json.obj([J.KV{"label", J.Json.str(MSG.T.settings.pair(lc))}, J.KV{"note", J.Json.str(U.Pair.note(lc, U.Pair.link(nf)))}, J.KV{"buttons", J.Json.arr(btns)},
        J.KV{"qr", J.Json.arr(Mob.set.strs(U.Pair.qr(nf)))}, J.KV{"qrnote", J.Json.str(U.Pair.qr.note(lc))}])
    case False{}:
      Mob.set.row(MSG.T.settings.pair(lc), U.Pair.note(lc, U.Pair.link(nf)), btns)

# the pairing link with Copy and QR code; while "pairqr" is on the row
# carries the code's rows ("qr": '1' dark), drawn under it
def Mob.set.pair(+lc: I.Loc, +on: Bool, +nf: Map<&2, String>) -> J.Json:
  +btns = {List.append(&2, J.Json, Bool.pick(List<&2, J.Json>, U.Pair.qr.ready(nf), [Mob.set.btn(MSG.T.settings.pair.code(lc), "flag", "pairqr", on)], Nil{}),
    Bool.pick(List<&2, J.Json>, String.is_empty(U.Pair.link(nf)), Nil{}, [Mob.set.btn(MSG.T.common.copy(lc), "copy", U.Pair.link(nf), False{})])) : List<&2, J.Json>}
  Mob.set.pair.k(lc, on, btns, nf)

# Your machines (U.Mate.rows): each of the owner's other machines, how its
# sync goes, and On/Off ("sync.<name>")
def Mob.mates.go.loc(+lc: I.Loc, rs: List<&2, U.MateRow>) -> List<&2, J.Json>:
  match rs:
    case Nil{}:
      Nil{}
    case U.MateRow{+nm, +note, +on, +key} <> t:
      Mob.set.row(nm, note, Mob.set.chips.loc(lc, ["on", "off"], on, key)) <> Mob.mates.go.loc(lc, t)

def Mob.mates.go(rs: List<&2, U.MateRow>) -> List<&2, J.Json>:
  Mob.mates.go.loc(I.En{}, rs)

def Mob.mates(+lc: I.Loc, +u: U.Ui) -> List<&2, J.Json>:
  Mob.set.row(U.Mate.title(lc), U.Mate.lead.loc(lc, u), Nil{}) <> Mob.mates.go.loc(lc, U.Mate.rows.loc(lc, u))

# Notification policy rows, shared with the window and web.
def Mob.notify.rows.loc(+lc: I.Loc, rs: List<&2, U.NotifyRow>) -> List<&2, J.Json>:
  match rs:
    case Nil{}:
      Nil{}
    case U.NotifyRow{nm, note, on, key} <> t:
      Mob.set.row.n(nm, note, Mob.set.chips.loc(lc, ["on", "off"], on, key)) <> Mob.notify.rows.loc(lc, t)

def Mob.notify.rows(rs: List<&2, U.NotifyRow>) -> List<&2, J.Json>:
  Mob.notify.rows.loc(I.En{}, rs)

def Mob.notify(+lc: I.Loc, st: M.State) -> List<&2, J.Json>:
  Mob.set.row.n(U.Notify.title(lc), U.Notify.lead(lc), Nil{}) <> Mob.notify.rows.loc(lc, U.Notify.rows.loc(lc, st))

# Settings > Usage (src/core/usage.bend), the first row titled "Usage: ...";
# a row with an action has its Refresh button
def Mob.usage.btn.loc(+lc: I.Loc, none: Bool, +act: String) -> List<&2, J.Json>:
  match none:
    case True{}:
      Nil{}
    case False{}:
      [Mob.set.btn(MSG.T.common.refresh(lc), act, "", False{})]

def Mob.usage.btn(none: Bool, +act: String) -> List<&2, J.Json>:
  Mob.usage.btn.loc(I.En{}, none, act)

def Mob.usage.row.loc(+lc: I.Loc, +label: String, +r: UZ.URow) -> J.Json:
  Mob.set.row(label, UZ.URow.note(r), Mob.usage.btn.loc(lc, String.is_empty(UZ.URow.act(r)), UZ.URow.act(r)))

def Mob.usage.row(+label: String, +r: UZ.URow) -> J.Json:
  Mob.usage.row.loc(I.En{}, label, r)

def Mob.usage.go.loc(+lc: I.Loc, rs: List<&2, UZ.URow>) -> List<&2, J.Json>:
  match rs:
    case Nil{}:
      Nil{}
    case +r <> t:
      Mob.usage.row.loc(lc, UZ.URow.label(r), r) <> Mob.usage.go.loc(lc, t)

def Mob.usage.go(rs: List<&2, UZ.URow>) -> List<&2, J.Json>:
  Mob.usage.go.loc(I.En{}, rs)

def Mob.usage.rows.loc(+lc: I.Loc, rs: List<&2, UZ.URow>) -> List<&2, J.Json>:
  match rs:
    case Nil{}:
      Nil{}
    case +r <> t:
      Mob.usage.row.loc(lc, MSG.T.settings.usage.label(lc, UZ.URow.label(r)), r) <> Mob.usage.go.loc(lc, t)

def Mob.usage.rows(rs: List<&2, UZ.URow>) -> List<&2, J.Json>:
  Mob.usage.rows.loc(I.En{}, rs)

def Mob.usage.controls(cs: List<&2, UP.UpControl>) -> List<&2, J.Json>:
  match cs:
    case Nil{}: Nil{}
    case UP.UpControl{label, action, value, on} <> rest:
      Mob.set.btn(label, action, value, on) <> Mob.usage.controls(rest)

def Mob.usage.providers(ps: List<&2, String>, +cur: String) -> List<&2, J.Json>:
  match ps:
    case Nil{}: Nil{}
    case +p <> rest: Mob.set.btn(p, "usage-provider", p, String.eq(p, cur)) <> Mob.usage.providers(rest, cur)

def Mob.usage.days(ds: List<&2, UG.UgDay>, +max: Nat) -> List<&2, J.Json>:
  match ds:
    case Nil{}: Nil{}
    case UG.UgDay{+date, +v} <> rest:
      J.Json.obj([J.KV{"date", J.Str{date}}, J.KV{"reading", J.Str{UG.Ug.reading(v)}}, J.KV{"known", J.Flag{UG.Ug.known(v)}},
        J.KV{"fraction", J.Num{Nat.show(Nat.div(Nat.mul(UP.Up.tokens(v), 10000n), Bool.pick(Nat, Nat.is_eq(max, 0n), 1n, max)))}}]) <> Mob.usage.days(rest, max)

def Mob.usage.ranked(ps: List<&2, UG.UgProject>, +rank: Nat) -> List<&2, J.Json>:
  match ps:
    case Nil{}: Nil{}
    case UG.UgProject{+id, title, n} <> rest:
      Mob.set.row(Nat.show(rank) ++ ". " ++ title, Nat.show(n) ++ " tokens · " ++ Bool.pick(String, String.is_empty(id), "No project identity", id), Nil{}) <> Mob.usage.ranked(rest, Nat.add(rank, 1n))

def Mob.usage.graph(have: Bool, +u: U.Ui) -> List<&2, J.Json>:
  match have:
    case False{}: [Mob.set.row("Daily recorded tokens", Bool.pick(String, String.is_empty(U.Ui.info(u, "models")), "Provider availability is loading.", "No enabled providers on this machine."), Nil{})]
    case True{}:
      +days = UG.Ug.days(U.Ui.state.of(u), U.Side.now(u), U.Usage.page.period(u), U.Usage.page.provider(u))
      [J.Json.obj([J.KV{"label", J.Str{"Daily recorded tokens · " ++ U.Usage.page.provider(u)}},
        J.KV{"note", J.Str{M.Tok.day(UG.Ug.start(U.Side.now(u), U.Usage.page.period(u))) ++ " – " ++ M.Tok.day(U.Side.now(u)) ++ " (UTC) · " ++ UG.Ug.coverage(U.Ui.state.of(u))}},
        J.KV{"buttons", J.Json.arr(Nil{})}, J.KV{"chart", J.Json.arr(Mob.usage.days(days, UG.Ug.max(days, 0n)))}])]

def Mob.usage.dashboard(show: Bool, +u: U.Ui) -> List<&2, J.Json>:
  match show:
    case False{}: Nil{}
    case True{}:
      +st = U.Ui.state.of(u)
      +projects = UG.Ug.projects(st, U.Ui.projects(st), U.Side.now(u), U.Usage.page.period(u))
      List.concat(&2, J.Json, [[Mob.set.row("Provider", "", Mob.usage.providers(U.Usage.page.providers(u), U.Usage.page.provider(u)))],
        Mob.usage.graph(Bool.not(String.is_empty(U.Usage.page.provider(u))), u),
        [Mob.set.row("Top 5 projects", "Recorded tokens across all models and providers · same UTC period. Missing usage is unknown." ++ Bool.pick(String, String.eq(U.Usage.page.period(u), "today"), " " ++ UG.Ug.coverage(st), ""), Nil{})],
        Bool.pick(List<&2, J.Json>, List.is_empty(&2, UG.UgProject, projects), [Mob.set.row("Projects", "No project token readings in this period.", Nil{})], Mob.usage.ranked(projects, 1n))])

def Mob.usage.pagers(cs: List<&2, UP.UpControl>) -> List<&2, J.Json>:
  match cs:
    case Nil{}: Nil{}
    case +h <> +rest: [Mob.set.row("Breakdown pages", "", Mob.usage.controls(h <> rest))]

def Mob.usage.page(+lc: I.Loc, +u: U.Ui) -> List<&2, J.Json>:
  +cs = U.Usage.page.controls(u)
  List.concat(&2, J.Json, [[Mob.set.row("Period", "UTC calendar boundaries", Mob.usage.controls(List.take(&2, UP.UpControl, cs, 2n))),
    Mob.set.row("View", "", Mob.usage.controls(List.take(&2, UP.UpControl, List.drop(&2, UP.UpControl, cs, 2n), 3n)))],
    Mob.usage.dashboard(String.eq(U.Usage.page.metric(u), "tokens"), u), Mob.usage.rows.loc(lc, U.Usage.page.rows(u)), Mob.usage.pagers(List.drop(&2, UP.UpControl, cs, 5n))])

def Mob.kn.acts(as: List<&2, KH.KAct>) -> List<&2, J.Json>:
  match as:
    case Nil{}: Nil{}
    case KH.KAct{l, a, v} <> t: Mob.set.btn(l, U.KN.action(a), v, False{}) <> Mob.kn.acts(t)

def Mob.kn(n: Maybe<&2, KH.KNote>) -> List<&2, J.Json>:
  match n:
    case None{}: Nil{}
    case Some{KH.KNote{_, title, body, acts}}: [Mob.set.row(title, body, Mob.kn.acts(acts))]

def Mob.lang.chips(+lc: I.Loc, vs: List<&2, String>, +u: U.Ui) -> List<&2, J.Json>:
  match vs:
    case Nil{}: Nil{}
    case +v <> t:
      Mob.set.btn(U.Lang.label(lc, v), "lang", v, String.eq(v, U.Lang.choice(u))) <> Mob.lang.chips(lc, t, u)

def Mob.lang.rows(+lc: I.Loc, +u: U.Ui) -> List<&2, J.Json>:
  [Mob.set.row.l(MSG.T.settings.lang(lc), "", Mob.lang.chips(lc, U.Lang.offered(u), u)),
    Mob.set.row.l(MSG.T.settings.agent.lang(lc), "", [Mob.set.btn(
      Bool.pick(String, U.Lang.agent.on(u), MSG.T.settings.lang.on(lc), MSG.T.settings.lang.off(lc)),
      "setting", "agent.lang=" ++ U.Lang.agent.value(Bool.not(U.Lang.agent.on(u)), u), U.Lang.agent.on(u))])]

def Mob.lang.loc(+lc: I.Loc, u: U.Ui) -> J.Json:
  J.Json.obj([J.KV{"rows", J.Json.arr(Mob.lang.rows(lc, u))}])

def Mob.kx.rows(ls: List<&2, String>) -> List<&2, J.Json>:
  match ls:
    case Nil{}: Nil{}
    case l <> t: Mob.set.row(l, "", Nil{}) <> Mob.kx.rows(t)

def Mob.kx.settings(+u: U.Ui) -> List<&2, J.Json>:
  Mob.kx.rows(KEX.KX.lines(KEX.KX.report(J.Json.get_obj(U.KX.result(U.Ui.infos(u), U.Ui.sel(u)), "report"))))

def Mob.lang(+u: U.Ui) -> J.Json:
  Mob.lang.loc(U.Lang.loc(u), u)

# Antigravity (client.bend's Agv.rows), as every client draws them: a
# button "open-url" the app opens in the browser itself
def Mob.agy.btns(bs: List<&2, U.AgBtn>) -> List<&2, J.Json>:
  match bs:
    case Nil{}:
      Nil{}
    case U.AgBtn{+l, +a, +v, on} <> t:
      Mob.set.btn(l, a, v, on) <> Mob.agy.btns(t)

def Mob.agy.fields(fs: List<&2, U.AgField>) -> List<&2, J.Json>:
  match fs:
    case Nil{}:
      Nil{}
    case U.AgField{+n, +h, sec, +v, _} <> t:
      Mob.set.field(n, h, sec, v) <> Mob.agy.fields(t)

def Mob.agy.rows(rs: List<&2, U.AgRow>) -> List<&2, J.Json>:
  match rs:
    case Nil{}:
      Nil{}
    case U.AgRow{+l, +n, fs, bs} <> t:
      Mob.set.row.f(l, n, Mob.agy.fields(fs), Mob.agy.btns(bs)) <> Mob.agy.rows(t)

def Mob.settings.rows.k(i: Nat, +lc: I.Loc, +u: U.Ui, +st: M.State, +nf: Map<&2, String>, +v: String, +note: String, +more: List<&2, J.Json>, +prov: String, +models: String, +model: String) -> List<&2, J.Json>:
  match i:
    case 0n: List.concat(&2, J.Json, [[Mob.set.row.l(MSG.T.settings.theme(lc), MSG.T.settings.theme.phone.note(lc), Mob.set.chips.loc(lc, ["dark", "light"], M.Setting.get(st, "ui.theme", ""), "ui.theme"))],
      Mob.lang.rows(lc, u),
      [Mob.set.row.l(MSG.T.settings.sidebar(lc), MSG.T.settings.sidebar.note(lc), Mob.set.chips.loc(lc, ["projects", "active"], U.Side.view(st), "sidebar.view"))]])
    case 1n: List.concat(&2, J.Json, [[Mob.set.row(Mob.set.days.label.loc(lc, M.Setting.get(st, "settle.days", "3")), MSG.T.settings.settle.note(lc), Mob.set.days(M.Setting.get(st, "settle.days", "3")))],
      [Mob.set.row(MSG.T.settings.settle.done(lc), MSG.T.settings.settle.done.note(lc), Mob.set.chips.loc(lc, ["on", "off"], Bool.pick(String, M.Lifecycle.on(st), "on", "off"), M.Lifecycle.key()))],
      [Mob.set.row(MSG.T.settings.restart(lc), MSG.T.settings.restart.note(lc), Mob.set.chips.loc(lc, ["on", "off"], Bool.pick(String, M.Carry.on(st), "on", "off"), "restart.continue"))],
      Mob.agy.rows(U.Fallbackv.rows(u))])
    case 2n: List.concat(&2, J.Json, [[Mob.set.row(MSG.T.settings.provider(lc), Bool.pick(String, String.is_empty(U.Models.cli(models, prov)), MSG.T.settings.provider.missing(lc, prov), U.Models.cli(models, prov)), Mob.set.chips.loc(lc, ["claude", "codex", "grok", "muse", "antigravity", "opencode", "pi", "omp"], prov, "agent.provider"))],
      [Mob.set.row(MSG.T.settings.model(lc), Bool.pick(String, String.is_empty(model), MSG.T.common.default(lc), model) ++ ": " ++ U.Models.label_of(U.Models.of(nf, prov), prov, model),
      Mob.set.chips.loc(lc, U.Models.settings(U.Models.of(nf, prov), prov, U.Ui.scratch(u, "@f.mq"), model), model, "agent.model"))],
      Mob.set.find(lc, u, nf, prov)])
    case 3n: List.concat(&2, J.Json, [])
    case 4n: List.concat(&2, J.Json, [[Mob.set.row.f("KiCad", U.KN.summary(nf) ++ " · " ++ U.Str.get(nf, "kicad.cli"), [Mob.set.field("kpath", MSG.T.settings.kicad.path(lc), False{}, U.Ui.scratch(u, "@f.kpath"))], [Mob.set.btn(MSG.T.settings.kicad.path.save(lc), "kicad-path-save", "", False{})])],
      Mob.kn(U.KN.of(nf, st)),
      [Mob.set.row(U.FreeCAD.label.loc(lc, nf), U.FreeCAD.note(lc, nf), Nil{})],
      [Mob.set.row(U.KiCad.ipc.label(lc, nf), U.KiCad.ipc.note(lc, nf), Bool.pick(List<&2, J.Json>, U.KiCad.ipc.ready(nf), Mob.set.chips.loc(lc, ["on", "off"], M.Setting.get(st, "kicad.ipc", "on"), "kicad.ipc"), Nil{}))],
      Mob.kx.settings(u)])
    case 5n: List.concat(&2, J.Json, [[Mob.set.row(U.Tail.label(lc, nf), U.Tail.note(lc, nf), Nil{})],
      Mob.mates(lc, u),
      [Mob.set.pair(lc, U.Ui.flag(u, "pairqr"), nf)],
      [Mob.set.row(MSG.T.settings.channel(lc), MSG.T.settings.channel.note(lc), Mob.set.chips.loc(lc, ["stable", "nightly"], M.Setting.get(st, "update.channel", "stable"), "update.channel"))],
      [Mob.set.row("Backplane " ++ v, U.Update.line.w(lc, st, nf, note), Mob.update.btn.loc(lc, U.Update.btn(st, nf, note)))]])
    case 6n: List.concat(&2, J.Json, [])
    case 7n: List.concat(&2, J.Json, [Mob.keys(lc, u), Mob.secrets(lc, u)])
    case 8n: List.concat(&2, J.Json, [Mob.agy.rows(U.Agv.rows.loc(lc, u))])
    case 9n: List.concat(&2, J.Json, [Mob.tz(lc, u)])
    case 10n: Mob.usage.page(lc, u)
    case 11n: List.concat(&2, J.Json, [[Mob.set.row.l(MSG.T.settings.tour(lc), MSG.T.settings.tour.note(lc), [Mob.set.btn(MSG.T.settings.tour.show(lc), "ob-open", "", False{})])],
      more])
    case 12n: Mob.agy.rows(U.Share.rows(u))
    case 13n: List.concat(&2, J.Json, [[Mob.set.row("How it works", U.Fab.set.about(), Nil{})],
      [Mob.set.row(U.Fab.set.order.label(), U.Fab.set.order.note(), Mob.set.chips.loc(lc, ["on", "off"], U.Fab.set.enabled(st), "order.enabled"))],
      [Mob.set.row(U.Fab.set.label(), U.Fab.set.note.k(U.Fab.enabled(st)), Bool.pick(List<&2, J.Json>, U.Fab.enabled(st), Mob.set.chips.loc(lc, ["on", "off"], U.Fab.set.on(st), "order.estimates"), Nil{}))]])
    case _: Nil{}

def Mob.settings.menu(ps: List<&2, String>, +lc: I.Loc) -> List<&2, J.Json>:
  match ps:
    case Nil{}: Nil{}
    case +p <> t:
      Mob.set.row(U.Settings.title(lc, p), "", [Mob.set.btn(U.Settings.title(lc, p) ++ " ›", "settings-page", p, False{})]) <> Mob.settings.menu(t, lc)

# the menu under Settings' search (client.bend's Settings.found): the pages
# it finds, each noting the rows that matched
def Mob.settings.found(ps: List<&2, String>, +lc: I.Loc, +q: String) -> List<&2, J.Json>:
  match ps:
    case Nil{}: Nil{}
    case +p <> t:
      Mob.set.row(U.Settings.title(lc, p), U.Settings.hint(lc, p, q), [Mob.set.btn(U.Settings.title(lc, p) ++ " ›", "settings-page", p, False{})]) <> Mob.settings.found(t, lc, q)

def Mob.settings.none(none: Bool, +lc: I.Loc) -> List<&2, J.Json>:
  match none:
    case True{}: [Mob.set.row(MSG.T.settings.search.none(lc), "", Nil{})]
    case False{}: Nil{}

def Mob.settings.root(+lc: I.Loc, +u: U.Ui) -> List<&2, J.Json>:
  +q = U.Settings.q(u)
  +ps = U.Settings.found(U.Settings.phone.pages(), lc, q)
  Mob.set.row.f(MSG.T.settings.search(lc), "", [Mob.set.field("setq", MSG.T.settings.search(lc), False{}, U.Ui.scratch(u, "@f.setq"))], Nil{})
    <> List.append(&2, J.Json, Mob.settings.none(List.is_empty(&2, String, ps), lc), Mob.settings.found(ps, lc, q))

def Mob.settings.body(root: Bool, +lc: I.Loc, +u: U.Ui, +st: M.State, +nf: Map<&2, String>, +v: String, +note: String, +more: List<&2, J.Json>) -> List<&2, J.Json>:
  match root:
    case True{}: Mob.settings.root(lc, u)
    case False{}:
      List.append(&2, J.Json, [Mob.set.row(U.Settings.title(lc, U.Settings.page(u)), "", [Mob.set.btn("‹ " ++ MSG.T.palette.settings(lc), "settings-page", "", False{})])],
        Mob.settings.rows.k(C.Codec.pos(U.Settings.pages(), U.Settings.page(u)), lc, u, st, nf, v, note, more, M.Setting.get(st, "agent.provider", "claude"), U.Str.get(nf, "models"), M.Setting.get(st, "agent.model", "")))

def Mob.settings.of.loc(+lc: I.Loc, +u: U.Ui, +st: M.State, +nf: Map<&2, String>, +v: String, +note: String, +more: List<&2, J.Json>) -> J.Json:
  J.Json.obj([J.KV{"rows", J.Json.arr(List.append(&2, J.Json, Mob.settings.body(String.is_empty(U.Settings.page(u)), lc, u, st, nf, v, note, more), Mob.notify(lc, st)))}])

def Mob.settings.of(+u: U.Ui, st: M.State, nf: Map<&2, String>, v: String, note: String, more: List<&2, J.Json>) -> J.Json:
  Mob.settings.of.loc(U.Lang.loc(u), u, st, nf, v, note, more)

def Mob.agy(+lc: I.Loc, +u: U.Ui) -> List<&2, J.Json>:
  Mob.agy.rows(U.Agv.rows.loc(lc, u))

def Mob.settings.k(+lc: I.Loc, +u: U.Ui, more: List<&2, J.Json>) -> J.Json:
  match u:
    case U.Ui{+st, _, _, _, _, _, _, _, _, +v, _, _, _, +note, +nf, _, _}:
      Mob.settings.of.loc(lc, u, st, nf, v, note, more)

# shown while "flag settings" has it open (the API keys' rows last)
def Mob.settings(+lc: I.Loc, open: Bool, +u: U.Ui) -> J.Json:
  match open:
    case True{}:
      Mob.settings.k(lc, u, Nil{})
    case False{}:
      J.Null{}

# A list of rows that each send an action with a value (the search, the
# file picker, the project picker)
def Mob.items(ps: List<&2, U.PItem>) -> List<&2, J.Json>:
  match ps:
    case Nil{}:
      Nil{}
    case U.PItem{+l, +a, v} <> t:
      J.Json.obj([J.KV{"label", J.Json.str(l)}, J.KV{"action", J.Json.str(a)}, J.KV{"value", J.Json.str(v)}, J.KV{"kind", J.Json.str(U.Palette.kind(a))}]) <> Mob.items(t)

# thread search or the file picker ("find-open" search/files), while open
def Mob.find(+lc: I.Loc, open: Bool, +u: U.Ui) -> J.Json:
  match open:
    case True{}:
      J.Json.obj([J.KV{"mode", J.Json.str(U.Palette.mode(u))}, J.KV{"query", J.Json.str(U.Palette.query(u))},
        J.KV{"rows", J.Json.arr(Mob.items(List.take(&2, U.PItem, U.Palette.items.loc(lc, u), 100n)))}])
    case False{}:
      J.Null{}

def Mob.focus.thread.r(+lc: I.Loc, focus: Bool, +u: U.Ui, +sel: String, render: Bool) -> J.Json:
  match focus:
    case True{}:
      Mob.main.r(lc, String.is_empty(sel), u, sel, render)
    case False{}:
      J.Null{}

def Mob.focus.thread(+lc: I.Loc, focus: Bool, +u: U.Ui, +sel: String) -> J.Json:
  Mob.focus.thread.r(lc, focus, u, sel, True{})

def Mob.focus.bot(+lc: I.Loc, focus: Bool, +u: U.Ui) -> J.Json:
  match focus:
    case True{}:
      MB.MBots.json(lc, u)
    case False{}:
      J.Null{}

def Mob.now(u: U.Ui) -> Nat:
  match u:
    case U.Ui{_, _, _, _, _, _, now, _, _, _, _, _, _, _, _, _, _}:
      now

def Mob.open(u: U.Ui) -> Map<&2, Bool>:
  match u:
    case U.Ui{_, _, _, _, o, _, _, _, _, _, _, _, _, _, _, _, _}:
      o

def Mob.scratch(u: U.Ui) -> Map<&2, String>:
  match u:
    case U.Ui{_, _, dr, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      dr

# ios-snappy: tabs. The thread list's view ("projects" or "active") is the
# device's own choice when it has made one (`Backplane.local("view", ..)`
# leaves it in each hub's scratch as @local.view), else the hub's setting
def Mob.view.of(+local: String, +hub: String) -> String:
  Bool.pick(String, Bool.or(String.eq(local, "projects"), String.eq(local, "active")), local, hub)

def Mob.view(+u: U.Ui) -> String:
  Mob.view.of(U.Ui.scratch(u, "@local.view"), U.Side.view(U.Ui.state.of(u)))

# the whole screen. A hub out of focus (focus False) builds no thread, bot
# or viewer (null: the merged screen reads them from the hub in focus),
# and the sidebar's groups are found once
def Mob.up(j: J.Json) -> Bool:
  match j:
    case J.Null{}:
      False{}
    case _:
      True{}

# something else is presented over the list (so the first-run tour waits,
# and shows once all of it is closed): an error, a menu, a delete, rename or
# remove question, Settings, Find, the folder picker, a bot or room form,
# and in the thread on screen its viewer, diff, terminal, history menu, and
# a bot's routine form. The apps add only what they keep to themselves (a
# picture opened large, the hubs sheet, the camera).
# when this phone last heard its hub (seconds; 0 never): the scratch
# "@heard", stamped while it is online (hubs.bend's One.run)
def Mob.heard(+u: U.Ui) -> Nat:
  M.Switch.num(U.Ui.scratch(u, "@heard"), 0n)

def Mob.heard.k(on: Bool, u: U.Ui, +now: Nat) -> U.Ui:
  match on:
    case True{}:
      U.Ui.scratch_set(u, "@heard", Nat.show(now))
    case False{}:
      u

def Mob.heard.set(u: U.Ui) -> U.Ui:
  match u:
    case U.Ui{st, sel, dr, live, op, +on, +now, ad, pa, v, rid, w, er, note, nf, ob, tr}:
      Mob.heard.k(on, U.Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}, now)

def Mob.covered.thread(+th: J.Json, +bot: J.Json) -> Bool:
  Bool.or(Bool.or(Bool.not(String.is_empty(J.Json.get_str(J.Json.get_obj(th, "viewer"), "open", ""))), Mob.up(J.Json.get_obj(th, "diff"))),
    Bool.or(Bool.or(Mob.up(J.Json.get_obj(th, "term")), Mob.up(J.Json.get_obj(J.Json.get_obj(J.Json.get_obj(th, "viewer"), "hist"), "menu"))),
      J.Json.get_bool(J.Json.get_obj(bot, "routine"), "open", False{})))

def Mob.covered.any(js: List<&2, J.Json>) -> Bool:
  match js:
    case Nil{}:
      False{}
    case h <> t:
      Bool.pick(Bool, Mob.up(h), True{}, Mob.covered.any(t))

def Mob.covered(+e: String, +ups: List<&2, J.Json>, +th: J.Json, +bot: J.Json) -> Bool:
  Bool.or(Bool.or(Bool.not(String.is_empty(e)), Mob.covered.any(ups)), Mob.covered.thread(th, bot))

def Mob.tour.gate(+lc: I.Loc, covered: Bool, +u: U.Ui) -> J.Json:
  match covered:
    case True{}:
      J.Null{}
    case False{}:
      Mob.tour(lc, u)

# The legacy island loop re-reads turn state from State. This display-only
# loop instead uses the activity-filtered temporary thread, retaining the
# existing top-level/personal-work policy and unchanged canonical State.
def Mob.signal.compact.go(ts: List<&2, M.Thread>, +st: M.State, +children: Map<&2, Bool>, acc: NO.Compact) -> NO.Compact:
  match ts:
    case Nil{}: acc
    case +t <> rest:
      Mob.signal.compact.go(rest, st, children, NO.Island.compact.add(NO.Island.compact.keep(st, children, t), st, M.Thread.id(t), acc))

def Mob.signal.compact(heard: Bool, ts: List<&2, M.Thread>, +st: M.State, info: Map<&2, String>) -> NO.Compact:
  match heard:
    case False{}: NO.Compact{0n, "", 0n}
    case True{}:
      Mob.signal.compact.go(SG.Signal.island.threads(ts, info), st, NO.Island.compact.children(st), NO.Compact{0n, "", 0n})

def Mob.screen.go(+lc: I.Loc, +focus: Bool, +u: U.Ui, +ix: U.SIx, +g: U.SGroups, +side: MSide) -> J.Json:
  match u:
    case U.Ui{+st, +sel, +dr, _, +o, +on, +now, _, _, +v, _, _, +e, +note, +nf, _, _}:
      +compact = Mob.signal.compact(N.Note.heard(on, now, Mob.heard(u)), U.Ui.threads(st), st, nf)
      +rmenu = Mob.rmenu(lc, u)
      +deleting = Mob.deleting(lc, u)
      +renaming = Mob.renaming(lc, u)
      +removing = Mob.removing(lc, u)
      +settings = Mob.settings(lc, U.Ui.flag(u, "settings"), u)
      +find = Mob.find(lc, U.Ui.flag(u, "palette"), u)
      +folders = PK.MPick.json.loc(lc, u)
      +newbot = MB.MBots.new(lc, u)
      +newroom = MB.MBots.rnew(u)
      +thread = Mob.focus.thread(lc, focus, u, sel)
      +bot = Mob.focus.bot(lc, focus, u)
      J.Json.obj([J.KV{"online", J.Json.bool(on)}, J.KV{"version", J.Json.str(v)}, J.KV{"seq", J.Json.nat(U.Ui.seq(u))}, J.KV{"error", J.Json.str(e)},
        J.KV{"bubHome", HOP.Hop.snapshot(u)}, J.KV{"lang", Mob.lang.loc(lc, u)}, J.KV{"note", J.Json.str(note)}, J.KV{"sel", J.Json.str(sel)}, J.KV{"making", J.Json.bool(U.Made.waiting(u))}, J.KV{"theme", J.Json.str(M.Setting.get(st, "ui.theme", ""))},
        J.KV{"empty", J.Json.str(Mob.empty.loc(lc, List.length(&2, M.Project, U.Ui.projects(st))))},
        J.KV{"projects", J.Json.arr(Mob.grouped(lc, g, o, ix, st, now, U.Find.q(o, dr), Bool.pick(String, focus, sel, "")))},
        J.KV{"legacy", J.Json.arr(Mob.legacy(ST.Legacy.threads(st), lc, ix, st, now))},
        J.KV{"older", Mob.older(g, o, U.Find.q(o, dr))},
        J.KV{"view", J.Json.str(Mob.view(u))},
        J.KV{"active", J.Json.arr(MSide.rows(side))},
        J.KV{"settled", MSide.settled(side)},
        J.KV{"rowMenu", rmenu},
        J.KV{"search", Mob.search.loc(lc, u)},
        J.KV{"deleting", deleting},
        J.KV{"renaming", renaming},
        J.KV{"removing", removing},
        J.KV{"tour", Mob.tour.gate(lc, Mob.covered(e, [rmenu, deleting, renaming, removing, settings, find, folders, newbot, newroom], thread, bot), u)},
        J.KV{"settings", settings},
        J.KV{"find", find},
        J.KV{"folders", folders},
        J.KV{"island", N.Note.compact.json.loc(lc, compact, st)},
        J.KV{"islandAt", J.Json.nat(NO.Island.compact.at(compact))},
        J.KV{"thread", thread},
        J.KV{"uber", J.Null{}},
        J.KV{"plane", J.Null{}},
        J.KV{"bots", J.Json.arr(MB.MBots.rows(lc, B.BU.rows(u)))},
        J.KV{"rooms", J.Json.arr(MB.MBots.rooms(B.BU.rooms(u)))},
        J.KV{"newBot", newbot},
        J.KV{"newRoom", newroom},
        J.KV{"bot", bot}])

# (the sidebar's rows, Side.rows.ix, made by the caller when it needs them too)
def Mob.screen.signals.go(+lc: I.Loc, +focus: Bool, +u: U.Ui, +ix: U.SIx, +rows: List<&2, U.SRow>) -> J.Json:
  Mob.screen.go(lc, focus, u, ix, U.Side.groups(U.Find.rows(rows, U.Find.q(Mob.open(u), Mob.scratch(u))), Mob.now(u)),
    Mob.side(lc, String.eq(Mob.view(u), "active"), ix, U.Ui.state.of(u), Mob.now(u), U.Find.q(Mob.open(u), Mob.scratch(u)), Mob.open(u), Bool.pick(String, focus, U.Ui.sel(u), "")))

def Mob.screen.signals(+lc: I.Loc, +focus: Bool, +u: U.Ui, ix: U.SIx, rows: List<&2, U.SRow>) -> J.Json:
  Mob.screen.signals.go(lc, focus, u, U.Side.signals.ix(ix, u), rows)

def Mob.screen.ixr(lc: I.Loc, focus: Bool, u: U.Ui, ix: U.SIx, rows: List<&2, U.SRow>) -> J.Json:
  Mob.screen.signals(lc, focus, U.Side.with_signals(u), ix, rows)

def Mob.screen.ix(+lc: I.Loc, +focus: Bool, +u: U.Ui, +ix: U.SIx) -> J.Json:
  Mob.screen.ixr(lc, focus, u, ix, U.Side.rows.ix(U.Ui.state.of(u), ix, Mob.now(u)))

# (what every row asks of the whole state is gathered once a screen:
# core's Side.ix)
def Mob.screen.f(focus: Bool, +u: U.Ui) -> J.Json:
  Mob.screen.ix(U.Lang.loc(u), focus, u, U.Side.ix(U.Ui.state.of(u)))

def Mob.screen(+u: U.Ui) -> J.Json:
  Mob.screen.f(True{}, u)
