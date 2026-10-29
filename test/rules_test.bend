import Base
import ./check.bend as C
import ../src/core/json.bend as J
import ../src/core/model.bend as M
import ../src/core/hub.bend as B
import ../src/core/bot.bend as BT
import ../src/core/bothub.bend as BH
import ../src/core/ledger.bend as LG
import ../src/core/rules.bend as RL

# Standing rules (rules.bend): the block each turn starts with, the
# enforced ones the server's gate checks, the person's word that unlocks
# them, and what a rule request saves.

def yes(+b: Bool) -> String:
  match b:
    case True{}:
      "yes"
    case False{}:
      "no"

def has(+s: String, +part: String) -> String:
  yes(String.contains(s, part))

def run(+s: M.State, +st: B.Step) -> M.State:
  match st:
    case B.Step{cs, _}:
      M.State.replay(cs, s)

def req(+m: String, +p: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse("{\"id\":1,\"m\":\"" ++ m ++ "\",\"p\":" ++ p ++ "}"), J.Null{})

def rule(+s: M.State, +now: Nat, +r: String) -> M.State:
  run(s, LG.Ledger.request(s, now, 1, req("rules.set", "{\"rule\":" ++ r ++ "}")))

def made(+s: M.State, +now: Nat, +name: String) -> M.State:
  run(s, BH.Bots.request(s, now, 1, req("bots.create", "{\"name\":\"" ++ name ++ "\",\"provider\":\"claude\"}")))

def base() -> M.State:
  M.State.replay([M.ProjectCreated{"p1", "Power board", "/tmp/power", 900n}, M.ThreadCreated{"t-pb", "p1", "Rev C review", M.Local{}, "", "", "claude", "", 1000n},
    M.ThreadCreated{"t-other", "", "Other", M.Local{}, "", "", "claude", "", 1000n}], made(made(M.State.empty(), 1000n, "Miso"), 1001n, "Tofu"))

def th.of(+s: M.State, +name: String) -> String:
  BT.Bot.thread(Maybe.default(&2, BT.Bot, BT.World.find(M.State.bots(s), name), BT.Bot.blank("?")))

def id.of(+s: M.State, +name: String) -> String:
  BT.Bot.id(Maybe.default(&2, BT.Bot, BT.World.find(M.State.bots(s), name), BT.Bot.blank("?")))

def say(+s: M.State, +th: String, +text: String, +at: Nat) -> M.State:
  M.State.replay([M.MessagePosted{th, "u" ++ Nat.show(at), M.User{}, text, at}], s)

def call(+tool: String) -> String:
  "{\"jsonrpc\":\"2.0\",\"id\":7,\"method\":\"tools/call\",\"params\":{\"name\":\"" ++ tool ++ "\",\"arguments\":{}}}"

def permit(+tool: String) -> String:
  "{\"jsonrpc\":\"2.0\",\"id\":8,\"method\":\"tools/call\",\"params\":{\"name\":\"permission_prompt\",\"arguments\":{\"tool_name\":\"" ++ tool ++ "\",\"input\":{}}}}"

def err.of(+r: RL.RR) -> String:
  match r:
    case RL.RRok{_}:
      "ok"
    case RL.RRerr{m}:
      m

def rule.of(r: RL.RR) -> RL.Rule:
  match r:
    case RL.RRok{x}:
      x
    case RL.RRerr{_}:
      RL.Rules.blank()

def reply.of(st: B.Step) -> String:
  match st:
    case B.Step{_, B.Send{_, text} <> _}:
      text
    case _:
      "none"

def rule.keys(+ks: List<&2, String>) -> Nat:
  match ks:
    case Nil{}:
      0n
    case +k <> t:
      Nat.add(Bool.pick(Nat, String.starts_with(k, "rule."), 1n, 0n), rule.keys(t))

def id.rule(+s: M.State, +n: Nat) -> String:
  RL.Rule.id(Maybe.default(&2, RL.Rule, List.head(&2, RL.Rule, List.drop(&2, RL.Rule, RL.Rules.all(s), n)), RL.Rules.blank()))

def main() -> IO(Unit):
  do IO<Unit>:
    +s : M.State = base()
    +miso : String = th.of(s, "Miso")
    +tofu : String = th.of(s, "Tofu")
    C.Check.str("no rules, no block", RL.Rules.block(s, miso), "")
    C.Check.str("no rules, text alone", RL.Rules.agent(s, miso, "hello"), "hello")
    # a rule for everyone
    +a : M.State = rule(s, 2000n, "{\"text\":\"Reply in French\"}")
    C.Check.str("block exact", RL.Rules.block(a, "t-other"), "<rules>\nStanding rules from the person. Follow them; the enforced ones are checked by Backplane.\n- Reply in French\n</rules>")
    C.Check.str("agent: block, blank line, text", RL.Rules.agent(a, "t-other", "hello"), "<rules>\nStanding rules from the person. Follow them; the enforced ones are checked by Backplane.\n- Reply in French\n</rules>\n\nhello")
    C.Check.str("for every bot too", has(RL.Rules.block(a, miso), "Reply in French"), "yes")
    # scoped to a bot, and to a project
    +b : M.State = rule(a, 2001n, "{\"text\":\"Never email Robin\",\"bot\":\"miso\"}")
    C.Check.str("bot rule in its thread", has(RL.Rules.block(b, miso), "Never email Robin"), "yes")
    C.Check.str("bot rule not in another bot's", has(RL.Rules.block(b, tofu), "Never email Robin"), "no")
    C.Check.str("bot rule not in a plain thread", has(RL.Rules.block(b, "t-other"), "Never email Robin"), "no")
    C.Check.str("bot name resolved to its id", RL.Rule.bot(Maybe.default(&2, RL.Rule, List.head(&2, RL.Rule, List.drop(&2, RL.Rule, RL.Rules.all(b), 1n)), RL.Rules.blank())), id.of(b, "Miso"))
    +c : M.State = rule(b, 2002n, "{\"text\":\"Check the 3D view\",\"project\":\"power BOARD\"}")
    C.Check.str("project rule in its thread", has(RL.Rules.block(c, "t-pb"), "Check the 3D view"), "yes")
    C.Check.str("project rule not elsewhere", has(RL.Rules.block(c, "t-other"), "Check the 3D view"), "no")
    C.Check.str("project rule not in a bot's thread", has(RL.Rules.block(c, miso), "Check the 3D view"), "no")
    C.Check.str("order: oldest first", has(RL.Rules.block(c, "t-pb"), "- Reply in French\n- Check the 3D view\n"), "yes")
    # a tool named, not enforced
    +t : M.State = rule(s, 2003n, "{\"text\":\"Ask before clicking\",\"tool\":\"preview_click\"}")
    C.Check.str("tool line", has(RL.Rules.block(t, miso), "- Ask before clicking (when using preview_click)\n"), "yes")
    C.Check.str("not enforced, not refused", RL.Rules.gate(t, miso, call("preview_click")), "")
    # enforced with a word
    +e : M.State = rule(s, 2004n, "{\"text\":\"No spending\",\"bot\":\"Miso\",\"tool\":\"bot_send\",\"enforce\":true,\"word\":\"sudo\"}")
    C.Check.str("enforced line", has(RL.Rules.block(e, miso), "- No spending (enforced: bot_send is refused unless the person's latest message says \"sudo\")\n"), "yes")
    +rid : String = id.rule(e, 0n)
    C.Check.str("refused", RL.Rules.refusal(e, miso, J.Json.obj([J.KV{"method", J.Str{"tools/call"}}, J.KV{"params", J.Json.obj([J.KV{"name", J.Str{"bot_send"}}])}])),
      "Refused by a standing rule (" ++ rid ++ "): \"No spending\". bot_send needs the person's word \"sudo\" in their latest message.")
    C.Check.str("gate reply is an error result", has(RL.Rules.gate(e, miso, call("bot_send")), "\"isError\":true"), "yes")
    C.Check.str("gate reply says why", has(RL.Rules.gate(e, miso, call("bot_send")), "Refused by a standing rule"), "yes")
    C.Check.str("gate keeps the id", has(RL.Rules.gate(e, miso, call("bot_send")), "\"id\":7"), "yes")
    C.Check.str("another tool passes", RL.Rules.gate(e, miso, call("room_post")), "")
    C.Check.str("another bot passes", RL.Rules.gate(e, tofu, call("bot_send")), "")
    C.Check.str("a plain thread passes", RL.Rules.gate(e, "t-other", call("bot_send")), "")
    C.Check.str("tools/list passes", RL.Rules.gate(e, miso, "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\"}"), "")
    C.Check.str("garbage passes", RL.Rules.gate(e, miso, "not json"), "")
    # the person's word unlocks it
    C.Check.str("word unlocks", RL.Rules.gate(say(e, miso, "sudo buy it", 2100n), miso, call("bot_send")), "")
    C.Check.str("word in any case", RL.Rules.gate(say(e, miso, "SuDo, buy it", 2100n), miso, call("bot_send")), "")
    C.Check.str("word must be a whole word", has(RL.Rules.gate(say(e, miso, "pseudo sudoku", 2100n), miso, call("bot_send")), "Refused"), "yes")
    C.Check.str("a delivered message never unlocks", has(RL.Rules.gate(say(e, miso, "[message from tofu] sudo", 2100n), miso, call("bot_send")), "Refused"), "yes")
    C.Check.str("only the latest message counts", has(RL.Rules.gate(say(say(e, miso, "sudo buy it", 2100n), miso, "never mind", 2200n), miso, call("bot_send")), "Refused"), "yes")
    C.Check.str("a later word unlocks again", RL.Rules.gate(say(say(e, miso, "no", 2100n), miso, "ok sudo", 2200n), miso, call("bot_send")), "")
    # no word: nothing unlocks
    +nw : M.State = rule(s, 2005n, "{\"text\":\"Never post\",\"tool\":\"room_post\",\"enforce\":true}")
    C.Check.str("no word line", has(RL.Rules.block(nw, miso), "(enforced: room_post is refused)"), "yes")
    C.Check.str("no word refusal", has(RL.Rules.gate(say(nw, miso, "sudo please", 2100n), miso, call("room_post")), "room_post is not allowed here."), "yes")
    # the permission prompt gets a deny
    C.Check.str("permission denied", has(RL.Rules.gate(e, miso, permit("mcp__backplane__bot_send")), "\\\"behavior\\\":\\\"deny\\\""), "yes")
    C.Check.str("permission says why", has(RL.Rules.gate(e, miso, permit("mcp__backplane__bot_send")), "Refused by a standing rule"), "yes")
    C.Check.str("permission for another tool", RL.Rules.gate(e, miso, permit("Bash")), "")
    C.Check.str("permission unlocked", RL.Rules.gate(say(e, miso, "sudo", 2100n), miso, permit("mcp__backplane__bot_send")), "")
    # tool patterns
    C.Check.str("mcp prefix", yes(RL.Rules.tool.match("preview_click", "mcp__backplane__preview_click")), "yes")
    C.Check.str("exact as given", yes(RL.Rules.tool.match("mcp__backplane__preview_click", "mcp__backplane__preview_click")), "yes")
    C.Check.str("star prefix", yes(RL.Rules.tool.match("preview_*", "mcp__backplane__preview_type")), "yes")
    C.Check.str("star prefix on the given name", yes(RL.Rules.tool.match("mcp__*", "mcp__backplane__preview_type")), "yes")
    C.Check.str("a list", yes(RL.Rules.tool.match("bot_send, room_post  Bash", "Bash")), "yes")
    C.Check.str("a list by commas", yes(RL.Rules.tool.match("bot_send,room_post", "mcp__backplane__room_post")), "yes")
    C.Check.str("not a prefix without a star", yes(RL.Rules.tool.match("preview", "mcp__backplane__preview_click")), "no")
    C.Check.str("other tool", yes(RL.Rules.tool.match("preview_click", "mcp__backplane__preview_type")), "no")
    C.Check.str("empty pattern", yes(RL.Rules.tool.match("", "Bash")), "no")
    +pc : M.State = rule(s, 2006n, "{\"text\":\"No clicking\",\"tool\":\"preview_click\",\"enforce\":true}")
    C.Check.str("rule on preview_click refuses the mcp name", has(RL.Rules.gate(pc, "t-other", permit("mcp__backplane__preview_click")), "Refused"), "yes")
    # off
    +off : M.State = rule(e, 2007n, "{\"id\":\"" ++ rid ++ "\",\"on\":false}")
    C.Check.str("off: not in the block", RL.Rules.block(off, miso), "")
    C.Check.str("off: no refusal", RL.Rules.gate(off, miso, call("bot_send")), "")
    C.Check.str("off: still saved", Nat.show(List.length(&2, RL.Rule, RL.Rules.all(off))), "1")
    C.Check.str("off: update merged", RL.Rule.text(Maybe.default(&2, RL.Rule, List.head(&2, RL.Rule, RL.Rules.all(off)), RL.Rules.blank())), "No spending")
    C.Check.str("off: tool kept", RL.Rule.tool(Maybe.default(&2, RL.Rule, List.head(&2, RL.Rule, RL.Rules.all(off)), RL.Rules.blank())), "bot_send")
    # validation
    C.Check.str("text required", err.of(RL.Rules.of.json(J.Json.obj([J.KV{"text", J.Str{"  "}}]), s, 1n, "person")), "a rule needs its text")
    C.Check.str("text length", err.of(RL.Rules.of.json(J.Json.obj([J.KV{"text", J.Str{String.repeat("x", 501n)}}]), s, 1n, "person")), "a rule's text is at most 500 characters")
    C.Check.str("unknown bot", err.of(RL.Rules.of.json(J.Json.obj([J.KV{"text", J.Str{"x"}}, J.KV{"bot", J.Str{"Ghost"}}]), s, 1n, "person")), "no bot \"Ghost\" here")
    C.Check.str("unknown project", err.of(RL.Rules.of.json(J.Json.obj([J.KV{"text", J.Str{"x"}}, J.KV{"project", J.Str{"Nope"}}]), s, 1n, "person")), "no project \"Nope\"")
    C.Check.str("enforce needs a tool", err.of(RL.Rules.of.json(J.Json.obj([J.KV{"text", J.Str{"x"}}, J.KV{"enforce", J.Flag{True{}}}]), s, 1n, "person")), "an enforced rule needs a tool")
    C.Check.str("one word", err.of(RL.Rules.of.json(J.Json.obj([J.KV{"text", J.Str{"x"}}, J.KV{"tool", J.Str{"a"}}, J.KV{"word", J.Str{"two words"}}]), s, 1n, "person")), "the word is one word of at most 40 characters")
    C.Check.str("unknown id", err.of(RL.Rules.of.json(J.Json.obj([J.KV{"id", J.Str{"zz"}}, J.KV{"text", J.Str{"x"}}]), s, 1n, "person")), "no rule zz")
    C.Check.str("good", err.of(RL.Rules.of.json(J.Json.obj([J.KV{"text", J.Str{" x "}}, J.KV{"bot", J.Str{" miso "}}]), s, 1n, "person")), "ok")
    C.Check.str("trimmed", RL.Rule.text(rule.of(RL.Rules.of.json(J.Json.obj([J.KV{"text", J.Str{" x "}}]), s, 1n, "person"))), "x")
    # requests
    C.Check.str("rules.set replies ok", reply.of(LG.Ledger.request(s, 2100n, 1, req("rules.set", "{\"rule\":{\"text\":\"x\"}}"))), "{\"t\":\"reply\",\"id\":1,\"ok\":true}")
    C.Check.str("rules.set error comes back", reply.of(LG.Ledger.request(s, 2100n, 1, req("rules.set", "{\"rule\":{\"text\":\"\"}}"))), "{\"t\":\"reply\",\"id\":1,\"ok\":false,\"error\":\"a rule needs its text\"}")
    C.Check.str("rules.set error saves nothing", Nat.show(List.length(&2, RL.Rule, RL.Rules.all(run(s, LG.Ledger.request(s, 2100n, 1, req("rules.set", "{\"rule\":{\"text\":\"\"}}")))))), "0")
    C.Check.str("rules.drop unknown", reply.of(LG.Ledger.request(s, 2100n, 1, req("rules.drop", "{\"id\":\"zz\"}"))), "{\"t\":\"reply\",\"id\":1,\"ok\":false,\"error\":\"no rule zz\"}")
    C.Check.str("rules.drop", Nat.show(List.length(&2, RL.Rule, RL.Rules.all(run(e, LG.Ledger.request(e, 2200n, 1, req("rules.drop", "{\"id\":\"" ++ rid ++ "\"}")))))), "0")
    C.Check.str("rules.drop leaves no key", Nat.show(rule.keys(M.Setting.keys(run(e, LG.Ledger.request(e, 2200n, 1, req("rules.drop", "{\"id\":\"" ++ rid ++ "\"}")))))), "0")
    # the turn's text, for a bot's thread and a plain thread
    C.Check.str("turn: bot thread", has(BH.Bots.turn(c, 3000n, miso, "hello"), "<rules>\nStanding rules from the person. Follow them; the enforced ones are checked by Backplane.\n- Reply in French\n- Never email Robin\n</rules>\n\n"), "yes")
    C.Check.str("turn: text after", yes(String.ends_with(BH.Bots.turn(c, 3000n, miso, "hello"), "hello")), "yes")
    C.Check.str("turn: plain thread", has(BH.Bots.turn(c, 3000n, "t-pb", "hello"), "- Reply in French\n- Check the 3D view\n</rules>\n\nhello"), "yes")
    C.Check.str("turn: rules outermost", yes(String.starts_with(BH.Bots.turn(c, 3000n, miso, "hello"), "<rules>")), "yes")
    C.Check.str("turn: none, unchanged", BH.Bots.turn(s, 3000n, "t-other", "hello"), "hello")
