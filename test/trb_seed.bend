import Base
import ../src/core/model.bend as M
import ../src/core/client.bend as U
import ../src/core/ruledata.bend as R
import ../src/core/tbdata.bend as T

# Shared, fictional screenshot fixture. No hub or live user data is read.
def now() -> Nat:
  1791849600n

def titles() -> List<&2, String>:
  ["Route sensor connector", "Review the power budget", "Prepare Rev C assembly", "Waiting for test fixtures", "Get connector drawing", "Verify wake timing", "Write factory test notes", "Review enclosure fit", "Order pilot boards", "Bring up I2C bus", "Capture idle current", "Old mounting option"]

def statuses() -> List<&2, String>:
  ["doing", "doing", "doing", "blocked", "blocked", "todo", "todo", "todo", "todo", "done", "done", "dropped"]

def tasks(xs: List<&2, String>, +i: Nat) -> List<&2, M.Change>:
  match xs:
    case Nil{}:
      Nil{}
    case title <> tail:
      +id = {"i" ++ Nat.show(i) : String}
      +status = {Maybe.default(&2, String, List.get(&2, String, statuses(), i), "todo") : String}
      +owner = {Bool.pick(String, Nat.is_eq(Nat.mod(i, 3n), 0n), "", Bool.pick(String, Nat.is_eq(Nat.mod(i, 3n), 1n), "b1", "b2")) : String}
      +due = {Bool.pick(String, Nat.is_eq(i, 3n), "2026-10-12", Bool.pick(String, Nat.is_eq(Nat.mod(i, 2n), 0n), "2026-10-15", "2026-10-28")) : String}
      +group = {Maybe.default(&2, String, List.get(&2, String, ["Hardware", "Firmware", "Pilot"], Nat.mod(i, 3n)), "") : String}
      M.SettingSet{"board." ++ id, T.Tb.value(T.TbItem{id, title, status, owner, "p1", due, group, ["https://example.com/review", "/tmp/power/notes.txt"],
        "Check the latest drawing and record the measurements before closing this task.", Nat.add(now(), i), Nat.add(now(), i), "person"}), now()} <> tasks(tail, 1n+i)

def rules() -> List<&2, M.Change>:
  [M.SettingSet{"rule.r1", R.Rules.value(R.Rule{"r1", "Ask before spending money", "", "", "purchase*", True{}, "sudo", True{}, 1n, "person"}), now()},
   M.SettingSet{"rule.r2", R.Rules.value(R.Rule{"r2", "Review connector pin numbering against the drawing", "b2", "p1", "", False{}, "", True{}, 2n, "person"}), now()},
   M.SettingSet{"rule.r3", R.Rules.value(R.Rule{"r3", "Keep pilot build notes with the project", "", "p1", "", False{}, "", False{}, 3n, "person"}), now()}]

def log() -> List<&2, M.Change>:
  List.concat(&2, M.Change, [[M.ProjectCreated{"p1", "Power board", "/tmp/power", now()},
    M.ThreadCreated{"bt1", "", "Miso", M.Local{}, "", "", "claude", "", now()}, M.BotSet{"b1", "Miso", "bt1", 0n, "Chief of staff", now()},
    M.ThreadCreated{"bt2", "", "Tofu", M.Local{}, "", "", "claude", "", now()}, M.BotSet{"b2", "Tofu", "bt2", 20n, "Hardware reviewer", now()},
    M.ThreadCreated{"t1", "p1", "Rev C", M.Local{}, "", "", "claude", "", now()}, M.TurnChanged{"t1", M.Running{}, now()},
    M.UberSet{"b1", "u1-x", now()}, M.RoomSet{"r1", "builds", "b1\nb2", now()},
    M.SettingSet{"onboard.done", "1", now()}, M.SettingSet{"onboard.uber", "done", now()}, M.SettingSet{"restart.continue", "off", now()}], rules(), tasks(titles(), 0n)])

def ui(+theme: String) -> U.Ui:
  U.Ui.online(U.Ui.far.st(U.Ui.init(now()), M.State.replay(M.SettingSet{"ui.theme", theme, now()} <> log(), M.State.empty())), True{})

def act(+u: U.Ui, +action: String, +value: String) -> U.Ui:
  U.Act.ui(U.Ui.act(u, action, value))

def settings(+u: U.Ui, form: Bool) -> U.Ui:
  Bool.pick(U.Ui, form, act(act(u, "flag", "settings"), "rule-edit", "r1"), act(u, "flag", "settings"))

def board(+u: U.Ui) -> U.Ui:
  act(u, "tb-open", "")

def status(+u: U.Ui) -> U.Ui:
  act(u, "uber", "")
