import Base
import ./check.bend as C
import ../src/core/client.bend as U
import ../src/core/model.bend as M
import ../src/core/presentation.bend as P
import ../src/core/json.bend as J
import ../src/core/hist.bend as H
import ../src/server/review.bend as R

def base() -> U.Ui:
  +st = M.State.replay([M.ProjectCreated{"p", "P", "/r", 1n}, M.ThreadCreated{"t", "p", "T", M.Local{}, "", "", "claude", "", 1n}], M.State.empty())
  +u = U.Act.ui(U.Ui.act(U.Ui.adopt(U.Ui.init(0n), st), "select", "t"))
  U.Ui.info_set(U.Ui.info_set(u, "fab.boards:t", "boards/alpha/alpha.kicad_pcb\nboards/beta/beta.kicad_pcb"), "fab.board:t", "boards/alpha/alpha.kicad_pcb")

def beta() -> U.Ui:
  U.Sch.put(True{}, U.Act.ui(U.Bd.pick(U.Ui.view_put(base(), "split", ""), "boards/beta/beta.kicad_pcb")), "/r/boards/beta/power.kicad_sch")

def review() -> U.Ui:
  U.Act.ui(U.Rp.open(beta(), "bom"))

def sent(cs: List<&2, U.Cmd>) -> String:
  match cs:
    case Nil{}: ""
    case U.Send{x} <> tail: x ++ sent(tail)
    case _ <> tail: sent(tail)

def late_boards() -> U.Ui:
  +early = U.Act.ui(U.Rp.ask.bom(U.Ui.info_set(U.Ui.info_set(review(), "view.tree:t", ""), "fab.boards:t", "")))
  U.Ui.scratch_set(U.Ui.info_set(early, "fab.boards:t", U.Ui.info(base(), "fab.boards:t")), "@rp.busy:t", "")

def pending_board() -> U.Ui:
  U.Ui.info_set(U.Sch.put(False{}, beta(), "/r/boards/beta/power.kicad_sch"), "fab.boards:t", "")

def custom_single() -> U.Ui:
  U.Ui.view_put(U.Ui.info_set(U.Ui.info_set(base(), "fab.boards:t", "carrier.kicad_pcb"), "fab.board:t", "carrier.kicad_pcb"), "board", "/r/carrier.kicad_pcb")

def main() -> IO(Unit):
  +u = review()
  +restored = U.Viewer.restore(base(), "t", U.Viewer.keep(u))
  +handoff = P.Session.adopt(base(), P.Session.pack(u))
  do IO<Unit>:
    C.Check.str("Review keeps the chosen board", U.View.kind.path(u, "split"), "/r/boards/beta/beta.kicad_pcb")
    C.Check.str("Review keeps the chosen sheet", U.View.kind.path(u, "schematic"), "/r/boards/beta/power.kicad_sch")
    C.Check.str("BOM names its board", U.Rp.bom.title(u), "BOM · Working copy · beta")
    C.Check.str("BOM reads the selected board", Bool.show(String.contains(sent(U.Act.cmds(U.Rp.ask.bom(u))), "boards/beta/beta.kicad_sch")), "True")
    C.Check.str("late board list corrects the initial BOM", Bool.show(String.contains(sent(U.Act.cmds(U.Rp.sync(U.Act{late_boards(), Nil{}}))), "boards/beta/beta.kicad_sch")), "True")
    C.Check.str("matching BOM does not repeat its read", sent(U.Act.cmds(U.Rp.sync(U.Act{U.Ui.scratch_set(u, "@rp.busy:t", ""), Nil{}}))), "")
    C.Check.str("sheet keeps its explicit board anchor", U.Ui.view_get(pending_board(), "view.design"), "/r/boards/beta/beta.kicad_pcb")
    C.Check.str("Split keeps beta before its board card arrives", U.View.kind.path(pending_board(), "split"), "/r/boards/beta/beta.kicad_pcb")
    C.Check.str("Schematic keeps Power before its board card arrives", U.View.kind.path(pending_board(), "schematic"), "/r/boards/beta/power.kicad_sch")
    C.Check.str("BOM reads beta before its board card arrives", U.Rp.bom.root(pending_board()), "/r/boards/beta/beta.kicad_sch")
    C.Check.str("drawing hierarchy reads beta before its card arrives", U.Bd.tree(pending_board(), U.Ui.view_get(pending_board(), "view.path")), "/r/boards/beta/beta.kicad_sch")
    C.Check.str("drawing root stays inside the thread folder", U.Bd.tree.saved("/r", "/elsewhere/beta.kicad_sch"), "")
    C.Check.str("beta has an identity before its board card arrives", U.Bd.id(pending_board()), "boards/beta/beta.kicad_pcb")
    C.Check.str("picking a board in Schematic replaces the anchor", U.Ui.view_get(U.Act.ui(U.Bd.pick(pending_board(), "boards/alpha/alpha.kicad_pcb")), "view.design"), "/r/boards/alpha/alpha.kicad_pcb")
    C.Check.str("single-board schematic stays project-defined", U.View.kind.path(custom_single(), "schematic"), "")
    C.Check.str("single-board BOM stays project-defined", U.Rp.bom.root(custom_single()), "")
    C.Check.str("unloaded single-board BOM stays project-defined", U.Rp.bom.root(U.Ui.info_set(custom_single(), "fab.boards:t", "")), "")
    C.Check.str("shared sheet survives a report detour", U.View.kind.path(U.Ui.view_put(U.Sch.put(False{}, beta(), "/r/shared/power.kicad_sch"), "review", "bom"), "schematic"), "/r/shared/power.kicad_sch")
    C.Check.str("shared sheet keeps the chosen BOM root", U.Rp.bom.root(U.Sch.put(False{}, beta(), "/r/shared/power.kicad_sch")), "/r/boards/beta/beta.kicad_sch")
    C.Check.str("a version of the same board keeps its schematic root", U.Ui.view_get(U.Ui.view_put(beta(), "board", H.Hist.git("/r/.git", "c2", "/r", "boards/beta/beta.kicad_pcb")), "view.tree"), "/r/boards/beta/beta.kicad_sch")
    C.Check.str("drawing survives reload", U.View.kind.path(restored, "split"), "/r/boards/beta/beta.kicad_pcb")
    C.Check.str("sheet survives reload", U.View.kind.path(restored, "schematic"), "/r/boards/beta/power.kicad_sch")
    C.Check.str("schematic root survives reload", U.Ui.view_get(restored, "view.tree"), "/r/boards/beta/beta.kicad_sch")
    C.Check.str("drawing crosses presentation handoff", U.View.kind.path(handoff, "split"), "/r/boards/beta/beta.kicad_pcb")
    C.Check.str("sheet crosses presentation handoff", U.View.kind.path(handoff, "schematic"), "/r/boards/beta/power.kicad_sch")
    C.Check.str("other threads keep independent context", U.Ui.info(u, "view.design:x"), "")
    C.Check.str("board choice survives Files too", U.View.kind.path(U.Ui.view_put(beta(), "files", "notes"), "board"), "/r/boards/beta/beta.kicad_pcb")
    C.Check.str("reload wire keeps the requested board", J.Json.get_str(J.Json.get_obj(P.Session.viewer_input(u, J.Json.obj([J.KV{"sel", J.Str{"t"}}, J.KV{"rid", J.Num{"17"}}, J.KV{"info", J.Json.obj(Nil{})}])), "info"), "view.design:t", ""), "/r/boards/beta/beta.kicad_pcb")
    C.Check.str("reload wire keeps the requested sheet", J.Json.get_str(J.Json.get_obj(P.Session.viewer_input(u, J.Json.obj([J.KV{"sel", J.Str{"t"}}, J.KV{"info", J.Json.obj(Nil{})}])), "info"), "view.sheet:t", ""), "/r/boards/beta/power.kicad_sch")
    C.Check.str("reload wire keeps the selected schematic root", J.Json.get_str(J.Json.get_obj(P.Session.viewer_input(u, J.Json.obj([J.KV{"sel", J.Str{"t"}}, J.KV{"info", J.Json.obj(Nil{})}])), "info"), "view.tree:t", ""), "/r/boards/beta/beta.kicad_sch")
    C.Check.str("wire context never copies another thread", J.Json.get_str(J.Json.get_obj(P.Session.viewer_input(u, J.Json.obj([J.KV{"sel", J.Str{"other"}}, J.KV{"info", J.Json.obj(Nil{})}])), "info"), "view.design:t", "none"), "none")
    C.Check.str("wire context bounds saved paths", J.Json.get_str(J.Json.get_obj(P.Session.viewer_input(U.Ui.info_set(u, "view.design:t", String.repeat("x", 1025n)), J.Json.obj([J.KV{"sel", J.Str{"t"}}, J.KV{"info", J.Json.obj(Nil{})}])), "info"), "view.design:t", "none"), "none")
    C.Check.str("wire context preserves request identity", Nat.show(J.Json.get_nat(P.Session.viewer_input(u, J.Json.obj([J.KV{"sel", J.Str{"t"}}, J.KV{"rid", J.Num{"17"}}])), "rid", 0n)), "17")
    C.Check.str("pending board choice beats the last accepted UI", J.Json.get_str(J.Json.get_obj(P.Session.viewer_input.current(u, J.Json.obj([J.KV{"view.design:t", J.Str{"/r/boards/alpha/alpha.kicad_pcb"}}]), J.Json.obj([J.KV{"sel", J.Str{"t"}}])), "info"), "view.design:t", ""), "/r/boards/alpha/alpha.kicad_pcb")
    C.Check.str("pending sheet choice beats the last accepted UI", J.Json.get_str(J.Json.get_obj(P.Session.viewer_input.current(u, J.Json.obj([J.KV{"view.sheet:t", J.Str{"/r/boards/beta/other.kicad_sch"}}]), J.Json.obj([J.KV{"sel", J.Str{"t"}}])), "info"), "view.sheet:t", ""), "/r/boards/beta/other.kicad_sch")
    C.Check.str("older saved viewers retain their board", U.View.kind.path(U.Ui.view_put(U.Viewer.restore(base(), "t", "{\"open\":\"1\",\"kind\":\"split\",\"path\":\"/r/boards/beta/beta.kicad_pcb\",\"split\":\"/r/boards/beta/power.kicad_sch\"}"), "review", "bom"), "split"), "/r/boards/beta/beta.kicad_pcb")
    C.Check.str("single-board sheet survives Review", U.View.kind.path(U.Ui.view_put(U.Ui.view_put(U.Ui.info_set(base(), "fab.boards:t", ""), "schematic", "/r/power.kicad_sch"), "review", "bom"), "schematic"), "/r/power.kicad_sch")
    C.Check.str("BOM root stays inside thread", R.RvJob.bom.root("/r", "/r/../secret.kicad_sch", "/r/main.kicad_sch"), "/r/main.kicad_sch")
    C.Check.str("BOM root follows beta", R.RvJob.bom.root("/r", "/r/boards/beta/beta.kicad_sch", "/r/main.kicad_sch"), "/r/boards/beta/beta.kicad_sch")
