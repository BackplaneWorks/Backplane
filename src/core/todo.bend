import Base
import ./json.bend as J

# An agent's todo list: live progress of the current run (V2's todo_list
# plan artifact). Claude's task tools (TaskCreate, TaskUpdate; TodoWrite on
# older versions), Codex's todo_list items and Grok's ACP plan all become
# the same list of steps, each pending, in_progress or completed, stored as
# JSON with the thread (TodosSet) and shown at the top of the thread,
# beside its subagents, while the turn runs or anything is left to do (laws
# todo_*). It shows as one line ("To-do 2/5") until the person opens it
# (client.bend's Todos.open, laws todos_*).
#
# Claude's task tools change the list a step at a time, so the hub keeps
# it: a created step waits under its tool call's id ("u:<id>") until the
# tool's result names its number ("Task #3 created ..."), which updates
# then use.

type Todo is Data:
  Todo{id: String, text: String, form: String, status: String}

def Todo.status(t: Todo) -> String:
  match t:
    case Todo{_, _, _, st}:
      st

def Todo.status.of(+s: String) -> String:
  Bool.pick(String, Bool.or(String.eq(s, "completed"), String.eq(s, "in_progress")), s, "pending")

# TodoWrite's input: {todos: [{content, status, activeForm}]}, the whole list
def Todo.write.go(xs: List<&2, J.Json>, +i: Nat) -> List<&2, Todo>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> t:
      Todo{Nat.show(i), J.Json.get_str(x, "content", ""), J.Json.get_str(x, "activeForm", ""), Todo.status.of(J.Json.get_str(x, "status", ""))} <> Todo.write.go(t, 1n+i)

def Todo.write(+input: J.Json) -> List<&2, Todo>:
  Todo.write.go(J.Json.elems(J.Json.get_obj(input, "todos")), 1n)

# TaskCreate: a new pending step, under its tool call's id until named
def Todo.create(ts: List<&2, Todo>, +use: String, +input: J.Json) -> List<&2, Todo>:
  List.append(&2, Todo, ts, [Todo{"u:" ++ use, J.Json.get_str(input, "subject", J.Json.get_str(input, "description", "")), J.Json.get_str(input, "activeForm", ""), "pending"}])

def Todo.named.k(hit: Bool, t: Todo, +n: String) -> Todo:
  match hit t:
    case True{} Todo{_, x, f, st}:
      Todo{n, x, f, st}
    case False{} _:
      t

# the created step's number, once its result says it
def Todo.named(ts: List<&2, Todo>, +use: String, +n: String) -> List<&2, Todo>:
  match ts:
    case Nil{}:
      Nil{}
    case Todo{+i, x, f, st} <> t:
      Todo.named.k(String.eq(i, "u:" ++ use), Todo{i, x, f, st}, n) <> Todo.named(t, use, n)

def Todo.or(+v: String, +d: String) -> String:
  Bool.pick(String, String.is_empty(v), d, v)

def Todo.changed(t: Todo, +input: J.Json) -> Todo:
  match t:
    case Todo{i, +x, +f, +st}:
      Todo{i, Todo.or(J.Json.get_str(input, "subject", ""), x), Todo.or(J.Json.get_str(input, "activeForm", ""), f),
        Bool.pick(String, String.is_empty(J.Json.get_str(input, "status", "")), st, Todo.status.of(J.Json.get_str(input, "status", "")))}

def Todo.update.one(hit: Bool, gone: Bool, t: Todo, +input: J.Json, rest: List<&2, Todo>) -> List<&2, Todo>:
  match hit gone:
    case True{} True{}:
      rest
    case True{} False{}:
      Todo.changed(t, input) <> rest
    case False{} _:
      t <> rest

# TaskUpdate: {taskId, status (deleted drops it), subject, activeForm}
def Todo.update(ts: List<&2, Todo>, +input: J.Json) -> List<&2, Todo>:
  match ts:
    case Nil{}:
      Nil{}
    case Todo{+i, x, f, st} <> t:
      Todo.update.one(String.eq(i, J.Json.get_str(input, "taskId", "")), String.eq(J.Json.get_str(input, "status", ""), "deleted"), Todo{i, x, f, st}, input, Todo.update(t, input))

def Todo.digit(keep: Bool, +c: U32, rest: String) -> String:
  match keep:
    case True{}:
      SCon{Chr{c}, rest}
    case False{}:
      ""

# the leading digits of s
def Todo.digits(s: String) -> String:
  match s:
    case SNil{}:
      SNil{}
    case SCon{Chr{+c}, t}:
      Todo.digit(Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57)), c, Todo.digits(t))

def Todo.result.k(ps: List<&2, String>) -> String:
  match ps:
    case _ <> rest <> _:
      Todo.digits(rest)
    case _:
      ""

# the number in a TaskCreate result ("Task #3 created successfully: ..."),
# or "" for anything else
def Todo.result(+text: String) -> String:
  Bool.pick(String, String.starts_with(text, "Task #"), Todo.result.k(String.split(text, '#')), "")

# Codex's todo_list item: {items: [{text, completed}]}; Grok's plan comes
# the same way with a status instead
def Todo.codex.go(xs: List<&2, J.Json>, +i: Nat) -> List<&2, Todo>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> t:
      Todo{Nat.show(i), J.Json.get_str(x, "text", ""), "",
        Bool.pick(String, J.Json.get_bool(x, "completed", False{}), "completed", Todo.status.of(J.Json.get_str(x, "status", "")))} <> Todo.codex.go(t, 1n+i)

def Todo.codex(+item: J.Json) -> List<&2, Todo>:
  Todo.codex.go(J.Json.elems(J.Json.get_obj(item, "items")), 1n)

def Todo.json.go(ts: List<&2, Todo>) -> List<&2, J.Json>:
  match ts:
    case Nil{}:
      Nil{}
    case Todo{i, text, f, st} <> t:
      J.Json.obj([J.KV{"id", J.Str{i}}, J.KV{"text", J.Str{text}}, J.KV{"form", J.Str{f}}, J.KV{"status", J.Str{st}}]) <> Todo.json.go(t)

# the list as stored
def Todo.show(ts: List<&2, Todo>) -> String:
  J.Json.show(J.Json.arr(Todo.json.go(ts)))

def Todo.read.go(xs: List<&2, J.Json>) -> List<&2, Todo>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> t:
      Todo{J.Json.get_str(x, "id", ""), J.Json.get_str(x, "text", ""), J.Json.get_str(x, "form", ""), Todo.status.of(J.Json.get_str(x, "status", ""))} <> Todo.read.go(t)

# the list from what was stored ("" or bad JSON: none)
def Todo.read(+text: String) -> List<&2, Todo>:
  Todo.read.go(J.Json.elems(Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{})))

def Todo.done.k(+st: String, +n: Nat) -> Nat:
  Bool.pick(Nat, String.eq(st, "completed"), 1n+n, n)

# how many steps are completed
def Todo.done(ts: List<&2, Todo>) -> Nat:
  match ts:
    case Nil{}:
      0n
    case Todo{_, _, _, +st} <> t:
      Todo.done.k(st, Todo.done(t))

# whether the list shows: while the turn runs, or while a step is left
def Todo.shown(+ts: List<&2, Todo>, run: Bool) -> Bool:
  Bool.and(Bool.not(List.is_empty(&2, Todo, ts)), Bool.or(run, Nat.is_lt(Todo.done(ts), List.length(&2, Todo, ts))))

# a step as a line: [x] done, [>] being done (in its active form, when it
# has one), [ ] to do
def Todo.line(t: Todo) -> String:
  match t:
    case Todo{_, +text, +f, +st}:
      Bool.pick(String, String.eq(st, "completed"), "[x] " ++ text,
        Bool.pick(String, String.eq(st, "in_progress"), "[>] " ++ Todo.or(f, text), "[ ] " ++ text))

# What the card shows
# -------------------
# A long list does not fit at the top of the thread, and its first steps are
# mostly done: the card shows `Todo.fit()` steps from the one before the
# first step not done, and a line each for the done steps before them and
# the steps after them (laws todo_rows_*). Every client draws these rows.

# a row of the card: a step's line, or a note for steps left out (its
# status "completed" for done ones, "pending" for the rest)
type TRow is Data:
  TRow{text: String, status: String}

def TRow.text(r: TRow) -> String:
  match r:
    case TRow{t, _}:
      t

def TRow.status(r: TRow) -> String:
  match r:
    case TRow{_, s}:
      s

# steps shown at once
def Todo.fit() -> Nat:
  6n

def Todo.open.k(done: Bool, +n: Nat) -> Nat:
  match done:
    case True{}:
      1n+n
    case False{}:
      0n

# how many steps come before the first one not completed
def Todo.open(ts: List<&2, Todo>) -> Nat:
  match ts:
    case Nil{}:
      0n
    case Todo{_, _, _, +st} <> t:
      Todo.open.k(String.eq(st, "completed"), Todo.open(t))

def Todo.rows.of(ts: List<&2, Todo>) -> List<&2, TRow>:
  match ts:
    case Nil{}:
      Nil{}
    case +t <> rest:
      TRow{Todo.line(t), Todo.status(t)} <> Todo.rows.of(rest)

def Todo.note(+n: Nat, +text: String, +st: String) -> List<&2, TRow>:
  Bool.pick(List<&2, TRow>, Nat.is_eq(n, 0n), Nil{}, [TRow{"… " ++ Nat.show(n) ++ " " ++ text, st}])

# the first step shown: one done step before the first open one, and never
# so late that fewer than fit steps follow
def Todo.first(+open: Nat, +len: Nat, +fit: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_lt(Nat.sub(open, 1n), Nat.sub(len, fit)), Nat.sub(open, 1n), Nat.sub(len, fit))

def Todo.rows.cut(ts: List<&2, Todo>, +s: Nat, +len: Nat, +fit: Nat) -> List<&2, TRow>:
  List.concat(&2, TRow, [Todo.note(s, "done", "completed"), Todo.rows.of(List.take(&2, Todo, List.drop(&2, Todo, ts, s), fit)),
    Todo.note(Nat.sub(Nat.sub(len, s), fit), "more", "pending")])

def Todo.rows.k(long: Bool, ts: List<&2, Todo>, +len: Nat, +fit: Nat, +open: Nat) -> List<&2, TRow>:
  match long:
    case True{}:
      Todo.rows.cut(ts, Todo.first(open, len, fit), len, fit)
    case False{}:
      Todo.rows.of(ts)

# the card's rows below its head
def Todo.rows(+ts: List<&2, Todo>) -> List<&2, TRow>:
  Todo.rows.k(Nat.is_gt(List.length(&2, Todo, ts), Todo.fit()), ts, List.length(&2, Todo, ts), Todo.fit(), Todo.open(ts))

# Identity-based three-way merge. A repeated snapshot is not an instruction
# to undo the person's text, status, deletion or ordering.
def Todo.id(t: Todo) -> String:
  match t:
    case Todo{i, _, _, _}: i

def Todo.find.k(hit: Bool, h: Todo, rest: Maybe<&2, Todo>) -> Maybe<&2, Todo>:
  match hit:
    case True{}: Some{h}
    case False{}: rest

def Todo.find(ts: List<&2, Todo>, +id: String) -> Maybe<&2, Todo>:
  match ts:
    case Nil{}: None{}
    case +h <> t:
      Todo.find.k(String.eq(Todo.id(h), id), h, Todo.find(t, id))

def Todo.same(a: Todo, b: Todo) -> Bool:
  match a b:
    case Todo{i, x, f, s} Todo{j, y, g, t}:
      Bool.and(String.eq(i, j), Bool.and(String.eq(x, y), Bool.and(String.eq(f, g), String.eq(s, t))))

def Todo.field(+old: String, +human: String, +agent: String) -> String:
  Bool.pick(String, String.eq(old, agent), human, agent)

def Todo.merge.row(old: Maybe<&2, Todo>, human: Todo, agent: Todo) -> Todo:
  match old human agent:
    case Some{Todo{_, x, f, s}} Todo{_, y, g, t} Todo{i, z, h, u}:
      Todo{i, Todo.field(x, y, z), Todo.field(f, g, h), Todo.field(s, t, u)}
    case None{} _ a: a

def Todo.merge.keep(old: Maybe<&2, Todo>, human: Todo) -> List<&2, Todo>:
  match old:
    case None{}: [human]
    case Some{_}: Nil{}

def Todo.merge.current.row(agent: Maybe<&2, Todo>, +old: Maybe<&2, Todo>, human: Todo) -> List<&2, Todo>:
  match agent:
    case Some{a}: [Todo.merge.row(old, human, a)]
    case None{}: Todo.merge.keep(old, human)

# Walk the person's current order, retaining person-created items.
def Todo.merge.current(current: List<&2, Todo>, +base: List<&2, Todo>, +next: List<&2, Todo>) -> List<&2, Todo>:
  match current:
    case Nil{}: Nil{}
    case +h <> t:
      List.append(&2, Todo, Todo.merge.current.row(Todo.find(next, Todo.id(h)), Todo.find(base, Todo.id(h)), h), Todo.merge.current(t, base, next))

def Todo.merge.missing(old: Maybe<&2, Todo>, +agent: Todo) -> List<&2, Todo>:
  match old:
    case None{}: [agent]
    case Some{b}: Bool.pick(List<&2, Todo>, Todo.same(b, agent), Nil{}, [agent])

def Todo.merge.new.row(exists: Maybe<&2, Todo>, old: Maybe<&2, Todo>, agent: Todo) -> List<&2, Todo>:
  match exists:
    case Some{_}: Nil{}
    case None{}: Todo.merge.missing(old, agent)

def Todo.merge.new(next: List<&2, Todo>, +base: List<&2, Todo>, +current: List<&2, Todo>) -> List<&2, Todo>:
  match next:
    case Nil{}: Nil{}
    case +h <> t:
      List.append(&2, Todo, Todo.merge.new.row(Todo.find(current, Todo.id(h)), Todo.find(base, Todo.id(h)), h), Todo.merge.new(t, base, current))

def Todo.merge(+base: List<&2, Todo>, +current: List<&2, Todo>, +next: List<&2, Todo>) -> List<&2, Todo>:
  List.append(&2, Todo, Todo.merge.current(current, base, next), Todo.merge.new(next, base, current))

def Todo.remove.k(hit: Bool, h: Todo, rest: List<&2, Todo>) -> List<&2, Todo>:
  match hit:
    case True{}: rest
    case False{}: h <> rest

def Todo.remove(ts: List<&2, Todo>, +id: String) -> List<&2, Todo>:
  match ts:
    case Nil{}: Nil{}
    case +h <> t:
      Todo.remove.k(String.eq(Todo.id(h), id), h, Todo.remove(t, id))

def Todo.insert(ts: List<&2, Todo>, n: Nat, h: Todo) -> List<&2, Todo>:
  match ts n:
    case Nil{} _: [h]
    case rest 0n: h <> rest
    case a <> t 1n+k: a <> Todo.insert(t, k, h)

def Todo.move.found(m: Maybe<&2, Todo>, ts: List<&2, Todo>, id: String, n: Nat) -> List<&2, Todo>:
  match m:
    case None{}: ts
    case Some{h}: Todo.insert(Todo.remove(ts, id), n, h)

def Todo.edit.k(op: String, +ts: List<&2, Todo>, +p: J.Json) -> List<&2, Todo>:
  match op:
    case "add":
      List.append(&2, Todo, ts, [Todo{J.Json.get_str(p, "id", ""), J.Json.get_str(p, "text", ""), "", "pending"}])
    case "delete": Todo.remove(ts, J.Json.get_str(p, "id", ""))
    case "move": Todo.move.found(Todo.find(ts, J.Json.get_str(p, "id", "")), ts, J.Json.get_str(p, "id", ""), J.Json.get_nat(p, "to", 0n))
    case "set":
      Todo.update(ts, J.Json.obj([J.KV{"taskId", J.Str{J.Json.get_str(p, "id", "")}}, J.KV{"subject", J.Str{J.Json.get_str(p, "text", "")}}, J.KV{"status", J.Str{J.Json.get_str(p, "status", "")}}]))
    case _: ts

def Todo.edit(+ts: List<&2, Todo>, +p: J.Json) -> List<&2, Todo>:
  Todo.edit.k(J.Json.get_str(p, "op", ""), ts, p)

def Todo.complete(+ts: List<&2, Todo>) -> Bool:
  Bool.and(Bool.not(List.is_empty(&2, Todo, ts)), Nat.is_eq(Todo.done(ts), List.length(&2, Todo, ts)))

# how far it got: "2/5", the steps completed of all
def Todo.count(+ts: List<&2, Todo>) -> String:
  Bool.pick(String, Todo.complete(ts), "done", Nat.show(Todo.done(ts)) ++ "/" ++ Nat.show(List.length(&2, Todo, ts)))

# the card's head: "Todo 2/5"
def Todo.head(+ts: List<&2, Todo>) -> String:
  "Todo " ++ Todo.count(ts)

# Every UI operation carries its originating thread, never the selection at
# delivery time. These values are shared by the web and native phones.
def Todo.ask(+th: String, +op: String, +id: String, +text: String, +status: String, +to: Nat) -> String:
  J.Json.show(J.Json.obj([J.KV{"thread", J.Str{th}}, J.KV{"op", J.Str{op}}, J.KV{"id", J.Str{id}},
    J.KV{"text", J.Str{text}}, J.KV{"status", J.Str{status}}, J.KV{"to", J.Num{Nat.show(to)}}]))

def Todo.check(+th: String, +t: Todo) -> String:
  Todo.ask(th, "set", Todo.id(t), "", Bool.pick(String, String.eq(Todo.status(t), "completed"), "pending", "completed"), 0n)

def Todo.text(t: Todo) -> String:
  match t:
    case Todo{_, x, _, _}: x

type TPlan is Data:
  TPlan{base: List<&2, Todo>, current: List<&2, Todo>, fresh: Bool}

def TPlan.replace(+base: List<&2, Todo>, +current: List<&2, Todo>, +next: List<&2, Todo>) -> TPlan:
  TPlan{next, Todo.merge(base, current, next), True{}}

def TPlan.create(fresh: Bool, +base: List<&2, Todo>, +current: List<&2, Todo>, use: String, input: J.Json) -> TPlan:
  match fresh:
    case True{}: TPlan.replace(base, current, Todo.create(base, use, input))
    case False{}: TPlan.replace(base, current, Todo.create(Nil{}, use, input))

def Todo.optional(t: Maybe<&2, Todo>) -> List<&2, Todo>:
  match t:
    case Some{h}: [h]
    case None{}: Nil{}

# TaskUpdate names fields explicitly, including a value equal to the last
# agent snapshot. Apply those fields even after a person's differing edit.
def Todo.explicit.found(found: Maybe<&2, Todo>, +ts: List<&2, Todo>, +base: List<&2, Todo>, +input: J.Json) -> List<&2, Todo>:
  match found:
    case Some{_}: Todo.update(ts, input)
    case None{}:
      Todo.update(List.append(&2, Todo, ts, Todo.optional(Todo.find(base, J.Json.get_str(input, "taskId", "")))), input)

def Todo.explicit(+ts: List<&2, Todo>, +base: List<&2, Todo>, +input: J.Json) -> List<&2, Todo>:
  Todo.explicit.found(Todo.find(ts, J.Json.get_str(input, "taskId", "")), ts, base, input)

def TPlan.step(name: String, +b: J.Json, p: TPlan) -> TPlan:
  match name p:
    case "TodoWrite" TPlan{+base, +current, _}: TPlan.replace(base, current, Todo.write(J.Json.get_obj(b, "input")))
    case "TaskCreate" TPlan{base, current, fresh}: TPlan.create(fresh, base, current, J.Json.get_str(b, "id", ""), J.Json.get_obj(b, "input"))
    case "TaskUpdate" TPlan{+base, +current, _}:
      TPlan{Todo.update(base, J.Json.get_obj(b, "input")), Todo.explicit(current, base, J.Json.get_obj(b, "input")), True{}}
    case "TaskNamed" TPlan{base, current, fresh}:
      TPlan{Todo.named(base, J.Json.get_str(b, "use", ""), J.Json.get_str(b, "number", "")),
        Todo.named(current, J.Json.get_str(b, "use", ""), J.Json.get_str(b, "number", "")), fresh}
    case _ _: p

def TPlan.tools(bs: List<&2, J.Json>, p: TPlan) -> TPlan:
  match bs:
    case Nil{}: p
    case +b <> t: TPlan.tools(t, TPlan.step(J.Json.get_str(b, "name", ""), b, p))
