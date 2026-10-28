import Base
import ../core/signal.bend as SGR
import ../core/i18n.bend as I
import ../core/msgs.bend as MSG
import ../core/json.bend as J
import ./dom.bend as D
import ../core/client.bend as U
import ./view.bend as V
import ../core/cbor.bend as C
import ../core/attach.bend as AT
import ../core/lightbox.bend as LB
import ../core/desk.bend as DK
import ./mail-actions.bend as MA
import ./presentation.bend as P
import ../core/presentation.bend as PS
import ./cat.bend as WC
import ../core/onboard.bend as OB
import ./workspace.bend as WS

import ../core/bub-move.bend as HOP

# What host.js calls. Everything the page decides happens behind these.

def lang(u: U.Ui) -> String:
  U.Ui.scratch(u, "@lang.tag")

def lang_preview(u: U.Ui) -> U.Ui:
  U.Ui.scratch_set(u, "@lang.preview", "1")

# a page this build drew itself uses the rail and the top bar; a hub's
# session keeps what its host asked for (thin-view's Session.renderer)
def stamp(+u: U.Ui) -> U.Ui:
  U.Ui.info_set(U.Ui.info_set(U.Ui.info_set(u, "local.mailroom", "on"), "local.workspace.layout",
    Bool.pick(String, String.is_empty(U.Ui.info(u, "local.workspace.layout")), "rail-v1", U.Ui.info(u, "local.workspace.layout"))), "local.workspace.chrome",
    Bool.pick(String, String.is_empty(U.Ui.info(u, "local.workspace.chrome")), "top-v1", U.Ui.info(u, "local.workspace.chrome")))

def ready(a: U.Act) -> U.Act:
  match a:
    case U.Act{u, cs}: U.Mailv.sync(U.Act{stamp(u), cs})

def init(now: Nat, cid: String) -> U.Ui:
  stamp(U.Ui.set_cid(U.Ui.init(now), cid))

# a draft this browser kept (Keep commands), read back at start
def restore(u: U.Ui, id: String, text: String) -> U.Ui:
  U.Ui.restore(u, id, text)

# a bot's browser has a new frame ({"t":"shot","bot":...,"n":...}): the
# Browser tab loads /bots/<bot>/screen.jpg?n=<n>, and says it is on show
# (client.bend's Ui.cast.seen) so every frame of the page comes
def recv.k(shot: Bool, u: U.Ui, +o: J.Json) -> U.Act:
  match shot:
    case True{}:
      U.Ui.cast.seen(U.Ui.info_set(u, "shot." ++ J.Json.get_str(o, "bot", ""), J.Json.as_raw(J.Json.get(o, "n"), "")), J.Json.get_str(o, "bot", ""))
    case False{}:
      U.Ui.recv_act(u, o)

# a 3D model's note ({"t":"plotnote", key, text}: parts with no model),
# kept under its source's key for the 3D pane
# a schematic's sheets ({"t":"sheets", root, rows, boxes}: core/hier.bend's
# rows and sheet boxes), kept under the project root for the Schematic
# tab's navigation (client.bend's Sch)
def recv.sheets(hit: Bool, u: U.Ui, +o: J.Json) -> U.Act:
  match hit:
    case True{}:
      U.Act{U.Sch.got(u, o), Nil{}}
    case False{}:
      recv.k(String.eq(J.Json.get_str(o, "t", ""), "shot"), u, o)

def recv.note(note: Bool, u: U.Ui, +o: J.Json) -> U.Act:
  match note:
    case True{}:
      U.Act{U.Ui.info_set(u, "plotnote:" ++ J.Json.get_str(o, "key", ""), J.Json.get_str(o, "text", "")), Nil{}}
    case False{}:
      recv.sheets(String.eq(J.Json.get_str(o, "t", ""), "sheets"), u, o)

# (the viewer asks the hub for its board, schematic or model when the
# source on screen changes)
def recv(u: U.Ui, +o: J.Json) -> U.Act:
  MA.MA.after(U.Solid.sync(ready(recv.note(String.eq(J.Json.get_str(o, "t", ""), "plotnote"), MA.MA.recv(u, o), o))))

# a bot's cat for an <img data-cat="<look>:<mood>:<px>"> (host.js keeps
# each answer)
def cat(name: String) -> String:
  WC.WCat.uri(name)

# the page's own address (a webhook's URL starts with it)
def page(u: U.Ui, origin: String) -> U.Ui:
  U.Ui.scratch_set(u, "@page", origin)

# where a Connect goes: the other hub's page, carrying the bot to open
# there (U.Far.opened's "@bot.want") in the address's fragment
def visit(+u: U.Ui, +url: String) -> String:
  Bool.pick(String, String.is_empty(U.Ui.scratch(u, "@bot.want")), url ++ "/", url ++ "/#bot=" ++ U.Ui.scratch(u, "@bot.want"))

# the page opened with "#bot=<name>" (decoded): that bot is shown once the
# log has it
def wanted(+u: U.Ui, +hash: String) -> U.Ui:
  Bool.pick(U.Ui, String.starts_with(hash, "#bot="), U.Want.bot(u, String.drop(hash, 5n)), u)

def Rv.focus.k(a: U.Act, cs: List<&2, U.Cmd>) -> U.Act:
  match a:
    case U.Act{+u, +ds}:
      U.Act{u, List.append(&2, U.Cmd, ds, cs)}

# a layer focus that moved: a comparison's marks follow it (core's rv-bits),
# before Solid.sync, so the watch it sends is for the comparison's new path
def Rv.focus(on: Bool, a: U.Act) -> U.Act:
  match on a:
    case True{} U.Act{+u, +cs}:
      Rv.focus.k(U.Ui.act(u, "rv-bits", U32.show(U.Rv.bits(U.Lfocus.kind(u), U.Lfocus.of(u)))), cs)
    case False{} +x:
      x

def act.other(u: U.Ui, +action: String, value: String) -> U.Act:
  U.Solid.sync(Rv.focus(Bool.or(String.eq(action, "view-layer-pick"), String.eq(action, "view-layer-mode")), U.Ui.act(u, action, value)))

def act.page(page: Bool, u: U.Ui, action: String, value: String) -> U.Act:
  match page:
    case True{}: U.Mailv.sync(U.Act{U.Ui.info_set(u, "pane:mr", value), Nil{}})
    case False{}: act.other(u, action, value)

def act.browser(browser: Bool, +u: U.Ui, +action: String, value: String) -> U.Act:
  match browser:
    case True{}: U.Teach.act.k(U.Keys.pos(U.Teach.actions(), action), u, U.Browse.owner(u), value)
    case False{}: act.page(String.eq(action, "web-mail-page"), u, action, value)

def act.project(project: Bool, u: U.Ui, +action: String, value: String) -> U.Act:
  match project:
    case True{}: U.Ui.act(u, "mail-filter", "project=" ++ value)
    case False{}: act.page(String.eq(action, "web-mail-page"), u, action, value)

def act.mail(i: Nat, u: U.Ui, +action: String, value: String) -> U.Act:
  match i:
    case 0n: MA.MA.action(0n, u, value)
    case 1n: MA.MA.action(1n, u, value)
    case 2n: MA.MA.action(2n, u, value)
    case _: act.project(String.eq(action, "web-mail-project"), u, action, value)

def act.workspace(workspace: Bool, u: U.Ui, +action: String, value: String) -> U.Act:
  match workspace:
    case True{}: U.Solid.sync(WS.WS.act(action, stamp(u), value))
    case False{}: act.mail(U.Keys.pos(["mail-send", "mail-open", "mail-mark"], action), stamp(u), action, value)

def act(u: U.Ui, +action: String, +value: String) -> U.Act:
  act.workspace(Bool.or(WS.WS.handles(action), Bool.and(String.eq(action, "bfield"), String.starts_with(value, "wspalette\u{1f}"))), u, action, value)

# a timeline gap px from its end is at the end and follows; further up it
# holds what is read (client.bend's Hold; host.js's held)
def hold_at_end(gap: Nat) -> Bool:
  U.Hold.at_end(gap)

# The ordinary view and the output-only lane use the same keyed pane.
def terminal_node.attrs(+u: U.Ui, node: D.Node) -> D.Node:
  match node:
    case D.El{tag, key, attrs, kids}:
      D.El{tag, key, List.append(&2, D.Attr, [D.At{"data-terminal-sequence", U.Ui.info(u, "local.terminal.sequence")},
        D.At{"data-terminal-incarnation", U.Ui.info(u, "local.terminal.incarnation")}, D.At{"data-terminal-pid", U.Ui.info(u, "local.terminal.pid")}], attrs), kids}
    case _: node

def terminal_node(+u: U.Ui) -> D.Node:
  terminal_node.attrs(u, V.View.term.loc(U.Lang.loc(u), U.Ui.term(u, WS.WS.owner(u))))

def view_sidebar(+u: U.Ui, reuse: Bool) -> D.Node:
  WS.WS.view(u, V.View.page.sidebar(stamp(u), reuse), terminal_node(u))

# Server-only entry: raw canonical signal bits are supplied explicitly;
# session/workspace/terminal state and signal visibility remain current.
def view_sidebar_indexed.page(raw: Bool, +u: U.Ui, reuse: Bool, +signals: Map<&2, Nat>) -> D.Node:
  match raw:
    case True{}: V.View.page.signals.sidebar.indexed(U.Side.with_signals.given(u, signals), reuse, signals)
    case False{}: V.View.page.signals.sidebar(U.Side.with_signals.given(u, signals), reuse)

def view_sidebar_indexed(+u: U.Ui, reuse: Bool, signals: Map<&2, Nat>) -> D.Node:
  WS.WS.view(u, view_sidebar_indexed.page(SGR.Signal.info.raw(Map.keys(&2, String, U.Ui.infos(u))), stamp(u), reuse, signals), terminal_node(u))

def view_sidebar_metadata(+u: U.Ui, reuse: Bool, plain: U.SIx, +signals: Map<&2, Nat>) -> D.Node:
  WS.WS.view(u, V.View.page.signals.sidebar.metadata(U.Side.with_signals.given(stamp(u), signals), reuse,
    SGR.Signal.info.raw(Map.keys(&2, String, U.Ui.infos(u))), plain, signals), terminal_node(u))

# Canonical text comes from a bounded hub-owned epoch/revision/online memo.
# Unknown reserved recipient entries still select the original fallback.
# Workspace, terminal and all recipient values are current for this frame.
def view_sidebar_materialized(+u: U.Ui, reuse: Bool, plain: U.SIx, signals: Map<&2, Nat>, text: Map<&2, String>) -> D.Node:
  WS.WS.view(u, V.View.page.signals.sidebar.metadata(U.Side.with_signal_text(stamp(u), text), reuse,
    SGR.Signal.info.raw(Map.keys(&2, String, U.Ui.infos(u))), plain, signals), terminal_node(u))

def view(+u: U.Ui) -> D.Node:
  view_sidebar(u, False{})

# Optional early presentation uses exactly the normal keyed view. These
# exports move renderer data only; they cannot install a Ui or mark it synced.
def presentation_view(u: U.Ui) -> J.Json:
  P.Node.write(view(u))

def presentation_node(json: J.Json) -> D.Node:
  P.Node.read(json)

# Only the shared allowlisted presentation choices cross the final handoff.
# State, live text, replay origin/cursor and typed caches stay with this Ui.
def presentation(u: U.Ui) -> J.Json:
  PS.Session.pack(u)

def presentation_handoff(u: U.Ui, session: J.Json) -> U.Ui:
  U.Ui.scratch_set(PS.Session.adopt(u, session), "@presentation", "")

def presentation_reply(session: J.Json, reply: J.Json) -> J.Json:
  PS.Session.reply(session, reply)

def presentation_json(text: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{})

def presentation_choice(action: String, session: J.Json, value: String) -> J.Json:
  match action:
    case "select": PS.Session.select(session, value)
    case "draft": PS.Session.draft(session, value)
    case _: session

def presentation_prepare(session: J.Json, input: J.Json, action: String) -> J.Json:
  PS.Session.prepare(session, input, action)

def presentation_select(u: U.Ui, id: String) -> U.Ui:
  U.Ui.select.nav(u, id)

def presentation_perform(+u: U.Ui, input: J.Json, +action: String, value: String) -> U.Act:
  act(presentation_handoff(u, PS.Session.prepare(presentation(u), input, action)), action, value)

def presentation_allowed(action: String) -> Bool:
  match action:
    case "select": True{}
    case "draft": True{}
    case "send": True{}
    case "send-alt": True{}
    case "fold": True{}
    case "subs-open": True{}
    case "subs-shut": True{}
    case "skill-close": True{}
    case _: False{}

def presentation_wait(u: U.Ui) -> U.Ui:
  U.Ui.scratch_set(u, "@presentation", "1")

def presentation_blocked(+u: U.Ui) -> U.Ui:
  U.Ui.error_set(u, MSG.T.viewer.loading(U.Lang.loc(u)))

# A corrupt cache is discarded explicitly. A fresh recipient owns the empty
# origin/cursor/live/body stores; only allowlisted UI choices cross back.
def presentation_reset(+u: U.Ui, now: Nat, cid: String) -> U.Ui:
  presentation_handoff(init(now, cid), presentation(u))

def presentation_overlay(node: D.Node, +session: J.Json) -> D.Node:
  P.Node.read.first(V.View.local(P.Node.overlay(node, J.Json.get_str(J.Json.get_obj(session, "drafts"), J.Json.get_str(session, "sel", ""), ""),
    J.Json.get_bool(J.Json.get_obj(session, "open"), "models", False{})), J.Json.get_str(J.Json.get_obj(session, "drafts"), J.Json.get_str(session, "sel", ""), "")))

# this page is a desk (core/desk.bend): host.js calls this after input,
# focus and visibility changes; it answers the desk.look to send, when due
def desk(u: U.Ui, focused: Bool, now: Nat) -> U.Act:
  U.Ui.desk(stamp(u), focused, now)

def synced(u: U.Ui) -> Bool:
  U.Tour.ready(u)

def desktop(u: U.Ui) -> U.Ui:
  U.Ui.scratch_set(u, "@desktop", "1")

def desktop_browser(u: U.Ui) -> U.Ui:
  U.Ui.scratch_set(u, "@desktop.browser", "1")

def online(u: U.Ui, +b: Bool) -> U.Ui:
  U.Ui.online(MA.MA.online(u, b), b)

# whether a link last heard from at `last` (ms) is dead at `now`: the hub
# sends a heartbeat every 15 s (core/desk.bend's Alive)
def alive_due(last: Nat, now: Nat) -> Bool:
  DK.Alive.due(last, now)

# the saved log put back on load is not a connection's: nothing is synced
# until the hub's own log arrives
def unsync(u: U.Ui) -> U.Ui:
  U.Ui.unsync(u)

def tick(u: U.Ui, now: Nat) -> U.Ui:
  U.Ui.tick(u, now)

# how many changes this page holds and which log they came from (a
# reconnect asks the server for the rest)
def seq(u: U.Ui) -> String:
  Nat.show(U.Ui.seq(u))

def origin(u: U.Ui) -> String:
  U.Ui.info(u, "origin")

# The wire: CBOR bytes in and out (the host only moves them)
# the selected thread's composer and asks as the page draws them: host.js
# redraws only these for a keystroke (view.bend's View.composer.u)
def composer(u: U.Ui) -> D.Node:
  V.View.composer.u(u)

def asks(+u: U.Ui) -> D.Node:
  V.View.asks.u(U.Lang.loc(u), u)

# the secret box's Save (host.js's secretSave): "<name>\u001f<purpose>\u001f
# <thread>\u001f<value>" to {"rpc": the secret.set request} or {"error"}
def secret_req(+packed: String) -> String:
  U.Sbox.req(packed)

# A sealed form's Save (host.js's sealedSave; client.bend's Sealed):
# "<form>\u001f<field>..." to {"rpc"} or {"error"}
def sealed_req(+packed: String) -> String:
  U.Sealed.req(packed)

# A value of each Json constructor, for wire.js: bend 2.0.36 tags a module's
# constructors with their file ("../core/json.Str"), so the JavaScript that
# builds Json learns the tags from Bend instead of spelling them.
def json_tags() -> J.Json:
  J.Arr{J.Item{J.Null{}, J.Item{J.Flag{False{}}, J.Item{J.Num{"0"}, J.Item{J.Str{""}, J.Item{J.Obj{J.Field{"", J.Null{}, J.End{}}}, J.End{}}}}}}}

# the wire's dictionaries, for host.js's own decoder (wire.js)
# a Node with each Node and Attr constructor, for host.js (same reason)
def dom_tags() -> D.Node:
  D.El{"", "", [D.At{"", ""}], [D.Txt{""}]}

def cbor_keys() -> List<&2, String>:
  C.Cbor.keys()

def cbor_words() -> List<&2, String>:
  C.Cbor.words()

def wire_in(bs: List<&2, U32>) -> J.Json:
  Maybe.default(&2, J.Json, C.Cbor.decode(bs), J.Null{})

def wire_out(text: String) -> List<&2, U32>:
  C.Cbor.encode(Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{}))

def show(j: J.Json) -> String:
  J.Json.show(j)

# what a message changes on the page: "" nothing (U.Ui.shows says no: a
# heartbeat, another thread's streamed text; or a delta_end, which the Ui
# does not fold), "live" only the shown thread's streaming row (a delta:
# U.Ui.delta adds to that thread's live text, and only that row draws it,
# view.bend's View.live.of), else "page"
def part.k(shows: Bool, +kind: String) -> String:
  match shows:
    case False{}:
      ""
    case True{}:
      Bool.pick(String, String.eq(kind, "delta"), "live", Bool.pick(String, String.eq(kind, "delta_end"), "", "page"))

def part(+u: U.Ui, +o: J.Json) -> String:
  part.k(U.Ui.shows(u, o), J.Json.get_str(o, "t", ""))

# the selected thread's live row as the page draws it, keyed
# "live:<thread>": host.js patches the timeline's row with that key in place,
# and draws the page when there is none
def live(+u: U.Ui) -> D.Node:
  V.View.live.u.loc(U.Lang.loc(u), u)

# the node a "bp-memo" element of the page stands for (view.bend's
# View.memo): host.js keeps it while the element stays the same
def make(n: D.Node) -> D.Node:
  V.View.make(n)

# a message's kind ("log", "changes", "delta"...): the host keeps only the
# log and its changes
def kind(+j: J.Json) -> String:
  J.Json.get_str(j, "t", "")

# how many bytes of a file go in one upload piece
def chunk() -> String:
  Nat.show(AT.Att.chunk())

# the thread a file goes to (POST /attach), and what it is uploading
# ("attach-up" refused a file over the cap when it is not that file)
def sel(u: U.Ui) -> String:
  U.Ui.sel(u)

def uploading(u: U.Ui, thread: String) -> String:
  U.Ui.uploading(u, thread)

# the shortcuts the page takes for itself, comma-separated (in the
# terminal, and elsewhere)
def keys_term() -> String:
  String.join(U.Ui.keys(True{}), ",")

def keys_page() -> String:
  String.join(U.Ui.keys(False{}), ",")

# The image lightbox (core/lightbox.bend decides the zoom, the pan, the
# easing and what closes it; host.js feeds it pointer events and sets the
# style it answers on the picture)

def lb_node.loc(+lc: I.Loc, url: String) -> D.Node:
  D.El{"div", "", [D.At{"class", "lightbox"}, D.At{"role", "dialog"}, D.At{"aria-modal", "true"}, D.At{"aria-label", MSG.T.native.image.plain(lc)}], [
    D.El{"img", "", [D.At{"class", "lb-img"}, D.At{"src", url}, D.At{"alt", ""}, D.At{"draggable", "false"}], Nil{}},
    D.El{"div", "", [D.At{"class", "lb-tools"}], [
      D.El{"button", "", [D.At{"data-lb-copy", "1"}], [D.Txt{MSG.T.image.copy(lc)}]},
      D.El{"button", "", [D.At{"data-lb-reveal", "1"}, D.At{"hidden", ""}], [D.Txt{MSG.T.image.show(lc)}]},
      D.El{"button", "", [D.At{"data-lb-save", "1"}], [D.Txt{MSG.T.common.save(lc)}]},
      D.El{"button", "", [D.At{"data-lb-share", "1"}, D.At{"hidden", ""}], [D.Txt{MSG.T.image.share(lc)}]}]},
    D.El{"div", "", [D.At{"class", "lb-status"}, D.At{"data-lb-status", "1"}, D.At{"role", "status"}], Nil{}},
    D.El{"button", "", [D.At{"class", "lb-close"}, D.At{"title", MSG.T.common.close(lc)}, D.At{"aria-label", MSG.T.common.close(lc)}, D.At{"data-lb-close", "1"}], [D.Txt{"×"}]}]}

def lb_node(url: String) -> D.Node:
  lb_node.loc(I.En{}, url)

def lb_open() -> LB.Lb:
  LB.Lb.open()

def lb_box(iw: Nat, ih: Nat, vw: Nat, vh: Nat) -> LB.LbBox:
  LB.LbBox{iw, ih, vw, vh}

# a wheel turn (dy > 0 zooms out); a trackpad pinch comes as a ctrl wheel
# with small steps, applied at once and made larger
def lb_wheel(lb: LB.Lb, down: Bool, +mag: Nat, +pinch: Bool, cx: Nat, cy: Nat, b: LB.LbBox, now: Nat) -> LB.Lb:
  LB.Lb.wheel(lb, Bool.not(down), Bool.pick(Nat, pinch, Nat.mul(mag, 4n), mag), Bool.not(pinch), cx, cy, b, now)

def lb_pinch(lb: LB.Lb, d0: Nat, d1: Nat, cx: Nat, cy: Nat, b: LB.LbBox, now: Nat) -> LB.Lb:
  LB.Lb.pinch(lb, d0, d1, cx, cy, b, now)

def lb_press(lb: LB.Lb, x: Nat, y: Nat, b: LB.LbBox, now: Nat) -> LB.Lb:
  LB.Lb.press(lb, x, y, b, now)

def lb_move(lb: LB.Lb, x: Nat, y: Nat, b: LB.LbBox, now: Nat) -> LB.Lb:
  LB.Lb.move(lb, x, y, b, now)

def lb_release(lb: LB.Lb, x: Nat, y: Nat, b: LB.LbBox, now: Nat) -> LB.Lb:
  LB.Lb.release(lb, x, y, b, now)

def lb_closes(lb: LB.Lb, x: Nat, y: Nat) -> Bool:
  LB.Lb.closes(lb, x, y)

def lb_key(k: String) -> Bool:
  String.eq(k, "Escape")

def lb_stamp(lb: LB.Lb, now: Nat) -> LB.Lb:
  LB.Lb.stamp(lb, now)

def lb_busy(lb: LB.Lb, now: Nat) -> Bool:
  LB.Lb.busy(lb, now)

def lb_css(lb: LB.Lb, b: LB.LbBox, now: Nat) -> String:
  LB.Lb.css(lb, b, now)

# the tour's action for a key while its dialog is up ("" for any other)
def tour_key(key: String) -> String:
  OB.Ob.dom(key)

# The host passes the frame's already resolved language tag.
def lb_node_in(tag: String, url: String) -> D.Node:
  lb_node.loc(I.I18n.of_tag(tag), url)

# The WebGL host supplies measurements; Bend chooses their displayed text.
def solid_waiting(tag: String, +seconds: Nat) -> String:
  MSG.T.viewer.solid.waiting(I.I18n.of_tag(tag), Nat.show(seconds), Bool.pick(String, Nat.is_ge(seconds, 15n), "yes", "no"))

def solid_stats(tag: String, size: String, triangles: String, waited: String, built: String, delayed: Bool) -> String:
  MSG.T.viewer.solid.stats(I.I18n.of_tag(tag), size, triangles, Bool.pick(String, delayed, "yes", "no"), waited, built)

# The host persists only this conversation's draft/receipt, never commands.
def mail_identity(+u: U.Ui) -> String:
  Bool.pick(String, String.is_empty(U.Ui.scratch(u, "@mr.conv")), "", MA.MA.key(u))

def mail_snapshot(u: U.Ui) -> String:
  MA.MA.snapshot(u)

def mail_restore(u: U.Ui, text: String) -> U.Ui:
  MA.MA.restore(u, text)


# Local draft bookkeeping for the current authenticated thin projection.
# This changes no read model, online/sync flag, outbox, cache body or command.
# Only the Mailroom reply field is supported; settings/password fields cannot
# enter this helper or the existing conversation-local snapshot policy.
def mail_local_context_if(valid: Bool, +u: U.Ui, +context: J.Json) -> U.Ui:
  match valid:
    case False{}: U.Ui.scratch_set(u, "@mr.conv", "")
    case True{}:
      U.Ui.scratch_set(U.Ui.scratch_set(U.Ui.info_set(U.Ui.scratch_set(u, "@bot", J.Json.get_str(context, "owner", "")), "origin", J.Json.get_str(context, "origin", "")),
        "@mr.conv", J.Json.get_str(context, "conv", "")), "@mr.to", J.Json.get_str(context, "recipient", ""))

def mail_local_context(+u: U.Ui, +context: J.Json) -> U.Ui:
  mail_local_context_if(Bool.and(String.eq(J.Json.get_str(context, "kind", ""), "mail-draft"),
    Bool.and(Bool.not(String.is_empty(J.Json.get_str(context, "owner", ""))), Bool.and(Bool.not(String.is_empty(J.Json.get_str(context, "origin", ""))),
      Bool.not(String.is_empty(J.Json.get_str(context, "conv", "")))))), u, context)

def mail_local_field(u: U.Ui, text: String) -> U.Ui:
  U.Ui.scratch_set(u, "@f.mrreply", text)

def mail_local_text(u: U.Ui) -> String:
  U.Ui.scratch(u, "@f.mrreply")

# Tiny lease comparison shared by the web and phone presentation hosts.
def bub_move(before: String, after: String) -> J.Json:
  HOP.Hop.text(before, after)
