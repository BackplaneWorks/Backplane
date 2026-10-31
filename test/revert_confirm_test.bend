import Base
import ../src/core/model.bend as M
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
    C.Check.str("a new message disarms", Bool.show(U.Ui.reverting(U.Ui.fold([M.MessagePosted{"t1", "new", M.User{}, "next", 1n}], armed()))), "False")
    C.Check.str("a new turn disarms", Bool.show(U.Ui.reverting(U.Ui.fold([M.TurnChanged{"t1", M.Running{}, 1n}], armed()))), "False")
    C.Check.str("a turn clears confirmation ownership", U.Ui.scratch(U.Ui.fold([M.TurnChanged{"t1", M.Running{}, 1n}], armed()), "@revert"), "")
    C.Check.str("closing Diff disarms", Bool.show(U.Ui.reverting(ui(U.Ui.toggle_panel(armed())))), "False")
    C.Check.str("reopening Diff stays disarmed", Bool.show(U.Ui.reverting(ui(U.Ui.toggle_panel(ui(U.Ui.toggle_panel(armed())))))), "False")
    C.Check.str("changing viewer disarms", Bool.show(U.Ui.reverting(U.Ui.view_pick(armed(), "board"))), "False")
    C.Check.str("a kept design detour stays disarmed", Bool.show(U.Ui.reverting(U.Ui.view_pick.p(U.Ui.view_pick.p(armed(), "board", "/board.kicad_pcb"), "diff", ""))), "False")
    C.Check.str("Cancel disarms", Bool.show(U.Ui.reverting(ui(U.Ui.act(armed(), "revert-no", "")))), "False")
    C.Check.str("another thread disarms", commands(U.Ui.revert(U.Ui.select.k(armed(), "t2"), "confirm")), "")
    C.Check.str("return to thread disarms", commands(U.Ui.revert(U.Ui.select.k(U.Ui.select.k(armed(), "t2"), "t1"), "confirm")), "")
    C.Check.str("the palette's Revert sends no revert", Bool.show(String.contains(commands(U.Ui.act(base(), "revert", "ask")), "git.revert")), "False")
    C.Check.str("the palette's Revert asks too", Bool.show(U.Ui.reverting(ui(U.Ui.act(base(), "revert", "ask")))), "True")
    C.Check.str("a phone's confirmed Revert (turn 0) is sent at once", Bool.show(String.contains(commands(U.Ui.act(base(), "revert", "0")), "git.revert")), "True")
    C.Check.str("a phone's Revert names turn 0", Bool.show(String.contains(commands(U.Ui.act(base(), "revert", "0")), "\"turn\":0")), "True")
    C.Check.str("an empty turn is turn 0", Bool.show(String.contains(commands(U.Ui.act(base(), "revert", "")), "\"turn\":0")), "True")
    C.Check.str("a phone's Revert leaves no armed question", Bool.show(U.Ui.reverting(ui(U.Ui.act(armed(), "revert", "0")))), "False")
