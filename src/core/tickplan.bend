import Base
import ./model.bend as M
import ./hub.bend as B
import ./usage.bend as U
import ./json.bend as J

# The minute planner keeps the existing tick policies and order. Historical
# task parents and shared worktree holders are indexed once, rather than
# searched again for every thread. Neither index survives this state snapshot.
def Tick.parents.go(xs: List<&2, M.Task>, acc: Map<&2, String>) -> Map<&2, String>:
  match xs:
    case Nil{}: acc
    case M.Task{_, p, +c, _, _, _, _, _, _} <> rest:
      Tick.parents.go(rest, Map.set(&2, String, acc, c, p))

def Tick.parents(s: M.State) -> Map<&2, String>:
  match s:
    case M.State{_, _, _, _, tasks, _, _, _, _, _, _}:
      # Hub.settle.parent.go takes the first task in Map.values order,
      # including terminal tasks. Reverse before overwriting duplicate children.
      Tick.parents.go(List.reverse(&2, M.Task, Map.values(&2, M.Task, tasks)), Map.new(&2, String))

def Tick.parent(r: Map<&2, String> & String) -> String:
  (_, p) = r
  p

def Tick.window.ended(ended: Bool, +parents: Map<&2, String>, +s: M.State, +th: String, +idle: Nat, w: Nat) -> Nat:
  match ended:
    case False{}: w
    case True{}:
      B.Hub.settle.w.b(Bool.and(Nat.is_gt(idle, 0n), Bool.or(B.Ephemeral.is(s, th),
        Bool.or(B.Hub.settle.bot.by(s, M.Launch.by(s, th)), B.Hub.settle.bot.by(s, Tick.parent(Map.get(String, "", parents, th)))))), idle, w)

def Tick.settles(ts: List<&2, M.Thread>, +parents: Map<&2, String>, +navs: Map<&2, M.Nav>, +kids: Map<&2, Nat>, +s: M.State, +now: Nat, +w: Nat, +idle: Nat) -> List<&2, M.Change>:
  match ts:
    case Nil{}: Nil{}
    case +t <> rest:
      +due = M.Settle.auto.at(now, Tick.window.ended(B.Hub.settle.ended(M.Thread.turn(t)), parents, s, M.Thread.id(t), idle, w), t, M.Nav.until(M.Nav.get(navs, M.Thread.id(t))))
      B.Hub.settle.k(Bool.and(due, Bool.not(M.Lifecycle.working(kids, s, t))), M.Shelf.of(t), t, now,
        B.Strand.k(B.Strand.due(due, kids, s, t), s, M.Thread.id(t), now, Tick.settles(rest, parents, navs, kids, s, now, w, idle)))

type DropRow is Data:
  DropRow{thread: M.Thread, drop: Bool}

type DropPlan is Data:
  DropPlan{rows: List<&2, DropRow>, holders: Map<&2, Nat>}

def Tick.holder(stays: Bool, +path: String, holders: Map<&2, Nat>) -> Map<&2, Nat>:
  match stays:
    case False{}: holders
    case True{}: Map.set(&2, Nat, holders, path, 1n)

def Tick.drop.row(+t: M.Thread, +drop: Bool, p: DropPlan) -> DropPlan:
  match p:
    case DropPlan{rows, holders}:
      DropPlan{DropRow{t, drop} <> rows, Tick.holder(Bool.and(B.Drop.alive(t), Bool.not(drop)), B.Hub.worktree(t), holders)}

def Tick.drop.plan(ts: List<&2, M.Thread>, +kids: Map<&2, Nat>, +s: M.State, +now: Nat, +w: Nat, acc: DropPlan) -> DropPlan:
  match ts:
    case Nil{}: acc
    case +t <> rest:
      Tick.drop.plan(rest, kids, s, now, w, Tick.drop.row(t, M.Worktree.drop.w(s, t, now, w, M.Work.busy.at(kids, s, M.Thread.id(t))), acc))

def Tick.drop.one(drop: Bool, +t: M.Thread, +holders: Map<&2, Nat>, +s: M.State, rest: List<&2, B.Out>) -> List<&2, B.Out>:
  match drop:
    case False{}: rest
    case True{}:
      # A dropping thread cannot be its own staying holder, so the indexed
      # presence is precisely Drop.held.go's "another live thread" test.
      B.Drop.out(Nat.is_eq(M.Work.kids.at.k(Map.get(Nat, 0n, holders, B.Hub.worktree(t))), 0n), s, t, rest)

def Tick.drop.outs(rows: List<&2, DropRow>, +holders: Map<&2, Nat>, +s: M.State, acc: List<&2, B.Out>) -> List<&2, B.Out>:
  match rows:
    case Nil{}: acc
    case DropRow{t, drop} <> rest:
      Tick.drop.outs(rest, holders, s, Tick.drop.one(drop, t, holders, s, acc))

def Tick.drop.done(p: DropPlan, s: M.State) -> List<&2, B.Out>:
  match p:
    case DropPlan{rows, holders}: Tick.drop.outs(rows, holders, s, Nil{})

def Tick.drops(+ts: List<&2, M.Thread>, kids: Map<&2, Nat>, +s: M.State, now: Nat, w: Nat) -> List<&2, B.Out>:
  Tick.drop.done(Tick.drop.plan(ts, kids, s, now, w, DropPlan{Nil{}, Map.new(&2, Nat)}), s)

def Tick.base(+s: M.State, +now: Nat, +kids: Map<&2, Nat>) -> B.Step:
  match s:
    case M.State{ps, +ts, _, _, _, _, _, _, _, _, _}:
      B.Step.before(B.Shelve.after(Tick.settles(Map.values(&2, M.Thread, ts), Tick.parents(s), M.State.navs(s), kids, s, now, B.Hub.settle.window(s), B.Hub.settle.idle(s)), s, now, kids),
        List.append(&2, B.Out, B.Merge.asks(Map.values(&2, M.Project, ps), Map.values(&2, M.Thread, ts), s), Tick.drops(Map.values(&2, M.Thread, ts), kids, s, now, B.Hub.wt.window(s))),
        B.Wt.stale(Map.values(&2, M.Thread, ts), s, now))

def Tick.main(+s: M.State, +now: Nat) -> B.Step:
  B.Step.join(B.Step.join(B.Step.join(B.Step.join(Tick.base(s, now, M.Work.kids.map(s)), B.Update.due(s, now)), B.Update.held(s, now)),
    B.Step.cat(B.Step.cat(B.Lr.far.tick(s), B.Fq.tick(s)), B.Wake.poll(s))), B.Step.cat(B.Deploy.tick(s, now), B.Ship.tick(s, now)))

# Status can carry megabytes of job history beside a few thread chips.
# Share its parsed object for the guard and projection (Chip.changes parses
# twice); preserve the existing policy for missing or unreadable status.
def Tick.chips.parsed(st: M.State, +o: J.Json, now: Nat) -> List<&2, M.Change>:
  U.Chip.changes.k(U.Bj.has(o, "generated_at"), st, o, now)

def Tick.chips(st: M.State, status: String, now: Nat) -> List<&2, M.Change>:
  Tick.chips.parsed(st, U.Bj.parse(status), now)
