import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/client.bend as U
import ../src/core/botui.bend as BI
import ../src/core/subs.bend as SB
import ../src/core/i18n.bend as I
import ../src/web/view.bend as V
import ../src/web/dom.bend as D

def state() -> M.State:
  M.State.replay([
    M.ThreadCreated{"main", "", "Main chat", M.Local{}, "", "", "codex", "", 1n},
    M.ThreadCreated{"ordinary", "", "Ordinary chat", M.Local{}, "", "", "codex", "", 1n},
    M.ThreadCreated{"main-child", "", "Main child", M.Local{}, "", "", "codex", "", 2n},
    M.ThreadCreated{"ordinary-child", "", "Ordinary child", M.Local{}, "", "", "codex", "", 2n},
    M.ThreadLaunched{"main-child", "main", False{}, 3n},
    M.ThreadLaunched{"ordinary-child", "ordinary", False{}, 3n},
    M.BotSet{"b", "Bub", "main", 1n, "", 1n}, M.UberSet{"b", "uid", 2n}], M.State.empty())

def ui() -> U.Ui:
  U.Ui.info_set(U.Ui.far.st(U.Ui.init(10n), state()), "local.mailroom", "on")

law default_is_chat:
  for +u: U.Ui
  for +id: String
  {BI.BU.tab.dflt(u, id) == "chat" : String}

def default_is_chat(u, id):
  {==}

law ordinary_subagents_unchanged:
  {V.View.thread.subs(I.En{}, state(), Map.new(&2, String), "ordinary", 10n) ==
    V.View.subs.at(I.En{}, Map.new(&2, String), "ordinary", SB.Subs.rows(state(), Map.new(&2, String), "ordinary", 10n), state(), 10n) : List<&2, D.Node>}

def ordinary_subagents_unchanged():
  {==}

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("Bub click explicitly opens Chat", U.Ui.scratch(U.Act.ui(U.Uber.show(ui())), "@btab"), "chat")
    C.Check.str("Bub click selects its conversation", U.Ui.sel(U.Act.ui(U.Uber.show(ui()))), "main")
    C.Check.str("empty Bub tab is Chat", BI.BU.tab.of(U.Ui.scratch_set(ui(), "@btab", ""), "b"), "chat")
    C.Check.str("explicit Mailroom remains selected", BI.BU.tab.of(U.Ui.scratch_set(ui(), "@btab", "mail"), "b"), "mail")
    C.Check.str("explicit Status remains selected", BI.BU.tab.of(U.Ui.scratch_set(ui(), "@btab", "status"), "b"), "status")
    C.Check.str("default Chat has an active conversation timeline", Bool.show(U.Ui.timeline(U.Ui.scratch_set(U.Ui.scratch_set(U.Bots.sel.set(ui(), "main"), "@bot", "b"), "@btab", ""))), "True")
    C.Check.str("explicit Mailroom does not become a Chat timeline", Bool.show(U.Ui.timeline(U.Ui.scratch_set(U.Ui.scratch_set(U.Bots.sel.set(ui(), "main"), "@bot", "b"), "@btab", "mail"))), "False")
    C.Check.str("Bub has real subagent rows before presentation", Bool.show(Bool.not(List.is_empty(&2, SB.SubRow, SB.Subs.rows(state(), Map.new(&2, String), "main", 10n)))), "True")
    C.Check.str("Bub Chat omits only subagent panel", Nat.show(List.length(&2, D.Node, V.View.thread.subs(I.En{}, state(), Map.new(&2, String), "main", 10n))), "0")
    C.Check.str("ordinary Chat retains subagent panel", Bool.show(Bool.not(List.is_empty(&2, D.Node, V.View.thread.subs(I.En{}, state(), Map.new(&2, String), "ordinary", 10n)))), "True")
