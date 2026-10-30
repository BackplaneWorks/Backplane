import Base
import ../core/json.bend as J
import ../core/msgs.bend as MSG
import ../core/model.bend as M
import ../core/signal.bend as SG
import ../core/i18n.bend as I
import ./dom.bend as D
import ./icon.bend as WI
import ./cat.bend as WC
import ./space.bend as WS
import ./mailroom.bend as WM
import ../core/client.bend as U
import ../core/bot.bend as BT
import ../core/botui.bend as BI
import ../core/cron.bend as CR
import ../core/sched.bend as SC
import ../core/plane.bend as PL
import ../core/uber.bend as UB
import ../core/task.bend as TK
import ../core/presentation.bend as PS

# The bots on the web (docs/bots.md): the sidebar's cats and rooms, a bot's
# header, tabs and tab bodies, a room, a bot on another machine. What is
# shown comes from core/botui.bend; this file only lays it out. A bot's Chat
# tab is the thread view (view.bend).
#
# Form fields send action "bfield" with "<field>\x1f<text>": an input with
# data-bfield="<field>" gets that prefix from host.js.

def WB.sep() -> String:
  "\u{1f}"

# a "bfield" value that sets a field
def WB.bf(+field: String, +v: String) -> String:
  field ++ WB.sep() ++ v

def WB.enter(+act: String) -> List<&2, D.Attr>:
  Bool.pick(List<&2, D.Attr>, String.is_empty(act), Nil{}, [D.At{"data-enter", act}])

# a one-line form field; Enter sends `act` (none when "")
def WB.input(+id: String, +field: String, +value: String, +ph: String, +act: String) -> D.Node:
  D.El{"input", id, List.append(&2, D.Attr, [D.At{"id", id}, D.At{"class", "field"}, D.At{"placeholder", ph}, D.At{"autocomplete", "off"}, D.At{"spellcheck", "false"},
    D.At{"value", value}, D.At{"data-bfield", field}, D.On{"input", "bfield"}], WB.enter(act)), Nil{}}

# a field that takes a secret (client.bend's Sealed): no binding and no
# value, so what is typed stays in the page; Enter, like the form's Save,
# is "sealed" with the form, and host.js reads every field of the form
# (data-sealed) once. `kind` is the input's type ("password" or "text").
def WB.sealed(+id: String, +form: String, +kind: String, +ph: String) -> D.Node:
  D.El{"input", id, [D.At{"id", id}, D.At{"class", "field"}, D.At{"type", kind}, D.At{"placeholder", ph}, D.At{"autocomplete", Bool.pick(String, String.eq(kind, "password"), "new-password", "off")},
    D.At{"spellcheck", "false"}, D.At{"data-sealed", form}, D.At{"data-enter", "sealed"}, D.At{"data-value", form}], Nil{}}

def WB.area(+id: String, +field: String, +value: String, +ph: String) -> D.Node:
  D.El{"textarea", id, [D.At{"id", id}, D.At{"class", "field"}, D.At{"rows", "3"}, D.At{"placeholder", ph}, D.At{"value", value},
    D.At{"data-bfield", field}, D.On{"input", "bfield"}], Nil{}}

def WB.div(+cls: String, kids: List<&2, D.Node>) -> D.Node:
  D.El{"div", "", [D.At{"class", cls}], kids}

def WB.note(+s: String) -> D.Node:
  WB.div("bot-note", [D.Txt{s}])

def WB.btns(kids: List<&2, D.Node>) -> D.Node:
  WB.div("bform-btns", kids)

def WB.ago.loc(+lc: I.Loc, +now: Nat, +at: Nat) -> String:
  Bool.pick(String, Nat.is_eq(at, 0n), "", I.I18n.ago(lc, now, at))

def WB.ago(+now: Nat, +at: Nat) -> String:
  WB.ago.loc(I.En{}, now, at)

def WB.mood.cls(+m: String) -> String:
  "mood " ++ m

def WB.running(t: M.TurnState) -> Bool:
  match t:
    case M.Running{}:
      True{}
    case _:
      False{}

def WB.keep(-A: Data, hit: Bool, x: A, rest: List<&2, A>) -> List<&2, A>:
  match hit:
    case True{}:
      x <> rest
    case False{}:
      rest

# Sidebar
# -------

# a bot: its cat, its name and its mood; a bot elsewhere shows its
# machine (BU.row.title); a click does what BU.act says
def WB.row(+lc: I.Loc, +r: BI.BRow) -> D.Node:
  match r:
    case BI.BRow{+id, _, _, +look, +mood, _, sel, _, _, _}:
      D.El{"div", "bot:" ++ id, [D.At{"class", Bool.pick(String, sel, "bot-row on", "bot-row")}, D.At{"title", id}, D.At{"data-value", id}, D.On{"click", BI.BU.act(r)}],
        [WC.WCat.img(look, mood, 28), WB.div("bot-text", [D.Dom.span("bot-name", BI.BU.row.title(r)), D.Dom.span(WB.mood.cls(mood), BI.Mood.note.loc(lc, mood))])]}

def WB.rows(+lc: I.Loc, rs: List<&2, BI.BRow>) -> List<&2, D.Node>:
  match rs:
    case Nil{}:
      Nil{}
    case r <> t:
      WB.row(lc, r) <> WB.rows(lc, t)

def WB.rrow(+lc: I.Loc, r: BI.RRow) -> D.Node:
  match r:
    case BI.RRow{+id, name, +n, sel}:
      D.El{"div", "room:" ++ id, [D.At{"class", Bool.pick(String, sel, "room-row on", "room-row")}, D.At{"data-value", id}, D.On{"click", "room"}],
        [D.Dom.span("hash", "#"), D.Dom.span("title", name), D.El{"span", "", [D.At{"class", "ago"}, D.At{"title", MSG.T.room.members.count(lc, n)}], [D.Txt{Nat.show(n)}]}]}

def WB.rrows(+lc: I.Loc, rs: List<&2, BI.RRow>) -> List<&2, D.Node>:
  match rs:
    case Nil{}:
      Nil{}
    case r <> t:
      WB.rrow(lc, r) <> WB.rrows(lc, t)

def WB.pchips(xs: List<&2, String>, +cur: String) -> List<&2, D.Node>:
  match xs:
    case Nil{}:
      Nil{}
    case +p <> t:
      D.Dom.button(Bool.pick(String, String.eq(p, cur), "on", ""), "bfield", WB.bf("provider", p), p, p) <> WB.pchips(t, cur)

def WB.prov(+u: U.Ui) -> String:
  Bool.pick(String, String.is_empty(BI.BU.field(u, "provider")), M.Setting.get(U.Ui.state(u), "agent.provider", "claude"), BI.BU.field(u, "provider"))

# the new bot: a name, a persona, a provider
def WB.bot.form(+lc: I.Loc, open: Bool, +u: U.Ui) -> List<&2, D.Node>:
  match open:
    case True{}:
      [WB.div("bform", [
        WB.input("bot-name", "name", BI.BU.field(u, "name"), U.Bots.name.hint.loc(lc, u), "bot-create"),
        WB.area("bot-persona", "persona", BI.BU.field(u, "persona"), MSG.T.web.bots.persona.placeholder(lc)),
        WB.div("chips-row", WB.pchips(["claude", "codex", "grok", "muse", "antigravity", "opencode", "pi", "omp"], WB.prov(u))),
        WB.btns([D.Dom.button("primary", "bot-create", "", MSG.T.bots.create.tip(lc), MSG.T.common.create(lc)), D.Dom.button("", "bot-new", "", MSG.T.common.cancel(lc), MSG.T.common.cancel(lc))])])]
    case False{}:
      Nil{}

def WB.picks.loc(+lc: I.Loc, ps: List<&2, BI.Pick>) -> List<&2, D.Node>:
  match ps:
    case Nil{}:
      Nil{}
    case BI.Pick{id, name, on} <> t:
      D.Dom.button(Bool.pick(String, on, "pick on", "pick"), "room-pick", id, MSG.T.bots.member(lc), name) <> WB.picks.loc(lc, t)

# a new room: a name and the bots in it

def WB.picks(ps: List<&2, BI.Pick>) -> List<&2, D.Node>:
  WB.picks.loc(I.En{}, ps)

def WB.room.form.loc(+lc: I.Loc, open: Bool, +u: U.Ui) -> List<&2, D.Node>:
  match open:
    case True{}:
      [WB.div("bform", [
        WB.input("room-name", "rname", BI.BU.field(u, "rname"), MSG.T.bots.name(lc), "room-save"),
        WB.div("chips-row", WB.picks.loc(lc, BI.BU.picks(u))),
        WB.btns([D.Dom.button("primary", "room-save", "", MSG.T.rooms.create.tip(lc), MSG.T.common.save(lc)), D.Dom.button("", "room-new", "", MSG.T.common.cancel(lc), MSG.T.common.cancel(lc))])])]
    case False{}:
      Nil{}

def WB.room.form(open: Bool, +u: U.Ui) -> List<&2, D.Node>:
  WB.room.form.loc(I.En{}, open, u)

def WB.shead(+label: String, +act: String, +title: String) -> D.Node:
  D.El{"div", "", [D.At{"class", "project-head bots-head"}], [D.Dom.span("name", label), WI.WIcon.button("icon", act, "", title, "plus", 16, "")]}

# the sidebar's bots and rooms (below the projects)
def WB.side(+lc: I.Loc, +u: U.Ui) -> List<&2, D.Node>:
  [D.El{"section", "bots", [D.At{"class", "bots"}], List.concat(&2, D.Node, [
    [WB.shead(MSG.T.tour.title.bots(lc), "bot-new", MSG.T.bots.new(lc))],
    WB.bot.form(lc, BI.BU.open(u, "@bnew"), u),
    WB.rows(lc, BI.BU.rows(u)),
    [WB.shead(MSG.T.bots.rooms(lc), "room-new", MSG.T.rooms.new(lc))],
    WB.room.form.loc(lc, BI.BU.open(u, "@rnew"), u),
    WB.rrows(lc, BI.BU.rooms(u))])}]

# whether the main area shows a bot, a room or a bot elsewhere
def WB.shown(v: BI.View) -> Bool:
  match v:
    case BI.VThread{}:
      False{}
    case _:
      True{}

# A bot
# -----

def WB.tabs.go(+lc: I.Loc, ts: List<&2, String>, +cur: String) -> List<&2, D.Node>:
  match ts:
    case Nil{}:
      Nil{}
    case +t <> r:
      D.Dom.button(Bool.pick(String, String.eq(t, cur), "tab on", "tab"), "bot-tab", t, BI.BU.tab.label(lc, t), BI.BU.tab.label(lc, t)) <> WB.tabs.go(lc, r, cur)

def WB.pages.menu(+lc: I.Loc, +ts: List<&2, String>, +cur: String) -> D.Node:
  D.El{"details", "bot-menu", [D.At{"class", "local-menu bot-mobile-menu"}], [
    D.El{"summary", "", [D.At{"aria-label", "Bot pages"}, D.At{"title", BI.BU.tab.label(lc, cur)}], [D.Txt{"Pages"}, WI.WIcon.svg("chevron-down", 14)]},
    D.El{"div", "", [D.At{"class", "local-menu-body"}], WB.tabs.go(lc, ts, cur)}]}

# a bot's header (BU.head, the same for a bot here, one mirrored from a
# linked machine and one only reported by one): its cat, its name with
# where it lives, its mood, then sub (its model) and tools (Stop)
def WB.header.layout(rail: Bool, +lc: I.Loc, hd: BI.Head, sub: List<&2, D.Node>, tools: List<&2, D.Node>) -> D.Node:
  match hd:
    case BI.Head{+title, +look, +mood, +note, +tabs, +tab, _, _}:
      D.El{"header", "", [D.At{"class", "thread-head bot-head"}], [
        D.Dom.button("back", "select", "", MSG.T.tour.back(lc), "‹"),
        WC.WCat.img(look, mood, 72),
        WB.div("bot-id", D.Dom.span("bot-title", title) <> D.Dom.span(WB.mood.cls(mood), note) <> sub),
        D.El{"div", "", [D.At{"class", "tools"}], List.append(&2, D.Node, tools, D.Dom.when(rail, WB.pages.menu(lc, tabs, BI.BU.nav.parent(tabs, tab))))}]}

def WB.header.loc(+lc: I.Loc, hd: BI.Head, sub: List<&2, D.Node>, tools: List<&2, D.Node>) -> D.Node:
  WB.header.layout(True{}, lc, hd, sub, tools)

def WB.header(hd: BI.Head, sub: List<&2, D.Node>, tools: List<&2, D.Node>) -> D.Node:
  WB.header.loc(I.En{}, hd, sub, tools)

def WB.head.t.layout(rail: Bool, +lc: I.Loc, t: M.Thread, +hd: BI.Head) -> D.Node:
  match t:
    case M.Thread{_, _, _, _, _, turn, _, _, _, _, _, _, _, _, _, +pr, +mo, _, _, _, _}:
      WB.header.layout(rail, lc, hd, [D.Dom.span("bot-model", pr ++ " · " ++ Bool.pick(String, String.is_empty(mo), MSG.T.common.default(lc), mo))], D.Dom.when(Bool.and(WB.running(turn), Bool.not(String.eq(BI.Head.tab(hd), "chat"))), D.Dom.button("", "interrupt", "", MSG.T.composer.stop.tip(lc), MSG.T.composer.stop(lc))))

def WB.head.t.loc(+lc: I.Loc, t: M.Thread, +hd: BI.Head) -> D.Node:
  WB.head.t.layout(True{}, lc, t, hd)

# the bot's header: its cat, name, mood and model

def WB.head.t(t: M.Thread, hd: BI.Head) -> D.Node:
  WB.head.t.loc(I.En{}, t, hd)

def WB.head.loc(+lc: I.Loc, +u: U.Ui, +b: BT.Bot, hd: BI.Head) -> D.Node:
  WB.head.t.layout(String.eq(U.Ui.info(u, "local.workspace.layout"), "rail-v1"), lc, U.Ui.thread(U.Ui.state(u), BT.Bot.thread(b)), hd)

def WB.head(+u: U.Ui, +b: BT.Bot, hd: BI.Head) -> D.Node:
  WB.head.loc(I.En{}, u, b, hd)

# the tabs a bot has (BU.head), the one shown on
def WB.tabs(+lc: I.Loc, +hd: BI.Head) -> D.Node:
  D.El{"nav", "", [D.At{"class", "bot-tabs bot-primary-tabs"}], [
    D.El{"div", "", [D.At{"class", "bot-desktop-tabs"}], WB.tabs.go(lc, BI.Head.tabs(hd), BI.BU.nav.parent(BI.Head.tabs(hd), BI.Head.tab(hd)))}]}

# Browser: the bot's page, a frame the hub serves (the pairing cookie
# carries the token, as for /img); n changes with each "shot" message.
# While the person drives it (client.bend's Teach.input) the frame takes
# their clicks, wheel and keys: host.js only forwards them, as the actions
# the img names (data-pt: a click in thousandths of the frame, data-wheel:
# a notch's pixels, data-keyto: a key's name, data-type: typed text).
def WB.frame.loc(+lc: I.Loc, +id: String, +n: String, +name: String) -> D.Node:
  D.El{"img", "bot-frame", [D.At{"src", "/bots/" ++ id ++ "/screen.jpg?n=" ++ n}, D.At{"alt", MSG.T.browser.owner(lc, name)}, D.At{"tabindex", "0"}, D.At{"draggable", "false"},
    D.At{"data-pt", "bb-click"}, D.At{"data-wheel", "bb-scroll"}, D.At{"data-keyto", "bb-key"}, D.At{"data-type", "bb-type"}], Nil{}}

def WB.frame(+id: String, +n: String, +name: String) -> D.Node:
  WB.frame.loc(I.En{}, id, n, name)

def WB.browser.in.loc(+lc: I.Loc, input: Bool, none: Bool, +n: String, +id: String, +name: String) -> D.Node:
  match none:
    case True{}:
      WB.note(MSG.T.browser.notopened(lc, name))
    case False{}:
      Bool.pick(D.Node, input, WB.div("bot-screen driven", [WB.frame.loc(lc, id, n, name)]),
        D.El{"a", "", [D.At{"class", "bot-screen"}, D.At{"href", "/bots/" ++ id ++ "/screen.jpg?n=" ++ n}, D.At{"target", "_blank"}, D.At{"data-lightbox", "/bots/" ++ id ++ "/screen.jpg?n=" ++ n}],
          [D.El{"img", "", [D.At{"src", "/bots/" ++ id ++ "/screen.jpg?n=" ++ n}, D.At{"alt", MSG.T.browser.owner(lc, name)}], Nil{}}]})

def WB.browser.in(input: Bool, none: Bool, +n: String, +id: String, +name: String) -> D.Node:
  WB.browser.in.loc(I.En{}, input, none, n, id, name)

def WB.browser.k.loc(+lc: I.Loc, none: Bool, +n: String, +id: String, +name: String) -> D.Node:
  WB.browser.in.loc(lc, False{}, none, n, id, name)

def WB.browser.k(none: Bool, +n: String, +id: String, +name: String) -> D.Node:
  WB.browser.k.loc(I.En{}, none, n, id, name)

# The local Electron guest paints directly in this measured DOM rectangle.
# Main/service own its actual tab and input authority; DOM claims grant neither.
def WB.browser.direct(+u: U.Ui, +id: String) -> Bool:
  Bool.and(String.eq(U.Ui.scratch(u, "@desktop.browser"), "1"),
    Bool.and(String.is_empty(U.Ui.info(u, "local.remote.url")), Bool.not(String.contains(id, "~"))))

def WB.browser.drive.loc(+lc: I.Loc, +u: U.Ui, +id: String) -> D.Node:
  +person = Bool.not(String.is_empty(U.Teach.drive(u, id)))
  WB.div("tk-strip desktop-browser-drive", [
    D.Dom.span("", Bool.pick(String, person, "You drive", "Agent drives · click the page to take over")),
    D.Dom.button("", Bool.pick(String, person, "tk-handback", "tk-takeover"), id,
      Bool.pick(String, person, MSG.T.teach.handback(lc), MSG.T.teach.takeover(lc)),
      Bool.pick(String, person, MSG.T.teach.handback(lc), MSG.T.teach.takeover(lc)))])

def WB.browser.surface.k(direct: Bool, +lc: I.Loc, +u: U.Ui, +id: String, +name: String) -> D.Node:
  match direct:
    case True{}:
      D.El{"div", "desktop-browser:" ++ id, [D.At{"class", "desktop-browser"}, D.At{"data-desktop-browser-owner", id}, D.At{"aria-label", MSG.T.browser.owner(lc, name)}],
        [D.El{"span", "desktop-browser-status:" ++ id, [D.At{"class", "desktop-browser-status"}, D.At{"role", "status"}, D.At{"aria-live", "polite"}], [D.Txt{"Connecting Browser…"}]},
         D.El{"button", "desktop-browser-reconnect:" ++ id, [D.At{"type", "button"}, D.At{"data-desktop-browser-reconnect", ""}, D.At{"hidden", ""}], [D.Txt{"Reconnect Browser"}]}]}
    case False{}:
      WB.browser.in.loc(lc, U.Teach.input(u, id), String.is_empty(U.Ui.info(u, "shot." ++ id)), U.Ui.info(u, "shot." ++ id), id, name)

def WB.browser.surface.ready(provisional: Bool, lc: I.Loc, +u: U.Ui, +id: String, name: String) -> D.Node:
  match provisional:
    case True{}: D.El{"div", "browser-loading", [D.At{"role", "status"}], [D.Txt{MSG.T.viewer.loading(lc)}]}
    case False{}: WB.browser.surface.k(WB.browser.direct(u, id), lc, u, id, name)

def WB.browser.surface.loc(+lc: I.Loc, +u: U.Ui, +id: String, +name: String) -> D.Node:
  WB.browser.surface.ready(String.eq(U.Ui.scratch(u, "@presentation"), "1"), lc, u, id, name)

def WB.tbtns(bs: List<&2, U.TBtn>) -> List<&2, D.Node>:
  match bs:
    case Nil{}:
      Nil{}
    case U.TBtn{+label, +action, +value, +primary} <> t:
      D.Dom.button(Bool.pick(String, primary, "primary", ""), action, value, label, label) <> WB.tbtns(t)

def WB.browser.tabs(ts: List<&2, J.Json>) -> List<&2, D.Node>:
  match ts:
    case Nil{}: Nil{}
    case +tab <> rest:
      +title = J.Json.get_str(tab, "title", "")
      WB.div(Bool.pick(String, J.Json.get_bool(tab, "active", False{}), "browser-tab on", "browser-tab"), [
        D.Dom.button("browser-tab-title", "br-select", J.Json.get_str(tab, "id", ""), J.Json.get_str(tab, "url", ""),
          Bool.pick(String, String.is_empty(title), J.Json.get_str(tab, "url", "New tab"), title)),
        D.Dom.button("", "br-close", J.Json.get_str(tab, "id", ""), "Close tab", "×")]) <> WB.browser.tabs(rest)

def WB.browser.controls(+lc: I.Loc, +u: U.Ui, +page: String) -> List<&2, D.Node>:
  [WB.div("browser-tabs", List.append(&2, D.Node, WB.browser.tabs(U.Browse.tabs(u, page)), [D.Dom.button("", "br-new", "", "New tab", "+")])),
   WB.div("browser-address", [D.Dom.button("", "br-back", "", MSG.T.tour.back(lc), "←"),
    D.Dom.button("", "br-forward", "", "Forward", "→"), D.Dom.button("", "br-reload", "", MSG.T.common.reload(lc), "↻"),
    WB.input("browser-address:" ++ page, U.Browse.field(page), U.Browse.address(u, page), MSG.T.browser.address(lc), "br-go"),
    D.Dom.button("", "br-go", "", MSG.T.common.go(lc), MSG.T.common.go(lc))])]

def WB.urlbar.loc(+lc: I.Loc, +u: U.Ui) -> D.Node:
  WB.div("bform row", [D.Dom.button("", "bb-back", "", MSG.T.tour.back(lc), MSG.T.tour.back(lc)), D.Dom.button("", "bb-reload", "", MSG.T.common.reload(lc), MSG.T.common.reload(lc)),
    WB.input("bot-url", "bburl", BI.BU.field(u, "bburl"), MSG.T.browser.address(lc), "bb-nav"), D.Dom.button("", "bb-nav", "", MSG.T.common.go(lc), MSG.T.common.go(lc))])

def WB.urlbar(+u: U.Ui) -> D.Node:
  WB.urlbar.loc(I.En{}, u)

def WB.notebar.loc(+lc: I.Loc, +u: U.Ui) -> D.Node:
  WB.div("bform row", [WB.input("tk-note", "tknote", BI.BU.field(u, "tknote"), MSG.T.teach.note.placeholder(lc), "tk-note"), D.Dom.button("", "tk-note", "", MSG.T.browser.note.tip(lc), MSG.T.teach.note.add(lc))])

# the strip above the page: who has it, and what to do

def WB.notebar(+u: U.Ui) -> D.Node:
  WB.notebar.loc(I.En{}, u)

def WB.strip.k.loc(+lc: I.Loc, s: U.TStrip, +u: U.Ui, +input: Bool) -> D.Node:
  match s:
    case U.TStrip{+mode, +title, +clock, +steps, bs}:
      +rec = {Nat.is_eq(mode, 1n) : Bool}
      WB.div("tk-strip mode" ++ Nat.show(mode), D.Dom.cat([WB.div("tk-head", [D.Dom.span("tk-title", title),
          D.Dom.span("tk-meta", Bool.pick(String, rec, clock ++ " · " ++ Nat.show(steps) ++ Bool.pick(String, Nat.is_eq(steps, 1n), " step", " steps"), "")),
          WB.div("tk-btns", WB.tbtns(bs))])],
        D.Dom.when(rec, WB.notebar.loc(lc, u))))

# one recorded step in the save sheet: its typed value is a chip that makes
# it a parameter (a secret has none), named in a field once marked

def WB.strip.k(s: U.TStrip, +u: U.Ui, +input: Bool) -> D.Node:
  WB.strip.k.loc(I.En{}, s, u, input)

def WB.srow.loc(+lc: I.Loc, r: U.TStepRow, +u: U.Ui) -> D.Node:
  match r:
    case U.TStepRow{+n, +text, _, +chip, +secret, +marked, +pname}:
      D.El{"div", "tk-step:" ++ Nat.show(n), [D.At{"class", "tk-step"}], D.Dom.cat([D.Dom.span("tk-n", Nat.show(n) ++ "."), D.Dom.span("tk-what", text)],
        D.Dom.cat(D.Dom.when(secret, D.Dom.span("tk-secret", "private")),
          D.Dom.cat(D.Dom.when(Bool.not(String.is_empty(chip)), D.Dom.button(Bool.pick(String, marked, "chip on", "chip"), "tk-mark", Nat.show(n), MSG.T.tasks.value.ask(lc), chip)),
            D.Dom.when(marked, WB.input("tkp-" ++ Nat.show(n), "tkp." ++ Nat.show(n), BI.BU.field(u, "tkp." ++ Nat.show(n)), pname, "tk-save")))))}

def WB.srow(r: U.TStepRow, +u: U.Ui) -> D.Node:
  WB.srow.loc(I.En{}, r, u)

def WB.srows.loc(+lc: I.Loc, rs: List<&2, U.TStepRow>, +u: U.Ui) -> List<&2, D.Node>:
  match rs:
    case Nil{}:
      Nil{}
    case r <> t:
      WB.srow.loc(lc, r, u) <> WB.srows.loc(lc, t, u)

def WB.srows(rs: List<&2, U.TStepRow>, +u: U.Ui) -> List<&2, D.Node>:
  WB.srows.loc(I.En{}, rs, u)

def WB.sheet(+lc: I.Loc, +u: U.Ui, +b: BT.Bot) -> D.Node:
  WB.div("bot-list tk-sheet", D.Dom.cat([D.Dom.span("tk-title", MSG.T.teach.save(lc)),
      WB.note(MSG.T.teach.name.note(lc)),
      WB.input("tk-name", "tkname", BI.BU.field(u, "tkname"), MSG.T.bots.name(lc), "tk-save"),
      WB.input("tk-desc", "tkdesc", BI.BU.field(u, "tkdesc"), MSG.T.teach.description(lc), "")],
    D.Dom.cat(WB.srows.loc(lc, U.Teach.rows.loc(lc, u, BT.Bot.id(b)), u),
      [WB.btns([D.Dom.button("primary", "tk-save", "", MSG.T.tasks.save.tip(lc), MSG.T.common.save(lc)), D.Dom.button("", "tk-discard", "", MSG.T.tasks.discard.tip(lc), MSG.T.teach.discard(lc))])])))

def WB.browser.m(+lc: I.Loc, sheet: Bool, +u: U.Ui, +b: BT.Bot) -> D.Node:
  match sheet:
    case True{}:
      WB.sheet(lc, u, b)
    case False{}:
      +id = BT.Bot.id(b)
      +input = {U.Teach.input(u, id) : Bool}
      WB.div("tk-live", List.append(&2, D.Node, WB.strip.k.loc(lc, U.Teach.strip.loc(lc, u, id), u, input) <> WB.browser.controls(lc, u, id),
        [WB.browser.surface.loc(lc, u, id, BT.Bot.name(b))]))

def WB.browser(+lc: I.Loc, +u: U.Ui, +b: BT.Bot) -> D.Node:
  WB.browser.m(lc, Nat.is_eq(U.Teach.mode(u, BT.Bot.id(b)), 2n), u, b)

# Tasks: what the bot has learned, each opened for its run form, then its
# recent runs with their screenshots
def WB.status.cls(+s: String) -> String:
  Bool.pick(String, String.eq(s, "ok"), "state ok", Bool.pick(String, Bool.or(String.eq(s, "failed"), String.eq(s, "stopped")), "state bad", "state"))

def WB.pfields(ps: List<&2, TK.TkParam>, +u: U.Ui) -> List<&2, D.Node>:
  match ps:
    case Nil{}:
      Nil{}
    case TK.TkParam{+name, +label, _, +example} <> t:
      WB.input("tkr-" ++ name, U.Teach.pfield(name), BI.BU.field(u, U.Teach.pfield(name)), TK.Tk.first(label, name) ++ Bool.pick(String, String.is_empty(example), "", " (" ++ example ++ ")"), "") <> WB.pfields(t, u)

def WB.task.open.loc(+lc: I.Loc, t: U.TTask, +u: U.Ui) -> List<&2, D.Node>:
  match t:
    case U.TTask{+slug, _, _, +ps, _, +asking, _}:
      D.Dom.cat(WB.pfields(ps, u), [WB.btns([D.Dom.button("primary", "tk-run", slug, MSG.T.tasks.run.tip(lc), MSG.T.common.run(lc)),
        D.Dom.button(Bool.pick(String, asking, "danger", ""), "tk-del", slug, MSG.T.tasks.delete.tip(lc), Bool.pick(String, asking, MSG.T.menu.delete.confirm(lc), MSG.T.common.delete(lc)))])])

def WB.task.open(t: U.TTask, +u: U.Ui) -> List<&2, D.Node>:
  WB.task.open.loc(I.En{}, t, u)

def WB.task.loc(+lc: I.Loc, t: U.TTask, +u: U.Ui) -> D.Node:
  match t:
    case U.TTask{+slug, +name, +desc, ps, +open, +asking, +last}:
      D.El{"div", "tk:" ++ slug, [D.At{"class", Bool.pick(String, open, "item open", "item")}], D.Dom.cat([
        D.El{"div", "", [D.At{"class", "item-main"}, D.At{"data-value", slug}, D.On{"click", "tk-open"}],
          [D.Dom.span("item-key", name), D.Dom.span("item-text", desc)]},
        D.Dom.span(WB.status.cls(last), last)],
        Bool.pick(List<&2, D.Node>, open, WB.task.open.loc(lc, U.TTask{slug, name, desc, ps, open, asking, last}, u), Nil{}))}

def WB.task(t: U.TTask, +u: U.Ui) -> D.Node:
  WB.task.loc(I.En{}, t, u)

def WB.tasks.go.loc(+lc: I.Loc, ts: List<&2, U.TTask>, +u: U.Ui) -> List<&2, D.Node>:
  match ts:
    case Nil{}:
      Nil{}
    case t <> r:
      WB.task.loc(lc, t, u) <> WB.tasks.go.loc(lc, r, u)

def WB.tasks.go(ts: List<&2, U.TTask>, +u: U.Ui) -> List<&2, D.Node>:
  WB.tasks.go.loc(I.En{}, ts, u)

def WB.shots(ss: List<&2, String>) -> List<&2, D.Node>:
  match ss:
    case Nil{}:
      Nil{}
    case +s <> t:
      D.El{"a", "", [D.At{"class", "tk-shot"}, D.At{"href", s}, D.At{"target", "_blank"}, D.At{"data-lightbox", s}], [D.El{"img", "", [D.At{"src", s}, D.At{"alt", "screenshot"}, D.At{"loading", "lazy"}], Nil{}}]} <> WB.shots(t)

def WB.run(r: U.TRun) -> D.Node:
  match r:
    case U.TRun{+id, +slug, +status, _, +summary, +shots}:
      D.El{"div", "tr:" ++ id, [D.At{"class", "item run"}], [
        WB.div("item-main", [D.Dom.span("item-key", slug), D.Dom.span("item-text", summary)]),
        D.Dom.span(WB.status.cls(status), status),
        WB.div("tk-shots", WB.shots(shots))]}

def WB.runs.go(rs: List<&2, U.TRun>) -> List<&2, D.Node>:
  match rs:
    case Nil{}:
      Nil{}
    case r <> t:
      WB.run(r) <> WB.runs.go(t)

def WB.tasks.loc(+lc: I.Loc, +u: U.Ui, +b: BT.Bot) -> D.Node:
  +ts = U.Teach.tasks(u, BT.Bot.id(b))
  +rs = List.take(&2, U.TRun, U.Teach.runs.shown(u, BT.Bot.id(b)), 8n)
  WB.div("bot-list", D.Dom.cat([WB.note(Bool.pick(String, List.is_empty(&2, U.TTask, ts), MSG.T.teach.empty(lc), MSG.T.teach.run.note(lc)))],
    D.Dom.cat(WB.tasks.go.loc(lc, ts, u),
      D.Dom.cat(D.Dom.when(Bool.not(List.is_empty(&2, U.TRun, rs)), D.Dom.span("tk-title", MSG.T.teach.runs(lc))), WB.runs.go(rs)))))

# Memory: grouped by kind

def WB.tasks(+u: U.Ui, +b: BT.Bot) -> D.Node:
  WB.tasks.loc(I.En{}, u, b)

def WB.kind.of(ms: List<&2, BT.Mem>, +k: String) -> List<&2, BT.Mem>:
  match ms:
    case Nil{}:
      Nil{}
    case BT.Mem{b, ke, +kd, x, tg, c, up} <> t:
      WB.keep(BT.Mem, String.eq(kd, k), BT.Mem{b, ke, kd, x, tg, c, up}, WB.kind.of(t, k))

def WB.tags(+tags: String) -> List<&2, D.Node>:
  Bool.pick(List<&2, D.Node>, String.is_empty(tags), Nil{}, [D.Dom.span("item-tags", tags)])

# a memory; Forget only for a bot here (BU.owns)
def WB.mem.loc(+lc: I.Loc, m: BT.Mem, +own: Bool, +now: Nat) -> D.Node:
  match m:
    case BT.Mem{_, +key, _, text, +tags, _, +up}:
      D.El{"div", "mem:" ++ key, [D.At{"class", "item"}], [
        WB.div("item-main", D.Dom.cat([D.Dom.span("item-key", key), D.Dom.span("item-text", text)], WB.tags(tags))),
        D.Dom.span("ago", WB.ago.loc(lc, now, up)),
        WB.div("item-btns", D.Dom.when(own, D.Dom.button("", "mem-forget", key, MSG.T.memory.forget.tip(lc), MSG.T.bots.memory.forget(lc))))]}

def WB.mem(m: BT.Mem, +own: Bool, +now: Nat) -> D.Node:
  WB.mem.loc(I.En{}, m, own, now)

def WB.mems.loc(+lc: I.Loc, ms: List<&2, BT.Mem>, +own: Bool, +now: Nat) -> List<&2, D.Node>:
  match ms:
    case Nil{}:
      Nil{}
    case m <> t:
      WB.mem.loc(lc, m, own, now) <> WB.mems.loc(lc, t, own, now)

def WB.mems(ms: List<&2, BT.Mem>, +own: Bool, +now: Nat) -> List<&2, D.Node>:
  WB.mems.loc(I.En{}, ms, own, now)

def WB.mgroup.loc(+lc: I.Loc, ms: List<&2, BT.Mem>, +k: String, +own: Bool, +now: Nat) -> List<&2, D.Node>:
  match ms:
    case Nil{}:
      Nil{}
    case m <> t:
      [WB.div("group", D.Dom.span("group-label", k) <> WB.mems.loc(lc, m <> t, own, now))]

def WB.mgroup(ms: List<&2, BT.Mem>, +k: String, +own: Bool, +now: Nat) -> List<&2, D.Node>:
  WB.mgroup.loc(I.En{}, ms, k, own, now)

def WB.mgroups.loc(+lc: I.Loc, ks: List<&2, String>, +ms: List<&2, BT.Mem>, +own: Bool, +now: Nat) -> List<&2, D.Node>:
  match ks:
    case Nil{}:
      Nil{}
    case +k <> t:
      List.append(&2, D.Node, WB.mgroup.loc(lc, WB.kind.of(ms, k), k, own, now), WB.mgroups.loc(lc, t, ms, own, now))

def WB.mgroups(ks: List<&2, String>, +ms: List<&2, BT.Mem>, +own: Bool, +now: Nat) -> List<&2, D.Node>:
  WB.mgroups.loc(I.En{}, ks, ms, own, now)

def WB.memory.k.loc(+lc: I.Loc, +ms: List<&2, BT.Mem>, +own: Bool, +now: Nat) -> D.Node:
  WB.div("bot-list", Bool.pick(List<&2, D.Node>, List.is_empty(&2, BT.Mem, ms), [WB.note(MSG.T.memory.empty(lc))], WB.mgroups.loc(lc, BT.Mem.kinds(), ms, own, now)))

def WB.memory.k(+ms: List<&2, BT.Mem>, +own: Bool, +now: Nat) -> D.Node:
  WB.memory.k.loc(I.En{}, ms, own, now)

# Memory is a bounded view just like a conversation. Rendering the whole
# store eventually exceeds the view transport's reply limit.
def WB.memory.size() -> Nat:
  24n

def WB.memory.field(+bot: String) -> String:
  "memory-page:" ++ bot

def WB.memory.page(+u: U.Ui, +bot: String, +count: Nat) -> Nat:
  +last = Nat.div(Nat.sub(count, 1n), WB.memory.size())
  Nat.min(last, Maybe.default(&2, Nat, PS.Session.nat(U.Ui.scratch(u, "@f." ++ WB.memory.field(bot)), PS.Session.nat.max()), 0n))

def WB.memory.pages(+lc: I.Loc, +bot: String, +page: Nat, +count: Nat) -> D.Node:
  +start = Nat.mul(page, WB.memory.size())
  D.El{"nav", "memory-pages", [D.At{"class", "bform-btns memory-pages"}, D.At{"aria-label", MSG.T.bots.tab.memory(lc)}], List.concat(&2, D.Node, [
    D.Dom.when(Nat.is_gt(page, 0n), D.Dom.button("", "bfield", WB.bf(WB.memory.field(bot), Nat.show(Nat.sub(page, 1n))), MSG.T.common.prev(lc), MSG.T.common.prev(lc))),
    [D.Dom.span("memory-range", Nat.show(Nat.add(start, 1n)) ++ "–" ++ Nat.show(Nat.min(count, Nat.add(start, WB.memory.size()))) ++ " / " ++ Nat.show(count))],
    D.Dom.when(Nat.is_lt(Nat.add(start, WB.memory.size()), count), D.Dom.button("", "bfield", WB.bf(WB.memory.field(bot), Nat.show(Nat.add(page, 1n))), MSG.T.common.more(lc), MSG.T.common.more(lc)))])}

def WB.memory.paged(+lc: I.Loc, +ms: List<&2, BT.Mem>, +own: Bool, +now: Nat, +bot: String, +page: Nat) -> D.Node:
  WB.div("memory-view", [WB.memory.k.loc(lc, List.take(&2, BT.Mem, List.drop(&2, BT.Mem, ms, Nat.mul(page, WB.memory.size())), WB.memory.size()), own, now),
    WB.memory.pages(lc, bot, page, List.length(&2, BT.Mem, ms))])

def WB.memory.loc.k(+lc: I.Loc, +u: U.Ui, +b: BT.Bot, ms: List<&2, BT.Mem>) -> D.Node:
  match ms:
    case Nil{}: WB.memory.k.loc(lc, Nil{}, BI.BU.owns(BT.Bot.id(b)), U.Side.now(u))
    case +m <> +tail:
      WB.memory.paged(lc, m <> tail, BI.BU.owns(BT.Bot.id(b)), U.Side.now(u), BT.Bot.id(b),
        WB.memory.page(u, BT.Bot.id(b), List.length(&2, BT.Mem, m <> tail)))

def WB.memory.loc(+lc: I.Loc, +u: U.Ui, +b: BT.Bot) -> D.Node:
  WB.memory.loc.k(lc, u, b, BI.BU.mems(u, BT.Bot.id(b)))

# Routines

def WB.memory(+u: U.Ui, +b: BT.Bot) -> D.Node:
  WB.memory.loc(I.En{}, u, b)

def WB.when(+cron: String) -> String:
  Bool.pick(String, String.is_empty(CR.Cron.describe(cron)), cron, CR.Cron.describe(cron))

# a routine's buttons: Run now for any bot, the rest for a bot here
def WB.routine.btns.loc(+lc: I.Loc, own: Bool, +id: String, +on: Bool) -> List<&2, D.Node>:
  match own:
    case True{}:
      [D.Dom.button(Bool.pick(String, on, "on", ""), "routine-toggle", id, Bool.pick(String, on, MSG.T.hooks.off(lc), MSG.T.routine.on.tip(lc)), Bool.pick(String, on, MSG.T.settings.lang.on(lc), MSG.T.settings.lang.off(lc))),
       D.Dom.button("", "routine-run", id, MSG.T.tasks.run.tip(lc), MSG.T.routine.run(lc)),
       D.Dom.button("", "routine-edit", id, MSG.T.queue.edit(lc), MSG.T.queue.edit(lc)),
       D.Dom.button("", "routine-delete", id, MSG.T.common.delete(lc), MSG.T.common.delete(lc))]
    case False{}:
      [D.Dom.button("", "routine-run", id, MSG.T.tasks.run.tip(lc), MSG.T.routine.run(lc))]

# the kind chip: Repeats, Once, or Reminder with a bell

def WB.routine.btns(own: Bool, +id: String, +on: Bool) -> List<&2, D.Node>:
  WB.routine.btns.loc(I.En{}, own, id, on)

def WB.kind.chip(+kind: String, +label: String) -> D.Node:
  D.El{"span", "", [D.At{"class", "kind kind-" ++ kind}], D.Dom.cat(D.Dom.when(String.eq(kind, "reminder"), WI.WIcon.svg("bell", 12)), [D.Txt{label}])}

# "Next Wed 14 Oct 09:30 EEST · in 3h", the zone when it is not yours; "Done · ..." when done
def WB.next.text.loc(+lc: I.Loc, +next: String, +rel: String, +zone: String, +done: Bool, +dtext: String, +on: Bool) -> String:
  Bool.pick(String, done, dtext, Bool.pick(String, String.is_empty(next), Bool.pick(String, on, "", MSG.T.settings.lang.off(lc)),
    MSG.T.routine.next(lc, next, Bool.pick(String, String.is_empty(rel), "none", "some"), rel, Bool.pick(String, String.eq(zone, "your time"), "none", "some"), zone)))

# a done routine keeps only Delete

def WB.next.text(+next: String, +rel: String, +zone: String, +done: Bool, +dtext: String, +on: Bool) -> String:
  WB.next.text.loc(I.En{}, next, rel, zone, done, dtext, on)

def WB.routine.btns.d.loc(+lc: I.Loc, done: Bool, own: Bool, +id: String, +on: Bool) -> List<&2, D.Node>:
  match done:
    case True{}:
      [D.Dom.button("", "routine-delete", id, MSG.T.common.delete(lc), MSG.T.common.delete(lc))]
    case False{}:
      WB.routine.btns.loc(lc, own, id, on)

def WB.routine.btns.d(done: Bool, own: Bool, +id: String, +on: Bool) -> List<&2, D.Node>:
  WB.routine.btns.d.loc(I.En{}, done, own, id, on)

def WB.routine.row.loc(+lc: I.Loc, w: SC.SRow, +id: String, name: String, prompt: String, +on: Bool, +own: Bool) -> D.Node:
  match w:
    case SC.SRow{+kind, +label, +when, +next, +rel, +zone, +done, +dtext}:
      D.El{"div", "ro:" ++ id, [D.At{"class", "item" ++ Bool.pick(String, done, " done", Bool.pick(String, on, "", " off"))}], [
        WB.div("item-main", [D.Dom.span("item-key", name),
          WB.div("item-kind", [WB.kind.chip(kind, label), D.Dom.span("item-when", when)]),
          D.Dom.span("item-next", WB.next.text.loc(lc, next, rel, zone, done, dtext, on)), D.Dom.span("item-text", prompt)]),
        WB.div("item-btns", WB.routine.btns.d.loc(lc, done, own, id, on))]}

def WB.routine.row(w: SC.SRow, +id: String, name: String, prompt: String, +on: Bool, +own: Bool) -> D.Node:
  WB.routine.row.loc(I.En{}, w, id, name, prompt, on, own)

def WB.routine(+lc: I.Loc, +u: U.Ui, +r: BT.Routine, +own: Bool, +now: Nat) -> D.Node:
  WB.routine.row.loc(lc, SC.Sched.row(lc, U.Ui.state(u), r, now, 0n, True{}), BT.Routine.id(r), BT.Routine.name(r), BT.Routine.prompt(r), BT.Routine.on(r), own)

def WB.routines.go(+lc: I.Loc, rs: List<&2, BT.Routine>, +u: U.Ui, +own: Bool, +now: Nat) -> List<&2, D.Node>:
  match rs:
    case Nil{}:
      Nil{}
    case r <> t:
      WB.routine(lc, u, r, own, now) <> WB.routines.go(lc, t, u, own, now)

def WB.cron.hint.k(empty: Bool, bad: Bool, +d: String) -> List<&2, D.Node>:
  match empty bad:
    case True{} _:
      Nil{}
    case False{} True{}:
      [D.Dom.span("hint bad", d)]
    case False{} False{}:
      [D.Dom.span("hint", d)]

# what the schedule or time being typed means, in your zone (U.Routine.hint);
# an error when it is neither
def WB.cron.hint(+lc: I.Loc, +u: U.Ui) -> List<&2, D.Node>:
  WB.cron.hint.k(String.is_empty(String.trim(BI.BU.field(u, "ocron"))), U.Routine.hint.bad(u), U.Routine.hint.loc(lc, u))

def WB.rform(+lc: I.Loc, +u: U.Ui) -> D.Node:
  WB.div("bform wide", D.Dom.cat([
    WB.input("routine-name", "oname", BI.BU.field(u, "oname"), MSG.T.bots.name(lc), ""),
    WB.input("routine-cron", "ocron", BI.BU.field(u, "ocron"), "every 15m · daily 09:30 · tomorrow 9am · in 2h · cron", "")],
    D.Dom.cat(WB.cron.hint(lc, u), [
    WB.area("routine-prompt", "oprompt", BI.BU.field(u, "oprompt"), U.Routine.prompt(lc, u)),
    WB.btns([D.Dom.button("primary", "routine-save", "", MSG.T.routine.save.tip(lc), Bool.pick(String, String.is_empty(BI.BU.field(u, "oid")), MSG.T.routine.add(lc), MSG.T.routine.save(lc))),
      D.Dom.button("", "routine-edit", "", MSG.T.routine.clear.tip(lc), MSG.T.common.clear(lc))])])))

def WB.routines(+lc: I.Loc, +u: U.Ui, +b: BT.Bot) -> D.Node:
  WB.div("bot-list", List.append(&2, D.Node, WB.routines.go(lc, BI.BU.routines(u, BT.Bot.id(b)), u, BI.BU.owns(BT.Bot.id(b)), U.Side.now(u)), D.Dom.when(BI.BU.owns(BT.Bot.id(b)), WB.rform(lc, u))))

# Webhooks
def WB.page(+u: U.Ui) -> String:
  U.Ui.scratch(u, "@page")

def WB.calls(+n: Nat, +last: String) -> String:
  Nat.show(n) ++ Bool.pick(String, Nat.is_eq(n, 1n), " call", " calls") ++ Bool.pick(String, String.is_empty(last), "", " · " ++ last)

def WB.hook.loc(+lc: I.Loc, h: BT.Hook, +now: Nat, +origin: String) -> D.Node:
  match h:
    case BT.Hook{+id, _, name, _, _, +last, +count}:
      D.El{"div", "hk:" ++ id, [D.At{"class", "item"}], [
        WB.div("item-main", [D.Dom.span("item-key", name), D.Dom.span("item-url", origin ++ "/hook/" ++ id)]),
        D.Dom.span("ago", WB.calls(count, WB.ago.loc(lc, now, last))),
        WB.div("item-btns", [D.Dom.copy.loc(lc, origin ++ "/hook/" ++ id), D.Dom.button("", "hook-token", id, MSG.T.hooks.token.tip(lc), MSG.T.hooks.token(lc)),
          D.Dom.button("", "hook-revoke", id, MSG.T.web.hooks.revoke(lc), MSG.T.hooks.revoke(lc))])]}

def WB.hook(h: BT.Hook, +now: Nat, +origin: String) -> D.Node:
  WB.hook.loc(I.En{}, h, now, origin)

def WB.hooks.go.loc(+lc: I.Loc, hs: List<&2, BT.Hook>, +now: Nat, +origin: String) -> List<&2, D.Node>:
  match hs:
    case Nil{}:
      Nil{}
    case h <> t:
      WB.hook.loc(lc, h, now, origin) <> WB.hooks.go.loc(lc, t, now, origin)

# how to call a webhook: the key is the secret's 32 bytes (hexkey), as
# core/sign.bend's Sign.header keys it

def WB.hooks.go(hs: List<&2, BT.Hook>, +now: Nat, +origin: String) -> List<&2, D.Node>:
  WB.hooks.go.loc(I.En{}, hs, now, origin)

def WB.curl(+url: String, +secret: String) -> String:
  "ts=$(date +%s); body='{\"text\":\"hello\"}'\n"
    ++ "sig=$(printf '%s.%s' \"$ts\" \"$body\" | openssl dgst -sha256 -mac HMAC -macopt hexkey:" ++ secret ++ " | awk '{print $NF}')\n"
    ++ "curl -X POST " ++ url ++ " -H \"X-Backplane-Timestamp: $ts\" -H \"X-Backplane-Signature: sha256=$sig\" -d \"$body\""

# a new webhook's secret, shown once (only this page has it)
def WB.secret.k.loc(+lc: I.Loc, show: Bool, +url: String, +secret: String) -> List<&2, D.Node>:
  match show:
    case True{}:
      [WB.div("secret", [
        D.Dom.span("secret-label", MSG.T.web.hooks.secret(lc)),
        WB.div("secret-line", [D.El{"code", "", Nil{}, [D.Txt{secret}]}, D.Dom.copy.loc(lc, secret)]),
        WB.div("codeblock", [D.Dom.copy.loc(lc, WB.curl(url, secret)), D.El{"pre", "", Nil{}, [D.El{"code", "", Nil{}, [D.Txt{WB.curl(url, secret)}]}]}]),
        D.Dom.span("secret-label", MSG.T.hooks.github.note(lc, url)),
        WB.btns([D.Dom.button("", "hook-done", "", MSG.T.hooks.secret.hide(lc), MSG.T.tour.done(lc))])])]
    case False{}:
      Nil{}

def WB.secret.k(show: Bool, +url: String, +secret: String) -> List<&2, D.Node>:
  WB.secret.k.loc(I.En{}, show, url, secret)

def WB.line.loc(+lc: I.Loc, +s: String) -> D.Node:
  WB.div("secret-line", [D.El{"code", "", Nil{}, [D.Txt{s}]}, D.Dom.copy.loc(lc, s)])

def WB.line(+s: String) -> D.Node:
  WB.line.loc(I.En{}, s)

def WB.token.curl(+url: String, +tok: String) -> String:
  "curl -X POST " ++ url ++ " -H \"Authorization: Bearer " ++ tok ++ "\" -F transcription=hello -F recordedAt=$(date +%s000)"

# a phone wants HTTPS: say how to get it when the hub has none
def WB.token.warn.loc(+lc: I.Loc, +url: String) -> List<&2, D.Node>:
  Bool.pick(List<&2, D.Node>, String.starts_with(url, "http://"),
    [D.Dom.span("secret-label", MSG.T.hooks.http.note(lc))],
    Nil{})

# a webhook's bearer token and where to post, shown until hidden

def WB.token.warn(+url: String) -> List<&2, D.Node>:
  WB.token.warn.loc(I.En{}, url)

def WB.token.k.loc(+lc: I.Loc, show: Bool, +url: String, +tok: String, +hid: String) -> List<&2, D.Node>:
  match show:
    case True{}:
      [WB.div("secret", List.concat(&2, D.Node, [
        [D.Dom.span("secret-label", MSG.T.hooks.url(lc)), WB.line.loc(lc, url),
         D.Dom.span("secret-label", MSG.T.hooks.header(lc)), WB.line.loc(lc, "Authorization: Bearer " ++ tok),
         D.Dom.span("secret-label", MSG.T.hooks.token(lc)), WB.line.loc(lc, tok)],
        WB.token.warn.loc(lc, url),
        [WB.div("codeblock", [D.Dom.copy.loc(lc, WB.token.curl(url, tok)), D.El{"pre", "", Nil{}, [D.El{"code", "", Nil{}, [D.Txt{WB.token.curl(url, tok)}]}]}]),
         D.Dom.span("secret-label", MSG.T.hooks.pebble.note(lc)),
         WB.btns([D.Dom.button("", "hook-retoken", hid, MSG.T.hooks.token.new.tip(lc), MSG.T.hooks.token.new(lc)),
           D.Dom.button("", "hook-untoken", hid, MSG.T.hooks.token.off.tip(lc), MSG.T.hooks.off(lc)),
           D.Dom.button("", "hook-done", "", MSG.T.hooks.token.hide(lc), MSG.T.tour.done(lc))])]]))]
    case False{}:
      Nil{}

def WB.token.k(show: Bool, +url: String, +tok: String, +hid: String) -> List<&2, D.Node>:
  WB.token.k.loc(I.En{}, show, url, tok, hid)

def WB.hooks.loc(+lc: I.Loc, +u: U.Ui, +b: BT.Bot) -> D.Node:
  WB.div("bot-list", List.concat(&2, D.Node, [
    WB.token.k.loc(lc, Bool.and(Bool.not(String.is_empty(U.Ui.scratch(u, "@hook.token"))),
        String.eq(BT.Hook.bot(BT.World.hook(M.State.bots(U.Ui.state(u)), U.Ui.scratch(u, "@hook.tokfor"))), BT.Bot.id(b))),
      U.Ui.scratch(u, "@hook.tokurl"), U.Ui.scratch(u, "@hook.token"), U.Ui.scratch(u, "@hook.tokfor")),
    WB.secret.k.loc(lc, Bool.and(Bool.not(String.is_empty(U.Ui.scratch(u, "@hook.secret"))),
        String.eq(BT.Hook.bot(BT.World.hook(M.State.bots(U.Ui.state(u)), U.Ui.scratch(u, "@hook.id"))), BT.Bot.id(b))),
      WB.page(u) ++ "/hook/" ++ U.Ui.scratch(u, "@hook.id"), U.Ui.scratch(u, "@hook.secret")),
    WB.hooks.go.loc(lc, BI.BU.hooks(u, BT.Bot.id(b)), U.Side.now(u), WB.page(u)),
    [WB.div("bform wide row", [WB.input("hook-name", "hname", BI.BU.field(u, "hname"), MSG.T.bots.name(lc), "hook-new"),
      D.Dom.button("primary", "hook-new", "", MSG.T.hooks.create.tip(lc), MSG.T.hooks.new(lc))])]]))

# Settings

def WB.hooks(+u: U.Ui, +b: BT.Bot) -> D.Node:
  WB.hooks.loc(I.En{}, u, b)

def WB.looks.loc(+lc: I.Loc, n: Nat, +i: Nat, +cur: Nat) -> List<&2, D.Node>:
  match n:
    case 0n:
      Nil{}
    case 1n+k:
      D.El{"button", "", [D.At{"class", Bool.pick(String, Nat.is_eq(i, cur), "look-pick on", "look-pick")}, D.At{"title", MSG.T.bots.look.number(lc, Nat.show(i))}, D.At{"data-value", Nat.show(i)}, D.On{"click", "bot-look"}],
        [WC.WCat.img(i, "idle", 40)]} <> WB.looks.loc(lc, k, 1n+i, cur)

def WB.looks(n: Nat, +i: Nat, +cur: Nat) -> List<&2, D.Node>:
  WB.looks.loc(I.En{}, n, i, cur)

def WB.set(+label: String, kids: List<&2, D.Node>) -> D.Node:
  WB.div("bset", D.Dom.span("set-label", label) <> kids)

def WB.persona.loc(+lc: I.Loc, +u: U.Ui, +b: BT.Bot) -> D.Node:
  WB.set(MSG.T.bots.persona(lc), [
    WB.area("bot-edit-persona", "persona", Bool.pick(String, String.is_empty(BI.BU.field(u, "persona")), BT.Bot.persona(b), BI.BU.field(u, "persona")), MSG.T.bots.persona.placeholder(lc)),
    WB.btns(Bool.pick(List<&2, D.Node>, String.is_empty(BI.BU.field(u, "persona")), Nil{},
      [D.Dom.button("primary", "bot-edit", "", MSG.T.bots.persona.save(lc), MSG.T.common.save(lc)), D.Dom.button("", "bfield", WB.bf("persona", ""), MSG.T.bots.persona.revert(lc), MSG.T.common.revert(lc))]))])

def WB.persona(+u: U.Ui, +b: BT.Bot) -> D.Node:
  WB.persona.loc(I.En{}, u, b)

def WB.gurl.loc(+lc: I.Loc, +url: String) -> List<&2, D.Node>:
  Bool.pick(List<&2, D.Node>, String.is_empty(url), Nil{}, [
    WB.div("bform-btns", [D.El{"a", "", [D.At{"class", "link"}, D.At{"href", url}, D.At{"target", "_blank"}, D.At{"rel", "noopener noreferrer"}], [D.Txt{MSG.T.google.open(lc)}]}])])

# the user's own OAuth client, shown on asking

def WB.gurl(+url: String) -> List<&2, D.Node>:
  WB.gurl.loc(I.En{}, url)

def WB.gown.k.loc(+lc: I.Loc, open: Bool, +u: U.Ui) -> List<&2, D.Node>:
  match open:
    case True{}:
      [WB.sealed("google-id", "google", "text", MSG.T.web.google.client.id(lc)),
       WB.sealed("google-secret", "google", "password", MSG.T.google.client.secret(lc)),
       WB.btns([D.Dom.button("", "sealed", "google", MSG.T.web.google.client.save(lc), MSG.T.google.client.save(lc)),
         D.Dom.button("", "bfield", WB.bf("gown", ""), MSG.T.google.client.hide(lc), MSG.T.common.hide(lc))])]
    case False{}:
      Nil{}

# one signed-in account and its own sign-out

def WB.gown.k(open: Bool, +u: U.Ui) -> List<&2, D.Node>:
  WB.gown.k.loc(I.En{}, open, u)

def WB.gsig.loc(+lc: I.Loc, +u: U.Ui, +e: String) -> List<&2, D.Node>:
  Bool.pick(List<&2, D.Node>, String.eq(U.Bots.sig.editing(u), e),
    [WB.div("bform wide", [WB.area("google-sig", "gsig", BI.BU.field(u, "gsig"), MSG.T.google.signature.placeholder(lc, e)),
      WB.btns([D.Dom.button("primary", "google", "sig-save", MSG.T.google.signature.save(lc), MSG.T.common.save(lc)), D.Dom.button("", "google", "sig-cancel", MSG.T.google.signature.cancel(lc), MSG.T.common.cancel(lc))])])],
    Nil{})

def WB.gsig(+u: U.Ui, +e: String) -> List<&2, D.Node>:
  WB.gsig.loc(I.En{}, u, e)

def WB.gaccts.loc(+lc: I.Loc, xs: List<&2, String>, +u: U.Ui) -> List<&2, D.Node>:
  match xs:
    case Nil{}:
      Nil{}
    case +e <> t:
      D.El{"div", "gacct:" ++ e, [D.At{"class", "item"}], [WB.div("item-main", [D.Dom.span("item-key", e),
          D.Dom.span("item-url", Bool.pick(String, String.is_empty(U.Bots.sig.of(u, e)), MSG.T.google.signature.none(lc), U.Bots.sig.of(u, e)))]),
        WB.div("item-btns", [D.Dom.button("", "google", "sig-edit:" ++ e, MSG.T.google.signature.tip(lc, e), MSG.T.google.signature(lc)),
          D.Dom.button("", "google", "signout:" ++ e, MSG.T.google.signout.tip(lc, e), MSG.T.agy.signout(lc))])]} <> List.append(&2, D.Node, WB.gsig.loc(lc, u, e), WB.gaccts.loc(lc, t, u))

# Google: connect (sign in in a new tab; again for another account; paste
# the page it lands on when that tab is on another device), status, and
# every account signed in with its own sign-out

def WB.gaccts(xs: List<&2, String>, +u: U.Ui) -> List<&2, D.Node>:
  WB.gaccts.loc(I.En{}, xs, u)

def WB.google.loc(+lc: I.Loc, +u: U.Ui) -> D.Node:
  +note = U.Sealed.say(u, "google", U.Sealed.say(u, "gpaste", U.Ui.scratch(u, "@google")))
  WB.set("Google", List.concat(&2, D.Node, [
    Bool.pick(List<&2, D.Node>, String.is_empty(note), Nil{}, [D.Dom.span("set-note", note)]),
    [WB.btns([D.Dom.button("primary", "google", "connect", MSG.T.agy.google.signin(lc), Bool.pick(String, List.is_empty(&2, String, U.Bots.google.accts(u)), MSG.T.google.connect(lc), MSG.T.agy.account.add(lc))),
       D.Dom.button("", "google", "status", MSG.T.google.status.tip(lc), MSG.T.bots.tab.status(lc)),
       D.Dom.button("", "bfield", WB.bf("gown", "1"), MSG.T.google.own.tip(lc), MSG.T.google.client.own(lc))])],
    WB.gaccts.loc(lc, U.Bots.google.accts(u), u),
    WB.gown.k.loc(lc, Bool.not(String.is_empty(BI.BU.field(u, "gown"))), u),
    WB.gurl.loc(lc, U.Ui.scratch(u, "@google.url")),
    Bool.pick(List<&2, D.Node>, String.is_empty(U.Ui.scratch(u, "@google.url")), Nil{}, [
      WB.div("bform wide row", [WB.sealed("google-paste", "gpaste", "text", MSG.T.web.google.callback(lc)),
        D.Dom.button("", "sealed", "gpaste", MSG.T.google.finish.tip(lc), MSG.T.common.finish(lc))])])]))

def WB.google(+u: U.Ui) -> D.Node:
  WB.google.loc(I.En{}, u)

def WB.peer.loc(+lc: I.Loc, p: BT.Peer) -> D.Node:
  match p:
    case BT.Peer{+id, name, url, _, _}:
      D.El{"div", "peer:" ++ id, [D.At{"class", "item"}], [WB.div("item-main", [D.Dom.span("item-key", name), D.Dom.span("item-url", url)]),
        WB.div("item-btns", [D.Dom.button("", "peer-revoke", id, MSG.T.machines.unlink.tip(lc), MSG.T.machines.unlink(lc))])]}

def WB.peer(p: BT.Peer) -> D.Node:
  WB.peer.loc(I.En{}, p)

def WB.peers.go.loc(+lc: I.Loc, ps: List<&2, BT.Peer>) -> List<&2, D.Node>:
  match ps:
    case Nil{}:
      Nil{}
    case p <> t:
      WB.peer.loc(lc, p) <> WB.peers.go.loc(lc, t)

def WB.peers.go(ps: List<&2, BT.Peer>) -> List<&2, D.Node>:
  WB.peers.go.loc(I.En{}, ps)

def WB.invite.loc(+lc: I.Loc, +s: String) -> List<&2, D.Node>:
  Bool.pick(List<&2, D.Node>, String.is_empty(s), Nil{}, [WB.div("secret-line", [D.El{"code", "", Nil{}, [D.Txt{s}]}, D.Dom.copy.loc(lc, s)])])

# linked machines: make an invite, join with one, the links

def WB.invite(+s: String) -> List<&2, D.Node>:
  WB.invite.loc(I.En{}, s)

def WB.machines.loc(+lc: I.Loc, +u: U.Ui) -> D.Node:
  WB.set(MSG.T.machines.linked(lc), List.concat(&2, D.Node, [
    WB.peers.go.loc(lc, BI.BU.peers(u)),
    [WB.div("bform wide row", [WB.input("peer-url", "purl", BI.BU.field(u, "purl"), MSG.T.machines.url(lc), "peer-invite"),
      D.Dom.button("", "peer-invite", "", MSG.T.machines.invite.tip(lc), MSG.T.machines.invite(lc))])],
    WB.invite.loc(lc, U.Ui.scratch(u, "@peer.invite")),
    [WB.div("bform wide row", [WB.sealed("peer-join", "invite", "text", MSG.T.machines.invite.paste(lc)),
      D.Dom.button("", "sealed", "invite", MSG.T.machines.join.tip(lc), MSG.T.bots.invite.join(lc))])],
    Bool.pick(List<&2, D.Node>, String.is_empty(U.Sealed.err(u, "invite")), Nil{}, [D.Dom.span("set-note", U.Sealed.err(u, "invite"))])]))

def WB.machines(+u: U.Ui) -> D.Node:
  WB.machines.loc(I.En{}, u)

def WB.del.k.loc(+lc: I.Loc, armed: Bool, +id: String, +name: String) -> List<&2, D.Node>:
  match armed:
    case True{}:
      [D.Dom.button("danger", "bot-delete", id, MSG.T.bots.delete.tip(lc), MSG.T.bots.delete.named(lc, name)), D.Dom.button("", "bfield", WB.bf("delbot", ""), MSG.T.bots.keep(lc), MSG.T.common.cancel(lc))]
    case False{}:
      [D.Dom.button("", "bfield", WB.bf("delbot", id), MSG.T.bots.delete.button.tip(lc), MSG.T.bots.delete(lc))]

def WB.del.k(armed: Bool, +id: String, +name: String) -> List<&2, D.Node>:
  WB.del.k.loc(I.En{}, armed, id, name)

def WB.settings.loc(+lc: I.Loc, +u: U.Ui, +b: BT.Bot) -> D.Node:
  WB.div("bot-list", [
    WB.persona.loc(lc, u, b),
    WB.set(MSG.T.bots.look(lc), [WB.div("looks", WB.looks.loc(lc, WC.WCat.looks(), 0n, Nat.mod(BT.Bot.look(b), WC.WCat.looks())))]),
    WB.google.loc(lc, u),
    WB.machines.loc(lc, u),
    WB.set("", [WB.btns(WB.del.k.loc(lc, String.eq(BI.BU.field(u, "delbot"), BT.Bot.id(b)), BT.Bot.id(b), BT.Bot.name(b)))])])

def WB.settings(+u: U.Ui, +b: BT.Bot) -> D.Node:
  WB.settings.loc(I.En{}, u, b)

def WB.pos(xs: List<&2, String>, +s: String, +i: Nat) -> Nat:
  match xs:
    case Nil{}:
      i
    case +x <> t:
      Bool.pick(Nat, String.eq(x, s), i, WB.pos(t, s, 1n+i))

def WB.body.k(+lc: I.Loc, i: Nat, +u: U.Ui, +b: BT.Bot) -> D.Node:
  match i:
    case 0n:
      WS.WSpace.view.loc(lc, BI.BU.space(u, b), BT.Bot.id(b))
    case 1n:
      WB.browser(lc, u, b)
    case 2n:
      WB.tasks.loc(lc, u, b)
    case 3n:
      WB.memory.loc(lc, u, b)
    case 4n:
      WB.routines(lc, u, b)
    case 5n:
      WB.hooks.loc(lc, u, b)
    case _:
      WB.settings.loc(lc, u, b)

# a tab other than Chat
def WB.body(+lc: I.Loc, +u: U.Ui, +b: BT.Bot, +tab: String) -> D.Node:
  D.El{"div", "bot-body:" ++ tab, [D.At{"class", "bot-body"}], [WB.body.k(lc, WB.pos(["space", "browser", "tasks", "memory", "routines", "hooks"], tab, 0n), u, b)]}

# The uberbot (docs/uber.md)
# --------------------------

# its sidebar row, pinned above the search: two buttons, so Tab reaches
# each and Enter/Space does its click. Its cat and name do action "uber"
# (Chat with the Mailroom on, else Status: Uber.show). What waits on the
# person (Plane.counts) is a button of its own that always opens the Status
# tab, where the counted rows are listed and each opens its thread
# ("uber-bot <bot>|status", Uber.bot.at): a count always leads to its rows.
def WB.uber.counts(none: Bool, +id: String, +name: String, +counts: String) -> List<&2, D.Node>:
  match none:
    case True{}: Nil{}
    case False{}:
      [D.El{"button", "uber-counts:" ++ id, [D.At{"type", "button"}, D.At{"class", "uber-counts"}, D.At{"title", name ++ ": " ++ counts}, D.At{"aria-label", name ++ " status: " ++ counts}, D.At{"data-value", id ++ "|status"}, D.On{"click", "uber-bot"}], [D.Txt{counts}]}]

def WB.uber.k(+lc: I.Loc, r: BI.BRow, +counts: String) -> D.Node:
  match r:
    case BI.BRow{+id, +name, _, +look, +mood, _, sel, _, _, _}:
      D.El{"div", "uber:" ++ id, [D.At{"class", Bool.pick(String, sel, "bot-row uber-row on", "bot-row uber-row")}], List.append(&2, D.Node,
        [D.El{"button", "uber-open:" ++ id, [D.At{"type", "button"}, D.At{"class", "uber-open"}, D.At{"title", id}, D.At{"aria-label", name}, D.At{"data-value", id}, D.On{"click", "uber"}],
          [WC.WCat.img(look, mood, 28), WB.div("bot-text", [D.Dom.span("bot-name", name), D.Dom.span("uber-mood", Bool.pick(String, String.is_empty(counts), BI.Mood.note.loc(lc, mood), ""))])]}],
        WB.uber.counts(String.is_empty(counts), id, name, counts))}

# (the counts are Plane.tally.words with the sidebar's index, made once a
# frame for both)
def WB.uber.m(+lc: I.Loc, m: Maybe<&2, BI.BRow>, +u: U.Ui, +ix: U.SIx) -> List<&2, D.Node>:
  match m:
    case Some{r}:
      [WB.uber.k(lc, r, PL.Plane.counts.t(lc, PL.Plane.tally(u, ix, U.Side.rows.ix(U.Ui.state(u), ix, U.Side.now(u)))))]
    case None{}:
      Nil{}

# no uberbot, no row
def WB.uber(+lc: I.Loc, +u: U.Ui, +ix: U.SIx) -> List<&2, D.Node>:
  WB.uber.m(lc, BI.BU.uber(u), u, ix)

# the dot of a row: thread states and bot moods alike
def WB.pdot(+s: String) -> String:
  Bool.pick(String, Bool.or(String.eq(s, "approval"), String.eq(s, "wait")), "dot approval",
    Bool.pick(String, String.eq(s, "question"), "dot input",
      Bool.pick(String, Bool.or(String.eq(s, "failed"), String.eq(s, "error")), "dot failed",
        Bool.pick(String, Bool.or(String.eq(s, "working"), Bool.or(String.eq(s, "work"), String.eq(s, "talk"))), "dot working",
          Bool.pick(String, Bool.or(String.eq(s, "waiting"), String.eq(s, "think")), "dot queued",
            Bool.pick(String, String.eq(s, "unread"), "dot waiting", "dot"))))))

def WB.pago.loc(+lc: I.Loc, +kind: String, +now: Nat, +at: Nat) -> String:
  Bool.pick(String, Bool.or(String.eq(kind, "routine"), Bool.or(Nat.is_eq(at, 0n), Nat.is_gt(at, now))), "", I.I18n.ago(lc, now, at))

# a row's mark: a dot for its state, a bell for a reminder

def WB.pago(+kind: String, +now: Nat, +at: Nat) -> String:
  WB.pago.loc(I.En{}, kind, now, at)

def WB.pactivity(+state: String) -> String:
  Bool.pick(String, String.eq(state, "stalled"), "stalled", Bool.pick(String, Bool.or(String.eq(state, "approval"), Bool.or(String.eq(state, "question"), String.eq(state, "failed"))), "blocked",
    Bool.pick(String, Bool.or(String.eq(state, "working"), String.eq(state, "waiting")), "active", Bool.pick(String, String.eq(state, "settled"), "settled", Bool.pick(String, String.eq(state, "finished"), "finished", "inactive")))))

def WB.pmark.signal(+lc: I.Loc, +state: String) -> D.Node:
  D.El{"span", "", [D.At{"class", "dot " ++ WB.pactivity(state)}, D.At{"role", "img"},
    D.At{"aria-label", SG.Signal.label.loc(lc, WB.pactivity(state))}, D.At{"title", SG.Signal.label.loc(lc, WB.pactivity(state))}], Nil{}}

def WB.pmark.activity(+state: String) -> Bool:
  Bool.or(String.eq(state, "stalled"), Bool.or(Bool.or(String.eq(state, "approval"), String.eq(state, "question")),
    Bool.or(Bool.or(String.eq(state, "failed"), String.eq(state, "working")),
      Bool.or(Bool.or(String.eq(state, "idle"), String.eq(state, "finished")), Bool.or(String.eq(state, "settled"), String.eq(state, "waiting"))))))

def WB.pmark.loc(+lc: I.Loc, +state: String) -> D.Node:
  Bool.pick(D.Node, String.eq(state, "reminder"), D.El{"span", "", [D.At{"class", "pbell"}], [WI.WIcon.svg("bell", 12)]},
    Bool.pick(D.Node, WB.pmark.activity(state), WB.pmark.signal(lc, state),
      D.El{"span", "", [D.At{"class", WB.pdot(state)}, D.At{"role", "img"}, D.At{"aria-label", state}], Nil{}}))

def WB.pmark(+state: String) -> D.Node:
  WB.pmark.loc(I.En{}, state)

# one row: a button for what a click sends (act, val), a fixed height
def WB.prow(+lc: I.Loc, r: PL.PRow, +now: Nat) -> D.Node:
  match r:
    case PL.PRow{+kind, +id, +act, +val, +title, +sub, +machine, +state, +at, away}:
      D.El{"button", kind ++ ":" ++ id, [D.At{"class", Bool.pick(String, away, "plane-row away", "plane-row")}, D.At{"title", title}, D.At{"data-value", val}, D.On{"click", act}],
        List.concat(&2, D.Node, [[WB.pmark.loc(lc, state)], D.Dom.when(Bool.not(String.is_empty(PL.Plane.why(lc, state))), D.Dom.span("why " ++ state, PL.Plane.why(lc, state))),
          [D.Dom.span("title", PL.Plane.title.shown(lc, kind, title)), D.Dom.span("sub", sub), D.Dom.span("mach", machine), D.Dom.span("ago", WB.pago.loc(lc, kind, now, at))]])}

def WB.prows(+lc: I.Loc, rs: List<&2, PL.PRow>, +now: Nat) -> List<&2, D.Node>:
  match rs:
    case Nil{}:
      Nil{}
    case r <> t:
      WB.prow(lc, r, now) <> WB.prows(lc, t, now)

# a labelled section; none of it when it is empty
def WB.psec(+lc: I.Loc, +label: String, rs: List<&2, PL.PRow>, +more: Nat, +now: Nat) -> List<&2, D.Node>:
  match rs:
    case Nil{}:
      Nil{}
    case _ <> _:
      [WB.div("plane-sec", D.Dom.span("plane-label", label) <> List.append(&2, D.Node, WB.prows(lc, rs, now), D.Dom.when(Nat.is_gt(more, 0n), D.Dom.span("plane-more", MSG.T.plane.more(lc, Nat.show(more))))))]

def WB.pcount(+label: String, +n: Nat) -> D.Node:
  WB.div(Bool.pick(String, Nat.is_eq(n, 0n), "plane-count", "plane-count on"), [D.Dom.span("n", Nat.show(n)), D.Dom.span("l", label)])

def WB.pmach(m: PL.PMach) -> D.Node:
  match m:
    case PL.PMach{+name, +state}:
      WB.div("plane-mach " ++ state, [D.Dom.span(Bool.pick(String, String.eq(state, "here"), "dot working", Bool.pick(String, String.eq(state, "syncing"), "dot queued", "dot")), ""), D.Dom.span("", name)])

def WB.pmachs(ms: List<&2, PL.PMach>) -> List<&2, D.Node>:
  match ms:
    case Nil{}:
      Nil{}
    case m <> t:
      WB.pmach(m) <> WB.pmachs(t)

# the strip: the counters and the machines
# where the conversation is held, and the offer to make this machine the
# home (core's Plane.home decides)
def WB.phome(+lc: I.Loc, +h: PL.PHome) -> List<&2, D.Node>:
  Bool.pick(List<&2, D.Node>, PL.PHome.on(h),
    [WB.div("plane-home", List.concat(&2, D.Node, [[D.Dom.span("", PL.Plane.home.line(lc, h))],
      Bool.pick(List<&2, D.Node>, PL.Plane.home.button(h), [D.Dom.button("plane-home-btn", "uber-home", "", PL.Plane.home.label(lc), PL.Plane.home.label(lc))], Nil{})]))],
    Nil{})

def WB.pstrip(+lc: I.Loc, +p: PL.Plane, +h: PL.PHome) -> D.Node:
  WB.div("plane-strip", List.concat(&2, D.Node, [[
    WB.pcount(MSG.T.plane.need(lc), List.length(&2, PL.PRow, PL.Plane.need(p))),
    WB.pcount(MSG.T.plane.running(lc), List.length(&2, PL.PRow, PL.Plane.run(p))),
    WB.pcount(MSG.T.plane.bots.working(lc), PL.Plane.working(p)),
    WB.pcount(MSG.T.plane.rooms.unread(lc), List.length(&2, PL.PRow, PL.Plane.rooms(p))),
    WB.div("plane-machs", WB.pmachs(PL.Plane.machines(p)))], WB.phome(lc, h)]))

def WB.pidle.more(+shown: Nat, +n: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_gt(n, shown), Nat.sub(n, shown), 0n)

def WB.pempty(+p: PL.Plane) -> Bool:
  Bool.and(List.is_empty(&2, PL.PRow, PL.Plane.need(p)), Bool.and(List.is_empty(&2, PL.PRow, PL.Plane.run(p)), List.is_empty(&2, PL.PRow, PL.Plane.idle(p))))

def WB.plane.k(+lc: I.Loc, +p: PL.Plane, +h: PL.PHome, +now: Nat) -> D.Node:
  WB.div("plane", [
    WB.pstrip(lc, p, h),
    WB.div("plane-grid", [
      WB.div("plane-main", List.concat(&2, D.Node, [
        WB.psec(lc, MSG.T.plane.needs(lc), PL.Plane.need(p), 0n, now),
        WB.psec(lc, MSG.T.plane.running(lc), PL.Plane.run(p), 0n, now),
        WB.psec(lc, MSG.T.web.plane.idle(lc), PL.Plane.idle(p), WB.pidle.more(List.length(&2, PL.PRow, PL.Plane.idle(p)), PL.Plane.idle_n(p)), now),
        D.Dom.when(WB.pempty(p), WB.note(MSG.T.sidebar.nothing(lc)))])),
      WB.div("plane-side", List.concat(&2, D.Node, [
        WB.psec(lc, MSG.T.tour.title.bots(lc), PL.Plane.bots(p), 0n, now),
        WB.psec(lc, MSG.T.bots.rooms(lc), PL.Plane.rooms(p), 0n, now),
        WB.psec(lc, MSG.T.plane.locks(lc), PL.Plane.locks(p), 0n, now), WB.psec(lc, MSG.T.bots.tab.routines(lc), PL.Plane.routines(p), 0n, now)]))])])

# the Status tab's body
def WB.plane(+lc: I.Loc, +u: U.Ui) -> D.Node:
  WB.plane.k(lc, PL.Plane.signals.of(u, U.Side.now(u), True{}), PL.Plane.home(u), U.Side.now(u))

# a tab's body: Status here, the others in WB.body; only the one shown is built
def WB.main.k(+lc: I.Loc, status: Bool, +u: U.Ui, +b: BT.Bot, +tab: String) -> D.Node:
  match status:
    case True{}:
      D.El{"div", "bot-body:status", [D.At{"class", "bot-body"}], [WB.plane(lc, u)]}
    case False{}:
      WB.body(lc, u, b, tab)

def WB.main.other(+lc: I.Loc, +u: U.Ui, +b: BT.Bot, +tab: String) -> D.Node:
  WB.main.k(lc, String.eq(tab, "status"), u, b, tab)

# Rooms and bots elsewhere
# ------------------------

# a composer for a room or a bot elsewhere
def WB.compose.loc(+lc: I.Loc, +id: String, +field: String, +value: String, +ph: String, +act: String) -> D.Node:
  WB.div("composer", [
    D.El{"textarea", id, [D.At{"id", id}, D.At{"rows", "2"}, D.At{"placeholder", ph}, D.At{"value", value}, D.At{"data-bfield", field}, D.On{"input", "bfield"}, D.At{"data-enter", act}], Nil{}},
    WB.div("composer-foot", [D.Dom.span("", ""), D.Dom.button("primary", act, "", "Enter", MSG.T.composer.send(lc))])])

def WB.compose(+id: String, +field: String, +value: String, +ph: String, +act: String) -> D.Node:
  WB.compose.loc(I.En{}, id, field, value, ph, act)

def WB.post.bot.loc(+lc: I.Loc, m: Maybe<&2, BT.Bot>, +from: String, text: String, +at: String, +st: M.State, +now: Nat, +fl: Nat, +fm: String) -> List<&2, D.Node>:
  match m:
    case Some{+b}:
      [WC.WCat.img(BT.Bot.look(b), BI.Mood.bot(st, now, b), 28),
       WB.div("post-main", [WB.div("post-from", [D.Dom.span("", BT.Bot.name(b)), D.Dom.span("ago", at)]), WB.div("body", D.Md.render.loc(lc, text))])]
    case None{}:
      [WC.WCat.img(fl, fm, 28),
       WB.div("post-main", [WB.div("post-from", [D.Dom.span("", from), D.Dom.span("ago", at)]), WB.div("body", D.Md.render.loc(lc, text))])]

def WB.post.bot(m: Maybe<&2, BT.Bot>, +from: String, text: String, +at: String, +st: M.State, +now: Nat, +fl: Nat, +fm: String) -> List<&2, D.Node>:
  WB.post.bot.loc(I.En{}, m, from, text, at, st, now, fl, fm)

def WB.post.k.loc(+lc: I.Loc, you: Bool, +id: String, +from: String, text: String, +at: String, +u: U.Ui, +fl: Nat, +fm: String) -> D.Node:
  match you:
    case True{}:
      D.El{"div", id, [D.At{"class", "msg user"}], [WB.div("body plain", D.Dom.linked(text))]}
    case False{}:
      D.El{"div", id, [D.At{"class", "post"}], WB.post.bot.loc(lc, BT.World.find.id(BT.World.bot(M.State.bots(U.Ui.state(u)), from)), from, text, at, U.Ui.state(u), U.Side.now(u), fl, fm)}

# a post; one from elsewhere shows the cat fl/fm (a bot elsewhere's own)

def WB.post.k(you: Bool, +id: String, +from: String, text: String, +at: String, +u: U.Ui, +fl: Nat, +fm: String) -> D.Node:
  WB.post.k.loc(I.En{}, you, id, from, text, at, u, fl, fm)

def WB.post.loc(+lc: I.Loc, p: BT.Post, +u: U.Ui, +fl: Nat, +fm: String) -> D.Node:
  match p:
    case BT.Post{+id, _, +from, text, _, +at}:
      WB.post.k.loc(lc, String.eq(from, "you"), id, from, text, I.I18n.ago(lc, U.Side.now(u), at), u, fl, fm)

def WB.post(p: BT.Post, +u: U.Ui, +fl: Nat, +fm: String) -> D.Node:
  WB.post.loc(I.En{}, p, u, fl, fm)

def WB.posts.loc(+lc: I.Loc, ps: List<&2, BT.Post>, +u: U.Ui, +fl: Nat, +fm: String) -> List<&2, D.Node>:
  match ps:
    case Nil{}:
      Nil{}
    case p <> t:
      WB.post.loc(lc, p, u, fl, fm) <> WB.posts.loc(lc, t, u, fl, fm)

def WB.posts(ps: List<&2, BT.Post>, +u: U.Ui, +fl: Nat, +fm: String) -> List<&2, D.Node>:
  WB.posts.loc(I.En{}, ps, u, fl, fm)

def WB.timeline(ns: List<&2, D.Node>) -> D.Node:
  D.El{"div", "timeline", [D.At{"class", "timeline"}, D.At{"id", "timeline"}], ns}

def WB.room.del.loc(+lc: I.Loc, armed: Bool, +id: String) -> List<&2, D.Node>:
  match armed:
    case True{}:
      [D.Dom.button("danger", "room-delete", id, MSG.T.rooms.delete.tip(lc), MSG.T.room.delete(lc)), D.Dom.button("", "bfield", WB.bf("delroom", ""), MSG.T.bots.keep(lc), MSG.T.common.cancel(lc))]
    case False{}:
      [D.Dom.button("", "bfield", WB.bf("delroom", id), MSG.T.rooms.delete.button.tip(lc), MSG.T.common.delete(lc))]

# a room: its members, its posts, and a line to write to it

def WB.room.del(armed: Bool, +id: String) -> List<&2, D.Node>:
  WB.room.del.loc(I.En{}, armed, id)

def WB.room.loc(+lc: I.Loc, +u: U.Ui, r: BT.Room) -> D.Node:
  match r:
    case BT.Room{+id, +name, ms, c, d}:
      D.El{"main", "room", [D.At{"class", "thread-view room-view"}], [
        D.El{"header", "", [D.At{"class", "thread-head"}], [
          D.Dom.button("back", "select", "", MSG.T.tour.back(lc), "‹"),
          D.Dom.span("title", "# " ++ name),
          D.Dom.span("meta", BI.BU.members(u, BT.Room{id, name, ms, c, d})),
          D.El{"div", "", [D.At{"class", "tools"}], WB.room.del.loc(lc, String.eq(BI.BU.field(u, "delroom"), id), id)}]},
        WB.timeline(Bool.pick(List<&2, D.Node>, List.is_empty(&2, BT.Post, BI.BU.posts(u, id)), [WB.note(MSG.T.rooms.empty(lc))], WB.posts.loc(lc, BI.BU.posts(u, id), u, 0n, "away"))),
        WB.compose.loc(lc, "room-post", "rpost", BI.BU.field(u, "rpost"), MSG.T.room.write(lc), "room-post")]}

# a bot only reported by a linked machine: the bot's view (BU.head: its
# cat, "name · machine", its mood, Chat alone) over the person's
# conversation with it, and a line to it

def WB.room(+u: U.Ui, r: BT.Room) -> D.Node:
  WB.room.loc(I.En{}, u, r)

def WB.remote.h(+lc: I.Loc, +u: U.Ui, +id: String, +hd: BI.Head) -> D.Node:
  D.El{"main", "remote", [D.At{"class", "thread-view bot-view room-view"}], [
    WB.header.loc(lc, hd, Nil{}, Nil{}),
    WB.tabs(lc, hd),
    WB.timeline(Bool.pick(List<&2, D.Node>, List.is_empty(&2, BT.Post, BI.BU.remote.posts(u, id)), [WB.note(MSG.T.remote.empty(lc))], WB.posts.loc(lc, BI.BU.remote.posts(u, id), u, BI.Head.look(hd), BI.Head.mood(hd)))),
    WB.compose.loc(lc, "remote-send", "rtext", BI.BU.field(u, "rtext"), MSG.T.bots.remote.write(lc, id), "remote-send")]}

def WB.remote(+lc: I.Loc, +u: U.Ui, +id: String) -> D.Node:
  WB.remote.h(lc, u, id, BI.BU.head(u, BI.VRemote{id}))

# Mailroom capability is enabled only now that this client draws Mailv.
def WB.mail.presentation(provisional: Bool, +lc: I.Loc, u: U.Ui) -> D.Node:
  match provisional:
    case True{}: D.El{"div", "mailroom", [D.At{"id", "mailroom"}, D.At{"role", "status"}], [D.Txt{MSG.T.viewer.loading(lc)}]}
    case False{}: WM.WM.page(u)

def WB.main.mail(mail: Bool, +lc: I.Loc, +u: U.Ui, b: BT.Bot, tab: String) -> D.Node:
  match mail:
    case True{}: WB.mail.presentation(String.eq(U.Ui.scratch(u, "@presentation"), "1"), lc, u)
    case False{}: WB.main.other(lc, u, b, tab)

# Keep child routes and their existing authority guards inside the parent.
# Only the selected panel is built.
def WB.main.nested(ts: List<&2, String>, +lc: I.Loc, +u: U.Ui, +b: BT.Bot, +tab: String) -> D.Node:
  match ts:
    case Nil{}:
      WB.main.mail(String.eq(tab, "mail"), lc, u, b, tab)
    case +h <> +rest:
      D.El{"div", "bot-parent:" ++ BI.BU.nav.parent(BI.BU.tabs.for(u, BT.Bot.id(b)), tab), [D.At{"class", "bot-body"}], [
        D.El{"nav", "bot-subtabs", [D.At{"class", "bot-tabs"}, D.At{"aria-label", BI.BU.tab.label(lc, BI.BU.nav.parent(BI.BU.tabs.for(u, BT.Bot.id(b)), tab))}], WB.tabs.go(lc, h <> rest, tab)},
        WB.main.mail(String.eq(tab, "mail"), lc, u, b, tab)]}

def WB.main(+lc: I.Loc, +u: U.Ui, +b: BT.Bot, +tab: String) -> D.Node:
  WB.main.nested(BI.BU.nav.sections(BI.BU.tabs.for(u, BT.Bot.id(b)), tab), lc, u, b, tab)
