import Base
import ./json.bend as J
import ./model.bend as M
import ./bot.bend as BT
import ./hub.bend as B
import ./rules.bend as RL
import ./tbdata.bend as TD

# The task board: what the person and their bots have to do, in one place.
# An item is the setting "board.<id>" holding
# {"id","title","status","owner","project","due","group","links","notes",
# "at","updated","by"}. The owner is a bot's id ("" is the person), the
# project a project's id ("" none), the due date YYYY-MM-DD, the group a
# free section name. Bots read and edit the board with the task_board_*
# tools (core/ledger.bend), the person with the board.* requests; a
# markdown todo list imports once (Tb.import). Settings, so they stay on
# this machine.

def Tb.id(+t: TD.TbItem) -> String:
  TD.Tb.id(t)

def Tb.title(+t: TD.TbItem) -> String:
  TD.Tb.title(t)

def Tb.status(+t: TD.TbItem) -> String:
  TD.Tb.status(t)

def Tb.owner(+t: TD.TbItem) -> String:
  TD.Tb.owner(t)

def Tb.project(+t: TD.TbItem) -> String:
  TD.Tb.project(t)

def Tb.due(+t: TD.TbItem) -> String:
  TD.Tb.due(t)

def Tb.group(+t: TD.TbItem) -> String:
  TD.Tb.group(t)

def Tb.links(+t: TD.TbItem) -> List<&2, String>:
  TD.Tb.links(t)

def Tb.notes(+t: TD.TbItem) -> String:
  TD.Tb.notes(t)

def Tb.at(+t: TD.TbItem) -> Nat:
  TD.Tb.at(t)

def Tb.updated(+t: TD.TbItem) -> Nat:
  TD.Tb.updated(t)

def Tb.by(+t: TD.TbItem) -> String:
  TD.Tb.by(t)

def Tb.blank(+id: String, +at: Nat) -> TD.TbItem:
  TD.Tb.blank(id, at)

def Tb.has(+xs: List<&2, String>, +x: String) -> Bool:
  TD.Tb.has(xs, x)

def Tb.statuses() -> List<&2, String>:
  TD.Tb.statuses()

def Tb.status.k(+l: String) -> String:
  TD.Tb.status.k(l)

def Tb.status.norm(+s: String) -> String:
  TD.Tb.status.norm(s)

def Tb.rank(+s: String) -> Nat:
  TD.Tb.rank(s)

def Tb.is.open(+t: TD.TbItem) -> Bool:
  TD.Tb.is.open(t)

def Tb.digit(+c: U32) -> Bool:
  TD.Tb.digit(c)

def Tb.num(+s: String, +acc: Nat) -> Nat:
  TD.Tb.num(s, acc)

def Tb.digits.all(+s: String, +ok: Bool) -> Bool:
  TD.Tb.digits.all(s, ok)

def Tb.date.parts(+y: String, +m: String, +d: String, +s1: String, +s2: String) -> Bool:
  TD.Tb.date.parts(y, m, d, s1, s2)

def Tb.date.k(+s: String) -> Bool:
  TD.Tb.date.k(s)

def Tb.date(+s: String) -> Bool:
  TD.Tb.date(s)

def Tb.strs(+xs: List<&2, J.Json>) -> List<&2, String>:
  TD.Tb.strs(xs)

def Tb.of.stored(+id: String, +o: J.Json) -> TD.TbItem:
  TD.Tb.of.stored(id, o)

def Tb.jstrs(+xs: List<&2, String>) -> List<&2, J.Json>:
  TD.Tb.jstrs(xs)

def Tb.raw(+t: TD.TbItem) -> J.Json:
  TD.Tb.raw(t)

def Tb.value(+t: TD.TbItem) -> String:
  TD.Tb.value(t)

def Tb.is(+k: String) -> Bool:
  TD.Tb.is(k)

def Tb.read.one(+m: Maybe<&2, J.Json>, +k: String, +acc: List<&2, TD.TbItem>) -> List<&2, TD.TbItem>:
  TD.Tb.read.one(m, k, acc)

def Tb.read.put(+hit: Bool, +k: String, +s: M.State, +acc: List<&2, TD.TbItem>) -> List<&2, TD.TbItem>:
  TD.Tb.read.put(hit, k, s, acc)

def Tb.read(+ks: List<&2, String>, +s: M.State, +acc: List<&2, TD.TbItem>) -> List<&2, TD.TbItem>:
  TD.Tb.read(ks, s, acc)

def Tb.all(+s: M.State) -> List<&2, TD.TbItem>:
  TD.Tb.all(s)

def Tb.get.go(+xs: List<&2, TD.TbItem>, +id: String) -> Maybe<&2, TD.TbItem>:
  TD.Tb.get.go(xs, id)

def Tb.get(+s: M.State, +id: String) -> Maybe<&2, TD.TbItem>:
  TD.Tb.get(s, id)

def Tb.due.key(+d: String) -> String:
  TD.Tb.due.key(d)

def Tb.before.k(+ra: Nat, +rb: Nat, +da: String, +db: String, +aa: Nat, +ab: Nat) -> Bool:
  TD.Tb.before.k(ra, rb, da, db, aa, ab)

def Tb.before(a: TD.TbItem, b: TD.TbItem) -> Bool:
  TD.Tb.before(a, b)

def Tb.order(+xs: List<&2, TD.TbItem>) -> List<&2, TD.TbItem>:
  TD.Tb.order(xs)

def Tb.keep(+ok: Bool, +t: TD.TbItem, +rest: List<&2, TD.TbItem>) -> List<&2, TD.TbItem>:
  TD.Tb.keep(ok, t, rest)

def Tb.open(+xs: List<&2, TD.TbItem>) -> List<&2, TD.TbItem>:
  TD.Tb.open(xs)

def Tb.owner.is(+want: String, +have: String) -> Bool:
  TD.Tb.owner.is(want, have)

def Tb.matches(+sts: List<&2, String>, +owner: String, +project: String, +group: String, +t: TD.TbItem) -> Bool:
  TD.Tb.matches(sts, owner, project, group, t)

def Tb.filter.go(+xs: List<&2, TD.TbItem>, +sts: List<&2, String>, +status: String, +owner: String, +project: String, +group: String) -> List<&2, TD.TbItem>:
  TD.Tb.filter.go(xs, sts, status, owner, project, group)

def Tb.lowers(+xs: List<&2, String>) -> List<&2, String>:
  TD.Tb.lowers(xs)

def Tb.filter(+xs: List<&2, TD.TbItem>, +status: String, +owner: String, +project: String, +group: String) -> List<&2, TD.TbItem>:
  TD.Tb.filter(xs, status, owner, project, group)

def Tb.owner.name(+s: M.State, +id: String) -> String:
  Bool.pick(String, String.is_empty(id), "person", BT.Bot.name(BT.World.bot(M.State.bots(s), id)))

def Tb.proj.name.go(+ps: List<&2, M.Project>, +id: String) -> String:
  match ps:
    case Nil{}:
      id
    case M.Project{+i, +t, _, _, _} <> r:
      Bool.pick(String, String.eq(i, id), t, Tb.proj.name.go(r, id))

def Tb.proj.name(+s: M.State, +id: String) -> String:
  Bool.pick(String, String.is_empty(id), "", Tb.proj.name.go(B.Hub.projects(s), id))

def Tb.json.k(+o: J.Json, +owner: String, +project: String) -> J.Json:
  match o:
    case J.Obj{fs}:
      J.Obj{J.Field{"ownerName", J.Str{owner}, J.Field{"projectName", J.Str{project}, fs}}}
    case _:
      o

# the item's JSON, with the owner's and the project's names added
def Tb.json(+t: TD.TbItem, +s: M.State) -> J.Json:
  Tb.json.k(Tb.raw(t), Tb.owner.name(s, Tb.owner(t)), Tb.proj.name(s, Tb.project(t)))

def Tb.text.at(+pre: String, +v: String) -> String:
  Bool.pick(String, String.is_empty(v), "", pre ++ v)

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

def Tb.text.join(+xs: List<&2, String>) -> String:
  String.join(Tb.text.nonempty(xs), "; ")

def Tb.text.owner(+id: String, +s: M.State) -> String:
  Bool.pick(String, String.is_empty(id), "", Tb.owner.name(s, id))

def Tb.text.parts(+t: TD.TbItem, +s: M.State) -> String:
  Tb.text.join([Tb.id(t), Tb.text.at("@", Tb.text.owner(Tb.owner(t), s)), Tb.text.at("%", Tb.proj.name(s, Tb.project(t))), Tb.text.at("due ", Tb.due(t)), Tb.group(t)])

def Tb.text.one(+t: TD.TbItem, +s: M.State) -> String:
  "[" ++ Tb.status(t) ++ "] " ++ Tb.title(t) ++ " (" ++ Tb.text.parts(t, s) ++ ")"

def Tb.text.go(+xs: List<&2, TD.TbItem>, +s: M.State) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case +t <> r:
      Tb.text.one(t, s) <> Tb.text.go(r, s)

# a compact listing for agents, a line an item:
# [status] title (id; @owner; %project; due D; group)
def Tb.text(+xs: List<&2, TD.TbItem>, +s: M.State) -> String:
  String.join(Tb.text.go(xs, s), "\n")

# Saving
# ------

# a saved or removed item (its id and the changes), or why not
type TbR is Data:
  TbOk{id: String, cs: List<&2, M.Change>}
  TbErr{msg: String}

def Tb.some(+m: Maybe<&2, J.Json>) -> Bool:
  match m:
    case Some{_}:
      True{}
    case None{}:
      False{}

def Tb.given(+o: J.Json, +k: String) -> Bool:
  Tb.some(J.Json.get(o, k))

def Tb.owner.res.k(+person: Bool, +s: M.State, +name: String) -> RL.RS:
  match person:
    case True{}:
      RL.RS{"", ""}
    case False{}:
      RL.Rules.bot.res(s, name)

# the person's own, or a bot by name or id
def Tb.owner.res(+s: M.State, +name: String) -> RL.RS:
  Tb.owner.res.k(String.eq(String.to_lower(name), "person"), s, name)

# a field changed only when the request gives it (the old value stays
# otherwise, even a bot that has since gone)
def Tb.res.pick(+given: Bool, +r: RL.RS, +old: String) -> RL.RS:
  match given:
    case True{}:
      r
    case False{}:
      RL.RS{old, ""}

def Tb.clean.skip(+h: String, +n: Nat) -> Bool:
  Bool.or(String.is_empty(String.trim(h)), Nat.is_ge(n, 20n))

def Tb.clean.put(+skip: Bool, +h: String, +rest: List<&2, String>) -> List<&2, String>:
  match skip:
    case True{}:
      rest
    case False{}:
      h <> rest

def Tb.clean(+xs: List<&2, String>, +n: Nat) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case +h <> t:
      Tb.clean.put(Tb.clean.skip(h, n), String.take(String.trim(h), 500n), Tb.clean(t, Bool.pick(Nat, Tb.clean.skip(h, n), n, Nat.add(n, 1n))))

# links given as an array, or as a string split on spaces and commas
def Tb.links.val(+text: String, +arr: List<&2, String>) -> List<&2, String>:
  Tb.clean(Bool.pick(List<&2, String>, String.is_empty(text), arr, RL.Rules.pats(text)), 0n)

def Tb.links.of(+m: Maybe<&2, J.Json>, +old: List<&2, String>) -> List<&2, String>:
  match m:
    case Some{+j}:
      Tb.links.val(J.Json.as_str(Some{j}, ""), Tb.strs(J.Json.elems(j)))
    case None{}:
      old

def Tb.notes.add(+base: String, +extra: String) -> String:
  Bool.pick(String, String.is_empty(extra), base, Bool.pick(String, String.is_empty(base), extra, base ++ "\n" ++ extra))

def Tb.title.err(+t: String) -> String:
  Bool.pick(String, String.is_empty(t), "an item needs a title", Bool.pick(String, Nat.is_gt(String.length(t), 300n), "a title is at most 300 characters", ""))

def Tb.due.err(+d: String) -> String:
  Bool.pick(String, Bool.or(String.is_empty(d), Tb.date(d)), "", "a due date is YYYY-MM-DD")

def Tb.notes.err(+n: String) -> String:
  Bool.pick(String, Nat.is_gt(String.length(n), 4000n), "notes are at most 4000 characters", "")

def Tb.set.one(+t: TD.TbItem) -> M.Change:
  M.SettingSet{"board." ++ Tb.id(t), Tb.value(t), Tb.updated(t)}

def Tb.set.of(+t: TD.TbItem) -> List<&2, M.Change>:
  [Tb.set.one(t)]

def Tb.make.fin(+err: String, +t: TD.TbItem) -> TbR:
  match err:
    case SNil{}:
      TbOk{Tb.id(t), Tb.set.of(t)}
    case SCon{_, _}:
      TbErr{err}

def Tb.make.k(+title: String, +due: String, +notes: String, +ow: RL.RS, +pr: RL.RS, +status: String, +group: String, +links: List<&2, String>, +old: TD.TbItem, +now: Nat, +by: String) -> TbR:
  Tb.make.fin(RL.Rules.err.first([Tb.title.err(title), Tb.due.err(due), Tb.notes.err(notes), RL.RS.err(ow), RL.RS.err(pr)]),
    TD.TbItem{Tb.id(old), title, status, RL.RS.v(ow), RL.RS.v(pr), due, String.take(group, 80n), links, notes, Tb.at(old), now, by})

# the item that results from the request on `old`: the fields given
# replace the old ones (notes_append adds a line to the notes)
def Tb.make(+s: M.State, +now: Nat, +by: String, +o: J.Json, +old: TD.TbItem) -> TbR:
  Tb.make.k(String.trim(J.Json.get_str(o, "title", Tb.title(old))), String.trim(J.Json.get_str(o, "due", Tb.due(old))),
    Tb.notes.add(String.trim(J.Json.get_str(o, "notes", Tb.notes(old))), String.trim(J.Json.get_str(o, "notes_append", ""))),
    Tb.res.pick(Tb.given(o, "owner"), Tb.owner.res(s, String.trim(J.Json.get_str(o, "owner", ""))), Tb.owner(old)),
    Tb.res.pick(Tb.given(o, "project"), RL.Rules.proj.res(s, String.trim(J.Json.get_str(o, "project", ""))), Tb.project(old)),
    Tb.status.norm(J.Json.get_str(o, "status", Tb.status(old))), String.trim(J.Json.get_str(o, "group", Tb.group(old))),
    Tb.links.of(J.Json.get(o, "links"), Tb.links(old)), old, now, by)

def Tb.add.k(+id: String, +s: M.State, +now: Nat, +by: String, +o: J.Json) -> TbR:
  Tb.make(s, now, by, o, Tb.blank(id, now))

# Add the item a request describes (a title is required). Answers its id
# and the change that saves it.
def Tb.add(+s: M.State, +now: Nat, +by: String, +o: J.Json) -> TbR:
  Tb.add.k(B.Hub.id("b", now, B.Hub.seq(s)), s, now, by, o)

def Tb.update.k(+m: Maybe<&2, TD.TbItem>, +s: M.State, +now: Nat, +by: String, +o: J.Json, +id: String) -> TbR:
  match m:
    case Some{old}:
      Tb.make(s, now, by, o, old)
    case None{}:
      TbErr{"no item " ++ id}

# Change an item: only the fields the request gives change (notes_append
# adds a line to the notes); the rest stays.
def Tb.update(+s: M.State, +now: Nat, +by: String, +id: String, +o: J.Json) -> TbR:
  Tb.update.k(Tb.get(s, id), s, now, by, o, id)

def Tb.remove.k(+found: Bool, +id: String, +now: Nat) -> TbR:
  match found:
    case True{}:
      TbOk{id, [M.SettingSet{"board." ++ id, "", now}]}
    case False{}:
      TbErr{"no item " ++ id}

def Tb.found(+m: Maybe<&2, TD.TbItem>) -> Bool:
  match m:
    case Some{_}:
      True{}
    case None{}:
      False{}

def Tb.remove(+s: M.State, +now: Nat, +id: String) -> TbR:
  Tb.remove.k(Tb.found(Tb.get(s, id)), id, now)

# Import
# ------

# one item read from a markdown list
type Imp is Data:
  Imp{title: String, status: String, due: String, group: String, project: String, notes: String}

# the import so far: the current group and project, whether the call fixed
# the project (headings then name none), whether the last line was an item
# (an indented line then adds to its notes), the items and the skipped
# lines (newest first) and how many were skipped
type IS is Data:
  IS{group: String, project: String, fixed: Bool, can: Bool, items: List<&2, Imp>, skipped: List<&2, String>, nskip: Nat}

def Tb.indent.ws(+c: U32) -> Bool:
  Bool.or(U32.is_eq(c, 32), U32.is_eq(c, 9))

# how many leading spaces and tabs a line has
def Tb.indent(+s: String, +going: Bool, +n: Nat) -> Nat:
  match s going:
    case _ False{}:
      n
    case SNil{} True{}:
      n
    case SCon{Chr{+c}, t} True{}:
      Tb.indent(t, Tb.indent.ws(c), Bool.pick(Nat, Tb.indent.ws(c), Nat.add(n, 1n), n))

# the leading digits of a line
def Tb.lead(+s: String, +going: Bool, +n: Nat) -> Nat:
  match s going:
    case _ False{}:
      n
    case SNil{} True{}:
      n
    case SCon{Chr{+c}, t} True{}:
      Tb.lead(t, Tb.digit(c), Bool.pick(Nat, Tb.digit(c), Nat.add(n, 1n), n))

def Tb.hash(+c: U32) -> Bool:
  U32.is_eq(c, 35)

# how many leading # a line has
def Tb.hashes(+s: String, +going: Bool, +n: Nat) -> Nat:
  match s going:
    case _ False{}:
      n
    case SNil{} True{}:
      n
    case SCon{Chr{+c}, t} True{}:
      Tb.hashes(t, Tb.hash(c), Bool.pick(Nat, Tb.hash(c), Nat.add(n, 1n), n))

# a heading: 1 to 6 # and then a space (or the end)
def Tb.head.ok(+n: Nat, +after: String) -> Bool:
  Bool.and(Bool.and(Nat.is_ge(n, 1n), Nat.is_le(n, 6n)), Bool.or(String.is_empty(after), String.starts_with(after, " ")))

def Tb.head(+rest: String) -> Bool:
  Tb.head.ok(Tb.hashes(rest, True{}, 0n), String.drop(rest, Tb.hashes(rest, True{}, 0n)))

def Tb.head.text(+rest: String) -> String:
  String.trim(String.drop(rest, Tb.hashes(rest, True{}, 0n)))

# the characters before a checkbox: "- ", "* ", "+ " or "1. "
def Tb.bullet.num(+d: Nat, +rest: String) -> Nat:
  Bool.pick(Nat, Bool.and(Nat.is_gt(d, 0n), String.eq(String.take(String.drop(rest, d), 2n), ". ")), Nat.add(d, 2n), 0n)

def Tb.bullet.k(+two: String, +d: Nat, +rest: String) -> Nat:
  Bool.pick(Nat, Bool.or(Bool.or(String.eq(two, "- "), String.eq(two, "* ")), String.eq(two, "+ ")), 2n, Tb.bullet.num(d, rest))

def Tb.bullet(+rest: String) -> Nat:
  Tb.bullet.k(String.take(rest, 2n), Tb.lead(rest, True{}, 0n), rest)

# the status a checkbox says, "" if it is none
def Tb.mark(+b: String) -> String:
  Bool.pick(String, String.eq(b, "[ ]"), "todo", Bool.pick(String, Bool.or(String.eq(b, "[x]"), String.eq(b, "[X]")), "done",
    Bool.pick(String, Bool.or(String.eq(b, "[~]"), String.eq(b, "[/]")), "doing", Bool.pick(String, String.eq(b, "[!]"), "blocked",
      Bool.pick(String, String.eq(b, "[-]"), "dropped", "")))))

# a title with a due date at its end: " (due YYYY-MM-DD)" or " due:YYYY-MM-DD"
type DT is Data:
  DT{title: String, due: String}

def Tb.due.cut(+t: String, +n: Nat, +tail: String) -> String:
  String.trim(String.take(t, Nat.sub(n, String.length(tail))))

def Tb.due.split.bk(+t: String, +n: Nat, +tail: String) -> DT:
  Bool.pick(DT, Bool.and(Nat.is_ge(n, 15n), Bool.and(String.starts_with(tail, " due:"), Tb.date(String.drop(tail, 5n)))),
    DT{Tb.due.cut(t, n, tail), String.drop(tail, 5n)}, DT{t, ""})

def Tb.due.split.b(+t: String, +n: Nat) -> DT:
  Tb.due.split.bk(t, n, String.drop(t, Nat.sub(n, 15n)))

def Tb.due.split.ak(+hit: Bool, +t: String, +n: Nat, +tail: String) -> DT:
  match hit:
    case True{}:
      DT{Tb.due.cut(t, n, tail), String.take(String.drop(tail, 6n), 10n)}
    case False{}:
      Tb.due.split.b(t, n)

def Tb.due.split.a(+t: String, +n: Nat, +tail: String) -> DT:
  Tb.due.split.ak(Bool.and(Nat.is_ge(n, 17n), Bool.and(String.starts_with(tail, " (due "), Bool.and(String.ends_with(tail, ")"), Tb.date(String.take(String.drop(tail, 6n), 10n))))), t, n, tail)

def Tb.due.split(+t: String) -> DT:
  Tb.due.split.a(t, String.length(t), String.drop(t, Nat.sub(String.length(t), 17n)))

def Tb.item.fin(+ok: Bool, +status: String, +title: String, +due: String) -> Maybe<&2, Imp>:
  match ok:
    case True{}:
      Some{Imp{String.take(title, 300n), status, due, "", "", ""}}
    case False{}:
      None{}

def Tb.item.ok(+ok: Bool, +status: String, +d: DT) -> Maybe<&2, Imp>:
  match d:
    case DT{+title, due}:
      Tb.item.fin(Bool.and(ok, Bool.not(String.is_empty(title))), status, title, due)

def Tb.item.k(+status: String, +rest: String, +r1: String) -> Maybe<&2, Imp>:
  Tb.item.ok(Bool.and(Bool.not(String.is_empty(status)), Bool.or(Nat.is_eq(String.length(r1), 3n), String.starts_with(String.drop(r1, 3n), " "))), status, Tb.due.split(String.trim(String.drop(r1, 3n))))

def Tb.item.r(+n: Nat, +r1: String, +rest: String) -> Maybe<&2, Imp>:
  Bool.pick(Maybe<&2, Imp>, Nat.is_eq(n, 0n), None{}, Tb.item.k(Tb.mark(String.take(r1, 3n)), rest, r1))

def Tb.item.r1(+n: Nat, +rest: String) -> Maybe<&2, Imp>:
  Tb.item.r(n, String.trim_start(String.drop(rest, n)), rest)

# the checkbox item a trimmed line is, if it is one
def Tb.item.of(+rest: String) -> Maybe<&2, Imp>:
  Tb.item.r1(Tb.bullet(rest), rest)

def Tb.is.item(+m: Maybe<&2, Imp>) -> Bool:
  match m:
    case Some{_}:
      True{}
    case None{}:
      False{}

# 0 blank, 1 heading, 2 item, 3 a line of the last item's notes, 4 skipped
def Tb.imp.code(+blank: Bool, +item: Bool, +head: Bool, +note: Bool) -> Nat:
  match blank item head note:
    case True{} _ _ _:
      0n
    case False{} True{} _ _:
      2n
    case False{} False{} True{} _:
      1n
    case False{} False{} False{} True{}:
      3n
    case False{} False{} False{} False{}:
      4n

def Tb.imp.with(+i: Imp, +group: String, +project: String) -> Imp:
  match i:
    case Imp{t, s, d, _, _, n}:
      Imp{t, s, d, group, project, n}

def Tb.imp.noted(+xs: List<&2, Imp>, +line: String) -> List<&2, Imp>:
  match xs:
    case Nil{}:
      Nil{}
    case Imp{t, s, d, g, p, +n} <> r:
      Imp{t, s, d, g, p, String.take(Tb.notes.add(n, line), 4000n)} <> r

def Tb.imp.head.proj(+m: Maybe<&2, M.Project>, +old: String) -> String:
  match m:
    case Some{M.Project{i, _, _, _, _}}:
      i
    case None{}:
      old

# a heading that names a project (by its title) puts the items under it in
# that project, unless the call fixed one
def Tb.imp.head.k(+fixed: Bool, +s: M.State, +hd: String, +old: String, +init: String) -> String:
  match fixed:
    case True{}:
      old
    case False{}:
      Tb.imp.head.proj(RL.Rules.proj.find(s, hd), init)

def Tb.imp.push(+m: Maybe<&2, Imp>, +g: String, +p: String, +xs: List<&2, Imp>) -> List<&2, Imp>:
  match m:
    case Some{i}:
      Tb.imp.with(i, g, p) <> xs
    case None{}:
      xs

def Tb.imp.skip(+room: Bool, +line: String, +xs: List<&2, String>) -> List<&2, String>:
  match room:
    case True{}:
      line <> xs
    case False{}:
      xs

def Tb.imp.go2(+code: Nat, +line: String, +rest: String, +hd: String, +it: Maybe<&2, Imp>, +s: M.State, +init: String, +g: String, +p: String, +fixed: Bool, +can: Bool, +items: List<&2, Imp>, +skipped: List<&2, String>, +nskip: Nat) -> IS:
  match code:
    case 0n:
      IS{g, p, fixed, can, items, skipped, nskip}
    case 1n:
      IS{hd, Tb.imp.head.k(fixed, s, hd, p, init), fixed, False{}, items, skipped, nskip}
    case 2n:
      IS{g, p, fixed, True{}, Tb.imp.push(it, g, p, items), skipped, nskip}
    case 3n:
      IS{g, p, fixed, True{}, Tb.imp.noted(items, rest), skipped, nskip}
    case _:
      IS{g, p, fixed, False{}, items, Tb.imp.skip(Nat.is_lt(nskip, 50n), String.trim(line), skipped), Nat.add(nskip, 1n)}

def Tb.imp.apply(+code: Nat, +line: String, +rest: String, +hd: String, +it: Maybe<&2, Imp>, +s: M.State, +init: String, +st: IS) -> IS:
  match st:
    case IS{+g, +p, fixed, can, items, skipped, nskip}:
      Tb.imp.go2(code, line, rest, hd, it, s, init, g, p, fixed, can, items, skipped, nskip)

def Tb.imp.can(+st: IS) -> Bool:
  match st:
    case IS{_, _, _, can, _, _, _}:
      can

def Tb.imp.line.k(+ind: Nat, +line: String, +rest: String, +s: M.State, +init: String, +st: IS) -> IS:
  Tb.imp.apply(Tb.imp.code(String.is_empty(rest), Tb.is.item(Tb.item.of(rest)), Bool.and(Nat.is_eq(ind, 0n), Tb.head(rest)),
      Bool.and(Nat.is_ge(ind, 2n), Tb.imp.can(st))), line, rest, Tb.head.text(rest), Tb.item.of(rest), s, init, st)

def Tb.imp.go(+ls: List<&2, String>, +s: M.State, +init: String, +st: IS) -> IS:
  match ls:
    case Nil{}:
      st
    case +line <> t:
      Tb.imp.go(t, s, init, Tb.imp.line.k(Tb.indent(line, True{}, 0n), line, String.trim(line), s, init, st))

# What an import made: the counts, the lines it skipped (at most 50, in
# order), the changes (one item each) and an error when the owner or the
# project asked for does not exist
type TbImp is Data:
  TbImp{added: Nat, dups: Nat, skipped: List<&2, String>, cs: List<&2, M.Change>, err: String}

# lower case, single spaces: two titles are the same when these match
type NW is Data:
  NW{acc: String, space: Bool}

def Tb.low(+c: U32) -> U32:
  Bool.pick(U32, Bool.and(U32.is_ge(c, 65), U32.is_le(c, 90)), (c + 32 : U32), c)

def Tb.ws(+c: U32) -> Bool:
  Bool.or(Bool.or(U32.is_eq(c, 32), U32.is_eq(c, 9)), Bool.or(U32.is_eq(c, 10), U32.is_eq(c, 13)))

def Tb.norm.step(+ws: Bool, +c: U32, +st: NW) -> NW:
  match ws st:
    case True{} NW{+acc, _}:
      NW{acc, Bool.not(String.is_empty(acc))}
    case False{} NW{acc, True{}}:
      NW{SCon{Chr{Tb.low(c)}, SCon{Chr{32}, acc}}, False{}}
    case False{} NW{acc, False{}}:
      NW{SCon{Chr{Tb.low(c)}, acc}, False{}}

def Tb.norm.go(+s: String, +st: NW) -> NW:
  match s:
    case SNil{}:
      st
    case SCon{Chr{+c}, t}:
      Tb.norm.go(t, Tb.norm.step(Tb.ws(c), c, st))

def Tb.norm.out(+st: NW) -> String:
  match st:
    case NW{acc, _}:
      String.reverse(acc)

def Tb.norm(+s: String) -> String:
  Tb.norm.out(Tb.norm.go(s, NW{SNil{}, False{}}))

# an item as the import compares it: its group and title, normalized
def Tb.dupkey(+g: String, +t: String) -> String:
  Tb.norm(g) ++ "\n" ++ Tb.norm(t)

def Tb.seen(+xs: List<&2, TD.TbItem>, +acc: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      acc
    case +t <> r:
      Tb.seen(r, Tb.dupkey(Tb.group(t), Tb.title(t)) <> acc)

# the keys of what is saved so far, the changes (newest first), the counts
type IF is Data:
  IF{seen: List<&2, String>, cs: List<&2, M.Change>, added: Nat, dups: Nat}

def Tb.imp.id(+now: Nat, +seq: Nat, +idx: Nat) -> String:
  "b" ++ Nat.show(now) ++ "-" ++ Nat.show(seq) ++ "-" ++ Nat.show(idx)

def Tb.imp.item(+i: Imp, +id: String, +now: Nat, +owner: String, +by: String) -> TD.TbItem:
  match i:
    case Imp{+t, s, d, +g, p, n}:
      TD.TbItem{id, t, s, owner, p, d, String.take(g, 80n), Nil{}, String.take(n, 4000n), now, now, by}

def Tb.imp.put(+dup: Bool, +key: String, +t: TD.TbItem, +st: IF) -> IF:
  match dup st:
    case True{} IF{seen, cs, added, +dups}:
      IF{seen, cs, added, Nat.add(dups, 1n)}
    case False{} IF{seen, cs, +added, dups}:
      IF{key <> seen, Tb.set.one(t) <> cs, Nat.add(added, 1n), dups}

def Tb.imp.dup(+st: IF, +key: String) -> Bool:
  match st:
    case IF{seen, _, _, _}:
      Tb.has(seen, key)

def Tb.imp.step.d(+key: String, +t: TD.TbItem, +st: IF) -> IF:
  Tb.imp.put(Tb.imp.dup(st, key), key, t, st)

def Tb.imp.step.k(+t: TD.TbItem, +st: IF) -> IF:
  Tb.imp.step.d(Tb.dupkey(Tb.group(t), Tb.title(t)), t, st)

def Tb.imp.step(+i: Imp, +now: Nat, +seq: Nat, +owner: String, +by: String, +idx: Nat, +st: IF) -> IF:
  Tb.imp.step.k(Tb.imp.item(i, Tb.imp.id(now, seq, idx), now, owner, by), st)

def Tb.imp.run(+xs: List<&2, Imp>, +now: Nat, +seq: Nat, +owner: String, +by: String, +idx: Nat, +st: IF) -> IF:
  match xs:
    case Nil{}:
      st
    case +i <> t:
      Tb.imp.run(t, now, seq, owner, by, Nat.add(idx, 1n), Tb.imp.step(i, now, seq, owner, by, idx, st))

def Tb.imp.out(+st: IS, +f: IF) -> TbImp:
  match st f:
    case IS{_, _, _, _, _, skipped, _} IF{_, cs, added, dups}:
      TbImp{added, dups, List.reverse(&2, String, skipped), List.reverse(&2, M.Change, cs), ""}

def Tb.imp.items(+st: IS) -> List<&2, Imp>:
  match st:
    case IS{_, _, _, _, items, _, _}:
      items

def Tb.imp.made(+st: IS, +now: Nat, +seq: Nat, +owner: String, +by: String, +seen: List<&2, String>) -> TbImp:
  Tb.imp.out(st, Tb.imp.run(List.reverse(&2, Imp, Tb.imp.items(st)), now, seq, owner, by, 0n, IF{seen, Nil{}, 0n, 0n}))

def Tb.import.run(+s: M.State, +now: Nat, +by: String, +markdown: String, +pr: RL.RS, +fixed: Bool, +owner: String) -> TbImp:
  Tb.imp.made(Tb.imp.go(String.lines(markdown), s, RL.RS.v(pr), IS{"", RL.RS.v(pr), fixed, False{}, Nil{}, Nil{}, 0n}), now, B.Hub.seq(s), owner, by, Tb.seen(Tb.all(s), Nil{}))

def Tb.import.k(+err: String, +s: M.State, +now: Nat, +by: String, +markdown: String, +pr: RL.RS, +fixed: Bool, +owner: String) -> TbImp:
  match err:
    case SNil{}:
      Tb.import.run(s, now, by, markdown, pr, fixed, owner)
    case SCon{_, _}:
      TbImp{0n, 0n, Nil{}, Nil{}, err}

def Tb.import.e(+pr: RL.RS, +ow: RL.RS, +s: M.State, +now: Nat, +by: String, +markdown: String, +fixed: Bool) -> TbImp:
  Tb.import.k(RL.Rules.err.first([RL.RS.err(pr), RL.RS.err(ow)]), s, now, by, markdown, pr, fixed, RL.RS.v(ow))

# Import a markdown todo list: `#` headings set the group (one that names
# a project, by its title, also the project, unless the call gave one),
# and checkbox items (`- [ ]`, `* [x]`, `1. [~]`...; [ ] todo, [x] done,
# [~] or [/] doing, [!] blocked, [-] dropped) become items, a due date
# written at the end of the title taken out of it, indented lines under
# an item added to its notes. Anything else is not imported: it comes back
# in `skipped` (at most 50 lines). An item whose group and title (lower
# case, single spaces) are already on the board, or earlier in the list,
# is not added again and counts in `dups`.
def Tb.import(+s: M.State, +now: Nat, +by: String, +markdown: String, +project: String, +owner: String) -> TbImp:
  Tb.import.e(RL.Rules.proj.res(s, String.trim(project)), Tb.owner.res(s, String.trim(owner)), s, now, by, markdown, Bool.not(String.is_empty(String.trim(project))))

def Tb.demo.apply(+r: TbImp, +s: M.State) -> M.State:
  match r:
    case TbImp{_, _, _, cs, _}:
      M.State.replay(cs, s)

# Examples the laws read
# ----------------------

# the board after importing a markdown list (as the person, at 1000)
def Tb.demo.import(+md: String, +s: M.State) -> M.State:
  Tb.demo.apply(Tb.import(s, 1000n, "person", md, "", ""), s)

# what an import did: items added, duplicates, the lines skipped
def Tb.demo.summary(+r: TbImp) -> String:
  match r:
    case TbImp{a, d, sk, _, _}:
      Nat.show(a) ++ " " ++ Nat.show(d) ++ " " ++ String.join(sk, " / ")

def Tb.demo.line(+t: TD.TbItem) -> String:
  Tb.status(t) ++ " " ++ Tb.group(t) ++ " " ++ Tb.title(t)

def Tb.demo.lines.go(+xs: List<&2, TD.TbItem>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case t <> r:
      Tb.demo.line(t) <> Tb.demo.lines.go(r)

# the items in board order, "status group title" each
def Tb.demo.lines(+s: M.State) -> String:
  String.join(Tb.demo.lines.go(Tb.order(Tb.all(s))), ";")

# one item, "Fix it" with a note, and the same after a request that only
# gives its status
def Tb.demo.item() -> M.State:
  M.State.replay([M.SettingSet{"board.i1", "{\"title\":\"Fix it\",\"status\":\"todo\",\"notes\":\"a note\",\"at\":5}", 5n}], M.State.empty())

def Tb.demo.updated.k(+r: TbR) -> M.State:
  match r:
    case TbOk{_, cs}:
      M.State.replay(cs, Tb.demo.item())
    case TbErr{_}:
      Tb.demo.item()

def Tb.demo.updated() -> M.State:
  Tb.demo.updated.k(Tb.update(Tb.demo.item(), 9n, "person", "i1", Maybe.default(&2, J.Json, J.Json.parse("{\"status\":\"doing\"}"), J.Null{})))

def Tb.demo.merged(+s: M.State) -> String:
  Tb.title(Maybe.default(&2, TD.TbItem, Tb.get(s, "i1"), Tb.blank("?", 0n))) ++ "|" ++ Tb.notes(Maybe.default(&2, TD.TbItem, Tb.get(s, "i1"), Tb.blank("?", 0n))) ++ "|" ++ Tb.status(Maybe.default(&2, TD.TbItem, Tb.get(s, "i1"), Tb.blank("?", 0n)))
