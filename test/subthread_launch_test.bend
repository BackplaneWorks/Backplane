import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/hub.bend as H
import ../src/core/mcp.bend as MC
import ../src/core/subthreads.bend as ST
import ../src/core/subs.bend as SB
import ../src/core/client.bend as U
import ../src/core/json.bend as J
import ../src/core/autosettle.bend as AS
import ../src/web/project-home.bend as HV
import ../src/web/presentation.bend as WP
import ../src/web/dom.bend as D

def base() -> M.State:
  M.State.replay([M.ProjectCreated{"p", "Project", "/tmp/project", 1n},
    M.ProjectCreated{"q", "Other", "/tmp/other", 1n},
    M.ThreadCreated{"coordinator", "p", "Coordinator", M.Local{}, "", "", "codex", "", 2n},
    M.ThreadCreated{"other", "q", "Other", M.Local{}, "", "", "codex", "", 2n}], M.State.empty())

def args() -> J.Json:
  J.Json.obj([J.KV{"prompt", J.Str{"Work on the lane"}}])

def launch(+s: M.State, now: Nat, a: J.Json) -> MC.Res:
  MC.Mcp.launch(s, now, "coordinator", a)

def save(+s: M.State, r: MC.Res) -> M.State:
  M.State.replay(MC.Res.changes(r), s)

def id(+s: M.State, now: Nat) -> String:
  H.Hub.id("t", now, H.Hub.seq(s))

def ids(ts: List<&2, M.Thread>) -> String:
  String.join(ST.Tree.ids(ts), ",")

def move(+s: M.State, +child: String, parent: String) -> M.State:
  save(s, MC.Mcp.update.metadata(True{}, s, 30n, "coordinator", child, J.Json.obj([J.KV{"parent", J.Str{parent}}])))

def finished(+s: M.State, +child: String) -> M.State:
  M.State.replay([M.WorktreeSkipped{child, 18n}, M.QueueTaken{child, 19n}, M.PendingChanged{child, 0n, 19n}, M.MessagePosted{child, "answer", M.Assistant{}, "Done", 20n}, M.TurnChanged{child, M.Completed{}, 20n},
    M.SettingSet{"autosettle.report." ++ child, "20", 20n}], s)

def due(+s: M.State, child: String) -> String:
  Bool.show(AS.Auto.due(s, M.State.thread.get(s, child), 50000n, AS.Auto.index(s)))

def first_message(cs: List<&2, M.Change>) -> String:
  match cs:
    case Nil{}: ""
    case M.MessagePosted{_, id, _, _, _} <> rest: id
    case M.TurnQueued{_, id, _, _} <> rest: id
    case _ <> rest: first_message(rest)

def home_wire(+s: M.State) -> String:
  J.Json.show(WP.Node.write(D.El{"div", "", Nil{}, HV.HomeView.threads(U.Ui.adopt.raw(U.Ui.init(50n), s), ST.Tree.all(s), "p", 0n, 33n)}))

def main() -> IO(Unit):
  do IO<Unit>:
    +s : M.State = base()
    +a : String = id(s, 10n)
    +s1 : M.State = save(s, launch(s, 10n, args()))
    +b : String = id(s1, 11n)
    +s2 : M.State = save(s1, launch(s1, 11n, args()))
    +c : String = id(s2, 12n)
    +s3 : M.State = save(s2, launch(s2, 12n, args()))
    C.Check.str("each coordinator lane keeps its launcher as parent", String.join([ST.Tree.parent(s3, a), ST.Tree.parent(s3, b), ST.Tree.parent(s3, c)], ","), "coordinator,coordinator,coordinator")
    C.Check.str("sidebar model nests all three lanes", String.join(ST.Tree.row_ids(ST.Tree.rows(s3, ST.Tree.roots.go(ST.Tree.all(s3), s3, Nil{}), "")), ","), "coordinator," ++ a ++ "," ++ b ++ "," ++ c ++ ",other")
    C.Check.str("project child list names three openable threads", ids(ST.Tree.children(s3, "coordinator")), a ++ "," ++ b ++ "," ++ c)
    C.Check.str("badge counts all three once", SB.Subs.badge(SB.Subs.count.ix(U.Side.ix(s3), s3, "coordinator")), "3 subthreads")
    C.Check.str("direct badge matches indexed badge", Nat.show(SB.Subs.count(s3, "coordinator")), "3")
    C.Check.str("home uses the same nested row identities", String.join(ST.Tree.row_ids(HV.HomeView.thread.rows(s3, [M.State.thread.get(s3, "coordinator")])), ","), "coordinator," ++ a ++ "," ++ b ++ "," ++ c)
    C.Check.str("home draws child depth and badge", Bool.show(Bool.and(String.contains(home_wire(s3), "data-depth"), Bool.and(String.contains(home_wire(s3), "↳ Work on the lane"), String.contains(home_wire(s3), "3 subthreads")))), "True")
    C.Check.str("reports go to coordinator", String.join(M.Launch.of(s3, "coordinator"), ","), a ++ "," ++ b ++ "," ++ c)
    C.Check.str("launch reports on by default", Bool.show(M.Launch.reports(s3, a)), "True")
    C.Check.str("child is in auto-settle software scope", Bool.show(AS.Auto.worker(s3, a, M.Work.kids.map(s3))), "True")
    C.Check.str("quiet report choice still creates child", ST.Tree.parent(save(s, launch(s, 10n, J.Json.obj([J.KV{"prompt", J.Str{"lane"}}, J.KV{"report", J.Flag{False{}}}]))), a), "coordinator")
    C.Check.str("explicit top level opts out", ST.Tree.parent(save(s, launch(s, 10n, J.Json.obj([J.KV{"prompt", J.Str{"lane"}}, J.KV{"subthread", J.Flag{False{}}}]))), a), "")
    C.Check.str("another project cannot silently lose parent", Bool.show(MC.Res.good(launch(s, 10n, J.Json.obj([J.KV{"prompt", J.Str{"lane"}}, J.KV{"project", J.Str{"q"}}])))), "False")
    C.Check.str("explicit top level allows another project", Bool.show(MC.Res.good(launch(s, 10n, J.Json.obj([J.KV{"prompt", J.Str{"lane"}}, J.KV{"project", J.Str{"q"}}, J.KV{"subthread", J.Flag{False{}}}])))), "True")
    +moved : M.State = move(s3, a, "")
    C.Check.str("manual detach wins", ST.Tree.parent(moved, a), "")
    C.Check.str("report toggle never restores parent", ST.Tree.parent(save(moved, MC.Mcp.report.k(True{}, moved, 31n, a, False{})), a), "")
    C.Check.str("manual attach uses supported event path", ST.Tree.parent(move(moved, a, "coordinator"), a), "coordinator")
    C.Check.str("manual move adjusts badge", SB.Subs.badge(SB.Subs.count(moved, "coordinator")), "2 subthreads")
    C.Check.str("cannot introduce cycle", Bool.show(H.Hub.thread_parent.valid(s3, "coordinator", a)), "False")
    C.Check.str("cannot cross projects", Bool.show(H.Hub.thread_parent.valid(s3, a, "other")), "False")
    C.Check.str("cannot attach to absent parent", Bool.show(H.Hub.thread_parent.valid(s3, a, "missing")), "False")
    C.Check.str("cannot move absent thread", Bool.show(H.Hub.thread_parent.valid(s3, "missing", "")), "False")
    C.Check.str("legacy launch events do not infer parents", M.Setting.get(M.State.apply(s3, M.ThreadLaunched{"other", "coordinator", True{}, 32n}), "thread.parent.other", ""), "")
    C.Check.str("seed is the exact launch prompt id", M.Setting.get(s1, "autosettle.seed." ++ a, ""), first_message(MC.Res.changes(launch(s, 10n, args()))))
    C.Check.str("finished reported child can settle", due(finished(s1, a), a), "True")
    +root : M.State = save(s, launch(s, 10n, J.Json.obj([J.KV{"prompt", J.Str{"lane"}}, J.KV{"subthread", J.Flag{False{}}}])))
    C.Check.str("explicit root worker still auto-settles after report", due(finished(root, a), a), "True")
    C.Check.str("root worker keeps report recipient", M.Launch.by(root, a), "coordinator")
    +busy : M.State = M.State.apply(move(finished(s1, a), a, ""), M.WorkSet{a, "bg", "job\tRunning\tcall\tBuild\tshell", 21n})
    C.Check.str("moved child with running work cannot settle", due(busy, a), "False")
    C.Check.str("moved child keeps report recipient", M.Launch.by(busy, a), "coordinator")
    +human : M.State = M.State.replay([M.MessagePosted{a, "human-followup", M.User{}, "One more thing", 21n},
      M.MessagePosted{a, "human-answer", M.Assistant{}, "Answer", 22n}, M.TurnChanged{a, M.Completed{}, 22n},
      M.SettingSet{"autosettle.report." ++ a, "22", 22n}], finished(s1, a))
    C.Check.str("human followup protects unread child reply", due(human, a), "False")
    C.Check.str("human followup protects unread moved reply", due(move(human, a, ""), a), "False")
    +batch : List<&2, M.Change> = [M.SettingSet{"autosettle.seed." ++ a, "seed-id", 40n},
      M.MessagePosted{a, "seed-id", M.User{}, "Do the work", 40n}]
    +metadata : List<&2, M.Change> = AS.Auto.messages.metadata(batch, s3)
    C.Check.str("same-step seed classifies initial prompt as worker", M.Setting.get(M.State.replay(metadata, s3), "autosettle.audience." ++ a, ""), "worker")
    C.Check.str("collector emits metadata only", Nat.show(List.length(&2, M.Change, metadata)), "1")
    +follow : List<&2, M.Change> = List.append(&2, M.Change, batch, [M.MessagePosted{a, "same-second-human", M.User{}, "Help me too", 40n}])
    C.Check.str("distinct same-second human followup wins over seed", M.Setting.get(M.State.replay(AS.Auto.messages.metadata(follow, s3), s3), "autosettle.audience." ++ a, ""), "person")
    +task : M.State = M.State.apply(s3, M.TaskDelegated{"task", "coordinator", a, 1n, "codex", "", 33n})
    C.Check.str("delegated visible lane is never counted twice", Nat.show(SB.Subs.count(task, "coordinator")), "3")
