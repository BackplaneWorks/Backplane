import Base
import ../src/app/side_row_types.bend as ST
import ../src/app/side_rows.bend as SR
import ../src/app/layout.bend as L
import ../src/app/nui.bend as N
import ../src/core/client.bend as U
import ../src/core/model.bend as M
import ../src/gfx/draw.bend as D
import ../src/gfx/tiles.bend as TI
import ./sidebar_test.bend as SD
import ../src/app/mm.bend as MM

def check(ok: Bool, message: String) -> IO(Unit):
  match ok:
    case True{}: IO.pure(Unit, Unit{})
    case False{}: IO.die(Unit, 1, "FAIL side_rows: " ++ message)

def same.hits(xs: List<&2, N.Hit>, ys: List<&2, N.Hit>) -> Bool:
  match xs ys:
    case Nil{} Nil{}: True{}
    case N.Hit{a0, a1, a2, a3, a4, a5} <> at N.Hit{b0, b1, b2, b3, b4, b5} <> bt:
      Bool.and(U32.is_eq(a0, b0), Bool.and(U32.is_eq(a1, b1), Bool.and(U32.is_eq(a2, b2), Bool.and(U32.is_eq(a3, b3),
        Bool.and(String.eq(a4, b4), Bool.and(String.eq(a5, b5), same.hits(at, bt)))))))
    case _ _: False{}

def same.out(a: L.Out, b: L.Out) -> Bool:
  match a b:
    case L.Out{ao, ah} L.Out{bo, bh}: Bool.and(TI.Eq.ops(ao, bo), same.hits(ah, bh))

def ctx(+selected: String, now: Nat, epoch: Nat) -> SR.SideRowCtx:
  SR.SideRowCtx{[L.Ty.none()], [M.State.empty()], [U.Side.ix(M.State.empty())], selected, now, epoch}

def project(c: D.Clip, y: U32, sn: Bool) -> ST.SideRowReq:
  ST.SideProjectRow{M.Thread.blank("t"), sn, c, y}

def active(c: D.Clip, y: U32, faded: Bool) -> ST.SideRowReq:
  ST.SideActiveRow{U.ARow{0n, 0n, M.Thread.blank("t"), "p", "Project", faded}, c, y}

def test.row(+req: ST.SideRowReq, +context: SR.SideRowCtx) -> IO(Unit):
  do IO<Unit>:
    +cold : SR.SideRowRun = SR.Batch.memo([req], context, Map.new(&2, ST.SideRowM))
    +hot : SR.SideRowRun = SR.Batch.memo([req], context, SR.Batch.rows(cold))
    check(Nat.is_eq(SR.Batch.hits(cold), 0n), "cold row misses")
    check(Nat.is_eq(SR.Batch.count(cold), 1n), "visible row retained")
    check(Nat.is_eq(SR.Batch.hits(hot), 1n), "same row reuses")
    check(same.out(SR.Batch.out(cold), SR.Batch.fresh([req], context)), "cold equals original row")
    check(same.out(SR.Batch.out(hot), SR.Batch.fresh([req], context)), "hot equals original row")

def test.changed(+req: ST.SideRowReq, +context: SR.SideRowCtx, old: Map<&2, ST.SideRowM>) -> IO(Unit):
  do IO<Unit>:
    +got : SR.SideRowRun = SR.Batch.memo([req], context, old)
    check(Nat.is_eq(SR.Batch.hits(got), 0n), "changed row misses")
    check(same.out(SR.Batch.out(got), SR.Batch.fresh([req], context)), "changed row equals original")

def test.age(+now: Nat, +at: Nat, +expected: Nat) -> IO(Unit):
  do IO<Unit>:
    check(Nat.is_eq(SR.Clock.age(now, at), expected), "age deadline")
    check(String.eq(U.Time.ago(now, at), U.Time.ago(Nat.sub(expected, 1n), at)), "age stable until deadline")
    check(Bool.not(String.eq(U.Time.ago(now, at), U.Time.ago(expected, at))), "age changes at deadline")

def test.countdown(+remaining: Nat, +expected: Nat) -> IO(Unit):
  do IO<Unit>:
    check(Nat.is_eq(SR.Clock.countdown(0n, remaining), expected), "countdown deadline")
    check(String.eq(U.Time.ago(remaining, 0n), U.Time.ago(remaining, Nat.sub(expected, 1n))), "countdown stable until deadline")
    check(Bool.not(String.eq(U.Time.ago(remaining, 0n), U.Time.ago(remaining, expected))), "countdown changes at deadline")

def list.context() -> SR.SideRowCtx:
  +st = SD.st()
  SR.SideRowCtx{[L.Ty.none()], [st], [U.Side.ix(st)], "a", SD.now(), 0n}

def list.memo(active: Bool, +q: String, +context: SR.SideRowCtx, +old: Map<&2, ST.SideRowM>) -> L.SideCol:
  match context:
    case SR.SideRowCtx{ty, st, ix, sel, now, _}:
      L.Lay.side.list.k.m(~(List<&2, MM.SidePaint>), ~MM.SideM.row, active, SR.Ctx.ty(ty), q, Map.new(&2, Bool), SR.Ctx.index(ix), SR.Ctx.state(st), sel, now, "", "", D.Clip{0, 96, 264, 900}, 96,
        [MM.SidePaint{context, old}])

def list.fresh(active: Bool, q: String, context: SR.SideRowCtx) -> L.Out:
  match context:
    case SR.SideRowCtx{ty, st, ix, sel, now, _}:
      L.Lay.col.out(L.Lay.side.list.k(active, SR.Ctx.ty(ty), q, Map.new(&2, Bool), SR.Ctx.index(ix), SR.Ctx.state(st), sel, now, "", "", D.Clip{0, 96, 264, 900}, 96))

def test.list(+active: Bool) -> IO(Unit):
  do IO<Unit>:
    +context : SR.SideRowCtx = list.context()
    +cold : L.SideCol = list.memo(active, "", context, Map.new(&2, ST.SideRowM))
    +old : Map<&2, ST.SideRowM> = SR.Memo.map(L.SideCol.rows(cold), Map.new(&2, ST.SideRowM))
    +hot : L.SideCol = list.memo(active, "", context, old)
    check(Nat.is_gt(List.length(&2, ST.SideRowM, L.SideCol.rows(cold)), 0n), "list has visible rows")
    check(Nat.is_eq(L.SideCol.hits(cold), 0n), "list cold misses")
    check(Nat.is_eq(L.SideCol.hits(hot), List.length(&2, ST.SideRowM, L.SideCol.rows(hot))), "list hot rows hit")
    check(same.out(L.SideCol.out(cold), list.fresh(active, "", context)), "cold list exact ops/hits and headers")
    check(same.out(L.SideCol.out(hot), list.fresh(active, "", context)), "hot list exact ops/hits and headers")
    +empty : L.SideCol = list.memo(active, "unmatchablexyz", context, old)
    check(List.is_empty(&2, ST.SideRowM, L.SideCol.rows(empty)), "search removes cached rows")
    check(same.out(L.SideCol.out(empty), list.fresh(active, "unmatchablexyz", context)), "changed membership exact")

def main() -> IO(Unit):
  do IO<Unit>:
    test.age(0n, 0n, 60n)
    test.age(59n, 0n, 60n)
    test.age(60n, 0n, 120n)
    test.age(3599n, 0n, 3600n)
    test.age(3600n, 0n, 7200n)
    test.age(86399n, 0n, 86400n)
    test.age(86400n, 0n, 172800n)
    test.age(10n, 1000n, 1060n)
    test.countdown(60n, 1n)
    test.countdown(61n, 2n)
    test.countdown(3599n, 60n)
    test.countdown(3600n, 1n)
    test.countdown(3660n, 61n)
    test.countdown(86400n, 1n)
    test.countdown(86460n, 61n)
    check(Nat.is_eq(SR.Clock.countdown(100n, 159n), 0n), "countdown below minute stays now")
    check(Nat.is_eq(SR.Clock.countdown(100n, 0n), 0n), "past countdown stays now")
    check(Nat.is_eq(SR.Row.due.project(True{}, False{}, 100n, 0n, 0n), 0n), "badge suppresses age deadline")
    test.row(project(D.Clip.all(), 100, False{}), ctx("", 59n, 0n))
    test.row(project(D.Clip.all(), 100, True{}), ctx("t", 60n, 0n))
    test.row(active(D.Clip.all(), 100, False{}), ctx("t", 60n, 0n))
    test.row(active(D.Clip.all(), 100, True{}), ctx("", 60n, 0n))
    +old : Map<&2, ST.SideRowM> = SR.Batch.rows(SR.Batch.memo([project(D.Clip.all(), 100, False{})], ctx("", 59n, 0n), Map.new(&2, ST.SideRowM)))
    test.changed(project(D.Clip.all(), 132, False{}), ctx("", 59n, 0n), old)
    test.changed(project(D.Clip{0, 96, 264, 900}, 100, False{}), ctx("", 59n, 0n), old)
    test.changed(project(D.Clip.all(), 100, False{}), ctx("t", 59n, 0n), old)
    test.changed(project(D.Clip.all(), 100, False{}), ctx("", 60n, 0n), old)
    test.changed(project(D.Clip.all(), 100, False{}), ctx("", 59n, 1n), old)
    test.changed(active(D.Clip.all(), 100, False{}), ctx("", 59n, 0n), old)
    +off : SR.SideRowRun = SR.Batch.memo([project(D.Clip{0, 0, 264, 50}, 100, False{})], ctx("", 59n, 0n), old)
    check(Nat.is_eq(SR.Batch.count(off), 0n), "offscreen row not cached")
    check(List.is_empty(&2, String, Map.keys(&2, ST.SideRowM, SR.Batch.rows(off))), "old offscreen row evicted")
    check(same.out(SR.Batch.out(off), SR.Batch.fresh([project(D.Clip{0, 0, 264, 50}, 100, False{})], ctx("", 59n, 0n))), "offscreen hit preserved")
    check(List.is_empty(&2, String, Map.keys(&2, ST.SideRowM, SR.Batch.rows(SR.Batch.memo(Nil{}, ctx("", 59n, 0n), old)))), "empty viewport releases old rows")
    test.list(False{})
    test.list(True{})
    IO.print("ok side rows: exact cold/hot/changed output, viewport bounds, clock deadlines")
