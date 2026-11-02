import Base
import ./check.bend as C
import ./stream_step_proof.bend as P
import ../src/core/stream-step.bend as S
import ../src/core/hub.bend as H
import ../src/core/model.bend as M
import ../src/core/json.bend as J

def signature(os: List<&2, H.Out>) -> List<&2, J.Json>:
  match os:
    case Nil{}: Nil{}
    case H.Live{th, text} <> rest:
      J.Json.arr([J.Str{th}, J.Str{text}]) <> signature(rest)
    case other <> rest: J.Str{H.Out.kind(other)} <> signature(rest)

def shown(step: H.Step) -> String:
  match step:
    case H.Step{cs, os}:
      Nat.show(List.length(&2, M.Change, cs)) ++ ":" ++ J.Json.show(J.Json.arr(signature(os)))

def rows(n: Nat) -> List<&2, P.LiveRow>:
  match n:
    case 0n: Nil{}
    case 1n+next:
      +k = next
      P.LiveRow{"thread-" ++ Nat.show(Nat.mod(k, 20n)), "delta \"\\\n Unicode λ " ++ Nat.show(k)} <> rows(k)

def state() -> M.State:
  M.State.replay([M.ProjectCreated{"p", "p", "/p", 1n},
    M.ThreadCreated{"thread-1", "p", "thread", M.Local{}, "", "", "claude", "", 1n},
    M.TurnChanged{"thread-1", M.Running{}, 2n},
    M.SettingSet{"mail.on", "on", 2n}, M.SettingSet{"fallback.on", "on", 2n},
    M.SettingSet{"fallback.chain", "unavailable/model", 2n},
    M.SettingSet{"thread.config.hold.thread-1", "on", 2n},
    M.AskOpened{"proposal", "thread-1", "input", "settings_proposal", "pending", "", 3n}], M.State.empty())

def parity(+name: String, +s: M.State, +caps: J.Json, +os: List<&2, H.Out>) -> IO(Unit):
  do IO<Unit>:
    C.Check.str(name ++ " eligible", Bool.show(S.StreamStep.eligible(H.Step{Nil{}, os})), "True")
    C.Check.str(name ++ " validate", shown(H.Fallback.validate(s, caps, 100n, H.Step{Nil{}, os})), shown(H.Step{Nil{}, os}))
    C.Check.str(name ++ " run fallback", shown(H.Run.fallback(s, 100n, H.Step{Nil{}, os})), shown(H.Step{Nil{}, os}))
    C.Check.str(name ++ " combined", shown(H.Run.fallback(s, 100n, H.Fallback.validate(s, caps, 100n, H.Step{Nil{}, os}))), shown(H.Step{Nil{}, os}))

def reject(name: String, step: H.Step) -> IO(Unit):
  C.Check.str(name, Bool.show(S.StreamStep.eligible(step)), "False")

def main() -> IO(Unit):
  do IO<Unit>:
    parity("empty", M.State.empty(), J.Null{}, Nil{})
    parity("single delta", state(), J.Null{}, [H.Live{"thread-1", "first"}])
    parity("mixed owners, order and payload preserved", state(), J.Json.obj([J.KV{"claude", J.Json.arr([J.Str{"model"}])}]), P.outputs(rows(80n)))
    parity("800 deltas and unavailable capabilities", state(), J.Json.obj(Nil{}), P.outputs(rows(800n)))
    reject("changes alone never bypass", H.Step{[M.SettingSet{"fallback.on", "on", 1n}], Nil{}})
    reject("changes beside Live never bypass", H.Step{[M.SettingSet{"liveness.thread-1", "1", 1n}], [H.Live{"thread-1", "delta"}]})
    reject("settings mutation cannot bypass validation", H.Step{[M.SettingSet{"fallback.chain", "invalid", 1n}], [H.Live{"thread-1", "delta"}]})
    reject("provider startup cannot bypass", H.Step{Nil{}, [H.CodexTurn{"thread-1", "/p", "model", "", M.FullAccess{}, "go"}]})
    reject("Live then send cannot bypass", H.Step{Nil{}, [H.Live{"thread-1", "delta"}, H.Send{1, "reply"}]})
    reject("send then Live cannot bypass", H.Step{Nil{}, [H.Send{1, "reply"}, H.Live{"thread-1", "delta"}]})
    reject("Live then Git then Live cannot bypass", H.Step{Nil{}, [H.Live{"thread-1", "a"}, H.Git{0, "", "checkpoint0", "thread-1", "/p", "", ""}, H.Live{"thread-1", "b"}]})
    reject("agent send cannot bypass", H.Step{Nil{}, [H.AgentSend{"thread-1", "go"}]})
