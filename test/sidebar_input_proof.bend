import Base
import ../src/core/client.bend as U
import ../src/core/model.bend as M
import ../src/core/mail.bend as ML
import ../src/core/mailrun.bend as MR
import ../src/core/maildisposition.bend as MD
import ../src/core/mailprivate.bend as PM

def project(st: M.State) -> M.State:
  U.SidebarInput.state(U.SidebarInput.of(st))

def run_gate(+st: M.State, +r: Maybe<&2, ML.Run>) -> {MD.Runs.gate(project(st), r) == MD.Runs.gate(st, r) : Maybe<&2, ML.Run>}:
  match st r:
    case M.State{_, _, _, _, _, _, _, _, _, _, _} None{}: {==}
    case M.State{_, _, _, _, _, _, _, _, _, _, _} Some{r}: {==}

def run_is.or(+p: Bool, +a: Maybe<&2, ML.Run>, +b: Maybe<&2, ML.Run>, h: {a == b : Maybe<&2, ML.Run>}) ->
  {Bool.or(p, MR.Run.is.got(a)) == Bool.or(p, MR.Run.is.got(b)) : Bool}:
  match p:
    case True{}: {==}
    case False{}: Equal.cong(Maybe<&2, ML.Run>, Bool, x => MR.Run.is.got(x), a, b, h)

def run_is(+st: M.State, +th: String) -> {MR.Run.is(project(st), th) == MR.Run.is(st, th) : Bool}:
  match st:
    case M.State{_, _, _, _, _, _, _, _, _, _, _}:
      run_is.or(PM.Private.is(st, th),
        MD.Runs.gate(project(st), ML.Box.run_of(M.State.mail(st), th)), MD.Runs.gate(st, ML.Box.run_of(M.State.mail(st), th)), run_gate(st, ML.Box.run_of(M.State.mail(st), th)))

def runs(+xs: List<&2, M.Thread>, +st: M.State) -> {U.Runs.keep(xs, project(st)) == U.Runs.keep(xs, st) : List<&2, M.Thread>}:
  match xs:
    case Nil{}: {==}
    case +t <> +rest:
      Equal.trans(List<&2, M.Thread>,
        U.Runs.add(MR.Run.is(project(st), M.Thread.id(t)), t, U.Runs.keep(rest, project(st))),
        U.Runs.add(MR.Run.is(st, M.Thread.id(t)), t, U.Runs.keep(rest, project(st))),
        U.Runs.add(MR.Run.is(st, M.Thread.id(t)), t, U.Runs.keep(rest, st)),
        Equal.cong(Bool, List<&2, M.Thread>, b => U.Runs.add(b, t, U.Runs.keep(rest, project(st))),
          MR.Run.is(project(st), M.Thread.id(t)), MR.Run.is(st, M.Thread.id(t)), run_is(st, M.Thread.id(t))),
        Equal.cong(List<&2, M.Thread>, List<&2, M.Thread>, tail => U.Runs.add(MR.Run.is(st, M.Thread.id(t)), t, tail),
          U.Runs.keep(rest, project(st)), U.Runs.keep(rest, st), runs(rest, st)))

def row(+st: M.State, +th: String, +acc: Map<&2, Nat>) -> {M.Work.rows.put(project(st), th, acc) == M.Work.rows.put(st, th, acc) : Map<&2, Nat>}:
  match st th:
    case M.State{_, _, _, _, _, _, _, _, _, _, _} SNil{}: {==}
    case M.State{_, _, _, _, _, _, _, _, _, _, _} SCon{_, _}: {==}

def row_ids(+keys: List<&2, String>, +n: Nat, +st: M.State, +acc: Map<&2, Nat>) -> {M.Work.rows.ids(keys, n, project(st), acc) == M.Work.rows.ids(keys, n, st, acc) : Map<&2, Nat>}:
  match keys:
    case Nil{}: {==}
    case +k <> +rest:
      Equal.trans(Map<&2, Nat>,
        M.Work.rows.ids(rest, n, project(st), M.Work.rows.put(project(st), String.drop(k, n), acc)),
        M.Work.rows.ids(rest, n, project(st), M.Work.rows.put(st, String.drop(k, n), acc)),
        M.Work.rows.ids(rest, n, st, M.Work.rows.put(st, String.drop(k, n), acc)),
        Equal.cong(Map<&2, Nat>, Map<&2, Nat>, m => M.Work.rows.ids(rest, n, project(st), m),
          M.Work.rows.put(project(st), String.drop(k, n), acc), M.Work.rows.put(st, String.drop(k, n), acc), row(st, String.drop(k, n), acc)),
        row_ids(rest, n, st, M.Work.rows.put(st, String.drop(k, n), acc)))

def rows(+st: M.State) -> {M.Work.rows.map(project(st)) == M.Work.rows.map(st) : Map<&2, Nat>}:
  match st:
    case M.State{_, _, _, _, _, _, _, _, _, _, _}:
      +bg = M.Setting.keys.pre(st, "bg.")
      +watch = M.Setting.keys.pre(st, "watch.")
      +empty : Map<&2, Nat> = Map.new(&2, Nat)
      +a = M.Work.rows.ids(bg, 3n, project(st), empty)
      +b = M.Work.rows.ids(bg, 3n, st, empty)
      Equal.trans(Map<&2, Nat>, M.Work.rows.ids(watch, 6n, project(st), a), M.Work.rows.ids(watch, 6n, project(st), b), M.Work.rows.ids(watch, 6n, st, b),
        Equal.cong(Map<&2, Nat>, Map<&2, Nat>, m => M.Work.rows.ids(watch, 6n, project(st), m), a, b, row_ids(bg, 3n, st, empty)), row_ids(watch, 6n, st, b))

def exact(+st: M.State) -> {U.Side.ix.inputs(U.SidebarInput.of(st)) == U.Side.ix(st) : U.SIx}:
  match st:
    case M.State{_, +ts, _, _, _, _, _, _, _, _, _}:
      +a = U.Runs.keep(Map.values(&2, M.Thread, ts), project(st))
      +b = U.Runs.keep(Map.values(&2, M.Thread, ts), st)
      +kids = U.Kids.set(st)
      +asks = U.SIx.asks.go(List.reverse(&2, M.Ask, M.Asks.all(st)), Map.new(&2, List<&2, M.Ask>))
      +counts = M.Work.kids.map(st)
      +new_work = M.Work.rows.map(project(st))
      +old_work = M.Work.rows.map(st)
      Equal.trans(U.SIx,
        U.SIx{U.SIx.by.go(List.reverse(&2, M.Thread, U.Kids.keep(a, kids)), Map.new(&2, List<&2, M.Thread>)), asks, counts, new_work},
        U.SIx{U.SIx.by.go(List.reverse(&2, M.Thread, U.Kids.keep(b, kids)), Map.new(&2, List<&2, M.Thread>)), asks, counts, new_work},
        U.SIx{U.SIx.by.go(List.reverse(&2, M.Thread, U.Kids.keep(b, kids)), Map.new(&2, List<&2, M.Thread>)), asks, counts, old_work},
        Equal.cong(List<&2, M.Thread>, U.SIx, xs => U.SIx{U.SIx.by.go(List.reverse(&2, M.Thread, U.Kids.keep(xs, kids)), Map.new(&2, List<&2, M.Thread>)), asks, counts, new_work}, a, b, runs(Map.values(&2, M.Thread, ts), st)),
        Equal.cong(Map<&2, Nat>, U.SIx, m => U.SIx{U.SIx.by.go(List.reverse(&2, M.Thread, U.Kids.keep(b, kids)), Map.new(&2, List<&2, M.Thread>)), asks, counts, m}, new_work, old_work, rows(st)))
