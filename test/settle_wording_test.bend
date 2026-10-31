import Base
import ./check.bend as C
import ../src/core/client.bend as U
import ../src/core/model.bend as M
import ../src/core/i18n.bend as I

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("default timer is visible", Bool.show(String.contains(U.Settings.settle.note(I.En{}, M.State.empty()), "30 idle minutes")), "True")
    C.Check.str("configured timer is visible", Bool.show(String.contains(U.Settings.settle.note.raw(I.En{}, "1"), "1 idle minute")), "True")
    C.Check.str("disabled timer says off", Bool.show(String.contains(U.Settings.settle.note.raw(I.En{}, "0"), "timer is off")), "True")
