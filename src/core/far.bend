import Base
import ./json.bend as J
import ./model.bend as M
import ./codec.bend as C
import ./bot.bend as BT

# Threads that live on a linked machine (docs/bots.md, "Threads elsewhere").
#
# A hub shares its bots and their threads with the owner's other machines
# it is linked to. The owning hub stays the authority: it pushes each
# shared change, as its own log numbers it, to every such peer (POST
# /far/push), and a peer that missed some asks for them (POST /far/log,
# paged). The peer keeps what it got (its mirror, one per link) and hands
# it to its clients, which fold it into their read model under ids of
# their own: a thing that lives elsewhere is named by the link and its
# id there ("<link>~<id>", Fr.id), so nothing here can be mistaken for it
# and nothing shows twice. What a client does to such a thread (write,
# stop, answer) goes back to its hub (Fr.route, POST /far/rpc).
#
# Order and loss: a batch says which log number it follows (`since`) and
# where it ends (`to`). A mirror takes a batch only when it follows what it
# has (since <= head), and only the items it lacks (n > head); a batch past
# a gap changes nothing and asks for what is missing (Fr.take). So a
# batch that comes twice, late, or out of order is harmless, and a restart
# or a partition on either side ends in one pull.

# Ids
# ---

# a thing on the machine a link reaches, named here
def Fr.id(+link: String, +id: String) -> String:
  link ++ "~" ++ id

# the same, keeping "" (no project, no parent) as it is
def Fr.opt(+link: String, +id: String) -> String:
  Bool.pick(String, String.is_empty(id), "", Fr.id(link, id))

# whether an id names a thing elsewhere (local ids never hold a "~")
def Fr.is(+id: String) -> Bool:
  String.contains(id, "~")

def Fr.link.k(ps: List<&2, String>) -> String:
  match ps:
    case l <> _ <> _:
      l
    case _:
      ""

# the link an id elsewhere lives behind ("" for an id here)
def Fr.link(+id: String) -> String:
  Fr.link.k(String.split(id, '~'))

def Fr.local.k(ps: List<&2, String>, +id: String) -> String:
  match ps:
    case _ <> +t <> +r:
      String.join(t <> r, "~")
    case _:
      id

# its id on its own machine (an id here is itself)
def Fr.local(+id: String) -> String:
  Fr.local.k(String.split(id, '~'), id)

# The thread that sent a message from another thread, as a machine that
# mirrors it names it: one of that machine's own ("<link>~<id>" there, the
# link being this one) is its own again, one of the owner's is named
# through the link. A message's id carries its sender (M.Link.id:
# "t:<from>:<n>"), so the mirror's "from" row opens the right thread.
def Fr.sender(+l: String, +from: String) -> String:
  Bool.pick(String, Fr.is(from), Bool.pick(String, String.eq(Fr.link(from), l), Fr.local(from), from), Fr.id(l, from))

def Fr.sent.k(+l: String, +from: String, +msg: String) -> String:
  Bool.pick(String, String.is_empty(from), msg, M.Link.id(Fr.sender(l, from), String.drop(msg, Nat.add(3n, String.length(from)))))

def Fr.sent(+l: String, +msg: String) -> String:
  Fr.sent.k(l, M.Link.from(msg), msg)

# Changes, as a client here reads them
# ------------------------------------
# The owning hub's change renamed into this hub's ids. Only what shows a
# thread, a bot and its space goes; settings, devices, links, webhooks,
# rooms (shared rooms travel on their own), ordering and usage stay home
# (None).
def Fr.change(+l: String, c: M.Change) -> Maybe<&2, M.Change>:
  match c:
    case M.ProjectCreated{+id, title, root, at}:
      Some{M.ProjectCreated{Fr.id(l, id), title, root, at}}
    case M.ProjectRemoved{+id, at}:
      Some{M.ProjectRemoved{Fr.id(l, id), at}}
    case M.ThreadCreated{+id, +project, title, env, branch, worktree, provider, model, at}:
      Some{M.ThreadCreated{Fr.id(l, id), Fr.opt(l, project), title, env, branch, worktree, provider, model, at}}
    case M.ThreadRenamed{+id, title, at}:
      Some{M.ThreadRenamed{Fr.id(l, id), title, at}}
    case M.MessagePosted{+thread, +msg, role, text, at}:
      Some{M.MessagePosted{Fr.id(l, thread), Fr.sent(l, msg), role, text, at}}
    case M.TurnChanged{+thread, state, at}:
      Some{M.TurnChanged{Fr.id(l, thread), state, at}}
    case M.PendingChanged{+thread, count, at}:
      Some{M.PendingChanged{Fr.id(l, thread), count, at}}
    case M.ActivityLogged{+thread, act, tone, kind, summary, at}:
      Some{M.ActivityLogged{Fr.id(l, thread), act, tone, kind, summary, at}}
    case M.ThreadSettled{+id, manual, at}:
      Some{M.ThreadSettled{Fr.id(l, id), manual, at}}
    case M.ThreadUnsettled{+id, at}:
      Some{M.ThreadUnsettled{Fr.id(l, id), at}}
    case M.ThreadArchived{+id, b, at}:
      Some{M.ThreadArchived{Fr.id(l, id), b, at}}
    case M.ThreadPinned{+id, b, at}:
      Some{M.ThreadPinned{Fr.id(l, id), b, at}}
    case M.ThreadForked{+id, +src, at}:
      Some{M.ThreadForked{Fr.id(l, id), Fr.id(l, src), at}}
    case M.ThreadDeleted{+id, at}:
      Some{M.ThreadDeleted{Fr.id(l, id), at}}
    case M.ModesSet{+id, runtime, interaction, model, at}:
      Some{M.ModesSet{Fr.id(l, id), runtime, interaction, model, at}}
    case M.TaskDelegated{+task, +parent, +child, depth, provider, model, at}:
      Some{M.TaskDelegated{Fr.id(l, task), Fr.opt(l, parent), Fr.opt(l, child), depth, provider, model, at}}
    case M.TaskStateSet{+task, state, summary, at}:
      Some{M.TaskStateSet{Fr.id(l, task), state, summary, at}}
    case M.TaskDelivered{+task, at}:
      Some{M.TaskDelivered{Fr.id(l, task), at}}
    case M.TurnQueued{+thread, msg, text, at}:
      Some{M.TurnQueued{Fr.id(l, thread), msg, text, at}}
    case M.QueueTaken{+thread, at}:
      Some{M.QueueTaken{Fr.id(l, thread), at}}
    case M.AskOpened{+id, +thread, kind, tool, detail, options, at}:
      Some{M.AskOpened{Fr.id(l, id), Fr.id(l, thread), kind, tool, detail, options, at}}
    case M.AskClosed{+id, answer, at}:
      Some{M.AskClosed{Fr.id(l, id), answer, at}}
    case M.QueueEdited{+thread, msg, text, at}:
      Some{M.QueueEdited{Fr.id(l, thread), msg, text, at}}
    case M.QueueRemoved{+thread, msg, at}:
      Some{M.QueueRemoved{Fr.id(l, thread), msg, at}}
    case M.QueueMoved{+thread, msg, to, at}:
      Some{M.QueueMoved{Fr.id(l, thread), msg, to, at}}
    case M.QueueTakenOne{+thread, msg, at}:
      Some{M.QueueTakenOne{Fr.id(l, thread), msg, at}}
    case M.ProviderSet{+id, provider, model, at}:
      Some{M.ProviderSet{Fr.id(l, id), provider, model, at}}
    case M.TodosSet{+thread, todos, at}:
      Some{M.TodosSet{Fr.id(l, thread), todos, at}}
    case M.BotSet{+id, name, +thread, look, persona, at}:
      Some{M.BotSet{Fr.id(l, id), name, Fr.id(l, thread), look, persona, at}}
    case M.BotDeleted{+id, at}:
      Some{M.BotDeleted{Fr.id(l, id), at}}
    case M.RoutineSet{+id, +bot, name, cron, prompt, on, at}:
      Some{M.RoutineSet{Fr.id(l, id), Fr.id(l, bot), name, cron, prompt, on, at}}
    case M.RoutineDeleted{+id, at}:
      Some{M.RoutineDeleted{Fr.id(l, id), at}}
    case M.RoutineFired{+id, at}:
      Some{M.RoutineFired{Fr.id(l, id), at}}
    case M.MemorySaved{+bot, key, kind, text, tags, at}:
      Some{M.MemorySaved{Fr.id(l, bot), key, kind, text, tags, at}}
    case M.MemoryForgot{+bot, key, at}:
      Some{M.MemoryForgot{Fr.id(l, bot), key, at}}
    case M.TaskSet{+bot, slug, json, at}:
      Some{M.TaskSet{Fr.id(l, bot), slug, json, at}}
    case M.TaskDeleted{+bot, slug, at}:
      Some{M.TaskDeleted{Fr.id(l, bot), slug, at}}
    case M.SpaceSet{+bot, spec, at}:
      Some{M.SpaceSet{Fr.id(l, bot), spec, at}}
    case M.SpacePageSet{+bot, html, at}:
      Some{M.SpacePageSet{Fr.id(l, bot), html, at}}
    case M.WorkSet{+thread, kind, rows, at}:
      Some{M.WorkSet{Fr.id(l, thread), kind, rows, at}}
    case M.ThreadLaunched{+id, +by, report, at}:
      Some{M.ThreadLaunched{Fr.id(l, id), Fr.sender(l, by), report, at}}
    case _:
      None{}

# What the owning hub shares
# --------------------------
# Its projects, bots and threads, shared only over an owner-trusted link
# by the server. Settings, devices, links, hooks and rooms stay home.

def Fr.st.threads(s: M.State) -> Map<&2, M.Thread>:
  match s:
    case M.State{_, ts, _, _, _, _, _, _, _, _, _}:
      ts

def Fr.st.asks(s: M.State) -> Map<&2, M.Ask>:
  match s:
    case M.State{_, _, _, _, _, _, _, ak, _, _, _}:
      ak

def Fr.st.tasks(s: M.State) -> Map<&2, M.Task>:
  match s:
    case M.State{_, _, _, _, tk, _, _, _, _, _, _}:
      tk

def Fr.thread.project(t: M.Thread) -> String:
  match t:
    case M.Thread{_, p, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      p

def Fr.got.thread(r: Map<&2, M.Thread> & M.Thread) -> M.Thread:
  (_, t) = r
  t

# A thread owned here. Mirrors are client projections, never re-exported.
def Fr.has(r: Map<&2, M.Thread> & Bool) -> Bool:
  (_, b) = r
  b

def Fr.thread.shared(+s: M.State, +th: String) -> Bool:
  Bool.and(Fr.has(Map.has(&2, M.Thread, Fr.st.threads(s), th)),
    Bool.not(Fr.is(th)))

def Fr.ask.thread(r: Map<&2, M.Ask> & M.Ask) -> String:
  match r:
    case (_, M.Ask{_, t, _, _, _, _, _, _}):
      t

def Fr.task.parent(r: Map<&2, M.Task> & M.Task) -> String:
  match r:
    case (_, M.Task{_, p, _, _, _, _, _, _, _}):
      p

def Fr.ask.shared(+s: M.State, +id: String) -> Bool:
  Fr.thread.shared(s, Fr.ask.thread(Map.get(M.Ask, M.Ask{id, "", "", "", "", "", False{}, ""}, Fr.st.asks(s), id)))

def Fr.task.shared(+s: M.State, +id: String) -> Bool:
  Fr.thread.shared(s, Fr.task.parent(Map.get(M.Task, M.Task.blank(id), Fr.st.tasks(s), id)))

# whether a change (with the state after it) goes to the owner's other
# machines
def Fr.shared(+s: M.State, c: M.Change) -> Bool:
  match c:
    case M.ProjectCreated{+id, _, _, _}:
      Bool.not(Fr.is(id))
    case M.ProjectRemoved{+id, _}:
      Bool.not(Fr.is(id))
    case M.ThreadCreated{+id, _, _, _, _, _, _, _, _}:
      Fr.thread.shared(s, id)
    case M.ThreadRenamed{+id, _, _}:
      Fr.thread.shared(s, id)
    case M.MessagePosted{+thread, _, _, _, _}:
      Fr.thread.shared(s, thread)
    case M.TurnChanged{+thread, _, _}:
      Fr.thread.shared(s, thread)
    case M.PendingChanged{+thread, _, _}:
      Fr.thread.shared(s, thread)
    case M.ActivityLogged{+thread, _, _, _, _, _}:
      Fr.thread.shared(s, thread)
    case M.ThreadSettled{+id, _, _}:
      Fr.thread.shared(s, id)
    case M.ThreadUnsettled{+id, _}:
      Fr.thread.shared(s, id)
    case M.ThreadArchived{+id, _, _}:
      Fr.thread.shared(s, id)
    case M.ThreadPinned{+id, _, _}:
      Fr.thread.shared(s, id)
    case M.ThreadForked{+id, _, _}:
      Fr.thread.shared(s, id)
    case M.ThreadDeleted{+id, _}:
      Fr.thread.shared(s, id)
    case M.ModesSet{+id, _, _, _, _}:
      Fr.thread.shared(s, id)
    case M.TaskDelegated{_, +parent, _, _, _, _, _}:
      Fr.thread.shared(s, parent)
    case M.TaskStateSet{+task, _, _, _}:
      Fr.task.shared(s, task)
    case M.TaskDelivered{+task, _}:
      Fr.task.shared(s, task)
    case M.TurnQueued{+thread, _, _, _}:
      Fr.thread.shared(s, thread)
    case M.QueueTaken{+thread, _}:
      Fr.thread.shared(s, thread)
    case M.AskOpened{_, +thread, _, _, _, _, _}:
      Fr.thread.shared(s, thread)
    case M.AskClosed{+id, _, _}:
      Fr.ask.shared(s, id)
    case M.QueueEdited{+thread, _, _, _}:
      Fr.thread.shared(s, thread)
    case M.QueueRemoved{+thread, _, _}:
      Fr.thread.shared(s, thread)
    case M.QueueMoved{+thread, _, _, _}:
      Fr.thread.shared(s, thread)
    case M.QueueTakenOne{+thread, _, _}:
      Fr.thread.shared(s, thread)
    case M.ProviderSet{+id, _, _, _}:
      Fr.thread.shared(s, id)
    case M.TodosSet{+thread, _, _}:
      Fr.thread.shared(s, thread)
    case M.BotSet{_, _, _, _, _, _}:
      True{}
    case M.BotDeleted{_, _}:
      True{}
    case M.RoutineSet{_, _, _, _, _, _, _}:
      True{}
    case M.RoutineDeleted{_, _}:
      True{}
    case M.RoutineFired{_, _}:
      True{}
    case M.MemorySaved{_, _, _, _, _, _}:
      True{}
    case M.MemoryForgot{_, _, _}:
      True{}
    case M.TaskSet{_, _, _, _}:
      True{}
    case M.TaskDeleted{_, _, _}:
      True{}
    case M.SpaceSet{_, _, _}:
      True{}
    case M.SpacePageSet{_, _, _}:
      True{}
    case M.WorkSet{+thread, _, _, _}:
      Fr.thread.shared(s, thread)
    case M.ThreadLaunched{+id, _, _, _}:
      Fr.thread.shared(s, id)
    case _:
      False{}

# The project a shared change belongs to ("" for none: bots, routines,
# memory, space), with the state after it: the project itself, or the
# project of the thread it is about. A mirror knows a project is complete
# once it holds the last change that has it as its scope (Fr.catalog).
def Fr.thread.of(r: Map<&2, M.Thread> & M.Thread) -> String:
  (_, t) = r
  Fr.thread.project(t)

def Fr.scope.th(+s: M.State, +th: String) -> String:
  Fr.thread.of(Map.get(M.Thread, M.Thread.blank(th), Fr.st.threads(s), th))

def Fr.scope.ask(+s: M.State, +id: String) -> String:
  Fr.scope.th(s, Fr.ask.thread(Map.get(M.Ask, M.Ask{id, "", "", "", "", "", False{}, ""}, Fr.st.asks(s), id)))

def Fr.scope.task(+s: M.State, +id: String) -> String:
  Fr.scope.th(s, Fr.task.parent(Map.get(M.Task, M.Task.blank(id), Fr.st.tasks(s), id)))

def Fr.scope(+s: M.State, c: M.Change) -> String:
  match c:
    case M.ProjectCreated{+id, _, _, _}:
      id
    case M.ProjectRemoved{+id, _}:
      id
    case M.ThreadCreated{+id, _, _, _, _, _, _, _, _}:
      Fr.scope.th(s, id)
    case M.ThreadRenamed{+id, _, _}:
      Fr.scope.th(s, id)
    case M.MessagePosted{+thread, _, _, _, _}:
      Fr.scope.th(s, thread)
    case M.TurnChanged{+thread, _, _}:
      Fr.scope.th(s, thread)
    case M.PendingChanged{+thread, _, _}:
      Fr.scope.th(s, thread)
    case M.ActivityLogged{+thread, _, _, _, _, _}:
      Fr.scope.th(s, thread)
    case M.ThreadSettled{+id, _, _}:
      Fr.scope.th(s, id)
    case M.ThreadUnsettled{+id, _}:
      Fr.scope.th(s, id)
    case M.ThreadArchived{+id, _, _}:
      Fr.scope.th(s, id)
    case M.ThreadPinned{+id, _, _}:
      Fr.scope.th(s, id)
    case M.ThreadForked{+id, _, _}:
      Fr.scope.th(s, id)
    case M.ThreadDeleted{+id, _}:
      Fr.scope.th(s, id)
    case M.ModesSet{+id, _, _, _, _}:
      Fr.scope.th(s, id)
    case M.TaskDelegated{_, +parent, _, _, _, _, _}:
      Fr.scope.th(s, parent)
    case M.TaskStateSet{+task, _, _, _}:
      Fr.scope.task(s, task)
    case M.TaskDelivered{+task, _}:
      Fr.scope.task(s, task)
    case M.TurnQueued{+thread, _, _, _}:
      Fr.scope.th(s, thread)
    case M.QueueTaken{+thread, _}:
      Fr.scope.th(s, thread)
    case M.AskOpened{_, +thread, _, _, _, _, _}:
      Fr.scope.th(s, thread)
    case M.AskClosed{+id, _, _}:
      Fr.scope.ask(s, id)
    case M.QueueEdited{+thread, _, _, _}:
      Fr.scope.th(s, thread)
    case M.QueueRemoved{+thread, _, _}:
      Fr.scope.th(s, thread)
    case M.QueueMoved{+thread, _, _, _}:
      Fr.scope.th(s, thread)
    case M.QueueTakenOne{+thread, _, _}:
      Fr.scope.th(s, thread)
    case M.ProviderSet{+id, _, _, _}:
      Fr.scope.th(s, id)
    case M.TodosSet{+thread, _, _}:
      Fr.scope.th(s, thread)
    case M.WorkSet{+thread, _, _, _}:
      Fr.scope.th(s, thread)
    case M.ThreadLaunched{+id, _, _, _}:
      Fr.scope.th(s, id)
    case _:
      ""

# Each project's last shared change, as the owning hub numbers it
# (project id → n). Lines are walked newest first at start, so a project
# keeps the first number it meets; a commit's are newer than all of them.
def Fr.nat.got(r: Map<&2, Nat> & Nat) -> Nat:
  (_, n) = r
  n

def Fr.str.got(r: Map<&2, String> & String) -> String:
  (_, v) = r
  v

def Fr.last.put(+p: String, +n: Nat, +m: Map<&2, Nat>) -> Map<&2, Nat>:
  Bool.pick(Map<&2, Nat>, Bool.or(String.is_empty(p), Nat.is_ge(Fr.nat.got(Map.get(Nat, 0n, m, p)), n)), m, Map.set(&2, Nat, m, p, n))

# a commit's changes (oldest first) numbered after `seq`
def Fr.lasts.add(cs: List<&2, M.Change>, +seq: Nat, +s: M.State, +m: Map<&2, Nat>) -> Map<&2, Nat>:
  match cs:
    case Nil{}:
      m
    case +c <> t:
      Fr.lasts.add(t, 1n+seq, s, Bool.pick(Map<&2, Nat>, Fr.shared(s, c), Fr.last.put(Fr.scope(s, c), 1n+seq, m), m))

# the larger of two maps' numbers, key by key
def Fr.lasts.merge(ks: List<&2, String>, +from: Map<&2, Nat>, m: Map<&2, Nat>) -> Map<&2, Nat>:
  match ks:
    case Nil{}:
      m
    case +k <> t:
      Fr.lasts.merge(t, from, Fr.last.put(k, Fr.nat.got(Map.get(Nat, 0n, from, k)), m))

# What a hub keeps beside its mirrors and pushes: its own projects' last
# numbers (lasts), and the newest catalog each link sent (link → its JSON
# text), for clients that join while a mirror is catching up
type Cx is Data:
  Cx{lasts: Map<&2, Nat>, cats: Map<&2, String>, ub: Map<&2, String>}

def Cx.empty() -> Cx:
  Cx{Map.new(&2, Nat), Map.new(&2, String), Map.new(&2, String)}

# The catalog: every project this hub shares, so a mirror can list them
# all at once, in their place, before their threads arrive: its title,
# its latest activity (how the sidebar ranks it) and the number of its
# last shared change (until the mirror holds that, it is catching up)
def Fr.acts(ts: List<&2, M.Thread>, +m: Map<&2, Nat>) -> Map<&2, Nat>:
  match ts:
    case Nil{}:
      m
    case M.Thread{_, +p, _, _, +a, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _} <> t:
      Fr.acts(t, Bool.pick(Map<&2, Nat>, Nat.is_ge(Fr.nat.got(Map.get(Nat, 0n, m, p)), a), m, Map.set(&2, Nat, m, p, a)))

def Fr.cat.one(skip: Bool, +id: String, +title: String, +at: Nat, +n: Nat, rest: List<&2, J.Json>) -> List<&2, J.Json>:
  match skip:
    case True{}:
      rest
    case False{}:
      J.Json.obj([J.KV{"id", J.Str{id}}, J.KV{"title", J.Str{title}}, J.KV{"at", J.Json.nat(at)}, J.KV{"n", J.Json.nat(n)}]) <> rest

def Fr.cat.go(ps: List<&2, M.Project>, +acts: Map<&2, Nat>, +lasts: Map<&2, Nat>) -> List<&2, J.Json>:
  match ps:
    case Nil{}:
      Nil{}
    case M.Project{+id, +ti, _, +c, +rm} <> t:
      Fr.cat.one(Bool.or(rm, Fr.is(id)), id, ti, M.Time.max(c, Fr.nat.got(Map.get(Nat, 0n, acts, id))), Fr.nat.got(Map.get(Nat, 0n, lasts, id)), Fr.cat.go(t, acts, lasts))

def Fr.st.projects(s: M.State) -> List<&2, M.Project>:
  match s:
    case M.State{ps, _, _, _, _, _, _, _, _, _, _}:
      Map.values(&2, M.Project, ps)

def Fr.catalog(+s: M.State, +lasts: Map<&2, Nat>) -> J.Json:
  J.Json.arr(Fr.cat.go(Fr.st.projects(s), Fr.acts(Map.values(&2, M.Thread, Fr.st.threads(s)), Map.new(&2, Nat)), lasts))

# Batches
# -------
# An item is a change as its own hub numbers it (its place in that log)
# and sends it (its wire JSON, ids its own).

type Item is Data:
  Item{n: Nat, c: J.Json}

def Item.n(i: Item) -> Nat:
  match i:
    case Item{n, _}:
      n

def Item.json(i: Item) -> J.Json:
  match i:
    case Item{n, c}:
      J.Json.obj([J.KV{"n", J.Json.nat(n)}, J.KV{"c", c}])

def Item.of(+o: J.Json) -> Item:
  Item{J.Json.get_nat(o, "n", 0n), Maybe.default(&2, J.Json, J.Json.get(o, "c"), J.Null{})}

def Fr.items.of(os: List<&2, J.Json>) -> List<&2, Item>:
  match os:
    case Nil{}:
      Nil{}
    case +o <> t:
      Item.of(o) <> Fr.items.of(t)

def Fr.items.json(xs: List<&2, Item>) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      Item.json(x) <> Fr.items.json(t)

def Fr.fresh.k(keep: Bool, x: Item, rest: List<&2, Item>) -> List<&2, Item>:
  match keep:
    case True{}:
      x <> rest
    case False{}:
      rest

# the items after head, in order
def Fr.fresh(xs: List<&2, Item>, +head: Nat) -> List<&2, Item>:
  match xs:
    case Nil{}:
      Nil{}
    case Item{+n, c} <> t:
      Fr.fresh.k(Nat.is_gt(n, head), Item{n, c}, Fr.fresh(t, head))

# what a batch leaves: the new head, the items to take, and whether a gap
# came first (then nothing is taken and the rest is asked for)
type Got is Data:
  Got{head: Nat, items: List<&2, Item>, gap: Bool}

def Fr.take.k(gap: Bool, +head: Nat, +to: Nat, xs: List<&2, Item>) -> Got:
  match gap:
    case True{}:
      Got{head, Nil{}, True{}}
    case False{}:
      Got{Bool.pick(Nat, Nat.is_gt(to, head), to, head), Fr.fresh(xs, head), False{}}

# a batch that follows `since` and ends at `to`, for a mirror at `head`
def Fr.take(+head: Nat, +since: Nat, +to: Nat, xs: List<&2, Item>) -> Got:
  Fr.take.k(Nat.is_gt(since, head), head, to, xs)

def Got.head(g: Got) -> Nat:
  match g:
    case Got{h, _, _}:
      h

def Got.items(g: Got) -> List<&2, Item>:
  match g:
    case Got{_, xs, _}:
      xs

def Got.gap(g: Got) -> Bool:
  match g:
    case Got{_, _, x}:
      x

# A client's fold
# ---------------
# A client keeps how far it has each link's changes beside the read model
# (a Nav under "far~<link>", so a fresh log forgets both at once), folds
# in only what it lacks, and never counts those changes in its own log's
# sequence (M.State.apply.go: a reconnect still asks this hub for exactly
# the changes it missed).

def Fr.nav(+link: String) -> String:
  "far~" ++ link

def Fr.head(+s: M.State, +link: String) -> Nat:
  M.Nav.until(M.Nav.get(M.State.navs(s), Fr.nav(link)))

def Fr.head.set(s: M.State, +link: String, +h: Nat) -> M.State:
  M.State.nav((x => M.Nav.snoozed(x, h)), s, Fr.nav(link))

def Fr.apply.k(m: Maybe<&2, M.Change>, s: M.State) -> M.State:
  match m:
    case Some{c}:
      M.State.apply.go(s, c)
    case None{}:
      s

def Fr.apply.c(m: Maybe<&2, M.Change>, +link: String, s: M.State) -> M.State:
  match m:
    case Some{c}:
      Fr.apply.k(Fr.change(link, c), s)
    case None{}:
      s

def Fr.apply(xs: List<&2, Item>, +link: String, s: M.State) -> M.State:
  match xs:
    case Nil{}:
      s
    case Item{_, +c} <> t:
      Fr.apply(t, link, Fr.apply.c(C.Change.decode(c), link, s))

def Fr.fold.k(g: Got, +link: String, s: M.State) -> M.State:
  match g:
    case Got{_, _, True{}}:
      s
    case Got{h, xs, False{}}:
      Fr.head.set(Fr.apply(xs, link, s), link, h)

# a link's batch folded into a client's read model
def Fr.fold(+s: M.State, +link: String, +since: Nat, +to: Nat, xs: List<&2, Item>) -> M.State:
  Fr.fold.k(Fr.take(Fr.head(s, link), since, to, xs), link, s)

# Machine metadata lives beside the cursor in the client projection, so
# every UI can label a row from M.State without a separate info lookup.
def Fr.mark(s: M.State, +link: String, +name: String, +away: Bool) -> M.State:
  M.State.nav((x => M.Nav{M.Nav.until(x), name, Bool.pick(String, away, "away", "")}), s, Fr.nav(link))

def Fr.name(+s: M.State, +id: String) -> String:
  M.Nav.key(M.Nav.get(M.State.navs(s), Fr.nav(Fr.link(id))), "pinned")

def Fr.away(+s: M.State, +id: String) -> Bool:
  Bool.and(Fr.is(id), String.eq(M.Nav.key(M.Nav.get(M.State.navs(s), Fr.nav(Fr.link(id))), "active"), "away"))

# A catalog (Fr.catalog) taken by a client: each project it lists shows
# at once, in its place (a project not here yet stands in with its title
# and latest activity until its own changes come), and is catching up
# while the link's head is short of its last number ("wait~<id>").
def Fr.wait.nav(+id: String) -> String:
  "wait~" ++ id

def Fr.syncing(+s: M.State, +id: String) -> Bool:
  Bool.and(Fr.is(id), Nat.is_gt(M.Nav.until(M.Nav.get(M.State.navs(s), Fr.wait.nav(id))), Fr.head(s, Fr.link(id))))

def Fr.has.project(r: Map<&2, M.Project> & Bool) -> Bool:
  (_, b) = r
  b

def Fr.st.has.project(s: M.State, +id: String) -> Bool:
  match s:
    case M.State{ps, _, _, _, _, _, _, _, _, _, _}:
      Fr.has.project(Map.has(&2, M.Project, ps, id))

# A project owned here (not a mirror of one elsewhere)
def Fr.project.shared(+s: M.State, +pid: String) -> Bool:
  Bool.and(Bool.and(Bool.not(String.is_empty(pid)), Fr.st.has.project(s, pid)), Bool.not(Fr.is(pid)))

def Fr.stand.in(here: Bool, s: M.State, +id: String, +title: String, +at: Nat) -> M.State:
  match here:
    case True{}:
      s
    case False{}:
      M.State.put_project(s, M.Project{id, title, "", at, False{}})

def Fr.cat.take(+s: M.State, +id: String, +title: String, +at: Nat, +n: Nat) -> M.State:
  M.State.nav((x => M.Nav.snoozed(x, n)), Fr.stand.in(Fr.st.has.project(s, id), s, id, title, at), Fr.wait.nav(id))

def Fr.cat.apply(es: List<&2, J.Json>, +link: String, +s: M.State) -> M.State:
  match es:
    case Nil{}:
      s
    case +e <> t:
      Fr.cat.apply(t, link, Fr.cat.take(s, Fr.id(link, J.Json.get_str(e, "id", "")), J.Json.get_str(e, "title", ""), J.Json.get_nat(e, "at", 0n), J.Json.get_nat(e, "n", 0n)))

# whether any of a link's projects is still catching up, and how many
def Fr.cat.count(es: List<&2, J.Json>, +link: String, +s: M.State, +acc: Nat) -> Nat:
  match es:
    case Nil{}:
      acc
    case +e <> t:
      Fr.cat.count(t, link, s, Bool.pick(Nat, Fr.syncing(s, Fr.id(link, J.Json.get_str(e, "id", ""))), 1n+acc, acc))

# link "L" (machine "desk") at head h, its catalog one project "p1"
# ("Board") whose last number is 5 (for laws)
def Fr.cat.demo(+h: Nat) -> M.State:
  Fr.cat.take(Fr.head.set(Fr.mark(M.State.empty(), "L", "desk", False{}), "L", h), "L~p1", "Board", 7n, 5n)

def Fr.title.tag(+s: M.State, +id: String) -> String:
  Bool.pick(String, Fr.away(s, id), " (away)", Bool.pick(String, Fr.syncing(s, id), " (syncing)", ""))

def Fr.title.k(far: Bool, +s: M.State, +id: String, title: String) -> String:
  match far:
    case False{}:
      title
    case True{}:
      title ++ " · " ++ Bool.pick(String, String.is_empty(Fr.name(s, id)), Fr.link(id), Fr.name(s, id)) ++ Fr.title.tag(s, id)

def Fr.title(+s: M.State, +id: String, title: String) -> String:
  Fr.title.k(Fr.is(id), s, id, title)

# A history reset removes only that link's projection; it never changes
# the local sequence, local entries, or another machine's mirror.
def Fr.map.step(-A: Data, remove: Bool, m: Map<&2, A>, key: String) -> Map<&2, A>:
  match remove:
    case True{}:
      Map.del(&2, A, m, key)
    case False{}:
      m

def Fr.map.go(-A: Data, ks: List<&2, String>, m: Map<&2, A>, +prefix: String) -> Map<&2, A>:
  match ks:
    case Nil{}:
      m
    case +k <> t:
      Fr.map.go(A, t, Fr.map.step(A, String.starts_with(k, prefix), m, k), prefix)

def Fr.map(-A: Data, +m: Map<&2, A>, +prefix: String) -> Map<&2, A>:
  Fr.map.go(A, Map.keys(&2, A, m), m, prefix)

def Fr.entries.keep(remove: Bool, e: M.Entry, acc: List<&2, M.Entry>) -> List<&2, M.Entry>:
  match remove:
    case True{}:
      acc
    case False{}:
      e <> acc

def Fr.entries.go(es: List<&2, M.Entry>, +prefix: String, acc: List<&2, M.Entry>) -> List<&2, M.Entry>:
  match es:
    case Nil{}:
      List.reverse(&2, M.Entry, acc)
    case +e <> t:
      Fr.entries.go(t, prefix, Fr.entries.keep(String.starts_with(M.Idx.thread(e), prefix), e, acc))

def Fr.index(es: List<&2, M.Entry>, by: Map<&2, List<&2, M.Entry>>) -> Map<&2, List<&2, M.Entry>>:
  match es:
    case Nil{}:
      by
    case +e <> t:
      Fr.index(t, M.Idx.post(by, e))

def Fr.world(w: BT.World, +prefix: String) -> BT.World:
  match w:
    case BT.World{bs, rs, ps, hs, ro, ms, pe, sp, ho, ta, pg, tk}:
      BT.World{Fr.map(BT.Bot, bs, prefix), rs, ps, hs, Fr.map(BT.Routine, ro, prefix), Fr.map(BT.Mem, ms, prefix), pe,
        Fr.map(String, sp, prefix), Fr.map(BT.Hops, ho, prefix), Fr.map(Nat, ta, prefix), Fr.map(BT.Page, pg, prefix), Fr.map(String, tk, prefix)}

def Fr.ext.spec(bw: List<&2, M.Ext>, +prefix: String, es: List<&2, M.Entry>) -> List<&2, M.Ext>:
  [M.Ext{Fr.world(M.Ext.world(M.Ext.of(bw)), prefix), Fr.index(List.reverse(&2, M.Entry, es), Map.new(&2, List<&2, M.Entry>))}]

# The index without the link's entries, without walking the rest. An entry
# is filed (M.Idx.post) under its thread, under its sender's key when
# another thread sent it, under the links when a message, and under
# "@steps:<thread>" when a step; only those lists can hold a removed entry,
# so only they are cut (each at most once), and a list left empty goes
# (law far_forget_equiv, against Fr.forget.spec's rebuild).
def Fr.keys.step(step: Bool, +th: String, +m: Map<&2, Bool>) -> Map<&2, Bool>:
  match step:
    case True{}:
      Map.set(&2, Bool, m, M.Idx.steps.key(th), True{})
    case False{}:
      m

def Fr.keys.link(msg: Bool, +m: Map<&2, Bool>) -> Map<&2, Bool>:
  match msg:
    case True{}:
      Map.set(&2, Bool, m, M.Idx.links(), True{})
    case False{}:
      m

def Fr.keys.sent(sent: Bool, msg: Bool, +from: String, +m: Map<&2, Bool>) -> Map<&2, Bool>:
  match sent:
    case True{}:
      Fr.keys.link(msg, Map.set(&2, Bool, m, from, True{}))
    case False{}:
      m

# the keys an entry is filed under, added to m
def Fr.keys.of(+e: M.Entry, +m: Map<&2, Bool>) -> Map<&2, Bool>:
  +th = M.Idx.thread(e)
  +from = M.Link.from(M.Idx.id(e))
  Fr.keys.step(M.Idx.is_step(e), th,
    Fr.keys.sent(Bool.and(Bool.not(String.is_empty(from)), Bool.not(String.eq(from, th))), M.Idx.is_msg(e), from, Map.set(&2, Bool, m, th, True{})))

def Fr.keys.one(hit: Bool, +e: M.Entry, +m: Map<&2, Bool>) -> Map<&2, Bool>:
  match hit:
    case True{}:
      Fr.keys.of(e, m)
    case False{}:
      m

# the keys the link's entries in a list are filed under
def Fr.keys.list(es: List<&2, M.Entry>, +prefix: String, m: Map<&2, Bool>) -> Map<&2, Bool>:
  match es:
    case Nil{}:
      m
    case +e <> t:
      Fr.keys.list(t, prefix, Fr.keys.one(String.starts_with(M.Idx.thread(e), prefix), e, m))

def Fr.keys.at(hit: Bool, es: List<&2, M.Entry>, +prefix: String, m: Map<&2, Bool>) -> Map<&2, Bool>:
  match hit:
    case True{}:
      Fr.keys.list(es, prefix, m)
    case False{}:
      m

# every list filed under one of the link's threads
def Fr.keys.go(ks: List<&2, String>, +by: Map<&2, List<&2, M.Entry>>, +prefix: String, m: Map<&2, Bool>) -> Map<&2, Bool>:
  match ks:
    case Nil{}:
      m
    case +k <> t:
      Fr.keys.go(t, by, prefix, Fr.keys.at(String.starts_with(k, prefix), M.Idx.get(by, k), prefix, m))

def Fr.by.put(empty: Bool, by: Map<&2, List<&2, M.Entry>>, +k: String, xs: List<&2, M.Entry>) -> Map<&2, List<&2, M.Entry>>:
  match empty:
    case True{}:
      Map.del(&2, List<&2, M.Entry>, by, k)
    case False{}:
      Map.set(&2, List<&2, M.Entry>, by, k, xs)

def Fr.by.cut.k(+by: Map<&2, List<&2, M.Entry>>, +k: String, +xs: List<&2, M.Entry>) -> Map<&2, List<&2, M.Entry>>:
  Fr.by.put(List.is_empty(&2, M.Entry, xs), by, k, xs)

def Fr.by.cut(ks: List<&2, String>, +by: Map<&2, List<&2, M.Entry>>, +prefix: String) -> Map<&2, List<&2, M.Entry>>:
  match ks:
    case Nil{}:
      by
    case +k <> t:
      Fr.by.cut(t, Fr.by.cut.k(by, k, Fr.entries.go(M.Idx.get(by, k), prefix, Nil{})), prefix)

def Fr.by(+by: Map<&2, List<&2, M.Entry>>, +prefix: String) -> Map<&2, List<&2, M.Entry>>:
  Fr.by.cut(Map.keys(&2, Bool, Fr.keys.go(Map.keys(&2, List<&2, M.Entry>, by), by, prefix, Map.new(&2, Bool))), by, prefix)

def Fr.ext(bw: List<&2, M.Ext>, +prefix: String) -> List<&2, M.Ext>:
  +x = M.Ext.of(bw)
  [M.Ext{Fr.world(M.Ext.world(x), prefix), Fr.by(M.Ext.by(x), prefix)}]

# the settings a link's things leave behind: the work their threads had
# (WorkSet's bg.<link>~... and watch.<link>~...); the rest stay
def Fr.settings(+sg: Map<&2, String>, +prefix: String) -> Map<&2, String>:
  Fr.map(String, Fr.map(String, sg, "bg." ++ prefix), "watch." ++ prefix)

def Fr.forget.spec(s: M.State, +link: String) -> M.State:
  match s:
    case M.State{ps, ts, +es, n, tk, sg, qu, ak, dv, +nv, bw}:
      +prefix = link ++ "~"
      +kept = Fr.entries.go(es, prefix, Nil{})
      M.State{Fr.map(M.Project, ps, prefix), Fr.map(M.Thread, ts, prefix), kept, n, Fr.map(M.Task, tk, prefix), Fr.settings(sg, prefix),
        Fr.map(List<&2, M.QItem>, qu, prefix), Fr.map(M.Ask, ak, prefix), dv,
        Map.del(&2, M.Nav, Fr.map(M.Nav, nv, prefix), Fr.nav(link)), Fr.ext.spec(bw, prefix, kept)}


# the link's projection removed in one pass over its threads (the index
# is cut, not rebuilt; Fr.forget.spec is the rebuild, law far_forget_equiv)
def Fr.forget(s: M.State, +link: String) -> M.State:
  match s:
    case M.State{ps, ts, +es, n, tk, sg, qu, ak, dv, +nv, bw}:
      +prefix = link ++ "~"
      M.State{Fr.map(M.Project, ps, prefix), Fr.map(M.Thread, ts, prefix), Fr.entries.go(es, prefix, Nil{}), n, Fr.map(M.Task, tk, prefix), Fr.settings(sg, prefix),
        Fr.map(List<&2, M.QItem>, qu, prefix), Fr.map(M.Ask, ak, prefix), dv,
        Map.del(&2, M.Nav, Fr.map(M.Nav, nv, prefix), Fr.nav(link)), Fr.ext(bw, prefix)}

# a state with a link's threads, steps and messages in both directions
def Fr.demo.idx() -> M.State:
  M.State.replay([M.ThreadCreated{"t1", "p1", "A", M.Local{}, "", "", "claude", "", 1n}, M.ThreadCreated{"p1~t1", "p1~p", "B", M.Local{}, "", "", "claude", "", 1n},
    M.ThreadCreated{"p1~t2", "p1~p", "C", M.Local{}, "", "", "claude", "", 1n}, M.ThreadCreated{"p2~t1", "p2~p", "D", M.Local{}, "", "", "claude", "", 1n},
    M.MessagePosted{"t1", "m1", M.User{}, "a", 2n}, M.MessagePosted{"p1~t1", "m2", M.User{}, "b", 3n}, M.ActivityLogged{"p1~t1", "s1", "step", "kicad", "x", 4n},
    M.ActivityLogged{"t1", "s2", "step", "kicad", "y", 5n}, M.MessagePosted{"t1", M.Link.id("p1~t1", "7"), M.User{}, "hi", 6n},
    M.MessagePosted{"p1~t2", M.Link.id("t1", "8"), M.User{}, "yo", 7n}, M.MessagePosted{"p2~t1", M.Link.id("p1~t2", "9"), M.User{}, "x", 8n},
    M.MessagePosted{"p1~t2", "m3", M.Assistant{}, "c", 9n}, M.MessagePosted{"p2~t1", "m4", M.Assistant{}, "d", 10n}], M.State.empty())

def Fr.epoch.nav(+link: String) -> String:
  "epoch~" ++ link

def Fr.epoch.of(+s: M.State, +link: String) -> String:
  M.Nav.key(M.Nav.get(M.State.navs(s), Fr.epoch.nav(link)), "pinned")

def Fr.epoch.keep(s: M.State, +link: String, +epoch: String) -> M.State:
  M.State.nav((x => M.Nav.keyed(x, "pinned", epoch)), s, Fr.epoch.nav(link))

def Fr.recv.base(reset: Bool, s: M.State, +link: String) -> M.State:
  match reset:
    case True{}:
      Fr.forget(s, link)
    case False{}:
      s

def Fr.epoch.changed(+old: String, +epoch: String) -> Bool:
  Bool.and(Bool.not(String.is_empty(epoch)), Bool.not(String.eq(old, epoch)))

def Fr.recv(+s: M.State, +link: String, +epoch: String, +since: Nat, +to: Nat, xs: List<&2, Item>) -> M.State:
  Fr.epoch.keep(Fr.fold(Fr.recv.base(Fr.epoch.changed(Fr.epoch.of(s, link), epoch), s, link), link, since, to, xs), link,
    Bool.pick(String, String.is_empty(epoch), Fr.epoch.of(s, link), epoch))

# A hub's mirror of a link
# ------------------------
# What the other end sent, in its order, how far it goes, its name, and
# whether it answered last time (away: shown, stale, until it does).

type In is Data:
  In{link: String, name: String, head: Nat, items: List<&2, Item>, away: Bool, epoch: String}

def In.new(+link: String, +name: String) -> In:
  In{link, name, 0n, Nil{}, True{}, ""}

def In.link(i: In) -> String:
  match i:
    case In{l, _, _, _, _, _}:
      l

def In.head(i: In) -> Nat:
  match i:
    case In{_, _, h, _, _, _}:
      h

def In.away(i: In) -> Bool:
  match i:
    case In{_, _, _, _, a, _}:
      a

def In.away.set(i: In, a: Bool) -> In:
  match i:
    case In{l, n, h, xs, _, e}:
      In{l, n, h, xs, a, e}

def In.name.set(i: In, +nm: String) -> In:
  match i:
    case In{l, n, h, xs, a, e}:
      In{l, Bool.pick(String, String.is_empty(nm), n, nm), h, xs, a, e}

def In.put.k(g: Got, i: In) -> In:
  match g i:
    case Got{_, _, True{}} _:
      i
    case Got{h, ys, False{}} In{l, n, _, xs, _, e}:
      In{l, n, h, List.append(&2, Item, xs, ys), False{}, e}

# a batch into the mirror (it answered, so it is here)
def In.put(+i: In, +since: Nat, +to: Nat, xs: List<&2, Item>) -> In:
  In.put.k(Fr.take(In.head(i), since, to, xs), i)

# what a batch brings that the mirror lacks (Nil on a gap)
def In.got(+i: In, +since: Nat, +to: Nat, xs: List<&2, Item>) -> Got:
  Fr.take(In.head(i), since, to, xs)

# the message a client folds: {"t":"far","link","name","away","since","head","items"}
def Fr.msg(+link: String, +name: String, away: Bool, +since: Nat, +head: Nat, xs: List<&2, Item>) -> String:
  J.Json.show(J.Json.obj([J.KV{"t", J.Str{"far"}}, J.KV{"link", J.Str{link}}, J.KV{"name", J.Str{name}}, J.KV{"away", J.Json.bool(away)},
    J.KV{"since", J.Json.nat(since)}, J.KV{"head", J.Json.nat(head)}, J.KV{"items", J.Json.arr(Fr.items.json(xs))}]))

def Fr.epoch.fields(o: J.Json) -> J.Json:
  match o:
    case J.Obj{fs}:
      fs
    case _:
      J.End{}

def In.epoch(i: In) -> String:
  match i:
    case In{_, _, _, _, _, e}:
      e

def In.epoch.set(i: In, e: String) -> In:
  match i:
    case In{l, n, h, xs, a, _}:
      In{l, n, h, xs, a, e}

# Optional on the wire: old clients still decode the same far message.
def Fr.with.epoch(+text: String, epoch: String) -> String:
  J.Json.show(J.Obj{J.Field{"epoch", J.Str{epoch}, Fr.epoch.fields(Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{}))}})

# the answer to a pull (POST /far/log): where it starts, the catalog, then
# the page with its epoch ({"head", "items", "more", "epoch"})
def Fr.log.json(+from: Nat, cat: J.Json, +page: String) -> String:
  J.Json.show(J.Obj{J.Field{"since", J.Json.nat(from), J.Field{"catalog", cat, Fr.epoch.fields(Maybe.default(&2, J.Json, J.Json.parse(page), J.Null{}))}}})

# a far message with a link's catalog (Fr.catalog's JSON text) beside it
def Fr.with.cat.k(none: Bool, +text: String, +cat: String) -> String:
  match none:
    case True{}:
      text
    case False{}:
      J.Json.show(J.Obj{J.Field{"catalog", Maybe.default(&2, J.Json, J.Json.parse(cat), J.Null{}), Fr.epoch.fields(Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{}))}})

def Fr.with.cat(+text: String, +cat: String) -> String:
  Fr.with.cat.k(String.is_empty(cat), text, cat)

def Fr.msg.epoch(+link: String, +name: String, away: Bool, +since: Nat, +head: Nat, xs: List<&2, Item>, epoch: String) -> String:
  J.Json.show(J.Json.obj([J.KV{"t", J.Str{"far"}}, J.KV{"link", J.Str{link}}, J.KV{"name", J.Str{name}}, J.KV{"away", J.Json.bool(away)},
    J.KV{"since", J.Json.nat(since)}, J.KV{"head", J.Json.nat(head)}, J.KV{"epoch", J.Str{epoch}}, J.KV{"items", J.Json.arr(Fr.items.json(xs))}]))

# all a mirror holds, for a client that just joined
def In.msg(i: In) -> String:
  match i:
    case In{+l, +n, +h, xs, a, +e}:
      Fr.msg.epoch(l, n, a, 0n, h, xs, e)

# A mirror for a client that is joining goes a chunk at a time (at most
# In.most() items, after its item `after`): each says the range it covers
# by sequence (since = the last chunk's end, to = its own; the last one
# ends at the mirror's head), so a client folds each once, in order, and a
# live batch that came first is never undone (core/outbox.bend)
type FC is Data:
  FC{text: String, next: Nat, done: Bool}

def In.most() -> Nat:
  500n

def FC.last(xs: List<&2, Item>, +d: Nat) -> Nat:
  match xs:
    case Nil{}:
      d
    case Item{n, _} <> Nil{}:
      n
    case _ <> t:
      FC.last(t, d)

def FC.of(done: Bool, +l: String, +nm: String, a: Bool, +h: Nat, +after: Nat, +xs: List<&2, Item>, +e: String) -> FC:
  match done:
    case True{}:
      FC{Fr.msg.epoch(l, nm, a, after, h, xs, e), h, True{}}
    case False{}:
      FC{Fr.msg.epoch(l, nm, a, after, FC.last(xs, h), xs, e), FC.last(xs, h), False{}}

# whether the first item is past head (True for none: the walk stops there)
def Fr.first.gt(xs: List<&2, Item>, +head: Nat) -> Bool:
  match xs:
    case Nil{}:
      True{}
    case Item{+n, _} <> _:
      Nat.is_gt(n, head)

# the items after head of an ascending list, by a tail-recursive walk that
# builds nothing (Fr.fresh recurses once per item: a join of a big mirror
# overflowed the stack and walked it all again for each chunk)
def Fr.after(xs: List<&2, Item>, +head: Nat, hit: Bool) -> List<&2, Item>:
  match xs hit:
    case +r True{}:
      r
    case Nil{} _:
      Nil{}
    case _ <> +t False{}:
      Fr.after(t, head, Fr.first.gt(t, head))

# whether more than k items remain
def Fr.over(xs: List<&2, Item>, +k: Nat) -> Bool:
  match xs k:
    case Nil{} _:
      False{}
    case _ <> _ 0n:
      True{}
    case _ <> +t 1n+p:
      Fr.over(t, p)

def In.chunk.k(i: In, +after: Nat, +ys: List<&2, Item>) -> FC:
  match i:
    case In{+l, +nm, +h, _, a, +e}:
      FC.of(Bool.not(Fr.over(ys, In.most())), l, nm, a, h, after, List.take(&2, Item, ys, In.most()), e)

# the next chunk of a mirror, after its item `after`
def In.chunk(+i: In, +after: Nat) -> FC:
  match i:
    case In{_, _, _, +xs, _, _}:
      In.chunk.k(i, after, Fr.after(xs, after, Fr.first.gt(xs, after)))

# Mirrors by link
# ---------------

def Ins.find(ms: List<&2, In>, +link: String) -> Maybe<&2, In>:
  match ms:
    case Nil{}:
      None{}
    case +i <> t:
      Bool.pick(Maybe<&2, In>, String.eq(In.link(i), link), Some{i}, Ins.find(t, link))

def Ins.get(+ms: List<&2, In>, +link: String, +name: String) -> In:
  Maybe.default(&2, In, Ins.find(ms, link), In.new(link, name))

# the mirror of `link` replaced (added when new)
def Ins.set(ms: List<&2, In>, +i: In) -> List<&2, In>:
  match ms:
    case Nil{}:
      [i]
    case +x <> +t:
      Bool.pick(List<&2, In>, String.eq(In.link(x), In.link(i)), i <> t, x <> Ins.set(t, i))

# Requests to a thread elsewhere
# ------------------------------
# (thread.delete is not one: law far_no_delete keeps a linked machine from
# deleting; a thread elsewhere is deleted on its own machine)
# A request naming a thread, an ask or a bot elsewhere goes to the hub it
# lives on, with that hub's ids; only these may (Fr.may): writing to it,
# stopping it, answering it, its queue, settling it, a side question, its
# modes, and a bot's space and routines. Anything else is refused here.

def Fr.methods() -> List<&2, String>:
  ["turn.start", "turn.interrupt", "ask.answer", "queue.move", "queue.remove", "queue.resume", "queue.steer", "queue.edit",
   "thread.settle", "thread.unsettle", "thread.back", "thread.rename", "thread.btw", "thread.modes", "thread.provider", "thread.effort", "bots.space", "bots.routine_run",
   "thread.create", "thread.fork", "thread.archive", "thread.unarchive", "thread.pin", "thread.unpin",
   "project.add", "project.new", "project.remove", "fs.list", "fs.mkdir", "fs.find"]

def Fr.may.go(ms: List<&2, String>, +m: String) -> Bool:
  match ms:
    case Nil{}:
      False{}
    case x <> t:
      Bool.or(String.eq(x, m), Fr.may.go(t, m))

def Fr.may(+m: String) -> Bool:
  Fr.may.go(Fr.methods(), m)

# the params that name a thing (the ones renamed on the way)
def Fr.keys() -> List<&2, String>:
  ["thread", "id", "bot", "routine", "task", "parent", "child", "project", "ask"]

def Fr.key(+k: String) -> Bool:
  Fr.may.go(Fr.keys(), k)

# the link a request's params name, "" when they name nothing elsewhere
def Fr.params.link(fs: J.Json) -> String:
  match fs:
    case J.Field{+k, J.Str{+v}, t}:
      Bool.pick(String, Bool.and(Fr.key(k), Fr.is(v)), Fr.link(v),
        Bool.pick(String, Bool.and(String.eq(k, "on"), Bool.and(Bool.not(String.is_empty(v)), Bool.not(Fr.is(v)))), v, Fr.params.link(t)))
    case J.Field{_, _, t}:
      Fr.params.link(t)
    case _:
      ""

# the params with the link's ids made local
def Fr.params.local(fs: J.Json, +link: String) -> J.Json:
  match fs:
    case J.Field{+k, J.Str{+v}, t}:
      J.Field{k, J.Str{Bool.pick(String, Bool.and(Fr.key(k), String.eq(Fr.link(v), link)), Fr.local(v), v)}, Fr.params.local(t, link)}
    case J.Field{k, v, t}:
      J.Field{k, v, Fr.params.local(t, link)}
    case x:
      x

def Fr.obj.fields(o: J.Json) -> J.Json:
  match o:
    case J.Obj{fs}:
      fs
    case _:
      J.End{}

def Fr.is.some(m: Maybe<&2, M.Change>) -> Bool:
  match m:
    case Some{_}:
      True{}
    case None{}:
      False{}

# a request bound elsewhere: which link, and the request as that hub
# takes it (its method, its params in its ids; no request id)
type Route is Data:
  Route{link: String, m: String, body: String}

# A routed request carries a nonce of its own (the asking client, its
# request id and the second), so two equal requests in one second are two
# signed bodies, not a replay of one. The owner reads only id, m and p.
def Fr.nonced(+body: String, +n: String) -> String:
  "{\"n\":" ++ J.Json.show(J.Str{n}) ++ "," ++ String.drop(body, 1n)

def Fr.route.k(+link: String, +m: String, +fs: J.Json) -> Maybe<&2, Route>:
  Bool.pick(Maybe<&2, Route>, String.is_empty(link), None{},
    Some{Route{link, m, J.Json.show(J.Json.obj([J.KV{"id", J.Num{"0"}}, J.KV{"m", J.Str{m}}, J.KV{"p", J.Obj{Fr.params.local(fs, link)}}]))}})

def Route.link.of(m: Maybe<&2, Route>) -> String:
  match m:
    case Some{Route{l, _, _}}:
      l
    case None{}:
      ""

# None: a request for this hub
def Fr.route(+o: J.Json) -> Maybe<&2, Route>:
  Fr.route.k(Fr.params.link(Fr.obj.fields(J.Json.get_obj(o, "p"))), J.Json.get_str(o, "m", ""), Fr.obj.fields(J.Json.get_obj(o, "p")))

# On the owning hub: a request from a linked machine is taken only when it
# is one of Fr.methods and what it names is shared (a bot's thread, an ask
# in one, a bot)
# the methods the owning hub answers from a job (a folder listed or made, a
# folder added as a project: the server resolves the path first)
def Fr.async(+m: String) -> Bool:
  Fr.may.go(["project.add", "project.new", "fs.list", "fs.mkdir", "fs.find"], m)

def Fr.allowed.p(+m: String) -> Bool:
  Fr.may.go(["thread.create", "project.remove"], m)

# what makes no claim on a thing: browsing and adding folders on the
# owner's machine (only the owner's own machines ever get this far)
def Fr.allowed.free(+m: String) -> Bool:
  Fr.async(m)

def Fr.allowed.k(+s: M.State, +m: String, +p: J.Json) -> Bool:
  Bool.pick(Bool, String.eq(m, "ask.answer"), Fr.ask.shared(s, J.Json.get_str(p, "id", "")),
    Bool.pick(Bool, Bool.or(String.eq(m, "bots.space"), String.eq(m, "bots.routine_run")), Bool.not(String.is_empty(J.Json.get_str(p, "bot", J.Json.get_str(p, "id", "")))),
      Bool.pick(Bool, Fr.allowed.p(m), Fr.project.shared(s, J.Json.get_str(p, "project", "")),
        Bool.pick(Bool, Fr.allowed.free(m), True{}, Fr.thread.shared(s, J.Json.get_str(p, "thread", ""))))))

def Fr.allowed(+s: M.State, +o: J.Json) -> Bool:
  Bool.and(Fr.may(J.Json.get_str(o, "m", "")), Fr.allowed.k(s, J.Json.get_str(o, "m", ""), J.Json.get_obj(o, "p")))

# The owning hub's reply keeps its success/error and payload; only the
# caller's request id is restored on the receiving hub.
def Fr.reply.fields(fs: J.Json, +id: String) -> J.Json:
  match fs:
    case J.Field{+k, +v, t}:
      J.Field{k, Bool.pick(J.Json, String.eq(k, "id"), J.Num{id}, v), Fr.reply.fields(t, id)}
    case x:
      x

def Fr.reply(o: J.Json, +id: String) -> String:
  J.Json.show(J.Obj{Fr.reply.fields(Fr.obj.fields(o), id)})

# The same on the receiving hub of a link: the thread a reply names (a new
# thread, a fork, a project's first) is named by its id here, which is the
# id the mirror will give it, so the asking client can go to it
def Fr.reply.on.thread(+v: J.Json, +link: String) -> J.Json:
  match v:
    case J.Str{+x}:
      J.Str{Bool.pick(String, Fr.is(x), x, Fr.opt(link, x))}
    case x:
      x

def Fr.reply.on.value(+k: String, +v: J.Json, +id: String, +link: String) -> J.Json:
  Bool.pick(J.Json, String.eq(k, "id"), J.Num{id}, Bool.pick(J.Json, String.eq(k, "thread"), Fr.reply.on.thread(v, link), v))

def Fr.reply.on.fields(fs: J.Json, +id: String, +link: String) -> J.Json:
  match fs:
    case J.Field{+k, +v, t}:
      J.Field{k, Fr.reply.on.value(k, v, id, link), Fr.reply.on.fields(t, id, link)}
    case x:
      x

def Fr.reply.on(o: J.Json, +id: String, +link: String) -> String:
  J.Json.show(J.Obj{Fr.reply.on.fields(Fr.obj.fields(o), id, link)})

# A late disk load never replaces a mirror already updated by traffic.
def In.loaded(+live: In, +disk: In) -> In:
  Bool.pick(In, Bool.and(In.away(live), Nat.is_ge(In.head(disk), In.head(live))), disk, live)

# How a thing elsewhere is shown
# ------------------------------
# its machine's name beside its own ("Kit · desk"); nothing for one here

def Fr.where(+id: String, +name: String) -> String:
  Bool.pick(String, Fr.is(id), " · " ++ name, "")

# The owning hub's feed
# ---------------------
# What it shares, as its log numbers it (newest first; each a wire line,
# kept as text so the hub holds no parsed copy): made once from the log
# (off the hub), then grown by each commit. A linked machine's pull reads
# a page of it after the number it has.

type Line is Data:
  Line{n: Nat, text: String}

def Line.n(l: Line) -> Nat:
  match l:
    case Line{n, _}:
      n

def Fr.line.k(sh: Bool, +n: Nat, +text: String, rest: List<&2, Line>) -> List<&2, Line>:
  match sh:
    case True{}:
      Line{n, text} <> rest
    case False{}:
      rest

def Fr.line.shared(m: Maybe<&2, M.Change>, +s: M.State) -> Bool:
  match m:
    case Some{c}:
      Fr.shared(s, c)
    case None{}:
      False{}

def Fr.line.parsed(m: Maybe<&2, J.Json>) -> Maybe<&2, M.Change>:
  match m:
    case Some{+o}:
      C.Change.decode(o)
    case None{}:
      None{}

# the shared lines of a log and each project's last number, in one pass
type Feed is Data:
  Feed{lines: List<&2, Line>, lasts: Map<&2, Nat>}

def Feed.lines(f: Feed) -> List<&2, Line>:
  match f:
    case Feed{ls, _}:
      ls

def Feed.lasts(f: Feed) -> Map<&2, Nat>:
  match f:
    case Feed{_, la}:
      la

def Fr.feed.step.k(sh: Bool, +p: String, +n: Nat, +text: String, f: Feed) -> Feed:
  match sh f:
    case True{} Feed{ls, la}:
      Feed{Line{n, text} <> ls, Fr.last.put(p, n, la)}
    case False{} Feed{ls, la}:
      Feed{ls, la}

def Fr.feed.step(m: Maybe<&2, M.Change>, +s: M.State, +n: Nat, +text: String, f: Feed) -> Feed:
  match m:
    case Some{+c}:
      Fr.feed.step.k(Fr.shared(s, c), Fr.scope(s, c), n, text, f)
    case None{}:
      f

def Fr.feed.done(f: Feed) -> Feed:
  match f:
    case Feed{ls, la}:
      Feed{List.reverse(&2, Line, ls), la}

# the shared lines of a log (newest first, `n` the number of the first),
# newest first; tail-recursive (a log holds tens of thousands)
def Fr.feed.go(ls: List<&2, String>, +n: Nat, +s: M.State, acc: Feed) -> Feed:
  match ls:
    case Nil{}:
      Fr.feed.done(acc)
    case +l <> t:
      Fr.feed.go(t, Nat.sub(n, 1n), s, Fr.feed.step(Fr.line.parsed(J.Json.parse(l)), s, n, l, acc))

def Fr.feed.full(+ls: List<&2, String>, +s: M.State) -> Feed:
  Fr.feed.go(ls, List.length(&2, String, ls), s, Feed{Nil{}, Map.new(&2, Nat)})

def Fr.feed(+ls: List<&2, String>, +s: M.State) -> List<&2, Line>:
  Feed.lines(Fr.feed.full(ls, s))

# a commit's changes (oldest first) with their wire lines (oldest first),
# numbered after `seq`, the shared ones newest first
def Fr.feed.add(cs: List<&2, M.Change>, ls: List<&2, String>, +seq: Nat, +s: M.State, acc: List<&2, Line>) -> List<&2, Line>:
  match cs ls:
    case +c <> t +l <> r:
      Fr.feed.add(t, r, 1n+seq, s, Fr.line.k(Fr.shared(s, c), 1n+seq, l, acc))
    case _ _:
      acc

def Fr.feed.keep(go: Bool, +n: Nat, +t: String, acc: List<&2, Line>) -> List<&2, Line>:
  match go:
    case True{}:
      Line{n, t} <> acc
    case False{}:
      acc

# the lines after `since`, oldest first (the feed is newest first, so they
# are the ones before the first at or below `since`)
# (it stops at the first line at or below since: law far_feed_after_stops)
def Fr.feed.after(fs: List<&2, Line>, +since: Nat, go: Bool, acc: List<&2, Line>) -> List<&2, Line>:
  match fs go:
    case _ False{}:
      acc
    case Nil{} True{}:
      acc
    case Line{+n, +t} <> r True{}:
      Fr.feed.after(r, since, Nat.is_gt(n, since), Fr.feed.keep(Nat.is_gt(n, since), n, t, acc))

# a page: the items after `since` up to about `cap` characters (one bigger
# line goes alone), where it ends (the last item's number when there is
# more, else the log's end), and whether there is more
type Page is Data:
  Page{head: Nat, items: List<&2, Item>, more: Bool}

type Pg is Data:
  Pg{room: Nat, last: Nat, count: Nat, items: List<&2, Item>, full: Bool}

def Fr.page.item(+n: Nat, +t: String) -> Item:
  Item{n, Maybe.default(&2, J.Json, J.Json.parse(t), J.Null{})}

def Fr.page.step.k(fits: Bool, first: Bool, p: Pg, +n: Nat, +t: String) -> Pg:
  match fits first p:
    case True{} _ Pg{r, _, c, xs, f}:
      Pg{Nat.sub(r, String.length(t)), n, 1n+c, Fr.page.item(n, t) <> xs, f}
    case False{} True{} Pg{_, _, c, xs, _}:
      Pg{0n, n, 1n+c, Fr.page.item(n, t) <> xs, True{}}
    case False{} False{} Pg{r, l, c, xs, _}:
      Pg{r, l, c, xs, True{}}

def Fr.page.step(p: Pg, +n: Nat, +t: String) -> Pg:
  match p:
    case Pg{+r, l, +c, xs, True{}}:
      Pg{r, l, c, xs, True{}}
    case Pg{+r, l, +c, xs, False{}}:
      Fr.page.step.k(Nat.is_le(String.length(t), r), Nat.is_eq(c, 0n), Pg{r, l, c, xs, False{}}, n, t)

def Fr.page.go(ls: List<&2, Line>, p: Pg) -> Pg:
  match ls:
    case Nil{}:
      p
    case Line{+n, +t} <> r:
      Fr.page.go(r, Fr.page.step(p, n, t))

def Fr.page.of(p: Pg, +total: Nat, +seq: Nat) -> Page:
  match p:
    case Pg{_, +l, +c, xs, _}:
      Page{Bool.pick(Nat, Nat.is_lt(c, total), l, seq), List.reverse(&2, Item, xs), Nat.is_lt(c, total)}

def Fr.page.k(+ls: List<&2, Line>, +since: Nat, +seq: Nat, +cap: Nat) -> Page:
  Fr.page.of(Fr.page.go(ls, Pg{cap, since, 0n, Nil{}, False{}}), List.length(&2, Line, ls), seq)

# what a pull from `since` gets (seq: this log's length)
def Fr.page(+fs: List<&2, Line>, +since: Nat, +seq: Nat, +cap: Nat) -> Page:
  Fr.page.k(Fr.feed.after(fs, since, True{}, Nil{}), since, seq, cap)

def Page.json(p: Page) -> String:
  match p:
    case Page{h, xs, m}:
      J.Json.show(J.Json.obj([J.KV{"head", J.Json.nat(h)}, J.KV{"items", J.Json.arr(Fr.items.json(xs))}, J.KV{"more", J.Json.bool(m)}]))

# What the history's identity is made of: the log's first line, and, when
# the log holds lines this build does not decode (a newer build wrote them:
# `len` lines against `seq` changes), how many, so the feed's numbering
# (by changes) starts a history of its own and mirrors pull it afresh.
# The version of what is shared (Fr.shared): a build that shares more
# kinds of change bumps it, so every mirror pulls the history afresh once.
def Fr.view.tag() -> String:
  "|view:2"

def Fr.history.body(+first: String, +len: Nat, +seq: Nat) -> String:
  Bool.pick(String, Nat.is_eq(len, seq), first, first ++ "|unknown:" ++ Nat.show(Nat.sub(len, seq))) ++ Fr.view.tag()

# Pushing to a linked machine
# ---------------------------
# One push at a time per link. A batch says it follows the last push that
# got through (`acked`) only while every push since did (`clean`); after a
# failure (or a restart: nothing is known) it says it follows only its own
# start, so the other end, missing what failed, finds the gap and pulls.
# Batches that come while one is out wait, merged, for it to end.

# A batch out is kept (`flight`, and where it began, `ffrom`) until the
# other end answers. A failure puts it back ahead of what waited, and the
# link holds (`hold`) for a backoff (2, 4 ... 60 s, `Out.delay`) on its
# own timer before it goes again; an answer through drops it.

type Out is Data:
  Out{link: String, acked: Nat, clean: Bool, busy: Bool, from: Nat, to: Nat, items: List<&2, Item>, flight: List<&2, Item>, ffrom: Nat, hold: Bool, fails: Nat}

def Out.new(+link: String) -> Out:
  Out{link, 0n, False{}, False{}, 0n, 0n, Nil{}, Nil{}, 0n, False{}, 0n}

def Out.link(o: Out) -> String:
  match o:
    case Out{l, _, _, _, _, _, _, _, _, _, _}:
      l

# a batch to send: what it follows, where it ends, its items
type Send is Data:
  Send{link: String, since: Nat, to: Nat, items: List<&2, Item>}

def Out.add.k(empty: Bool, o: Out, +from: Nat, +to: Nat, xs: List<&2, Item>) -> Out:
  match empty o:
    case True{} Out{l, a, c, b, f, _, ys, fl, ff, h, n}:
      Out{l, a, c, b, f, to, ys, fl, ff, h, n}
    case False{} Out{l, a, c, b, f, _, +ys, fl, ff, h, n}:
      Out{l, a, c, b, Bool.pick(Nat, List.is_empty(&2, Item, ys), from, f), to, List.append(&2, Item, ys, xs), fl, ff, h, n}

# a commit that took the log from `from` to `to`, with what it shares
def Out.add(o: Out, +from: Nat, +to: Nat, +xs: List<&2, Item>) -> Out:
  Out.add.k(List.is_empty(&2, Item, xs), o, from, to, xs)

def Out.next.k(go: Bool, o: Out) -> Maybe<&2, Send> & Out:
  match go o:
    case True{} Out{+l, +a, +c, _, +f, +t, +xs, _, _, h, n}:
      (Some{Send{l, Bool.pick(Nat, c, a, f), t, xs}}, Out{l, a, c, True{}, f, t, Nil{}, xs, f, h, n})
    case False{} _:
      (None{}, o)

# the push to start now, if one is due (none out, not holding, something waiting)
def Out.next(o: Out) -> Maybe<&2, Send> & Out:
  match o:
    case Out{l, a, c, +b, f, t, +xs, fl, ff, +h, n}:
      Out.next.k(Bool.and(Bool.and(Bool.not(b), Bool.not(h)), Bool.not(List.is_empty(&2, Item, xs))), Out{l, a, c, b, f, t, xs, fl, ff, h, n})

# the push out ended: through (up to `to`), or not (its items go back first)
def Out.done(o: Out, ok: Bool, +to: Nat) -> Out:
  match o ok:
    case Out{l, _, _, _, f, t, xs, +_fl, _, _, _} True{}:
      Out{l, to, True{}, False{}, f, t, xs, Nil{}, 0n, False{}, 0n}
    case Out{l, a, _, _, f, t, xs, fl, ff, _, n} False{}:
      Out{l, a, False{}, False{}, Bool.pick(Nat, List.is_empty(&2, Item, fl), f, ff), t, List.append(&2, Item, fl, xs), Nil{}, 0n, True{}, 1n+n}

# the state after the push due now was started (what the laws read)
def Out.sent.k(m: Maybe<&2, Send>, p: Out) -> Out:
  match m:
    case Some{_}:
      p
    case None{}:
      p

def Out.sent.p(r: Maybe<&2, Send> & Out) -> Out:
  (m, p) = r
  Out.sent.k(m, p)

def Out.sent(o: Out) -> Out:
  Out.sent.p(Out.next(o))

# a push is due now
def Out.due.k(m: Maybe<&2, Send>, p: Out) -> Bool:
  match m p:
    case Some{_} _:
      True{}
    case None{} _:
      False{}

def Out.due.p(r: Maybe<&2, Send> & Out) -> Bool:
  (m, p) = r
  Out.due.k(m, p)

def Out.due(o: Out) -> Bool:
  Out.due.p(Out.next(o))

# the hold ended (its timer fired)
def Out.release(o: Out) -> Out:
  match o:
    case Out{l, a, c, b, f, t, xs, fl, ff, _, n}:
      Out{l, a, c, b, f, t, xs, fl, ff, False{}, n}

def Out.holding(o: Out) -> Bool:
  match o:
    case Out{_, _, _, _, _, _, _, _, _, +h, _}:
      h

def Out.waiting(o: Out) -> List<&2, Item>:
  match o:
    case Out{_, _, _, _, _, _, +xs, _, _, _, _}:
      xs

def Out.flight(o: Out) -> List<&2, Item>:
  match o:
    case Out{_, _, _, _, _, _, _, +fl, _, _, _}:
      fl

def Out.fails(o: Out) -> Nat:
  match o:
    case Out{_, _, _, _, _, _, _, _, _, _, +n}:
      n

def Out.begins(o: Out) -> Nat:
  match o:
    case Out{_, _, _, _, +f, _, _, _, _, _, _}:
      f

# seconds to hold after `n` failures in a row: 2, 4, 8 ... 60
def Out.delay.go(n: Nat, +d: Nat) -> Nat:
  match n:
    case 0n:
      d
    case 1n+k:
      Out.delay.go(k, Bool.pick(Nat, Nat.is_ge(d, 32n), 60n, (d * 2n : Nat)))

def Out.delay(+n: Nat) -> Nat:
  Out.delay.go(Nat.sub(n, 1n), 2n)

def Outs.get(os: List<&2, Out>, +link: String) -> Out:
  match os:
    case Nil{}:
      Out.new(link)
    case +o <> t:
      Bool.pick(Out, String.eq(Out.link(o), link), o, Outs.get(t, link))

def Outs.set(os: List<&2, Out>, +o: Out) -> List<&2, Out>:
  match os:
    case Nil{}:
      [o]
    case +x <> +t:
      Bool.pick(List<&2, Out>, String.eq(Out.link(x), Out.link(o)), o <> t, x <> Outs.set(t, o))

def Send.body(s: Send, +name: String) -> String:
  match s:
    case Send{_, since, to, xs}:
      J.Json.show(J.Json.obj([J.KV{"name", J.Str{name}}, J.KV{"since", J.Json.nat(since)}, J.KV{"head", J.Json.nat(to)}, J.KV{"items", J.Json.arr(Fr.items.json(xs))}]))

# What a hub keeps for threads elsewhere
# --------------------------------------
# its mirrors (one per link), its pushes (one per link), its own feed
# (None until made from the log), and the lines committed meanwhile

type Fh is Data:
  Fh{ins: List<&2, In>, outs: List<&2, Out>, feed: Maybe<&2, List<&2, Line>>, early: List<&2, Line>, pulls: List<&2, String>, epoch: String, cx: Cx}

def Fh.empty() -> Fh:
  Fh{Nil{}, Nil{}, None{}, Nil{}, Nil{}, "", Cx.empty()}

def Fh.ins(f: Fh) -> List<&2, In>:
  match f:
    case Fh{i, _, _, _, _, _, _}:
      i

def Fh.in(+f: Fh, +link: String, +name: String) -> In:
  Ins.get(Fh.ins(f), link, name)

def Fh.in.set(f: Fh, i: In) -> Fh:
  match f:
    case Fh{ms, os, fd, e, p, ep, cx}:
      Fh{Ins.set(ms, i), os, fd, e, p, ep, cx}

def Fh.out(+f: Fh, +link: String) -> Out:
  match f:
    case Fh{_, os, _, _, _, _, _}:
      Outs.get(os, link)

def Fh.out.set(f: Fh, o: Out) -> Fh:
  match f:
    case Fh{ms, os, fd, e, p, ep, cx}:
      Fh{ms, Outs.set(os, o), fd, e, p, ep, cx}

# the feed, None until it is made from the log
def Fh.feed(f: Fh) -> Maybe<&2, List<&2, Line>>:
  match f:
    case Fh{_, _, fd, _, _, _, _}:
      fd

def Fh.feed.put(m: Maybe<&2, List<&2, Line>>, e: List<&2, Line>, ls: List<&2, Line>) -> Maybe<&2, List<&2, Line>> & List<&2, Line>:
  match m:
    case Some{fs}:
      (Some{List.append(&2, Line, ls, fs)}, e)
    case None{}:
      (None{}, List.append(&2, Line, ls, e))

def Fh.feed.at.k(f: Fh, fd: Maybe<&2, List<&2, Line>>, e: List<&2, Line>) -> Fh:
  match f:
    case Fh{ms, os, _, _, p, ep, cx}:
      Fh{ms, os, fd, e, p, ep, cx}

def Fh.feed.at(f: Fh, r: Maybe<&2, List<&2, Line>> & List<&2, Line>) -> Fh:
  (fd, e) = r
  Fh.feed.at.k(f, fd, e)

# a commit's shared lines (newest first) join the feed, or wait for it
def Fh.feed.add(f: Fh, ls: List<&2, Line>) -> Fh:
  match f:
    case Fh{ms, os, fd, e, p, ep, cx}:
      Fh.feed.at(Fh{ms, os, None{}, Nil{}, p, ep, cx}, Fh.feed.put(fd, e, ls))

# the feed made from the log (up to where it was when asked), with what
# was committed since on top; its projects' numbers join the commits'
def Fh.feed.ready(f: Fh, fd: Feed) -> Fh:
  match f fd:
    case Fh{ms, os, _, e, p, ep, Cx{la, ca, ub}} Feed{ls, +fl}:
      Fh{ms, os, Some{List.append(&2, Line, e, ls)}, Nil{}, p, ep, Cx{Fr.lasts.merge(Map.keys(&2, Nat, fl), fl, la), ca, ub}}

# a commit's changes counted into the projects' numbers
def Fh.lasts.add(f: Fh, cs: List<&2, M.Change>, +seq: Nat, +s: M.State) -> Fh:
  match f:
    case Fh{ms, os, fd, e, p, ep, Cx{la, ca, ub}}:
      Fh{ms, os, fd, e, p, ep, Cx{Fr.lasts.add(cs, seq, s, la), ca, ub}}

def Fh.lasts(f: Fh) -> Map<&2, Nat>:
  match f:
    case Fh{_, _, _, _, _, _, Cx{la, _, _}}:
      la

# the newest catalog a link sent ("" for none), and keeping one
def Fh.cat(+f: Fh, +link: String) -> String:
  match f:
    case Fh{_, _, _, _, _, _, Cx{_, ca, _}}:
      Fr.str.got(Map.get(String, "", ca, link))

def Fh.cat.set(f: Fh, +link: String, +text: String) -> Fh:
  match f:
    case Fh{ms, os, fd, e, p, ep, Cx{la, +ca, ub}}:
      Fh{ms, os, fd, e, p, ep, Cx{la, Bool.pick(Map<&2, String>, String.is_empty(text), ca, Map.set(&2, String, ca, link, text)), ub}}

# What the uberbot's sync keeps in memory, not in the log (ubersync.bend,
# server.bend): when the holder was last heard, which machines answered,
# each link's last version vector and whether a request is out
def Fh.ub(+f: Fh, +k: String) -> String:
  match f:
    case Fh{_, _, _, _, _, _, Cx{_, _, ub}}:
      Fr.str.got(Map.get(String, "", ub, k))

def Fh.ub.map(f: Fh) -> Map<&2, String>:
  match f:
    case Fh{_, _, _, _, _, _, Cx{_, _, ub}}:
      ub

def Fh.ub.set(f: Fh, +k: String, +v: String) -> Fh:
  match f:
    case Fh{ms, os, fd, e, p, ep, Cx{la, ca, ub}}:
      Fh{ms, os, fd, e, p, ep, Cx{la, ca, Map.set(&2, String, ub, k, v)}}

def Fh.pulling(+f: Fh, +link: String) -> Bool:
  match f:
    case Fh{_, _, _, _, p, _, _}:
      Fr.may.go(p, link)

def Fh.pull.on(f: Fh, +link: String) -> Fh:
  match f:
    case Fh{ms, os, fd, e, p, ep, cx}:
      Fh{ms, os, fd, e, link <> p, ep, cx}

def Fh.pull.off.go(ps: List<&2, String>, +link: String) -> List<&2, String>:
  match ps:
    case Nil{}:
      Nil{}
    case +x <> +t:
      Bool.pick(List<&2, String>, String.eq(x, link), t, x <> Fh.pull.off.go(t, link))

def Fh.pull.off(f: Fh, +link: String) -> Fh:
  match f:
    case Fh{ms, os, fd, e, p, ep, cx}:
      Fh{ms, os, fd, e, Fh.pull.off.go(p, link), ep, cx}

def Fh.epoch(f: Fh) -> String:
  match f:
    case Fh{_, _, _, _, _, ep, _}:
      ep

def Fh.epoch.set(f: Fh, +epoch: String) -> Fh:
  match f:
    case Fh{ms, os, fd, e, p, +ep, cx}:
      Fh{ms, os, fd, e, p, Bool.pick(String, String.is_empty(epoch), ep, epoch), cx}

# lines (newest first) as items, oldest first
def Fr.lines.items(ls: List<&2, Line>, acc: List<&2, Item>) -> List<&2, Item>:
  match ls:
    case Nil{}:
      acc
    case Line{+n, +t} <> r:
      Fr.lines.items(r, Fr.page.item(n, t) <> acc)

# A mirror kept on disk
# ---------------------
# <home>/far/<link>.jsonl: each batch taken, its items as lines, then
# {"head": n}; read back at start (the mirror shows, away, until its
# machine answers)

def Fr.file.items(xs: List<&2, Item>) -> String:
  match xs:
    case Nil{}:
      ""
    case x <> t:
      J.Json.show(Item.json(x)) ++ "\n" ++ Fr.file.items(t)

def Fr.file.batch(xs: List<&2, Item>, +head: Nat) -> String:
  Fr.file.items(xs) ++ "{\"head\":" ++ Nat.show(head) ++ "}\n"

def Bool.is_some.j(m: Maybe<&2, J.Json>) -> Bool:
  match m:
    case Some{_}:
      True{}
    case None{}:
      False{}

def Fr.some.j(m: Maybe<&2, J.Json>) -> Bool:
  match m:
    case Some{_}:
      True{}
    case None{}:
      False{}

# New files carry an epoch header before their batches. Old files remain
# readable; the first epoch-aware pull replaces that legacy projection.
def Fr.file.line(m: Maybe<&2, J.Json>, i: In) -> In:
  match m i:
    case Some{+o} In{+l, +n, +h, +xs, +a, +e}:
      Bool.pick(In, Fr.some.j(J.Json.get(o, "c")), In{l, n, h, Item.of(o) <> xs, a, e},
        In{l, n, J.Json.get_nat(o, "head", h), xs, a, J.Json.get_str(o, "epoch", e)})
    case None{} _:
      i

def Fr.file.scan(ls: List<&2, String>, i: In) -> In:
  match ls:
    case Nil{}:
      i
    case l <> t:
      Fr.file.scan(t, Fr.file.line(J.Json.parse(l), i))

def Fr.file.done(i: In) -> In:
  match i:
    case In{l, n, h, xs, a, e}:
      In{l, n, h, List.reverse(&2, Item, xs), a, e}

def Fr.file.read(+text: String, +link: String, +name: String) -> In:
  Fr.file.done(Fr.file.scan(String.lines(text), In.new(link, name)))

# what a mirror holds, oldest first
def In.items(i: In) -> List<&2, Item>:
  match i:
    case In{_, _, _, xs, _, _}:
      xs

def In.name.of(i: In) -> String:
  match i:
    case In{_, n, _, _, _, _}:
      n

# a push that only says where it ends, as a gap: the other end pulls
def Send.gap(s: Send) -> Send:
  match s:
    case Send{l, _, +to, _}:
      Send{l, to, to, Nil{}}

# a mirrored bot's space page (its newest SpacePageSet; "" for none), for
# GET /bots/<link>~<id>/space.html on the hub that mirrors it
def Fr.page.html.k(hit: Bool, +o: J.Json, +cur: String) -> String:
  match hit:
    case True{}:
      J.Json.get_str(o, "html", "")
    case False{}:
      cur

def Fr.page.html.go(xs: List<&2, Item>, +bot: String, +cur: String) -> String:
  match xs:
    case Nil{}:
      cur
    case Item{_, +c} <> t:
      Fr.page.html.go(t, bot, Fr.page.html.k(Bool.and(String.eq(J.Json.get_str(c, "$", ""), "SpacePageSet"), String.eq(J.Json.get_str(c, "bot", ""), bot)), c, cur))

def In.page(i: In, +bot: String) -> String:
  match i:
    case In{_, _, _, xs, _, _}:
      Fr.page.html.go(xs, bot, "")
