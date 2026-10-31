import Base
import ./check.bend as C
import ../src/core/client.bend as U

def ui(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}: u

def sent(cs: List<&2, U.Cmd>) -> String:
  match cs:
    case Nil{}: ""
    case U.Send{+s} <> tail: s ++ sent(tail)
    case _ <> tail: sent(tail)

def commands(a: U.Act) -> String:
  match a:
    case U.Act{_, cs}: sent(cs)

def base() -> U.Ui:
  U.Ui.select.k(U.Ui.init(0n), "t1")

def armed() -> U.Ui:
  ui(U.Ui.revert(base(), "0"))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("first click sends nothing", commands(U.Ui.revert(base(), "0")), "")
    C.Check.str("first click asks", Bool.show(U.Ui.reverting(armed())), "True")
    C.Check.str("unarmed confirm sends nothing", commands(U.Ui.revert(base(), "confirm")), "")
    C.Check.str("confirmation sends revert", Bool.show(String.contains(commands(U.Ui.revert(armed(), "confirm")), "git.revert")), "True")
    C.Check.str("confirmation disarms", Bool.show(U.Ui.reverting(ui(U.Ui.revert(armed(), "confirm")))), "False")
    C.Check.str("Cancel disarms", Bool.show(U.Ui.reverting(ui(U.Ui.act(armed(), "revert-no", "")))), "False")
    C.Check.str("another thread disarms", commands(U.Ui.revert(U.Ui.select.k(armed(), "t2"), "confirm")), "")
    C.Check.str("return to thread disarms", commands(U.Ui.revert(U.Ui.select.k(U.Ui.select.k(armed(), "t2"), "t1"), "confirm")), "")
