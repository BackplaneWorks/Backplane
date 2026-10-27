import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/client.bend as U
import ../src/core/subthreads.bend as ST
import ../src/core/i18n.bend as I
import ../src/web/dom.bend as D
import ../src/web/view.bend as V
import ../src/web/presentation.bend as P
import ../src/core/json.bend as J

def events() -> List<&2, M.Change>:
  [M.ProjectCreated{"p", "Synthetic", "/tmp/delegated-tree", 1n},
   M.ProjectCreated{"q", "Other", "/tmp/other", 1n},
   M.ThreadCreated{"parent", "p", "Chat fixes", M.Local{}, "", "", "codex", "", 2n},
   M.ThreadCreated{"todos", "p", "Completed todos", M.Local{}, "", "", "codex", "", 3n},
   M.ThreadCreated{"scrollbars", "p", "Scrollbars", M.Local{}, "", "", "codex", "", 4n},
   M.ThreadCreated{"paging", "p", "Infinite scroll", M.Local{}, "", "", "codex", "", 5n},
   M.ThreadCreated{"other", "q", "Other project", M.Local{}, "", "", "codex", "", 6n},
   M.TaskDelegated{"task-todos", "parent", "todos", 1n, "codex", "gpt-6.1-sol", 7n},
   M.TaskDelegated{"task-scrollbars", "parent", "scrollbars", 1n, "codex", "gpt-6.1-sol", 8n},
   M.TaskDelegated{"task-paging", "parent", "paging", 1n, "codex", "gpt-6.1-sol", 9n},
   M.TurnChanged{"parent", M.Running{}, 10n},
   M.TurnChanged{"todos", M.Running{}, 11n},
   M.SettingSet{"thread.tree.parent", "1", 12n}]

def state() -> M.State:
  M.State.replay(events(), M.State.empty())

def rows(+s: M.State) -> List<&2, ST.TRow>:
  ST.Tree.rows(s, [M.State.thread.get(s, "parent")], "parent")

def ids(+s: M.State) -> String:
  String.join(ST.Tree.row_ids(rows(s)), ",")

def active(rs: List<&2, U.SRow>) -> String:
  match rs:
    case Nil{}: ""
    case U.SRow{_, M.Project{"p", _, _, _, _}, M.Shelves{_, a, _, _, _}} <> _: String.join(ST.Tree.ids(a), ",")
    case _ <> rest: active(rest)

def roots(rs: List<&2, U.SRow>) -> String:
  match rs:
    case Nil{}: ""
    case U.SRow{_, M.Project{"p", _, _, _, _}, M.Shelves{p, a, st, _, sn}} <> _: String.join(ST.Tree.ids(List.concat(&2, M.Thread, [p, a, st, sn])), ",")
    case _ <> rest: roots(rest)

def dom_ids(ns: List<&2, D.Node>) -> String:
  match ns:
    case Nil{}: ""
    case D.Txt{_} <> rest: dom_ids(rest)
    case D.El{_, +key, _, kids} <> rest:
      Bool.pick(String, String.starts_with(key, "tree:"), String.drop(key, 5n) ++ ",", "") ++ dom_ids(kids) ++ dom_ids(rest)

def card(+s: M.State) -> List<&2, D.Node>:
  V.View.tree.rows(I.En{}, rows(s), U.Side.ix(s), s, "parent", 100n, False{})

def ui(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}: u

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("badge counts three actual delegation records", Nat.show(U.SIx.kids(U.Side.ix(state()), "parent")), "3")
    C.Check.str("expanded parent shows its three delegated children", ids(state()), "parent,todos,scrollbars,paging")
    C.Check.str("actual web tree DOM includes the same children", dom_ids(card(state())), "parent,todos,scrollbars,paging,")
    C.Check.str("collapsed parent remains one row", ids(M.State.apply(state(), M.SettingSet{"thread.tree.parent", "0", 13n})), "parent")
    C.Check.str("selection navigates to existing child identity", U.Ui.sel(ui(U.Ui.select(U.Ui.fold(events(), U.Ui.init(100n)), "paging"))), "paging")
    C.Check.str("explicit empty parent setting detaches", ST.Tree.parent(M.State.apply(state(), M.ThreadParentSet{"todos", "", 13n}), "todos"), "")
    C.Check.str("detached child stays a sidebar root", roots(U.Side.rows(M.State.apply(state(), M.ThreadParentSet{"todos", "", 13n}), 100n)), "parent,todos")
    C.Check.str("manual link overrides delegation", ST.Tree.parent(M.State.apply(state(), M.ThreadParentSet{"paging", "todos", 13n}), "paging"), "todos")
    C.Check.str("cross-project manual link is rejected", ST.Tree.parent(M.State.apply(state(), M.ThreadParentSet{"paging", "other", 13n}), "paging"), "")
    C.Check.str("deleted parent cannot hide child", ST.Tree.parent(M.State.apply(state(), M.ThreadDeleted{"parent", 13n}), "paging"), "")
    C.Check.str("archiving parent promotes live children", ST.Tree.display_parent(M.State.apply(state(), M.ThreadArchived{"parent", True{}, 13n}), "paging"), "")
    C.Check.str("cycles fall back to roots", ST.Tree.parent(M.State.apply(state(), M.ThreadParentSet{"parent", "paging", 13n}), "paging"), "")
    C.Check.str("bot-owned delegates stay roots", ST.Tree.parent(M.State.apply(state(), M.BotSet{"bot", "Bot", "parent", 0n, "", 13n}), "paging"), "")
    +settled : M.State = M.State.apply(state(), M.ThreadSettled{"parent", True{}, 13n})
    C.Check.str("active child keeps settled parent on active project shelf", active(U.Side.rows(settled, 100n)), "parent")
    C.Check.str("indexed shelves agree for active child with settled parent", active(U.Side.rows.ix(settled, U.Side.ix(settled), 100n)), "parent")
    C.Check.str("settled child remains under expanded parent", ids(M.State.apply(state(), M.ThreadSettled{"paging", True{}, 13n})), "parent,todos,scrollbars,paging")
    C.Check.str("terminal task retains child history link", ST.Tree.parent(M.State.apply(state(), M.TaskStateSet{"task-paging", M.TaskDone{}, "done", 13n}), "paging"), "parent")
    C.Check.str("search expands children without changing saved collapse", String.join(ST.Tree.row_ids(ST.Tree.rows_search(M.State.apply(state(), M.SettingSet{"thread.tree.parent", "0", 13n}), [M.State.thread.get(state(), "parent")], "", "scroll")), ","), "parent,todos,scrollbars,paging")
    C.Check.str("another project cannot include these children", String.join(ST.Tree.row_ids(ST.Tree.rows(state(), [M.State.thread.get(state(), "other")], "")), ","), "other")
    C.Check.str("projection does not persist inferred parent settings", M.Setting.get(state(), "thread.parent.paging", "missing"), "missing")
    IO.print("TREE_JSON " ++ J.Json.show(P.Node.write(D.Dom.div("sidebar", V.View.expand(card(state()))))))
