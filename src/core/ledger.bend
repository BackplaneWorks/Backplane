import Base
import ./json.bend as J
import ./model.bend as M
import ./bot.bend as BT
import ./hub.bend as B
import ./mcp.bend as MC
import ./codec.bend as C
import ./told.bend as TD
import ./rules.bend as RL
import ./tboard.bend as TB

# The tools of the three ledgers, for every thread's agent (a bot's or a
# plain thread's): told_check and told_mark (what the person was already
# told, told.bend) and the task_board_* tools (the shared task board,
# tboard.bend). Also the person's own requests for rules and the board:
# rules.set, rules.drop, board.set and board.drop.

def Ledger.names() -> List<&2, String>:
  ["told_check", "told_mark", "task_board_list", "task_board_add", "task_board_update", "task_board_remove", "task_board_import"]

def Ledger.tools() -> List<&2, J.Json>:
  [MC.Tool.def("told_check", "Before telling the person something (a digest item, a reminder, news), check whether they were already told: by its topic key, or by text (matching words). Answers {told: bool, matches: [{key, text, channel, bot, at, ago}]}. Covers the last 7 days by default (up to 30) on this machine, including every alert already sent (asks, finished work, failures, merges, notify_person notes, which are recorded by themselves). Alerting is notify_person's policy (needs_you, done, or a routine's reminder or digest); this only tells you what was already said.",
     [MC.Tool.prop("key", "string", "The topic key, e.g. lunch-dana-1028 (as given to told_mark)."), MC.Tool.prop("text", "string", "The text you are about to say; matched by its words."),
      MC.Tool.prop("days", "number", "How many days back (default 7, at most 30).")], Nil{}),
   MC.Tool.def("told_mark", "Record that you told the person something without an alert (in a digest, a room post, a chat answer, your Space, an email), so you and other bots skip it next time. Alerts, notify_person's included, are recorded by themselves. key: a short stable topic key (e.g. lunch-dana-1028); without one, it is made from the text. channel: digest, room, chat, email or other. Answers {key}.",
     [MC.Tool.prop("key", "string", "A short stable topic key; made from the text when omitted."), MC.Tool.prop("text", "string", "What you told them, in a sentence."),
      MC.Tool.prop("channel", "string", "digest, room, chat, email or other (default chat).")], ["text"]),
   MC.Tool.def("task_board_list", "List the shared task board: open items by default (all includes done and dropped). Answers {text, items}: text is one line per item, [status] title (id; @owner; %project; due D; group), and items has the full records.",
     [MC.Tool.prop("status", "string", "todo, doing, blocked, done or dropped; several separated by commas."), MC.Tool.prop("owner", "string", "A bot's name or id, me for yourself, or person."),
      MC.Tool.prop("project", "string", "A project's name or id."), MC.Tool.prop("group", "string", "A section name."), MC.Tool.prop("all", "boolean", "Include done and dropped items.")], Nil{}),
   MC.Tool.def("task_board_add", "Add an item to the shared task board. Answers {id}.",
     [MC.Tool.prop("title", "string", "What is to be done (at most 300 characters)."), MC.Tool.prop("status", "string", "todo (default), doing, blocked, done or dropped."),
      MC.Tool.prop("owner", "string", "A bot's name or id, me for yourself, or person (default)."), MC.Tool.prop("project", "string", "A project's name or id."),
      MC.Tool.prop("due", "string", "YYYY-MM-DD."), MC.Tool.prop("group", "string", "A section name."),
      MC.Tool.prop("links", "string", "URLs, thread ids or paths: a JSON array, or a string split on spaces and commas (at most 20)."), MC.Tool.prop("notes", "string", "Notes (at most 4000 characters).")], ["title"]),
   MC.Tool.def("task_board_update", "Change an item on the task board: only the fields you give change. Answers {id}.",
     [MC.Tool.prop("id", "string", "The item's id."), MC.Tool.prop("title", "string", "New title."), MC.Tool.prop("status", "string", "todo, doing, blocked, done or dropped."),
      MC.Tool.prop("owner", "string", "A bot's name or id, me for yourself, or person."), MC.Tool.prop("project", "string", "A project's name or id."),
      MC.Tool.prop("due", "string", "YYYY-MM-DD, or empty to clear."), MC.Tool.prop("group", "string", "A section name."), MC.Tool.prop("links", "string", "Replaces the links: a JSON array, or a string split on spaces and commas."),
      MC.Tool.prop("notes", "string", "Replaces the notes."), MC.Tool.prop("notes_append", "string", "A line added to the notes.")], ["id"]),
   MC.Tool.def("task_board_remove", "Delete an item from the task board for good. Prefer task_board_update with status dropped, which keeps a record.",
     [MC.Tool.prop("id", "string", "The item's id.")], ["id"]),
   MC.Tool.def("task_board_import", "Import a markdown todo list (checkbox items `- [ ]`, `- [x]`, `- [~]`; headings become groups). Lines without a checkbox are not imported and are returned as `skipped`. Importing the same list again adds nothing. Answers {added, duplicates, skipped}.",
     [MC.Tool.prop("markdown", "string", "The list."), MC.Tool.prop("project", "string", "Put every item in this project (a name or id)."), MC.Tool.prop("owner", "string", "A bot's name or id, me for yourself, or person (default).")], ["markdown"])]

def Ledger.known(+name: String) -> Bool:
  Nat.is_lt(C.Codec.pos(Ledger.names(), name), 7n)

# a tools/call naming one of the ledger tools
def Ledger.is(+o: J.Json) -> Bool:
  Bool.and(String.eq(J.Json.get_str(o, "method", ""), "tools/call"), Ledger.known(J.Json.get_str(J.Json.get_obj(o, "params"), "name", "")))

# Who is calling
# --------------

# the calling thread's bot id, "" for a thread that is no bot's
def Ledger.bot(+s: M.State, +thread: String) -> String:
  RL.Rules.bot.thread(s, thread)

# who an item or a rule is credited to: the bot's id, else the thread
def Ledger.by.k(+bot: String, +thread: String) -> String:
  Bool.pick(String, String.is_empty(bot), "thread:" ++ thread, bot)

def Ledger.by(+s: M.State, +thread: String) -> String:
  Ledger.by.k(Ledger.bot(s, thread), thread)

def Ledger.with(+o: J.Json, +key: String, +v: String) -> J.Json:
  match o:
    case J.Obj{fs}:
      J.Obj{J.Field{key, J.Str{v}, fs}}
    case _:
      o

# "me" in a request's owner is the calling bot (the person where there is none)
def Ledger.me.k(+me: Bool, +o: J.Json, +bot: String) -> J.Json:
  match me:
    case True{}:
      Ledger.with(o, "owner", Bool.pick(String, String.is_empty(bot), "person", bot))
    case False{}:
      o

def Ledger.me(+o: J.Json, +bot: String) -> J.Json:
  Ledger.me.k(String.eq(String.to_lower(String.trim(J.Json.get_str(o, "owner", ""))), "me"), o, bot)

def Ledger.ok(+body: J.Json, +cs: List<&2, M.Change>) -> MC.Res:
  MC.Res{True{}, body, B.Step{cs, Nil{}}}

# Told
# ----

def Ledger.ago.k(+d: Nat) -> String:
  Bool.pick(String, Nat.is_lt(d, 60n), "just now", Bool.pick(String, Nat.is_lt(d, 3600n), Nat.show(Nat.div(d, 60n)) ++ "m ago",
    Bool.pick(String, Nat.is_lt(d, 86400n), Nat.show(Nat.div(d, 3600n)) ++ "h ago", Nat.show(Nat.div(d, 86400n)) ++ "d ago")))

def Ledger.ago(+now: Nat, +at: Nat) -> String:
  Ledger.ago.k(Nat.sub(now, at))

# the bot's name for a record ("" for none)
def Ledger.bot.name(+s: M.State, +id: String) -> String:
  Bool.pick(String, String.is_empty(id), "", BT.Bot.name(BT.World.bot(M.State.bots(s), id)))

def Ledger.told.json(+t: TD.Told, +s: M.State, +now: Nat) -> J.Json:
  match t:
    case TD.Told{_, k, x, c, b, a}:
      J.Json.obj([J.KV{"key", J.Str{k}}, J.KV{"text", J.Str{x}}, J.KV{"channel", J.Str{c}}, J.KV{"bot", J.Str{Ledger.bot.name(s, b)}},
        J.KV{"at", J.Json.nat(a)}, J.KV{"ago", J.Str{Ledger.ago(now, a)}}])

def Ledger.told.items(+xs: List<&2, TD.Told>, +s: M.State, +now: Nat) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case t <> r:
      Ledger.told.json(t, s, now) <> Ledger.told.items(r, s, now)

def Ledger.told.found(+xs: List<&2, J.Json>) -> MC.Res:
  MC.Res.ok(J.Json.obj([J.KV{"told", J.Flag{Bool.not(List.is_empty(&2, J.Json, xs))}}, J.KV{"matches", J.Json.arr(xs)}]))

def Ledger.check.k(+none: Bool, +s: M.State, +now: Nat, +key: String, +text: String, +days: Nat) -> MC.Res:
  match none:
    case True{}:
      MC.Res.fail("give a key or a text to check")
    case False{}:
      Ledger.told.found(Ledger.told.items(TD.Told.check(s, now, key, text, days), s, now))

def Ledger.check(+s: M.State, +now: Nat, +a: J.Json) -> MC.Res:
  Ledger.check.k(Bool.and(String.is_empty(String.trim(J.Json.get_str(a, "key", ""))), String.is_empty(String.trim(J.Json.get_str(a, "text", "")))), s, now,
    J.Json.get_str(a, "key", ""), J.Json.get_str(a, "text", ""), J.Json.get_nat(a, "days", 0n))

def Ledger.mark.k(+none: Bool, +s: M.State, +now: Nat, +bot: String, +key: String, +text: String, +channel: String) -> MC.Res:
  match none:
    case True{}:
      MC.Res.fail("a text is needed")
    case False{}:
      Ledger.ok(J.Json.obj([J.KV{"key", J.Str{TD.Told.key.pick(key, text)}}]), TD.Told.mark(s, now, key, text, channel, bot))

def Ledger.mark(+s: M.State, +now: Nat, +bot: String, +a: J.Json) -> MC.Res:
  Ledger.mark.k(String.is_empty(String.trim(J.Json.get_str(a, "text", ""))), s, now, bot, J.Json.get_str(a, "key", ""), J.Json.get_str(a, "text", ""), J.Json.get_str(a, "channel", ""))

def Ledger.own.m(+me: Bool, +person: Bool, +o: String, +bot: String, +s: M.State) -> RL.RS:
  match me person:
    case True{} _:
      Bool.pick(RL.RS, String.is_empty(bot), RL.RS{"", "you are not a bot, so there is no \"me\""}, RL.RS{bot, ""})
    case False{} True{}:
      RL.RS{"person", ""}
    case False{} False{}:
      Bool.pick(RL.RS, String.is_empty(o), RL.RS{"", ""}, RL.Rules.bot.res(s, o))

# The board
# ---------

def Ledger.own.k(+o: String, +bot: String, +s: M.State) -> RL.RS:
  Ledger.own.m(String.eq(String.to_lower(o), "me"), String.eq(String.to_lower(o), "person"), o, bot, s)

# the owner a list asks for: a bot (by name or id), me, person, or any
def Ledger.own(+s: M.State, +bot: String, +o: String) -> RL.RS:
  Ledger.own.k(String.trim(o), bot, s)

def Ledger.list.items(+xs: List<&2, TB.TbItem>, +s: M.State) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case t <> r:
      TB.Tb.json(t, s) <> Ledger.list.items(r, s)

def Ledger.list.out(+xs: List<&2, TB.TbItem>, +s: M.State) -> MC.Res:
  MC.Res.ok(J.Json.obj([J.KV{"text", J.Str{TB.Tb.text(xs, s)}}, J.KV{"items", J.Json.arr(Ledger.list.items(xs, s))}]))

def Ledger.list.pick(+open: Bool, +xs: List<&2, TB.TbItem>) -> List<&2, TB.TbItem>:
  match open:
    case True{}:
      TB.Tb.open(xs)
    case False{}:
      xs

def Ledger.list.e(+ok: Bool, +ow: RL.RS, +pr: RL.RS, +s: M.State, +a: J.Json, +err: String) -> MC.Res:
  match ok:
    case True{}:
      Ledger.list.out(TB.Tb.order(Ledger.list.pick(Bool.and(String.is_empty(String.trim(J.Json.get_str(a, "status", ""))), Bool.not(J.Json.get_bool(a, "all", False{}))),
        TB.Tb.filter(TB.Tb.all(s), String.trim(J.Json.get_str(a, "status", "")), RL.RS.v(ow), RL.RS.v(pr), String.trim(J.Json.get_str(a, "group", ""))))), s)
    case False{}:
      MC.Res.fail(err)

def Ledger.list.k(+err: String, +ow: RL.RS, +pr: RL.RS, +s: M.State, +a: J.Json) -> MC.Res:
  Ledger.list.e(String.is_empty(err), ow, pr, s, a, err)

def Ledger.list.r(+ow: RL.RS, +pr: RL.RS, +s: M.State, +a: J.Json) -> MC.Res:
  Ledger.list.k(RL.Rules.err.first([RL.RS.err(ow), RL.RS.err(pr)]), ow, pr, s, a)

def Ledger.list(+s: M.State, +bot: String, +a: J.Json) -> MC.Res:
  Ledger.list.r(Ledger.own(s, bot, J.Json.get_str(a, "owner", "")), RL.Rules.proj.res(s, String.trim(J.Json.get_str(a, "project", ""))), s, a)

# a saved item: its id, and the changes
def Ledger.saved(+r: TB.TbR) -> MC.Res:
  match r:
    case TB.TbOk{+id, cs}:
      Ledger.ok(J.Json.obj([J.KV{"id", J.Str{id}}]), cs)
    case TB.TbErr{msg}:
      MC.Res.fail(msg)

def Ledger.add(+s: M.State, +now: Nat, +by: String, +bot: String, +a: J.Json) -> MC.Res:
  Ledger.saved(TB.Tb.add(s, now, by, Ledger.me(a, bot)))

def Ledger.update.k(+none: Bool, +s: M.State, +now: Nat, +by: String, +bot: String, +a: J.Json) -> MC.Res:
  match none:
    case True{}:
      MC.Res.fail("an id is needed")
    case False{}:
      Ledger.saved(TB.Tb.update(s, now, by, String.trim(J.Json.get_str(a, "id", "")), Ledger.me(a, bot)))

def Ledger.update(+s: M.State, +now: Nat, +by: String, +bot: String, +a: J.Json) -> MC.Res:
  Ledger.update.k(String.is_empty(String.trim(J.Json.get_str(a, "id", ""))), s, now, by, bot, a)

def Ledger.remove.k(+none: Bool, +s: M.State, +now: Nat, +a: J.Json) -> MC.Res:
  match none:
    case True{}:
      MC.Res.fail("an id is needed")
    case False{}:
      Ledger.saved(TB.Tb.remove(s, now, String.trim(J.Json.get_str(a, "id", ""))))

def Ledger.remove(+s: M.State, +now: Nat, +a: J.Json) -> MC.Res:
  Ledger.remove.k(String.is_empty(String.trim(J.Json.get_str(a, "id", ""))), s, now, a)

def Ledger.strs(+xs: List<&2, String>) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      J.Str{x} <> Ledger.strs(t)

def Ledger.imported.k(+ok: Bool, +added: Nat, +dups: Nat, +skipped: List<&2, String>, +cs: List<&2, M.Change>, +err: String) -> MC.Res:
  match ok:
    case True{}:
      Ledger.ok(J.Json.obj([J.KV{"added", J.Json.nat(added)}, J.KV{"duplicates", J.Json.nat(dups)}, J.KV{"skipped", J.Json.arr(Ledger.strs(skipped))}]), cs)
    case False{}:
      MC.Res.fail(err)

def Ledger.imported(+r: TB.TbImp) -> MC.Res:
  match r:
    case TB.TbImp{added, dups, skipped, cs, +err}:
      Ledger.imported.k(String.is_empty(err), added, dups, skipped, cs, err)

def Ledger.import.k(+none: Bool, +s: M.State, +now: Nat, +by: String, +bot: String, +a: J.Json) -> MC.Res:
  match none:
    case True{}:
      MC.Res.fail("the markdown list is needed")
    case False{}:
      Ledger.imported(TB.Tb.import(s, now, by, J.Json.get_str(a, "markdown", ""), J.Json.get_str(a, "project", ""), J.Json.get_str(Ledger.me(a, bot), "owner", "")))

def Ledger.import(+s: M.State, +now: Nat, +by: String, +bot: String, +a: J.Json) -> MC.Res:
  Ledger.import.k(String.is_empty(String.trim(J.Json.get_str(a, "markdown", ""))), s, now, by, bot, a)

# One call
# --------

def Ledger.call.k(+i: Nat, +s: M.State, +now: Nat, +thread: String, +a: J.Json) -> MC.Res:
  match i:
    case 0n:
      Ledger.check(s, now, a)
    case 1n:
      Ledger.mark(s, now, Ledger.bot(s, thread), a)
    case 2n:
      Ledger.list(s, Ledger.bot(s, thread), a)
    case 3n:
      Ledger.add(s, now, Ledger.by(s, thread), Ledger.bot(s, thread), a)
    case 4n:
      Ledger.update(s, now, Ledger.by(s, thread), Ledger.bot(s, thread), a)
    case 5n:
      Ledger.remove(s, now, a)
    case 6n:
      Ledger.import(s, now, Ledger.by(s, thread), Ledger.bot(s, thread), a)
    case _:
      MC.Res.fail("unknown tool")

# one tools/call of a ledger tool from the agent of `thread`; `p` is the
# call's params ({name, arguments})
def Ledger.call(+s: M.State, +now: Nat, +thread: String, +id: J.Json, +p: J.Json) -> MC.McpOut:
  MC.Mcp.reply(id, Ledger.call.k(C.Codec.pos(Ledger.names(), J.Json.get_str(p, "name", "")), s, now, thread, J.Json.get_obj(p, "arguments")))

# The person's requests
# ---------------------

def Ledger.methods() -> List<&2, String>:
  ["rules.set", "rules.drop", "board.set", "board.drop"]

def Ledger.method(+m: String) -> Bool:
  Nat.is_lt(C.Codec.pos(Ledger.methods(), m), 4n)

def Ledger.reply.err(+client: U32, +id: String, +msg: String) -> B.Step:
  B.Step.outs([B.Reply.err(client, id, msg)])

def Ledger.rule.set(+r: RL.RR, +client: U32, +id: String) -> B.Step:
  match r:
    case RL.RRok{rule}:
      B.Step{RL.Rules.set.of(rule), [B.Reply.ok(client, id)]}
    case RL.RRerr{msg}:
      Ledger.reply.err(client, id, msg)

def Ledger.rule.drop(+found: Bool, +id: String, +now: Nat, +client: U32, +cid: String) -> B.Step:
  match found:
    case True{}:
      B.Step{RL.Rules.drop(id, now), [B.Reply.ok(client, cid)]}
    case False{}:
      Ledger.reply.err(client, cid, "no rule " ++ id)

def Ledger.item.set(+r: TB.TbR, +client: U32, +id: String) -> B.Step:
  match r:
    case TB.TbOk{_, cs}:
      B.Step{cs, [B.Reply.ok(client, id)]}
    case TB.TbErr{msg}:
      Ledger.reply.err(client, id, msg)

def Ledger.board.set(+given: Bool, +s: M.State, +now: Nat, +item: J.Json) -> TB.TbR:
  match given:
    case True{}:
      TB.Tb.update(s, now, "person", String.trim(J.Json.get_str(item, "id", "")), item)
    case False{}:
      TB.Tb.add(s, now, "person", item)

def Ledger.some(+m: Maybe<&2, RL.Rule>) -> Bool:
  match m:
    case Some{_}:
      True{}
    case None{}:
      False{}

def Ledger.request.k(+i: Nat, +s: M.State, +now: Nat, +client: U32, +id: String, +p: J.Json) -> B.Step:
  match i:
    case 0n:
      Ledger.rule.set(RL.Rules.of.json(J.Json.get_obj(p, "rule"), s, now, "person"), client, id)
    case 1n:
      Ledger.rule.drop(Ledger.some(RL.Rules.get(s, J.Json.get_str(p, "id", ""))), J.Json.get_str(p, "id", ""), now, client, id)
    case 2n:
      Ledger.item.set(Ledger.board.set(Bool.not(String.is_empty(String.trim(J.Json.get_str(J.Json.get_obj(p, "item"), "id", "")))), s, now, J.Json.get_obj(p, "item")), client, id)
    case 3n:
      Ledger.item.set(TB.Tb.remove(s, now, String.trim(J.Json.get_str(p, "id", ""))), client, id)
    case _:
      Ledger.reply.err(client, id, "unknown request")

# a rules.* or board.* request from a client: the change it makes and its
# reply (the client's id echoed); an error comes back as the reply's error
def Ledger.request(+s: M.State, +now: Nat, +client: U32, +o: J.Json) -> B.Step:
  Ledger.request.k(C.Codec.pos(Ledger.methods(), J.Json.get_str(o, "m", "")), s, now, client, J.Json.as_raw(J.Json.get(o, "id"), "0"), J.Json.get_obj(o, "p"))
