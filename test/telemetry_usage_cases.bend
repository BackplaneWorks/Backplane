import Base
import ../src/core/model.bend as M
import ../src/core/telemetry.bend as T

def UsageCases.consent() -> List<&2, M.Change>:
  [M.SettingSet{"telemetry.usage.enabled", "1", 86400n}, M.SettingSet{"telemetry.asked", "1", 86400n}, M.SettingSet{"telemetry.usage.asked", "1", 86400n}]

def UsageCases.turn(key: String, known: String) -> M.Change:
  M.TurnUsage{"t", "private-provider", "private-model", key, 100n, 20n, 30n, 40n, 10n, 999n, known, "private-basis", 86400n}

def UsageCases.mail() -> M.Change:
  M.MailPosted{"mail", "private-conversation", "private-sender", "private-recipient", "note", "private-ref", "fyi", "private-subject", 999n, 86400n}

def UsageCases.work() -> List<&2, M.Change>:
  [M.ThreadCreated{"t", "private-project", "private-title", M.Local{}, "private-branch", "/private/path", "private-provider", "private-model", 86400n},
   M.MessagePosted{"t", "m1", M.User{}, "private-prompt", 86400n}, M.MessagePosted{"t", "m2", M.Assistant{}, "private-reply", 86400n}, M.MessagePosted{"t", "m3", M.System{}, "private-system", 86400n},
   UsageCases.turn("k", "irwotc"), UsageCases.turn("k", "irwotc"), UsageCases.mail(), UsageCases.mail(),
   M.BotWoke{"private-bot", 0n, 86400n}, M.RoomPosted{"roommail", "private-room", "private-bot", "private-content", 0n, 86400n},
   M.UsageSet{"t", 999999n, 999999n, 86400n}, M.ThreadForked{"t", "source", 86400n},
   M.ThreadCreated{"peer~t", "peer~p", "private-title", M.Local{}, "", "", "provider", "model", 86400n},
   M.MessagePosted{"peer~t", "remote-message", M.User{}, "private-remote", 86400n}]

def UsageCases.log() -> List<&2, M.Change>:
  List.append(&2, M.Change, UsageCases.consent(), UsageCases.work())

def UsageCases.state() -> M.State:
  M.State.replay(UsageCases.log(), M.State.empty())

def UsageCases.view() -> T.TmView:
  T.TmView{True{}, True{}, "0", "0.12.0", "dev", "linux", "x64", "1970-01-02", 86400n}

def UsageCases.id() -> String:
  "7d0c2f6e-3b1a-4f7e-9a51-2c8e0d4b6f10"
