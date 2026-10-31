import Base
import ./check.bend as C
import ./design_context_test.bend as F
import ../src/core/client.bend as U
import ../src/core/model.bend as M
import ../src/core/json.bend as J

def withfork(+u: U.Ui) -> U.Ui:
  U.Ui.adopt(u, M.State.apply(U.Ui.state(u), M.ThreadCreated{"fork", "p", "Fork", M.Local{}, "", "", "claude", "", 2n}))

def made(+u: U.Ui) -> U.Ui:
  U.Made.reply(U.Made.demo(U.Ui.scratch(u, "@want.new"), "fork"), withfork(u))

def main() -> IO(Unit):
  +src = F.beta()
  +asked = U.Act.ui(U.Ui.fork(src))
  +done = made(asked)
  +child = U.Act.ui(U.Ui.act(done, "select", "fork"))
  do IO<Unit>:
    C.Check.str("fork keeps viewer open", U.Ui.info(done, "panel:fork"), "1")
    C.Check.str("fork keeps Split", U.Ui.info(done, "view.kind:fork"), "split")
    C.Check.str("fork keeps beta", U.Ui.info(done, "view.path:fork"), "/r/boards/beta/beta.kicad_pcb")
    C.Check.str("fork keeps its schematic root", U.Ui.info(done, "view.tree:fork"), "/r/boards/beta/beta.kicad_sch")
    C.Check.str("fork BOM uses its root before the board card", U.Rp.bom.root(child), "/r/boards/beta/beta.kicad_sch")
    C.Check.str("fork keeps Power", U.Ui.scratch(done, "@split.sheet:fork"), "/r/boards/beta/power.kicad_sch")
    C.Check.str("source keeps its sheet", U.Ui.scratch(done, "@split.sheet:t"), "/r/boards/beta/power.kicad_sch")
    C.Check.str("fork has independent board", U.Ui.info(U.Ui.view_put(child, "board", "/r/boards/alpha/alpha.kicad_pcb"), "view.path:t"), "/r/boards/beta/beta.kicad_pcb")
    C.Check.str("snapshot belongs to the press", U.Ui.info(made(U.Ui.view_put(asked, "board", "/r/boards/alpha/alpha.kicad_pcb")), "view.path:fork"), "/r/boards/beta/beta.kicad_pcb")
    C.Check.str("superseding creation drops the fork snapshot", U.Ui.scratch(U.Made.want(asked), "@want.viewer"), "")
    C.Check.str("stale replies do not copy the viewer", U.Ui.info(U.Made.reply(U.Made.demo("999999", "fork"), withfork(asked)), "view.kind:fork"), "")
    C.Check.str("worktree paths follow new root", U.Viewer.fork.path("/old/boards/beta/power.kicad_sch", "/old", "/new"), "/new/boards/beta/power.kicad_sch")
    C.Check.str("unrelated paths stay put", U.Viewer.fork.path("/else/file.step", "/old", "/new"), "/else/file.step")
