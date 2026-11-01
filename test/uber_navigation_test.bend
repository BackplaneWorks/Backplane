import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/client.bend as U
import ../src/core/bot.bend as BT
import ../src/core/botui.bend as B
import ../src/core/i18n.bend as I
import ../src/core/json.bend as J
import ../src/web/bots.bend as W
import ../src/web/dom.bend as D
import ../src/mobile/bots.bend as MB

# Exercise actual shared selection, web buttons and phone projection. No hub,
# providers, live data or actions are started by this fixture.
def ui(+route: String) -> U.Ui:
  U.Ui.scratch_set(U.Ui.info_set(U.Ui.far.st(U.Ui.init(100n), M.State.replay([
    M.ThreadCreated{"t", "", "Kit", M.Local{}, "", "", "claude", "", 1n},
    M.BotSet{"b", "Kit", "t", 0n, "hello", 1n}, M.UberSet{"b", "u", 2n},
    M.MemorySaved{"b", "tea", "pref", "green", "", 3n}, M.HookSet{"h", "b", "hook", False{}, 3n}], M.State.empty())), "local.mailroom", "on"), "@btab", route)

def bot(+u: U.Ui) -> BT.Bot:
  BT.World.bot(M.State.bots(U.Ui.state(u)), "b")

def head(+u: U.Ui) -> B.Head:
  B.BU.head.bot(u, bot(u))

def strs(xs: List<&2, String>) -> String:
  match xs:
    case Nil{}: ""
    case x <> t: x ++ ";" ++ strs(t)

def attr(xs: List<&2, D.Attr>, +key: String) -> String:
  match xs:
    case Nil{}: ""
    case D.At{+k, v} <> t: Bool.pick(String, String.eq(k, key), v, attr(t, key))
    case _ <> t: attr(t, key)

def buttons(xs: List<&2, D.Node>, +selected: Bool) -> String:
  match xs:
    case Nil{}: ""
    case D.El{_, _, +ats, _} <> t:
      Bool.pick(String, Bool.or(Bool.not(selected), String.eq(attr(ats, "class"), "tab on")), attr(ats, "data-value") ++ ";", "") ++ buttons(t, selected)
    case _ <> t: buttons(t, selected)

# the bar's buttons sit in its "bot-desktop-tabs" div (WB.tabs)
def nav.in(xs: List<&2, D.Node>, on: Bool) -> String:
  match xs:
    case D.El{_, _, _, ys} <> _: buttons(ys, on)
    case _: ""

def nav(n: D.Node, on: Bool) -> String:
  match n:
    case D.El{_, _, _, xs}: nav.in(xs, on)
    case _: ""

def phone(+route: String) -> J.Json:
  MB.MBots.bot(I.En{}, ui(route), bot(ui(route)))

def ids(xs: List<&2, J.Json>) -> String:
  match xs:
    case Nil{}: ""
    case x <> t: J.Json.get_str(x, "id", "") ++ ";" ++ ids(t)

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("exact Uberbot bar", strs(B.BU.tabs.for(ui("chat"), "b")), "chat;mail;space;browser;memory;routines;settings;")
    C.Check.str("no tab picked opens Chat (the person's choice)", B.BU.tab.of(ui(""), "b"), "chat")
    C.Check.str("selected Status route retained", B.BU.tab.of(ui("status"), "b"), "status")
    C.Check.str("selected Tasks route retained", B.BU.tab.of(ui("tasks"), "b"), "tasks")
    C.Check.str("selected Webhooks route retained", B.BU.tab.of(ui("hooks"), "b"), "hooks")
    C.Check.str("unknown selection has sensible default", B.BU.tab.of(ui("unknown"), "b"), "chat")
    C.Check.str("Status under Mailroom", strs(B.BU.nav.sections(B.BU.tabs.for(ui("status"), "b"), "status")), "mail;status;")
    C.Check.str("Tasks under Browser", strs(B.BU.nav.sections(B.BU.tabs.for(ui("tasks"), "b"), "tasks")), "browser;tasks;")
    C.Check.str("Webhooks under Settings", strs(B.BU.nav.sections(B.BU.tabs.for(ui("hooks"), "b"), "hooks")), "settings;hooks;")
    C.Check.str("remote tabs unchanged", strs(B.BU.tabs.for(ui("chat"), "p~b")), "chat;space;memory;routines;")
    C.Check.str("regular bots keep their existing tabs", strs(B.BU.tabs.for(ui("chat"), "other")), "chat;mail;space;browser;tasks;memory;routines;hooks;settings;")
    C.Check.str("web top bar exact order", nav(W.WB.tabs(I.En{}, head(ui("status"))), False{}), "chat;mail;space;browser;memory;routines;settings;")
    C.Check.str("web Status highlights Mailroom", nav(W.WB.tabs(I.En{}, head(ui("status"))), True{}), "mail;")
    C.Check.str("web Tasks highlights Browser", nav(W.WB.tabs(I.En{}, head(ui("tasks"))), True{}), "browser;")
    C.Check.str("web Webhooks highlights Settings", nav(W.WB.tabs(I.En{}, head(ui("hooks"))), True{}), "settings;")
    C.Check.str("phone route remains Tasks", J.Json.get_str(phone("tasks"), "tab", ""), "tasks")
    C.Check.str("phone parent Browser", J.Json.get_str(phone("tasks"), "navTab", ""), "browser")
    C.Check.str("phone same Browser sections", ids(J.Json.elems(J.Json.get_obj(phone("tasks"), "subtabs"))), "browser;tasks;")
    C.Check.str("phone Status parent", J.Json.get_str(phone("status"), "navTab", ""), "mail")
    C.Check.str("phone old hooks body retained", ids(J.Json.elems(J.Json.get_obj(phone("hooks"), "hooks"))), "h;")
    C.Check.str("phone old settings body retained", J.Json.get_str(J.Json.get_obj(phone("settings"), "settings"), "persona", ""), "hello")
    C.Check.str("phone Memory body retained", J.Json.get_str(Maybe.default(&2, J.Json, List.head(&2, J.Json, J.Json.elems(J.Json.get_obj(phone("memory"), "memory"))), J.Null{}), "text", ""), "green")

# Exports exercised by the emitted-JS driver; emissions never count as a test.
def panel(+route: String) -> D.Node:
  W.WB.main(I.En{}, ui(route), bot(ui(route)), route)

def clicked(a: U.Act) -> String:
  match a:
    case U.Act{u, _}: B.BU.tab.of(u, "b")

def click(route: String) -> String:
  clicked(U.Ui.act(ui("chat"), "bot-tab", route))

def navbar(+route: String) -> D.Node:
  W.WB.tabs(I.En{}, head(ui(route)))

def tabsof(id: String) -> List<&2, String>:
  B.BU.tabs.for(ui("chat"), id)
