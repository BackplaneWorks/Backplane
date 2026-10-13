import Base
import ./json.bend as J
import ./model.bend as M
import ./bot.bend as BT

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


# Names for client scope words; no hub dependency.
def Rules.project.name.go(ps: List<&2, M.Project>, +id: String) -> String:
  match ps:
    case Nil{}:
      id
    case M.Project{+i, +title, _, _, _} <> t:
      Bool.pick(String, String.eq(i, id), title, Rules.project.name.go(t, id))

def Rules.projects(s: M.State) -> List<&2, M.Project>:
  match s:
    case M.State{ps, _, _, _, _, _, _, _, _, _, _}:
      Map.values(&2, M.Project, ps)

def Rules.project.name(+s: M.State, +id: String) -> String:
  Bool.pick(String, String.is_empty(id), "", Rules.project.name.go(Rules.projects(s), id))

def Rules.bot.name(+s: M.State, +id: String) -> String:
  BT.Bot.name(BT.World.bot(M.State.bots(s), id))

def Rules.scope(+r: Rule, +s: M.State) -> String:
  +b = Rule.bot(r)
  +p = Rule.project(r)
  Bool.pick(String, Bool.and(String.is_empty(b), String.is_empty(p)), "Everywhere",
    Bool.pick(String, String.is_empty(b), "", "Bot " ++ Rules.bot.name(s, b)) ++
    Bool.pick(String, String.is_empty(p), "", Bool.pick(String, String.is_empty(b), "", " · ") ++ "Project " ++ Rules.project.name(s, p)))
