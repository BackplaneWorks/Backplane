import Base
import ./check.bend as C
import ./settle_life_test.bend as L
import ../src/core/model.bend as M
import ../src/core/hub.bend as B
import ../src/core/client.bend as U
import ../src/core/codec.bend as K
import ../src/core/far.bend as F

# Existing work remains alive when manually settled. A distinct accepted
# work identity after that decision must wake it even without a busy edge.
def held() -> M.State:
  L.log([M.WorkSet{"t1", "bg", "old\tworking\tc0\tOld job\tshell", 3n}, M.ThreadSettled{"t1", True{}, 4n}])

def monitor() -> M.State:
  M.State.replay([M.TurnChanged{"t1", M.Completed{}, 5n}], held())

def idle() -> M.State:
  L.log([M.TurnChanged{"t1", M.Completed{}, 3n}, M.ThreadSettled{"t1", True{}, 4n}])

def job(+at: Nat) -> List<&2, M.Change>:
  [M.WorkSet{"t1", "bg", "old\tworking\tc0\tOld job\tshell\nnew\tworking\tc1\tNew agent\tagent", at}]

def phase(+s: M.State) -> String:
  M.Phase.name(U.Side.phase(s, M.State.thread.get(s, "t1")))

def marked(cs: List<&2, M.Change>, +n: Nat) -> List<&2, F.Item>:
  match cs:
    case Nil{}: Nil{}
    case c <> rest: F.Item{n, K.Change.wire(c)} <> marked(rest, 1n+n)

def remote_log() -> List<&2, M.Change>:
  [M.ThreadCreated{"t1", "", "Remote", M.Local{}, "", "", "codex", "", 1n},
   M.TurnChanged{"t1", M.Completed{}, 2n}, M.WorkSet{"t1", "bg", "old\tworking", 3n},
   M.ThreadSettled{"t1", True{}, 4n}]

def remote() -> M.State:
  F.Fr.fold(M.State.empty(), "far", 0n, 4n, marked(remote_log(), 1n))

def remote_work() -> List<&2, M.Change>:
  List.append(&2, M.Change, job(6n), B.Shelve.step(monitor(), job(6n)))

def remote_awake() -> M.State:
  F.Fr.fold(remote(), "far", 4n, 6n, marked(remote_work(), 5n))

# A fixed 12,000-entry history puts a duplicate at the deep end and forces
# a missing user identity to traverse the whole list, without a timing test.
def history(+n: Nat, s: M.State) -> M.State:
  match n:
    case 0n: s
    case 1n+k:
      history(k, M.State.apply.go(s, M.MessagePosted{"t1", "history:" ++ Nat.show(k), M.Assistant{}, "prior reply", 3n}))

def long_history() -> M.State:
  history(12000n, L.log([M.MessagePosted{"t1", "deep-duplicate", M.User{}, "earlier input", 3n},
    M.WorkSet{"t1", "bg", "old\tworking", 3n}, M.ThreadSettled{"t1", True{}, 4n}]))

def main() -> IO(Unit):
  +long = long_history()
  +newjob = L.after(monitor(), job(6n))
  +cleared = L.after(newjob, [M.WorkSet{"t1", "bg", "", 7n}])
  +coordinator = B.Queue.changes(B.Hub.inbox.id(held(), 6n, "coordinator", "t1", M.User{}, "Resume", "coordinator:1"))
  +watched = M.State.replay([M.WorkSet{"t1", "watch", B.Watch.line("w1", "watching", "Old monitor", "file", "/tmp/old", "i1", 100n), 3n}], held())
  do IO<Unit>:
    C.Check.str("existing manual settled idle new turn already wakes", L.step(idle(), [M.MessagePosted{"t1", "m2", M.User{}, "go", 6n}, M.TurnChanged{"t1", M.Running{}, 6n}]), "wake:t1@6")
    C.Check.str("existing manual settled running new user input already wakes", L.step(held(), [M.MessagePosted{"t1", "m2", M.User{}, "go", 4n}]), "wake:t1@4")
    C.Check.str("actual coordinator delivery wakes a manually settled running turn", L.step(held(), coordinator), "wake:t1@6")
    C.Check.str("distinct queued input wakes while old work lives", L.step(held(), [M.TurnQueued{"t1", "q2", "resume", 6n}]), "wake:t1@6")
    C.Check.str("new owned agent during running turn wakes", L.step(held(), job(6n)), "wake:t1@6")
    C.Check.str("new owned agent during monitoring wakes", L.step(monitor(), job(6n)), "wake:t1@6")
    C.Check.str("new owned shell same second as manual settle wakes", L.step(held(), [M.WorkSet{"t1", "bg", "shell-new\tworking\tc2\tNew shell\tshell", 4n}]), "wake:t1@4")
    C.Check.str("same busy count replacement still is fresh work", L.step(monitor(), [M.WorkSet{"t1", "bg", "new\tworking", 6n}]), "wake:t1@6")
    C.Check.str("actual monitor start while another job runs wakes", L.step(monitor(), B.Queue.changes(B.Watch.start(monitor(), 6n, "t1", "file", "/tmp/fresh", "New monitor", 60n))), "wake:t1@6")
    C.Check.str("new delegated child while parent running wakes", L.step(held(), [M.TaskDelegated{"k-new", "t1", "child", 1n, "codex", "", 6n}]), "wake:t1@6")
    C.Check.str("coordinator turn resumes while an old owned job lives", L.step(monitor(), [M.TurnChanged{"t1", M.Running{}, 6n}]), "wake:t1@6")
    C.Check.str("new job makes shelf active", L.shelf_at(newjob, "t1"), "active")
    C.Check.str("completed main turn with owned jobs is monitoring", phase(newjob), "monitoring")
    C.Check.str("clearing one of several owned jobs does not settle", L.step(newjob, [M.WorkSet{"t1", "bg", "new\tworking", 7n}]), "")
    C.Check.str("all owned work completed settles again", L.shelf_at(cleared, "t1"), "settled")
    C.Check.str("repeated running state does not undo manual settle", L.step(held(), [M.TurnChanged{"t1", M.Running{}, 6n}]), "")
    C.Check.str("background heartbeat keeps identity and manual settle", L.step(held(), [M.WorkSet{"t1", "bg", "old\tprogress\tc0\tOld job\tshell", 6n}]), "")
    C.Check.str("unchanged work snapshot leaves manual settle", L.step(held(), [M.WorkSet{"t1", "bg", "old\tworking\tc0\tOld job\tshell", 6n}]), "")
    C.Check.str("watch launch acknowledgment leaves manual settle", L.step(watched, B.Queue.changes(B.Watch.identified(watched, 6n, "t1", "w1", "i2"))), "")
    C.Check.str("new ended callback is not fresh work", L.step(monitor(), [M.WorkSet{"t1", "bg", "old\tworking\nlate\t" ++ M.Bg.ended.mark() ++ "done", 6n}]), "")
    C.Check.str("old timestamp new row cannot supersede manual settle", L.step(held(), job(3n)), "")
    C.Check.str("old callback on settled idle thread cannot wake it", L.step(idle(), [M.TurnChanged{"t1", M.Running{}, 3n}]), "")
    C.Check.str("deleting old job does not wake", L.step(monitor(), [M.WorkSet{"t1", "bg", "", 6n}]), "")
    C.Check.str("duplicate delegation does not wake", L.step(M.State.replay([M.TaskDelegated{"k1", "t1", "child", 1n, "codex", "", 3n}], held()), [M.TaskDelegated{"k1", "t1", "child", 1n, "codex", "", 6n}]), "")
    C.Check.str("duplicate queued identity does not wake", L.step(M.State.replay([M.TurnQueued{"t1", "q1", "old", 3n}], held()), [M.TurnQueued{"t1", "q1", "old", 6n}]), "")
    C.Check.str("assistant reply and tool read remain passive", L.step(held(), [M.MessagePosted{"t1", "reply", M.Assistant{}, "done", 6n}, M.ActivityLogged{"t1", "read", "tool", "Read", "file", 6n}]), "")
    C.Check.str("history reread retains manual settlement", L.shelf_at(M.State.replay(L.reread(remote_log()), M.State.empty()), "t1"), "settled")
    C.Check.str("tick with no fresh work retains manual settlement", L.tickon(monitor(), "t1"), "")
    C.Check.str("work followed by manual settle in one transaction stays settled", L.step(held(), [M.WorkSet{"t1", "bg", "new\tworking", 6n}, M.ThreadSettled{"t1", True{}, 6n}]), "")
    C.Check.str("manual settle then work in same transaction wakes", L.step(held(), [M.ThreadSettled{"t1", True{}, 6n}, M.WorkSet{"t1", "bg", "new\tworking", 6n}]), "wake:t1@6")
    C.Check.str("input followed by manual settle in same transaction stays settled", L.step(held(), [M.MessagePosted{"t1", "m2", M.User{}, "resume", 6n}, M.ThreadSettled{"t1", True{}, 6n}]), "")
    C.Check.str("idle work followed by manual settle also stays settled", L.step(idle(), [M.TurnChanged{"t1", M.Running{}, 6n}, M.ThreadSettled{"t1", True{}, 6n}]), "")
    C.Check.str("failure remains failed after new work finishes", phase(L.after(L.after(M.State.replay([M.TurnChanged{"t1", M.Failed{}, 5n}], held()), job(6n)), [M.WorkSet{"t1", "bg", "", 7n}])), "failed")
    C.Check.str("Stop remains stopped after new work finishes", phase(L.after(L.after(M.State.replay([M.TurnChanged{"t1", M.Interrupted{}, 5n}], held()), job(6n)), [M.WorkSet{"t1", "bg", "", 7n}])), "stopped")
    C.Check.str("approval precedence survives automatic wake", phase(L.after(M.State.replay([M.AskOpened{"a1", "t1", "approval", "Bash", "May I?", "", 5n}], held()), job(6n))), "approval")
    C.Check.str("owner mirror receives work and wake", L.shelf_at(remote_awake(), "far~t1"), "active")
    C.Check.str("mirror also shows owned work monitoring", M.Phase.name(U.Side.phase(remote_awake(), M.State.thread.get(remote_awake(), "far~t1"))), "monitoring")
    C.Check.str("old far replay cannot restore settle", L.shelf_at(F.Fr.fold(remote_awake(), "far", 0n, 4n, marked(remote_log(), 1n)), "far~t1"), "active")
    C.Check.str("duplicate far batch stays active once", L.shelf_at(F.Fr.fold(remote_awake(), "far", 4n, 6n, marked(remote_work(), 5n)), "far~t1"), "active")
    C.Check.str("far ordered later manual settle wins", L.shelf_at(F.Fr.fold(remote_awake(), "far", 6n, 7n, marked([M.ThreadSettled{"t1", True{}, 7n}], 7n)), "far~t1"), "settled")
    C.Check.str("far gap cannot apply a future wake early", L.shelf_at(F.Fr.fold(remote(), "far", 5n, 6n, marked([M.ThreadWoke{"t1", 6n}], 6n)), "far~t1"), "settled")
    C.Check.str("long history deep duplicate is recognized without reopening", L.step(long, [M.MessagePosted{"t1", "deep-duplicate", M.User{}, "earlier input", 6n}]), "")
    C.Check.str("long history missing real user identity wakes", L.step(long, [M.MessagePosted{"t1", "fresh-after-history", M.User{}, "resume", 6n}]), "wake:t1@6")
    C.Check.str("long history passive assistant event bypasses identity lookup", Bool.show(B.Shelve.fresh.event(long, M.MessagePosted{"t1", "passive-reply", M.Assistant{}, "reply", 6n}, "t1")), "False")
    C.Check.str("long history other thread user event bypasses identity lookup", Bool.show(B.Shelve.fresh.event(long, M.MessagePosted{"other", "other-input", M.User{}, "go", 6n}, "t1")), "False")
    C.Check.str("long history stale user event bypasses identity lookup", Bool.show(B.Shelve.fresh.event(long, M.MessagePosted{"t1", "stale-input", M.User{}, "old", 3n}, "t1")), "False")
    C.Check.str("long history mixed running heartbeat and assistant remain passive", L.step(long, [M.TurnChanged{"t1", M.Running{}, 6n}, M.MessagePosted{"t1", "passive-reply", M.Assistant{}, "reply", 6n}]), "")
    C.Check.str("different seconds later manual settle wins over fresh work", L.shelf_at(L.after(held(), [M.WorkSet{"t1", "bg", "new\tworking", 6n}, M.ThreadSettled{"t1", True{}, 7n}]), "t1"), "settled")
    C.Check.str("different seconds fresh work after manual settle wins", L.shelf_at(L.after(held(), [M.ThreadSettled{"t1", True{}, 6n}, M.WorkSet{"t1", "bg", "new\tworking", 8n}]), "t1"), "active")
    C.Check.str("off: a finished thread stays active and shows complete", L.shelf_at(L.after(L.off(Nil{}), L.done()), "t1") ++ " " ++ phase(L.after(L.off(Nil{}), L.done())), "active complete")
    C.Check.str("off: all owned work completed leaves it active", L.shelf_at(L.after(L.after(L.off([M.WorkSet{"t1", "bg", "old\tworking", 3n}, M.TurnChanged{"t1", M.Completed{}, 5n}]), job(6n)), [M.WorkSet{"t1", "bg", "", 7n}]), "t1"), "active")
    C.Check.str("different seconds fresh then manual then fresh reopens", L.shelf_at(L.after(held(), [M.WorkSet{"t1", "bg", "first\tworking", 6n}, M.ThreadSettled{"t1", True{}, 7n}, M.WorkSet{"t1", "bg", "second\tworking", 8n}]), "t1"), "active")
