import Base
import ./side_row_types.bend as ST
import ../core/title.bend as TTL
import ../core/i18n.bend as I
import ../core/cef.bend as CF
import ../core/msgs.bend as MSG
import ../core/winnote.bend as NT
import ../core/codec.bend as CC
import ../core/kihealth.bend as KH
import ../core/model.bend as M
import ../core/json.bend as J
import ../core/client.bend as U
import ../core/usage.bend as UZ
import ../core/fm.bend as FM
import ../core/todo.bend as TD
import ../core/edit.bend as ED
import ../core/images.bend as IM
import ../core/ime.bend as IME
import ../core/links.bend as LK
import ../gfx/raster.bend as R
import ../gfx/draw.bend as D
import ../gfx/text.bend as X
import ../gfx/icon.bend as IC
import ../web/dom.bend as N
import ./theme.bend as T
import ./nui.bend as S
import ./viewer.bend as V
import ../core/board.bend as B
import ../core/schematic.bend as SC
import ../core/hier.bend as HI
import ../core/glb.bend as G
import ../gfx/face.bend as FA
import ../gfx/bitmap.bend as BM
import ../core/diff.bend as DF
import ../core/vt.bend as VT
import ../core/attach.bend as AT
import ../core/eq.bend as Q
import ../core/fold.bend as FO
import ../core/subs.bend as SB
import ../core/menu.bend as MN
import ../core/badge.bend as BG
import ../core/voice.bend as VC
import ../core/mech.bend as MC
import ../core/hist.bend as HS
import ../core/review.bend as RVW
import ../core/look.bend as LO
import ../core/layers.bend as LY
import ../core/tools.bend as TO
import ../core/emb.bend as EM
import ../core/onboard.bend as OB
import ../core/sched.bend as SCH
import ../core/tz.bend as TZ
import ../core/stamp.bend as STP
import ../core/order.bend as OR

# Nui -> draw ops and hit regions. Ops are gathered bottom-up (paint
# order) and reversed once into the topmost-first order the rasterizer
# wants; hits likewise. Text is measured and drawn in the system's fonts
# (Ty), the same faces and sizes as the Backplane web app.

type Out is Data:
  Out{ops: List<&2, R.Op>, hits: List<&2, S.Hit>}

def Out.none() -> Out:
  Out{Nil{}, Nil{}}

# a then b on top
def Out.cat(a: Out, b: Out) -> Out:
  match a b:
    case Out{o1, h1} Out{o2, h2}:
      Out{List.append(&2, R.Op, o1, o2), List.append(&2, S.Hit, h1, h2)}

def Out.ops(os: List<&2, R.Op>) -> Out:
  Out{os, Nil{}}

def Out.all(xs: List<&2, Out>) -> Out:
  match xs:
    case Nil{}:
      Out.none()
    case x <> t:
      Out.cat(x, Out.all(t))

def Out.hits_below.k(keep: Bool, h: S.Hit, rest: List<&2, S.Hit>) -> List<&2, S.Hit>:
  match keep:
    case True{}:
      h <> rest
    case False{}:
      rest

def Out.hits_below.go(hs: List<&2, S.Hit>, +top: U32) -> List<&2, S.Hit>:
  match hs:
    case Nil{}:
      Nil{}
    case S.Hit{+x0, +y0, +x1, +y1, +a, +v} <> t:
      Out.hits_below.k(U32.is_ge(y0, top), S.Hit{x0, y0, x1, y1, a, v}, Out.hits_below.go(t, top))

# o with only the hits that start at or below top (what scrolled under a
# header cannot be clicked through it)
def Out.hits_below(+top: U32, o: Out) -> Out:
  match o:
    case Out{os, hs}:
      Out{os, Out.hits_below.go(hs, top)}

def Out.when(on: Bool, o: Out) -> Out:
  match on:
    case True{}:
      o
    case False{}:
      Out.none()

# The light theme: every op of the UI in the palette's light twin
# (theme.bend's Th.light); the dark theme draws them as they are
# (a colour's top byte, a caret's or a cat anchor's tag, is kept)
def Theme.op(o: R.Op) -> R.Op:
  match o:
    case R.Op{x0, y0, x1, y1, +c, a, sh}:
      R.Op{x0, y0, x1, y1, U32.or(U32.and(c, 4278190080), T.Th.light(U32.and(c, 16777215))), a, sh}

# (onto a reversed accumulator: a frame holds many ops)
def Theme.onto(os: List<&2, R.Op>, acc: List<&2, R.Op>) -> List<&2, R.Op>:
  match os:
    case Nil{}:
      acc
    case o <> t:
      Theme.onto(t, Theme.op(o) <> acc)

def Theme.ops(light: Bool, os: List<&2, R.Op>) -> List<&2, R.Op>:
  match light:
    case True{}:
      List.reverse(&2, R.Op, Theme.onto(os, Nil{}))
    case False{}:
      os

# the window's plain ground in its theme
def Theme.ground(+flip: U32) -> U32:
  T.Th.of(V.Look.ui(flip), T.Th.bg())

# Typography
# ----------

# the glyph cache and the faces: interface, bold, meta, chat body, code,
# and the device language the frame's text is built in (set once a frame)
type Ty is Data:
  Ty{cache: Map<&2, X.TG>, ui: X.Face, bold: X.Face, meta: X.Face, body: X.Face, code: X.Face, loc: I.Loc}

# a type with only the bitmap font (never used: Aux keeps the real one)
def Ty.none() -> Ty:
  Ty{Map.new(&2, X.TG), X.Text.bitmap(), X.Text.bitmap(), X.Text.bitmap(), X.Text.bitmap(), X.Text.bitmap(), I.En{}}

# the type with glyphs added to its cache (the faces' ASCII tables stay)
def Ty.with_cache(t: Ty, c: Map<&2, X.TG>) -> Ty:
  match t:
    case Ty{_, ui, bold, meta, body, code, loc}:
      Ty{c, ui, bold, meta, body, code, loc}

# the type with the frame's language
def Ty.with_loc(t: Ty, +l: I.Loc) -> Ty:
  match t:
    case Ty{c, ui, bold, meta, body, code, _}:
      Ty{c, ui, bold, meta, body, code, l}

def Ty.loc(t: Ty) -> I.Loc:
  match t:
    case Ty{_, _, _, _, _, _, l}:
      l

# a frame's missing glyphs fetched (X.Text.wants); the next layout draws them
def Ty.fill(t: Ty, ws: List<&2, X.Want>) -> IO(Ty):
  match t:
    case Ty{c, ui, bold, meta, body, code, loc}:
      IO.bind(Map<&2, X.TG>, Ty, X.Text.fill(ws, c), c2 => IO.pure(Ty, Ty{c2, ui, bold, meta, body, code, loc}))

def Ty.cache(t: Ty) -> Map<&2, X.TG>:
  match t:
    case Ty{c, _, _, _, _, _, _}:
      c

# 0 interface 14px, 1 bold, 2 meta 12px, 3 chat body 14px, 4 code 13px
def Ty.face(t: Ty, k: Nat) -> X.Face:
  match t k:
    case Ty{_, f, _, _, _, _, _} 0n:
      f
    case Ty{_, _, f, _, _, _, _} 1n:
      f
    case Ty{_, _, _, f, _, _, _} 2n:
      f
    case Ty{_, _, _, _, f, _, _} 3n:
      f
    case Ty{_, _, _, _, _, f, _} _:
      f

def Ty.text(+ty: Ty, +k: Nat, s: String, +c: D.Clip, +x: U32, +y: U32, +lh: U32, +color: U32) -> List<&2, R.Op>:
  X.Text.line(Ty.cache(ty), Ty.face(ty, k), s, c, x, y, lh, color)

def Ty.width(+ty: Ty, +k: Nat, s: String) -> U32:
  X.Text.width(Ty.cache(ty), Ty.face(ty, k), s)

def Ty.fit(+ty: Ty, +k: Nat, +s: String, +px: U32) -> String:
  X.Text.fit(Ty.cache(ty), Ty.face(ty, k), s, px)

def Ty.wrap(+ty: Ty, +k: Nat, s: String, +px: U32) -> List<&2, String>:
  X.Text.wrap(Ty.cache(ty), Ty.face(ty, k), s, px)

# Backplane's type: Berkeley Mono where installed, else the system's
# monospace; FreeType via fontconfig, the Spleen bitmap without them
def Ty.fonts.ui() -> List<&2, String>:
  ["Berkeley Mono", "monospace"]

def Ty.fonts.bold() -> List<&2, String>:
  ["Berkeley Mono:weight=bold", "monospace:weight=bold"]

def Ty.fonts.code() -> List<&2, String>:
  ["Berkeley Mono", "SF Mono", "SFMono-Regular", "Menlo", "Consolas", "Liberation Mono", "monospace"]

def Ty.fonts.warm(fs: List<&2, X.Face>, c: Map<&2, X.TG>) -> IO(Map<&2, X.TG>):
  match fs:
    case Nil{}:
      IO.pure(Map<&2, X.TG>, c)
    case +f <> t:
      IO.bind(Map<&2, X.TG>, Map<&2, X.TG>, X.Text.warm(c, f), c2 => IO.bind(Map<&2, X.TG>, Map<&2, X.TG>, X.Text.ensure(c2, f, "▾▸←−×"), c3 => Ty.fonts.warm(t, c3)))

def Ty.load() -> IO(Ty):
  do IO<Ty>:
    +ui : X.Face <- X.Text.face(Ty.fonts.ui(), 14)
    +bold : X.Face <- X.Text.face(Ty.fonts.bold(), 14)
    +meta : X.Face <- X.Text.face(Ty.fonts.ui(), 12)
    +code : X.Face <- X.Text.face(Ty.fonts.code(), 13)
    +c : Map<&2, X.TG> <- Ty.fonts.warm([ui, bold, meta, code], Map.new(&2, X.TG))
    return Ty{IC.Ico.warm(c, 16), X.Face.tabled(ui, c), X.Face.tabled(bold, c), X.Face.tabled(meta, c), X.Face.tabled(ui, c), X.Face.tabled(code, c), I.En{}}

# Shapes
# ------

# a - b, or 0 when b is larger (U32 subtraction wraps)
def U32.dim(+a: U32, +b: U32) -> U32:
  Bool.pick(U32, U32.is_gt(a, b), (a - b : U32), 0)

def Lay.hit(+x: U32, +y: U32, +w: U32, +h: U32, action: String, value: String) -> S.Hit:
  S.Hit{x, y, (x + w : U32), (y + h : U32), action, value}

# a Nat position (core/menu.bend's boxes) as a pixel
def Picker.px(+n: Nat) -> U32:
  U32.from_nat(n)

# a filled box; corners stay square (house rule), r is kept for callers
def Lay.round(+c: D.Clip, +x: U32, +y: U32, +w: U32, +h: U32, +r: U32, +color: U32) -> List<&2, R.Op>:
  [D.Draw.rect(c, x, y, w, h, color, 255)]

# a box with a 1px border
def Lay.box(+c: D.Clip, +x: U32, +y: U32, +w: U32, +h: U32, +r: U32, +fill: U32, +line: U32) -> List<&2, R.Op>:
  List.append(&2, R.Op, Lay.round(c, x, y, w, h, r, line), Lay.round(c, (x + 1 : U32), (y + 1 : U32), (w - 2 : U32), (h - 2 : U32), (r - 1 : U32), fill))

# a text button: rounded, 10px padding, 28px tall
def Lay.button.w(+ty: Ty, +label: String) -> U32:
  (Ty.width(ty, 2n, label) + 20 : U32)

def Lay.button(+ty: Ty, +c: D.Clip, +x: U32, +y: U32, +label: String, +color: U32, action: String, value: String) -> Out:
  +w = Lay.button.w(ty, label)
  Out{List.append(&2, R.Op, Lay.box(c, x, y, w, 28, 6, T.Th.raise(), T.Th.line()),
      Ty.text(ty, 2n, label, c, (x + 10 : U32), y, 28, color)),
    [Lay.hit(x, y, w, 28, action, value)]}

# a chip that is on or off
def Lay.chip(+ty: Ty, +x: U32, +y: U32, +label: String, +on: Bool, action: String, value: String) -> Out:
  +w = Lay.button.w(ty, label)
  Out{List.append(&2, R.Op, Lay.box(D.Clip.all(), x, y, w, 28, 6, Bool.pick(U32, on, T.Th.sel(), T.Th.raise()), Bool.pick(U32, on, T.Th.accent(), T.Th.line())),
      Ty.text(ty, 2n, label, D.Clip.all(), (x + 10 : U32), y, 28, Bool.pick(U32, on, T.Th.text(), T.Th.dim()))),
    [Lay.hit(x, y, w, 28, action, value)]}

# an icon (core/icons.bend) at 16 px with its top-left at (x, y)
def Lay.icon(+ty: Ty, +name: String, +c: D.Clip, +x: U32, +y: U32, +color: U32) -> List<&2, R.Op>:
  IC.Ico.op(Ty.cache(ty), name, 16, c, x, y, color)

# a square 28 px button holding an icon
def Lay.ibutton(+ty: Ty, +c: D.Clip, +x: U32, +y: U32, +name: String, +color: U32, action: String, value: String) -> Out:
  Out{List.append(&2, R.Op, Lay.box(c, x, y, 28, 28, 6, T.Th.raise(), T.Th.line()), Lay.icon(ty, name, c, (x + 6 : U32), (y + 6 : U32), color)),
    [Lay.hit(x, y, 28, 28, action, value)]}

# Sidebar
# -------

# the dot's color for a thread's phase, in the sidebar and on the line at
# the end of its timeline (M.Phase.of orders the phases)
def Lay.phase(p: M.Phase) -> U32:
  match p:
    case M.PhApproval{}:
      T.Th.warn()
    case M.PhInput{}:
      T.Th.input()
    case M.PhWorking{}:
      T.Th.accent()
    case M.PhMonitoring{_}:
      T.Th.waiting()
    case M.PhFailed{}:
      T.Th.bad()
    case M.PhComplete{}:
      T.Th.ok()
    case _:
      T.Th.faint()

# a thread's row; the selected one is raised, with its project's colour
# (tint) down its left edge
def Lay.row.k(+ty: Ty, +on: Bool, id: String, title: String, +ago: String, +ac: U32, +dot: U32, +tint: U32, +c: D.Clip, +y: U32) -> Out:
  +aw = Ty.width(ty, 2n, ago)
  Out{List.concat(&2, R.Op, [
      Bool.pick(List<&2, R.Op>, on, List.append(&2, R.Op, Lay.round(c, 6, (y + 1 : U32), 252, 30, 6, T.Th.raise()), [D.Draw.rect(c, 6, (y + 1 : U32), 2, 30, tint, 255)]), Nil{}),
      [D.Draw.disc(c, 19.0, U32.to_f32((y + 16 : U32)), 3.0, dot, 255)],
      Ty.text(ty, 0n, Ty.fit(ty, 0n, title, (252 - 32 - aw - 12 : U32)), c, 32, y, 32, Bool.pick(U32, on, T.Th.text(), T.Th.soft())),
      Ty.text(ty, 2n, ago, c, (252 - aw : U32), y, 32, ac)]),
    [Lay.hit(0, y, 264, 32, "select", id)]}

# a row's right-hand note: how many subagents it has at work, in the
# accent, else what the row shows there
def Lay.row.note(+badge: String, +other: String) -> String:
  Bool.pick(String, String.is_empty(badge), other, badge)

def Lay.row.nc(+badge: String, +other: U32) -> U32:
  Bool.pick(U32, String.is_empty(badge), other, T.Th.accent())

# a sidebar row; a snoozed one shows the time left instead of its age
def Lay.row.on(+ty: Ty, +t: M.Thread, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat, +snoozed: Bool, +c: D.Clip, +y: U32) -> Out:
  match t:
    case M.Thread{+id, +p, title, _, +a, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      +b = SB.Subs.badge(SB.Subs.count.ix(ix, st, id))
      Lay.row.k(ty, String.eq(id, sel), id, U.Label.title(st, id, TTL.Title.shown(Ty.loc(ty), title)), Lay.row.note(b, Bool.pick(String, snoozed, U.Time.ago(U.Side.until(st, id), now), U.Time.ago(now, a))), Lay.row.nc(b, T.Th.faint()),
        Lay.phase(U.Side.phase.ix(ix, st, t)), BG.Badge.rgb_of(st, p), c, y)

# whether a row at y (32 tall) shows in c at all (one scrolled above the
# top sits at y modulo 2^32, past the bottom)
def Lay.row.seen(c: D.Clip, +y: U32) -> Bool:
  match c:
    case D.Clip{_, +cy0, _, +cy1}:
      Bool.and(U32.is_lt(y, cy1), U32.is_gt((y + 32 : U32), cy0))

def Lay.row.id(t: M.Thread) -> String:
  match t:
    case M.Thread{id, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      id

# a row out of sight draws nothing (a long sidebar lays out only what
# shows); its hit stays, for the links, hints and slide that read rows
def Lay.row.k2(seen: Bool, +ty: Ty, +t: M.Thread, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat, +snoozed: Bool, +c: D.Clip, +y: U32) -> Out:
  match seen:
    case True{}:
      Lay.row.on(ty, t, ix, st, sel, now, snoozed, c, y)
    case False{}:
      Out{Nil{}, [Lay.hit(0, y, 264, 32, "select", Lay.row.id(t))]}

def Lay.row(+ty: Ty, +t: M.Thread, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat, +snoozed: Bool, +c: D.Clip, +y: U32) -> Out:
  Lay.row.k2(Lay.row.seen(c, y), ty, t, ix, st, sel, now, snoozed, c, y)

# The active view
# ---------------
# One list of every live thread, what waits on the person first, then the
# ones settled lately, faded (core's Active.rows). A row shows its
# project's name where the other view shows the age.

def Lay.arow.k(+ty: Ty, +on: Bool, id: String, title: String, +proj: String, +badge: String, +faded: Bool, +dot: U32, +tint: U32, +c: D.Clip, +y: U32) -> Out:
  +pn = Ty.fit(ty, 2n, Lay.row.note(badge, proj), 96)
  +pw = Ty.width(ty, 2n, pn)
  Out{List.concat(&2, R.Op, [
      Bool.pick(List<&2, R.Op>, on, List.append(&2, R.Op, Lay.round(c, 6, (y + 1 : U32), 252, 30, 6, T.Th.raise()), [D.Draw.rect(c, 6, (y + 1 : U32), 2, 30, tint, 255)]), Nil{}),
      [D.Draw.disc(c, 19.0, U32.to_f32((y + 16 : U32)), 3.0, Bool.pick(U32, faded, T.Th.line(), dot), 255)],
      Ty.text(ty, 0n, Ty.fit(ty, 0n, title, (252 - 32 - pw - 12 : U32)), c, 32, y, 32, Bool.pick(U32, on, T.Th.text(), Bool.pick(U32, faded, T.Th.faint(), T.Th.soft()))),
      Ty.text(ty, 2n, pn, c, (252 - pw : U32), y, 32, Lay.row.nc(badge, Bool.pick(U32, faded, T.Th.line(), T.Th.faint())))]),
    [Lay.hit(0, y, 264, 32, "select", id)]}

def Lay.arow.on(+ty: Ty, r: U.ARow, +ix: U.SIx, +st: M.State, +sel: String, +c: D.Clip, +y: U32) -> Out:
  match r:
    case U.ARow{_, _, +t, +pid, +pt, +faded}:
      Lay.arow.k(ty, String.eq(M.Thread.id(t), sel), M.Thread.id(t), U.Label.title(st, M.Thread.id(t), TTL.Title.shown(Ty.loc(ty), U.Active.title(t))), pt, SB.Subs.badge(SB.Subs.count.ix(ix, st, M.Thread.id(t))), faded, Lay.phase(U.Side.phase.ix(ix, st, t)), BG.Badge.rgb_of(st, pid), c, y)

def Lay.arow.id(r: U.ARow) -> String:
  match r:
    case U.ARow{_, _, t, _, _, _}:
      M.Thread.id(t)

def Lay.arow.k2(seen: Bool, +ty: Ty, r: U.ARow, +ix: U.SIx, +st: M.State, +sel: String, +c: D.Clip, +y: U32) -> Out:
  match seen:
    case True{}:
      Lay.arow.on(ty, r, ix, st, sel, c, y)
    case False{}:
      Out{Nil{}, [Lay.hit(0, y, 264, 32, "select", Lay.arow.id(r))]}

# rows from y down; answers the output and the next y
type Col is Data:
  Col{out: Out, y: U32}

def Lay.col.cat(o: Out, c: Col) -> Col:
  match c:
    case Col{o2, y}:
      Col{Out.cat(o, o2), y}

# The sidebar alone carries visible row memos. Other layout keeps Col.
type SideCol is Data:
  SideCol{out: Out, y: U32, rows: List<&2, ST.SideRowM>, hits: Nat}

def SideCol.lift(c: Col) -> SideCol:
  match c:
    case Col{o, y}: SideCol{o, y, Nil{}, 0n}

def SideCol.col(c: SideCol) -> Col:
  match c:
    case SideCol{o, y, _, _}: Col{o, y}

def SideCol.out(c: SideCol) -> Out:
  match c:
    case SideCol{o, _, _, _}: o

def SideCol.rows(c: SideCol) -> List<&2, ST.SideRowM>:
  match c:
    case SideCol{_, _, rows, _}: rows

def SideCol.hits(c: SideCol) -> Nat:
  match c:
    case SideCol{_, _, _, hits}: hits

def SideCol.row(step: ST.SideRowStep, y: U32) -> SideCol:
  match step:
    case ST.SideRowStep{ops, hs, rows, hits}: SideCol{Out{ops, hs}, y, rows, hits}

def SideCol.cat(o: Out, c: SideCol) -> SideCol:
  match c:
    case SideCol{o2, y, rows, hits}: SideCol{Out.cat(o, o2), y, rows, hits}

def SideCol.join(a: SideCol, b: SideCol) -> SideCol:
  match a b:
    case SideCol{ao, _, ar, ah} SideCol{bo, by, br, bh}:
      SideCol{Out.cat(ao, bo), by, List.append(&2, ST.SideRowM, ar, br), Nat.add(ah, bh)}

def SideCol.then(a: SideCol, k: U32 -> SideCol) -> SideCol:
  match a:
    case SideCol{o, +y, rows, hits}:
      SideCol.join(SideCol{o, y, rows, hits}, k(y))

def SideCol.layer(c: SideCol, k: Col -> Out) -> SideCol:
  match c:
    case SideCol{o, +y, rows, hits}: SideCol{k(Col{o, y}), y, rows, hits}

type SideFreshCtx is Data:
  SideFreshCtx{ty: Ty, ix: U.SIx, state: M.State, selected: String, now: Nat}

def Lay.fresh.step(o: Out) -> ST.SideRowStep:
  match o:
    case Out{ops, hs}: ST.SideRowStep{ops, hs, Nil{}, 0n}

def Lay.fresh.req(req: ST.SideRowReq, ctx: SideFreshCtx) -> ST.SideRowStep:
  match req ctx:
    case ST.SideProjectRow{t, sn, c, y} SideFreshCtx{ty, ix, st, sel, now}:
      Lay.fresh.step(Lay.row(ty, t, ix, st, sel, now, sn, c, y))
    case ST.SideActiveRow{r, +c, +y} SideFreshCtx{ty, ix, st, sel, _}:
      Lay.fresh.step(Lay.arow.k2(Lay.row.seen(c, y), ty, r, ix, st, sel, c, y))

def Lay.fresh.run(cs: List<&2, SideFreshCtx>, req: ST.SideRowReq) -> ST.SideRowStep:
  match cs:
    case ctx <> _: Lay.fresh.req(req, ctx)
    case Nil{}: Lay.fresh.req(req, SideFreshCtx{Ty.none(), U.Side.ix(M.State.empty()), M.State.empty(), "", 0n})

def Lay.fresh(ty: Ty, ix: U.SIx, st: M.State, sel: String, now: Nat) -> List<&2, SideFreshCtx>:
  [SideFreshCtx{ty, ix, st, sel, now}]

def Lay.rows.m(~PC: Data, ~paint: PC -> ST.SideRowReq -> ST.SideRowStep, +ty: Ty, ts: List<&2, M.Thread>, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat, +snoozed: Bool, +c: D.Clip, +y: U32, +pc: PC) -> SideCol:
  match ts:
    case Nil{}:
      SideCol{Out.none(), y, Nil{}, 0n}
    case t <> rest:
      SideCol.join(SideCol.row(paint(pc, ST.SideProjectRow{t, snoozed, c, y}), (y + 32 : U32)), Lay.rows.m(~PC, ~paint, ty, rest, ix, st, sel, now, snoozed, c, (y + 32 : U32), pc))

def Lay.rows(+ty: Ty, ts: List<&2, M.Thread>, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat, +snoozed: Bool, +c: D.Clip, +y: U32) -> Col:
  SideCol.col(Lay.rows.m(~(List<&2, SideFreshCtx>), ~Lay.fresh.run, ty, ts, ix, st, sel, now, snoozed, c, y, Lay.fresh(ty, ix, st, sel, now)))

def Lay.col.then(a: Col, k: U32 -> Col) -> Col:
  match a:
    case Col{o, +y}:
      Lay.col.cat(o, k(y))

def Lay.shelf.k.m(~PC: Data, ~paint: PC -> ST.SideRowReq -> ST.SideRowStep, +ty: Ty, open: Bool, +key: String, +label: String, ts: List<&2, M.Thread>, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat, +c: D.Clip, +y: U32, +pc: PC) -> SideCol:
  match open:
    case True{}:
      SideCol.cat(Out{List.append(&2, R.Op, Lay.icon(ty, "chevron-down", c, 12, (y + 6 : U32), T.Th.dim()), Ty.text(ty, 2n, label, c, 32, y, 28, T.Th.dim())), [Lay.hit(0, y, 264, 28, "toggle-settled", key)]},
        Lay.rows.m(~PC, ~paint, ty, ts, ix, st, sel, now, False{}, c, (y + 28 : U32), pc))
    case False{}:
      SideCol{Out{List.append(&2, R.Op, Lay.icon(ty, "chevron-right", c, 12, (y + 6 : U32), T.Th.faint()), Ty.text(ty, 2n, label, c, 32, y, 28, T.Th.faint())), [Lay.hit(0, y, 264, 28, "toggle-settled", key)]}, (y + 28 : U32), Nil{}, 0n}

def Lay.shelf.k(+ty: Ty, open: Bool, +key: String, +label: String, ts: List<&2, M.Thread>, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat, +c: D.Clip, +y: U32) -> Col:
  SideCol.col(Lay.shelf.k.m(~(List<&2, SideFreshCtx>), ~Lay.fresh.run, ty, open, key, label, ts, ix, st, sel, now, c, y, Lay.fresh(ty, ix, st, sel, now)))

# a collapsible shelf (Settled, Archived): its name and count, then its rows
# when open; nothing when empty
def Lay.shelf.m(~PC: Data, ~paint: PC -> ST.SideRowReq -> ST.SideRowStep, +ty: Ty, open: Bool, +key: String, +name: String, st: List<&2, M.Thread>, +ix: U.SIx, +s: M.State, +sel: String, +now: Nat, +c: D.Clip, +y: U32, +pc: PC) -> SideCol:
  match st:
    case Nil{}:
      SideCol{Out.none(), y, Nil{}, 0n}
    case +t <> +rest:
      Lay.shelf.k.m(~PC, ~paint, ty, open, key, name ++ " " ++ Nat.show(1n+List.length(&2, M.Thread, rest)), t <> rest, ix, s, sel, now, c, y, pc)

def Lay.shelf(+ty: Ty, open: Bool, +key: String, +name: String, st: List<&2, M.Thread>, +ix: U.SIx, +s: M.State, +sel: String, +now: Nat, +c: D.Clip, +y: U32) -> Col:
  SideCol.col(Lay.shelf.m(~(List<&2, SideFreshCtx>), ~Lay.fresh.run, ty, open, key, name, st, ix, s, sel, now, c, y, Lay.fresh(ty, ix, s, sel, now)))

# snoozed threads under a label, soonest to wake first
def Lay.snoozed.m(~PC: Data, ~paint: PC -> ST.SideRowReq -> ST.SideRowStep, +ty: Ty, ts: List<&2, M.Thread>, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat, +c: D.Clip, +y: U32, +pc: PC) -> SideCol:
  match ts:
    case Nil{}:
      SideCol{Out.none(), y, Nil{}, 0n}
    case +t <> +rest:
      SideCol.cat(Out.ops(Ty.text(ty, 2n, MSG.T.sidebar.snoozed(Ty.loc(ty)), c, 16, y, 28, T.Th.faint())), Lay.rows.m(~PC, ~paint, ty, t <> rest, ix, st, sel, now, True{}, c, (y + 28 : U32), pc))

def Lay.snoozed(+ty: Ty, ts: List<&2, M.Thread>, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat, +c: D.Clip, +y: U32) -> Col:
  SideCol.col(Lay.snoozed.m(~(List<&2, SideFreshCtx>), ~Lay.fresh.run, ty, ts, ix, st, sel, now, c, y, Lay.fresh(ty, ix, st, sel, now)))

# A project's badge editor, open under its name: the tints, then the
# icons, each set placed by the flow in core/menu.bend (law
# menu_chips_apart), so neither set ever overlaps itself or the other
def Look.n(n: Nat, +w: Nat) -> List<&2, Nat>:
  match n:
    case 0n:
      Nil{}
    case 1n+k:
      w <> Look.n(k, w)

def Look.swatches(bs: List<&2, MN.Box>, cs: List<&2, String>, +cur: String, +id: String, +c: D.Clip) -> Out:
  match bs:
    case Nil{}:
      Out.none()
    case MN.Box{+x, +y, +w, +h} <> bt:
      match cs:
        case Nil{}:
          Out.none()
        case +k <> ct:
          Out.cat(Out{List.append(&2, R.Op,
              Bool.pick(List<&2, R.Op>, String.eq(k, cur), [D.Draw.rect(c, Picker.px(Nat.sub(x, 2n)), Picker.px(Nat.sub(y, 2n)), Picker.px(Nat.add(w, 4n)), Picker.px(Nat.add(h, 4n)), T.Th.text(), 255)], Nil{}),
              [D.Draw.rect(c, Picker.px(x), Picker.px(y), Picker.px(w), Picker.px(h), BG.Badge.rgb(k), 255)]),
              [Lay.hit(Picker.px(x), Picker.px(y), Picker.px(w), Picker.px(h), "look-set", BG.Badge.color_key(id) ++ "=" ++ k)]},
            Look.swatches(bt, ct, cur, id, c))

def Look.icons(+ty: Ty, bs: List<&2, MN.Box>, ns: List<&2, String>, +cur: String, +id: String, +tint: U32, +c: D.Clip) -> Out:
  match bs:
    case Nil{}:
      Out.none()
    case MN.Box{+x, +y, _, _} <> bt:
      match ns:
        case Nil{}:
          Out.none()
        case +k <> nt:
          Out.cat(Out{List.append(&2, R.Op,
              Lay.box(c, Picker.px(x), Picker.px(y), 28, 28, 0, Bool.pick(U32, String.eq(k, cur), T.Th.sel(), T.Th.raise()), Bool.pick(U32, String.eq(k, cur), tint, T.Th.line())),
              Lay.icon(ty, k, c, Picker.px(Nat.add(x, 6n)), Picker.px(Nat.add(y, 6n)), Bool.pick(U32, String.eq(k, cur), tint, T.Th.dim()))),
              [Lay.hit(Picker.px(x), Picker.px(y), 28, 28, "look-set", BG.Badge.icon_key(id) ++ "=" ++ k)]},
            Look.icons(ty, bt, nt, cur, id, tint, c))

def Look.panel(+ty: Ty, +st: M.State, +id: String, +rm: Bool, +c: D.Clip, +y: U32) -> Col:
  +sws = Look.n(List.length(&2, String, BG.Badge.colors()), 18n)
  +iws = Look.n(List.length(&2, String, BG.Badge.icons()), 28n)
  +y0 = {Nat.add(U32.to_nat(y), 12n) : Nat}
  +y1 = {Nat.add(MN.Flow.under(sws, 18n, 248n, 18n, y0, 18n, 5n), 5n) : Nat}
  +y2 = MN.Flow.under(iws, 18n, 248n, 18n, y1, 28n, 4n)
  +yb = {Nat.add(y2, 8n) : Nat}
  +hh = {Nat.sub(Nat.add(yb, 34n), U32.to_nat(y)) : Nat}
  Col{Out.all([
      Out{Lay.box(c, 8, (y + 2 : U32), 248, Picker.px(Nat.sub(hh, 2n)), 0, T.Th.raise(), T.Th.line()), [Lay.hit(8, (y + 2 : U32), 248, Picker.px(Nat.sub(hh, 2n)), "look-bg", id)]},
      Look.swatches(MN.Flow.place(sws, 18n, 248n, 18n, y0, 18n, 5n), BG.Badge.colors(), BG.Badge.color(st, id), id, c),
      Look.icons(ty, MN.Flow.place(iws, 18n, 248n, 18n, y1, 28n, 4n), BG.Badge.icons(), BG.Badge.icon(st, id), id, BG.Badge.rgb_of(st, id), c),
      Lay.button(ty, c, 18, Picker.px(yb), Bool.pick(String, rm, MSG.T.menu.remove.confirm(Ty.loc(ty)), MSG.T.menu.project.remove(Ty.loc(ty))), T.Th.bad(), "proj-remove", id)]),
    Picker.px(Nat.add(U32.to_nat(y), Nat.add(hh, 6n)))}

def Look.open(open: Bool, +ty: Ty, +st: M.State, +id: String, +rm: Bool, +c: D.Clip, +y: U32) -> Col:
  match open:
    case True{}:
      Look.panel(ty, st, id, rm, c, y)
    case False{}:
      Col{Out.none(), y}

# a project's name under its badge's icon (a click opens the editor; its
# hit starts past x 10, where a drag drops onto the header), its
# + for a new thread, the editor when open, then its threads
# (quiet: no live thread, so its name and badge are faded)
def Lay.project.m(~PC: Data, ~paint: PC -> ST.SideRowReq -> ST.SideRowStep, +ty: Ty, r: U.SRow, +quiet: Bool, +open: Map<&2, Bool>, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat, +look: String, +rm: String, +c: D.Clip, +y: U32, +pc: PC) -> SideCol:
  match r:
    case U.SRow{_, M.Project{+id, title, _, _, _}, M.Shelves{pinned, active, settled, hidden, snoozed}}:
      +fade = Bool.or(quiet, U.Label.syncing(st, id))
      SideCol.cat(Out.all([Out{Ty.text(ty, 2n, Ty.fit(ty, 2n, U.Label.title(st, id, title), 180), c, 38, (y + 4 : U32), 28, Bool.pick(U32, fade, T.Th.faint(), T.Th.dim())), [Lay.hit(0, y, 228, 32, "project", id)]},
          Out{Lay.icon(ty, BG.Badge.icon(st, id), c, 14, (y + 10 : U32), Bool.pick(U32, fade, T.Th.faint(), BG.Badge.rgb_of(st, id))), [Lay.hit(12, (y + 4 : U32), 24, 28, "look", id)]},
          Out{Lay.icon(ty, "plus", c, 236, (y + 10 : U32), T.Th.faint()), [Lay.hit(230, (y + 4 : U32), 28, 28, "new-thread", id)]}]),
        SideCol.then(SideCol.lift(Look.open(String.eq(look, id), ty, st, id, String.eq(rm, id), c, (y + 32 : U32))), y0 =>
        SideCol.then(Lay.rows.m(~PC, ~paint, ty, pinned, ix, st, sel, now, False{}, c, y0, pc), y1 =>
          SideCol.then(Lay.rows.m(~PC, ~paint, ty, active, ix, st, sel, now, False{}, c, y1, pc), y2 =>
            SideCol.then(Lay.snoozed.m(~PC, ~paint, ty, snoozed, ix, st, sel, now, c, y2, pc), y3 =>
              SideCol.then(Lay.shelf.m(~PC, ~paint, ty, U.Flag.get(open, id), id, MSG.T.notify.settled(Ty.loc(ty)), settled, ix, st, sel, now, c, y3, pc), y4 =>
                Lay.shelf.m(~PC, ~paint, ty, U.Flag.get(open, "arch:" ++ id), "arch:" ++ id, MSG.T.sidebar.archived(Ty.loc(ty)), M.Archive.of(hidden), ix, st, sel, now, c, y4, pc)))))))

def Lay.project(+ty: Ty, r: U.SRow, +quiet: Bool, +open: Map<&2, Bool>, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat, +look: String, +rm: String, +c: D.Clip, +y: U32) -> Col:
  SideCol.col(Lay.project.m(~(List<&2, SideFreshCtx>), ~Lay.fresh.run, ty, r, quiet, open, ix, st, sel, now, look, rm, c, y, Lay.fresh(ty, ix, st, sel, now)))

def Lay.projects.m(~PC: Data, ~paint: PC -> ST.SideRowReq -> ST.SideRowStep, +ty: Ty, rs: List<&2, U.SRow>, +quiet: Bool, +open: Map<&2, Bool>, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat, +look: String, +rm: String, +c: D.Clip, +y: U32, +pc: PC) -> SideCol:
  match rs:
    case Nil{}:
      SideCol{Out.none(), y, Nil{}, 0n}
    case r <> t:
      SideCol.then(Lay.project.m(~PC, ~paint, ty, r, quiet, open, ix, st, sel, now, look, rm, c, y, pc), y2 => Lay.projects.m(~PC, ~paint, ty, t, quiet, open, ix, st, sel, now, look, rm, c, (y2 + 8 : U32), pc))

def Lay.projects(+ty: Ty, rs: List<&2, U.SRow>, +quiet: Bool, +open: Map<&2, Bool>, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat, +look: String, +rm: String, +c: D.Clip, +y: U32) -> Col:
  SideCol.col(Lay.projects.m(~(List<&2, SideFreshCtx>), ~Lay.fresh.run, ty, rs, quiet, open, ix, st, sel, now, look, rm, c, y, Lay.fresh(ty, ix, st, sel, now)))

# the older projects (quiet for Quiet.days): folded under one row at the
# end; a search shows them in place
def Lay.older.rows.m(~PC: Data, ~paint: PC -> ST.SideRowReq -> ST.SideRowStep, open: Bool, +ty: Ty, rs: List<&2, U.SRow>, +op: Map<&2, Bool>, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat, +look: String, +rm: String, +c: D.Clip, +y: U32, +pc: PC) -> SideCol:
  match open:
    case True{}:
      Lay.projects.m(~PC, ~paint, ty, rs, True{}, op, ix, st, sel, now, look, rm, c, y, pc)
    case False{}:
      SideCol{Out.none(), y, Nil{}, 0n}

def Lay.older.rows(open: Bool, +ty: Ty, rs: List<&2, U.SRow>, +op: Map<&2, Bool>, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat, +look: String, +rm: String, +c: D.Clip, +y: U32) -> Col:
  SideCol.col(Lay.older.rows.m(~(List<&2, SideFreshCtx>), ~Lay.fresh.run, open, ty, rs, op, ix, st, sel, now, look, rm, c, y, Lay.fresh(ty, ix, st, sel, now)))

def Lay.older.k.m(~PC: Data, ~paint: PC -> ST.SideRowReq -> ST.SideRowStep, searching: Bool, +ty: Ty, +rs: List<&2, U.SRow>, +op: Map<&2, Bool>, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat, +look: String, +rm: String, +c: D.Clip, +y: U32, +pc: PC) -> SideCol:
  match searching:
    case True{}:
      Lay.projects.m(~PC, ~paint, ty, rs, True{}, op, ix, st, sel, now, look, rm, c, y, pc)
    case False{}:
      SideCol.cat(Out{List.append(&2, R.Op, Lay.icon(ty, Bool.pick(String, U.Flag.get(op, "old-projects"), "chevron-down", "chevron-right"), c, 12, (y + 6 : U32), T.Th.faint()),
          Ty.text(ty, 2n, MSG.T.sidebar.older(Ty.loc(ty), Nat.show(List.length(&2, U.SRow, rs))), c, 32, y, 28, T.Th.faint())), [Lay.hit(0, y, 264, 28, "side-older", "")]},
        Lay.older.rows.m(~PC, ~paint, U.Flag.get(op, "old-projects"), ty, rs, op, ix, st, sel, now, look, rm, c, (y + 32 : U32), pc))

def Lay.older.k(searching: Bool, +ty: Ty, +rs: List<&2, U.SRow>, +op: Map<&2, Bool>, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat, +look: String, +rm: String, +c: D.Clip, +y: U32) -> Col:
  SideCol.col(Lay.older.k.m(~(List<&2, SideFreshCtx>), ~Lay.fresh.run, searching, ty, rs, op, ix, st, sel, now, look, rm, c, y, Lay.fresh(ty, ix, st, sel, now)))

def Lay.older.m(~PC: Data, ~paint: PC -> ST.SideRowReq -> ST.SideRowStep, +ty: Ty, rs: List<&2, U.SRow>, +searching: Bool, +op: Map<&2, Bool>, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat, +look: String, +rm: String, +c: D.Clip, +y: U32, +pc: PC) -> SideCol:
  match rs:
    case Nil{}:
      SideCol{Out.none(), y, Nil{}, 0n}
    case +r <> +t:
      Lay.older.k.m(~PC, ~paint, searching, ty, r <> t, op, ix, st, sel, now, look, rm, c, y, pc)

def Lay.older(+ty: Ty, rs: List<&2, U.SRow>, +searching: Bool, +op: Map<&2, Bool>, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat, +look: String, +rm: String, +c: D.Clip, +y: U32) -> Col:
  SideCol.col(Lay.older.m(~(List<&2, SideFreshCtx>), ~Lay.fresh.run, ty, rs, searching, op, ix, st, sel, now, look, rm, c, y, Lay.fresh(ty, ix, st, sel, now)))

# the projects view: live projects, then the quiet ones faded, then the
# older ones folded (core's Side.groups)
def Lay.groups.m(~PC: Data, ~paint: PC -> ST.SideRowReq -> ST.SideRowStep, +ty: Ty, g: U.SGroups, +q: String, +op: Map<&2, Bool>, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat, +look: String, +rm: String, +c: D.Clip, +y: U32, +pc: PC) -> SideCol:
  match g:
    case U.SGroups{l, qs, o}:
      SideCol.then(Lay.projects.m(~PC, ~paint, ty, l, False{}, op, ix, st, sel, now, look, rm, c, y, pc), y1 =>
        SideCol.then(Lay.projects.m(~PC, ~paint, ty, qs, True{}, op, ix, st, sel, now, look, rm, c, y1, pc), y2 =>
          Lay.older.m(~PC, ~paint, ty, o, Bool.not(String.is_empty(q)), op, ix, st, sel, now, look, rm, c, y2, pc)))

def Lay.groups(+ty: Ty, g: U.SGroups, +q: String, +op: Map<&2, Bool>, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat, +look: String, +rm: String, +c: D.Clip, +y: U32) -> Col:
  SideCol.col(Lay.groups.m(~(List<&2, SideFreshCtx>), ~Lay.fresh.run, ty, g, q, op, ix, st, sel, now, look, rm, c, y, Lay.fresh(ty, ix, st, sel, now)))

# Active list layout
# ------------------

def Lay.arows.m(~PC: Data, ~paint: PC -> ST.SideRowReq -> ST.SideRowStep, +ty: Ty, rs: List<&2, U.ARow>, +ix: U.SIx, +st: M.State, +sel: String, +c: D.Clip, +y: U32, +pc: PC) -> SideCol:
  match rs:
    case Nil{}:
      SideCol{Out.none(), y, Nil{}, 0n}
    case r <> t:
      SideCol.join(SideCol.row(paint(pc, ST.SideActiveRow{r, c, y}), (y + 32 : U32)), Lay.arows.m(~PC, ~paint, ty, t, ix, st, sel, c, (y + 32 : U32), pc))

def Lay.arows(+ty: Ty, rs: List<&2, U.ARow>, +ix: U.SIx, +st: M.State, +sel: String, +c: D.Clip, +y: U32) -> Col:
  SideCol.col(Lay.arows.m(~(List<&2, SideFreshCtx>), ~Lay.fresh.run, ty, rs, ix, st, sel, c, y, Lay.fresh(ty, ix, st, sel, 0n)))

# the settled rows: folded under one "Settled N" row, shown while it is
# open or a search is on (core's Active.open)
def Lay.afaded.rows.m(~PC: Data, ~paint: PC -> ST.SideRowReq -> ST.SideRowStep, open: Bool, +ty: Ty, rs: List<&2, U.ARow>, +ix: U.SIx, +st: M.State, +sel: String, +c: D.Clip, +y: U32, +pc: PC) -> SideCol:
  match open:
    case True{}:
      Lay.arows.m(~PC, ~paint, ty, rs, ix, st, sel, c, y, pc)
    case False{}:
      SideCol{Out.none(), y, Nil{}, 0n}

def Lay.afaded.rows(open: Bool, +ty: Ty, rs: List<&2, U.ARow>, +ix: U.SIx, +st: M.State, +sel: String, +c: D.Clip, +y: U32) -> Col:
  SideCol.col(Lay.afaded.rows.m(~(List<&2, SideFreshCtx>), ~Lay.fresh.run, open, ty, rs, ix, st, sel, c, y, Lay.fresh(ty, ix, st, sel, 0n)))

def Lay.afaded.k.m(~PC: Data, ~paint: PC -> ST.SideRowReq -> ST.SideRowStep, searching: Bool, +open: Bool, +ty: Ty, +rs: List<&2, U.ARow>, +ix: U.SIx, +st: M.State, +sel: String, +c: D.Clip, +y: U32, +pc: PC) -> SideCol:
  match searching:
    case True{}:
      Lay.arows.m(~PC, ~paint, ty, rs, ix, st, sel, c, y, pc)
    case False{}:
      SideCol.cat(Out{List.append(&2, R.Op, Lay.icon(ty, Bool.pick(String, open, "chevron-down", "chevron-right"), c, 12, (y + 6 : U32), T.Th.faint()),
          Ty.text(ty, 2n, MSG.T.sidebar.settled.count(Ty.loc(ty), Nat.show(List.length(&2, U.ARow, rs))), c, 32, y, 28, T.Th.faint())), [Lay.hit(0, y, 264, 28, "side-settled", "")]},
        Lay.afaded.rows.m(~PC, ~paint, open, ty, rs, ix, st, sel, c, (y + 32 : U32), pc))

def Lay.afaded.k(searching: Bool, +open: Bool, +ty: Ty, +rs: List<&2, U.ARow>, +ix: U.SIx, +st: M.State, +sel: String, +c: D.Clip, +y: U32) -> Col:
  SideCol.col(Lay.afaded.k.m(~(List<&2, SideFreshCtx>), ~Lay.fresh.run, searching, open, ty, rs, ix, st, sel, c, y, Lay.fresh(ty, ix, st, sel, 0n)))

def Lay.afaded.m(~PC: Data, ~paint: PC -> ST.SideRowReq -> ST.SideRowStep, +ty: Ty, rs: List<&2, U.ARow>, +q: String, +op: Map<&2, Bool>, +ix: U.SIx, +st: M.State, +sel: String, +c: D.Clip, +y: U32, +pc: PC) -> SideCol:
  match rs:
    case Nil{}:
      SideCol{Out.none(), y, Nil{}, 0n}
    case +r <> +t:
      Lay.afaded.k.m(~PC, ~paint, Bool.not(String.is_empty(q)), U.Flag.get(op, "active-settled"), ty, r <> t, ix, st, sel, c, y, pc)

def Lay.afaded(+ty: Ty, rs: List<&2, U.ARow>, +q: String, +op: Map<&2, Bool>, +ix: U.SIx, +st: M.State, +sel: String, +c: D.Clip, +y: U32) -> Col:
  SideCol.col(Lay.afaded.m(~(List<&2, SideFreshCtx>), ~Lay.fresh.run, ty, rs, q, op, ix, st, sel, c, y, Lay.fresh(ty, ix, st, sel, 0n)))

def Lay.active.k.m(~PC: Data, ~paint: PC -> ST.SideRowReq -> ST.SideRowStep, p: U.AParts, +ty: Ty, +q: String, +op: Map<&2, Bool>, +ix: U.SIx, +st: M.State, +sel: String, +c: D.Clip, +y: U32, +pc: PC) -> SideCol:
  match p:
    case U.AParts{l, f}:
      SideCol.then(Lay.arows.m(~PC, ~paint, ty, l, ix, st, sel, c, y, pc), y1 => Lay.afaded.m(~PC, ~paint, ty, f, q, op, ix, st, sel, c, y1, pc))

def Lay.active.k(p: U.AParts, +ty: Ty, +q: String, +op: Map<&2, Bool>, +ix: U.SIx, +st: M.State, +sel: String, +c: D.Clip, +y: U32) -> Col:
  SideCol.col(Lay.active.k.m(~(List<&2, SideFreshCtx>), ~Lay.fresh.run, p, ty, q, op, ix, st, sel, c, y, Lay.fresh(ty, ix, st, sel, 0n)))

def Lay.active.m(~PC: Data, ~paint: PC -> ST.SideRowReq -> ST.SideRowStep, +ty: Ty, rs: List<&2, U.ARow>, +q: String, +op: Map<&2, Bool>, +ix: U.SIx, +st: M.State, +sel: String, +c: D.Clip, +y: U32, +pc: PC) -> SideCol:
  match rs:
    case Nil{}:
      SideCol{Out.ops(Ty.text(ty, 2n, MSG.T.sidebar.nothing(Ty.loc(ty)), c, 16, y, 32, T.Th.faint())), (y + 32 : U32), Nil{}, 0n}
    case +r <> +t:
      Lay.active.k.m(~PC, ~paint, U.Active.parts(r <> t), ty, q, op, ix, st, sel, c, y, pc)

def Lay.active(+ty: Ty, rs: List<&2, U.ARow>, +q: String, +op: Map<&2, Bool>, +ix: U.SIx, +st: M.State, +sel: String, +c: D.Clip, +y: U32) -> Col:
  SideCol.col(Lay.active.m(~(List<&2, SideFreshCtx>), ~Lay.fresh.run, ty, rs, q, op, ix, st, sel, c, y, Lay.fresh(ty, ix, st, sel, 0n)))

# the list the "sidebar.view" setting picks (only that one is laid out)
def Lay.side.list.k.m(~PC: Data, ~paint: PC -> ST.SideRowReq -> ST.SideRowStep, active: Bool, +ty: Ty, +q: String, +op: Map<&2, Bool>, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat, +look: String, +rm: String, +c: D.Clip, +y: U32, +pc: PC) -> SideCol:
  match active:
    case True{}:
      Lay.active.m(~PC, ~paint, ty, U.Active.rows.ix(st, ix, now, q), q, op, ix, st, sel, c, y, pc)
    case False{}:
      Lay.groups.m(~PC, ~paint, ty, U.Side.groups(U.Side.shown.ix(st, ix, now, q), now), q, op, ix, st, sel, now, look, rm, c, y, pc)

def Lay.side.list.k(active: Bool, +ty: Ty, +q: String, +op: Map<&2, Bool>, +ix: U.SIx, +st: M.State, +sel: String, +now: Nat, +look: String, +rm: String, +c: D.Clip, +y: U32) -> Col:
  SideCol.col(Lay.side.list.k.m(~(List<&2, SideFreshCtx>), ~Lay.fresh.run, active, ty, q, op, ix, st, sel, now, look, rm, c, y, Lay.fresh(ty, ix, st, sel, now)))

# (what every row asks of the whole state is gathered once a frame:
# core's Side.ix)
def Lay.side.list(active: Bool, +ty: Ty, +q: String, +op: Map<&2, Bool>, +st: M.State, +sel: String, +now: Nat, +look: String, +rm: String, +c: D.Clip, +y: U32) -> Col:
  Lay.side.list.k(active, ty, q, op, U.Side.ix(st), st, sel, now, look, rm, c, y)

# a field's caret: one that blinks (D.Draw.caret: a blink changes only
# it), or a steady one
def Lay.field.caret(steady: Bool, +c: D.Clip, +x: U32, +y: U32, +blink: Bool) -> R.Op:
  match steady:
    case True{}:
      D.Draw.rect(c, x, y, 1, 14, T.Th.accent(), 255)
    case False{}:
      D.Draw.caret(c, x, y, 1, 14, T.Th.accent(), 255, blink)

def Lay.field.x(+ty: Ty, +c: D.Clip, +x: U32, +y: U32, +w: U32, +text: String, +focused: Bool, +blink: Bool, +steady: Bool, +caret: Nat, +mark: Nat, +placeholder: String, +id: String) -> Out:
  +vis = Ty.fit(ty, 2n, text, (w - 16 : U32))
  +a = {Ty.width(ty, 2n, String.take(vis, ED.Edit.lo(caret, mark))) : U32}
  +b = {Ty.width(ty, 2n, String.take(vis, ED.Edit.hi(caret, mark))) : U32}
  Out{List.concat(&2, R.Op, [
      Lay.box(c, x, y, w, 28, 6, T.Th.bg(), Bool.pick(U32, focused, T.Th.accent(), T.Th.line())),
      Bool.pick(List<&2, R.Op>, Bool.and(focused, U32.is_gt(b, a)), [D.Draw.rect(c, (x + 8 + a : U32), (y + 5 : U32), (b - a : U32), 18, T.Th.sel(), 255)], Nil{}),
      Bool.pick(List<&2, R.Op>, String.is_empty(text), Ty.text(ty, 2n, placeholder, c, (x + 8 : U32), y, 28, T.Th.faint()),
        Ty.text(ty, 2n, vis, c, (x + 8 : U32), y, 28, T.Th.text())),
      Bool.pick(List<&2, R.Op>, focused, [Lay.field.caret(steady, c, (x + 8 + Ty.width(ty, 2n, String.take(vis, caret)) : U32), (y + 7 : U32), blink)], Nil{})]),
    Bool.pick(List<&2, S.Hit>, focused, [Lay.hit(x, y, w, 28, "focus", id), Lay.hit((x + 8 + Ty.width(ty, 2n, String.take(vis, caret)) : U32), (y + 7 : U32), 1, 18, "ime-spot", "")], [Lay.hit(x, y, w, 28, "focus", id)])}

def Lay.field(+ty: Ty, +c: D.Clip, +x: U32, +y: U32, +w: U32, +text: String, +focused: Bool, +blink: Bool, +caret: Nat, +mark: Nat, +placeholder: String, +id: String) -> Out:
  Lay.field.x(ty, c, x, y, w, text, focused, blink, False{}, caret, mark, placeholder, id)

def Lay.adding(+ty: Ty, adding: Bool, +dy: U32, +path: String, +focused: Bool, +caret: Nat, +mark: Nat) -> Out:
  match adding:
    case True{}:
      Out.cat(Lay.field.x(ty, D.Clip.all(), 12, (58 + dy : U32), 186, path, focused, True{}, True{}, caret, mark, MSG.T.sidebar.path.placeholder(Ty.loc(ty)), "project-path"),
        Lay.button(ty, D.Clip.all(), 204, (58 + dy : U32), MSG.T.common.add(Ty.loc(ty)), T.Th.accent(), "add-project", path))
    case False{}:
      Out.none()

def Lay.col.out(c: Col) -> Out:
  match c:
    case Col{o, _}:
      o

def Links.row.k(sel: Bool, +v: String, +y: U32, m: Map<&2, U32>) -> Map<&2, U32>:
  match sel:
    case True{}:
      Map.set(&2, U32, m, v, (y + 1 : U32))
    case False{}:
      m

# Links in the sidebar
# --------------------
# A message between two threads draws a connector between their rows in
# the sidebar gutter: bright while fresh, fading over 15 minutes.

def Links.rows(hs: List<&2, S.Hit>, m: Map<&2, U32>) -> Map<&2, U32>:
  match hs:
    case Nil{}:
      m
    case S.Hit{_, +y0, _, _, +a, +v} <> t:
      Links.rows(t, Links.row.k(String.eq(a, "select"), v, y0, m))

def Links.y.k(r: Map<&2, U32> & U32) -> U32:
  (_, y) = r
  y

# a row's y plus one (0: not on screen)
def Links.y(+m: Map<&2, U32>, +id: String) -> U32:
  Links.y.k(Map.get(U32, 0, m, id))

# a link fades in steps, one a minute from two minutes old (the same fade
# as before, 185 levels over 13 minutes, moved once a minute so a frame
# with nothing else new can wait for it: Links.due)
def Links.step(+age: Nat) -> Nat:
  Nat.div(Nat.sub(age, 120n), 60n)

def Links.alpha(+age: Nat) -> U32:
  U32.from_nat(Nat.sub(255n, Nat.div(Nat.mul(Links.step(age), 11100n), 780n)))

def Links.one.k(ok: Bool, +ya: U32, +yb: U32, +x: U32, +alpha: U32, +c: D.Clip) -> List<&2, R.Op>:
  match ok:
    case True{}:
      [D.Draw.rect(c, x, (U32.min(ya, yb) + 15 : U32), 2, (U32.max(ya, yb) - U32.min(ya, yb) + 2 : U32), T.Th.accent(), alpha),
       D.Draw.rect(c, x, (ya + 15 : U32), (14 - x : U32), 2, T.Th.accent(), alpha),
       D.Draw.rect(c, x, (yb + 15 : U32), (14 - x : U32), 2, T.Th.accent(), alpha),
       D.Draw.disc(c, 15.0, U32.to_f32((yb + 16 : U32)), 2.5, T.Th.accent(), alpha)]
    case False{}:
      Nil{}

def Links.one(l: U.Link2, +rows: Map<&2, U32>, +k: Nat, +now: Nat, +c: D.Clip) -> List<&2, R.Op>:
  match l:
    case U.Link2{+from, +to, +at}:
      Links.one.k(Bool.and(Bool.and(U32.is_gt(Links.y(rows, from), 0), U32.is_gt(Links.y(rows, to), 0)), Bool.not(String.eq(from, to))),
        (Links.y(rows, from) - 1 : U32), (Links.y(rows, to) - 1 : U32), U32.from_nat(Nat.sub(9n, Nat.mul(Nat.mod(k, 3n), 3n))),
        Links.alpha(Nat.sub(now, at)), c)

def Links.all(ls: List<&2, U.Link2>, +rows: Map<&2, U32>, +k: Nat, +now: Nat, +c: D.Clip) -> List<&2, R.Op>:
  match ls:
    case Nil{}:
      Nil{}
    case l <> t:
      List.append(&2, R.Op, Links.one(l, rows, k, now, c), Links.all(t, rows, 1n+k, now, c))

# the sidebar's rows with their links drawn over them
# the age at which a link's alpha (Links.alpha) next changes; 0: never
# again (it has faded out)
def Links.next(+age: Nat) -> Nat:
  Bool.pick(Nat, U32.is_eq(Links.alpha(age), 0), 0n, Nat.add(120n, Nat.mul(Nat.add(Links.step(age), 1n), 60n)))

def Links.due.one(l: U.Link2, +now: Nat) -> Nat:
  match l:
    case U.Link2{_, _, +at}:
      +a = Links.next(Nat.sub(now, at))
      Bool.pick(Nat, Nat.is_eq(a, 0n), 0n, Nat.add(at, a))

# the earlier of two times, 0 being none
def Due.min(+a: Nat, +b: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_eq(a, 0n), b, Bool.pick(Nat, Nat.is_eq(b, 0n), a, Bool.pick(Nat, Nat.is_lt(a, b), a, b)))

# when the links' fades next change (epoch seconds; 0: none fades)
def Links.due(ls: List<&2, U.Link2>, +now: Nat) -> Nat:
  match ls:
    case Nil{}:
      0n
    case l <> t:
      Due.min(Links.due.one(l, now), Links.due(t, now))

# a frame whose links still fade is due again then: the window draws it
# whole at that time even if nothing else changed (main.bend's Due.ok)
def Due.hit(+at: Nat) -> List<&2, S.Hit>:
  Bool.pick(List<&2, S.Hit>, Nat.is_eq(at, 0n), Nil{}, [Lay.hit(0, 0, 0, 0, "due", Nat.show(at))])

# (the rows' places are found only when there are links to draw)
def Links.over(o: Out, ls: List<&2, U.Link2>, +now: Nat, +c: D.Clip) -> Out:
  match o ls:
    case _ Nil{}:
      o
    case Out{os, +hs} +l <> +lt:
      Out{List.append(&2, R.Op, os, Links.all(l <> lt, Links.rows(hs, Map.new(&2, U32)), 0n, now, c)), List.append(&2, S.Hit, hs, Due.hit(Links.due(l <> lt, now)))}

# Machines
# --------
# Backplane is one app on every machine the owner has: which machine's
# projects show is a quiet choice at the foot of the sidebar (only with
# more than one), next to Settings; its list opens upward.

def Mach.rows(+ty: Ty, rs: List<&2, U.MRow>, +y: U32) -> Out:
  match rs:
    case Nil{}:
      Out.none()
    case U.MRow{+n, +url, +on} <> t:
      Out.cat(Out{List.append(&2, R.Op,
          [D.Draw.disc(D.Clip.all(), 22.0, U32.to_f32((y + 15 : U32)), 3.0, Bool.pick(U32, on, T.Th.accent(), T.Th.line()), 255)],
          Ty.text(ty, 2n, Ty.fit(ty, 2n, n, 200), D.Clip.all(), 34, y, 30, Bool.pick(U32, on, T.Th.text(), T.Th.soft()))),
          [Lay.hit(4, y, 256, 30, "machine", url)]},
        Mach.rows(ty, t, (y + 30 : U32)))

def Mach.menu(+ty: Ty, open: Bool, +rs: List<&2, U.MRow>, +h: U32) -> Out:
  match open:
    case True{}:
      +hh = {(U32.from_nat(List.length(&2, U.MRow, rs)) * 30 + 8 : U32) : U32}
      Out.cat(Out.ops(Lay.box(D.Clip.all(), 4, (h - 52 - hh : U32), 256, hh, 0, T.Th.raise(), T.Th.line())),
        Mach.rows(ty, rs, (h - 48 - hh : U32)))
    case False{}:
      Out.none()

# the machine shown, at the right of the Settings row
def Mach.bar.k(any: Bool, +ty: Ty, +rs: List<&2, U.MRow>, +h: U32) -> Out:
  match any:
    case True{}:
      +nm = Ty.fit(ty, 2n, U.Machines.current(rs), 110)
      +nw = Ty.width(ty, 2n, nm)
      Out{List.append(&2, R.Op, Ty.text(ty, 2n, nm, D.Clip.all(), (236 - nw : U32), (h - 48 : U32), 48, T.Th.faint()),
          Lay.icon(ty, "chevron-up", D.Clip.all(), 238, (h - 32 : U32), T.Th.faint())), [Lay.hit((228 - nw : U32), (h - 48 : U32), (35 + nw : U32), 48, "flag", "machines")]}
    case False{}:
      Out.none()

# Hotkey hints: while alt or ctrl is held (nui.bend's Hint) each row a
# number reaches shows it: projects by their new-thread button, threads by
# their row. The numbers come from the lists the keys count in.

def Lay.badge(+ty: Ty, +c: D.Clip, +y0: U32, +y1: U32, +k: Nat) -> List<&2, R.Op>:
  +s = Nat.show(k)
  +y = {(y0 + (((y1 - y0 : U32) - 20 : U32) / 2 : U32) : U32) : U32}
  List.append(&2, R.Op, [D.Draw.rect(c, 230, y, 22, 20, T.Th.accent(), 255)],
    Ty.text(ty, 2n, s, c, (241 - (Ty.width(ty, 2n, s) / 2 : U32) : U32), y, 20, T.Th.bg()))

def Lay.badge.k(on: Bool, +ty: Ty, +c: D.Clip, +y0: U32, +y1: U32, +k: Nat) -> List<&2, R.Op>:
  match on:
    case True{}:
      Lay.badge(ty, c, y0, y1, k)
    case False{}:
      Nil{}

def Lay.badge.at(+a: String, +act: String, +ty: Ty, +c: D.Clip, +y0: U32, +y1: U32, +k: Nat) -> List<&2, R.Op>:
  Lay.badge.k(Bool.and(String.eq(a, act), Bool.and(Nat.is_gt(k, 0n), Nat.is_le(k, 9n))), ty, c, y0, y1, k)

def Lay.badges(+ty: Ty, +c: D.Clip, hs: List<&2, S.Hit>, +act: String, +ids: List<&2, String>) -> List<&2, R.Op>:
  match hs:
    case Nil{}:
      Nil{}
    case S.Hit{_, +y0, _, +y1, +a, +v} <> t:
      List.append(&2, R.Op, Lay.badge.at(a, act, ty, c, y0, y1, U.Side.num(ids, v, 0n)), Lay.badges(ty, c, t, act, ids))

def Lay.hints.o(o: Out, +ty: Ty, +c: D.Clip, +act: String, +ids: List<&2, String>) -> Out:
  match o:
    case Out{ops, +hs}:
      Out{List.append(&2, R.Op, ops, Lay.badges(ty, c, hs, act, ids)), hs}

def Lay.hints.k(on: Bool, o: Out, +ty: Ty, +c: D.Clip, +alt: Bool, +ids: List<&2, String>) -> Out:
  match on:
    case True{}:
      Lay.hints.o(o, ty, c, Bool.pick(String, alt, "new-thread", "select"), ids)
    case False{}:
      o

def Lay.hints(+hint: String, o: Out, +ty: Ty, +c: D.Clip, +ids: List<&2, String>) -> Out:
  Lay.hints.k(Bool.not(String.is_empty(hint)), o, ty, c, String.eq(hint, "alt"), ids)

# the list a held key counts in
def Lay.hint.ids(alt: Bool, ctrl: Bool, +u: U.Ui) -> List<&2, String>:
  match alt ctrl:
    case True{} _:
      U.Side.projects(u)
    case False{} True{}:
      U.Side.here(u)
    case False{} False{}:
      Nil{}

# the sidebar's search, at the top of its list: projects by name, or in
# the active view threads by title or project (the query holds while it
# has text: core's Find.q)
def Lay.finding(+ty: Ty, +active: Bool, +y: U32, +q: String, +focus: String, +caret: Nat, +mark: Nat) -> Out:
  Lay.field.x(ty, D.Clip.all(), 12, y, 240, q, String.eq(focus, S.Fld.id("pq", "proj-go", "")), True{}, True{}, caret, mark, Bool.pick(String, active, MSG.T.palette.search(Ty.loc(ty)), MSG.T.sidebar.search.projects(Ty.loc(ty))), S.Fld.id("pq", "proj-go", ""))

# the sidebar's rows, and how far they may scroll (nui.bend's Side.most):
# until the end of the list, scrolled up by sd, meets the bottom
def Side.most(c: Col, +sd: U32, +bottom: U32) -> Out:
  match c:
    case Col{o, +y}:
      Out.cat(o, Out{Nil{}, [Lay.hit(0, 0, 0, 0, "side-most", U32.show(U32.dim((y + sd + 8 : U32), bottom)))]})

def Lay.sidebar.k.m(~PC: Data, ~paint: PC -> ST.SideRowReq -> ST.SideRowStep, +ty: Ty, +c: D.Clip, +st: M.State, +ix: U.SIx, +sel: String, +open: Map<&2, Bool>, on: Bool, +now: Nat, +adding: Bool, +v: String,
    +sd: U32, +caret: Nat, +mark: Nat, +focus: String, +path: String, +h: U32, +settings: Bool, +ms: List<&2, U.MRow>, +hint: String, +ids: List<&2, String>, +look: String, +rm: String, +q: String, utop: Out, +uh: U32, more: D.Clip -> U32 -> Col, +pc: PC) -> SideCol:
  +top = {D.Clip{0, 0, 264, 52} : D.Clip}
  +many = Nat.is_gt(List.length(&2, U.MRow, ms), 1n)
  +active = String.eq(U.Side.view(st), "active")
  +yf = {(Bool.pick(U32, adding, 96, 60) + uh : U32) : U32}
  +y0 = {(yf + 36 : U32) : U32}
  +body = SideCol.then(Lay.side.list.k.m(~PC, ~paint, active, ty, q, open, ix, st, sel, now, look, rm,
    D.Clip{0, y0, 264, (h - 48 : U32)}, U32.sub(y0, sd), pc), y => SideCol.lift(more(D.Clip{0, y0, 264, (h - 48 : U32)}, y)))
  SideCol.layer(body, col => Out.all([
    Out.ops([D.Draw.rect(D.Clip.all(), 0, 0, 264, h, T.Th.panel(), 255), D.Draw.rect(D.Clip.all(), 263, 0, 1, h, T.Th.line(), 255),
      D.Draw.rect(top, 0, 51, 264, 1, T.Th.line(), 255),
      D.Draw.disc(top, 19.0, 26.0, 3.5, Bool.pick(U32, on, T.Th.ok(), T.Th.bad()), 255)]),
    Out.ops(List.append(&2, R.Op, Ty.text(ty, 1n, "Backplane", top, 30, 0, 52, T.Th.text()),
      Ty.text(ty, 2n, Ty.fit(ty, 2n, "v" ++ v, (158 - U32.min(Ty.width(ty, 1n, "Backplane"), 158) : U32)), top, (30 + Ty.width(ty, 1n, "Backplane") + 8 : U32), 0, 52, T.Th.faint()))),
    Out{Lay.icon(ty, "plus", top, 234, 18, T.Th.dim()), [Lay.hit(228, 12, 28, 28, "vw-projo", "")]},
    utop,
    Lay.adding(ty, adding, uh, path, String.eq(focus, "project-path"), caret, mark),
    Links.over(Lay.hints(hint, Side.most(col, sd, (h - 48 : U32)), ty, D.Clip{0, y0, 264, (h - 48 : U32)}, ids), U.Links.recent(st, now), now, D.Clip{0, y0, 264, (h - 48 : U32)}),
    Lay.finding(ty, active, yf, q, focus, caret, mark),
    Out{List.append(&2, R.Op, [D.Draw.rect(D.Clip.all(), 0, (h - 48 : U32), 263, 48, T.Th.panel(), 255), D.Draw.rect(D.Clip.all(), 0, (h - 48 : U32), 263, 1, T.Th.line(), 255)],
        List.append(&2, R.Op, Lay.icon(ty, "settings", D.Clip.all(), 16, (h - 32 : U32), Bool.pick(U32, settings, T.Th.text(), T.Th.dim())), Ty.text(ty, 2n, MSG.T.palette.settings(Ty.loc(ty)), D.Clip.all(), 40, (h - 48 : U32), 48, Bool.pick(U32, settings, T.Th.text(), T.Th.dim())))),
      [Lay.hit(0, (h - 48 : U32), 120, 48, "flag", "settings")]},
    Mach.bar.k(many, ty, ms, h),
    Mach.menu(ty, Bool.and(many, U.Flag.get(open, "machines")), ms, h)]))

def Lay.sidebar.k(+ty: Ty, +c: D.Clip, +st: M.State, +ix: U.SIx, +sel: String, +open: Map<&2, Bool>, on: Bool, +now: Nat, +adding: Bool, +v: String,
    +sd: U32, +caret: Nat, +mark: Nat, +focus: String, +path: String, +h: U32, +settings: Bool, +ms: List<&2, U.MRow>, +hint: String, +ids: List<&2, String>, +look: String, +rm: String, +q: String, utop: Out, +uh: U32, more: D.Clip -> U32 -> Col) -> Out:
  SideCol.out(Lay.sidebar.k.m(~(List<&2, SideFreshCtx>), ~Lay.fresh.run, ty, c, st, ix, sel, open, on, now, adding, v, sd, caret, mark, focus, path, h, settings, ms, hint, ids, look, rm, q, utop, uh, more, Lay.fresh(ty, ix, st, sel, now)))

# Timeline
# --------

# one laid-out line. kind: 0 text, 1 code, 2 user, 3 activity, 4 gap,
# 5 heading, 6 user padding, 7 footer with a Copy button, 8 code header,
# 9 code bottom, 10 a message sent to another thread, 11 the thread a
# message came from (copy holds the other thread), 12 an image row, 13 a
# line with links, 14 a run of tool calls folded into one row (copy holds
# what a click asks, FO.Fold.ask; w is 1 when the run is open), 28 the time
# under a user message (a reply's is its footer's text). w is a user
# bubble's width. Kind 29 is a zero-height entry boundary for anchoring.
type TLine is Data:
  TLine{text: String, color: U32, kind: U32, copy: String, w: U32}

def TL.h.k(k: Nat) -> U32:
  match k:
    case 0n:
      23
    case 1n:
      21
    case 2n:
      23
    case 3n:
      22
    case 4n:
      14
    case 5n:
      28
    case 6n:
      10
    case 7n:
      22
    case 8n:
      32
    case 10n:
      24
    case 11n:
      22
    case 12n:
      20
    case 13n:
      23
    case 14n:
      24
    case 15n:
      22
    case 16n:
      24
    case 17n:
      8
    case 18n:
      26
    case 19n:
      23
    case 20n:
      1
    case 21n:
      17
    case 22n:
      23
    case 23n:
      18
    case 24n:
      26
    case 25n:
      26
    case 26n:
      26
    case 27n:
      24
    case 28n:
      20
    case 29n:
      0
    case _:
      10

def TL.h(+kind: U32) -> U32:
  TL.h.k(U32.to_nat(kind))

# the text of a node list, markup dropped
def Nd.plain(ns: List<&2, N.Node>) -> String:
  match ns:
    case Nil{}:
      ""
    case N.Txt{s} <> t:
      s ++ Nd.plain(t)
    case N.El{"button", _, _, _} <> t:
      Nd.plain(t)
    case N.El{_, _, _, ks} <> t:
      Nd.plain(ks) ++ Nd.plain(t)

# the links (<a>) of a node list, in order
def Nd.href(xs: List<&2, N.Attr>) -> String:
  match xs:
    case Nil{}:
      ""
    case N.At{"href", v} <> _:
      v
    case _ <> t:
      Nd.href(t)

# the ref (data-ref) of a node's attributes, and whether it was a bare id
def Nd.ref(xs: List<&2, N.Attr>) -> String:
  match xs:
    case Nil{}:
      ""
    case N.At{"data-ref", v} <> _:
      v
    case _ <> t:
      Nd.ref(t)

def Nd.bare(xs: List<&2, N.Attr>) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case N.At{"data-bare", v} <> _:
      String.eq(v, "1")
    case _ <> t:
      Nd.bare(t)

def Nd.link(ref: String, +at: List<&2, N.Attr>, +ks: List<&2, N.Node>) -> LK.Piece:
  match ref:
    case SNil{}:
      LK.PLink{Nd.plain(ks), Nd.href(at)}
    case SCon{h, t}:
      LK.PRef{Nd.plain(ks), SCon{h, t}, Nd.bare(at)}

def Nd.links(ns: List<&2, N.Node>) -> List<&2, LK.Piece>:
  match ns:
    case Nil{}:
      Nil{}
    case N.Txt{_} <> t:
      Nd.links(t)
    case N.El{"a", _, +at, +ks} <> t:
      Nd.link(Nd.ref(at), at, ks) <> Nd.links(t)
    case N.El{_, _, _, ks} <> t:
      List.append(&2, LK.Piece, Nd.links(ks), Nd.links(t))

# Links
# -----
# A paragraph with links lays out as kind 13 lines: the paragraph's links
# ride in the line's copy field (LK.Link.enc); drawing cuts each line into
# runs (LK.Link.cut) and each link run hits "open-url" with its address, or
# "file-open" with a file's path (a right-click there opens its menu).

def TL.linked.k(+none: Bool, ls: List<&2, String>, +color: U32, +enc: String) -> List<&2, TLine>:
  match ls:
    case Nil{}:
      Nil{}
    case l <> t:
      TLine{l, color, Bool.pick(U32, none, 0, 13), Bool.pick(String, none, "", enc), 0} <> TL.linked.k(none, t, color, enc)

def TL.linked(ls: List<&2, String>, +color: U32, +enc: String) -> List<&2, TLine>:
  TL.linked.k(String.is_empty(enc), ls, color, enc)

def TL.link.w(+ty: Ty, p: LK.Piece) -> U32:
  match p:
    case LK.PText{s}:
      Ty.width(ty, 3n, s)
    case LK.PLink{l, _}:
      Ty.width(ty, 3n, l)
    case LK.PRef{l, _, _}:
      Ty.width(ty, 3n, l)

def TL.link.seg(+ty: Ty, p: LK.Piece, +c: D.Clip, +x: U32, +y: U32, +color: U32) -> Out:
  match p:
    case LK.PText{s}:
      Out.ops(Ty.text(ty, 3n, s, c, x, y, 23, color))
    case LK.PRef{l, _, _}:
      Out.ops(Ty.text(ty, 3n, l, c, x, y, 23, color))
    case LK.PLink{+l, +u}:
      Out{D.Draw.rect(c, x, (y + 19 : U32), Ty.width(ty, 3n, l), 1, T.Th.accent(), 255) <> Ty.text(ty, 3n, l, c, x, y, 23, T.Th.accent()),
        [Lay.hit(x, y, Ty.width(ty, 3n, l), 23, Bool.pick(String, LK.Link.is_web(u), "open-url", "file-open"), u)]}

def TL.link.segs(ps: List<&2, LK.Piece>, +ty: Ty, +c: D.Clip, +x: U32, +y: U32, +color: U32) -> Out:
  match ps:
    case Nil{}:
      Out.none()
    case +p <> t:
      Out.cat(TL.link.seg(ty, p, c, x, y, color), TL.link.segs(t, ty, c, (x + TL.link.w(ty, p) : U32), y, color))

def TL.texts(ls: List<&2, String>, +color: U32, +kind: U32) -> List<&2, TLine>:
  match ls:
    case Nil{}:
      Nil{}
    case l <> t:
      TLine{l, color, kind, "", 0} <> TL.texts(t, color, kind)

# an item's lines: the first after its mark (p0), the rest indented (p1)
def TL.marked(ls: List<&2, String>, first: Bool, +p0: String, +p1: String, +enc: String) -> List<&2, TLine>:
  match ls:
    case Nil{}:
      Nil{}
    case l <> t:
      TLine{Bool.pick(String, first, p0, p1) ++ l, T.Th.soft(), Bool.pick(U32, String.is_empty(enc), 0, 13), enc, 0} <> TL.marked(t, False{}, p0, p1, enc)

def TL.bullets(ls: List<&2, String>, first: Bool, +enc: String) -> List<&2, TLine>:
  TL.marked(ls, first, "•  ", "   ", enc)

def TL.items(+ty: Ty, ns: List<&2, N.Node>, +px: U32) -> List<&2, TLine>:
  match ns:
    case Nil{}:
      Nil{}
    case +n <> t:
      List.append(&2, TLine, TL.bullets(Ty.wrap(ty, 3n, Nd.plain([n]), (px - 24 : U32)), True{}, LK.Link.enc(Nd.links([n]))), TL.items(ty, t, px))

# an ordered item's number as written (Md.oli's value)
def Nd.value(xs: List<&2, N.Attr>) -> String:
  match xs:
    case Nil{}:
      ""
    case N.At{"value", v} <> _:
      v
    case _ <> t:
      Nd.value(t)

def TL.num(+ty: Ty, n: N.Node, +px: U32) -> List<&2, TLine>:
  match n:
    case N.El{_, _, at, +ks}:
      TL.marked(Ty.wrap(ty, 3n, Nd.plain(ks), (px - 28 : U32)), True{}, Nd.value(at) ++ ". ", "    ", LK.Link.enc(Nd.links(ks)))
    case N.Txt{s}:
      TL.texts(Ty.wrap(ty, 3n, s, px), T.Th.soft(), 0)

def TL.nums(+ty: Ty, ns: List<&2, N.Node>, +px: U32) -> List<&2, TLine>:
  match ns:
    case Nil{}:
      Nil{}
    case n <> t:
      List.append(&2, TLine, TL.num(ty, n, px), TL.nums(ty, t, px))

# Tables
# ------
# A table lays out as kind 18 lines (a row's first line: a rule above it)
# and kind 19 lines (the rest of a wrapped row), then a kind 20 rule. A
# line's text is its cells' text joined by tabs (so a selection copies
# into a spreadsheet); its copy field is a flag (1: the head row) and then
# each column's width, one character each; w is the table's width. Each
# column is as wide as its widest cell, all scaled down to fit the panel,
# and a cell wraps within its column.

type TRow is Data:
  TRow{head: Bool, cells: List<&2, String>}

def TL.cells(ns: List<&2, N.Node>) -> List<&2, String>:
  match ns:
    case Nil{}:
      Nil{}
    case N.El{_, _, _, ks} <> t:
      Nd.plain(ks) <> TL.cells(t)
    case N.Txt{s} <> t:
      s <> TL.cells(t)

def TL.th(ns: List<&2, N.Node>) -> Bool:
  match ns:
    case N.El{"th", _, _, _} <> _:
      True{}
    case _:
      False{}

def TL.trow(n: N.Node) -> TRow:
  match n:
    case N.El{_, _, _, +ks}:
      TRow{TL.th(ks), TL.cells(ks)}
    case N.Txt{s}:
      TRow{False{}, [s]}

def TL.trows(ns: List<&2, N.Node>) -> List<&2, TRow>:
  match ns:
    case Nil{}:
      Nil{}
    case n <> t:
      TL.trow(n) <> TL.trows(t)

# column widths widened to fit a row's cells (16 px of padding each)
def TL.tw.row(cs: List<&2, String>, ws: List<&2, U32>, +ty: Ty) -> List<&2, U32>:
  match cs ws:
    case Nil{} +rest:
      rest
    case +c <> ct Nil{}:
      (Ty.width(ty, 3n, c) + 16 : U32) <> TL.tw.row(ct, Nil{}, ty)
    case +c <> ct +w <> wt:
      U32.max(w, (Ty.width(ty, 3n, c) + 16 : U32)) <> TL.tw.row(ct, wt, ty)

def TL.tw(rs: List<&2, TRow>, ws: List<&2, U32>, +ty: Ty) -> List<&2, U32>:
  match rs:
    case Nil{}:
      ws
    case TRow{_, cs} <> t:
      TL.tw(t, TL.tw.row(cs, ws, ty), ty)

# the longest word in a cell, as a width (a column never goes narrower)
def TL.word(ws: List<&2, String>, +ty: Ty, +acc: U32) -> U32:
  match ws:
    case Nil{}:
      acc
    case +w <> t:
      TL.word(t, ty, U32.max(acc, Ty.width(ty, 3n, w)))

def TL.mw.row(cs: List<&2, String>, ms: List<&2, U32>, +ty: Ty) -> List<&2, U32>:
  match cs ms:
    case Nil{} +rest:
      rest
    case +c <> ct Nil{}:
      (TL.word(String.split(c, ' '), ty, 0) + 16 : U32) <> TL.mw.row(ct, Nil{}, ty)
    case +c <> ct +m <> mt:
      U32.max(m, (TL.word(String.split(c, ' '), ty, 0) + 16 : U32)) <> TL.mw.row(ct, mt, ty)

def TL.mw(rs: List<&2, TRow>, ms: List<&2, U32>, +ty: Ty) -> List<&2, U32>:
  match rs:
    case Nil{}:
      ms
    case TRow{_, cs} <> t:
      TL.mw(t, TL.mw.row(cs, ms, ty), ty)

def TL.sum(ws: List<&2, U32>, +acc: U32) -> U32:
  match ws:
    case Nil{}:
      acc
    case w <> t:
      TL.sum(t, (acc + w : U32))

# what the columns no wider than share take
def TL.small(ws: List<&2, U32>, +share: U32, +acc: U32) -> U32:
  match ws:
    case Nil{}:
      acc
    case +w <> t:
      TL.small(t, share, Bool.pick(U32, U32.is_gt(w, share), acc, (acc + w : U32)))

# a column no wider than share keeps its width; the wider ones split the
# rest of the panel in proportion
def TL.scale(ws: List<&2, U32>, ms: List<&2, U32>, +share: U32, +rest: U32, +big: U32) -> List<&2, U32>:
  match ws ms:
    case +w <> t +m <> mt:
      Bool.pick(U32, U32.is_gt(w, share), U32.max(m, U32.max(48, ((w * rest : U32) / U32.max(big, 1) : U32))), w) <> TL.scale(t, mt, share, rest, big)
    case _ _:
      Nil{}

def TL.fit.k(+ws: List<&2, U32>, ms: List<&2, U32>, +px: U32, +total: U32, +share: U32) -> List<&2, U32>:
  TL.scale(ws, ms, share, U32.dim(px, TL.small(ws, share, 0)), U32.dim(total, TL.small(ws, share, 0)))

# the widths shrunk to the panel when they are wider
def TL.fit(+ws: List<&2, U32>, ms: List<&2, U32>, +px: U32, +total: U32) -> List<&2, U32>:
  Bool.pick(List<&2, U32>, U32.is_gt(total, px), TL.fit.k(ws, ms, px, total, (px / U32.max(U32.from_nat(List.length(&2, U32, ws)), 1) : U32)), ws)

def TL.enc(ws: List<&2, U32>) -> String:
  match ws:
    case Nil{}:
      SNil{}
    case w <> t:
      SCon{Chr{w}, TL.enc(t)}

# each cell wrapped to its column
def TL.wraps(cs: List<&2, String>, ws: List<&2, U32>, +ty: Ty) -> List<&2, List<&2, String>>:
  match cs ws:
    case +c <> ct +w <> wt:
      Ty.wrap(ty, 3n, c, (U32.dim(w, 16) : U32)) <> TL.wraps(ct, wt, ty)
    case _ _:
      Nil{}

def TL.tall(ls: List<&2, List<&2, String>>, +acc: Nat) -> Nat:
  match ls:
    case Nil{}:
      acc
    case +l <> t:
      TL.tall(t, Bool.pick(Nat, Nat.is_gt(List.length(&2, String, l), acc), List.length(&2, String, l), acc))

def TL.nth(ls: List<&2, List<&2, String>>, +k: Nat) -> List<&2, String>:
  match ls:
    case Nil{}:
      Nil{}
    case l <> t:
      Maybe.default(&2, String, List.get(&2, String, l, k), "") <> TL.nth(t, k)

def TL.rowlines(n: Nat, +k: Nat, +ls: List<&2, List<&2, String>>, +color: U32, +enc: String, +w: U32) -> List<&2, TLine>:
  match n:
    case 0n:
      Nil{}
    case 1n+m:
      TLine{String.join(TL.nth(ls, k), "\t"), color, Bool.pick(U32, Nat.is_eq(k, 0n), 18, 19), enc, w} <> TL.rowlines(m, 1n+k, ls, color, enc, w)

def TL.trow.lines.k(+ls: List<&2, List<&2, String>>, +head: Bool, +ws: List<&2, U32>, +w: U32) -> List<&2, TLine>:
  TL.rowlines(TL.tall(ls, 1n), 0n, ls, Bool.pick(U32, head, T.Th.text(), T.Th.soft()), SCon{Chr{Bool.pick(U32, head, 1, 0)}, TL.enc(ws)}, w)

def TL.trow.lines(r: TRow, +ws: List<&2, U32>, +w: U32, +ty: Ty) -> List<&2, TLine>:
  match r:
    case TRow{+head, cs}:
      TL.trow.lines.k(TL.wraps(cs, ws, ty), head, ws, w)

def TL.table.rows(rs: List<&2, TRow>, +ws: List<&2, U32>, +w: U32, +ty: Ty) -> List<&2, TLine>:
  match rs:
    case Nil{}:
      [TLine{"", T.Th.line(), 20, "", w}]
    case r <> t:
      List.append(&2, TLine, TL.trow.lines(r, ws, w, ty), TL.table.rows(t, ws, w, ty))

def TL.table.k(+rs: List<&2, TRow>, +ws: List<&2, U32>, +ty: Ty) -> List<&2, TLine>:
  TL.table.rows(rs, ws, TL.sum(ws, 0), ty)

def TL.table(+ty: Ty, ns: List<&2, N.Node>, +px: U32) -> List<&2, TLine>:
  +rs = TL.trows(ns)
  +ws = TL.tw(rs, Nil{}, ty)
  TL.table.k(rs, TL.fit(ws, TL.mw(rs, Nil{}, ty), U32.dim(px, 1), TL.sum(ws, 0)), ty)

# a table line's cells, each in its column, with a rule left of each
def TL.cells.draw(cs: List<&2, String>, ws: String, +ty: Ty, +c: D.Clip, +x: U32, +y: U32, +h: U32, +color: U32) -> List<&2, R.Op>:
  match cs ws:
    case +s <> ct SCon{Chr{+w}, wt}:
      List.append(&2, R.Op, D.Draw.rect(c, x, y, 1, h, T.Th.line(), 255) <> Ty.text(ty, 3n, Ty.fit(ty, 3n, s, U32.dim(w, 16)), c, (x + 8 : U32), (y + h - 23 : U32), 23, color),
        TL.cells.draw(ct, wt, ty, c, (x + w : U32), y, h, color))
    case _ _:
      [D.Draw.rect(c, x, y, 1, h, T.Th.line(), 255)]

def TL.trow.draw(+ty: Ty, +first: Bool, +text: String, +color: U32, +enc: String, +bw: U32, +c: D.Clip, +x0: U32, +y: U32) -> Out:
  match enc:
    case SCon{Chr{+flag}, ws}:
      Out.ops(List.concat(&2, R.Op, [Bool.pick(List<&2, R.Op>, U32.is_eq(flag, 1), [D.Draw.rect(c, x0, y, bw, Bool.pick(U32, first, 26, 23), T.Th.raise(), 255)], Nil{}),
        Bool.pick(List<&2, R.Op>, first, [D.Draw.rect(c, x0, y, bw, 1, T.Th.line(), 255)], Nil{}),
        TL.cells.draw(String.split(text, '\t'), ws, ty, c, x0, y, Bool.pick(U32, first, 26, 23), color)]))
    case SNil{}:
      Out.none()

def TL.code(ls: List<&2, String>) -> List<&2, TLine>:
  match ls:
    case Nil{}:
      Nil{}
    case l <> t:
      TLine{l, T.Th.text(), 1, "", 0} <> TL.code(t)

def TL.trim.k(blank: Bool, +ls: List<&2, String>) -> List<&2, String>:
  match blank:
    case True{}:
      List.take(&2, String, ls, Nat.sub(List.length(&2, String, ls), 1n))
    case False{}:
      ls

# a code block's text without its trailing blank line
def TL.trim(+ls: List<&2, String>) -> List<&2, String>:
  TL.trim.k(String.is_empty(Maybe.default(&2, String, List.last(&2, String, ls), "x")), ls)

def TL.block(+ty: Ty, n: N.Node, +px: U32) -> List<&2, TLine>:
  match n:
    case N.El{"p", _, _, +ks}:
      TL.linked(Ty.wrap(ty, 3n, Nd.plain(ks), px), T.Th.soft(), LK.Link.enc(Nd.links(ks)))
    case N.El{"ul", _, _, ks}:
      TL.items(ty, ks, px)
    case N.El{"ol", _, _, ks}:
      TL.nums(ty, ks, px)
    case N.El{"table", _, _, ks}:
      TL.table(ty, ks, px)
    case N.El{"blockquote", _, _, ks}:
      TL.texts(Ty.wrap(ty, 3n, Nd.plain(ks), (px - 12 : U32)), T.Th.faint(), 22)
    case N.El{"hr", _, _, _}:
      [TLine{"", T.Th.line(), 21, "", 0}]
    case N.El{"div", _, _, +ks}:
      List.concat(&2, TLine, [[TLine{"code", T.Th.faint(), 8, Nd.plain(ks), 0}], TL.code(TL.trim(String.split(Nd.plain(ks), '\n'))), [TLine{"", 0, 9, "", 0}]])
    case N.El{_, _, _, ks}:
      TL.texts(Ty.wrap(ty, 1n, Nd.plain(ks), px), T.Th.text(), 5)
    case N.Txt{s}:
      TL.texts(Ty.wrap(ty, 3n, s, px), T.Th.soft(), 0)

# markdown blocks as lines, a gap between blocks
def TL.blocks(+ty: Ty, ns: List<&2, N.Node>, +px: U32) -> List<&2, TLine>:
  match ns:
    case Nil{}:
      Nil{}
    case n <> Nil{}:
      TL.block(ty, n, px)
    case n <> t:
      List.append(&2, TLine, TL.block(ty, n, px), TLine{"", 0, 4, "", 0} <> TL.blocks(ty, t, px))

def TL.widest(+ty: Ty, ls: List<&2, String>, +acc: U32) -> U32:
  match ls:
    case Nil{}:
      acc
    case l <> t:
      TL.widest(ty, t, U32.max(acc, Ty.width(ty, 3n, l)))

def TL.user(ls: List<&2, String>, +copy: String, first: Bool, +w: U32) -> List<&2, TLine>:
  match ls:
    case Nil{}:
      Nil{}
    case l <> t:
      TLine{l, T.Th.text(), 2, Bool.pick(String, first, copy, ""), w} <> TL.user(t, copy, False{}, w)

def TL.bubble(+ty: Ty, +ls: List<&2, String>, +text: String) -> List<&2, TLine>:
  +w = {(TL.widest(ty, ls, 0) + 40 : U32) : U32}
  List.concat(&2, TLine, [[TLine{"", 0, 6, "", w}], TL.user(ls, text, True{}, w), [TLine{"", 0, 6, "", w}, TLine{"", 0, 4, "", 0}]])

def TL.first_line(+text: String) -> String:
  Maybe.default(&2, String, List.head(&2, String, String.split(text, '\n')), "")

def TL.from.k(none: Bool, +from: String, +st: M.State, +px: U32, +ty: Ty) -> List<&2, TLine>:
  match none:
    case True{}:
      Nil{}
    case False{}:
      [TLine{Ty.fit(ty, 2n, MSG.T.thread.from(Ty.loc(ty), U.Tasks.title(st, from)), px), T.Th.accent(), 11, from, 0}]

def TL.id_of(e: M.Entry) -> String:
  match e:
    case M.Msg{i, _, _, _, _}:
      i
    case M.Act{i, _, _, _, _, _}:
      i

def TL.mine.k(same: Bool, e: M.Entry, rest: List<&2, M.Entry>) -> List<&2, M.Entry>:
  match same:
    case True{}:
      e <> rest
    case False{}:
      rest

# an entry of the thread, or one it sent (asked only when not its own)
def TL.mine.or(own: Bool, +e: M.Entry, +th: String) -> Bool:
  match own:
    case True{}:
      True{}
    case False{}:
      String.eq(M.Link.from(TL.id_of(e)), th)

def TL.thread_of(e: M.Entry) -> String:
  match e:
    case M.Msg{_, t, _, _, _}:
      t
    case M.Act{_, t, _, _, _, _}:
      t

# a thread's entries and the messages it sent to other threads, oldest first
def TL.mine(es: List<&2, M.Entry>, +th: String, acc: List<&2, M.Entry>) -> List<&2, M.Entry>:
  match es:
    case Nil{}:
      acc
    case +e <> t:
      TL.mine(t, th, TL.mine.k(TL.mine.or(String.eq(TL.thread_of(e), th), e, th), e, acc))

# the newest `left` of a thread's entries (and the messages it sent),
# oldest first: the log is newest first, so the walk stops once it has
# them instead of going through the whole log each frame
type NW is Data:
  NW{left: Nat, acc: List<&2, M.Entry>}

def TL.newest.step(own: Bool, e: M.Entry, st: NW) -> NW:
  match own st:
    case True{} NW{0n, acc}:
      NW{0n, acc}
    case True{} NW{1n+k, acc}:
      NW{k, e <> acc}
    case False{} NW{l, acc}:
      NW{l, acc}

def TL.at_of(e: M.Entry) -> Nat:
  match e:
    case M.Msg{_, _, _, _, at}:
      at
    case M.Act{_, _, _, _, _, at}:
      at

# an entry from before the thread began (with an hour to spare for the
# clock) ends the walk: the log is in time order, and nothing older is
# the thread's (a thread with few entries no longer walks the whole log)
def TL.newest.cut(old: Bool, st: NW) -> NW:
  match old st:
    case True{} NW{_, acc}:
      NW{0n, acc}
    case False{} _:
      st

def TL.newest(es: List<&2, M.Entry>, +th: String, +since: Nat, st: NW) -> List<&2, M.Entry>:
  match es st:
    case +_ NW{0n, acc}:
      acc
    case Nil{} NW{_, acc}:
      acc
    case +e <> t NW{1n+k, acc}:
      TL.newest(t, th, since, TL.newest.cut(Nat.is_lt(Nat.add(TL.at_of(e), 3600n), since), TL.newest.step(TL.mine.or(String.eq(TL.thread_of(e), th), e, th), e, NW{1n+k, acc})))

def TL.height(ls: List<&2, TLine>, +acc: U32) -> U32:
  match ls:
    case Nil{}:
      acc
    case TLine{_, _, +k, _, _} <> t:
      TL.height(t, (acc + TL.h(k) : U32))

def TL.copy(+ty: Ty, +c: D.Clip, +x1: U32, +y: U32, +lh: U32, +copy: String) -> Out:
  +label0 = MSG.T.common.copy(Ty.loc(ty))
  Bool.pick(Out, String.is_empty(copy), Out.none(),
    Out{Ty.text(ty, 2n, label0, c, (x1 - Ty.width(ty, 2n, label0) - 12 : U32), y, lh, T.Th.faint()), [Lay.hit((x1 - 56 : U32), y, 56, lh, "copy", copy)]})

def TL.sides(+c: D.Clip, +x0: U32, +x1: U32, +y: U32, +h: U32) -> List<&2, R.Op>:
  [D.Draw.rect(c, x0, y, 1, h, T.Th.line(), 255), D.Draw.rect(c, (x1 - 1 : U32), y, 1, h, T.Th.line(), 255)]

# Images
# ------
# An image a reply names takes whole 20 px lines. Each of its lines hits
# the image's full rectangle ("img"), so the window draws the image (under
# the UI, from the frame's base) whenever any part of it is in view. The
# window notes each image's state in the client info as "img:<path>":
# "<w>x<h>" once decoded (its size, shrunk to fit), "err:<why>", or nothing
# while it loads.

def TL.img.rows(k: Nat, +i: Nat, +size: String, +path: String) -> List<&2, TLine>:
  match k:
    case 0n:
      Nil{}
    case 1n+j:
      TLine{size, 0, 12, path, U32.from_nat(i)} <> TL.img.rows(j, 1n+i, size, path)

def TL.img.h(+size: String) -> Nat:
  Maybe.default(&2, Nat, Nat.read(Maybe.default(&2, String, List.get(&2, String, String.split(size, 'x'), 1n), "")), 0n)

# a notice the window made (core/notice.bend) in the frame's language;
# a hub's or a tool's own words as they came
def Lay.notice(+l: I.Loc, +s: String) -> String:
  NT.Notice.shown(l, s)

def TL.img.k(state: Nat, +ty: Ty, +st: String, +path: String, +px: U32) -> List<&2, TLine>:
  match state:
    case 0n:
      [TLine{MSG.T.image.loading.path(Ty.loc(ty), Ty.fit(ty, 2n, path, (px - 80 : U32))), T.Th.faint(), 12, path, 4096}]
    case 1n:
      [TLine{Ty.fit(ty, 2n, MSG.T.image.failed.path(Ty.loc(ty), path, Lay.notice(Ty.loc(ty), String.drop(st, 4n))), px), T.Th.dim(), 3, "", 0}]
    case _:
      TL.img.rows(Nat.div(Nat.add(TL.img.h(st), 27n), 20n), 0n, st, path)

def TL.img(+ty: Ty, +st: String, +path: String, +px: U32) -> List<&2, TLine>:
  TL.img.k(Bool.pick(Nat, String.is_empty(st), 0n, Bool.pick(Nat, String.starts_with(st, "err:"), 1n, 2n)), ty, st, path, px)

def TL.images(+ty: Ty, ps: List<&2, String>, +pics: Map<&2, String>, +px: U32) -> List<&2, TLine>:
  match ps:
    case Nil{}:
      Nil{}
    case +p <> t:
      List.append(&2, TLine, TL.img(ty, U.Str.get(pics, "img:" ++ p), p, px), TL.images(ty, t, pics, px))

def TL.img.size(+size: String, +i: Nat) -> U32:
  U32.from_nat(Maybe.default(&2, Nat, Nat.read(Maybe.default(&2, String, List.get(&2, String, String.split(size, 'x'), i), "")), 0n))

# a line of an image: row k of it (4096: still loading, a note)
def TL.img.draw(loading: Bool, +ty: Ty, +text: String, +color: U32, +path: String, +k: U32, +c: D.Clip, +x0: U32, +y: U32) -> Out:
  match loading:
    case True{}:
      Out{Ty.text(ty, 2n, text, c, x0, y, 20, color), [Lay.hit(x0, y, 1, 20, "img", path)]}
    case False{}:
      Bool.pick(Out, U32.is_lt(y, (k * 20 + 8 : U32)), Out.none(),
        Out{Nil{}, [Lay.hit(x0, (y - k * 20 + 4 : U32), TL.img.size(text, 0n), TL.img.size(text, 1n), "img", path)]})

# Terminals in the chat (core/emb.bend)
# -------------------------------------
# A live one is a head line (kind 24: its title and pin, the copy its key,
# bw its columns) and a line per row (kind 23: text its columns, color its
# rows, the copy its key, bw the row). Each row line paints its strip of
# the terminal's background and hits the whole screen (focus "emb:<key>"), so
# Ew.over draws the screen over the thread wherever any row is in view and
# a click gives it the keys. An inactive one is one row (25) a click
# starts again; a pinned one leaves a row (26) and is drawn at the top.

def Ew.cw(+ty: Ty) -> U32:
  U32.max(Ty.width(ty, 4n, "0"), 1)

def Ew.w(+ty: Ty, +cols: U32, +x0: U32, +x1: U32) -> U32:
  U32.min((cols * Ew.cw(ty) + 24 : U32), (x1 - x0 : U32))

def Ew.row(+ty: Ty, +text: String, +rows: U32, +key: String, +i: U32, +c: D.Clip, +x0: U32, +x1: U32, +y: U32) -> Out:
  +w = Ew.w(ty, EM.Emb.num(text), x0, x1)
  Out{[D.Draw.rect(c, x0, y, w, 18, VT.Vt.bg(), 255)], [Lay.hit(x0, (y - i * 18 : U32), w, (rows * 18 : U32), "focus", EM.Emb.focus(key))]}

def Ew.head(+ty: Ty, +text: String, +color: U32, +key: String, +cols: U32, +c: D.Clip, +x0: U32, +x1: U32, +y: U32) -> Out:
  +w = Ew.w(ty, cols, x0, x1)
  Out{List.concat(&2, R.Op, [Lay.icon(ty, "terminal", c, x0, (y + 5 : U32), T.Th.faint()), Ty.text(ty, 2n, text, c, (x0 + 22 : U32), y, 26, color),
      Lay.icon(ty, "pin", c, (x0 + w - 20 : U32), (y + 5 : U32), T.Th.faint())]),
    [Lay.hit((x0 + w - 26 : U32), y, 26, 26, "emb-pin", key)]}

def TL.draw.k(+ty: Ty, k: Nat, +text: String, +color: U32, +copy: String, +bw: U32, +c: D.Clip, +x0: U32, +x1: U32, +y: U32) -> Out:
  match k:
    case 1n:
      Out.ops(List.concat(&2, R.Op, [[D.Draw.rect(c, x0, y, (x1 - x0 : U32), 21, T.Th.code(), 255)], TL.sides(c, x0, x1, y, 21),
        Ty.text(ty, 4n, text, c, (x0 + 14 : U32), y, 21, color)]))
    case 8n:
      Out.cat(Out.ops(List.concat(&2, R.Op, [[D.Draw.rect(c, x0, y, (x1 - x0 : U32), 32, T.Th.code(), 255), D.Draw.rect(c, x0, y, (x1 - x0 : U32), 1, T.Th.line(), 255),
          D.Draw.rect(c, x0, (y + 31 : U32), (x1 - x0 : U32), 1, T.Th.line(), 255)], TL.sides(c, x0, x1, y, 32),
          Ty.text(ty, 2n, text, c, (x0 + 14 : U32), y, 32, color)])),
        TL.copy(ty, c, x1, y, 32, copy))
    case 9n:
      Out.ops(List.concat(&2, R.Op, [[D.Draw.rect(c, x0, y, (x1 - x0 : U32), 9, T.Th.code(), 255), D.Draw.rect(c, x0, (y + 9 : U32), (x1 - x0 : U32), 1, T.Th.line(), 255)],
        TL.sides(c, x0, x1, y, 10)]))
    case 2n:
      Out.cat(Out.ops(D.Draw.rect(c, (x1 - bw : U32), y, bw, 23, T.Th.raise(), 255) <> Ty.text(ty, 3n, text, c, (x1 - bw + 20 : U32), y, 23, color)),
        Bool.pick(Out, String.is_empty(copy), Out.none(), Out{Nil{}, [Lay.hit((x1 - bw : U32), y, bw, 23, "copy", copy)]}))
    case 6n:
      Out.ops([D.Draw.rect(c, (x1 - bw : U32), y, bw, 10, T.Th.raise(), 255)])
    case 7n:
      Out.cat(Out.ops(Ty.text(ty, 2n, text, c, x0, y, 22, T.Th.faint())), TL.copy(ty, c, x1, y, 22, copy))
    case 29n:
      Out.none()
    case 28n:
      Out.ops(Ty.text(ty, 2n, text, c, (x1 - Ty.width(ty, 2n, text) - 4 : U32), y, 20, T.Th.faint()))
    case 4n:
      Out.none()
    case 3n:
      Out.ops(Ty.text(ty, 2n, text, c, x0, y, 22, color))
    case 10n:
      Out{List.append(&2, R.Op, [D.Draw.rect(c, x0, (y + 4 : U32), 2, 16, color, 255)], Ty.text(ty, 2n, text, c, (x0 + 10 : U32), y, 24, color)),
        [Lay.hit(x0, y, (Ty.width(ty, 2n, text) + 12 : U32), 24, "select", copy)]}
    case 11n:
      Out{Ty.text(ty, 2n, text, c, (x1 - Ty.width(ty, 2n, text) : U32), y, 22, color), [Lay.hit((x1 - Ty.width(ty, 2n, text) : U32), y, Ty.width(ty, 2n, text), 22, "select", copy)]}
    case 13n:
      TL.link.segs(LK.Link.cut(text, LK.Link.dec(copy)), ty, c, x0, y, color)
    case 12n:
      TL.img.draw(U32.is_eq(bw, 4096), ty, text, color, copy, bw, c, x0, y)
    case 14n:
      Out{List.append(&2, R.Op, Lay.icon(ty, Bool.pick(String, U32.is_eq(bw, 1), "chevron-down", "chevron-right"), c, x0, (y + 4 : U32), color),
          Ty.text(ty, 2n, text, c, (x0 + 22 : U32), y, 24, color)),
        [Lay.hit(x0, y, (Ty.width(ty, 2n, text) + 26 : U32), 24, "fold", copy)]}
    case 5n:
      Out.ops(Ty.text(ty, 1n, text, c, x0, y, 28, color))
    case 15n:
      Out.ops(D.Draw.disc(c, U32.to_f32((x0 + 4 : U32)), U32.to_f32((y + 11 : U32)), 3.0, color, 255) <> Ty.text(ty, 2n, text, c, (x0 + 14 : U32), y, 22, T.Th.faint()))
    case 16n:
      Out{List.append(&2, R.Op, Lay.icon(ty, "history", c, x0, (y + 4 : U32), color), Ty.text(ty, 2n, text, c, (x0 + 22 : U32), y, 24, color)),
        [Lay.hit(x0, y, (Ty.width(ty, 2n, text) + 26 : U32), 24, "hist-show", copy)]}
    case 17n:
      Out.ops([D.Draw.rect(c, x0, (y + 3 : U32), (x1 - x0 : U32), 2, T.Th.accent(), 255)])
    case 18n:
      TL.trow.draw(ty, True{}, text, color, copy, bw, c, x0, y)
    case 19n:
      TL.trow.draw(ty, False{}, text, color, copy, bw, c, x0, y)
    case 20n:
      Out.ops([D.Draw.rect(c, x0, y, bw, 1, color, 255)])
    case 21n:
      Out.ops([D.Draw.rect(c, x0, (y + 8 : U32), (x1 - x0 : U32), 1, color, 255)])
    case 22n:
      Out.ops(D.Draw.rect(c, x0, y, 2, 23, T.Th.line(), 255) <> Ty.text(ty, 3n, text, c, (x0 + 12 : U32), y, 23, color))
    case 23n:
      Ew.row(ty, text, color, copy, bw, c, x0, x1, y)
    case 24n:
      Ew.head(ty, text, color, copy, bw, c, x0, x1, y)
    case 25n:
      Out{List.append(&2, R.Op, Lay.icon(ty, "play", c, x0, (y + 5 : U32), color), Ty.text(ty, 2n, text, c, (x0 + 22 : U32), y, 26, color)),
        [Lay.hit(x0, y, (Ty.width(ty, 2n, text) + 26 : U32), 26, "emb-restart", copy)]}
    case 26n:
      Out{List.append(&2, R.Op, Lay.icon(ty, "pin", c, x0, (y + 5 : U32), color), Ty.text(ty, 2n, text, c, (x0 + 22 : U32), y, 26, color)),
        [Lay.hit(x0, y, (Ty.width(ty, 2n, text) + 26 : U32), 26, "emb-pin", copy)]}
    case 27n:
      Out.ops(List.append(&2, R.Op, Lay.icon(ty, "bell", c, x0, (y + 4 : U32), color), Ty.text(ty, 2n, text, c, (x0 + 22 : U32), y, 24, color)))
    case _:
      Out.ops(Ty.text(ty, 3n, text, c, x0, y, 23, color))

def TL.draw(+ty: Ty, l: TLine, +c: D.Clip, +x0: U32, +x1: U32, +y: U32) -> Out:
  match l:
    case TLine{text, color, +kind, +copy, +w}:
      TL.draw.k(ty, U32.to_nat(kind), text, color, copy, w, c, x0, x1, y)

# whether the head line starts above y1
def TL.more(ls: List<&2, TLine>, +y: U32, +y1: U32) -> Bool:
  match ls:
    case Nil{}:
      False{}
    case _ <> _:
      U32.is_lt(y, y1)

# Selecting text in the timeline
# ------------------------------
# A drag over the thread's lines selects text (nui.bend keeps where it
# began and where it is, as "@ts": thread|line,x|line,x). Here those
# points become characters: the selection is drawn over the lines it
# covers, and its text rides on a hit region no pointer can reach
# ("tl-sel"), which ctrl+c copies.

# lines la..lb (their numbers in the thread's lines), from window x xa on
# la to xb on lb; `on` is false when nothing is selected
type TSel is Data:
  TSel{on: Bool, la: Nat, xa: U32, lb: Nat, xb: U32}

def TSel.none() -> TSel:
  TSel{False{}, 0n, 0, 0n, 0}

def TSel.num(+s: String, +i: Nat) -> Nat:
  Maybe.default(&2, Nat, Nat.read(Maybe.default(&2, String, List.get(&2, String, String.split(s, ','), i), "")), 0n)

# in order: the earlier point first
def TSel.put(+la: Nat, +xa: Nat, +lb: Nat, +xb: Nat) -> TSel:
  Bool.pick(TSel, Bool.or(Nat.is_lt(la, lb), Bool.and(Nat.is_eq(la, lb), Nat.is_le(xa, xb))),
    TSel{Bool.not(Bool.and(Nat.is_eq(la, lb), Nat.is_eq(xa, xb))), la, U32.from_nat(xa), lb, U32.from_nat(xb)},
    TSel{True{}, lb, U32.from_nat(xb), la, U32.from_nat(xa)})

def TSel.parts(+ps: List<&2, String>, +thread: String) -> TSel:
  Bool.pick(TSel, Bool.and(Nat.is_eq(List.length(&2, String, ps), 3n), String.eq(Maybe.default(&2, String, List.head(&2, String, ps), ""), thread)),
    TSel.put(TSel.num(Maybe.default(&2, String, List.get(&2, String, ps, 1n), ""), 0n), TSel.num(Maybe.default(&2, String, List.get(&2, String, ps, 1n), ""), 1n),
      TSel.num(Maybe.default(&2, String, List.get(&2, String, ps, 2n), ""), 0n), TSel.num(Maybe.default(&2, String, List.get(&2, String, ps, 2n), ""), 1n)),
    TSel.none())

# the selection "@ts" holds, when it is this thread's
def TSel.of(+raw: String, +thread: String) -> TSel:
  Bool.pick(TSel, String.is_empty(raw), TSel.none(), TSel.parts(String.split(raw, '|'), thread))

# which lines hold text a selection can take, the face they are drawn in
# and where that text starts (as TL.draw.k draws them)
def TL.textual(+k: Nat) -> Bool:
  Bool.not(Bool.or(Bool.or(Bool.or(Nat.is_eq(k, 4n), Nat.is_eq(k, 6n)), Bool.or(Nat.is_eq(k, 7n), Nat.is_eq(k, 9n))), Bool.or(Nat.is_eq(k, 12n), Bool.or(Nat.is_eq(k, 20n), Bool.or(Nat.is_eq(k, 21n), Bool.or(Nat.is_eq(k, 23n), Nat.is_eq(k, 29n)))))))

def TL.face(+k: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_eq(k, 1n), 4n, Bool.pick(Nat, Bool.or(Bool.or(Nat.is_eq(k, 8n), Nat.is_eq(k, 3n)), Bool.or(Bool.or(Nat.is_eq(k, 10n), Nat.is_eq(k, 11n)), Nat.is_eq(k, 15n))), 2n,
    Bool.pick(Nat, Nat.is_eq(k, 5n), 1n, 3n)))

def TL.tx(+ty: Ty, +k: Nat, +text: String, +bw: U32, +x0: U32, +x1: U32) -> U32:
  Bool.pick(U32, Bool.or(Bool.or(Nat.is_eq(k, 1n), Nat.is_eq(k, 8n)), Nat.is_eq(k, 15n)), (x0 + 14 : U32), Bool.pick(U32, Nat.is_eq(k, 2n), (x1 - bw + 20 : U32),
    Bool.pick(U32, Nat.is_eq(k, 10n), (x0 + 10 : U32), Bool.pick(U32, Nat.is_eq(k, 11n), (x1 - Ty.width(ty, 2n, text) : U32),
      Bool.pick(U32, Nat.is_eq(k, 22n), (x0 + 12 : U32), Bool.pick(U32, Bool.or(Nat.is_eq(k, 18n), Nat.is_eq(k, 19n)), (x0 + 8 : U32), x0))))))

# the characters of text before window x (text starting at tx)
def TL.col(+ty: Ty, +k: Nat, +text: String, +tx: U32, +x: U32) -> Nat:
  X.Fit.count(text, Ty.cache(ty), Ty.face(ty, TL.face(k)), (U32.dim(x, tx) * 64 + 192 : U32), 0)

# the part of line i the selection covers, as characters [a, b)
def TL.sel.a(+ty: Ty, +sel: TSel, +i: Nat, +k: Nat, +text: String, +tx: U32) -> Nat:
  match sel:
    case TSel{_, +la, +xa, _, _}:
      Bool.pick(Nat, Nat.is_eq(i, la), TL.col(ty, k, text, tx, xa), 0n)

def TL.sel.b(+ty: Ty, +sel: TSel, +i: Nat, +k: Nat, +text: String, +tx: U32) -> Nat:
  match sel:
    case TSel{_, _, _, +lb, +xb}:
      Bool.pick(Nat, Nat.is_eq(i, lb), TL.col(ty, k, text, tx, xb), String.length(text))

def TL.sel.in(+sel: TSel, +i: Nat, +k: Nat) -> Bool:
  match sel:
    case TSel{+on, +la, _, +lb, _}:
      Bool.and(Bool.and(on, TL.textual(k)), Bool.and(Nat.is_ge(i, la), Nat.is_le(i, lb)))

def TL.sel.last(+sel: TSel, +i: Nat) -> Bool:
  match sel:
    case TSel{_, _, _, +lb, _}:
      Nat.is_eq(i, lb)

def TL.sel.box(+ty: Ty, +c: D.Clip, +k: Nat, +text: String, +tx: U32, +a: Nat, +b: Nat, +more: Bool, +y: U32) -> List<&2, R.Op>:
  +x0 = {(tx + Ty.width(ty, TL.face(k), String.take(text, a)) : U32) : U32}
  +x1 = {(tx + Ty.width(ty, TL.face(k), String.take(text, b)) + Bool.pick(U32, more, 6, 0) : U32) : U32}
  Bool.pick(List<&2, R.Op>, U32.is_gt(x1, x0), [D.Draw.rect(c, x0, (y + 2 : U32), (x1 - x0 : U32), (TL.h(U32.from_nat(k)) - 4 : U32), T.Th.accent(), 70)], Nil{})

def TL.sel.draw.k(on: Bool, +ty: Ty, +sel: TSel, +i: Nat, +k: Nat, +text: String, +tx: U32, +c: D.Clip, +y: U32) -> List<&2, R.Op>:
  match on:
    case True{}:
      TL.sel.box(ty, c, k, text, tx, TL.sel.a(ty, sel, i, k, text, tx), TL.sel.b(ty, sel, i, k, text, tx), Bool.not(TL.sel.last(sel, i)), y)
    case False{}:
      Nil{}

# line i's share of the selection, drawn over it
def TL.sel.draw(+ty: Ty, +sel: TSel, +i: Nat, l: TLine, +c: D.Clip, +x0: U32, +x1: U32, +y: U32) -> Out:
  match l:
    case TLine{+text, _, +kind, _, +bw}:
      Out.ops(TL.sel.draw.k(TL.sel.in(sel, i, U32.to_nat(kind)), ty, sel, i, U32.to_nat(kind), text, TL.tx(ty, U32.to_nat(kind), text, bw, x0, x1), c, y))

def TL.sel.piece(in: Bool, +ty: Ty, +sel: TSel, +i: Nat, +k: Nat, +text: String, +tx: U32, rest: List<&2, String>) -> List<&2, String>:
  match in:
    case True{}:
      ED.Edit.slice(text, TL.sel.a(ty, sel, i, k, text, tx), TL.sel.b(ty, sel, i, k, text, tx)) <> rest
    case False{}:
      rest

# the selected text of every line it covers (in view or not)
def TL.sel.texts(ls: List<&2, TLine>, +ty: Ty, +sel: TSel, +i: Nat, +x0: U32, +x1: U32) -> List<&2, String>:
  match ls:
    case Nil{}:
      Nil{}
    case TLine{+text, _, +kind, _, +bw} <> t:
      TL.sel.piece(TL.sel.in(sel, i, U32.to_nat(kind)), ty, sel, i, U32.to_nat(kind), text, TL.tx(ty, U32.to_nat(kind), text, bw, x0, x1),
        TL.sel.texts(t, ty, sel, 1n+i, x0, x1))

def TL.sel.hit.k(on: Bool, +ty: Ty, +ls: List<&2, TLine>, +sel: TSel, +x0: U32, +x1: U32) -> Out:
  match on:
    case True{}:
      Out{Nil{}, [Lay.hit(0, 0, 0, 0, "tl-sel", String.join(TL.sel.texts(ls, ty, sel, 0n, x0, x1), "\n"))]}
    case False{}:
      Out.none()

def TSel.on(+s: TSel) -> Bool:
  match s:
    case TSel{+on, _, _, _, _}:
      on

# the selected text, for ctrl+c
def TL.sel.hit(+ty: Ty, +ls: List<&2, TLine>, +sel: TSel, +x0: U32, +x1: U32) -> Out:
  TL.sel.hit.k(TSel.on(sel), ty, ls, sel, x0, x1)

# draw the lines from y down while they start above y1; i numbers them in
# the thread's lines. Each line is a "tl-line" region (under its own
# buttons and links) where a drag selects text
def TL.draw_all(+ty: Ty, ls: List<&2, TLine>, more: Bool, +c: D.Clip, +x0: U32, +x1: U32, +y: U32, +y1: U32, +i: Nat, +sel: TSel) -> Out:
  match ls more:
    case TLine{_, _, 29, _, _} <> +rest True{}:
      TL.draw_all(ty, rest, TL.more(rest, y, y1), c, x0, x1, y, y1, 1n+i, sel)
    case TLine{+t, +co, +k, +cp, +bw} <> +rest True{}:
      Out.all([Out{Nil{}, [Lay.hit(x0, y, (x1 - x0 : U32), TL.h(k), "tl-line", Nat.show(i))]}, TL.draw(ty, TLine{t, co, k, cp, bw}, c, x0, x1, y),
        TL.sel.draw(ty, sel, i, TLine{t, co, k, cp, bw}, c, x0, x1, y),
        TL.draw_all(ty, rest, TL.more(rest, (y + TL.h(k) : U32), y1), c, x0, x1, (y + TL.h(k) : U32), y1, 1n+i, sel)])
    case _ _:
      Out.none()

def TL.phase.k(idle: Bool, +label: String, +dot: U32) -> List<&2, TLine>:
  match idle:
    case True{}:
      Nil{}
    case False{}:
      [TLine{label, dot, 15, "", 0}]

# the line under the timeline: the reply streaming in, else the phase
def TL.live.k(+ty: Ty, empty: Bool, +ph: M.Phase, text: String, +px: U32) -> List<&2, TLine>:
  match empty:
    case False{}:
      TL.blocks(ty, N.Md.render(text), px)
    case True{}:
      TL.phase.k(String.is_empty(M.Phase.label(ph)), M.Phase.label(ph), Lay.phase(ph))

# Live text, settled prefix once (laws live_prefix_exact, live_memo_exact)
# ----------------------------------------------------------------------
# A reply streams in whole each frame, and laying it all out each time is
# quadratic over the reply. Its text up to the last blank line outside a
# fence (client.bend's Live.head) lays out exactly as it does alone, so it
# is laid out once and kept in the thread's memo; only the text after it
# is laid out per frame.

# the streamed reply's settled prefix (client.bend's Live.head), laid out
# once at its width: `any` says whether it rendered to any block (a gap
# goes between it and the tail's blocks)
type LiveMemo is Data:
  LiveMemo{px: U32, text: String, any: Bool, lines: List<&2, TLine>}
  LiveLocal{tag: String, px: U32, text: String, any: Bool, lines: List<&2, TLine>}

# Untagged snapshots and the original memo API use English. The localized
# variant also supplies the containing timeline memo's layout identity.
def TL.locale.tag(+tag: String) -> String:
  Bool.pick(String, String.eq(tag, "ja"), "ja", "en")

def TL.locale(u: U.Ui) -> String:
  TL.locale.tag(U.Ui.scratch(u, "@lang.tag"))

def TL.live.tag(lm: LiveMemo) -> String:
  match lm:
    case LiveMemo{_, _, _, _}:
      "en"
    case LiveLocal{tag, _, _, _, _}:
      tag

def TL.live.local(+tag: String, lm: LiveMemo) -> LiveMemo:
  match lm:
    case LiveMemo{px, text, a, ls}:
      LiveLocal{tag, px, text, a, ls}
    case LiveLocal{_, px, text, a, ls}:
      LiveLocal{tag, px, text, a, ls}

def TL.live.none() -> LiveMemo:
  LiveMemo{0, "", False{}, Nil{}}

def TL.live.any(ns: List<&2, N.Node>) -> Bool:
  match ns:
    case Nil{}:
      False{}
    case _ <> _:
      True{}

def TL.live.fresh.k(+ty: Ty, +text: String, +ns: List<&2, N.Node>, +px: U32) -> LiveMemo:
  LiveMemo{px, text, TL.live.any(ns), TL.blocks(ty, ns, px)}

def TL.live.fresh(+ty: Ty, +text: String, +px: U32) -> LiveMemo:
  TL.live.fresh.k(ty, text, N.Md.render(text), px)

def TL.live.keep(same: Bool, +ty: Ty, lm: LiveMemo, text: String, +px: U32) -> LiveMemo:
  match same:
    case True{}:
      lm
    case False{}:
      TL.live.fresh(ty, text, px)

# the prefix's lines: the memo's when it was made for this text and width
def TL.live.get(+ty: Ty, lm: LiveMemo, +text: String, +px: U32) -> LiveMemo:
  match lm:
    case LiveMemo{+lpx, +ltext, +a, +ls}:
      TL.live.keep(Bool.and(U32.is_eq(lpx, px), Q.Eq.str(ltext, text)), ty, LiveMemo{lpx, ltext, a, ls}, text, px)
    case LiveLocal{+tag, +lpx, +ltext, +a, +ls}:
      TL.live.local(tag, TL.live.keep(Bool.and(U32.is_eq(lpx, px), Q.Eq.str(ltext, text)), ty, LiveLocal{tag, lpx, ltext, a, ls}, text, px))

def TL.live.cat.k(a: Bool, ls: List<&2, TLine>, +rest: List<&2, TLine>, more: Bool) -> List<&2, TLine>:
  match a more:
    case True{} True{}:
      List.append(&2, TLine, ls, TLine{"", 0, 4, "", 0} <> rest)
    case True{} False{}:
      ls
    case False{} _:
      rest

# the prefix's blocks, a gap, the tail's blocks (TL.blocks's own seam)
def TL.live.cat(+ty: Ty, lm: LiveMemo, +ns: List<&2, N.Node>, +px: U32) -> List<&2, TLine>:
  match lm:
    case LiveMemo{_, _, a, ls}:
      TL.live.cat.k(a, ls, TL.blocks(ty, ns, px), TL.live.any(ns))
    case LiveLocal{_, _, _, a, ls}:
      TL.live.cat.k(a, ls, TL.blocks(ty, ns, px), TL.live.any(ns))

# the reply streaming in, else the phase; the settled prefix from the memo
def TL.live.m(+ty: Ty, empty: Bool, +ph: M.Phase, +text: String, lm: LiveMemo, +px: U32) -> List<&2, TLine>:
  match empty:
    case False{}:
      TL.live.cat(ty, TL.live.get(ty, lm, U.Live.head(text), px), N.Md.render(U.Live.rest(text)), px)
    case True{}:
      TL.phase.k(String.is_empty(M.Phase.label(ph)), M.Phase.label(ph), Lay.phase(ph))

# Main area
# ---------

def Lay.running(t: M.TurnState) -> Bool:
  match t:
    case M.Running{}:
      True{}
    case _:
      False{}

# a header button's action and value, written "action|value"
def Lay.act(+a: String) -> String:
  Maybe.default(&2, String, List.head(&2, String, String.split(a, '|')), "")

def Lay.val(+a: String) -> String:
  String.join(List.drop(&2, String, String.split(a, '|'), 1n), "|")

def Lay.buttons.k(fits: Bool, +o: Out, rest: Out) -> Out:
  match fits:
    case True{}:
      Out.cat(o, rest)
    case False{}:
      Out.none()

# header buttons right to left from x, as many as fit right of lo (in a
# narrow main area the rest would slide under the sidebar, drawn where
# they cannot be clicked); answers the output
def Lay.buttons(+ty: Ty, bs: List<&2, S.Hit>, +y: U32, +x: U32, +lo: U32) -> Out:
  match bs:
    case Nil{}:
      Out.none()
    case S.Hit{_, _, _, _, +label, +action} <> t:
      Lay.buttons.k(U32.is_ge(x, (lo + Lay.button.w(ty, label) : U32)), Lay.button(ty, D.Clip.all(), (x - Lay.button.w(ty, label) : U32), y, label, T.Th.soft(), Lay.act(action), Lay.val(action)),
        Lay.buttons(ty, t, y, (x - Lay.button.w(ty, label) - 8 : U32), lo))

def Lay.buttons.w(+ty: Ty, bs: List<&2, S.Hit>) -> U32:
  match bs:
    case Nil{}:
      0
    case S.Hit{_, _, _, _, +label, _} <> t:
      (Lay.button.w(ty, label) + 8 + Lay.buttons.w(ty, t) : U32)

# Header tools (core/tools.bend): an icon, then the label unless compact;
# lit when on, red when it stops or destroys; a line between groups
def Lay.tool.w(+ty: Ty, +label: String, +full: Bool) -> U32:
  Bool.pick(U32, full, (Ty.width(ty, 2n, label) + 38 : U32), 28)

def Lay.tool.fg(on: Bool, danger: Bool) -> U32:
  match on danger:
    case _ True{}:
      T.Th.bad()
    case True{} False{}:
      T.Th.text()
    case False{} False{}:
      T.Th.soft()

def Lay.tool.label(full: Bool, +ty: Ty, +label: String, +x: U32, +y: U32, +fg: U32) -> List<&2, R.Op>:
  match full:
    case True{}:
      Ty.text(ty, 2n, label, D.Clip.all(), (x + 28 : U32), y, 28, fg)
    case False{}:
      Nil{}

def Lay.tool(+ty: Ty, +x: U32, +y: U32, +icon: String, +label: String, action: String, value: String, +on: Bool, +danger: Bool, +full: Bool) -> Out:
  +w = Lay.tool.w(ty, label, full)
  +fg = Lay.tool.fg(on, danger)
  Out{List.concat(&2, R.Op, [Lay.box(D.Clip.all(), x, y, w, 28, 6, Bool.pick(U32, on, T.Th.sel(), T.Th.raise()), Bool.pick(U32, on, T.Th.accent(), T.Th.line())),
      Lay.icon(ty, icon, D.Clip.all(), (x + 6 : U32), (y + 6 : U32), fg), Lay.tool.label(full, ty, label, x, y, fg)]),
    [Lay.hit(x, y, w, 28, action, value)]}

# the space after a tool: wider, with a line in it, before a new group
def Lay.tool.gap(gap: Bool) -> U32:
  Bool.pick(U32, gap, 17, 6)

def Lay.tool.sep(sep: Bool, +x: U32, +y: U32) -> Out:
  match sep:
    case True{}:
      Out.ops([D.Draw.rect(D.Clip.all(), (x + 8 : U32), (y + 6 : U32), 1, 16, T.Th.line(), 255)])
    case False{}:
      Out.none()

# the tools right to left from x (ts is right to left), as many as fit
# right of lo (in a narrow main area the rest would slide under the
# sidebar, drawn where they cannot be clicked); sep: the tool drawn last
# started a group, so a line goes between it and this one
def Lay.tools(+ty: Ty, ts: List<&2, TO.Tool>, +y: U32, +x: U32, +lo: U32, +full: Bool, sep: Bool) -> Out:
  match ts:
    case Nil{}:
      Out.none()
    case TO.Tool{+i, +l, _, a, v, +on, +dg, +gap} <> t:
      +w = Lay.tool.w(ty, l, full)
      Lay.buttons.k(U32.is_ge(x, (lo + w : U32)), Out.cat(Lay.tool.sep(sep, x, y), Lay.tool(ty, (x - w : U32), y, i, l, a, v, on, dg, full)),
        Lay.tools(ty, t, y, (x - w - Lay.tool.gap(gap) : U32), lo, full, gap))

def Lay.tools.w(+ty: Ty, ts: List<&2, TO.Tool>, +full: Bool) -> U32:
  match ts:
    case Nil{}:
      0
    case TO.Tool{_, +l, _, _, _, _, _, +gap} <> t:
      (Lay.tool.w(ty, l, full) + Lay.tool.gap(gap) + Lay.tools.w(ty, t, full) : U32)

def Lay.header.bs.loc(+l: I.Loc, turn: M.TurnState, override: M.Override, +sa: Nat, +ua: Nat, pinned: Bool, panel: Bool, archived: Bool, snoozed: Bool, del: Bool, forked: Bool, term: Bool) -> List<&2, TO.Tool>:
  List.reverse(&2, TO.Tool, TO.Tools.head.loc(l, TO.Head{Lay.running(turn), TO.Tools.settled(override, Nat.is_gt(sa, ua)), pinned, snoozed, archived, del, forked, term, panel},
    "vw-find", "snooze", "vw-key", "term"))

def Lay.header.bs(turn: M.TurnState, override: M.Override, +sa: Nat, +ua: Nat, pinned: Bool, panel: Bool, archived: Bool, snoozed: Bool, del: Bool, forked: Bool, term: Bool) -> List<&2, TO.Tool>:
  Lay.header.bs.loc(I.En{}, turn, override, sa, ua, pinned, panel, archived, snoozed, del, forked, term)

# the title and branch get what the buttons leave
# s fitted to px, or nothing when there is no room for a few letters
def Lay.fit_or_none(+ty: Ty, +k: Nat, +s: String, +px: U32) -> String:
  Bool.pick(String, U32.is_lt(px, 32), "", Ty.fit(ty, k, s, px))

# the thread's project badge (its icon in its colour), then the title and
# branch in what the buttons leave
# (the title keeps at least its first 160 px: buttons that would cover it
# are left out, as buttons that do not fit are)
# a thread's Boat chip after its title ("Boat · large · running"); a click
# opens Settings, where Usage says more (core/usage.bend's Chip)
def Hdr.chip(none: Bool, +ty: Ty, +chip: String, +x: U32, +hy: U32) -> Out:
  match none:
    case True{}:
      Out.none()
    case False{}:
      Out{Ty.text(ty, 2n, chip, D.Clip.all(), x, hy, 52, T.Th.accent()), [Lay.hit((x - 4 : U32), (hy + 12 : U32), (Ty.width(ty, 2n, chip) + 8 : U32), 28, "usage-open", "")]}

def Lay.header.k(+ty: Ty, +id: String, +title: String, +branch: String, +chip: String, +icon: String, +tint: U32, +bs: List<&2, TO.Tool>, +x0: U32, +x1: U32, +hy: U32) -> Out:
  +keep = {U32.min(Ty.width(ty, 0n, title), 160) : U32}
  # labels while they fit beside the title's first 160 px, else icons only
  +full = U32.is_ge(U32.dim((x1 - x0 : U32), 80), (Lay.tools.w(ty, bs, True{}) + keep : U32))
  +room = {U32.max(U32.dim(U32.dim((x1 - x0 : U32), Lay.tools.w(ty, bs, full)), 80), keep) : U32}
  +t = Ty.fit(ty, 0n, title, room)
  +tw = Ty.width(ty, 0n, t)
  +cw = {Bool.pick(U32, String.is_empty(chip), 0, (Ty.width(ty, 2n, chip) + 12 : U32)) : U32}
  Out.all([
    Out.ops([D.Draw.rect(D.Clip.all(), x0, (hy + 51 : U32), (x1 - x0 : U32), 1, T.Th.line(), 255)]),
    Out.ops(Lay.icon(ty, icon, D.Clip.all(), (x0 + 24 : U32), (hy + 18 : U32), tint)),
    Out.ops(Ty.text(ty, 0n, t, D.Clip.all(), (x0 + 48 : U32), hy, 52, T.Th.text())),
    # a click on the title renames the thread (client.bend's Ren)
    Out{Nil{}, [Lay.hit((x0 + 44 : U32), (hy + 8 : U32), (tw + 8 : U32), 36, "row-rename", id)]},
    Hdr.chip(String.is_empty(chip), ty, chip, (x0 + 60 + tw : U32), hy),
    Out.ops(Ty.text(ty, 2n, Lay.fit_or_none(ty, 2n, branch, U32.dim(room, (tw + 12 + cw : U32))), D.Clip.all(), (x0 + 60 + tw + cw : U32), hy, 52, T.Th.faint())),
    Lay.tools(ty, bs, (hy + 12 : U32), (x1 - 16 : U32), (x0 + 60 + tw + cw : U32), full, False{})])

# The title being renamed: a field over it (Enter or Save keeps it,
# Cancel leaves it), the client's @f.title
def Ren.field(+ty: Ty, +text: String, +focus: String, +blink: Bool, +caret: Nat, +mark: Nat, +x0: U32, +x1: U32, +hy: U32) -> Out:
  +label0 = MSG.T.common.save(Ty.loc(ty))
  +label1 = MSG.T.common.cancel(Ty.loc(ty))
  +fid = S.Fld.id("title", "rename-save", "")
  +sw = Lay.button.w(ty, label0)
  +cw = Lay.button.w(ty, label1)
  +fw = {U32.min(520, U32.dim(U32.shr((x1 - x0 : U32)), (sw + cw + 20 : U32))) : U32}
  Out.all([
    Out.ops([D.Draw.rect(D.Clip.all(), (x0 + 40 : U32), (hy + 4 : U32), (fw + sw + cw + 28 : U32), 44, T.Th.bg(), 255)]),
    Lay.field(ty, D.Clip.all(), (x0 + 44 : U32), (hy + 12 : U32), fw, text, String.eq(focus, fid), blink, caret, mark, MSG.T.thread.title.placeholder(Ty.loc(ty)), fid),
    Lay.button(ty, D.Clip.all(), (x0 + 52 + fw : U32), (hy + 12 : U32), label0, T.Th.accent(), "rename-save", ""),
    Lay.button(ty, D.Clip.all(), (x0 + 58 + fw + sw : U32), (hy + 12 : U32), label1, T.Th.soft(), "rename-no", "")])

def Ren.field.if(on: Bool, +ty: Ty, +text: String, +focus: String, +blink: Bool, +caret: Nat, +mark: Nat, +x0: U32, +x1: U32, +hy: U32) -> Out:
  match on:
    case True{}:
      Ren.field(ty, text, focus, blink, caret, mark, x0, x1, hy)
    case False{}:
      Out.none()

def Lay.header(+ty: Ty, t: M.Thread, +st: M.State, +chip: String, mini: List<&2, TO.Tool>, +x0: U32, +x1: U32, panel: Bool, term: Bool, +snoozed: Bool, del: Bool, +hy: U32) -> Out:
  match t:
    case M.Thread{+id, +p, +title, _, _, turn, _, +archived, override, +sa, +ua, pinned, _, +branch, _, _, _, _, _, _, _}:
      Lay.header.k(ty, id, U.Label.title(st, id, TTL.Title.shown(Ty.loc(ty), title)), branch, chip, BG.Badge.icon(st, p), BG.Badge.rgb_of(st, p),
        # (the tools run right to left: the put-away Buy it, last, is leftmost)
        List.append(&2, TO.Tool, Lay.header.bs.loc(Ty.loc(ty), turn, override, sa, ua, pinned, panel, archived, snoozed, del, Bool.not(String.is_empty(M.Fork.source(st, id))), term), mini), x0, x1, hy)

def Lay.lines(+ty: Ty, ls: List<&2, String>, +c: D.Clip, +x: U32, +y: U32) -> Out:
  match ls:
    case Nil{}:
      Out.none()
    case l <> t:
      Out.cat(Out.ops(Ty.text(ty, 3n, l, c, x, y, 23, T.Th.text())), Lay.lines(ty, t, c, x, (y + 23 : U32)))

def Picker.name.loc(+l: I.Loc, +m: String) -> String:
  Bool.pick(String, String.is_empty(m), MSG.T.common.default(l), m)

# the widest model name, and the widest label, in pixels

def Picker.name(+m: String) -> String:
  Picker.name.loc(I.En{}, m)

def Picker.names_w(+ty: Ty, ms: List<&2, String>) -> Nat:
  match ms:
    case Nil{}:
      0n
    case +m <> t:
      MN.Menu.max(U32.to_nat(Ty.width(ty, 2n, Picker.name.loc(Ty.loc(ty), m))), Picker.names_w(ty, t))

def Picker.labels_w(+ty: Ty, ms: List<&2, String>, +info: String, +prov: String) -> Nat:
  match ms:
    case Nil{}:
      0n
    case +m <> t:
      MN.Menu.max(U32.to_nat(Ty.width(ty, 2n, U.Models.label_of(info, prov, m))), Picker.labels_w(ty, t, info, prov))

# the effort chips' widths, and all of them in a line with their gaps
def Picker.chip_ws(+ty: Ty, es: List<&2, String>) -> List<&2, Nat>:
  match es:
    case Nil{}:
      Nil{}
    case +e <> t:
      U32.to_nat(Lay.button.w(ty, U.Chip.name(e))) <> Picker.chip_ws(ty, t)

def Picker.line_w(ws: List<&2, Nat>) -> Nat:
  match ws:
    case Nil{}:
      0n
    case +w <> t:
      Nat.add(Nat.add(w, 4n), Picker.line_w(t))

# reasoning effort chips, where the flow put them
def Picker.chips(+ty: Ty, bs: List<&2, MN.Box>, es: List<&2, String>, +cur: String, +prov: String) -> Out:
  match bs:
    case Nil{}:
      Out.none()
    case MN.Box{+x, +y, _, _} <> bt:
      match es:
        case Nil{}:
          Out.none()
        case +e <> et:
          Out.cat(Lay.chip(ty, Picker.px(x), Picker.px(y), U.Chip.name(e), U.Chip.on(e, cur, prov), "effort", e), Picker.chips(ty, bt, et, cur, prov))

# one model row: its name clipped to the name column, its label past it
def Picker.row(+ty: Ty, +m: String, +cur: String, +info: String, +prov: String, +x: Nat, +y: Nat, +w: Nat, +col: Nat) -> Out:
  +on = String.eq(m, cur)
  +nx = {Nat.add(x, 10n) : Nat}
  +lx = MN.Menu.label_x(nx, col)
  +lw = {Nat.sub(Nat.add(x, w), Nat.add(lx, 10n)) : Nat}
  Out{List.concat(&2, R.Op, [
      Bool.pick(List<&2, R.Op>, on, Lay.round(D.Clip.all(), Picker.px(x), Picker.px(y), Picker.px(w), 28, 5, T.Th.sel()), Nil{}),
      Ty.text(ty, 2n, Ty.fit(ty, 2n, Picker.name.loc(Ty.loc(ty), m), Picker.px(col)), D.Clip{Picker.px(nx), Picker.px(y), Picker.px(Nat.add(nx, col)), Picker.px(Nat.add(y, 28n))},
        Picker.px(nx), Picker.px(y), 28, Bool.pick(U32, on, T.Th.text(), T.Th.soft())),
      Ty.text(ty, 2n, Lay.fit_or_none(ty, 2n, U.Models.label_of(info, prov, m), Picker.px(lw)), D.Clip{Picker.px(lx), Picker.px(y), Picker.px(Nat.add(lx, lw)), Picker.px(Nat.add(y, 28n))},
        Picker.px(lx), Picker.px(y), 28, T.Th.faint())]),
    [Lay.hit(Picker.px(x), Picker.px(y), Picker.px(w), 28, "model", m)]}

def Picker.rows(+ty: Ty, bs: List<&2, MN.Box>, ms: List<&2, String>, +cur: String, +info: String, +prov: String, +col: Nat) -> Out:
  match bs:
    case Nil{}:
      Out.none()
    case MN.Box{+x, +y, +w, _} <> bt:
      match ms:
        case Nil{}:
          Out.none()
        case +m <> mt:
          Out.cat(Picker.row(ty, m, cur, info, prov, x, y, w, col), Picker.rows(ty, bt, mt, cur, info, prov, col))

def Picker.pos(xs: List<&2, String>, +x: String, +i: Nat) -> Nat:
  match xs:
    case Nil{}:
      0n
    case +h <> t:
      Bool.pick(Nat, String.eq(h, x), i, Picker.pos(t, x, 1n+i))

# the first row shown: where the wheel left it (raw), else so the current
# model shows; never past the last page (law menu_scroll_in_list)
def Picker.off(+raw: String, +ms: List<&2, String>, +cur: String, +n: Nat, +shown: Nat) -> Nat:
  MN.Menu.off(Maybe.default(&2, Nat, Nat.read(raw), MN.Menu.reveal(Picker.pos(ms, cur, 0n), 0n, shown)), n, shown)

# a thin bar at the right edge when not every model fits
def Picker.bar(more: Bool, +x: Nat, +y: Nat, +span: Nat, +off: Nat, +shown: Nat, +n: Nat) -> List<&2, R.Op>:
  match more:
    case True{}:
      [D.Draw.rect(D.Clip.all(), Picker.px(x), Picker.px(Nat.add(y, Nat.div(Nat.mul(span, off), n))), 3, Picker.px(MN.Menu.max(Nat.div(Nat.mul(span, shown), n), 8n)), T.Th.faint(), 255)]
    case False{}:
      Nil{}

# the model menu, above the composer (bottom) and at its left edge, never
# above `limit` nor wider than x0..x1: effort chips on top where the
# provider has them, then the models, as many as fit (the wheel scrolls
# the rest). Every place comes from core/menu.bend (laws menu_*).
def Lay.picker.k(+ty: Ty, +ms: List<&2, String>, +cur: String, +info: String, +prov: String, +x0: U32, +x1: U32, +bottom: U32, +limit: U32, +es: List<&2, String>, +effort: String, +raw: String) -> Out:
  +x = U32.to_nat(x0)
  +n = List.length(&2, String, ms)
  +cws = Picker.chip_ws(ty, es)
  +names = Picker.names_w(ty, ms)
  +want = MN.Menu.max(MN.Menu.max(Nat.add(Nat.add(names, Picker.labels_w(ty, ms, info, prov)), 56n), Nat.add(Picker.line_w(cws), 12n)), 280n)
  +w = MN.Menu.width(want, U32.to_nat((x1 - x0 : U32)))
  +col = MN.Menu.col(names, Nat.sub(w, 40n))
  +rel = MN.Flow.under(cws, 6n, Nat.sub(w, 6n), 6n, 6n, 28n, 8n)
  +room = {Nat.sub(Nat.sub(U32.to_nat(bottom), U32.to_nat(limit)), Nat.add(rel, 6n)) : Nat}
  +shown = MN.Menu.fit(n, room)
  +span = MN.Menu.span(n, room)
  +hb = {Nat.add(Nat.add(rel, span), 6n) : Nat}
  +top = {Nat.sub(U32.to_nat(bottom), hb) : Nat}
  +off = Picker.off(raw, ms, cur, n, shown)
  Out.all([
    Out{Lay.box(D.Clip.all(), x0, Picker.px(top), Picker.px(w), Picker.px(hb), 8, T.Th.panel(), T.Th.line()),
      [Lay.hit(x0, Picker.px(top), Picker.px(w), Picker.px(hb), "menu", Nat.show(off) ++ "|" ++ Nat.show(Nat.sub(n, shown)))]},
    Out.ops(Bool.pick(List<&2, R.Op>, List.is_empty(&2, String, es), Nil{}, [D.Draw.rect(D.Clip.all(), (x0 + 1 : U32), Picker.px(Nat.sub(Nat.add(top, rel), 5n)), Picker.px(Nat.sub(w, 2n)), 1, T.Th.line(), 255)])),
    Picker.chips(ty, MN.Flow.place(cws, Nat.add(x, 6n), Nat.sub(Nat.add(x, w), 6n), Nat.add(x, 6n), Nat.add(top, 6n), 28n, 8n), es, effort, prov),
    Picker.rows(ty, MN.Menu.rows(shown, Nat.add(x, 6n), Nat.add(top, rel), Nat.sub(w, 12n)), List.drop(&2, String, ms, off), cur, info, prov, col),
    Out.ops(Picker.bar(Nat.is_lt(shown, n), Nat.sub(Nat.add(x, w), 4n), Nat.add(top, rel), span, off, shown, n))])

def Lay.picker(open: Bool, +ty: Ty, +ms: List<&2, String>, +cur: String, +info: String, +prov: String, +x0: U32, +x1: U32, +bottom: U32, +limit: U32, +es: List<&2, String>, +effort: String, +raw: String) -> Out:
  match open:
    case True{}:
      Lay.picker.k(ty, ms, cur, info, prov, x0, x1, bottom, limit, es, effort, raw)
    case False{}:
      Out.none()

def Skills.row(+ty: Ty, +n: String, +d: String, +x: U32, +y: U32, +w: U32, on: Bool) -> Out:
  +nw = Ty.width(ty, 2n, n)
  Out{List.concat(&2, R.Op, [Bool.pick(List<&2, R.Op>, on, Lay.round(D.Clip.all(), (x + 4 : U32), y, (w - 8 : U32), 28, 5, T.Th.sel()), Nil{}),
      Ty.text(ty, 2n, n, D.Clip.all(), (x + 14 : U32), y, 28, T.Th.text()),
      Ty.text(ty, 2n, Lay.fit_or_none(ty, 2n, d, U32.dim(w, (nw + 48 : U32))), D.Clip.all(), (x + 30 + nw : U32), y, 28, T.Th.dim())]),
    [Lay.hit(x, y, w, 28, "skill", n)]}

def Skills.rows(+ty: Ty, rs: List<&2, U.SkillRow>, +x: U32, +y: U32, +w: U32) -> Out:
  match rs:
    case Nil{}:
      Out.none()
    case U.SkillRow{+n, +d, on} <> t:
      Out.cat(Skills.row(ty, n, d, x, y, w, on), Skills.rows(ty, t, x, (y + 28 : U32), w))

# the skills a `$` being typed may complete to, or the bots, threads,
# projects and rooms a `@`, `>`, `%` or `#` may, above the composer with
# their summaries (the arrow keys move the highlight, Tab or Enter takes it)
def Lay.skills(+ty: Ty, +rs: List<&2, U.SkillRow>, +x0: U32, +x1: U32, +bottom: U32) -> Out:
  +n = {U32.from_nat(List.length(&2, U.SkillRow, rs)) : U32}
  +h = {(n * 28 + 8 : U32) : U32}
  Out.when(U32.is_gt(n, 0),
    Out.cat(Out.ops(Lay.box(D.Clip.all(), x0, (bottom - h : U32), (x1 - x0 : U32), h, 8, T.Th.panel(), T.Th.line())),
      Skills.rows(ty, rs, x0, (bottom - h + 4 : U32), (x1 - x0 : U32))))

# where position p of the draft sits: its wrapped line and x
# the shown line position p is on, and how far into it (ss: where each
# line starts, core/edit.bend's Edit.starts)
def Comp.row(+ss: List<&2, Nat>, +p: Nat) -> Nat:
  ED.Edit.row(ss, p)

def Comp.x(+ty: Ty, +draft: String, +ss: List<&2, Nat>, +p: Nat) -> U32:
  Ty.width(ty, 3n, ED.Edit.before(ss, draft, p))

# the selection behind the shown lines (j is the first one's number),
# from line rlo at xlo to line rhi at xhi
def Comp.sel.go(ls: List<&2, String>, +ty: Ty, +j: Nat, +rlo: Nat, +xlo: U32, +rhi: Nat, +xhi: U32, +x: U32, +y: U32) -> List<&2, R.Op>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      +a = {Bool.pick(U32, Nat.is_eq(j, rlo), xlo, 0) : U32}
      +b = {Bool.pick(U32, Nat.is_eq(j, rhi), xhi, (Ty.width(ty, 3n, l) + 6 : U32)) : U32}
      List.append(&2, R.Op, Bool.pick(List<&2, R.Op>, Bool.and(Bool.and(Nat.is_ge(j, rlo), Nat.is_le(j, rhi)), U32.is_gt(b, a)),
        [D.Draw.rect(D.Clip.all(), (x + a : U32), y, (b - a : U32), 20, T.Th.sel(), 255)], Nil{}),
        Comp.sel.go(t, ty, 1n+j, rlo, xlo, rhi, xhi, x, (y + 23 : U32)))

def Comp.sel(on: Bool, +ty: Ty, +shown: List<&2, String>, +top: Nat, +draft: String, +ss: List<&2, Nat>, +lo: Nat, +hi: Nat, +x: U32, +y: U32) -> Out:
  match on:
    case True{}:
      Out.ops(Comp.sel.go(shown, ty, top, Comp.row(ss, lo), Comp.x(ty, draft, ss, lo), Comp.row(ss, hi), Comp.x(ty, draft, ss, hi), x, y))
    case False{}:
      Out.none()

# the same over a span, dy below the line's top and hh tall, in a colour
def Comp.span.go(ls: List<&2, String>, +ty: Ty, +j: Nat, +rlo: Nat, +xlo: U32, +rhi: Nat, +xhi: U32, +x: U32, +y: U32, +dy: U32, +hh: U32, +color: U32) -> List<&2, R.Op>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      +a = {Bool.pick(U32, Nat.is_eq(j, rlo), xlo, 0) : U32}
      +b = {Bool.pick(U32, Nat.is_eq(j, rhi), xhi, (Ty.width(ty, 3n, l) + 6 : U32)) : U32}
      List.append(&2, R.Op, Bool.pick(List<&2, R.Op>, Bool.and(Bool.and(Nat.is_ge(j, rlo), Nat.is_le(j, rhi)), U32.is_gt(b, a)),
        [D.Draw.rect(D.Clip.all(), (x + a : U32), (y + dy : U32), (b - a : U32), hh, color, 255)], Nil{}),
        Comp.span.go(t, ty, 1n+j, rlo, xlo, rhi, xhi, x, (y + 23 : U32), dy, hh, color))

# what the input method is composing (core/ime.bend's runs, positions in
# the shown draft): the clause being converted behind the text like a
# selection, all of it underlined
def Comp.pre(pre: List<&2, IME.Run>, +ty: Ty, +shown: List<&2, String>, +top: Nat, +draft: String, +ss: List<&2, Nat>, +x: U32, +y: U32) -> Out:
  match pre:
    case Nil{}:
      Out.none()
    case IME.Run{+lo, +hi, +hot} <> t:
      Out.cat(Out.cat(
        Out.when(hot, Out.ops(Comp.span.go(shown, ty, top, Comp.row(ss, lo), Comp.x(ty, draft, ss, lo), Comp.row(ss, hi), Comp.x(ty, draft, ss, hi), x, y, 0, 20, T.Th.sel()))),
        Out.ops(Comp.span.go(shown, ty, top, Comp.row(ss, lo), Comp.x(ty, draft, ss, lo), Comp.row(ss, hi), Comp.x(ty, draft, ss, hi), x, y, 19, 1, T.Th.text()))),
        Comp.pre(t, ty, shown, top, draft, ss, x, y))

# how much taller than two lines the composer is for n wrapped lines
def Comp.eh(+n: Nat) -> U32:
  (U32.from_nat(Nat.sub(S.Cs.rows(n), 2n)) * 23 : U32)

# the draft wrapped as the composer shows it, cw wide
def Comp.wrap(+ty: Ty, +draft: String, +cw: U32) -> List<&2, String>:
  Ty.wrap(ty, 3n, draft, (cw - 40 : U32))

# a scrollbar at x beside v of n lines from top, the view th tall
def Comp.bar(on: Bool, +n: Nat, +v: Nat, +top: Nat, +x: U32, +y: U32, +th: U32) -> Out:
  match on:
    case True{}:
      +bh = {U32.max(16, U32.from_nat(Nat.div(Nat.mul(U32.to_nat(th), v), n))) : U32}
      +by = {U32.from_nat(Nat.div(Nat.mul(U32.to_nat((th - bh : U32)), top), Nat.sub(n, v))) : U32}
      Out.ops([D.Draw.rect(D.Clip.all(), x, y, 4, th, T.Th.raise(), 255), D.Draw.rect(D.Clip.all(), x, (y + by : U32), 4, bh, T.Th.dim(), 255)])
    case False{}:
      Out.none()

# the composer grows upward with its draft (its footer stays put); held
# is the wheel's choice of first line (nui.bend's Cs). Its footer row sits
# 10 px in from the box's bottom and sides, and the model chip's text lines
# up with the draft's.
def Comp.foot(+h: U32) -> U32:
  (h - 58 : U32)

def Comp.send_x(+ty: Ty, +x1: U32, +send: String) -> U32:
  (x1 - 10 - Lay.button.w(ty, send) : U32)

# the model's label fitted to what the mic, the paperclip and the send
# button leave (in a narrow main area it ran under them)
def Comp.models(+ty: Ty, +models: String, +x0: U32, +x1: U32, +send: String) -> String:
  Ty.fit(ty, 2n, models, U32.dim(U32.dim(Comp.send_x(ty, x1, send), (x0 + 8 : U32)), 122))

# the chip's label cut to fit before its context tail ("ctx 30% of 1M"),
# which is never the part cut off
def Comp.chip(+ty: Ty, +label: String, +tail: String, +x0: U32, +x1: U32, +send: String) -> String:
  Ty.fit(ty, 2n, label, U32.dim(U32.dim(U32.dim(Comp.send_x(ty, x1, send), (x0 + 8 : U32)), 122), Ty.width(ty, 2n, tail))) ++ tail

def Lay.composer(+ty: Ty, +draft: String, +ls: List<&2, String>, +held: String, +caret: Nat, +mark: Nat, +focused: Bool, +blink: Bool, +x0: U32, +x1: U32, +h: U32, +send: String, +act: String, +models: String, +rs: List<&2, U.SkillRow>, pre: List<&2, IME.Run>) -> Out:
  +tw = {(x1 - x0 - 40 : U32) : U32}
  +n = List.length(&2, String, ls)
  +v = S.Cs.rows(n)
  +eh = Comp.eh(n)
  +ss = ED.Edit.starts(ls, draft)
  +crow = Comp.row(ss, caret)
  +top = S.Cs.top(held, n, crow)
  +shown = List.take(&2, String, List.drop(&2, String, ls, top), v)
  +row = Nat.sub(crow, top)
  +cx = Comp.x(ty, draft, ss, caret)
  +y = {(h - 120 - eh : U32) : U32}
  +fy = Comp.foot(h)
  Out.all([
    Out.ops(Lay.box(D.Clip.all(), x0, y, (x1 - x0 : U32), (100 + eh : U32), 10, T.Th.panel(), Bool.pick(U32, focused, T.Th.focus(), T.Th.line()))),
    Comp.sel(Bool.and(focused, Bool.not(Nat.is_eq(caret, mark))), ty, shown, top, draft, ss, ED.Edit.lo(caret, mark), ED.Edit.hi(caret, mark), (x0 + 16 : U32), (y + 12 : U32)),
    Comp.pre(pre, ty, shown, top, draft, ss, (x0 + 16 : U32), (y + 12 : U32)),
    Bool.pick(Out, String.is_empty(draft),
      Out.ops(Ty.text(ty, 3n, Ty.fit(ty, 3n, MSG.T.composer.placeholder(Ty.loc(ty)), tw), D.Clip.all(), (x0 + 16 : U32), (y + 10 : U32), 23, T.Th.faint())),
      Lay.lines(ty, shown, D.Clip{(x0 + 1 : U32), (y + 1 : U32), (x1 - 1 : U32), (y + 60 + eh : U32)}, (x0 + 16 : U32), (y + 10 : U32))),
    Bool.pick(Out, Bool.and(focused, Bool.and(Nat.is_ge(crow, top), Nat.is_lt(row, v))), Out.ops([D.Draw.caret(D.Clip.all(), (x0 + 16 + cx : U32), (y + 14 + (U32.from_nat(row) * 23 : U32) : U32), 1, 16, T.Th.accent(), 255, blink)]), Out.none()),
    Bool.pick(Out, Bool.and(focused, Bool.and(Nat.is_ge(crow, top), Nat.is_lt(row, v))), Out{Nil{}, [Lay.hit((x0 + 16 + cx : U32), (y + 14 + (U32.from_nat(row) * 23 : U32) : U32), 1, 18, "ime-spot", "")]}, Out.none()),
    Comp.bar(Nat.is_gt(n, v), n, v, top, (x1 - 12 : U32), (y + 12 : U32), (U32.from_nat(v) * 23 - 4 : U32)),
    Out{Nil{}, [Lay.hit(x0, y, (x1 - x0 : U32), (64 + eh : U32), "focus", "composer"), Lay.hit(0, 0, 0, 0, "cs", Nat.show(top) ++ "," ++ Nat.show(Nat.sub(n, v)))]},
    Out{List.append(&2, R.Op, Ty.text(ty, 2n, models, D.Clip.all(), (x0 + 16 : U32), fy, 28, T.Th.dim()), Lay.icon(ty, "chevron-down", D.Clip.all(), (x0 + 22 + Ty.width(ty, 2n, models) : U32), (fy + 6 : U32), T.Th.dim())), [Lay.hit((x0 + 8 : U32), fy, (Ty.width(ty, 2n, models) + 38 : U32), 28, "flag", "models")]},
    Lay.button(ty, D.Clip.all(), Comp.send_x(ty, x1, send), fy, send, Bool.pick(U32, String.eq(act, "interrupt"), T.Th.bad(), T.Th.accent()), act, ""),
    Lay.skills(ty, rs, x0, x1, (y - 4 : U32))])

# Caret moves only layout can place
# ---------------------------------
# Up and Down in the composer go by the lines it shows, and a press or a
# drag in it puts the caret under the pointer; both depend on how the
# draft wraps, so nui.bend leaves them in the scratch ("@cq", its Cq) and
# the window places them after each event with the font: "u0"/"d0" a line
# up or down ("u1"/"d1" with shift, selecting), "p,x,y" a press, "m,x,y"
# a drag. The composer's region and first shown line come from the last
# frame's hits. Going up or down keeps the column it started from while
# the caret stays where it put it ("@cgoal", signed as "@mark" is).

# the composer's region in the last frame (all zero when it has none)
def Comp.hit(hs: List<&2, S.Hit>) -> S.Hit:
  match hs:
    case Nil{}:
      S.Hit{0, 0, 0, 0, "", ""}
    case S.Hit{+a, +b, +c, +d, +ac, +v} <> t:
      Bool.pick(S.Hit, Bool.and(String.eq(ac, "focus"), String.eq(v, "composer")), S.Hit{a, b, c, d, ac, v}, Comp.hit(t))

# how many characters of l lie left of dx, counting one when dx passes
# the middle of it
def Comp.col(l: String, +ty: Ty, +pen: U32, +dx: U32) -> Nat:
  match l:
    case SNil{}:
      0n
    case SCon{Chr{+c}, t}:
      +w = Ty.width(ty, 3n, SCon{Chr{c}, SNil{}})
      Bool.pick(Nat, U32.is_gt((pen + pen + w : U32), (dx + dx : U32)), 0n, 1n+Comp.col(t, ty, (pen + w : U32), dx))

# the position on line r nearest dx pixels in
def Comp.at(+ty: Ty, +ls: List<&2, String>, +ss: List<&2, Nat>, +r: Nat, +dx: U32) -> Nat:
  Nat.add(ED.Edit.start(ss, r), Comp.col(Maybe.default(&2, String, List.get(&2, String, ls, r), ""), ty, 0, dx))

# a line up (or down) from line r, at goal pixels in; past the first line is
# the start, past the last the end
def Comp.vert(up: Bool, +ty: Ty, +draft: String, +ls: List<&2, String>, +ss: List<&2, Nat>, +r: Nat, +goal: U32) -> Nat:
  match up:
    case True{}:
      Bool.pick(Nat, Nat.is_eq(r, 0n), 0n, Comp.at(ty, ls, ss, Nat.sub(r, 1n), goal))
    case False{}:
      Bool.pick(Nat, Nat.is_ge(1n+r, List.length(&2, String, ls)), String.length(draft), Comp.at(ty, ls, ss, 1n+r, goal))

# the position under (x, y) in the composer h (top: its first shown line)
def Comp.under(h: S.Hit, +ty: Ty, +ls: List<&2, String>, +ss: List<&2, Nat>, +top: Nat, +x: U32, +y: U32) -> Nat:
  match h:
    case S.Hit{+x0, +y0, _, _, _, _}:
      +r = Nat.add(top, U32.to_nat((U32.dim(y, (y0 + 10 : U32)) / 23 : U32)))
      Comp.at(ty, ls, ss, Bool.pick(Nat, Nat.is_ge(r, List.length(&2, String, ls)), Nat.sub(List.length(&2, String, ls), 1n), r), U32.dim(x, (x0 + 16 : U32)))

# the caret at c with the selection's other end at m; kept is the goal
# column to keep ("" none)
def Comp.put(+n: S.Nui, +c: Nat, +m: Nat, +goal: String) -> S.Nui:
  +n1 = S.Mark.save(S.Nui.with_caret(n, c), m)
  S.Nui.with_ui(n1, U.Ui.scratch_set(U.Ui.scratch_set(S.Nui.ui(n1), "@cq", ""), "@cgoal", Bool.pick(String, String.is_empty(goal), "", S.Mark.sig(n1) ++ goal)))

def Comp.hit.w(h: S.Hit) -> U32:
  match h:
    case S.Hit{+x0, _, +x1, _, _, _}:
      U32.dim(x1, x0)

# what q asks: 0 a press, 1 a drag, 2 a line up, 3 a line down
def Comp.kind(+q: String) -> Nat:
  Bool.pick(Nat, String.starts_with(q, "p"), 0n, Bool.pick(Nat, String.starts_with(q, "m"), 1n, Bool.pick(Nat, String.starts_with(q, "u"), 2n, 3n)))

# a line up or down, from the column it started at while the caret stays
# where the last one put it
def Comp.vert.go(up: Bool, +shift: Bool, +ty: Ty, +n: S.Nui, +draft: String, +ls: List<&2, String>, +ss: List<&2, Nat>, +goal: U32) -> S.Nui:
  +v = Comp.vert(up, ty, draft, ls, ss, Comp.row(ss, S.Nui.caret_of(n)), goal)
  Comp.put(n, v, Bool.pick(Nat, shift, S.Nui.mark(n), v), U32.show(goal))

def Comp.goal(+ty: Ty, +n: S.Nui, +draft: String, +ss: List<&2, Nat>) -> U32:
  U32.from_nat(S.Mark.of(U.Ui.scratch(S.Nui.ui(n), "@cgoal"), S.Mark.sig(n), U32.to_nat(Comp.x(ty, draft, ss, S.Nui.caret_of(n)))))

def Comp.under.q(+ty: Ty, +n: S.Nui, +h: S.Hit, +q: String, +ls: List<&2, String>, +ss: List<&2, Nat>) -> Nat:
  Comp.under(h, ty, ls, ss, S.Cs.num(S.Cs.region(S.Nui.hits(n)), 0n), U32.from_nat(S.Cs.num(q, 1n)), U32.from_nat(S.Cs.num(q, 2n)))

def Comp.press.go(+n: S.Nui, +u: Nat) -> S.Nui:
  Comp.put(n, u, u, "")

def Comp.place.k(k: Nat, +ty: Ty, +n: S.Nui, +h: S.Hit, +q: String, +draft: String, +ls: List<&2, String>, +ss: List<&2, Nat>) -> S.Nui:
  match k:
    case 0n:
      Comp.press.go(n, Comp.under.q(ty, n, h, q, ls, ss))
    case 1n:
      Comp.put(n, Comp.under.q(ty, n, h, q, ls, ss), S.Nui.mark(n), "")
    case +j:
      Comp.vert.go(Nat.is_eq(j, 2n), String.eq(String.drop(q, 1n), "1"), ty, n, draft, ls, ss, Comp.goal(ty, n, draft, ss))

def Comp.place.ls(+ty: Ty, +n: S.Nui, +h: S.Hit, +q: String, +draft: String, +ls: List<&2, String>) -> S.Nui:
  Comp.place.k(Comp.kind(q), ty, n, h, q, draft, ls, ED.Edit.starts(ls, draft))

def Comp.place.d(+ty: Ty, +n: S.Nui, +h: S.Hit, +q: String, +draft: String) -> S.Nui:
  Comp.place.ls(ty, n, h, q, draft, Comp.wrap(ty, draft, Comp.hit.w(h)))

# one asked-for move q in the composer region h
def Comp.place(+ty: Ty, +n: S.Nui, +h: S.Hit, +q: String) -> S.Nui:
  Comp.place.d(ty, n, h, q, U.Ui.draft_text(S.Nui.ui(n)))

def Comp.pend.h(found: Bool, +ty: Ty, +n: S.Nui, +h: S.Hit, +q: String) -> S.Nui:
  match found:
    case True{}:
      Comp.place(ty, n, h, q)
    case False{}:
      S.Nui.with_ui(n, U.Ui.scratch_set(S.Nui.ui(n), "@cq", ""))

def Comp.pend.k(none: Bool, +ty: Ty, +n: S.Nui, +q: String) -> S.Nui:
  match none:
    case True{}:
      n
    case False{}:
      +h = Comp.hit(S.Nui.hits(n))
      Comp.pend.h(U32.is_gt(Comp.hit.w(h), 40), ty, n, h, q)

# the window state after an event, with any move it asked for placed
def Comp.pend(+ty: Ty, s: S.Step) -> S.Step:
  match s:
    case S.Step{+n, cs, d}:
      S.Step{Comp.pend.k(String.is_empty(U.Ui.scratch(S.Nui.ui(n), "@cq")), ty, n, U.Ui.scratch(S.Nui.ui(n), "@cq")), cs, d}

# whether the head line ends above top (it is out of view)
def TL.above(ls: List<&2, TLine>, +y: U32, +top: U32) -> Bool:
  match ls:
    case TLine{_, _, +k, _, _} <> _:
      U32.is_lt((y + TL.h(k) : U32), top)
    case Nil{}:
      False{}

# the lines from the first one that reaches the view
def Lay.skip(ls: List<&2, TLine>, above: Bool, +y: U32, +top: U32) -> List<&2, TLine>:
  match ls above:
    case TLine{_, _, +k, _, _} <> +rest True{}:
      Lay.skip(rest, TL.above(rest, (y + TL.h(k) : U32), top), (y + TL.h(k) : U32), top)
    case _ _:
      ls

def Lay.skip_y(ls: List<&2, TLine>, +y: U32, +top: U32) -> U32:
  match ls:
    case Nil{}:
      y
    case TLine{_, _, +k, _, _} <> rest:
      Bool.pick(U32, U32.is_lt((y + TL.h(k) : U32), top), Lay.skip_y(rest, (y + TL.h(k) : U32), top), y)

# the timeline sits between top and the composer, pinned to the bottom;
# sc scrolls up into history
# the lines from the first one still (partly) below `off` pixels, and how
# many pixels the skipped lines took
def TL.drop.more(ls: List<&2, TLine>, +acc: U32, +off: U32) -> Bool:
  match ls:
    case TLine{_, _, +k, _, _} <> _:
      U32.is_le((acc + TL.h(k) : U32), off)
    case Nil{}:
      False{}

def TL.drop(ls: List<&2, TLine>, more: Bool, +acc: U32, +off: U32) -> List<&2, TLine>:
  match ls more:
    case TLine{_, _, +k, _, _} <> +rest True{}:
      TL.drop(rest, TL.drop.more(rest, (acc + TL.h(k) : U32), off), (acc + TL.h(k) : U32), off)
    case _ _:
      ls

def TL.drop.acc(ls: List<&2, TLine>, more: Bool, +acc: U32, +off: U32) -> U32:
  match ls more:
    case TLine{_, _, +k, _, _} <> +rest True{}:
      TL.drop.acc(rest, TL.drop.more(rest, (acc + TL.h(k) : U32), off), (acc + TL.h(k) : U32), off)
    case _ _:
      acc

# how far down the lines the jump's rule is (the whole height when none)
def TL.mark.y(ls: List<&2, TLine>, +acc: U32) -> U32:
  match ls:
    case Nil{}:
      acc
    case TLine{_, _, +k, _, _} <> t:
      Bool.pick(U32, U32.is_eq(k, 17), acc, TL.mark.y(t, (acc + TL.h(k) : U32)))

# the scroll that puts the rule a little under the top of the view
def TL.jump.sc(+y: U32, +over: U32) -> U32:
  U32.dim(over, U32.min(U32.dim(y, 24), over))

# Reading while the thread grows (client.bend's Hold)
# ---------------------------------------------------
# Scrolled up (sc > 0), the timeline holds what is read where it is: the
# last frame's place rides on a hit no pointer reaches ("tl-at":
# "sc|off|top|page first|visible entry|entry top"), nui.bend's Hold.take keeps it as "@tl.at"
# (with the thread) and the next frame starts from its `off` while sc is
# still the one it answered, the top's move made good (Hold.off): entries
# and streamed text below, the asks, todo card and composer under the
# timeline, and panels above it move nothing on screen. sc answered is
# what is now below the view. At the end (sc 0) the timeline follows.

# a field of the place ("" past the last)
def TL.hold.at(+hold: String, +i: Nat) -> String:
  Maybe.default(&2, String, List.get(&2, String, String.split(hold, '|'), i), "")

def TL.hold.num(+s: String) -> Nat:
  U32.to_nat(Maybe.default(&2, U32, U32.read(s), 0))

def TL.hold.k(still: Bool, +hold: String, +off: U32, +top: U32, +over: U32) -> U32:
  match still:
    case True{}:
      U32.from_nat(U.Hold.off(TL.hold.num(TL.hold.at(hold, 1n)), TL.hold.num(TL.hold.at(hold, 2n)), U32.to_nat(top), U32.to_nat(over)))
    case False{}:
      off

# the pixels hidden above the view: from the last frame's place while it
# was taken at this sc (and sc is not 0), else from sc
def TL.hold.off(+hold: String, +sc: U32, +off: U32, +top: U32, +over: U32) -> U32:
  TL.hold.k(Bool.and(U32.is_ne(sc, 0), String.eq(TL.hold.at(hold, 0n), U32.show(sc))), hold, off, top, over)

# First entry with pixels below off. Keep its content-coordinate top as
# well as its id; offsets can be deep inside a tall message.
def TL.hold.head(ls: List<&2, TLine>, stop: Bool, +off: U32, +acc: U32, +id: String, +y: U32) -> String:
  match ls stop:
    case _ True{}:
      id ++ "|" ++ U32.show(y)
    case Nil{} False{}:
      id ++ "|" ++ U32.show(y)
    case TLine{_, _, 29, cp, _} <> t False{}:
      TL.hold.head(t, False{}, off, acc, cp, acc)
    case TLine{_, _, +k, _, _} <> t False{}:
      TL.hold.head(t, U32.is_gt((acc + TL.h(k) : U32), off), off, (acc + TL.h(k) : U32), id, y)

def TL.hold.matches(ls: List<&2, TLine>, +id: String) -> Bool:
  match ls:
    case TLine{_, _, 29, cp, _} <> _:
      String.eq(cp, id)
    case _:
      False{}

# Find the old boundary, stopping at once when it is found.
def TL.hold.find(ls: List<&2, TLine>, hit: Bool, +id: String, +acc: U32) -> Maybe<&2, U32>:
  match ls hit:
    case _ True{}:
      Some{acc}
    case Nil{} False{}:
      None{}
    case TLine{_, _, +k, _, _} <> +t False{}:
      TL.hold.find(t, TL.hold.matches(t, id), id, (acc + TL.h(k) : U32))

# Move the entry's offset with both its content top and the panel top.
def TL.hold.move(+off: Nat, +y0: Nat, +y1: Nat, +t0: Nat, +t1: Nat, +over: Nat) -> Nat:
  U.Hold.off(Nat.add(Nat.sub(off, y0), y1), t0, t1, over)

def TL.hold.place(at: Maybe<&2, U32>, +hold: String, +top: U32, +over: U32, fallback: U32) -> U32:
  match at:
    case Some{y}:
      U32.from_nat(TL.hold.move(TL.hold.num(TL.hold.at(hold, 1n)), TL.hold.num(TL.hold.at(hold, 5n)), U32.to_nat(y), TL.hold.num(TL.hold.at(hold, 2n)), U32.to_nat(top), U32.to_nat(over)))
    case None{}:
      fallback

def TL.hold.entry(still: Bool, +ls: List<&2, TLine>, +hold: String, +top: U32, +over: U32, off: U32) -> U32:
  match still:
    case True{}:
      TL.hold.place(TL.hold.find(ls, TL.hold.matches(ls, TL.hold.at(hold, 4n)), TL.hold.at(hold, 4n), 0), hold, top, over, off)
    case False{}:
      off

# the timeline sits between top and the composer, pinned to the bottom;
# sc scrolls up into history. `off` is how many pixels of it are hidden
# above the view (never negative: all arithmetic stays unsigned-safe).
# hold: the last frame's place for this thread ("" none); first: the
# page's first entry
def Lay.timeline(+ty: Ty, +ls: List<&2, TLine>, +x0: U32, +x1: U32, +top: U32, +h: U32, +sc: U32, +sel: TSel, +hold: String, +first: String, chrome: Out) -> Out:
  +bot = {(h - 132 : U32) : U32}
  +view = {U32.dim(bot, (top + 16 : U32)) : U32}
  +total = TL.height(ls, 0)
  +over = {U32.dim(total, view) : U32}
  +off = TL.hold.entry(Bool.and(U32.is_ne(sc, 0), String.eq(TL.hold.at(hold, 0n), U32.show(sc))), ls, hold, top, over, TL.hold.off(hold, sc, U32.dim(over, U32.min(sc, over)), top, over))
  +sc2 = {Bool.pick(U32, U32.is_eq(sc, 0), 0, (over - off : U32)) : U32}
  +rest = TL.drop(ls, TL.drop.more(ls, 0, off), 0, off)
  +acc = TL.drop.acc(ls, TL.drop.more(ls, 0, off), 0, off)
  +ry = {((top + 16 + acc : U32) - off : U32) : U32}
  Out.all([TL.draw_all(ty, rest, TL.more(rest, ry, bot), D.Clip{x0, top, x1, bot}, x0, x1, ry, bot,
    Nat.sub(List.length(&2, TLine, ls), List.length(&2, TLine, rest)), sel), TL.sel.hit(ty, ls, sel, x0, x1), chrome,
    Out{Nil{}, [Lay.hit(0, 0, 0, 0, "tl-at", U32.show(sc2) ++ "|" ++ U32.show(off) ++ "|" ++ U32.show(top) ++ "|" ++ first ++ "|" ++ TL.hold.head(ls, False{}, off, 0, "", 0)), Lay.hit(0, 0, 0, 0, "tl-most", U32.show(over)), Lay.hit(0, 0, 0, 0, "tl-jump", U32.show(TL.jump.sc(TL.mark.y(ls, 0), over))), Lay.hit(0, 0, 0, 0, "tl-clip", U32.show(x0) ++ "," ++ U32.show(top) ++ "," ++ U32.show(x1) ++ "," ++ U32.show(bot))]}])

# Terminals over the thread
# -------------------------
# Each live terminal the timeline shows left focus hits ("emb:<key>")
# over its whole
# screen; its screen is drawn once over them, cut to the timeline
# ("tl-clip"). Pinned ones are drawn at the top, under the subagents.

def Ew.at(+v: String, +i: Nat) -> U32:
  EM.Emb.num(Maybe.default(&2, String, List.get(&2, String, String.split(v, ','), i), ""))

# where the timeline is: a hit no pointer reaches holds it ("x0,y0,x1,y1")
def Ew.clip(hs: List<&2, S.Hit>) -> D.Clip:
  match hs:
    case Nil{}:
      D.Clip.all()
    case S.Hit{_, _, _, _, "tl-clip", +v} <> _:
      D.Clip{Ew.at(v, 0n), Ew.at(v, 1n), Ew.at(v, 2n), Ew.at(v, 3n)}
    case _ <> t:
      Ew.clip(t)

# a terminal's screen: a hit that gives it the focus
def Ew.is(+a: String, +v: String) -> Bool:
  Bool.and(String.eq(a, "focus"), String.starts_with(v, "emb:"))

def Ew.seen(ks: List<&2, String>, +k: String) -> Bool:
  match ks:
    case Nil{}:
      False{}
    case x <> t:
      Bool.or(String.eq(x, k), Ew.seen(t, k))

def Ew.take(+h: S.Hit, +ks: List<&2, String>) -> List<&2, S.Hit>:
  match h:
    case S.Hit{x0, y0, x1, y1, +a, +v}:
      Bool.pick(List<&2, S.Hit>, Bool.and(Ew.is(a, v), Bool.not(Ew.seen(ks, v))), [S.Hit{x0, y0, x1, y1, a, v}], Nil{})

def Ew.add(+h: S.Hit, +ks: List<&2, String>) -> List<&2, String>:
  match h:
    case S.Hit{_, _, _, _, +a, +v}:
      Bool.pick(List<&2, String>, Ew.is(a, v), v <> ks, ks)

# the terminals' hits, one a terminal
def Ew.uniq(hs: List<&2, S.Hit>, +ks: List<&2, String>) -> List<&2, S.Hit>:
  match hs:
    case Nil{}:
      Nil{}
    case +h <> t:
      List.append(&2, S.Hit, Ew.take(h, ks), Ew.uniq(t, Ew.add(h, ks)))

def Ew.cut(+c: D.Clip, +x0: U32, +y0: U32, +x1: U32, +y1: U32) -> D.Clip:
  match c:
    case D.Clip{+a, +b, +cx, +cy}:
      D.Clip{U32.max(a, x0), U32.max(b, y0), U32.min(cx, x1), U32.min(cy, y1)}

def Pins.one(+rows: U32) -> U32:
  (26 + rows * 18 + 12 : U32)

def Pins.height(vs: List<&2, EM.EV>) -> U32:
  match vs:
    case Nil{}:
      0
    case EM.EV{_, _, _, _, +r, _, _} <> t:
      (Pins.one(r) + Pins.height(t) : U32)

# Subagents
# ---------
# The panel over the timeline (core/subs.bend has the rows): its border and
# count turn the accent while any are at work; a task's row opens its
# thread, a subagent's opens or shuts its steps.

def Sa.color(+word: String) -> U32:
  Bool.pick(U32, String.eq(word, "running"), T.Th.accent(),
    Bool.pick(U32, Bool.or(String.eq(word, "held"), Bool.or(String.eq(word, "done"), String.eq(word, "reporting"))), T.Th.ok(),
    Bool.pick(U32, Bool.or(Bool.or(Bool.or(String.eq(word, "waiting"), String.eq(word, "waiting on")), String.eq(word, "needs you")), Bool.or(String.eq(word, "watching"), String.eq(word, "launching"))), T.Th.accent(),
      Bool.pick(U32, Bool.or(String.is_empty(word), String.eq(word, "starting")), T.Th.faint(),
        Bool.pick(U32, Bool.or(String.eq(word, "cancelled"), String.eq(word, "cancelling")), T.Th.warn(), T.Th.bad())))))

def Sa.steps.n(+ss: List<&2, SB.SubStep>) -> U32:
  U32.from_nat(List.length(&2, SB.SubStep, ss))

# how tall a row is: its line, what it does now, the steps it shows
def Sa.h(r: SB.SubRow) -> U32:
  match r:
    case SB.SubRow{_, _, _, _, _, +doing, _, _, _, +ss}:
      (28 + Bool.pick(U32, String.is_empty(doing), 0, 20) + Sa.steps.n(ss) * 20 : U32)

def Sa.step(+ty: Ty, s: SB.SubStep, +x: U32, +x1: U32, +y: U32) -> List<&2, R.Op>:
  match s:
    case SB.SubStep{+kind, +text}:
      +lab = SB.Subs.step.label(kind)
      +lw = {Bool.pick(U32, String.is_empty(lab), 0, (Ty.width(ty, 2n, lab) + 8 : U32)) : U32}
      List.append(&2, R.Op, Ty.text(ty, 2n, lab, D.Clip.all(), x, y, 20, T.Th.faint()),
        Ty.text(ty, 2n, Ty.fit(ty, 2n, text, (x1 - x - lw : U32)), D.Clip.all(), (x + lw : U32), y, 20, T.Th.dim()))

def Sa.steps(+ty: Ty, ss: List<&2, SB.SubStep>, +x: U32, +x1: U32, +y: U32) -> List<&2, R.Op>:
  match ss:
    case Nil{}:
      Nil{}
    case s <> t:
      List.append(&2, R.Op, Sa.step(ty, s, x, x1, y), Sa.steps(ty, t, x, x1, (y + 20 : U32)))

# a task shows its model; a subagent the chevron that opens it
def Sa.lead(agent: Bool, +ty: Ty, +open: Bool, +who: String, +x0: U32, +y: U32) -> List<&2, R.Op>:
  match agent:
    case True{}:
      Lay.icon(ty, Bool.pick(String, open, "chevron-down", "chevron-right"), D.Clip.all(), (x0 + 24 : U32), (y + 6 : U32), T.Th.dim())
    case False{}:
      Ty.text(ty, 2n, who, D.Clip.all(), (x0 + 26 : U32), y, 28, T.Th.dim())

def Sa.hit(agent: Bool, +open: Bool, +key: String, +x0: U32, +x1: U32, +y: U32, +h: U32) -> S.Hit:
  match agent:
    case True{}:
      Lay.hit(x0, y, (x1 - x0 : U32), h, "fold", SB.Subs.ask(open, key))
    case False{}:
      Lay.hit(x0, y, (x1 - x0 : U32), h, "select", key)

# a row's Stop (SB.Subs.cancel: watched work, a delegated task), left of
# its state word; its region comes before the row's
def Sa.stop(none: Bool, +ty: Ty, +act: String, +key: String, +x: U32, +y: U32) -> Out:
  match none:
    case True{}:
      Out.none()
    case False{}:
      Out{Ty.text(ty, 2n, SB.Subs.cancel.word(act), D.Clip.all(), x, y, 28, T.Th.warn()), [Lay.hit((x - 4 : U32), y, (Ty.width(ty, 2n, SB.Subs.cancel.word(act)) + 8 : U32), 28, act, key)]}

def Sa.stop.x(+ty: Ty, +word: String, +act: String, +x1: U32) -> U32:
  +a = Ty.width(ty, 2n, word)
  +b = Ty.width(ty, 2n, SB.Subs.cancel.word(act))
  ((x1 - a : U32) - (b + 24 : U32) : U32)

def Sa.row.k(+ty: Ty, r: SB.SubRow, +x0: U32, +x1: U32, +y: U32) -> Out:
  match r:
    case SB.SubRow{+key, +agent, +title, +who, +word, +doing, +live, +open, _, ss}:
      +lb = U.StateWord.label(Ty.loc(ty), word)
      +ww = {(Ty.width(ty, 2n, lb) + Bool.pick(U32, String.is_empty(SB.Subs.cancel(SB.SubRow{key, agent, title, who, word, doing, live, open, 0n, Nil{}})), 0, (Ty.width(ty, 2n, SB.Subs.cancel.word(SB.Subs.cancel(SB.SubRow{key, agent, title, who, word, doing, live, open, 0n, Nil{}}))) + 12 : U32)) : U32) : U32}
      +lw = {Bool.pick(U32, agent, 20, (Ty.width(ty, 2n, who) + 12 : U32)) : U32}
      +tx = {(x0 + 26 + lw : U32) : U32}
      +dh = {Bool.pick(U32, String.is_empty(doing), 0, 20) : U32}
      Out{List.concat(&2, R.Op, [
          [D.Draw.disc(D.Clip.all(), U32.to_f32((x0 + 14 : U32)), U32.to_f32((y + 14 : U32)), Bool.pick(F32, live, 4.0, 3.0), Sa.color(word), 255)],
          Sa.lead(agent, ty, open, who, x0, y),
          Ty.text(ty, 0n, Ty.fit(ty, 0n, TTL.Title.shown(Ty.loc(ty), title), (x1 - tx - ww - 24 : U32)), D.Clip.all(), tx, y, 28, Bool.pick(U32, live, T.Th.text(), T.Th.soft())),
          Ty.text(ty, 2n, lb, D.Clip.all(), (x1 - Ty.width(ty, 2n, lb) - 12 : U32), y, 28, Sa.color(word)),
          Ty.text(ty, 2n, Ty.fit(ty, 2n, doing, (x1 - tx - 12 : U32)), D.Clip.all(), tx, (y + 26 : U32), 18, T.Th.accent()),
          Sa.steps(ty, ss, tx, (x1 - 12 : U32), (y + 28 + dh : U32))]),
        [Sa.hit(agent, open, key, x0, x1, y, (28 + dh : U32))]}

def Sa.row(+ty: Ty, +r: SB.SubRow, +x0: U32, +x1: U32, +y: U32) -> Out:
  Out.cat(Sa.row.k(ty, r, x0, x1, y), Sa.stop(String.is_empty(SB.Subs.cancel(r)), ty, SB.Subs.cancel(r), SB.Subs.key(r), Sa.stop.x(ty, U.StateWord.label(Ty.loc(ty), SB.Subs.word(r)), SB.Subs.cancel(r), x1), y))

def Sa.rows(+ty: Ty, rs: List<&2, SB.SubRow>, +x0: U32, +x1: U32, +y: U32) -> Out:
  match rs:
    case Nil{}:
      Out.none()
    case +r <> rest:
      Out.cat(Sa.row(ty, r, x0, x1, y), Sa.rows(ty, rest, x0, x1, (y + Sa.h(r) : U32)))

def Sa.sum(rs: List<&2, SB.SubRow>) -> U32:
  match rs:
    case Nil{}:
      0
    case +r <> rest:
      (Sa.h(r) + Sa.sum(rest) : U32)

# A pane that scrolls by rows (the subagents panel, the todo card): the
# first row shown is the info's "pane:<key>" (nui.bend's Pane, the wheel
# over the pane's "pane" hit), and the rows from there that fit its height.
def Pane.off(+info: Map<&2, String>, +key: String) -> Nat:
  Maybe.default(&2, Nat, Nat.read(U.Str.get(info, "pane:" ++ key)), 0n)

def Pane.clamp(+off: Nat, +n: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_lt(off, n), off, Nat.sub(n, 1n))

# "rows 4–9 of 12" when not all of them show, else ""
def Pane.range.loc(+l: I.Loc, +from: Nat, +shown: Nat, +n: Nat) -> String:
  Bool.pick(String, Nat.is_lt(shown, n), MSG.T.pane.range(l, Nat.show(1n+from), Nat.show(Nat.add(from, shown)), Nat.show(n)), "")

# the subagent rows from the first shown that fit room (at least one): a row
# that does not fit leaves no room for any after it

def Pane.range(+from: Nat, +shown: Nat, +n: Nat) -> String:
  Pane.range.loc(I.En{}, from, shown, n)

def Sa.fit.ok(+h: U32, +room: U32, +first: Bool) -> Bool:
  Bool.or(U32.is_le(h, room), first)

def Sa.fit.put(ok: Bool, r: SB.SubRow, rest: List<&2, SB.SubRow>) -> List<&2, SB.SubRow>:
  match ok:
    case True{}:
      r <> rest
    case False{}:
      Nil{}

def Sa.fit(rs: List<&2, SB.SubRow>, +room: U32, +first: Bool) -> List<&2, SB.SubRow>:
  match rs:
    case Nil{}:
      Nil{}
    case +r <> rest:
      Sa.fit.put(Sa.fit.ok(Sa.h(r), room, first), r,
        Sa.fit(rest, Bool.pick(U32, Sa.fit.ok(Sa.h(r), room, first), (room - U32.min(Sa.h(r), room) : U32), 0), False{}))

# the panel's rows on screen: open ones from the scrolled-to row, as many as
# a third of the window holds
def Subs.vis(+all: List<&2, SB.SubRow>, +open: Bool, +off: Nat, +h: U32) -> List<&2, SB.SubRow>:
  Sa.fit(List.drop(&2, SB.SubRow, SB.Subs.panel.rows(open, all), off), (h / 3 : U32), True{})

# the panel's box: its head, and its rows when open
def Subs.box.h(+rs: List<&2, SB.SubRow>, +open: Bool) -> U32:
  (Sa.sum(rs) + Bool.pick(U32, open, 36, 32) : U32)

# the subagent viewer: every task this thread delegated and each of the
# agent's own subagents, what each does now, and the steps of those open.
# Its head opens and shuts it; once all is done it folds to the head
# (subs.bend's panel). Never taller than a third of the window: the wheel
# over it scrolls its rows (Pane)
def Lay.subs.k(some: Bool, +ty: Ty, +all: List<&2, SB.SubRow>, +rs: List<&2, SB.SubRow>, +open: Bool, +th: String, +off: Nat, +x0: U32, +x1: U32, +y: U32) -> Out:
  match some:
    case True{}:
      +n = List.length(&2, SB.SubRow, SB.Subs.panel.rows(open, all))
      +on = Nat.is_gt(SB.Subs.live(all), 0n)
      +word = {SB.Subs.panel.word(all) ++ Pane.range.loc(Ty.loc(ty), off, List.length(&2, SB.SubRow, rs), n) : String}
      +bh = Subs.box.h(rs, open)
      Out.all([
        Out{Nil{}, [Lay.hit(x0, y, (x1 - x0 : U32), bh, "pane", "subs:" ++ th ++ "|" ++ Nat.show(Nat.sub(n, 1n)))]},
        Out.ops(Lay.box(D.Clip.all(), x0, y, (x1 - x0 : U32), bh, 8, T.Th.panel(), Bool.pick(U32, on, T.Th.accent(), T.Th.line()))),
        Out.ops(Ty.text(ty, 2n, MSG.T.thread.subagents(Ty.loc(ty)), D.Clip.all(), (x0 + 12 : U32), y, 32, Bool.pick(U32, on, T.Th.text(), T.Th.faint()))),
        Out.ops(Ty.text(ty, 2n, word, D.Clip.all(), (x1 - Ty.width(ty, 2n, word) - 36 : U32), y, 32, Bool.pick(U32, on, T.Th.accent(), T.Th.faint()))),
        Out.ops(Lay.icon(ty, Bool.pick(String, open, "chevron-down", "chevron-right"), D.Clip.all(), (x1 - 28 : U32), (y + 8 : U32), T.Th.faint())),
        Out{Nil{}, [Lay.hit(x0, y, (x1 - x0 : U32), 30, SB.Subs.panel.action(open), SB.Subs.panel.ask(open, th, all))]},
        Sa.rows(ty, rs, x0, x1, (y + 30 : U32))])
    case False{}:
      Out.none()

def Lay.subs(+ty: Ty, +all: List<&2, SB.SubRow>, +rs: List<&2, SB.SubRow>, +open: Bool, +th: String, +off: Nat, +x0: U32, +x1: U32, +y: U32) -> Out:
  Lay.subs.k(Bool.not(List.is_empty(&2, SB.SubRow, all)), ty, all, rs, open, th, off, x0, x1, y)

def Subs.height(+all: List<&2, SB.SubRow>, +rs: List<&2, SB.SubRow>, +open: Bool) -> U32:
  Bool.pick(U32, List.is_empty(&2, SB.SubRow, all), 0, (Subs.box.h(rs, open) + 12 : U32))

def Lay.parent.k(+ty: Ty, +p: String, +st: M.State, +thread: String, +x0: U32, +y: U32) -> Out:
  +label = U.Tasks.up.label(st, thread, p)
  Out.when(Bool.not(String.is_empty(p)),
    Out{Ty.text(ty, 2n, label, D.Clip.all(), x0, y, 28, T.Th.accent()), [Lay.hit(x0, y, (Ty.width(ty, 2n, label) + 8 : U32), 28, "select", p)]})

# a child thread links back to its parent, a launched one to its launcher
def Lay.parent(+ty: Ty, +st: M.State, +thread: String, +x0: U32, +y: U32) -> Out:
  Lay.parent.k(ty, U.Tasks.up(st, thread), st, thread, x0, y)

# Asks: what the agent waits on the user for
# ------------------------------------------

def Page.num(+s: String, +i: Nat) -> U32:
  U32.from_nat(Maybe.default(&2, Nat, Nat.read(Maybe.default(&2, String, List.get(&2, String, String.split(s, 'x'), i), "")), 0n))

def Page.fit.w(+bw: U32, +bh: U32, +pw: U32, +ph: U32) -> U32:
  U32.min(bw, Bool.pick(U32, U32.is_gt((bw * ph : U32), (bh * pw : U32)), pw, ((bw * ph : U32) / U32.max(bh, 1) : U32)))

# a picture fitted into cw x ch, never enlarged, with a rule round it; `size`
# is "WxH" once it is loaded. An "img" region asks the window to load it, and
# a click on it opens the lightbox.
def Lay.thumb(+ty: Ty, +key: String, +size: String, +x: U32, +y: U32, +cw: U32, +ch: U32) -> Out:
  +bw = Page.num(size, 0n)
  +bh = Page.num(size, 1n)
  +fw = Page.fit.w(bw, bh, cw, ch)
  +fh = {((fw * bh : U32) / U32.max(bw, 1) : U32) : U32}
  Out.cat(Out.when(String.is_empty(size), Out.ops(Ty.text(ty, 2n, MSG.T.image.loading(Ty.loc(ty)), D.Clip.all(), x, y, 20, T.Th.faint()))),
    Out{Bool.pick(List<&2, R.Op>, String.is_empty(size), Nil{}, D.Draw.frame(D.Clip.all(), (x - 1 : U32), (y - 1 : U32), (fw + 2 : U32), (fh + 2 : U32), T.Th.line())),
      [Lay.hit(x, y, U32.max(fw, 1), U32.max(fh, 1), "img", key)]})

# a run's or a pause's screenshot: the hub's URL path under the key the
# window loads it by (main.bend's Pics): "tshot:<port>:<path>", the port of
# the hub the window itself runs (info "listen"), used when that hub is here
def Lay.tshot(+listen: String, +url: String) -> String:
  "tshot:" ++ Maybe.default(&2, String, List.last(&2, String, String.split(listen, ':')), "") ++ ":" ++ url

def Ask.plan_lines(+ty: Ty, +text: String, +px: U32) -> List<&2, String>:
  List.take(&2, String, Ty.wrap(ty, 4n, text, px), 8n)

# an approval's detail wraps (font 4, mono) in at most Ask.cap() lines; the
# rest scroll with the wheel (the pane "ask:<id>")
def Ask.cap() -> Nat:
  14n

def Ask.lines.k(approval: Bool, +ty: Ty, +d: String, +px: U32) -> List<&2, String>:
  match approval:
    case True{}:
      Ty.wrap(ty, 4n, d, px)
    case False{}:
      Nil{}

def Ask.lines(+ty: Ty, +k: String, +d: String, +px: U32) -> List<&2, String>:
  Ask.lines.k(String.eq(k, "approval"), ty, d, px)

# the lines shown: all up to the cap, never none
def Ask.shown(+n: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_lt(n, Ask.cap()), Bool.pick(Nat, Nat.is_eq(n, 0n), 1n, n), Ask.cap())

# the first line shown, kept so the last line of the detail ends the pane
def Ask.off(+off: Nat, +n: Nat, +show: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_gt(Nat.add(off, show), n), Nat.sub(n, show), off)

def Ask.h.k(approval: Bool, +n: Nat) -> U32:
  match approval:
    case True{}:
      (54 + (U32.from_nat(Ask.shown(n)) * 21 : U32) : U32)
    case False{}:
      76

def Ask.std.h(+ty: Ty, a: M.Ask, +px: U32) -> U32:
  match a:
    case M.Ask{_, _, +k, _, +d, _, _, _}:
      Bool.pick(U32, String.eq(k, "plan"), (U32.from_nat(List.length(&2, String, Ask.plan_lines(ty, d, px))) * 21 + 76 : U32),
        Ask.h.k(String.eq(k, "approval"), List.length(&2, String, Ask.lines(ty, k, d, px))))

# a paused browser task: its head, the question (at most four lines), the
# screenshot and a row of buttons
def Ask.web.lines(+ty: Ty, t: U.TAsk, +px: U32) -> List<&2, String>:
  match t:
    case U.TAsk{_, _, _, _, _, +q, _, _, _}:
      List.take(&2, String, Ty.wrap(ty, 3n, q, px), 4n)

def Ask.web.shot(t: U.TAsk) -> String:
  match t:
    case U.TAsk{_, _, _, _, _, _, _, +shot, _}:
      shot

def Ask.web.h.k(+ls: Nat, +shot: String) -> U32:
  (U32.from_nat(ls) * 22 + 78 + Bool.pick(U32, String.is_empty(shot), 0, 160) : U32)

def Ask.web.h(+ty: Ty, +t: U.TAsk, +px: U32) -> U32:
  Ask.web.h.k(List.length(&2, String, Ask.web.lines(ty, t, px)), Ask.web.shot(t))

def Ask.h.w(web: Bool, +ty: Ty, a: M.Ask, +nf: Map<&2, String>, +px: U32) -> U32:
  match web:
    case True{}:
      Ask.web.h(ty, U.Teach.ask.from.loc(Ty.loc(ty), a, nf), px)
    case False{}:
      Ask.std.h(ty, a, px)

def Ask.h(+ty: Ty, a: M.Ask, +nf: Map<&2, String>, +px: U32) -> U32:
  match a:
    case M.Ask{_, _, +k, _, _, _, _, _}:
      Ask.h.w(String.eq(k, "browser"), ty, a, nf, px)

def Asks.h(+ty: Ty, as: List<&2, M.Ask>, +nf: Map<&2, String>, +px: U32) -> U32:
  match as:
    case Nil{}:
      0
    case a <> t:
      (Ask.h(ty, a, nf, px) + 8 + Asks.h(ty, t, nf, px) : U32)

def Ask.buttons(+ty: Ty, bs: List<&2, S.Hit>, +y: U32, +x: U32) -> Out:
  match bs:
    case Nil{}:
      Out.none()
    case S.Hit{_, _, _, _, +label, +value} <> t:
      Out.cat(Lay.button(ty, D.Clip.all(), (x - Lay.button.w(ty, label) : U32), y, label, T.Th.soft(), "answer", value),
        Ask.buttons(ty, t, y, (x - Lay.button.w(ty, label) - 8 : U32)))

def Ask.opts(ls: List<&2, String>, +id: String) -> List<&2, S.Hit>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      S.Hit{0, 0, 0, 0, l, id ++ "|" ++ l} <> Ask.opts(t, id)

def Lay.plan.lines(+ty: Ty, ls: List<&2, String>, +x0: U32, +y: U32) -> Out:
  match ls:
    case Nil{}:
      Out.none()
    case l <> t:
      Out.cat(Out.ops(Ty.text(ty, 3n, l, D.Clip.all(), (x0 + 14 : U32), y, 21, T.Th.soft())), Lay.plan.lines(ty, t, x0, (y + 21 : U32)))

# the buttons of an ask, right to left
def Ask.choices.loc(+l: I.Loc, +k: String, +id: String, +o: String, +draft: String) -> List<&2, S.Hit>:
  Bool.pick(List<&2, S.Hit>, String.eq(k, "send"), [S.Hit{0, 0, 0, 0, "Send", id ++ "|accept"}, S.Hit{0, 0, 0, 0, "Not now", id ++ "|decline"}],
  Bool.pick(List<&2, S.Hit>, String.eq(k, "plan"), [S.Hit{0, 0, 0, 0, MSG.T.approval.implement(l), id ++ "|implement"}, S.Hit{0, 0, 0, 0, MSG.T.approval.dismiss(l), id ++ "|dismiss"}],
    Bool.pick(List<&2, S.Hit>, String.eq(k, "input"),
      List.append(&2, S.Hit, Bool.pick(List<&2, S.Hit>, String.is_empty(String.trim(draft)), [S.Hit{0, 0, 0, 0, MSG.T.approval.dismiss(l), id ++ "|"}], [S.Hit{0, 0, 0, 0, MSG.T.approval.draft(l), id ++ "|" ++ String.trim(draft)}]),
        List.reverse(&2, S.Hit, Ask.opts(String.split(o, '\n'), id))),
      [S.Hit{0, 0, 0, 0, MSG.T.approval.allow(l), id ++ "|accept"}, S.Hit{0, 0, 0, 0, MSG.T.approval.session(l), id ++ "|session"}, S.Hit{0, 0, 0, 0, MSG.T.approval.deny(l), id ++ "|decline"}])))

def Ask.choices(+k: String, +id: String, +o: String, +draft: String) -> List<&2, S.Hit>:
  Ask.choices.loc(I.En{}, k, id, o, draft)

def Ask.text.lines(+ty: Ty, ls: List<&2, String>, +x0: U32, +y: U32) -> Out:
  match ls:
    case Nil{}:
      Out.none()
    case l <> t:
      Out.cat(Out.ops(Ty.text(ty, 4n, l, D.Clip.all(), (x0 + 14 : U32), y, 21, T.Th.text())), Ask.text.lines(ty, t, x0, (y + 21 : U32)))

# the detail of an approval, from its first line shown
def Ask.detail(+ty: Ty, +id: String, ls: List<&2, String>, +n: Nat, +show: Nat, +off: Nat, +x0: U32, +x1: U32, +y: U32, +h: U32) -> Out:
  Out.all([Out{Nil{}, [Lay.hit(x0, y, (x1 - x0 : U32), h, "pane", "ask:" ++ id ++ "|" ++ Nat.show(Nat.sub(n, show)))]},
    Ask.text.lines(ty, List.take(&2, String, List.drop(&2, String, ls, off), show), x0, (y + 26 : U32))])

def Ask.approval(+ty: Ty, +id: String, +ls: List<&2, String>, +info: Map<&2, String>, +x0: U32, +x1: U32, +y: U32, +h: U32) -> Out:
  +n = List.length(&2, String, ls)
  +show = Ask.shown(n)
  Ask.detail(ty, id, ls, n, show, Ask.off(Pane.off(info, "ask:" ++ id), n, show), x0, x1, y, h)

def Ask.body.k(plan: Bool, approval: Bool, +ty: Ty, +k: String, +id: String, +d: String, +info: Map<&2, String>, +x0: U32, +x1: U32, +y: U32, +h: U32) -> Out:
  match plan approval:
    case True{} _:
      Lay.plan.lines(ty, Ask.plan_lines(ty, d, (x1 - x0 - 32 : U32)), x0, (y + 28 : U32))
    case False{} True{}:
      Ask.approval(ty, id, Ask.lines(ty, k, d, (x1 - x0 - 32 : U32)), info, x0, x1, y, h)
    case False{} False{}:
      Out.ops(Ty.text(ty, 3n, Ty.fit(ty, 4n, d, (x1 - x0 - 28 : U32)), D.Clip.all(), (x0 + 14 : U32), (y + 26 : U32), 22, T.Th.text()))

def Ask.body(+ty: Ty, +k: String, +id: String, +d: String, +info: Map<&2, String>, +x0: U32, +x1: U32, +y: U32, +h: U32) -> Out:
  Ask.body.k(String.eq(k, "plan"), String.eq(k, "approval"), ty, k, id, d, info, x0, x1, y, h)

# "· 1–14 of 30" after the head while the detail scrolls
def Ask.range(+ty: Ty, +info: Map<&2, String>, +k: String, +id: String, +d: String, +px: U32) -> String:
  +ls = Ask.lines(ty, k, d, px)
  +n = List.length(&2, String, ls)
  +show = Ask.shown(n)
  Bool.pick(String, String.eq(k, "approval"), Pane.range.loc(Ty.loc(ty), Ask.off(Pane.off(info, "ask:" ++ id), n, show), show, n), "")

def Ask.std.card(+ty: Ty, a: M.Ask, +draft: String, +info: Map<&2, String>, +x0: U32, +x1: U32, +y: U32) -> Out:
  match a:
    case M.Ask{+id, _, +k, +tool, +d, +o, _, _}:
      +h = Ask.std.h(ty, M.Ask{id, "", k, tool, d, o, True{}, ""}, (x1 - x0 - 32 : U32))
      +head = U.Ask.head.loc(Ty.loc(ty), k, tool, d) ++ Ask.range(ty, info, k, id, d, (x1 - x0 - 32 : U32))
      Out.all([
        Out.ops(Lay.box(D.Clip.all(), x0, y, (x1 - x0 : U32), h, 0, T.Th.panel(), T.Th.warn())),
        Out.ops(Ty.text(ty, 2n, head, D.Clip.all(), (x0 + 14 : U32), (y + 4 : U32), 22, T.Th.warn())),
        Ask.body(ty, k, id, d, info, x0, x1, y, h),
        Ask.buttons(ty, Ask.choices.loc(Ty.loc(ty), U.Ask.bkind(k, tool), id, o, draft), (y + h - 36 : U32), (x1 - 12 : U32))])

# the buttons of a paused task's card, right to left (the first is leftmost)
def Ask.tbtns(bs: List<&2, U.TBtn>, +ty: Ty, +y: U32, +x: U32) -> Out:
  match bs:
    case Nil{}:
      Out.none()
    case U.TBtn{+label, +action, +value, +primary} <> t:
      Out.cat(Lay.button(ty, D.Clip.all(), (x - Lay.button.w(ty, label) : U32), y, label, Bool.pick(U32, primary, T.Th.accent(), T.Th.soft()), action, value),
        Ask.tbtns(t, ty, y, (x - Lay.button.w(ty, label) - 8 : U32)))

def Ask.web.btns(t: U.TAsk) -> List<&2, U.TBtn>:
  match t:
    case U.TAsk{_, _, _, _, _, _, _, _, bs}:
      List.reverse(&2, U.TBtn, bs)

def Ask.web.head.loc(+l: I.Loc, t: U.TAsk) -> String:
  match t:
    case U.TAsk{_, _, +slug, +n, _, _, _, _, _}:
      MSG.T.teach.paused(l, slug, Nat.show(n))

def Ask.web.head(t: U.TAsk) -> String:
  Ask.web.head.loc(I.En{}, t)

def Ask.rows(ls: List<&2, String>, +ty: Ty, +x: U32, +y: U32) -> List<&2, R.Op>:
  match ls:
    case Nil{}:
      Nil{}
    case l <> t:
      List.append(&2, R.Op, Ty.text(ty, 3n, l, D.Clip.all(), x, y, 22, T.Th.text()), Ask.rows(t, ty, x, (y + 22 : U32)))

def Ask.web.card.k(+t: U.TAsk, +ty: Ty, +nf: Map<&2, String>, +x0: U32, +x1: U32, +y: U32) -> Out:
  +ls = {Ask.web.lines(ty, t, (x1 - x0 - 28 : U32)) : List<&2, String>}
  +shot = {Ask.web.shot(t) : String}
  +h = {Ask.web.h(ty, t, (x1 - x0 - 32 : U32)) : U32}
  +ly = {(y + 28 : U32) : U32}
  +sy = {(ly + U32.from_nat(List.length(&2, String, ls)) * 22 + 4 : U32) : U32}
  Out.all([
    Out.ops(Lay.box(D.Clip.all(), x0, y, (x1 - x0 : U32), h, 0, T.Th.panel(), T.Th.warn())),
    Out.ops(Ty.text(ty, 2n, Ask.web.head.loc(Ty.loc(ty), t), D.Clip.all(), (x0 + 14 : U32), (y + 4 : U32), 22, T.Th.warn())),
    Out.ops(Ask.rows(ls, ty, (x0 + 14 : U32), ly)),
    Out.when(Bool.not(String.is_empty(shot)), Lay.thumb(ty, Lay.tshot(U.Str.get(nf, "listen"), shot), U.Str.get(nf, "img:" ++ Lay.tshot(U.Str.get(nf, "listen"), shot)), (x0 + 14 : U32), sy, (x1 - x0 - 28 : U32), 148)),
    Ask.tbtns(Ask.web.btns(t), ty, (y + h - 36 : U32), (x1 - 12 : U32))])

def Ask.card.k(web: Bool, +ty: Ty, a: M.Ask, +draft: String, +nf: Map<&2, String>, +x0: U32, +x1: U32, +y: U32) -> Out:
  match web:
    case True{}:
      Ask.web.card.k(U.Teach.ask.from.loc(Ty.loc(ty), a, nf), ty, nf, x0, x1, y)
    case False{}:
      Ask.std.card(ty, a, draft, nf, x0, x1, y)

def Ask.card(+ty: Ty, a: M.Ask, +draft: String, +nf: Map<&2, String>, +x0: U32, +x1: U32, +y: U32) -> Out:
  match a:
    case M.Ask{_, _, +k, _, _, _, _, _}:
      Ask.card.k(String.eq(k, "browser"), ty, a, draft, nf, x0, x1, y)

def Lay.asks(+ty: Ty, as: List<&2, M.Ask>, +draft: String, +nf: Map<&2, String>, +x0: U32, +x1: U32, +y: U32) -> Out:
  match as:
    case Nil{}:
      Out.none()
    case +a <> t:
      Out.cat(Ask.card(ty, a, draft, nf, x0, x1, y), Lay.asks(ty, t, draft, nf, x0, x1, (y + Ask.h(ty, a, nf, (x1 - x0 - 32 : U32)) + 8 : U32)))

# A side question (/btw) and its answer, in a card over the composer until
# closed; its answer wraps to at most Btw.rows() lines
def Btw.rows() -> Nat:
  12n

def Btw.lines(+ty: Ty, +a: String, +w: U32) -> List<&2, String>:
  List.take(&2, String, Ty.wrap(ty, 2n, U.Btw.text(Ty.loc(ty), a), (w - 32 : U32)), Btw.rows())

# the card's height with its gap below (0 with no question)
def Btw.h(+ty: Ty, +q: String, +a: String, +w: U32) -> U32:
  Bool.pick(U32, String.is_empty(q), 0, (48 + (U32.from_nat(List.length(&2, String, Btw.lines(ty, a, w))) * 20 : U32) : U32))

def Btw.card(+ty: Ty, +q: String, +a: String, +x0: U32, +x1: U32, +y: U32) -> Out:
  +ls = Btw.lines(ty, a, (x1 - x0 : U32))
  Out.all([
    Out.ops(List.concat(&2, R.Op, [Lay.box(D.Clip.all(), x0, y, (x1 - x0 : U32), (40 + (U32.from_nat(List.length(&2, String, ls)) * 20 : U32) : U32), 0, T.Th.raise(), T.Th.line()),
      Ty.text(ty, 2n, Ty.fit(ty, 2n, "btw · " ++ q, (x1 - x0 - 64 : U32)), D.Clip.all(), (x0 + 16 : U32), (y + 6 : U32), 24, T.Th.dim())])),
    Lay.lines(ty, ls, D.Clip.all(), (x0 + 16 : U32), (y + 30 : U32)),
    Lay.ibutton(ty, D.Clip.all(), (x1 - 34 : U32), (y + 4 : U32), "x", T.Th.soft(), "btw-close", "")])

def Btw.card.if(show: Bool, +ty: Ty, +q: String, +a: String, +x0: U32, +x1: U32, +y: U32) -> Out:
  match show:
    case True{}:
      Btw.card(ty, q, a, x0, x1, y)
    case False{}:
      Out.none()

# messages waiting for the running turn (stored on the server)
def Lay.qhits(bs: List<&2, U.QBtn>, acc: List<&2, S.Hit>) -> List<&2, S.Hit>:
  match bs:
    case Nil{}:
      acc
    case U.QBtn{l, a, v} <> t:
      Lay.qhits(t, S.Hit{0, 0, 0, 0, l, a ++ "|" ++ v} <> acc)

def Lay.qrow(+ty: Ty, q: M.QItem, +first: Bool, +run: Bool, +more: Nat, +x0: U32, +x1: U32, +y: U32) -> Out:
  match q:
    case M.QItem{+msg, +text}:
      +bs = Lay.qhits(U.Qu.row_buttons(Ty.loc(ty), run, first, msg), Nil{})
      +tag = U.Qu.tag(Ty.loc(ty), run, msg)
      +tail = Bool.pick(String, Nat.is_eq(more, 0n), "", MSG.T.queue.more(Ty.loc(ty), Nat.show(more)))
      +tw = (x1 - x0 - Lay.buttons.w(ty, bs) - Ty.width(ty, 2n, tag ++ tail) - 40 : U32)
      Out.cat(Out.ops(List.concat(&2, R.Op, [Lay.box(D.Clip.all(), x0, y, (x1 - x0 : U32), 30, 0, T.Th.raise(), T.Th.line()),
          Ty.text(ty, 2n, tag, D.Clip.all(), (x0 + 12 : U32), y, 30, T.Th.queued()),
          Ty.text(ty, 2n, Ty.fit(ty, 2n, String.join(String.split(U.Qu.label(Ty.loc(ty), msg, text), '\n'), " "), tw) ++ tail, D.Clip.all(), (x0 + 24 + Ty.width(ty, 2n, tag) : U32), y, 30, T.Th.dim())])),
        Lay.buttons(ty, bs, (y + 1 : U32), (x1 - 4 : U32), x0))

def Lay.qrows(qs: List<&2, M.QItem>, +ty: Ty, +i: Nat, +n: Nat, +run: Bool, +more: Nat, +x0: U32, +x1: U32, +y: U32) -> Out:
  match qs:
    case Nil{}:
      Out.none()
    case q <> t:
      Out.cat(Lay.qrow(ty, q, Nat.is_eq(i, 0n), run, Bool.pick(Nat, Nat.is_eq(1n+i, n), more, 0n), x0, x1, y),
        Lay.qrows(t, ty, 1n+i, n, run, more, x0, x1, (y + 32 : U32)))

# how many queued rows are shown
def Lay.qn(+qs: List<&2, M.QItem>) -> Nat:
  Bool.pick(Nat, Nat.is_gt(List.length(&2, M.QItem, qs), U.Qu.shown()), U.Qu.shown(), List.length(&2, M.QItem, qs))

# The todo list above the queue: its head, then its rows (client.bend's
# Todos.rows: the steps around the one being done and a line for the rest,
# every step as many as fit, or none shut), then a long list's last line
# ("Show all 24", "Show fewer"). The head and that line are clicks (fold).
def Lay.tdh(+ts: List<&2, TD.Todo>, +rs: List<&2, TD.TRow>, +more: String) -> U32:
  Bool.pick(U32, List.is_empty(&2, TD.Todo, ts), 0,
    ((U32.from_nat(List.length(&2, TD.TRow, rs)) * 22 : U32) + 34 + Bool.pick(U32, String.is_empty(more), 0, 22) : U32))

def Lay.tdlines(rs: List<&2, TD.TRow>, +ty: Ty, +x0: U32, +x1: U32, +y: U32) -> List<&2, R.Op>:
  match rs:
    case Nil{}:
      Nil{}
    case +r <> rest:
      List.append(&2, R.Op, Ty.text(ty, 2n, Ty.fit(ty, 2n, TD.TRow.text(r), (x1 - x0 - 24 : U32)), D.Clip.all(), (x0 + 12 : U32), y, 22,
          Bool.pick(U32, String.eq(TD.TRow.status(r), "completed"), T.Th.faint(), Bool.pick(U32, String.eq(TD.TRow.status(r), "in_progress"), T.Th.text(), T.Th.dim()))),
        Lay.tdlines(rest, ty, x0, x1, (y + 22 : U32)))

# how many steps the window shows of every step: what half its height holds
def Lay.tdcap(+h: U32) -> Nat:
  U32.to_nat(U32.max((U32.shr(h) / 22 : U32), 6))

def Lay.todos.k(some: Bool, +ty: Ty, +ts: List<&2, TD.Todo>, +rs: List<&2, TD.TRow>, +more: String, +open: Bool, +all: Bool, +th: String, +off: Nat, +n: Nat, +x0: U32, +x1: U32, +bottom: U32) -> Out:
  match some:
    case True{}:
      +hh = Lay.tdh(ts, rs, more)
      +top = {(bottom - hh : U32) : U32}
      +ry = {(top + 28 + (U32.from_nat(List.length(&2, TD.TRow, rs)) * 22 : U32) : U32) : U32}
      Out.all([
        Out.ops(List.concat(&2, R.Op, [Lay.box(D.Clip.all(), x0, top, (x1 - x0 : U32), (hh - 4 : U32), 0, T.Th.raise(), T.Th.line()),
          Ty.text(ty, 2n, TD.Todo.head(ts) ++ Pane.range.loc(Ty.loc(ty), off, List.length(&2, TD.TRow, rs), n), D.Clip.all(), (x0 + 12 : U32), (top + 2 : U32), 26, T.Th.soft()),
          Lay.icon(ty, Bool.pick(String, open, "chevron-down", "chevron-right"), D.Clip.all(), (x1 - 28 : U32), (top + 6 : U32), T.Th.faint()),
          Lay.tdlines(rs, ty, x0, x1, (top + 28 : U32)),
          Ty.text(ty, 2n, more, D.Clip.all(), (x0 + 12 : U32), ry, 22, T.Th.accent())])),
        Out{Nil{}, List.concat(&2, S.Hit, [[Lay.hit(x0, top, (x1 - x0 : U32), hh, "pane", "todos:" ++ th ++ "|" ++ Nat.show(Nat.sub(n, 1n)))],
          [Lay.hit(x0, top, (x1 - x0 : U32), 28, "fold", U.Todos.toggle(open, th))],
          Bool.pick(List<&2, S.Hit>, String.is_empty(more), Nil{}, [Lay.hit(x0, ry, (x1 - x0 : U32), 22, "fold", U.Todos.more.ask(all, th))])])}])
    case False{}:
      Out.none()

def Lay.todos(+ty: Ty, +ts: List<&2, TD.Todo>, +rs: List<&2, TD.TRow>, +more: String, +open: Bool, +all: Bool, +th: String, +off: Nat, +n: Nat, +x0: U32, +x1: U32, +bottom: U32) -> Out:
  Lay.todos.k(Bool.not(List.is_empty(&2, TD.Todo, ts)), ty, ts, rs, more, open, all, th, off, n, x0, x1, bottom)

# the queue's height above the composer
def Lay.qh(+qs: List<&2, M.QItem>) -> U32:
  (U32.from_nat(Lay.qn(qs)) * 32 : U32)

# the thread's waiting messages above the composer, next first, each with
# its buttons (U.Qu.buttons); the rows end where the composer's box starts
def Lay.queued(+ty: Ty, +qs: List<&2, M.QItem>, +run: Bool, +x0: U32, +x1: U32, +h: U32) -> Out:
  Lay.qrows(List.take(&2, M.QItem, qs, Lay.qn(qs)), ty, 0n, Lay.qn(qs), run, Nat.sub(List.length(&2, M.QItem, qs), Lay.qn(qs)), x0, x1, (h - 124 - Lay.qh(qs) : U32))

def Lay.empty(+ty: Ty, +x0: U32, +x1: U32, +h: U32, +n: Nat) -> Out:
  +msg = Bool.pick(String, Nat.is_eq(n, 0n), MSG.T.empty.add(Ty.loc(ty)), MSG.T.empty.thread(Ty.loc(ty)))
  Out.ops(Ty.text(ty, 3n, msg, D.Clip.all(), (x0 + U32.shr((x1 - x0 - Ty.width(ty, 3n, msg) : U32)) : U32), (U32.shr(h) - 12 : U32), 23, T.Th.faint()))

# Health notice: square outline, wrapped text, shared actions.
def Kn.h(n: Maybe<&2, KH.KNote>, +ty: Ty, +w: U32) -> U32:
  match n:
    case None{}: 0
    case Some{KH.KNote{_, title, body, _}}:
      (70 + U32.from_nat(List.length(&2, String, Ty.wrap(ty, 1n, title, U32.dim(w, 64)))) * 24 + U32.from_nat(List.length(&2, String, Ty.wrap(ty, 2n, body, U32.dim(w, 32)))) * 20 : U32)

def Kn.lines(ls: List<&2, String>, +ty: Ty, +face: Nat, +x: U32, +y: U32, +lh: U32, +color: U32) -> List<&2, R.Op>:
  match ls:
    case Nil{}: Nil{}
    case l <> t: List.append(&2, R.Op, Ty.text(ty, face, l, D.Clip.all(), x, y, lh, color), Kn.lines(t, ty, face, x, (y + lh : U32), lh, color))

def Kn.button(+ty: Ty, +x: U32, +y: U32, +label: String, action: String, value: String) -> Out:
  +w = Lay.button.w(ty, label)
  Out{List.append(&2, R.Op, Lay.box(D.Clip.all(), x, y, w, 28, 0, T.Th.raise(), T.Th.line()),
    Ty.text(ty, 2n, label, D.Clip.all(), (x + 10 : U32), y, 28, T.Th.text())), [Lay.hit(x, y, w, 28, action, value)]}

def Kn.acts(as: List<&2, KH.KAct>, +ty: Ty, +x: U32, +y: U32) -> Out:
  match as:
    case Nil{}: Out.none()
    case KH.KAct{+l, a, v} <> t:
      Out.cat(Kn.button(ty, x, y, l, U.KN.action(a), v), Kn.acts(t, ty, (x + Lay.button.w(ty, l) + 8 : U32), y))

def Kn.close(on: Bool, +key: String, +ty: Ty, +x: U32, +y: U32) -> Out:
  match on:
    case True{}: Kn.button(ty, x, y, "×", "setting", "kicad.notice.hidden=" ++ key)
    case False{}: Out.none()

def Kn.draw(n: Maybe<&2, KH.KNote>, dismiss: Bool, +ty: Ty, +x: U32, +y: U32, +w: U32) -> Out:
  match n:
    case None{}: Out.none()
    case Some{KH.KNote{+key, +title, +body, +acts}}:
      +h = Kn.h(Some{KH.KNote{key, title, body, acts}}, ty, w)
      +ts = Ty.wrap(ty, 1n, title, U32.dim(w, 64))
      +bs = Ty.wrap(ty, 2n, body, U32.dim(w, 32))
      +by = {(y + 12 + U32.from_nat(List.length(&2, String, ts)) * 24 : U32) : U32}
      Out.all([Out.ops(List.concat(&2, R.Op, [Lay.box(D.Clip.all(), x, y, w, h, 0, T.Th.panel(), T.Th.warn()),
        Kn.lines(ts, ty, 1n, (x + 16 : U32), (y + 10 : U32), 24, T.Th.text()), Kn.lines(bs, ty, 2n, (x + 16 : U32), by, 20, T.Th.dim())])),
        Kn.acts(acts, ty, (x + 16 : U32), (y + h - 42 : U32)), Kn.close(dismiss, key, ty, (x + w - 42 : U32), (y + 6 : U32))])

def Kn.path(+ty: Ty, +n: S.Nui, +x: U32, +y: U32, +w: U32) -> Out:
  Lay.field(ty, D.Clip.all(), x, y, w, U.Ui.scratch(S.Nui.ui(n), "@f.kpath"), String.eq(S.Nui.focus_of(n), S.Fld.id("kpath", "setting", "kicad.path=")), S.Blink.on(n), S.Nui.caret_of(n), S.Nui.mark(n),
    M.Setting.get(U.Ui.state(S.Nui.ui(n)), "kicad.path", MSG.T.settings.kicad.automatic(Ty.loc(ty), U.Ui.info(S.Nui.ui(n), "kicad.cli"))), S.Fld.id("kpath", "setting", "kicad.path="))

# Settings
# --------

# the width a pair of chips takes on a row's right, never under the
# English floor (so English rows stay as they were)
def Set.pairw(+ty: Ty, +a: String, +b: String, +floor: U32) -> U32:
  U32.max(floor, (Lay.button.w(ty, a) + Lay.button.w(ty, b) + 22 : U32))

def Set.head(+ty: Ty, +label: String, +x: U32, +y: U32) -> Out:
  Out.ops(Ty.text(ty, 1n, label, D.Clip.all(), x, y, 28, T.Th.text()))

# a row whose right `rw` pixels hold controls
def Set.row.r(+ty: Ty, +label: String, +note: String, +x: U32, +y: U32, +w: U32, +rw: U32) -> Out:
  Out.ops(List.append(&2, R.Op, Ty.text(ty, 0n, Ty.fit(ty, 0n, label, U32.dim(w, (32 + rw : U32))), D.Clip.all(), (x + 16 : U32), y, 28, T.Th.soft()),
    Ty.text(ty, 2n, Ty.fit(ty, 2n, note, U32.dim(w, (32 + rw : U32))), D.Clip.all(), (x + 16 : U32), (y + 24 : U32), 20, T.Th.faint())))

def Set.row(+ty: Ty, +label: String, +note: String, +x: U32, +y: U32, +w: U32) -> Out:
  Set.row.r(ty, label, note, x, y, w, 0)

def Set.card(+x: U32, +y: U32, +w: U32, +h: U32) -> Out:
  Out.ops(Lay.box(D.Clip.all(), x, y, w, h, 10, T.Th.panel(), T.Th.line()))

def Set.days(+ty: Ty, +d: String, +x: U32, +y: U32) -> Out:
  +n = {Maybe.default(&2, U32, U32.read(d), 3) : U32}
  Out.all([
    Lay.button(ty, D.Clip.all(), x, y, "−", T.Th.soft(), "setting", "settle.days=" ++ U32.show(U32.max((n - 1 : U32), 1))),
    Out.ops(Ty.text(ty, 0n, MSG.T.settings.settle.days(Ty.loc(ty), U32.show(n), Bool.pick(String, U32.is_eq(n, 1), "one", "other")), D.Clip.all(), (x + 44 : U32), y, 28, T.Th.text())),
    Lay.button(ty, D.Clip.all(), (x + 116 : U32), y, "+", T.Th.soft(), "setting", "settle.days=" ++ U32.show(U32.min((n + 1 : U32), 90)))])

# what the chosen model is ("opus: Opus 5.5 · claude-opus-5-5")
def Set.model_line.loc(+l: I.Loc, +label: String, +m: String) -> String:
  Bool.pick(String, String.is_empty(m), MSG.T.settings.model.default(l, label), m ++ Bool.pick(String, String.is_empty(label), "", ": " ++ label))

def Set.model_line(+label: String, +m: String) -> String:
  Set.model_line.loc(I.En{}, label, m)

def Set.cli.loc(+l: I.Loc, +v: String, +prov: String) -> String:
  Bool.pick(String, String.is_empty(v), MSG.T.settings.provider.missing(l, prov), v)

def Set.cli(+v: String, +prov: String) -> String:
  Set.cli.loc(I.En{}, v, prov)

def Set.label.loc(+l: I.Loc, +v: String) -> String:
  Bool.pick(String, String.is_empty(v), MSG.T.common.default(l), v)

def Set.label(+v: String) -> String:
  Set.label.loc(I.En{}, v)

def Set.wraps(+ty: Ty, +v: String, +x0: U32, +x1: U32, +x: U32) -> Bool:
  Bool.and(U32.is_gt(x, x0), U32.is_gt((x + Lay.button.w(ty, Set.label.loc(Ty.loc(ty), v)) : U32), x1))

# chips from (x, y), wrapping at x1 back to x0 on the next row
def Set.chips(+ty: Ty, xs: List<&2, String>, +cur: String, +key: String, +x0: U32, +x1: U32, +x: U32, +y: U32) -> Out:
  match xs:
    case Nil{}:
      Out.none()
    case +v <> t:
      +label = Set.label.loc(Ty.loc(ty), v)
      +wrap = Bool.and(U32.is_gt(x, x0), U32.is_gt((x + Lay.button.w(ty, label) : U32), x1))
      +cx = {Bool.pick(U32, wrap, x0, x) : U32}
      +cy = {Bool.pick(U32, wrap, (y + 34 : U32), y) : U32}
      Out.cat(Lay.chip(ty, cx, cy, label, String.eq(v, cur), "setting", key ++ "=" ++ v),
        Set.chips(ty, t, cur, key, x0, x1, (cx + Lay.button.w(ty, label) + 8 : U32), cy))

# how many extra rows the chips take
def Set.rows(+ty: Ty, xs: List<&2, String>, +x0: U32, +x1: U32, +x: U32) -> U32:
  match xs:
    case Nil{}:
      0
    case +v <> t:
      +label = Set.label.loc(Ty.loc(ty), v)
      +wrap = Bool.and(U32.is_gt(x, x0), U32.is_gt((x + Lay.button.w(ty, label) : U32), x1))
      +cx = {Bool.pick(U32, wrap, x0, x) : U32}
      (Bool.pick(U32, wrap, 1, 0) + Set.rows(ty, t, x0, x1, (cx + Lay.button.w(ty, label) + 8 : U32)) : U32)

def Set.fork(+ty: Ty, +fork: String, +x: U32, +y: U32) -> Out:
  Bool.pick(Out, String.is_empty(fork),
    Lay.button(ty, D.Clip.all(), x, y, MSG.T.settings.kicad.install(Ty.loc(ty)), T.Th.accent(), "kicad-install", ""),
    Out.ops(Ty.text(ty, 2n, MSG.T.settings.kicad.available(Ty.loc(ty), fork), D.Clip.all(), x, y, 28, T.Th.ok())))

# The settings screen, in place of the thread. Taller than the window, it
# scrolls ("@set.sc", clamped here); the header stays on top.
# a button or chip: its label, and the action and value it sends
type VBtn is Data:
  VBtn{label: String, action: String, value: String}

# Voice in Settings: the OpenAI key (only ever shown masked), the
# microphone, the dictionary (its terms as chips, × takes one out), and
# what a Pebble ring needs to send
def Set.key.note.loc(+l: I.Loc, +masked: String) -> String:
  Bool.pick(String, String.is_empty(masked), MSG.T.settings.voice.key.note(l),
    MSG.T.settings.voice.key.saved(l, masked))

# chips from (x, y), wrapping at x1 back to x0 on the next row; the one
# whose value is `on` is lit

def Set.key.note(+masked: String) -> String:
  Set.key.note.loc(I.En{}, masked)

def Vx.chips(bs: List<&2, VBtn>, +ty: Ty, +on: String, +x0: U32, +x1: U32, +x: U32, +y: U32) -> Out:
  match bs:
    case Nil{}:
      Out.none()
    case VBtn{+l, +a, +v} <> t:
      +cx = {Bool.pick(U32, Set.wraps(ty, l, x0, x1, x), x0, x) : U32}
      +cy = {Bool.pick(U32, Set.wraps(ty, l, x0, x1, x), (y + 34 : U32), y) : U32}
      Out.cat(Lay.chip(ty, cx, cy, l, String.eq(v, on), a, v), Vx.chips(t, ty, on, x0, x1, (cx + Lay.button.w(ty, l) + 8 : U32), cy))

def Vx.chips.rows(bs: List<&2, VBtn>, +ty: Ty, +x0: U32, +x1: U32, +x: U32) -> U32:
  match bs:
    case Nil{}:
      0
    case VBtn{+l, _, _} <> t:
      +cx = {Bool.pick(U32, Set.wraps(ty, l, x0, x1, x), x0, x) : U32}
      (Bool.pick(U32, Set.wraps(ty, l, x0, x1, x), 1, 0) + Vx.chips.rows(t, ty, x0, x1, (cx + Lay.button.w(ty, l) + 8 : U32)) : U32)

def Vx.dev.btns(ds: List<&2, U.VDev>) -> List<&2, VBtn>:
  match ds:
    case Nil{}:
      Nil{}
    case U.VDev{+name, +label} <> t:
      VBtn{label, "setting", "voice.device=" ++ name} <> Vx.dev.btns(t)

# the microphones as chips, the default first
def Vx.devs.loc(+l: I.Loc, +u: U.Ui) -> List<&2, VBtn>:
  VBtn{MSG.T.voice.default(l), "setting", "voice.device="} <> Vx.dev.btns(U.Voice.devices(u))

def Vx.devs(+u: U.Ui) -> List<&2, VBtn>:
  Vx.devs.loc(I.En{}, u)

def Vx.term.btns(ts: List<&2, String>) -> List<&2, VBtn>:
  match ts:
    case Nil{}:
      Nil{}
    case +t <> r:
      VBtn{t ++ "  ×", "voice-unterm", t} <> Vx.term.btns(r)

def Set.voice(+ty: Ty, +st: M.State, +n: S.Nui, +x: U32, +w: U32, +y: U32) -> Out:
  +label0 = MSG.T.common.save(Ty.loc(ty))
  +label1 = MSG.T.queue.remove(Ty.loc(ty))
  +label2 = MSG.T.common.add(Ty.loc(ty))
  +u = S.Nui.ui(n)
  +kid = S.Fld.id("vkey", "voice-key", "")
  +tid = S.Fld.id("vterm", "voice-term", "")
  +devs = Vx.devs.loc(Ty.loc(ty), u)
  +terms = Vx.term.btns(U.Voice.dict(u))
  +dr = {(Vx.chips.rows(devs, ty, (x + 16 : U32), (x + w - 16 : U32), (x + 16 : U32)) * 34 : U32) : U32}
  +tr = {(Vx.chips.rows(terms, ty, (x + 16 : U32), (x + w - 16 : U32), (x + 16 : U32)) * 34 : U32) : U32}
  +sw = Lay.button.w(ty, label0)
  +rw = Lay.button.w(ty, label1)
  +aw = Lay.button.w(ty, label2)
  +dev = M.Setting.get(st, "voice.device", "")
  Out.all([
    Set.head(ty, MSG.T.settings.voice(Ty.loc(ty)), x, y),
    Set.card(x, (y + 32 : U32), w, (334 + dr + tr : U32)),
    Set.row(ty, MSG.T.settings.voice.key(Ty.loc(ty)), Set.key.note.loc(Ty.loc(ty), U.Ui.scratch(u, "@voice.key")), x, (y + 34 : U32), w),
    Lay.field(ty, D.Clip.all(), (x + 16 : U32), (y + 84 : U32), (w - 44 - sw - rw : U32), U.Ui.scratch(u, "@f.vkey"), String.eq(S.Nui.focus_of(n), kid), S.Blink.on(n),
      S.Nui.caret_of(n), S.Nui.mark(n), "sk-…", kid),
    Lay.button(ty, D.Clip.all(), (x + w - 22 - sw - rw : U32), (y + 84 : U32), label0, T.Th.accent(), "voice-key", ""),
    Lay.button(ty, D.Clip.all(), (x + w - 16 - rw : U32), (y + 84 : U32), label1, T.Th.soft(), "voice-unkey", ""),
    Set.row(ty, MSG.T.settings.voice.microphone(Ty.loc(ty)), MSG.T.settings.voice.microphone.note(Ty.loc(ty), Bool.pick(String, String.is_empty(dev), MSG.T.settings.voice.default.mic(Ty.loc(ty), Bool.pick(String, String.is_empty(U.Voice.default(u)), "none", "named"), U.Voice.label(u, U.Voice.default(u))), U.Voice.label(u, dev))), x, (y + 120 : U32), w),
    Vx.chips(devs, ty, "voice.device=" ++ dev, (x + 16 : U32), (x + w - 16 : U32), (x + 16 : U32), (y + 168 : U32)),
    Set.row(ty, MSG.T.settings.voice.dictionary(Ty.loc(ty)), MSG.T.settings.voice.dictionary.note(Ty.loc(ty)), x, (y + 206 + dr : U32), w),
    Vx.chips(terms, ty, "", (x + 16 : U32), (x + w - 16 : U32), (x + 16 : U32), (y + 254 + dr : U32)),
    Lay.field(ty, D.Clip.all(), (x + 16 : U32), (y + 254 + dr + tr + Bool.pick(U32, List.is_empty(&2, VBtn, terms), 0, 34) : U32), (w - 38 - aw : U32), U.Ui.scratch(u, "@f.vterm"),
      String.eq(S.Nui.focus_of(n), tid), S.Blink.on(n), S.Nui.caret_of(n), S.Nui.mark(n), MSG.T.settings.voice.term(Ty.loc(ty)), tid),
    Lay.button(ty, D.Clip.all(), (x + w - 16 - aw : U32), (y + 254 + dr + tr + Bool.pick(U32, List.is_empty(&2, VBtn, terms), 0, 34) : U32), label2, T.Th.soft(), "voice-term", ""),
    Out.ops(Ty.text(ty, 2n, Ty.fit(ty, 2n, MSG.T.settings.voice.pebble(Ty.loc(ty)), (w - 32 : U32)),
      D.Clip.all(), (x + 16 : U32), (y + 328 + dr + tr : U32), 24, T.Th.faint()))])

# the "restart.continue" row: on or off
def Set.carry(+ty: Ty, +on: Bool, +x: U32, +w: U32, +y: U32) -> Out:
  +label0 = MSG.T.settings.lang.off(Ty.loc(ty))
  +label1 = MSG.T.settings.lang.on(Ty.loc(ty))
  Out.all([
    Set.row.r(ty, MSG.T.settings.restart(Ty.loc(ty)), MSG.T.settings.restart.note(Ty.loc(ty)), x, y, w, Set.pairw(ty, label0, label1, 150)),
    Lay.chip(ty, (x + w - 16 - Lay.button.w(ty, label0) - 6 - Lay.button.w(ty, label1) : U32), (y + 14 : U32), label1, on, "setting", "restart.continue=on"),
    Lay.chip(ty, (x + w - 16 - Lay.button.w(ty, label0) : U32), (y + 14 : U32), label0, Bool.not(on), "setting", "restart.continue=off")])

def Set.voice.h(+ty: Ty, +n: S.Nui, +x: U32, +w: U32) -> U32:
  +u = S.Nui.ui(n)
  +devs = Vx.devs.loc(Ty.loc(ty), u)
  (386 + (Vx.chips.rows(devs, ty, (x + 16 : U32), (x + w - 16 : U32), (x + 16 : U32)) * 34 : U32)
    + (Vx.chips.rows(Vx.term.btns(U.Voice.dict(u)), ty, (x + 16 : U32), (x + w - 16 : U32), (x + 16 : U32)) * 34 : U32) : U32)

# API keys (client.bend's ApiKeys): the note, Add and View; the form (a
# name, the key shown as dots, Save) and the names each with Delete
def Set.keys.rows(ns: List<&2, String>, +ty: Ty, +x: U32, +w: U32, +y: U32) -> List<&2, Out>:
  match ns:
    case Nil{}:
      Nil{}
    case +k <> t:
      +label0 = MSG.T.common.delete(Ty.loc(ty))
      Out.all([Set.row.r(ty, k, "", x, y, w, (Lay.button.w(ty, label0) + 16 : U32)),
        Lay.button(ty, D.Clip.all(), (x + w - 16 - Lay.button.w(ty, label0) : U32), (y + 2 : U32), label0, T.Th.soft(), "key-del", k)]) <> Set.keys.rows(t, ty, x, w, (y + 40 : U32))

def Set.keys.list(open: Bool, +u: U.Ui, +ty: Ty, +x: U32, +w: U32, +y: U32) -> Out:
  match open:
    case True{}:
      Bool.pick(Out, List.is_empty(&2, String, U.ApiKeys.names(u)), Set.row(ty, MSG.T.settings.keys.empty(Ty.loc(ty)), "", x, y, w), Out.all(Set.keys.rows(U.ApiKeys.names(u), ty, x, w, y)))
    case False{}:
      Out.none()

def Set.keys.fields(+ty: Ty, +n: S.Nui, +x: U32, +w: U32, +y: U32) -> Out:
  +label0 = MSG.T.common.save(Ty.loc(ty))
  +u = S.Nui.ui(n)
  +nid = S.Fld.id("kname", "key-add", "")
  +vid = S.Fld.id("kval", "key-add", "")
  +sw = Lay.button.w(ty, label0)
  +nw = {U32.shr((w - 32 : U32)) : U32}
  Out.all([
    Out.ops(Ty.text(ty, 2n, Ty.fit(ty, 2n, U.ApiKeys.form.note(Ty.loc(ty)), (w - 32 : U32)), D.Clip.all(), (x + 16 : U32), y, 20, T.Th.faint())),
    Lay.field(ty, D.Clip.all(), (x + 16 : U32), (y + 26 : U32), (nw - 8 : U32), U.Ui.scratch(u, "@f.kname"), String.eq(S.Nui.focus_of(n), nid), S.Blink.on(n),
      S.Nui.caret_of(n), S.Nui.mark(n), MSG.T.bots.name(Ty.loc(ty)), nid),
    Lay.field(ty, D.Clip.all(), (x + 16 + nw : U32), (y + 26 : U32), (w - 32 - nw - 8 - sw : U32), U.ApiKeys.dots(U.Ui.scratch(u, "@f.kval")), String.eq(S.Nui.focus_of(n), vid), S.Blink.on(n),
      S.Nui.caret_of(n), S.Nui.mark(n), MSG.T.common.key(Ty.loc(ty)), vid),
    Lay.button(ty, D.Clip.all(), (x + w - 16 - sw : U32), (y + 26 : U32), label0, T.Th.accent(), "key-add", "")])

def Set.keys.form(open: Bool, +ty: Ty, +n: S.Nui, +x: U32, +w: U32, +y: U32) -> Out:
  match open:
    case True{}:
      Set.keys.fields(ty, n, x, w, y)
    case False{}:
      Out.none()

def Set.keys.fh(open: Bool) -> U32:
  Bool.pick(U32, open, 72, 0)

def Set.keys.lh(open: Bool, +u: U.Ui) -> U32:
  Bool.pick(U32, open, (U32.from_nat(Bool.pick(Nat, List.is_empty(&2, String, U.ApiKeys.names(u)), 1n, List.length(&2, String, U.ApiKeys.names(u)))) * 40 : U32), 0)

# the card's height
def Set.keys.h(+n: S.Nui) -> U32:
  (70 + Set.keys.fh(U.Ui.flag(S.Nui.ui(n), "keyform")) + Set.keys.lh(U.Ui.flag(S.Nui.ui(n), "keys"), S.Nui.ui(n)) : U32)

def Set.keys(+ty: Ty, +n: S.Nui, +x: U32, +w: U32, +y: U32) -> Out:
  +label0 = MSG.T.settings.keys.add(Ty.loc(ty))
  +label1 = MSG.T.settings.keys.view(Ty.loc(ty))
  +label2 = MSG.T.settings.keys(Ty.loc(ty))
  +u = S.Nui.ui(n)
  +aw = Lay.button.w(ty, label0)
  +vw = Lay.button.w(ty, label1)
  +fh = Set.keys.fh(U.Ui.flag(u, "keyform"))
  Out.all([
    Set.head(ty, label2, x, y),
    Set.card(x, (y + 32 : U32), w, (Set.keys.h(n) - 10 : U32)),
    Set.row.r(ty, label2, U.ApiKeys.note.loc(Ty.loc(ty), u), x, (y + 34 : U32), w, (aw + vw + 22 : U32)),
    Lay.chip(ty, (x + w - 16 - vw - 6 - aw : U32), (y + 44 : U32), label0, U.Ui.flag(u, "keyform"), "flag", "keyform"),
    Lay.chip(ty, (x + w - 16 - vw : U32), (y + 44 : U32), label1, U.Ui.flag(u, "keys"), "flag", "keys"),
    Set.keys.form(U.Ui.flag(u, "keyform"), ty, n, x, w, (y + 88 : U32)),
    Set.keys.list(U.Ui.flag(u, "keys"), u, ty, x, w, (y + 88 + fh : U32))])

# Time zone: where the person is (a phone reports it, or they set it), with
# a field for a zone name, Set, and Automatic
# Usage (src/core/usage.bend): a card of rows, a label and a note each; a
# row with an action has its button (Refresh) on the right
def Set.usage.row.k(none: Bool, +ty: Ty, +r: UZ.URow, +x: U32, +w: U32, +y: U32) -> Out:
  match none:
    case True{}:
      Set.row(ty, UZ.URow.label(r), UZ.URow.note(r), x, y, w)
    case False{}:
      +label0 = MSG.T.common.refresh(Ty.loc(ty))
      Out.all([Set.row.r(ty, UZ.URow.label(r), UZ.URow.note(r), x, y, w, (Lay.button.w(ty, label0) + 22 : U32)),
        Lay.button(ty, D.Clip.all(), (x + w - 16 - Lay.button.w(ty, label0) : U32), (y + 10 : U32), label0, T.Th.soft(), UZ.URow.act(r), "")])

def Set.usage.rows(rs: List<&2, UZ.URow>, +ty: Ty, +x: U32, +w: U32, +y: U32) -> Out:
  match rs:
    case Nil{}:
      Out.none()
    case +r <> t:
      Out.all([Set.usage.row.k(String.is_empty(UZ.URow.act(r)), ty, r, x, w, y), Set.usage.rows(t, ty, x, w, (y + 52 : U32))])

def Set.usage.h(+n: S.Nui) -> U32:
  (72 + U32.from_nat(List.length(&2, UZ.URow, U.Usage.rows(S.Nui.ui(n)))) * 52 : U32)

def Set.usage(+ty: Ty, +n: S.Nui, +x: U32, +w: U32, +y: U32) -> Out:
  Out.all([
    Set.head(ty, MSG.T.settings.usage(Ty.loc(ty)), x, y),
    Set.card(x, (y + 32 : U32), w, (Set.usage.h(n) - 56 : U32)),
    Set.usage.rows(U.Usage.rows(S.Nui.ui(n)), ty, x, w, (y + 34 : U32))])

def Set.tz.h() -> U32:
  150

def Set.tz(+ty: Ty, +n: S.Nui, +x: U32, +w: U32, +y: U32) -> Out:
  +label0 = MSG.T.settings.lang.auto(Ty.loc(ty))
  +label1 = MSG.T.common.set(Ty.loc(ty))
  +label2 = MSG.T.settings.zone(Ty.loc(ty))
  +u = S.Nui.ui(n)
  +id = S.Fld.id("tzname", "tz-set", "")
  +aw = Lay.button.w(ty, label0)
  +sw = Lay.button.w(ty, label1)
  Out.all([
    Set.head(ty, label2, x, y),
    Set.card(x, (y + 32 : U32), w, 108),
    Set.row.r(ty, label2, U.Tzs.line.loc(Ty.loc(ty), u), x, (y + 34 : U32), w, (aw + 22 : U32)),
    Bool.pick(Out, U.Tzs.auto(u), Lay.button(ty, D.Clip.all(), (x + w - 16 - aw : U32), (y + 44 : U32), label0, T.Th.soft(), "tz-auto", ""),
      Out.ops(Ty.text(ty, 2n, label0, D.Clip.all(), (x + w - 6 - aw : U32), (y + 44 : U32), 28, T.Th.faint()))),
    Lay.field(ty, D.Clip.all(), (x + 16 : U32), (y + 74 : U32), (w - 32 - sw - 8 : U32), U.Ui.scratch(u, "@f.tzname"), String.eq(S.Nui.focus_of(n), id), S.Blink.on(n),
      S.Nui.caret_of(n), S.Nui.mark(n), MSG.T.settings.zone.placeholder(Ty.loc(ty)), id),
    Lay.button(ty, D.Clip.all(), (x + w - 16 - sw : U32), (y + 74 : U32), label1, T.Th.accent(), "tz-set", ""),
    Out.ops(Ty.text(ty, 2n, Ty.fit(ty, 2n, U.Tzs.err(u), (w - 32 : U32)), D.Clip.all(), (x + 16 : U32), (y + 110 : U32), 20, T.Th.faint()))])

# the version row's button's words (U.Update.btn), measured for the row
def Set.update.label(i: Nat, +l: I.Loc) -> String:
  match i:
    case 1n:
      MSG.T.settings.update.install(l)
    case 2n:
      MSG.T.settings.update.check(l)
    case 3n:
      MSG.T.settings.update.now(l)
    case _:
      ""

# the version row's button (U.Update.btn)
def Set.update.btn(i: Nat, +ty: Ty, +x: U32, +w: U32, +y: U32) -> Out:
  match i:
    case 1n:
      +label0 = MSG.T.settings.update.install(Ty.loc(ty))
      Lay.button(ty, D.Clip.all(), (x + w - 16 - Lay.button.w(ty, label0) : U32), y, label0, T.Th.accent(), "update", "")
    case 2n:
      +label0 = MSG.T.settings.update.check(Ty.loc(ty))
      Lay.button(ty, D.Clip.all(), (x + w - 16 - Lay.button.w(ty, label0) : U32), y, label0, T.Th.soft(), "update-check", "")
    case 3n:
      +label0 = MSG.T.settings.update.now(Ty.loc(ty))
      Lay.button(ty, D.Clip.all(), (x + w - 16 - Lay.button.w(ty, label0) : U32), y, label0, T.Th.accent(), "update-now", "")
    case _:
      Out.none()

# The pairing code
# ----------------

# a row's dark runs as boxes of m pixels a module
def Pq.run(+on: Bool, +start: U32, +i: U32, +x: U32, +y: U32, +m: U32, +acc: List<&2, R.Op>) -> List<&2, R.Op>:
  Bool.pick(List<&2, R.Op>, Bool.and(on, U32.is_gt(i, start)), D.Draw.rect(D.Clip.all(), (x + (start * m : U32) : U32), y, ((i - start : U32) * m : U32), m, 0, 255) <> acc, acc)

def Pq.line(s: String, +i: U32, +start: U32, +on: Bool, +x: U32, +y: U32, +m: U32, +acc: List<&2, R.Op>) -> List<&2, R.Op>:
  match s:
    case SNil{}:
      Pq.run(on, start, i, x, y, m, acc)
    case SCon{Chr{+c}, t}:
      +d = {U32.is_eq(c, 49) : Bool}
      +same = {Bool.not(Bool.xor(d, on)) : Bool}
      Pq.line(t, (i + 1 : U32), Bool.pick(U32, same, start, i), d, x, y, m, Bool.pick(List<&2, R.Op>, same, acc, Pq.run(on, start, i, x, y, m, acc)))

def Pq.rows(rs: List<&2, String>, +x: U32, +y: U32, +m: U32, +acc: List<&2, R.Op>) -> List<&2, R.Op>:
  match rs:
    case Nil{}:
      acc
    case r <> t:
      Pq.rows(t, x, (y + m : U32), m, Pq.line(r, 0, 0, False{}, x, y, m, acc))

# the code on white, four modules of quiet zone around, over the settings;
# a click anywhere puts it away
def Pq.card(+ty: Ty, +rows: List<&2, String>, +x0: U32, +x1: U32, +h: U32) -> Out:
  +n = {U32.from_nat(List.length(&2, String, rows)) : U32}
  +room = {U32.dim(U32.min((x1 - x0 : U32), h), 140) : U32}
  +m = {U32.max(2, U32.min(10, (room / (n + 8 : U32) : U32))) : U32}
  +side = {((n + 8 : U32) * m : U32) : U32}
  +cx = {(x0 + U32.shr(U32.dim((x1 - x0 : U32), side)) : U32) : U32}
  +cy = {U32.shr(U32.dim(h, (side + 40 : U32))) : U32}
  +note = U.Pair.qr.note(Ty.loc(ty))
  Out{List.concat(&2, R.Op, [
      [D.Draw.rect(D.Clip.all(), x0, 0, (x1 - x0 : U32), h, T.Th.bg(), 235), D.Draw.rect(D.Clip.all(), cx, cy, side, side, 16777215, 255)],
      List.reverse(&2, R.Op, Pq.rows(rows, (cx + (m * 4 : U32) : U32), (cy + (m * 4 : U32) : U32), m, Nil{})),
      Ty.text(ty, 2n, note, D.Clip.all(), (x0 + U32.shr(U32.dim((x1 - x0 : U32), Ty.width(ty, 2n, note))) : U32), (cy + side + 12 : U32), 24, T.Th.dim())]),
    [Lay.hit(x0, 0, (x1 - x0 : U32), h, "flag", "pairqr")]}

def Pq.over(on: Bool, +ty: Ty, +info: Map<&2, String>, +x0: U32, +x1: U32, +h: U32) -> Out:
  match on:
    case True{}:
      Pq.card(ty, U.Pair.qr(info), x0, x1, h)
    case False{}:
      Out.none()

def Pq.on(+n: S.Nui) -> Bool:
  U.Ui.flag(S.Nui.ui(n), "pairqr")

# the pairing link's row: Copy, and QR code once the hub sends the code
def Set.pair(+ty: Ty, +info: Map<&2, String>, +on: Bool, +x: U32, +w: U32, +y: U32) -> Out:
  +label0 = MSG.T.common.copy(Ty.loc(ty))
  +label1 = MSG.T.settings.pair.code(Ty.loc(ty))
  +pair = U.Pair.link(info)
  +cw = {Lay.button.w(ty, label0) : U32}
  Out.all([
    Set.row.r(ty, MSG.T.settings.pair(Ty.loc(ty)), U.Pair.note(Ty.loc(ty), pair), x, y, w, Set.pairw(ty, label0, label1, 180)),
    Bool.pick(Out, String.is_empty(pair), Out.none(), Lay.button(ty, D.Clip.all(), (x + w - 16 - cw : U32), (y + 10 : U32), label0, T.Th.soft(), "copy", pair)),
    Bool.pick(Out, U.Pair.qr.ready(info), Lay.chip(ty, (x + w - 16 - cw - 6 - Lay.button.w(ty, label1) : U32), (y + 10 : U32), label1, on, "flag", "pairqr"), Out.none())])

# the first-run tour again (core/onboard.bend: action "ob-open")
# Your machines (U.Mate.rows): what the hub found of the owner's other
# machines, each with how its sync goes and On/Off ("sync.<name>")
def Set.mates.rows(rs: List<&2, U.MateRow>, +ty: Ty, +x: U32, +w: U32, +y: U32) -> List<&2, Out>:
  match rs:
    case Nil{}:
      Nil{}
    case U.MateRow{+nm, +note, +on, +key} <> t:
      +label0 = MSG.T.settings.lang.on(Ty.loc(ty))
      +label1 = MSG.T.settings.lang.off(Ty.loc(ty))
      Out.all([Set.row.r(ty, nm, note, x, y, w, (Lay.button.w(ty, label0) + Lay.button.w(ty, label1) + 22 : U32)),
        Lay.chip(ty, (x + w - 16 - Lay.button.w(ty, label1) - 6 - Lay.button.w(ty, label0) : U32), (y + 12 : U32), label0, String.eq(on, "on"), "setting", key ++ "=on"),
        Lay.chip(ty, (x + w - 16 - Lay.button.w(ty, label1) : U32), (y + 12 : U32), label1, String.eq(on, "off"), "setting", key ++ "=off")]) <> Set.mates.rows(t, ty, x, w, (y + 52 : U32))

# the section's height, its head included
def Set.mates.h(+n: S.Nui) -> U32:
  (32 + 66 + U32.from_nat(List.length(&2, U.MateRow, U.Mate.rows(S.Nui.ui(n)))) * 52 + 24 : U32)

def Set.mates(+ty: Ty, +n: S.Nui, +x: U32, +w: U32, +y: U32) -> Out:
  +u = S.Nui.ui(n)
  Out.all([
    Set.head(ty, U.Mate.title(Ty.loc(ty)), x, y),
    Set.card(x, (y + 32 : U32), w, (Set.mates.h(n) - 56 : U32)),
    Set.row(ty, MSG.T.settings.sync(Ty.loc(ty)), U.Mate.lead.loc(Ty.loc(ty), u), x, (y + 34 : U32), w),
    Out.all(Set.mates.rows(U.Mate.rows.loc(Ty.loc(ty), u), ty, x, w, (y + 86 : U32)))])

# Antigravity (client.bend's Agv.rows): each row its label, its note
# wrapped, its fields side by side (a secret shown as dots), its buttons
# wrapping; the card holds them all
def Set.agy.btns(bs: List<&2, U.AgBtn>, +ty: Ty, +x0: U32, +x1: U32, +x: U32, +y: U32) -> Out:
  match bs:
    case Nil{}:
      Out.none()
    case U.AgBtn{+l, +a, +v, on} <> t:
      +cx = {Bool.pick(U32, Set.wraps(ty, l, x0, x1, x), x0, x) : U32}
      +cy = {Bool.pick(U32, Set.wraps(ty, l, x0, x1, x), (y + 34 : U32), y) : U32}
      Out.cat(Lay.chip(ty, cx, cy, l, on, a, v), Set.agy.btns(t, ty, x0, x1, (cx + Lay.button.w(ty, l) + 8 : U32), cy))

def Set.agy.brows(bs: List<&2, U.AgBtn>, +ty: Ty, +x0: U32, +x1: U32, +x: U32) -> U32:
  match bs:
    case Nil{}:
      0
    case U.AgBtn{+l, _, _, _} <> t:
      +cx = {Bool.pick(U32, Set.wraps(ty, l, x0, x1, x), x0, x) : U32}
      (Bool.pick(U32, Set.wraps(ty, l, x0, x1, x), 1, 0) + Set.agy.brows(t, ty, x0, x1, (cx + Lay.button.w(ty, l) + 8 : U32)) : U32)

def Set.agy.field(+ty: Ty, +n: S.Nui, +f: U.AgField, +x: U32, +y: U32, +w: U32) -> Out:
  match f:
    case U.AgField{+nm, +h, sec, +v, +e}:
      +id = S.Fld.id(nm, e, "")
      Lay.field(ty, D.Clip.all(), x, y, w, Bool.pick(String, sec, U.ApiKeys.dots(v), v), String.eq(S.Nui.focus_of(n), id), S.Blink.on(n), S.Nui.caret_of(n), S.Nui.mark(n), h, id)

def Set.agy.fields(fs: List<&2, U.AgField>, +ty: Ty, +n: S.Nui, +x: U32, +y: U32, +fw: U32) -> Out:
  match fs:
    case Nil{}:
      Out.none()
    case +f <> t:
      Out.cat(Set.agy.field(ty, n, f, x, y, fw), Set.agy.fields(t, ty, n, (x + fw + 8 : U32), y, fw))

def Set.agy.lines(ls: List<&2, String>, +ty: Ty, +x: U32, +y: U32) -> Out:
  match ls:
    case Nil{}:
      Out.none()
    case +l <> t:
      Out.cat(Out.ops(Ty.text(ty, 2n, l, D.Clip.all(), x, y, 20, T.Th.faint())), Set.agy.lines(t, ty, x, (y + 20 : U32)))

def Set.agy.fh(fs: List<&2, U.AgField>) -> U32:
  Bool.pick(U32, List.is_empty(&2, U.AgField, fs), 0, 36)

def Set.agy.bh(+bs: List<&2, U.AgBtn>, +ty: Ty, +x: U32, +w: U32) -> U32:
  Bool.pick(U32, List.is_empty(&2, U.AgBtn, bs), 0, (34 + (Set.agy.brows(bs, ty, (x + 16 : U32), (x + w - 16 : U32), (x + 16 : U32)) * 34 : U32) : U32))

def Set.agy.nh(+ty: Ty, +note: String, +w: U32) -> U32:
  (U32.from_nat(List.length(&2, String, Ty.wrap(ty, 2n, note, (w - 32 : U32)))) * 20 : U32)

# a row's height
def Set.agy.rh(r: U.AgRow, +ty: Ty, +x: U32, +w: U32) -> U32:
  match r:
    case U.AgRow{_, +nt, fs, bs}:
      (36 + Set.agy.nh(ty, nt, w) + Set.agy.fh(fs) + Set.agy.bh(bs, ty, x, w) : U32)

def Set.agy.fw(+fs: List<&2, U.AgField>, +w: U32) -> U32:
  ((w - 32 - (U32.from_nat(Nat.sub(List.length(&2, U.AgField, fs), 1n)) * 8 : U32) : U32) / U32.max(U32.from_nat(List.length(&2, U.AgField, fs)), 1) : U32)

def Set.agy.row(r: U.AgRow, +ty: Ty, +n: S.Nui, +x: U32, +w: U32, +y: U32) -> Out:
  match r:
    case U.AgRow{+l, +nt, +fs, bs}:
      +ny = {(y + 26 : U32) : U32}
      +fy = {(ny + Set.agy.nh(ty, nt, w) + 4 : U32) : U32}
      +by = {(fy + Set.agy.fh(fs) : U32) : U32}
      Out.all([
        Out.ops(Ty.text(ty, 0n, Ty.fit(ty, 0n, l, (w - 32 : U32)), D.Clip.all(), (x + 16 : U32), y, 28, T.Th.soft())),
        Set.agy.lines(Ty.wrap(ty, 2n, nt, (w - 32 : U32)), ty, (x + 16 : U32), ny),
        Set.agy.fields(fs, ty, n, (x + 16 : U32), fy, Set.agy.fw(fs, w)),
        Set.agy.btns(bs, ty, (x + 16 : U32), (x + w - 16 : U32), (x + 16 : U32), by)])

def Set.agy.rows(rs: List<&2, U.AgRow>, +ty: Ty, +n: S.Nui, +x: U32, +w: U32, +y: U32) -> Out:
  match rs:
    case Nil{}:
      Out.none()
    case +r <> t:
      Out.cat(Set.agy.row(r, ty, n, x, w, y), Set.agy.rows(t, ty, n, x, w, (y + Set.agy.rh(r, ty, x, w) : U32)))

def Set.agy.sum(rs: List<&2, U.AgRow>, +ty: Ty, +x: U32, +w: U32) -> U32:
  match rs:
    case Nil{}:
      0
    case +r <> t:
      (Set.agy.rh(r, ty, x, w) + Set.agy.sum(t, ty, x, w) : U32)

# the section's height, its head included
def Set.agy.h(+ty: Ty, +n: S.Nui, +x: U32, +w: U32) -> U32:
  (52 + Set.agy.sum(U.Agv.rows.loc(Ty.loc(ty), S.Nui.ui(n)), ty, x, w) : U32)

def Set.agy(+ty: Ty, +n: S.Nui, +x: U32, +w: U32, +y: U32) -> Out:
  +rs = U.Agv.rows.loc(Ty.loc(ty), S.Nui.ui(n))
  Out.all([
    Set.head(ty, "Antigravity", x, y),
    Set.card(x, (y + 32 : U32), w, (Set.agy.sum(rs, ty, x, w) + 10 : U32)),
    Set.agy.rows(rs, ty, n, x, w, (y + 38 : U32))])

def Set.tour(+ty: Ty, +x: U32, +w: U32, +y: U32) -> Out:
  +label0 = MSG.T.settings.tour.show(Ty.loc(ty))
  +bw = Lay.button.w(ty, label0)
  Out.all([
    Set.head(ty, MSG.T.settings.tour(Ty.loc(ty)), x, y),
    Set.card(x, (y + 32 : U32), w, 66),
    Set.row.r(ty, MSG.T.settings.tour.welcome(Ty.loc(ty)), MSG.T.settings.tour.note(Ty.loc(ty)), x, (y + 34 : U32), w, (bw + 22 : U32)),
    Lay.button(ty, D.Clip.all(), (x + w - 16 - bw : U32), (y + 44 : U32), label0, T.Th.soft(), "ob-open", "")])

# Notify me for: wrap the notes and use the same heights for drawing and scrolling.
def Set.notify.rh(r: U.NotifyRow, +ty: Ty, +w: U32) -> U32:
  match r:
    case U.NotifyRow{_, note, _, _}:
      (32 + Set.agy.nh(ty, note, w) : U32)

def Set.notify.row(r: U.NotifyRow, +ty: Ty, +x: U32, +w: U32, +y: U32) -> Out:
  match r:
    case U.NotifyRow{nm, note, +on, +key}:
      +label0 = MSG.T.settings.lang.on(Ty.loc(ty))
      +label1 = MSG.T.settings.lang.off(Ty.loc(ty))
      Out.all([
        Set.row.r(ty, nm, "", x, y, w, (Lay.button.w(ty, label0) + Lay.button.w(ty, label1) + 22 : U32)),
        Lay.chip(ty, (x + w - 16 - Lay.button.w(ty, label1) - 6 - Lay.button.w(ty, label0) : U32), (y + 2 : U32), label0, String.eq(on, "on"), "setting", key ++ "=on"),
        Lay.chip(ty, (x + w - 16 - Lay.button.w(ty, label1) : U32), (y + 2 : U32), label1, String.eq(on, "off"), "setting", key ++ "=off"),
        Set.agy.lines(Ty.wrap(ty, 2n, note, (w - 32 : U32)), ty, (x + 16 : U32), (y + 26 : U32))])

def Set.notify.rows(rs: List<&2, U.NotifyRow>, +ty: Ty, +x: U32, +w: U32, +y: U32) -> Out:
  match rs:
    case Nil{}:
      Out.none()
    case +r <> t:
      Out.cat(Set.notify.row(r, ty, x, w, y), Set.notify.rows(t, ty, x, w, (y + Set.notify.rh(r, ty, w) : U32)))

def Set.notify.sum(rs: List<&2, U.NotifyRow>, +ty: Ty, +w: U32) -> U32:
  match rs:
    case Nil{}:
      0
    case r <> t:
      (Set.notify.rh(r, ty, w) + Set.notify.sum(t, ty, w) : U32)

def Set.notify.h(+st: M.State, +ty: Ty, +w: U32) -> U32:
  (90 + Set.agy.nh(ty, U.Notify.lead(Ty.loc(ty)), w) + Set.notify.sum(U.Notify.rows.loc(Ty.loc(ty), st), ty, w) : U32)

def Set.notify(+st: M.State, +ty: Ty, +x: U32, +w: U32, +y: U32) -> Out:
  Out.all([
    Set.head(ty, U.Notify.title(Ty.loc(ty)), x, y),
    Set.card(x, (y + 32 : U32), w, (Set.notify.h(st, ty, w) - 56 : U32)),
    Set.agy.lines(Ty.wrap(ty, 2n, U.Notify.lead(Ty.loc(ty)), (w - 32 : U32)), ty, (x + 16 : U32), (y + 38 : U32)),
    Set.notify.rows(U.Notify.rows.loc(Ty.loc(ty), st), ty, x, w, (y + 50 + Set.agy.nh(ty, U.Notify.lead(Ty.loc(ty)), w) : U32))])

def Set.lang.chips(vs: List<&2, String>, +ty: Ty, +u: U.Ui, +x: U32, +y: U32) -> Out:
  match vs:
    case Nil{}: Out.none()
    case +v <> t:
      +label = U.Lang.label(Ty.loc(ty), v)
      Out.all([Lay.chip(ty, x, y, label, String.eq(U.Lang.choice(u), v), "lang", v),
        Set.lang.chips(t, ty, u, (x + Lay.button.w(ty, label) + 6 : U32), y)])

# the chips' width with a 6 px gap after each (the caller drops the last)
def Set.lang.w(vs: List<&2, String>, +ty: Ty, +u: U.Ui) -> U32:
  match vs:
    case Nil{}: 0
    case +v <> t:
      (Lay.button.w(ty, U.Lang.label(Ty.loc(ty), v)) + 6 + Set.lang.w(t, ty, u) : U32)

# right-aligned to the card's padding like Theme's; the agent row is an
# On/Off pair like the notification rows
def Set.lang(+ty: Ty, +u: U.Ui, +x: U32, +w: U32, +y: U32) -> Out:
  +l = Ty.loc(ty)
  +cw = {U32.dim(Set.lang.w(U.Lang.offered(u), ty, u), 6) : U32}
  +on = U.Lang.agent.on(u)
  +onlabel = MSG.T.settings.lang.on(l)
  +offlabel = MSG.T.settings.lang.off(l)
  +ow = Lay.button.w(ty, onlabel)
  +fw = Lay.button.w(ty, offlabel)
  Out.all([
    Set.row.r(ty, MSG.T.settings.lang(l), MSG.T.settings.lang.note(l), x, y, w, (cw + 8 : U32)),
    Set.lang.chips(U.Lang.offered(u), ty, u, (x + w - 16 - cw : U32), (y + 14 : U32)),
    Set.row.r(ty, MSG.T.settings.agent.lang(l), MSG.T.settings.agent.lang.note(l), x, (y + 52 : U32), w, (ow + fw + 14 : U32)),
    Lay.chip(ty, (x + w - 16 - fw - 6 - ow : U32), (y + 66 : U32), onlabel, on, "setting", "agent.lang=" ++ U.Lang.agent.value(True{}, u)),
    Lay.chip(ty, (x + w - 16 - fw : U32), (y + 66 : U32), offlabel, Bool.not(on), "setting", "agent.lang=off")])

# IPC's controls use the same measured labels and right padding as Theme.
def Set.ipc(+ty: Ty, +ipc: String, +x: U32, +w: U32, +y: U32) -> Out:
  +on = MSG.T.settings.lang.on(Ty.loc(ty))
  +off = MSG.T.settings.lang.off(Ty.loc(ty))
  +ow = Lay.button.w(ty, on)
  +fw = Lay.button.w(ty, off)
  Out.all([
    Lay.chip(ty, (x + w - 16 - fw - 6 - ow : U32), y, on, String.eq(ipc, "on"), "setting", "kicad.ipc=on"),
    Lay.chip(ty, (x + w - 16 - fw : U32), y, off, String.eq(ipc, "off"), "setting", "kicad.ipc=off")])

# Settings' Ordering group (client.bend's Fab.set.*): Ordering on or off
# (order.enabled), then Price estimates, whose chips show with it on
def Set.order.est(on: Bool, +ty: Ty, +est: String, +x: U32, +w: U32, +y: U32) -> Out:
  match on:
    case True{}:
      Out.all([Lay.chip(ty, (x + w - 120 : U32), y, "On", String.eq(est, "on"), "setting", "order.estimates=on"),
        Lay.chip(ty, (x + w - 70 : U32), y, "Off", String.eq(est, "off"), "setting", "order.estimates=off")])
    case False{}:
      Out.none()

def Set.order(+ty: Ty, +st: M.State, +x: U32, +w: U32, +y: U32) -> Out:
  Out.all([Set.head(ty, "Ordering", x, y),
    Set.card(x, (y + 32 : U32), w, 116),
    Set.row.r(ty, U.Fab.set.order.label(), U.Fab.set.order.note(), x, (y + 34 : U32), w, 140),
    Lay.chip(ty, (x + w - 120 : U32), (y + 48 : U32), "On", U.Fab.enabled(st), "setting", "order.enabled=on"),
    Lay.chip(ty, (x + w - 70 : U32), (y + 48 : U32), "Off", Bool.not(U.Fab.enabled(st)), "setting", "order.enabled=off"),
    Set.row.r(ty, U.Fab.set.label(), U.Fab.set.note.k(U.Fab.enabled(st)), x, (y + 86 : U32), w, 140),
    Set.order.est(U.Fab.enabled(st), ty, U.Fab.set.on(st), x, w, (y + 100 : U32))])

# the browser engine row (core/cef.bend): Chrome apart from the window, or
# Embedded in it; a note only when Embedded cannot run
def Set.browser(+ty: Ty, +eng: String, +err: String, +x: U32, +w: U32, +y: U32) -> Out:
  Out.all([Set.row.r(ty, "Browser", CF.Cef.note(err), x, Bool.pick(U32, String.is_empty(err), (y + 12 : U32), y), w, 190),
    Lay.chip(ty, (x + w - 16 - Lay.button.w(ty, "Embedded") - 6 - Lay.button.w(ty, "Chrome") : U32), (y + 14 : U32), "Chrome", Bool.not(String.eq(eng, "cef")), "setting", "browser.engine=chrome"),
    Lay.chip(ty, (x + w - 16 - Lay.button.w(ty, "Embedded") : U32), (y + 14 : U32), "Embedded", String.eq(eng, "cef"), "setting", "browser.engine=cef")])

def Set.fallback.h(+ty: Ty, +n: S.Nui, +x: U32, +w: U32) -> U32:
  (20 + Set.agy.sum(U.Fallbackv.rows(S.Nui.ui(n)), ty, x, w) : U32)

def Set.fallback(+ty: Ty, +n: S.Nui, +x: U32, +w: U32, +y: U32) -> Out:
  +rs = U.Fallbackv.rows(S.Nui.ui(n))
  Out.cat(Set.card(x, y, w, (Set.agy.sum(rs, ty, x, w) + 10 : U32)), Set.agy.rows(rs, ty, n, x, w, (y + 8 : U32)))

# Keep the first Settings group in its own native layout call. The full
# screen's single row expression otherwise exceeds Bend's call arity.
def Set.threads(+ty: Ty, +st: M.State, +n: S.Nui, +x: U32, +w: U32, +b: U32) -> Out:
  Out.all([
    Set.head(ty, MSG.T.tour.title.threads(Ty.loc(ty)), x, (b + 76 : U32)),
    Set.card(x, (b + 108 : U32), w, 128),
    Set.row.r(ty, MSG.T.settings.settle(Ty.loc(ty)), MSG.T.settings.settle.note(Ty.loc(ty)), x, (b + 110 : U32), w, 190),
    Set.days(ty, M.Setting.get(st, "settle.days", "3"), (x + w - 172 : U32), (b + 126 : U32)),
    Set.carry(ty, M.Carry.on(st), x, w, (b + 174 : U32)),
    Set.notify(st, ty, x, w, (b + 256 : U32)),
    Set.fallback(ty, n, x, w, (b + 256 + Set.notify.h(st, ty, w) : U32))])

def Lay.settings(+ty: Ty, +st: M.State, +n: S.Nui, +info: Map<&2, String>, +v: String, +note: String, +x0: U32, +x1: U32, +h: U32, +raw: U32) -> Out:
  +label0 = MSG.T.settings.theme.light(Ty.loc(ty))
  +label1 = MSG.T.settings.theme.dark(Ty.loc(ty))
  +label2 = MSG.T.settings.sidebar.active(Ty.loc(ty))
  +label3 = MSG.T.tour.title.projects(Ty.loc(ty))
  +label4 = MSG.T.settings.channel.nightly(Ty.loc(ty))
  +label5 = MSG.T.settings.channel.stable(Ty.loc(ty))
  +label6 = MSG.T.common.close(Ty.loc(ty))
  +w = {U32.min((x1 - x0 - 64 : U32), 720) : U32}
  +x = {(x0 + U32.shr((x1 - x0 - w : U32)) : U32) : U32}
  +prov = M.Setting.get(st, "agent.provider", "claude")
  +ipc = M.Setting.get(st, "kicad.ipc", "on")
  +ms = U.Models.menu(U.Str.get(info, "models"), prov)
  +dy = {(Set.rows(ty, ms, (x + 16 : U32), (x + w - 16 : U32), (x + 16 : U32)) * 34 + 26 : U32) : U32}
  +tp = U.TextGen.provider(st)
  +tm = U.TextGen.model(st)
  +tms = U.Models.menu(U.Str.get(info, "models"), tp)
  +er = {(Set.rows(ty, tms, (x + 16 : U32), (x + w - 16 : U32), (x + 16 : U32)) * 34 : U32) : U32}
  +ey = {(202 + er : U32) : U32}
  +kn = U.KN.of(info, st)
  +kh = {(40 + Kn.h(kn, ty, (w - 32 : U32)) : U32) : U32}
  # (the Browser row, then the Ordering group, then Appearance)
  +fy = {(ey + 278 + kh : U32) : U32}
  +total = {(1196 + Set.fallback.h(ty, n, x, w) + Set.notify.h(st, ty, w) + dy + fy + Set.voice.h(ty, n, x, w) + Set.mates.h(n) + Set.keys.h(n) + Set.agy.h(ty, n, x, w) + Set.tz.h() + Set.usage.h(n) + 132 : U32) : U32}
  +off = {U32.min(raw, U32.dim(total, h)) : U32}
  +b = {(0 - off : U32) : U32}
  +b2 = {(b + 64 + Set.fallback.h(ty, n, x, w) + Set.notify.h(st, ty, w) : U32) : U32}
  Out.all([
    Out{Nil{}, [Lay.hit(x0, 52, (x1 - x0 : U32), (h - 52 : U32), "set-bg", U32.show(U32.dim(total, h)))]},
    Out.hits_below(52, Out.all([
    Set.threads(ty, st, n, x, w, b),
    Set.head(ty, MSG.T.settings.agents(Ty.loc(ty)), x, (b2 + 192 : U32)),
    Set.card(x, (b2 + 224 : U32), w, (124 + dy : U32)),
    Set.row(ty, MSG.T.settings.agents.default(Ty.loc(ty)), MSG.T.settings.agents.note(Ty.loc(ty), Set.cli.loc(Ty.loc(ty), U.Models.cli(U.Str.get(info, "models"), prov), prov)), x, (b2 + 226 : U32), w),
    Set.chips(ty, ["claude", "codex", "grok", "muse", "antigravity"], prov, "agent.provider", (x + 16 : U32), (x + w - 16 : U32), (x + 16 : U32), (b2 + 274 : U32)),
    Set.chips(ty, ms, M.Setting.get(st, "agent.model", ""), "agent.model", (x + 16 : U32), (x + w - 16 : U32), (x + 16 : U32), (b2 + 308 : U32)),
    Out.ops(Ty.text(ty, 2n, Ty.fit(ty, 2n, Set.model_line.loc(Ty.loc(ty), U.Models.label_of(U.Str.get(info, "models"), prov, M.Setting.get(st, "agent.model", "")), M.Setting.get(st, "agent.model", "")), (w - 32 : U32)),
      D.Clip.all(), (x + 16 : U32), (b2 + 318 + dy : U32), 24, T.Th.dim())),
    Set.head(ty, MSG.T.settings.writing(Ty.loc(ty)), x, (b2 + 368 + dy : U32)),
    Set.card(x, (b2 + 400 + dy : U32), w, (150 + er : U32)),
    Set.row(ty, MSG.T.settings.writing.model(Ty.loc(ty)), MSG.T.settings.writing.note(Ty.loc(ty)), x, (b2 + 402 + dy : U32), w),
    Set.chips(ty, ["claude", "codex", "grok", "muse"], tp, "text.provider", (x + 16 : U32), (x + w - 16 : U32), (x + 16 : U32), (b2 + 450 + dy : U32)),
    Set.chips(ty, tms, tm, "text.model", (x + 16 : U32), (x + w - 16 : U32), (x + 16 : U32), (b2 + 484 + dy : U32)),
    Out.ops(Ty.text(ty, 2n, Ty.fit(ty, 2n, Set.model_line.loc(Ty.loc(ty), U.Models.label_of(U.Str.get(info, "models"), tp, tm), tm), (w - 32 : U32)),
      D.Clip.all(), (x + 16 : U32), (b2 + 520 + dy + er : U32), 24, T.Th.dim())),
    Set.head(ty, MSG.T.settings.cad(Ty.loc(ty)), x, (b2 + 368 + dy + ey : U32)),
    Set.card(x, (b2 + 400 + dy + ey : U32), w, (252 + kh : U32)),
    Set.row(ty, "KiCad", U.KN.summary(info), x, (b2 + 402 + dy + ey : U32), w),
    Kn.path(ty, n, (x + 16 : U32), (b2 + 450 + dy + ey : U32), (w - 32 : U32)),
    Kn.draw(kn, False{}, ty, (x + 16 : U32), (b2 + 490 + dy + ey : U32), (w - 32 : U32)),
    Set.row.r(ty, U.KiCad.ipc.label(Ty.loc(ty), info), U.KiCad.ipc.note(Ty.loc(ty), info), x, (b2 + 486 + dy + ey + kh : U32), w, Bool.pick(U32, U.KiCad.ipc.ready(info), Set.pairw(ty, MSG.T.settings.lang.on(Ty.loc(ty)), MSG.T.settings.lang.off(Ty.loc(ty)), 140), 0)),
    Bool.pick(Out, U.KiCad.ipc.ready(info), Set.ipc(ty, ipc, x, w, (b2 + 500 + dy + ey + kh : U32)), Out.none()),
    Set.row(ty, U.FreeCAD.label.loc(Ty.loc(ty), info), U.FreeCAD.note(Ty.loc(ty), info), x, (b2 + 546 + dy + ey + kh : U32), w),
    Set.browser(ty, M.Setting.get(st, "browser.engine", "chrome"), U.Str.get(info, "browser.engine.err"), x, w, (b2 + 598 + dy + ey + kh : U32)),
    Set.order(ty, st, x, w, (b2 + 674 + dy + ey + kh : U32)),
    Set.head(ty, MSG.T.settings.appearance(Ty.loc(ty)), x, (b2 + 570 + dy + fy : U32)),
    Set.card(x, (b2 + 602 + dy + fy : U32), w, 220),
    Set.row.r(ty, MSG.T.settings.theme(Ty.loc(ty)), MSG.T.settings.theme.note(Ty.loc(ty)), x, (b2 + 604 + dy + fy : U32), w, Set.pairw(ty, label0, label1, 150)),
    Lay.chip(ty, (x + w - 16 - Lay.button.w(ty, label0) - 6 - Lay.button.w(ty, label1) : U32), (b2 + 618 + dy + fy : U32), label1, Bool.not(String.eq(M.Setting.get(st, "ui.theme", ""), "light")), "setting", "ui.theme=dark"),
    Lay.chip(ty, (x + w - 16 - Lay.button.w(ty, label0) : U32), (b2 + 618 + dy + fy : U32), label0, String.eq(M.Setting.get(st, "ui.theme", ""), "light"), "setting", "ui.theme=light"),
    Set.lang(ty, S.Nui.ui(n), x, w, (b2 + 656 + dy + fy : U32)),
    Set.row.r(ty, MSG.T.settings.sidebar(Ty.loc(ty)), MSG.T.settings.sidebar.note(Ty.loc(ty)), x, (b2 + 760 + dy + fy : U32), w, Set.pairw(ty, label2, label3, 190)),
    Lay.chip(ty, (x + w - 16 - Lay.button.w(ty, label2) - 6 - Lay.button.w(ty, label3) : U32), (b2 + 774 + dy + fy : U32), label3, Bool.not(String.eq(U.Side.view(st), "active")), "setting", "sidebar.view=projects"),
    Lay.chip(ty, (x + w - 16 - Lay.button.w(ty, label2) : U32), (b2 + 774 + dy + fy : U32), label2, String.eq(U.Side.view(st), "active"), "setting", "sidebar.view=active"),
    Set.head(ty, MSG.T.settings.network(Ty.loc(ty)), x, (b2 + 846 + dy + fy : U32)),
    Set.card(x, (b2 + 878 + dy + fy : U32), w, 216),
    Set.row(ty, U.Tail.label(Ty.loc(ty), info), U.Tail.note(Ty.loc(ty), info), x, (b2 + 880 + dy + fy : U32), w),
    Set.pair(ty, info, Pq.on(n), x, w, (b2 + 932 + dy + fy : U32)),
    Set.row.r(ty, "Backplane " ++ v, U.Update.line.w(Ty.loc(ty), st, info, note), x, (b2 + 984 + dy + fy : U32), w, U32.max(150, (Lay.button.w(ty, Set.update.label(U.Update.btn(st, info, note), Ty.loc(ty))) + 22 : U32))),
    Set.update.btn(U.Update.btn(st, info, note), ty, x, w, (b2 + 994 + dy + fy : U32)),
    Set.row.r(ty, MSG.T.settings.channel(Ty.loc(ty)), MSG.T.settings.channel.note(Ty.loc(ty)), x, (b2 + 1036 + dy + fy : U32), w, Set.pairw(ty, label4, label5, 190)),
    Lay.chip(ty, (x + w - 16 - Lay.button.w(ty, label4) - 6 - Lay.button.w(ty, label5) : U32), (b2 + 1050 + dy + fy : U32), label5, Bool.not(String.eq(M.Setting.get(st, "update.channel", "stable"), "nightly")), "setting", "update.channel=stable"),
    Lay.chip(ty, (x + w - 16 - Lay.button.w(ty, label4) : U32), (b2 + 1050 + dy + fy : U32), label4, String.eq(M.Setting.get(st, "update.channel", "stable"), "nightly"), "setting", "update.channel=nightly"),
    Set.voice(ty, st, n, x, w, (b2 + 1114 + dy + fy : U32)),
    Set.mates(ty, n, x, w, (b2 + 1114 + dy + fy + Set.voice.h(ty, n, x, w) : U32)),
    Set.keys(ty, n, x, w, (b2 + 1114 + dy + fy + Set.voice.h(ty, n, x, w) + Set.mates.h(n) : U32)),
    Set.agy(ty, n, x, w, (b2 + 1114 + dy + fy + Set.voice.h(ty, n, x, w) + Set.mates.h(n) + Set.keys.h(n) + 16 : U32)),
    Set.tz(ty, n, x, w, (b2 + 1114 + dy + fy + Set.voice.h(ty, n, x, w) + Set.mates.h(n) + Set.keys.h(n) + Set.agy.h(ty, n, x, w) + 16 : U32)),
    Set.usage(ty, n, x, w, (b2 + 1114 + dy + fy + Set.voice.h(ty, n, x, w) + Set.mates.h(n) + Set.keys.h(n) + Set.agy.h(ty, n, x, w) + Set.tz.h() + 16 : U32)),
    Set.tour(ty, x, w, (b2 + 1114 + dy + fy + Set.voice.h(ty, n, x, w) + Set.mates.h(n) + Set.keys.h(n) + Set.agy.h(ty, n, x, w) + Set.tz.h() + Set.usage.h(n) + 16 : U32))])),
    Out{List.append(&2, R.Op, [D.Draw.rect(D.Clip.all(), x0, 0, (x1 - x0 : U32), 51, T.Th.bg(), 255), D.Draw.rect(D.Clip.all(), x0, 51, (x1 - x0 : U32), 1, T.Th.line(), 255)],
      Ty.text(ty, 0n, MSG.T.palette.settings(Ty.loc(ty)), D.Clip.all(), (x0 + 24 : U32), 0, 52, T.Th.text())), Nil{}},
    Lay.button(ty, D.Clip.all(), (x1 - 16 - Lay.button.w(ty, label6) : U32), 12, label6, T.Th.soft(), "flag", "settings"),
    Pq.over(Pq.on(n), ty, info, x0, x1, h)])

# Terminal
# --------

# Terminal cells, including combining marks (0) and East Asian wide (2).
def Term.cells(s: String, acc: U32) -> U32:
  match s:
    case SNil{}:
      acc
    case SCon{Chr{cp}, t}:
      Term.cells(t, (acc + VT.Vt.width(cp) : U32))

def Term.runs(+ty: Ty, rs: List<&2, VT.VRun>, +cw: U32, +c: D.Clip, +x: U32, +y: U32) -> List<&2, R.Op>:
  match rs:
    case Nil{}:
      Nil{}
    case VT.VRun{+t, +fg, +bg, +bold, _} <> rest:
      +w = {(Term.cells(t, 0) * cw : U32) : U32}
      List.concat(&2, R.Op, [
        Bool.pick(List<&2, R.Op>, U32.is_eq(bg, VT.Vt.bg()), Nil{}, [D.Draw.rect(c, x, y, w, 18, bg, 255)]),
        Ty.text(ty, Bool.pick(Nat, bold, 1n, 4n), t, c, x, y, 18, fg),
        Term.runs(ty, rest, cw, c, (x + w : U32), y)])

def Term.lines(+ty: Ty, ls: List<&2, VT.VLine>, +cw: U32, +c: D.Clip, +x: U32, +y: U32) -> List<&2, R.Op>:
  match ls:
    case Nil{}:
      Nil{}
    case VT.VLine{rs} <> t:
      List.append(&2, R.Op, Term.runs(ty, rs, cw, c, x, y), Term.lines(ty, t, cw, c, x, (y + 18 : U32)))

# a focused terminal's cursor blinks (a caret: D.Draw.caret); an
# unfocused one is a thin bar
def Term.cursor.k(focused: Bool, +blink: Bool, +cw: U32, +c: D.Clip, +x: U32, +y: U32) -> List<&2, R.Op>:
  match focused:
    case True{}:
      [D.Draw.caret(c, x, y, cw, 18, T.Th.accent(), 150, blink)]
    case False{}:
      [D.Draw.rect(c, x, y, 2, 18, T.Th.accent(), 255)]

def Term.cursor(cur: VT.VCur, +focused: Bool, +blink: Bool, +cw: U32, +c: D.Clip, +x: U32, +y: U32) -> List<&2, R.Op>:
  match cur:
    case VT.VCur{+cx, +cy, True{}}:
      Term.cursor.k(focused, blink, cw, c, (x + cx * cw : U32), (y + cy * 18 : U32))
    case VT.VCur{_, _, False{}}:
      Nil{}

def Ew.screen(+ty: Ty, +v: VT.Vt, +c: D.Clip, +x: U32, +y: U32, +w: U32, +h: U32, +focused: Bool, +blink: Bool) -> List<&2, R.Op>:
  List.concat(&2, R.Op, [Term.lines(ty, VT.Vt.lines(v, 0n), Ew.cw(ty), c, (x + 12 : U32), y), Term.cursor(VT.Vt.cursor(v), focused, blink, Ew.cw(ty), c, (x + 12 : U32), y),
    D.Draw.frame(c, x, y, w, h, Bool.pick(U32, focused, T.Th.focus(), T.Th.line()))])

def Ew.draws(hs: List<&2, S.Hit>, +ty: Ty, +ec: U.Ec, +c: D.Clip, +fkey: String, +blink: Bool) -> List<&2, R.Op>:
  match hs:
    case Nil{}:
      Nil{}
    case S.Hit{+x0, +y0, +x1, +y1, _, +k} <> t:
      List.append(&2, R.Op, Ew.screen(ty, U.Ec.vt(ec, EM.Emb.focused(k)), Ew.cut(c, x0, y0, x1, y1), x0, y0, (x1 - x0 : U32), (y1 - y0 : U32), String.eq(EM.Emb.focused(k), fkey), blink), Ew.draws(t, ty, ec, c, fkey, blink))

def Ew.over(+ty: Ty, o: Out, +ec: U.Ec, +fkey: String, +blink: Bool) -> Out:
  match o:
    case Out{os, +hs}:
      Out{List.append(&2, R.Op, os, Ew.draws(Ew.uniq(hs, Nil{}), ty, ec, Ew.clip(hs), fkey, blink)), hs}

def Lay.pin(+ty: Ty, +ec: U.Ec, v: EM.EV, +x0: U32, +x1: U32, +y: U32, +fkey: String, +blink: Bool) -> Out:
  match v:
    case EM.EV{+key, _, +title, +cols, +rows, _, _}:
      +w = Ew.w(ty, cols, x0, x1)
      +sy = {(y + 26 : U32) : U32}
      Out.all([Out{List.concat(&2, R.Op, [Lay.icon(ty, "terminal", D.Clip.all(), x0, (y + 5 : U32), T.Th.faint()), Ty.text(ty, 2n, Ty.fit(ty, 2n, title, (w - 56 : U32)), D.Clip.all(), (x0 + 22 : U32), y, 26, T.Th.dim()),
          Lay.icon(ty, "pin", D.Clip.all(), (x0 + w - 20 : U32), (y + 5 : U32), T.Th.accent()),
          [D.Draw.rect(D.Clip.all(), x0, sy, w, (rows * 18 : U32), VT.Vt.bg(), 255)]]),
        [Lay.hit((x0 + w - 26 : U32), y, 26, 26, "emb-pin", key), Lay.hit(x0, sy, w, (rows * 18 : U32), "focus", EM.Emb.focus(key))]},
        Out.ops(Ew.screen(ty, U.Ec.vt(ec, key), D.Clip{x0, sy, (x0 + w : U32), (sy + rows * 18 : U32)}, x0, sy, w, (rows * 18 : U32), String.eq(key, fkey), blink))])

# the thread's pinned terminals, under the subagents
def Lay.pins(+ty: Ty, +ec: U.Ec, vs: List<&2, EM.EV>, +x0: U32, +x1: U32, +y: U32, +fkey: String, +blink: Bool) -> Out:
  match vs:
    case Nil{}:
      Out.none()
    case EM.EV{k, i, t, c, +r, l, p} <> rest:
      Out.cat(Lay.pin(ty, ec, EM.EV{k, i, t, c, r, l, p}, x0, x1, y, fkey, blink), Lay.pins(ty, ec, rest, x0, x1, (y + Pins.one(r) : U32), fkey, blink))

# the thread's terminal (ctrl+j), a shell in the thread's directory
def Lay.term(+ty: Ty, +v: VT.Vt, +x0: U32, +x1: U32, +y0: U32, +h: U32, +focused: Bool, +blink: Bool) -> Out:
  +cw = {U32.max(Ty.width(ty, 4n, "0"), 1) : U32}
  +c = {D.Clip{x0, (y0 + 28 : U32), x1, h} : D.Clip}
  Out.all([
    Out.ops([D.Draw.rect(D.Clip.all(), x0, y0, (x1 - x0 : U32), (h - y0 : U32), VT.Vt.bg(), 255),
      D.Draw.rect(D.Clip.all(), x0, y0, (x1 - x0 : U32), 1, Bool.pick(U32, focused, T.Th.focus(), T.Th.line()), 255)]),
    Out.ops(Ty.text(ty, 2n, MSG.T.viewer.terminal.title(Ty.loc(ty), Bool.pick(String, String.is_empty(VT.Vt.title(v)), "none", "named"), VT.Vt.title(v)), D.Clip.all(), (x0 + 16 : U32), y0, 28, T.Th.faint())),
    Out.ops(Ty.text(ty, 2n, "ctrl+j", D.Clip.all(), (x1 - 16 - Ty.width(ty, 2n, "ctrl+j") : U32), y0, 28, T.Th.faint())),
    Out{List.append(&2, R.Op, Term.lines(ty, VT.Vt.lines(v, 0n), cw, c, (x0 + 12 : U32), (y0 + 30 : U32)), Term.cursor(VT.Vt.cursor(v), focused, blink, cw, c, (x0 + 12 : U32), (y0 + 30 : U32))),
      [Lay.hit(x0, y0, (x1 - x0 : U32), (h - y0 : U32), "focus", "terminal")]}])

# Viewer chrome
# -------------

type KV2 is Data:
  KV2{k: String, v: String}

def Info.row(kv: List<&2, String>) -> KV2:
  match kv:
    case k <> v:
      KV2{k, String.join(v, "=")}
    case Nil{}:
      KV2{"", ""}

def Info.rows(ps: List<&2, String>) -> List<&2, KV2>:
  match ps:
    case Nil{}:
      Nil{}
    case p <> t:
      Info.row(String.split(p, '=')) <> Info.rows(t)

# a field the readers name (core/board.bend, core/sch.bend's Info.kv) in
# the device language, by its name; one not listed shows as it is
def Info.keys() -> List<&2, String>:
  ["net", "layer", "width", "ref", "value", "footprint", "size", "drill", "type", "shape", "name", "symbol", "lib", "text", "sheet", "file", "netclass"]

def Info.key.k(+l: I.Loc, i: Nat, +k: String) -> String:
  match i:
    case 0n: MSG.T.viewer.field.net(l)
    case 1n: MSG.T.viewer.field.layer(l)
    case 2n: MSG.T.viewer.field.width(l)
    case 3n: MSG.T.viewer.field.ref(l)
    case 4n: MSG.T.viewer.field.value(l)
    case 5n: MSG.T.viewer.field.footprint(l)
    case 6n: MSG.T.viewer.field.size(l)
    case 7n: MSG.T.viewer.field.drill(l)
    case 8n: MSG.T.viewer.field.ktype(l)
    case 9n: MSG.T.viewer.field.shape(l)
    case 10n: MSG.T.viewer.field.name(l)
    case 11n: MSG.T.viewer.field.symbol(l)
    case 12n: MSG.T.viewer.field.lib(l)
    case 13n: MSG.T.viewer.field.text(l)
    case 14n: MSG.T.viewer.field.sheet(l)
    case 15n: MSG.T.viewer.field.file(l)
    case 16n: MSG.T.viewer.field.netclass(l)
    case _: k

def Info.key(+l: I.Loc, +k: String) -> String:
  Info.key.k(l, CC.Codec.pos(Info.keys(), k), k)

# what was picked ("via", "pad 3", "global label"), the same way
def Info.kinds() -> List<&2, String>:
  ["via", "track", "footprint", "zone", "symbol", "label", "global label", "hier label", "sheet pin", "sheet", "netclass flag"]

def Info.kind.k(+l: I.Loc, i: Nat, +t: String) -> String:
  match i:
    case 0n: MSG.T.viewer.item.via(l)
    case 1n: MSG.T.viewer.item.track(l)
    case 2n: MSG.T.viewer.item.footprint(l)
    case 3n: MSG.T.viewer.item.zone(l)
    case 4n: MSG.T.viewer.item.symbol(l)
    case 5n: MSG.T.viewer.item.label(l)
    case 6n: MSG.T.viewer.item.globallabel(l)
    case 7n: MSG.T.viewer.item.hierlabel(l)
    case 8n: MSG.T.viewer.item.sheetpin(l)
    case 9n: MSG.T.viewer.item.sheet(l)
    case 10n: MSG.T.viewer.item.netclassflag(l)
    case _: t

def Info.title(+l: I.Loc, +t: String) -> String:
  Bool.pick(String, String.starts_with(t, "pad "), MSG.T.viewer.item.pad(l, String.drop(t, 4n)),
    Bool.pick(String, String.starts_with(t, "pin "), MSG.T.viewer.item.pin(l, String.drop(t, 4n)), Info.kind.k(l, CC.Codec.pos(Info.kinds(), t), t)))

def Info.draw.k(+ty: Ty, r: KV2, +x: U32, +y: U32, +w: U32) -> Out:
  KV2{k, v} = r
  Out.ops(List.append(&2, R.Op, Ty.text(ty, 2n, Ty.fit(ty, 2n, Info.key(Ty.loc(ty), k), 78), D.Clip.all(), (x + 14 : U32), y, 22, T.Th.faint()),
    Ty.text(ty, 2n, Ty.fit(ty, 2n, v, (w - 110 : U32)), D.Clip.all(), (x + 96 : U32), y, 22, T.Th.text())))

def Info.draw(+ty: Ty, rs: List<&2, KV2>, +x: U32, +y: U32, +w: U32) -> Out:
  match rs:
    case Nil{}:
      Out.none()
    case r <> t:
      Out.cat(Info.draw.k(ty, r, x, y, w), Info.draw(ty, t, x, (y + 22 : U32), w))

# what the picked item is: its kind, then key = value rows (the info the
# parser recorded: net, ref, value, footprint, pin, label...)
# the value of the first "key=value" part (key given with its "=")
def Info.get(ps: List<&2, String>, +k: String) -> String:
  match ps:
    case Nil{}:
      ""
    case +p <> t:
      Bool.pick(String, String.starts_with(p, k), String.drop(p, String.length(k)), Info.get(t, k))

def Lay.inspect(+ty: Ty, +info: String, +draft: String, +x0: U32, +x1: U32, +h: U32) -> Out:
  +label0 = MSG.T.viewer.mention(Ty.loc(ty))
  +ps = String.split(info, ';')
  +title = Maybe.default(&2, String, List.head(&2, String, ps), "")
  +rows = Info.rows(List.drop(&2, String, ps, 1n))
  +n = {U32.from_nat(List.length(&2, KV2, rows)) : U32}
  +w = {U32.min((x1 - x0 - 32 : U32), 420) : U32}
  +ch = {(n * 22 + 88 : U32) : U32}
  +y = {(h - 16 - ch : U32) : U32}
  +mention = "[" ++ info ++ "] "
  Out.when(Bool.not(String.is_empty(info)), Out.all([
    Out.ops(Lay.box(D.Clip.all(), (x0 + 16 : U32), y, w, ch, 10, T.Th.panel(), T.Th.line())),
    Out.ops(Ty.text(ty, 1n, Info.title(Ty.loc(ty), title), D.Clip.all(), (x0 + 30 : U32), (y + 8 : U32), 28, T.Th.text())),
    Out{Lay.icon(ty, "x", D.Clip.all(), (x0 + w - 10 : U32), (y + 12 : U32), T.Th.dim()), [Lay.hit((x0 + w - 16 : U32), (y + 6 : U32), 28, 28, "vw-unpick", "")]},
    Info.draw(ty, rows, (x0 + 16 : U32), (y + 38 : U32), w),
    Lay.button(ty, D.Clip.all(), (x0 + 30 : U32), (y + ch - 40 : U32), label0, T.Th.accent(), "draft", draft ++ Bool.pick(String, String.is_empty(draft), "", " ") ++ mention),
    Out.when(String.eq(title, "sheet"), Lay.button(ty, D.Clip.all(), (x0 + 38 + Lay.button.w(ty, label0) : U32), (y + ch - 40 : U32), MSG.T.viewer.sheet.open(Ty.loc(ty)), T.Th.accent(), "vw-open", Info.get(ps, "name=")))]))

def Lay.tab(+ty: Ty, +label: String, +k: U32, +cur: U32, +x: U32) -> Out:
  Lay.chip(ty, x, 12, label, U32.is_eq(k, cur), "vw-kind", U32.show(k))

def Lay.shown(+k: U32, +empty: Bool, +ts: FA.Scene3, b: BM.Bitmap) -> Bool:
  match b:
    case BM.Bitmap{w, _, _, _, _, _, _, _}:
      Bool.pick(Bool, U32.is_ge(k, 3), True{}, Bool.pick(Bool, U32.is_eq(k, 2), Bool.not(FA.Scene3.is_empty(ts)), Bool.not(empty)))

def Lay.help.loc(+l: I.Loc, +k: U32) -> String:
  Bool.pick(String, Bool.or(U32.is_eq(k, 5), U32.is_eq(k, 9)), "",
  Bool.pick(String, U32.is_eq(k, 4), MSG.T.viewer.help.scroll(l),
  Bool.pick(String, U32.is_eq(k, 3), MSG.T.viewer.help.browser(l),
    Bool.pick(String, U32.is_eq(k, 2), MSG.T.viewer.help.solid(l), MSG.T.viewer.help.board(l)))))

# Diff view
# ---------

type DRow is Data:
  DRow{text: String, kind: U32}

def Lay.help(+k: U32) -> String:
  Lay.help.loc(I.En{}, k)

def DRow.lines(ls: List<&2, DF.DLine>, rest: List<&2, DRow>) -> List<&2, DRow>:
  match ls:
    case Nil{}:
      rest
    case DF.DLine{k, t, _, _} <> tl:
      DRow{t, (k + 10 : U32)} <> DRow.lines(tl, rest)

def DRow.hunks(hs: List<&2, DF.DHunk>, rest: List<&2, DRow>) -> List<&2, DRow>:
  match hs:
    case Nil{}:
      rest
    case DF.DHunk{hd, ls} <> t:
      DRow{hd, 3} <> DRow.lines(ls, DRow.hunks(t, rest))

def DRow.files(fs: List<&2, DF.DFile>) -> List<&2, DRow>:
  match fs:
    case Nil{}:
      Nil{}
    case DF.DFile{_, nw, st, hs} <> t:
      DRow{nw ++ "  " ++ st, 1} <> DRow.hunks(hs, DRow{"", 2} <> DRow.files(t))

def DRow.color(+k: U32) -> U32:
  Bool.pick(U32, U32.is_eq(k, 11), T.Th.ok(), Bool.pick(U32, U32.is_eq(k, 12), T.Th.bad(), Bool.pick(U32, Bool.or(U32.is_eq(k, 3), U32.is_eq(k, 13)), T.Th.faint(), T.Th.soft())))

def DRow.draw(+ty: Ty, r: DRow, +c: D.Clip, +x0: U32, +x1: U32, +y: U32) -> List<&2, R.Op>:
  match r:
    case DRow{t, +k}:
      List.append(&2, R.Op,
        Bool.pick(List<&2, R.Op>, U32.is_eq(k, 11), [D.Draw.rect(c, x0, y, (x1 - x0 : U32), 20, 1127459, 255)], Bool.pick(List<&2, R.Op>, U32.is_eq(k, 12), [D.Draw.rect(c, x0, y, (x1 - x0 : U32), 20, 3279380, 255)], Nil{})),
        Ty.text(ty, Bool.pick(Nat, U32.is_eq(k, 1), 1n, 4n), Bool.pick(String, U32.is_eq(k, 11), "+", Bool.pick(String, U32.is_eq(k, 12), "-", " ")) ++ t, c, (x0 + 12 : U32), y, 20, Bool.pick(U32, U32.is_eq(k, 1), T.Th.text(), DRow.color(k))))

def DRow.more(rs: List<&2, DRow>, +y: U32, +y1: U32) -> Bool:
  match rs:
    case Nil{}:
      False{}
    case _ <> _:
      U32.is_lt(y, y1)

# rows from y down while they start above y1
def DRow.all(+ty: Ty, rs: List<&2, DRow>, more: Bool, +c: D.Clip, +x0: U32, +x1: U32, +y: U32, +y1: U32) -> List<&2, R.Op>:
  match rs more:
    case r <> +t True{}:
      List.append(&2, R.Op, DRow.draw(ty, r, c, x0, x1, y), DRow.all(ty, t, DRow.more(t, (y + 20 : U32), y1), c, x0, x1, (y + 20 : U32), y1))
    case _ _:
      Nil{}

def DRow.go(+ty: Ty, +rs: List<&2, DRow>, +c: D.Clip, +x0: U32, +x1: U32, +y: U32, +y1: U32) -> List<&2, R.Op>:
  DRow.all(ty, rs, DRow.more(rs, y, y1), c, x0, x1, y, y1)

# what this thread changed (checkpoint 0 against the files now), with the
# git actions T3Code offers
def Lay.diff(+ty: Ty, +text: String, +x0: U32, +x1: U32, +h: U32, +scroll: U32) -> Out:
  +label0 = MSG.T.palette.commit(Ty.loc(ty))
  +label1 = MSG.T.viewer.diff.pr(Ty.loc(ty))
  +label2 = MSG.T.palette.revert(Ty.loc(ty))
  +fs = DF.Diff.parse(text)
  +top = {96 : U32}
  +bc = Lay.button.w(ty, label0)
  +bp = Lay.button.w(ty, label1)
  Out.all([
    Out.ops(Ty.text(ty, 2n, Bool.pick(String, List.is_empty(&2, DF.DFile, fs), MSG.T.viewer.diff.empty(Ty.loc(ty)), DF.Diff.summary(fs)), D.Clip.all(), (x0 + 16 : U32), 58, 30, T.Th.dim())),
    Lay.button(ty, D.Clip.all(), (x1 - 16 - bc : U32), 60, label0, T.Th.accent(), "git", "commit_push"),
    Lay.button(ty, D.Clip.all(), (x1 - 24 - bc - bp : U32), 60, label1, T.Th.soft(), "git", "commit_push_pr"),
    Lay.button(ty, D.Clip.all(), (x1 - 32 - bc - bp - Lay.button.w(ty, label2) : U32), 60, label2, T.Th.bad(), "revert", "0"),
    Out.ops(DRow.go(ty, List.drop(&2, DRow, DRow.files(fs), U32.to_nat((scroll / 20 : U32))), D.Clip{x0, top, x1, h}, x0, x1, top, h))])

def Lay.diff.if(open: Bool, +ty: Ty, +diff: String, +x0: U32, +x1: U32, +h: U32, +scroll: U32) -> Out:
  match open:
    case True{}:
      Lay.diff(ty, diff, x0, x1, h, scroll)
    case False{}:
      Out.none()

# a board, schematic or model on show opens in the bundled KiCad
def Lay.kicad.on(+k: U32, +empty: Bool, +ts: FA.Scene3, +bm: BM.Bitmap) -> Bool:
  Bool.and(U32.is_le(k, 2), Lay.shown(k, empty, ts, bm))

# a - b, never below 0
def Lay.room(+a: U32, +b: U32) -> U32:
  Bool.pick(U32, U32.is_gt(a, b), (a - b : U32), 0)

# the header width the Open in KiCad button takes
def Lay.kicad.w(+ty: Ty, +on: Bool) -> U32:
  Bool.pick(U32, on, (Lay.button.w(ty, MSG.T.palette.kicad(Ty.loc(ty))) + Lay.button.w(ty, MSG.T.viewer.kicad10(Ty.loc(ty))) + 20 : U32), 0)

def Lay.kicad(on: Bool, +ty: Ty, +x1: U32) -> Out:
  match on:
    case True{}:
      +open = MSG.T.palette.kicad(Ty.loc(ty))
      +copy = MSG.T.viewer.kicad10(Ty.loc(ty))
      Out.all([Lay.button(ty, D.Clip.all(), (x1 - 16 - Lay.button.w(ty, open) : U32), 12, open, T.Th.text(), "kicad-open", ""),
        Lay.button(ty, D.Clip.all(), (x1 - 28 - Lay.button.w(ty, open) - Lay.button.w(ty, copy) : U32), 12, copy, T.Th.text(), "kicad-export10", "")])
    case False{}:
      Out.none()

# a tab or chip of the viewer's header, drawn only when it ends inside the
# panel (a narrow panel shows the first ones; nothing spills over)
def Lay.fits(on: Bool, o: Out) -> Out:
  match on:
    case True{}:
      o
    case False{}:
      Out.none()

def Lay.viewer.k(+ty: Ty, +x0: U32, +x1: U32, +h: U32, +empty: Bool, x: V.Vx, +info: String, +draft: String, +diff: String, +scroll: U32, +mech: Bool) -> Out:
  match x:
    case V.Vx{+k, +p, +ts, _, _, +busy, _, _, _, _, +bm}:
      +label0 = MSG.T.viewer.board(Ty.loc(ty))
      +label1 = MSG.T.viewer.schematic(Ty.loc(ty))
      +label2 = MSG.T.viewer.solid(Ty.loc(ty))
      +label3 = MSG.T.viewer.mech(Ty.loc(ty))
      +label4 = MSG.T.bots.tab.browser(Ty.loc(ty))
      +label5 = MSG.T.viewer.diff(Ty.loc(ty))
      +label6 = MSG.T.viewer.files(Ty.loc(ty))
      +label7 = MSG.T.viewer.review(Ty.loc(ty))
      +message = Lay.notice(Ty.loc(ty), busy)
      +help = Lay.help.loc(Ty.loc(ty), k)
      +bw = Lay.button.w(ty, label0)
      +sw = Lay.button.w(ty, label1)
      +tw = Lay.button.w(ty, label2)
      +mw = Lay.button.w(ty, label3)
      +rw = Lay.button.w(ty, label4)
      +dw = Lay.button.w(ty, label5)
      +fw = Lay.button.w(ty, label6)
      +vw = Lay.button.w(ty, label7)
      +tabs = {(x0 + 80 + bw + sw + tw + mw + fw + rw + dw + vw : U32) : U32}
      +ko = Bool.and(Bool.and(Lay.kicad.on(k, empty, ts, bm), Bool.and(Bool.not(MC.Mech.is_model(p)), Bool.not(mech))),
        U32.is_le((tabs + Lay.kicad.w(ty, True{}) + 8 : U32), x1))
      +kw = Lay.kicad.w(ty, ko)
      +room = Lay.room((x1 - x0 : U32), (96 + bw + sw + tw + mw + fw + rw + dw + vw + kw : U32))
      Out.all([
        Out.ops([D.Draw.rect(D.Clip.all(), x0, 0, (x1 - x0 : U32), 52, T.Th.bg(), 255), D.Draw.rect(D.Clip.all(), x0, 51, (x1 - x0 : U32), 1, T.Th.line(), 255),
          D.Draw.rect(D.Clip.all(), x0, 0, 1, h, T.Th.line(), 255)]),
        Lay.fits(U32.is_le((x0 + 16 + bw : U32), (x1 - 8 : U32)), Lay.tab(ty, label0, 0, k, (x0 + 16 : U32))),
        Lay.fits(U32.is_le((x0 + 24 + bw + sw : U32), (x1 - 8 : U32)), Lay.tab(ty, label1, 1, k, (x0 + 24 + bw : U32))),
        Lay.fits(U32.is_le((x0 + 32 + bw + sw + tw : U32), (x1 - 8 : U32)), Lay.chip(ty, (x0 + 32 + bw + sw : U32), 12, label2, Bool.and(U32.is_eq(k, 2), Bool.not(mech)), "vw-kind", "2")),
        Lay.fits(U32.is_le((x0 + 40 + bw + sw + tw + mw : U32), (x1 - 8 : U32)), Lay.chip(ty, (x0 + 40 + bw + sw + tw : U32), 12, label3, Bool.or(U32.is_eq(k, 5), mech), "mech", "")),
        Lay.fits(U32.is_le((x0 + 48 + bw + sw + tw + mw + fw : U32), (x1 - 8 : U32)), Lay.chip(ty, (x0 + 48 + bw + sw + tw + mw : U32), 12, label6, Bool.and(U32.is_ge(k, 6), U32.is_lt(k, 9)), "fm", "")),
        Lay.fits(U32.is_le((x0 + 56 + bw + sw + tw + mw + fw + rw : U32), (x1 - 8 : U32)), Lay.tab(ty, label4, 3, k, (x0 + 56 + bw + sw + tw + mw + fw : U32))),
        Lay.fits(U32.is_le((x0 + 64 + bw + sw + tw + mw + fw + rw + dw : U32), (x1 - 8 : U32)), Lay.tab(ty, label5, 4, k, (x0 + 64 + bw + sw + tw + mw + fw + rw : U32))),
        Lay.fits(U32.is_le((x0 + 72 + bw + sw + tw + mw + fw + rw + dw + vw : U32), (x1 - 8 : U32)), Lay.chip(ty, (x0 + 72 + bw + sw + tw + mw + fw + rw + dw : U32), 12, label7, U32.is_eq(k, 9), "rv-page", "bom")),
        Lay.diff.if(U32.is_eq(k, 4), ty, diff, x0, x1, h, scroll),
        Lay.kicad(ko, ty, x1),
        Lay.fits(U32.is_ge(room, 60), Out.ops(Ty.text(ty, 2n, Ty.fit(ty, 2n, help, room), D.Clip.all(),
          (x1 - 16 - kw - Ty.width(ty, 2n, Ty.fit(ty, 2n, help, room)) : U32), 0, 52, T.Th.faint()))),
        Out.when(Bool.and(Bool.not(String.is_empty(busy)), Bool.not(Lay.shown(k, empty, ts, bm))),
          Out.ops(Ty.text(ty, 3n, message, D.Clip.all(), (x0 + U32.shr((x1 - x0 - U32.min(Ty.width(ty, 3n, message), (x1 - x0 : U32)) : U32)) : U32), (U32.shr(h) - 12 : U32), 23, T.Th.dim()))),
        Lay.inspect(ty, info, draft, x0, x1, h)])

def Lay.picked(x: V.Vx) -> String:
  match x:
    case V.Vx{_, p, _, _, _, _, _, _, _, _, bm}:
      p

def Lay.viewer.vw(+ty: Ty, v: V.Vw, +draft: String, +diff: String, +mech: Bool, +x0: U32, +x1: U32, +h: U32) -> Out:
  match v:
    case V.Vw{+ss, V.View{_, _, oy}, _, _, _, _, _, _, _, _, +vx}:
      Lay.viewer.k(ty, x0, x1, h, List.is_empty(&2, V.Seen, ss), vx, V.Pick.info(ss, Lay.picked(vx)), draft, diff, D.Draw.floor(F32.max(oy, 0.0)), mech)

# Layers and sheets
# -----------------
# Over a board or schematic: a "Layers" chip opening a list of the layers
# the scene has items on (a click turns one on or off, S.Nui.flip), and
# for a schematic with sheets, the trail from the root to the sheet on
# show and the whole sheet tree in the same list (a click goes there).

def Side.color(l: U32 & U32) -> U32:
  (c, _) = l
  c

def Side.has(+present: U32, +i: U32, +count: Nat) -> Bool:
  Bool.and(Nat.is_lt(U32.to_nat(i), count), Bool.not(U32.is_eq(U32.and(present, V.Look.bit(i)), 0)))

# a layer's row: its colour (a click shows or hides it, vw-layer) and its
# name (a click makes it the active layer, the rest dimmed, vw-layer-pick;
# core/layers.bend); the active one lit
def Side.layer(+ty: Ty, +k: U32, +l: U32, +flip: U32, +act: Bool, +x: U32, +y: U32, +w: U32, +c: D.Clip) -> Out:
  +on = V.Look.shows(k, l, flip)
  Out{List.concat(&2, R.Op, [Bool.pick(List<&2, R.Op>, act, [D.Draw.rect(c, (x + 4 : U32), y, (w - 8 : U32), 22, T.Th.sel(), 255)], Nil{}),
      [D.Draw.rect(c, (x + 12 : U32), (y + 6 : U32), 10, 10, Side.color(V.Look.on(k, l, flip)), Bool.pick(U32, on, 255, 50))],
      Ty.text(ty, 2n, LY.Lyr.name(k, l), c, (x + 32 : U32), y, 22, Bool.pick(U32, act, T.Th.accent(), Bool.pick(U32, on, T.Th.text(), T.Th.faint())))]),
    [Lay.hit(x, y, 28, 22, "vw-layer", U32.show(l)), Lay.hit((x + 28 : U32), y, (w - 28 : U32), 22, "vw-layer-pick", U32.show(l))]}

def Side.layer.if(show: Bool, +ty: Ty, +k: U32, +l: U32, +flip: U32, +act: Bool, +x: U32, +y: U32, +w: U32, +c: D.Clip) -> Out:
  match show:
    case True{}:
      Side.layer(ty, k, l, flip, act, x, y, w, c)
    case False{}:
      Out.none()

# one row per layer the scene has, in KiCad's order (LY.Lyr.order), from y down
def Side.layers(ls: List<&2, U32>, +y: U32, +ty: Ty, +k: U32, +present: U32, +flip: U32, +f: LY.Focus, +x: U32, +w: U32, +c: D.Clip) -> List<&2, Out>:
  match ls:
    case Nil{}:
      Nil{}
    case +i <> t:
      Side.layer.if(Side.has(present, i, V.Look.count(k)), ty, k, i, flip, Bool.and(LY.Lyr.on(f), U32.is_eq(i, LY.Lyr.active(f))), x, y, w, c)
      <> Side.layers(t, Bool.pick(U32, Side.has(present, i, V.Look.count(k)), (y + 22 : U32), y), ty, k, present, flip, f, x, w, c)

def Side.n(ls: List<&2, U32>, +present: U32, +count: Nat, +acc: U32) -> U32:
  match ls:
    case Nil{}:
      acc
    case +i <> t:
      Side.n(t, present, count, Bool.pick(U32, Side.has(present, i, count), (acc + 1 : U32), acc))

# the order the list follows (a comparison's marks after the kind's own)
def Side.order(+k: U32) -> List<&2, U32>:
  LY.Lyr.order(U32.or(k, LO.Look.diff()))

# KiCad's high contrast over a board's list: the layers other than the
# active one as they are, dimmed or hidden (vw-layer-mode)
def Side.mode(+ty: Ty, +m: U32, +cur: U32, +label: String, +x: U32, +y: U32, +w: U32, +c: D.Clip) -> Out:
  Out{List.append(&2, R.Op, Lay.box(c, x, y, w, 24, 4, Bool.pick(U32, U32.is_eq(m, cur), T.Th.sel(), T.Th.raise()), Bool.pick(U32, U32.is_eq(m, cur), T.Th.accent(), T.Th.line())),
      Ty.text(ty, 2n, label, c, (x + (w - Ty.width(ty, 2n, label)) / 2 : U32), y, 24, Bool.pick(U32, U32.is_eq(m, cur), T.Th.text(), T.Th.dim()))),
    [Lay.hit(x, y, w, 24, "vw-layer-mode", U32.show(m))]}

def Side.modes(board: Bool, +ty: Ty, +f: LY.Focus, +x: U32, +y: U32, +w: U32, +c: D.Clip) -> Out:
  match board:
    case True{}:
      +cur = {Bool.pick(U32, LY.Lyr.on(f), LY.Lyr.mode(f), 0) : U32}
      +bw = {((w - 24) / 3 : U32) : U32}
      Out.all([Side.mode(ty, 0, cur, MSG.T.viewer.layers.all(Ty.loc(ty)), (x + 12 : U32), y, bw, c), Side.mode(ty, 1, cur, MSG.T.viewer.layers.dim(Ty.loc(ty)), (x + 12 + bw : U32), y, bw, c),
        Side.mode(ty, 2, cur, MSG.T.viewer.layers.only(Ty.loc(ty)), (x + 12 + bw * 2 : U32), y, bw, c)])
    case False{}:
      Out.none()

def Side.sheet(+ty: Ty, r: HI.Row, +j: Nat, +at: Nat, +x: U32, +y: U32, +w: U32, +c: D.Clip) -> Out:
  match r:
    case HI.Row{+d, +nm, _, +loop}:
      +ind = {U32.from_nat(Nat.mul(d, 14n)) : U32}
      Out{List.append(&2, R.Op, Bool.pick(List<&2, R.Op>, Nat.is_eq(j, at), [D.Draw.rect(c, x, y, w, 22, T.Th.sel(), 255)], Nil{}),
          Ty.text(ty, 2n, MSG.T.viewer.sheet.title(Ty.loc(ty), nm, Bool.pick(String, loop, "loop", "other")), c, (x + 12 + ind : U32), y, 22,
            Bool.pick(U32, loop, T.Th.faint(), Bool.pick(U32, Nat.is_eq(j, at), T.Th.text(), T.Th.dim())))),
        Bool.pick(List<&2, S.Hit>, loop, Nil{}, [Lay.hit(x, y, w, 22, "vw-sheet", Nat.show(j))])}

def Side.sheets(rs: List<&2, HI.Row>, +j: Nat, +at: Nat, +ty: Ty, +x: U32, +y: U32, +w: U32, +c: D.Clip) -> List<&2, Out>:
  match rs:
    case Nil{}:
      Nil{}
    case r <> t:
      Side.sheet(ty, r, j, at, x, y, w, c) <> Side.sheets(t, 1n+j, at, ty, x, (y + 22 : U32), w, c)

def Side.reset(+ty: Ty, +x: U32, +y: U32, +c: D.Clip) -> Out:
  Out{Ty.text(ty, 2n, MSG.T.viewer.layers.reset(Ty.loc(ty)), c, (x + 12 : U32), y, 22, T.Th.accent()), [Lay.hit(x, y, 200, 22, "vw-layers-reset", "")]}

# the open list: the sheet tree (schematics with sheets), then a board's
# All/Dim/Only, then the layers
def Side.list(+ty: Ty, +k: U32, +present: U32, +flip: U32, +f: LY.Focus, +rs: List<&2, HI.Row>, +at: Nat, +x1: U32, +h: U32) -> Out:
  +w = {240 : U32}
  +x = {(x1 - 16 - w : U32) : U32}
  +ns = {U32.from_nat(Bool.pick(Nat, Nat.is_gt(List.length(&2, HI.Row, rs), 1n), List.length(&2, HI.Row, rs), 0n)) : U32}
  +sh = {Bool.pick(U32, U32.is_eq(ns, 0), 0, (ns * 22 + 9 : U32)) : U32}
  +board = U32.is_eq(k, 0)
  +mh = {Bool.pick(U32, board, 32, 0) : U32}
  +nl = {Side.n(Side.order(k), present, V.Look.count(k), 0) : U32}
  +reset = Bool.or(Bool.not(U32.is_eq(U32.and(flip, 536870911), 0)), LY.Lyr.on(f))
  +rh = {Bool.pick(U32, reset, 22, 0) : U32}
  +ph = {U32.min((sh + mh + nl * 22 + rh + 12 : U32), (h - 112 : U32)) : U32}
  +c = {D.Clip{x, 96, (x + w : U32), (96 + ph : U32)} : D.Clip}
  Out.all([
    Out{Lay.box(D.Clip.all(), x, 96, w, ph, 6, T.Th.panel(), T.Th.line()), [Lay.hit(x, 96, w, ph, "vw-panel", "")]},
    Out.all(Side.sheets(Bool.pick(List<&2, HI.Row>, U32.is_eq(ns, 0), Nil{}, rs), 0n, at, ty, x, 102, w, c)),
    Out.ops(Bool.pick(List<&2, R.Op>, U32.is_eq(ns, 0), Nil{}, [D.Draw.rect(c, (x + 8 : U32), (102 + sh - 5 : U32), (w - 16 : U32), 1, T.Th.line(), 255)])),
    Side.modes(board, ty, f, x, (102 + sh : U32), w, c),
    Out.all(Side.layers(Side.order(k), (102 + sh + mh : U32), ty, k, present, flip, f, x, w, c)),
    Out.when(reset, Side.reset(ty, x, (102 + sh + mh + nl * 22 : U32), c))])

def Side.list.if(open: Bool, +ty: Ty, +k: U32, +ss: List<&2, V.Seen>, +flip: U32, +f: LY.Focus, +rs: List<&2, HI.Row>, +at: Nat, +x1: U32, +h: U32) -> Out:
  match open:
    case True{}:
      Side.list(ty, k, V.Layers.present(ss, 0), flip, f, rs, at, x1, h)
    case False{}:
      Out.none()

# the sheets from the root to the one on show, each a chip that goes there
def Side.trail(cs: List<&2, Nat>, +rs: List<&2, HI.Row>, +at: Nat, +ty: Ty, +x: U32) -> List<&2, Out>:
  match cs:
    case Nil{}:
      Nil{}
    case +j <> t:
      Lay.chip(ty, x, 60, HI.Hier.name(rs, j), Nat.is_eq(j, at), "vw-sheet", Nat.show(j))
      <> Side.trail(t, rs, at, ty, (x + Lay.button.w(ty, HI.Hier.name(rs, j)) + 6 : U32))

# the project's boards as chips when it has several (client.bend's Bd),
# before the sheet trail; their width with a gap after each
def Side.boards(fs: List<&2, String>, +cur: String, +ty: Ty, +x: U32) -> List<&2, Out>:
  match fs:
    case Nil{}:
      Nil{}
    case +f <> t:
      Lay.chip(ty, x, 60, U.Bd.name(f), U.Bd.on(cur, f), "board-pick", f)
      <> Side.boards(t, cur, ty, (x + Lay.button.w(ty, U.Bd.name(f)) + 6 : U32))

def Side.boards.w(fs: List<&2, String>, +ty: Ty) -> U32:
  match fs:
    case Nil{}:
      0
    case +f <> t:
      (Lay.button.w(ty, U.Bd.name(f)) + 6 + Side.boards.w(t, ty) : U32)

def Side.board.fs(+u: U.Ui) -> List<&2, String>:
  Bool.pick(List<&2, String>, U.Bd.many(u), U.Bd.files(u), Nil{})

# the viewer's own light or dark ground (the sun shows on the dark one,
# the moon on the light), whatever the window's theme
def Side.ground(+ty: Ty, +n: S.Nui, +x: U32) -> Out:
  Lay.ibutton(ty, D.Clip.all(), x, 60, Bool.pick(String, S.Nui.vlight(n), "moon", "sun"), T.Th.soft(), "vw-light", "")

# 3D has no layer list: the ground button alone, where Layers would be
def Side.ground.if(on: Bool, +ty: Ty, +n: S.Nui, +x1: U32) -> Out:
  match on:
    case True{}:
      Side.ground(ty, n, (x1 - 16 - 28 : U32))
    case False{}:
      Out.none()

# the Layers chip names the active layer while one stands out
def Side.head.loc(+l: I.Loc, +k: U32, +f: LY.Focus) -> String:
  Bool.pick(String, LY.Lyr.on(f), MSG.T.viewer.layers.active(l, LY.Lyr.name(k, LY.Lyr.active(f))), MSG.T.viewer.layers(l))

def Side.head(+k: U32, +f: LY.Focus) -> String:
  Side.head.loc(I.En{}, k, f)

def Side.k(board: Bool, +ty: Ty, +n: S.Nui, +k: U32, +ss: List<&2, V.Seen>, +x0: U32, +x1: U32, +h: U32) -> Out:
  match board:
    case True{}:
      +rs = S.Nui.rows(n)
      +at = S.Nui.sheet.at(n)
      +open = Bool.not(String.is_empty(U.Ui.scratch(S.Nui.ui(n), "@vw.layers")))
      +sheets = Bool.and(U32.is_eq(k, 1), Nat.is_gt(List.length(&2, HI.Row, rs), 1n))
      Out.all([
        Out.all(Side.boards(Side.board.fs(S.Nui.ui(n)), U.Bd.shown(S.Nui.ui(n), U.Ui.view_get(S.Nui.ui(n), "view.path")), ty, (x0 + 16 : U32))),
        Out.all(Side.trail(Bool.pick(List<&2, Nat>, sheets, HI.Hier.chain(rs, at), Nil{}), rs, at, ty, (x0 + 16 + Side.boards.w(Side.board.fs(S.Nui.ui(n)), ty) : U32))),
        Lay.chip(ty, (x1 - 16 - Lay.button.w(ty, Side.head.loc(Ty.loc(ty), k, S.Nui.lfocus(n))) : U32), 60, Side.head.loc(Ty.loc(ty), k, S.Nui.lfocus(n)), open, "vw-layers", ""),
        Side.ground(ty, n, (x1 - 22 - Lay.button.w(ty, Side.head.loc(Ty.loc(ty), k, S.Nui.lfocus(n))) - 28 : U32)),
        Side.list.if(open, ty, k, ss, S.Nui.look(n), S.Nui.lfocus(n), Bool.pick(List<&2, HI.Row>, sheets, rs, Nil{}), at, x1, h)])
    case False{}:
      Out.none()

def Lay.toast(+ty: Ty, +e: String, +w: U32, +h: U32) -> Out:
  +tw = {(Ty.width(ty, 2n, e) + 28 : U32) : U32}
  Bool.pick(Out, String.is_empty(e), Out.none(),
    Out{List.append(&2, R.Op, Lay.box(D.Clip.all(), (w - tw - 16 : U32), (h - 52 : U32), tw, 32, 8, T.Th.raise(), T.Th.bad()),
        Ty.text(ty, 2n, e, D.Clip.all(), (w - tw - 2 : U32), (h - 52 : U32), 32, T.Th.bad())),
      [Lay.hit((w - tw - 16 : U32), (h - 52 : U32), tw, 32, "dismiss", "")]})

def Lay.rev(o: Out) -> Out:
  match o:
    case Out{os, hs}:
      Out{List.reverse(&2, R.Op, os), List.reverse(&2, S.Hit, hs)}

def Lay.scene(n: S.Nui) -> List<&2, V.Seen>:
  match n:
    case S.Nui{_, _, _, _, _, _, _, _, _, _, _, _, V.Vw{ss, _, _, _, _, _, _, _, _, _, vx}}:
      ss

# the viewer's chrome; the scene under it is drawn by the viewer
# A browser page
# --------------
# Each thread's agent drives a page of its own (a bot's is the bot's): the
# hub writes its frames as <home>/shots/<page>.jpg and says when a new one
# is there ("shot.<page>", nui.bend's Shot). The frame is an image the
# window loads ("img" regions, main.bend's Pics) under the key
# "shot:<page>:<n>"; the one last loaded ("shot.ok.<page>") shows while
# the next loads.

# the frame fitted to the pane (never enlarged), centred, with a rule round it
def Page.hits(input: Bool, +x: U32, +y: U32, +w: U32, +h: U32, +key: String) -> List<&2, S.Hit>:
  match input:
    case True{}:
      [Lay.hit(x, y, w, h, "bb-frame", U32.show(x) ++ " " ++ U32.show(y) ++ " " ++ U32.show(w) ++ " " ++ U32.show(h)), Lay.hit(x, y, w, h, "img", key)]
    case False{}:
      [Lay.hit(x, y, w, h, "img", key)]

# the frame fitted to the pane (never enlarged), centred, with a rule round
# it; while the person drives it (`input`) the rule is the accent and a
# press on it is a click in the page (nui.bend's Bb)
def Page.draw(+key: String, +size: String, +x0: U32, +x1: U32, +top: U32, +h: U32, +input: Bool) -> Out:
  +bw = Page.num(size, 0n)
  +bh = Page.num(size, 1n)
  +fw = Page.fit.w(bw, bh, (x1 - x0 - 48 : U32), (h - top - 24 : U32))
  +fh = {((fw * bh : U32) / U32.max(bw, 1) : U32) : U32}
  +x = {(x0 + U32.shr((x1 - x0 - fw : U32)) : U32) : U32}
  Out{D.Draw.frame(D.Clip.all(), (x - 1 : U32), (top - 1 : U32), (fw + 2 : U32), (fh + 2 : U32), Bool.pick(U32, input, T.Th.accent(), T.Th.line())), Page.hits(input, x, top, fw, fh, key)}

def Page.k(state: Nat, +ty: Ty, +key: String, +size: String, +none: String, +x0: U32, +x1: U32, +top: U32, +h: U32, +input: Bool) -> Out:
  match state:
    case 0n:
      Out.ops(Ty.text(ty, 2n, MSG.T.viewer.page.loading(Ty.loc(ty)), D.Clip.all(), (x0 + 24 : U32), top, 28, T.Th.faint()))
    case 1n:
      Out.ops(Ty.text(ty, 2n, none, D.Clip.all(), (x0 + 24 : U32), top, 28, T.Th.faint()))
    case _:
      Page.draw(key, size, x0, x1, top, h, input)

def Page.state(+shown: String, +size: String, +next: String) -> Nat:
  Bool.pick(Nat, Bool.and(Bool.not(String.is_empty(shown)), Bool.not(String.is_empty(size))), 2n, Bool.pick(Nat, String.starts_with(next, "err:"), 1n, 0n))

# page id's newest frame from top down, or `none` while it has none
def Lay.page.jpeg(+ty: Ty, +u: U.Ui, +id: String, +none: String, +x0: U32, +x1: U32, +top: U32, +h: U32, +input: Bool) -> Out:
  +n = U.Ui.info(u, "shot." ++ id)
  +key = "shot:" ++ id ++ ":" ++ Bool.pick(String, String.is_empty(n), "0", n)
  +shown = U.Ui.info(u, "shot.ok." ++ id)
  +size = U.Ui.info(u, "img:" ++ shown)
  Out.cat(Out{Nil{}, [Lay.hit(x0, top, 1, 1, "img", key)]},
    Page.k(Page.state(shown, size, U.Ui.info(u, "img:" ++ key)), ty, shown, size, none, x0, x1, (top + 12 : U32), h, input))

# The embedded page (core/cef.bend)
# ---------------------------------
# With the engine "cef" the page is a native child window of this one: the
# window draws a plain background and leaves one "cef" hit, the pane's
# rectangle, for main.bend to place it by (browser.place). No hit while an
# overlay drawn over the main area is open (a toast does not hide it), nor
# before the page exists (the text says so, as it does without cef), so an
# empty Browser tab never makes a page.
def Page.cef.k(has: Bool, shows: Bool, +ty: Ty, +id: String, +none: String, +x0: U32, +x1: U32, +top: U32, +h: U32) -> Out:
  match has:
    case True{}:
      Out{[D.Draw.rect(D.Clip.all(), x0, top, (x1 - x0 : U32), U32.dim(h, top), T.Th.bg(), 255)],
        Bool.pick(List<&2, S.Hit>, shows, [Lay.hit(x0, top, (x1 - x0 : U32), U32.dim(h, top), "cef", id)], Nil{})}
    case False{}:
      Out.ops(Ty.text(ty, 2n, none, D.Clip.all(), (x0 + 24 : U32), (top + 12 : U32), 28, T.Th.faint()))

def Page.cef(+ty: Ty, +u: U.Ui, +id: String, +none: String, +x0: U32, +x1: U32, +top: U32, +h: U32) -> Out:
  Page.cef.k(Bool.or(Bool.not(String.is_empty(U.Ui.info(u, "shot." ++ id))), Bool.not(String.is_empty(U.Ui.info(u, CF.Cef.page.key(id))))),
    CF.Cef.shows(True{}, U.CefUI.covered(u)), ty, id, none, x0, x1, top, h)

def Lay.page.engine(native: Bool, +ty: Ty, +u: U.Ui, +id: String, +none: String, +x0: U32, +x1: U32, +top: U32, +h: U32, +input: Bool) -> Out:
  match native:
    case True{}: Page.cef(ty, u, id, none, x0, x1, top, h)
    case False{}: Lay.page.jpeg(ty, u, id, none, x0, x1, top, h, input)

def Lay.page.in(+ty: Ty, +u: U.Ui, +id: String, +none: String, +x0: U32, +x1: U32, +top: U32, +h: U32, +input: Bool) -> Out:
  Lay.page.engine(U.CefUI.native(u, id), ty, u, id, none, x0, x1, top, h, input)

def Lay.page(+ty: Ty, +u: U.Ui, +id: String, +none: String, +x0: U32, +x1: U32, +top: U32, +h: U32) -> Out:
  Lay.page.in(ty, u, id, none, x0, x1, top, h, False{})

def Page.back(on: Bool, +ty: Ty, +u: U.Ui, +x1: U32) -> Out:
  match on:
    case True{}: Lay.button(ty, D.Clip.all(), U32.dim(x1, (Lay.button.w(ty, "Hand back") + 16 : U32)), 58, "Hand back", T.Th.soft(), "tk-handback", U.Ui.page(u))
    case False{}: Out.none()

def Page.strip(on: Bool, +ty: Ty, +u: U.Ui, +x0: U32, +x1: U32) -> Out:
  match on:
    case True{}:
      Out.cat(Out.ops(List.append(&2, R.Op, [D.Draw.rect(D.Clip.all(), x0, 52, (x1 - x0 : U32), 44, T.Th.bg(), 255)],
          Ty.text(ty, 2n, U.CefUI.strip(u, U.Ui.page(u)), D.Clip.all(), (x0 + 16 : U32), 58, 32, T.Th.dim()))),
        Page.back(Bool.not(String.is_empty(U.Teach.drive(u, U.Ui.page(u)))), ty, u, x1))
    case False{}: Out.none()

# the Browser tab: the selected thread's own page
def Lay.page.if(on: Bool, +ty: Ty, +u: U.Ui, +x0: U32, +x1: U32, +h: U32) -> Out:
  match on:
    case True{}:
      Out.cat(Page.strip(U.CefUI.strip_on(u, U.Ui.page(u)), ty, u, x0, x1),
        Out.cat(Lay.button(ty, D.Clip.all(), (x0 + 24 : U32), Bool.pick(U32, U.CefUI.strip_on(u, U.Ui.page(u)), 100, 58), U.Teach.control.label(u, U.Ui.page(u)), T.Th.soft(), U.Teach.control.action(u, U.Ui.page(u)), ""),
        Lay.page.in(ty, u, U.Ui.page(u), MSG.T.viewer.page.empty(Ty.loc(ty)), x0, x1, Bool.pick(U32, U.CefUI.strip_on(u, U.Ui.page(u)), 138, 96), h, U.Teach.input(u, U.Ui.page(u)))))
    case False{}:
      Out.none()

# The Mechanical page
# -------------------
# The selected thread's parts (client.bend's Mpage): a chip for each part,
# Open in 3D and Refresh, then the part's renders two by two, each fitted
# to its cell under its view's name. A render is an image the window loads
# ("img" regions, main.bend's Pics) under its key "v:<sum>:<path>", so a
# render drawn again is loaded again.

def Mech.chips(ps: List<&2, MC.MPart>, +ty: Ty, +cur: String, +x0: U32, +x1: U32, +x: U32, +y: U32) -> Out:
  match ps:
    case Nil{}:
      Out.none()
    case MC.MPart{+p, +name, _} <> t:
      +w = Lay.button.w(ty, name)
      +wrap = Bool.and(U32.is_gt((x + w : U32), x1), U32.is_gt(x, x0))
      +cx = Bool.pick(U32, wrap, x0, x)
      +cy = Bool.pick(U32, wrap, (y + 36 : U32), y)
      Out.cat(Lay.chip(ty, cx, cy, name, String.eq(p, cur), "mech-part", p), Mech.chips(t, ty, cur, x0, x1, (cx + w + 8 : U32), cy))

# how far down the chips reach (the same walk, heights only)
def Mech.chips.h(ps: List<&2, MC.MPart>, +ty: Ty, +x0: U32, +x1: U32, +x: U32, +y: U32) -> U32:
  match ps:
    case Nil{}:
      (y + 28 : U32)
    case MC.MPart{_, +name, _} <> t:
      +w = Lay.button.w(ty, name)
      +wrap = Bool.and(U32.is_gt((x + w : U32), x1), U32.is_gt(x, x0))
      Mech.chips.h(t, ty, x0, x1, (Bool.pick(U32, wrap, x0, x) + w + 8 : U32), Bool.pick(U32, wrap, (y + 36 : U32), y))

# one render in a cell: its view's name, then the image fitted (never
# enlarged) and centred, with a rule round it; "loading" until its size is known
def Mech.shot(+ty: Ty, +u: U.Ui, +root: String, +f: MC.MFile, +x: U32, +y: U32, +cw: U32, +ch: U32) -> Out:
  +key = MC.Mech.key(root, f)
  +size = U.Ui.info(u, "img:" ++ key)
  +bw = Page.num(size, 0n)
  +bh = Page.num(size, 1n)
  +fw = Page.fit.w(bw, bh, cw, (ch - 24 : U32))
  +fh = {((fw * bh : U32) / U32.max(bw, 1) : U32) : U32}
  +ix = {(x + U32.shr((cw - fw : U32)) : U32) : U32}
  Out.all([
    Out.ops(Ty.text(ty, 2n, MC.Mech.view.of(f), D.Clip.all(), x, y, 20, T.Th.faint())),
    Out.when(String.is_empty(size), Out.ops(Ty.text(ty, 2n, Bool.pick(String, String.starts_with(U.Ui.info(u, "img:" ++ key), "err:"), MSG.T.image.failed(Ty.loc(ty)), MSG.T.image.loading(Ty.loc(ty))), D.Clip.all(), (x + 12 : U32), (y + 40 : U32), 20, T.Th.faint()))),
    Out{Bool.pick(List<&2, R.Op>, String.is_empty(size), Nil{}, D.Draw.frame(D.Clip.all(), (ix - 1 : U32), (y + 23 : U32), (fw + 2 : U32), (fh + 2 : U32), T.Th.line())),
      [Lay.hit(ix, (y + 24 : U32), U32.max(fw, 1), U32.max(fh, 1), "img", key)]}])

def Mech.grid(fs: List<&2, MC.MFile>, +i: U32, +ty: Ty, +u: U.Ui, +root: String, +x0: U32, +y0: U32, +cw: U32, +ch: U32) -> Out:
  match fs:
    case Nil{}:
      Out.none()
    case +f <> t:
      Out.cat(Mech.shot(ty, u, root, f, (x0 + (U32.and(i, 1) * (cw + 16 : U32) : U32) : U32), (y0 + (U32.shr(i) * (ch + 12 : U32) : U32) : U32), cw, ch),
        Mech.grid(t, (i + 1 : U32), ty, u, root, x0, y0, cw, ch))

def Mech.renders.k(none: Bool, +ty: Ty, +u: U.Ui, +name: String, fs: List<&2, MC.MFile>, +x0: U32, +x1: U32, +y: U32, +h: U32) -> Out:
  match none:
    case True{}:
      Out.ops(Ty.text(ty, 2n, Ty.fit(ty, 2n, MC.Mech.unrendered(name), (x1 - x0 - 32 : U32)), D.Clip.all(), (x0 + 16 : U32), y, 28, T.Th.faint()))
    case False{}:
      Mech.grid(fs, 0, ty, u, U.Mpage.root(u), (x0 + 16 : U32), y, U32.shr((x1 - x0 - 48 : U32)),
        U32.min(U32.shr(Lay.room(h, (y + 28 : U32))), (((U32.shr((x1 - x0 - 48 : U32)) * 3 : U32) / 4 : U32) + 28 : U32)))

def Mech.part(m: Maybe<&2, MC.MPart>, +ty: Ty, +u: U.Ui, +x0: U32, +x1: U32, +y: U32, +h: U32) -> Out:
  match m:
    case Some{MC.MPart{_, +name, +fs}}:
      Mech.renders.k(List.is_empty(&2, MC.MFile, fs), ty, u, name, fs, x0, x1, y, h)
    case None{}:
      Out.none()

def Mech.say(+ty: Ty, +text: String, +x0: U32, +x1: U32, +y: U32) -> Out:
  Out.ops(Ty.text(ty, 2n, Ty.fit(ty, 2n, text, (x1 - x0 - 32 : U32)), D.Clip.all(), (x0 + 16 : U32), y, 28, T.Th.faint()))

def Mech.page.k(state: Nat, +ty: Ty, +u: U.Ui, +x0: U32, +x1: U32, +h: U32) -> Out:
  match state:
    case 0n:
      Mech.say(ty, MSG.T.viewer.mech.loading(Ty.loc(ty)), x0, x1, 68)
    case 1n:
      Mech.say(ty, MC.Mech.none(), x0, x1, 68)
    case _:
      +label0 = MSG.T.viewer.solid(Ty.loc(ty))
      +label1 = MSG.T.common.refresh(Ty.loc(ty))
      +cur = U.Mpage.cur.path(u)
      +bw = Lay.button.w(ty, label0)
      +gl = U.Mpage.render.label.loc(Ty.loc(ty), u)
      +gw = Lay.button.w(ty, gl)
      +rw = Lay.button.w(ty, label1)
      +cx1 = {(x1 - 40 - bw - gw - rw : U32) : U32}
      +top = {(Mech.chips.h(U.Mpage.parts(u), ty, (x0 + 16 : U32), cx1, (x0 + 16 : U32), 64) + 16 : U32) : U32}
      +err = U.Mpage.err(u)
      Out.all([
        Mech.chips(U.Mpage.parts(u), ty, cur, (x0 + 16 : U32), cx1, (x0 + 16 : U32), 64),
        Lay.button(ty, D.Clip.all(), (x1 - 32 - bw - gw - rw : U32), 64, label0, T.Th.text(), "mech-3d", MC.Mech.abs(U.Mpage.root(u), cur)),
        Lay.button(ty, D.Clip.all(), (x1 - 24 - gw - rw : U32), 64, gl, Bool.pick(U32, String.is_empty(U.Mpage.busy(u)), T.Th.accent(), T.Th.faint()),
          Bool.pick(String, String.is_empty(U.Mpage.busy(u)), "mech-render", ""), cur),
        Lay.button(ty, D.Clip.all(), (x1 - 16 - rw : U32), 64, label1, T.Th.text(), "mech-refresh", ""),
        Out.when(Bool.not(String.is_empty(err)), Mech.say(ty, MSG.T.viewer.render.failed(Ty.loc(ty), err), x0, x1, (top - 6 : U32))),
        Mech.part(U.Mpage.cur(u), ty, u, x0, x1, Bool.pick(U32, String.is_empty(err), top, (top + 28 : U32)), h)])

# over a part in 3D: the thread's parts (a click shows another in 3D) and
# Renders, its pictures
# after the parts: Renders for one of them, or the name of another model
# an agent showed
def Mech.bar.more(listed: Bool, +ty: Ty, +u: U.Ui, +x1: U32, +rw: U32) -> Out:
  match listed:
    case True{}:
      Lay.button(ty, D.Clip.all(), (x1 - 60 - rw : U32), 64, MSG.T.viewer.mech.renders(Ty.loc(ty)), T.Th.text(), "mech-renders", U.Ui.view_get(u, "view.path"))
    case False{}:
      Lay.chip(ty, (x1 - 60 - Lay.button.w(ty, Ty.fit(ty, 2n, MC.Mech.last(U.Ui.view_get(u, "view.path")), 220)) : U32), 64, Ty.fit(ty, 2n, MC.Mech.last(U.Ui.view_get(u, "view.path")), 220), True{}, "", "")

def Mech.bar.if(on: Bool, +ty: Ty, +u: U.Ui, +x0: U32, +x1: U32) -> Out:
  match on:
    case True{}:
      +rw = Lay.button.w(ty, MSG.T.viewer.mech.renders(Ty.loc(ty)))
      Out.all([
        Mech.chips(U.Mpage.parts(u), ty, U.Mpage.shown(u), (x0 + 16 : U32), (x1 - 68 - U32.max(rw, U32.min((Ty.width(ty, 2n, MC.Mech.last(U.Ui.view_get(u, "view.path"))) + 20 : U32), 240)) : U32), (x0 + 16 : U32), 64),
        Mech.bar.more(U.Mpage.listed(u), ty, u, x1, rw)])
    case False{}:
      Out.none()

# the Mechanical tab: the page, and where to get FreeCAD when the hub has none
def Lay.mech.if(on: Bool, +ty: Ty, +u: U.Ui, +x0: U32, +x1: U32, +h: U32) -> Out:
  match on:
    case True{}:
      +note = U.Mpage.note.loc(Ty.loc(ty), u)
      Out.cat(Mech.page.k(U.Mpage.state(u), ty, u, x0, x1, Bool.pick(U32, String.is_empty(note), h, (h - 36 : U32))),
        Out.when(Bool.not(String.is_empty(note)), Mech.say(ty, note, x0, x1, (h - 36 : U32))))
    case False{}:
      Out.none()

# The design history bar
# ----------------------
# Under a board or schematic with a history (core/client.bend's Hs.bar):
# the arrows, play and the position, a scrubber with a tick a step, and
# what the agent said before the step with a way back to it; a comparison
# names its two sides (each opens the menu of versions) over a legend. The
# inspector and the layer list sit above it (Hb.h).

def Hb.on(b: U.HBar) -> Bool:
  match b:
    case U.HBar{on, _, _, _, _, _, _, _, _, _, _, _, _}:
      on

def Hb.cmp(b: U.HBar) -> Bool:
  match b:
    case U.HBar{_, _, _, _, _, _, _, _, _, c, _, _, _}:
      c

# how much of the panel's foot the bar takes
def Hb.h(+b: U.HBar) -> U32:
  U.HBar.h(b)

def Hb.ground(+x0: U32, +x1: U32, +y: U32, +h: U32) -> Out:
  Out{[D.Draw.rect(D.Clip.all(), x0, y, (x1 - x0 : U32), 1, T.Th.line(), 255), D.Draw.rect(D.Clip.all(), x0, (y + 1 : U32), (x1 - x0 : U32), (h - 1 : U32), T.Th.bg(), 255)],
    [Lay.hit(x0, y, (x1 - x0 : U32), h, "vw-panel", "")]}

# the scrubber: a tick a step and one for the live file at the end
def Hb.tick(+ty: Ty, +id: String, +i: U32, +n: U32, +at: U32, +xa: U32, +sw: U32, +y: U32) -> Out:
  +x = {(xa + ((i * sw : U32) / U32.max(n, 1) : U32) : U32) : U32}
  +cw = {U32.max((sw / U32.max(n, 1) : U32), 6) : U32}
  +cur = U32.is_eq(i, at)
  Out{[D.Draw.rect(D.Clip.all(), (x - Bool.pick(U32, cur, 2, 1) : U32), (y - Bool.pick(U32, cur, 6, 4) : U32), Bool.pick(U32, cur, 4, 2), Bool.pick(U32, cur, 16, 12),
      Bool.pick(U32, cur, T.Th.accent(), T.Th.dim()), 255)],
    [Lay.hit((x - U32.shr(cw) : U32), (y - 8 : U32), cw, 20, "hist-go", id)]}

def Hb.ticks(ss: List<&2, HS.Step>, +ty: Ty, +i: U32, +n: U32, +at: U32, +xa: U32, +sw: U32, +y: U32) -> Out:
  match ss:
    case Nil{}:
      Hb.tick(ty, "", i, n, at, xa, sw, y)
    case +s <> t:
      Out.cat(Hb.tick(ty, HS.Step.id(s), i, n, at, xa, sw, y), Hb.ticks(t, ty, (i + 1 : U32), n, at, xa, sw, y))

# the scrubber's positions run 0 (the first step) to n (live)
def Hb.scrub(+ty: Ty, +ss: List<&2, HS.Step>, +n: Nat, +at: Nat, +x0: U32, +x1: U32, +y: U32) -> Out:
  +sw = {(x1 - x0 - 32 : U32) : U32}
  Out.cat(Out.ops([D.Draw.rect(D.Clip.all(), (x0 + 16 : U32), (y + 1 : U32), sw, 2, T.Th.line(), 255)]),
    Hb.ticks(ss, ty, 0, U32.from_nat(n), Bool.pick(U32, Nat.is_eq(at, 0n), U32.from_nat(n), (U32.from_nat(at) - 1 : U32)), (x0 + 16 : U32), sw, y))

def Hb.chips.w(+ty: Ty, +live: Bool) -> U32:
  (Lay.button.w(ty, MSG.T.viewer.history.changes(Ty.loc(ty))) + Lay.button.w(ty, MSG.T.viewer.history.compare(Ty.loc(ty))) + Bool.pick(U32, live, (Lay.button.w(ty, MSG.T.history.live(Ty.loc(ty))) + 8 : U32), 0) + 8 : U32)

def Hb.live(on: Bool, +ty: Ty, +x: U32, +y: U32) -> Out:
  match on:
    case True{}:
      Lay.button(ty, D.Clip.all(), x, y, MSG.T.history.live(Ty.loc(ty)), T.Th.accent(), "hist-live", "")
    case False{}:
      Out.none()

# the changes, compare and live buttons, from xr
def Hb.chips(+ty: Ty, +diff: Bool, +at: Nat, +xr: U32, +y: U32) -> Out:
  +label0 = MSG.T.viewer.history.changes(Ty.loc(ty))
  +label1 = MSG.T.viewer.history.compare(Ty.loc(ty))
  Out.all([
    Lay.chip(ty, xr, y, label0, diff, "hist-diff", ""),
    Lay.button(ty, D.Clip.all(), (xr + Lay.button.w(ty, label0) + 8 : U32), y, label1, T.Th.text(), "cmp", "a"),
    Hb.live(Nat.is_gt(at, 0n), ty, (xr + Lay.button.w(ty, label0) + Lay.button.w(ty, label1) + 16 : U32), y)])

# whether the buttons fit beside the arrows (else they go on the last row)
def Hb.wide(+ty: Ty, +at: Nat, +x0: U32, +x1: U32) -> Bool:
  U32.is_ge((x1 - x0 : U32), (Hb.chips.w(ty, Nat.is_gt(at, 0n)) + 290 : U32))

def Hb.chips.if(on: Bool, +ty: Ty, +diff: Bool, +at: Nat, +xr: U32, +y: U32) -> Out:
  match on:
    case True{}:
      Hb.chips(ty, diff, at, xr, y)
    case False{}:
      Out.none()

# the arrows, play, the position, and on the right the buttons (when wide)
def Hb.controls(+ty: Ty, +title: String, +playing: Bool, +diff: Bool, +at: Nat, +wide: Bool, +x0: U32, +x1: U32, +y: U32) -> Out:
  +xr = {Bool.pick(U32, wide, (x1 - 16 - Hb.chips.w(ty, Nat.is_gt(at, 0n)) : U32), x1) : U32}
  Out.all([
    Lay.ibutton(ty, D.Clip.all(), (x0 + 16 : U32), y, "skip-back", T.Th.soft(), "hist-first", ""),
    Lay.ibutton(ty, D.Clip.all(), (x0 + 48 : U32), y, "chevron-left", T.Th.soft(), "hist-prev", ""),
    Lay.ibutton(ty, D.Clip.all(), (x0 + 80 : U32), y, Bool.pick(String, playing, "pause", "play"), T.Th.accent(), "hist-play", ""),
    Lay.ibutton(ty, D.Clip.all(), (x0 + 112 : U32), y, "chevron-right", T.Th.soft(), "hist-next", ""),
    Lay.ibutton(ty, D.Clip.all(), (x0 + 144 : U32), y, "skip-forward", T.Th.soft(), "hist-last", ""),
    Out.ops(Ty.text(ty, 2n, Ty.fit(ty, 2n, title, Lay.room(xr, (x0 + 196 : U32))), D.Clip.all(), (x0 + 186 : U32), y, 28, T.Th.text())),
    Hb.chips.if(wide, ty, diff, at, xr, y)])

# what the step followed: what the agent said before it (else the tool
# call), and a way back to it in the thread; x1 is where the text stops
def Hb.said(none: Bool, +ty: Ty, +text: String, +x0: U32, +x1: U32, +y: U32) -> Out:
  match none:
    case True{}:
      Out.ops(Ty.text(ty, 2n, Ty.fit(ty, 2n, text, Lay.room(x1, (x0 + 32 : U32))), D.Clip.all(), (x0 + 16 : U32), y, 28, T.Th.faint()))
    case False{}:
      Out.cat(Lay.ibutton(ty, D.Clip.all(), (x0 + 16 : U32), y, "message-square", T.Th.accent(), "hist-jump", ""),
        Out{Ty.text(ty, 2n, Ty.fit(ty, 2n, text, Lay.room(x1, (x0 + 68 : U32))), D.Clip.all(), (x0 + 52 : U32), y, 28, T.Th.dim()),
          [Lay.hit((x0 + 52 : U32), y, Lay.room(x1, (x0 + 68 : U32)), 28, "hist-jump", "")]})

def Hb.walk(+ty: Ty, +u: U.Ui, b: U.HBar, +x0: U32, +x1: U32, +y: U32) -> Out:
  match b:
    case U.HBar{_, +n, +at, +title, _, +label, +said, +pl, +df, _, _, _, _}:
      +wide = Hb.wide(ty, at, x0, x1)
      +xs = {Bool.pick(U32, wide, x1, (x1 - 16 - Hb.chips.w(ty, Nat.is_gt(at, 0n)) : U32)) : U32}
      Out.all([Hb.ground(x0, x1, y, 96), Hb.controls(ty, title, pl, df, at, wide, x0, x1, (y + 8 : U32)),
        Hb.scrub(ty, U.Hs.track(u), n, at, x0, x1, (y + 46 : U32)),
        Hb.said(Nat.is_eq(at, 0n), ty, Bool.pick(String, Nat.is_eq(at, 0n), U.Hs.live.said.loc(Ty.loc(ty), n),
          Bool.pick(String, String.is_empty(said), label, said)), x0, xs, (y + 62 : U32)),
        Hb.chips.if(Bool.not(wide), ty, df, at, xs, (y + 62 : U32))])

# a comparison's legend: what each mark's colour means
def Hb.key(+ty: Ty, +layer: U32, +label: String, +x: U32, +y: U32) -> Out:
  Out.ops(D.Draw.rect(D.Clip.all(), x, (y + 9 : U32), 10, 10, LO.Look.mark.color(False{}, layer), 255) <> Ty.text(ty, 2n, label, D.Clip.all(), (x + 16 : U32), y, 28, T.Th.dim()))

def Hb.legend(+ty: Ty, +x0: U32, +y: U32) -> Out:
  +label0 = MSG.T.viewer.history.removed(Ty.loc(ty))
  +label1 = MSG.T.viewer.history.changed(Ty.loc(ty))
  +x2 = {(x0 + 16 + 32 + Ty.width(ty, 2n, label0) : U32) : U32}
  +x3 = {(x2 + 32 + Ty.width(ty, 2n, label1) : U32) : U32}
  Out.all([Hb.key(ty, 26, label0, (x0 + 16 : U32), y), Hb.key(ty, 27, label1, x2, y), Hb.key(ty, 28, MSG.T.viewer.history.added(Ty.loc(ty)), x3, y)])

def Hb.side.label.loc(+l: I.Loc, +name: String, +v: String) -> String:
  name ++ ": " ++ Bool.pick(String, String.is_empty(v), MSG.T.viewer.history.pick(l), v)

# Design review: compare anything (client.bend's Rv.*). Old/Diff/New on the
# legend row's right; the legend itself only for Diff.

def Hb.side.label(+name: String, +v: String) -> String:
  Hb.side.label.loc(I.En{}, name, v)

def Hb.legend.if(on: Bool, +ty: Ty, +x0: U32, +y: U32) -> Out:
  match on:
    case True{}:
      Hb.legend(ty, x0, y)
    case False{}:
      Out.none()

def Rvw.flip(+ty: Ty, +showing: String, +x1: U32, +y: U32) -> Out:
  +label0 = MSG.T.viewer.history.old(Ty.loc(ty))
  +label1 = MSG.T.viewer.diff(Ty.loc(ty))
  +label2 = MSG.T.viewer.history.new(Ty.loc(ty))
  +wo = {Lay.button.w(ty, label0) : U32}
  +wd = {Lay.button.w(ty, label1) : U32}
  +wn = {Lay.button.w(ty, label2) : U32}
  +x = {(x1 - 16 - wo - wd - wn - 8 : U32) : U32}
  Out.all([Lay.chip(ty, x, y, label0, String.eq(showing, "old"), "rv-as", "old"),
    Lay.chip(ty, (x + wo + 4 : U32), y, label1, String.eq(showing, "diff"), "rv-as", "diff"),
    Lay.chip(ty, (x + wo + wd + 8 : U32), y, label2, String.eq(showing, "new"), "rv-as", "new")])

# the ref field in the versions menu: Enter asks the hub (rv-ref "<side>\t<text>")
def Rvw.ref(+ty: Ty, +n: S.Nui, +side: String, +x: U32, +y: U32, +w: U32) -> Out:
  +id = {S.Fld.id("rvref", "rv-ref", side ++ "\t") : String}
  Lay.field(ty, D.Clip.all(), x, y, w, U.Ui.scratch(S.Nui.ui(n), "@f.rvref"), String.eq(S.Nui.focus_of(n), id), S.Blink.on(n),
    S.Nui.caret_of(n), S.Nui.mark(n), MSG.T.viewer.history.ref(Ty.loc(ty)), id)

def Hb.compare(+ty: Ty, +n: S.Nui, b: U.HBar, +ss: List<&2, V.Seen>, +x0: U32, +x1: U32, +y: U32) -> Out:
  match b:
    case U.HBar{_, _, _, _, _, _, _, _, _, _, +a, +bb, +menu}:
      +label0 = MSG.T.common.close(Ty.loc(ty))
      +cw = Lay.button.w(ty, label0)
      +bom = MSG.T.review.bom(Ty.loc(ty))
      +room = {Lay.room((x1 - x0 : U32), (cw + Lay.button.w(ty, bom) + 72 : U32)) : U32}
      +la = Ty.fit(ty, 2n, Hb.side.label.loc(Ty.loc(ty), MSG.T.viewer.history.old(Ty.loc(ty)), a), U32.shr(room))
      +lb = Ty.fit(ty, 2n, Hb.side.label.loc(Ty.loc(ty), MSG.T.viewer.history.new(Ty.loc(ty)), bb), U32.shr(room))
      Out.all([Hb.ground(x0, x1, y, 72),
        Lay.chip(ty, (x0 + 16 : U32), (y + 8 : U32), la, String.eq(menu, "a"), "cmp-side", "a"),
        Lay.chip(ty, (x0 + 24 + Lay.button.w(ty, la) : U32), (y + 8 : U32), lb, String.eq(menu, "b"), "cmp-side", "b"),
        Lay.button(ty, D.Clip.all(), (x1 - 16 - cw : U32), (y + 8 : U32), label0, T.Th.text(), "cmp-close", ""),
        Lay.button(ty, D.Clip.all(), (x1 - 24 - cw - Lay.button.w(ty, bom) : U32), (y + 8 : U32), bom, T.Th.text(), "rv-bom", ""),
        Hb.legend.if(String.eq(U.Rv.showing(S.Nui.ui(n)), "diff"), ty, x0, (y + 40 : U32)),
        Rvw.flip(ty, U.Rv.showing(S.Nui.ui(n)), x1, (y + 40 : U32))])

# The menu of versions, above the bar: a row each (its name, then what it
# is), as many as fit; a press outside shuts it
def Hb.rows(rs: List<&2, U.HRow>, +k: Nat, +ty: Ty, +x: U32, +w: U32, +y: U32) -> Out:
  match rs k:
    case _ 0n:
      Out.none()
    case Nil{} _:
      Out.none()
    case U.HRow{+l, +sub, +v} <> t 1n+j:
      +lw = {U32.min(Ty.width(ty, 2n, l), U32.shr(w)) : U32}
      Out.all([Out{List.append(&2, R.Op, Ty.text(ty, 2n, Ty.fit(ty, 2n, l, lw), D.Clip.all(), (x + 12 : U32), y, 30, T.Th.text()),
          Ty.text(ty, 2n, Ty.fit(ty, 2n, sub, Lay.room(w, (lw + 36 : U32))), D.Clip.all(), (x + 24 + lw : U32), y, 30, T.Th.faint())),
          [Lay.hit(x, y, w, 30, "cmp-pick", v)]},
        Hb.rows(t, j, ty, x, w, (y + 30 : U32))])

def Hb.menu(+ty: Ty, +n: S.Nui, +u: U.Ui, +side: String, +x0: U32, +x1: U32, +top: U32, +bot: U32) -> Out:
  +rs = U.Hs.menu.loc(Ty.loc(ty), u, side)
  +w = {U32.min((x1 - x0 - 32 : U32), 460) : U32}
  +fit = {(Lay.room(bot, (top + 82 : U32)) / 30 : U32) : U32}
  +k = U32.min(fit, U32.from_nat(List.length(&2, U.HRow, rs)))
  +mh = {(k * 30 + 78 : U32) : U32}
  +y = {(bot - 8 - mh : U32) : U32}
  Out.all([Out{Nil{}, [Lay.hit(x0, 0, (x1 - x0 : U32), bot, "cmp-shut", "")]},
    Out{List.append(&2, R.Op, Lay.box(D.Clip.all(), (x0 + 16 : U32), y, w, mh, 6, T.Th.raise(), T.Th.line()),
        Ty.text(ty, 2n, Bool.pick(String, String.eq(side, "b"), MSG.T.viewer.history.compare.new(Ty.loc(ty)), MSG.T.viewer.history.compare.old(Ty.loc(ty))), D.Clip.all(), (x0 + 28 : U32), (y + 6 : U32), 30, T.Th.faint())),
      [Lay.hit((x0 + 16 : U32), y, w, mh, "vw-panel", "")]},
    Rvw.ref(ty, n, side, (x0 + 28 : U32), (y + 34 : U32), (w - 24 : U32)),
    Hb.rows(rs, U32.to_nat(k), ty, (x0 + 16 : U32), w, (y + 72 : U32))])

def Hb.menu.if(open: Bool, +ty: Ty, +n: S.Nui, +u: U.Ui, +side: String, +x0: U32, +x1: U32, +top: U32, +bot: U32) -> Out:
  match open:
    case True{}:
      Hb.menu(ty, n, u, side, x0, x1, top, bot)
    case False{}:
      Out.none()

def Hb.body(cmp: Bool, +ty: Ty, +n: S.Nui, +u: U.Ui, +b: U.HBar, +ss: List<&2, V.Seen>, +x0: U32, +x1: U32, +h: U32) -> Out:
  match cmp:
    case True{}:
      Hb.compare(ty, n, b, ss, x0, x1, (h - 72 : U32))
    case False{}:
      Hb.walk(ty, u, b, x0, x1, (h - 96 : U32))

def Hb.menu.side(b: U.HBar) -> String:
  match b:
    case U.HBar{_, _, _, _, _, _, _, _, _, _, _, _, m}:
      m

def Hb.draw.k(on: Bool, +ty: Ty, +n: S.Nui, +u: U.Ui, +b: U.HBar, +ss: List<&2, V.Seen>, +x0: U32, +x1: U32, +h: U32) -> Out:
  match on:
    case True{}:
      Out.cat(Hb.body(Hb.cmp(b), ty, n, u, b, ss, x0, x1, h),
        Hb.menu.if(Bool.not(String.is_empty(Hb.menu.side(b))), ty, n, u, Hb.menu.side(b), x0, x1, 96, (h - Hb.h(b) : U32)))
    case False{}:
      Out.none()

def Hb.for(+ty: Ty, on: Bool, +u: U.Ui) -> U.HBar:
  match on:
    case True{}:
      U.Hs.bar.loc(Ty.loc(ty), u)
    case False{}:
      U.HBar{False{}, 0n, 0n, "", "", "", "", False{}, False{}, False{}, "", "", ""}

# the bar at the panel's foot (h), when there is one
def Hb.draw(+ty: Ty, +n: S.Nui, +u: U.Ui, +b: U.HBar, +ss: List<&2, V.Seen>, +x0: U32, +x1: U32, +h: U32) -> Out:
  Hb.draw.k(Hb.on(b), ty, n, u, b, ss, x0, x1, h)

# The Files tab (client.bend's Fm): the folder's trail and Up, then its rows
# (a page of them when long, with buttons for the rest); an image fitted in
# the pane; a PDF a page at a time, drawn on the hub by pdftoppm (a click
# opens either in the lightbox)
def Fv.trail(ps: List<&2, U.PItem>, +ty: Ty, +x: U32, +x1: U32) -> List<&2, Out>:
  match ps:
    case Nil{}:
      Nil{}
    case U.PItem{+l, +a, +v} <> t:
      Lay.fits(U32.is_le((x + Lay.button.w(ty, l) : U32), x1), Lay.chip(ty, x, 64, l, False{}, a, v)) <> Fv.trail(t, ty, (x + Lay.button.w(ty, l) + 4 : U32), x1)

def Fv.row(r: U.FmRow, +ty: Ty, +x0: U32, +x1: U32, +y: U32) -> Out:
  match r:
    case U.FmRow{+l, +a, +v, +d, +k}:
      +c = Bool.pick(U32, String.is_empty(a), T.Th.faint(), T.Th.text())
      +sw = Ty.width(ty, 2n, d)
      Out{List.concat(&2, R.Op, [Lay.icon(ty, U.Fm.icon(k), D.Clip.all(), (x0 + 16 : U32), (y + 7 : U32), Bool.pick(U32, String.eq(k, "folder"), T.Th.accent(), c)),
          Ty.text(ty, 2n, Ty.fit(ty, 2n, l, U32.dim((x1 - x0 : U32), (sw + 72 : U32))), D.Clip.all(), (x0 + 42 : U32), y, 30, c),
          Ty.text(ty, 2n, d, D.Clip.all(), (x1 - 16 - sw : U32), y, 30, T.Th.faint())]),
        Bool.pick(List<&2, S.Hit>, String.is_empty(a), Nil{}, [Lay.hit(x0, y, (x1 - x0 : U32), 30, a, v)])}

def Fv.rows(rs: List<&2, U.FmRow>, +ty: Ty, +x0: U32, +x1: U32, +y: U32) -> List<&2, Out>:
  match rs:
    case Nil{}:
      Nil{}
    case r <> t:
      Fv.row(r, ty, x0, x1, y) <> Fv.rows(t, ty, x0, x1, (y + 30 : U32))

def Fv.list.k(ready: Bool, +ty: Ty, +u: U.Ui, +x0: U32, +x1: U32, +h: U32) -> Out:
  match ready:
    case True{}:
      +label0 = MSG.T.common.more(Ty.loc(ty))
      +all = U.Fm.rows(u)
      +fit = {U32.to_nat(U32.max(((h - 150 : U32) / 30 : U32), 1)) : Nat}
      +top = U.Fm.top(u)
      +n = List.length(&2, U.FmRow, all)
      Out.all(List.concat(&2, Out, [
        Bool.pick(List<&2, Out>, Nat.is_eq(n, 0n), [Out.ops(Ty.text(ty, 2n, MSG.T.viewer.files.empty(Ty.loc(ty)), D.Clip.all(), (x0 + 16 : U32), 104, 30, T.Th.faint()))], Nil{}),
        Fv.rows(List.take(&2, U.FmRow, List.drop(&2, U.FmRow, all, top), fit), ty, x0, x1, 104),
        [Lay.fits(Nat.is_gt(top, 0n), Lay.button(ty, D.Clip.all(), (x0 + 16 : U32), (h - 40 : U32), MSG.T.common.earlier(Ty.loc(ty)), T.Th.soft(), "fm-page", Nat.show(Nat.sub(top, fit)))),
         Lay.fits(Nat.is_gt(n, Nat.add(top, fit)), Lay.button(ty, D.Clip.all(), (x1 - 16 - Lay.button.w(ty, label0) : U32), (h - 40 : U32), label0, T.Th.soft(), "fm-page", Nat.show(Nat.add(top, fit)))),
         Lay.fits(Nat.is_gt(n, fit), Out.ops(Ty.text(ty, 2n, MSG.T.viewer.files.range(Ty.loc(ty), Nat.show(1n+top), Nat.show(Bool.pick(Nat, Nat.is_lt(Nat.add(top, fit), n), Nat.add(top, fit), n)), Nat.show(n)),
           D.Clip.all(), (x0 + U32.shr((x1 - x0 : U32)) - 40 : U32), (h - 40 : U32), 28, T.Th.faint())))]]))
    case False{}:
      Out.ops(Ty.text(ty, 2n, MSG.T.viewer.files.loading(Ty.loc(ty)), D.Clip.all(), (x0 + 16 : U32), 104, 30, T.Th.faint()))

def Fv.files(+ty: Ty, +u: U.Ui, +x0: U32, +x1: U32, +h: U32) -> Out:
  +label0 = MSG.T.queue.up(Ty.loc(ty))
  +uw = Lay.button.w(ty, label0)
  Out.all(List.concat(&2, Out, [[Out.ops([D.Draw.rect(D.Clip.all(), x0, 52, (x1 - x0 : U32), (h - 52 : U32), T.Th.panel(), 255)])],
    Fv.trail(U.Fm.trail.loc(Ty.loc(ty), u), ty, (x0 + 16 : U32), (x1 - 24 - uw : U32)),
    [Lay.button(ty, D.Clip.all(), (x1 - 16 - uw : U32), 64, label0, T.Th.soft(), "fm-up", ""),
     Out.ops([D.Draw.rect(D.Clip.all(), x0, 99, (x1 - x0 : U32), 1, T.Th.line(), 255)]),
     Fv.list.k(U.Fm.ready(u), ty, u, x0, x1, h)]]))

# a picture (an image, or a PDF's page) fitted under the head; its key is
# the window's image key ("full:" decodes it large)
def Fv.pic(+ty: Ty, +u: U.Ui, +key: String, +x0: U32, +x1: U32, +y: U32, +h: U32) -> Out:
  +size = U.Ui.info(u, "img:" ++ key)
  +bw = Page.num(size, 0n)
  +bh = Page.num(size, 1n)
  +pw = {(x1 - x0 - 32 : U32) : U32}
  +ph = {(h - y - 16 : U32) : U32}
  +fw = Page.fit.w(bw, bh, pw, ph)
  +fh = {((fw * bh : U32) / U32.max(bw, 1) : U32) : U32}
  +ix = {(x0 + 16 + U32.shr((pw - fw : U32)) : U32) : U32}
  Out.all([
    Out.when(String.is_empty(size), Out.ops(Ty.text(ty, 2n, MSG.T.viewer.loading(Ty.loc(ty)), D.Clip.all(), (x0 + 16 : U32), y, 30, T.Th.faint()))),
    Out.when(String.starts_with(size, "err:"), Out.ops(Ty.text(ty, 2n, MSG.T.viewer.picture.failed(Ty.loc(ty)), D.Clip.all(), (x0 + 16 : U32), y, 30, T.Th.faint()))),
    Out{Bool.pick(List<&2, R.Op>, Bool.or(String.is_empty(size), String.starts_with(size, "err:")), Nil{}, D.Draw.frame(D.Clip.all(), (ix - 1 : U32), (y - 1 : U32), (fw + 2 : U32), (fh + 2 : U32), T.Th.line())),
      [Lay.hit(ix, y, U32.max(fw, 1), U32.max(fh, 1), "img", key)]}])

# (no panel painted under it: the window draws a picture into the frame's
# base, under the UI, and a panel would cover it, as the Mech page's renders)
def Fv.head(+ty: Ty, +u: U.Ui, +x0: U32, +x1: U32, +h: U32, more: Out) -> Out:
  +label0 = MSG.T.viewer.files.back(Ty.loc(ty))
  +bw = Lay.button.w(ty, label0)
  Out.all([Lay.button(ty, D.Clip.all(), (x0 + 16 : U32), 64, label0, T.Th.soft(), "fm", ""),
    Out.ops(Ty.text(ty, 2n, Ty.fit(ty, 2n, U.Fm.name(u), U32.dim((x1 - x0 : U32), (bw + 260 : U32))), D.Clip.all(), (x0 + 28 + bw : U32), 64, 28, T.Th.text())),
    more,
    Out.ops([D.Draw.rect(D.Clip.all(), x0, 99, (x1 - x0 : U32), 1, T.Th.line(), 255)])])

def Fv.image(+ty: Ty, +u: U.Ui, +x0: U32, +x1: U32, +h: U32) -> Out:
  Out.cat(Fv.head(ty, u, x0, x1, h, Out.none()), Fv.pic(ty, u, "full:" ++ U.Ui.view_get(u, "view.path"), x0, x1, 112, h))

def Fv.pdf.body(+ty: Ty, +u: U.Ui, +x0: U32, +x1: U32, +h: U32) -> Out:
  Bool.pick(Out, Bool.not(String.is_empty(U.Pdf.err(u))), Out.ops(Ty.text(ty, 2n, Ty.fit(ty, 2n, U.Pdf.err(u), (x1 - x0 - 32 : U32)), D.Clip.all(), (x0 + 16 : U32), 112, 30, T.Th.dim())),
    Bool.pick(Out, String.is_empty(U.Pdf.png(u)), Out.ops(Ty.text(ty, 2n, MSG.T.viewer.pdf.drawing(Ty.loc(ty), U.Pdf.page(u)), D.Clip.all(), (x0 + 16 : U32), 112, 30, T.Th.faint())),
      Fv.pic(ty, u, "full:" ++ U.Pdf.png(u), x0, x1, 112, h)))

def Fv.pdf(+ty: Ty, +u: U.Ui, +x0: U32, +x1: U32, +h: U32) -> Out:
  +label0 = MSG.T.tour.next(Ty.loc(ty))
  +label1 = MSG.T.common.prev(Ty.loc(ty))
  +p = Bool.pick(Nat, Nat.is_eq(FM.Fm.num(U.Pdf.page(u)), 0n), 1n, FM.Fm.num(U.Pdf.page(u)))
  +n = FM.Fm.num(U.Pdf.pages(u))
  +nw = Lay.button.w(ty, label0)
  +pw = Lay.button.w(ty, label1)
  +label = MSG.T.viewer.pdf.page(Ty.loc(ty), Nat.show(p), Bool.pick(String, Nat.is_eq(n, 0n), "unknown", "known"), Nat.show(n))
  +lw = Ty.width(ty, 2n, label)
  Out.cat(Fv.head(ty, u, x0, x1, h, Out.all([
      Lay.fits(Nat.is_gt(p, 1n), Lay.button(ty, D.Clip.all(), (x1 - 16 - nw - 12 - lw - 12 - pw : U32), 64, label1, T.Th.soft(), "pdf-page", Nat.show(Nat.sub(p, 1n)))),
      Out.ops(Ty.text(ty, 2n, label, D.Clip.all(), (x1 - 16 - nw - 12 - lw : U32), 64, 28, T.Th.dim())),
      Lay.fits(Bool.or(Nat.is_eq(n, 0n), Nat.is_lt(p, n)), Lay.button(ty, D.Clip.all(), (x1 - 16 - nw : U32), 64, label0, T.Th.soft(), "pdf-page", Nat.show(1n+p)))])),
    Fv.pdf.body(ty, u, x0, x1, h))

def Fv.k(i: U32, +ty: Ty, +u: U.Ui, +x0: U32, +x1: U32, +h: U32) -> Out:
  match i:
    case 6:
      Fv.files(ty, u, x0, x1, h)
    case 7:
      Fv.image(ty, u, x0, x1, h)
    case 8:
      Fv.pdf(ty, u, x0, x1, h)
    case _:
      Out.none()

# The Review tab (client.bend's Rp): the page chips and Refresh, the title,
# then the report's Markdown as the timeline lays it out, from the first
# line shown (the wheel over it moves that by three, Pane with key rp:<thread>)
def Rpw.chips(ts: List<&2, U.RpTab>, +ty: Ty, +x: U32, +x1: U32) -> Out:
  match ts:
    case Nil{}:
      Out.none()
    case U.RpTab{+p, +l, +on} <> t:
      Out.cat(Lay.fits(U32.is_le((x + Lay.button.w(ty, l) : U32), x1), Lay.chip(ty, x, 64, l, on, "rv-page", p)), Rpw.chips(t, ty, (x + Lay.button.w(ty, l) + 4 : U32), x1))

def Rpw.draw(ls: List<&2, TLine>, more: Bool, +ty: Ty, +c: D.Clip, +x0: U32, +x1: U32, +y: U32, +y1: U32) -> Out:
  match ls more:
    case TLine{+t, +co, +k, +cp, +bw} <> +rest True{}:
      Out.cat(TL.draw(ty, TLine{t, co, k, cp, bw}, c, x0, x1, y),
        Rpw.draw(rest, TL.more(rest, (y + TL.h(k) : U32), y1), ty, c, x0, x1, (y + TL.h(k) : U32), y1))
    case _ _:
      Out.none()

def Rpw.body(has: Bool, +ty: Ty, +v: U.RpView, +th: String, +off: Nat, +x0: U32, +x1: U32, +h: U32) -> Out:
  match has:
    case True{}:
      +ls = {TL.blocks(ty, N.Md.render(U.RpView.md(v)), U32.max((x1 - x0 - 32 : U32), 40)) : List<&2, TLine>}
      +n = {List.length(&2, TLine, ls) : Nat}
      +last = {Nat.sub(n, U32.to_nat(U32.min(U32.from_nat(n), (Lay.room(h, 142) / 24 : U32)))) : Nat}
      +from = {Bool.pick(Nat, Nat.is_lt(off, last), off, last) : Nat}
      Out.all([Out{Nil{}, [Lay.hit(x0, 142, (x1 - x0 : U32), Lay.room(h, 142), "pane", "rp:" ++ th ++ "|" ++ Nat.show(last))]},
        Rpw.draw(List.drop(&2, TLine, ls, from), True{}, ty, D.Clip{x0, 142, x1, h}, (x0 + 16 : U32), (x1 - 16 : U32), 142, h)])
    case False{}:
      Out.ops(Ty.text(ty, 2n, U.RpView.note.loc(Ty.loc(ty), v), D.Clip.all(), (x0 + 16 : U32), 150, 30, T.Th.faint()))

# Review comments and the checklist in the window (client.bend's Rc and Rp).
# Pins are drawn over the board or schematic from the view's own transform
# (a handful of squares, no scene redraw); a click on one opens it
# ("rv-pin"), a right click on the viewer starts a note (nui.bend's Rcw).
# The note form, the open comment and the Checks page's boxes follow.
type Crow is Data:
  Crow{kind: U32, text: String, index: Nat, done: Bool}

def Rcw.view(v: V.Vw) -> V.View:
  match v:
    case V.Vw{_, vi, _, _, _, _, _, _, _, _, _}:
      vi

def Rcw.part(xs: List<&2, String>, +i: Nat) -> String:
  Maybe.default(&2, String, List.get(&2, String, xs, i), "")

def Rcw.f(+s: String) -> F32:
  Maybe.default(&2, F32, F32.read(s), 0.0)

def Rcw.sq(+ty: Ty, +x: U32, +y: U32, +fill: U32, +line: U32, +cnt: Nat, +sel: Bool) -> List<&2, R.Op>:
  List.concat(&2, R.Op, [Bool.pick(List<&2, R.Op>, sel, [D.Draw.rect(D.Clip.all(), (x - 2 : U32), (y - 2 : U32), 20, 20, T.Th.text(), 255)], Nil{}),
    Lay.box(D.Clip.all(), x, y, 16, 16, 0, fill, line),
    Bool.pick(List<&2, R.Op>, Nat.is_gt(cnt, 0n), Ty.text(ty, 1n, Nat.show(cnt), D.Clip.all(), (x + 4 : U32), y, 16, T.Th.bg()), Nil{})])

def Rcw.pin.in(on: Bool, +ty: Ty, +id: String, +res: Bool, +cnt: Nat, +sel: Bool, +ux: U32, +uy: U32) -> Out:
  match on:
    case True{}:
      Out{Rcw.sq(ty, (ux - 8 : U32), (uy - 8 : U32), Bool.pick(U32, res, T.Th.panel(), T.Th.accent()), Bool.pick(U32, res, T.Th.faint(), T.Th.bg()), cnt, sel),
        [Lay.hit((ux - 8 : U32), (uy - 8 : U32), 16, 16, "rv-pin", id)]}
    case False{}:
      Out.none()

def Rcw.pin.at(+sx: F32, +sy: F32, +ty: Ty, +id: String, +res: Bool, +cnt: Nat, +sel: Bool, +x0: U32, +x1: U32, +y1: U32) -> Out:
  Rcw.pin.in(Bool.and(Bool.and(F32.is_ge(sx, U32.to_f32((x0 + 10 : U32))), F32.is_le(sx, U32.to_f32(U32.dim(x1, 10)))),
      Bool.and(F32.is_ge(sy, 62.0), F32.is_le(sy, U32.to_f32(U32.dim(y1, 10))))),
    ty, id, res, cnt, sel, D.Draw.floor(sx), D.Draw.floor(sy))

def Rcw.pins(ps: List<&2, RVW.Pin>, +ty: Ty, +v: V.View, +sel: String, +x0: U32, +x1: U32, +y1: U32) -> Out:
  match ps:
    case Nil{}:
      Out.none()
    case RVW.Pin{+id, +px, +py, _, _, +res, +cnt} <> t:
      Out.cat(Rcw.pins(t, ty, v, sel, x0, x1, y1),
        Rcw.pin.at(V.View.x(v, px), V.View.y(v, py), ty, id, res, cnt, String.eq(id, sel), x0, x1, y1))

# a card: bordered panel, and a hit under its buttons so a press on it never drags the view
def Rcw.card(+x: U32, +y: U32, +w: U32, +h: U32) -> Out:
  Out{Lay.box(D.Clip.all(), x, y, w, h, 0, T.Th.panel(), T.Th.line()), [Lay.hit(x, y, w, h, "noop", "")]}

def Rcw.lines(ls: List<&2, String>, +ty: Ty, +x: U32, +y: U32, +color: U32) -> List<&2, R.Op>:
  match ls:
    case Nil{}:
      Nil{}
    case l <> t:
      List.append(&2, R.Op, Ty.text(ty, 2n, l, D.Clip.all(), x, y, 20, color), Rcw.lines(t, ty, x, (y + 20 : U32), color))

def Rcw.take(ls: List<&2, String>, +n: Nat) -> List<&2, String>:
  match ls n:
    case +l <> +t 1n+k:
      l <> Rcw.take(t, k)
    case _ _:
      Nil{}

def Rcw.reps(rs: List<&2, String>, +ty: Ty, +w: U32) -> List<&2, String>:
  match rs:
    case Nil{}:
      Nil{}
    case +r <> t:
      List.append(&2, String, Ty.wrap(ty, 2n, Rcw.part(String.split(r, '\u{1f}'), 1n) ++ ": " ++ Rcw.part(String.split(r, '\u{1f}'), 0n), w), Rcw.reps(t, ty, w))

# the open comment, top right
def Rcw.open.k(+ty: Ty, +n: S.Nui, o: U.RcOpen, +x0: U32, +x1: U32, +y1: U32) -> Out:
  match o:
    case U.RcOpen{+id, +text, +by, +place, +res, +reps}:
      +label0 = MSG.T.review.reply(Ty.loc(ty))
      +cw = {U32.min(360, U32.dim((x1 - x0 : U32), 24)) : U32}
      +tw = {U32.dim(cw, 24) : U32}
      +ls = {Rcw.take(List.append(&2, String, Ty.wrap(ty, 2n, text, tw), Rcw.reps(reps, ty, tw)), 12n) : List<&2, String>}
      +nl = {U32.from_nat(List.length(&2, String, ls)) : U32}
      +ch = {(122 + nl * 20 : U32) : U32}
      +cx = {(x0 + 12 : U32) : U32}
      +cy = {U32.max(62, U32.dim(y1, (ch + 8 : U32))) : U32}
      +fy = {(cy + 40 + nl * 20 + 8 : U32) : U32}
      +fid = {S.Fld.id("rcreply", "rv-reply-send", "") : String}
      Out.all([Rcw.card(cx, cy, cw, ch),
        Out.ops(Ty.text(ty, 2n, Ty.fit(ty, 2n, MSG.T.review.comment.meta(Ty.loc(ty), by, place, Bool.pick(String, res, "resolved", "open")), U32.dim(tw, 24)), D.Clip.all(), (cx + 12 : U32), (cy + 6 : U32), 24, T.Th.dim())),
        Out{Lay.icon(ty, "x", D.Clip.all(), (cx + cw - 26 : U32), (cy + 10 : U32), T.Th.dim()), [Lay.hit((cx + cw - 32 : U32), (cy + 4 : U32), 28, 28, "rv-pin", "")]},
        Out.ops(Rcw.lines(ls, ty, (cx + 12 : U32), (cy + 38 : U32), T.Th.text())),
        Lay.field(ty, D.Clip.all(), (cx + 12 : U32), fy, tw, U.Ui.scratch(S.Nui.ui(n), "@f.rcreply"), String.eq(S.Nui.focus_of(n), fid), S.Blink.on(n), S.Nui.caret_of(n), S.Nui.mark(n), label0, fid),
        Lay.button(ty, D.Clip.all(), (cx + 12 : U32), (fy + 36 : U32), label0, T.Th.accent(), "rv-reply-send", ""),
        Lay.button(ty, D.Clip.all(), (cx + 20 + Lay.button.w(ty, label0) : U32), (fy + 36 : U32), Bool.pick(String, res, MSG.T.review.reopen(Ty.loc(ty)), MSG.T.review.resolve(Ty.loc(ty))), T.Th.soft(), Bool.pick(String, res, "rv-reopen", "rv-resolve"), id)])

def Rcw.open(m: Maybe<&2, U.RcOpen>, +ty: Ty, +n: S.Nui, +x0: U32, +x1: U32, +y1: U32) -> Out:
  match m:
    case Some{+o}:
      Rcw.open.k(ty, n, o, x0, x1, y1)
    case None{}:
      Out.none()

# the note being written: a marker at its place and the form beside it
def Rcw.note.k(+ty: Ty, +n: S.Nui, +v: V.View, +xs: List<&2, String>, +x0: U32, +x1: U32, +y1: U32) -> Out:
  +label0 = MSG.T.composer.send(Ty.loc(ty))
  +sx = {V.View.x(v, Rcw.f(Rcw.part(xs, 0n))) : F32}
  +sy = {V.View.y(v, Rcw.f(Rcw.part(xs, 1n))) : F32}
  +ux = {D.Draw.floor(F32.max(sx, 0.0)) : U32}
  +uy = {D.Draw.floor(F32.max(sy, 0.0)) : U32}
  +cw = {U32.min(340, U32.dim((x1 - x0 : U32), 24)) : U32}
  +cx = {U32.max((x0 + 8 : U32), U32.min((ux + 16 : U32), U32.dim(x1, (cw + 8 : U32)))) : U32}
  +cy = {U32.max(62, U32.min((uy + 16 : U32), U32.dim(y1, 138))) : U32}
  +fid = {S.Fld.id("rcnote", "rv-note-send", "") : String}
  Out.all([Out.ops(Rcw.sq(ty, U32.dim(ux, 8), U32.dim(uy, 8), T.Th.warn(), T.Th.bg(), 0n, False{})),
    Rcw.card(cx, cy, cw, 124),
    Out.ops(Ty.text(ty, 2n, Ty.fit(ty, 2n, MSG.T.review.here(Ty.loc(ty)), U32.dim(cw, 24)), D.Clip.all(), (cx + 12 : U32), (cy + 6 : U32), 24, T.Th.dim())),
    Lay.field(ty, D.Clip.all(), (cx + 12 : U32), (cy + 38 : U32), U32.dim(cw, 24), U.Ui.scratch(S.Nui.ui(n), "@f.rcnote"), String.eq(S.Nui.focus_of(n), fid), S.Blink.on(n), S.Nui.caret_of(n), S.Nui.mark(n), MSG.T.review.note(Ty.loc(ty)), fid),
    Lay.button(ty, D.Clip.all(), (cx + 12 : U32), (cy + 78 : U32), label0, T.Th.accent(), "rv-note-send", ""),
    Lay.button(ty, D.Clip.all(), (cx + 20 + Lay.button.w(ty, label0) : U32), (cy + 78 : U32), MSG.T.common.cancel(Ty.loc(ty)), T.Th.soft(), "rv-note-shut", "")])

def Rcw.note.if(none: Bool, +at: String, +ty: Ty, +n: S.Nui, +v: V.View, +x0: U32, +x1: U32, +y1: U32) -> Out:
  match none:
    case True{}:
      Out.none()
    case False{}:
      Rcw.note.k(ty, n, v, String.split(at, '\t'), x0, x1, y1)

def Rcw.note(+at: String, +ty: Ty, +n: S.Nui, +v: V.View, +x0: U32, +x1: U32, +y1: U32) -> Out:
  Rcw.note.if(String.is_empty(at), at, ty, n, v, x0, x1, y1)

def Rcw.all(+ty: Ty, +n: S.Nui, +x0: U32, +x1: U32, +y1: U32) -> Out:
  +u = S.Nui.ui(n)
  Out.all([Rcw.pins(U.Rc.pins(u), ty, Rcw.view(S.Nui.vw(n)), U.Rc.sel(u), x0, x1, y1),
    Rcw.note(U.Rc.at(u), ty, n, Rcw.view(S.Nui.vw(n)), x0, x1, y1),
    Rcw.open(U.Rc.open(u), ty, n, x0, x1, y1)])

def Rcw.if(on: Bool, +ty: Ty, +n: S.Nui, +x0: U32, +x1: U32, +y1: U32) -> Out:
  match on:
    case True{}:
      Rcw.all(ty, n, x0, x1, y1)
    case False{}:
      Out.none()

# The Checks page: the gate line, then the checklist's boxes grouped by
# their heading (a click ticks one: "rv-tick" and its index), then the
# report's Markdown, all scrolling by rows of 24 px. Kinds: 0 passes, 1
# fails, 2 a group's heading, 3 a box and its first line, 4 its next lines.
def Rcw.wrows(ls: List<&2, String>, +i: Nat, +d: Bool, +first: Bool) -> List<&2, Crow>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      Crow{Bool.pick(U32, first, 3, 4), l, i, d} <> Rcw.wrows(t, i, d, False{})

def Rcw.thead(+g: String, +prev: String) -> List<&2, Crow>:
  Bool.pick(List<&2, Crow>, Bool.and(Bool.not(String.is_empty(g)), Bool.not(String.eq(g, prev))), [Crow{2, g, 0n, False{}}], Nil{})

def Rcw.trows(ts: List<&2, U.RpTask>, +prev: String, +ty: Ty, +w: U32) -> List<&2, Crow>:
  match ts:
    case Nil{}:
      Nil{}
    case U.RpTask{+i, +text, +done, +group} <> t:
      List.append(&2, Crow, Rcw.thead(group, prev), List.append(&2, Crow, Rcw.wrows(Ty.wrap(ty, 2n, text, w), i, done, True{}), Rcw.trows(t, group, ty, w)))

def Rcw.tick(+x: U32, +y: U32) -> List<&2, R.Op>:
  [D.Draw.rect(D.Clip.all(), (x + 3 : U32), (y + 8 : U32), 2, 2, T.Th.bg(), 255), D.Draw.rect(D.Clip.all(), (x + 4 : U32), (y + 9 : U32), 2, 2, T.Th.bg(), 255),
   D.Draw.rect(D.Clip.all(), (x + 5 : U32), (y + 10 : U32), 2, 2, T.Th.bg(), 255), D.Draw.rect(D.Clip.all(), (x + 6 : U32), (y + 9 : U32), 2, 2, T.Th.bg(), 255),
   D.Draw.rect(D.Clip.all(), (x + 7 : U32), (y + 8 : U32), 2, 2, T.Th.bg(), 255), D.Draw.rect(D.Clip.all(), (x + 8 : U32), (y + 7 : U32), 2, 2, T.Th.bg(), 255),
   D.Draw.rect(D.Clip.all(), (x + 9 : U32), (y + 6 : U32), 2, 2, T.Th.bg(), 255), D.Draw.rect(D.Clip.all(), (x + 10 : U32), (y + 5 : U32), 2, 2, T.Th.bg(), 255),
   D.Draw.rect(D.Clip.all(), (x + 11 : U32), (y + 4 : U32), 2, 2, T.Th.bg(), 255)]

def Rcw.box(+x: U32, +y: U32, +done: Bool) -> List<&2, R.Op>:
  List.append(&2, R.Op, Lay.box(D.Clip.all(), x, y, 16, 16, 0, Bool.pick(U32, done, T.Th.accent(), T.Th.bg()), Bool.pick(U32, done, T.Th.accent(), T.Th.dim())),
    Bool.pick(List<&2, R.Op>, done, Rcw.tick(x, y), Nil{}))

def Rcw.row(+r: Crow, +ty: Ty, +x0: U32, +x1: U32, +y: U32) -> Out:
  match r:
    case Crow{0, +t, _, _}:
      Out.ops(Ty.text(ty, 2n, t, D.Clip.all(), x0, y, 24, T.Th.ok()))
    case Crow{1, +t, _, _}:
      Out.ops(Ty.text(ty, 2n, t, D.Clip.all(), x0, y, 24, T.Th.bad()))
    case Crow{2, +t, _, _}:
      Out.ops(Ty.text(ty, 2n, t, D.Clip.all(), x0, y, 24, T.Th.faint()))
    case Crow{3, +t, +i, +d}:
      Out{List.append(&2, R.Op, Rcw.box(x0, (y + 4 : U32), d), Ty.text(ty, 2n, t, D.Clip.all(), (x0 + 26 : U32), y, 24, Bool.pick(U32, d, T.Th.dim(), T.Th.text()))),
        [Lay.hit(x0, y, (x1 - x0 : U32), 24, "rv-tick", Nat.show(i))]}
    case Crow{5, _, _, _}:
      Out.none()
    case Crow{_, +t, +i, +d}:
      Out{Ty.text(ty, 2n, t, D.Clip.all(), (x0 + 26 : U32), y, 24, Bool.pick(U32, d, T.Th.dim(), T.Th.text())),
        [Lay.hit(x0, y, (x1 - x0 : U32), 24, "rv-tick", Nat.show(i))]}

def Rcw.rdraw(rs: List<&2, Crow>, +ty: Ty, +x0: U32, +x1: U32, +y: U32, +room: Nat) -> Out:
  match rs room:
    case +r <> +t 1n+k:
      Out.cat(Rcw.row(r, ty, x0, x1, y), Rcw.rdraw(t, ty, x0, x1, (y + 24 : U32), k))
    case _ _:
      Out.none()

def Rcw.checks(+ty: Ty, +u: U.Ui, +v: U.RpView, +th: String, +off: Nat, +x0: U32, +x1: U32, +h: U32) -> Out:
  +why = {U.Rp.gate(u) : String}
  +head = {Crow{Bool.pick(U32, String.is_empty(why), 0, 1), Bool.pick(String, String.is_empty(why), MSG.T.review.passes(Ty.loc(ty)), Ty.fit(ty, 2n, MSG.T.review.fails(Ty.loc(ty), why), U32.dim((x1 - x0 : U32), 32))), 0n, False{}} : Crow}
  +rows = {head <> List.append(&2, Crow, Rcw.trows(U.Rp.tasks(u), "", ty, U32.dim((x1 - x0 : U32), 90)), [Crow{5, "", 0n, False{}}]) : List<&2, Crow>}
  +ls = {TL.blocks(ty, N.Md.render(U.RpView.md(v)), U32.max((x1 - x0 - 32 : U32), 40)) : List<&2, TLine>}
  +n1 = {List.length(&2, Crow, rows) : Nat}
  +total = {Nat.add(n1, List.length(&2, TLine, ls)) : Nat}
  +room = {U32.to_nat((Lay.room(h, 142) / 24 : U32)) : Nat}
  +last = {Nat.sub(total, Bool.pick(Nat, Nat.is_lt(total, room), total, room)) : Nat}
  +from = {Bool.pick(Nat, Nat.is_lt(off, last), off, last) : Nat}
  +f1 = {Bool.pick(Nat, Nat.is_lt(from, n1), from, n1) : Nat}
  +s1 = {Nat.sub(n1, f1) : Nat}
  +shown = {Bool.pick(Nat, Nat.is_lt(s1, room), s1, room) : Nat}
  Out.all([Out{Nil{}, [Lay.hit(x0, 142, (x1 - x0 : U32), Lay.room(h, 142), "pane", "rp:" ++ th ++ "|" ++ Nat.show(last))]},
    Rcw.rdraw(List.drop(&2, Crow, rows, f1), ty, (x0 + 16 : U32), (x1 - 16 : U32), 142, room),
    Rpw.draw(List.drop(&2, TLine, ls, Nat.sub(from, f1)), True{}, ty, D.Clip{x0, 142, x1, h}, (x0 + 16 : U32), (x1 - 16 : U32), (142 + U32.from_nat(shown) * 24 : U32), h)])

def Rcw.body(on: Bool, has: Bool, +ty: Ty, +u: U.Ui, +v: U.RpView, +th: String, +off: Nat, +x0: U32, +x1: U32, +h: U32) -> Out:
  match on:
    case True{}:
      Rcw.checks(ty, u, v, th, off, x0, x1, h)
    case False{}:
      Rpw.body(has, ty, v, th, off, x0, x1, h)

# The Release page (client.bend's Rl): the tag field (empty releases the
# proposed one), the two toggles (Publish is off until the person turns it
# on), the button, then the error in red and the report's Markdown
def Rlw.lines(ls: List<&2, String>, +ty: Ty, +x: U32, +y: U32, +color: U32) -> Out:
  match ls:
    case Nil{}:
      Out.none()
    case +l <> t:
      Out.cat(Out.ops(Ty.text(ty, 2n, l, D.Clip.all(), x, y, 24, color)), Rlw.lines(t, ty, x, (y + 24 : U32), color))

def Rlw.toggle(+ty: Ty, +x: U32, +y: U32, +on: Bool, +label: String, action: String) -> Out:
  Out{List.append(&2, R.Op, Rcw.box(x, y, on), Ty.text(ty, 2n, label, D.Clip.all(), (x + 26 : U32), (y - 4 : U32), 24, T.Th.text())),
    [Lay.hit(x, (y - 4 : U32), (Ty.width(ty, 2n, label) + 26 : U32), 24, action, "")]}

def Rlw.make(busy: Bool, +ty: Ty, +x: U32, +y: U32, +label: String) -> Out:
  match busy:
    case True{}:
      Out{List.append(&2, R.Op, Lay.box(D.Clip.all(), x, y, Lay.button.w(ty, label), 28, 6, T.Th.raise(), T.Th.line()), Ty.text(ty, 2n, label, D.Clip.all(), (x + 10 : U32), y, 28, T.Th.faint())), Nil{}}
    case False{}:
      Lay.button(ty, D.Clip.all(), x, y, label, T.Th.accent(), "rv-release", "")

def Rlw.report(+ty: Ty, +v: U.RpView, +th: String, +off: Nat, +x0: U32, +x1: U32, +y: U32, +h: U32) -> Out:
  +es = {Bool.pick(List<&2, String>, String.is_empty(U.RpView.err(v)), Nil{}, Ty.wrap(ty, 2n, U.RpView.err(v), U32.max((x1 - x0 - 32 : U32), 40))) : List<&2, String>}
  +ne = {List.length(&2, String, es) : Nat}
  +y1 = {(y + U32.from_nat(ne) * 24 : U32) : U32}
  +ls = {TL.blocks(ty, N.Md.render(U.RpView.md(v)), U32.max((x1 - x0 - 32 : U32), 40)) : List<&2, TLine>}
  +n = {List.length(&2, TLine, ls) : Nat}
  +last = {Nat.sub(n, U32.to_nat(U32.min(U32.from_nat(n), (Lay.room(h, y1) / 24 : U32)))) : Nat}
  +from = {Bool.pick(Nat, Nat.is_lt(off, last), off, last) : Nat}
  Out.all([Out{Nil{}, [Lay.hit(x0, y1, (x1 - x0 : U32), Lay.room(h, y1), "pane", "rp:" ++ th ++ "|" ++ Nat.show(last))]},
    Rlw.lines(es, ty, (x0 + 16 : U32), y, T.Th.bad()),
    Rpw.draw(List.drop(&2, TLine, ls, from), True{}, ty, D.Clip{x0, y1, x1, h}, (x0 + 16 : U32), (x1 - 16 : U32), y1, h)])

def Rlw.page(f: U.RlForm, +ty: Ty, +n: S.Nui, +v: U.RpView, +th: String, +off: Nat, +x0: U32, +x1: U32, +h: U32) -> Out:
  match f:
    case U.RlForm{+tag, +next, +publish, +force}:
      +fid = {S.Fld.id("rltag", "rv-release", "") : String}
      Out.all([Lay.field(ty, D.Clip.all(), (x0 + 16 : U32), 150, U32.min(300, U32.dim((x1 - x0 : U32), 32)), U.Ui.scratch(S.Nui.ui(n), "@f.rltag"), String.eq(S.Nui.focus_of(n), fid), S.Blink.on(n), S.Nui.caret_of(n), S.Nui.mark(n), next, fid),
        Rlw.toggle(ty, (x0 + 16 : U32), 196, publish, MSG.T.review.publish(Ty.loc(ty)), "rv-release-pub"),
        Rlw.toggle(ty, (x0 + 16 : U32), 224, force, MSG.T.review.force(Ty.loc(ty)), "rv-release-force"),
        Rlw.make(U.RpView.busy(v), ty, (x0 + 16 : U32), 254, Bool.pick(String, U.RpView.busy(v), MSG.T.review.release.making(Ty.loc(ty), tag), MSG.T.review.release.make(Ty.loc(ty), tag))),
        Out.ops([D.Draw.rect(D.Clip.all(), x0, 292, (x1 - x0 : U32), 1, T.Th.line(), 255)]),
        Bool.pick(Out, Bool.and(String.is_empty(U.RpView.md(v)), String.is_empty(U.RpView.err(v))),
          Out.ops(Ty.text(ty, 2n, Ty.fit(ty, 2n, U.RpView.note.loc(Ty.loc(ty), v), U32.dim((x1 - x0 : U32), 32)), D.Clip.all(), (x0 + 16 : U32), 300, 30, T.Th.faint())),
          Rlw.report(ty, v, th, off, x0, x1, 300, h))])

def Rlw.sel(rel: Bool, +ty: Ty, +n: S.Nui, +v: U.RpView, +th: String, +off: Nat, +x0: U32, +x1: U32, +h: U32) -> Out:
  match rel:
    case True{}:
      Rlw.page(U.Rl.form(S.Nui.ui(n)), ty, n, v, th, off, x0, x1, h)
    case False{}:
      Rcw.body(Bool.and(String.eq(U.RpView.page(v), "checks"), Bool.not(String.is_empty(U.RpView.md(v)))), Bool.not(String.is_empty(U.RpView.md(v))), ty, S.Nui.ui(n), v, th, off, x0, x1, h)

def Rpw.page(+ty: Ty, +n: S.Nui, +x0: U32, +x1: U32, +h: U32) -> Out:
  +label0 = MSG.T.common.refresh(Ty.loc(ty))
  +u = {S.Nui.ui(n) : U.Ui}
  +v = {U.Rp.view.loc(Ty.loc(ty), u) : U.RpView}
  +rw = Lay.button.w(ty, label0)
  +th = {U.Ui.sel(u) : String}
  Out.all([Out.ops([D.Draw.rect(D.Clip.all(), x0, 52, (x1 - x0 : U32), (h - 52 : U32), T.Th.panel(), 255)]),
    Rpw.chips(U.RpView.tabs(v), ty, (x0 + 16 : U32), (x1 - 32 - rw : U32)),
    Lay.button(ty, D.Clip.all(), (x1 - 16 - rw : U32), 64, label0, T.Th.soft(), "rv-run", ""),
    Out.ops(Ty.text(ty, 2n, Ty.fit(ty, 2n, MSG.T.review.page.title(Ty.loc(ty), U.RpView.title(v), Bool.pick(String, U.RpView.busy(v), "busy", "idle")), (x1 - x0 - 32 : U32)), D.Clip.all(), (x0 + 16 : U32), 102, 28, T.Th.dim())),
    Out.ops([D.Draw.rect(D.Clip.all(), x0, 137, (x1 - x0 : U32), 1, T.Th.line(), 255)]),
    Rlw.sel(String.eq(U.RpView.page(v), "release"), ty, n, v, th, Pane.off(U.Ui.infos(u), "rp:" ++ th), x0, x1, h)])

def Rpw.if(open: Bool, +ty: Ty, +n: S.Nui, +x0: U32, +x1: U32, +h: U32) -> Out:
  match open:
    case True{}:
      Rpw.page(ty, n, x0, x1, h)
    case False{}:
      Out.none()

def Lay.viewer.b(+ty: Ty, +n: S.Nui, +b: U.HBar, +x0: U32, +x1: U32, +h: U32) -> Out:
  Out.all([
    Lay.mech.if(U32.is_eq(S.Vw.kind(S.Nui.vw(n)), 5), ty, S.Nui.ui(n), x0, x1, h),
    Lay.viewer.vw(ty, S.Nui.vw(n), U.Ui.draft_text(S.Nui.ui(n)), U.Ui.view_get(S.Nui.ui(n), "diff"), U.Mpage.onpart(S.Nui.ui(n)), x0, x1, (h - Hb.h(b) : U32)),
    Mech.bar.if(U.Mpage.onpart(S.Nui.ui(n)), ty, S.Nui.ui(n), x0, x1),
    Lay.page.if(U32.is_eq(S.Vw.kind(S.Nui.vw(n)), 3), ty, S.Nui.ui(n), x0, x1, h),
    Side.k(U32.is_lt(S.Vw.kind(S.Nui.vw(n)), 2), ty, n, S.Vw.kind(S.Nui.vw(n)), Lay.scene(n), x0, x1, (h - Hb.h(b) : U32)),
    Side.ground.if(U32.is_eq(S.Vw.kind(S.Nui.vw(n)), 2), ty, n, x1),
    Hb.draw(ty, n, S.Nui.ui(n), b, Lay.scene(n), x0, x1, h),
    Fv.k(S.Vw.kind(S.Nui.vw(n)), ty, S.Nui.ui(n), x0, x1, h),
    Rpw.if(U32.is_eq(S.Vw.kind(S.Nui.vw(n)), 9), ty, n, x0, x1, h),
    Rcw.if(Bool.and(U32.is_lt(S.Vw.kind(S.Nui.vw(n)), 2), String.is_empty(U.Ui.scratch(S.Nui.ui(n), "@cmp.menu"))), ty, n, x0, x1, (h - Hb.h(b) : U32))])

# the viewer, with the design history's bar at its foot when it has one
# (only a board or a schematic: the native kind agrees with the client's)
def Lay.viewer(+ty: Ty, +n: S.Nui, +x0: U32, +x1: U32, +h: U32) -> Out:
  Lay.viewer.b(ty, n, Hb.for(ty, U32.is_lt(S.Vw.kind(S.Nui.vw(n)), 2), S.Nui.ui(n)), x0, x1, h)

def Lay.note.k(+ty: Ty, empty: Bool, offer: Bool, +note: String, +x: U32, +y: U32) -> Out:
  match empty offer:
    case True{} _:
      Out.none()
    case False{} True{}:
      Out{List.append(&2, R.Op, Lay.round(D.Clip.all(), x, y, (Ty.width(ty, 2n, note) + 16 : U32), 20, 10, T.Th.sel()),
          Ty.text(ty, 2n, note, D.Clip.all(), (x + 8 : U32), y, 20, T.Th.accent())),
        [Lay.hit(x, y, (Ty.width(ty, 2n, note) + 16 : U32), 20, "update", "")]}
    case False{} False{}:
      Out.ops(Ty.text(ty, 2n, note, D.Clip.all(), x, y, 20, T.Th.warn()))

# the footer's words: "update <version>" offers one; the hub's own
# progress words show as it sent them
def Lay.note.shown(+l: I.Loc, +note: String) -> String:
  Bool.pick(String, String.starts_with(note, "update "), MSG.T.sidebar.update(l, String.drop(note, 7n)), Lay.notice(l, note))

# an update to offer (clickable) or one being applied (status only), in
# the sidebar's footer beside Settings
def Lay.note(+ty: Ty, +note: String, +h: U32) -> Out:
  +shown = Lay.note.shown(Ty.loc(ty), note)
  Lay.note.k(ty, String.is_empty(note), String.starts_with(note, "update "), Ty.fit(ty, 2n, shown, 150), U32.dim(248, (Ty.width(ty, 2n, Ty.fit(ty, 2n, shown, 150)) + 16 : U32)), (h - 34 : U32))

def Lay.ghost.k(on: Bool, +ty: Ty, +title: String, +y: U32) -> Out:
  match on:
    case True{}:
      Out.ops(List.concat(&2, R.Op, [Lay.box(D.Clip.all(), 6, (y - 15 : U32), 252, 30, 0, T.Th.raise(), T.Th.accent()),
        Ty.text(ty, 0n, Ty.fit(ty, 0n, title, 220), D.Clip.all(), 32, (y - 16 : U32), 32, T.Th.text())]))
    case False{}:
      Out.none()

# a row being dragged, drawn under the pointer
def Lay.ghost(+ty: Ty, +n: S.Nui) -> Out:
  match n:
    case S.Nui{+u, _, _, _, +my, _, _, _, _, _, _, _, vw}:
      Lay.ghost.k(Bool.and(Bool.not(String.is_empty(U.Ui.scratch(u, "@drag"))), S.Row.far(S.Row.y(n), my)), ty,
        U.Drag.title(U.Ui.state(u), U.Ui.scratch(u, "@drag")), U32.max(my, 67))

# Row menus
# ---------
# A right-click on a thread's row, a project's name or a file link opens its menu
# (core's Menu) where the pointer was ("@menu.x", "@menu.y"), kept inside
# the window; a press anywhere else shuts it ("menu-close" under it).

def Rm.items(+ty: Ty, xs: List<&2, U.MItem>, +x: U32, +y: U32) -> Out:
  match xs:
    case Nil{}:
      Out.none()
    case U.MItem{+l, +a, +v, +t} <> r:
      Out.cat(Out{Ty.text(ty, 2n, Ty.fit(ty, 2n, l, 184), D.Clip.all(), (x + 12 : U32), y, 30, Bool.pick(U32, String.eq(t, "danger"), T.Th.bad(), T.Th.soft())),
          [Lay.hit(x, y, 208, 30, a, v)]},
        Rm.items(ty, r, x, (y + 30 : U32)))

# (d where no pointer set it)
def Rm.num(+s: String, +d: U32) -> U32:
  Bool.pick(U32, String.is_empty(s), d, U32.from_nat(M.Switch.num(s, 0n)))

def Rm.box(+ty: Ty, +xs: List<&2, U.MItem>, +px: U32, +py: U32, +w: U32, +h: U32) -> Out:
  +hh = {(U32.from_nat(List.length(&2, U.MItem, xs)) * 30 + 8 : U32) : U32}
  +x = {U32.min(px, (w - 212 : U32)) : U32}
  +y = {U32.min(py, (h - hh - 4 : U32)) : U32}
  Out.all([Out{Nil{}, [Lay.hit(0, 0, w, h, "menu-close", "")]},
    Out.ops(Lay.box(D.Clip.all(), x, y, 208, hh, 0, T.Th.raise(), T.Th.line())),
    Rm.items(ty, xs, x, (y + 4 : U32))])

def Rm.draw.k(none: Bool, +ty: Ty, +u: U.Ui, +w: U32, +h: U32) -> Out:
  match none:
    case True{}:
      Out.none()
    case False{}:
      Rm.box(ty, U.Menu.items.loc(Ty.loc(ty), u, True{}), Rm.num(U.Ui.scratch(u, "@menu.x"), 40), Rm.num(U.Ui.scratch(u, "@menu.y"), 140), w, h)

def Rm.draw(+ty: Ty, +u: U.Ui, +w: U32, +h: U32) -> Out:
  Rm.draw.k(String.is_empty(U.Menu.open(u)), ty, u, w, h)

# the sidebar's body (what Lay.sidebar.u draws but the dragged ghost and
# the remove dialog), the part the window may keep (main.bend's memo)
def Lay.fresh.n(ty: Ty, ix: U.SIx, n: S.Nui) -> List<&2, SideFreshCtx>:
  match n:
    case S.Nui{U.Ui{st, sel, _, _, _, _, now, _, _, _, _, _, _, _, _, _, _}, _, _, _, _, _, _, _, _, _, _, _, _}:
      Lay.fresh(ty, ix, st, sel, now)

def Lay.sidebar.a.ix.m(~PC: Data, ~paint: PC -> ST.SideRowReq -> ST.SideRowStep, +ty: Ty, +n: S.Nui, +h: U32, +ix: U.SIx, utop: Out, +uh: U32, more: D.Clip -> U32 -> Col, +pc: PC) -> SideCol:
  match n:
    case S.Nui{U.Ui{+st, +sel, _, _, +open, on, +now, adding, _, +v, _, _, _, +note, _, _, _}, _, _, _, _, +sd, _, +caret, +focus, +path, _, _, _}:
      SideCol.layer(Lay.sidebar.k.m(~PC, ~paint, ty, D.Clip{0, 52, 264, h}, st, ix, sel, open, on, now, adding, v, sd, caret, S.Nui.mark(n), focus, path, h, U.Flag.get(open, "settings"), U.Machines.rows.loc(Ty.loc(ty), S.Nui.ui(n)),
        U.Ui.scratch(S.Nui.ui(n), "@hint"), Lay.hint.ids(String.eq(U.Ui.scratch(S.Nui.ui(n), "@hint"), "alt"), String.eq(U.Ui.scratch(S.Nui.ui(n), "@hint"), "ctrl"), S.Nui.ui(n)), U.Ui.scratch(S.Nui.ui(n), "@look"), U.Ui.scratch(S.Nui.ui(n), "@rm"),
        U.Ui.scratch(S.Nui.ui(n), "@f.pq"), utop, uh, more, pc), col => Out.cat(Lay.col.out(col), Lay.note(ty, note, h)))

def Lay.sidebar.a.ix(+ty: Ty, +n: S.Nui, +h: U32, +ix: U.SIx, utop: Out, +uh: U32, more: D.Clip -> U32 -> Col) -> Out:
  SideCol.out(Lay.sidebar.a.ix.m(~(List<&2, SideFreshCtx>), ~Lay.fresh.run, ty, n, h, ix, utop, uh, more, Lay.fresh.n(ty, ix, n)))

# (the index of the state made here: the window passes the one it keeps,
# main.bend's MM.Ix)
def Lay.sidebar.a(+ty: Ty, +n: S.Nui, +h: U32, utop: Out, +uh: U32, more: D.Clip -> U32 -> Col) -> Out:
  Lay.sidebar.a.ix(ty, n, h, U.Side.ix(U.Ui.state(S.Nui.ui(n))), utop, uh, more)

# the sidebar from its body: the ghost a drag shows and the remove dialog
# on top, in the order Lay.sidebar.u has them
def Lay.sidebar.with(a: Out, +ty: Ty, +n: S.Nui, +h: U32) -> Out:
  Out.all([a, Lay.ghost(ty, n), Rm.draw(ty, S.Nui.ui(n), S.Nui.width(n), h)])

def Lay.sidebar.u(+ty: Ty, +n: S.Nui, +h: U32, utop: Out, +uh: U32, more: D.Clip -> U32 -> Col) -> Out:
  Lay.sidebar.with(Lay.sidebar.a(ty, n, h, utop, uh, more), ty, n, h)

# the sidebar: its body given (the window's memo of it), or laid out
def Lay.side.m(m: Maybe<&2, Out>, +ty: Ty, +n: S.Nui, +h: U32, utop: Out, +uh: U32, more: D.Clip -> U32 -> Col) -> Out:
  match m:
    case Some{a}:
      Lay.sidebar.with(a, ty, n, h)
    case None{}:
      Lay.sidebar.u(ty, n, h, utop, uh, more)

def Lay.sidebar.x(+ty: Ty, +n: S.Nui, +h: U32, more: D.Clip -> U32 -> Col) -> Out:
  Lay.sidebar.u(ty, n, h, Out.none(), 0, more)

# the sidebar with nothing under the projects
def Lay.sidebar(+ty: Ty, +n: S.Nui, +h: U32) -> Out:
  Lay.sidebar.x(ty, n, h, c => y => Col{Out.none(), y})

# Command palette (ctrl+k)
# ------------------------

def Pal.rows(+ty: Ty, ps: List<&2, U.PItem>, +i: Nat, +sel: Nat, +x: U32, +y: U32, +w: U32, left: Nat) -> Out:
  match ps left:
    case U.PItem{+l, +a, _} <> t 1n+m:
      Out.cat(Out{List.concat(&2, R.Op, [
          Bool.pick(List<&2, R.Op>, Nat.is_eq(i, sel), [D.Draw.rect(D.Clip.all(), (x + 4 : U32), y, (w - 8 : U32), 30, T.Th.sel(), 255)], Nil{}),
          Ty.text(ty, 0n, Ty.fit(ty, 0n, l, (w - 140 : U32)), D.Clip.all(), (x + 16 : U32), y, 30, Bool.pick(U32, Nat.is_eq(i, sel), T.Th.text(), T.Th.soft())),
          Ty.text(ty, 2n, U.Palette.kind(a), D.Clip.all(), (x + w - 16 - Ty.width(ty, 2n, U.Palette.kind(a)) : U32), y, 30, T.Th.faint())]),
          [Lay.hit(x, y, w, 30, "vw-pal", Nat.show(i))]},
        Pal.rows(ty, t, 1n+i, sel, x, (y + 30 : U32), w, m))
    case _ _:
      Out.none()

def Pal.prompt.loc(+l: I.Loc, +mode: String) -> String:
  Bool.pick(String, String.eq(mode, "search"), MSG.T.palette.search.full(l), Bool.pick(String, String.eq(mode, "files"), MSG.T.palette.file.placeholder(l), MSG.T.palette.command.placeholder(l)))

def Pal.prompt(+mode: String) -> String:
  Pal.prompt.loc(I.En{}, mode)

def Lay.palette(+ty: Ty, +u: U.Ui, +ww: U32, +hh: U32, +blink: Bool) -> Out:
  +w = {U32.min(600, (ww - 40 : U32)) : U32}
  +x = {U32.shr((ww - w : U32)) : U32}
  +ps = U.Palette.items.loc(Ty.loc(ty), u)
  +n = {U32.from_nat(Nat.min(List.length(&2, U.PItem, ps), 12n)) : U32}
  +q = U.Palette.query(u)
  Out.all([
    Out{[D.Draw.rect(D.Clip.all(), 0, 0, ww, hh, 0, 140)], [Lay.hit(0, 0, ww, hh, "vw-palx", "")]},
    Out{Lay.box(D.Clip.all(), x, 80, w, (n * 30 + 58 : U32), 0, T.Th.panel(), T.Th.line()), [Lay.hit(x, 80, w, (n * 30 + 58 : U32), "", "")]},
    Out.ops(Ty.text(ty, 0n, Bool.pick(String, String.is_empty(q), Pal.prompt.loc(Ty.loc(ty), U.Palette.mode(u)), q), D.Clip.all(), (x + 16 : U32), 86, 36, Bool.pick(U32, String.is_empty(q), T.Th.faint(), T.Th.text()))),
    Out.ops([D.Draw.rect(D.Clip.all(), (x + 16 + Bool.pick(U32, String.is_empty(q), 0, Ty.width(ty, 0n, q)) : U32), 95, 1, 18, T.Th.accent(), Bool.pick(U32, blink, 255, 0)),
      D.Draw.rect(D.Clip.all(), x, 126, w, 1, T.Th.line(), 255)]),
    Pal.rows(ty, ps, 0n, U.Palette.index(u), x, 132, w, 12n)])

# Project picker
# --------------
# The projects (or the folder being browsed), a filter line (fuzzy), then
# what can be done: go to a project or add one, add the folder, create the
# typed folder, go up, or open a subfolder. A project's path sits at the
# row's right.

def Proj.tone(+a: String, +on: Bool) -> U32:
  Bool.pick(U32, Bool.or(String.eq(a, "proj-cd"), String.eq(a, "proj-pick")), Bool.pick(U32, on, T.Th.text(), T.Th.soft()), Bool.pick(U32, String.is_empty(a), T.Th.faint(), T.Th.accent()))

def Proj.detail(+ty: Ty, +d: String, +x: U32, +y: U32, +w: U32) -> List<&2, R.Op>:
  +f = Ty.fit(ty, 2n, d, (w * 2 / 5 : U32))
  Ty.text(ty, 2n, f, D.Clip.all(), (x + w - 16 - Ty.width(ty, 2n, f) : U32), y, 28, T.Th.faint())

def Proj.rows(+ty: Ty, +u: U.Ui, ps: List<&2, U.PItem>, +i: Nat, +sel: Nat, +x: U32, +y: U32, +w: U32, left: Nat) -> Out:
  match ps left:
    case U.PItem{+l, +a, +v} <> t 1n+m:
      +d = U.Proj.detail(u, U.PItem{l, a, v})
      Out.cat(Out{List.concat(&2, R.Op, [
          Bool.pick(List<&2, R.Op>, Nat.is_eq(i, sel), [D.Draw.rect(D.Clip.all(), (x + 4 : U32), y, (w - 8 : U32), 28, T.Th.sel(), 255)], Nil{}),
          Ty.text(ty, 2n, Ty.fit(ty, 2n, l, Bool.pick(U32, String.is_empty(d), (w - 32 : U32), (w * 3 / 5 - 24 : U32))), D.Clip.all(), (x + 16 : U32), y, 28, Proj.tone(a, Nat.is_eq(i, sel))),
          Proj.detail(ty, d, x, y, w)]),
          [Lay.hit(x, y, w, 28, "vw-proj", Nat.show(i))]},
        Proj.rows(ty, u, t, 1n+i, sel, x, (y + 28 : U32), w, m))
    case _ _:
      Out.none()

def Proj.first(+sel: Nat, +rows: Nat) -> Nat:
  Nat.sub(sel, Nat.sub(rows, 1n))

def Lay.proj(+ty: Ty, +u: U.Ui, +ww: U32, +hh: U32, +blink: Bool) -> Out:
  +w = {U32.min(640, (ww - 40 : U32)) : U32}
  +x = {U32.shr((ww - w : U32)) : U32}
  +ps = U.Proj.items.loc(Ty.loc(ty), u)
  +rows = Nat.max(Nat.min(U32.to_nat((U32.dim(hh, 300) / 28 : U32)), 16n), 4n)
  +n = {U32.from_nat(Nat.min(List.length(&2, U.PItem, ps), rows)) : U32}
  +q = U.Proj.query(u)
  +err = U.Ui.info(u, "proj.err")
  +ey = {Bool.pick(U32, String.is_empty(err), 0, 26) : U32}
  +sel = U.Proj.index(u)
  +top = Proj.first(sel, rows)
  +dir = U.Proj.browsing(u)
  +bh = {(n * 28 + 118 + ey : U32) : U32}
  Out.all([
    Out{[D.Draw.rect(D.Clip.all(), 0, 0, ww, hh, 0, 140)], [Lay.hit(0, 0, ww, hh, "vw-projx", "")]},
    Out{Lay.box(D.Clip.all(), x, 80, w, bh, 0, T.Th.panel(), T.Th.line()), [Lay.hit(x, 80, w, bh, "", "")]},
    Out.ops(Ty.text(ty, 2n, Ty.fit(ty, 2n, Bool.pick(String, dir, U.Proj.dir(u), MSG.T.tour.title.projects(Ty.loc(ty))), (w - 32 : U32)), D.Clip.all(), (x + 16 : U32), 84, 26, T.Th.dim())),
    Out.ops(Ty.text(ty, 0n, Bool.pick(String, String.is_empty(q), Bool.pick(String, dir, MSG.T.picker.folder.placeholder(Ty.loc(ty)), MSG.T.picker.project.placeholder(Ty.loc(ty))), q), D.Clip.all(), (x + 16 : U32), 110, 36,
      Bool.pick(U32, String.is_empty(q), T.Th.faint(), T.Th.text()))),
    Out.ops([D.Draw.rect(D.Clip.all(), (x + 16 + Bool.pick(U32, String.is_empty(q), 0, Ty.width(ty, 0n, q)) : U32), 119, 1, 18, T.Th.accent(), Bool.pick(U32, blink, 255, 0)),
      D.Draw.rect(D.Clip.all(), x, 150, w, 1, T.Th.line(), 255)]),
    Out.when(Bool.not(String.is_empty(err)), Out.ops(Ty.text(ty, 2n, Ty.fit(ty, 2n, err, (w - 32 : U32)), D.Clip.all(), (x + 16 : U32), 152, 26, T.Th.bad()))),
    Proj.rows(ty, u, List.drop(&2, U.PItem, ps, top), top, sel, x, (154 + ey : U32), w, rows),
    Out.ops(Ty.text(ty, 2n, Bool.pick(String, dir, MSG.T.picker.keys.folder(Ty.loc(ty)), MSG.T.picker.keys.project(Ty.loc(ty))), D.Clip.all(), (x + 16 : U32), (158 + ey + n * 28 : U32), 36, T.Th.faint()))])

def Lay.term_on(+n: S.Nui) -> Bool:
  Bool.and(U.Ui.flag(S.Nui.ui(n), "term"), Bool.not(String.is_empty(U.Ui.sel(S.Nui.ui(n)))))

# the terminal's height (0 when closed)
def Lay.term_h(+n: S.Nui) -> U32:
  Bool.pick(U32, Lay.term_on(n), (S.Nui.height(n) * 2 / 5 : U32), 0)

def Lay.term.of.k(on: Bool, +ty: Ty, +n: S.Nui, +x1: U32, +h: U32) -> Out:
  match on:
    case True{}:
      Lay.term(ty, U.Ui.term(S.Nui.ui(n), U.Ui.sel(S.Nui.ui(n))), 264, x1, (h - Lay.term_h(n) : U32), h, String.eq(S.Nui.focus_of(n), "terminal"), S.Blink.on(n))
    case False{}:
      Out.none()

def Lay.term.of(+ty: Ty, +n: S.Nui, +x1: U32, +h: U32) -> Out:
  Lay.term.of.k(Lay.term_on(n), ty, n, x1, h)

# the window as the main area sees it with the terminal below
def Lay.shrink(n: S.Nui, +dh: U32) -> S.Nui:
  match n:
    case S.Nui{u, w, h, mx, my, sd, sc, ca, fo, pa, hi, ms, vw}:
      S.Nui{u, w, (h - dh : U32), mx, my, sd, sc, ca, fo, pa, hi, ms, vw}

def Hover.nums(ps: List<&2, String>) -> List<&2, U32>:
  match ps:
    case Nil{}:
      Nil{}
    case p <> t:
      Maybe.default(&2, U32, U32.read(p), 0) <> Hover.nums(t)

def Hover.rect(xs: List<&2, U32>, +alpha: U32) -> Out:
  match xs:
    case +x0 <> +y0 <> +x1 <> +y1 <> _:
      Out.when(Bool.and(U32.is_lt((y1 - y0 : U32), 90), U32.is_gt(x1, x0)),
        Out.ops([D.Draw.rect(D.Clip.all(), x0, y0, (x1 - x0 : U32), (y1 - y0 : U32), T.Th.text(), ((alpha * 22 : U32) / 255 : U32)),
          D.Draw.rect(D.Clip.all(), x0, (y1 - 1 : U32), (x1 - x0 : U32), 1, T.Th.accent(), ((alpha * 160 : U32) / 255 : U32))]))
    case _:
      Out.none()

# the region under the pointer, lit (big regions like panels are skipped)
def Lay.hover(+key: String, +alpha: U32) -> Out:
  Bool.pick(Out, String.is_empty(key), Out.none(), Hover.rect(Hover.nums(String.split(key, ',')), alpha))

# a message's attachments, as the lines closing its bubble
def TL.chip(+p: String) -> String:
  Bool.pick(String, AT.Att.image(p), "▣ ", "▤ ") ++ AT.Att.label(p)

def TL.chips.texts(ps: List<&2, String>) -> List<&2, String>:
  match ps:
    case Nil{}:
      Nil{}
    case +p <> t:
      TL.chip(p) <> TL.chips.texts(t)

def TL.chips(ps: List<&2, String>, +w: U32) -> List<&2, TLine>:
  match ps:
    case Nil{}:
      Nil{}
    case +p <> t:
      TLine{TL.chip(p), Bool.pick(U32, AT.Att.image(p), T.Th.accent(), T.Th.soft()), 2, "", w} <> TL.chips(t, w)

# a user message: its text (wrapped), then its attachments
def TL.bubble.a(+ty: Ty, +text: String, +px: U32) -> List<&2, TLine>:
  +ls = Ty.wrap(ty, 3n, AT.Att.body(text), U32.min((px * 7 / 10 : U32), 560))
  +ps = AT.Att.paths(text)
  +w = {(TL.widest(ty, List.append(&2, String, ls, TL.chips.texts(ps)), 0) + 40 : U32) : U32}
  List.concat(&2, TLine, [[TLine{"", 0, 6, "", w}], TL.user(ls, AT.Att.body(text), True{}, w), TL.chips(ps, w), [TLine{"", 0, 6, "", w}, TLine{"", 0, 4, "", 0}]])

# an entry's own lines in three parts: what depends on the entry and the
# width alone (the costly part: markdown, wrapping, measuring; kept across
# frames), the image paths it names, and its footer. The images' lines
# are made each frame, since their sizes arrive later.
type TLPart is Data:
  TLPart{head: List<&2, TLine>, paths: List<&2, String>, tail: List<&2, TLine>}

# a tool call or other activity in a line; a design step as a row that
# shows it in the viewer (kind 16, copy its id)
def TL.part.act(step: Bool, +ty: Ty, e: M.Entry, +px: U32) -> TLPart:
  match step e:
    case True{} M.Act{+id, _, _, _, _, _}:
      TLPart{[TLine{Ty.fit(ty, 2n, HS.Hist.row(e), (px - 22 : U32)), T.Th.accent(), 16, id, 0}], Nil{}, Nil{}}
    case _ M.Act{_, _, +tone, +kind, +summary, _}:
      Bool.pick(TLPart, String.eq(tone, EM.Emb.tone()), TLPart{Nil{}, Nil{}, Nil{}},
        TLPart{[TLine{Ty.fit(ty, 2n, RVW.Review.label(tone, kind, summary) ++ "  " ++ RVW.Review.say(tone, kind, summary), px), Bool.pick(U32, String.eq(tone, "error"), T.Th.bad(), T.Th.dim()), 3, "", 0}], Nil{}, Nil{}})
    case _ M.Msg{_, _, _, +text, _}:
      TLPart{[TLine{Ty.fit(ty, 2n, text, px), T.Th.dim(), 3, "", 0}], Nil{}, Nil{}}

def TL.part.own(+ty: Ty, e: M.Entry, +px: U32) -> TLPart:
  match e:
    case M.Msg{_, _, M.User{}, +text, _}:
      TLPart{TL.bubble.a(ty, text, px), Nil{}, Nil{}}
    case M.Msg{_, _, _, +text, _}:
      TLPart{TL.blocks(ty, N.Md.render(text), px), IM.Img.paths(text), [TLine{"", 0, 7, text, 0}, TLine{"", 0, 4, "", 0}]}
    case M.Act{+id, +th, +tone, +kind, +summary, +at}:
      TL.part.act(String.eq(tone, HS.Step.tone()), ty, M.Act{id, th, tone, kind, summary, at}, px)

# only this thread's own entries have parts; a line it sent elsewhere is
# made each frame (it names the other thread)
def TL.part(mine: Bool, +ty: Ty, e: M.Entry, +px: U32) -> TLPart:
  match mine:
    case True{}:
      TL.part.own(ty, e, px)
    case False{}:
      TLPart{Nil{}, Nil{}, Nil{}}

def TL.part.join(+ty: Ty, p: TLPart, +pics: Map<&2, String>, +px: U32) -> List<&2, TLine>:
  match p:
    case TLPart{h, ps, t}:
      List.concat(&2, TLine, [h, TL.images(ty, ps, pics, px), t])

def TL.mine_of(+e: M.Entry, +sel: String) -> Bool:
  String.eq(TL.thread_of(e), sel)

# an entry of this thread (headed by the thread it came from, if another
# thread sent it) or one this thread sent elsewhere (a link line), from
# its part
# a terminal's lines, made each frame (whether it is live changes with
# the clock, the pin and the person's messages)
def TL.emb.rows(n: Nat, +i: U32, +cols: U32, +rows: U32, +key: String) -> List<&2, TLine>:
  match n:
    case 0n:
      Nil{}
    case 1n+j:
      TLine{U32.show(cols), rows, 23, key, i} <> TL.emb.rows(j, (i + 1 : U32), cols, rows, key)

def TL.emb.k(live: Bool, pinned: Bool, +ty: Ty, +th: String, ev: EM.EV, +px: U32) -> List<&2, TLine>:
  match live pinned ev:
    case True{} True{} EM.EV{+key, _, +title, _, _, _, _}:
      [TLine{Ty.fit(ty, 2n, MSG.T.terminal.pinned(Ty.loc(ty), title), (px - 26 : U32)), T.Th.faint(), 26, key, 0}]
    case True{} False{} EM.EV{+key, _, +title, +cols, +rows, _, _}:
      TLine{Ty.fit(ty, 2n, title, (Ew.w(ty, cols, 0, px) - 56 : U32)), T.Th.dim(), 24, key, cols} <> List.append(&2, TLine, TL.emb.rows(U32.to_nat(rows), 0, cols, rows, key), [TLine{"", 0, 4, "", 0}])
    case False{} _ EM.EV{_, +id, +title, _, _, _, _}:
      [TLine{Ty.fit(ty, 2n, MSG.T.terminal.inactive(Ty.loc(ty), title), (px - 26 : U32)), T.Th.faint(), 25, th ++ "\t" ++ id, 0}]

def TL.emb.ev(+ev: EM.EV, +ty: Ty, +th: String, +px: U32) -> List<&2, TLine>:
  match ev:
    case EM.EV{k, i, t, c, r, +live, +pinned}:
      TL.emb.k(live, pinned, ty, th, EM.EV{k, i, t, c, r, live, pinned}, px)

def TL.emb(+ty: Ty, +e: M.Entry, +st: M.State, +px: U32, +pics: Map<&2, String>) -> List<&2, TLine>:
  TL.emb.ev(EM.Emb.view(st, e, EM.Emb.last_user(M.State.mine(st, TL.thread_of(e)), 0n), EM.Emb.at.num(U.Str.get(pics, "clock"), 0n)), ty, TL.thread_of(e), px)

def TL.entry.t(term: Bool, +ty: Ty, +e: M.Entry, +st: M.State, +px: U32, +pics: Map<&2, String>, p: TLPart) -> List<&2, TLine>:
  match term:
    case True{}:
      TL.emb(ty, e, st, px, pics)
    case False{}:
      TL.part.join(ty, p, pics, px)

# a reminder (a Reminded in the log): a bell, the text and when, in the person's zone
def TL.remind.is(e: M.Entry) -> Bool:
  match e:
    case M.Act{_, _, +tone, _, _, _}:
      String.eq(tone, "remind")
    case _:
      False{}

def TL.remind.line(+ty: Ty, e: M.Entry, +st: M.State, +px: U32) -> List<&2, TLine>:
  match e:
    case M.Act{_, _, _, _, +text, +at}:
      [TLine{Ty.fit(ty, 2n, MSG.T.thread.reminder(Ty.loc(ty), text, TZ.Tz.show.nz(SCH.Sched.person(st, 0n, True{}), at)), (px - 26 : U32)), T.Th.accent(), 27, "", 0}]
    case _:
      Nil{}

def TL.entry.r(rem: Bool, +ty: Ty, +e: M.Entry, +st: M.State, +px: U32, +pics: Map<&2, String>, p: TLPart) -> List<&2, TLine>:
  match rem:
    case True{}:
      TL.remind.line(ty, e, st, px)
    case False{}:
      TL.entry.t(EM.Emb.is(e), ty, e, st, px, pics, p)

# The time under a message (core/stamp.bend): a user message's on a line
# of its own under the bubble (kind 28, before its gap), a reply's in its
# footer (kind 7)
def TL.stamp.hit(+user: Bool, +k: U32) -> Bool:
  Bool.pick(Bool, user, U32.is_eq(k, 4), U32.is_eq(k, 7))

def TL.stamp.line(hit: Bool, user: Bool, l: TLine, +stamp: String) -> List<&2, TLine>:
  match hit user l:
    case False{} _ _:
      [l]
    case True{} True{} _:
      [TLine{stamp, T.Th.faint(), 28, "", 0}, l]
    case True{} False{} TLine{_, c, k, cp, w}:
      [TLine{stamp, c, k, cp, w}]

def TL.stamp.at(ls: List<&2, TLine>, +user: Bool, +stamp: String) -> List<&2, TLine>:
  match ls:
    case Nil{}:
      Nil{}
    case TLine{+x, +c, +k, +cp, +w} <> t:
      List.append(&2, TLine, TL.stamp.line(TL.stamp.hit(user, k), user, TLine{x, c, k, cp, w}, stamp), TL.stamp.at(t, user, stamp))

def TL.stamp.on(none: Bool, ls: List<&2, TLine>, +user: Bool, +stamp: String) -> List<&2, TLine>:
  match none:
    case True{}:
      ls
    case False{}:
      TL.stamp.at(ls, user, stamp)

def TL.is.user(r: M.Role) -> Bool:
  match r:
    case M.User{}:
      True{}
    case _:
      False{}

def TL.entry.q(mine: Bool, +ty: Ty, e: M.Entry, +st: M.State, +px: U32, +pics: Map<&2, String>, p: TLPart, +stamp: String) -> List<&2, TLine>:
  match mine e:
    case True{} M.Msg{+id, _, r, _, _}:
      List.append(&2, TLine, TL.from.k(String.is_empty(M.Link.from(id)), M.Link.from(id), st, px, ty), TL.stamp.on(String.is_empty(stamp), TL.part.join(ty, p, pics, px), TL.is.user(r), stamp))
    case True{} +a:
      TL.entry.r(TL.remind.is(a), ty, a, st, px, pics, p)
    case False{} M.Msg{_, +t, _, +x, _}:
      [TLine{Ty.fit(ty, 2n, "→ " ++ U.Tasks.title(st, t) ++ ":  " ++ TL.first_line(x), px), T.Th.accent(), 10, t, 0}]
    case False{} _:
      Nil{}

def TL.entry.p(mine: Bool, +ty: Ty, +e: M.Entry, +st: M.State, +px: U32, +pics: Map<&2, String>, p: TLPart, +sp: STP.Stamps) -> List<&2, TLine>:
  TL.entry.q(mine, ty, e, st, px, pics, p, STP.Stamp.text.loc(Ty.loc(ty), sp, e))

def TL.entry(+ty: Ty, +e: M.Entry, +st: M.State, +sel: String, +px: U32, +pics: Map<&2, String>, +sp: STP.Stamps) -> List<&2, TLine>:
  TL.entry.p(TL.mine_of(e, sel), ty, e, st, px, pics, TL.part(TL.mine_of(e, sel), ty, e, px), sp)

# Tool calls folded
# -----------------
# A run of tool calls shows as one row (FO.Fold decides the runs, their
# labels and which are open); an entry's lines are shown as its role says.
# What is open comes from the info map and the turn, both read each frame,
# so the memo of entries' parts is untouched by it.

def TL.fold.line(+ty: Ty, +id: String, +label: String, +open: Bool, +px: U32) -> TLine:
  TLine{Ty.fit(ty, 2n, label, (px - 26 : U32)), T.Th.dim(), 14, FO.Fold.ask(open, id), Bool.pick(U32, open, 1, 0)}

def TL.fold.shown(open: Bool, ls: List<&2, TLine>) -> List<&2, TLine>:
  match open:
    case True{}:
      ls
    case False{}:
      Nil{}

def TL.role(r: FO.Role, +ty: Ty, ls: List<&2, TLine>, +px: U32) -> List<&2, TLine>:
  match r:
    case FO.RShow{}:
      ls
    case FO.RHead{+id, +label, +open}:
      TL.fold.line(ty, id, label, open, px) <> TL.fold.shown(open, ls)
    case FO.RIn{open}:
      TL.fold.shown(open, ls)
    case FO.RWork{+id, +label, open, inner}:
      match open:
        case True{}:
          TL.fold.line(ty, id, label, True{}, px) <> TL.role(inner, ty, ls, px)
        case False{}:
          [TL.fold.line(ty, id, label, False{}, px)]

def TL.turn(t: M.Thread) -> Bool:
  match t:
    case M.Thread{_, _, _, _, _, turn, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      Lay.running(turn)

# the roles of the open thread's entries (its last run stays open while
# its turn runs)
def TL.roles.loc(+l: I.Loc, es: List<&2, M.Entry>, +st: M.State, +sel: String, +pics: Map<&2, String>) -> List<&2, FO.Role>:
  FO.Fold.roles.loc(l, es, pics, TL.turn(U.Ui.thread(st, sel)))

def TL.roles(es: List<&2, M.Entry>, +st: M.State, +sel: String, +pics: Map<&2, String>) -> List<&2, FO.Role>:
  TL.roles.loc(I.En{}, es, st, sel, pics)

# The timeline's memo
# -------------------
# Each entry of the open thread with its part, made at one width. The next
# frame keeps the part of every entry exactly equal to the one it was made
# for and makes the rest (laws tl_again_exact, tl_keep, tl_memo_exact,
# tl_entries_memo: the memo's timeline is the timeline from scratch).

type TLE is Data:
  TLE{e: M.Entry, part: TLPart}

# (items: the entries' parts; lv: the live reply's settled prefix)
type TLMemo is Data:
  TLMemo{px: U32, sel: String, items: List<&2, TLE>, lv: LiveMemo}

def TL.memo.none() -> TLMemo:
  TLMemo{0, "", Nil{}, TL.live.none()}

def TL.memo.lv(m: TLMemo) -> LiveMemo:
  match m:
    case TLMemo{_, _, _, lv}:
      lv

def TL.memo.with_live(m: TLMemo, lv: LiveMemo) -> TLMemo:
  match m:
    case TLMemo{px, sel, its, _}:
      TLMemo{px, sel, its, lv}

def TL.memo.item(+ty: Ty, +e: M.Entry, +px: U32, +sel: String) -> TLE:
  TLE{e, TL.part(TL.mine_of(e, sel), ty, e, px)}

def TL.memo.make(+ty: Ty, es: List<&2, M.Entry>, +px: U32, +sel: String) -> List<&2, TLE>:
  match es:
    case Nil{}:
      Nil{}
    case +e <> t:
      TL.memo.item(ty, e, px, sel) <> TL.memo.make(ty, t, px, sel)

def TL.memo.one.k(same: Bool, +ty: Ty, +e: M.Entry, +px: U32, +sel: String, it: TLE) -> TLE:
  match same:
    case True{}:
      it
    case False{}:
      TL.memo.item(ty, e, px, sel)

# an entry equal to the one its item was made for keeps the item
def TL.memo.one(+ty: Ty, +e: M.Entry, +px: U32, +sel: String, it: TLE) -> TLE:
  match it:
    case TLE{+pe, +pp}:
      TL.memo.one.k(Q.Eq.entry(e, pe), ty, e, px, sel, TLE{pe, pp})

# the entries now, over the items last made at the same width and thread
def TL.memo.again(es: List<&2, M.Entry>, +ty: Ty, +px: U32, +sel: String, prev: List<&2, TLE>) -> List<&2, TLE>:
  match es prev:
    case Nil{} _:
      Nil{}
    case +e <> t +it <> pt:
      TL.memo.one(ty, e, px, sel, it) <> TL.memo.again(t, ty, px, sel, pt)
    case +e <> t Nil{}:
      TL.memo.item(ty, e, px, sel) <> TL.memo.make(ty, t, px, sel)

# Aligning a memo
# ---------------
# The page shown moves: a new entry pushes the oldest out of a full page,
# and scrolling to the top adds a page in front. Paired place by place,
# every item would then meet another entry and be laid out again. So the
# last memo is lined up with the entries first (by entry id; TL.memo.one
# still checks the whole entry, so a wrong guess costs only a layout): the
# items before the first entry's are dropped, or the entries before the
# first item's are made. Law tl_align: an aligned memo is still a memo.

type TLAlign is Data:
  TLSame{}
  TLDrop{d: Nat}
  TLPad{k: Nat}

def TL.eid(e: M.Entry) -> String:
  match e:
    case M.Msg{i, _, _, _, _}:
      i
    case M.Act{i, _, _, _, _, _}:
      i

def TL.align.hit(its: List<&2, TLE>, +id: String) -> Bool:
  match its:
    case TLE{e, _} <> _:
      Q.Eq.str(TL.eid(e), id)
    case Nil{}:
      False{}

# where the item for entry id is among its
def TL.align.at(its: List<&2, TLE>, hit: Bool, +id: String, +d: Nat) -> Maybe<&2, Nat>:
  match its hit:
    case Nil{} _:
      None{}
    case +h <> +r True{}:
      Some{d}
    case +h <> t False{}:
      TL.align.at(t, TL.align.hit(t, id), id, 1n+d)

def TL.align.ehit(es: List<&2, M.Entry>, +id: String) -> Bool:
  match es:
    case e <> _:
      Q.Eq.str(TL.eid(e), id)
    case Nil{}:
      False{}

# where entry id is among es
def TL.align.eat(es: List<&2, M.Entry>, hit: Bool, +id: String, +k: Nat) -> Maybe<&2, Nat>:
  match es hit:
    case Nil{} _:
      None{}
    case +h <> +r True{}:
      Some{k}
    case +h <> t False{}:
      TL.align.eat(t, TL.align.ehit(t, id), id, 1n+k)

def TL.align.first(its: List<&2, TLE>) -> String:
  match its:
    case TLE{e, _} <> _:
      TL.eid(e)
    case Nil{}:
      ""

def TL.align.efirst(es: List<&2, M.Entry>) -> String:
  match es:
    case e <> _:
      TL.eid(e)
    case Nil{}:
      ""

def TL.align.pad(m: Maybe<&2, Nat>) -> TLAlign:
  match m:
    case Some{k}:
      TLPad{k}
    case None{}:
      TLSame{}

def TL.align.k(m: Maybe<&2, Nat>, +es: List<&2, M.Entry>, +its: List<&2, TLE>) -> TLAlign:
  match m:
    case Some{d}:
      TLDrop{d}
    case None{}:
      TL.align.pad(TL.align.eat(es, TL.align.ehit(es, TL.align.first(its)), TL.align.first(its), 0n))

# how the items line up with the entries: the first entry's item found
# some places in (dropped before it), else the first item's entry found
# some places in (those before it made), else as they are
def TL.align.of(+es: List<&2, M.Entry>, +its: List<&2, TLE>) -> TLAlign:
  TL.align.k(TL.align.at(its, TL.align.hit(its, TL.align.efirst(es)), TL.align.efirst(es), 0n), es, its)

def TL.skip(d: Nat, its: List<&2, TLE>) -> List<&2, TLE>:
  match d its:
    case 0n +xs:
      xs
    case 1n+k Nil{}:
      Nil{}
    case 1n+k +x <> t:
      TL.skip(k, t)

def TL.eskip(d: Nat, es: List<&2, M.Entry>) -> List<&2, M.Entry>:
  match d es:
    case 0n +xs:
      xs
    case 1n+k Nil{}:
      Nil{}
    case 1n+k +x <> t:
      TL.eskip(k, t)

def TL.etake(k: Nat, es: List<&2, M.Entry>) -> List<&2, M.Entry>:
  match k es:
    case 0n +xs:
      Nil{}
    case 1n+j Nil{}:
      Nil{}
    case 1n+j +e <> t:
      e <> TL.etake(j, t)

def TL.cat(xs: List<&2, TLE>, ys: List<&2, TLE>) -> List<&2, TLE>:
  match xs:
    case Nil{}:
      ys
    case +x <> t:
      x <> TL.cat(t, ys)

def TL.ecat(xs: List<&2, M.Entry>, ys: List<&2, M.Entry>) -> List<&2, M.Entry>:
  match xs:
    case Nil{}:
      ys
    case +x <> t:
      x <> TL.ecat(t, ys)

def TL.memo.apply(c: TLAlign, +es: List<&2, M.Entry>, its: List<&2, TLE>, +ty: Ty, +px: U32, +sel: String) -> List<&2, TLE>:
  match c:
    case TLSame{}:
      its
    case TLDrop{d}:
      TL.skip(d, its)
    case TLPad{k}:
      TL.cat(TL.memo.make(ty, TL.etake(k, es), px, sel), its)

# the entries an aligned memo was made for (tl_align)
def TL.memo.apply.es(c: TLAlign, +es: List<&2, M.Entry>, old: List<&2, M.Entry>) -> List<&2, M.Entry>:
  match c:
    case TLSame{}:
      old
    case TLDrop{d}:
      TL.eskip(d, old)
    case TLPad{k}:
      TL.ecat(TL.etake(k, es), old)

def TL.memo.aligned(+es: List<&2, M.Entry>, +ty: Ty, +px: U32, +sel: String, +its: List<&2, TLE>) -> List<&2, TLE>:
  TL.memo.apply(TL.align.of(es, its), es, its, ty, px, sel)

def TL.memo.of.k(same: Bool, +ty: Ty, +es: List<&2, M.Entry>, +px: U32, +sel: String, prev: List<&2, TLE>) -> TLMemo:
  match same:
    case True{}:
      TLMemo{px, sel, TL.memo.again(es, ty, px, sel, TL.memo.aligned(es, ty, px, sel, prev)), TL.live.none()}
    case False{}:
      TLMemo{px, sel, TL.memo.make(ty, es, px, sel), TL.live.none()}

def TL.memo.of(+ty: Ty, es: List<&2, M.Entry>, +px: U32, +sel: String, prev: TLMemo) -> TLMemo:
  match prev:
    case TLMemo{ppx, psel, items, _}:
      TL.memo.of.k(Bool.and(U32.is_eq(ppx, px), Q.Eq.str(psel, sel)), ty, es, px, sel, items)

# Recently shown threads keep their memos, the latest first, so going
# back to one lays out only what changed in it. Any memo is a sound start
# (tl_memo_exact holds for every earlier memo); laws tl_memos_hit (the
# thread just left is found again) and tl_memos_bounded (at most
# TL.memos.max() are kept).

def TL.memos.max() -> Nat:
  8n

def TL.memo.sel(m: TLMemo) -> String:
  match m:
    case TLMemo{_, sel, _, _}:
      sel

def TL.memos.pick.k(hit: Bool, m: TLMemo, rest: TLMemo) -> TLMemo:
  match hit:
    case True{}:
      m
    case False{}:
      rest

# the memo kept for a thread (none: start from scratch)
def TL.memos.pick(ms: List<&2, TLMemo>, +sel: String) -> TLMemo:
  match ms:
    case Nil{}:
      TL.memo.none()
    case +m <> t:
      TL.memos.pick.k(Q.Eq.str(TL.memo.sel(m), sel), m, TL.memos.pick(t, sel))

def TL.memos.drop.k(same: Bool, m: TLMemo, rest: List<&2, TLMemo>) -> List<&2, TLMemo>:
  match same:
    case True{}:
      rest
    case False{}:
      m <> rest

def TL.memos.drop(ms: List<&2, TLMemo>, +sel: String) -> List<&2, TLMemo>:
  match ms:
    case Nil{}:
      Nil{}
    case +m <> t:
      TL.memos.drop.k(Q.Eq.str(TL.memo.sel(m), sel), m, TL.memos.drop(t, sel))

def TL.memos.cut(n: Nat, ms: List<&2, TLMemo>) -> List<&2, TLMemo>:
  match n ms:
    case 0n _:
      Nil{}
    case 1n+k Nil{}:
      Nil{}
    case 1n+k m <> t:
      m <> TL.memos.cut(k, t)

# m first, then the others that fit
def TL.memos.keep(+m: TLMemo, ms: List<&2, TLMemo>) -> List<&2, TLMemo>:
  m <> TL.memos.cut(Nat.sub(TL.memos.max(), 1n), TL.memos.drop(ms, TL.memo.sel(m)))

# the memo in front (the one this frame drew from)
def TL.memos.now(ms: List<&2, TLMemo>) -> TLMemo:
  match ms:
    case Nil{}:
      TL.memo.none()
    case m <> _:
      m

# the entries the memo's items were made for
def TL.memo.es(its: List<&2, TLE>) -> List<&2, M.Entry>:
  match its:
    case Nil{}:
      Nil{}
    case TLE{e, _} <> t:
      e <> TL.memo.es(t)

# the entry the design history went back to (client.bend's Hs.jump) under
# a rule the timeline scrolls to (kind 17, Lay.timeline's "tl-jump")
def TL.mark(hit: Bool, ls: List<&2, TLine>) -> List<&2, TLine>:
  match hit:
    case True{}:
      TLine{"", 0, 17, "", 0} <> ls
    case False{}:
      ls

# An entry boundary has no pixels or text. It lets the held view find the
# same entry after an earlier image loads, a tool group opens, or text wraps.
def TL.entry.bound(+e: M.Entry, ls: List<&2, TLine>) -> List<&2, TLine>:
  TLine{"", 0, 29, M.Idx.id(e), 0} <> ls

# the entries, each as its role says (one with no role as it is)
def TL.entries.r(+ty: Ty, es: List<&2, M.Entry>, rs: List<&2, FO.Role>, +st: M.State, +sel: String, +px: U32, +pics: Map<&2, String>, +sp: STP.Stamps) -> List<&2, TLine>:
  match es rs:
    case Nil{} _:
      Nil{}
    case +e <> t r <> rt:
      List.append(&2, TLine, TL.entry.bound(e, TL.mark(String.eq(M.Idx.id(e), U.Str.get(pics, "hist.jump")), TL.role(r, ty, TL.entry(ty, e, st, sel, px, pics, sp), px))), TL.entries.r(ty, t, rt, st, sel, px, pics, sp))
    case +e <> t Nil{}:
      List.append(&2, TLine, TL.entry.bound(e, TL.entry(ty, e, st, sel, px, pics, sp)), TL.entries.r(ty, t, Nil{}, st, sel, px, pics, sp))

# the times under the messages (the clock is the client's last tick)
def TL.stamps(+es: List<&2, M.Entry>, +st: M.State, +sel: String, +pics: Map<&2, String>) -> STP.Stamps:
  STP.Stamps.of(es, st, FO.Fold.now(pics), TL.turn(U.Ui.thread(st, sel)))

# the timeline from scratch, in the frame's language
def TL.entries.loc(+ty: Ty, +es: List<&2, M.Entry>, +st: M.State, +sel: String, +px: U32, +pics: Map<&2, String>) -> List<&2, TLine>:
  TL.entries.r(ty, es, TL.roles.loc(Ty.loc(ty), es, st, sel, pics), st, sel, px, pics, TL.stamps(es, st, sel, pics))

# and in English (the memo laws' form)
def TL.entries(+ty: Ty, +es: List<&2, M.Entry>, +st: M.State, +sel: String, +px: U32, +pics: Map<&2, String>) -> List<&2, TLine>:
  TL.entries.r(ty, es, TL.roles(es, st, sel, pics), st, sel, px, pics, TL.stamps(es, st, sel, pics))

# the memo's items, each as its role says
def TL.entries.mr(+ty: Ty, its: List<&2, TLE>, rs: List<&2, FO.Role>, +st: M.State, +sel: String, +px: U32, +pics: Map<&2, String>, +sp: STP.Stamps) -> List<&2, TLine>:
  match its rs:
    case Nil{} _:
      Nil{}
    case TLE{+e, p} <> t r <> rt:
      List.append(&2, TLine, TL.entry.bound(e, TL.mark(String.eq(M.Idx.id(e), U.Str.get(pics, "hist.jump")), TL.role(r, ty, TL.entry.p(TL.mine_of(e, sel), ty, e, st, px, pics, p, sp), px))), TL.entries.mr(ty, t, rt, st, sel, px, pics, sp))
    case TLE{+e, p} <> t Nil{}:
      List.append(&2, TLine, TL.entry.bound(e, TL.entry.p(TL.mine_of(e, sel), ty, e, st, px, pics, p, sp)), TL.entries.mr(ty, t, Nil{}, st, sel, px, pics, sp))

# the timeline from the memo's items, in the frame's language (law
# tl_entries_locale)
def TL.entries.m.loc(+ty: Ty, +its: List<&2, TLE>, +st: M.State, +sel: String, +px: U32, +pics: Map<&2, String>) -> List<&2, TLine>:
  TL.entries.mr(ty, its, TL.roles.loc(Ty.loc(ty), TL.memo.es(its), st, sel, pics), st, sel, px, pics, TL.stamps(TL.memo.es(its), st, sel, pics))

# and in English
def TL.entries.m(+ty: Ty, +its: List<&2, TLE>, +st: M.State, +sel: String, +px: U32, +pics: Map<&2, String>) -> List<&2, TLine>:
  TL.entries.mr(ty, its, TL.roles(TL.memo.es(its), st, sel, pics), st, sel, px, pics, TL.stamps(TL.memo.es(its), st, sel, pics))

def TL.memo.lines.k(same: Bool, +ty: Ty, m: TLMemo, +es: List<&2, M.Entry>, +st: M.State, +sel: String, +px: U32, +pics: Map<&2, String>) -> List<&2, TLine>:
  match same m:
    case True{} TLMemo{_, _, its, _}:
      TL.entries.m(ty, TL.memo.again(es, ty, px, sel, its), st, sel, px, pics)
    case False{} _:
      TL.entries(ty, es, st, sel, px, pics)

def TL.memo.fits(m: TLMemo, +px: U32, +sel: String) -> Bool:
  match m:
    case TLMemo{mpx, msel, _, _}:
      Bool.and(U32.is_eq(mpx, px), Q.Eq.str(msel, sel))

# the timeline from a memo already brought up to this frame (Lay.memo, as
# the window does before each frame): its items are the entries' parts, so
# they are not compared again (law tl_frame_exact)
def TL.memo.shown(+ty: Ty, m: TLMemo, +st: M.State, +sel: String, +px: U32, +pics: Map<&2, String>) -> List<&2, TLine>:
  match m:
    case TLMemo{_, _, its, _}:
      TL.entries.m(ty, its, st, sel, px, pics)

# the same in the frame's language: what the window draws
def TL.memo.shown.loc(+ty: Ty, m: TLMemo, +st: M.State, +sel: String, +px: U32, +pics: Map<&2, String>) -> List<&2, TLine>:
  match m:
    case TLMemo{_, _, its, _}:
      TL.entries.m.loc(ty, its, st, sel, px, pics)

# the timeline, from the memo when it was made for this width and thread
def TL.memo.lines(+ty: Ty, +m: TLMemo, +es: List<&2, M.Entry>, +st: M.State, +sel: String, +px: U32, +pics: Map<&2, String>) -> List<&2, TLine>:
  TL.memo.lines.k(TL.memo.fits(m, px, sel), ty, m, es, st, sel, px, pics)

# messages the server has not stored yet, marked as sending
def TL.sending(+ty: Ty, ps: List<&2, String>, +px: U32) -> List<&2, TLine>:
  match ps:
    case Nil{}:
      Nil{}
    case +p <> t:
      List.concat(&2, TLine, [TL.bubble.a(ty, p, px), [TLine{MSG.T.thread.sending(Ty.loc(ty)), T.Th.faint(), 3, "", 0}], TL.sending(ty, t, px)])

# the draft's last lines and where the caret sits among them
# the next message's attachments, as chips from x (a click removes one),
# then the file still uploading
def Lay.atts(+ty: Ty, ps: List<&2, String>, +up: String, +x: U32, +y: U32) -> Out:
  match ps:
    case Nil{}:
      Out.when(Bool.not(String.is_empty(up)), Out.ops(Ty.text(ty, 2n, up ++ " …", D.Clip.all(), x, y, 28, T.Th.faint())))
    case +p <> t:
      Out.cat(Lay.chip(ty, x, y, TL.chip(p) ++ "  ×", AT.Att.image(p), "detach", p),
        Lay.atts(ty, t, up, (x + Lay.button.w(ty, TL.chip(p) ++ "  ×") + 6 : U32), y))

# the path field "Attach" opens above the composer (Enter attaches)
def Lay.attach_field(+ty: Ty, on: Bool, +path: String, +caret: Nat, +mark: Nat, +x0: U32, +x1: U32, +y: U32) -> Out:
  match on:
    case True{}:
      +label0 = MSG.T.composer.attach(Ty.loc(ty))
      Out.cat(Lay.field.x(ty, D.Clip.all(), x0, y, (x1 - x0 - 12 - Lay.button.w(ty, label0) : U32), path, True{}, True{}, True{}, caret, mark, MSG.T.composer.attach.placeholder(Ty.loc(ty)), "attach-path"),
        Lay.button(ty, D.Clip.all(), (x1 - Lay.button.w(ty, label0) : U32), y, label0, T.Th.accent(), "attach-file", path))
    case False{}:
      Out.none()

# the composer with its attachments
def Lay.composer.a(+ty: Ty, +draft: String, +ls: List<&2, String>, +held: String, +caret: Nat, +mark: Nat, +focus: String, +blink: Bool, +x0: U32, +x1: U32, +h: U32, +send: String, +act: String, +models: String,
    +rs: List<&2, U.SkillRow>, +atts: List<&2, String>, +up: String, +path: String, pre: List<&2, IME.Run>) -> Out:
  +y = {(h - 120 - Comp.eh(List.length(&2, String, ls)) : U32) : U32}
  +fy = Comp.foot(h)
  +ml = Comp.models(ty, models, x0, x1, send)
  Out.all([
    Lay.composer(ty, draft, ls, held, caret, mark, String.eq(focus, "composer"), blink, x0, x1, h, send, act, ml, rs, pre),
    Lay.atts(ty, atts, up, (x0 + 58 + Ty.width(ty, 2n, ml) : U32), fy),
    Lay.ibutton(ty, D.Clip.all(), (Comp.send_x(ty, x1, send) - 34 : U32), fy, "paperclip", T.Th.soft(), "focus", "attach-path"),
    Lay.attach_field(ty, String.eq(focus, "attach-path"), path, caret, mark, x0, x1, (y - 36 : U32))])

# The project card (client.bend's Fab.*): the goal, Edit and Buy it on one
# row above the timeline; a click on the goal opens the whole intent under
# it (at most Fab.max() lines, wrapped, headings faint)
def Fab.max() -> Nat:
  16n

def Fab.wrap.k(blank: Bool, +ty: Ty, +l: String, +px: U32, rest: List<&2, String>) -> List<&2, String>:
  match blank:
    case True{}:
      rest
    case False{}:
      List.append(&2, String, Ty.wrap(ty, 2n, l, px), rest)

def Fab.wrap(ls: List<&2, String>, +ty: Ty, +px: U32) -> List<&2, String>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      Fab.wrap.k(String.is_empty(String.trim(l)), ty, l, px, Fab.wrap(t, ty, px))

def Fab.lines(+ty: Ty, +intent: String, +px: U32) -> List<&2, String>:
  List.take(&2, String, Fab.wrap(String.lines(intent), ty, px), Fab.max())

# the card's height, the gap under it included (0: no card)
def Fab.h(shown: Bool, open: Bool, +ty: Ty, +intent: String, +px: U32) -> U32:
  match shown open:
    case False{} _:
      0
    case True{} False{}:
      44
    case True{} True{}:
      (44 + U32.from_nat(List.length(&2, String, Fab.lines(ty, intent, px))) * 20 + 10 : U32)

def Fab.rows(ls: List<&2, String>, +ty: Ty, +x: U32, +y: U32) -> List<&2, R.Op>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      List.append(&2, R.Op, Ty.text(ty, 2n, l, D.Clip.all(), x, y, 20, Bool.pick(U32, String.starts_with(l, "#"), T.Th.faint(), T.Th.text())),
        Fab.rows(t, ty, x, (y + 20 : U32)))

def Fab.buy.w(+ty: Ty, +buy: String) -> U32:
  Bool.pick(U32, String.is_empty(buy), 0, (Lay.button.w(ty, buy) + 6 : U32))

# Buy it is the card's one filled button: the accent behind dark text
def Fab.buy.btn.k(none: Bool, +ty: Ty, +buy: String, +x: U32, +y: U32) -> Out:
  match none:
    case True{}:
      Out.none()
    case False{}:
      Out{List.append(&2, R.Op, Lay.box(D.Clip.all(), x, y, Lay.button.w(ty, buy), 28, 6, T.Th.accent(), T.Th.accent()),
          Ty.text(ty, 2n, buy, D.Clip.all(), (x + 10 : U32), y, 28, T.Th.bg())),
        [Lay.hit(x, y, Lay.button.w(ty, buy), 28, "buy", "")]}

def Fab.buy.btn(+ty: Ty, +buy: String, +x: U32, +y: U32) -> Out:
  Fab.buy.btn.k(String.is_empty(buy), ty, buy, x, y)

def Lay.fab.body(open: Bool, +ty: Ty, +intent: String, +x0: U32, +x1: U32, +y: U32) -> Out:
  match open:
    case True{}:
      Out.ops(List.append(&2, R.Op, [D.Draw.rect(D.Clip.all(), (x0 + 1 : U32), (y + 35 : U32), (x1 - x0 - 2 : U32), 1, T.Th.line(), 255)],
        Fab.rows(Fab.lines(ty, intent, (x1 - x0 - 24 : U32)), ty, (x0 + 12 : U32), (y + 40 : U32))))
    case False{}:
      Out.none()

def Lay.fab(shown: Bool, +open: Bool, +ty: Ty, +line: String, +intent: String, +buy: String, +x0: U32, +x1: U32, +y: U32) -> Out:
  match shown:
    case True{}:
      +h = Fab.h(True{}, open, ty, intent, (x1 - x0 - 24 : U32))
      +bw = Fab.buy.w(ty, buy)
      +ew = Lay.button.w(ty, "Edit")
      +gx = {(x1 - 4 - bw - ew - 10 : U32) : U32}
      Out.all([Out.ops(Lay.box(D.Clip.all(), x0, y, (x1 - x0 : U32), (h - 8 : U32), 0, T.Th.raise(), T.Th.line())),
        Out.ops(Ty.text(ty, 2n, Ty.fit(ty, 2n, line, (gx - x0 - 12 : U32)), D.Clip.all(), (x0 + 12 : U32), (y + 4 : U32), 28, T.Th.text())),
        Out{Nil{}, [Lay.hit(x0, y, (gx - x0 : U32), 36, "fab-open", "")]},
        Lay.button(ty, D.Clip.all(), (x1 - 4 - bw - ew : U32), (y + 4 : U32), "Edit", T.Th.text(), "fab-edit", ""),
        Fab.buy.btn(ty, buy, (x1 - bw + 2 : U32), (y + 4 : U32)),
        Lay.fab.body(open, ty, intent, x0, x1, y)])
    case False{}:
      Out.none()

# the card's height and drawing from the client's info (fab.shown:,
# fab.open:, fab.intent:, fab.line: for the thread)
def Fab.h.of(+ty: Ty, +pics: Map<&2, String>, +sel: String, +cw: U32) -> U32:
  Fab.h(String.eq(U.Str.get(pics, "fab.shown:" ++ sel), "1"), String.eq(U.Str.get(pics, "fab.open:" ++ sel), "1"), ty, U.Str.get(pics, "fab.intent:" ++ sel), (cw - 24 : U32))

def Lay.fab.of(+ty: Ty, +pics: Map<&2, String>, +sel: String, +x0: U32, +x1: U32, +y: U32) -> Out:
  Lay.fab(String.eq(U.Str.get(pics, "fab.shown:" ++ sel), "1"), String.eq(U.Str.get(pics, "fab.open:" ++ sel), "1"), ty,
    U.Str.get(pics, "fab.line:" ++ sel), U.Str.get(pics, "fab.intent:" ++ sel), "", x0, x1, y)

# Ready to order (client.bend's Fab.ready): the parts to order and Buy it in
# a bar right above the composer, once the agent's turn is over
def Ready.h(+line: String) -> U32:
  Bool.pick(U32, String.is_empty(line), 0, 44)

def Ready.line(+st: M.State, +pics: Map<&2, String>, +sel: String, run: Bool) -> String:
  U.Fab.ready.shown.k(U.Fab.min.of(st, sel), U.Fab.ready.k(U.Fab.buy.of(st, pics, sel), U.Str.get(pics, "fab.ready:" ++ sel), run))

def Lay.ready.k(none: Bool, +ty: Ty, +line: String, +buy: String, +x0: U32, +x1: U32, +y: U32) -> Out:
  match none:
    case True{}:
      Out.none()
    case False{}:
      +bw = Fab.buy.w(ty, buy)
      # its ×: put away (client.bend's Fab.min), Buy it stays in the header
      +xw = Lay.button.w(ty, "×")
      Out.all([Out.ops(Lay.box(D.Clip.all(), x0, y, (x1 - x0 : U32), 36, 0, T.Th.raise(), T.Th.accent())),
        Out.ops(Ty.text(ty, 2n, Ty.fit(ty, 2n, line, (x1 - x0 - bw - xw - 28 : U32)), D.Clip.all(), (x0 + 12 : U32), (y + 4 : U32), 28, T.Th.text())),
        Fab.buy.btn(ty, buy, (x1 - bw - xw - 2 : U32), (y + 4 : U32)),
        Lay.button(ty, D.Clip.all(), (x1 - xw - 4 : U32), (y + 4 : U32), "×", T.Th.dim(), "fab-hide", "")])

def Lay.ready(+ty: Ty, +line: String, +buy: String, +x0: U32, +x1: U32, +y: U32) -> Out:
  Lay.ready.k(String.is_empty(line), ty, line, buy, x0, x1, y)

# the page's first entry ("" for none)
def TL.memo.first(m: TLMemo) -> String:
  match m:
    case TLMemo{_, _, its, _}:
      TL.align.first(its)

# Away from the end with something new below it (client.bend's
# Hold.pill): "3 new ↓" over the timeline's foot, a click back down
# ("vw-bottom", nui.bend's Hold.bottom). Square, in the panel's colours.
def TL.pill(none: Bool, +ty: Ty, +label: String, +x0: U32, +x1: U32, +bot: U32) -> Out:
  match none:
    case True{}:
      Out.none()
    case False{}:
      +w = {(Ty.width(ty, 2n, label) + 24 : U32) : U32}
      +x = {(x0 + U32.shr(U32.dim((x1 - x0 : U32), w)) : U32) : U32}
      +y = {(bot - 40 : U32) : U32}
      Out{List.append(&2, R.Op, Lay.box(D.Clip.all(), x, y, w, 28, 0, T.Th.raise(), T.Th.accent()),
          Ty.text(ty, 2n, label, D.Clip.all(), (x + 12 : U32), y, 28, T.Th.text())),
        [Lay.hit(x, y, w, 28, "vw-bottom", "")]}

def Lay.thread.k(+ty: Ty, +memo: TLMemo, t: M.Thread, +draft: String, +held: String, +st: M.State, +sel: String, +live: String, +caret: Nat, +mark: Nat, +focus: String, +blink: Bool,
    +x0: U32, +x1: U32, +h: U32, +sc: U32, pa: Bool, term: Bool, +info: String, +picking: Bool, +pend: List<&2, String>, +rs: List<&2, U.SkillRow>, +now: Nat,
    +atts: List<&2, String>, +up: String, +path: String, +pics: Map<&2, String>, del: Bool, +tsel: TSel, +moff: String, +ph: M.Phase, +hy: U32, more: Bool, +bq: String, +ba: String, +ren: String, +rtext: String, +ec: U.Ec, +fkey: String, pre: List<&2, IME.Run>, +hold: String, +pill: String) -> Out:
  match t:
    case M.Thread{id, p, ti, c, a, +turn, pe, ar, o, sa, ua, pi, en, br, wt, +pr, +mo, rm, im, se, de}:
      +cw = {U32.min((x1 - x0 - 64 : U32), 780) : U32}
      +cx0 = {(x0 + U32.shr((x1 - x0 - cw : U32)) : U32) : U32}
      +cx1 = {(cx0 + cw : U32) : U32}
      +kn = U.KN.thread(pics, st, sel)
      +kx = U.KX.note(pics, sel)
      +kxh = Kn.h(kx, ty, cw)
      +kh = (Kn.h(kn, ty, cw) + kxh : U32)
      +ts = SB.Subs.rows(st, pics, sel, now)
      +sopen = SB.Subs.panel.open(pics, sel, ts)
      +soff = Pane.clamp(Pane.off(pics, "subs:" ++ sel), List.length(&2, SB.SubRow, SB.Subs.panel.rows(sopen, ts)))
      +svis = Subs.vis(ts, sopen, soff, h)
      # the row under the header: the parent's link
      +pl = {Bool.pick(U32, String.is_empty(U.Tasks.up(st, sel)), 0, 32) : U32}
      +pins = U.Ec.pins(ec, st, sel)
      # the project card (Lay.fab) under the parent's link: its height and
      # drawing come from helpers (no binders here: Lay.thread.k is at Bend's
      # arity limit for the closure the timeline's split makes)
      +top = {(52 + hy + kh + pl + Fab.h.of(ty, pics, sel, cw) + Subs.height(ts, svis, sopen) + Pins.height(pins) : U32) : U32}
      +asks = U.Asks.open(st, sel)
      +ah = Asks.h(ty, asks, pics, (cw - 32 : U32))
      +qs = M.Queue.context(M.Queue.items(st, sel))
      +tds = U.Todos.shown(st, sel, Lay.running(turn))
      +tdo = U.Todos.open(pics, sel)
      +tda = U.Todos.all(pics, sel)
      +tdall = U.Todos.rows(pics, sel, U.Todos.cap.all(), tds)
      +tdoff = Pane.clamp(Pane.off(pics, "todos:" ++ sel), List.length(&2, TD.TRow, tdall))
      +tdr = List.take(&2, TD.TRow, List.drop(&2, TD.TRow, tdall, tdoff), Lay.tdcap(h))
      +tdm = {Bool.pick(String, tdo, U.Todos.more(Ty.loc(ty), tda, tds), "") : String}
      +qh = {(Lay.qh(qs) + Lay.tdh(tds, tdr, tdm) : U32) : U32}
      # the composer grows by eh, and all above it moves up
      +ls = Comp.wrap(ty, draft, cw)
      # (and by the Ready to order bar's height when it shows: it sits right above)
      +eh = {(Comp.eh(List.length(&2, String, ls)) + Ready.h(Ready.line(st, pics, sel, Lay.running(turn))) : U32) : U32}
      +stop = U.Follow.stop(Lay.running(turn), draft, atts)
      +bh = Btw.h(ty, bq, ba, cw)
      # the timeline and what sits around it are laid out at once
      tl ch = Lay.timeline(ty,
        List.concat(&2, TLine, [TL.memo.shown.loc(ty, memo, st, sel, cw, pics), TL.sending(ty, pend, cw),
          TL.live.m(ty, String.is_empty(live), ph, live, TL.memo.lv(memo), cw)]),
        cx0, cx1, top, (h - ah - qh - eh - bh : U32), sc, tsel, hold, TL.memo.first(memo), Out.none()) Out.all([Lay.header(ty, M.Thread{id, p, ti, c, a, turn, pe, ar, o, sa, ua, pi, en, br, wt, pr, mo, rm, im, se, de}, st, UZ.Chip.of(st, sel, now), U.Fab.mini.tools.of(st, pics, sel, Lay.running(turn)), x0, x1, pa, term, M.Snooze.on(now, U.Side.until(st, sel)), del, hy),
          Kn.draw(kx, False{}, ty, cx0, (52 + hy : U32), cw),
          Kn.draw(kn, True{}, ty, cx0, (52 + hy + kxh : U32), cw),
          Ren.field.if(String.eq(ren, sel), ty, rtext, focus, blink, caret, mark, x0, x1, hy),
          Lay.parent(ty, st, sel, cx0, (58 + hy + kh : U32)),
          Lay.fab.of(ty, pics, sel, cx0, cx1, (60 + hy + kh + pl : U32)),
          Lay.subs(ty, ts, svis, sopen, sel, soff, cx0, cx1, (60 + hy + kh + pl + Fab.h.of(ty, pics, sel, cw) : U32)),
          Lay.pins(ty, ec, pins, cx0, cx1, (60 + hy + kh + pl + Fab.h.of(ty, pics, sel, cw) + Subs.height(ts, svis, sopen) : U32), fkey, blink),
          Lay.composer.a(ty, draft, ls, held, caret, mark, focus, blink, cx0, cx1, h, U.Follow.button(Ty.loc(ty), st, sel, Lay.running(turn), stop, draft), U.Follow.act(stop), Comp.chip(ty, U.Effort.label(U.Models.label(pr, mo), U.Effort.of(st, sel)), U.Usage.tail(st, sel), cx0, cx1, U.Follow.button(Ty.loc(ty), st, sel, Lay.running(turn), stop, draft)), rs, atts, up, path, pre),
          Lay.ready(ty, Ready.line(st, pics, sel, Lay.running(turn)), U.Fab.ready.btn.of(st, pics, sel), cx0, cx1, (h - eh - 124 + 2 : U32)),
          Lay.queued(ty, qs, Lay.running(turn), cx0, cx1, (h - eh : U32)),
          Lay.todos(ty, tds, tdr, tdm, tdo, tda, sel, tdoff, List.length(&2, TD.TRow, tdall), cx0, cx1, (h - eh - 124 - Lay.qh(qs) : U32)),
          Lay.asks(ty, asks, draft, pics, cx0, cx1, (h - 132 - ah - qh - eh - bh + 4 : U32)),
          Btw.card.if(Bool.not(String.is_empty(bq)), ty, bq, ba, cx0, cx1, (h - 132 - qh - eh - bh + 4 : U32)),
          Lay.picker(picking, ty, U.Models.menu(info, pr), mo, info, pr, cx0, cx1, (h - 128 - eh : U32), (60 + hy : U32), U.Chip.all(info, pr, mo), U.Effort.of(st, sel), moff)])
      Out.cat(Out.cat(tl, TL.pill(String.is_empty(pill), ty, pill, cx0, cx1, (h - ah - qh - eh - bh - 132 : U32))), ch)

# while files dragged in hover over the window, an accent ring around the
# composer (placed as Lay.thread.k places it) shows where they drop
def Lay.drop_mark(on: Bool, +ty: Ty, +draft: String, +x0: U32, +x1: U32, +h: U32) -> Out:
  match on:
    case True{}:
      +cw = {U32.min((x1 - x0 - 64 : U32), 780) : U32}
      +cx0 = {(x0 + U32.shr((x1 - x0 - cw : U32)) - 5 : U32) : U32}
      +eh = Comp.eh(List.length(&2, String, Comp.wrap(ty, draft, cw)))
      +y = {(h - 125 - eh : U32) : U32}
      Out.ops([D.Draw.rect(D.Clip.all(), cx0, y, (cw + 10 : U32), 2, T.Th.accent(), 255),
        D.Draw.rect(D.Clip.all(), cx0, (y + 108 + eh : U32), (cw + 10 : U32), 2, T.Th.accent(), 255),
        D.Draw.rect(D.Clip.all(), cx0, y, 2, (110 + eh : U32), T.Th.accent(), 255),
        D.Draw.rect(D.Clip.all(), (cx0 + cw + 8 : U32), y, 2, (110 + eh : U32), T.Th.accent(), 255)])
    case False{}:
      Out.none()

# while the page is full, a hit (no area) says earlier entries exist:
# the wheel near the timeline's top shows them (nui.bend's Tl.up)
def Lay.earlier(more: Bool) -> Out:
  match more:
    case True{}:
      Out{Nil{}, [Lay.hit(0, 0, 0, 0, "tl-more", "")]}
    case False{}:
      Out.none()

def Lay.viewer.if(open: Bool, +ty: Ty, +n: S.Nui, +x0: U32, +w: U32, +h: U32) -> Out:
  match open:
    case True{}:
      Lay.viewer(ty, n, x0, w, h)
    case False{}:
      Out.none()

def Lay.palette.if(open: Bool, +ty: Ty, +n: S.Nui, +w: U32, +h: U32) -> Out:
  match open:
    case True{}:
      Lay.palette(ty, S.Nui.ui(n), w, h, S.Blink.on(n))
    case False{}:
      Out.none()

def Lay.proj.if(open: Bool, +ty: Ty, +n: S.Nui, +w: U32, +h: U32) -> Out:
  match open:
    case True{}:
      Lay.proj(ty, S.Nui.ui(n), w, h, S.Blink.on(n))
    case False{}:
      Out.none()

# the main area's right edge (half the window with the viewer open)
def Lay.x1(+n: S.Nui) -> U32:
  Bool.pick(U32, S.Nui.panel(n), (264 + U32.shr((S.Nui.width(n) - 264 : U32)) : U32), S.Nui.width(n))

# The performance overlay (ctrl+shift+h, nui.bend's Hud): its text in the
# code font, top right, over everything; no hit regions
def Lay.perf.lines(ls: List<&2, String>, +ty: Ty, +x: U32, +y: U32) -> List<&2, R.Op>:
  match ls:
    case Nil{}:
      Nil{}
    case l <> t:
      List.append(&2, R.Op, Ty.text(ty, 4n, l, D.Clip.all(), x, y, 16, T.Th.text()), Lay.perf.lines(t, ty, x, (y + 16 : U32)))

def Lay.perf.wide(ls: List<&2, String>, +ty: Ty, +m: U32) -> U32:
  match ls:
    case Nil{}:
      m
    case l <> t:
      Lay.perf.wide(t, ty, U32.max(m, Ty.width(ty, 4n, l)))

def Lay.perf.box(+ty: Ty, +ls: List<&2, String>, +w: U32) -> Out:
  +bw = (Lay.perf.wide(ls, ty, 0) + 20 : U32)
  +x = U32.dim((w - 16 : U32), bw)
  Out{List.append(&2, R.Op, Lay.box(D.Clip.all(), x, 60, bw, (U32.from_nat(List.length(&2, String, ls)) * 16 + 16 : U32), 0, T.Th.raise(), T.Th.line()),
    Lay.perf.lines(ls, ty, (x + 10 : U32), 68)), Nil{}}

def Lay.perf.if(on: Bool, +ty: Ty, +text: String, +w: U32) -> Out:
  match on:
    case True{}:
      Lay.perf.box(ty, String.lines(text), w)
    case False{}:
      Out.none()

# the timeline's width, as Lay.frame lays it out
def Lay.tl.px(+n: S.Nui) -> U32:
  +w = S.Nui.width(n)
  +x1 = Bool.pick(U32, S.Nui.panel(n), (264 + U32.shr((w - 264 : U32)) : U32), w)
  U32.min((x1 - 264 - 64 : U32), 780)

def TL.created(t: M.Thread) -> Nat:
  match t:
    case M.Thread{_, _, _, c, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      c

# A page held while read (client.bend's Hold): scrolled up, the page keeps
# its first entry ("@tl.pin.<thread>", nui.bend's Hold.take) instead of
# sliding as entries come in, so nothing leaves above what is read; it
# grows by at most TL.held.room() entries that way. Back at the end the pin
# goes and the page is the newest again.
type NH is Data:
  NH{left: Nat, room: Nat, seen: Bool, acc: List<&2, M.Entry>}

def TL.held.room() -> Nat:
  240n

def TL.held.step(own: Bool, +e: M.Entry, +pin: String, st: NH) -> NH:
  match own st:
    case True{} NH{_, 0n, s, acc}:
      NH{0n, 0n, s, acc}
    case True{} NH{0n, 1n+r, s, acc}:
      NH{0n, r, Bool.or(s, String.eq(TL.eid(e), pin)), e <> acc}
    case True{} NH{1n+l, 1n+r, s, acc}:
      NH{l, r, Bool.or(s, String.eq(TL.eid(e), pin)), e <> acc}
    case False{} st2:
      st2

def TL.held.cut(old: Bool, st: NH) -> NH:
  match old st:
    case True{} NH{_, _, _, acc}:
      NH{0n, 0n, True{}, acc}
    case False{} _:
      st

# the newest `left` of a thread's entries and on back to the pinned one
# (TL.newest's walk otherwise)
def TL.held(es: List<&2, M.Entry>, +th: String, +since: Nat, +pin: String, st: NH) -> List<&2, M.Entry>:
  match es st:
    case +_ NH{0n, _, True{}, acc}:
      acc
    case +_ NH{_, 0n, _, acc}:
      acc
    case Nil{} NH{_, _, _, acc}:
      acc
    case +e <> t NH{l, r, s, acc}:
      TL.held(t, th, since, pin, TL.held.cut(Nat.is_lt(Nat.add(TL.at_of(e), 3600n), since), TL.held.step(TL.mine.or(String.eq(TL.thread_of(e), th), e, th), e, pin, NH{l, r, s, acc})))

def TL.held.or(free: Bool, +es: List<&2, M.Entry>, +th: String, +since: Nat, +pin: String, +left: Nat) -> List<&2, M.Entry>:
  match free:
    case True{}:
      TL.newest(es, th, since, NW{left, Nil{}})
    case False{}:
      TL.held(es, th, since, pin, NH{left, Nat.add(left, TL.held.room()), False{}, Nil{}})

# the open thread's entries (none with no thread open)
def Lay.memo.es(none: Bool, +n: S.Nui) -> List<&2, M.Entry>:
  match none:
    case True{}:
      Nil{}
    case False{}:
      TL.held.or(String.is_empty(U.Ui.scratch(S.Nui.ui(n), "@tl.pin." ++ U.Ui.sel(S.Nui.ui(n)))), M.State.mine(U.Ui.state(S.Nui.ui(n)), U.Ui.sel(S.Nui.ui(n))), U.Ui.sel(S.Nui.ui(n)), TL.created(U.Ui.thread(U.Ui.state(S.Nui.ui(n)), U.Ui.sel(S.Nui.ui(n)))),
        U.Ui.scratch(S.Nui.ui(n), "@tl.pin." ++ U.Ui.sel(S.Nui.ui(n))), U.Hold.page(M.State.mine(U.Ui.state(S.Nui.ui(n)), U.Ui.sel(S.Nui.ui(n))), U.Ui.scratch(S.Nui.ui(n), "@away." ++ U.Ui.sel(S.Nui.ui(n))), U.Page.shown(S.Nui.ui(n), U.Ui.sel(S.Nui.ui(n)))))

# Both the entries and live prefix are discarded when the locale changes.
# Checking here also covers memos of threads left before a language switch.
def TL.memo.locale.k(same: Bool, m: TLMemo) -> TLMemo:
  match same:
    case True{}:
      m
    case False{}:
      TL.memo.none()

def TL.memo.locale(+tag: String, +m: TLMemo) -> TLMemo:
  TL.memo.locale.k(String.eq(tag, TL.live.tag(TL.memo.lv(m))), m)

# the memo brought up to this frame: the open thread's entries at its width
# (only the newest page of a thread, core's Page: a long thread laid out
# whole cost every frame 15 ms on a real log; "earlier" shows another page)
def Lay.memo.local(+tag: String, +ty: Ty, +n: S.Nui, +prev: TLMemo) -> TLMemo:
  TL.memo.with_live(TL.memo.of(ty, Lay.memo.es(String.is_empty(U.Ui.sel(S.Nui.ui(n))), n), Lay.tl.px(n), U.Ui.sel(S.Nui.ui(n)), prev),
    TL.live.local(tag, TL.live.get(ty, TL.memo.lv(prev), U.Live.head(U.Ui.live.of(S.Nui.ui(n), U.Ui.sel(S.Nui.ui(n)))), Lay.tl.px(n))))

def Lay.memo(+ty: Ty, +n: S.Nui, prev: TLMemo) -> TLMemo:
  Lay.memo.local(TL.locale(S.Nui.ui(n)), ty, n, TL.memo.locale(TL.locale(S.Nui.ui(n)), prev))

# the memos brought up to this frame: the open thread's in front, from the
# one kept for it
def Lay.memos(+ty: Ty, +n: S.Nui, +ms: List<&2, TLMemo>) -> List<&2, TLMemo>:
  TL.memos.keep(Lay.memo(ty, n, TL.memos.pick(ms, U.Ui.sel(S.Nui.ui(n)))), ms)

def TL.memo.size(m: TLMemo) -> Nat:
  match m:
    case TLMemo{_, _, its, _}:
      List.length(&2, TLE, its)

# Voice
# -----
# Dictation (client.bend's Voice): the mic button beside the paperclip,
# red while it records; above the composer, what is being heard (the
# microphone, its level, the words so far), a note when nothing comes in
# (the helper's hints) or something failed, and the words to clarify, one
# at a time. Placed as Lay.thread.k places the composer.

def Vx.send(+l: I.Loc, t: M.Thread, +st: M.State, +sel: String, +draft: String, +atts: List<&2, String>) -> String:
  match t:
    case M.Thread{_, _, _, _, _, +turn, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      U.Follow.button(l, st, sel, Lay.running(turn), U.Follow.stop(Lay.running(turn), draft, atts), draft)

def Vx.mic(+ty: Ty, +state: String, +x: U32, +y: U32) -> Out:
  Out{List.append(&2, R.Op,
      Lay.box(D.Clip.all(), x, y, 28, 28, 6, Bool.pick(U32, String.eq(state, "live"), T.Th.bad(), T.Th.raise()),
        Bool.pick(U32, String.is_empty(state), T.Th.line(), T.Th.bad())),
      Lay.icon(ty, "mic", D.Clip.all(), (x + 6 : U32), (y + 6 : U32), Bool.pick(U32, String.eq(state, "live"), T.Th.bg(), Bool.pick(U32, String.is_empty(state), T.Th.soft(), T.Th.bad())))),
    [Lay.hit(x, y, 28, 28, "voice", "")]}

# the level meter: 0..100 over w pixels
def Vx.meter(+lv: String, +x: U32, +y: U32, +w: U32) -> List<&2, R.Op>:
  +v = {U32.min(Maybe.default(&2, U32, U32.read(lv), 0), 100) : U32}
  [D.Draw.rect(D.Clip.all(), x, y, w, 6, T.Th.raise(), 255), D.Draw.rect(D.Clip.all(), x, y, ((w * v : U32) / 100 : U32), 6, T.Th.ok(), 255)]

def Vx.head.loc(+l: I.Loc, +state: String, +dev: String) -> String:
  Bool.pick(String, String.eq(state, "starting"), MSG.T.voice.starting(l),
    Bool.pick(String, String.eq(state, "stopping"), MSG.T.voice.finishing(l), MSG.T.voice.listening(l, Bool.pick(String, String.is_empty(dev), "none", "named"), dev)))

# what is being heard: a red dot, the microphone, the level, the words so
# far, and Stop

def Vx.head(+state: String, +dev: String) -> String:
  Vx.head.loc(I.En{}, state, dev)

def Vx.strip(+ty: Ty, +u: U.Ui, +x0: U32, +x1: U32, +y: U32) -> Out:
  +label0 = MSG.T.composer.stop(Ty.loc(ty))
  +head = Vx.head.loc(Ty.loc(ty), U.Ui.scratch(u, "@voice"), U.Ui.scratch(u, "@voice.dev"))
  +hw = {Ty.width(ty, 2n, Ty.fit(ty, 2n, head, U32.shr((x1 - x0 : U32)))) : U32}
  +mx = {(x0 + 34 + hw : U32) : U32}
  +bx = {(x1 - 8 - Lay.button.w(ty, label0) : U32) : U32}
  Out.all([
    Out.ops(List.concat(&2, R.Op, [Lay.box(D.Clip.all(), x0, y, (x1 - x0 : U32), 36, 6, T.Th.panel(), T.Th.bad()),
      [D.Draw.rect(D.Clip.all(), (x0 + 12 : U32), (y + 13 : U32), 10, 10, T.Th.bad(), 255)],
      Ty.text(ty, 2n, Ty.fit(ty, 2n, head, U32.shr((x1 - x0 : U32))), D.Clip.all(), (x0 + 30 : U32), (y + 4 : U32), 28, T.Th.text()),
      Vx.meter(U.Ui.scratch(u, "@voice.lv"), (mx + 8 : U32), (y + 15 : U32), 64),
      Ty.text(ty, 2n, Ty.fit(ty, 2n, U.Ui.scratch(u, "@voice.part"), U32.dim(bx, (mx + 90 : U32))), D.Clip.all(), (mx + 84 : U32), (y + 4 : U32), 28, T.Th.faint())])),
    Lay.button(ty, D.Clip.all(), bx, (y + 4 : U32), label0, T.Th.bad(), "voice", "")])

def Vx.hint.loc(+l: I.Loc, +h: String, +dev: String) -> String:
  +d = Bool.pick(String, String.is_empty(dev), MSG.T.voice.microphone(l), dev)
  Bool.pick(String, String.eq(h, "no-audio"), MSG.T.voice.noaudio(l, d),
    Bool.pick(String, String.eq(h, "silent"), MSG.T.voice.silent(l, d), h))

# a note above the strip: why nothing comes in, or what failed (× closes)

def Vx.hint(+h: String, +dev: String) -> String:
  Vx.hint.loc(I.En{}, h, dev)

def Vx.note(+ty: Ty, +text: String, +color: U32, +x0: U32, +x1: U32, +y: U32) -> Out:
  Out.all([
    Out.ops(List.append(&2, R.Op, Lay.box(D.Clip.all(), x0, y, (x1 - x0 : U32), 36, 6, T.Th.raise(), color),
      Ty.text(ty, 2n, Ty.fit(ty, 2n, text, (x1 - x0 - 52 : U32)), D.Clip.all(), (x0 + 12 : U32), (y + 4 : U32), 28, T.Th.text()))),
    Lay.ibutton(ty, D.Clip.all(), (x1 - 32 : U32), (y + 4 : U32), "x", T.Th.dim(), "voice-err", "")])

# buttons in a row from x, each 6 px after the last
def Vx.row(bs: List<&2, VBtn>, +ty: Ty, +x: U32, +y: U32) -> Out:
  match bs:
    case Nil{}:
      Out.none()
    case VBtn{+label, +action, value} <> t:
      Out.cat(Lay.button(ty, D.Clip.all(), x, y, label, Bool.pick(U32, String.eq(action, "voice-use"), T.Th.accent(), T.Th.soft()), action, value),
        Vx.row(t, ty, (x + Lay.button.w(ty, label) + 6 : U32), y))

def Vx.ask.btns.loc(+l: I.Loc, +w: String, +g: String) -> List<&2, VBtn>:
  List.append(&2, VBtn, Bool.pick(List<&2, VBtn>, String.is_empty(g), Nil{}, [VBtn{MSG.T.voice.choose(l, g), "voice-use", "0"}]),
    [VBtn{MSG.T.voice.fix(l), "voice-fix", "0"}, VBtn{MSG.T.voice.dictionary.add(l), "voice-add", "0"}, VBtn{MSG.T.tour.skip(l), "voice-skip", "0"}])

# the first word to clarify: heard as, the guess, and what to do with it

def Vx.ask.btns(+w: String, +g: String) -> List<&2, VBtn>:
  Vx.ask.btns.loc(I.En{}, w, g)

def Vx.ask.k(x: VC.Unsure, +ty: Ty, +n: Nat, +x0: U32, +x1: U32, +y: U32) -> Out:
  match x:
    case VC.Unsure{+w, +g}:
      +q = MSG.T.voice.heard(Ty.loc(ty), w, Bool.pick(String, Nat.is_gt(n, 1n), "many", "one"), Nat.show(n))
      Out.all([
        Out.ops(List.append(&2, R.Op, Lay.box(D.Clip.all(), x0, y, (x1 - x0 : U32), 36, 6, T.Th.panel(), T.Th.warn()),
          Ty.text(ty, 2n, Ty.fit(ty, 2n, q, U32.shr((x1 - x0 : U32))), D.Clip.all(), (x0 + 12 : U32), (y + 4 : U32), 28, T.Th.text()))),
        Vx.row(Vx.ask.btns.loc(Ty.loc(ty), w, g), ty, (x0 + 24 + Ty.width(ty, 2n, Ty.fit(ty, 2n, q, U32.shr((x1 - x0 : U32)))) : U32), (y + 4 : U32))])

# the right spelling being typed (Enter or Save replaces the word and adds
# it to the dictionary)
def Vx.fixing(+ty: Ty, +n: S.Nui, +x0: U32, +x1: U32, +y: U32) -> Out:
  +label0 = MSG.T.common.save(Ty.loc(ty))
  +label1 = MSG.T.common.cancel(Ty.loc(ty))
  +id = S.Fld.id("vfix", "voice-fixed", "")
  +sw = Lay.button.w(ty, label0)
  +cw = Lay.button.w(ty, label1)
  Out.all([
    Out.ops(Lay.box(D.Clip.all(), x0, y, (x1 - x0 : U32), 36, 6, T.Th.panel(), T.Th.warn())),
    Lay.field(ty, D.Clip.all(), (x0 + 4 : U32), (y + 4 : U32), (x1 - x0 - 20 - sw - cw : U32), U.Ui.scratch(S.Nui.ui(n), "@f.vfix"), String.eq(S.Nui.focus_of(n), id), S.Blink.on(n),
      S.Nui.caret_of(n), S.Nui.mark(n), MSG.T.voice.spelling(Ty.loc(ty)), id),
    Lay.button(ty, D.Clip.all(), (x1 - 8 - sw - 6 - cw : U32), (y + 4 : U32), label0, T.Th.accent(), "voice-fixed", ""),
    Lay.button(ty, D.Clip.all(), (x1 - 8 - cw : U32), (y + 4 : U32), label1, T.Th.soft(), "form-close", "@voice.fix")])

def Vx.ask(fixing: Bool, +ty: Ty, +n: S.Nui, +us: List<&2, VC.Unsure>, +x0: U32, +x1: U32, +y: U32) -> Out:
  match fixing:
    case True{}:
      Vx.fixing(ty, n, x0, x1, y)
    case False{}:
      Vx.ask.k(Maybe.default(&2, VC.Unsure, List.head(&2, VC.Unsure, us), VC.Unsure{"", ""}), ty, List.length(&2, VC.Unsure, us), x0, x1, y)

def Vx.ask.if(on: Bool, +ty: Ty, +n: S.Nui, +x0: U32, +x1: U32, +y: U32) -> Out:
  match on:
    case True{}:
      Vx.ask(Bool.not(String.is_empty(U.Ui.scratch(S.Nui.ui(n), "@voice.fix"))), ty, n, U.Voice.unsure(S.Nui.ui(n)), x0, x1, y)
    case False{}:
      Out.none()

def Vx.strip.if(on: Bool, +ty: Ty, +u: U.Ui, +x0: U32, +x1: U32, +y: U32) -> Out:
  match on:
    case True{}:
      Vx.strip(ty, u, x0, x1, y)
    case False{}:
      Out.none()

def Vx.note.if(on: Bool, +ty: Ty, +u: U.Ui, +x0: U32, +x1: U32, +y: U32) -> Out:
  match on:
    case True{}:
      Bool.pick(Out, String.is_empty(U.Ui.scratch(u, "@voice.err")),
        Vx.note(ty, Vx.hint.loc(Ty.loc(ty), U.Ui.scratch(u, "@voice.hint"), U.Ui.scratch(u, "@voice.dev")), T.Th.warn(), x0, x1, y),
        Vx.note(ty, MSG.T.voice.error(Ty.loc(ty), U.Ui.scratch(u, "@voice.err")), T.Th.bad(), x0, x1, y))
    case False{}:
      Out.none()

# the mic and what sits above the composer for dictation, stacked up from
# the composer's top: the words to clarify, the strip, the note
def Vx.view.k(+ty: Ty, +n: S.Nui, +u: U.Ui, +sel: String, +cx0: U32, +cx1: U32, +y: U32, +fy: U32, +send: String) -> Out:
  +here = Bool.or(String.eq(U.Ui.scratch(u, "@voice.th"), sel), String.is_empty(U.Ui.scratch(u, "@voice.th")))
  +asking = Bool.and(String.eq(U.Ui.scratch(u, "@voice.uth"), sel), Bool.not(List.is_empty(&2, VC.Unsure, U.Voice.unsure(u))))
  +strip = Bool.and(here, U.Voice.on(u))
  +noting = Bool.and(here, Bool.or(Bool.not(String.is_empty(U.Ui.scratch(u, "@voice.err"))), Bool.and(U.Voice.on(u), Bool.not(String.is_empty(U.Ui.scratch(u, "@voice.hint"))))))
  +ay = {(y - 42 : U32) : U32}
  +sy = {(ay - Bool.pick(U32, asking, 42, 0) : U32) : U32}
  +ny = {(sy - Bool.pick(U32, strip, 42, 0) : U32) : U32}
  Out.all([
    Vx.mic(ty, Bool.pick(String, here, U.Ui.scratch(u, "@voice"), ""), (Comp.send_x(ty, cx1, send) - 68 : U32), fy),
    Vx.ask.if(asking, ty, n, cx0, cx1, (ay + 2 : U32)),
    Vx.strip.if(strip, ty, u, cx0, cx1, (sy + 2 : U32)),
    Vx.note.if(noting, ty, u, cx0, cx1, (ny + 2 : U32))])

def Vx.view(+ty: Ty, +n: S.Nui, +st: M.State, +sel: String, +x0: U32, +x1: U32, +h: U32) -> Out:
  +cw = {U32.min((x1 - x0 - 64 : U32), 780) : U32}
  +cx0 = {(x0 + U32.shr((x1 - x0 - cw : U32)) : U32) : U32}
  +eh = Comp.eh(List.length(&2, String, Comp.wrap(ty, S.Imx.draft(n), cw)))
  Vx.view.k(ty, n, S.Nui.ui(n), sel, cx0, (cx0 + cw : U32), (h - 120 - eh : U32), Comp.foot(h), Vx.send(Ty.loc(ty), U.Ui.thread(st, sel), st, sel, U.Ui.draft_text(S.Nui.ui(n)), U.Ui.atts(S.Nui.ui(n), sel)))

# the page is full: a thread with more entries than one page shows
def Lay.more(+memo: TLMemo, +n: S.Nui, +sel: String) -> Bool:
  Nat.is_ge(TL.memo.size(memo), U.Page.shown(S.Nui.ui(n), sel))

def Lay.thread.at.k(+ty: Ty, +memo: TLMemo, +n: S.Nui, +x0: U32, +x1: U32, +hy: U32) -> Out:
  match n:
    case S.Nui{U.Ui{+st, +sel, _, +live, +op, _, _, _, pa, _, _, _, _, _, +nf, _, _}, _, +h, _, _, _, +sc, +caret, +focus, +path, _, _, _}:
      Out.cat(Out.cat(Out.cat(Lay.drop_mark(U.Flag.get(op, "drop"), ty, S.Imx.draft(n), x0, x1, h), Lay.thread.k(ty, memo, U.Ui.thread(st, sel), S.Imx.draft(n), S.Cs.held(n), st, sel, U.Str.get(live, sel), S.Imx.at(n), S.Imx.mk(n), focus, S.Blink.on(n), x0, x1, h, sc, pa, U.Flag.get(op, "term"),
        U.Str.get(nf, "models"), U.Flag.get(op, "models"), U.Ui.pending(S.Nui.ui(n), sel), U.Ui.skills(S.Nui.ui(n)), U.Side.now(S.Nui.ui(n)),
        U.Ui.atts(S.Nui.ui(n), sel), U.Ui.uploading(S.Nui.ui(n), sel), path, nf, U.Ui.deleting(S.Nui.ui(n)), TSel.of(U.Ui.scratch(S.Nui.ui(n), "@ts"), sel), U.Ui.scratch(S.Nui.ui(n), "@menu.off"), U.Side.phase(st, U.Ui.thread(st, sel)), hy, Lay.more(memo, n, sel), U.Btw.question(S.Nui.ui(n)), U.Btw.reply.of(S.Nui.ui(n)),
        U.Ren.on(S.Nui.ui(n)), U.Ui.scratch(S.Nui.ui(n), "@f.title"), U.Ec.of(S.Nui.ui(n), sel), EM.Emb.focused(focus), S.Imx.runs(n), S.Hold.of(n), U.Hold.pill(S.Nui.ui(n), sel))),
        Vx.view(ty, n, st, sel, x0, x1, h)), Lay.earlier(Lay.more(memo, n, sel)))

def Lay.thread.at(+ty: Ty, +memo: TLMemo, +n: S.Nui, +x0: U32, +x1: U32, +hy: U32) -> Out:
  Ew.over(ty, Lay.thread.at.k(ty, memo, n, x0, x1, hy), U.Ec.of(S.Nui.ui(n), U.Ui.sel(S.Nui.ui(n))), EM.Emb.focused(S.Nui.focus_of(n)), S.Blink.on(n))

# the thread view with its header at the top
def Lay.thread(+ty: Ty, +memo: TLMemo, +n: S.Nui, +x0: U32, +x1: U32) -> Out:
  Lay.thread.at(ty, memo, n, x0, x1, 0)

# only the screen shown is laid out (a Bool.pick would lay out all three)
def Lay.main.k(settings: Bool, empty: Bool, +ty: Ty, +memo: TLMemo, +n: S.Nui, +st: M.State, +nf: Map<&2, String>, +v: String, +note: String, +x0: U32, +x1: U32, +h: U32) -> Out:
  match settings empty:
    case True{} _:
      Lay.settings(ty, st, n, nf, v, note, x0, x1, h, Maybe.default(&2, U32, U32.read(U.Ui.scratch(S.Nui.ui(n), "@set.sc")), 0))
    case False{} True{}:
      Out.cat(Lay.empty(ty, x0, x1, h, List.length(&2, M.Project, U.Ui.projects(st))), Kn.draw(U.KN.visible(nf, st), True{}, ty, (x0 + 32 : U32), (U32.shr(h) + 30 : U32), (x1 - x0 - 64 : U32)))
    case False{} False{}:
      Lay.thread(ty, memo, n, x0, x1)

def Lay.main(+ty: Ty, +memo: TLMemo, +n: S.Nui, +x0: U32, +x1: U32) -> Out:
  match n:
    case S.Nui{U.Ui{+st, +sel, _, _, +op, _, _, _, _, +v, _, _, _, +note, +nf, _, _}, _, +h, _, _, _, _, _, _, _, _, _, _}:
      Lay.main.k(U.Flag.get(op, "settings"), String.is_empty(sel), ty, memo, n, st, nf, v, note, x0, x1, h)

# the main area: the one given (a bot's, a room's), or the usual one
def Lay.main.m(m: Maybe<&2, Out>, +ty: Ty, +memo: TLMemo, +n: S.Nui, +x0: U32, +x1: U32) -> Out:
  match m:
    case Some{o}:
      o
    case None{}:
      Lay.main(ty, memo, n, x0, x1)

# The first-run tour (core/onboard.bend, client.bend's Tour.*): the window
# dims except the part the step points at (outlined in the accent), and a
# card holds the step, Skip, Back and Next. Its hits are all "ob-" actions;
# nui.bend's Tour.event takes the keys.
def Tour.rect(+g: String, +h: U32, +x1: U32) -> D.Clip:
  Bool.pick(D.Clip, String.eq(g, "sidebar"), D.Clip{0, 0, 264, h},
    Bool.pick(D.Clip, String.eq(g, "add"), D.Clip{224, 8, 260, 44},
      Bool.pick(D.Clip, String.eq(g, "settings"), D.Clip{0, U32.dim(h, 48), 264, h},
        Bool.pick(D.Clip, String.eq(g, "head"), D.Clip{264, 0, U32.max(x1, 265), 52}, D.Clip{0, 0, 0, 0}))))

def Tour.dim(c: D.Clip, +ww: U32, +hh: U32) -> List<&2, R.Op>:
  match c:
    case D.Clip{+x0, +y0, +x1, +y1}:
      Bool.pick(List<&2, R.Op>, U32.is_eq(x1, 0), [D.Draw.rect(D.Clip.all(), 0, 0, ww, hh, 0, 140)],
        List.concat(&2, R.Op, [[D.Draw.rect(D.Clip.all(), 0, 0, ww, y0, 0, 140), D.Draw.rect(D.Clip.all(), 0, y1, ww, U32.dim(hh, y1), 0, 140),
          D.Draw.rect(D.Clip.all(), 0, y0, x0, (y1 - y0 : U32), 0, 140), D.Draw.rect(D.Clip.all(), x1, y0, U32.dim(ww, x1), (y1 - y0 : U32), 0, 140)],
          D.Draw.frame(D.Clip.all(), x0, y0, (x1 - x0 : U32), (y1 - y0 : U32), T.Th.accent()),
          D.Draw.frame(D.Clip.all(), (x0 + 1 : U32), (y0 + 1 : U32), (x1 - x0 - 2 : U32), (y1 - y0 - 2 : U32), T.Th.accent())]))

# beside the sidebar's parts, else in the middle
def Tour.x(+g: String, +ww: U32, +cw: U32) -> U32:
  Bool.pick(U32, Bool.or(Bool.or(String.eq(g, "sidebar"), String.eq(g, "add")), String.eq(g, "settings")),
    U32.min(288, U32.dim(ww, (cw + 12 : U32))), U32.shr(U32.dim(ww, cw)))

def Tour.lines(ls: List<&2, String>, +ty: Ty, +k: Nat, +lh: U32, +c: U32, +cl: D.Clip, +x: U32, +y: U32) -> List<&2, R.Op>:
  match ls:
    case Nil{}:
      Nil{}
    case l <> t:
      List.append(&2, R.Op, Ty.text(ty, k, l, cl, x, y, lh, c), Tour.lines(t, ty, k, lh, c, cl, x, (y + lh : U32)))

def Tour.back(on: Bool, +ty: Ty, +x: U32, +y: U32) -> Out:
  match on:
    case True{}:
      Lay.button(ty, D.Clip.all(), x, y, MSG.T.tour.back(Ty.loc(ty)), T.Th.soft(), "ob-back", "")
    case False{}:
      Out.none()

# The card never leaves the words out: the body scrolls (the wheel over the
# card, Tour.wheel in nui.bend; "@tour.sc" lines, kept to what overflows)
# inside an area as tall as the window leaves after the buttons, which are
# always on the card. Where not even two body lines fit above them (the
# window under about 250 px tall) the card is compact: the title joins the
# text, the step's number goes beside Skip, and the card is the buttons and
# the lines that fit; under 100 px it is no shorter, and what is below the
# window is cut by it (the keys still work).
def Tour.bar.k(over: Bool, +nl: U32, +nv: U32, +off: U32, +x: U32, +y: U32) -> Out:
  match over:
    case True{}:
      Out.ops(Lay.box(D.Clip.all(), x, (y + (off * nv * 22 / nl : U32) : U32), 3, (nv * nv * 22 / nl : U32), 0, T.Th.accent(), T.Th.accent()))
    case False{}:
      Out.none()

def Tour.bar(+nl: U32, +nv: U32, +off: U32, +x: U32, +y: U32) -> Out:
  Tour.bar.k(U32.is_gt(nl, nv), nl, nv, off, x, y)

# the uberbot step's choices (client.bend's Tour.choices): a button each,
# one above the other, their hits the choice's own action and value
def Tour.choices(cs: List<&2, U.TChoice>, +ty: Ty, +x: U32, +y: U32) -> Out:
  match cs:
    case Nil{}:
      Out.none()
    case U.TChoice{l, a, v} <> t:
      Out.cat(Lay.button(ty, D.Clip.all(), x, y, l, T.Th.accent(), a, v), Tour.choices(t, ty, x, (y + 34 : U32)))

def Lay.tour(+ty: Ty, +n: S.Nui, +ww: U32, +hh: U32) -> Out:
  +label0 = MSG.T.tour.skip(Ty.loc(ty))
  +u = S.Nui.ui(n)
  +cs = U.Tour.choices(u)
  +cr = {(U32.from_nat(List.length(&2, U.TChoice, cs)) * 34 : U32) : U32}
  +i = U.Tour.at(u)
  +g = U.Tour.target(0n, u)
  +body = U.Tour.body.loc(Ty.loc(ty), 0n, u)
  +title = OB.Ob.title(Ty.loc(ty), i)
  +cw0 = {U32.min(440, U32.dim(ww, 24)) : U32}
  +cw = {Bool.pick(U32, U32.is_gt((U32.from_nat(List.length(&2, String, Ty.wrap(ty, 0n, body, U32.dim(cw0, 32)))) * 22 + 130 : U32), hh), U32.min(680, U32.dim(ww, 24)), cw0) : U32}
  +cx = Tour.x(g, ww, cw)
  +tl = Ty.wrap(ty, 1n, title, U32.dim(cw, 32))
  +th = {(U32.from_nat(List.length(&2, String, tl)) * 28 : U32) : U32}
  +avail = {U32.dim(hh, 24) : U32}
  +compact = {U32.is_lt(avail, (142 + th : U32)) : Bool}
  +all = Ty.wrap(ty, 0n, Bool.pick(String, compact, title ++ ": " ++ body, body), U32.dim(cw, 32))
  +nl = {U32.from_nat(List.length(&2, String, all)) : U32}
  +top = {Bool.pick(U32, compact, 8, (38 + th : U32)) : U32}
  +nv = {U32.max(1, U32.min(nl, (U32.dim(avail, (top + 60 + cr : U32)) / 22 : U32))) : U32}
  +ch = {(top + 60 + nv * 22 + cr : U32) : U32}
  +cy = {U32.min(96, U32.dim(hh, (ch + 12 : U32))) : U32}
  +by = {(cy + ch - 44 : U32) : U32}
  +bt = {(cy + top : U32) : U32}
  +off = {U32.min(U32.from_nat(Maybe.default(&2, Nat, Nat.read(U.Ui.scratch(u, "@tour.sc")), 0n)), U32.dim(nl, nv)) : U32}
  +nb = OB.Ob.forward(Ty.loc(ty), i)
  +nx = {(cx + cw - 16 - Lay.button.w(ty, nb) : U32) : U32}
  Out.all([
    Out{Tour.dim(Tour.rect(g, hh, Lay.x1(n)), ww, hh), [Lay.hit(0, 0, ww, hh, "", "")]},
    Out{Lay.box(D.Clip.all(), cx, cy, cw, ch, 0, T.Th.panel(), T.Th.line()), [Lay.hit(cx, cy, cw, ch, "ob-scroll", Nat.show(U32.to_nat(U32.dim(nl, nv))))]},
    Out.ops(List.concat(&2, R.Op, [Ty.text(ty, 2n, U.Tour.counter.loc(Ty.loc(ty), u), D.Clip.all(), Bool.pick(U32, compact, (cx + 16 + Lay.button.w(ty, label0) + 12 : U32), (cx + 16 : U32)), Bool.pick(U32, compact, (by + 4 : U32), (cy + 8 : U32)), 20, T.Th.faint()),
      Bool.pick(List<&2, R.Op>, compact, Nil{}, Tour.lines(tl, ty, 1n, 28, T.Th.text(), D.Clip.all(), (cx + 16 : U32), (cy + 30 : U32))),
      Tour.lines(all, ty, 0n, 22, Bool.pick(U32, compact, T.Th.text(), T.Th.dim()), D.Clip{(cx + 16 : U32), bt, (cx + cw - 16 : U32), (bt + nv * 22 : U32)}, (cx + 16 : U32), (bt - off * 22 : U32))])),
    Tour.bar(nl, nv, off, (cx + cw - 8 : U32), bt),
    Tour.choices(cs, ty, (cx + 16 : U32), (bt + nv * 22 + 8 : U32)),
    Lay.button(ty, D.Clip.all(), (cx + 16 : U32), by, label0, T.Th.soft(), "ob-skip", ""),
    Tour.back(U.Tour.back(u), ty, (nx - 6 - Lay.button.w(ty, MSG.T.tour.back(Ty.loc(ty))) : U32), by),
    Lay.button(ty, D.Clip.all(), nx, by, nb, T.Th.accent(), "ob-next", "")])

def Lay.tour.if(open: Bool, +ty: Ty, +n: S.Nui, +w: U32, +h: U32) -> Out:
  match open:
    case True{}:
      Lay.tour(ty, n, w, h)
    case False{}:
      Out.none()

# What the input method composes in a single-line field (the composer has
# its own, Comp.pre): the field's box is the "focus" hit of the focused id,
# 28 tall; the clause being converted is shaded and all of it underlined,
# placed as Lay.field.x places the text it shows.
def Lay.ime.run(+ty: Ty, +vis: String, +x: U32, +y: U32, r: IME.Run) -> List<&2, R.Op>:
  match r:
    case IME.Run{+lo, +hi, +hot}:
      +a = {Ty.width(ty, 2n, String.take(vis, lo)) : U32}
      +b = {Ty.width(ty, 2n, String.take(vis, hi)) : U32}
      List.append(&2, R.Op, Bool.pick(List<&2, R.Op>, Bool.and(hot, U32.is_gt(b, a)), [D.Draw.rect(D.Clip.all(), (x + 8 + a : U32), (y + 5 : U32), (b - a : U32), 18, T.Th.sel(), 255)], Nil{}),
        Bool.pick(List<&2, R.Op>, U32.is_gt(b, a), [D.Draw.rect(D.Clip.all(), (x + 8 + a : U32), (y + 22 : U32), (b - a : U32), 1, T.Th.text(), 255)], Nil{}))

def Lay.ime.runs(rs: List<&2, IME.Run>, +ty: Ty, +vis: String, +x: U32, +y: U32) -> List<&2, R.Op>:
  match rs:
    case Nil{}:
      Nil{}
    case r <> t:
      List.append(&2, R.Op, Lay.ime.run(ty, vis, x, y, r), Lay.ime.runs(t, ty, vis, x, y))

def Lay.ime.field(h: S.Hit, +ty: Ty, +shown: String, rs: List<&2, IME.Run>) -> Out:
  match h:
    case S.Hit{+x0, +y0, +x1, +y1, _, _}:
      Out.when(U32.is_eq((y1 - y0 : U32), 28), Out.ops(Lay.ime.runs(rs, ty, Ty.fit(ty, 2n, shown, ((x1 - x0 : U32) - 16 : U32)), x0, y0)))

def Lay.ime.find(hs: List<&2, S.Hit>, +id: String) -> Maybe<&2, S.Hit>:
  match hs:
    case Nil{}:
      None{}
    case S.Hit{+x0, +y0, +x1, +y1, +a, +v} <> t:
      Bool.pick(Maybe<&2, S.Hit>, Bool.and(String.eq(a, "focus"), String.eq(v, id)), Some{S.Hit{x0, y0, x1, y1, a, v}}, Lay.ime.find(t, id))

def Lay.ime.at(m: Maybe<&2, S.Hit>, +ty: Ty, +n: S.Nui) -> Out:
  match m:
    case Some{h}:
      Lay.ime.field(h, ty, S.Imx.field_shown(n), S.Imx.field_runs(n))
    case None{}:
      Out.none()

def Lay.ime.cat(o: Out, x: Out) -> Out:
  Out.cat(o, x)

def Lay.ime.go(on: Bool, +ty: Ty, +n: S.Nui, o: Out) -> Out:
  match on:
    case True{}:
      match o:
        case Out{+os, +hs}:
          Lay.ime.cat(Out{os, hs}, Lay.ime.at(Lay.ime.find(hs, S.Nui.focus_of(n)), ty, n))
    case False{}:
      o

def Lay.ime.over(+ty: Ty, +n: S.Nui, o: Out) -> Out:
  Lay.ime.go(S.Imx.other(n), ty, n, o)

# the whole window, topmost first; `more` lays out what the sidebar shows
# under the projects, `main` a main area in place of the usual one
def Lay.frame.us(+ty: Ty, +memo: TLMemo, +n: S.Nui, utop: Out, +uh: U32, more: D.Clip -> U32 -> Col, main: Maybe<&2, Out>, side: Maybe<&2, Out>) -> Out:
  match n:
    case S.Nui{U.Ui{_, _, _, _, _, _, _, _, +pa, _, _, _, +err, note, _, _, _}, +w, +h, _, _, _, _, _, _, _, _, _, _}:
      # the main area and the sidebar are laid out at once
      mn sb = Lay.main.m(main, ty, memo, Lay.shrink(n, Lay.term_h(n)), 264, Bool.pick(U32, pa, (264 + U32.shr((w - 264 : U32)) : U32), w)) Lay.side.m(side, ty, n, h, utop, uh, more)
      Lay.rev(Lay.ime.over(ty, n, Out.all([
        mn,
        Lay.viewer.if(pa, ty, n, (264 + U32.shr((w - 264 : U32)) : U32), w, h),
        Lay.term.of(ty, n, Bool.pick(U32, pa, (264 + U32.shr((w - 264 : U32)) : U32), w), h),
        sb,
        Lay.toast(ty, err, w, h),
        Lay.palette.if(U.Ui.flag(S.Nui.ui(n), "palette"), ty, n, w, h),
        Lay.proj.if(U.Ui.flag(S.Nui.ui(n), "proj"), ty, n, w, h),
        Lay.hover(U.Ui.info(S.Nui.ui(n), "hover"), S.Hover.alpha(n, S.Nui.ms_of(n))),
        Lay.perf.if(U.Ui.flag(S.Nui.ui(n), "perf"), ty, U.Ui.info(S.Nui.ui(n), "perf.hud"), w),
        Lay.tour.if(U.Tour.on(S.Nui.ui(n)), ty, n, w, h)])))

def Lay.frame.u(+ty: Ty, +memo: TLMemo, +n: S.Nui, utop: Out, +uh: U32, more: D.Clip -> U32 -> Col, main: Maybe<&2, Out>) -> Out:
  Lay.frame.us(ty, memo, n, utop, uh, more, main, None{})

# (nothing pinned above the sidebar's search field)
def Lay.frame.x(+ty: Ty, +memo: TLMemo, +n: S.Nui, more: D.Clip -> U32 -> Col, main: Maybe<&2, Out>) -> Out:
  Lay.frame.u(ty, memo, n, Out.none(), 0, more, main)

def Lay.frame(+ty: Ty, +memo: TLMemo, +n: S.Nui) -> Out:
  Lay.frame.x(ty, memo, n, c => y => Col{Out.none(), y}, None{})
