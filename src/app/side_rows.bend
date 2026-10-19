import Base
import ./side_row_types.bend as ST
import ./layout.bend as L
import ./theme.bend as T
import ../core/client.bend as U
import ../core/model.bend as M
import ../core/subs.bend as SB
import ../core/badge.bend as BG
import ../core/title.bend as TTL
import ../core/i18n.bend as I
import ../core/eq.bend as Q
import ../gfx/draw.bend as D

# A row's resolved presentation, independent of membership and ordering.
# The caller owns the typography epoch: discard the map or advance it
# whenever fonts/glyphs change. Locale also belongs to the exact key.
# Box the deep typography/state across native continuations.
type SideRowCtx is Data:
  SideRowCtx{ty: List<&2, L.Ty>, state: List<&2, M.State>, index: List<&2, U.SIx>, selected: String, now: Nat, epoch: Nat}

def Ctx.ty(xs: List<&2, L.Ty>) -> L.Ty:
  match xs:
    case t <> _: t
    case Nil{}: L.Ty.none()

def Ctx.state(xs: List<&2, M.State>) -> M.State:
  match xs:
    case s <> _: s
    case Nil{}: M.State.empty()

def Ctx.index(xs: List<&2, U.SIx>) -> U.SIx:
  match xs:
    case ix <> _: ix
    case Nil{}: U.Side.ix(M.State.empty())

def Clock.unit(+seconds: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_lt(seconds, 3600n), 60n, Bool.pick(Nat, Nat.is_lt(seconds, 86400n), 3600n, 86400n))

# Absolute seconds at the next change of Time.ago's displayed label.
def Clock.age(+now: Nat, +at: Nat) -> Nat:
  +seconds = Nat.sub(now, at)
  Nat.add(at, Nat.mul(Nat.add(Nat.div(seconds, Clock.unit(seconds)), 1n), Clock.unit(seconds)))

def Clock.countdown.k(small: Bool, +now: Nat, +until: Nat, +unit: Nat) -> Nat:
  match small:
    case True{}: 0n
    case False{}:
      Nat.add(Nat.sub(until, Nat.mul(Nat.div(Nat.sub(until, now), unit), unit)), 1n)

def Clock.countdown(+now: Nat, +until: Nat) -> Nat:
  Clock.countdown.k(Nat.is_lt(Nat.sub(until, now), 60n), now, until, Clock.unit(Nat.sub(until, now)))

def Key.project(+ty: L.Ty, +t: M.Thread, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat, +snoozed: Bool, c: D.Clip, y: U32, epoch: Nat) -> ST.SideRowK:
  match t:
    case M.Thread{+id, +p, title, _, +at, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      +badge = SB.Subs.badge(SB.Subs.count.ix(ix, st, id))
      ST.SideRowK{False{}, String.eq(id, sel), id, U.Label.title(st, id, TTL.Title.shown(L.Ty.loc(ty), title)),
        L.Lay.row.note(badge, Bool.pick(String, snoozed, U.Time.ago(U.Side.until(st, id), now), U.Time.ago(now, at))), "", False{}, L.Lay.row.nc(badge, T.Th.faint()),
        L.Lay.phase(U.Side.phase.ix(ix, st, t)), BG.Badge.rgb_of(st, p), c, y, I.I18n.tag(L.Ty.loc(ty)), epoch}

def Key.active(+ty: L.Ty, row: U.ARow, +ix: U.SIx, +st: M.State, +sel: String, c: D.Clip, y: U32, epoch: Nat) -> ST.SideRowK:
  match row:
    case U.ARow{_, _, +t, pid, pt, faded}:
      ST.SideRowK{True{}, String.eq(M.Thread.id(t), sel), M.Thread.id(t), U.Label.title(st, M.Thread.id(t), TTL.Title.shown(L.Ty.loc(ty), U.Active.title(t))),
        pt, SB.Subs.badge(SB.Subs.count.ix(ix, st, M.Thread.id(t))), faded, 0,
        L.Lay.phase(U.Side.phase.ix(ix, st, t)), BG.Badge.rgb_of(st, pid), c, y, I.I18n.tag(L.Ty.loc(ty)), epoch}

def Key.of(req: ST.SideRowReq, ctx: SideRowCtx) -> ST.SideRowK:
  match req ctx:
    case ST.SideProjectRow{t, sn, c, y} SideRowCtx{ty, st, ix, sel, now, epoch}:
      Key.project(Ctx.ty(ty), t, Ctx.index(ix), Ctx.state(st), sel, now, sn, c, y, epoch)
    case ST.SideActiveRow{r, c, y} SideRowCtx{ty, st, ix, sel, _, epoch}:
      Key.active(Ctx.ty(ty), r, Ctx.index(ix), Ctx.state(st), sel, c, y, epoch)

def Key.paint(ty: L.Ty, key: ST.SideRowK) -> L.Out:
  match key:
    case ST.SideRowK{False{}, on, id, title, note, _, _, ac, dot, tint, c, y, _, _}:
      L.Lay.row.k(ty, on, id, title, note, ac, dot, tint, c, y)
    case ST.SideRowK{True{}, on, id, title, note, badge, faded, _, dot, tint, c, y, _, _}:
      L.Lay.arow.k(ty, on, id, title, note, badge, faded, dot, tint, c, y)

def Key.id(k: ST.SideRowK) -> String:
  match k:
    case ST.SideRowK{_, _, id, _, _, _, _, _, _, _, _, _, _, _}: id

def Key.clip(a: D.Clip, b: D.Clip) -> Bool:
  match a b:
    case D.Clip{a0, a1, a2, a3} D.Clip{b0, b1, b2, b3}:
      Bool.and(U32.is_eq(a0, b0), Bool.and(U32.is_eq(a1, b1), Bool.and(U32.is_eq(a2, b2), U32.is_eq(a3, b3))))

def Key.same.words(a: ST.SideRowK, b: ST.SideRowK) -> Bool:
  match a b:
    case ST.SideRowK{a0, a1, _, _, _, _, a6, a7, a8, a9, a10, a11, _, a13} ST.SideRowK{b0, b1, _, _, _, _, b6, b7, b8, b9, b10, b11, _, b13}:
      Bool.and(Q.Eq.bool(a0, b0), Bool.and(Q.Eq.bool(a1, b1), Bool.and(Q.Eq.bool(a6, b6),
        Bool.and(U32.is_eq(a7, b7), Bool.and(U32.is_eq(a8, b8), Bool.and(U32.is_eq(a9, b9),
          Bool.and(Key.clip(a10, b10), Bool.and(U32.is_eq(a11, b11), Nat.is_eq(a13, b13)))))))))

def Key.same.text(ok: Bool, a: ST.SideRowK, b: ST.SideRowK) -> Bool:
  match ok a b:
    case False{} _ _: False{}
    case True{} ST.SideRowK{_, _, a2, a3, a4, a5, _, _, _, _, _, _, a12, _} ST.SideRowK{_, _, b2, b3, b4, b5, _, _, _, _, _, _, b12, _}:
      Bool.and(Q.Eq.str(a2, b2), Bool.and(Q.Eq.str(a3, b3), Bool.and(Q.Eq.str(a4, b4), Bool.and(Q.Eq.str(a5, b5), Q.Eq.str(a12, b12)))))

def Key.same(+a: ST.SideRowK, +b: ST.SideRowK) -> Bool:
  Key.same.text(Key.same.words(a, b), a, b)

def Key.none() -> ST.SideRowK:
  ST.SideRowK{False{}, False{}, "", "", "", "", False{}, 0, 0, 0, D.Clip.all(), 0, "", 0n}

def Memo.none() -> ST.SideRowM:
  ST.SideRowM{Key.none(), Nil{}, Nil{}, 0n}

def Memo.out(m: ST.SideRowM) -> L.Out:
  match m:
    case ST.SideRowM{_, ops, hits, _}: L.Out{ops, hits}

def Memo.key(m: ST.SideRowM) -> ST.SideRowK:
  match m:
    case ST.SideRowM{k, _, _, _}: k

def Memo.make(key: ST.SideRowK, out: L.Out, due: Nat) -> ST.SideRowM:
  match out:
    case L.Out{ops, hits}: ST.SideRowM{key, ops, hits, due}

def Step.make(out: L.Out, ms: List<&2, ST.SideRowM>, hit: Nat) -> ST.SideRowStep:
  match out:
    case L.Out{ops, hits}: ST.SideRowStep{ops, hits, ms, hit}

def Memo.lookup.k(r: Map<&2, ST.SideRowM> & ST.SideRowM) -> ST.SideRowM:
  (_, m) = r
  m

def Memo.lookup(ms: Map<&2, ST.SideRowM>, id: String) -> ST.SideRowM:
  Memo.lookup.k(Map.get(ST.SideRowM, Memo.none(), ms, id))

# Rebuild from this layout's visible rows; no hidden or retired row stays.
def Memo.map(ms: List<&2, ST.SideRowM>, rows: Map<&2, ST.SideRowM>) -> Map<&2, ST.SideRowM>:
  match ms:
    case Nil{}: rows
    case +m <> rest: Memo.map(rest, Map.set(&2, ST.SideRowM, rows, Key.id(Memo.key(m)), m))

def Row.due.project(badge: Bool, snoozed: Bool, +now: Nat, at: Nat, until: Nat) -> Nat:
  match badge snoozed:
    case True{} _: 0n
    case False{} True{}: Clock.countdown(now, until)
    case False{} False{}: Clock.age(now, at)

def Row.due(req: ST.SideRowReq, ctx: SideRowCtx) -> Nat:
  match req ctx:
    case ST.SideProjectRow{+t, sn, _, _} SideRowCtx{_, +st, ix, _, +now, _}:
      Row.due.project(Nat.is_gt(SB.Subs.count.ix(Ctx.index(ix), Ctx.state(st), M.Thread.id(t)), 0n), sn, now, U.Active.activity(t), U.Side.until(Ctx.state(st), M.Thread.id(t)))
    case ST.SideActiveRow{_, _, _} _: 0n

# A non-visible row retains its original hit and never enters the map.
def Row.fresh(req: ST.SideRowReq, ctx: SideRowCtx) -> L.Out:
  match req ctx:
    case ST.SideProjectRow{t, sn, c, y} SideRowCtx{ty, st, ix, sel, now, _}:
      L.Lay.row(Ctx.ty(ty), t, Ctx.index(ix), Ctx.state(st), sel, now, sn, c, y)
    case ST.SideActiveRow{r, +c, +y} SideRowCtx{ty, st, ix, sel, _, _}:
      L.Lay.arow.k2(L.Lay.row.seen(c, y), Ctx.ty(ty), r, Ctx.index(ix), Ctx.state(st), sel, c, y)

def Row.seen(req: ST.SideRowReq) -> Bool:
  match req:
    case ST.SideProjectRow{_, _, c, y}: L.Lay.row.seen(c, y)
    case ST.SideActiveRow{_, c, y}: L.Lay.row.seen(c, y)

def Row.pick(hit: Bool, ctx: SideRowCtx, +key: ST.SideRowK, +old: ST.SideRowM, due: Nat) -> ST.SideRowStep:
  match hit ctx:
    case True{} _:
      Step.make(Memo.out(old), [Memo.make(key, Memo.out(old), due)], 1n)
    case False{} SideRowCtx{ty, _, _, _, _, _}:
      +out = Key.paint(Ctx.ty(ty), key)
      Step.make(out, [Memo.make(key, out, due)], 0n)

def Row.resolve(+req: ST.SideRowReq, +ctx: SideRowCtx, +key: ST.SideRowK, +old: ST.SideRowM) -> ST.SideRowStep:
  Row.pick(Key.same(key, Memo.key(old)), ctx, key, old, Row.due(req, ctx))

def Row.visible(seen: Bool, +req: ST.SideRowReq, +ctx: SideRowCtx, old: Map<&2, ST.SideRowM>) -> ST.SideRowStep:
  match seen:
    case False{}: Step.make(Row.fresh(req, ctx), Nil{}, 0n)
    case True{}:
      +key = Key.of(req, ctx)
      Row.resolve(req, ctx, key, Memo.lookup(old, Key.id(key)))

def Row.memo(+req: ST.SideRowReq, ctx: SideRowCtx, old: Map<&2, ST.SideRowM>) -> ST.SideRowStep:
  Row.visible(Row.seen(req), req, ctx, old)

type SideRowRun is Data:
  SideRowRun{outs: List<&2, L.Out>, rows: Map<&2, ST.SideRowM>, hits: Nat, count: Nat}

def Run.put(ms: List<&2, ST.SideRowM>, rows: Map<&2, ST.SideRowM>) -> Map<&2, ST.SideRowM>:
  match ms:
    case Nil{}: rows
    case +m <> _: Map.set(&2, ST.SideRowM, rows, Key.id(Memo.key(m)), m)

def Run.add(step: ST.SideRowStep, run: SideRowRun) -> SideRowRun:
  match step run:
    case ST.SideRowStep{ops, hs, +ms, hit} SideRowRun{os, rows, hits, count}:
      SideRowRun{L.Out{ops, hs} <> os, Run.put(ms, rows), Nat.add(hits, hit), Nat.add(count, List.length(&2, ST.SideRowM, ms))}

def Batch.go(reqs: List<&2, ST.SideRowReq>, +ctx: SideRowCtx, +old: Map<&2, ST.SideRowM>, run: SideRowRun) -> SideRowRun:
  match reqs:
    case Nil{}: run
    case req <> rest: Batch.go(rest, ctx, old, Run.add(Row.memo(req, ctx, old), run))

def Batch.memo(reqs: List<&2, ST.SideRowReq>, ctx: SideRowCtx, old: Map<&2, ST.SideRowM>) -> SideRowRun:
  Batch.go(reqs, ctx, old, SideRowRun{Nil{}, Map.new(&2, ST.SideRowM), 0n, 0n})

def Batch.out(run: SideRowRun) -> L.Out:
  match run:
    case SideRowRun{os, _, _, _}: L.Out.all(List.reverse(&2, L.Out, os))

def Batch.rows(run: SideRowRun) -> Map<&2, ST.SideRowM>:
  match run:
    case SideRowRun{_, rows, _, _}: rows

def Batch.hits(run: SideRowRun) -> Nat:
  match run:
    case SideRowRun{_, _, hits, _}: hits

def Batch.count(run: SideRowRun) -> Nat:
  match run:
    case SideRowRun{_, _, _, count}: count

def Batch.fresh(reqs: List<&2, ST.SideRowReq>, +ctx: SideRowCtx) -> L.Out:
  match reqs:
    case Nil{}: L.Out.none()
    case req <> rest: L.Out.cat(Row.fresh(req, ctx), Batch.fresh(rest, ctx))
