import Base
import ./check.bend as C
import ../src/core/json.bend as J
import ../src/core/model.bend as M
import ../src/core/codec.bend as CD
import ../src/core/hub.bend as B
import ../src/core/bothub.bend as BH
import ../src/core/told.bend as TD

# "Already told" (told.bend): keys, words, marking, checking, pruning, and
# the alerts a turn's end marks.

def yes(+b: Bool) -> String:
  match b:
    case True{}:
      "yes"
    case False{}:
      "no"

def run(+cs: List<&2, M.Change>) -> M.State:
  M.State.replay(cs, M.State.empty())

def more(+s: M.State, +cs: List<&2, M.Change>) -> M.State:
  M.State.replay(cs, s)

def join(+xs: List<&2, String>) -> String:
  String.join(xs, ",")

def n(+xs: List<&2, TD.Told>) -> String:
  Nat.show(List.length(&2, TD.Told, xs))

def keys(+xs: List<&2, TD.Told>) -> String:
  match xs:
    case Nil{}:
      ""
    case t <> r:
      TD.Told.key(t) ++ " " ++ keys(r)

# a mark made at `now`, applied
def marked(+s: M.State, +now: Nat, +key: String, +text: String, +channel: String, +bot: String) -> M.State:
  more(s, TD.Told.mark(s, now, key, text, channel, bot))

def tagged(+c: M.Change) -> String:
  J.Json.get_str(CD.Change.encode(c), "$", "") ++ ":" ++ J.Json.get_str(CD.Change.encode(c), "key", "")

def tags(+cs: List<&2, M.Change>) -> String:
  match cs:
    case Nil{}:
      ""
    case c <> t:
      tagged(c) ++ " " ++ tags(t)

# told keys among a step's changes
def told.in(+cs: List<&2, M.Change>) -> Nat:
  match cs:
    case Nil{}:
      0n
    case M.SettingSet{+k, _, _} <> t:
      Nat.add(Bool.pick(Nat, String.starts_with(k, "told."), 1n, 0n), told.in(t))
    case _ <> t:
      told.in(t)

def cs.of(+st: B.Step) -> List<&2, M.Change>:
  match st:
    case B.Step{cs, _}:
      cs

# a thread with a user message and the assistant's reply, turn done
def plain() -> M.State:
  run([M.ThreadCreated{"tw", "", "Smarter notifications", M.Local{}, "", "", "claude", "", 100n},
    M.MessagePosted{"tw", "u1", M.User{}, "go", 1000n}, M.TurnChanged{"tw", M.Running{}, 1000n},
    M.MessagePosted{"tw", "m1", M.Assistant{}, "Finished the push.\nSecond line.", 1010n}, M.TurnChanged{"tw", M.Completed{}, 1011n}])

# a bot woken by a room message that posts nothing
def chatter() -> M.State:
  run([M.ThreadCreated{"tb", "", "Bub", M.Local{}, "", "", "claude", "", 100n}, M.BotSet{"b1", "Bub", "tb", 0n, "", 100n},
    M.RoomSet{"r1", "ring-requests", "b1\nPebble@backplane", 100n},
    M.MessagePosted{"tb", "u1", M.User{}, "[message from Pebble@backplane in #ring-requests]\nshort version", 1000n}, M.TurnChanged{"tb", M.Running{}, 1000n},
    M.MessagePosted{"tb", "m1", M.Assistant{}, "Noted.", 1004n}, M.TurnChanged{"tb", M.Completed{}, 1005n}])

def day() -> Nat:
  86400n

def main() -> IO(Unit):
  do IO<Unit>:
    # keys: trimmed, lower case, runs of other characters one "-", 120 at most
    C.Check.str("key norm", TD.Told.key.norm("  Lunch: Dana!! NYC, Wed 10/28  "), "lunch:-dana-nyc-wed-10-28")
    C.Check.str("key keeps . _ : -", TD.Told.key.norm("a.b_c:d-e"), "a.b_c:d-e")
    C.Check.str("key empty", TD.Told.key.norm("   "), "")
    C.Check.str("key 120", Nat.show(String.length(TD.Told.key.norm(String.repeat("ab ", 100n)))), "120")
    C.Check.str("key non-ascii", TD.Told.key.norm("a–b"), "a-b")
    # words: runs of letters and digits, 3 or more, distinct, lower case
    C.Check.str("words", join(TD.Told.words("Dana lunch in NYC on Wed 10/28, lunch again")), "dana,lunch,nyc,wed,again")
    C.Check.str("words digits", join(TD.Told.words("Invoice 2026 and 42")), "invoice,2026,and")
    C.Check.str("words cap", Nat.show(List.length(&2, String, TD.Told.words(String.repeat("w1 w2 w3 aaa bbb ccc ddd eee fff ggg hhh iii jjj kkk lll mmm nnn ooo ppp qqq rrr sss ttt uuu vvv www xxx yyy zzz aab aac aad aae aaf aag aah aai aaj aak aal aam aan aao aap aaq ", 2n)))), "40")
    C.Check.str("key of text", TD.Told.key.of("Dana lunch in NYC Wed 10/28 at noon, with the whole group"), "dana-lunch-nyc-wed-noon-with-the-whole")
    # same: 60% of the smaller set and 2 words; one word must be shared
    C.Check.str("same overlap", yes(TD.Told.same(TD.Told.words("Dana lunch in NYC Wed 10/28"), TD.Told.words("lunch with Dana in NYC on 10/28"))), "yes")
    C.Check.str("same too little", yes(TD.Told.same(TD.Told.words("Dana lunch in NYC Wed"), TD.Told.words("lunch with Priya downtown"))), "no")
    C.Check.str("same one word shared", yes(TD.Told.same(TD.Told.words("peach"), TD.Told.words("renew Peach pass now"))), "yes")
    C.Check.str("same one word not shared", yes(TD.Told.same(TD.Told.words("peach"), TD.Told.words("renew pass now"))), "no")
    C.Check.str("same empty", yes(TD.Told.same(TD.Told.words("a b"), TD.Told.words("renew pass now"))), "no")
    C.Check.str("same two of five is not enough", yes(TD.Told.same(TD.Told.words("alpha beta gamma delta epsilon"), TD.Told.words("alpha beta zeta eta theta"))), "no")
    C.Check.str("same three of five is", yes(TD.Told.same(TD.Told.words("alpha beta gamma delta epsilon"), TD.Told.words("alpha beta gamma eta theta"))), "yes")
    # marking
    +s0 : M.State = M.State.empty()
    C.Check.str("mark nothing", Nat.show(List.length(&2, M.Change, TD.Told.mark(s0, 1000n, "", "  ", "chat", ""))), "0")
    +k1 : M.State = marked(s0, 1000n, "Lunch: Dana!!", "Dana lunch in NYC Wed 10/28", "digest", "b1")
    C.Check.str("mark one", n(TD.Told.all(k1)), "1")
    C.Check.str("mark key normalized", keys(TD.Told.all(k1)), "lunch:-dana- ")
    C.Check.str("mark record", J.Json.show(TD.Told.json(Maybe.default(&2, TD.Told, List.head(&2, TD.Told, TD.Told.all(k1)), TD.Told{"", "", "", "", "", 0n}))),
      "{\"key\":\"lunch:-dana-\",\"text\":\"Dana lunch in NYC Wed 10/28\",\"channel\":\"digest\",\"bot\":\"b1\",\"at\":1000}")
    C.Check.str("mark key from text", keys(TD.Told.all(marked(s0, 1000n, "", "Dana lunch in NYC Wed 10/28", "digest", ""))), "dana-lunch-nyc-wed ")
    C.Check.str("mark text cut to 280", Nat.show(String.length(TD.Told.text(Maybe.default(&2, TD.Told, List.head(&2, TD.Told, TD.Told.all(marked(s0, 1000n, "k", String.repeat("abcdefghij ", 60n), "chat", ""))), TD.Told{"", "", "", "", "", 0n})))), "280")
    C.Check.str("channel unknown", TD.Told.channel(Maybe.default(&2, TD.Told, List.head(&2, TD.Told, TD.Told.all(marked(s0, 1000n, "k", "text", "pigeon", ""))), TD.Told{"", "", "", "", "", 0n})), "other")
    C.Check.str("channel empty", TD.Told.channel(Maybe.default(&2, TD.Told, List.head(&2, TD.Told, TD.Told.all(marked(s0, 1000n, "k", "text", "", ""))), TD.Told{"", "", "", "", "", 0n})), "chat")
    C.Check.str("channel case", TD.Told.channel(Maybe.default(&2, TD.Told, List.head(&2, TD.Told, TD.Told.all(marked(s0, 1000n, "k", "text", " Email ", ""))), TD.Told{"", "", "", "", "", 0n})), "email")
    # checking
    C.Check.str("check by key", n(TD.Told.check(k1, 1100n, "lunch:-dana-", "", 0n)), "1")
    C.Check.str("check by unnormalized key", n(TD.Told.check(k1, 1100n, "  LUNCH: Dana!! ", "", 0n)), "1")
    C.Check.str("check other key", n(TD.Told.check(k1, 1100n, "dentist", "", 0n)), "0")
    C.Check.str("check by text", n(TD.Told.check(k1, 1100n, "", "lunch with Dana in NYC on 10/28", 0n)), "1")
    C.Check.str("check text unrelated", n(TD.Told.check(k1, 1100n, "", "dentist on Tuesday", 0n)), "0")
    C.Check.str("check nothing given", n(TD.Told.check(k1, 1100n, "", "", 0n)), "0")
    +aged : M.State = marked(s0, 1000n, "k", "Dana lunch in NYC", "chat", "")
    C.Check.str("check default 7 days: inside", n(TD.Told.check(aged, Nat.add(1000n, Nat.mul(6n, day())), "k", "", 0n)), "1")
    C.Check.str("check default 7 days: outside", n(TD.Told.check(aged, Nat.add(1000n, Nat.mul(8n, day())), "k", "", 0n)), "0")
    C.Check.str("check 30 days", n(TD.Told.check(aged, Nat.add(1000n, Nat.mul(20n, day())), "k", "", 30n)), "1")
    C.Check.str("check days capped at 30", n(TD.Told.check(aged, Nat.add(1000n, Nat.mul(40n, day())), "k", "", 99n)), "0")
    +many : M.State = marked(marked(marked(s0, 1000n, "a1", "first thing", "chat", ""), 1001n, "a1", "second thing", "chat", ""), 1002n, "a1", "third thing", "chat", "")
    C.Check.str("check newest first", join(TD.Told.words(TD.Told.text(Maybe.default(&2, TD.Told, List.head(&2, TD.Told, TD.Told.check(many, 1100n, "a1", "", 0n)), TD.Told{"", "", "", "", "", 0n})))), "third,thing")
    # pruning: older than 30 days goes, 29 days stays
    +p : M.State = marked(marked(s0, 1000n, "aged", "an aged record", "chat", ""), Nat.add(1000n, Nat.mul(2n, day())), "new", "a newer record", "chat", "")
    C.Check.str("prune 31 days", Nat.show(told.in(TD.Told.prune(p, Nat.add(1000n, Nat.mul(31n, day()))))), "1")
    C.Check.str("prune 29 days", Nat.show(told.in(TD.Told.prune(p, Nat.add(1000n, Nat.mul(29n, day()))))), "0")
    C.Check.str("prune removes the key", n(TD.Told.all(more(p, TD.Told.prune(p, Nat.add(1000n, Nat.mul(31n, day())))))), "1")
    C.Check.str("prune keeps the newer one", keys(TD.Told.all(more(p, TD.Told.prune(p, Nat.add(1000n, Nat.mul(31n, day())))))), "new ")
    C.Check.str("prune nothing to do", Nat.show(List.length(&2, M.Change, TD.Told.prune(s0, 5000000n))), "0")
    C.Check.str("prune by hand-written key reads the value", Nat.show(told.in(TD.Told.prune(more(s0, [M.SettingSet{"told.x", "{\"key\":\"x\",\"text\":\"t\",\"channel\":\"chat\",\"bot\":\"\",\"at\":10}", 10n}]), 5000000n))), "1")
    C.Check.str("setting empty removes", n(TD.Told.all(more(k1, [M.SettingSet{Maybe.default(&2, String, List.head(&2, String, M.Setting.keys(k1)), ""), "", 1200n}]))), "0")
    C.Check.str("setting empty removes the key", Nat.show(List.length(&2, String, M.Setting.keys(more(k1, [M.SettingSet{Maybe.default(&2, String, List.head(&2, String, M.Setting.keys(k1)), ""), "", 1200n}])))), "0")
    C.Check.str("other empty settings stay", Nat.show(List.length(&2, String, M.Setting.keys(more(s0, [M.SettingSet{"theme", "", 1n}])))), "1")
    # a turn's end that alerts is marked
    +al : List<&2, M.Change> = TD.Told.alert(plain(), 2000n, "tw", "completed")
    C.Check.str("alert marks", Nat.show(told.in(al)), "1")
    +alerted : M.State = more(plain(), al)
    C.Check.str("alert record", J.Json.show(TD.Told.json(Maybe.default(&2, TD.Told, List.head(&2, TD.Told, TD.Told.all(alerted)), TD.Told{"", "", "", "", "", 0n}))),
      "{\"key\":\"turn-tw\",\"text\":\"Smarter notifications: Finished the push.\",\"channel\":\"alert\",\"bot\":\"\",\"at\":2000}")
    C.Check.str("alert found by key", n(TD.Told.check(alerted, 2100n, "turn-tw", "", 0n)), "1")
    C.Check.str("alert found by what it said", n(TD.Told.check(alerted, 2100n, "", "finished the push for notifications", 0n)), "1")
    C.Check.str("failed alerts", Nat.show(told.in(TD.Told.alert(plain(), 2000n, "tw", "failed"))), "1")
    C.Check.str("stopped marks nothing", Nat.show(told.in(TD.Told.alert(plain(), 2000n, "tw", "stopped"))), "0")
    C.Check.str("no state marks nothing", Nat.show(told.in(TD.Told.alert(plain(), 2000n, "tw", ""))), "0")
    C.Check.str("quiet end marks nothing", Nat.show(told.in(TD.Told.alert(chatter(), 2000n, "tb", "completed"))), "0")
    # a bot turn the person started that fails alerts (the policy's fail), and is marked with the bot
    +asked : M.State = more(chatter(), [M.MessagePosted{"tb", "u5", M.User{}, "order the parts", 1500n}, M.TurnChanged{"tb", M.Running{}, 1500n}])
    C.Check.str("person's bot turn fails loudly", Nat.show(told.in(TD.Told.alert(asked, 2000n, "tb", "failed"))), "1")
    C.Check.str("bot alert carries the bot", TD.Told.bot(Maybe.default(&2, TD.Told, List.head(&2, TD.Told, TD.Told.all(more(asked, TD.Told.alert(asked, 2000n, "tb", "failed")))), TD.Told{"", "", "", "", "", 0n})), "b1")
    # the hub marks it: Ended.out writes the alert record, and Alerts.changes
    # (every step passes through it before it is stored) marks the record
    +pre : M.State = more(plain(), [M.MessagePosted{"tw", "u2", M.User{}, "again", 3000n}, M.TurnChanged{"tw", M.Running{}, 3000n}])
    +end : B.Step = B.Ended.out(pre, 3010n, "tw",
      B.Step{[M.MessagePosted{"tw", "m2", M.Assistant{}, "Second answer.", 3005n}, M.TurnChanged{"tw", M.Completed{}, 3006n}], Nil{}})
    C.Check.str("hub marks the end", Nat.show(told.in(B.Alerts.changes(pre, cs.of(end)))), "1")
    C.Check.str("hub marks what the turn said", yes(String.contains(tags(B.Alerts.changes(pre, cs.of(end))), "SettingSet:told.")), "yes")
    C.Check.str("hub quiet end", Nat.show(told.in(B.Alerts.changes(chatter(), cs.of(B.Ended.out(chatter(), 3010n, "tb", B.Step{[M.MessagePosted{"tb", "m2", M.Assistant{}, "Ok.", 3005n}, M.TurnChanged{"tb", M.Completed{}, 3006n}], Nil{}}))))), "0")
    # the minute tick prunes
    C.Check.str("tick prunes", Nat.show(told.in(cs.of(BH.Bots.tick(p, Nat.add(1000n, Nat.mul(31n, day())), BH.Tz{0n, True{}})))), "1")
    C.Check.str("tick quiet", Nat.show(told.in(cs.of(BH.Bots.tick(p, 2000n, BH.Tz{0n, True{}})))), "0")
