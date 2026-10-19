import Base
import ../src/core/cef.bend as CF
import ../src/core/client.bend as U
import ../src/core/model.bend as M
import ../src/core/json.bend as J
import ./check.bend as C

def ui() -> U.Ui:
  U.Act.ui(U.Ui.toggle_panel(U.Ui.info_set(U.Ui.select.k(U.Ui.fold([M.ThreadCreated{"t1", "", "Browser test", M.Local{}, "", "", "codex", "", 1n}], U.Ui.init(0n)), "t1"), "view.kind:t1", "browser")))

def sent(a: U.Act) -> String:
  match a:
    case U.Act{_, U.Send{s} <> _}: s
    case _: ""

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("hostname", CF.Cef.url("example.com/a?q=1"), "https://example.com/a?q=1")
    C.Check.str("trim address", CF.Cef.url("  https://example.com/x  "), "https://example.com/x")
    C.Check.str("local service", CF.Cef.url("localhost:3787/path"), "http://localhost:3787/path")
    C.Check.str("IPv4 service", CF.Cef.url("127.0.0.1:8080"), "http://127.0.0.1:8080")
    C.Check.str("tailnet dev service", CF.Cef.url("framework-1:3000/test"), "http://framework-1:3000/test")
    C.Check.str("blank tab", CF.Cef.url("about:blank"), "about:blank")
    C.Check.str("license page", CF.Cef.url("about:credits"), "about:credits")
    C.Check.str("no script address", CF.Cef.url("javascript:alert(1)"), "")
    C.Check.str("no data address", CF.Cef.url("data:text/html,test"), "")
    C.Check.str("no empty address", CF.Cef.url("  "), "")
    C.Check.str("browser owner", U.Browse.owner(ui()), "t1")
    C.Check.str("new tab targets thread", sent(U.Ui.act(ui(), "br-new", "")), "{\"id\":0,\"m\":\"bots.browser\",\"p\":{\"bot\":\"t1\",\"op\":\"toolbar\",\"command\":\"tabs\",\"new\":true}}")
    C.Check.str("stable select id", sent(U.Ui.act(ui(), "br-select", "tab-42")), "{\"id\":0,\"m\":\"bots.browser\",\"p\":{\"bot\":\"t1\",\"op\":\"toolbar\",\"command\":\"tabs\",\"selectId\":\"tab-42\"}}")
    C.Check.str("current URL displayed", U.Browse.address(U.Ui.info_set(ui(), "browser.tabs.t1", "{\"url\":\"https://example.com\",\"tabs\":[]}"), "t1"), "https://example.com")
    C.Check.str("address draft preserves empty edit", U.Browse.address(U.Ui.scratch_set(U.Ui.info_set(ui(), "browser.tabs.t1", "{\"url\":\"https://example.com\"}"), "@browse.edit", "brurl:t1"), "t1"), "")
    C.Check.str("bot target isolated", U.Browse.owner(U.Ui.scratch_set(U.Ui.scratch_set(ui(), "@bot", "b1"), "@btab", "browser")), "b1")
    C.Check.str("toolbar strips protocol overrides", J.Json.show(CF.Cef.toolbar.args(Maybe.default(&2, J.Json, J.Json.parse("{\"command\":\"navigate\",\"url\":\"localhost:3000\",\"op\":\"evaluate\",\"page\":\"other\",\"args\":{\"expression\":\"bad\"}}"), J.Null{}))), "{\"url\":\"http://localhost:3000\"}")
    C.Check.str("external localhost lookalike stays HTTPS", CF.Cef.url("localhost.example.com"), "https://localhost.example.com")
    C.Check.str("mixed-case HTTPS retained", CF.Cef.url("HTTPS://example.com/X"), "HTTPS://example.com/X")
    C.Check.str("mixed-case loopback", CF.Cef.url("LOCALHOST/path"), "http://LOCALHOST/path")
    C.Check.str("tab offset clamps after close", Nat.show(U.Browse.offset.k(9n, 2n)), "1")
