# Frozen origin/main 82e17d2a0 renderers for paired fixture screenshots.
# Test-only; no production import. Actions and shared unchanged helpers retained.
import Base
import ../../src/core/subthreads.bend as ST
import ../../src/core/textscale.bend as TS
import ../../src/core/codec.bend as C
import ../../src/core/title.bend as TTL
import ../../src/core/msgs.bend as MSG
import ../../src/core/kexport.bend as KEX
import ../../src/core/json.bend as J
import ../../src/core/kihealth.bend as KH
import ../../src/core/model.bend as M
import ../../src/core/sidebar-read.bend as SR
import ../../src/core/signal.bend as SG
import ../../src/core/i18n.bend as I
import ../../src/web/dom.bend as D
import ../../src/web/sidebar-slot.bend as SS
import ../../src/web/usage-page.bend as UV
import ../../src/web/settings-menu.bend as SM
import ../../src/web/icon.bend as WI
import ../../src/core/tools.bend as TO
import ../../src/core/client.bend as U
import ../../src/core/usage.bend as UZ
import ../../src/core/usage_page.bend as UP
import ../../src/core/attach.bend as AT
import ../../src/core/vt.bend as VT
import ../../src/core/diff.bend as DF
import ../../src/core/mech.bend as MC
import ../../src/core/look.bend as LO
import ../../src/core/board.bend as B
import ../../src/core/layers.bend as LY
import ../../src/core/schematic.bend as SC
import ../../src/core/hier.bend as HI
import ../../src/core/images.bend as IM
import ../../src/core/fold.bend as FO
import ../../src/core/subs.bend as SB
import ../../src/core/badge.bend as BG
import ../../src/core/todo.bend as TD
import ../../src/core/bot.bend as BT
import ../../src/core/botui.bend as BI
import ../../src/core/ubersync.bend as US
import ../../src/web/bots.bend as WB
import ../../src/core/voice.bend as VC
import ../../src/core/hist.bend as HS
import ../../src/core/review.bend as RVW
import ../../src/core/emb.bend as EM
import ../../src/core/qr.bend as QR
import ../../src/core/onboard.bend as OB
import ../../src/core/sched.bend as SCH
import ../../src/core/tz.bend as TZ
import ../../src/core/stamp.bend as STP
import ../../src/core/order.bend as OR

import ../../src/web/view.bend as V

def Before.thread.live.loc(+lc: I.Loc, +st: M.State, +info: Map<&2, String>, +live: Map<&2, String>, +id: String) -> List<&2, D.Node>:
  V.View.thread.live.hide.loc(lc, Bool.and(String.is_empty(U.Str.get(live, id)), Bool.not(BI.BU.chat.summary(st, id))), info, live, id)

def Before.header(+lc: I.Loc, t: M.Thread, +st: M.State, +chip: String, mini: List<&2, TO.Tool>, +panel: Bool, snoozed: Bool, picking: Bool, term: Bool, +diff: Bool, del: Bool, +ren: String, +rtext: String) -> D.Node:
  match t:
    case M.Thread{+id, +p, title, _, _, turn, _, archived, override, +sa, +ua, pinned, _, +branch, _, _, model, _, _, _, _}:
      D.El{"header", "", [D.At{"class", "thread-head"}, D.At{"style", "--tint:" ++ BG.Badge.css_of(st, p)}], List.concat(&2, D.Node, [[
        D.Dom.button("back", "select", "", MSG.T.tour.title.threads(lc), "‹"),
        D.El{"span", "", [D.At{"class", "badge"}], [WI.WIcon.svg(BG.Badge.icon(st, p), 16)]},
        D.El{"div", "", [D.At{"class", "title-box"}], V.View.title.k.loc(lc, String.eq(ren, id), id, U.Label.title(st, id, TTL.Title.shown(lc, title)), rtext)},
        D.Dom.span("meta", Bool.pick(String, String.is_empty(branch), "", branch))], V.View.boat.chip.loc(lc, chip), [
        D.El{"div", "", [D.At{"class", "tools thread-controls"}], D.Dom.span("control-label", V.View.group.label(lc, False{})) <> List.append(&2, D.Node,
          V.View.header.tools(lc, List.append(&2, TO.Tool, mini, TO.Tools.head.loc(lc, TO.Head{False{}, TO.Tools.settled(override, Nat.is_gt(sa, ua)), pinned, snoozed, archived, del,
            Bool.not(String.is_empty(M.Fork.source(st, id))), term, panel}, "flag", "snooze", "term-toggle", "@term-size"))),
          D.Dom.span("control-label", V.View.group.label(lc, True{})) <> List.take(&2, D.Node, V.View.snoozes.loc(lc, True{}), 3n))}]])}

# the model menu's rows: each model's name, then what its CLI calls it,
# in two columns (the names' as wide as the longest, --namew)

def Before.top(+lc: I.Loc, hd: List<&2, D.Node>, t: M.Thread, +st: M.State, +chip: String, mini: List<&2, TO.Tool>, +panel: Bool, snoozed: Bool, picking: Bool, term: Bool, +diff: Bool, del: Bool, +ren: String, +rtext: String) -> List<&2, D.Node>:

  match hd:
    case Nil{}:
      [Before.header(lc, t, st, chip, mini, panel, snoozed, picking, term, diff, del, ren, rtext)]
    case h <> r:
      h <> r

# The two parts of a thread's page that follow its draft: the composer and
# the asks (an answer with the draft). The page draws them with these, and
# host.js redraws only them for a keystroke (app.bend's composer and asks):
# rebuilding a long thread's page for every key took 280 ms a key.

def Before.workspace(+lc: I.Loc, +cur: String, +panel: Bool, term: Bool) -> D.Node:
  D.El{"nav", "workspace", [D.At{"class", "workspace-bar"}, D.At{"data-loading", MSG.T.viewer.loading(lc)}, D.At{"aria-label", MSG.T.thread.viewer(lc)}],
    V.View.tool(TO.Tools.term.loc(lc, term, "term-toggle", "@term-size")) <>
    D.Dom.button("commands", "ws-palette-open", "", "Command palette · Ctrl+Shift+P / Cmd+Shift+P", V.View.commands.label(lc)) <>
    List.append(&2, D.Node, V.View.destinations(lc, Bool.pick(String, panel, cur, "")),
      List.append(&2, D.Node, D.Dom.when(panel, D.Dom.button("viewer-close", "panel", "", MSG.T.viewer.close(lc), MSG.T.viewer.close(lc))), [D.El{"span", "viewer-feedback", [D.At{"class", "workspace-feedback"}, D.At{"role", "status"}], Nil{}}]))}

def Before.thread.p(+lc: I.Loc, u: U.Ui, t: M.Thread, pend: List<&2, String>, +atts: List<&2, String>, +up: String, +term: Bool, hd: List<&2, D.Node>, vo: List<&2, D.Node>, +mic: String, fab: List<&2, D.Node>, rd: List<&2, D.Node>, mini: List<&2, TO.Tool>, +pill: String) -> D.Node:
  match u t:
    case U.Ui{+st, +sel, +drafts, +live, +op, _, +now, _, +panel, _, _, _, _, _, +nf, _, +tr} M.Thread{+id, p, ti, c, a, +turn, pe, ar, o, sa, ua, pi, en, br, wt, +pr, +mo, rm, im, se, de}:
      D.El{"main", "thread", [D.At{"class", Bool.pick(String, term, "thread-view with-term", "thread-view") ++ Bool.pick(String, U.Flag.get(op, "drop"), " dropping", "")},
        D.At{"data-thread", id}, D.At{"data-drop", "attach"}, D.At{"data-drop-hover", "drop-hover"}], List.concat(&2, D.Node, [
        Before.top(lc, hd, M.Thread{id, p, ti, c, a, turn, pe, ar, o, sa, ua, pi, en, br, wt, pr, mo, rm, im, se, de}, st, UZ.Chip.of(st, id, now), mini, panel, M.Snooze.on(now, U.Side.until(st, id)), U.Flag.get(op, "snooze"), term, String.eq(U.Str.get(nf, "view.kind:" ++ id), "diff"), String.eq(U.Str.get(drafts, "@del"), id), U.Str.get(drafts, "@ren"), U.Str.get(drafts, "@f.title")),
        V.View.kx.note(U.KX.note(nf, sel), nf, sel), V.View.kn.loc(lc, U.KN.thread(nf, st, sel), True{}),
        [Before.workspace(lc, U.Fm.tab(U.Str.get(nf, "view.kind:" ++ id)), panel, term)],
        V.View.thread.workbar(lc, BI.BU.chat.summary(st, id), nf, id, V.View.running(turn)),
        V.View.subthreads(lc, st, id), V.View.thread.subs(lc, st, nf, id, now),
        [D.El{"div", "timeline", [D.At{"class", "timeline"}, D.At{"id", "timeline"}], List.concat(&2, D.Node, [
          fab, V.View.parent(st, id, U.Tasks.up(st, id)),
          [D.El{"div", "work", [D.At{"class", "work"}], V.View.pins.loc(lc, U.Ec.pins(U.Ec.make(U.MailvLocal.terms(tr), st, id, now), st, id), U.Ec.make(U.MailvLocal.terms(tr), st, id, now))}],
          V.View.paged(lc, drafts, M.State.mine(st, id), st, id, nf, V.View.running(turn), U.Ec.make(U.MailvLocal.terms(tr), st, id, now)),
          V.View.sending.loc(lc, pend), Before.thread.live.loc(lc, st, nf, live, id), V.View.hold.pill(pill)])},
         V.View.asks.box(lc, st, id, U.Str.get(drafts, sel), nf, drafts),
         V.View.todos(lc, nf, id, U.Todos.shown(st, id, V.View.running(turn))),
         V.View.queued(lc, M.Queue.context(M.Queue.items(st, id)), V.View.running(turn))],
        rd, [V.View.composer.t(lc, st, sel, drafts, op, nf, turn, pr, mo, atts, up, vo, mic)]])}

def Before.thread.h(+lc: I.Loc, +u: U.Ui, +t: M.Thread, hd: List<&2, D.Node>) -> D.Node:
  Before.thread.p(lc, u, t, U.Ui.pending(u, U.Ui.sel(u)), U.Ui.atts(u, U.Ui.sel(u)), U.Ui.uploading(u, U.Ui.sel(u)), U.Ui.flag(u, "term"), hd, D.Dom.cat(V.View.btw(lc, u), V.View.voice.loc(lc, u, U.Ui.sel(u))), V.View.mic.of(u, U.Ui.sel(u)), Nil{}, V.View.ready(u), U.Fab.mini.tools(u), U.Hold.pill(u, M.Thread.id(t)))

def Before.bar.loc(+lc: I.Loc, kids: List<&2, D.Node>, +hint: String) -> D.Node:
  D.El{"div", "", [D.At{"class", "viewer-bar"}], List.append(&2, D.Node, kids, [D.Dom.span("hint", hint), V.View.fit.loc(lc)])}

# a hierarchical schematic's sheets as chips (the one on show lit; the
# root's value is "", the project's own file); none with one sheet

def Before.board.chips(fs: List<&2, String>, +cur: String) -> List<&2, D.Node>:
  match fs:
    case Nil{}:
      Nil{}
    case +f <> t:
      D.Dom.button(Bool.pick(String, U.Bd.on(cur, f), "on", ""), "board-pick", f, f, U.Bd.name(f)) <> Before.board.chips(t, cur)

def Before.boards(+u: U.Ui, +what: String) -> List<&2, D.Node>:
  Bool.pick(List<&2, D.Node>, U.Bd.many(u), Before.board.chips(U.Bd.files(u), U.Bd.shown(u, U.Ui.view_get(u, "view.path"))), [D.Dom.span("what", what)])

def Before.plot(+lc: I.Loc, +u: U.Ui, +what: List<&2, D.Node>) -> List<&2, D.Node>:
  Before.bar.loc(lc, List.append(&2, D.Node, what, [V.View.layers.loc(lc, String.eq(U.Solid.kind(u), "schematic"), Nat.is_eq(HS.Hist.kind(U.Ui.view_get(u, "view.path")), 2n), U.Solid.off(u), U.Lfocus.of(u))]), MSG.T.viewer.pan.tip(lc))
   <> D.El{"div", "", [D.At{"class", "viewer-stage"}], D.El{"canvas", "plot", [D.At{"id", "plot"}, D.At{"data-key", U.Solid.want(u)},
        D.At{"data-off", U32.show(LY.Masks.off(LY.Lyr.masks(U.Lfocus.kind(u), U.Solid.off(u), U.Lfocus.of(u))))},
        D.At{"data-dim", U32.show(LY.Masks.dim(LY.Lyr.masks(U.Lfocus.kind(u), U.Solid.off(u), U.Lfocus.of(u))))}, D.At{"data-lift", U32.show(LY.Lyr.top(U.Lfocus.of(u)))},
        D.At{"data-fade", Nat.show(U.Hs.fade())}, D.At{"data-fam", HS.Hist.key.family(U.Solid.want(u))}, D.At{"data-sheets", U.Sch.boxes(u)}], Nil{}} <> (V.View.hist.menu(lc, u) <> V.View.rc.loc(lc, u))}
   <> V.View.hist(lc, U.Hs.bar.loc(lc, u), u)

# over a part in 3D: the thread's parts and Renders; the board: its name
# after the parts: Renders for one of them, or the name of another model
# an agent showed (its path on hover)

def Before.solid.what.loc(+lc: I.Loc, on: Bool, +u: U.Ui) -> List<&2, D.Node>:
  match on:
    case True{}:
      List.append(&2, D.Node, V.View.mech.chips(U.Mpage.parts(u), U.Mpage.shown(u)), V.View.solid.more.loc(lc, U.Mpage.listed(u), u))
    case False{}:
      Before.boards(u, MSG.T.viewer.board3d.tip(lc))

# a model turned in 3D: host.js (solid.js) draws the hub's model (a "plot"
# with a mesh, as the phones get) on the canvas whose data-key it carries,
# and says in the note why there is none

def Before.solid.loc(+lc: I.Loc, +u: U.Ui) -> List<&2, D.Node>:
  Before.bar.loc(lc, Before.solid.what.loc(lc, U.Mpage.onpart(u), u), MSG.T.viewer.orbit.tip(lc))
    <> List.append(&2, D.Node, D.Dom.when(Bool.not(String.is_empty(U.Ui.info(u, "plotnote:" ++ U.Solid.want(u)))), D.Dom.span("viewer-say", U.Ui.info(u, "plotnote:" ++ U.Solid.want(u)))),
      [D.El{"div", "", [D.At{"class", "viewer-stage"}], [D.El{"canvas", "solid", [D.At{"id", "solid"}, D.At{"data-key", U.Solid.want(u)}], Nil{}},
        D.El{"div", "", [D.At{"class", "viewer-note"}], Nil{}}, D.El{"div", "", [D.At{"class", "viewer-stat"}], Nil{}}]}])

# The viewer pane: a tab for each thing it shows (the one on show lit),
# Close, then that thing

def Before.aside.k(+lc: I.Loc, i: Nat, +u: U.Ui) -> List<&2, D.Node>:
  match i:
    case 0n:
      V.View.diff.loc(lc, U.Ui.view_get(u, "diff"))
    case 1n:
      V.View.browser.loc(lc, u)
    case 2n:
      V.View.mech(lc, u)
    case 3n:
      Before.solid.loc(lc, u)
    case 4n:
      V.View.sch.after(Before.plot(lc, u, List.append(&2, D.Node, Bool.pick(List<&2, D.Node>, U.Bd.many(u), Before.board.chips(U.Bd.files(u), U.Bd.shown(u, U.Ui.view_get(u, "view.path"))), Nil{}), List.append(&2, D.Node, V.View.sheets.loc(lc, u), [V.View.kicad.loc(lc)]))),
        V.View.sch.below(lc, u))

    case 6n:
      V.View.files(lc, u)
    case 7n:
      V.View.image.loc(lc, u)
    case 8n:
      V.View.pdf.loc(lc, u)
    case 9n:
      V.View.review(lc, u)
    case _:
      Before.plot(lc, u, List.append(&2, D.Node, Before.boards(u, MSG.T.viewer.board.tip(lc)), [V.View.kicad.loc(lc)]))

def Before.aside(+lc: I.Loc, +kind: String, +u: U.Ui) -> D.Node:
  D.El{"aside", "viewer", [D.At{"class", "viewer " ++ Bool.pick(String, String.is_empty(kind), "board", kind)}, D.At{"id", "viewer"}],
    V.View.split.loc(lc) <> V.View.tabs.loc(lc, Bool.pick(String, U.Mpage.onpart(u), "mech", U.Fm.tab(U.Solid.kind(u)))) <> Before.aside.k(lc, V.View.aside.n(kind), u)}

def Before.aside.if(+lc: I.Loc, open: Bool, +kind: String, +u: U.Ui) -> List<&2, D.Node>:
  match open:
    case True{}:
      [Before.aside(lc, kind, u)]
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
# a choice is a button, or (the policy, the issue form) a link to a new tab

