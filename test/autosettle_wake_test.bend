import Base
import ./check.bend as C
import ./autosettle_test.bend as F
import ../src/core/autosettle.bend as A
import ../src/core/model.bend as M
import ../src/core/hub.bend as B

def due(+s: M.State) -> String:
  Bool.show(A.Auto.due(s, M.State.thread.get(s, "t"), 50000n, A.Auto.index(s)))

def target() -> M.Change:
  M.ThreadCreated{"other", "p", "Other", M.Local{}, "", "", "codex", "", 1n}

def job.id() -> String:
  "improvement-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"

def job.seed() -> String:
  "improvement-request-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"

def job(cs: List<&2, M.Change>) -> M.State:
  M.State.replay(List.append(&2, M.Change, [M.ProjectCreated{"p", "Project", "/tmp/autosettle", 1n},
    M.ThreadCreated{job.id(), "p", "Improvement", M.Local{}, "", "", "codex", "", 1n},
    M.MessagePosted{job.id(), job.seed(), M.User{}, "Fix this bug", 2n},
    M.MessagePosted{job.id(), "answer", M.Assistant{}, "Fixed", 4n}, M.TurnChanged{job.id(), M.Completed{}, 5n}], cs), M.State.empty())

def job.due(+s: M.State, now: Nat) -> String:
  Bool.show(A.Auto.due(s, M.State.thread.get(s, job.id()), now, A.Auto.index(s)))

def main() -> IO(Unit):
  +gone : M.State = F.base(True{}, [M.WakeSet{"wake", "t", "work", "w-other-1", ["end"], True{}, "Wait", 6n}])
  +live : M.State = F.base(True{}, [target(), M.WorkSet{"other", "watch", "w-other-1\twatching\t\tbuild\trun", 6n}, M.WakeSet{"wake", "t", "work", "w-other-1", ["end"], True{}, "Wait", 6n}])
  +file : M.State = F.base(True{}, [M.WakeSet{"wake", "t", "file", "/tmp/receipt.json", ["changed"], True{}, "Wait", 6n}])
  +settled : M.State = M.State.replay(A.Auto.changes(file, 50000n), file)
  +cleaned : M.State = M.State.replay(A.Auto.changes(settled, 50060n), settled)
  do IO<Unit>:
    C.Check.str("wake on finished work is not owned running work", due(gone), "True")
    C.Check.str("wake on live watched work holds subscriber", due(live), "False")
    C.Check.str("passive file subscription allows completed quiet owner to settle", due(file), "True")
    C.Check.str("settled owner automatically retires passive subscription", M.Setting.get(cleaned, "wakes.by.t", ""), "")
    C.Check.str("settled owner automatically retires stale work subscription", M.Setting.get(M.State.replay(A.Auto.changes(M.State.replay(A.Auto.changes(gone, 50000n), gone), 50060n), M.State.replay(A.Auto.changes(gone, 50000n), gone)), "wakes.by.t", ""), "")
    C.Check.str("wake on completed thread is stale", due(F.base(True{}, [target(), M.TurnChanged{"other", M.Completed{}, 6n}, M.WakeSet{"wake", "t", "thread", "other", ["end"], True{}, "Wait", 6n}])), "True")
    C.Check.str("wake on completed thread with a pending ask stays held", due(F.base(True{}, [target(), M.TurnChanged{"other", M.Completed{}, 6n}, M.AskOpened{"a", "other", "approval", "Bash", "Allow?", "", 7n}, M.WakeSet{"wake", "t", "thread", "other", ["end"], True{}, "Wait", 6n}])), "False")
    C.Check.str("indirect wake chain retains real running work", due(F.base(True{}, [target(), M.WorkSet{"other", "watch", "w-other-1\twatching", 6n}, M.ThreadCreated{"middle", "p", "Middle", M.Local{}, "", "", "codex", "", 1n}, M.WakeSet{"a", "middle", "work", "w-other-1", ["end"], True{}, "Wait", 6n}, M.WakeSet{"b", "t", "thread", "middle", ["end"], True{}, "Wait", 6n}])), "False")
    C.Check.str("idle wake cycle is not running work", due(F.base(True{}, [target(), M.TurnChanged{"other", M.Completed{}, 6n}, M.WakeSet{"a", "other", "thread", "t", ["end"], True{}, "Wait", 6n}, M.WakeSet{"b", "t", "thread", "other", ["end"], True{}, "Wait", 6n}])), "True")
    C.Check.str("wake cycle with real work remains held", due(F.base(True{}, [target(), M.WorkSet{"other", "watch", "w-other-1\twatching", 6n}, M.WakeSet{"a", "other", "thread", "t", ["end"], True{}, "Wait", 6n}, M.WakeSet{"b", "t", "thread", "other", ["end"], True{}, "Wait", 6n}])), "False")
    C.Check.str("unknown wake kind stays conservative", due(F.base(True{}, [M.WakeSet{"wake", "t", "future-kind", "other", ["end"], True{}, "Wait", 6n}])), "False")
    C.Check.str("stale wake never bypasses unread protection", due(F.base(False{}, [M.WakeSet{"wake", "t", "work", "gone", ["end"], True{}, "Wait", 6n}])), "False")
    C.Check.str("unopened improvement job uses worker grace", job.due(job(Nil{}), 905n), "True")
    C.Check.str("improvement job still observes worker grace", job.due(job(Nil{}), 904n), "False")
    C.Check.str("improvement seed metadata does not become personal", M.Setting.get(M.State.replay(B.Shelve.auto.fix(job(Nil{}), [M.MessagePosted{job.id(), job.seed(), M.User{}, "Fix this bug", 2n}]), job(Nil{})), "autosettle.audience." ++ job.id(), ""), "worker")
    C.Check.str("restart continuation is not a person's unread input", job.due(job([M.MessagePosted{job.id(), "retry6-10", M.User{}, M.Carry.text() ++ " Continue from your last step.", 6n}]), 906n), "True")
    C.Check.str("genuine human followup still requires reading", job.due(job([M.MessagePosted{job.id(), "human", M.User{}, "Do one more thing", 6n}]), 50000n), "False")
    C.Check.str("unread human input with a bracket stays protected", job.due(job([M.MessagePosted{job.id(), "human", M.User{}, "[Please] do one more thing", 6n}]), 50000n), "False")
    C.Check.str("pending improvement approval stays visible", job.due(job([M.AskOpened{"approval", job.id(), "approval", "Bash", "Allow?", "", 6n}]), 50000n), "False")
