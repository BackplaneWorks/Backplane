import Base
import ./check.bend as C
import ../src/core/term-scroll.bend as T
import ../src/core/vt.bend as V
import ../src/core/utf8.bend as U8
import ../src/core/client.bend as U
import ../src/web/workspace.bend as W
import ../src/server/thin-actor.bend as A
import ../src/core/json.bend as J

def ui() -> U.Ui:
  U.Ui.term_set(U.Ui.scratch_set(U.Ui.flag_on(U.Ui.init(0n), "term"), "@terminal.owner", "t"), "t", V.Vt.feed(V.Vt.new(20, 5), U8.Utf8.encode("1\r\n2\r\n3\r\n4\r\n5\r\n6\r\n7\r\n")))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("up reaches oldest kept row", Nat.show(T.TermScroll.move(2040n, 2048n, True{}, 19n)), "2048")
    C.Check.str("down stops at live screen", Nat.show(T.TermScroll.move(3n, 2048n, False{}, 19n)), "0")
    +up : U.Act = W.WS.term.key(ui(), "PageUp\t1")
    C.Check.str("shift page up sends no shell input", Nat.show(List.length(&2, U.Cmd, U.Act.cmds(up))), "0")
    C.Check.str("shift page up reaches retained history", Nat.show(W.WS.term.scroll.at(U.Act.ui(up))), U32.show(V.Vt.history(U.Ui.term(ui(), "t"))))
    C.Check.str("modifier press keeps scrollback", Nat.show(W.WS.term.scroll.at(U.Act.ui(W.WS.term.key(U.Act.ui(up), "Shift\t1")))), Nat.show(W.WS.term.scroll.at(U.Act.ui(up))))
    C.Check.str("shift page down returns live", Nat.show(W.WS.term.scroll.at(U.Act.ui(W.WS.term.key(U.Act.ui(up), "PageDown\t1")))), "0")
    C.Check.str("wheel up moves viewport", Bool.show(Nat.is_gt(W.WS.term.scroll.at(U.Act.ui(W.WS.term.wheel(ui(), "-100"))), 0n)), "True")
    C.Check.str("typing returns to live screen", Nat.show(W.WS.term.scroll.at(U.Act.ui(W.WS.term.key(U.Act.ui(up), "a\t0")))), "0")
    C.Check.str("viewport key needs a picture", Bool.show(A.TerminalInput.request(True{}, J.Json.obj([J.KV{"terminalInput", J.Flag{True{}}}, J.KV{"action", J.Str{"term-key"}}, J.KV{"value", J.Str{"PageUp\t1"}}]))), "False")
