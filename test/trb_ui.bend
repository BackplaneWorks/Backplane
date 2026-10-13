import Base
import ../src/core/json.bend as J
import ../src/core/model.bend as M
import ../src/core/client.bend as U
import ../src/core/ruledata.bend as RD
import ../src/core/tbdata.bend as TD

def ui() -> U.Ui:
  U.Ui.far.st(U.BoardUi.demo(), M.State.replay([
    M.ProjectCreated{"p1", "Power", "/tmp/power", 1n}, M.ThreadCreated{"t1", "", "Miso", M.Local{}, "", "", "claude", "", 1n}, M.BotSet{"b1", "Miso", "t1", 0n, "", 1n},
    M.SettingSet{"rule.r1", "{\"text\":\"Ask first\",\"bot\":\"b1\",\"project\":\"p1\",\"tool\":\"spend*\",\"enforce\":true,\"word\":\"sudo\",\"on\":false}", 1n},
    M.SettingSet{"board.owned", "{\"title\":\"Review\",\"owner\":\"b1\",\"project\":\"p1\",\"group\":\"Rev C\",\"notes\":\"Check the drawing\",\"links\":[\"https://example.com\",\"/tmp/drawing.pdf\"]}", 1n}], U.Ui.state(U.BoardUi.demo())))

def act(+u: U.Ui, +a: String, +v: String) -> U.Ui:
  U.Act.ui(U.Ui.act(u, a, v))

def sent(cs: List<&2, U.Cmd>) -> String:
  match cs:
    case Nil{}:
      ""
    case U.Send{s} <> _:
      s
    case _ <> t:
      sent(t)

def request(+u: U.Ui, +a: String, +v: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(sent(U.Act.cmds(U.Ui.act(u, a, v)))), J.Null{})

def params(+u: U.Ui, +a: String, +v: String) -> J.Json:
  J.Json.get_obj(request(u, a, v), "p")

def method(+u: U.Ui, +a: String, +v: String) -> String:
  J.Json.get_str(request(u, a, v), "m", "")
