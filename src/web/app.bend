import Base
import ../core/json.bend as J
import ./dom.bend as D
import ../core/client.bend as U
import ./view.bend as V
import ../core/cbor.bend as C
import ../core/attach.bend as AT
import ../core/lightbox.bend as LB
import ../core/desk.bend as DK
import ./cat.bend as WC
import ../core/onboard.bend as OB

# What host.js calls. Everything the page decides happens behind these.

def lang(u: U.Ui) -> String:
  U.Ui.scratch(u, "@lang.tag")

def lang_preview(u: U.Ui) -> U.Ui:
  U.Ui.scratch_set(u, "@lang.preview", "1")

def init(now: Nat, cid: String) -> U.Ui:
  U.Ui.set_cid(U.Ui.init(now), cid)

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
# a schematic's sheets ({"t":"sheets", root, rows}: core/hier.bend's rows),
# kept under the project root for the Schematic tab's sheet chips
def recv.sheets(hit: Bool, u: U.Ui, +o: J.Json) -> U.Act:
  match hit:
    case True{}:
      U.Act{U.Ui.info_set(u, "sheets:" ++ J.Json.get_str(o, "root", ""), J.Json.get_str(o, "rows", "")), Nil{}}
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
  U.Solid.sync(recv.note(String.eq(J.Json.get_str(o, "t", ""), "plotnote"), u, o))

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

def act(u: U.Ui, +action: String, value: String) -> U.Act:
  U.Solid.sync(Rv.focus(Bool.or(String.eq(action, "view-layer-pick"), String.eq(action, "view-layer-mode")), U.Ui.act(u, action, value)))

# a timeline gap px from its end is at the end and follows; further up it
# holds what is read (client.bend's Hold; host.js's held)
def hold_at_end(gap: Nat) -> Bool:
  U.Hold.at_end(gap)

def view(u: U.Ui) -> D.Node:
  V.View.page(u)

# this page is a desk (core/desk.bend): host.js calls this after input,
# focus and visibility changes; it answers the desk.look to send, when due
def desk(u: U.Ui, focused: Bool, now: Nat) -> U.Act:
  U.Ui.desk(u, focused, now)

def online(u: U.Ui, b: Bool) -> U.Ui:
  U.Ui.online(u, b)

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
def live(u: U.Ui) -> D.Node:
  V.View.live.u(u)

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

def lb_node(url: String) -> D.Node:
  D.El{"div", "", [D.At{"class", "lightbox"}], [
    D.El{"img", "", [D.At{"class", "lb-img"}, D.At{"src", url}, D.At{"alt", ""}, D.At{"draggable", "false"}], Nil{}},
    D.El{"button", "", [D.At{"class", "lb-close"}, D.At{"title", "Close"}, D.At{"data-lb-close", "1"}], [D.Txt{"×"}]}]}

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
