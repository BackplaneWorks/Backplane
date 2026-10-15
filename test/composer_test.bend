import Base
import ./check.bend as C
import ../src/core/client.bend as U
import ../src/core/json.bend as J

# The composer's `$skill` suggestions and effort chips, through the shared
# client state: typing `$` fetches the thread's skills once, the reply fills
# the list, Tab/Enter (the "skill" action) completes the first match.

def ui(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}:
      u

def omp_info() -> String:
  "{\"omp\":[\"m-think\",\"m-plain\"],\"about\":{\"omp\":{\"cli\":\"18.11.1\",\"labels\":{\"m-think\":\"Think\",\"m-plain\":\"Plain\"},\"efforts\":{\"default\":[],\"m-think\":[\"off\",\"auto\",\"low\",\"medium\",\"high\",\"xhigh\",\"max\"],\"m-plain\":[]}}}}"

def sent.go(cs: List<&2, U.Cmd>) -> String:
  match cs:
    case Nil{}:
      ""
    case U.Send{t} <> r:
      J.Json.get_str(Maybe.default(&2, J.Json, J.Json.parse(t), J.Null{}), "m", "") ++ " " ++ sent.go(r)
    case _ <> r:
      sent.go(r)

def sent(a: U.Act) -> String:
  match a:
    case U.Act{_, cs}:
      sent.go(cs)

def names(rs: List<&2, U.SkillRow>) -> String:
  match rs:
    case Nil{}:
      ""
    case U.SkillRow{n, _, _} <> t:
      n ++ "," ++ names(t)

def reply() -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse("{\"t\":\"reply\",\"id\":1,\"ok\":true,\"result\":{\"thread\":\"T\",\"skills\":[{\"name\":\"pcb-review\",\"description\":\"Review\"},{\"name\":\"parts\",\"description\":\"Find parts\"},{\"name\":\"docs\",\"description\":\"Docs\"}]}}"), J.Null{})

def main() -> IO(Unit):
  +u0 = ui(U.Ui.select(U.Ui.init(0n), "T"))
  +a1 = U.Ui.act(u0, "draft", "please run $p")
  +u2 = U.Ui.recv(ui(a1), reply())
  +a3 = U.Ui.act(u2, "draft", "please run $pa")
  +a4 = U.Ui.act(ui(a3), "skill", "")
  +a5 = U.Ui.act(u2, "skill", "$pcb-review")
  +a6 = U.Ui.act(ui(a4), "draft", "please run $parts now $q")
  do IO<Unit>:
    C.Check.str("dollar fetches skills", sent(a1), "skills.list ")
    C.Check.str("offers by prefix", names(U.Ui.skills(u2)), "$pcb-review,$parts,")
    C.Check.str("fetched once", sent(a3), "")
    C.Check.str("narrows", names(U.Ui.skills(ui(a3))), "$parts,")
    C.Check.str("tab completes first", U.Ui.draft_text(ui(a4)), "please run $parts ")
    C.Check.str("click completes that one", U.Ui.draft_text(ui(a5)), "please run $pcb-review ")
    C.Check.str("done offers none", names(U.Ui.skills(ui(a4))), "")
    C.Check.str("no match offers none", names(U.Ui.skills(ui(a6))), "")
    C.Check.str("effort chips claude", String.join(U.Effort.chips("", "claude", ""), ","), ",low,medium,high,xhigh,max")
    C.Check.str("effort chips grok", String.join(U.Effort.chips("", "grok", ""), ","), ",low,medium,high")
    C.Check.str("effort chips omp reasoning", String.join(U.Effort.chips(omp_info(), "omp", "m-think"), ","), ",off,auto,low,medium,high,xhigh,max")
    C.Check.str("effort chips omp plain", String.join(U.Effort.chips(omp_info(), "omp", "m-plain"), ","), "")
    C.Check.str("effort chips muse", String.join(U.Effort.chips("", "muse", ""), ","), ",minimal,low,medium,high,xhigh,max")
    C.Check.str("effort label", U.Effort.label("claude · opus", "high"), "claude · opus · high")
    C.Check.str("effort sends", sent(U.Ui.act(u0, "effort", "high")), "thread.effort ")
