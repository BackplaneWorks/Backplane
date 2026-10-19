import Base
import ../../src/app/side_row_types.bend as ST
import ../../src/core/client.bend as U
import ../../src/core/model.bend as M
import ../../src/core/json.bend as J
import ../../src/core/utf8.bend as U8
import ../../src/app/layout.bend as L
import ../../src/app/nui.bend as N
import ../../src/app/bots.bend as BV
import ../../src/app/cats.bend as CA
import ../../src/app/mm.bend as MM
import ../../src/gfx/draw.bend as D
import ../../src/gfx/raster.bend as R
import ../../src/gfx/text.bend as X
import ../../src/server/perf.bend as P
import ../../src/server/argv.bend as AV
import ./memo_replay.bend as MR
import ../../src/app/side_rows.bend as SR

# PR3 component baseline, before any row cache. No hub, provider, window,
# rasterizer or mutable home. Every timed result is consumed at an IO
# boundary. Membership and paint timings are independent components,
# not a subtraction estimate. The index and paint lists are made outside
# the paint timers, as the window already does for its index.
#   build/sidebar_rows EVENTS THREAD [REPEATS]
type Fixture is Data:
  Fixture{ty: List<&2, L.Ty>, ns: List<&2, N.Nui>, ix: List<&2, U.SIx>, groups: List<&2, U.SGroups>, active: List<&2, U.ARow>, br: List<&2, BV.BRows>, cats: List<&2, Map<&2, CA.CatF>>, wraps: List<&2, Map<&2, BV.PW>>}

def Fixture.n(ns: List<&2, N.Nui>) -> N.Nui:
  match ns:
    case n <> _:
      n
    case Nil{}:
      N.Nui.init(U.Ui.init(0n), 1400, 900)

# Keep repeated row inputs narrow: unpack the deep Nui inside this def.
def Fixture.u(ns: List<&2, N.Nui>) -> U.Ui:
  N.Nui.ui(Fixture.n(ns))

def Fixture.br(bs: List<&2, BV.BRows>) -> BV.BRows:
  match bs:
    case b <> _: b
    case Nil{}: BV.BRows.of(U.Ui.init(0n))

def Fixture.with_ty.one(f: Fixture, ty: L.Ty) -> Fixture:
  match f:
    case Fixture{_, ns, ix, groups, active, br, cats, wraps}:
      Fixture{[ty], ns, ix, groups, active, br, cats, wraps}

# Keep the deep fixture behind a list across IO continuations: native
# segment arguments have an arity limit, unlike the type checker.
def Fixture.with_ty(fs: List<&2, Fixture>, ty: L.Ty) -> List<&2, Fixture>:
  match fs:
    case Nil{}: Nil{}
    case f <> _: [Fixture.with_ty.one(f, ty)]

def Fixture.clip() -> D.Clip:
  D.Clip{0, 96, 264, 852}

def Members.threads(ts: List<&2, M.Thread>) -> List<&2, N.Hit>:
  match ts:
    case Nil{}:
      Nil{}
    case t <> rest:
      L.Lay.hit(0, 0, 0, 0, "thread", M.Thread.id(t)) <> Members.threads(rest)

def Members.projects(rs: List<&2, U.SRow>) -> List<&2, N.Hit>:
  match rs:
    case Nil{}:
      Nil{}
    case U.SRow{_, M.Project{id, _, _, _, _}, M.Shelves{p, a, s, h, sn}} <> rest:
      L.Lay.hit(0, 0, 0, 0, "project", id) <> List.concat(&2, N.Hit, [Members.threads(p), Members.threads(a), Members.threads(s), Members.threads(h), Members.threads(sn), Members.projects(rest)])

def Members.active(rs: List<&2, U.ARow>) -> List<&2, N.Hit>:
  match rs:
    case Nil{}:
      Nil{}
    case U.ARow{_, _, t, _, _, _} <> rest:
      L.Lay.hit(0, 0, 0, 0, "thread", M.Thread.id(t)) <> Members.active(rest)

def Bench.col(c: L.Col) -> L.Out:
  match c:
    case L.Col{o, _}:
      o

# 0/1: fresh membership, forced as identifiers; 2/3: existing row paint
# over prebuilt membership; 4: the full sidebar including bots and links.
def Fixture.ix(xs: List<&2, U.SIx>) -> U.SIx:
  match xs:
    case ix <> _: ix
    case Nil{}: U.Side.ix(M.State.empty())

def Fixture.groups(xs: List<&2, U.SGroups>) -> U.SGroups:
  match xs:
    case g <> _: g
    case Nil{}: U.SGroups{Nil{}, Nil{}, Nil{}}

def Fixture.cats(xs: List<&2, Map<&2, CA.CatF>>) -> Map<&2, CA.CatF>:
  match xs:
    case c <> _: c
    case Nil{}: CA.Cats.none()

def Fixture.wraps(xs: List<&2, Map<&2, BV.PW>>) -> Map<&2, BV.PW>:
  match xs:
    case w <> _: w
    case Nil{}: BV.Bv.wraps.none()

def Fixture.typ(xs: List<&2, L.Ty>) -> L.Ty:
  match xs:
    case t <> _: t
    case Nil{}: L.Ty.none()

def Bench.render.one(kind: Nat, f: Fixture) -> L.Out:
  match kind f:
    case 0n Fixture{_, +ns, +ix, _, _, _, _, _}:
      L.Out{Nil{}, Members.projects(U.SGroups.all(U.Side.groups(U.Side.shown.ix(U.Ui.state(Fixture.u(ns)), Fixture.ix(ix), U.Side.now(Fixture.u(ns)), ""), U.Side.now(Fixture.u(ns)))))}
    case 1n Fixture{_, +ns, +ix, _, _, _, _, _}:
      L.Out{Nil{}, Members.active(U.Active.rows.ix(U.Ui.state(Fixture.u(ns)), Fixture.ix(ix), U.Side.now(Fixture.u(ns)), ""))}
    case 2n Fixture{ty, +ns, ix, groups, _, _, _, _}:
      Bench.col(L.Lay.groups(Fixture.typ(ty), Fixture.groups(groups), "", Map.new(&2, Bool), Fixture.ix(ix), U.Ui.state(Fixture.u(ns)), U.Ui.sel(Fixture.u(ns)), U.Side.now(Fixture.u(ns)), "", "", Fixture.clip(), 96))
    case 3n Fixture{ty, +ns, ix, _, active, _, _, _}:
      Bench.col(L.Lay.active(Fixture.typ(ty), active, "", Map.new(&2, Bool), Fixture.ix(ix), U.Ui.state(Fixture.u(ns)), U.Ui.sel(Fixture.u(ns)), Fixture.clip(), 96))
    case _ Fixture{ty, +ns, ix, _, _, br, cats, wraps}:
      BV.Bv.side.a.ixr(Fixture.typ(ty), Fixture.cats(cats), Fixture.wraps(wraps), Fixture.n(ns), Fixture.ix(ix), Fixture.br(br))

def Bench.render(kind: Nat, fs: List<&2, Fixture>) -> L.Out:
  match fs:
    case Nil{}: L.Out.none()
    case f <> _: Bench.render.one(kind, f)

def Bench.hits(hs: List<&2, N.Hit>, +acc: Nat) -> Nat:
  match hs:
    case Nil{}:
      acc
    case N.Hit{x, y, x1, y1, a, v} <> rest:
      Bench.hits(rest, Nat.mod(Nat.add(acc, Nat.add(Nat.add(String.length(a), String.length(v)), U32.to_nat((x + y + x1 + y1 : U32)))), 1000000n))

def Bench.force(+o: L.Out) -> Nat:
  Nat.add(List.length(&2, X.Want, X.Text.wants(MR.ops(o), Nil{})), Nat.add(List.length(&2, N.Hit, MR.hits(o)), Nat.add(List.length(&2, R.Op, MR.ops(o)), Bench.hits(MR.hits(o), 0n))))

def Bench.name(kind: Nat) -> String:
  match kind:
    case 0n: "membership.projects"
    case 1n: "membership.active"
    case 2n: "paint.projects"
    case 3n: "paint.active"
    case _: "sidebar.whole"

def Bench.one(+kind: Nat, +f: List<&2, Fixture>) -> IO(Nat):
  do IO<Nat>:
    +a : U32 <- P.Perf.us()
    +o : L.Out = Bench.render(kind, f)
    P.Perf.add("sidebar.output", U32.from_nat(Bench.force(o)))
    +b : U32 <- P.Perf.us()
    return U32.to_nat((b - a : U32))

def Bench.at(+xs: List<&2, Nat>, i: Nat) -> Nat:
  Maybe.default(&2, Nat, List.head(&2, Nat, List.drop(&2, Nat, xs, i)), 0n)

def Bench.sum(xs: List<&2, Nat>, +acc: Nat) -> Nat:
  match xs:
    case Nil{}: acc
    case x <> rest: Bench.sum(rest, Nat.add(acc, x))

def Bench.report(+kind: Nat, +f: List<&2, Fixture>, +xs: List<&2, Nat>) -> IO(Unit):
  do IO<Unit>:
    +sorted : List<&2, Nat> = List.sort(~Nat, ~Nat.is_lt, xs)
    +n : Nat = List.length(&2, Nat, xs)
    IO.print(Bench.name(kind) ++ " n=" ++ Nat.show(n) ++ " total_us=" ++ Nat.show(Bench.sum(xs, 0n))
      ++ " p50_us=" ++ Nat.show(Bench.at(sorted, Nat.div(n, 2n))) ++ " p99_us=" ++ Nat.show(Bench.at(sorted, Nat.sub(Nat.div(Nat.add(Nat.mul(n, 99n), 99n), 100n), 1n)))
      ++ " checksum=" ++ Nat.show(Bench.force(Bench.render(kind, f))))

def Bench.run(fuel: Nat, +kind: Nat, +f: List<&2, Fixture>, xs: List<&2, Nat>) -> IO(Unit):
  match fuel:
    case 0n:
      Bench.report(kind, f, xs)
    case 1n+left:
      IO.bind(Nat, Unit, Bench.one(kind, f), us => Bench.run(left, kind, f, us <> xs))

def Fixture.make(+tys: List<&2, L.Ty>, +ns: List<&2, N.Nui>, +ixs: List<&2, U.SIx>, +brs: List<&2, BV.BRows>) -> List<&2, Fixture>:
  [Fixture{tys, ns, ixs, [U.Side.groups(U.Side.shown.ix(U.Ui.state(N.Nui.ui(Fixture.n(ns))), Fixture.ix(ixs), U.Side.now(N.Nui.ui(Fixture.n(ns))), ""), U.Side.now(N.Nui.ui(Fixture.n(ns))))],
    U.Active.rows.ix(U.Ui.state(N.Nui.ui(Fixture.n(ns))), Fixture.ix(ixs), U.Side.now(N.Nui.ui(Fixture.n(ns))), ""), brs,
    [BV.Bv.warm.r(Fixture.br(brs), CA.Cats.none(), BV.Bv.clock.of(Fixture.n(ns)))],
    [BV.Bv.wraps.ix(Fixture.typ(tys), Fixture.n(ns), BV.Bv.wraps.none(), Fixture.ix(ixs))]}]

def Fixture.wants(+f: List<&2, Fixture>) -> List<&2, X.Want>:
  X.Text.wants(MR.ops(L.Out.all([Bench.render(2n, f), Bench.render(3n, f), Bench.render(4n, f)])), Nil{})

def Fixture.ty(fs: List<&2, Fixture>) -> L.Ty:
  match fs:
    case Nil{}: L.Ty.none()
    case Fixture{ty, _, _, _, _, _, _, _} <> _: Fixture.typ(ty)

def Bench.all(+f: List<&2, Fixture>, +n: Nat) -> IO(Unit):
  do IO<Unit>:
    Bench.run(n, 0n, f, Nil{})
    Bench.run(n, 1n, f, Nil{})
    Bench.run(n, 2n, f, Nil{})
    Bench.run(n, 3n, f, Nil{})
    Bench.run(n, 4n, f, Nil{})

# Each IO continuation carries only a list pointer to the deep state.
# Do not retain raw Ui/Nui or several typography records while warming.
def Fixture.ui(us: List<&2, U.Ui>) -> U.Ui:
  match us:
    case u <> _: u
    case Nil{}: U.Ui.init(0n)

def Fixture.from_ui(tys: List<&2, L.Ty>, +us: List<&2, U.Ui>) -> List<&2, Fixture>:
  Fixture.make(tys, [MR.prepared(N.Nui.init(Fixture.ui(us), 1400, 900), 0n)],
    [U.Side.ix(U.Ui.state(Fixture.ui(us)))], [BV.BRows.of(Fixture.ui(us))])

def Fixture.events(tys: List<&2, L.Ty>, +xs: List<&2, J.Json>, thread: String) -> List<&2, Fixture>:
  +last = Maybe.default(&2, J.Json, List.head(&2, J.Json, List.reverse(&2, J.Json, xs)), J.Null{})
  Fixture.from_ui(tys, [U.Ui.tick(MR.real(xs, thread), J.Json.get_nat(last, "at", 1790180000n))])

def Fixture.clock(fs: List<&2, Fixture>) -> Nat:
  match fs:
    case Nil{}: 0n
    case Fixture{_, ns, _, _, _, _, _, _} <> _:
      U.Side.now(N.Nui.ui(Fixture.n(ns)))

# The row experiment uses the original layout's hit positions and its
# prebuilt membership. Request preparation is outside both timers;
# presentation resolution, cache lookup, bounded map construction and
# output assembly are inside the memo timer. The fresh timer calls the
# original row functions without constructing keys or maps.
type RowPlan is Data:
  RowPlan{requests: List<&2, ST.SideRowReq>, context: SR.SideRowCtx, rows: Map<&2, ST.SideRowM>, name: String}

def Plan.context(fs: List<&2, Fixture>) -> SR.SideRowCtx:
  match fs:
    case Nil{}: SR.SideRowCtx{Nil{}, Nil{}, Nil{}, "", 0n, 0n}
    case Fixture{ty, +ns, ix, _, _, _, _, _} <> _:
      SR.SideRowCtx{ty, [U.Ui.state(Fixture.u(ns))], ix, U.Ui.sel(Fixture.u(ns)), U.Side.now(Fixture.u(ns)), 0n}

def Plan.active.rows(fs: List<&2, Fixture>) -> List<&2, U.ARow>:
  match fs:
    case Nil{}: Nil{}
    case Fixture{_, _, _, _, rows, _, _, _} <> _: rows

def Plan.active.map(rs: List<&2, U.ARow>, m: Map<&2, Maybe<&2, U.ARow>>) -> Map<&2, Maybe<&2, U.ARow>>:
  match rs:
    case Nil{}: m
    case +r <> rest:
      Plan.active.map(rest, Map.set(&2, Maybe<&2, U.ARow>, m, L.Lay.arow.id(r), Some{r}))

def Plan.active.get(r: Map<&2, Maybe<&2, U.ARow>> & Maybe<&2, U.ARow>) -> Maybe<&2, U.ARow>:
  (_, row) = r
  row

def Plan.active.request(row: Maybe<&2, U.ARow>, y: U32) -> List<&2, ST.SideRowReq>:
  match row:
    case None{}: Nil{}
    case Some{r}: [ST.SideActiveRow{r, Fixture.clip(), y}]

def Plan.request(active: Bool, +id: String, y: U32, ctx: SR.SideRowCtx, rows: Map<&2, Maybe<&2, U.ARow>>) -> List<&2, ST.SideRowReq>:
  match active ctx:
    case False{} SR.SideRowCtx{_, +st, _, _, now, _}:
      [ST.SideProjectRow{U.Ui.thread(SR.Ctx.state(st), id), Nat.is_gt(U.Side.until(SR.Ctx.state(st), id), now), Fixture.clip(), y}]
    case True{} _:
      Plan.active.request(Plan.active.get(Map.get(Maybe<&2, U.ARow>, None{}, rows, id)), y)

def Plan.hits.keep(select: Bool, +id: String, y: U32, +active: Bool, +ctx: SR.SideRowCtx, +rows: Map<&2, Maybe<&2, U.ARow>>) -> List<&2, ST.SideRowReq>:
  match select:
    case False{}: Nil{}
    case True{}: Plan.request(active, id, y, ctx, rows)

def Plan.hits(hs: List<&2, N.Hit>, +active: Bool, +ctx: SR.SideRowCtx, +rows: Map<&2, Maybe<&2, U.ARow>>) -> List<&2, ST.SideRowReq>:
  match hs:
    case Nil{}: Nil{}
    case N.Hit{_, y, _, _, action, id} <> rest:
      List.append(&2, ST.SideRowReq, Plan.hits.keep(String.eq(action, "select"), id, y, active, ctx, rows), Plan.hits(rest, active, ctx, rows))

def Plan.make(+requests: List<&2, ST.SideRowReq>, +ctx: SR.SideRowCtx, name: String) -> RowPlan:
  RowPlan{requests, ctx, SR.Batch.rows(SR.Batch.memo(requests, ctx, Map.new(&2, ST.SideRowM))), name}

def Plan.of(+f: List<&2, Fixture>, +active: Bool) -> RowPlan:
  +ctx = Plan.context(f)
  Plan.make(Plan.hits(MR.hits(Bench.render(Bool.pick(Nat, active, 3n, 2n), f)), active, ctx, Plan.active.map(Plan.active.rows(f), Map.new(&2, Maybe<&2, U.ARow>))), ctx, Bool.pick(String, active, "rows.active", "rows.projects"))

def RowBench.fresh(p: RowPlan) -> L.Out:
  match p:
    case RowPlan{reqs, ctx, _, _}: SR.Batch.fresh(reqs, ctx)

def RowBench.memo(p: RowPlan) -> SR.SideRowRun:
  match p:
    case RowPlan{reqs, ctx, rows, _}: SR.Batch.memo(reqs, ctx, rows)

def RowBench.check(+p: RowPlan) -> IO(Unit):
  do IO<Unit>:
    +hot : SR.SideRowRun = RowBench.memo(p)
    +fresh : L.Out = RowBench.fresh(p)
    MR.exact(MR.Eqh.out(SR.Batch.out(hot), fresh), "real-snapshot hot rows exactly equal fresh ops and hits")
    MR.exact(Nat.is_gt(SR.Batch.count(hot), 0n), "real snapshot has visible rows")
    MR.exact(Nat.is_eq(SR.Batch.hits(hot), SR.Batch.count(hot)), "every retained visible row hits")
    MR.exact(List.is_empty(&2, X.Want, X.Text.wants(MR.ops(fresh), Nil{})), "row fonts warmed")
    IO.print("row exactness: visible=" ++ Nat.show(SR.Batch.count(hot)) ++ " hits=" ++ Nat.show(SR.Batch.hits(hot)) ++ " checksum=" ++ Nat.show(Bench.force(fresh)))

type RowTimed is Data:
  RowTimed{out: L.Out, bookkeeping: Nat}

def RowBench.memo.work(+run: SR.SideRowRun) -> RowTimed:
  RowTimed{SR.Batch.out(run), Nat.add(SR.Batch.hits(run), List.length(&2, String, Map.keys(&2, ST.SideRowM, SR.Batch.rows(run))))}

def RowBench.work(memo: Bool, p: RowPlan) -> RowTimed:
  match memo:
    case False{}: RowTimed{RowBench.fresh(p), 0n}
    case True{}: RowBench.memo.work(RowBench.memo(p))

def RowBench.force(work: RowTimed) -> Nat:
  match work:
    case RowTimed{o, bookkeeping}: Nat.add(Bench.force(o), bookkeeping)

def RowBench.one(+memo: Bool, +p: RowPlan) -> IO(Nat):
  do IO<Nat>:
    +a : U32 <- P.Perf.us()
    +work : RowTimed = RowBench.work(memo, p)
    P.Perf.add("sidebar.rows.output", U32.from_nat(RowBench.force(work)))
    +b : U32 <- P.Perf.us()
    return U32.to_nat((b - a : U32))

def RowBench.report(memo: Bool, p: RowPlan, +xs: List<&2, Nat>, label: String) -> IO(Unit):
  match p:
    case RowPlan{_, _, _, name}:
      do IO<Unit>:
        +sorted : List<&2, Nat> = List.sort(~Nat, ~Nat.is_lt, xs)
        +n : Nat = List.length(&2, Nat, xs)
        IO.print(name ++ "." ++ label ++ Bool.pick(String, memo, ".memo", ".fresh") ++ " n=" ++ Nat.show(n) ++ " total_us=" ++ Nat.show(Bench.sum(xs, 0n))
          ++ " p50_us=" ++ Nat.show(Bench.at(sorted, Nat.div(n, 2n))) ++ " p99_us=" ++ Nat.show(Bench.at(sorted, Nat.sub(Nat.div(Nat.add(Nat.mul(n, 99n), 99n), 100n), 1n))))

def RowBench.run(fuel: Nat, +memo: Bool, +p: RowPlan, xs: List<&2, Nat>, +label: String) -> IO(Unit):
  match fuel:
    case 0n: RowBench.report(memo, p, xs, label)
    case 1n+left:
      IO.bind(Nat, Unit, RowBench.one(memo, p), us => RowBench.run(left, memo, p, us <> xs, label))

def RowBench.abba(+p: RowPlan, +n: Nat) -> IO(Unit):
  do IO<Unit>:
    RowBench.check(p)
    RowBench.run(n, False{}, p, Nil{}, "1A")
    RowBench.run(n, True{}, p, Nil{}, "2B")
    RowBench.run(n, True{}, p, Nil{}, "3B")
    RowBench.run(n, False{}, p, Nil{}, "4A")

# Full sidebar: both paths redo membership, headers, links and bot rows.
# Only B retains project/thread row paint. Its rebuilt visible-row map
# and exact input resolution are consumed inside the measured interval.
def Full.paint.one(f: Fixture, old: List<&2, MM.SideM>) -> List<&2, MM.SidePaint>:
  match f:
    case Fixture{ty, ns, ix, _, _, _, _, _}:
      MM.SideM.paint(Fixture.typ(ty), Fixture.n(ns), Fixture.ix(ix), old)

def Full.paint(fs: List<&2, Fixture>, old: List<&2, MM.SideM>) -> List<&2, MM.SidePaint>:
  match fs:
    case Nil{}: Nil{}
    case f <> _: Full.paint.one(f, old)

def Full.memo.one(f: Fixture, paint: List<&2, MM.SidePaint>) -> L.SideCol:
  match f:
    case Fixture{ty, ns, ix, _, _, br, cats, wraps}:
      BV.Bv.side.rows.ixr(~(List<&2, MM.SidePaint>), ~MM.SideM.row, Fixture.typ(ty), Fixture.cats(cats), Fixture.wraps(wraps), Fixture.n(ns), Fixture.ix(ix), Fixture.br(br), paint)

def Full.memo.ready(fs: List<&2, Fixture>, paint: List<&2, MM.SidePaint>) -> L.SideCol:
  match fs:
    case Nil{}: L.SideCol{L.Out.none(), 0, Nil{}, 0n}
    case f <> _: Full.memo.one(f, paint)

def Full.memo(+fs: List<&2, Fixture>, old: List<&2, MM.SideM>) -> L.SideCol:
  Full.memo.ready(fs, Full.paint(fs, old))

def Full.keep(+side: L.SideCol) -> List<&2, MM.SideM>:
  [MM.SideM.from("harness", side)]

def Full.timed(+side: L.SideCol) -> RowTimed:
  RowTimed{L.SideCol.out(side), Nat.add(L.SideCol.hits(side),
    List.length(&2, String, Map.keys(&2, ST.SideRowM, MM.SideM.rows(Full.keep(side)))))}

def Full.work(memo: Bool, f: List<&2, Fixture>, old: List<&2, MM.SideM>) -> RowTimed:
  match memo:
    case False{}: RowTimed{Bench.render(4n, f), 0n}
    case True{}: Full.timed(Full.memo(f, old))

def Full.one(+memo: Bool, +f: List<&2, Fixture>, +old: List<&2, MM.SideM>) -> IO(Nat):
  do IO<Nat>:
    +a : U32 <- P.Perf.us()
    +work : RowTimed = Full.work(memo, f, old)
    P.Perf.add("sidebar.full.output", U32.from_nat(RowBench.force(work)))
    +b : U32 <- P.Perf.us()
    return U32.to_nat((b - a : U32))

def Full.report(+xs: List<&2, Nat>, name: String) -> IO(Unit):
  do IO<Unit>:
    +sorted : List<&2, Nat> = List.sort(~Nat, ~Nat.is_lt, xs)
    +n : Nat = List.length(&2, Nat, xs)
    IO.print(name ++ " n=" ++ Nat.show(n) ++ " total_us=" ++ Nat.show(Bench.sum(xs, 0n))
      ++ " p50_us=" ++ Nat.show(Bench.at(sorted, Nat.div(n, 2n)))
      ++ " p99_us=" ++ Nat.show(Bench.at(sorted, Nat.sub(Nat.div(Nat.add(Nat.mul(n, 99n), 99n), 100n), 1n))))

def Full.run(fuel: Nat, +memo: Bool, +f: List<&2, Fixture>, +old: List<&2, MM.SideM>, xs: List<&2, Nat>, +name: String) -> IO(Unit):
  match fuel:
    case 0n: Full.report(xs, name)
    case 1n+left:
      IO.bind(Nat, Unit, Full.one(memo, f, old), us => Full.run(left, memo, f, old, us <> xs, name))

def Full.check(+f: List<&2, Fixture>, +old: List<&2, MM.SideM>, label: String) -> IO(Unit):
  do IO<Unit>:
    +side : L.SideCol = Full.memo(f, old)
    MR.exact(MR.Eqh.out(L.SideCol.out(side), Bench.render(4n, f)), label)
    MR.exact(Nat.is_eq(List.length(&2, ST.SideRowM, L.SideCol.rows(side)),
      List.length(&2, String, Map.keys(&2, ST.SideRowM, MM.SideM.rows(Full.keep(side))))), "visible row map has no duplicate identifiers")

def Full.abba(+f: List<&2, Fixture>, +n: Nat, +name: String) -> IO(Unit):
  do IO<Unit>:
    +cold : L.SideCol = Full.memo(f, Nil{})
    +old : List<&2, MM.SideM> = Full.keep(cold)
    +hot : L.SideCol = Full.memo(f, old)
    Full.check(f, Nil{}, name ++ " cold exact")
    Full.check(f, old, name ++ " hot exact")
    MR.exact(Nat.is_gt(List.length(&2, ST.SideRowM, L.SideCol.rows(hot)), 0n), "full sidebar contains visible rows")
    MR.exact(Nat.is_eq(L.SideCol.hits(hot), List.length(&2, ST.SideRowM, L.SideCol.rows(hot))), "all unchanged visible rows hit")
    MR.exact(List.is_empty(&2, X.Want, X.Text.wants(MR.ops(Bench.render(4n, f)), Nil{})), "full sidebar timing glyphs warmed")
    IO.print(name ++ " visible=" ++ Nat.show(List.length(&2, ST.SideRowM, L.SideCol.rows(hot))) ++ " hits=" ++ Nat.show(L.SideCol.hits(hot)))
    Full.run(n, False{}, f, old, Nil{}, name ++ ".1A.fresh")
    Full.run(n, True{}, f, old, Nil{}, name ++ ".2B.memo")
    Full.run(n, True{}, f, old, Nil{}, name ++ ".3B.memo")
    Full.run(n, False{}, f, old, Nil{}, name ++ ".4A.fresh")

def Full.ui(fs: List<&2, Fixture>) -> U.Ui:
  match fs:
    case Nil{}: U.Ui.init(0n)
    case Fixture{_, ns, _, _, _, _, _, _} <> _: Fixture.u(ns)

# Rebuild the same derived index/bot inputs for both painters after a
# client change. This setup is outside the paint-only timing interval.
def Full.reframe.one(f: Fixture, +ns: List<&2, N.Nui>) -> List<&2, Fixture>:
  match f:
    case Fixture{ty, _, _, groups, active, _, cats, wraps}:
      [Fixture{ty, ns, [U.Side.ix(U.Ui.state(Fixture.u(ns)))], groups, active,
        [BV.BRows.of(Fixture.u(ns))], cats, wraps}]

def Full.reframe(fs: List<&2, Fixture>, ns: List<&2, N.Nui>) -> List<&2, Fixture>:
  match fs:
    case Nil{}: Nil{}
    case f <> _: Full.reframe.one(f, ns)

def Full.nui(fs: List<&2, Fixture>) -> N.Nui:
  match fs:
    case Nil{}: Fixture.n(Nil{})
    case Fixture{_, ns, _, _, _, _, _, _} <> _: Fixture.n(ns)

def Full.edit.ui(kind: Nat, +u: U.Ui) -> U.Ui:
  match kind:
    case 0n: u
    case 1n: U.Ui.tick(u, Nat.add(U.Side.now(u), 1n))
    case 2n: U.Ui.tick(u, Nat.add(U.Side.now(u), 60n))
    case 3n: U.Ui.tick(u, Nat.add(U.Side.now(u), 3600n))
    case 4n: U.Ui.tick(u, Nat.add(U.Side.now(u), 86400n))
    case 5n: U.Act.ui(U.Ui.select(u, U.Side.nth(U.Side.visible(u), 1n)))
    case 6n: U.Ui.setting.here(u, ["sidebar.view", "active"])
    case 7n: U.Ui.setting.here(u, ["sidebar.view", "projects"])
    case 8n: U.Ui.scratch_set(u, "@f.pq", "unmatchablexyz")
    case 9n: U.Ui.scratch_set(u, "@f.pq", "")
    case 10n: U.Ui.flag_on(u, "old-projects")
    case 11n: U.Ui.flag_on(u, "active-settled")
    case _: u

def Full.edit.geometry(kind: Nat, n: N.Nui) -> N.Nui:
  match kind n:
    case 12n N.Nui{u, w, h, mx, my, _, sc, ca, fo, pa, hs, ms, vw}:
      N.Nui{u, w, h, mx, my, 64, sc, ca, fo, pa, hs, ms, vw}
    case 13n N.Nui{u, w, h, mx, my, _, sc, ca, fo, pa, hs, ms, vw}:
      N.Nui{u, w, h, mx, my, 1024, sc, ca, fo, pa, hs, ms, vw}
    case 14n _:
      MR.step_nui(N.Nui.resize(n, 1100, 500))
    case _ _: n

def Full.edit.ui.boxed(kind: Nat, ns: List<&2, N.Nui>) -> U.Ui:
  Full.edit.ui(kind, Fixture.u(ns))

def Full.edit.nui(+kind: Nat, +ns: List<&2, N.Nui>) -> N.Nui:
  Full.edit.geometry(kind, N.Nui.with_ui(Fixture.n(ns), Full.edit.ui.boxed(kind, ns)))

def Full.edit(+f: List<&2, Fixture>, kind: Nat) -> List<&2, Fixture>:
  Full.reframe(f, [Full.edit.nui(kind, [Full.nui(f)])])

def Full.inputs(fuel: Nat, +f: List<&2, Fixture>, +old: List<&2, MM.SideM>, +i: Nat) -> IO(Unit):
  match fuel:
    case 0n: IO.print("full sidebar input replay: 16 states, zero stale ops/hits")
    case 1n+left:
      do IO<Unit>:
        +next : List<&2, Fixture> = Full.edit(f, i)
        Full.check(next, old, "full sidebar changed input " ++ Nat.show(i))
        Full.inputs(left, next, Full.keep(Full.memo(next, old)), 1n+i)

def Full.events(xs: List<&2, J.Json>, +f: List<&2, Fixture>, +old: List<&2, MM.SideM>, +i: Nat) -> IO(Unit):
  match xs:
    case Nil{}: IO.print("full sidebar real-event replay: " ++ Nat.show(i) ++ " events, zero stale ops/hits")
    case +x <> rest:
      do IO<Unit>:
        +next : List<&2, Fixture> = Full.reframe(f, [MR.ss.recv(Full.nui(f), MR.msg(x), J.Json.get_nat(x, "at", U.Side.now(Full.ui(f))))])
        Full.check(next, old, "full sidebar real event " ++ Nat.show(i))
        Full.events(rest, next, Full.keep(Full.memo(next, old)), 1n+i)

def Full.real(+tys: List<&2, L.Ty>, +xs: List<&2, J.Json>, +thread: String) -> IO(Unit):
  do IO<Unit>:
    +cut : Nat = Nat.sub(List.length(&2, J.Json, xs), 67n)
    +f : List<&2, Fixture> = Fixture.events(tys, List.take(&2, J.Json, xs, cut), thread)
    Full.events(List.drop(&2, J.Json, xs, cut), f, Nil{}, 0n)

def Bench.selected(mode: String, +f: List<&2, Fixture>, +repeats: Nat) -> IO(Unit):
  match mode:
    case "full":
      do IO<Unit>:
        IO.print("full sidebar ABBA: fresh membership/header/bots on both paths; B resolves keys and builds visible map")
        Full.inputs(16n, f, Nil{}, 0n)
        Full.abba(Full.edit(f, 7n), repeats, "full.projects")
        Full.abba(Full.edit(f, 6n), repeats, "full.active")
    case "rows":
      do IO<Unit>:
        IO.print("row ABBA: independent projects then active; current inputs resolved inside memo timer; map and ops forced")
        RowBench.abba(Plan.of(f, False{}), repeats)
        RowBench.abba(Plan.of(f, True{}), repeats)
    case _: Bench.all(f, repeats)

def Bench.ready(+f: List<&2, Fixture>, +repeats: Nat, mode: String) -> IO(Unit):
  do IO<Unit>:
    MR.exact(List.is_empty(&2, X.Want, Fixture.wants(f)), "component baseline glyphs fully warmed")
    MR.exact(Nat.is_gt(repeats, 0n), "positive repeat count")
    IO.print("sidebar baseline: warmed fonts, prebuilt index; paint lists prebuilt; snapshot clock=" ++ Nat.show(Fixture.clock(f)))
    Bench.selected(mode, f, repeats)

def Bench.warm(fuel: Nat, +f: List<&2, Fixture>, +repeats: Nat, +mode: String) -> IO(Unit):
  match fuel:
    case 0n: Bench.ready(f, repeats, mode)
    case 1n+left:
      IO.bind(L.Ty, Unit, L.Ty.fill(Fixture.ty(f), Fixture.wants(f)),
        ty => Bench.warm(left, Fixture.with_ty(f, ty), repeats, mode))

def Bench.loaded.mode(mode: String, +tys: List<&2, L.Ty>, +as: List<&2, String>, +xs: List<&2, J.Json>) -> IO(Unit):
  match mode:
    case "full":
      do IO<Unit>:
        Full.real(tys, xs, MR.arg(as, 1n))
        Bench.warm(2n, Fixture.events(tys, xs, MR.arg(as, 1n)),
          Maybe.default(&2, Nat, Nat.read(MR.arg(as, 2n)), 60n), mode)
    case _:
      Bench.warm(2n, Fixture.events(tys, xs, MR.arg(as, 1n)),
        Maybe.default(&2, Nat, Nat.read(MR.arg(as, 2n)), 60n), mode)

def Bench.loaded(tys: List<&2, L.Ty>, +as: List<&2, String>, bytes: List<&2, U32>) -> IO(Unit):
  Bench.loaded.mode(MR.arg(as, 3n), tys, as, MR.items(MR.lines(U8.Lines.feed(U8.Lines.new(), bytes)), Nil{}))

def Bench.read(tys: List<&2, L.Ty>, +as: List<&2, String>) -> IO(Unit):
  IO.bind(List<&2, U32>, Unit, MR.read(MR.arg(as, 0n)), bytes => Bench.loaded(tys, as, bytes))

def main() -> IO(Unit):
  do IO<Unit>:
    args : List<String> <- AV.Argv.args()
    +as : List<&2, String> = MR.copy(args)
    ty : L.Ty <- L.Ty.load()
    Bench.read([ty], as)
