import Base
import ./json.bend as J
import ./model.bend as M
import ./bot.bend as BT

type TbItem is Data:
  TbItem{id: String, title: String, status: String, owner: String, project: String, due: String, group: String, links: List<&2, String>, notes: String, at: Nat, updated: Nat, by: String}

def Tb.id(+t: TbItem) -> String:
  match t:
    case TbItem{i, _, _, _, _, _, _, _, _, _, _, _}:
      i

def Tb.title(+t: TbItem) -> String:
  match t:
    case TbItem{_, x, _, _, _, _, _, _, _, _, _, _}:
      x

def Tb.status(+t: TbItem) -> String:
  match t:
    case TbItem{_, _, s, _, _, _, _, _, _, _, _, _}:
      s

def Tb.owner(+t: TbItem) -> String:
  match t:
    case TbItem{_, _, _, o, _, _, _, _, _, _, _, _}:
      o

def Tb.project(+t: TbItem) -> String:
  match t:
    case TbItem{_, _, _, _, p, _, _, _, _, _, _, _}:
      p

def Tb.due(+t: TbItem) -> String:
  match t:
    case TbItem{_, _, _, _, _, d, _, _, _, _, _, _}:
      d

def Tb.group(+t: TbItem) -> String:
  match t:
    case TbItem{_, _, _, _, _, _, g, _, _, _, _, _}:
      g

def Tb.links(+t: TbItem) -> List<&2, String>:
  match t:
    case TbItem{_, _, _, _, _, _, _, l, _, _, _, _}:
      l

def Tb.notes(+t: TbItem) -> String:
  match t:
    case TbItem{_, _, _, _, _, _, _, _, n, _, _, _}:
      n

def Tb.at(+t: TbItem) -> Nat:
  match t:
    case TbItem{_, _, _, _, _, _, _, _, _, a, _, _}:
      a

def Tb.updated(+t: TbItem) -> Nat:
  match t:
    case TbItem{_, _, _, _, _, _, _, _, _, _, u, _}:
      u

def Tb.by(+t: TbItem) -> String:
  match t:
    case TbItem{_, _, _, _, _, _, _, _, _, _, _, b}:
      b

def Tb.blank(+id: String, +at: Nat) -> TbItem:
  TbItem{id, "", "todo", "", "", "", "", Nil{}, "", at, at, ""}

# Status
# ------

def Tb.has(+xs: List<&2, String>, +x: String) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case +h <> t:
      Bool.or(String.eq(h, x), Tb.has(t, x))

def Tb.statuses() -> List<&2, String>:
  ["doing", "blocked", "todo", "done", "dropped"]

def Tb.status.k(+l: String) -> String:
  Bool.pick(String, Tb.has(Tb.statuses(), l), l, "todo")

# a status as written: lower case; an unknown or an empty one is todo
def Tb.status.norm(+s: String) -> String:
  Tb.status.k(String.to_lower(String.take(String.trim(s), 20n)))

# the order of statuses: doing, blocked, todo, then done and dropped
def Tb.rank(+s: String) -> Nat:
  Bool.pick(Nat, String.eq(s, "doing"), 0n, Bool.pick(Nat, String.eq(s, "blocked"), 1n, Bool.pick(Nat, String.eq(s, "todo"), 2n,
    Bool.pick(Nat, String.eq(s, "done"), 3n, 4n))))

# not done and not dropped
def Tb.is.open(+t: TbItem) -> Bool:
  Nat.is_lt(Tb.rank(Tb.status(t)), 3n)

# Dates
# -----

def Tb.digit(+c: U32) -> Bool:
  Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57))

def Tb.num(+s: String, +acc: Nat) -> Nat:
  match s:
    case SNil{}:
      acc
    case SCon{Chr{+c}, t}:
      Tb.num(t, Nat.add(Nat.mul(acc, 10n), U32.to_nat((c - 48 : U32))))

def Tb.digits.all(+s: String, +ok: Bool) -> Bool:
  match s ok:
    case _ False{}:
      False{}
    case SNil{} True{}:
      True{}
    case SCon{Chr{+c}, t} True{}:
      Tb.digits.all(t, Tb.digit(c))

def Tb.date.parts(+y: String, +m: String, +d: String, +s1: String, +s2: String) -> Bool:
  Bool.and(Bool.and(Tb.digits.all(y, True{}), Tb.digits.all(m, True{})), Bool.and(Tb.digits.all(d, True{}),
    Bool.and(Bool.and(String.eq(s1, "-"), String.eq(s2, "-")),
      Bool.and(Bool.and(Nat.is_ge(Tb.num(m, 0n), 1n), Nat.is_le(Tb.num(m, 0n), 12n)), Bool.and(Nat.is_ge(Tb.num(d, 0n), 1n), Nat.is_le(Tb.num(d, 0n), 31n))))))

def Tb.date.k(+s: String) -> Bool:
  Tb.date.parts(String.take(s, 4n), String.take(String.drop(s, 5n), 2n), String.drop(s, 8n), String.take(String.drop(s, 4n), 1n), String.take(String.drop(s, 7n), 1n))

# a due date is YYYY-MM-DD (the month 1 to 12, the day 1 to 31)
def Tb.date(+s: String) -> Bool:
  Bool.and(Nat.is_eq(String.length(s), 10n), Tb.date.k(s))

# Reading
# -------

def Tb.strs(+xs: List<&2, J.Json>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case +o <> t:
      J.Json.as_str(Some{o}, "") <> Tb.strs(t)

def Tb.of.stored(+id: String, +o: J.Json) -> TbItem:
  TbItem{id, J.Json.get_str(o, "title", ""), Tb.status.norm(J.Json.get_str(o, "status", "todo")), J.Json.get_str(o, "owner", ""), J.Json.get_str(o, "project", ""),
    J.Json.get_str(o, "due", ""), J.Json.get_str(o, "group", ""), Tb.strs(J.Json.elems(J.Json.get_obj(o, "links"))), J.Json.get_str(o, "notes", ""),
    J.Json.get_nat(o, "at", 0n), J.Json.get_nat(o, "updated", 0n), J.Json.get_str(o, "by", "")}

def Tb.jstrs(+xs: List<&2, String>) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      J.Str{x} <> Tb.jstrs(t)

def Tb.raw(+t: TbItem) -> J.Json:
  match t:
    case TbItem{i, x, s, o, p, d, g, l, n, a, u, b}:
      J.Json.obj([J.KV{"id", J.Str{i}}, J.KV{"title", J.Str{x}}, J.KV{"status", J.Str{s}}, J.KV{"owner", J.Str{o}}, J.KV{"project", J.Str{p}}, J.KV{"due", J.Str{d}},
        J.KV{"group", J.Str{g}}, J.KV{"links", J.Json.arr(Tb.jstrs(l))}, J.KV{"notes", J.Str{n}}, J.KV{"at", J.Json.nat(a)}, J.KV{"updated", J.Json.nat(u)}, J.KV{"by", J.Str{b}}])

def Tb.value(+t: TbItem) -> String:
  J.Json.show(Tb.raw(t))

def Tb.is(+k: String) -> Bool:
  String.starts_with(k, "board.")

def Tb.read.one(+m: Maybe<&2, J.Json>, +k: String, +acc: List<&2, TbItem>) -> List<&2, TbItem>:
  match m:
    case Some{o}:
      Tb.of.stored(String.drop(k, 6n), o) <> acc
    case None{}:
      acc

def Tb.read.put(+hit: Bool, +k: String, +s: M.State, +acc: List<&2, TbItem>) -> List<&2, TbItem>:
  match hit:
    case True{}:
      Tb.read.one(J.Json.parse(M.Setting.get(s, k, "")), k, acc)
    case False{}:
      acc

def Tb.read(+ks: List<&2, String>, +s: M.State, +acc: List<&2, TbItem>) -> List<&2, TbItem>:
  match ks:
    case Nil{}:
      acc
    case +k <> t:
      Tb.read(t, s, Tb.read.put(Tb.is(k), k, s, acc))

# every item, in one pass over the settings' keys (Tb.order sorts them)
def Tb.all(+s: M.State) -> List<&2, TbItem>:
  Tb.read(M.Setting.keys(s), s, Nil{})

def Tb.get.go(+xs: List<&2, TbItem>, +id: String) -> Maybe<&2, TbItem>:
  match xs:
    case Nil{}:
      None{}
    case +t <> r:
      BT.Pick.first(TbItem, String.eq(Tb.id(t), id), t, Tb.get.go(r, id))

def Tb.get(+s: M.State, +id: String) -> Maybe<&2, TbItem>:
  Tb.get.go(Tb.all(s), id)

# Order
# -----

def Tb.due.key(+d: String) -> String:
  Bool.pick(String, String.is_empty(d), "9999-99-99", d)

def Tb.before.k(+ra: Nat, +rb: Nat, +da: String, +db: String, +aa: Nat, +ab: Nat) -> Bool:
  Bool.or(Nat.is_lt(ra, rb), Bool.and(Nat.is_eq(ra, rb), Bool.or(String.is_lt(da, db), Bool.and(String.eq(da, db), Nat.is_le(aa, ab)))))

# a is not after b: by status (open first), then due date (none last),
# then when it was made
def Tb.before(a: TbItem, b: TbItem) -> Bool:
  match a b:
    case TbItem{_, _, sa, _, _, da, _, _, _, aa, _, _} TbItem{_, _, sb, _, _, db, _, _, _, ab, _, _}:
      Tb.before.k(Tb.rank(sa), Tb.rank(sb), Tb.due.key(da), Tb.due.key(db), aa, ab)

# open before closed (doing, blocked, todo, then done, dropped); within a
# status the earliest due first, none last, then the oldest first. A merge
# sort, so hundreds of items cost little.
def Tb.order(+xs: List<&2, TbItem>) -> List<&2, TbItem>:
  List.sort(~TbItem, ~Tb.before, xs)

def Tb.keep(+ok: Bool, +t: TbItem, +rest: List<&2, TbItem>) -> List<&2, TbItem>:
  match ok:
    case True{}:
      t <> rest
    case False{}:
      rest

def Tb.open(+xs: List<&2, TbItem>) -> List<&2, TbItem>:
  match xs:
    case Nil{}:
      Nil{}
    case +t <> r:
      Tb.keep(Tb.is.open(t), t, Tb.open(r))

# Filtering
# ---------

def Tb.owner.is(+want: String, +have: String) -> Bool:
  Bool.or(String.is_empty(want), Bool.or(String.eq(want, have), Bool.and(String.eq(want, "person"), String.is_empty(have))))

# `status` may list several, separated by commas or spaces; `owner` is a
# bot's id, or "person" for the person's own
def Tb.matches(+sts: List<&2, String>, +owner: String, +project: String, +group: String, +t: TbItem) -> Bool:
  Bool.and(Bool.or(List.is_empty(&2, String, sts), Tb.has(sts, Tb.status(t))),
    Bool.and(Bool.and(Tb.owner.is(owner, Tb.owner(t)), Bool.or(String.is_empty(project), String.eq(project, Tb.project(t)))),
      Bool.or(String.is_empty(group), String.eq(group, Tb.group(t)))))

def Tb.filter.go(+xs: List<&2, TbItem>, +sts: List<&2, String>, +status: String, +owner: String, +project: String, +group: String) -> List<&2, TbItem>:
  match xs:
    case Nil{}:
      Nil{}
    case +t <> r:
      Tb.keep(Tb.matches(sts, owner, project, group, t), t, Tb.filter.go(r, sts, status, owner, project, group))

def Tb.lowers(+xs: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case +h <> t:
      Tb.status.norm(h) <> Tb.lowers(t)

# empty means any; the owner and the project are ids
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


def Tb.filter(+xs: List<&2, TbItem>, +status: String, +owner: String, +project: String, +group: String) -> List<&2, TbItem>:
  Tb.filter.go(xs, Tb.lowers(Rules.pats(status)), status, owner, project, group)


def Tb.words(+s: String) -> String:
  Bool.pick(String, String.eq(s, "doing"), "Doing", Bool.pick(String, String.eq(s, "blocked"), "Blocked",
    Bool.pick(String, String.eq(s, "todo"), "To do", Bool.pick(String, String.eq(s, "done"), "Done", "Dropped"))))

def Tb.projects(s: M.State) -> List<&2, M.Project>:
  match s:
    case M.State{ps, _, _, _, _, _, _, _, _, _, _}:
      Map.values(&2, M.Project, ps)

def Tb.owner.name(+s: M.State, +id: String) -> String:
  Bool.pick(String, String.is_empty(id), "you", BT.Bot.name(BT.World.bot(M.State.bots(s), id)))

def Tb.proj.name.go(ps: List<&2, M.Project>, +id: String) -> String:
  match ps:
    case Nil{}:
      id
    case M.Project{+i, +t, _, _, _} <> r:
      Bool.pick(String, String.eq(i, id), t, Tb.proj.name.go(r, id))

def Tb.proj.name(+s: M.State, +id: String) -> String:
  Bool.pick(String, String.is_empty(id), "", Tb.proj.name.go(Tb.projects(s), id))

def Tb.text.put(+none: Bool, +h: String, +rest: List<&2, String>) -> List<&2, String>:
  match none:
    case True{}:
      rest
    case False{}:
      h <> rest

def Tb.text.nonempty(+xs: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case +h <> t:
      Tb.text.put(String.is_empty(h), h, Tb.text.nonempty(t))

