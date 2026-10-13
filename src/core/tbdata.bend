import Base
import ./json.bend as J
import ./model.bend as M
import ./bot.bend as BT
import ./ruledata.bend as RD

# The task board's data: an item (the setting "board.<id>"), reading the
# items from the settings, their order, filters and status words. Pure and
# light, so the clients can import it without the hub (tboard.bend keeps the
# saving, importing and the agent's text, and wraps these under the same names).

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
def Tb.filter(+xs: List<&2, TbItem>, +status: String, +owner: String, +project: String, +group: String) -> List<&2, TbItem>:
  Tb.filter.go(xs, Tb.lowers(RD.Rules.pats(status)), status, owner, project, group)

# Names
# -----


# Due dates in words
# ------------------

def Tb.month.names() -> List<&2, String>:
  ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"]

# a due date as "Oct 28" (the date as written when it is no date)
def Tb.due.words(+d: String) -> String:
  Bool.pick(String, Tb.date(d), Maybe.default(&2, String, List.get(&2, String, Tb.month.names(), Nat.sub(Tb.num(String.take(String.drop(d, 5n), 2n), 0n), 1n)), "")
    ++ " " ++ Nat.show(Tb.num(String.drop(d, 8n), 0n)), d)

def Tb.days.doe(+era: Nat, +yoe: Nat, +doy: Nat) -> Nat:
  Nat.sub(Nat.add(Nat.mul(era, 146097n), Nat.sub(Nat.add(Nat.add(Nat.mul(yoe, 365n), Nat.div(yoe, 4n)), doy), Nat.div(yoe, 100n))), 719468n)

def Tb.days.era(+y: Nat, +mp: Nat, +d: Nat) -> Nat:
  Tb.days.doe(Nat.div(y, 400n), Nat.mod(y, 400n), Nat.add(Nat.div(Nat.add(Nat.mul(153n, mp), 2n), 5n), Nat.sub(d, 1n)))

# the days since 1970-01-01 of a civil date (y, m, d), months from 1
def Tb.days.civil(+y: Nat, +m: Nat, +d: Nat) -> Nat:
  Tb.days.era(Nat.sub(y, Bool.pick(Nat, Nat.is_le(m, 2n), 1n, 0n)), Bool.pick(Nat, Nat.is_gt(m, 2n), Nat.sub(m, 3n), Nat.add(m, 9n)), d)

# the days since the epoch of a due date, 0 for no date
def Tb.days(+d: String) -> Nat:
  Bool.pick(Nat, Tb.date(d), Tb.days.civil(Tb.num(String.take(d, 4n), 0n), Tb.num(String.take(String.drop(d, 5n), 2n), 0n), Tb.num(String.drop(d, 8n), 0n)), 0n)

# the day of a moment (seconds, UTC)
def Tb.today(+now: Nat) -> Nat:
  Nat.div(now, 86400n)

# 0 no due date, 1 later than two days away, 2 due within two days (today
# too), 3 past
def Tb.due.state.k(+dd: Nat, +today: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_lt(dd, today), 3n, Bool.pick(Nat, Nat.is_le(dd, Nat.add(today, 2n)), 2n, 1n))

def Tb.due.state(+d: String, +now: Nat) -> Nat:
  Bool.pick(Nat, Tb.date(d), Tb.due.state.k(Tb.days(d), Tb.today(now)), 0n)
