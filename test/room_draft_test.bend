import Base
import ./check.bend as C
import ../src/core/client.bend as U

def ui(a: U.Act) -> U.Ui:
  U.Act.ui(a)

def room(u: U.Ui, id: String) -> U.Ui:
  U.Ui.scratch_set(u, "@room", id)

def typed(+u: U.Ui, +text: String) -> U.Ui:
  U.Bots.bfield(u, "rpost\u{1f}" ++ text)

def first() -> U.Ui:
  typed(room(U.Ui.init(0n), "one"), "first 日本語\nsecond line")

def both() -> U.Ui:
  typed(room(first(), "two"), "second room")

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("returning to a room keeps its exact draft", U.Bots.field(room(both(), "one"), "rpost"), "first 日本語\nsecond line")
    C.Check.str("each room has its own draft", U.Bots.field(both(), "rpost"), "second room")
    C.Check.str("posting clears only that room", U.Bots.field(ui(U.Bots.room.post(both())), "rpost"), "")
    C.Check.str("another room's draft survives posting", U.Bots.field(room(ui(U.Bots.room.post(both())), "one"), "rpost"), "first 日本語\nsecond line")
    C.Check.str("ordinary form fields keep their keys", U.Bots.field(typed(U.Ui.scratch_set(U.Ui.init(0n), "@f.oname", "name"), "post"), "oname"), "name")
