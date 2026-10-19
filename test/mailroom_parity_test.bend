import Base
import ./check.bend as C
import ./plane_test.bend as PT
import ../src/core/client.bend as U
import ../src/core/model.bend as M
import ../src/core/plane.bend as P
import ../src/core/botui.bend as BU
import ../src/core/uber.bend as UB
import ../src/core/mailview.bend as MV
import ../src/app/mailroom.bend as MR
import ../src/app/mm.bend as MM
import ../src/app/nui.bend as N
import ../src/gfx/draw.bend as D

# Compare complete row identities: preserving an id alone would lose the
# approval's action, remote machine or urgency state.
def rows(rs: List<&2, P.PRow>) -> String:
  match rs:
    case Nil{}: ""
    case P.PRow{k, i, a, v, t, s, m, st, at, aw} <> rest:
      k ++ "/" ++ i ++ "/" ++ a ++ "/" ++ v ++ "/" ++ t ++ "/" ++ s ++ "/" ++ m ++ "/" ++ st ++ "/" ++ Nat.show(at) ++ Bool.pick(String, aw, "away", "here") ++ "\n" ++ rows(rest)

def expected(+p: P.Plane) -> List<&2, P.PRow>:
  List.concat(&2, P.PRow, [P.Plane.need(p), P.Plane.run(p), P.Plane.locks(p), P.Plane.routines(p), P.Plane.rooms(p), P.Plane.idle(p), P.Plane.bots(p)])

def check(+u: U.Ui) -> IO(Unit):
  do IO<Unit>:
    +p : P.Plane = P.Plane.of(u, PT.now())
    C.Check.str("every Plane row retains state, machine and action", rows(MR.Mr.all(p)), rows(expected(p)))
    C.Check.str("Today rows", rows(MR.Mr.chip_rows(p, "today")), rows(P.Plane.idle(p)))
    C.Check.str("Unread rooms", rows(MR.Mr.chip_rows(p, "unread")), rows(P.Plane.rooms(p)))
    C.Check.str("needs rank retained", rows(P.Plane.need(p)), rows(List.take(&2, P.PRow, MR.Mr.all(p), List.length(&2, P.PRow, P.Plane.need(p)))))

def ui() -> U.Ui:
  U.Ui.info_set(U.Ui.scratch_set(U.Ui.far.st(U.Ui.init(PT.now()), M.State.replay([M.UberSet{"b1", "uid", 199999n}], U.Ui.state(PT.ui(Nil{})))), "@bot", "b1"), "local.mailroom", "on")

def bool(name: String, +got: Bool, +want: Bool) -> IO(Unit):
  C.Check.str(name, Bool.pick(String, got, "true", "false"), Bool.pick(String, want, "true", "false"))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("overscan does not cover search", N.Hit.value_at([MR.Mr.row.hit(D.Clip{282, 310, 1382, 888}, 282, 1100, 206, "c")], "mail-open", 410, 222), "")
    C.Check.str("partial row only hits visible pixels", N.Hit.value_at([MR.Mr.row.hit(D.Clip{282, 310, 1382, 888}, 282, 1100, 284, "c")], "mail-open", 410, 305), "")
    C.Check.str("visible portion still opens", N.Hit.value_at([MR.Mr.row.hit(D.Clip{282, 310, 1382, 888}, 282, 1100, 284, "c")], "mail-open", 410, 320), "c")
    check(PT.ui(Nil{}))
    check(PT.far(True{}))
    check(PT.far(False{}))
    check(PT.ui([M.SettingSet{"deploy.row", "waiting", 199999n}]))
    C.Check.str("local tabs after Chat", String.join(BU.BU.tabs.mail("b1"), ","), "chat,mail,space,browser,tasks,memory,routines,hooks,settings")
    C.Check.str("far tabs unchanged", String.join(BU.BU.tabs.of("link~bot"), ","), "chat,space,memory,routines")
    C.Check.str("uber first tabs", String.join(List.take(&2, String, BU.BU.tabs.for(ui(), "b1"), 2n), ","), "mail,status")
    C.Check.str("uber defaults to mail", BU.BU.tab.dflt(ui(), "b1"), "mail")
    C.Check.str("bot opened by its thread resolves its Mailroom owner", U.Mailv.owner(U.Bots.sel.set(U.Ui.scratch_set(ui(), "@bot", ""), "bt1")), "b1")
    C.Check.str("ordinary bot defaults to Chat", BU.BU.tab.dflt(ui(), "b2"), "chat")
    bool("running starts collapsed", U.Mailv.run_open(ui()), False{})
    C.Check.str("bot chip filters", U.Mailv.bot(U.Mailv.chip(ui(), "b2")), "b2")
    C.Check.str("chip leaves selected bot", U.Ui.scratch(U.Mailv.chip(ui(), "b2"), "@bot"), "b1")
    C.Check.str("chip leaves selected thread", U.Ui.sel(U.Mailv.chip(ui(), "b2")), U.Ui.sel(ui()))
    bool("unrelated info does not change mail pane key", String.eq(MM.Mm.mail(N.Nui.init(ui(), 1400, 860), "plane"), MM.Mm.mail(N.Nui.init(U.Ui.info_set(ui(), "unrelated", "x"), 1400, 860), "plane")), True{})
    bool("view changes invalidate pane key", String.eq(MM.Mm.mail(N.Nui.init(ui(), 1400, 860), "plane"), MM.Mm.mail(N.Nui.init(U.Mailv.chip(ui(), "b2"), 1400, 860), "plane")), False{})
    IO.print("mailroom parity passed")
