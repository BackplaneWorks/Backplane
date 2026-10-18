import Base
import ./check.bend as C
import ../src/core/json.bend as J
import ../src/core/model.bend as M
import ../src/core/codec.bend as CD
import ../src/core/hub.bend as B
import ../src/core/bot.bend as BT
import ../src/core/bothub.bend as BH
import ../src/core/mcp.bend as MC
import ../src/core/ring.bend as RG
import ../src/core/client.bend as U

def yes(b: Bool) -> String:
  match b:
    case True{}:
      "yes"
    case False{}:
      "no"

def tag(c: M.Change) -> String:
  J.Json.get_str(CD.Change.encode(c), "$", "")

def tags(cs: List<&2, M.Change>) -> String:
  match cs:
    case Nil{}:
      ""
    case c <> t:
      tag(c) ++ " " ++ tags(t)

def fx(os: List<&2, B.Out>) -> String:
  match os:
    case Nil{}:
      ""
    case B.BotFx{k, _, _, _} <> t:
      "fx:" ++ k ++ " " ++ fx(t)
    case B.Send{_, text} <> t:
      "send:" ++ J.Json.get_str(Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{}), "t", "") ++ " " ++ fx(t)
    case _ <> t:
      "out " ++ fx(t)

def st.tags(st: B.Step) -> String:
  match st:
    case B.Step{cs, os}:
      tags(cs) ++ "| " ++ fx(os)

def run(s: M.State, st: B.Step) -> M.State:
  match st:
    case B.Step{cs, _}:
      M.State.replay(cs, s)

def req(+m: String, +p: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse("{\"id\":1,\"m\":\"" ++ m ++ "\",\"p\":" ++ p ++ "}"), J.Null{})

def ask(+s: M.State, +now: Nat, +m: String, +p: String) -> B.Step:
  BH.Bots.request(s, now, 1, req(m, p))

def made(+s: M.State, +now: Nat, +m: String, +p: String) -> M.State:
  run(s, ask(s, now, m, p))

def two() -> M.State:
  made(made(M.State.empty(), 1000n, "bots.create", "{\"name\":\"Miso\",\"provider\":\"claude\"}"), 1001n, "bots.create", "{\"name\":\"Tofu\",\"provider\":\"codex\"}")

def bot.of(+s: M.State, +name: String) -> BT.Bot:
  Maybe.default(&2, BT.Bot, BT.World.find(M.State.bots(s), name), BT.Bot.blank("?"))

def id.of(+s: M.State, +name: String) -> String:
  BT.Bot.id(bot.of(s, name))

def th.of(+s: M.State, +name: String) -> String:
  BT.Bot.thread(bot.of(s, name))

def out.reply(o: MC.McpOut) -> String:
  match o:
    case MC.McpOut{r, _, _}:
      r

def out.step(o: MC.McpOut) -> B.Step:
  match o:
    case MC.McpOut{_, st, _}:
      st

def call(+s: M.State, +now: Nat, +thread: String, +tool: String, +args: String) -> String:
  out.reply(BH.Bots.mcp(s, now, thread, J.Null{}, "t", J.Json.arr(Nil{}), Nil{}, "{\"jsonrpc\":\"2.0\",\"id\":7,\"method\":\"tools/call\",\"params\":{\"name\":\"" ++ tool ++ "\",\"arguments\":" ++ args ++ "}}"))

def call.step(+s: M.State, +now: Nat, +thread: String, +tool: String, +args: String) -> B.Step:
  out.step(BH.Bots.mcp(s, now, thread, J.Null{}, "t", J.Json.arr(Nil{}), Nil{}, "{\"jsonrpc\":\"2.0\",\"id\":7,\"method\":\"tools/call\",\"params\":{\"name\":\"" ++ tool ++ "\",\"arguments\":" ++ args ++ "}}"))

def woke(+s: M.State, +name: String) -> String:
  Nat.show(BT.World.hop(M.State.bots(s), id.of(s, name)))

# a bot at hop h sends: the message goes out at h+1, or not past the bound
def hops(n: Nat, +s: M.State) -> M.State:
  match n:
    case 0n:
      s
    case 1n+k:
      hops(k, run(s, call.step(s, 2000n, th.of(s, "Miso"), "bot_send", "{\"to\":\"Tofu\",\"text\":\"hi\"}")))

def mem.state() -> M.State:
  made(made(made(two(), 1100n, "bots.remember", "{\"bot\":\"" ++ id.of(two(), "Miso") ++ "\",\"key\":\"User TZ\",\"kind\":\"user\",\"text\":\"Lives in Lisbon\"}"),
    1101n, "bots.remember", "{\"bot\":\"" ++ id.of(two(), "Miso") ++ "\",\"key\":\"howto.invoice\",\"kind\":\"howto\",\"text\":\"Invoices go through the Stripe dashboard\",\"tags\":\"billing\"}"),
    1102n, "bots.remember", "{\"bot\":\"" ++ id.of(two(), "Miso") ++ "\",\"key\":\"fact.cat\",\"kind\":\"fact\",\"text\":\"The office cat is called Pixel\"}")

def hook.of(+s: M.State) -> String:
  BT.Hook.id(Maybe.default(&2, BT.Hook, List.head(&2, BT.Hook, BT.World.hooks(M.State.bots(s), id.of(s, "Tofu"))), BT.Hook.blank("?")))

def ring.body() -> String:
  "--x\r\nContent-Disposition: form-data; name=\"transcription\"\r\n\r\nremind me to order stencils\r\n--x\r\nContent-Disposition: form-data; name=\"recordedAt\"\r\n\r\n1790000000000\r\n--x\r\nContent-Disposition: form-data; name=\"client\"\r\n\r\nring\r\n--x--\r\n"

def ring.test() -> String:
  "--y\r\nContent-Disposition: form-data; name=\"transcription\"\r\n\r\nThis is a test\r\n--y\r\nContent-Disposition: form-data; name=\"test\"\r\n\r\ntrue\r\n--y--\r\n"

def tags.cs(st: B.Step) -> List<&2, M.Change>:
  match st:
    case B.Step{cs, _}:
      cs

def changes.show(cs: List<&2, M.Change>) -> String:
  match cs:
    case Nil{}:
      ""
    case c <> t:
      J.Json.show(CD.Change.encode(c)) ++ changes.show(t)

def res.step(r: MC.Res) -> B.Step:
  match r:
    case MC.Res{_, _, st}:
      st

def res.body(r: MC.Res) -> J.Json:
  match r:
    case MC.Res{_, b, _}:
      b

def proj() -> M.State:
  run(two(), B.Step{[M.ProjectCreated{"p1", "Power board", "/tmp/power", 1000n}, M.ThreadCreated{"t-pb", "p1", "Rev C review", M.Local{}, "", "", "claude", "", 1000n}], Nil{}})

def spawned(+s: M.State, +maker: String, +name: String) -> M.State:
  run(s, call.step(s, 1500n, th.of(s, maker), "bot_create", "{\"name\":\"" ++ name ++ "\",\"persona\":\"helps\"}"))

def Maybe.is.k(m: Maybe<&2, BT.Bot>) -> Bool:
  match m:
    case Some{_}:
      True{}
    case None{}:
      False{}

def fx.body(os: List<&2, B.Out>) -> String:
  match os:
    case Nil{}:
      ""
    case B.BotFx{_, _, _, c} <> _:
      c
    case _ <> t:
      fx.body(t)

def st.body(st: B.Step) -> String:
  match st:
    case B.Step{_, os}:
      fx.body(os)

def json(+t: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(t), J.Null{})

def room.of(+s: M.State, +name: String) -> BT.Room:
  Maybe.default(&2, BT.Room, BT.World.find_room(M.State.bots(s), name), BT.Room.blank(""))

# how many threads the sidebar leaves out as delegated children
def hidden(+s: M.State) -> Nat:
  Nat.sub(List.length(&2, M.Thread, U.Ui.threads(s)), List.length(&2, M.Thread, U.Kids.keep(U.Ui.threads(s), U.Kids.set(s))))

# a person's zone: Helsinki, with the rules of New York known too
def zst() -> M.State:
  M.State.replay([M.SettingSet{"tz.rule.Europe/Helsinki", "EET-2EEST,M3.5.0/3,M10.5.0/4", 1n}, M.SettingSet{"tz.rule.America/New_York", "EST5EDT,M3.2.0,M11.1.0", 1n},
    M.SettingSet{"person.tz", "Europe/Helsinki", 1n}], two())

def tick(+s: M.State, +now: Nat) -> B.Step:
  BH.Bots.tick(s, now, BH.Tz{0n, True{}})

def routine.json(+s: M.State, +now: Nat) -> String:
  call(s, now, th.of(s, "Miso"), "routine_list", "{}")

def rid(+s: M.State) -> String:
  BT.Routine.id(Maybe.default(&2, BT.Routine, List.head(&2, BT.Routine, BT.World.routines(M.State.bots(s), id.of(s, "Miso"))), BT.Routine.blank("?")))

def hub.ask(+s: M.State, +now: Nat, +m: String, +p: String) -> B.Step:
  BH.Bots.hub(s, now, 1, req(m, p))

def main() -> IO(Unit):
  do IO<Unit>:
    +s : M.State = two()
    C.Check.str("create", st.tags(ask(M.State.empty(), 1000n, "bots.create", "{\"name\":\"Miso\"}")), "ThreadCreated BotSet | send:reply fx:home ")
    C.Check.str("two bots", Nat.show(List.length(&2, BT.Bot, BT.World.bots(M.State.bots(s)))), "2")
    C.Check.str("create as the uberbot (the tour): the backup job follows", st.tags(ask(M.State.empty(), 1000n, "bots.create", "{\"name\":\"Miso\",\"uber\":true}")), "ThreadCreated BotSet | send:reply fx:home fx:uber ")
    C.Check.str("name taken", st.tags(ask(s, 1002n, "bots.create", "{\"name\":\"miso\"}")), "| send:reply ")
    C.Check.str("bad name", st.tags(ask(s, 1002n, "bots.create", "{\"name\":\"has space\"}")), "| send:reply ")
    C.Check.str("bot thread", yes(BT.World.is_bot_thread(M.State.bots(s), th.of(s, "Tofu"))), "yes")
    C.Check.str("say", st.tags(ask(s, 1003n, "bots.say", "{\"bot\":\"" ++ id.of(s, "Miso") ++ "\",\"text\":\"hello\"}")), "BotWoke MessagePosted TurnChanged | send:reply out out out out ")
    C.Check.str("send", st.tags(call.step(s, 1004n, th.of(s, "Miso"), "bot_send", "{\"to\":\"tofu\",\"text\":\"hi\"}")), "RoomSet RoomPosted BotWoke MessagePosted MessageFrom TurnChanged | out out out ")
    # a request between bots goes in a room of the two, named for its topic
    +rq : M.State = run(s, call.step(s, 1004n, th.of(s, "Miso"), "bot_send", "{\"to\":\"Tofu\",\"topic\":\"BOM review\",\"text\":\"check U3\"}"))
    C.Check.str("request room", BT.Room.members.show(BT.Room.members(room.of(rq, "BOM review"))), id.of(s, "Miso") ++ "\n" ++ id.of(s, "Tofu"))
    C.Check.str("request room not dm", yes(BT.Room.is_dm(room.of(rq, "BOM review"))), "no")
    C.Check.str("request readable", yes(String.contains(call(rq, 1005n, th.of(rq, "Tofu"), "room_read", "{\"room\":\"BOM review\"}"), "check U3")), "yes")
    C.Check.str("request note", yes(String.contains(call(s, 1004n, th.of(s, "Miso"), "bot_send", "{\"to\":\"Tofu\",\"topic\":\"BOM review\",\"text\":\"x\"}"), "#BOM review")), "yes")
    C.Check.str("request room reused", yes(String.starts_with(st.tags(call.step(rq, 1005n, th.of(rq, "Miso"), "bot_send", "{\"to\":\"Tofu\",\"topic\":\"BOM review\",\"text\":\"and U4\"}")), "RoomPosted ")), "yes")
    C.Check.str("reply in the same room", yes(String.starts_with(st.tags(call.step(rq, 1005n, th.of(rq, "Tofu"), "bot_send", "{\"to\":\"Miso\",\"topic\":\"bom review\",\"text\":\"done\"}")), "RoomPosted ")), "yes")
    C.Check.str("new topic new room", yes(String.starts_with(st.tags(call.step(rq, 1005n, th.of(rq, "Miso"), "bot_send", "{\"to\":\"Tofu\",\"topic\":\"stencil\",\"text\":\"order\"}")), "RoomSet ")), "yes")
    C.Check.str("no topic: both names", yes(BT.Room.live(room.of(run(s, call.step(s, 1004n, th.of(s, "Tofu"), "bot_send", "{\"to\":\"Miso\",\"text\":\"hi\"}")), "miso + tofu"))), "yes")
    # (Tofu's Codex turn runs: the post rides its next tool result, hub.bend's Ride)
    C.Check.str("person's post reaches both", (st.tags(ask(rq, 1006n, "bots.post", "{\"room\":\"" ++ BT.Room.id(room.of(rq, "BOM review")) ++ "\",\"text\":\"thanks\"}"))), "RoomPosted BotWoke MessagePosted MessageFrom TurnChanged BotWoke MessagePosted MessageFrom SettingSet | send:reply out out out out ")
    # a request sent again (same msg) is answered and does nothing twice
    +sy : String = "{\"bot\":\"" ++ id.of(s, "Miso") ++ "\",\"text\":\"again\",\"msg\":\"cx1-1\"}"
    C.Check.str("say with msg first time", st.tags(ask(s, 1003n, "bots.say", sy)), "BotWoke MessagePosted TurnChanged | send:reply out out out out ")
    C.Check.str("say with msg again", st.tags(ask(made(s, 1003n, "bots.say", sy), 1004n, "bots.say", sy)), "| send:reply ")
    C.Check.str("say with another msg", st.tags(ask(made(s, 1003n, "bots.say", sy), 1004n, "bots.say", "{\"bot\":\"" ++ id.of(s, "Miso") ++ "\",\"text\":\"again\",\"msg\":\"cx1-2\"}")), "BotWoke MessagePosted TurnChanged | send:reply out out out out ")
    +rp : String = "{\"room\":\"" ++ BT.Room.id(room.of(rq, "BOM review")) ++ "\",\"text\":\"thanks\",\"msg\":\"cx1-3\"}"
    C.Check.str("post with msg first time", st.tags(ask(rq, 1006n, "bots.post", rp)), "RoomPosted BotWoke MessagePosted MessageFrom TurnChanged BotWoke MessagePosted MessageFrom SettingSet | send:reply out out out out ")
    C.Check.str("post with msg again", st.tags(ask(made(rq, 1006n, "bots.post", rp), 1007n, "bots.post", rp)), "| send:reply ")
    # a private message only when asked for
    C.Check.str("send direct", st.tags(call.step(s, 1004n, th.of(s, "Miso"), "bot_send", "{\"to\":\"Tofu\",\"text\":\"psst\",\"direct\":true}")), "RoomPosted BotWoke MessagePosted MessageFrom TurnChanged | out out out ")
    C.Check.str("direct is dm", yes(String.contains(changes.show(tags.cs(call.step(s, 1004n, th.of(s, "Miso"), "bot_send", "{\"to\":\"Tofu\",\"text\":\"psst\",\"direct\":true}"))), "\"dm:Miso:Tofu\"")), "yes")
    C.Check.str("request not dm", yes(String.contains(changes.show(tags.cs(call.step(s, 1004n, th.of(s, "Miso"), "bot_send", "{\"to\":\"Tofu\",\"text\":\"hi\"}"))), "dm:")), "no")
    C.Check.str("send hop", woke(run(s, call.step(s, 1004n, th.of(s, "Miso"), "bot_send", "{\"to\":\"Tofu\",\"text\":\"hi\"}")), "Tofu"), "1")
    C.Check.str("send self", yes(String.contains(call(s, 1004n, th.of(s, "Miso"), "bot_send", "{\"to\":\"Miso\",\"text\":\"hi\"}"), "that is you")), "yes")
    C.Check.str("send nobody", yes(String.contains(call(s, 1004n, th.of(s, "Miso"), "bot_send", "{\"to\":\"Nope\",\"text\":\"hi\"}"), "no bot by that name")), "yes")
    C.Check.str("not a bot", yes(String.contains(call(M.State.empty(), 1004n, "t1", "bot_send", "{\"to\":\"Nope\",\"text\":\"hi\"}"), "unknown tool")), "yes")
    # Miso keeps its own hop (0) here, so each send goes out at 1; set Miso
    # deep: its message is held, logged for the person, heard by no bot
    C.Check.str("hop bound", yes(String.contains(call(run(s, B.Step{[M.BotWoke{id.of(s, "Miso"), 6n, 1005n}], Nil{}}), 1005n, th.of(s, "Miso"), "bot_send",
      "{\"to\":\"Tofu\",\"text\":\"hi\"}"), "no bot was woken")), "yes")
    C.Check.str("hop bound held", st.tags(call.step(run(s, B.Step{[M.BotWoke{id.of(s, "Miso"), 6n, 1005n}], Nil{}}), 1005n, th.of(s, "Miso"), "bot_send",
      "{\"to\":\"Tofu\",\"text\":\"hi\"}")), "RoomSet RoomPosted | ")
    C.Check.str("hop 5 ok", yes(String.contains(call(run(s, B.Step{[M.BotWoke{id.of(s, "Miso"), 5n, 1005n}], Nil{}}), 1005n, th.of(s, "Miso"), "bot_send",
      "{\"to\":\"Tofu\",\"text\":\"hi\"}"), "posted in")), "yes")
    C.Check.str("posts logged", Nat.show(List.length(&2, BT.Post, BT.World.posts(M.State.bots(hops(3n, s)), BT.Room.id(room.of(hops(3n, s), "miso + tofu")), 10n))), "3")
    C.Check.str("room create", st.tags(call.step(s, 1006n, th.of(s, "Miso"), "room_create", "{\"name\":\"team\",\"members\":\"Tofu, ghost@elsewhere\"}")), "RoomSet | ")
    +r : M.State = run(s, call.step(s, 1006n, th.of(s, "Miso"), "room_create", "{\"name\":\"team\",\"members\":\"Tofu\"}"))
    C.Check.str("room post", st.tags(call.step(r, 1007n, th.of(r, "Miso"), "room_post", "{\"room\":\"team\",\"text\":\"standup\"}")), "RoomPosted BotWoke MessagePosted MessageFrom TurnChanged | out out out ")
    C.Check.str("room read", yes(String.contains(call(run(r, call.step(r, 1007n, th.of(r, "Miso"), "room_post", "{\"room\":\"team\",\"text\":\"standup\"}")), 1008n, th.of(r, "Tofu"), "room_read", "{\"room\":\"team\"}"), "standup")), "yes")
    C.Check.str("room by person", st.tags(ask(r, 1009n, "bots.post", "{\"room\":\"" ++ BT.Room.id(Maybe.default(&2, BT.Room, BT.World.find_room(M.State.bots(r), "team"), BT.Room.blank("")) ) ++ "\",\"text\":\"hey\"}")),
      "RoomPosted BotWoke MessagePosted MessageFrom TurnChanged BotWoke MessagePosted MessageFrom TurnChanged | send:reply out out out out out out out ")
    +m : M.State = mem.state()
    C.Check.str("mem key norm", yes(String.contains(call(m, 1200n, th.of(m, "Miso"), "memory_list", "{}"), "user-tz")), "yes")
    C.Check.str("mem recall", yes(String.contains(call(m, 1200n, th.of(m, "Miso"), "memory_recall", "{\"query\":\"send the invoice\"}"), "Stripe")), "yes")
    C.Check.str("mem recall miss", yes(String.contains(call(m, 1200n, th.of(m, "Miso"), "memory_recall", "{\"query\":\"send the invoice\"}"), "Pixel")), "no")
    C.Check.str("mem block has user", yes(String.contains(BH.Bots.turn(m, 1200n, th.of(m, "Miso"), "what's up?"), "Lisbon")), "yes")
    C.Check.str("mem block skips unrelated", yes(String.contains(BH.Bots.turn(m, 1200n, th.of(m, "Miso"), "what's up?"), "Stripe")), "no")
    C.Check.str("mem block related", yes(String.contains(BH.Bots.turn(m, 1200n, th.of(m, "Miso"), "a billing question"), "Stripe")), "yes")
    C.Check.str("mem block not for others", BH.Bots.turn(m, 1200n, th.of(m, "Tofu"), "hi"), "hi")
    C.Check.str("mem replace", Nat.show(List.length(&2, BT.Mem, BT.World.mems(M.State.bots(made(m, 1300n, "bots.remember", "{\"bot\":\"" ++ id.of(m, "Miso") ++ "\",\"key\":\"user tz\",\"kind\":\"user\",\"text\":\"Now in Porto\"}")), id.of(m, "Miso")))), "3")
    C.Check.str("mem forget", Nat.show(List.length(&2, BT.Mem, BT.World.mems(M.State.bots(run(m, call.step(m, 1300n, th.of(m, "Miso"), "memory_forget", "{\"key\":\"fact.cat\"}"))), id.of(m, "Miso")))), "2")
    C.Check.str("fit budget", yes(Nat.is_le(BT.Mem.used(BT.Mem.fit(["aaaa", "bbbb", "cccc"], 11n)), 11n)), "yes")
    C.Check.str("fit stops", Nat.show(List.length(&2, String, BT.Mem.fit(["aaaa", "bbbb", "cccc"], 11n))), "2")
    # routines: made at 1000, every 15m; due at the first quarter hour after
    +o : M.State = made(s, 1000n, "bots.routine", "{\"bot\":\"" ++ id.of(s, "Miso") ++ "\",\"name\":\"check mail\",\"cron\":\"every 15m\",\"prompt\":\"Look at the inbox\"}")
    C.Check.str("routine bad", st.tags(ask(s, 1000n, "bots.routine", "{\"bot\":\"" ++ id.of(s, "Miso") ++ "\",\"cron\":\"whenever\",\"prompt\":\"x\"}")), "| send:reply ")
    C.Check.str("routine not yet", st.tags(BH.Bots.tick(o, 1500n, BH.Tz{0n, True{}})), "| ")
    C.Check.str("routine due", st.tags(BH.Bots.tick(o, 1860n, BH.Tz{0n, True{}})), "RoutineFired RoutineSlot BotWoke MessagePosted MessageFrom TurnChanged | out out out out ")
    C.Check.str("routine once", st.tags(BH.Bots.tick(run(o, BH.Bots.tick(o, 1860n, BH.Tz{0n, True{}})), 1900n, BH.Tz{0n, True{}})), "| ")
    C.Check.str("routine by bot", yes(String.contains(call(s, 1000n, th.of(s, "Tofu"), "routine_set", "{\"name\":\"r\",\"schedule\":\"daily 09:30\",\"prompt\":\"p\"}"), "daily at 09:30")), "yes")
    # routines-cal: one-time routines, reminders and zones
    +z : M.State = zst()
    +zd : M.State = made(z, 1791892800n, "bots.routine", "{\"bot\":\"" ++ id.of(z, "Miso") ++ "\",\"name\":\"digest\",\"cron\":\"daily 09:30\",\"prompt\":\"news\"}")
    C.Check.str("zoned: 09:29:59 not yet", st.tags(tick(zd, 1791959399n)), "| ")
    C.Check.str("zoned: 09:30 Helsinki summer", st.tags(tick(zd, 1791959400n)), "RoutineFired RoutineSlot BotWoke MessagePosted MessageFrom TurnChanged | out out out out ")
    +zw : M.State = made(z, 1797163200n, "bots.routine", "{\"bot\":\"" ++ id.of(z, "Miso") ++ "\",\"name\":\"digest\",\"cron\":\"daily 09:30\",\"prompt\":\"news\"}")
    C.Check.str("zoned: 07:29:59Z not yet in winter", st.tags(tick(zw, 1797233399n)), "| ")
    C.Check.str("zoned: 09:30 Helsinki winter", st.tags(tick(zw, 1797233400n)), "RoutineFired RoutineSlot BotWoke MessagePosted MessageFrom TurnChanged | out out out out ")
    +zf : M.State = run(zd, tick(zd, 1791959400n))
    C.Check.str("zoned: not again", st.tags(tick(zf, 1791959460n)), "| ")
    C.Check.str("zoned: next day", st.tags(tick(zf, 1792045800n)), "RoutineFired RoutineSlot BotWoke MessagePosted MessageFrom TurnChanged | out out out out ")
    +zn : M.State = M.State.replay([M.SettingSet{"person.tz", "America/New_York", 1791959500n}], zf)
    C.Check.str("zone change west does not refire at once", st.tags(tick(zn, 1791959520n)), "| ")
    C.Check.str("zone change west does not refire at 09:30 there", st.tags(tick(zn, 1791984600n)), "| ")
    C.Check.str("zone change west fires the next day", st.tags(tick(zn, 1792071000n)), "RoutineFired RoutineSlot BotWoke MessagePosted MessageFrom TurnChanged | out out out out ")
    +rm : M.State = run(z, call.step(z, 1791957600n, th.of(z, "Miso"), "remind_person", "{\"at\":\"in 20m\",\"text\":\"stand up\"}"))
    C.Check.str("remind_person logs a reminder routine", st.tags(call.step(z, 1791957600n, th.of(z, "Miso"), "remind_person", "{\"at\":\"in 20m\",\"text\":\"stand up\"}")), "RoutineSet RoutinePlan | ")
    C.Check.str("remind_person says when", yes(String.contains(call(z, 1791957600n, th.of(z, "Miso"), "remind_person", "{\"at\":\"in 20m\",\"text\":\"stand up\"}"), "I'll remind them Wed 14 Oct 09:20 EEST (in 20m): stand up")), "yes")
    C.Check.str("reminder: not before", st.tags(tick(rm, 1791958799n)), "| ")
    C.Check.str("reminder: Reminded RoutineFired RoutineDone and its alert record, no turn", st.tags(tick(rm, 1791958800n)), "Reminded RoutineFired RoutineDone SettingSet | ")
    C.Check.str("reminder: the alert is a reminder", yes(String.contains(changes.show(tags.cs(tick(rm, 1791958800n))), "reminder\\tremind-")), "yes")
    +rmoff : M.State = M.State.replay([M.SettingSet{"notify.reminders", "off", 1791957700n}], rm)
    C.Check.str("reminder: alerts turned off leave an empty kind", yes(String.contains(changes.show(tags.cs(tick(rmoff, 1791958800n))), "\"value\":\"\\t\\t")), "yes")
    C.Check.str("reminder: once", st.tags(tick(run(rm, tick(rm, 1791958800n)), 1791958900n)), "| ")
    C.Check.str("reminder lands in the bot's thread", yes(String.contains(changes.show(tags.cs(tick(rm, 1791958800n))), "\"thread\":\"" ++ th.of(z, "Miso") ++ "\"")), "yes")
    C.Check.str("reminder: listed as a reminder", yes(String.contains(routine.json(rm, 1791957700n), "kind\\\":\\\"reminder")), "yes")
    C.Check.str("reminder: done in the list", yes(String.contains(routine.json(run(rm, tick(rm, 1791958800n)), 1791958900n), "done\\\":true")), "yes")
    C.Check.str("reminder: deletable", st.tags(call.step(rm, 1791957700n, th.of(z, "Miso"), "routine_delete", "{\"id\":\"" ++ rid(rm) ++ "\"}")), "RoutineDeleted | ")
    C.Check.str("remind_person needs text", yes(String.contains(call(z, 1791957600n, th.of(z, "Miso"), "remind_person", "{\"at\":\"in 20m\",\"text\":\" \"}"), "what to remind")), "yes")
    C.Check.str("remind_person past time", yes(String.contains(call(z, 1791957600n, th.of(z, "Miso"), "remind_person", "{\"at\":\"2026-01-01T09:00\",\"text\":\"x\"}"), "I can read")), "yes")
    +on : M.State = run(z, call.step(z, 1791957600n, th.of(z, "Miso"), "routine_set", "{\"name\":\"once\",\"at\":\"in 20m\",\"prompt\":\"look\"}"))
    C.Check.str("routine_set at: plan logged", st.tags(call.step(z, 1791957600n, th.of(z, "Miso"), "routine_set", "{\"name\":\"once\",\"at\":\"in 20m\",\"prompt\":\"look\"}")), "RoutineSet RoutinePlan | ")
    C.Check.str("routine_set at: says when", yes(String.contains(call(z, 1791957600n, th.of(z, "Miso"), "routine_set", "{\"name\":\"once\",\"at\":\"in 20m\",\"prompt\":\"look\"}"), "once, Wed 14 Oct 09:20 EEST (in 20m)")), "yes")
    C.Check.str("routine_set at: fires and is done", st.tags(tick(on, 1791958800n)), "RoutineFired RoutineDone BotWoke MessagePosted MessageFrom TurnChanged | out out out out ")
    C.Check.str("routine_set at: only once", st.tags(tick(run(on, tick(on, 1791958800n)), 1791959000n)), "| ")
    C.Check.str("routine_set schedule says when", yes(String.contains(call(z, 1791957600n, th.of(z, "Miso"), "routine_set", "{\"name\":\"r\",\"schedule\":\"daily 09:30\",\"prompt\":\"p\"}"), "daily at 09:30, your time (Europe/Helsinki); next Wed 14 Oct 09:30 EEST")), "yes")
    C.Check.str("routine_set both", yes(String.contains(call(z, 1791957600n, th.of(z, "Miso"), "routine_set", "{\"name\":\"r\",\"schedule\":\"daily 09:30\",\"at\":\"in 2h\",\"prompt\":\"p\"}"), "not both")), "yes")
    C.Check.str("routine_set neither", yes(String.contains(call(z, 1791957600n, th.of(z, "Miso"), "routine_set", "{\"name\":\"r\",\"prompt\":\"p\"}"), "give a schedule")), "yes")
    C.Check.str("routine_set bad time", yes(String.contains(call(z, 1791957600n, th.of(z, "Miso"), "routine_set", "{\"name\":\"r\",\"at\":\"whenever\",\"prompt\":\"p\"}"), "I can read")), "yes")
    C.Check.str("routine_set bad zone", yes(String.contains(call(z, 1791957600n, th.of(z, "Miso"), "routine_set", "{\"name\":\"r\",\"schedule\":\"daily 09:30\",\"tz\":\"../x\",\"prompt\":\"p\"}"), "not a time zone")), "yes")
    C.Check.str("routine_set machine zone is stored empty", yes(String.contains(changes.show(tags.cs(call.step(z, 1791957600n, th.of(z, "Miso"), "routine_set", "{\"name\":\"r\",\"schedule\":\"daily 09:30\",\"tz\":\"machine\",\"prompt\":\"p\"}"))), "\"tz\":\"\"")), "yes")
    C.Check.str("routine_set edit keeps the zone", yes(String.contains(changes.show(tags.cs(call.step(run(z, call.step(z, 1791957600n, th.of(z, "Miso"), "routine_set", "{\"name\":\"r\",\"schedule\":\"daily 09:30\",\"tz\":\"America/New_York\",\"prompt\":\"p\"}")), 1791957700n, th.of(z, "Miso"), "routine_set", "{\"id\":\"" ++ rid(run(z, call.step(z, 1791957600n, th.of(z, "Miso"), "routine_set", "{\"name\":\"r\",\"schedule\":\"daily 09:30\",\"tz\":\"America/New_York\",\"prompt\":\"p\"}"))) ++ "\",\"name\":\"r\",\"schedule\":\"daily 10:00\",\"prompt\":\"p\"}"))), "\"tz\":\"America/New_York\"")), "yes")
    C.Check.str("person_timezone", yes(String.contains(call(z, 1791957600n, th.of(z, "Miso"), "person_timezone", "{}"), "\\\"zone\\\":\\\"Europe/Helsinki\\\"")), "yes")
    C.Check.str("person_timezone abbreviation", yes(String.contains(call(z, 1791957600n, th.of(z, "Miso"), "person_timezone", "{}"), "EEST")), "yes")
    C.Check.str("person_timezone set known", st.tags(call.step(z, 1791957600n, th.of(z, "Miso"), "person_timezone", "{\"zone\":\"America/New_York\"}")), "SettingSet | ")
    C.Check.str("person_timezone set unknown looks it up", st.tags(call.step(z, 1791957600n, th.of(z, "Miso"), "person_timezone", "{\"zone\":\"Asia/Tokyo\"}")), "| fx:zone ")
    C.Check.str("person_timezone auto", st.tags(call.step(z, 1791957600n, th.of(z, "Miso"), "person_timezone", "{\"zone\":\"auto\"}")), "SettingSet | ")
    C.Check.str("person_timezone bad name", st.tags(call.step(z, 1791957600n, th.of(z, "Miso"), "person_timezone", "{\"zone\":\"a b\"}")), "| ")
    C.Check.str("zone.report logs the device zone", st.tags(hub.ask(z, 1791957600n, "zone.report", "{\"zone\":\"Asia/Tokyo\"}")), "SettingSet | send:reply ")
    C.Check.str("zone.report same zone", st.tags(hub.ask(run(z, hub.ask(z, 1791957600n, "zone.report", "{\"zone\":\"Asia/Tokyo\"}")), 1791957700n, "zone.report", "{\"zone\":\"Asia/Tokyo\"}")), "| send:reply ")
    C.Check.str("zone.report bad name", st.tags(hub.ask(z, 1791957600n, "zone.report", "{\"zone\":\"x y\"}")), "| send:reply ")
    C.Check.str("zone.set known", st.tags(hub.ask(z, 1791957600n, "zone.set", "{\"zone\":\"America/New_York\"}")), "SettingSet | send:reply ")
    C.Check.str("zone.set unknown looks it up", st.tags(hub.ask(z, 1791957600n, "zone.set", "{\"zone\":\"Asia/Tokyo\"}")), "| fx:zone ")
    C.Check.str("zone.set auto", st.tags(hub.ask(z, 1791957600n, "zone.set", "{\"zone\":\"auto\"}")), "SettingSet | send:reply ")
    C.Check.str("zone.set bad", st.tags(hub.ask(z, 1791957600n, "zone.set", "{\"zone\":\"/etc\"}")), "| send:reply ")
    # an old log: a routine with no plan behaves as before
    +lg : M.State = M.State.replay([M.RoutineSet{"o1", id.of(s, "Miso"), "old", "every 15m", "p", True{}, 1000n}], s)
    C.Check.str("old log: not due early", st.tags(tick(lg, 1500n)), "| ")
    C.Check.str("old log: due as before, no slot", st.tags(tick(lg, 1860n)), "RoutineFired BotWoke MessagePosted MessageFrom TurnChanged | out out out out ")
    # webhooks
    +h : M.State = made(s, 1000n, "bots.hook", "{\"bot\":\"" ++ id.of(s, "Tofu") ++ "\",\"name\":\"github\"}")
    C.Check.str("hook make", st.tags(ask(s, 1000n, "bots.hook", "{\"bot\":\"" ++ id.of(s, "Tofu") ++ "\",\"name\":\"github\"}")), "HookSet | fx:hook ")
    C.Check.str("hook hit", st.tags(BH.Bots.hooked(h, 1010n, hook.of(h), "{\"action\":\"opened\"}")), "HookHit BotWoke MessagePosted MessageFrom TurnChanged | out out out ")
    C.Check.str("hook untrusted", yes(String.contains(BH.Bots.turn(h, 1010n, "x", "y"), "y")), "yes")
    C.Check.str("hook revoked", st.tags(BH.Bots.hooked(made(h, 1011n, "bots.hook_revoke", "{\"hook\":\"" ++ hook.of(h) ++ "\"}"), 1012n, hook.of(h), "{}")), "| ")
    C.Check.str("hook unknown", st.tags(BH.Bots.hooked(h, 1010n, "nope", "{}")), "| ")
    # a linked machine's bot
    C.Check.str("incoming", st.tags(BH.Bots.incoming(s, 1010n, "laptop", Maybe.default(&2, J.Json, J.Json.parse("{\"from\":\"Nori\",\"to\":\"miso\",\"text\":\"yo\",\"hop\":2}"), J.Null{}))),
      "RoomPosted BotWoke MessagePosted MessageFrom TurnChanged | out out out out ")
    C.Check.str("incoming too deep", st.tags(BH.Bots.incoming(s, 1010n, "laptop", Maybe.default(&2, J.Json, J.Json.parse("{\"from\":\"Nori\",\"to\":\"miso\",\"text\":\"yo\",\"hop\":9}"), J.Null{}))), "RoomPosted | ")
    C.Check.str("remote no peer", yes(String.contains(call(s, 1004n, th.of(s, "Miso"), "bot_send", "{\"to\":\"Nori@laptop\",\"text\":\"hi\"}"), "no linked machine")), "yes")
    +p : M.State = run(s, B.Step{[M.PeerSet{"p1", "laptop", "http://laptop:3787", False{}, 1000n}], Nil{}})
    C.Check.str("remote send", st.tags(call.step(p, 1004n, th.of(p, "Miso"), "bot_send", "{\"to\":\"Nori@laptop\",\"text\":\"hi\"}")), "RoomSet RoomPosted | fx:send ")
    C.Check.str("remote request shared", yes(String.contains(st.body(call.step(p, 1004n, th.of(p, "Miso"), "bot_send", "{\"to\":\"Nori@laptop\",\"topic\":\"fab\",\"text\":\"hi\"}")), "\"members\":[\"Miso@\",\"Nori\"]")), "yes")
    C.Check.str("remote request tos", yes(String.contains(st.body(call.step(p, 1004n, th.of(p, "Miso"), "bot_send", "{\"to\":\"Nori@laptop\",\"topic\":\"fab\",\"text\":\"hi\"}")), "\"tos\":[\"Nori\"]")), "yes")
    C.Check.str("remote direct", st.tags(call.step(p, 1004n, th.of(p, "Miso"), "bot_send", "{\"to\":\"Nori@laptop\",\"text\":\"hi\",\"direct\":true}")), "RoomPosted | fx:send ")
    # shared rooms: a room with members on a linked machine
    +sr : M.State = run(p, call.step(p, 1006n, th.of(p, "Miso"), "room_create", "{\"name\":\"team\",\"members\":\"Tofu, Nori@laptop\"}"))
    C.Check.str("shared post", st.tags(call.step(sr, 1007n, th.of(sr, "Miso"), "room_post", "{\"room\":\"team\",\"text\":\"standup\"}")),
      "RoomPosted BotWoke MessagePosted MessageFrom TurnChanged | fx:send out out out ")
    C.Check.str("shared tos", yes(String.contains(st.body(call.step(sr, 1007n, th.of(sr, "Miso"), "room_post", "{\"room\":\"team\",\"text\":\"standup\"}")), "\"tos\":[\"Nori\"]")), "yes")
    C.Check.str("shared members", yes(String.contains(st.body(call.step(sr, 1007n, th.of(sr, "Miso"), "room_post", "{\"room\":\"team\",\"text\":\"standup\"}")), "\"members\":[\"Miso@\",\"Tofu@\",\"Nori\"]")), "yes")
    C.Check.str("shared room id", yes(String.contains(st.body(call.step(sr, 1007n, th.of(sr, "Miso"), "room_post", "{\"room\":\"team\",\"text\":\"standup\"}")), "\"room\":\"" ++ BT.Room.id(room.of(sr, "team")) ++ "\"")), "yes")
    # a room a person makes in the app, with a bot on a linked machine picked
    +pr : M.State = made(p, 1008n, "bots.room", "{\"name\":\"crew\",\"members\":\"" ++ id.of(p, "Tofu") ++ "\\nNori@laptop\"}")
    C.Check.str("picked remote kept", BT.Room.members.show(BT.Room.members(room.of(pr, "crew"))), id.of(p, "Tofu") ++ "\nNori@laptop")
    C.Check.str("person post reaches it", st.tags(ask(pr, 1009n, "bots.post", "{\"room\":\"" ++ BT.Room.id(room.of(pr, "crew")) ++ "\",\"text\":\"hey\"}")),
      "RoomPosted BotWoke MessagePosted MessageFrom TurnChanged | send:reply fx:send out out out ")
    C.Check.str("person post tos", yes(String.contains(st.body(ask(pr, 1009n, "bots.post", "{\"room\":\"" ++ BT.Room.id(room.of(pr, "crew")) ++ "\",\"text\":\"hey\"}")), "\"tos\":[\"Nori\"]")), "yes")
    # the other machine: it keeps the room, and its bots can post back
    +q : M.State = run(s, B.Step{[M.PeerSet{"p9", "desk", "http://desk:3787", False{}, 1000n}], Nil{}})
    +post : String = "{\"from\":\"Nori\",\"to\":\"miso\",\"tos\":[\"miso\"],\"members\":[\"Nori@\",\"miso\",\"Ghost@far\"],\"room\":\"r5-0\",\"roomName\":\"team\",\"text\":\"hi\",\"hop\":1}"
    C.Check.str("shared in", st.tags(BH.Bots.incoming(q, 1010n, "desk", json(post))), "RoomSet RoomPosted BotWoke MessagePosted MessageFrom TurnChanged | out out out out ")
    +q2 : M.State = run(q, BH.Bots.incoming(q, 1010n, "desk", json(post)))
    C.Check.str("shared kept", BT.Room.members.show(BT.Room.members(room.of(q2, "team"))), "Nori@desk\n" ++ id.of(q2, "Miso") ++ "\nGhost@far")
    # Miso is still on the first one: the second waits, and the room is unchanged
    C.Check.str("shared again", st.tags(BH.Bots.incoming(q2, 1011n, "desk", json(post))), "RoomPosted BotWoke MessagePosted MessageFrom TurnChanged | out out out out ")
    C.Check.str("shared read", yes(String.contains(call(q2, 1012n, th.of(q2, "Miso"), "room_read", "{\"room\":\"team\"}"), "Nori@desk")), "yes")
    # a request from the other machine: answering it goes back to its room
    +fq : M.State = run(q, BH.Bots.incoming(q, 1010n, "desk", json("{\"from\":\"Nori\",\"to\":\"miso\",\"tos\":[\"miso\"],\"members\":[\"Nori@\",\"miso\"],\"room\":\"r7-0\",\"roomName\":\"fab\",\"text\":\"quote?\",\"hop\":1}")))
    C.Check.str("remote reply reuses", st.tags(call.step(fq, 1012n, th.of(fq, "Miso"), "bot_send", "{\"to\":\"Nori@desk\",\"topic\":\"fab\",\"text\":\"on it\"}")), "RoomPosted | fx:send ")
    C.Check.str("remote reply same id", yes(String.contains(st.body(call.step(fq, 1012n, th.of(fq, "Miso"), "bot_send", "{\"to\":\"Nori@desk\",\"topic\":\"fab\",\"text\":\"on it\"}")), "\"room\":\"r7-0\"")), "yes")
    C.Check.str("shared back", st.tags(call.step(q2, 1012n, th.of(q2, "Miso"), "room_post", "{\"room\":\"team\",\"text\":\"on it\"}")), "RoomPosted | fx:send ")
    C.Check.str("shared back members", yes(String.contains(st.body(call.step(q2, 1012n, th.of(q2, "Miso"), "room_post", "{\"room\":\"team\",\"text\":\"on it\"}")),
      "\"members\":[\"Nori\",\"Miso@\",\"Ghost@far\"]")), "yes")
    C.Check.str("shared not a member", st.tags(BH.Bots.incoming(q, 1010n, "desk", json("{\"from\":\"Nori\",\"tos\":[\"tofu\"],\"members\":[\"Nori@\",\"miso\"],\"room\":\"r5-0\",\"roomName\":\"team\",\"text\":\"hi\",\"hop\":1}"))),
      "RoomSet RoomPosted | ")
    C.Check.str("shared too deep", st.tags(BH.Bots.incoming(q, 1010n, "desk", json("{\"from\":\"Nori\",\"tos\":[\"miso\"],\"members\":[\"Nori@\",\"miso\"],\"room\":\"r5-0\",\"roomName\":\"team\",\"text\":\"hi\",\"hop\":9}"))), "RoomSet RoomPosted | ")
    +lr : M.State = run(q, call.step(q, 1006n, th.of(q, "Miso"), "room_create", "{\"name\":\"ours\",\"members\":\"Tofu\"}"))
    C.Check.str("shared no takeover", st.tags(BH.Bots.incoming(lr, 1010n, "desk", json("{\"from\":\"Nori\",\"tos\":[\"miso\"],\"members\":[\"Nori@\",\"miso\"],\"room\":\"" ++ BT.Room.id(room.of(lr, "ours")) ++ "\",\"roomName\":\"ours\",\"text\":\"hi\",\"hop\":1}"))), "| ")
    C.Check.str("shared deleted stays", st.tags(BH.Bots.incoming(run(q2, B.Step{[M.RoomDeleted{"r5-0", 1011n}], Nil{}}), 1012n, "desk", json(post))), "| ")
    C.Check.str("dm still dm", st.tags(BH.Bots.incoming(q, 1010n, "desk", json("{\"from\":\"Nori\",\"to\":\"miso\",\"room\":\"dm:a:b\",\"members\":[],\"text\":\"yo\",\"hop\":1}"))),
      "RoomPosted BotWoke MessagePosted MessageFrom TurnChanged | out out out out ")
    # tools list: a bot sees its own tools, a thread does not
    C.Check.str("tools bot", yes(String.contains(out.reply(BH.Bots.mcp(s, 1n, th.of(s, "Miso"), J.Null{}, "t", J.Json.arr(Nil{}), Nil{}, "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\"}")), "memory_save")), "yes")
    C.Check.str("tools thread", yes(String.contains(out.reply(BH.Bots.mcp(s, 1n, "t9", J.Null{}, "t", J.Json.arr(Nil{}), Nil{}, "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\"}")), "memory_save")), "no")
    # task skills (core/teach.bend, docs/teach.md)
    C.Check.str("tools task list", yes(String.contains(out.reply(BH.Bots.mcp(s, 1n, th.of(s, "Miso"), J.Null{}, "t", J.Json.arr(Nil{}), Nil{}, "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\"}")), "task_list")), "yes")
    C.Check.str("tools task run", yes(String.contains(out.reply(BH.Bots.mcp(s, 1n, th.of(s, "Miso"), J.Null{}, "t", J.Json.arr(Nil{}), Nil{}, "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\"}")), "task_run")), "yes")
    C.Check.str("tools task step", yes(String.contains(out.reply(BH.Bots.mcp(s, 1n, th.of(s, "Miso"), J.Null{}, "t", J.Json.arr(Nil{}), Nil{}, "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\"}")), "task_step")), "yes")
    C.Check.str("tools task pause", yes(String.contains(out.reply(BH.Bots.mcp(s, 1n, th.of(s, "Miso"), J.Null{}, "t", J.Json.arr(Nil{}), Nil{}, "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\"}")), "task_pause")), "yes")
    C.Check.str("tools task end", yes(String.contains(out.reply(BH.Bots.mcp(s, 1n, th.of(s, "Miso"), J.Null{}, "t", J.Json.arr(Nil{}), Nil{}, "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\"}")), "task_end")), "yes")
    C.Check.str("tools task save", yes(String.contains(out.reply(BH.Bots.mcp(s, 1n, th.of(s, "Miso"), J.Null{}, "t", J.Json.arr(Nil{}), Nil{}, "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\"}")), "task_save")), "yes")
    C.Check.str("task save", st.tags(call.step(s, 1300n, th.of(s, "Miso"), "task_save", "{\"slug\":\"Order status\",\"name\":\"Order status\",\"description\":\"Looks an order up\",\"steps\":[{\"do\":\"navigate\",\"url\":\"https://a.test/\"}]}")), "TaskSet | fx:task-file ")
    +tk : M.State = run(s, call.step(s, 1300n, th.of(s, "Miso"), "task_save", "{\"slug\":\"Order status\",\"name\":\"Order status\",\"description\":\"Looks an order up\",\"steps\":[{\"do\":\"navigate\",\"url\":\"https://a.test/\"}]}"))
    C.Check.str("task list", yes(String.contains(call(tk, 1301n, th.of(tk, "Miso"), "task_list", "{}"), "order-status")), "yes")
    C.Check.str("task list name", yes(String.contains(call(tk, 1301n, th.of(tk, "Miso"), "task_list", "{}"), "Order status")), "yes")
    C.Check.str("task get", yes(String.contains(call(tk, 1302n, th.of(tk, "Miso"), "task_get", "{\"slug\":\"order-status\"}"), "https://a.test/")), "yes")
    C.Check.str("task get missing", yes(String.contains(call(tk, 1302n, th.of(tk, "Miso"), "task_get", "{\"slug\":\"nope\"}"), "no such task")), "yes")
    C.Check.str("task save keeps the rest", yes(String.contains(call(run(tk, call.step(tk, 1303n, th.of(tk, "Miso"), "task_save", "{\"slug\":\"order-status\",\"description\":\"better\"}")), 1304n, th.of(tk, "Miso"), "task_get", "{\"slug\":\"order-status\"}"), "https://a.test/")), "yes")
    C.Check.str("task save new text", yes(String.contains(call(run(tk, call.step(tk, 1303n, th.of(tk, "Miso"), "task_save", "{\"slug\":\"order-status\",\"description\":\"better\"}")), 1304n, th.of(tk, "Miso"), "task_get", "{\"slug\":\"order-status\"}"), "better")), "yes")
    C.Check.str("task save needs a name", yes(String.contains(call(s, 1303n, th.of(s, "Miso"), "task_save", "{\"slug\":\"x\"}"), "name the task")), "yes")
    C.Check.str("task delete", st.tags(call.step(tk, 1305n, th.of(tk, "Miso"), "task_delete", "{\"slug\":\"order-status\"}")), "TaskDeleted | fx:task-gone ")
    C.Check.str("task deleted gone", yes(String.contains(call(run(tk, call.step(tk, 1305n, th.of(tk, "Miso"), "task_delete", "{\"slug\":\"order-status\"}")), 1306n, th.of(tk, "Miso"), "task_list", "{}"), "order-status")), "no")
    C.Check.str("task run request", st.tags(ask(tk, 1307n, "bots.task", "{\"bot\":\"" ++ id.of(tk, "Miso") ++ "\",\"op\":\"run\",\"slug\":\"order-status\",\"params\":{\"order\":\"W1\"}}")), "BotWoke MessagePosted MessageFrom TurnChanged | send:reply out out out out ")
    C.Check.str("task run missing", st.tags(ask(tk, 1307n, "bots.task", "{\"bot\":\"" ++ id.of(tk, "Miso") ++ "\",\"op\":\"run\",\"slug\":\"nope\"}")), "| send:reply ")
    C.Check.str("task delete request", st.tags(ask(tk, 1308n, "bots.task", "{\"bot\":\"" ++ id.of(tk, "Miso") ++ "\",\"op\":\"delete\",\"slug\":\"order-status\"}")), "TaskDeleted | send:reply fx:task-gone ")
    +browserThread : M.State = M.State.replay([M.ThreadCreated{"ordinary", "", "Thread", M.Local{}, "", "", "claude", "", 100n}], M.State.empty())
    C.Check.str("ordinary thread browser request", st.tags(ask(browserThread, 1309n, "bots.browser", "{\"bot\":\"ordinary\",\"op\":\"takeover\"}")), "| out ")
    C.Check.str("missing browser owner rejected", yes(String.contains(st.body(ask(browserThread, 1309n, "bots.browser", "{\"bot\":\"missing\",\"op\":\"takeover\"}")), "no such bot")), "yes")
    C.Check.str("browser request", st.tags(ask(s, 1309n, "bots.browser", "{\"bot\":\"" ++ id.of(s, "Miso") ++ "\",\"op\":\"click\"}")), "| out ")
    C.Check.str("teach request", st.tags(ask(s, 1309n, "bots.teach", "{\"bot\":\"" ++ id.of(s, "Miso") ++ "\",\"op\":\"start\"}")), "| out ")
    C.Check.str("teach unknown bot", st.tags(ask(s, 1309n, "bots.teach", "{\"bot\":\"nobody\",\"op\":\"start\"}")), "| send:reply ")
    C.Check.str("prompt tasks", yes(String.contains(BH.Bots.prompt(s, th.of(s, "Miso"), "/h"), "task_pause")), "yes")
    C.Check.str("prompt never complete", yes(String.contains(BH.Bots.prompt(s, th.of(s, "Miso"), "/h"), "Never complete a purchase, send or deletion without a confirm answer")), "yes")
    C.Check.str("turn lists tasks", yes(String.contains(BH.Bots.turn(tk, 1400n, th.of(tk, "Miso"), "hello"), "<tasks>\n- order-status: Order status")), "yes")
    C.Check.str("turn without tasks", yes(String.contains(BH.Bots.turn(s, 1400n, th.of(s, "Miso"), "hello"), "<tasks>")), "no")
    C.Check.str("prompt", yes(String.contains(BH.Bots.prompt(s, th.of(s, "Miso"), "/h"), "You are Miso")), "yes")
    C.Check.str("prompt rooms", yes(String.contains(BH.Bots.prompt(s, th.of(s, "Miso"), "/h"), "it goes in a room of the two of you")), "yes")
    C.Check.str("prompt threads", yes(String.contains(BH.Bots.prompt(s, th.of(s, "Miso"), "/h"), "New work gets its own visible thread: start it with thread_launch and its `project`")), "yes")
    # the schemas a bot sees name every parameter the server takes
    +tl : String = out.reply(BH.Bots.mcp(s, 1n, th.of(s, "Miso"), J.Null{}, "t", J.Json.arr(Nil{}), Nil{}, "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\"}"))
    C.Check.str("launch schema project", yes(String.contains(tl, "\"project\":{\"type\":\"string\",\"description\":\"The project's name or id (default: this thread's; bots must give one); on another machine its <link>~<id> from machine_list.\"}")), "yes")
    C.Check.str("send schema topic", yes(String.contains(tl, "\"topic\":{\"type\":\"string\"")), "yes")
    C.Check.str("send schema direct", yes(String.contains(tl, "\"direct\":{\"type\":\"boolean\"")), "yes")
    C.Check.str("delegate schema project", yes(String.contains(tl, "\"project\":{\"type\":\"string\",\"description\":\"Bots only")), "yes")
    C.Check.str("space", st.tags(call.step(s, 1004n, th.of(s, "Miso"), "space_set", "{\"spec\":\"{\\\"title\\\":\\\"hi\\\"}\"}")), "SpaceSet | ")
    C.Check.str("space bad", yes(String.contains(call(s, 1004n, th.of(s, "Miso"), "space_set", "{\"spec\":\"{nope\"}"), "not valid JSON")), "yes")
    C.Check.str("space press", yes(String.contains(tags(Maybe.default(&2, List<&2, M.Change>, None{}, Nil{})), "")), "yes")
    C.Check.str("page", st.tags(call.step(s, 1004n, th.of(s, "Miso"), "space_page", "{\"html\":\"<h1>hi</h1>\"}")), "SpacePageSet | ")
    C.Check.str("page kept", BT.Page.html(BT.World.spage(M.State.bots(run(s, call.step(s, 1004n, th.of(s, "Miso"), "space_page", "{\"html\":\"<h1>hi</h1>\"}"))), id.of(s, "Miso"))), "<h1>hi</h1>")
    C.Check.str("page too big", yes(String.contains(call(s, 1004n, th.of(s, "Miso"), "space_page", "{\"html\":\"" ++ String.repeat(String.repeat("x", 1024n), 257n) ++ "\"}"), "over 256 KB")), "yes")
    C.Check.str("page codec", yes(String.contains(J.Json.show(CD.Change.encode(M.SpacePageSet{"b", "<p>", 9n})), "\"html\":\"<p>\"")), "yes")
    C.Check.str("delete", st.tags(ask(s, 1004n, "bots.delete", "{\"bot\":\"" ++ id.of(s, "Tofu") ++ "\"}")), "BotDeleted | send:reply out fx:gone ")
    C.Check.str("deleted gone", Nat.show(List.length(&2, BT.Bot, BT.World.bots(M.State.bots(made(s, 1004n, "bots.delete", "{\"bot\":\"" ++ id.of(s, "Tofu") ++ "\"}"))))), "1")
    C.Check.str("codec", yes(String.contains(J.Json.show(CD.Change.encode(M.RoomPosted{"p", "r", "a", "t", 2n, 9n})), "\"hop\":2")), "yes")
    # the Pebble ring
    C.Check.str("ring text", RG.Ring.text(ring.body()), "remind me to order stencils")
    C.Check.str("ring part", RG.Ring.part(ring.body(), "client"), "ring")
    C.Check.str("ring no part", RG.Ring.part(ring.body(), "audio"), "")
    C.Check.str("ring test flag", yes(RG.Ring.test(ring.test())), "yes")
    C.Check.str("ring note flag", yes(RG.Ring.test(ring.body())), "no")
    C.Check.str("ring wakes", st.tags(BH.Bots.ring(h, 1010n, hook.of(h), ring.body())), "HookHit BotWoke MessagePosted MessageFrom TurnChanged | out out out ")
    C.Check.str("ring words reach the bot", yes(String.contains(changes.show(tags.cs(BH.Bots.ring(h, 1010n, hook.of(h), ring.body()))), "[voice note from your Pebble ring]\\nremind me to order stencils")), "yes")
    C.Check.str("ring test wakes no one", st.tags(BH.Bots.ring(h, 1010n, hook.of(h), ring.test())), "HookHit | ")
    # a bearer token's form, as JSON for the bot
    C.Check.str("form json", RG.Ring.form(ring.body(), ""), "{\"source\":\"pebble-index-01\",\"transcription\":\"remind me to order stencils\",\"recordedAt\":1790000000000,\"client\":\"ring\"}")
    C.Check.str("form audio", RG.Ring.form("--z\r\nContent-Disposition: form-data; name=\"audio\"; filename=\"a.m4a\"\r\nContent-Type: audio/mp4\r\n\r\n\u{0}\u{1}m4a\r\n--z\r\nContent-Disposition: form-data; name=\"recordedAt\"\r\n\r\n12\r\n--z--\r\n", " 5 "), "{\"source\":\"form-data\",\"recordedAt\":12,\"audio\":\"omitted, 5 bytes\"}")
    C.Check.str("form audio no size", RG.Ring.form("--z\r\nContent-Disposition: form-data; name=\"audio\"\r\n\r\nxx\r\n--z--\r\n", ""), "{\"source\":\"form-data\",\"audio\":\"omitted\"}")
    C.Check.str("form quotes", RG.Ring.form("--q\r\nContent-Disposition: form-data; name=\"transcription\"\r\n\r\nsay \"hi\"\nnow\r\n--q--\r\n", ""), "{\"source\":\"form-data\",\"transcription\":\"say \\\"hi\\\"\\nnow\"}")
    C.Check.str("form is form", yes(RG.Ring.is_form("multipart/form-data; boundary=abc")), "yes")
    C.Check.str("json is not form", yes(RG.Ring.is_form("application/json")), "no")
    C.Check.str("form reaches the bot as data", yes(String.contains(changes.show(tags.cs(BH.Bots.hooked(h, 1010n, hook.of(h), RG.Ring.form(ring.body(), "")))), "untrusted data")), "yes")
    # a webhook's bearer token: kept by the server, asked for by a client
    C.Check.str("token show", st.tags(ask(h, 1010n, "bots.hook_token", "{\"hook\":\"" ++ hook.of(h) ++ "\"}")), "| fx:token ")
    C.Check.str("token new", st.tags(ask(h, 1010n, "bots.hook_token", "{\"hook\":\"" ++ hook.of(h) ++ "\",\"op\":\"new\"}")), "| fx:retoken ")
    C.Check.str("token off", st.tags(ask(h, 1010n, "bots.hook_token", "{\"hook\":\"" ++ hook.of(h) ++ "\",\"op\":\"off\"}")), "| fx:untoken ")
    C.Check.str("token no hook", st.tags(ask(h, 1010n, "bots.hook_token", "{\"hook\":\"h0-0\"}")), "| send:reply ")
    C.Check.str("token revoked", st.tags(ask(made(h, 1011n, "bots.hook_revoke", "{\"hook\":\"" ++ hook.of(h) ++ "\"}"), 1012n, "bots.hook_token", "{\"hook\":\"" ++ hook.of(h) ++ "\"}")), "| send:reply ")
    C.Check.str("ring revoked", st.tags(BH.Bots.ring(made(h, 1011n, "bots.hook_revoke", "{\"hook\":\"" ++ hook.of(h) ++ "\"}"), 1012n, hook.of(h), ring.body())), "| ")
    # sub-bots
    C.Check.str("bot_create", st.tags(call.step(s, 1500n, th.of(s, "Miso"), "bot_create", "{\"name\":\"Kit\",\"persona\":\"watches CI\"}")), "ThreadCreated BotSet SettingSet | fx:home ")
    +k1 : M.State = spawned(s, "Miso", "Kit")
    C.Check.str("sub-bot is a bot", Nat.show(List.length(&2, BT.Bot, BT.World.bots(M.State.bots(k1)))), "3")
    C.Check.str("sub-bot knows its maker", M.Setting.get(k1, "bot.parent." ++ id.of(k1, "Kit"), ""), id.of(k1, "Miso"))
    +k2 : M.State = spawned(k1, "Kit", "Kitten")
    C.Check.str("two levels", yes(Maybe.is.k(BT.World.find(M.State.bots(k2), "Kitten"))), "yes")
    C.Check.str("three levels refused", yes(String.contains(call(k2, 1600n, th.of(k2, "Kitten"), "bot_create", "{\"name\":\"Speck\",\"persona\":\"x\"}"), "levels deep")), "yes")
    C.Check.str("name taken", yes(String.contains(call(k2, 1600n, th.of(k2, "Miso"), "bot_create", "{\"name\":\"kit\",\"persona\":\"x\"}"), "already exists")), "yes")
    C.Check.str("bad name", yes(String.contains(call(k2, 1600n, th.of(k2, "Miso"), "bot_create", "{\"name\":\"a b\",\"persona\":\"x\"}"), "letters, digits")), "yes")
    C.Check.str("spawn bound", yes(BH.Spawn.bound(BH.Spawn.next(5n))), "yes")
    # projects
    +pj : M.State = proj()
    C.Check.str("project_list", yes(String.contains(call(pj, 1600n, th.of(pj, "Miso"), "project_list", "{}"), "Power board")), "yes")
    C.Check.str("bot sees project threads", yes(String.contains(call(pj, 1600n, th.of(pj, "Miso"), "thread_list", "{}"), "Rev C review")), "yes")
    C.Check.str("bot list hides bot threads", yes(String.contains(call(pj, 1600n, th.of(pj, "Miso"), "thread_list", "{}"), th.of(pj, "Tofu"))), "no")
    C.Check.str("bot launches in a project", st.tags(call.step(pj, 1600n, th.of(pj, "Miso"), "thread_launch", "{\"project\":\"power board\",\"prompt\":\"check the LDO\"}")),
      "ThreadCreated ThreadLaunched MessagePosted TurnQueued | out ")
    C.Check.str("launched thread is in the project", yes(String.contains(changes.show(tags.cs(call.step(pj, 1600n, th.of(pj, "Miso"), "thread_launch", "{\"project\":\"p1\",\"prompt\":\"check the LDO\"}"))), "\"project\":\"p1\"")), "yes")
    C.Check.str("launched message names the bot", yes(String.contains(changes.show(tags.cs(call.step(pj, 1600n, th.of(pj, "Miso"), "thread_launch", "{\"project\":\"p1\",\"prompt\":\"check the LDO\"}"))), "t:" ++ th.of(pj, "Miso"))), "yes")
    C.Check.str("bot must name a project", yes(String.contains(call(pj, 1600n, th.of(pj, "Miso"), "thread_launch", "{\"prompt\":\"x\"}"), "name an existing project")), "yes")
    C.Check.str("bot reads a project thread", yes(String.contains(call(pj, 1600n, th.of(pj, "Miso"), "thread_read", "{\"threadId\":\"t-pb\"}"), "not in this project")), "no")
    # a bot's delegated work: a visible thread in the project it names
    C.Check.str("bot delegate needs a project", yes(String.contains(call(pj, 1600n, th.of(pj, "Miso"), "delegate_task", "{\"prompt\":\"x\"}"), "name a project")), "yes")
    C.Check.str("bot delegate unknown project", yes(String.contains(call(pj, 1600n, th.of(pj, "Miso"), "delegate_task", "{\"prompt\":\"x\",\"project\":\"nope\"}"), "name a project")), "yes")
    +dg : B.Step = call.step(pj, 1600n, th.of(pj, "Miso"), "delegate_task", "{\"prompt\":\"check the LDO\",\"project\":\"Power board\"}")
    C.Check.str("bot delegates", st.tags(dg), "ThreadCreated ModesSet TaskDelegated TaskStateSet MessagePosted TurnChanged | out out ")
    C.Check.str("bot delegate in the project", yes(String.contains(changes.show(tags.cs(dg)), "\"project\":\"p1\"")), "yes")
    C.Check.str("bot delegate names the bot", yes(String.contains(changes.show(tags.cs(dg)), "t:" ++ th.of(pj, "Miso"))), "yes")
    +dj : M.State = run(pj, dg)
    C.Check.str("bot delegate shown", Nat.show(hidden(dj)), "0")
    +tj : M.State = run(pj, call.step(pj, 1600n, "t-pb", "delegate_task", "{\"prompt\":\"x\"}"))
    C.Check.str("thread delegate still under it", Nat.show(hidden(tj)), "1")
    # new projects: only with the person's yes (the server asks; these are its pure parts)
    C.Check.str("project_create listed", yes(String.contains(out.reply(BH.Bots.mcp(pj, 1600n, th.of(pj, "Miso"), J.Null{}, "t", J.Json.arr(Nil{}), Nil{}, "{\"jsonrpc\":\"2.0\",\"id\":7,\"method\":\"tools/list\"}")), "project_create")), "yes")
    C.Check.str("project_create is the hub's", yes(BH.Bots.newproj.is(Maybe.default(&2, J.Json, J.Json.parse("{\"method\":\"tools/call\",\"params\":{\"name\":\"project_create\"}}"), J.Null{}))), "yes")
    C.Check.str("project_create asks", yes(BH.Bots.newproj.asks(M.Asks.all(pj), th.of(pj, "Miso"))), "yes")
    C.Check.str("project_create card", BH.Bots.newproj.detail(Maybe.default(&2, J.Json, J.Json.parse("{\"path\":\"~/boards/fan\",\"name\":\"Fan\"}"), J.Null{})), "New project Fan\nFolder: ~/boards/fan (made when missing)")
    C.Check.str("project_create adds", st.tags(res.step(BH.Bots.newproj.made(pj, 1700n, "/tmp/fan/", ""))), "ProjectCreated | ")
    C.Check.str("project_create names by folder", yes(String.contains(changes.show(tags.cs(res.step(BH.Bots.newproj.made(pj, 1700n, "/tmp/fan", "")))), "\"title\":\"fan\"")), "yes")
    C.Check.str("project_create again", st.tags(res.step(BH.Bots.newproj.made(pj, 1700n, "/tmp/power", "x"))), "| ")
    C.Check.str("project_create again answers it", yes(String.contains(J.Json.show(res.body(BH.Bots.newproj.made(pj, 1700n, "/tmp/power", "x"))), "\"id\":\"p1\"")), "yes")
