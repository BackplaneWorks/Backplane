import Base
import ./check.bend as C
import ../src/core/chat-pending.bend as P
import ../src/core/model.bend as M
import ../src/core/client.bend as U
import ../src/core/json.bend as J
import ../src/core/i18n.bend as I
import ../src/web/view.bend as V
import ../src/web/presentation.bend as W
import ../src/mobile/screen.bend as S

def ui(+turn: M.TurnState, bot: Bool) -> U.Ui:
  U.Side.with_signals(U.Ui.online(U.Ui.select.k(U.Ui.fold(List.append(&2, M.Change,
    [M.ThreadCreated{"t", "", "Chat", M.Local{}, "", "", "codex", "", 1n},
     M.MessagePosted{"t", "u", M.User{}, "Hello", 2n}, M.TurnChanged{"t", turn, 3n}],
    Bool.pick(List<&2, M.Change>, bot, [M.BotSet{"b", "Bub", "t", 1n, "", 1n}], Nil{})), U.Ui.init(4n)), "t"), True{}))

def web(u: U.Ui) -> String:
  J.Json.show(W.Node.write(V.View.live.u(u)))

def phone(u: U.Ui) -> String:
  J.Json.show(J.Json.get_obj(J.Json.get_obj(S.Mob.screen(u), "thread"), "entries"))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("web shows a Working message before deltas", Bool.show(String.contains(web(ui(M.Running{}, False{})), "Working")), "True")
    C.Check.str("phone shows a Working message before deltas", Bool.show(String.contains(phone(ui(M.Running{}, False{})), "Working")), "True")
    C.Check.str("bot chat shows the same pending message", Bool.show(String.contains(phone(ui(M.Running{}, True{})), "Working")), "True")
    C.Check.str("posted message appears whole on phone", Bool.show(String.contains(phone(U.Ui.fold([M.MessagePosted{"t", "answer", M.Assistant{}, "Complete reply", 5n}, M.TurnChanged{"t", M.Completed{}, 6n}], ui(M.Running{}, False{}))), "Complete reply")), "True")
    C.Check.str("posted completion clears pending phone message", Bool.show(String.contains(phone(U.Ui.fold([M.MessagePosted{"t", "answer", M.Assistant{}, "Complete reply", 5n}, M.TurnChanged{"t", M.Completed{}, 6n}], ui(M.Running{}, False{}))), "Working")), "False")
    C.Check.str("working visible before first delta", Bool.show(P.Pending.visible(1n)), "True")
    C.Check.str("waiting for work visible", Bool.show(P.Pending.visible(7n)), "True")
    C.Check.str("needs input has no working claim", Bool.show(P.Pending.visible(2n)), "False")
    C.Check.str("input alongside work has no working claim", Bool.show(P.Pending.visible(3n)), "False")
    C.Check.str("idle no bubble", Bool.show(P.Pending.visible(0n)), "False")
    C.Check.str("finished no bubble", Bool.show(P.Pending.visible(5n)), "False")
    C.Check.str("web response does not stream", web(U.Ui.delta(ui(M.Running{}, False{}), "t", "SECRET PARTIAL REPLY")), web(ui(M.Running{}, False{})))
    C.Check.str("bot response does not stream", web(U.Ui.delta(ui(M.Running{}, True{}), "t", "SECRET PARTIAL REPLY")), web(ui(M.Running{}, True{})))
    C.Check.str("phone response does not stream", phone(U.Ui.delta(ui(M.Running{}, False{}), "t", "SECRET PARTIAL REPLY")), phone(ui(M.Running{}, False{})))
    C.Check.str("web completion removes bubble", web(ui(M.Completed{}, False{})), J.Json.show(W.Node.write(V.View.one(Nil{}))))
    C.Check.str("web stop removes bubble", web(ui(M.Interrupted{}, False{})), web(ui(M.Completed{}, False{})))
    C.Check.str("web failure removes bubble", web(ui(M.Failed{}, False{})), web(ui(M.Completed{}, False{})))
    C.Check.str("canonical replies remain whole", P.Pending.text(I.En{}, "real-message", "Full completed reply"), "Full completed reply")
    C.Check.str("transient replies show only Working", P.Pending.text(I.En{}, "@live:t", "SECRET PARTIAL REPLY"), "Working")
    C.Check.str("localized pending bubble", P.Pending.label(I.Ja{}), "作業中")
