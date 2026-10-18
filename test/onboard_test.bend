import Base
import ../src/core/i18n.bend as I
import ./check.bend as C
import ../src/core/onboard.bend as OB
import ../src/core/tools.bend as TO
import ../src/core/tailnet.bend as TN
import ../src/core/model.bend as M
import ../src/core/client.bend as U
import ../src/mobile/screen.bend as S
import ../src/core/json.bend as J

# The first-run tour (core/onboard.bend, client.bend's Tour.*): the words,
# the moves between steps, and when it shows.

def ui(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}:
      u

def cmds(a: U.Act) -> List<&2, U.Cmd>:
  match a:
    case U.Act{_, cs}:
      cs

def sent(a: U.Act) -> String:
  Nat.show(List.length(&2, U.Cmd, cmds(a)))

# the focus a step leaves: "composer", "" or none
def focus.of(cs: List<&2, U.Cmd>) -> String:
  match cs:
    case U.Focus{id} <> _:
      "focus:" ++ id
    case _:
      "none"

def focus(a: U.Act) -> String:
  match a:
    case U.Act{_, cs}:
      focus.of(cs)

def logged() -> M.State:
  M.State.replay([M.ProjectCreated{"p1", "p", "/p", 1n}], M.State.empty())

def finished() -> M.State:
  M.State.replay([M.ProjectCreated{"p1", "p", "/p", 1n}, M.SettingSet{"onboard.done", "1", 2n}, M.SettingSet{"onboard.uber", "1", 2n}], M.State.empty())

# a client that holds this connection's state, with no setting yet
def fresh() -> U.Ui:
  U.Ui.scratch_set(U.Ui.adopt.raw(U.Ui.init(1000n), logged()), "@sync", "1")

# done long ago, before the uberbot step: no onboard.uber
def old() -> U.Ui:
  U.Ui.scratch_set(U.Ui.adopt.raw(U.Ui.init(1000n), M.State.replay([M.ProjectCreated{"p1", "p", "/p", 1n}, M.SettingSet{"onboard.done", "1", 2n}], M.State.empty())), "@sync", "1")

def done() -> U.Ui:
  U.Ui.scratch_set(U.Ui.adopt.raw(U.Ui.init(1000n), finished()), "@sync", "1")

# the info a hub sends for Settings' Tailscale row
def tn(+t: String, +pair: String) -> Map<&2, String>:
  Map.set(&2, String, Map.set(&2, String, Map.set(&2, String, Map.new(&2, String), "tailnet", t), "listen", "127.0.0.1:3787"), "pair", pair)

# the same from a hub that reports its tailnet state
def tt(+state: String, +t: String, +pair: String) -> Map<&2, String>:
  Map.set(&2, String, tn(t, pair), "tailscale", state)

def at(u: U.Ui) -> String:
  Nat.show(U.Tour.at(u))

def act(u: U.Ui, a: String) -> U.Ui:
  ui(U.Ui.act(u, a, ""))

def act3(u: U.Ui, a: String, v: String) -> U.Ui:
  ui(U.Ui.act(u, a, v))

# a string holds a piece (found by splitting on it)
def holds(+s: String, +p: String) -> Bool:
  String.contains(s, p)

def end() -> String:
  Nat.show(OB.Ob.count())

# the text of every request a step sent
def sends.of(cs: List<&2, U.Cmd>) -> String:
  match cs:
    case U.Send{t} <> r:
      t ++ "|" ++ sends.of(r)
    case _ <> r:
      sends.of(r)
    case Nil{}:
      ""

def sends(a: U.Act) -> String:
  sends.of(cmds(a))

# three bots: Kit said once, Pip and Zed three times (Pip is older)
def bots3() -> M.State:
  M.State.replay([M.ThreadCreated{"t1", "", "Kit", M.Local{}, "", "", "claude", "", 100n}, M.BotSet{"b1", "Kit", "t1", 7n, "", 100n},
    M.ThreadCreated{"t2", "", "Pip", M.Local{}, "", "", "claude", "", 101n}, M.BotSet{"b2", "Pip", "t2", 8n, "", 101n},
    M.ThreadCreated{"t3", "", "Zed", M.Local{}, "", "", "claude", "", 102n}, M.BotSet{"b3", "Zed", "t3", 9n, "", 102n},
    M.MessagePosted{"t1", "m1", M.User{}, "a", 110n},
    M.MessagePosted{"t2", "m2", M.User{}, "a", 111n}, M.MessagePosted{"t2", "m3", M.User{}, "b", 112n}, M.MessagePosted{"t2", "m4", M.User{}, "c", 113n},
    M.MessagePosted{"t3", "m5", M.User{}, "a", 114n}, M.MessagePosted{"t3", "m6", M.User{}, "b", 115n}, M.MessagePosted{"t3", "m7", M.User{}, "c", 116n}], M.State.empty())

def uberstep(+st: M.State) -> U.Ui:
  U.Ui.scratch_set(U.Ui.scratch_set(U.Ui.adopt.raw(U.Ui.init(1000n), st), "@sync", "1"), "@tour", "9")

def labels(cs: List<&2, U.TChoice>) -> String:
  match cs:
    case U.TChoice{l, a, v} <> r:
      l ++ " " ++ a ++ " " ++ v ++ "; " ++ labels(r)
    case Nil{}:
      "."

# every focus a step leaves, in order
def focuses.of(cs: List<&2, U.Cmd>) -> String:
  match cs:
    case U.Focus{id} <> r:
      "focus:" ++ id ++ " " ++ focuses.of(r)
    case _ <> r:
      focuses.of(r)
    case Nil{}:
      ""

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("steps are sound", Bool.show(OB.Ob.sound()), "True")
    C.Check.str("moves stay in range", Bool.show(OB.Ob.nav(20n)), "True")
    C.Check.str("first step", OB.Ob.title(I.En{}, 0n), "Welcome to Backplane")
    C.Check.str("a step past the end is blank", OB.Ob.title(I.En{}, 40n) ++ OB.Ob.body(I.En{}, 1n, "7", 40n), "")
    # when it shows
    C.Check.str("waits for the hub's state", at(U.Ui.init(1000n)), end())
    C.Check.str("a cached state is not current", at(U.Ui.adopt.raw(U.Ui.init(1000n), logged())), end())
    C.Check.str("a log makes it current", at(U.Ui.adopt_log(U.Ui.init(1000n), logged(), U.Reset.demo("o"))), "0")
    C.Check.str("a new connection is not current", at(U.Ui.online(fresh(), False{})), end())
    C.Check.str("first run shows step 0", at(fresh()), "0")
    C.Check.str("done never shows by itself", at(done()), end())
    C.Check.str("reopened after done", at(act(done(), "ob-open")), "0")
    C.Check.str("reopen shuts Settings", Bool.show(U.Ui.flag(act(U.Ui.flag_on(done(), "settings"), "ob-open"), "settings")), "False")
    # moves
    C.Check.str("next", at(act(fresh(), "ob-next")), "1")
    C.Check.str("next, next, back", at(act(act(act(fresh(), "ob-next"), "ob-next"), "ob-back")), "1")
    C.Check.str("back stays at the first", at(act(fresh(), "ob-back")), "0")
    C.Check.str("a move sends nothing", sent(U.Ui.act(fresh(), "ob-next", "")), "0")
    C.Check.str("skip ends it", at(act(fresh(), "ob-skip")), end())
    C.Check.str("skip tells the hub", sent(U.Ui.act(fresh(), "ob-skip", "")), "3")
    C.Check.str("skip is kept as a setting", M.Setting.get(U.Ui.state(act(fresh(), "ob-skip")), "onboard.done", ""), "1")
    C.Check.str("last step reads Done", OB.Ob.forward(I.En{}, 9n) ++ " " ++ OB.Ob.forward(I.En{}, 8n), "Done Next")
    C.Check.str("next from the last ends it", Bool.show(OB.Ob.live(OB.Ob.next(9n))), "False")
    C.Check.str("a stray next with no tour does nothing", sent(U.Ui.act(done(), "ob-next", "")), "0")
    C.Check.str("a stray next leaves it done", at(act(done(), "ob-next")), end())
    C.Check.str("counter", OB.Ob.counter(I.En{}, 2n), "3 of 10")
    # a done from another client
    C.Check.str("done elsewhere closes an auto tour that moved on", at(U.Ui.adopt.raw(act(act(fresh(), "ob-next"), "ob-next"), finished())), end())
    C.Check.str("done elsewhere leaves a hand-opened tour", at(U.Ui.adopt.raw(act(done(), "ob-open"), finished())), "0")
    C.Check.str("a hand-opened tour moves on and stays", at(act(act(done(), "ob-open"), "ob-next")), "1")
    C.Check.str("a hand-opened tour works offline", at(act(U.Ui.online(done(), False{}), "ob-open")), "0")
    # the keyboard goes back
    C.Check.str("skip with no thread leaves no focus", focus(U.Ui.act(fresh(), "ob-skip", "")), "focus:")
    C.Check.str("skip in a thread focuses its composer", focus(U.Ui.act(U.Ui.select.k(fresh(), "t1"), "ob-skip", "")), "focus:composer")
    C.Check.str("showing it again takes the keyboard off Settings", focus(U.Ui.act(U.Ui.flag_on(done(), "settings"), "ob-open", "")), "focus:")
    # what is on screen, not what is selected
    C.Check.str("a cached state waits for the hub", at(U.Ui.unsync(fresh())), end())
    C.Check.str("skip with a remote chat showing focuses its field", focus(U.Ui.act(U.Bots.views(U.Ui.select.k(fresh(), "t1"), "", "", "x"), "ob-skip", "")), "focus:remote-send")
    C.Check.str("skip with Settings up focuses nothing", focus(U.Ui.act(U.Ui.flag_on(U.Ui.select.k(fresh(), "t1"), "settings"), "ob-skip", "")), "focus:")
    C.Check.str("no thread's head under a remote chat", U.Tour.target(0n, U.Ui.scratch_set(U.Bots.views(U.Ui.select.k(fresh(), "t1"), "", "", "x"), "@tour", "m3")), "")
    # the uberbot step alone, for an install that finished the tour before it
    C.Check.str("existing: shows alone", at(old()) ++ "|" ++ U.Tour.counter(old()) ++ "|" ++ Bool.show(U.Tour.back(old())), "9||False")
    C.Check.str("existing: not from a cached state", at(U.Ui.unsync(old())), end())
    C.Check.str("existing: back goes nowhere", at(act(old(), "ob-back")), "9")
    C.Check.str("existing: Done answers it", at(act(old(), "ob-next")) ++ M.Setting.get(U.Ui.state(act(old(), "ob-next")), "onboard.uber", ""), end() ++ "1")
    C.Check.str("existing: Skip answers it", at(act(old(), "ob-skip")) ++ M.Setting.get(U.Ui.state(act(old(), "ob-skip")), "onboard.uber", ""), end() ++ "1")
    C.Check.str("existing: a choice answers it", at(act3(old(), "uber-new", "")) ++ M.Setting.get(U.Ui.state(act3(old(), "uber-new", "")), "onboard.uber", ""), end() ++ "1")
    C.Check.str("existing: the full tour's Skip sets both", M.Setting.get(U.Ui.state(act(fresh(), "ob-skip")), "onboard.uber", ""), "1")
    C.Check.str("existing: an uberbot is not asked", at(U.Ui.adopt.raw(old(), M.State.replay([M.UberSet{"b1", "u1-a", 9n}, M.SettingSet{"onboard.done", "1", 9n}], bots3()))), end())
    C.Check.str("existing: the full tour says 10 of 10 at its end", U.Tour.counter(act(fresh(), "ob-next")), "2 of 10")
    # the uberbot step
    C.Check.str("uberbot: the last step", OB.Ob.title(I.En{}, 9n) ++ "|" ++ OB.Ob.target(0n, 9n) ++ "|" ++ OB.Ob.target(2n, 9n), "Your uberbot|sidebar|")
    C.Check.str("uberbot: no choices on other steps", labels(U.Tour.choices(fresh())), ".")
    C.Check.str("uberbot: no bots creates one", labels(U.Tour.choices(uberstep(logged()))), "Create my uberbot uber-new ; .")
    C.Check.str("uberbot: no bots words", Bool.show(holds(U.Tour.body(0n, uberstep(logged())), "Create it now; you can rename it later.")), "True")
    C.Check.str("uberbot: pick the most used, ties the oldest", labels(U.Tour.choices(uberstep(bots3()))), "Make Pip my uberbot uber-promote b2; Make Zed uber-promote b3; Make Kit uber-promote b1; Create a new one uber-new ; .")
    C.Check.str("uberbot: pick words suggest it", Bool.show(holds(U.Tour.body(0n, uberstep(bots3())), "Pip is the one you use most")), "True")
    C.Check.str("uberbot: have, no choices", labels(U.Tour.choices(uberstep(M.State.replay([M.UberSet{"b1", "u1-a", 200n}], bots3())))), ".")
    C.Check.str("uberbot: have words, window and phone", Bool.show(Bool.and(holds(U.Tour.body(0n, uberstep(M.State.replay([M.UberSet{"b1", "u1-a", 200n}], bots3()))), "Kit is your uberbot. Click it at the top of the sidebar for the Status of everything."),
      holds(U.Tour.body(2n, uberstep(M.State.replay([M.UberSet{"b1", "u1-a", 200n}], bots3()))), "at the top of the list"))), "True")
    C.Check.str("uberbot: promote asks the hub and ends the tour", Bool.show(holds(sends(U.Ui.act(uberstep(bots3()), "uber-promote", "b2")), "bots.uber")) ++ at(act3(uberstep(bots3()), "uber-promote", "b2")), "True" ++ end())
    C.Check.str("uberbot: promote marks the tour done", M.Setting.get(U.Ui.state(act3(uberstep(bots3()), "uber-promote", "b2")), "onboard.done", ""), "1")
    C.Check.str("uberbot: create opens the form for it and ends the tour", U.Ui.scratch(ui(U.Ui.act(uberstep(logged()), "uber-new", "")), "@bnew") ++ U.Ui.scratch(act(uberstep(logged()), "uber-new"), "@uber.new") ++ at(act(uberstep(logged()), "uber-new")), "11" ++ end())
    C.Check.str("uberbot: create ends with the keyboard in the name field", focuses.of(cmds(U.Ui.act(uberstep(logged()), "uber-new", ""))), "focus: focus:bot-name ")
    C.Check.str("uberbot: the form hints at it", U.Bots.name.hint(act(uberstep(logged()), "uber-new")) ++ "|" ++ U.Bots.name.hint(U.Ui.init(1n)), "Name your uberbot|Name")
    C.Check.str("uberbot: the form's request carries uber", Bool.show(holds(sends(U.Ui.act(act3(act(uberstep(logged()), "uber-new"), "bfield", "name\u{1f}Ace"), "bot-create", "")), "\"uber\":true")), "True")
    C.Check.str("uberbot: an ordinary new bot does not", Bool.show(holds(sends(U.Ui.act(act3(U.Ui.init(1n), "bfield", "name\u{1f}Ace"), "bot-create", "")), "uber")), "False")
    C.Check.str("uberbot: closing the form by + forgets it", U.Ui.scratch(act(act(act(uberstep(logged()), "uber-new"), "bot-new"), "bot-new"), "@uber.new"), "")
    C.Check.str("uberbot: a stray promote with no tour just asks", at(act3(done(), "uber-promote", "b2")) ++ Bool.show(holds(sends(U.Ui.act(done(), "uber-promote", "b2")), "bots.uber")), end() ++ "True")
    C.Check.str("uberbot: back from it", at(act(uberstep(logged()), "ob-back")), "8")
    C.Check.str("uberbot: the phone screen carries the choices", J.Json.show(J.Json.get_obj(J.Json.get_obj(S.Mob.screen(uberstep(logged())), "tour"), "choices")), "[{\"label\":\"Create my uberbot\",\"action\":\"uber-new\",\"value\":\"\"}]")
    # Settings' Tailscale row
    C.Check.str("tailscale off: the flag", U.Tail.label(I.En{}, tn("off", "")) ++ "|" ++ U.Tail.note(I.En{}, tn("off", "")), "Tailscale: off|Listening on 127.0.0.1:3787 only: started with --no-tailscale.")
    C.Check.str("tailscale not connected: no pairing link", U.Tail.label(I.En{}, tn("on", "")) ++ "|" ++ U.Tail.note(I.En{}, tn("on", "")), "Tailscale: not connected|Listening on 127.0.0.1:3787 only. Sign in to Tailscale, then restart Backplane.")
    C.Check.str("tailscale up: a pairing link", U.Tail.label(I.En{}, tn("on", "http://x/#token=t")) ++ "|" ++ U.Tail.label(I.En{}, tn("https", "https://x/#token=t")), "Tailscale: on|Tailscale: https")
    C.Check.str("a hub reachable on the tailnet with no pairing link (--host 0.0.0.0)", U.Tail.label(I.En{}, tt("reachable", "on", "")) ++ "|" ++ U.Tail.note(I.En{}, tt("reachable", "on", "")), "Tailscale: on|Also on your tailnet: your own devices get in, others need the pairing token.")
    C.Check.str("a hub that found no tailscale", U.Tail.label(I.En{}, tt("unavailable", "on", "")) ++ "|" ++ U.Tail.note(I.En{}, tt("unavailable", "on", "http://x/#token=t")), "Tailscale: not connected|Listening on 127.0.0.1:3787 only. Sign in to Tailscale, then restart Backplane.")
    C.Check.str("a hub still looking", U.Tail.label(I.En{}, tt("checking", "on", "")), "Tailscale: checking")
    C.Check.str("a hub with the flag", U.Tail.label(I.En{}, tt("off", "off", "")), "Tailscale: off")
    C.Check.str("the tailnet report keeps both", TN.Tailnet.report.state(TN.Tailnet.report("reachable", "http://x")) ++ "|" ++ TN.Tailnet.report.link(TN.Tailnet.report("reachable", "http://x")) ++ "|" ++ TN.Tailnet.report.state("") ++ "|" ++ TN.Tailnet.report.link("") ++ "|" ++ TN.Tailnet.report.link(TN.Tailnet.report("unavailable", "")), "reachable|http://x|||")
    # the words
    C.Check.str("the buttons are named by tools.bend's labels", Bool.show(Bool.and(Bool.and(holds(OB.Ob.body(I.En{}, 0n, "7", 3n), TO.Tools.label(TO.Tools.settle(False{}))), holds(OB.Ob.body(I.En{}, 0n, "7", 3n), TO.Tools.label(TO.Tools.snooze(False{}, "", "")))),
      Bool.and(Bool.and(String.starts_with(TO.Tools.label(TO.Tools.fork()), "Fork"), holds(OB.Ob.body(I.En{}, 0n, "7", 3n), "Fork starts")), holds(OB.Ob.body(I.En{}, 1n, "7", 3n), TO.Tools.label(TO.Tools.panel(False{})))))), "True")
    C.Check.str("the buttons read as sentences", Bool.show(holds(OB.Ob.body(I.En{}, 0n, "7", 3n), "Settle takes a thread out of the active list. Snooze hides it until later. Fork starts a new thread from this one.")), "True")
    C.Check.str("worktrees by default, shared otherwise", Bool.show(holds(OB.Ob.body(I.En{}, 0n, "7", 2n), "Git projects use separate worktrees by default; without git, or in local mode, threads share the project folder.")), "True")
    C.Check.str("web: Enter on a button presses it", Bool.show(holds(OB.Ob.body(I.En{}, 1n, "7", 0n), "on a focused button it presses that button")), "True")
    C.Check.str("tailnet: the other device needs Tailscale too", Bool.show(Bool.and(holds(OB.Ob.body(I.En{}, 2n, "7", 7n), "this phone needs Tailscale on the same tailnet"), holds(OB.Ob.body(I.En{}, 0n, "7", 8n), "install Tailscale on it and join the same tailnet"))), "True")
    C.Check.str("machines: background sync, named as Settings names it", Bool.show(Bool.and(holds(OB.Ob.body(I.En{}, 0n, "7", 7n), "Settings, " ++ U.Mate.title(I.En{})), Bool.and(holds(OB.Ob.body(I.En{}, 0n, "7", 7n), "every two minutes"), holds(OB.Ob.body(I.En{}, 2n, "7", 7n), "says syncing")))), "True")
    C.Check.str("scratch expires, with its days", Bool.show(holds(OB.Ob.body(I.En{}, 0n, "9", 1n), "9 days is deleted with its folder, unless pinned")), "True")
    C.Check.str("days come from the setting", U.Tour.days(U.Ui.adopt.raw(U.Ui.init(1n), M.State.replay([M.SettingSet{"scratch.days", "3", 1n}], M.State.empty()))), "3")
    C.Check.str("days default to 7", U.Tour.days(U.Ui.init(1n)), "7")
    C.Check.str("keys: no guarantee claimed", Bool.show(holds(OB.Ob.body(I.En{}, 0n, "7", 6n), "nothing enforces it")), "True")
    C.Check.str("agents: signed in on the hub", Bool.show(holds(OB.Ob.body(I.En{}, 0n, "7", 5n), "sign in to it on the machine that runs this hub")), "True")
    C.Check.str("tailnet: sign in, restart", Bool.show(holds(OB.Ob.body(I.En{}, 1n, "7", 7n), "sign in, then restart Backplane")), "True")
    C.Check.str("pairing: the QR code", Bool.show(holds(OB.Ob.body(I.En{}, 0n, "7", 8n), "Pairing link, QR code")), "True")
    C.Check.str("phones: folder-plus, not the sidebar", Bool.show(holds(OB.Ob.body(I.En{}, 2n, "7", 1n), "folder-plus")) ++ Bool.show(holds(OB.Ob.body(I.En{}, 0n, "7", 1n), "folder-plus")), "TrueFalse")
    C.Check.str("phones: no voice-key field", Bool.show(holds(OB.Ob.body(I.En{}, 2n, "7", 6n), "on a computer or the web page")), "True")
    C.Check.str("phones point at nothing, the head needs a thread", OB.Ob.target(2n, 3n) ++ "|" ++ U.Tour.target(0n, act(fresh(), "ob-next")) ++ "|" ++ U.Tour.target(0n, act(act(act(U.Ui.select.k(fresh(), "t1"), "ob-next"), "ob-next"), "ob-next")), "|add|head")
    C.Check.str("the head's highlight waits for a thread", U.Tour.target(0n, act(act(act(fresh(), "ob-next"), "ob-next"), "ob-next")), "")
    C.Check.str("keys: one policy", OB.Ob.dom("Enter") ++ OB.Ob.dom("ArrowRight") ++ OB.Ob.dom("ArrowLeft") ++ OB.Ob.dom("Escape") ++ "|" ++ OB.Ob.dom("a"), "ob-nextob-nextob-backob-skip|")
    # the phones' screen
    C.Check.str("phone screen carries the tour", J.Json.get_str(J.Json.get_obj(S.Mob.screen(fresh()), "tour"), "title", ""), "Welcome to Backplane")
    C.Check.str("phone body is the phone's", Bool.show(holds(J.Json.get_str(J.Json.get_obj(S.Mob.screen(act(fresh(), "ob-next")), "tour"), "body", ""), "folder-plus")), "True")
    C.Check.str("phone screen holds the tour while Settings is up", J.Json.show(J.Json.get_obj(S.Mob.screen(U.Ui.flag_on(fresh(), "settings")), "tour")), "null")
    C.Check.str("phone screen holds the tour while Find is up", J.Json.show(J.Json.get_obj(S.Mob.screen(U.Ui.flag_on(fresh(), "palette")), "tour")), "null")
    C.Check.str("phone screen holds the tour while a row menu is up", J.Json.show(J.Json.get_obj(S.Mob.screen(U.Ui.scratch_set(fresh(), "@menu", "r:t1")), "tour")), "null")
    C.Check.str("phone screen shows the tour again once Settings is shut", J.Json.get_str(J.Json.get_obj(S.Mob.screen(U.Ui.flag_off(U.Ui.flag_on(fresh(), "settings"), "settings")), "tour"), "title", ""), "Welcome to Backplane")
    C.Check.str("phone screen has none once done", J.Json.show(J.Json.get_obj(S.Mob.screen(done()), "tour")), "null")
