import Base
import ../core/msgs.bend as MSG
import ../core/space.bend as SP
import ../gfx/raster.bend as R
import ../gfx/draw.bend as D
import ./theme.bend as T
import ./nui.bend as S
import ./layout.bend as L
import ./forms.bend as F

# A bot's space (core/space.bend's blocks) drawn natively, top down in a
# column w wide from (x, y). Every decision about the spec (what is kept,
# caps, percentages, tones) is core/space.bend's; this only places it.
#
# A button runs action "space" with "<bot>\x1f<action>"; an input is a
# form field ("sp.<action>") whose Enter, or its Send button, runs
# "space-input" with "<bot>\x1f<action>\x1f<text>".

def Spv.sep() -> String:
  SCon{Chr{31}, SNil{}}

def Spv.key(+bot: String, +action: String) -> String:
  bot ++ Spv.sep() ++ action

def Spv.lines(ls: List<&2, String>, +ty: L.Ty, +k: Nat, +c: D.Clip, +x: U32, +y: U32, +lh: U32, +color: U32) -> List<&2, R.Op>:
  match ls:
    case Nil{}:
      Nil{}
    case l <> t:
      List.append(&2, R.Op, L.Ty.text(ty, k, l, c, x, y, lh, color), Spv.lines(t, ty, k, c, x, (y + lh : U32), lh, color))

def Spv.n(+ls: List<&2, String>) -> U32:
  U32.from_nat(List.length(&2, String, ls))

# wrapped text in the chat face, 22 px lines
def Spv.text(+ty: L.Ty, +s: String, +color: U32, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  +ls = L.Ty.wrap(ty, 3n, s, w)
  L.Col{L.Out.ops(Spv.lines(ls, ty, 3n, c, x, y, L.Ty.px(ty, 22), color)), (y + Spv.n(ls) * L.Ty.px(ty, 22) + 10 : U32)}

def Spv.tone(+t: String) -> U32:
  Bool.pick(U32, String.eq(t, "ok"), T.Th.ok(), Bool.pick(U32, String.eq(t, "warn"), T.Th.warn(), Bool.pick(U32, String.eq(t, "bad"), T.Th.bad(), T.Th.dim())))

# a stat card; cards are all one height, so a row of them lines up
def Spv.stat(+ty: L.Ty, +l: String, +v: String, +hint: String, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  +hh = {L.Ty.px(ty, 20) : U32}
  L.Col{L.Out.ops(List.concat(&2, R.Op, [
      L.Lay.box(c, x, y, w, (L.Ty.px(ty, 64) + hh : U32), 0, T.Th.panel(), T.Th.line()),
      L.Ty.text(ty, 2n, L.Ty.fit(ty, 2n, l, (w - L.Ty.px(ty, 24) : U32)), c, (x + 12 : U32), (y + 8 : U32), L.Ty.px(ty, 20), T.Th.dim()),
      L.Ty.text(ty, 1n, L.Ty.fit(ty, 1n, v, (w - L.Ty.px(ty, 24) : U32)), c, (x + 12 : U32), (y + L.Ty.px(ty, 30) : U32), L.Ty.px(ty, 24), T.Th.text()),
      L.Ty.text(ty, 2n, L.Ty.fit(ty, 2n, hint, (w - L.Ty.px(ty, 24) : U32)), c, (x + 12 : U32), (y + L.Ty.px(ty, 56) : U32), L.Ty.px(ty, 20), T.Th.faint())])),
    (y + L.Ty.px(ty, 76) + hh : U32)}

def Spv.progress(+ty: L.Ty, +l: String, +p: Nat, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  +pct = SP.Space.pct(p)
  +pw = L.Ty.width(ty, 2n, pct)
  +fw = {U32.from_nat(Nat.div(Nat.mul(U32.to_nat(w), p), 1000n)) : U32}
  L.Col{L.Out.ops(List.concat(&2, R.Op, [
      L.Ty.text(ty, 2n, L.Ty.fit(ty, 2n, l, L.U32.dim(w, (pw + 16 : U32))), c, x, y, L.Ty.px(ty, 20), T.Th.soft()),
      L.Ty.text(ty, 2n, pct, c, (x + w - pw : U32), y, L.Ty.px(ty, 20), T.Th.dim()),
      [D.Draw.rect(c, x, (y + L.Ty.px(ty, 24) : U32), w, 6, T.Th.raise(), 255), D.Draw.rect(c, x, (y + L.Ty.px(ty, 24) : U32), fw, 6, T.Th.accent(), 255)]])),
    (y + L.Ty.px(ty, 44) : U32)}

def Spv.list(xs: List<&2, String>, +ty: L.Ty, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  match xs:
    case Nil{}:
      L.Col{L.Out.none(), (y + 6 : U32)}
    case +s <> t:
      +ls = L.Ty.wrap(ty, 3n, s, (w - L.Ty.px(ty, 20) : U32))
      L.Lay.col.cat(L.Out.ops(List.append(&2, R.Op, [D.Draw.disc(c, U32.to_f32((x + 6 : U32)), U32.to_f32((y + 11 : U32)), 2.5, T.Th.dim(), 255)],
          Spv.lines(ls, ty, 3n, c, (x + L.Ty.px(ty, 20) : U32), y, L.Ty.px(ty, 22), T.Th.text()))),
        Spv.list(t, ty, c, x, w, (y + Spv.n(ls) * L.Ty.px(ty, 22) + 2 : U32)))

def Spv.kv(rs: List<&2, SP.Cell>, +ty: L.Ty, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  match rs:
    case Nil{}:
      L.Col{L.Out.none(), (y + 8 : U32)}
    case SP.Cell{+k, +v} <> t:
      +kw = {(w * 2 / 5 : U32) : U32}
      L.Lay.col.cat(L.Out.ops(List.concat(&2, R.Op, [
          L.Ty.text(ty, 2n, L.Ty.fit(ty, 2n, k, (kw - 12 : U32)), c, x, y, 26, T.Th.dim()),
          L.Ty.text(ty, 0n, L.Ty.fit(ty, 0n, v, (w - kw : U32)), c, (x + kw : U32), y, 26, T.Th.text()),
          [D.Draw.rect(c, x, (y + 26 : U32), w, 1, T.Th.line(), 255)]])),
        Spv.kv(t, ty, c, x, w, (y + 27 : U32)))

def Spv.cells(cs: List<&2, String>, +ty: L.Ty, +k: Nat, +color: U32, +c: D.Clip, +x: U32, +cw: U32, +y: U32) -> List<&2, R.Op>:
  match cs:
    case Nil{}:
      Nil{}
    case +s <> t:
      List.append(&2, R.Op, L.Ty.text(ty, k, L.Ty.fit(ty, k, s, (cw - 12 : U32)), c, x, y, 28, color), Spv.cells(t, ty, k, color, c, (x + cw : U32), cw, y))

def Spv.trows(rs: List<&2, List<&2, String>>, +ty: L.Ty, +c: D.Clip, +x: U32, +w: U32, +cw: U32, +y: U32) -> L.Col:
  match rs:
    case Nil{}:
      L.Col{L.Out.none(), (y + 10 : U32)}
    case r <> t:
      L.Lay.col.cat(L.Out.ops(List.append(&2, R.Op, Spv.cells(r, ty, 2n, T.Th.soft(), c, (x + 8 : U32), cw, y), [D.Draw.rect(c, x, (y + 28 : U32), w, 1, T.Th.line(), 255)])),
        Spv.trows(t, ty, c, x, w, cw, (y + 29 : U32)))

# how many cells the first row has
def Spv.cols(rs: List<&2, List<&2, String>>) -> Nat:
  match rs:
    case Nil{}:
      0n
    case r <> _:
      List.length(&2, String, r)

def Spv.table(+ty: L.Ty, +h: List<&2, String>, +rs: List<&2, List<&2, String>>, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  +cols = {U32.from_nat(Nat.max(Nat.max(List.length(&2, String, h), Spv.cols(rs)), 1n)) : U32}
  +cw = {((w - 16 : U32) / cols : U32) : U32}
  L.Lay.col.cat(L.Out.ops(List.concat(&2, R.Op, [L.Lay.box(c, x, y, w, 30, 0, T.Th.panel(), T.Th.line()), Spv.cells(h, ty, 2n, T.Th.dim(), c, (x + 8 : U32), cw, (y + 1 : U32))])),
    Spv.trows(rs, ty, c, x, w, cw, (y + 30 : U32)))

def Spv.badge(+ty: L.Ty, +s: String, +tone: String, +c: D.Clip, +x: U32, +y: U32) -> L.Col:
  +bw = {(L.Ty.width(ty, 2n, s) + 16 : U32) : U32}
  L.Col{L.Out.ops(List.append(&2, R.Op, L.Lay.box(c, x, y, bw, 22, 0, T.Th.bg(), Spv.tone(SP.Space.tone(tone))), L.Ty.text(ty, 2n, s, c, (x + 8 : U32), y, 22, Spv.tone(SP.Space.tone(tone))))),
    (y + 32 : U32)}

def Spv.code(+ty: L.Ty, +s: String, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  +ls = String.lines(s)
  +h = {(Spv.n(ls) * L.Ty.px(ty, 20) + 16 : U32) : U32}
  L.Col{L.Out.ops(List.append(&2, R.Op, L.Lay.box(c, x, y, w, h, 0, T.Th.code(), T.Th.line()), Spv.lines(ls, ty, 4n, c, (x + 12 : U32), (y + 8 : U32), L.Ty.px(ty, 20), T.Th.soft()))),
    (y + h + 10 : U32)}

def Spv.button(+ty: L.Ty, +l: String, +a: String, +bot: String, +c: D.Clip, +x: U32, +y: U32) -> L.Col:
  L.Col{F.Fm.button(ty, c, x, y, l, T.Th.accent(), "space", Spv.key(bot, a)), (y + 38 : U32)}

def Spv.input(+ty: L.Ty, +f: F.FS, +l: String, +a: String, +p: String, +bot: String, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  +label0 = MSG.T.composer.send(L.Ty.loc(ty))
  +ly = {Bool.pick(U32, String.is_empty(l), 0, 22) : U32}
  +pre = Spv.key(bot, a) ++ Spv.sep()
  +sw = F.Fm.button.w(ty, label0)
  L.Col{L.Out.all([
      L.Out.ops(L.Ty.text(ty, 2n, L.Ty.fit(ty, 2n, l, w), c, x, y, 20, T.Th.dim())),
      F.Fm.field(ty, f, c, x, (y + ly : U32), (w - sw - 8 : U32), "sp." ++ a, "space-input", pre, p),
      F.Fm.button(ty, c, (x + w - sw : U32), (y + ly : U32), label0, T.Th.accent(), "space-input", pre ++ F.Fm.text(f, "sp." ++ a))]),
    (y + ly + 40 : U32)}

# how many columns the widest row has
def Spv.row.n(+bs: List<&2, SP.Block>) -> U32:
  U32.from_nat(Nat.max(List.length(&2, SP.Block, bs), 1n))

# a leaf block (a row's blocks are leaves: core/space.bend keeps no rows
# in rows)
def Spv.leaf(+ty: L.Ty, +f: F.FS, b: SP.Block, +bot: String, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  match b:
    case SP.BHeading{s}:
      L.Col{L.Out.ops(L.Ty.text(ty, 1n, L.Ty.fit(ty, 1n, s, w), c, x, (y + 6 : U32), 28, T.Th.text())), (y + 40 : U32)}
    case SP.BText{+s}:
      Spv.text(ty, s, T.Th.soft(), c, x, w, y)
    case SP.BStat{+l, +v, +h}:
      Spv.stat(ty, l, v, h, c, x, w, y)
    case SP.BProgress{+l, +p}:
      Spv.progress(ty, l, p, c, x, w, y)
    case SP.BList{xs}:
      Spv.list(xs, ty, c, x, w, y)
    case SP.BKv{rs}:
      Spv.kv(rs, ty, c, x, w, y)
    case SP.BTable{+h, +rs}:
      Spv.table(ty, h, rs, c, x, w, y)
    case SP.BButton{+l, +a}:
      Spv.button(ty, l, a, bot, c, x, y)
    case SP.BInput{+l, +a, +p}:
      Spv.input(ty, f, l, a, p, bot, c, x, w, y)
    case SP.BDivider{}:
      L.Col{L.Out.ops([D.Draw.rect(c, x, (y + 8 : U32), w, 1, T.Th.line(), 255)]), (y + 18 : U32)}
    case SP.BBadge{+s, +t}:
      Spv.badge(ty, s, t, c, x, y)
    case SP.BCode{+s}:
      Spv.code(ty, s, c, x, w, y)
    case SP.BRow{_}:
      L.Col{L.Out.none(), y}

def Spv.max(+a: U32, +b: U32) -> U32:
  Bool.pick(U32, U32.is_gt(a, b), a, b)

# two columns side by side: both outputs, down to the lower one
def Spv.row.join(a: L.Col, b: L.Col) -> L.Col:
  match a b:
    case L.Col{oa, +ya} L.Col{ob, +yb}:
      L.Col{L.Out.cat(oa, ob), Spv.max(ya, yb)}

def Spv.row.go(bs: List<&2, SP.Block>, +ty: L.Ty, +f: F.FS, +bot: String, +c: D.Clip, +x: U32, +cw: U32, +y: U32) -> L.Col:
  match bs:
    case Nil{}:
      L.Col{L.Out.none(), y}
    case b <> t:
      Spv.row.join(Spv.leaf(ty, f, b, bot, c, x, cw, y), Spv.row.go(t, ty, f, bot, c, (x + cw + 12 : U32), cw, y))

# buttons and badges keep their own width; a row of only those flows
def Spv.inline(b: SP.Block) -> Bool:
  match b:
    case SP.BButton{_, _}:
      True{}
    case SP.BBadge{_, _}:
      True{}
    case _:
      False{}

def Spv.inline.all(bs: List<&2, SP.Block>) -> Bool:
  match bs:
    case Nil{}:
      True{}
    case b <> t:
      Bool.and(Spv.inline(b), Spv.inline.all(t))

def Spv.inline.w(+ty: L.Ty, b: SP.Block) -> U32:
  match b:
    case SP.BButton{+l, _}:
      F.Fm.button.w(ty, l)
    case SP.BBadge{+s, _}:
      (L.Ty.width(ty, 2n, s) + 16 : U32)
    case _:
      0

def Spv.flow(bs: List<&2, SP.Block>, +ty: L.Ty, +f: F.FS, +bot: String, +c: D.Clip, +x: U32, +y: U32) -> L.Col:
  match bs:
    case Nil{}:
      L.Col{L.Out.none(), y}
    case +b <> t:
      Spv.row.join(Spv.leaf(ty, f, b, bot, c, x, Spv.inline.w(ty, b), y), Spv.flow(t, ty, f, bot, c, (x + Spv.inline.w(ty, b) + 8 : U32), y))

def Spv.row.k(inline: Bool, +ty: L.Ty, +f: F.FS, +bs: List<&2, SP.Block>, +bot: String, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  match inline:
    case True{}:
      Spv.flow(bs, ty, f, bot, c, x, y)
    case False{}:
      Spv.row.go(bs, ty, f, bot, c, x, ((w - (Spv.row.n(bs) - 1 : U32) * 12 : U32) / Spv.row.n(bs) : U32), y)

# blocks side by side, each an equal share of the width
def Spv.row(+ty: L.Ty, +f: F.FS, +bs: List<&2, SP.Block>, +bot: String, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  Spv.row.k(Spv.inline.all(bs), ty, f, bs, bot, c, x, w, y)

def Spv.block(+ty: L.Ty, +f: F.FS, b: SP.Block, +bot: String, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  match b:
    case SP.BRow{+bs}:
      Spv.row(ty, f, bs, bot, c, x, w, y)
    case _:
      Spv.leaf(ty, f, b, bot, c, x, w, y)

def Spv.blocks(bs: List<&2, SP.Block>, +ty: L.Ty, +f: F.FS, +bot: String, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  match bs:
    case Nil{}:
      L.Col{L.Out.none(), y}
    case b <> t:
      L.Lay.col.then(Spv.block(ty, f, b, bot, c, x, w, y), y2 => Spv.blocks(t, ty, f, bot, c, x, w, (y2 + 6 : U32)))

# a space: its title, then its blocks
def Spv.draw(+ty: L.Ty, +f: F.FS, sp: SP.Space, +bot: String, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  match sp:
    case SP.Space{+title, bs}:
      +ty2 = {Bool.pick(U32, String.is_empty(title), y, (y + 44 : U32)) : U32}
      L.Lay.col.cat(L.Out.ops(L.Ty.text(ty, 1n, L.Ty.fit(ty, 1n, title, w), c, x, y, 36, T.Th.text())), Spv.blocks(bs, ty, f, bot, c, x, w, ty2))
