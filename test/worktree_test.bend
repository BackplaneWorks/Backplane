import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/codec.bend as K
import ../src/core/hub.bend as H
import ../src/core/json.bend as J
import ../src/core/client.bend as U

# Thread worktrees and merges: a new thread asks for a worktree, its first
# message waits for it, then runs there (or in the folder when there is no
# repository); a merged branch settles its thread once.

def changes.go(cs: List<&2, M.Change>) -> List<&2, String>:
  match cs:
    case Nil{}:
      Nil{}
    case c <> t:
      J.Json.get_str(K.Change.encode(c), "$", "") <> changes.go(t)

# the changes' tags
def tags(st: H.Step) -> String:
  String.join(changes.go(H.Queue.changes(st)), " ")

def out.one(o: H.Out) -> String:
  match o:
    case H.Git{_, _, job, _, cwd, a, _}:
      "git:" ++ job ++ "@" ++ cwd ++ "(" ++ a ++ ")"
    case H.AgentStart{_, cwd, _, _, _, _, _}:
      "start@" ++ cwd
    case H.AgentSend{_, _}:
      "send"
    case H.Send{_, _}:
      "reply"
    case _:
      "other"

def outs.go(os: List<&2, H.Out>) -> List<&2, String>:
  match os:
    case Nil{}:
      Nil{}
    case o <> t:
      out.one(o) <> outs.go(t)

# the effects, replies left out
def outs(st: H.Step) -> String:
  String.join(List.filter(~String, ~(x => Bool.not(String.eq(x, "reply"))), outs.go(H.Step.outs_of(st))), " ")

def base() -> List<&2, M.Change>:
  [M.ProjectCreated{"p1", "P", "/r/board", 50n}]

def state(es: List<&2, M.Change>) -> M.State:
  M.State.replay(es, M.State.empty())

def step(+es: List<&2, M.Change>, +now: Nat, +text: String) -> H.Step:
  H.Hub.request_text(state(es), now, 1, text)

# the log after a request
def after(+es: List<&2, M.Change>, +now: Nat, +text: String) -> List<&2, M.Change>:
  List.append(&2, M.Change, es, H.Queue.changes(step(es, now, text)))

def made() -> List<&2, M.Change>:
  after(base(), 100n, "{\"id\":1,\"m\":\"thread.create\",\"p\":{\"project\":\"p1\"}}")

def sent() -> List<&2, M.Change>:
  after(made(), 101n, "{\"id\":2,\"m\":\"turn.start\",\"p\":{\"thread\":\"t100-1\",\"text\":\"fix the footprint\"}}")

def ready() -> List<&2, M.Change>:
  after(sent(), 102n, "{\"id\":0,\"m\":\"thread.worktree\",\"p\":{\"thread\":\"t100-1\",\"branch\":\"backplane/0a1b2c3d\",\"path\":\"/r/.backplane/worktrees/backplane-0a1b2c3d/board\"}}")

def shelf.name(sh: M.Shelf) -> String:
  match sh:
    case M.Pinned{}:
      "pinned"
    case M.Active{}:
      "active"
    case M.Settled{}:
      "settled"
    case M.Hidden{}:
      "hidden"
    case M.Snoozed{}:
      "snoozed"

def shelf(+es: List<&2, M.Change>) -> String:
  shelf.name(M.Shelf.of(H.Hub.thread(state(es), "t100-1")))

def merged(+es: List<&2, M.Change>, +now: Nat) -> List<&2, M.Change>:
  after(es, now, "{\"id\":0,\"m\":\"thread.merged\",\"p\":{\"thread\":\"t100-1\",\"url\":\"https://github.com/o/r/pull/7\"}}")

def settled(+es: List<&2, M.Change>, +now: Nat) -> List<&2, M.Change>:
  after(es, now, "{\"id\":1,\"m\":\"thread.settle\",\"p\":{\"thread\":\"t100-1\"}}")

# ready, then its turn done
def idle() -> List<&2, M.Change>:
  List.append(&2, M.Change, ready(), [M.TurnChanged{"t100-1", M.Completed{}, 150n}])

def removed() -> List<&2, M.Change>:
  after(merged(idle(), 1000n), 1001n, "{\"id\":0,\"m\":\"thread.unrooted\",\"p\":{\"thread\":\"t100-1\"}}")

def back() -> List<&2, M.Change>:
  after(removed(), 5000n, "{\"id\":3,\"m\":\"turn.start\",\"p\":{\"thread\":\"t100-1\",\"text\":\"one more thing\"}}")

def hits(hs: List<&2, J.KV>) -> String:
  match hs:
    case Nil{}:
      ""
    case J.KV{k, v} <> t:
      k ++ "=" ++ J.Json.as_raw(Some{v}, "") ++ " " ++ hits(t)

def flag(b: Bool) -> String:
  match b:
    case True{}:
      "T"
    case False{}:
      "F"

def roundtrip(c: M.Change) -> String:
  J.Json.show(K.Change.encode(Maybe.default(&2, M.Change, K.Change.decode(K.Change.encode(c)), M.ThreadDeleted{"", 0n})))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("create asks for a worktree", outs(step(base(), 100n, "{\"id\":1,\"m\":\"thread.create\",\"p\":{\"project\":\"p1\"}}")), "git:worktree@/r/board(auto)")
    C.Check.str("create local", outs(step(base(), 100n, "{\"id\":1,\"m\":\"thread.create\",\"p\":{\"project\":\"p1\",\"env\":\"local\"}}")), "")
    C.Check.str("create by setting", outs(step(List.append(&2, M.Change, base(), [M.SettingSet{"thread.env", "local", 60n}]), 100n, "{\"id\":1,\"m\":\"thread.create\",\"p\":{\"project\":\"p1\"}}")), "")
    C.Check.str("first message waits", tags(step(made(), 101n, "{\"id\":2,\"m\":\"turn.start\",\"p\":{\"thread\":\"t100-1\",\"text\":\"fix the footprint\"}}")), "ThreadRenamed MessagePosted TurnQueued")
    C.Check.str("first message only asks for a title", outs(step(made(), 101n, "{\"id\":2,\"m\":\"turn.start\",\"p\":{\"thread\":\"t100-1\",\"text\":\"fix the footprint\"}}")), "git:title@(fix the footprint)")
    C.Check.str("written title replaces the placeholder", J.Json.show(K.Change.encode(Maybe.default(&2, M.Change, List.head(&2, M.Change, H.Queue.changes(step(sent(), 103n, "{\"id\":0,\"m\":\"thread.titled\",\"p\":{\"thread\":\"t100-1\",\"placeholder\":\"fix the footprint\",\"title\":\"\\\"Fix the U3 footprint.\\\"\"}}"))), M.ThreadDeleted{"", 0n}))),
      "{\"$\":\"ThreadRenamed\",\"id\":\"t100-1\",\"title\":\"Fix the U3 footprint\",\"at\":103}")
    C.Check.str("the user's rename wins", tags(step(List.append(&2, M.Change, sent(), [M.ThreadRenamed{"t100-1", "Mine", 102n}]), 103n, "{\"id\":0,\"m\":\"thread.titled\",\"p\":{\"thread\":\"t100-1\",\"placeholder\":\"fix the footprint\",\"title\":\"Fix the U3 footprint\"}}")), "")
    C.Check.str("no answer keeps the placeholder", tags(step(sent(), 103n, "{\"id\":0,\"m\":\"thread.titled\",\"p\":{\"thread\":\"t100-1\",\"placeholder\":\"fix the footprint\",\"title\":\"\"}}")), "")
    C.Check.str("ready runs it", tags(step(sent(), 102n, "{\"id\":0,\"m\":\"thread.worktree\",\"p\":{\"thread\":\"t100-1\",\"branch\":\"backplane/0a1b2c3d\",\"path\":\"/r/.backplane/worktrees/backplane-0a1b2c3d/board\"}}")), "WorktreeSet QueueTaken TurnChanged")
    C.Check.str("ready runs it there", outs(step(sent(), 102n, "{\"id\":0,\"m\":\"thread.worktree\",\"p\":{\"thread\":\"t100-1\",\"branch\":\"backplane/0a1b2c3d\",\"path\":\"/r/.backplane/worktrees/backplane-0a1b2c3d/board\"}}")),
      "git:checkpoint0@/r/.backplane/worktrees/backplane-0a1b2c3d/board() git:step@/r/.backplane/worktrees/backplane-0a1b2c3d/board() git:name@/r/.backplane/worktrees/backplane-0a1b2c3d/board(backplane/0a1b2c3d) start@/r/.backplane/worktrees/backplane-0a1b2c3d/board send")
    C.Check.str("viewer follows the worktree", U.Ui.dir(state(ready()), "/r/.backplane/worktrees/backplane-0a1b2c3d/board", "p1"), "/r/.backplane/worktrees/backplane-0a1b2c3d/board")
    C.Check.str("next message runs in the worktree", outs(step(ready(), 200n, "{\"id\":3,\"m\":\"turn.start\",\"p\":{\"thread\":\"t100-1\",\"text\":\"and the other\"}}")),
      "git:checkpoint0@/r/.backplane/worktrees/backplane-0a1b2c3d/board() git:step@/r/.backplane/worktrees/backplane-0a1b2c3d/board(m200-8) start@/r/.backplane/worktrees/backplane-0a1b2c3d/board send")
    C.Check.str("skipped runs in the folder", outs(step(sent(), 102n, "{\"id\":0,\"m\":\"thread.local\",\"p\":{\"thread\":\"t100-1\"}}")), "git:checkpoint0@/r/board() git:step@/r/board() start@/r/board send")
    C.Check.str("skipped is local", tags(step(sent(), 102n, "{\"id\":0,\"m\":\"thread.local\",\"p\":{\"thread\":\"t100-1\"}}")), "WorktreeSkipped QueueTaken TurnChanged")
    C.Check.str("no wait past the window", tags(step(made(), 300n, "{\"id\":2,\"m\":\"turn.start\",\"p\":{\"thread\":\"t100-1\",\"text\":\"go\"}}")), "ThreadRenamed MessagePosted TurnChanged")
    C.Check.str("stranded wait runs on the tick", outs(H.Hub.tick(state(sent()), 400n)), "git:checkpoint0@/r/board() git:step@/r/board() start@/r/board send")
    C.Check.str("tick leaves a fresh wait", outs(H.Hub.tick(state(sent()), 150n)), "")
    C.Check.str("tick asks gh", outs(H.Hub.tick(state(ready()), 1000n)), "git:merged@/r/board(t100-1\tbackplane/0a1b2c3d\t/r/.backplane/worktrees/backplane-0a1b2c3d/board)")
    C.Check.str("tick skips branchless", outs(H.Hub.tick(state(after(base(), 100n, "{\"id\":1,\"m\":\"thread.create\",\"p\":{\"project\":\"p1\",\"env\":\"local\"}}")), 1000n)), "")
    C.Check.str("merge settles", shelf(merged(ready(), 1000n)), "settled")
    C.Check.str("merge settles a pinned one", shelf(merged(List.append(&2, M.Change, ready(), [M.ThreadPinned{"t100-1", True{}, 500n}]), 1000n)), "settled")
    C.Check.str("merge once", tags(step(merged(ready(), 1000n), 1100n, "{\"id\":0,\"m\":\"thread.merged\",\"p\":{\"thread\":\"t100-1\",\"url\":\"u\"}}")), "")
    C.Check.str("brought back stays", shelf(merged(after(merged(ready(), 1000n), 1100n, "{\"id\":1,\"m\":\"thread.unsettle\",\"p\":{\"thread\":\"t100-1\"}}"), 1200n)), "active")
    C.Check.str("merged not asked again", outs(H.Hub.tick(state(merged(ready(), 1000n)), 2000n)), "")
    C.Check.str("merge kept", M.Setting.get(state(merged(ready(), 1000n)), "merged.t100-1", ""), "https://github.com/o/r/pull/7")
    C.Check.str("gh hits", hits(H.Merge.hits("[{\"headRefName\":\"x\",\"url\":\"u1\"},{\"headRefName\":\"backplane/0a1b2c3d\",\"url\":\"u2\"}]", "t1\tbackplane/0a1b2c3d\nt2\tnope")), "t1=u2 ")
    C.Check.str("gh hits a renamed branch by its commit", hits(H.Merge.hits("[{\"headRefName\":\"backplane/other\",\"headRefOid\":\"c2\",\"url\":\"u1\"}]", "t1\tbackplane/0a1b2c3d\t/w\tc1 c2 c3\nt2\tnope\t/w2\tc4")), "t1=u1 ")
    C.Check.str("gh commit not the worktree's", hits(H.Merge.hits("[{\"headRefName\":\"x\",\"headRefOid\":\"c9\",\"url\":\"u1\"}]", "t1\tb\t/w\tc1 c2")), "")
    C.Check.str("gh no reflog", hits(H.Merge.hits("[{\"headRefName\":\"b\",\"headRefOid\":\"\",\"url\":\"u1\"},{\"headRefName\":\"x\",\"url\":\"u2\"}]", "t1\tb\t/w\t\nt2\tc\t/w2")), "t1=u1 ")
    C.Check.str("gh garbage", hits(H.Merge.hits("gh: not logged in", "t1\tb")), "")
    C.Check.str("hardware", flag(H.Wt.hardware(["README.md", "board.kicad_pro"])) ++ flag(H.Wt.hardware(["src", "Cargo.toml"])), "TF")
    C.Check.str("seed from the id", Nat.show(H.Wt.seed("t100-1")) ++ " " ++ Nat.show(H.Wt.seed("t100-2")), "1001 1002")
    C.Check.str("codec skipped", roundtrip(M.WorktreeSkipped{"a", 3n}), "{\"$\":\"WorktreeSkipped\",\"id\":\"a\",\"at\":3}")
    C.Check.str("codec merged", roundtrip(M.ThreadMerged{"a", "u", 3n}), "{\"$\":\"ThreadMerged\",\"id\":\"a\",\"url\":\"u\",\"at\":3}")
    C.Check.str("merged worktree goes", outs(H.Hub.tick(state(merged(idle(), 1000n)), 1060n)), "git:wt_remove@/r/.backplane/worktrees/backplane-0a1b2c3d/board(/r/board)")
    C.Check.str("busy worktree stays", outs(H.Hub.tick(state(merged(ready(), 1000n)), 1060n)), "")
    C.Check.str("settled worktree stays", outs(H.Hub.tick(state(settled(idle(), 1000n)), 1060n)), "")
    C.Check.str("settled worktree goes after days", outs(H.Hub.tick(state(settled(idle(), 1000n)), 260200n)), "git:wt_remove@/r/.backplane/worktrees/backplane-0a1b2c3d/board(/r/board)")
    C.Check.str("worktree days setting", outs(H.Hub.tick(state(settled(List.append(&2, M.Change, idle(), [M.SettingSet{"worktree.days", "1", 160n}]), 1000n)), 87400n)), "git:wt_remove@/r/.backplane/worktrees/backplane-0a1b2c3d/board(/r/board)")
    C.Check.str("active worktree stays", outs(H.Hub.tick(state(idle()), 900000n)), "git:merged@/r/board(t100-1\tbackplane/0a1b2c3d\t/r/.backplane/worktrees/backplane-0a1b2c3d/board)")
    C.Check.str("folder never removed", outs(H.Hub.tick(state(settled(after(sent(), 102n, "{\"id\":0,\"m\":\"thread.worktree\",\"p\":{\"thread\":\"t100-1\",\"branch\":\"b\",\"path\":\"/r/board\"}}"), 1000n)), 900000n)), "")
    C.Check.str("unrooted", tags(step(merged(idle(), 1000n), 1001n, "{\"id\":0,\"m\":\"thread.unrooted\",\"p\":{\"thread\":\"t100-1\"}}")), "WorktreeRemoved")
    C.Check.str("removed is gone", flag(M.Worktree.gone(H.Hub.thread(state(removed()), "t100-1"))), "T")
    C.Check.str("removed not asked again", outs(H.Hub.tick(state(removed()), 2000n)), "")
    C.Check.str("coming back makes it again", outs(step(removed(), 5000n, "{\"id\":3,\"m\":\"turn.start\",\"p\":{\"thread\":\"t100-1\",\"text\":\"one more thing\"}}")), "git:worktree@/r/board(again)")
    # the hub records its ask (wt.again.<thread>, Wt.asked) before the message
    # it queues, so the message waits for that worktree (Worktree.waits)
    C.Check.str("coming back waits", tags(step(removed(), 5000n, "{\"id\":3,\"m\":\"turn.start\",\"p\":{\"thread\":\"t100-1\",\"text\":\"one more thing\"}}")), "SettingSet MessagePosted TurnQueued")
    C.Check.str("coming back records the ask", M.Setting.get(state(back()), "wt.again.t100-1", ""), "5000")
    C.Check.str("coming back asks once", outs(step(back(), 5010n, "{\"id\":4,\"m\":\"turn.start\",\"p\":{\"thread\":\"t100-1\",\"text\":\"and this\"}}")), "")
    C.Check.str("back runs there", outs(step(back(), 5020n, "{\"id\":0,\"m\":\"thread.worktree\",\"p\":{\"thread\":\"t100-1\",\"branch\":\"backplane/0a1b2c3d\",\"path\":\"/r/.backplane/worktrees/backplane-0a1b2c3d/board\"}}")),
      "git:checkpoint0@/r/.backplane/worktrees/backplane-0a1b2c3d/board() git:step@/r/.backplane/worktrees/backplane-0a1b2c3d/board() start@/r/.backplane/worktrees/backplane-0a1b2c3d/board send")
    C.Check.str("codec removed", roundtrip(M.WorktreeRemoved{"a", 3n}), "{\"$\":\"WorktreeRemoved\",\"id\":\"a\",\"at\":3}")
