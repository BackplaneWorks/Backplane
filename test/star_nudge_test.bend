import Base
import ./check.bend as C
import ../src/core/star-nudge.bend as S
import ../src/server/star-nudge.bend as N
import ../src/core/hub.bend as B
import ../src/core/bothub.bend as BH
import ../src/core/model.bend as M
import ../src/core/json.bend as J
import ../src/core/client.bend as U
import ../src/core/far.bend as F
import ../src/core/diskq.bend as DQ

# Exercise the same wrapper as server Bot.request, using the real deciders.
def request(+s: M.State, +who: U32, +method: String, +id: String, +text: String) -> B.Step:
  +o = J.Json.obj([J.KV{"id", J.Num{"1"}}, J.KV{"m", J.Str{method}}, J.KV{"p", J.Json.obj([
    J.KV{"thread", J.Str{"t"}}, J.KV{"msg", J.Str{id}}, J.KV{"text", J.Str{text}}])}])
  N.Star.request(s, 10n, who, o, BH.Bots.hub(s, 10n, who, o))

def changes(step: B.Step) -> List<&2, M.Change>:
  match step:
    case B.Step{cs, _}: cs

def apply(+s: M.State, step: B.Step) -> M.State:
  M.State.replay(changes(step), s)

def initial() -> M.State:
  M.State.replay([M.ProjectCreated{"p", "p", "/tmp/p", 1n}, M.ThreadCreated{"t", "p", "Chat", M.Local{}, "", "/tmp/p", "claude", "", 2n},
    M.SettingSet{"onboard.done", "1", 2n}, M.SettingSet{"onboard.uber", "1", 2n}], M.State.empty())

def send(+s: M.State, +id: String) -> M.State:
  apply(s, request(s, 1, "turn.start", id, "Hello"))

def four() -> M.State:
  send(send(send(send(initial(), "m1"), "m2"), "m3"), "m4")

def five() -> M.State:
  send(four(), "m5")

def status(s: M.State) -> String:
  J.Json.get_str(S.Star.state(s), "status", "")

def count(s: M.State) -> String:
  Nat.show(S.Star.count(S.Star.state(s)))

def offers.os(os: List<&2, B.Out>) -> List<&2, String>:
  match os:
    case Nil{}: Nil{}
    case B.SavedSend{+client, +text} <> +rest:
      +o = S.Star.read(text)
      Bool.pick(List<&2, String>, Bool.not(String.is_empty(J.Json.get_str(J.Json.get_obj(o, "info"), "star.nudge.offer", ""))),
        (U32.show(client) ++ ":" ++ J.Json.get_str(J.Json.get_obj(o, "info"), "star.nudge.offer", "")) <> offers.os(rest), offers.os(rest))
    case _ <> rest: offers.os(rest)

def offers(step: B.Step) -> String:
  match step:
    case B.Step{_, os}: J.Json.show(J.Json.arr(S.Star.strings(offers.os(os))))

def decision(+s: M.State, +action: String, +token: String) -> M.State:
  M.State.replay(S.Star.decide(s, token, action, 20n), s)

def old() -> M.State:
  M.State.replay([M.MessagePosted{"t", "old1", M.User{}, "one", 3n}, M.MessagePosted{"t", "old2", M.User{}, "two", 4n},
    M.MessagePosted{"t", "old3", M.User{}, "three", 5n}, M.MessagePosted{"t", "old4", M.User{}, "four", 6n}, M.MessagePosted{"t", "old5", M.User{}, "five", 7n}], initial())

def offered(+s: M.State) -> U.Ui:
  U.Ui.info_set(U.Ui.adopt.raw(U.Ui.init(30n), s), "star.nudge.offer", "m5")

def decided.replay() -> M.State:
  M.State.replay(changes(request(four(), 1, "turn.start", "m5", "Hello")), four())

def atomic(step: B.Step) -> String:
  match step:
    case B.Step{+cs, _}: status(M.State.replay(cs, four())) ++ ":" ++ Bool.show(B.Hub.has_msg(B.Hub.entries(M.State.replay(cs, four())), "m5"))

def damaged(raw: String) -> String:
  count(send(M.State.replay([M.SettingSet{S.Star.key(), raw, 9n}], initial()), "new"))

def durable.os(os: List<&2, B.Out>) -> String:
  match os:
    case B.SavedSend{+client, +text} <> _: "saved:" ++ Bool.show(DQ.Dq.local(B.SavedSend{client, text}))
    case _ <> rest: durable.os(rest)
    case Nil{}: "none"

def durable(step: B.Step) -> String:
  match step:
    case B.Step{_, os}: durable.os(os)

def first.rpc(cs: List<&2, U.Cmd>) -> J.Json:
  match cs:
    case U.Send{text} <> _: S.Star.read(text)
    case _ <> rest: first.rpc(rest)
    case Nil{}: J.Null{}

def retry.rpc() -> J.Json:
  +u = U.Ui.select.k(U.Ui.adopt.raw(U.Ui.init(30n), four()), "t")
  first.rpc(U.Act.cmds(U.Ui.act(U.Ui.scratch_set(u, "t", "Keep this draft"), "retry", "")))

def retry.step() -> B.Step:
  +o = retry.rpc()
  N.Star.request(four(), 30n, 9, o, BH.Bots.hub(four(), 30n, 9, o))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("new and old logs start uncounted", count(initial()) ++ ":" ++ count(old()), "0:0")
    C.Check.str("first new accepted prompt on old log counts once", count(send(old(), "new")), "1")
    C.Check.str("first four prompts keep silent shared count", status(four()) ++ ":" ++ count(four()) ++ ":" ++ offers(request(send(send(initial(), "m1"), "m2"), 2, "turn.start", "m3", "Hello")), "counting:4:[]")
    C.Check.str("fifth acceptance atomically saves sent and actual prompt", atomic(request(four(), 1, "turn.start", "m5", "Hello")), "sent:True")
    C.Check.str("only fifth sender receives one offer", offers(request(four(), 17, "turn.start", "m5", "Hello")), "[\"17:m5\"]")
    C.Check.str("next client sees terminal offer state", offers(request(five(), 18, "turn.start", "m6", "Hello")), "[]")
    C.Check.str("counter stops at five", count(send(send(five(), "m6"), "m7")), "5")
    C.Check.str("duplicate send before threshold", count(send(send(initial(), "m1"), "m1")), "1")
    C.Check.str("duplicate fifth prompt produces no second offer", offers(request(five(), 18, "turn.start", "m5", "Hello")), "[]")
    C.Check.str("bounded ledger survives deleted message history", count(send(M.State.replay([M.SettingSet{S.Star.key(), M.Setting.get(four(), S.Star.key(), ""), 20n}], initial()), "m1")), "4")
    C.Check.str("internal client zero cannot count", count(apply(initial(), request(initial(), 0, "turn.start", "system", "Wake up"))), "0")
    C.Check.str("empty text cannot count", count(apply(initial(), request(initial(), 2, "turn.start", "empty", "  \n\t"))), "0")
    C.Check.str("agent inbox is not a human prompt", count(apply(initial(), B.Hub.inbox(initial(), 10n, "agent", "t", M.User{}, "Message"))), "0")
    C.Check.str("assistant reply is not a human prompt", count(M.State.replay([M.MessagePosted{"t", "assistant", M.Assistant{}, "reply", 10n}], four())), "4")
    C.Check.str("dismiss saved", status(decision(five(), "dismiss", "m5")), "dismissed")
    C.Check.str("open records click, never claims a star", status(decision(five(), "open", "m5")), "opened")
    C.Check.str("second concurrent decision cannot change first", status(decision(decision(five(), "dismiss", "m5"), "open", "m5")), "dismissed")
    C.Check.str("stale token cannot decide", status(decision(five(), "dismiss", "other")), "sent")
    C.Check.str("invalid action cannot decide", status(decision(five(), "star", "m5")), "sent")
    C.Check.str("restart replay keeps sent state and no new offer", status(decided.replay()) ++ ":" ++ offers(request(decided.replay(), 25, "turn.start", "m6", "Hello")), "sent:[]")
    C.Check.str("dismissed hub stays silent on next platform", offers(request(decision(five(), "dismiss", "m5"), 3, "turn.start", "m6", "Hello")), "[]")
    C.Check.str("only offered connection can render", Bool.show(U.Star.visible(offered(five()))) ++ ":" ++ Bool.show(U.Star.visible(U.Ui.adopt.raw(U.Ui.init(30n), five()))), "True:False")
    C.Check.str("shared dismissal clears even stale offered UI", Bool.show(U.Star.visible(offered(decision(five(), "dismiss", "m5")))), "False")
    C.Check.str("new UI after restart has no private offer", Bool.show(U.Star.visible(U.Ui.adopt.raw(U.Ui.init(30n), five()))), "False")
    C.Check.str("unknown saved version/status fails closed", damaged("{\"status\":\"future\"}"), "0")
    C.Check.str("damaged saved ledger fails closed", damaged("{\"status\":\"counting\",\"ids\":[\"x\",null],\"token\":\"x\"}"), "1")
    C.Check.str("corrupt JSON is not reset", damaged("broken"), "0")
    C.Check.str("star settings do not cross peer hubs", Bool.show(Maybe.is_some(&2, M.Change, F.Fr.change("peer", M.SettingSet{S.Star.key(), M.Setting.get(five(), S.Star.key(), ""), 10n}))), "False")
    C.Check.str("offer is held by the disk queue", durable(request(four(), 17, "turn.start", "m5", "Hello")), "saved:False")
    C.Check.str("decision ACK is held even on a no-change retry", durable(N.Star.action(decision(five(), "dismiss", "m5"), 30n, 17, J.Json.obj([J.KV{"id", J.Num{"5"}}, J.KV{"p", J.Json.obj([J.KV{"token", J.Str{"m5"}}, J.KV{"action", J.Str{"dismiss"}}])}]))), "saved:False")
    C.Check.str("real Retry action marks its resend", Bool.show(J.Json.get_bool(J.Json.get_obj(retry.rpc(), "p"), "retry", False{})), "True")
    C.Check.str("accepted Retry cannot advance eligibility or offer", count(apply(four(), retry.step())) ++ ":" ++ offers(retry.step()), "4:[]")
    C.Check.str("canonical public repository", S.Star.url(), "https://github.com/BackplaneWorks/Backplane")
