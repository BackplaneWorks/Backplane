import Base
import ../src/core/i18n.bend as I
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/codec.bend as K
import ../src/core/hub.bend as H
import ../src/core/bothub.bend as BH
import ../src/core/json.bend as J
import ../src/core/client.bend as U

# Scratch threads: the Scratch project is made once, a scratch thread gets
# a folder of its own, one left alone past its window is deleted (not when
# pinned), and /give hands a conversation to threads and projects.

def changes.go(cs: List<&2, M.Change>) -> List<&2, String>:
  match cs:
    case Nil{}:
      Nil{}
    case c <> t:
      J.Json.get_str(K.Change.encode(c), "$", "") <> changes.go(t)

def tags(st: H.Step) -> String:
  String.join(changes.go(H.Queue.changes(st)), " ")

# whether a step deletes a thread (settling on the way is the usual tick's)
def dels(st: H.Step) -> String:
  Bool.show(String.contains(tags(st), "ThreadDeleted"))

def out.one(o: H.Out) -> String:
  match o:
    case H.Git{_, _, job, _, cwd, _, _}:
      "git:" ++ job ++ "@" ++ cwd
    case H.BotFx{k, a, _, _}:
      k ++ ":" ++ a
    case H.AgentStop{_}:
      "stop"
    case H.Send{_, text}:
      Bool.pick(String, String.contains(text, "\"error\""), "err", "reply")
    case _:
      "other"

def outs.go(os: List<&2, H.Out>) -> List<&2, String>:
  match os:
    case Nil{}:
      Nil{}
    case o <> t:
      out.one(o) <> outs.go(t)

def outs(st: H.Step) -> String:
  String.join(outs.go(H.Step.outs_of(st)), " ")

def base() -> List<&2, M.Change>:
  [M.ProjectCreated{"scratch", "Scratch", "/h/scratch", 1n}, M.ProjectCreated{"p1", "P", "/r/board", 1n},
   M.ThreadCreated{"t1", "scratch", "Idea", M.Local{}, "", "", "claude", "", 10n},
   M.WorktreeSet{"t1", "", "/h/scratch/t1", 11n},
   M.MessagePosted{"t1", "u1", M.User{}, "what about a buck converter", 12n},
   M.ThreadCreated{"t2", "p1", "Board", M.Local{}, "", "", "claude", "", 20n}]

def state(es: List<&2, M.Change>) -> M.State:
  M.State.replay(es, M.State.empty())

def req(+m: String, +p: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse("{\"id\":1,\"m\":\"" ++ m ++ "\",\"p\":" ++ p ++ "}"), J.Null{})

def give(+es: List<&2, M.Change>, +text: String) -> H.Step:
  BH.Bots.request(state(es), 100n, 1, req("bots.give", "{\"thread\":\"t1\",\"text\":\"" ++ text ++ "\"}"))

def queued(+es: List<&2, M.Change>, +th: String) -> String:
  M.Queue.of(state(es), th)

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("made once", tags(H.Step{M.Scratch.ensure(state(Nil{}), "/h/scratch", 1n), Nil{}}), "ProjectCreated")
    C.Check.str("not twice", tags(H.Step{M.Scratch.ensure(state(base()), "/h/scratch", 1n), Nil{}}), "")
    +mk : H.Step = H.Hub.request_text(state(base()), 30n, 1, "{\"id\":1,\"m\":\"thread.create\",\"p\":{\"project\":\"scratch\",\"env\":\"worktree\"}}")
    C.Check.str("a scratch thread gets its own folder, no worktree", outs(mk), "reply scratch:t30-6")
    C.Check.str("works locally", tags(mk), "ThreadCreated")
    C.Check.str("in its folder from the start", H.Hub.cwd(state(List.append(&2, M.Change, base(), H.Queue.changes(mk))), H.Hub.thread(state(List.append(&2, M.Change, base(), H.Queue.changes(mk))), "t30-6")), "/h/scratch/t30-6")
    C.Check.str("a fork gets a folder of its own", outs(H.Hub.request_text(state(base()), 30n, 1, "{\"id\":1,\"m\":\"thread.fork\",\"p\":{\"thread\":\"t1\"}}")), "reply scratch:t30-6")
    # a thread the log never made (a phone once sent its row as "host:port|t1") forks nothing
    C.Check.str("a fork of no thread is refused", outs(H.Hub.request_text(state(base()), 30n, 1, "{\"id\":1,\"m\":\"thread.fork\",\"p\":{\"thread\":\"10.0.2.2:41693|t1\"}}")), "err")
    C.Check.str("and logs nothing", tags(H.Hub.request_text(state(base()), 30n, 1, "{\"id\":1,\"m\":\"thread.fork\",\"p\":{\"thread\":\"10.0.2.2:41693|t1\"}}")), "")
    C.Check.str("a project thread still gets a worktree", outs(H.Hub.request_text(state(base()), 30n, 1, "{\"id\":1,\"m\":\"thread.create\",\"p\":{\"project\":\"p1\",\"env\":\"worktree\"}}")),
      "reply git:worktree@/r/board")
    +week : Nat = Nat.add(12n, 604800n)
    C.Check.str("kept within its window", dels(H.Hub.tick(state(base()), Nat.sub(week, 1n))), "False")
    +old : H.Step = H.Hub.tick(state(base()), week)
    # Scratch is retired (docs/bots.md "Scratch"): earlier chats are never
    # expired, so the tick deletes nothing past the old window either
    C.Check.str("kept once its old window passed", dels(old), "False")
    C.Check.str("and its folder stays", outs(old), "")
    C.Check.str("pinned is kept", dels(H.Hub.tick(state(List.append(&2, M.Change, base(), [M.ThreadPinned{"t1", True{}, 13n}])), week)), "False")
    C.Check.str("one waiting on work is kept", dels(H.Hub.tick(state(List.append(&2, M.Change, base(), [M.WorkSet{"t1", "bg", "b1\tsleep\tc1\tsleep\tshell", 13n}])), week)), "False")
    C.Check.str("the old days setting deletes nothing", dels(H.Hub.tick(state(List.append(&2, M.Change, base(), [M.SettingSet{"scratch.days", "1", 13n}])), Nat.add(12n, 86400n))), "False")
    C.Check.str("gone is not swept again", dels(H.Hub.tick(state(List.append(&2, M.Change, base(), [M.ThreadDeleted{"t1", 13n}])), week)), "False")
    +gt : H.Step = give(base(), "/give >board look at this")
    C.Check.str("give to a thread", tags(gt), "TurnQueued ActivityLogged ActivityLogged")
    +es : List<&2, M.Change> = List.append(&2, M.Change, base(), H.Queue.changes(gt))
    C.Check.str("the thread has the conversation", Bool.show(String.contains(queued(es, "t2"), "User: what about a buck converter")), "True")
    C.Check.str("with the note", Bool.show(String.contains(queued(es, "t2"), "note with it: >board look at this")), "True")
    +gp : H.Step = give(base(), "/give %p")
    C.Check.str("give to a project", tags(gp), "ThreadCreated ModesSet TurnQueued ActivityLogged")
    C.Check.str("a project thread's worktree", outs(gp), "reply git:worktree@/r/board")
    +bots : List<&2, M.Change> = List.append(&2, M.Change, base(), [M.ThreadCreated{"tb", "", "helper", M.Local{}, "", "", "claude", "", 5n}, M.BotSet{"b1", "helper", "tb", 0n, "", 5n}])
    +gb : H.Step = give(bots, "/give @helper summarise")
    C.Check.str("give to a bot wakes it", tags(gb), "BotWoke MessagePosted MessageFrom TurnChanged ActivityLogged")
    C.Check.str("naming nothing is refused", outs(give(base(), "/give nothing")), "err")
    C.Check.str("never to itself", outs(give(base(), "/give >idea")), "err")
    C.Check.str("button says Give", U.Follow.button(I.En{}, state(base()), "t1", False{}, False{}, "/give >board"), "Give")
