import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/client.bend as U
import ../src/core/subthreads.bend as ST
import ../src/core/hub.bend as B
import ../src/core/codec.bend as CD
import ../src/core/far.bend as FR
import ../src/core/json.bend as J

def base() -> M.State:
  M.State.replay([
    M.ProjectCreated{"p", "Board", "/tmp/board", 1n},
    M.ProjectCreated{"q", "Case", "/tmp/case", 2n},
    M.ProjectCreated{"scratch", "Scratch", "/tmp/history", 1n},
    M.ThreadCreated{"parent", "p", "Manager", M.Local{}, "", "", "claude", "", 3n},
    M.ThreadCreated{"child", "p", "Connector", M.Local{}, "", "", "claude", "", 4n},
    M.ThreadCreated{"grandchild", "p", "Pinout", M.Local{}, "", "", "claude", "", 5n},
    M.ThreadCreated{"other", "q", "Case", M.Local{}, "", "", "claude", "", 6n},
    M.ThreadCreated{"history", "scratch", "Previous chat", M.Local{}, "", "/tmp/history/history", "claude", "", 7n},
    M.MessagePosted{"history", "m", M.User{}, "Keep this history", 8n},
    M.ThreadParentSet{"child", "parent", 9n},
    M.ThreadParentSet{"grandchild", "child", 10n}], M.State.empty())

def expanded() -> M.State:
  M.State.replay([M.SettingSet{"thread.tree.parent", "1", 11n}, M.SettingSet{"thread.tree.child", "1", 12n}], base())

def ids(ts: List<&2, M.Thread>) -> String:
  String.join(ST.Tree.ids(ts), ",")

def visible(+s: M.State) -> String:
  String.join(ST.Tree.row_ids(ST.Tree.rows(s, [M.State.thread.get(s, "parent")], "")), ",")

def roots(+s: M.State) -> String:
  ids(ST.Tree.roots.go(ST.Tree.all(s), s, Nil{}))

def projects(rs: List<&2, U.SRow>) -> String:
  match rs:
    case Nil{}: ""
    case U.SRow{_, M.Project{id, _, _, _, _}, _} <> rest: id ++ "," ++ projects(rest)

def changes(st: B.Step) -> List<&2, M.Change>:
  match st:
    case B.Step{cs, _}: cs

def deleted(cs: List<&2, M.Change>) -> Bool:
  match cs:
    case Nil{}: False{}
    case M.ThreadDeleted{_, _} <> rest: True{}
    case _ <> rest: deleted(rest)

def create() -> B.Step:
  B.Hub.thread_create(base(), 100n, 1, "1", J.Json.obj([J.KV{"project", J.Str{"p"}}, J.KV{"parent", J.Str{"parent"}}, J.KV{"title", J.Str{"Visible child"}}]))

def created() -> M.State:
  M.State.replay(changes(create()), base())

def delegated() -> M.State:
  M.State.replay([M.ThreadCreated{"task-child", "p", "Review layout", M.Local{}, "", "", "claude", "", 12n},
    M.TaskDelegated{"task", "parent", "task-child", 1n, "claude", "", 12n}], base())

def root_candidates(+s: M.State) -> String:
  ids(U.Kids.keep(U.Ui.threads(s), U.Kids.set(s)))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("root traversal keeps nested archive and cycle fallbacks", String.join([
      ST.Tree.root(expanded(), "grandchild"),
      ST.Tree.root(M.State.apply(expanded(), M.ThreadArchived{"parent", True{}, 30n}), "grandchild"),
      ST.Tree.root(M.State.apply(expanded(), M.ThreadParentSet{"parent", "grandchild", 30n}), "grandchild")], ","), "parent,child,grandchild")
    C.Check.str("root dedup preserves first occurrence and prior shelf roots", ids(ST.Tree.roots.go([
      M.State.thread.get(expanded(), "child"), M.State.thread.get(expanded(), "parent"),
      M.State.thread.get(expanded(), "grandchild"), M.State.thread.get(expanded(), "other")], expanded(), ["other"])), "parent")
    C.Check.str("multi-project search forest keeps root order and children", String.join(ST.Tree.row_ids(ST.Tree.rows_search(expanded(), [
      M.State.thread.get(expanded(), "other"), M.State.thread.get(expanded(), "parent")], "", "search")), ","), "other,parent,child,grandchild")
    C.Check.str("collapsed children have one visible anchor", visible(base()), "parent")
    C.Check.str("expanded hierarchy has each ID once", visible(expanded()), "parent,child,grandchild")
    C.Check.str("deep link reveals initially unconfigured ancestors", String.join(ST.Tree.row_ids(ST.Tree.rows(base(), [M.State.thread.get(base(), "parent")], "grandchild")), ","), "parent,child,grandchild")
    C.Check.str("explicit collapse wins", visible(M.State.apply(expanded(), M.SettingSet{"thread.tree.parent", "0", 13n})), "parent")
    C.Check.str("parent relationship survives codec", J.Json.show(CD.Change.encode(M.ThreadParentSet{"child", "parent", 9n})), "{\"$\":\"ThreadParentSet\",\"id\":\"child\",\"parent\":\"parent\",\"at\":9}")
    C.Check.str("new child records its actual parent", ST.Tree.parent(created(), B.Hub.id("t", 100n, B.Hub.seq(base()))), "parent")
    C.Check.str("child creates no delegated activity", Nat.show(U.SIx.kids(U.Side.ix(created()), "parent")), "0")
    C.Check.str("replayed delegation seeds visible parent", ST.Tree.parent(delegated(), "task-child"), "parent")
    C.Check.str("expanded sidebar contains delegated child", Bool.show(String.contains(visible(M.State.apply(delegated(), M.SettingSet{"thread.tree.parent", "1", 13n})), "task-child")), "True")
    C.Check.str("completed delivered task stays under parent", ST.Tree.parent(M.State.replay([M.TaskStateSet{"task", M.TaskDone{}, "done", 14n}, M.TaskDelivered{"task", 14n}], delegated()), "task-child"), "parent")
    C.Check.str("explicit move to root wins over delegation", ST.Tree.parent(M.State.apply(delegated(), M.ThreadParentSet{"task-child", "", 15n}), "task-child"), "")
    C.Check.str("moved delegated child remains a project root candidate", Bool.show(String.contains(root_candidates(M.State.apply(delegated(), M.ThreadParentSet{"task-child", "", 15n})), "task-child")), "True")
    C.Check.str("archived parent leaves delegated child reachable", Bool.show(String.contains(root_candidates(M.State.apply(delegated(), M.ThreadArchived{"parent", True{}, 15n})), "task-child")), "True")
    C.Check.str("deleted parent leaves delegated child reachable", Bool.show(String.contains(root_candidates(M.State.apply(delegated(), M.ThreadDeleted{"parent", 15n})), "task-child")), "True")
    C.Check.str("cross-project bot work stays at project root", ST.Tree.parent(M.State.apply(delegated(), M.TaskDelegated{"other-task", "other", "task-child", 1n, "claude", "", 13n}), "task-child"), "")
    +bot : M.State = M.State.replay([M.ThreadCreated{"bot-child", "p", "Bot work", M.Local{}, "", "", "claude", "", 11n},
      M.BotSet{"b", "Manager bot", "parent", 1n, "", 11n}, M.TaskDelegated{"bot-task", "parent", "bot-child", 1n, "claude", "", 12n}], base())
    C.Check.str("bot delegation never seeds a hidden sidebar parent", M.Setting.get(bot, "thread.parent.bot-child", ""), "")
    C.Check.str("same-project bot work stays a project root candidate", Bool.show(String.contains(root_candidates(bot), "bot-child")), "True")
    C.Check.str("drag order remains authoritative", projects(U.Side.rows(M.State.apply(base(), M.ProjectOrdered{"p", "8", 100n}), 101n)), "p,q,")
    C.Check.str("codec decodes parent link", ST.Tree.parent(M.State.apply(base(), Maybe.default(&2, M.Change, CD.Change.decode(CD.Change.encode(M.ThreadParentSet{"child", "parent", 9n})), M.SettingSet{"bad", "bad", 0n})), "child"), "parent")
    C.Check.str("archiving parent exposes live child", ST.Tree.display_parent(M.State.apply(base(), M.ThreadArchived{"parent", True{}, 30n}), "child"), "")
    C.Check.str("parent projection changes no activity", Nat.show(U.Active.activity(M.State.thread.get(expanded(), "parent"))), "3")
    C.Check.str("legacy history retains original ID", ids(ST.Legacy.threads(base())), "history")
    C.Check.str("legacy project absent from project list", projects(U.Side.rows(base(), 20n)), "q,p,")
    C.Check.str("two active projects stay ordered across activity", projects(U.Side.rows(M.State.apply(base(), M.TurnChanged{"parent", M.Running{}, 100n}), 101n)), projects(U.Side.rows(M.State.apply(base(), M.TurnChanged{"other", M.Running{}, 100n}), 101n)))
    C.Check.str("legacy chats do not expire in hub tick", Bool.show(deleted(changes(B.Hub.tick.k(base(), 999999n, M.Work.kids.map(base()))))), "False")
    C.Check.str("missing parent promotes child", ST.Tree.parent(M.State.apply(base(), M.ThreadDeleted{"parent", 30n}), "child"), "")
    C.Check.str("cycle cannot hide children", ST.Tree.parent(M.State.apply(base(), M.ThreadParentSet{"parent", "grandchild", 30n}), "child"), "")
    C.Check.str("cross-project parent ignored", ST.Tree.parent(M.State.apply(base(), M.ThreadParentSet{"child", "other", 30n}), "child"), "")
    C.Check.str("mirror preserves both identities", J.Json.show(CD.Change.wire(Maybe.default(&2, M.Change, FR.Fr.change("hub", M.ThreadParentSet{"child", "parent", 9n}), M.SettingSet{"bad", "bad", 0n}))), "{\"$\":\"ThreadParentSet\",\"id\":\"hub~child\",\"parent\":\"hub~parent\",\"at\":9}")
