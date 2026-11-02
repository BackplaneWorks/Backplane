import Base
import ./check.bend as C
import ./chat_sidebar_projection_test.bend as F
import ./chat_sidebar_projection_display_test.bend as PAGE
import ./allowance_ui_test.bend as Q
import ../src/core/client.bend as U
import ../src/core/model.bend as M
import ../src/core/json.bend as J
import ../src/web/sidebar-slot.bend as SLOT

def updated(+u: U.Ui) -> U.Ui:
  U.Ui.adopt(u, M.State.apply(U.Ui.state(u), M.SettingSet{"allowance.codex", Q.snapshot(), 10n}))

def sidebar(u: U.Ui) -> String:
  J.Json.show(SLOT.Slot.sidebar(J.Json.get_obj(PAGE.frame(u, J.Null{}), "shown")))

def main() -> IO(Unit):
  do IO<Unit>:
    +u : U.Ui = F.ordinary(F.state(), 10n)
    C.Check.str("quota setting leaves actual complete sidebar byte exact", sidebar(updated(u)), sidebar(u))
    C.Check.str("quota setting frame with retained sidebar equals full frame", PAGE.exact(u, updated(u), 1n, 2n), "same")
    C.Check.str("allowance opened frame with retained sidebar equals full frame", PAGE.exact(u, U.Act.ui(U.Ui.act(u, "allowance-open", "")), 1n, 1n), "same")
    C.Check.str("allowance close frame with retained sidebar equals full frame", PAGE.exact(U.Ui.flag_on(u, "allowance"), u, 1n, 1n), "same")
    C.Check.str("allowance update with IME draft retains exact full output", PAGE.exact(u, U.Ui.scratch_set(updated(u), "self", "未確定 👩🏽‍💻 draft"), 1n, 2n), "same")
