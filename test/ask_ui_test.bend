import Base
import ./check.bend as C
import ../src/app/layout.bend as L
import ../src/app/nui.bend as N
import ../src/web/view.bend as V
import ../src/web/dom.bend as D
import ../src/mobile/screen.bend as S
import ../src/core/json.bend as J

def AnswerUI.win(xs: List<&2, N.Hit>) -> String:
  match xs:
    case Nil{}:
      ""
    case N.Hit{_, _, _, _, +label, +value} <> rest:
      label ++ "=" ++ value ++ ";" ++ AnswerUI.win(rest)

def AnswerUI.web(xs: List<&2, D.Node>) -> String:
  match xs:
    case Nil{}:
      ""
    case D.El{"button", _, [D.At{"class", +cls}, D.At{"title", _}, D.At{"data-value", +value}, D.On{"click", _}], [D.Txt{+label}]} <> rest:
      label ++ "=" ++ value ++ "[" ++ cls ++ "];" ++ AnswerUI.web(rest)
    case _ <> rest:
      AnswerUI.web(rest)

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("window omp approval shows Allow and Deny only", AnswerUI.win(L.Ask.choices("approval", "q", "Allow\nDeny", "")), "Allow=q|Allow;Deny=q|Deny;")
    C.Check.str("window other approval keeps session option", AnswerUI.win(L.Ask.choices("approval", "q", "", "")), "Allow=q|accept;Allow for session=q|session;Deny=q|decline;")
    C.Check.str("web omp approval shows Allow and Deny only", AnswerUI.web(V.View.ask.btns("approval", "q", "Allow\nDeny", "")), "Allow=q|Allow[primary];Deny=q|Deny[];")
    C.Check.str("web other approval keeps session option", AnswerUI.web(V.View.ask.btns("approval", "q", "", "")), "Allow=q|accept[primary];Allow for session=q|session[];Deny=q|decline[];")
    C.Check.str("phone omp approval shows Allow and Deny only", J.Json.show(J.Json.arr(S.Mob.ask.btns(2n, "q", "Allow\nDeny", ""))), "[{\"label\":\"Allow\",\"value\":\"q|Allow\",\"primary\":true},{\"label\":\"Deny\",\"value\":\"q|Deny\",\"primary\":false}]")
    C.Check.str("phone other approval keeps session option", J.Json.show(J.Json.arr(S.Mob.ask.btns(2n, "q", "", ""))), "[{\"label\":\"Allow\",\"value\":\"q|accept\",\"primary\":true},{\"label\":\"Allow for session\",\"value\":\"q|session\",\"primary\":false},{\"label\":\"Deny\",\"value\":\"q|decline\",\"primary\":false}]")
