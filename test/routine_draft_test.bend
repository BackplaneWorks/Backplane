import Base
import ./check.bend as C
import ../src/core/client.bend as U

def ui(a: U.Act) -> U.Ui:
  U.Act.ui(a)

def form(+when: String) -> U.Ui:
  U.Ui.scratch_set(U.Ui.scratch_set(U.Ui.scratch_set(U.Ui.init(0n), "@f.oname", "Bring-up"), "@f.ocron", when), "@f.oprompt", "check 日本語")

def focus(cs: List<&2, U.Cmd>) -> String:
  match cs:
    case Nil{}: ""
    case U.Focus{id} <> _: id
    case _ <> t: focus(t)

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("invalid schedule keeps name", U.Bots.field(ui(U.Bots.routine.save(form("bananas"))), "oname"), "Bring-up")
    C.Check.str("invalid schedule keeps prompt", U.Bots.field(ui(U.Bots.routine.save(form("bananas"))), "oprompt"), "check 日本語")
    C.Check.str("invalid schedule keeps schedule", U.Bots.field(ui(U.Bots.routine.save(form("bananas"))), "ocron"), "bananas")
    C.Check.str("empty schedule keeps prompt", U.Bots.field(ui(U.Bots.routine.save(form(""))), "oprompt"), "check 日本語")
    C.Check.str("invalid schedule focuses its field", focus(U.Act.cmds(U.Bots.routine.save(form("bananas")))), "routine-cron")
    C.Check.str("valid schedule clears accepted form", U.Bots.field(ui(U.Bots.routine.save(form("every 15m"))), "oprompt"), "")
