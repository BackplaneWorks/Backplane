import Base
import ./check.bend as C
import ../src/core/client.bend as U
import ../src/core/model.bend as M
import ../src/core/i18n.bend as I
import ../src/core/json.bend as J
import ../src/web/view.bend as V
import ../src/web/bots.bend as WB
import ../src/web/presentation.bend as W

def base() -> M.State:
  M.State.replay([M.ProjectCreated{"scratch", "Scratch", "/tmp/old", 0n},
    M.ThreadCreated{"history", "scratch", "Old chat", M.Local{}, "", "", "codex", "", 1n},
    M.ThreadCreated{"child", "p1", "Child", M.Local{}, "", "", "codex", "", 20n}, M.ThreadParentSet{"child", "t1", 21n}], U.Side.demo.work())

def ui(st: M.State) -> U.Ui:
  U.Ui.adopt.raw(U.Ui.init(25n), st)

def selected(st: M.State, id: String) -> U.Ui:
  U.Ui.select.k(ui(st), id)

# Compare the complete production tree, including action IDs and digit
# destination attributes, against the former independently projected path.
def same(+raw: U.Ui) -> String:
  +u = U.Side.with_signals(raw)
  +ix = U.Side.ix(U.Ui.state(u))
  +signals = U.Side.signals.ix(ix, u)
  +bots = WB.WB.side(I.En{}, u)
  +before = V.View.sidebar.strip(V.View.sidebar.keys(V.View.sidebar.w(I.En{}, bots, WB.WB.uber(I.En{}, u, signals), U.Machines.rows.loc(I.En{}, u), U.Menu.items.loc(I.En{}, u, True{}), signals, u), U.Side.projects.ix(u, ix), U.Side.here.ix(u, ix)),
    V.View.strip(I.En{}, U.Side.projects.ix(u, ix), U.Ui.state(u), U.Side.cur(u)))
  +after = V.View.sidebar.frame(I.En{}, bots, u, ix, signals)
  Bool.pick(String, C.Check.same(J.Json.show(W.Node.write(before)), J.Json.show(W.Node.write(after))), "same", "DIFFERENT")

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("shared sidebar tree preserve project folds", same(selected(base(), "t1")), "same")
    C.Check.str("shared sidebar tree preserve expanded subthreads", same(selected(M.State.apply(base(), M.SettingSet{"thread.tree.t1", "1", 22n}), "t1")), "same")
    C.Check.str("shared sidebar tree preserve explicit collapse and deep selection", same(selected(M.State.apply(base(), M.SettingSet{"thread.tree.t1", "0", 22n}), "child")), "same")
    C.Check.str("shared sidebar tree preserve active mode ordering", same(selected(M.State.apply(base(), M.SettingSet{"sidebar.view", "active", 22n}), "t1")), "same")
    C.Check.str("shared sidebar tree preserve active settled expansion", same(U.Ui.flag_on(selected(M.State.apply(base(), M.SettingSet{"sidebar.view", "active", 22n}), "t3"), "active-settled")), "same")
    C.Check.str("shared sidebar tree preserve project search", same(U.Ui.scratch_set(selected(base(), "t1"), "@f.pq", "One")), "same")
    C.Check.str("shared sidebar tree preserve active tree search", same(U.Ui.scratch_set(selected(M.State.apply(base(), M.SettingSet{"sidebar.view", "active", 22n}), "child"), "@f.pq", "Child")), "same")
    C.Check.str("shared sidebar tree preserve legacy history targets", same(U.Ui.flag_on(selected(base(), "history"), "conversation-history")), "same")
    C.Check.str("shared sidebar tree preserve other current project", same(selected(base(), "t3")), "same")
    C.Check.str("shared sidebar tree preserves open project shelves", same(U.Ui.flag_on(U.Ui.flag_on(selected(base(), "t1"), "p1"), "arch:p1")), "same")
    C.Check.str("shared sidebar tree preserves old projects", same(U.Ui.flag_on(selected(base(), "t1"), "old-projects")), "same")
    C.Check.str("shared sidebar tree preserves empty search", same(U.Ui.scratch_set(selected(base(), "t1"), "@f.pq", "no matching project")), "same")
