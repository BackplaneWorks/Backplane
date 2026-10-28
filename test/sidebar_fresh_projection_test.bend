import Base
import ./check.bend as C
import ./sidebar_projection_test.bend as Old
import ../src/core/client.bend as U
import ../src/core/model.bend as M
import ../src/core/i18n.bend as I
import ../src/core/json.bend as J
import ../src/web/view.bend as V
import ../src/web/bots.bend as WB
import ../src/web/presentation.bend as W

# Full production tree, including display signals and exact key targets.
# The oracle keeps the three independent projections used before this change.
def same(+u: U.Ui) -> String:
  +ix = U.Side.ix(U.Ui.state(u))
  +signals = U.Side.signals.ix(ix, u)
  +bots = WB.WB.side(I.En{}, u)
  +before = V.View.sidebar.strip(V.View.sidebar.keys(V.View.sidebar.w(I.En{}, bots, WB.WB.uber(I.En{}, u, signals), U.Machines.rows.loc(I.En{}, u), U.Menu.items.loc(I.En{}, u, True{}), signals, u), U.Side.projects.ix(u, ix), U.Side.here.ix(u, ix)),
    V.View.strip(I.En{}, U.Side.projects.ix(u, ix), U.Ui.state(u), U.Side.cur(u)))
  +after = V.View.sidebar.frame(I.En{}, bots, u, ix, signals)
  Bool.pick(String, C.Check.same(J.Json.show(W.Node.write(before)), J.Json.show(W.Node.write(after))), "same", "DIFFERENT")

def active() -> M.State:
  M.State.apply(Old.base(), M.SettingSet{"sidebar.view", "active", 22n})

def selected(st: M.State, id: String) -> U.Ui:
  U.Side.with_signals(Old.selected(st, id))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("shared sidebar tree preserve active mode ordering", Old.same(Old.selected(active(), "t1")), "same")
    C.Check.str("shared sidebar tree preserve active settled expansion", Old.same(U.Ui.flag_on(Old.selected(active(), "t3"), "active-settled")), "same")
    C.Check.str("shared sidebar tree preserve active tree search", Old.same(U.Ui.scratch_set(Old.selected(active(), "child"), "@f.pq", "Child")), "same")
    C.Check.str("fresh active asks and owned work", same(selected(active(), "t1")), "same")
    C.Check.str("fresh active child and search", same(U.Ui.scratch_set(selected(active(), "child"), "@f.pq", "Child")), "same")
    C.Check.str("fresh active folded settled", same(U.Ui.flag_on(selected(active(), "t3"), "active-settled")), "same")
    C.Check.str("fresh active legacy current project", same(U.Ui.flag_on(selected(active(), "history"), "conversation-history")), "same")
    C.Check.str("fresh active unrelated project", same(selected(active(), "t3")), "same")
    C.Check.str("display signal must not choose key targets", same(U.Ui.info_set(U.Ui.info_set(selected(active(), "t1"), "@signal:t1", "0"), "@signal:t3", "3")), "same")
    C.Check.str("fresh changed approval state", same(selected(M.State.apply(active(), M.AskClosed{"a1", "yes", 26n}), "t1")), "same")
    C.Check.str("fresh new question state", same(selected(M.State.apply(active(), M.AskOpened{"new", "child", "input", "", "Question", "", 26n}), "child")), "same")
    C.Check.str("fresh removed project", same(selected(M.State.apply(active(), M.ProjectRemoved{"p2", 26n}), "t1")), "same")
    C.Check.str("fresh settled current thread", same(selected(M.State.apply(active(), M.ThreadSettled{"child", False{}, 26n}), "child")), "same")
    C.Check.str("fresh no matching search", same(U.Ui.scratch_set(selected(active(), "t1"), "@f.pq", "no matching project")), "same")
    C.Check.str("fresh clock quiet shelf boundary", same(U.Side.with_signals(U.Ui.select.k(U.Ui.adopt.raw(U.Ui.init(900000n), active()), "t1"))), "same")
