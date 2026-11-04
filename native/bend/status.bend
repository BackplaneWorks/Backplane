import Base
import ../../src/core/model.bend as M
import ../../src/core/client.bend as U
import ./kcommon.bend as C

# Status kernel: real Side.ix + Side.phase.ix (src/core/client.bend) over N
# generated threads, as the sidebar does. Mirrors test/gpui/bend/ts/status.ts.

def G.turn(+k: Nat) -> M.TurnState:
  match k:
    case 0n: M.Idle{}
    case 1n: M.Running{}
    case 2n: M.Completed{}
    case 3n: M.Interrupted{}
    case 1n+1n+1n+1n+_: M.Failed{}

def G.env(+i: Nat) -> M.EnvMode:
  Bool.pick(M.EnvMode, C.K.is(C.K.mod(i, 9n), 0n), M.Worktree{}, M.Local{})

def G.wt(+i: Nat) -> String:
  Bool.pick(String, C.K.is(C.K.mod(i, 18n), 0n), "/w", "")

def G.thread.h(+i: Nat, +np: Nat, +h: Nat) -> M.Thread:
  M.Thread{C.K.id("t", i), C.K.id("p", C.K.mod(Nat.mul(i, 7n), np)), "thread", 0n, 0n, G.turn(C.K.mod(C.K.div(h, 8n), 5n)),
    Bool.pick(Nat, C.K.is(C.K.mod(h, 23n), 0n), 1n, 0n), C.K.is(C.K.mod(i, 13n), 0n), M.NoOverride{}, 0n, 0n, False{}, G.env(i), "", G.wt(i),
    "claude", "", M.FullAccess{}, M.DefaultMode{}, "", C.K.is(C.K.mod(i, 41n), 0n)}

def G.thread(+i: Nat, +np: Nat) -> M.Thread:
  G.thread.h(i, np, C.K.h(i))

def G.kind(+k: Nat) -> String:
  match k:
    case 0n: "approval"
    case 1n: "input"
    case 2n: "plan"
    case 3n: "browser"
    case 1n+1n+1n+1n+_: "other"

def G.ask.h(+i: Nat, +h: Nat) -> M.Ask:
  M.Ask{C.K.id("a", i), C.K.id("t", i), G.kind(C.K.mod(C.K.div(h, 64n), 5n)), "Bash", "", "", C.K.is(C.K.mod(C.K.div(h, 512n), 2n), 0n), ""}

def G.ask(+i: Nat) -> M.Ask:
  G.ask.h(i, C.K.h(i))

def G.tstate(+k: Nat) -> M.TaskState:
  match k:
    case 0n: M.TaskQueued{}
    case 1n: M.TaskRunning{}
    case 2n: M.TaskDone{}
    case 3n: M.TaskFailed{}
    case 1n+1n+1n+1n+_: M.TaskCancelled{}

def G.task(+i: Nat) -> M.Task:
  M.Task{C.K.id("k", i), C.K.id("t", i), C.K.id("t", Nat.add(i, 1n)), 1n, G.tstate(C.K.mod(C.K.div(C.K.h(i), 32n), 5n)), "", False{}, "claude", ""}

def G.qitem(+i: Nat) -> M.QItem:
  Bool.pick(M.QItem, C.K.is(C.K.mod(C.K.div(C.K.h(i), 4n), 2n), 0n), M.QItem{"start", ""}, M.QItem{"handoff:x", ""})

type Maps is Data:
  Maps{ths: Map<&2, M.Thread>, ak: Map<&2, M.Ask>, tk: Map<&2, M.Task>, sg: Map<&2, String>, qu: Map<&2, List<&2, M.QItem>>}

def G.ask.put(on: Bool, +i: Nat, ak: Map<&2, M.Ask>) -> Map<&2, M.Ask>:
  match on:
    case True{}: Map.set(&2, M.Ask, ak, C.K.id("a", i), G.ask(i))
    case False{}: ak

def G.task.put(on: Bool, +i: Nat, tk: Map<&2, M.Task>) -> Map<&2, M.Task>:
  match on:
    case True{}: Map.set(&2, M.Task, tk, C.K.id("k", i), G.task(i))
    case False{}: tk

def G.set.put(on: Bool, +key: String, +v: String, sg: Map<&2, String>) -> Map<&2, String>:
  match on:
    case True{}: Map.set(&2, String, sg, key, v)
    case False{}: sg

def G.sets(+i: Nat, sg: Map<&2, String>) -> Map<&2, String>:
  G.set.put(C.K.is(C.K.mod(i, 31n), 0n), "order.wait." ++ C.K.id("t", i), "1",
    G.set.put(C.K.is(C.K.mod(i, 29n), 0n), "watch." ++ C.K.id("t", i), "x\ty",
      G.set.put(C.K.is(C.K.mod(i, 17n), 0n), "bg." ++ C.K.id("t", i), "b\twork\nc\twork", sg)))

def G.queue.put(on: Bool, +i: Nat, qu: Map<&2, List<&2, M.QItem>>) -> Map<&2, List<&2, M.QItem>>:
  match on:
    case True{}: Map.set(&2, List<&2, M.QItem>, qu, C.K.id("t", i), [G.qitem(i)])
    case False{}: qu

def G.step(+i: Nat, +np: Nat, m: Maps) -> Maps:
  match m:
    case Maps{ths, ak, tk, sg, qu}:
      Maps{Map.set(&2, M.Thread, ths, C.K.id("t", i), G.thread(i, np)),
        G.ask.put(C.K.is(C.K.mod(i, 11n), 0n), i, ak), G.task.put(C.K.is(C.K.mod(i, 6n), 0n), i, tk),
        G.sets(i, sg), G.queue.put(C.K.is(C.K.mod(i, 5n), 0n), i, qu)}

def G.fill(k: Nat, +i: Nat, +np: Nat, m: Maps) -> Maps:
  match k:
    case 0n: m
    case 1n+rest: G.fill(rest, Nat.add(i, 1n), np, G.step(i, np, m))

def G.state.m(m: Maps, ps: Map<&2, M.Project>, es: List<&2, M.Entry>, sq: Nat, dv: Map<&2, M.Device>, nv: Map<&2, M.Nav>, bw: List<&2, M.Ext>) -> M.State:
  match m:
    case Maps{ths, ak, tk, sg, qu}:
      M.State{ps, ths, es, sq, tk, sg, qu, ak, dv, nv, bw}

def G.state.k(e: M.State, m: Maps) -> M.State:
  match e:
    case M.State{ps, _, es, sq, _, _, _, _, dv, nv, bw}:
      G.state.m(m, ps, es, sq, dv, nv, bw)

def G.state(+n: Nat) -> M.State:
  G.state.k(M.State.empty(), G.fill(n, 0n, Nat.add(Nat.div(n, 10n), 1n),
    Maps{Map.new(&2, M.Thread), Map.new(&2, M.Ask), Map.new(&2, M.Task), Map.new(&2, String), Map.new(&2, List<&2, M.QItem>)}))

# the threads in order, as the sidebar reads them
def G.list(k: Nat, +i: Nat, +np: Nat, acc: List<&2, M.Thread>) -> List<&2, M.Thread>:
  match k:
    case 0n: List.reverse(&2, M.Thread, acc)
    case 1n+rest: G.list(rest, Nat.add(i, 1n), np, G.thread(i, np) <> acc)

def Ph.code(p: M.Phase) -> Nat:
  match p:
    case M.PhApproval{}: 0n
    case M.PhInput{}: 1n
    case M.PhWorking{}: 2n
    case M.PhMonitoring{n}: Nat.add(3n, Nat.mul(11n, n))
    case M.PhFailed{}: 4n
    case M.PhComplete{}: 5n
    case M.PhStopped{}: 6n
    case M.PhIdle{}: 7n

def S.loop(ths: List<&2, M.Thread>, +ix: U.SIx, +st: M.State, +acc: Nat) -> Nat:
  match ths:
    case Nil{}: acc
    case t <> rest: S.loop(rest, ix, st, C.K.mix(acc, Ph.code(U.Side.phase.ix(ix, st, t))))

def S.once(+ths: List<&2, M.Thread>, +st: M.State) -> Nat:
  S.loop(ths, U.Side.ix(st), st, 0n)

def S.many(k: Nat, +ths: List<&2, M.Thread>, +st: M.State, +acc: Nat) -> Nat:
  match k:
    case 0n: acc
    case 1n+rest: S.many(rest, ths, st, S.once(ths, st))

def S.reps(k: Nat, +ths: List<&2, M.Thread>, +st: M.State, +inner: Nat) -> IO(Unit):
  match k:
    case 0n: IO.pure(Unit, Unit{})
    case 1n+rest:
      do IO<Unit>:
        t0 : Nat <- IO.now()
        +d : Nat = S.many(inner, ths, st, 0n)
        IO.print("digest " ++ Nat.show(d))
        t1 : Nat <- IO.now()
        IO.print("ms " ++ C.K.fmt(Nat.sub(t1, t0), inner))
        S.reps(rest, ths, st, inner)

def main() -> IO(Unit):
  do IO<Unit>:
    +a : C.Args <- C.K.args()
    +n : Nat = C.K.Args.a(a)
    +r : Nat = C.K.Args.b(a)
    t0 : Nat <- IO.now()
    +st : M.State = G.state(n)
    +ths : List<&2, M.Thread> = G.list(n, 0n, Nat.add(Nat.div(n, 10n), 1n), Nil{})
    +len : Nat = List.length(&2, M.Thread, ths)
    t1 : Nat <- IO.now()
    IO.print("threads " ++ Nat.show(len))
    IO.print("gen_ms " ++ Nat.show(Nat.sub(t1, t0)))
    S.reps(r, ths, st, Bool.pick(Nat, Nat.is_eq(C.K.Args.c(a), 0n), 1n, C.K.Args.c(a)))
