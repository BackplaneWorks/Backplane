import Base
import ../src/core/cef.bend as CF
import ../src/core/json.bend as J
import ../src/core/client.bend as U
import ../src/core/model.bend as M
import ../src/core/hub.bend as B
import ../src/core/bothub.bend as BH
import ./check.bend as C

def yes(b: Bool) -> String:
  match b:
    case True{}: "yes"
    case False{}: "no"

def parsed(s: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(s), J.Null{})

def routed(s: B.Step) -> String:
  match s:
    case B.Step{_, B.Teach{_, _, _, _} <> _}: "teach"
    case B.Step{_, _}: "refused"

def browser_route(+id: String) -> String:
  routed(BH.Bots.request(M.State.replay([M.ThreadCreated{"t1", "", "Thread", M.Local{}, "", "", "claude", "", 1n}], M.State.empty()), 2n, 1,
    parsed("{\"id\":1,\"m\":\"bots.browser\",\"p\":{\"bot\":\"" ++ id ++ "\",\"op\":\"handback\"}}")))

def sent(a: U.Act) -> String:
  match a:
    case U.Act{_, U.Send{t} <> _}: t
    case U.Act{_, _}: ""

def reps(n: Nat) -> List<&2, String>:
  match n:
    case 0n: Nil{}
    case 1n+k: "x" <> reps(k)

def keys(kv: List<&2, J.KV>) -> String:
  J.Json.show(J.Json.obj(kv))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("fallback explained", CF.Cef.reason("cef", "chrome", ""), "native Linux X11 window/runtime is not active")
    C.Check.str("explicit Chrome quiet", CF.Cef.reason("chrome", "chrome", ""), "")
    C.Check.str("runtime error retained", CF.Cef.reason("cef", "chrome", "missing runtime"), "missing runtime")
    C.Check.str("default engine request", CF.Cef.default(), "cef")
    C.Check.str("unset engine request", CF.Cef.requested(""), "cef")
    C.Check.str("explicit Chrome request", CF.Cef.requested("chrome"), "chrome")
    C.Check.str("explicit Chrome stays off", yes(CF.Cef.engine(CF.Cef.requested("chrome"), True{}, True{})), "no")
    C.Check.str("default embeds when available", yes(CF.Cef.engine(CF.Cef.default(), True{}, True{})), "yes")
    C.Check.str("embedded requires parent", yes(CF.Cef.engine("cef", False{}, True{})), "no")
    C.Check.str("embedded requires runtime", yes(CF.Cef.engine("cef", True{}, False{})), "no")
    C.Check.str("embedded enabled", yes(CF.Cef.engine("cef", True{}, True{})), "yes")
    C.Check.str("engine key chrome", CF.Cef.want.key(False{}, 42, ":96"), "chrome")
    C.Check.str("engine key cef", CF.Cef.want.key(True{}, 42, ":96"), "cef 42 :96")
    C.Check.str("cef args", J.Json.show(CF.Cef.args(True{}, 42, ":96")), "{\"engine\":\"cef\",\"parent\":42,\"display\":\":96\"}")
    C.Check.str("chrome args", J.Json.show(CF.Cef.args(False{}, 42, ":96")), "{\"engine\":\"chrome\"}")
    C.Check.str("restart flags cef", String.join(CF.Cef.argv(True{}, 42, ":96"), " "), "--engine cef --parent 42 --display :96")
    C.Check.str("restart flags chrome", String.join(CF.Cef.argv(False{}, 42, ":96"), " "), "")
    C.Check.str("window here embeds", yes(CF.Cef.from(0, False{})), "yes")
    C.Check.str("loopback link embeds", yes(CF.Cef.from(9, True{})), "yes")
    C.Check.str("remote client refused", yes(CF.Cef.from(9, False{})), "no")
    C.Check.str("loopback v4", yes(CF.Cef.loopback("127.0.0.1")), "yes")
    C.Check.str("loopback mapped", yes(CF.Cef.loopback("::ffff:127.0.0.1")), "yes")
    C.Check.str("tailnet not loopback", yes(CF.Cef.loopback("100.64.0.7")), "no")
    C.Check.str("marked loopback client", yes(CF.Cef.session.is_local(CF.Cef.session.near(CF.Cef.empty(), 9), 9)), "yes")
    C.Check.str("unmarked client", yes(CF.Cef.session.is_local(CF.Cef.session.near(CF.Cef.empty(), 9), 10)), "no")
    C.Check.str("leave drops loopback mark", yes(CF.Cef.session.is_local(CF.Cef.session.leave(CF.Cef.session.near(CF.Cef.empty(), 9), 9), 9)), "no")
    C.Check.str("agent shield", yes(CF.Cef.shield("")), "yes")
    C.Check.str("takeover unshielded", yes(CF.Cef.shield("takeover")), "no")
    C.Check.str("teach unshielded", yes(CF.Cef.shield("teach")), "no")
    C.Check.str("shield not resent", yes(CF.Cef.shield.due("1", "")), "no")
    C.Check.str("shield sent on change", yes(CF.Cef.shield.due("1", "teach")), "yes")
    C.Check.str("uncovered", yes(CF.Cef.covered(False{}, False{}, False{}, False{}, False{}, False{}, False{}, False{})), "no")
    C.Check.str("palette covers", yes(CF.Cef.covered(True{}, False{}, False{}, False{}, False{}, False{}, False{}, False{})), "yes")
    C.Check.str("perf HUD covers", yes(CF.Cef.covered(False{}, False{}, False{}, False{}, False{}, False{}, True{}, False{})), "yes")
    C.Check.str("lightbox covers", yes(CF.Cef.covered(False{}, False{}, False{}, False{}, False{}, False{}, False{}, True{})), "yes")
    C.Check.str("placement unchanged", Nat.show(CF.Cef.place.step("a 1 2 3 4", "a 1 2 3 4")), "0")
    C.Check.str("placement new", Nat.show(CF.Cef.place.step("", "a 1 2 3 4")), "1")
    C.Check.str("placement resize", Nat.show(CF.Cef.place.step("a 1 2 3 4", "a 1 2 3 5")), "1")
    C.Check.str("placement hide", Nat.show(CF.Cef.place.step("a 1 2 3 4", "")), "2")
    C.Check.str("chrome forgets once", Nat.show(CF.Cef.post.step(False{}, "a 1 2 3 4", "")), "3")
    C.Check.str("chrome idle", Nat.show(CF.Cef.post.step(False{}, "", "")), "0")
    C.Check.str("hit bounds become pane key", CF.Cef.at.hit("t1", 832, 96, 1400, 900), "t1 832 96 568 804")
    C.Check.str("empty hit no key", CF.Cef.at.hit("t1", 832, 96, 832, 900), "")
    C.Check.str("place codec", CF.Cef.rpc("browser.place", CF.Cef.place.of("t1 832 96 568 804")), "{\"id\":0,\"m\":\"browser.place\",\"p\":{\"page\":\"t1\",\"x\":832,\"y\":96,\"w\":568,\"h\":804}}")
    C.Check.str("hide codec", CF.Cef.rpc("browser.place", CF.Cef.place.of("")), "{\"id\":0,\"m\":\"browser.place\",\"p\":{}}")
    C.Check.str("embed codec", J.Json.show(parsed(CF.Cef.rpc("browser.embed", CF.Cef.embed(42, ":96")))), "{\"id\":0,\"m\":\"browser.embed\",\"p\":{\"xid\":42,\"display\":\":96\"}}")
    C.Check.str("hub reads the place", yes(CF.Cef.visible(parsed("{\"page\":\"t1\",\"x\":832,\"y\":96,\"w\":568,\"h\":804}"))), "yes")
    C.Check.str("hide not visible", yes(CF.Cef.visible(J.Json.obj(Nil{}))), "no")
    C.Check.str("embed once per join", yes(CF.Cef.embed.due("42", "", "o", "#1", "#1")), "no")
    C.Check.str("embed on a new join", yes(CF.Cef.embed.due("42", "", "o", "#1", "#2")), "yes")
    C.Check.str("embed waits for the log", yes(CF.Cef.embed.due("42", "", "", "", "#2")), "no")
    C.Check.str("no embed to another hub", yes(CF.Cef.embed.due("42", "http://x", "o", "", "#2")), "no")
    C.Check.str("xid text", CF.Cef.xid.text(0), "")
    C.Check.str("unknown owner refused", yes(CF.Cef.owner("unknown", False{}, False{})), "no")
    C.Check.str("thread owner", yes(CF.Cef.owner("thread", True{}, False{})), "yes")
    C.Check.str("bot owner", yes(CF.Cef.owner("bot", False{}, True{})), "yes")
    C.Check.str("real thread RPC reaches browser authority", browser_route("t1"), "teach")
    C.Check.str("unknown thread RPC refused", browser_route("invented"), "refused")
    C.Check.str("thread strip handback targets its page", sent(U.Ui.act(U.Ui.init(0n), "tk-handback", "t1")), "{\"id\":0,\"m\":\"bots.browser\",\"p\":{\"bot\":\"t1\",\"op\":\"handback\"}}")
    C.Check.str("handback names page", U.Teach.target(7n, "b1", "t9"), "t9")
    C.Check.str("handback default bot", U.Teach.target(7n, "b1", ""), "b1")
    C.Check.str("default remains JPEG", yes(U.CefUI.native(U.Ui.init(0n), "t1")), "no")
    C.Check.str("native page by its id", yes(U.CefUI.native(U.Ui.info_set(U.Ui.init(0n), "browser.engine.live", "cef"), "b1")), "yes")
    C.Check.str("far page by its id", yes(U.CefUI.native(U.Ui.info_set(U.Ui.init(0n), "browser.engine.live", "cef"), "peer~b1")), "no")
    C.Check.str("palette hides native", yes(U.CefUI.covered(U.Ui.flag_on(U.Ui.init(0n), "palette"))), "yes")
    C.Check.str("perf hides native", yes(U.CefUI.covered(U.Ui.flag_on(U.Ui.init(0n), "perf"))), "yes")
    C.Check.str("fallback keeps handback", yes(CF.Cef.strip_on("chrome", "takeover")), "yes")
    C.Check.str("Chrome idle has no new strip", yes(CF.Cef.strip_on("chrome", "")), "no")
    C.Check.str("local page native", yes(CF.Cef.page_local("t1")), "yes")
    C.Check.str("far page keeps JPEG", yes(CF.Cef.page_local("peer~t1")), "no")
    C.Check.str("native drive hint", CF.Cef.drive_text("", True{}), "Agent drives · click the page to take over")
    C.Check.str("web drive hint", CF.Cef.drive_text("", False{}), "Agent drives · view only")
    C.Check.str("held drive hint", CF.Cef.drive_text("takeover", True{}), "You drive")
    C.Check.str("settings note quiet", CF.Cef.note(""), "")
    C.Check.str("settings note error", CF.Cef.note("no runtime"), "Chrome fallback (CEF unavailable: no runtime)")
    C.Check.str("engine generation advances", CF.Cef.session.token(CF.Cef.session.request_set(CF.Cef.empty(), "cef 1 :9")), "1")
    C.Check.str("old engine reply ignored", yes(CF.Cef.current(CF.Cef.session.request_set(CF.Cef.session.request_set(CF.Cef.empty(), "a"), "b"), "1")), "no")
    C.Check.str("current engine reply used", yes(CF.Cef.current(CF.Cef.session.request_set(CF.Cef.session.request_set(CF.Cef.empty(), "a"), "b"), "2")), "yes")
    C.Check.str("disconnected parent cleared", yes(CF.Cef.session.known(CF.Cef.session.leave(CF.Cef.session.embed_set(CF.Cef.empty(), 7, 42, ":96"), 7))), "no")
    C.Check.str("other client leaving keeps parent", yes(CF.Cef.session.known(CF.Cef.session.leave(CF.Cef.session.embed_set(CF.Cef.empty(), 7, 42, ":96"), 8))), "yes")
    C.Check.str("reconcile after rpc", yes(CF.Cef.after("rpc")), "yes")
    C.Check.str("reconcile after leave", yes(CF.Cef.after("leave")), "yes")
    C.Check.str("no reconcile per agent line", yes(CF.Cef.after("agent_line")), "no")
    C.Check.str("revive first", yes(CF.Cef.revive(True{}, True{}, 0n)), "yes")
    C.Check.str("revive bounded", yes(CF.Cef.revive(True{}, True{}, 3n)), "no")
    C.Check.str("revive needs a place", yes(CF.Cef.revive(True{}, False{}, 0n)), "no")
    C.Check.str("page key told", keys(CF.Cef.pages.told("t1", "")), "{\"browser.page.t1\":\"1\"}")
    C.Check.str("pushed out page unset", keys(CF.Cef.pages.told("t1", "t0")), "{\"browser.page.t1\":\"1\",\"browser.page.t0\":\"\"}")
    C.Check.str("pages bounded", Nat.show(List.length(&2, String, CF.Cef.pages.add(reps(64n), "y"))), "64")
    C.Check.str("nothing pushed from few", CF.Cef.pages.out(["a", "b"]), "")
    C.Check.str("oldest pushed from full", CF.Cef.pages.out(List.append(&2, String, reps(63n), ["old"])), "old")
    C.Check.str("native link may embed", yes(CF.Cef.link("", "")), "yes")
    C.Check.str("loopback browser cannot embed", yes(CF.Cef.link("", "http://localhost")), "no")
    C.Check.str("proxied client cannot embed", yes(CF.Cef.link("owner", "")), "no")
