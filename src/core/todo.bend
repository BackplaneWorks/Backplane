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

# how far it got: "2/5", the steps completed of all
def Todo.count(+ts: List<&2, Todo>) -> String:
  Nat.show(Todo.done(ts)) ++ "/" ++ Nat.show(List.length(&2, Todo, ts))

# the card's head: "Todo 2/5"
def Todo.head(+ts: List<&2, Todo>) -> String:
  "Todo " ++ Todo.count(ts)

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
