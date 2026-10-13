import Base
import ./json.bend as J
import ./model.bend as M
import ./bot.bend as BT

# Standing rules' data: a rule (the setting "rule.<id>"), reading the rules
# from the settings, their order, the words for a rule's scope, and the
# splitting of a pattern list. Pure and light, so the clients can import it
# without the hub (rules.bend keeps the checks, the block, the gate and the
# saving, and wraps these under the same names).

type Rule is Data:
  Rule{id: String, text: String, bot: String, project: String, tool: String, enforce: Bool, word: String, on: Bool, at: Nat, by: String}

def Rule.id(+r: Rule) -> String:
  match r:
    case Rule{i, _, _, _, _, _, _, _, _, _}:
      i

def Rule.text(+r: Rule) -> String:
  match r:
    case Rule{_, x, _, _, _, _, _, _, _, _}:
      x

def Rule.bot(+r: Rule) -> String:
  match r:
    case Rule{_, _, b, _, _, _, _, _, _, _}:
      b

def Rule.project(+r: Rule) -> String:
  match r:
    case Rule{_, _, _, p, _, _, _, _, _, _}:
      p

def Rule.tool(+r: Rule) -> String:
  match r:
    case Rule{_, _, _, _, t, _, _, _, _, _}:
      t

def Rule.enforce(+r: Rule) -> Bool:
  match r:
    case Rule{_, _, _, _, _, e, _, _, _, _}:
      e

def Rule.word(+r: Rule) -> String:
  match r:
    case Rule{_, _, _, _, _, _, w, _, _, _}:
      w

def Rule.on(+r: Rule) -> Bool:
  match r:
    case Rule{_, _, _, _, _, _, _, o, _, _}:
      o

def Rule.at(+r: Rule) -> Nat:
  match r:
    case Rule{_, _, _, _, _, _, _, _, a, _}:
      a

def Rule.by(+r: Rule) -> String:
  match r:
    case Rule{_, _, _, _, _, _, _, _, _, b}:
      b

def Rules.blank() -> Rule:
  Rule{"", "", "", "", "", False{}, "", True{}, 0n, ""}

def Rules.json(+r: Rule) -> J.Json:
  match r:
    case Rule{i, x, b, p, t, e, w, o, a, y}:
      J.Json.obj([J.KV{"id", J.Str{i}}, J.KV{"text", J.Str{x}}, J.KV{"bot", J.Str{b}}, J.KV{"project", J.Str{p}}, J.KV{"tool", J.Str{t}},
        J.KV{"enforce", J.Flag{e}}, J.KV{"word", J.Str{w}}, J.KV{"on", J.Flag{o}}, J.KV{"at", J.Json.nat(a)}, J.KV{"by", J.Str{y}}])

def Rules.value(+r: Rule) -> String:
  J.Json.show(Rules.json(r))

def Rules.of.stored(+o: J.Json) -> Rule:
  Rule{J.Json.get_str(o, "id", ""), J.Json.get_str(o, "text", ""), J.Json.get_str(o, "bot", ""), J.Json.get_str(o, "project", ""),
    J.Json.get_str(o, "tool", ""), J.Json.get_bool(o, "enforce", False{}), J.Json.get_str(o, "word", ""), J.Json.get_bool(o, "on", True{}),
    J.Json.get_nat(o, "at", 0n), J.Json.get_str(o, "by", "")}

def Rules.is(+k: String) -> Bool:
  String.starts_with(k, "rule.")

def Rules.with.id(+r: Rule, +id: String) -> Rule:
  match r:
    case Rule{_, x, b, p, t, e, w, o, a, y}:
      Rule{id, x, b, p, t, e, w, o, a, y}

# a stored rule without its id takes the one in its key
def Rules.read.id(+id: String, +r: Rule, +key: String) -> Rule:
  Bool.pick(Rule, String.is_empty(id), Rules.with.id(r, key), r)

def Rules.read.one(+m: Maybe<&2, J.Json>, +k: String, +acc: List<&2, Rule>) -> List<&2, Rule>:
  match m:
    case Some{+o}:
      Rules.read.id(J.Json.get_str(o, "id", ""), Rules.of.stored(o), String.drop(k, 5n)) <> acc
    case None{}:
      acc

def Rules.read.put(+hit: Bool, +k: String, +s: M.State, +acc: List<&2, Rule>) -> List<&2, Rule>:
  match hit:
    case True{}:
      Rules.read.one(J.Json.parse(M.Setting.get(s, k, "")), k, acc)
    case False{}:
      acc

def Rules.read(+ks: List<&2, String>, +s: M.State, +acc: List<&2, Rule>) -> List<&2, Rule>:
  match ks:
    case Nil{}:
      acc
    case +k <> t:
      Rules.read(t, s, Rules.read.put(Rules.is(k), k, s, acc))

# oldest first, then by id
def Rules.older.k(+aa: Nat, +ab: Nat, +ia: String, +ib: String) -> Bool:
  Bool.or(Nat.is_lt(aa, ab), Bool.and(Nat.is_eq(aa, ab), String.is_le(ia, ib)))

def Rules.older(a: Rule, b: Rule) -> Bool:
  match a b:
    case Rule{ia, _, _, _, _, _, _, _, aa, _} Rule{ib, _, _, _, _, _, _, _, ab, _}:
      Rules.older.k(aa, ab, ia, ib)

# every rule, oldest first, in one pass over the settings' keys
def Rules.all(+s: M.State) -> List<&2, Rule>:
  List.sort(~Rule, ~Rules.older, Rules.read(M.Setting.keys(s), s, Nil{}))

def Rules.get.go(+rs: List<&2, Rule>, +id: String) -> Maybe<&2, Rule>:
  match rs:
    case Nil{}:
      None{}
    case +r <> t:
      BT.Pick.first(Rule, String.eq(Rule.id(r), id), r, Rules.get.go(t, id))

def Rules.get(+s: M.State, +id: String) -> Maybe<&2, Rule>:
  Rules.get.go(Rules.read(M.Setting.keys(s), s, Nil{}), id)

# Which threads
# -------------

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


# A rule's scope in words
# -----------------------

# who a rule is for, given the bot's and the project's names ("" for any)
def Rules.scope.bot(+bot: String) -> String:
  Bool.pick(String, String.is_empty(bot), "", "Bot " ++ bot)

def Rules.scope.proj(+project: String) -> String:
  Bool.pick(String, String.is_empty(project), "", "Project " ++ project)

def Rules.scope.join(+b: String, +p: String) -> String:
  Bool.pick(String, String.is_empty(b), Bool.pick(String, String.is_empty(p), "Everywhere", p),
    Bool.pick(String, String.is_empty(p), b, b ++ " · " ++ p))

def Rules.scope.words(+bot: String, +project: String) -> String:
  Rules.scope.join(Rules.scope.bot(bot), Rules.scope.proj(project))

def Rules.note.tool(+tool: String) -> String:
  Bool.pick(String, String.is_empty(tool), "", "tool " ++ tool)

def Rules.note.enforce(+enforce: Bool, +word: String) -> String:
  match enforce:
    case True{}:
      Bool.pick(String, String.is_empty(word), "enforced", "enforced unless \"" ++ word ++ "\"")
    case False{}:
      ""

def Rules.note.off(+on: Bool) -> String:
  Bool.pick(String, on, "", "off")

def Rules.note.keep(+h: String, +rest: List<&2, String>) -> List<&2, String>:
  Bool.pick(List<&2, String>, String.is_empty(h), rest, h <> rest)

def Rules.note.nonempty(+xs: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case +h <> t:
      Rules.note.keep(h, Rules.note.nonempty(t))

# "Bot Miso · tool bot_send · enforced unless "sudo" · off"
def Rules.note(+bot: String, +project: String, +tool: String, +enforce: Bool, +word: String, +on: Bool) -> String:
  String.join(Rules.note.nonempty([Rules.scope.words(bot, project), Rules.note.tool(tool), Rules.note.enforce(enforce, word), Rules.note.off(on)]), " · ")
