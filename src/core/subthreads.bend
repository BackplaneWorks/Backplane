import Base
import ./model.bend as M
import ./far.bend as FR
import ./bot.bend as BT

# Visible conversations use explicit parenting or their original delegation.
# Existing settings events keep old logs and phone kept-state decodable.
def Tree.key(id: String) -> String:
  "thread.parent." ++ id

def Tree.open_key(id: String) -> String:
  "thread.tree." ++ id

def Tree.project(t: M.Thread) -> String:
  match t:
    case M.Thread{_, p, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}: p

def Tree.alive(t: M.Thread) -> Bool:
  match t:
    case M.Thread{_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, d}: Bool.not(d)

def Tree.archived(t: M.Thread) -> Bool:
  match t:
    case M.Thread{_, _, _, _, _, _, _, a, _, _, _, _, _, _, _, _, _, _, _, _, _}: a

def Tree.all(s: M.State) -> List<&2, M.Thread>:
  match s:
    case M.State{_, ts, _, _, _, _, _, _, _, _, _}: Map.values(&2, M.Thread, ts)

def Tree.same(a: Bool, b: Bool) -> Bool:
  match a b:
    case True{} True{}: True{}
    case False{} False{}: True{}
    case _ _: False{}

def Tree.raw.k(ok: Bool, p: String) -> String:
  match ok:
    case True{}: p
    case False{}: ""

# Invalid, missing and cross-project links never swallow a conversation.
def Tree.raw.from(+s: M.State, +id: String, p: String) -> String:
  match p:
    case SNil{}: ""
    case SCon{+c, +t}:
      Tree.raw.k(Bool.and(Bool.not(String.eq(id, SCon{c, t})),
        Bool.and(Tree.alive(M.State.thread.get(s, id)), Bool.and(Tree.alive(M.State.thread.get(s, SCon{c, t})),
          String.eq(Tree.project(M.State.thread.get(s, id)), Tree.project(M.State.thread.get(s, SCon{c, t})))))), SCon{c, t})

# A transient lookup, never saved to State or the event store. Explicit links
# (including an empty ThreadParentSet detach) override delegation. Bot work
# remains a project root. Build once per tree walk, not once per ancestry edge.
def Tree.parents.has(r: Map<&2, String> & Bool) -> Bool:
  (_, found) = r
  found

def Tree.parents.add(skip: Bool, links: Map<&2, String>, child: String, parent: String) -> Map<&2, String>:
  match skip:
    case True{}: links
    case False{}: Map.set(&2, String, links, Tree.key(child), parent)

def Tree.parents.tasks(ts: List<&2, M.Task>, +world: BT.World, links: Map<&2, String>) -> Map<&2, String>:
  match ts:
    case Nil{}: links
    case M.Task{_, +parent, +child, _, _, _, _, _, _} <> rest:
      +saved = links
      Tree.parents.tasks(rest, world, Tree.parents.add(Bool.or(BT.World.is_bot_thread(world, parent),
        Tree.parents.has(Map.has(&2, String, saved, Tree.key(child)))), saved, child, parent))

def Tree.parents(+s: M.State) -> Map<&2, String>:
  match s:
    case M.State{_, _, _, _, tasks, settings, _, _, _, _, _}:
      Tree.parents.tasks(Map.values(&2, M.Task, tasks), M.State.bots(s), settings)

def Tree.parents.get(r: Map<&2, String> & String) -> String:
  (_, value) = r
  value

def Tree.raw.with(+s: M.State, +id: String, +parents: Map<&2, String>) -> String:
  Tree.raw.from(s, id, Tree.parents.get(Map.get(String, "", parents, Tree.key(id))))

# Creation permits at most 64 levels. Corrupt settings, including a cycle
# unrelated to the starting row, fall back to roots instead of recursing.
def Tree.raw(+s: M.State, +id: String) -> String:
  Tree.raw.with(s, id, Tree.parents(s))

def Tree.chain.with(fuel: Nat, +s: M.State, id: String, +origin: String, +parents: Map<&2, String>) -> Bool:
  match fuel id:
    case _ SNil{}: True{}
    case 0n SCon{c, t}: False{}
    case 1n+n SCon{+c, +t}:
      Bool.and(Bool.not(String.eq(SCon{c, t}, origin)), Tree.chain.with(n, s, Tree.raw.with(s, SCon{c, t}, parents), origin, parents))

def Tree.chain(fuel: Nat, +s: M.State, id: String, +origin: String) -> Bool:
  Tree.chain.with(fuel, s, id, origin, Tree.parents(s))

def Tree.parent.at.with(+s: M.State, +id: String, +p: String, +parents: Map<&2, String>) -> String:
  Tree.raw.k(Tree.chain.with(64n, s, p, id, parents), p)

def Tree.parent.at(+s: M.State, +id: String, +p: String) -> String:
  Tree.parent.at.with(s, id, p, Tree.parents(s))

def Tree.parent.with(+s: M.State, +id: String, +parents: Map<&2, String>) -> String:
  Tree.parent.at.with(s, id, Tree.raw.with(s, id, parents), parents)

# Archiving affects one conversation. Live children of an archived parent
# become roots; the saved relationship remains for unarchive and deep links.
def Tree.parent(+s: M.State, +id: String) -> String:
  Tree.parent.with(s, id, Tree.parents(s))

def Tree.display_parent.at(+s: M.State, +id: String, p: String) -> String:
  match p:
    case SNil{}: ""
    case SCon{+c, +t}:
      Tree.raw.k(Tree.same(Tree.archived(M.State.thread.get(s, id)), Tree.archived(M.State.thread.get(s, SCon{c, t}))), SCon{c, t})

def Tree.display_parent.with(+s: M.State, +id: String, +parents: Map<&2, String>) -> String:
  Tree.display_parent.at(s, id, Tree.parent.with(s, id, parents))

# Tree.root validates the entire raw ancestry once before entering root.go.
# Its remaining suffix cannot contain a cycle or an invalid project edge.
def Tree.display_parent(+s: M.State, +id: String) -> String:
  Tree.display_parent.with(s, id, Tree.parents(s))

def Tree.display_raw.with(+s: M.State, +id: String, +parents: Map<&2, String>) -> String:
  +p = Tree.raw.with(s, id, parents)
  Tree.raw.k(Tree.same(Tree.archived(M.State.thread.get(s, id)), Tree.archived(M.State.thread.get(s, p))), p)

def Tree.display_raw(+s: M.State, +id: String) -> String:
  Tree.display_raw.with(s, id, Tree.parents(s))

def Tree.root.go.with(fuel: Nat, +s: M.State, id: String, p: String, +parents: Map<&2, String>) -> String:
  match fuel p:
    case _ SNil{}: id
    case 0n SCon{c, t}: id
    case 1n+n SCon{+c, +t}:
      Tree.root.go.with(n, s, SCon{c, t}, Tree.display_raw.with(s, SCon{c, t}, parents), parents)

def Tree.root.go(fuel: Nat, +s: M.State, id: String, p: String) -> String:
  Tree.root.go.with(fuel, s, id, p, Tree.parents(s))

def Tree.root.with(+s: M.State, +id: String, +parents: Map<&2, String>) -> String:
  Tree.root.go.with(64n, s, id, Tree.display_parent.with(s, id, parents), parents)

def Tree.root(+s: M.State, +id: String) -> String:
  Tree.root.with(s, id, Tree.parents(s))

def Tree.has(xs: List<&2, String>, +id: String) -> Bool:
  match xs:
    case Nil{}: False{}
    case +h <> t: Bool.or(String.eq(h, id), Tree.has(t, id))

def Tree.ids(ts: List<&2, M.Thread>) -> List<&2, String>:
  match ts:
    case Nil{}: Nil{}
    case t <> rest: M.Thread.id(t) <> Tree.ids(rest)

def Tree.roots.add(skip: Bool, t: M.Thread, rest: List<&2, M.Thread>) -> List<&2, M.Thread>:
  match skip:
    case True{}: rest
    case False{}: t <> rest

def Tree.seen.has(r: Map<&2, Bool> & Bool) -> Bool:
  (_, found) = r
  found

def Tree.seen.of(ids: List<&2, String>, seen: Map<&2, Bool>) -> Map<&2, Bool>:
  match ids:
    case Nil{}: seen
    case +id <> rest: Tree.seen.of(rest, Map.set(&2, Bool, seen, id, True{}))

def Tree.roots.map.with(ts: List<&2, M.Thread>, +s: M.State, +seen: Map<&2, Bool>, +parents: Map<&2, String>) -> List<&2, M.Thread>:
  match ts:
    case Nil{}: Nil{}
    case +t <> rest:
      +id = Tree.root.with(s, M.Thread.id(t), parents)
      Tree.roots.add(Bool.or(Bool.not(Tree.alive(t)), Tree.seen.has(Map.has(&2, Bool, seen, id))), M.State.thread.get(s, id),
        Tree.roots.map.with(rest, s, Map.set(&2, Bool, seen, id, True{}), parents))

def Tree.roots.map(ts: List<&2, M.Thread>, +s: M.State, +seen: Map<&2, Bool>) -> List<&2, M.Thread>:
  Tree.roots.map.with(ts, s, seen, Tree.parents(s))

def Tree.roots.go.with(ts: List<&2, M.Thread>, +s: M.State, seen: List<&2, String>, +parents: Map<&2, String>) -> List<&2, M.Thread>:
  Tree.roots.map.with(ts, s, Tree.seen.of(seen, Map.new(&2, Bool)), parents)

# The subtree occupies the earliest shelf of any of its conversations.
# This makes a working child reachable even when its parent was settled;
# it never changes the parent's own status, timestamps or agent count.
def Tree.roots.go(ts: List<&2, M.Thread>, +s: M.State, seen: List<&2, String>) -> List<&2, M.Thread>:
  Tree.roots.go.with(ts, s, seen, Tree.parents(s))

def Tree.shelves.k.with(+s: M.State, +p: List<&2, M.Thread>, +a: List<&2, M.Thread>, +st: List<&2, M.Thread>, +h: List<&2, M.Thread>, +sn: List<&2, M.Thread>, +parents: Map<&2, String>) -> M.Shelves:
  +pr = Tree.roots.go.with(p, s, Nil{}, parents)
  +ar = Tree.roots.go.with(a, s, Tree.ids(pr), parents)
  +sr = Tree.roots.go.with(sn, s, List.append(&2, String, Tree.ids(pr), Tree.ids(ar)), parents)
  +tr = Tree.roots.go.with(st, s, List.concat(&2, String, [Tree.ids(pr), Tree.ids(ar), Tree.ids(sr)]), parents)
  M.Shelves{pr, ar, tr, Tree.roots.go.with(h, s, Nil{}, parents), sr}

def Tree.shelves.k(+s: M.State, +p: List<&2, M.Thread>, +a: List<&2, M.Thread>, +st: List<&2, M.Thread>, +h: List<&2, M.Thread>, +sn: List<&2, M.Thread>) -> M.Shelves:
  Tree.shelves.k.with(s, p, a, st, h, sn, Tree.parents(s))

def Tree.shelves.with(s: M.State, sh: M.Shelves, +parents: Map<&2, String>) -> M.Shelves:
  match sh:
    case M.Shelves{p, a, st, h, sn}: Tree.shelves.k.with(s, p, a, st, h, sn, parents)

def Tree.shelves(+s: M.State, sh: M.Shelves) -> M.Shelves:
  Tree.shelves.with(s, sh, Tree.parents(s))

def Tree.children.go.with(ts: List<&2, M.Thread>, +s: M.State, +id: String, +parents: Map<&2, String>) -> List<&2, M.Thread>:
  match ts:
    case Nil{}: Nil{}
    case +t <> rest:
      Tree.roots.add(Bool.not(String.eq(Tree.display_parent.with(s, M.Thread.id(t), parents), id)), t, Tree.children.go.with(rest, s, id, parents))

def Tree.children.go(ts: List<&2, M.Thread>, +s: M.State, +id: String) -> List<&2, M.Thread>:
  Tree.children.go.with(ts, s, id, Tree.parents(s))

def Tree.cmp.le(r: (String & String) & Cmp) -> Bool:
  (_, c) = r
  Cmp.is_le(c)

def Tree.before(a: M.Thread, b: M.Thread) -> Bool:
  match a b:
    case M.Thread{ia, _, _, +ca, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _} M.Thread{ib, _, _, +cb, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      Bool.or(Nat.is_lt(ca, cb), Bool.and(Nat.is_eq(ca, cb), Tree.cmp.le(String.cmp(ia, ib))))

def Tree.children.with(+s: M.State, +id: String, +parents: Map<&2, String>) -> List<&2, M.Thread>:
  List.sort(~M.Thread, ~Tree.before, Tree.children.go.with(Tree.all(s), s, id, parents))

def Tree.children(+s: M.State, +id: String) -> List<&2, M.Thread>:
  Tree.children.with(s, id, Tree.parents(s))

def Tree.ancestor.with(fuel: Nat, +s: M.State, from: String, +id: String, +parents: Map<&2, String>) -> Bool:
  match fuel from:
    case _ SNil{}: False{}
    case 0n SCon{c, t}: False{}
    case 1n+n SCon{+c, +t}:
      Bool.or(String.eq(Tree.display_parent.with(s, SCon{c, t}, parents), id), Tree.ancestor.with(n, s, Tree.display_parent.with(s, SCon{c, t}, parents), id, parents))

# First selection reveals an unconfigured branch. An explicit saved
# collapse still wins, so the expander works while a child is selected.
def Tree.ancestor(fuel: Nat, +s: M.State, from: String, +id: String) -> Bool:
  Tree.ancestor.with(fuel, s, from, id, Tree.parents(s))

def Tree.open.k.with(value: String, +s: M.State, +id: String, selected: String, +parents: Map<&2, String>) -> Bool:
  match value:
    case SNil{}: Tree.ancestor.with(64n, s, selected, id, parents)
    case SCon{c, t}: String.eq(SCon{c, t}, "1")

def Tree.open.k(value: String, +s: M.State, +id: String, selected: String) -> Bool:
  Tree.open.k.with(value, s, id, selected, Tree.parents(s))

def Tree.open.with(+s: M.State, +id: String, selected: String, +parents: Map<&2, String>) -> Bool:
  Tree.open.k.with(M.Setting.get(s, Tree.open_key(id), ""), s, id, selected, parents)

type TRow is Data:
  TRow{thread: M.Thread, depth: Nat, expanded: Bool, children: Nat}

type TJob is Data:
  TJob{thread: M.Thread, depth: Nat}

def Tree.open(+s: M.State, +id: String, selected: String) -> Bool:
  Tree.open.with(s, id, selected, Tree.parents(s))

def Tree.jobs(ts: List<&2, M.Thread>, +depth: Nat) -> List<&2, TJob>:
  match ts:
    case Nil{}: Nil{}
    case t <> rest: TJob{t, depth} <> Tree.jobs(rest, depth)

def Tree.next(open: Bool, kids: List<&2, M.Thread>, depth: Nat, rest: List<&2, TJob>) -> List<&2, TJob>:
  match open:
    case True{}: List.append(&2, TJob, Tree.jobs(kids, 1n+depth), rest)
    case False{}: rest

def Tree.index.push(r: Map<&2, List<&2, M.Thread>> & List<&2, M.Thread>, p: String, t: M.Thread) -> Map<&2, List<&2, M.Thread>>:
  (m, xs) = r
  Map.set(&2, List<&2, M.Thread>, m, p, t <> xs)

def Tree.index.add(empty: Bool, m: Map<&2, List<&2, M.Thread>>, +p: String, t: M.Thread) -> Map<&2, List<&2, M.Thread>>:
  match empty:
    case True{}: m
    case False{}: Tree.index.push(Map.get(List<&2, M.Thread>, Nil{}, m, p), p, t)

def Tree.index.with(ts: List<&2, M.Thread>, +s: M.State, m: Map<&2, List<&2, M.Thread>>, +parents: Map<&2, String>) -> Map<&2, List<&2, M.Thread>>:
  match ts:
    case Nil{}: m
    case +t <> rest:
      +p = Tree.display_parent.with(s, M.Thread.id(t), parents)
      Tree.index.with(rest, s, Tree.index.add(String.is_empty(p), m, p, t), parents)

def Tree.index(ts: List<&2, M.Thread>, +s: M.State, m: Map<&2, List<&2, M.Thread>>) -> Map<&2, List<&2, M.Thread>>:
  Tree.index.with(ts, s, m, Tree.parents(s))

def Tree.index.get(r: Map<&2, List<&2, M.Thread>> & List<&2, M.Thread>) -> List<&2, M.Thread>:
  (_, xs) = r
  List.sort(~M.Thread, ~Tree.before, xs)

def Tree.walk.with(fuel: Nat, jobs: List<&2, TJob>, +s: M.State, +selected: String, +search: Bool, +by: Map<&2, List<&2, M.Thread>>, +parents: Map<&2, String>) -> List<&2, TRow>:
  match fuel jobs:
    case 0n _: Nil{}
    case 1n+n Nil{}: Nil{}
    case 1n+n TJob{+t, +depth} <> rest:
      +kids = Tree.index.get(Map.get(List<&2, M.Thread>, Nil{}, by, M.Thread.id(t)))
      +open = Bool.or(search, Tree.open.with(s, M.Thread.id(t), selected, parents))
      TRow{t, depth, open, List.length(&2, M.Thread, kids)} <> Tree.walk.with(n, Tree.next(open, kids, depth, rest), s, selected, search, by, parents)

# A forest can only reach its roots' projects: Tree.raw rejects every
# cross-project link. Do not validate/index unrelated projects per shelf.
def Tree.walk(fuel: Nat, jobs: List<&2, TJob>, +s: M.State, +selected: String, +search: Bool, +by: Map<&2, List<&2, M.Thread>>) -> List<&2, TRow>:
  Tree.walk.with(fuel, jobs, s, selected, search, by, Tree.parents(s))

def Tree.projects(ts: List<&2, M.Thread>, +s: M.State, projects: Map<&2, Bool>) -> Map<&2, Bool>:
  match ts:
    case Nil{}: projects
    case t <> rest:
      Tree.projects(rest, s, Map.set(&2, Bool, projects, Tree.project(M.State.thread.get(s, M.Thread.id(t))), True{}))

def Tree.candidates(ts: List<&2, M.Thread>, +projects: Map<&2, Bool>) -> List<&2, M.Thread>:
  match ts:
    case Nil{}: Nil{}
    case +t <> rest:
      Tree.roots.add(Bool.not(Tree.seen.has(Map.has(&2, Bool, projects, Tree.project(t)))), t, Tree.candidates(rest, projects))

def Tree.rows.k.with(ts: List<&2, M.Thread>, +s: M.State, selected: String, search: Bool, +parents: Map<&2, String>) -> List<&2, TRow>:
  match ts:
    case Nil{}: Nil{}
    case +t <> +rest:
      +all = Tree.all(s)
      Tree.walk.with(1n+List.length(&2, M.Thread, all), Tree.jobs(t <> rest, 0n), s, selected, search,
        Tree.index.with(Tree.candidates(all, Tree.projects(t <> rest, s, Map.new(&2, Bool))), s, Map.new(&2, List<&2, M.Thread>), parents), parents)

def Tree.rows.k(ts: List<&2, M.Thread>, +s: M.State, selected: String, search: Bool) -> List<&2, TRow>:
  Tree.rows.k.with(ts, s, selected, search, Tree.parents(s))

def Tree.rows.with(+s: M.State, ts: List<&2, M.Thread>, selected: String, +parents: Map<&2, String>) -> List<&2, TRow>:
  Tree.rows.k.with(ts, s, selected, False{}, parents)

def Tree.rows(+s: M.State, ts: List<&2, M.Thread>, selected: String) -> List<&2, TRow>:
  Tree.rows.with(s, ts, selected, Tree.parents(s))

def Tree.rows_search.with(+s: M.State, ts: List<&2, M.Thread>, selected: String, q: String, +parents: Map<&2, String>) -> List<&2, TRow>:
  Tree.rows.k.with(ts, s, selected, Bool.not(String.is_empty(q)), parents)

def Tree.rows_search(+s: M.State, ts: List<&2, M.Thread>, selected: String, q: String) -> List<&2, TRow>:
  Tree.rows_search.with(s, ts, selected, q, Tree.parents(s))

def Tree.row_ids(rs: List<&2, TRow>) -> List<&2, String>:
  match rs:
    case Nil{}: Nil{}
    case TRow{t, _, _, _} <> rest: M.Thread.id(t) <> Tree.row_ids(rest)

# Archived legacy chats remain reachable, with their original project,
# worktree, IDs and entries. No replacement project is created.
def Legacy.go(ts: List<&2, M.Thread>) -> List<&2, M.Thread>:
  match ts:
    case Nil{}: Nil{}
    case +t <> rest:
      Tree.roots.add(Bool.not(Bool.and(M.Scratch.is(FR.Fr.local(Tree.project(t))), Tree.alive(t))), t, Legacy.go(rest))

def Legacy.threads(s: M.State) -> List<&2, M.Thread>:
  List.sort(~M.Thread, ~Tree.before, Legacy.go(Tree.all(s)))

# Retired conversations may still own work or wait on the person. They
# remain on the visible list even when Conversation history is collapsed.
# Selection also keeps a just-settled row in place without changing it.
def Legacy.unsettled(sh: M.Shelf) -> Bool:
  match sh:
    case M.Active{}: True{}
    case M.Pinned{}: True{}
    case _: False{}

def Legacy.waiting(as: List<&2, M.Ask>, +thread: String) -> Bool:
  match as:
    case Nil{}: False{}
    case M.Ask{_, th, _, _, _, _, open, _} <> rest:
      Bool.or(Bool.and(open, String.eq(th, thread)), Legacy.waiting(rest, thread))

def Legacy.attention(+kids: Map<&2, Nat>, +s: M.State, +t: M.Thread, +selected: String) -> Bool:
  Bool.and(Bool.not(M.Thread.hidden(t)), Bool.or(String.eq(M.Thread.id(t), selected),
    Bool.or(Legacy.unsettled(M.Shelf.of(t)), Bool.or(Legacy.waiting(M.Asks.all(s), M.Thread.id(t)), M.Lifecycle.working(kids, s, t)))))

def Legacy.pick(ts: List<&2, M.Thread>, +kids: Map<&2, Nat>, +s: M.State, +selected: String, +live: Bool) -> List<&2, M.Thread>:
  match ts:
    case Nil{}: Nil{}
    case +t <> rest:
      Tree.roots.add(Bool.not(Tree.same(Legacy.attention(kids, s, t, selected), live)), t, Legacy.pick(rest, kids, s, selected, live))

def Legacy.live(+s: M.State, selected: String) -> List<&2, M.Thread>:
  Legacy.pick(Legacy.threads(s), M.Work.kids.map(s), s, selected, True{})

def Legacy.history(+s: M.State, selected: String) -> List<&2, M.Thread>:
  Legacy.pick(Legacy.threads(s), M.Work.kids.map(s), s, selected, False{})
