import Base
import ./json.bend as J
import ./model.bend as M
import ./bot.bend as BT
import ./ruledata.bend as RD
import ./hub.bend as B
import ./mcp.bend as MC

# Standing rules. A rule is the setting "rule.<id>" holding
# {"id","text","bot","project","tool","enforce","word","on","at","by"}:
# what the person wants always done (or never done), for every thread, one
# bot's threads, or one project's. Every turn's text starts with the
# <rules> block of the ones that apply (Rules.agent, called from bothub's
# Bots.turn). A rule with a tool pattern and `enforce` is also checked:
# Backplane refuses the agent's call of a matching tool (its own MCP tools
# and anything that asks permission; a provider's built-in tools in a
# bypass mode never reach the hub) unless the person's latest message says
# the rule's word (Rules.refusal, answered by the server's MCP gate,
# Rules.gate). Settings, so they stay on this machine.

type Rule is Data:
  Rule{id: String, text: String, bot: String, project: String, tool: String, enforce: Bool, word: String, on: Bool, at: Nat, by: String}

def Rule.data(r: Rule) -> RD.Rule:
  match r:
    case Rule{id, text, bot, project, tool, enforce, word, on, at, by}:
      RD.Rule{id, text, bot, project, tool, enforce, word, on, at, by}

def Rule.here(r: RD.Rule) -> Rule:
  match r:
    case RD.Rule{id, text, bot, project, tool, enforce, word, on, at, by}:
      Rule{id, text, bot, project, tool, enforce, word, on, at, by}

def Rule.datas(rs: List<&2, Rule>) -> List<&2, RD.Rule>:
  match rs:
    case Nil{}:
      Nil{}
    case r <> t:
      Rule.data(r) <> Rule.datas(t)

def Rule.data_m(r: Maybe<&2, Rule>) -> Maybe<&2, RD.Rule>:
  match r:
    case Some{x}:
      Some{Rule.data(x)}
    case None{}:
      None{}

def Rule.heres(rs: List<&2, RD.Rule>) -> List<&2, Rule>:
  match rs:
    case Nil{}:
      Nil{}
    case r <> t:
      Rule.here(r) <> Rule.heres(t)

def Rule.here_m(r: Maybe<&2, RD.Rule>) -> Maybe<&2, Rule>:
  match r:
    case Some{x}:
      Some{Rule.here(x)}
    case None{}:
      None{}

def Rule.id(+r: Rule) -> String:
  RD.Rule.id(Rule.data(r))

def Rule.text(+r: Rule) -> String:
  RD.Rule.text(Rule.data(r))

def Rule.bot(+r: Rule) -> String:
  RD.Rule.bot(Rule.data(r))

def Rule.project(+r: Rule) -> String:
  RD.Rule.project(Rule.data(r))

def Rule.tool(+r: Rule) -> String:
  RD.Rule.tool(Rule.data(r))

def Rule.enforce(+r: Rule) -> Bool:
  RD.Rule.enforce(Rule.data(r))

def Rule.word(+r: Rule) -> String:
  RD.Rule.word(Rule.data(r))

def Rule.on(+r: Rule) -> Bool:
  RD.Rule.on(Rule.data(r))

def Rule.at(+r: Rule) -> Nat:
  RD.Rule.at(Rule.data(r))

def Rule.by(+r: Rule) -> String:
  RD.Rule.by(Rule.data(r))

def Rules.blank() -> Rule:
  Rule.here(RD.Rules.blank())

def Rules.json(+r: Rule) -> J.Json:
  RD.Rules.json(Rule.data(r))

def Rules.value(+r: Rule) -> String:
  RD.Rules.value(Rule.data(r))

def Rules.of.stored(+o: J.Json) -> Rule:
  Rule.here(RD.Rules.of.stored(o))

def Rules.is(+k: String) -> Bool:
  RD.Rules.is(k)

def Rules.with.id(+r: Rule, +id: String) -> Rule:
  Rule.here(RD.Rules.with.id(Rule.data(r), id))

def Rules.read.id(+id: String, +r: Rule, +key: String) -> Rule:
  Rule.here(RD.Rules.read.id(id, Rule.data(r), key))

def Rules.read.one(+m: Maybe<&2, J.Json>, +k: String, +acc: List<&2, Rule>) -> List<&2, Rule>:
  Rule.heres(RD.Rules.read.one(m, k, Rule.datas(acc)))

def Rules.read.put(+hit: Bool, +k: String, +s: M.State, +acc: List<&2, Rule>) -> List<&2, Rule>:
  Rule.heres(RD.Rules.read.put(hit, k, s, Rule.datas(acc)))

def Rules.read(+ks: List<&2, String>, +s: M.State, +acc: List<&2, Rule>) -> List<&2, Rule>:
  Rule.heres(RD.Rules.read(ks, s, Rule.datas(acc)))

def Rules.older.k(+aa: Nat, +ab: Nat, +ia: String, +ib: String) -> Bool:
  RD.Rules.older.k(aa, ab, ia, ib)

def Rules.older(a: Rule, b: Rule) -> Bool:
  RD.Rules.older(Rule.data(a), Rule.data(b))

def Rules.all(+s: M.State) -> List<&2, Rule>:
  Rule.heres(RD.Rules.all(s))

def Rules.get.go(+rs: List<&2, Rule>, +id: String) -> Maybe<&2, Rule>:
  Rule.here_m(RD.Rules.get.go(Rule.datas(rs), id))

def Rules.get(+s: M.State, +id: String) -> Maybe<&2, Rule>:
  Rule.here_m(RD.Rules.get(s, id))

# Which threads
# -------------

def Thread.project(+t: M.Thread) -> String:
  match t:
    case M.Thread{_, p, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      p

def Rules.bot.of(+m: Maybe<&2, BT.Bot>) -> String:
  match m:
    case Some{b}:
      BT.Bot.id(b)
    case None{}:
      ""

# the bot a thread belongs to, "" for a thread that is no bot's
def Rules.bot.thread(+s: M.State, +thread: String) -> String:
  Rules.bot.of(BT.World.of_thread(M.State.bots(s), thread))

def Rules.applies.k(+on: Bool, +bot: String, +project: String, +r: Rule) -> Bool:
  Bool.and(on, Bool.and(Bool.or(String.is_empty(Rule.bot(r)), String.eq(Rule.bot(r), bot)),
    Bool.or(String.is_empty(Rule.project(r)), String.eq(Rule.project(r), project))))

# the rule is on, and it is for every bot or this thread's, and for every
# project or this thread's
def Rules.applies(+s: M.State, +thread: String, +r: Rule) -> Bool:
  Rules.applies.k(Rule.on(r), Rules.bot.thread(s, thread), Thread.project(M.State.thread.get(s, thread)), r)

def Rules.keep(+ok: Bool, +r: Rule, +rest: List<&2, Rule>) -> List<&2, Rule>:
  match ok:
    case True{}:
      r <> rest
    case False{}:
      rest

def Rules.for.go(+rs: List<&2, Rule>, +s: M.State, +thread: String) -> List<&2, Rule>:
  match rs:
    case Nil{}:
      Nil{}
    case +r <> t:
      Rules.keep(Rules.applies(s, thread, r), r, Rules.for.go(t, s, thread))

# the rules that apply to a thread, oldest first
def Rules.for(+s: M.State, +thread: String) -> List<&2, Rule>:
  Rules.for.go(Rules.all(s), s, thread)

# Tool patterns
# -------------

# the part of a tool name after its last "__" (an MCP name is
# mcp__<server>__<tool>): the characters since the last pair of underscores
type TN is Data:
  TN{cur: String, under: Bool}

def Rules.strip.step(+under: Bool, +c: U32, +cur: String) -> TN:
  match under:
    case True{}:
      Bool.pick(TN, U32.is_eq(c, 95), TN{SNil{}, False{}}, TN{SCon{Chr{c}, cur}, False{}})
    case False{}:
      TN{SCon{Chr{c}, cur}, U32.is_eq(c, 95)}

def Rules.strip.next(+c: U32, +st: TN) -> TN:
  match st:
    case TN{cur, under}:
      Rules.strip.step(under, c, cur)

def Rules.strip.go(+s: String, +st: TN) -> TN:
  match s:
    case SNil{}:
      st
    case SCon{Chr{+c}, t}:
      Rules.strip.go(t, Rules.strip.next(c, st))

def Rules.strip.out(+st: TN) -> String:
  match st:
    case TN{cur, _}:
      String.reverse(cur)

def Rules.strip(+s: String) -> String:
  Rules.strip.out(Rules.strip.go(s, TN{SNil{}, False{}}))

def Rules.pat.star(+p: String, +name: String) -> Bool:
  Bool.and(String.ends_with(p, "*"), String.starts_with(name, String.take(p, Nat.sub(String.length(p), 1n))))

# one pattern: an exact name, or a prefix ending in "*"
def Rules.pat(+p: String, +name: String) -> Bool:
  Bool.or(String.eq(p, name), Rules.pat.star(p, name))

def Rules.sep(+c: U32) -> Bool:
  Bool.or(Bool.or(U32.is_eq(c, 32), U32.is_eq(c, 44)), Bool.or(U32.is_eq(c, 9), U32.is_eq(c, 10)))

type PS is Data:
  PS{cur: String, acc: List<&2, String>}

def Rules.pats.push(+cur: String, +acc: List<&2, String>) -> List<&2, String>:
  match cur:
    case SNil{}:
      acc
    case SCon{_, _}:
      String.reverse(cur) <> acc

def Rules.pats.step(+sep: Bool, +c: U32, +st: PS) -> PS:
  match sep st:
    case True{} PS{cur, acc}:
      PS{SNil{}, Rules.pats.push(cur, acc)}
    case False{} PS{cur, acc}:
      PS{SCon{Chr{c}, cur}, acc}

def Rules.pats.go(+s: String, +st: PS) -> PS:
  match s:
    case SNil{}:
      st
    case SCon{Chr{+c}, t}:
      Rules.pats.go(t, Rules.pats.step(Rules.sep(c), c, st))

def Rules.pats.out(+st: PS) -> List<&2, String>:
  match st:
    case PS{cur, acc}:
      List.reverse(&2, String, Rules.pats.push(cur, acc))

# the patterns in a rule's tool field: separated by commas and/or spaces
def Rules.pats(+s: String) -> List<&2, String>:
  Rules.pats.out(Rules.pats.go(s, PS{SNil{}, Nil{}}))

def Rules.pats.any(+ps: List<&2, String>, +name: String, +bare: String) -> Bool:
  match ps:
    case Nil{}:
      False{}
    case +p <> t:
      Bool.or(Bool.or(Rules.pat(p, name), Rules.pat(p, bare)), Rules.pats.any(t, name, bare))

# whether a tool name is named by the rule's patterns: the name as given,
# or without its mcp__<server>__ prefix
def Rules.tool.match(+pattern: String, +name: String) -> Bool:
  Rules.pats.any(Rules.pats(pattern), name, Rules.strip(name))

# The block
# ---------

def Rules.line.tool(+enforce: Bool, +tool: String, +word: String) -> String:
  match enforce:
    case True{}:
      Bool.pick(String, String.is_empty(word), " (enforced: " ++ tool ++ " is refused)",
        " (enforced: " ++ tool ++ " is refused unless the person's latest message says \"" ++ word ++ "\")")
    case False{}:
      " (when using " ++ tool ++ ")"

def Rules.line.k(+none: Bool, +enforce: Bool, +tool: String, +word: String) -> String:
  match none:
    case True{}:
      ""
    case False{}:
      Rules.line.tool(enforce, tool, word)

def Rules.line(+r: Rule) -> String:
  match r:
    case Rule{_, +x, _, _, +t, e, w, _, _, _}:
      "- " ++ x ++ Rules.line.k(String.is_empty(t), e, t, w) ++ "\n"

def Rules.lines(+rs: List<&2, Rule>) -> String:
  match rs:
    case Nil{}:
      ""
    case r <> t:
      Rules.line(r) ++ Rules.lines(t)

def Rules.block.k(+rs: List<&2, Rule>) -> String:
  match rs:
    case Nil{}:
      ""
    case _ <> _:
      "<rules>\nStanding rules from the person. Follow them; the enforced ones are checked by Backplane.\n" ++ Rules.lines(rs) ++ "</rules>"

# The <rules> block of the rules that apply to a thread ("" when none)
def Rules.block(+s: M.State, +thread: String) -> String:
  Rules.block.k(Rules.for(s, thread))

def Rules.agent.k(+block: String, +text: String) -> String:
  Bool.pick(String, String.is_empty(block), text, block ++ "\n\n" ++ text)

# a turn's text with the thread's rules before it
def Rules.agent(+s: M.State, +thread: String, +text: String) -> String:
  Rules.agent.k(Rules.block(s, thread), text)

# The person's word
# -----------------

# the text of the thread's newest user message, as the person's word: one
# another thread sent (thread_send, a launch report: its id is a link,
# M.Link.id) reads as "[" so it never unlocks anything; a message this
# thread sent elsewhere (filed here too) is skipped
def Rules.last.of(+id: String, +text: String) -> String:
  Bool.pick(String, String.is_empty(M.Link.from(id)), text, "[from " ++ M.Link.from(id) ++ "]")

def Rules.last.pick(+mine: Bool, +id: String, +text: String, +acc: String) -> String:
  match mine:
    case True{}:
      Rules.last.of(id, text)
    case False{}:
      acc

# the walk (newest first) stops at the thread's own newest user message
def Rules.last.go(+es: List<&2, M.Entry>, +thread: String, +done: Bool, +acc: String) -> String:
  match es done:
    case _ True{}:
      acc
    case Nil{} False{}:
      acc
    case M.Msg{+id, +th, M.User{}, +text, _} <> t False{}:
      Rules.last.go(t, thread, String.eq(th, thread), Rules.last.pick(String.eq(th, thread), id, text, acc))
    case _ <> t False{}:
      Rules.last.go(t, thread, False{}, acc)

def Rules.person.last(+s: M.State, +thread: String) -> String:
  Rules.last.go(M.State.mine(s, thread), thread, False{}, "")

def Rules.alnum(+c: U32) -> Bool:
  Bool.or(Bool.or(Bool.and(U32.is_ge(c, 97), U32.is_le(c, 122)), Bool.and(U32.is_ge(c, 65), U32.is_le(c, 90))), Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57)))

def Rules.low(+c: U32) -> U32:
  Bool.pick(U32, Bool.and(U32.is_ge(c, 65), U32.is_le(c, 90)), (c + 32 : U32), c)

# the word is at the start of the text, in any case, and a letter or
# digit does not follow it
def Rules.word.at(+w: String, +r: String, +ok: Bool) -> Bool:
  match w r ok:
    case _ _ False{}:
      False{}
    case SNil{} SNil{} True{}:
      True{}
    case SNil{} SCon{Chr{+c}, +rt0} True{}:
      Bool.not(Rules.alnum(c))
    case SCon{+a0, +b0} SNil{} True{}:
      False{}
    case SCon{Chr{+a}, wt} SCon{Chr{+b}, rt} True{}:
      Rules.word.at(wt, rt, U32.is_eq(Rules.low(a), Rules.low(b)))

def Rules.word.hit(+prev: Bool, +w: String, +r: String) -> Bool:
  Bool.and(Bool.not(prev), Rules.word.at(w, r, True{}))

def Rules.word.go(+r: String, +hit: Bool, +w: String) -> Bool:
  match r hit:
    case _ True{}:
      True{}
    case SNil{} False{}:
      False{}
    case SCon{Chr{+c}, +t} False{}:
      Rules.word.go(t, Rules.word.hit(Rules.alnum(c), w, t), w)

# the word is in the text as a whole word (letters and digits on neither
# side), in any case
def Rules.word.in(+word: String, +text: String) -> Bool:
  Bool.and(Bool.not(String.is_empty(word)), Rules.word.go(text, Rules.word.hit(False{}, word, text), word))

# Refusing
# --------

# the person's latest message can unlock a rule only when it is theirs:
# a message delivered from a room, a bot, a routine, a webhook or a space
# starts with "["
def Rules.unlocked.k(+has: Bool, +from: Bool, +word: String, +last: String) -> Bool:
  match has from:
    case True{} False{}:
      Rules.word.in(word, last)
    case _ _:
      False{}

def Rules.unlocked(+r: Rule, +last: String) -> Bool:
  Rules.unlocked.k(Bool.not(String.is_empty(Rule.word(r))), String.starts_with(String.trim(last), "["), Rule.word(r), last)

def Rules.refuses(+r: Rule, +tool: String, +last: String) -> Bool:
  Bool.and(Bool.and(Rule.enforce(r), Bool.not(String.is_empty(Rule.tool(r)))), Bool.and(Rules.tool.match(Rule.tool(r), tool), Bool.not(Rules.unlocked(r, last))))

def Rules.say.k(+none: Bool, +tool: String, +word: String) -> String:
  match none:
    case True{}:
      tool ++ " is not allowed here."
    case False{}:
      tool ++ " needs the person's word \"" ++ word ++ "\" in their latest message."

def Rules.say(+r: Rule, +tool: String) -> String:
  match r:
    case Rule{+i, +x, _, _, _, _, +w, _, _, _}:
      "Refused by a standing rule (" ++ i ++ "): \"" ++ x ++ "\". " ++ Rules.say.k(String.is_empty(w), tool, w)

def Rules.refusal.pick(+hit: Bool, +r: Rule, +tool: String, +rest: String) -> String:
  match hit:
    case True{}:
      Rules.say(r, tool)
    case False{}:
      rest

def Rules.refusal.go(+rs: List<&2, Rule>, +tool: String, +last: String) -> String:
  match rs:
    case Nil{}:
      ""
    case +r <> t:
      Rules.refusal.pick(Rules.refuses(r, tool, last), r, tool, Rules.refusal.go(t, tool, last))

# the tool a request calls: tools/call's name, or for the permission
# prompt the tool it asks about; "" for any other message
def Rules.tool.of.k(+call: Bool, +o: J.Json) -> String:
  match call:
    case True{}:
      Bool.pick(String, String.eq(J.Json.get_str(J.Json.get_obj(o, "params"), "name", ""), "permission_prompt"),
        J.Json.get_str(J.Json.get_obj(J.Json.get_obj(o, "params"), "arguments"), "tool_name", ""), J.Json.get_str(J.Json.get_obj(o, "params"), "name", ""))
    case False{}:
      ""

def Rules.tool.of(+o: J.Json) -> String:
  Rules.tool.of.k(String.eq(J.Json.get_str(o, "method", ""), "tools/call"), o)

def Rules.refusal.t(+none: Bool, +tool: String, +s: M.State, +thread: String) -> String:
  match none:
    case True{}:
      ""
    case False{}:
      Rules.refusal.go(Rules.for(s, thread), tool, Rules.person.last(s, thread))

def Rules.refusal.k(+tool: String, +s: M.State, +thread: String) -> String:
  Rules.refusal.t(String.is_empty(tool), tool, s, thread)

# "" when the request is allowed, else why not: an enforced rule that
# applies to this thread and names the tool refuses it unless the person's
# latest message says the rule's word
def Rules.refusal(+s: M.State, +thread: String, +o: J.Json) -> String:
  Rules.refusal.k(Rules.tool.of(o), s, thread)

def Rules.gate.reply(+perm: Bool, +id: J.Json, +msg: String) -> String:
  match perm:
    case True{}:
      MC.Mcp.result(id, MC.Perm.text(MC.Perm.deny(msg)))
    case False{}:
      MC.McpOut.reply(MC.Mcp.reply(id, MC.Res.fail(msg)))

def Rules.gate.m(+none: Bool, +msg: String, +o: J.Json) -> String:
  match none:
    case True{}:
      ""
    case False{}:
      Rules.gate.reply(String.eq(J.Json.get_str(J.Json.get_obj(o, "params"), "name", ""), "permission_prompt"), J.Json.get_obj(o, "id"), msg)

def Rules.gate.k(+msg: String, +o: J.Json) -> String:
  Rules.gate.m(String.is_empty(msg), msg, o)

def Rules.gate.o(+m: Maybe<&2, J.Json>, +s: M.State, +thread: String) -> String:
  match m:
    case Some{+o}:
      Rules.gate.k(Rules.refusal(s, thread, o), o)
    case None{}:
      ""

# the server's gate on an agent's MCP message: "" lets it through, else the
# full JSON-RPC reply that refuses it (an error result for a tool call, a
# deny for the permission prompt)
def Rules.gate(+s: M.State, +thread: String, +body: String) -> String:
  Rules.gate.o(J.Json.parse(body), s, thread)

# Saving
# ------

# a saved rule, or why not
type RR is Data:
  RRok{rule: Rule}
  RRerr{msg: String}

# a name or id resolved: the id, or the error
type RS is Data:
  RS{v: String, err: String}

def Rules.proj.live(+p: M.Project) -> Bool:
  match p:
    case M.Project{_, _, _, _, rm}:
      Bool.not(rm)

def Rules.proj.hit(+p: M.Project, +name: String) -> Bool:
  match p:
    case M.Project{+i, +t, _, _, rm}:
      Bool.and(Bool.not(rm), Bool.or(String.eq(i, name), String.eq(String.to_lower(t), String.to_lower(name))))

def Rules.proj.go(+ps: List<&2, M.Project>, +name: String) -> Maybe<&2, M.Project>:
  match ps:
    case Nil{}:
      None{}
    case +p <> t:
      BT.Pick.first(M.Project, Rules.proj.hit(p, name), p, Rules.proj.go(t, name))

# a live project by id, or by name in any case
def Rules.proj.find(+s: M.State, +name: String) -> Maybe<&2, M.Project>:
  Rules.proj.go(B.Hub.projects(s), name)

def Rules.proj.res.k(+m: Maybe<&2, M.Project>, +name: String) -> RS:
  match m:
    case Some{M.Project{i, _, _, _, _}}:
      RS{i, ""}
    case None{}:
      RS{"", "no project \"" ++ name ++ "\""}

def Rules.proj.res.e(+none: Bool, +s: M.State, +name: String) -> RS:
  match none:
    case True{}:
      RS{"", ""}
    case False{}:
      Rules.proj.res.k(Rules.proj.find(s, name), name)

def Rules.proj.res(+s: M.State, +name: String) -> RS:
  Rules.proj.res.e(String.is_empty(name), s, name)

def Rules.bot.res.k(+m: Maybe<&2, BT.Bot>, +name: String) -> RS:
  match m:
    case Some{b}:
      RS{BT.Bot.id(b), ""}
    case None{}:
      RS{"", "no bot \"" ++ name ++ "\" here"}

def Rules.bot.res.e(+none: Bool, +s: M.State, +name: String) -> RS:
  match none:
    case True{}:
      RS{"", ""}
    case False{}:
      Rules.bot.res.k(BT.World.find(M.State.bots(s), name), name)

# a bot given by name (any case) or id, resolved to its id
def Rules.bot.res(+s: M.State, +name: String) -> RS:
  Rules.bot.res.e(String.is_empty(name), s, name)

def Rules.err.first(+es: List<&2, String>) -> String:
  match es:
    case Nil{}:
      ""
    case +e <> t:
      Bool.pick(String, String.is_empty(e), Rules.err.first(t), e)

def RS.v(+r: RS) -> String:
  match r:
    case RS{v, _}:
      v

def RS.err(+r: RS) -> String:
  match r:
    case RS{_, e}:
      e

def Rules.text.err(+t: String) -> String:
  Bool.pick(String, String.is_empty(t), "a rule needs its text",
    Bool.pick(String, Nat.is_gt(String.length(t), 500n), "a rule's text is at most 500 characters", ""))

def Rules.enforce.err(+enforce: Bool, +tool: String) -> String:
  Bool.pick(String, Bool.and(enforce, String.is_empty(tool)), "an enforced rule needs a tool", "")

def Rules.word.err(+w: String) -> String:
  Bool.pick(String, Bool.or(String.contains(w, " "), Bool.or(String.contains(w, ","), Nat.is_gt(String.length(w), 40n))), "the word is one word of at most 40 characters", "")

def Rules.build(+err: String, +r: Rule) -> RR:
  match err:
    case SNil{}:
      RRok{r}
    case SCon{_, _}:
      RRerr{err}

def Rules.of.fields(+id: String, +text: String, +bot: RS, +project: RS, +tool: String, +enforce: Bool, +word: String, +on: Bool, +now: Nat, +by: String) -> RR:
  Rules.build(Rules.err.first([Rules.text.err(text), RS.err(bot), RS.err(project), Rules.enforce.err(enforce, tool), Rules.word.err(word)]),
    Rule{id, text, RS.v(bot), RS.v(project), tool, enforce, word, on, now, by})

def Rules.of.old(+s: M.State, +o: J.Json, +old: Rule, +now: Nat, +by: String) -> RR:
  Rules.of.fields(Rule.id(old), String.trim(J.Json.get_str(o, "text", Rule.text(old))), Rules.bot.res(s, String.trim(J.Json.get_str(o, "bot", Rule.bot(old)))),
    Rules.proj.res(s, String.trim(J.Json.get_str(o, "project", Rule.project(old)))), String.trim(J.Json.get_str(o, "tool", Rule.tool(old))),
    J.Json.get_bool(o, "enforce", Rule.enforce(old)), String.trim(J.Json.get_str(o, "word", Rule.word(old))), J.Json.get_bool(o, "on", Rule.on(old)), now, by)

def Rules.of.known(+m: Maybe<&2, Rule>, +s: M.State, +o: J.Json, +now: Nat, +by: String, +id: String) -> RR:
  match m:
    case Some{old}:
      Rules.of.old(s, o, old, now, by)
    case None{}:
      RRerr{"no rule " ++ id}

def Rules.of.new(+s: M.State, +o: J.Json, +now: Nat, +by: String) -> RR:
  Rules.of.old(s, o, Rules.with.id(Rules.blank(), B.Hub.id("r", now, B.Hub.seq(s))), now, by)

def Rules.of.id(+given: Bool, +s: M.State, +o: J.Json, +now: Nat, +by: String) -> RR:
  match given:
    case True{}:
      Rules.of.known(Rules.get(s, J.Json.get_str(o, "id", "")), s, o, now, by, J.Json.get_str(o, "id", ""))
    case False{}:
      Rules.of.new(s, o, now, by)

# A rule from a request's JSON, or why not. With an "id" it changes that
# rule (the fields given replace the old ones); without, it makes one. The
# text is required, trimmed everything, `bot` (an id or a name here) and
# `project` (an id or a name) resolved, `enforce` needs a tool.
def Rules.of.json(+o: J.Json, +s: M.State, +now: Nat, +by: String) -> RR:
  Rules.of.id(Bool.not(String.is_empty(String.trim(J.Json.get_str(o, "id", "")))), s, o, now, by)

def Rules.set.of(+r: Rule) -> List<&2, M.Change>:
  [M.SettingSet{"rule." ++ Rule.id(r), Rules.value(r), Rule.at(r)}]

def Rules.drop(+id: String, +now: Nat) -> List<&2, M.Change>:
  [M.SettingSet{"rule." ++ id, "", now}]

# Examples the laws read
# ----------------------

# Miso's thread t-miso, Tofu's t-tofu, t-pb in project p1, t-other in none;
# `rules` are saved settings (their ids are r1, r2...)
def Rules.demo.base() -> M.State:
  M.State.replay([M.ProjectCreated{"p1", "Power board", "/tmp/power", 1n}, M.ThreadCreated{"t-miso", "", "Miso", M.Local{}, "", "", "claude", "", 1n},
    M.BotSet{"b-miso", "Miso", "t-miso", 0n, "", 1n}, M.ThreadCreated{"t-tofu", "", "Tofu", M.Local{}, "", "", "claude", "", 1n}, M.BotSet{"b-tofu", "Tofu", "t-tofu", 0n, "", 1n},
    M.ThreadCreated{"t-pb", "p1", "Rev C", M.Local{}, "", "", "claude", "", 1n}, M.ThreadCreated{"t-other", "", "Other", M.Local{}, "", "", "claude", "", 1n}], M.State.empty())

def Rules.demo.put(+s: M.State, +r: Rule) -> M.State:
  M.State.replay(Rules.set.of(r), s)

# Miso's rule, and one for project p1
def Rules.demo.scoped() -> M.State:
  Rules.demo.put(Rules.demo.put(Rules.demo.base(), Rule{"r1", "Miso only", "b-miso", "", "", False{}, "", True{}, 10n, "person"}),
    Rule{"r2", "Power only", "", "p1", "", False{}, "", True{}, 11n, "person"})

# Miso may not bot_send unless the person says sudo
def Rules.demo.enforced() -> M.State:
  Rules.demo.put(Rules.demo.base(), Rule{"r1", "No spending", "b-miso", "", "bot_send", True{}, "sudo", True{}, 10n, "person"})

# the same rule, turned off
def Rules.demo.off() -> M.State:
  Rules.demo.put(Rules.demo.base(), Rule{"r1", "No spending", "b-miso", "", "bot_send", True{}, "sudo", False{}, 10n, "person"})

def Rules.demo.say(+s: M.State, +thread: String, +text: String) -> M.State:
  M.State.replay([M.MessagePosted{thread, "u9", M.User{}, text, 20n}], s)

# a tools/call of `tool`
def Rules.demo.call(+tool: String) -> String:
  "{\"jsonrpc\":\"2.0\",\"id\":7,\"method\":\"tools/call\",\"params\":{\"name\":\"" ++ tool ++ "\",\"arguments\":{}}}"

# a user message under a given id (a link id is another thread's)
def Rules.demo.say.id(+s: M.State, +thread: String, +id: String, +text: String) -> M.State:
  M.State.replay([M.MessagePosted{thread, id, M.User{}, text, 20n}], s)

