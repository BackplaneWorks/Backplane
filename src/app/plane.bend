import Base
import ../core/client.bend as U
import ../core/plane.bend as P
import ../gfx/raster.bend as R
import ../gfx/draw.bend as D
import ./theme.bend as T
import ./layout.bend as L

# The uberbot's Status tab in the window (core/plane.bend, docs/uber.md):
# a strip of counters and machines, the threads that need the person, run
# and idle in the main column, the bots, rooms and routines in a side
# column (under the main one on a narrow window). Every row is Pl.rh()
# high and only the rows that reach the visible part are drawn; a click
# sends the row's own action. Positions are `rel`: from the top of the
# content, which sits at `top`, scrolled up by `off`.

def Pl.rh() -> U32:
  30

# a row or label of height h at rel reaches the part on show
def Pl.vis(+rel: U32, +h: U32, +off: U32, +vh: U32) -> Bool:
  Bool.and(U32.is_gt((rel + h : U32), off), U32.is_lt(rel, (off + vh : U32)))

def Pl.dot(+s: String) -> U32:
  Bool.pick(U32, Bool.or(Bool.or(String.eq(s, "approval"), String.eq(s, "question")), Bool.or(String.eq(s, "wait"), String.eq(s, "unread"))), T.Th.warn(),
    Bool.pick(U32, Bool.or(String.eq(s, "failed"), String.eq(s, "error")), T.Th.bad(),
      Bool.pick(U32, Bool.or(Bool.or(String.eq(s, "working"), String.eq(s, "work")), String.eq(s, "talk")), T.Th.ok(),
        Bool.pick(U32, Bool.or(String.eq(s, "queued"), String.eq(s, "think")), T.Th.dim(),
          Bool.pick(U32, String.eq(s, "finished"), T.Th.soft(), T.Th.faint())))))

# a thread's row: dot, title, its project, then its machine and age at the right
def Pl.row.thread(+ty: L.Ty, +c: D.Clip, +title: String, +sub: String, +machine: String, +state: String, +at: Nat, +now: Nat, +x: U32, +w: U32, +y: U32) -> List<&2, R.Op>:
  +age = {U.Time.ago(now, at) : String}
  +aw = L.Ty.width(ty, 2n, age)
  +mw = Bool.pick(U32, String.is_empty(machine), 0, (L.Ty.width(ty, 2n, machine) + 14 : U32))
  +avail = L.U32.dim(w, (aw + mw + 40 : U32))
  +tt = {L.Ty.fit(ty, 0n, title, ((avail * 3 : U32) / 5 : U32)) : String}
  +tw = L.Ty.width(ty, 0n, tt)
  List.concat(&2, R.Op, [[D.Draw.disc(c, U32.to_f32((x + 8 : U32)), U32.to_f32((y + 15 : U32)), 4.0, Pl.dot(state), 255)],
    L.Ty.text(ty, 0n, tt, c, (x + 22 : U32), y, 30, T.Th.text()),
    L.Ty.text(ty, 2n, L.Ty.fit(ty, 2n, sub, L.U32.dim(avail, (tw + 12 : U32))), c, (x + 34 + tw : U32), y, 30, T.Th.faint()),
    L.Ty.text(ty, 2n, machine, c, (x + w - 8 - aw - mw : U32), y, 30, T.Th.faint()),
    L.Ty.text(ty, 2n, age, c, (x + w - 8 - aw : U32), y, 30, T.Th.dim())])

# a side row: dot, title, and what it says at the right
def Pl.row.side(+ty: L.Ty, +c: D.Clip, +title: String, +sub: String, +machine: String, +state: String, +x: U32, +w: U32, +y: U32) -> List<&2, R.Op>:
  +sw = L.Ty.width(ty, 2n, sub)
  +sc = {L.U32.dim(w, 40) : U32}
  +sp = {U32.min(sw, (sc / 2 : U32)) : U32}
  +tt = {L.Ty.fit(ty, 0n, title ++ Bool.pick(String, String.is_empty(machine), "", " · " ++ machine), L.U32.dim(sc, (sp + 12 : U32))) : String}
  List.concat(&2, R.Op, [[D.Draw.disc(c, U32.to_f32((x + 8 : U32)), U32.to_f32((y + 15 : U32)), 4.0, Pl.dot(state), 255)],
    L.Ty.text(ty, 0n, tt, c, (x + 22 : U32), y, 30, T.Th.text()),
    L.Ty.text(ty, 2n, L.Ty.fit(ty, 2n, sub, sp), c, (x + w - 8 - L.Ty.width(ty, 2n, L.Ty.fit(ty, 2n, sub, sp)) : U32), y, 30, T.Th.dim())])

def Pl.row.ops(side: Bool, +title: String, +sub: String, +machine: String, +state: String, +at: Nat, +ty: L.Ty, +c: D.Clip, +x: U32, +w: U32, +y: U32, +now: Nat) -> List<&2, R.Op>:
  match side:
    case True{}:
      Pl.row.side(ty, c, title, sub, machine, state, x, w, y)
    case False{}:
      Pl.row.thread(ty, c, title, sub, machine, state, at, now, x, w, y)

def Pl.row(side: Bool, r: P.PRow, +ty: L.Ty, +c: D.Clip, +x: U32, +w: U32, +y: U32, +now: Nat) -> L.Out:
  match r:
    case P.PRow{_, _, +act, +val, +title, +sub, +machine, +state, +at, _}:
      L.Out{List.append(&2, R.Op, Pl.row.ops(side, title, sub, machine, state, at, ty, c, x, w, y, now), [D.Draw.rect(c, x, (y + 29 : U32), w, 1, T.Th.line(), 255)]),
        [L.Lay.hit(x, y, w, 30, act, val)]}

def Pl.row.if(on: Bool, side: Bool, r: P.PRow, +ty: L.Ty, +c: D.Clip, +x: U32, +w: U32, +y: U32, +now: Nat) -> L.Out:
  match on:
    case True{}:
      Pl.row(side, r, ty, c, x, w, y, now)
    case False{}:
      L.Out.none()

def Pl.rows(rs: List<&2, P.PRow>, +side: Bool, +ty: L.Ty, +c: D.Clip, +x: U32, +w: U32, +rel: U32, +top: U32, +off: U32, +vh: U32, +now: Nat) -> L.Col:
  match rs:
    case Nil{}:
      L.Col{L.Out.none(), rel}
    case r <> t:
      L.Lay.col.cat(Pl.row.if(Pl.vis(rel, 30, off, vh), side, r, ty, c, x, w, (top + rel - off : U32), now),
        Pl.rows(t, side, ty, c, x, w, (rel + 30 : U32), top, off, vh, now))

def Pl.label.if(on: Bool, +ty: L.Ty, +c: D.Clip, +s: String, +x: U32, +y: U32) -> L.Out:
  match on:
    case True{}:
      L.Out.ops(L.Ty.text(ty, 2n, s, c, x, y, 28, T.Th.faint()))
    case False{}:
      L.Out.none()

def Pl.sect.rows(rs: List<&2, P.PRow>, +label: String, +more: Nat, +side: Bool, +ty: L.Ty, +c: D.Clip, +x: U32, +w: U32, +rel: U32, +top: U32, +off: U32, +vh: U32, +now: Nat) -> L.Col:
  L.Lay.col.cat(Pl.label.if(Pl.vis(rel, 28, off, vh), ty, c, label, x, (top + rel - off : U32)),
    L.Lay.col.then(Pl.rows(rs, side, ty, c, x, w, (rel + 28 : U32), top, off, vh, now), +y2 =>
      L.Col{Pl.label.if(Bool.and(Nat.is_gt(more, 0n), Pl.vis(y2, 28, off, vh)), ty, c, "+" ++ Nat.show(more) ++ " more", x, (top + y2 - off : U32)),
        (y2 + Bool.pick(U32, Nat.is_gt(more, 0n), 28, 0) + 12 : U32)}))

# a section: nothing at all when it has no rows
def Pl.sect.k(none: Bool, rs: List<&2, P.PRow>, +label: String, +more: Nat, +side: Bool, +ty: L.Ty, +c: D.Clip, +x: U32, +w: U32, +rel: U32, +top: U32, +off: U32, +vh: U32, +now: Nat) -> L.Col:
  match none:
    case True{}:
      L.Col{L.Out.none(), rel}
    case False{}:
      Pl.sect.rows(rs, label, more, side, ty, c, x, w, rel, top, off, vh, now)

def Pl.sect(+rs: List<&2, P.PRow>, +label: String, +more: Nat, +side: Bool, +ty: L.Ty, +c: D.Clip, +x: U32, +w: U32, +rel: U32, +top: U32, +off: U32, +vh: U32, +now: Nat) -> L.Col:
  Pl.sect.k(List.is_empty(&2, P.PRow, rs), rs, label, more, side, ty, c, x, w, rel, top, off, vh, now)

def Pl.len(rs: List<&2, P.PRow>) -> Nat:
  List.length(&2, P.PRow, rs)

# Strip
# -----

def Pl.cell(+ty: L.Ty, +c: D.Clip, +n: Nat, +label: String, +hot: Bool, +x: U32, +y: U32) -> L.Out:
  L.Out.ops(List.append(&2, R.Op, L.Ty.text(ty, 1n, Nat.show(n), c, x, y, 28, Bool.pick(U32, Bool.and(hot, Nat.is_gt(n, 0n)), T.Th.warn(), Bool.pick(U32, Nat.is_gt(n, 0n), T.Th.text(), T.Th.faint()))),
    L.Ty.text(ty, 2n, label, c, x, (y + 26 : U32), 20, T.Th.dim())))

def Pl.machine.dot(+s: String) -> U32:
  Bool.pick(U32, String.eq(s, "here"), T.Th.ok(), Bool.pick(U32, String.eq(s, "syncing"), T.Th.accent(), T.Th.faint()))

def Pl.machines(ms: List<&2, P.PMach>, +ty: L.Ty, +c: D.Clip, +x: U32, +y: U32) -> List<&2, R.Op>:
  match ms:
    case Nil{}:
      Nil{}
    case P.PMach{+n, +s} <> t:
      List.concat(&2, R.Op, [[D.Draw.disc(c, U32.to_f32((x + 4 : U32)), U32.to_f32((y + 14 : U32)), 4.0, Pl.machine.dot(s), 255)],
        L.Ty.text(ty, 2n, n, c, (x + 14 : U32), y, 28, Bool.pick(U32, String.eq(s, "here"), T.Th.soft(), T.Th.faint())),
        Pl.machines(t, ty, c, (x + 14 + L.Ty.width(ty, 2n, n) + 20 : U32), y)])

# where the machines go, and under them the home line (core's Plane.home)
def Pl.strip.mh(+p: P.Plane) -> U32:
  Bool.pick(U32, List.is_empty(&2, P.PMach, P.Plane.machines(p)), 56, 92)

def Pl.strip.h(+p: P.Plane, +h: P.PHome) -> U32:
  (Pl.strip.mh(p) + Bool.pick(U32, P.PHome.on(h), 36, 0) : U32)

# "Home: desk" and, on a machine that is not the home, the button that makes it
def Pl.home.btn(offer: Bool, +ty: L.Ty, +c: D.Clip, +x: U32, +y: U32) -> L.Out:
  match offer:
    case True{}:
      L.Lay.button(ty, c, x, y, P.Plane.home.label(), T.Th.text(), "uber-home", "")
    case False{}:
      L.Out.none()

def Pl.home(+h: P.PHome, +ty: L.Ty, +c: D.Clip, +x: U32, +y: U32) -> L.Out:
  +line = {P.Plane.home.line(h) : String}
  L.Out.cat(L.Out.ops(L.Ty.text(ty, 2n, line, c, x, y, 28, T.Th.soft())),
    Pl.home.btn(P.Plane.home.button(h), ty, c, (x + L.Ty.width(ty, 2n, line) + 20 : U32), y))

def Pl.strip.if(on: Bool, +p: P.Plane, +h: P.PHome, +ty: L.Ty, +c: D.Clip, +x: U32, +y: U32) -> L.Out:
  match on:
    case True{}:
      L.Out.all([
        Pl.cell(ty, c, Pl.len(P.Plane.need(p)), "Need you", True{}, x, y),
        Pl.cell(ty, c, Pl.len(P.Plane.run(p)), "Running", False{}, (x + 130 : U32), y),
        Pl.cell(ty, c, P.Plane.working(p), "Bots working", False{}, (x + 260 : U32), y),
        Pl.cell(ty, c, Pl.len(P.Plane.rooms(p)), "Unread rooms", False{}, (x + 390 : U32), y),
        L.Out.ops(Pl.machines(P.Plane.machines(p), ty, c, x, (y + 56 : U32))),
        Bool.pick(L.Out, P.PHome.on(h), Pl.home(h, ty, c, x, (y + Pl.strip.mh(p) : U32)), L.Out.none())])
    case False{}:
      L.Out.none()

def Pl.strip(+p: P.Plane, +h: P.PHome, +ty: L.Ty, +c: D.Clip, +x: U32, +rel: U32, +top: U32, +off: U32, +vh: U32) -> L.Out:
  +y = {(top + rel - off : U32) : U32}
  Pl.strip.if(Pl.vis(rel, Pl.strip.h(p, h), off, vh), p, h, ty, c, x, y)

# Columns
# -------

def Pl.none.if(on: Bool, +ty: L.Ty, +c: D.Clip, +x: U32, +y: U32) -> L.Out:
  match on:
    case True{}:
      L.Out.ops(L.Ty.text(ty, 0n, "Nothing active", c, x, y, 28, T.Th.faint()))
    case False{}:
      L.Out.none()

def Pl.main(+p: P.Plane, +ty: L.Ty, +c: D.Clip, +x: U32, +w: U32, +rel: U32, +top: U32, +off: U32, +vh: U32, +now: Nat) -> L.Col:
  +quiet = Bool.and(Bool.and(List.is_empty(&2, P.PRow, P.Plane.need(p)), List.is_empty(&2, P.PRow, P.Plane.run(p))), List.is_empty(&2, P.PRow, P.Plane.idle(p)))
  L.Lay.col.cat(Pl.none.if(quiet, ty, c, x, (top + rel - off : U32)),
    L.Lay.col.then(L.Col{L.Out.none(), Bool.pick(U32, quiet, (rel + 40 : U32), rel)}, r1 =>
      L.Lay.col.then(Pl.sect(P.Plane.need(p), "NEEDS YOU", 0n, False{}, ty, c, x, w, r1, top, off, vh, now), r2 =>
        L.Lay.col.then(Pl.sect(P.Plane.run(p), "RUNNING", 0n, False{}, ty, c, x, w, r2, top, off, vh, now), r3 =>
          Pl.sect(P.Plane.idle(p), "IDLE " ++ Nat.show(P.Plane.idle_n(p)), Nat.sub(P.Plane.idle_n(p), Pl.len(P.Plane.idle(p))), False{}, ty, c, x, w, r3, top, off, vh, now)))))

def Pl.side(+p: P.Plane, +ty: L.Ty, +c: D.Clip, +x: U32, +w: U32, +rel: U32, +top: U32, +off: U32, +vh: U32, +now: Nat) -> L.Col:
  L.Lay.col.then(Pl.sect(P.Plane.bots(p), "BOTS", 0n, True{}, ty, c, x, w, rel, top, off, vh, now), r2 =>
    L.Lay.col.then(Pl.sect(P.Plane.rooms(p), "ROOMS", 0n, True{}, ty, c, x, w, r2, top, off, vh, now), r3 =>
      L.Lay.col.then(Pl.sect(P.Plane.board(p), "Board · " ++ Nat.show(P.Plane.board_n(p)) ++ " open", Nat.sub(P.Plane.board_n(p), Pl.len(P.Plane.board(p))), True{}, ty, c, x, w, r3, top, off, vh, now), r4 =>
        Pl.sect(P.Plane.routines(p), "ROUTINES", 0n, True{}, ty, c, x, w, r4, top, off, vh, now))))

def Pl.wide() -> U32:
  760

def Pl.side.w() -> U32:
  300

# both columns: the side one right of the main one when wide, else under it
def Pl.cols(+wide: Bool, +p: P.Plane, +ty: L.Ty, +c: D.Clip, +x: U32, +w: U32, +rel: U32, +top: U32, +off: U32, +vh: U32, +now: Nat) -> L.Col:
  +mw = {Bool.pick(U32, wide, (w - Pl.side.w() - 24 : U32), w) : U32}
  L.Lay.col.then(Pl.main(p, ty, c, x, mw, rel, top, off, vh, now), +y2 =>
    L.Lay.col.then(Pl.side(p, ty, c, Bool.pick(U32, wide, (x + mw + 24 : U32), x), Bool.pick(U32, wide, Pl.side.w(), w), Bool.pick(U32, wide, rel, (y2 + 4 : U32)), top, off, vh, now), y3 =>
      L.Col{L.Out.none(), U32.max(y3, Bool.pick(U32, wide, y2, 0))}))

# The status plane from top (scrolled up by off) in x..x+w; the column's y
# is where it ends
def Pl.draw(+p: P.Plane, +hm: P.PHome, +ty: L.Ty, +c: D.Clip, +x: U32, +w: U32, +top: U32, +off: U32, +vh: U32, +now: Nat) -> L.Col:
  +sh = Pl.strip.h(p, hm)
  L.Lay.col.cat(Pl.strip(p, hm, ty, c, x, 0, top, off, vh),
    L.Lay.col.then(Pl.cols(U32.is_ge(w, Pl.wide()), p, ty, c, x, w, (sh + 8 : U32), top, off, vh, now), y =>
      L.Col{L.Out.none(), (top + y - off : U32)}))
