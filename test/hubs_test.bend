import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/codec.bend as K
import ../src/core/json.bend as J
import ../src/core/client.bend as U
import ../src/core/notice.bend as NO
import ../src/mobile/hubs.bend as H
import ../src/mobile/app.bend as A
import ../src/mobile/screen.bend as S

# The phone with several hubs: one client each, one screen, actions
# routed by the hub their value names.

def items(cs: List<&2, M.Change>) -> List<&2, J.Json>:
  match cs:
    case Nil{}:
      Nil{}
    case c <> t:
      K.Change.encode(c) <> items(t)

def machines(+name: String) -> String:
  "{\"self\":{\"name\":\"" ++ name ++ "\",\"url\":\"http://x\"},\"peers\":[{\"name\":\"desk\",\"url\":\"http://100.2.2.2:3787\"},{\"name\":\"lap\",\"url\":\"http://100.1.1.1:3787\"},{\"name\":\"desk\",\"url\":\"http://100.2.2.2:3787\"}]}"

def log(cs: List<&2, M.Change>, +name: String) -> J.Json:
  J.Json.obj([J.KV{"t", J.Json.str("log")}, J.KV{"items", J.Json.arr(items(cs))},
    J.KV{"info", J.Json.obj([J.KV{"machines", J.Json.str(machines(name))}])}])

def two() -> H.Hubs:
  H.Hubs.after(H.Hubs.recv(
    H.Hubs.after(H.Hubs.recv(H.Hubs.set(H.Hubs.init(0n, "c"), ["100.1.1.1:3787", "box:3787"]), "100.1.1.1:3787",
      log([M.ProjectCreated{"p1", "Lap", "/lap", 50n}, M.ThreadCreated{"t1", "p1", "One", M.Local{}, "", "", "claude", "", 100n}], "lap"))),
    "box:3787",
    log([M.ProjectCreated{"p2", "Box", "/box", 50n}, M.ThreadCreated{"t2", "p2", "Two", M.Local{}, "", "", "claude", "", 100n}], "")))

def strs(js: List<&2, J.Json>, +key: String) -> String:
  match js:
    case Nil{}:
      ""
    case j <> t:
      J.Json.get_str(j, key, "") ++ ";" ++ strs(t, key)

def screen(+h: H.Hubs, +key: String) -> String:
  J.Json.get_str(H.Hubs.screen(h), key, "")

def list(+h: H.Hubs, +key: String, +field: String) -> String:
  strs(J.Json.elems(J.Json.get_obj(H.Hubs.screen(h), key)), field)

def rows(+h: H.Hubs) -> String:
  strs(J.Json.elems(J.Json.get_obj(Maybe.default(&2, J.Json, List.head(&2, J.Json, J.Json.elems(J.Json.get_obj(H.Hubs.screen(h), "projects"))), J.Null{}), "threads")), "id")

def cmds(cs: List<&2, H.HCmd>) -> String:
  match cs:
    case Nil{}:
      ""
    case H.HCmd{k, U.Send{_}} <> t:
      "send@" ++ k ++ " " ++ cmds(t)
    case H.HCmd{k, _} <> t:
      "cmd@" ++ k ++ " " ++ cmds(t)

# what the commands send and copy, each with its hub
def said(cs: List<&2, H.HCmd>) -> String:
  match cs:
    case Nil{}:
      ""
    case H.HCmd{k, U.Send{t}} <> r:
      k ++ " " ++ t ++ " " ++ said(r)
    case H.HCmd{k, U.Copy{t}} <> r:
      k ++ " copy " ++ t ++ " " ++ said(r)
    case _ <> r:
      said(r)

def sel(+h: H.Hubs, +k: String) -> String:
  U.Ui.sel(H.Hubs.ui(h, k))

def yes(b: Bool) -> String:
  Bool.pick(String, b, "yes", "no")

def act(+h: H.Hubs, +a: String, +v: String) -> H.Hubs:
  H.Hubs.after(H.Hubs.act(h, a, v))

def searching() -> H.Hubs:
  act(act(two(), "proj-find", "on"), "proj-find-q", "bo")

def obj(+h: H.Hubs, +key: String, +field: String) -> String:
  J.Json.get_str(J.Json.get_obj(H.Hubs.screen(h), key), field, "")

def changes(cs: List<&2, M.Change>) -> J.Json:
  J.Json.obj([J.KV{"t", J.Json.str("changes")}, J.KV{"items", J.Json.arr(items(cs))}])

def delta(+th: String) -> J.Json:
  J.Json.obj([J.KV{"t", J.Json.str("delta")}, J.KV{"thread", J.Json.str(th)}, J.KV{"text", J.Json.str("tok")}])

# a turn's end as the hub writes it: with the thread's alert record (the
# phone alerts for the record, not the turn: core/notice.bend)
def settled(+th: String) -> List<&2, M.Change>:
  [M.TurnChanged{th, M.Completed{}, 300n}, M.SettingSet{NO.Alert.key(th), NO.Alert.line("done", "turn-" ++ th, "m1", "300.9", ""), 300n}]

def alerts(a: H.HAct) -> Nat:
  match a:
    case H.HAct{_, _, als}:
      List.length(&2, J.Json, als)

# a chunk of a join: a log saying more, then changes, the last boot false
def chunk(+t: String, +more: Bool, +boot: Bool, cs: List<&2, M.Change>) -> J.Json:
  J.Json.obj([J.KV{"t", J.Json.str(t)}, J.KV{"more", J.Json.bool(more)}, J.KV{"boot", J.Json.bool(boot)}, J.KV{"items", J.Json.arr(items(cs))}])

# box joins in chunks: its first chunk has t2 at work, the next its end
def split() -> H.Hubs:
  H.Hubs.after(H.Hubs.recv(H.Hubs.set(H.Hubs.init(0n, "c"), ["box:3787"]), "box:3787",
    chunk("log", True{}, True{}, [M.ProjectCreated{"p2", "Box", "/box", 50n}, M.ThreadCreated{"t2", "p2", "Two", M.Local{}, "", "", "claude", "", 100n}, M.TurnChanged{"t2", M.Running{}, 200n}])))

# t2 on the hub out of focus is at work
def busy() -> H.Hubs:
  H.Hubs.after(H.Hubs.recv(two(), "box:3787", changes([M.TurnChanged{"t2", M.Running{}, 200n}])))

# how many threads the merged island shows
def island(+h: H.Hubs) -> String:
  Nat.show(J.Json.get_nat(J.Json.get_obj(H.Hubs.screen(h), "island"), "running", 0n))

def tick(h: H.Hubs, +now: Nat) -> H.Hubs:
  H.Hubs.after(H.Hubs.tick(h, now))

# busy() heard at 1000, then box gone quiet
def unheard(+now: Nat) -> H.Hubs:
  tick(H.Hubs.after(H.Hubs.online(tick(busy(), 1000n), "box:3787", False{})), now)

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("first hub is current", H.Hubs.cur(two()), "100.1.1.1:3787")
    # a log arriving in chunks is history: no alert from a chunk or the
    # last one; after it, a turn's end alerts again
    C.Check.str("a split join's chunk raises no alert", Nat.show(alerts(H.Hubs.recv(split(), "box:3787", chunk("changes", False{}, True{}, [M.TurnChanged{"t2", M.Completed{}, 300n}])))), "0")
    C.Check.str("a catch-up chunk of a live client raises no alert", Nat.show(alerts(H.Hubs.recv(busy(), "box:3787", chunk("changes", False{}, True{}, [M.TurnChanged{"t2", M.Completed{}, 300n}])))), "0")
    C.Check.str("a split join's last chunk raises no alert", Nat.show(alerts(H.Hubs.recv(split(), "box:3787", chunk("changes", False{}, False{}, [M.TurnChanged{"t2", M.Completed{}, 300n}])))), "0")
    C.Check.str("after a split join a settled turn alerts", Nat.show(alerts(H.Hubs.recv(H.Hubs.after(H.Hubs.recv(split(), "box:3787", chunk("changes", False{}, False{}, Nil{}))), "box:3787", changes(settled("t2"))))), "1")
    C.Check.str("a split join holds what one log holds", Nat.show(U.Ui.seq(H.Hubs.ui(H.Hubs.after(H.Hubs.recv(split(), "box:3787", chunk("changes", False{}, False{}, [M.TurnChanged{"t2", M.Completed{}, 300n}]))), "box:3787"))), "4")
    # every paired hub's messages redraw the merged screen; streamed text
    # only for the thread open on the hub in focus
    C.Check.str("an unfocused completion redraws", yes(A.shows(busy(), "box:3787", changes([M.TurnChanged{"t2", M.Completed{}, 300n}]))), "yes")
    C.Check.str("an unfocused approval redraws", yes(A.shows(busy(), "box:3787", changes([M.AskOpened{"a1", "t2", "approval", "Bash", "ls", "", 300n}]))), "yes")
    C.Check.str("unfocused streamed text does not", yes(A.shows(busy(), "box:3787", delta("t2"))), "no")
    C.Check.str("an unfocused settled turn alerts", Nat.show(alerts(H.Hubs.recv(busy(), "box:3787", changes(settled("t2"))))), "1")
    C.Check.str("a turn's end alone does not", Nat.show(alerts(H.Hubs.recv(busy(), "box:3787", changes([M.TurnChanged{"t2", M.Completed{}, 300n}])))), "0")
    C.Check.str("the same record again does not", Nat.show(alerts(H.Hubs.recv(H.Hubs.after(H.Hubs.recv(busy(), "box:3787", changes(settled("t2")))), "box:3787", changes([M.SettingSet{NO.Alert.key("t2"), NO.Alert.line("done", "turn-t2", "m1", "300.9", ""), 301n}])))), "0")
    C.Check.str("streamed text alerts nothing", Nat.show(alerts(H.Hubs.recv(busy(), "box:3787", delta("t2")))), "0")
    C.Check.str("connecting raises no old alerts", Nat.show(alerts(H.Hubs.recv(H.Hubs.set(H.Hubs.init(0n, "c"), ["box:3787"]), "box:3787",
      log([M.ProjectCreated{"p2", "Box", "/box", 50n}, M.ThreadCreated{"t2", "p2", "Two", M.Local{}, "", "", "claude", "", 100n}, M.TurnChanged{"t2", M.Completed{}, 120n}], "")))), "0")
    # a hub the phone no longer reaches keeps its last state, but an
    # offline hub cannot claim current work: its running threads leave the
    # island at once (core/signal.bend's Signal.visible), not after
    # notify.bend's Note.fresh
    C.Check.str("a working hub's thread is on the island", island(tick(busy(), 1000n)), "1")
    C.Check.str("a hub just gone takes its threads off the island", island(unheard(1001n)), "0")
    C.Check.str("an offline hub does not advertise active work", island(unheard(11799n)), "0")
    C.Check.str("a hub quiet three hours leaves the island", island(unheard(11800n)), "0")
    C.Check.str("it comes back when the hub answers", island(H.Hubs.after(H.Hubs.online(unheard(50000n), "box:3787", True{}))), "1")
    C.Check.str("a hub never heard and away shows nothing", island(tick(H.Hubs.after(H.Hubs.online(busy(), "box:3787", False{})), 20000n)), "0")
    # a Live Activity per hub: each hub's island alone, named by its machine
    C.Check.str("an island per hub", list(busy(), "islands", "hub"), "100.1.1.1:3787;box:3787;")
    C.Check.str("a hub's island is its own", J.Json.show(J.Json.get_obj(Maybe.default(&2, J.Json, List.get(&2, J.Json, J.Json.elems(J.Json.get_obj(H.Hubs.screen(busy()), "islands")), 1n), J.Null{}), "island")),
      "{\"running\":1,\"headline\":\"1 running · box\",\"lines\":[]}")
    C.Check.str("an idle hub's island is empty", J.Json.show(J.Json.get_obj(Maybe.default(&2, J.Json, List.head(&2, J.Json, J.Json.elems(J.Json.get_obj(H.Hubs.screen(busy()), "islands"))), J.Null{}), "island")),
      "{\"running\":0,\"headline\":\"All agents idle · lap\",\"lines\":[]}")
    C.Check.str("an activity's token goes to its hub alone", cmds(H.Hubs.cmds(H.Hubs.register.at(two(), "box:3787", "ios", "tok", "activity", "", "", "b"))), "send@box:3787 ")
    C.Check.str("an alert token goes to every hub", cmds(H.Hubs.cmds(H.Hubs.register(two(), "ios", "tok", "alert", "", "", "b"))), "send@100.1.1.1:3787 send@box:3787 ")
    C.Check.str("an unfocused hub's screen has no thread", J.Json.show(J.Json.get_obj(S.Mob.screen.f(False{}, H.Hubs.ui(busy(), "box:3787")), "thread")), "null")
    # a typed draft builds no screen unless what the composer offers changes
    C.Check.str("nothing offered", A.offer(act(two(), "select", "box:3787|t2")), "")
    C.Check.str("plain draft offers nothing", A.offer(act(act(two(), "select", "box:3787|t2"), "draft", "hi")), "")
    C.Check.str("% offers projects", yes(String.starts_with(A.offer(act(act(two(), "select", "box:3787|t2"), "draft", "hi %")), "%")), "yes")
    # a socket that keeps failing says offline again: no screen for that
    C.Check.str("online again is the same", yes(A.online_same(two(), "box:3787", True{})), "yes")
    C.Check.str("offline is a change", yes(A.online_same(two(), "box:3787", False{})), "no")
    C.Check.str("offline again is the same", yes(A.online_same(H.Hubs.after(H.Hubs.online(two(), "box:3787", False{})), "box:3787", False{})), "yes")
    C.Check.str("projects of both", list(two(), "projects", "id"), "100.1.1.1:3787|p1;box:3787|p2;")
    C.Check.str("one list: other hub keeps its machine marker", list(two(), "projects", "machine"), ";box;")
    C.Check.str("thread ids qualified", rows(two()), "100.1.1.1:3787|t1;")
    C.Check.str("hubs listed", list(two(), "hubs", "name"), "lap;box;")
    C.Check.str("nothing selected", screen(two(), "sel"), "")
    C.Check.str("peers not paired, once each", list(two(), "found", "url"), "http://100.2.2.2:3787;")
    C.Check.str("select moves to its hub", H.Hubs.cur(H.Hubs.after(H.Hubs.act(two(), "select", "box:3787|t2"))), "box:3787")
    C.Check.str("selected there", sel(H.Hubs.after(H.Hubs.act(two(), "select", "box:3787|t2")), "box:3787"), "t2")
    C.Check.str("screen sel qualified", screen(H.Hubs.after(H.Hubs.act(two(), "select", "box:3787|t2")), "sel"), "box:3787|t2")
    C.Check.str("thread shown with its machine", J.Json.get_str(J.Json.get_obj(H.Hubs.screen(H.Hubs.after(H.Hubs.act(two(), "select", "box:3787|t2"))), "thread"), "title", ""), "Two · box")
    C.Check.str("offline selected owner stays named", J.Json.get_str(J.Json.get_obj(H.Hubs.screen(H.Hubs.after(H.Hubs.online(H.Hubs.after(H.Hubs.act(two(), "select", "box:3787|t2")), "box:3787", False{}))), "thread"), "title", ""), "Two · box (away)")
    C.Check.str("single hub title stays plain", J.Json.get_str(H.Hubs.origin.row(False{}, J.Json.obj([J.KV{"title", J.Str{"One"}}]), "lap", False{}), "title", ""), "One")
    C.Check.str("active direct owner named", J.Json.get_str(H.Hubs.origin.row(True{}, J.Json.obj([J.KV{"id", J.Str{"box:3787|t2"}}, J.KV{"title", J.Str{"Two"}}]), "box", True{}), "title", ""), "Two · box")
    C.Check.str("active owner id stays routable", J.Json.get_str(H.Hubs.origin.row(True{}, J.Json.obj([J.KV{"id", J.Str{"box:3787|t2"}}, J.KV{"title", J.Str{"Two"}}]), "box", False{}), "id", ""), "box:3787|t2")
    C.Check.str("mirror title never names its relay", J.Json.get_str(H.Hubs.origin.row(True{}, J.Json.obj([J.KV{"id", J.Str{"lap:3787|p1~t2"}}, J.KV{"title", J.Str{"Two · box"}}]), "lap", True{}), "title", ""), "Two · box")
    C.Check.str("reported-only title stays with owner", J.Json.get_str(H.Hubs.origin.row(True{}, J.Json.obj([J.KV{"kind", J.Str{"remote"}}, J.KV{"title", J.Str{"Kit · box"}}]), "lap", True{}), "title", ""), "Kit · box")
    C.Check.str("the hub left drops its thread",
      sel(H.Hubs.after(H.Hubs.act(H.Hubs.after(H.Hubs.act(two(), "select", "100.1.1.1:3787|t1")), "select", "box:3787|t2")), "100.1.1.1:3787"), "")
    C.Check.str("a thread's action goes to the current hub",
      cmds(H.Hubs.cmds(H.Hubs.act(H.Hubs.after(H.Hubs.act(two(), "select", "box:3787|t2")), "pin", ""))), "send@box:3787 ")
    C.Check.str("a row's action goes to its hub", cmds(H.Hubs.cmds(H.Hubs.act(two(), "row-settle", "box:3787|t2"))), "send@box:3787 ")
    C.Check.str("the Ready row's × goes to its thread's hub", cmds(H.Hubs.cmds(H.Hubs.act(two(), "fab-hide", "box:3787|t2"))), "send@box:3787 ")
    C.Check.str("and puts the bar away for its project there",
      M.Setting.get(U.Ui.state.of(H.Hubs.ui(H.Hubs.after(H.Hubs.act(two(), "fab-hide", "box:3787|t2")), "box:3787")), "fab.min.p2", ""), "1")
    C.Check.str("the menu's Buy it brings it back",
      M.Setting.get(U.Ui.state.of(H.Hubs.ui(H.Hubs.after(H.Hubs.act(H.Hubs.after(H.Hubs.act(two(), "fab-hide", "box:3787|t2")), "fab-show", "box:3787|t2")), "box:3787")), "fab.min.p2", ""), "0")
    # a row's menu names its thread with its hub: the hub gets the plain id
    C.Check.str("a row's fork names the plain thread", said(H.Hubs.cmds(H.Hubs.act(two(), "fork", "box:3787|t2"))), "box:3787 {\"id\":0,\"m\":\"thread.fork\",\"p\":{\"thread\":\"t2\"}} ")
    C.Check.str("the fork opens on its hub", H.Hubs.cur(H.Hubs.after(H.Hubs.act(two(), "fork", "box:3787|t2"))), "box:3787")
    C.Check.str("the header's fork is the open thread's", said(H.Hubs.cmds(H.Hubs.act(H.Hubs.after(H.Hubs.act(two(), "select", "box:3787|t2")), "fork", ""))), "box:3787 {\"id\":1,\"m\":\"thread.fork\",\"p\":{\"thread\":\"t2\"}} ")
    C.Check.str("a row's merge back names the plain thread", said(H.Hubs.cmds(H.Hubs.act(two(), "back", "box:3787|t2"))), "box:3787 {\"id\":0,\"m\":\"thread.back\",\"p\":{\"thread\":\"t2\"}} ")
    C.Check.str("a row's link is copied without its hub", said(H.Hubs.cmds(H.Hubs.act(two(), "ref-copy", "box:3787|backplane://thread/t2"))), "box:3787 copy backplane://thread/t2 ")
    C.Check.str("a row's action keeps the current hub", H.Hubs.cur(H.Hubs.after(H.Hubs.act(two(), "row-settle", "box:3787|t2"))), "100.1.1.1:3787")
    C.Check.str("unknown hub: the current", cmds(H.Hubs.cmds(H.Hubs.act(two(), "add-project", "/some/path"))), "send@100.1.1.1:3787 ")
    C.Check.str("add a project on a hub", cmds(H.Hubs.cmds(H.Hubs.act(two(), "add-project", "box:3787|/some/path"))), "send@box:3787 ")
    C.Check.str("unpaired: current stays while paired", H.Hubs.cur(H.Hubs.set(H.Hubs.after(H.Hubs.act(two(), "select", "box:3787|t2")), ["box:3787"])), "box:3787")
    C.Check.str("unpaired: current falls to the first", H.Hubs.cur(H.Hubs.set(two(), ["box:3787"])), "box:3787")
    C.Check.str("kept hub keeps its log", list(H.Hubs.set(two(), ["box:3787"]), "projects", "id"), "box:3787|p2;")
    C.Check.str("one hub: no machine name", list(H.Hubs.set(two(), ["box:3787"]), "projects", "machine"), ";")
    C.Check.str("no hubs", screen(H.Hubs.init(0n, "c"), "empty"), "Pair with a hub to start.")
    C.Check.str("register tells every hub", cmds(H.Hubs.cmds(H.Hubs.register(two(), "ios", "tok", "alert", "", "sandbox", "b"))), "send@100.1.1.1:3787 send@box:3787 ")
    C.Check.str("picker opens on its hub", H.Hubs.cur(H.Hubs.after(H.Hubs.act(two(), "picker-open", "box:3787|"))), "box:3787")
    C.Check.str("picker asks that hub", cmds(H.Hubs.cmds(H.Hubs.act(two(), "picker-open", "box:3787|"))), "send@box:3787 send@box:3787 ")
    C.Check.str("picker's paths stay as they are",
      J.Json.get_str(J.Json.get_obj(H.Hubs.screen(H.Hubs.after(H.Hubs.act(H.Hubs.after(H.Hubs.act(two(), "picker-open", "box:3787|")), "picker-type", "~/"))), "folders"), "text", ""), "~/")
    C.Check.str("snooze value", H.Hubs.rest("box:3787|t2|1h"), "t2|1h")
    C.Check.str("search filters every hub", list(searching(), "projects", "id"), "box:3787|p2;")
    C.Check.str("search shown", yes(J.Json.get_bool(J.Json.get_obj(H.Hubs.screen(searching()), "search"), "open", False{})), "yes")
    C.Check.str("search query", obj(searching(), "search", "query"), "bo")
    C.Check.str("search off shows all", list(act(searching(), "proj-find", "off"), "projects", "id"), "100.1.1.1:3787|p1;box:3787|p2;")
    C.Check.str("go selects on its hub", sel(act(searching(), "proj-go", "box:3787|p2"), "box:3787"), "t2")
    C.Check.str("go moves focus", H.Hubs.cur(act(searching(), "proj-go", "box:3787|p2")), "box:3787")
    C.Check.str("go shuts every search", list(act(searching(), "proj-go", "box:3787|p2"), "projects", "id"), "100.1.1.1:3787|p1;box:3787|p2;")
    C.Check.str("remove arms a dialog", obj(act(two(), "proj-remove", "box:3787|p2"), "removing", "id"), "box:3787|p2")
    C.Check.str("dialog names it", obj(act(two(), "proj-remove", "box:3787|p2"), "removing", "title"), "Remove Box?")
    C.Check.str("confirm sends remove", cmds(H.Hubs.cmds(H.Hubs.act(act(two(), "proj-remove", "box:3787|p2"), "proj-remove", "box:3787|p2"))), "send@box:3787 ")
    C.Check.str("cancel disarms", obj(act(act(two(), "proj-remove", "box:3787|p2"), "proj-keep", ""), "removing", "id"), "")
    # renaming another hub's thread: its id routed, its title left as it is
    C.Check.str("rename names the thread on its hub", obj(act(two(), "row-rename", "box:3787|t2"), "renaming", "id"), "box:3787|t2")
    C.Check.str("rename's title is the title alone", obj(act(two(), "row-rename", "box:3787|t2"), "renaming", "text"), "Two")
    C.Check.str("rename goes to the thread's hub", cmds(H.Hubs.cmds(H.Hubs.act(act(two(), "row-rename", "box:3787|t2"), "rename-save", "box:3787|t2\u{1f}Deux"))), "send@box:3787 ")
    C.Check.str("no match says so", screen(act(searching(), "proj-find-q", "zzz"), "empty"), "No project matches.")
