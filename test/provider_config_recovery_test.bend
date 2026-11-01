import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/hub.bend as H
import ../src/core/mcp.bend as MC
import ../src/core/json.bend as J
import ../src/core/codec.bend as CD
import ../src/core/accounts.bend as A
import ../src/core/watchdog.bend as W
import ../src/core/restartretry.bend as R

def text() -> String:
  "Finish the original task 日本語 once; keep results.  \n"

def json(s: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(s), J.Null{})

# An interrupted ordinary thread with real held input, genuine history,
# provider-owned sessions, named accounts and conservative permissions.
def held() -> M.State:
  M.State.replay([M.MessagePosted{"t1", "original", M.User{}, text(), 3n},
    M.MessagePosted{"t1", "result", M.Assistant{}, "Step one completed", 4n},
    M.SessionSet{"t1", "claude-session", 4n},
    M.SettingSet{"session.t1.codex", "codex-session", 4n},
    M.ModesSet{"t1", M.Supervised{}, M.PlanMode{}, "opus", 4n},
    M.SettingSet{A.Accounts.binding.key("t1", "claude"), A.Accounts.binding.show("owner-A", 2n), 4n},
    M.SettingSet{A.Accounts.binding.key("t1", "codex"), A.Accounts.binding.show("owner-B", 3n), 4n},
    M.TaskDelegated{"original-task", "parent", "t1", 1n, "claude", "opus", 4n},
    M.TaskStateSet{"original-task", M.TaskRunning{}, "Step one completed", 4n},
    M.TurnQueued{"t1", "fallback:held:original", text(), 5n},
    M.TurnQueued{"t1", "handoff:fork", "Genuine fork context", 5n},
    M.SettingSet{"fallback.input.t1", text(), 5n},
    M.SettingSet{"provider.resume.t1", "{\"target\":\"codex/gpt-6.1-sol\",\"reset\":11}", 5n}], H.Demo.state("claude", M.Interrupted{}))

def pick(+s: M.State) -> H.Step:
  H.Hub.request(s, 10n, 0, json("{\"id\":1,\"m\":\"thread.provider\",\"p\":{\"thread\":\"t1\",\"provider\":\"codex\",\"model\":\"gpt-6.1-sol\"}}"))

def result(r: MC.Res) -> H.Step:
  match r:
    case MC.Res{_, _, step}: step

def config(+s: M.State, +args: String) -> H.Step:
  result(MC.Mcp.call.k(CD.Codec.pos(MC.Mcp.names(), "thread_update"), s, 10n, "t1", J.Null{}, json(args)))

def fold(+s: M.State, step: H.Step) -> M.State:
  M.State.replay(H.Queue.changes(step), s)

def dispatch(os: List<&2, H.Out>) -> Nat:
  match os:
    case Nil{}: 0n
    case H.AgentStart{_, _, _, _, _, _, _} <> rest: 1n+dispatch(rest)
    case H.AgentSend{_, _} <> rest: 1n+dispatch(rest)
    case H.CodexTurn{_, _, _, _, _, _} <> rest: 1n+dispatch(rest)
    case H.GrokTurn{_, _, _, _, _, _} <> rest: 1n+dispatch(rest)
    case _ <> rest: dispatch(rest)

def starts(step: H.Step) -> String:
  Nat.show(dispatch(H.Step.outs_of(step)))

# Both raw replay and the server's Fallback.manual path must stay stopped.
def chosen() -> M.State:
  fold(held(), config(held(), "{\"provider\":\"codex\",\"model\":\"gpt-6.1-sol\"}"))

def applied() -> M.State:
  fold(held(), H.Fallback.after(held(), J.Null{}, 10n, config(held(), "{\"provider\":\"codex\",\"model\":\"gpt-6.1-sol\"}")))

def release.held() -> M.State:
  M.State.replay(H.Resume.set(held(), 5n, "t1", "carry"), held())

def ride.held() -> M.State:
  fold(held(), H.Ride.wait(held(), 5n, "t1", "waiting", M.User{}, "An already queued request"))

def retry.held() -> M.State:
  +s = held()
  M.State.replay([M.SettingSet{"restart.retry.t1", J.Json.show(J.Json.obj([J.KV{"due", J.Num{"11"}}, J.KV{"target", J.Str{"codex/gpt-6.1-sol"}},
    J.KV{"activity", J.Json.nat(R.Retry.activity(H.Hub.thread(s, "t1")))}, J.KV{"message", J.Str{"original"}}])), 5n}], s)

def auth.held() -> M.State:
  M.State.replay([M.SettingSet{"auth.wait.t1", "codex/gpt-6.1-sol", 5n}], held())

def worktree.held() -> M.State:
  M.State.replay([M.ThreadCreated{"t1", "p1", "waiting for worktree", M.Worktree{}, "", "", "claude", "opus", 1n}, M.TurnChanged{"t1", M.Interrupted{}, 5n}], held())

def stops(os: List<&2, H.Out>) -> Nat:
  match os:
    case Nil{}: 0n
    case H.AgentStop{_} <> rest: 1n+stops(rest)
    case _ <> rest: stops(rest)

def task(s: M.State) -> String:
  MC.McpOut.reply(MC.Mcp.handle(s, 20n, "parent", J.Null{}, "test", json("{\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"task_status\",\"arguments\":{\"taskId\":\"original-task\"}}}")))

def permissions.thread(t: M.Thread) -> String:
  match t:
    case M.Thread{_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, rt, im, _, _}:
      CD.Runtime.name(rt) ++ "/" ++ CD.Interaction.name(im)

def permissions(s: M.State) -> String:
  permissions.thread(H.Hub.thread(s, "t1"))

def original(qs: List<&2, M.QItem>) -> String:
  match qs:
    case Nil{}: "missing"
    case M.QItem{+id, +text} <> rest: Bool.pick(String, String.eq(id, "fallback:held:original"), text, original(rest))

def same.model() -> M.State:
  M.State.replay([M.ProviderSet{"t1", "codex", "old-model", 6n}], held())

def next.queue(+s: M.State) -> M.State:
  M.State.replay([M.TurnQueued{"t1", "next-real", "A later real request", 21n}], s)

def main() -> IO(Unit):
  +post = applied()
  +raw = chosen()
  +released = fold(release.held(), config(release.held(), "{\"provider\":\"codex\",\"model\":\"gpt-6.1-sol\"}"))
  do IO<Unit>:
    C.Check.str("MCP quota-held config has no immediate dispatch", starts(config(held(), "{\"provider\":\"codex\",\"model\":\"gpt-6.1-sol\"}")), "0")
    C.Check.str("raw configured thread cannot resume on reset", starts(H.ProviderResume.tick(raw, 11n)), "0")
    C.Check.str("raw thread.provider cannot resume on reset", starts(H.ProviderResume.tick(fold(held(), pick(held())), 11n)), "0")
    C.Check.str("fallback manual may clear provider marker", M.Setting.get(post, "provider.resume.t1", "missing"), "")
    C.Check.str("cleared quota marker never releases real queue", starts(H.Wt.start(post, 11n, "t1", False{})), "0")
    C.Check.str("late terminal callback cannot drain configured queue", starts(H.Queue.drain(post, 11n, "t1", H.Step{[M.TurnChanged{"t1", M.Completed{}, 11n}], Nil{}})), "0")
    C.Check.str("queued ride hook cannot start configured thread", starts(H.Ride.after(fold(ride.held(), config(ride.held(), "{\"provider\":\"codex\",\"model\":\"gpt-6.1-sol\"}")), 11n, H.Step{[M.TurnChanged{"t1", M.Interrupted{}, 11n}], Nil{}})), "0")
    C.Check.str("restart release cannot carry configured thread", starts(H.Rec.release(released, 11n)), "0")
    C.Check.str("watchdog and reset tick dispatch nothing", starts(W.Watchdog.tick(post, 1000n)), "0")
    C.Check.str("configuration preserves interrupted lifecycle", CD.Turn.name(M.Thread.turn(H.Hub.thread(post, "t1"))), "interrupted")
    C.Check.str("configuration keeps original queued input verbatim", original(M.Queue.items(post, "t1")), text())
    C.Check.str("configuration keeps genuine queued context", Bool.show(String.contains(M.Queue.of(post, "t1"), "Genuine fork context")), "True")
    C.Check.str("configuration keeps saved turn input", M.Setting.get(post, "fallback.input.t1", ""), text())
    C.Check.str("configuration restores selected provider session", M.Switch.sess(H.Hub.thread(post, "t1")), "codex-session")
    C.Check.str("configuration retains prior provider session", M.Setting.get(post, "session.t1.claude", ""), "claude-session")
    C.Check.str("configuration retains claude account", A.Accounts.id(post, "t1", "claude"), "owner-A")
    C.Check.str("configuration retains codex account", A.Accounts.id(post, "t1", "codex"), "owner-B")
    C.Check.str("unavailable selected account cannot fall back", Bool.show(A.Accounts.allowed(post, "t1", "codex")), "False")
    C.Check.str("configuration retains permissions", permissions(post), permissions(held()))
    C.Check.str("configuration leaves original task untouched", task(post), task(held()))
    C.Check.str("model-only selection cannot trigger quota resume", starts(H.ProviderResume.tick(fold(same.model(), config(same.model(), "{\"model\":\"gpt-6.1-sol\"}")), 11n)), "0")
    C.Check.str("reselect same provider still holds queued input", starts(H.Wt.start(fold(post, config(post, "{\"provider\":\"codex\",\"model\":\"gpt-6.1-sol\"}")), 12n, "t1", False{})), "0")
    C.Check.str("UI configuration does not stop the old idle process", Nat.show(stops(H.Step.outs_of(pick(held())))), "0")
    C.Check.str("restart retry cannot dispatch configured task", starts(R.Retry.tick(fold(retry.held(), config(retry.held(), "{\"provider\":\"codex\",\"model\":\"gpt-6.1-sol\"}")), 11n)), "0")
    C.Check.str("keyring recovery cannot release configuration hold", starts(H.Auth.recover(True{}, fold(auth.held(), config(auth.held(), "{\"provider\":\"codex\",\"model\":\"gpt-6.1-sol\"}")), 11n)), "0")
    C.Check.str("configuration leaves credential recovery identity intact", M.Setting.get(fold(auth.held(), config(auth.held(), "{\"provider\":\"codex\",\"model\":\"gpt-6.1-sol\"}")), "auth.wait.t1", ""), "codex/gpt-6.1-sol")
    C.Check.str("stale worktree hook cannot drain configured queue", starts(H.Wt.stale(H.Rec.threads(fold(worktree.held(), config(worktree.held(), "{\"provider\":\"codex\",\"model\":\"gpt-6.1-sol\"}"))), fold(worktree.held(), config(worktree.held(), "{\"provider\":\"codex\",\"model\":\"gpt-6.1-sol\"}")), 1000n)), "0")
    C.Check.str("original task control is real", Bool.show(String.contains(task(held()), "Step one completed")), "True")
    C.Check.str("explicit resume remains available", starts(H.Hub.resume(post, 20n, 0, "1", "t1")), "1")
    C.Check.str("new real prompt remains available", starts(H.Hub.post(post, 20n, "t1", "next", M.User{}, "Continue the task")), "1")
    C.Check.str("explicit resume restores later ordinary queue delivery", starts(H.Queue.drain(next.queue(fold(post, H.Hub.resume(post, 20n, 0, "1", "t1"))), 22n, "t1", H.Step{[M.TurnChanged{"t1", M.Completed{}, 22n}], Nil{}})), "1")
    C.Check.str("real prompt restores later ordinary queue delivery", starts(H.Queue.drain(next.queue(fold(post, H.Fallback.after(post, J.Null{}, 20n, H.Hub.post(post, 20n, "t1", "next", M.User{}, "Continue the task")))), 22n, "t1", H.Step{[M.TurnChanged{"t1", M.Completed{}, 22n}], Nil{}})), "1")
    C.Check.str("configuration hold survives persisted replay", starts(H.Wt.start(M.State.replay([M.SettingSet{"provider.resume.t1", "", 11n}], raw), 12n, "t1", False{})), "0")
    C.Check.str("late assistant output cannot release saved work", starts(H.Wt.start(fold(post, H.Fallback.after(post, J.Null{}, 11n, H.Step{[M.MessagePosted{"t1", "late-output", M.Assistant{}, "late provider output", 11n}], Nil{}})), 12n, "t1", False{})), "0")
    C.Check.str("configuration creates no new history", Nat.show(M.Switch.count(post, "t1")), Nat.show(M.Switch.count(held(), "t1")))
    C.Check.str("UI model-only configuration cannot trigger reset", starts(H.ProviderResume.tick(fold(same.model(), H.Hub.request(same.model(), 10n, 0, json("{\"id\":1,\"m\":\"thread.modes\",\"p\":{\"thread\":\"t1\",\"model\":\"gpt-6.1-sol\"}}"))), 11n)), "0")
