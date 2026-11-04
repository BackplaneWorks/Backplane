import Base
import ../core/i18n.bend as I
import ../core/msgs.bend as MSG
import ../core/client.bend as U
import ./dom.bend as D
import ./icon.bend as WI

# The phone arrangement's parts. A browser on a phone says so in every view
# request (host.js's phone-shell.js: "form": "phone"), thin-view's
# Session.renderer keeps it as the connection's "local.workspace.form", and
# view.bend's View.phone.page then draws the same pieces one screen at a
# time: a header with back, the screen, and on the root screens a tab bar.
# Which tab is open is this browser's own ("@ph.tab" in the session it
# holds), never the hub's, so the person's other clients never follow it.
# style.css's "phone:" sections lay it out, under .app.phone only.

def Phone.on(+u: U.Ui) -> Bool:
  String.eq(U.Ui.info(u, "local.workspace.form"), "phone")

# the root tabs: Now (what needs the person, then what runs: the default),
# Projects (the sidebar's projects and threads), Bots (its bots and
# rooms); Settings is the "settings" flag
def Phone.tab.k(+t: String) -> String:
  match t:
    case "projects": "projects"
    case "bots": "bots"
    case _: "now"

def Phone.now.label(lc: I.Loc) -> String:
  match lc:
    case I.Ja{}: "いま"
    case _: "Now"

def Phone.now.none(lc: I.Loc) -> String:
  match lc:
    case I.Ja{}: "対応が必要なものはありません"
    case _: "Nothing needs you"

def Phone.now.hint(lc: I.Loc) -> String:
  match lc:
    case I.Ja{}: "質問・承認・失敗・完了したスレッドがここに先に表示されます。"
    case _: "Threads that ask you something, fail or finish show here first, then the ones at work."

def Phone.tab(+u: U.Ui) -> String:
  Phone.tab.k(U.Ui.scratch(u, "@ph.tab"))

# the Now tab's bands (by core's Urg.key: rank * 2, + 1 when pinned):
# what needs the person (an approval, a question, a failure), what finished
# or stopped, what is at work, what is idle
def Phone.now.band(lc: I.Loc, +k: Nat) -> String:
  match lc:
    case I.Ja{}: Bool.pick(String, Nat.is_eq(k, 0n), "対応待ち", Bool.pick(String, Nat.is_eq(k, 1n), "完了", Bool.pick(String, Nat.is_eq(k, 2n), "作業中", "待機")))
    case _: Bool.pick(String, Nat.is_eq(k, 0n), "Needs you", Bool.pick(String, Nat.is_eq(k, 1n), "Finished", Bool.pick(String, Nat.is_eq(k, 2n), "At work", "Idle")))

def Phone.handles(+action: String) -> Bool:
  String.starts_with(action, "ph-")

def Phone.tab.settings(open: Bool, +u: U.Ui) -> U.Act:
  match open:
    case True{}: U.Act{u, Nil{}}
    case False{}: U.Ui.act(u, "flag", "settings")

def Phone.tab.pick(settings: Bool, +u: U.Ui, +value: String) -> U.Act:
  match settings:
    case True{}: Phone.tab.settings(U.Ui.flag(u, "settings"), u)
    case False{}: U.Act{U.Ui.flag_off(U.Ui.scratch_set(u, "@ph.tab", Phone.tab.k(value)), "settings"), Nil{}}

# ph-tab NAME opens a root tab; ph-sheet NAME opens a hub-drawn sheet ("" shuts it)
def Phone.act(+u: U.Ui, +action: String, +value: String) -> U.Act:
  match action:
    case "ph-tab": Phone.tab.pick(String.eq(value, "settings"), u, value)
    case "ph-sheet": U.Act{U.Ui.scratch_set(u, "@ph.sheet", value), Nil{}}
    case _: U.Act{u, Nil{}}

def Phone.badge(none: Bool, +n: Nat) -> List<&2, D.Node>:
  match none:
    case True{}: Nil{}
    case False{}: [D.El{"span", "", [D.At{"class", "ph-badge"}, D.At{"aria-label", Nat.show(n)}], [D.Txt{Bool.pick(String, Nat.is_gt(n, 99n), "99+", Nat.show(n))}]}]

def Phone.tab.button(+cur: String, +name: String, +label: String, +icon: String, +badge: Nat) -> D.Node:
  D.El{"button", "ph-tab:" ++ name, [D.At{"type", "button"}, D.At{"class", Bool.pick(String, String.eq(cur, name), "ph-tab on", "ph-tab")},
    D.At{"aria-current", Bool.pick(String, String.eq(cur, name), "page", "false")}, D.At{"data-value", name}, D.On{"click", "ph-tab"}],
    List.append(&2, D.Node, [WI.WIcon.svg(icon, 22), D.Dom.span("ph-tab-label", label)], Phone.badge(Nat.is_eq(badge, 0n), badge))}

# the bar under a root screen: need = threads needing you, unread = rooms
def Phone.tabs(+lc: I.Loc, +cur: String, +need: Nat, +unread: Nat) -> D.Node:
  D.El{"nav", "ph-tabs", [D.At{"class", "ph-tabs"}, D.At{"aria-label", "Sections"}], [
    Phone.tab.button(cur, "now", Phone.now.label(lc), "bell", need),
    Phone.tab.button(cur, "projects", MSG.T.tour.title.projects(lc), "folder", 0n),
    Phone.tab.button(cur, "bots", MSG.T.tour.title.bots(lc), "cpu", unread),
    Phone.tab.button(cur, "settings", MSG.T.palette.settings(lc), "settings", 0n)]}

# the header's ‹: host.js also sends it for the system back and an edge swipe
def Phone.back(+action: String, +value: String, +label: String) -> D.Node:
  D.El{"button", "ph-back", [D.At{"type", "button"}, D.At{"class", "ph-back"}, D.At{"aria-label", label}, D.At{"title", label},
    D.At{"data-value", value}, D.At{"data-ph-back", "1"}, D.On{"click", action}], [WI.WIcon.svg("chevron-left", 26)]}

def Phone.head(back: List<&2, D.Node>, title: List<&2, D.Node>, tools: List<&2, D.Node>) -> D.Node:
  D.El{"header", "ph-head", [D.At{"class", "ph-head"}], List.concat(&2, D.Node, [back,
    [D.El{"div", "ph-title", [D.At{"class", "ph-title"}], title}],
    [D.El{"div", "ph-tools", [D.At{"class", "ph-tools"}], tools}]])}

def Phone.sub(none: Bool, +text: String) -> List<&2, D.Node>:
  match none:
    case True{}: Nil{}
    case False{}: [D.Dom.span("ph-sub", text)]

# a title in words, with a smaller line under it when there is one
def Phone.title(+name: String, +sub: String) -> List<&2, D.Node>:
  D.Dom.span("ph-name", name) <> Phone.sub(String.is_empty(sub), sub)

def Phone.tool(+action: String, +value: String, +label: String, +icon: String) -> D.Node:
  D.El{"button", "", [D.At{"type", "button"}, D.At{"class", "ph-tool"}, D.At{"aria-label", label}, D.At{"title", label}, D.At{"data-value", value}, D.On{"click", action}],
    [WI.WIcon.svg(icon, 22)]}

# a sheet the hub draws (open while "@ph.sheet" names it); the backdrop and
# a drag down on its grab shut it. Menus (<details class="local-menu">) are
# shown as sheets by style.css alone and need none of this.
# (close: what the backdrop, a drag down and the system back send)
def Phone.sheet.on(+name: String, +title: String, +close: String, body: List<&2, D.Node>) -> D.Node:
  D.El{"div", "ph-sheet:" ++ name, [D.At{"class", "ph-sheet-layer"}], [
    D.El{"button", "", [D.At{"type", "button"}, D.At{"class", "ph-sheet-back"}, D.At{"aria-label", "Close"}, D.At{"data-value", ""}, D.On{"click", close}], Nil{}},
    D.El{"section", "", [D.At{"class", "ph-sheet"}, D.At{"role", "dialog"}, D.At{"aria-modal", "true"}, D.At{"aria-label", title}],
      List.append(&2, D.Node, [D.El{"div", "", [D.At{"class", "ph-sheet-grab"}], Nil{}}, D.El{"header", "", [D.At{"class", "ph-sheet-title"}], [D.Txt{title}]}], body)}]}

def Phone.sheet(+name: String, +title: String, body: List<&2, D.Node>) -> D.Node:
  Phone.sheet.on(name, title, "ph-sheet", body)

def Phone.sheet.if(open: Bool, +name: String, +title: String, body: List<&2, D.Node>) -> List<&2, D.Node>:
  match open:
    case True{}: [Phone.sheet(name, title, body)]
    case False{}: Nil{}

# a list row's gestures for phone-shell.js: lead/trail are JSON lists of
# {label, action, value, tone} shown under a swipe right/left, long is
# "action\u{1f}value" sent on a long press
def Phone.row(n: D.Node, +lead: String, +trail: String, +long: String) -> D.Node:
  match n:
    case D.El{tag, key, attrs, kids}:
      D.El{tag, key, List.append(&2, D.Attr, attrs, [D.At{"data-ph-lead", lead}, D.At{"data-ph-trail", trail}, D.At{"data-ph-long", long}]), kids}
    case D.Txt{t}: D.Txt{t}

# a screen with nothing in it yet: what it is for and what to do
def Phone.empty(+icon: String, +title: String, +text: String, actions: List<&2, D.Node>) -> D.Node:
  D.El{"div", "ph-empty", [D.At{"class", "ph-empty"}], [WI.WIcon.svg(icon, 40), D.Dom.span("ph-empty-title", title), D.Dom.span("ph-empty-text", text),
    D.El{"div", "", [D.At{"class", "ph-empty-actions"}], actions}]}

# a row of chips that scrolls sideways (a viewer's views, a bot's tabs)
def Phone.chips(kids: List<&2, D.Node>) -> D.Node:
  D.El{"div", "", [D.At{"class", "ph-chips"}], kids}
