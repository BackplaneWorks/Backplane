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
  ui(U.Ui.revert(base(), "ask"))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("first click sends nothing", commands(U.Ui.revert(base(), "ask")), "")
    C.Check.str("first click asks", Bool.show(U.Ui.reverting(armed())), "True")
    C.Check.str("unarmed confirm sends nothing", commands(U.Ui.revert(base(), "confirm")), "")
    C.Check.str("confirmation sends revert", Bool.show(String.contains(commands(U.Ui.revert(armed(), "confirm")), "git.revert")), "True")
    C.Check.str("confirmation disarms", Bool.show(U.Ui.reverting(ui(U.Ui.revert(armed(), "confirm")))), "False")
    C.Check.str("Cancel disarms", Bool.show(U.Ui.reverting(ui(U.Ui.act(armed(), "revert-no", "")))), "False")
    C.Check.str("another thread disarms", commands(U.Ui.revert(U.Ui.select.k(armed(), "t2"), "confirm")), "")
    C.Check.str("return to thread disarms", commands(U.Ui.revert(U.Ui.select.k(U.Ui.select.k(armed(), "t2"), "t1"), "confirm")), "")
    C.Check.str("the palette's Revert sends no revert", Bool.show(String.contains(commands(U.Ui.act(base(), "revert", "ask")), "git.revert")), "False")
    C.Check.str("the palette's Revert asks too", Bool.show(U.Ui.reverting(ui(U.Ui.act(base(), "revert", "ask")))), "True")
    C.Check.str("a phone's confirmed Revert (turn 0) is sent at once", Bool.show(String.contains(commands(U.Ui.act(base(), "revert", "0")), "git.revert")), "True")
    C.Check.str("a phone's Revert names turn 0", Bool.show(String.contains(commands(U.Ui.act(base(), "revert", "0")), "\"turn\":0")), "True")
    C.Check.str("an empty turn is turn 0", Bool.show(String.contains(commands(U.Ui.act(base(), "revert", "")), "\"turn\":0")), "True")
    C.Check.str("a phone's Revert leaves no armed question", Bool.show(U.Ui.reverting(ui(U.Ui.act(armed(), "revert", "0")))), "False")
