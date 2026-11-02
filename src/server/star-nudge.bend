import Base
import ../core/star-nudge.bend as S
import ../core/model.bend as M
import ../core/hub.bend as B
import ../core/json.bend as J

# Only Bot.request.client calls this, after the ordinary
# decider accepted the input. Forwarded hub, inbox/MCP/agent/automatic
# retry/wake paths never call it; explicit client retries carry a marker.
# A room fanout or a mention still counts just once.
def Star.method(+m: String) -> Bool:
  Bool.or(String.eq(m, "turn.start"), Bool.or(String.eq(m, "bots.say"), Bool.or(String.eq(m, "bots.post"), String.eq(m, "bots.tell"))))

def Star.accepted(cs: List<&2, M.Change>) -> String:
  match cs:
    case Nil{}: ""
    case M.MessagePosted{_, id, M.User{}, _, _} <> _: id
    case M.RoomPosted{id, _, "you", _, _, _} <> _: id
    case _ <> rest: Star.accepted(rest)

# Targeted info folds into just this connection's private Ui/Actor; it is
# not an OS alert and is never broadcast. Mark sent in the same commit as
# the fifth prompt, before the ordinary hub writer emits any Out. If that
# socket disappeared, prefer a missed suggestion to nagging on reconnect.
def Star.offer(+client: U32, +token: String) -> B.Out:
  B.SavedSend{client, J.Json.show(J.Json.obj([J.KV{"t", J.Str{"info"}}, J.KV{"info", J.Json.obj([J.KV{"star.nudge.offer", J.Str{token}}])}]))}

def Star.finish(+s: M.State, +now: Nat, +client: U32, +id: String, +step: B.Step) -> B.Step:
  +more = S.Star.accept(s, id, now)
  +after = M.State.replay(more, s)
  B.Step.before(more, Bool.pick(List<&2, B.Out>, Bool.and(Bool.not(List.is_empty(&2, M.Change, more)), S.Star.sent(S.Star.state(after))), [Star.offer(client, id)], Nil{}), step)

def Star.after.count(ok: Bool, +s: M.State, +now: Nat, +client: U32, step: B.Step) -> B.Step:
  match ok step:
    case False{} _: step
    case True{} B.Step{+cs, os}: Star.finish(s, now, client, Star.accepted(cs), B.Step{cs, os})

def Star.after(+s: M.State, +now: Nat, +client: U32, +o: J.Json, step: B.Step) -> B.Step:
  +p = J.Json.get_obj(o, "p")
  Star.after.count(Bool.and(U32.is_gt(client, 0), Bool.and(Bool.and(Star.method(J.Json.get_str(o, "m", "")), Bool.not(J.Json.get_bool(p, "retry", False{}))),
    Bool.and(Bool.not(String.is_empty(String.trim(J.Json.get_str(p, "text", "")))),
      Bool.or(Bool.not(String.eq(J.Json.get_str(o, "m", ""), "turn.start")), B.Hub.owns(s, J.Json.get_str(p, "thread", "")))))), s, now, client, step)

# A decision is reliable and idempotent: clients can resend it from their
# ordinary outbox. Neither side ever infers that a GitHub star was made.
def Star.saved(out: B.Out) -> B.Out:
  match out:
    case B.Send{client, text}: B.SavedSend{client, text}
    case _: out

def Star.action(+s: M.State, +now: Nat, +client: U32, +o: J.Json) -> B.Step:
  B.Step{S.Star.decide(s, J.Json.get_str(J.Json.get_obj(o, "p"), "token", ""), J.Json.get_str(J.Json.get_obj(o, "p"), "action", ""), now),
    [Star.saved(B.Reply.ok(client, J.Json.as_raw(J.Json.get(o, "id"), "0")))]}

def Star.request.k(action: Bool, +s: M.State, +now: Nat, +client: U32, +o: J.Json, step: B.Step) -> B.Step:
  match action:
    case True{}: Star.action(s, now, client, o)
    case False{}: Star.after(s, now, client, o, step)

def Star.request(+s: M.State, +now: Nat, +client: U32, +o: J.Json, step: B.Step) -> B.Step:
  Star.request.k(String.eq(J.Json.get_str(o, "m", ""), "star.nudge"), s, now, client, o, step)
