import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/hub.bend as B
import ../src/core/fallback.bend as FB
import ../src/core/json.bend as J

# A limit whose reset came only in words (hub.bend's Reset.*): Claude's
# session limit on 2026-10-30, recorded at 14:59 EDT, resets at 16:10 EDT.

def ny() -> M.State:
  M.State.replay([M.SettingSet{"tz.rule.America/New_York", "EST5EDT,M3.2.0,M11.1.0", 1n},
    M.SettingSet{"person.tz", "America/New_York", 1n}], M.State.empty())

def utc() -> M.State:
  M.State.empty()

def why() -> String:
  "You've hit your session limit · resets 4:10pm (America/New_York)"

def at() -> Nat:
  1793386753n

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("words read in their zone", Nat.show(B.Reset.words(ny(), at(), why())), "1793391000")
    C.Check.str("a zone the hub has no rule for: the person's", Nat.show(B.Reset.words(ny(), at(), "You've hit your session limit · resets 4:10pm (Europe/Nowhere)")), "1793391000")
    C.Check.str("past today's time: tomorrow's", Nat.show(B.Reset.words(ny(), 1793392000n, why())), "1793477400")
    C.Check.str("no words: still unknown", Nat.show(B.Reset.words(ny(), at(), "usage limit reached")), "0")
    C.Check.str("words it cannot read: still unknown", Nat.show(B.Reset.words(ny(), at(), "limit · resets soon-ish")), "0")
    C.Check.str("fill takes the words' time", Nat.show(FB.Limit.reset(B.Reset.fill(ny(), at(), FB.Limit{"claude", why(), 0n}))), "1793391000")
    C.Check.str("fill keeps a reported reset", Nat.show(FB.Limit.reset(B.Reset.fill(ny(), at(), FB.Limit{"claude", why(), 1793390000n}))), "1793390000")
    +rec : M.State = M.State.replay([M.SettingSet{FB.Limit.key("claude"), FB.Limit.json(B.Reset.fill(ny(), at(), FB.Limit{"claude", why(), 0n}), at()), at()}], ny())
    C.Check.str("held before the reset", Bool.show(FB.Limit.active(rec, "claude", 1793390999n)), "True")
    C.Check.str("free at the reset", Bool.show(FB.Limit.active(rec, "claude", 1793391000n)), "False")
    +held : M.State = M.State.replay(B.Queue.changes(B.ProviderResume.hold(ny(), at(), "t1", FB.Limit{"claude", why(), 0n}, B.Step.none())), ny())
    C.Check.str("the held turn will resume at the reset", Bool.show(String.contains(M.Setting.get(held, "provider.resume.t1", ""), "\"reset\":1793391000")), "True")
    # the 0.13 lane's turn end at 16:25:56 EDT found the to-do lane's record
    # from 14:59 ("resets 4:10pm", reset 0) and marked Claude limited again
    +old : J.Json = Maybe.default(&2, J.Json, J.Json.parse("{\"scope\":\"claude\",\"reason\":\"You've hit your session limit · resets 4:10pm (America/New_York)\",\"reset\":0,\"at\":1793386753}"), J.Null{})
    C.Check.str("a pending record is read as of when it was made", Nat.show(FB.Limit.reset(B.Pending.of(ny(), 1793394356n, old))), "1793391000")
    C.Check.str("after its reset it is stale", Bool.show(B.Pending.due(B.Pending.of(ny(), 1793394356n, old), 1793394356n)), "False")
    C.Check.str("before its reset it still holds", Bool.show(B.Pending.due(B.Pending.of(ny(), 1793390000n, old), 1793390000n)), "True")
    +ev : J.Json = Maybe.default(&2, J.Json, J.Json.parse("{\"scope\":\"claude\",\"reason\":\"Provider usage limit (five_hour)\",\"reset\":1793391000,\"at\":1793387898}"), J.Null{})
    C.Check.str("a reported reset that passed: stale", Bool.show(B.Pending.due(B.Pending.of(ny(), 1793394356n, ev), 1793394356n)), "False")
    +unk : J.Json = Maybe.default(&2, J.Json, J.Json.parse("{\"scope\":\"claude\",\"reason\":\"usage limit reached\",\"reset\":0,\"at\":1793386753}"), J.Null{})
    C.Check.str("an unknown reset still holds", Bool.show(B.Pending.due(B.Pending.of(ny(), 1793394356n, unk), 1793394356n)), "True")
