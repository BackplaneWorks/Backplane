import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/hub.bend as H
import ../src/core/fold.bend as FO

# A restart is one quiet row a turn: when the restart cut it, and which
# ask went unanswered (Rec.ask.close's row; the turn adds none of its own,
# Rec.told). Backplane's continue message is the "Continuing" row, so the
# hub's own row after it is left out (FO.Fold.shown); restart rows are
# never red (FO.Tool.tone).

def rows(cs: List<&2, M.Change>) -> String:
  match cs:
    case Nil{}: ""
    case M.ActivityLogged{_, _, _, "restart", +sum, _} <> t: sum ++ "|" ++ rows(t)
    case _ <> t: rows(t)

def recovered(+s: M.State) -> String:
  rows(H.Queue.changes(H.Hub.recover(s, 10n)))

def asked(+cs: List<&2, M.Change>) -> M.State:
  M.State.replay(List.append(&2, M.Change, H.Rec.demo.base(), List.append(&2, M.Change, [M.TurnChanged{"t1", M.Running{}, 2n}], cs)), M.State.empty())

def ids(es: List<&2, M.Entry>) -> String:
  match es:
    case Nil{}: ""
    case M.Msg{+id, _, _, _, _} <> t: id ++ "|" ++ ids(t)
    case M.Act{+id, _, _, _, _, _} <> t: id ++ "|" ++ ids(t)

def carry() -> M.Entry:
  M.Msg{"c1", "t1", M.User{}, M.Carry.text() ++ " " ++ H.Carry.cut(), 10n}

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("a cut turn: one row, when", recovered(asked(Nil{})), "Backplane restarted at 00:00 during this turn.|")
    C.Check.str("an open question: one row quoting it", recovered(asked([M.AskOpened{"a1", "t1", "input", "AskUserQuestion", "Which connector for ground?\nUSB-C: small", "USB-C\nBarrel", 3n}])),
      "Backplane restarted at 00:00 during this turn. Its question was not answered: “Which connector for ground?”|")
    C.Check.str("an open approval: one row naming it", recovered(H.Rec.demo(M.Running{})), "Backplane restarted at 00:00 during this turn. Its request to use Bash was not answered: “ls”|")
    C.Check.str("an open plan", recovered(asked([M.AskOpened{"a1", "t1", "plan", "ExitPlanMode", "1. do it", "", 3n}])), "Backplane restarted at 00:00 during this turn. Its plan was not answered.|")
    C.Check.str("an idle thread with an open ask: its row alone", recovered(H.Rec.demo(M.Completed{})), "Backplane restarted at 00:00 during this turn. Its request to use Bash was not answered: “ls”|")
    C.Check.str("in the person's zone", recovered(asked([M.SettingSet{"person.tz", "Europe/Athens", 1n}, M.SettingSet{"tz.rule.Europe/Athens", "EET-2EEST,M3.5.0/3,M10.5.0/4", 1n}])), "Backplane restarted at 02:00 during this turn.|")
    C.Check.str("the continue message stands for the hub's Continuing row", ids(FO.Fold.shown([carry(), M.Act{"r1", "t1", "info", "restart", "Continuing after the restart", 10n}, M.Msg{"m1", "t1", M.Assistant{}, "again.0", 11n}])), "c1|m1|")
    C.Check.str("a Continuing row without it stays", ids(FO.Fold.shown([M.Msg{"u1", "t1", M.User{}, "go on", 9n}, M.Act{"r1", "t1", "info", "restart", "Continuing after the restart", 10n}])), "u1|r1|")
    C.Check.str("a person's message is not the continue message", Bool.pick(String, FO.Restart.told("Backplane is great"), "yes", "no"), "no")
    C.Check.str("the continue message is", Bool.pick(String, FO.Restart.told(M.Carry.text() ++ " " ++ H.Carry.cut()), "yes", "no"), "yes")
    C.Check.str("a restart row is never red", FO.Tool.tone("error", "restart"), "info restart")
    C.Check.str("another error stays red", FO.Tool.tone("error", "result"), "error")
