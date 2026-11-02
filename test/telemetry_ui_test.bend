import Base
import ./check.bend as C
import ../src/server/telemetry-ui.bend as UI
import ../src/server/telemetry.bend as TI
import ../src/core/model.bend as M

def cfg() -> List<&2, TI.TmCfg>:
  TI.Tio.plat.cfg(("linux", "x64"), True{}, False{}, "", "", "", 0n, "release")

def shared() -> M.State:
  M.State.replay([M.SettingSet{"telemetry.asked", "1", 1n}, M.SettingSet{"telemetry.usage.asked", "1", 1n}], M.State.empty())

def marked() -> List<&2, TI.TmCfg>:
  UI.Human.mark(True{}, True{}, shared(), "view.action", "fold", cfg(), 86400n)

def action(+method: String, +name: String) -> String:
  TI.Tio.human.day(UI.Human.mark(True{}, True{}, shared(), method, name, cfg(), 86400n))

def check(name: String, b: Bool) -> IO(Unit):
  C.Check.str(name, Bool.pick(String, b, "yes", "no"), "yes")

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("successful authenticated send marks actual UTC action day", action("view.action", "send"), "1970-01-02")
    C.Check.str("alternate send accepted", action("view.action", "send-alt"), "1970-01-02")
    C.Check.str("local fold accepted", action("view.action", "fold"), "1970-01-02")
    C.Check.str("subscription open accepted", action("view.action", "subs-open"), "1970-01-02")
    C.Check.str("subscription shut accepted", action("view.action", "subs-shut"), "1970-01-02")
    C.Check.str("bot tab interaction accepted", action("view.action", "bot-tab"), "1970-01-02")
    C.Check.str("unauthenticated request never marks", TI.Tio.human.day(UI.Human.mark(False{}, True{}, shared(), "view.action", "send", cfg(), 86400n)), "")
    C.Check.str("failed reply admission never marks", TI.Tio.human.day(UI.Human.mark(True{}, False{}, shared(), "view.action", "send", cfg(), 86400n)), "")
    C.Check.str("unasked sharing never marks", TI.Tio.human.day(UI.Human.mark(True{}, True{}, M.State.empty(), "view.action", "send", cfg(), 86400n)), "")
    C.Check.str("opted out sharing never marks", TI.Tio.human.day(UI.Human.mark(True{}, True{}, M.State.apply(shared(), M.SettingSet{"telemetry.share", "off", 2n}), "view.action", "send", cfg(), 86400n)), "")
    C.Check.str("disabled worker never marks", TI.Tio.human.day(UI.Human.mark(True{}, True{}, shared(), "view.action", "send", TI.Tio.empty(False{}), 86400n)), "")
    C.Check.str("automatic open never marks", action("view.open", "send"), "")
    C.Check.str("automatic refresh never marks", action("view.update", "send"), "")
    C.Check.str("history page never marks", action("view.page", "send"), "")
    C.Check.str("search never marks", action("view.search", "send"), "")
    C.Check.str("message continuation never marks", action("view.message", "send"), "")
    C.Check.str("cancel never marks", action("view.cancel", "send"), "")
    C.Check.str("MCP send never marks", action("mcp.call", "send"), "")
    C.Check.str("provider completion never marks", action("agent.done", "send"), "")
    C.Check.str("unlisted UI action never marks", action("view.action", "pulse"), "")
    C.Check.str("empty UI action never marks", action("view.action", ""), "")
    C.Check.str("new config still has no telemetry id", TI.Tio.id(marked()), "")
    C.Check.str("share clears pending action before id exists", TI.Tio.human.day(UI.Privacy.committed([M.SettingSet{"telemetry.share", "off", 2n}], marked())), "")
    C.Check.str("asked clears pending action before id exists", TI.Tio.human.day(UI.Privacy.committed([M.SettingSet{"telemetry.asked", "0", 2n}], marked())), "")
    C.Check.str("reset clears pending action before id exists", TI.Tio.human.day(UI.Privacy.committed([M.SettingSet{"telemetry.reset", "1", 2n}], marked())), "")
    C.Check.str("same-batch off then on cannot revive pending action", TI.Tio.human.day(UI.Privacy.committed([M.SettingSet{"telemetry.share", "off", 2n}, M.SettingSet{"telemetry.share", "on", 3n}], marked())), "")
    C.Check.str("same setting write conservatively clears pending action", TI.Tio.human.day(UI.Privacy.committed([M.SettingSet{"telemetry.asked", "1", 2n}], marked())), "")
    C.Check.str("unrelated setting retains action", TI.Tio.human.day(UI.Privacy.committed([M.SettingSet{"theme", "dark", 2n}], marked())), "1970-01-02")
    C.Check.str("empty committed batch retains action", TI.Tio.human.day(UI.Privacy.committed(Nil{}, marked())), "1970-01-02")
    C.Check.str("nonprivacy event retains action", TI.Tio.human.day(UI.Privacy.committed([M.ThreadCreated{"t", "", "Fixture", M.Local{}, "", "", "codex", "", 2n}], marked())), "1970-01-02")
    C.Check.str("cached id fixture is valid before clear", TI.Tio.id(TI.Tio.done(TI.Tio.busy(marked(), 1n), 1n, "7d0c2f6e-3b1a-4f7e-9a51-2c8e0d4b6f10")), "7d0c2f6e-3b1a-4f7e-9a51-2c8e0d4b6f10")
    C.Check.str("clear preserves cached id", TI.Tio.id(UI.Privacy.committed([M.SettingSet{"telemetry.reset", "1", 2n}], TI.Tio.done(TI.Tio.busy(marked(), 1n), 1n, "7d0c2f6e-3b1a-4f7e-9a51-2c8e0d4b6f10"))), "7d0c2f6e-3b1a-4f7e-9a51-2c8e0d4b6f10")
    C.Check.str("later eligible action may mark after privacy clear", TI.Tio.human.day(UI.Human.mark(True{}, True{}, shared(), "view.action", "fold", UI.Privacy.committed([M.SettingSet{"telemetry.share", "on", 2n}], marked()), 172800n)), "1970-01-03")
    check("privacy names require exact key", Bool.not(UI.Privacy.key("telemetry.share.extra")))
