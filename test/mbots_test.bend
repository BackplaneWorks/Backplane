import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/codec.bend as K
import ../src/core/json.bend as J
import ../src/core/client.bend as U
import ../src/mobile/hubs.bend as H
import ../src/mobile/app.bend as A

# Bots on the phones: the cats and rooms of every hub in the list, a bot's
# view and tabs from the hub in focus, and actions routed to it.

def items(cs: List<&2, M.Change>) -> List<&2, J.Json>:
  match cs:
    case Nil{}:
      Nil{}
    case c <> t:
      K.Change.encode(c) <> items(t)

def log(cs: List<&2, M.Change>) -> J.Json:
  J.Json.obj([J.KV{"t", J.Json.str("log")}, J.KV{"items", J.Json.arr(items(cs))}])

def spec() -> String:
  "{\"title\":\"Desk\",\"blocks\":[{\"type\":\"stat\",\"label\":\"Mail\",\"value\":\"3\"},{\"type\":\"button\",\"label\":\"Go\",\"action\":\"go\"}]}"

def changes(cs: List<&2, M.Change>) -> J.Json:
  J.Json.obj([J.KV{"t", J.Json.str("changes")}, J.KV{"items", J.Json.arr(items(cs))}])

def two() -> H.Hubs:
  H.Hubs.after(H.Hubs.recv(
    H.Hubs.after(H.Hubs.recv(H.Hubs.set(H.Hubs.init(0n, "c"), ["lap:3787", "box:3787"]), "lap:3787",
      log([M.ProjectCreated{"p1", "Lap", "/lap", 50n}]))),
    "box:3787",
    log([M.ThreadCreated{"bt1", "", "Kit", M.Local{}, "", "", "claude", "", 100n},
      M.BotSet{"b1", "Kit", "bt1", 7n, "helps", 100n},
      M.RoomSet{"r1", "den", "b1", 110n},
      M.RoomPosted{"x1", "r1", "you", "hello", 0n, 120n},
      M.MemorySaved{"b1", "tea", "pref", "green", "", 130n},
      M.RoutineSet{"o1", "b1", "brief", "0 9 * * *", "news", True{}, 140n},
      M.HookSet{"h1", "b1", "gh", False{}, 150n},
      M.SpaceSet{"b1", spec(), 160n}])))

def strs(js: List<&2, J.Json>, +key: String) -> String:
  match js:
    case Nil{}:
      ""
    case j <> t:
      J.Json.get_str(j, key, "") ++ ";" ++ strs(t, key)

def list(+h: H.Hubs, +key: String, +field: String) -> String:
  strs(J.Json.elems(J.Json.get_obj(H.Hubs.screen(h), key)), field)

def bot(+h: H.Hubs) -> J.Json:
  J.Json.get_obj(H.Hubs.screen(h), "bot")

def go(+h: H.Hubs, +a: String, +v: String) -> H.Hubs:
  H.Hubs.after(H.Hubs.act(h, a, v))

def shown() -> H.Hubs:
  go(two(), "bot", "box:3787|b1")

def tab(+t: String) -> J.Json:
  bot(go(shown(), "bot-tab", t))

# the bot shown, after it wrote a page
def paged() -> H.Hubs:
  go(H.Hubs.after(H.Hubs.recv(go(shown(), "bot-tab", "space"), "box:3787", changes([M.SpacePageSet{"b1", "<h1>Desk</h1>", 170n}]))), "bot-tab", "space")

def page() -> J.Json:
  J.Json.get_obj(bot(paged()), "page")

def cmds(cs: List<&2, H.HCmd>) -> String:
  match cs:
    case Nil{}:
      ""
    case H.HCmd{k, U.Send{_}} <> t:
      "send@" ++ k ++ " " ++ cmds(t)
    case H.HCmd{k, _} <> t:
      "cmd@" ++ k ++ " " ++ cmds(t)

def shot() -> J.Json:
  J.Json.obj([J.KV{"t", J.Json.str("shot")}, J.KV{"bot", J.Json.str("b1")}, J.KV{"n", J.Json.nat(4n)}])

# box also lists a bot on a linked machine
def far() -> H.Hubs:
  H.Hubs.after(A.recv(two(), "box:3787", J.Json.obj([J.KV{"t", J.Json.str("info")},
    J.KV{"info", J.Json.obj([J.KV{"bots.remote", J.Json.str("[{\"name\":\"Nori\",\"peer\":\"p1\",\"peerName\":\"laptop\",\"look\":3,\"state\":\"ready\"}]")}])}])))

def picks(+h: H.Hubs) -> List<&2, J.Json>:
  J.Json.elems(J.Json.get_obj(J.Json.get_obj(H.Hubs.screen(h), "newRoom"), "picks"))

def ons(js: List<&2, J.Json>) -> List<&2, J.Json>:
  match js:
    case Nil{}:
      Nil{}
    case j <> t:
      J.Json.get_obj(j, "on") <> ons(t)

# a bot that learned a task, ran it, and is paused on a risky step
def tk_task() -> String:
  "{\"v\":1,\"slug\":\"jlcpcb\",\"name\":\"Check order\",\"description\":\"Look up an order\",\"origin\":\"show\",\"start\":\"https://jlcpcb.com\",\"params\":[{\"name\":\"order\",\"label\":\"Order number\",\"kind\":\"text\",\"example\":\"\"}],\"steps\":[],\"urls\":[],\"created\":1,\"updated\":2}"

def tk_run() -> String:
  "{\"slug\":\"jlcpcb\",\"run\":\"r1\",\"status\":\"ok\",\"n\":3,\"summary\":\"done\",\"shots\":[\"/bots/b1/tasks/shot?run=r1&n=2\"]}"

def tk_ask() -> String:
  "{\"bot\":\"b1\",\"run\":\"r1\",\"slug\":\"jlcpcb\",\"n\":3,\"kind\":\"purchase\",\"question\":\"Place the order?\",\"risky\":true,\"shot\":\"/bots/b1/tasks/shot?run=r1&n=3\"}"

def taught() -> H.Hubs:
  H.Hubs.after(A.recv(shown(), "box:3787", changes([M.TaskSet{"b1", "jlcpcb", tk_task(), 200n}, M.ActivityLogged{"bt1", "r1:b", "task", "run", tk_run(), 310n},
    M.AskOpened{"k1", "bt1", "browser", "task_pause", tk_ask(), "", 400n}])))

def drives(+h: H.Hubs, +drive: String, +teach: String) -> H.Hubs:
  H.Hubs.after(A.recv(h, "box:3787", J.Json.obj([J.KV{"t", J.Json.str("info")},
    J.KV{"info", J.Json.obj([J.KV{"drive.b1", J.Json.str(drive)}, J.KV{"teach.b1", J.Json.str(teach)}])}])))

def tasks_tab() -> J.Json:
  J.Json.get_obj(bot(go(taught(), "bot-tab", "tasks")), "tasks")

def br(+h: H.Hubs) -> J.Json:
  J.Json.get_obj(bot(go(h, "bot-tab", "browser")), "browser")

def first(js: List<&2, J.Json>) -> J.Json:
  match js:
    case Nil{}:
      J.Null{}
    case j <> _:
      j

def btn_labels(+h: H.Hubs) -> String:
  strs(J.Json.elems(J.Json.get_obj(br(h), "btns")), "label")

def thread_asks(+h: H.Hubs) -> J.Json:
  J.Json.get_obj(J.Json.get_obj(H.Hubs.screen(h), "thread"), "asks")

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("cats of every hub, ids qualified", list(two(), "bots", "id"), "box:3787|b1;")
    C.Check.str("cats named by machine", list(two(), "bots", "machine"), "box;")
    C.Check.str("a cat's mood", list(two(), "bots", "note"), "ready;")
    C.Check.str("a cat's key", list(two(), "bots", "cat"), "7:idle;")
    C.Check.str("a rig for it", J.Json.get_str(Maybe.default(&2, J.Json, List.head(&2, J.Json, J.Json.elems(Maybe.default(&2, J.Json, J.Json.parse(A.cat(7n, "idle")), J.Null{}))), J.Null{}), "name", "") ++ "", "tail")
    C.Check.str("rooms listed", list(two(), "rooms", "id"), "box:3787|r1;")
    C.Check.str("no bot shown", J.Json.show(bot(two())), "null")
    C.Check.str("a bot moves the focus", H.Hubs.cur(shown()), "box:3787")
    C.Check.str("the bot shown", J.Json.get_str(bot(shown()), "name", ""), "Kit")
    C.Check.str("the bot header keeps its owning machine", J.Json.get_str(bot(shown()), "title", ""), "Kit · box")
    C.Check.str("its id stays its hub's", J.Json.get_str(bot(shown()), "id", ""), "b1")
    C.Check.str("chat first, as the thread", J.Json.get_str(bot(shown()), "tab", ""), "chat")
    C.Check.str("its thread is the chat, with its owning machine", J.Json.get_str(J.Json.get_obj(H.Hubs.screen(shown()), "thread"), "title", ""), "Kit · box")
    C.Check.str("tabs", strs(J.Json.elems(J.Json.get_obj(bot(shown()), "tabs")), "label"), "Chat;Space;Browser;Tasks;Memory;Routines;Webhooks;Settings;")
    C.Check.str("no tab content on chat", J.Json.show(J.Json.get_obj(bot(shown()), "memory")), "null")
    C.Check.str("memory", strs(J.Json.elems(J.Json.get_obj(tab("memory"), "memory")), "text"), "green;")
    C.Check.str("routines", strs(J.Json.elems(J.Json.get_obj(tab("routines"), "routines")), "id"), "o1;")
    C.Check.str("hooks", strs(J.Json.elems(J.Json.get_obj(tab("hooks"), "hooks")), "path"), "/hook/h1;")
    C.Check.str("settings", J.Json.get_str(J.Json.get_obj(tab("settings"), "settings"), "persona", ""), "helps")
    C.Check.str("space values untouched", strs(J.Json.elems(J.Json.get_obj(J.Json.get_obj(tab("space"), "space"), "blocks")), "value"), "3;;")
    C.Check.str("space sends", strs(J.Json.elems(J.Json.get_obj(J.Json.get_obj(tab("space"), "space"), "blocks")), "send"), ";b1\u{1f}go;")
    C.Check.str("no page, blocks", J.Json.show(J.Json.get_obj(tab("space"), "page")), "null")
    C.Check.str("a page", J.Json.get_str(page(), "url", "") ++ " " ++ J.Json.get_str(page(), "n", "") ++ " " ++ J.Json.get_str(page(), "bot", ""), "/bots/b1/space.html 170 b1")
    C.Check.str("a page, no blocks", J.Json.show(J.Json.get_obj(bot(paged()), "space")), "null")
    C.Check.str("a space link goes to the hub in focus", cmds(H.Hubs.cmds(H.Hubs.act(paged(), "space-link", "b1\u{1f}space:go"))), "send@box:3787 ")
    C.Check.str("a web link sends nothing", cmds(H.Hubs.cmds(H.Hubs.act(paged(), "space-link", "b1\u{1f}https://example.com"))), "")
    C.Check.str("browser url", J.Json.get_str(J.Json.get_obj(tab("browser"), "browser"), "url", ""), "/bots/b1/screen.jpg")
    C.Check.str("no frame yet", J.Json.get_str(J.Json.get_obj(tab("browser"), "browser"), "n", ""), "0")
    C.Check.str("a new frame", J.Json.get_str(J.Json.get_obj(bot(H.Hubs.after(A.recv(go(shown(), "bot-tab", "browser"), "box:3787", shot()))), "browser"), "n", ""), "4")
    C.Check.str("a routine's action goes to the hub in focus", cmds(H.Hubs.cmds(H.Hubs.act(go(shown(), "bot-tab", "routines"), "routine-run", "o1"))), "send@box:3787 ")
    C.Check.str("a room", J.Json.get_str(bot(go(two(), "room", "box:3787|r1")), "members", ""), "Kit")
    C.Check.str("its posts", strs(J.Json.elems(J.Json.get_obj(bot(go(two(), "room", "box:3787|r1")), "posts")), "from"), "you;")
    C.Check.str("back to the list", J.Json.show(bot(go(go(two(), "room", "box:3787|r1"), "select", ""))), "null")
    C.Check.str("the new bot form on its hub", H.Hubs.cur(go(two(), "bot-new", "box:3787|")), "box:3787")
    C.Check.str("its providers", strs(J.Json.elems(J.Json.get_obj(J.Json.get_obj(H.Hubs.screen(go(two(), "bot-new", "box:3787|")), "newBot"), "providers")), "provider"), "claude;codex;grok;muse;antigravity;omp;")
    C.Check.str("a field typed", J.Json.get_str(J.Json.get_obj(H.Hubs.screen(go(go(two(), "bot-new", "box:3787|"), "bfield", "name\u{1f}Tom")), "newBot"), "name", ""), "Tom")
    C.Check.str("a form closed", J.Json.show(J.Json.get_obj(H.Hubs.screen(go(go(two(), "bot-new", "box:3787|"), "form-close", "@bnew")), "newBot")), "null")
    C.Check.str("the routine form opens blank", J.Json.show(J.Json.get_obj(J.Json.get_obj(bot(go(go(shown(), "bot-tab", "routines"), "routine-edit", "")), "routine"), "open")), "true")
    C.Check.str("and closes", J.Json.show(J.Json.get_obj(J.Json.get_obj(bot(go(go(go(shown(), "bot-tab", "routines"), "routine-edit", ""), "form-close", "@onew")), "routine"), "open")), "false")
    C.Check.str("the room form's picks", strs(J.Json.elems(J.Json.get_obj(J.Json.get_obj(H.Hubs.screen(go(two(), "room-new", "box:3787|")), "newRoom"), "picks")), "id"), "b1;")
    C.Check.str("bots elsewhere can be picked", strs(picks(go(far(), "room-new", "box:3787|")), "id"), "b1;Nori@laptop;")
    C.Check.str("named with their machine", strs(picks(go(far(), "room-new", "box:3787|")), "name"), "Kit;Nori@laptop;")
    C.Check.str("picked on", J.Json.show(J.Json.arr(ons(picks(go(go(far(), "room-new", "box:3787|"), "room-pick", "Nori@laptop"))))), "[false,true]")
    C.Check.str("a member no machine lists stays", strs(picks(go(go(two(), "room-new", "box:3787|"), "bfield", "rmembers\u{1f}Ghost@far")), "id"), "b1;Ghost@far;")
    C.Check.str("a task listed", strs(J.Json.elems(J.Json.get_obj(tasks_tab(), "items")), "name"), "Check order;")
    C.Check.str("its param is a run field", strs(J.Json.elems(J.Json.get_obj(first(J.Json.elems(J.Json.get_obj(tasks_tab(), "items"))), "params")), "field"), "tkrun.order;")
    C.Check.str("its last run", strs(J.Json.elems(J.Json.get_obj(tasks_tab(), "items")), "last"), "ok;")
    C.Check.str("a run with its picture", J.Json.show(J.Json.get_obj(first(J.Json.elems(J.Json.get_obj(tasks_tab(), "runs"))), "shots")), "[\"/bots/b1/tasks/shot?run=r1&n=2\"]")
    C.Check.str("opening a task", J.Json.show(J.Json.get_obj(first(J.Json.elems(J.Json.get_obj(J.Json.get_obj(bot(go(go(taught(), "bot-tab", "tasks"), "tk-open", "jlcpcb")), "tasks"), "items"))), "open")), "true")
    C.Check.str("idle strip", btn_labels(taught()), "Teach a task;Take over;")
    C.Check.str("an idle frame takes no input", J.Json.show(J.Json.get_obj(br(taught()), "input")), "false")
    C.Check.str("recording", btn_labels(drives(taught(), "teach", "rec 1000 2")), "Stop & save;Discard;")
    C.Check.str("the person has it", btn_labels(drives(taught(), "takeover", "")), "Hand back;")
    C.Check.str("and the frame takes input", J.Json.show(J.Json.get_obj(br(drives(taught(), "takeover", "")), "input")), "true")
    C.Check.str("a paused run is a browser ask", strs(J.Json.elems(thread_asks(taught())), "kind"), "browser;")
    C.Check.str("with its picture", strs(J.Json.elems(thread_asks(taught())), "image"), "/bots/b1/tasks/shot?run=r1&n=3;")
    C.Check.str("a risky step is confirmed, never continued", strs(J.Json.elems(J.Json.get_obj(first(J.Json.elems(thread_asks(taught()))), "buttons")), "label"), "Confirm;Skip this step;Stop task;Take over;")
    C.Check.str("answers go by the ask's id", strs(J.Json.elems(J.Json.get_obj(first(J.Json.elems(thread_asks(taught()))), "buttons")), "value"), "k1|confirm;k1|skip;k1|stop;;")
    C.Check.str("the Browser tab carries the same card", strs(J.Json.elems(J.Json.get_obj(br(taught()), "asks")), "id"), "k1;")
    C.Check.str("take over is its own action", strs(J.Json.elems(J.Json.get_obj(first(J.Json.elems(thread_asks(taught()))), "buttons")), "action"), "answer;answer;answer;tk-takeover;")
    C.Check.str("no ask, no card", J.Json.show(J.Json.get_obj(br(shown()), "asks")), "[]")
