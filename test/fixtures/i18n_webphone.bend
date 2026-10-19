import Base
import ../../src/core/client.bend as U
import ../../src/core/model.bend as M

# Synthetic idle data only. No send or provider process is needed.
def Fixture.changes() -> List<&2, M.Change>:
  [M.ProjectCreated{"p43", "試作基板", "/tmp/backplane-i18n-fixture", 100n},
    M.ThreadCreated{"t43", "p43", "New thread", M.Local{}, "", "", "claude", "", 100n},
    M.MessagePosted{"t43", "m43", M.User{}, "接続を確認してください。", 101n},
    M.MessagePosted{"t43", "a43", M.Assistant{}, "配線を確認しました。", 102n},
    M.ThreadCreated{"bt43", "p43", "ミケ", M.Local{}, "", "", "claude", "", 100n},
    M.BotSet{"b43", "ミケ", "bt43", 0n, "設計を手伝うボット", 100n},
    M.RoomSet{"r43", "設計", "b43", 100n},
    M.SettingSet{"onboard.done", "1", 100n},
    M.SettingSet{"restart.continue", "off", 100n}]

def Fixture.go(u: U.Ui, action: String, value: String) -> U.Ui:
  U.Act.ui(U.Ui.act(u, action, value))

def Fixture.ui(+tag: String) -> U.Ui:
  U.Lang.apply(U.Ui.scratch_set(U.Ui.fold(Fixture.changes(), U.Ui.init(200n)), "@lang.preview", "1"), tag, tag)

def Fixture.thread(tag: String) -> U.Ui:
  Fixture.go(Fixture.ui(tag), "select", "t43")

def Fixture.settings(tag: String) -> U.Ui:
  Fixture.go(Fixture.thread(tag), "flag", "settings")

def Fixture.menu(tag: String) -> U.Ui:
  Fixture.go(Fixture.thread(tag), "row-menu", "t43")

def Fixture.bot(tag: String) -> U.Ui:
  Fixture.go(Fixture.thread(tag), "bot", "b43")
