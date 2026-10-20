import Base
import ../core/client.bend as U
import ../core/edit.bend as ED
import ../gfx/raster.bend as R
import ../gfx/draw.bend as D
import ./theme.bend as T
import ./nui.bend as S
import ./layout.bend as L

# Small pieces the bots' screens are made of (bots.bend, space.bend):
# chips and buttons cut to a clip, and form fields whose text lives in the
# client's scratch ("@f.<name>", nui.bend's Fld).

# what a field needs from the window: the focused id, whether the caret
# shows, the caret and the selection's other end
type FS is Data:
  FS{focus: String, blink: Bool, caret: Nat, mark: Nat, u: U.Ui}

def Fm.fs(+n: S.Nui) -> FS:
  FS{S.Nui.focus_of(n), S.Blink.on(n), S.Nui.caret_of(n), S.Nui.mark(n), S.Nui.ui(n)}

def FS.focus(+f: FS) -> String:
  match f:
    case FS{+fo, _, _, _, _}:
      fo

def FS.blink(+f: FS) -> Bool:
  match f:
    case FS{_, +b, _, _, _}:
      b

def FS.caret(+f: FS) -> Nat:
  match f:
    case FS{_, _, +c, _, _}:
      c

def FS.mark(+f: FS) -> Nat:
  match f:
    case FS{_, _, _, +m, _}:
      m

def FS.ui(+f: FS) -> U.Ui:
  match f:
    case FS{_, _, _, _, +u}:
      u

def Fm.text(+f: FS, +name: String) -> String:
  U.Ui.scratch(FS.ui(f), "@f." ++ name)

# a chip that is on or off, cut to c
def Fm.chip(+ty: L.Ty, +c: D.Clip, +x: U32, +y: U32, +label: String, +on: Bool, action: String, value: String) -> L.Out:
  +w = L.Lay.button.w(ty, label)
  L.Out{List.append(&2, R.Op, L.Lay.box(c, x, y, w, 28, 0, Bool.pick(U32, on, T.Th.sel(), T.Th.raise()), Bool.pick(U32, on, T.Th.accent(), T.Th.line())),
      L.Ty.text(ty, 2n, label, c, (x + 10 : U32), y, 28, Bool.pick(U32, on, T.Th.text(), T.Th.dim()))),
    [L.Lay.hit(x, y, w, 28, action, value)]}

# a row of chips from x, answers the output (they wrap at x1 back to x0)
type Flow is Data:
  Flow{out: L.Out, x: U32, y: U32}

def Flow.out(f: Flow) -> L.Out:
  match f:
    case Flow{o, _, _}:
      o

def Flow.y(f: Flow) -> U32:
  match f:
    case Flow{_, _, y}:
      y

def Flow.add(f: Flow, o: L.Out) -> L.Out:
  match f:
    case Flow{o2, _, _}:
      L.Out.cat(o, o2)

# a one-line field for form field `name`: Enter runs `action` with `pre`
# and the text
def Fm.field(+ty: L.Ty, +f: FS, +c: D.Clip, +x: U32, +y: U32, +w: U32, +name: String, +action: String, +pre: String, +ph: String) -> L.Out:
  +id = S.Fld.id(name, action, pre)
  L.Lay.field(ty, c, x, y, w, Fm.text(f, name), String.eq(FS.focus(f), id), FS.blink(f), FS.caret(f), FS.mark(f), ph, id)

def Fm.rows(ls: List<&2, String>, +ty: L.Ty, +c: D.Clip, +x: U32, +y: U32, +lh: U32, +color: U32) -> List<&2, R.Op>:
  match ls:
    case Nil{}:
      Nil{}
    case l <> t:
      List.append(&2, R.Op, L.Ty.text(ty, 2n, l, c, x, y, lh, color), Fm.rows(t, ty, c, x, (y + lh : U32), lh, color))

# text wrapped to px in face k, one line per entry
def Fm.wrap(+ty: L.Ty, +k: Nat, +s: String, +px: U32) -> List<&2, String>:
  L.Ty.wrap(ty, k, s, px)

def Fm.caret.row(+ty: L.Ty, +s: String, +p: Nat, +px: U32) -> Nat:
  Nat.sub(List.length(&2, String, L.Ty.wrap(ty, 2n, String.take(s, p), px)), 1n)

def Fm.caret.x(+ty: L.Ty, +s: String, +p: Nat, +px: U32) -> U32:
  L.Ty.width(ty, 2n, Maybe.default(&2, String, List.last(&2, String, L.Ty.wrap(ty, 2n, String.take(s, p), px)), ""))

def Fm.area.caret(on: Bool, +blink: Bool, +ty: L.Ty, +s: String, +p: Nat, +px: U32, +top: Nat, +rows: Nat, +c: D.Clip, +x: U32, +y: U32) -> List<&2, R.Op>:
  match on:
    case True{}:
      +r = Fm.caret.row(ty, s, p, px)
      Bool.pick(List<&2, R.Op>, Bool.and(Nat.is_ge(r, top), Nat.is_lt(Nat.sub(r, top), rows)),
        [D.Draw.caret(c, (x + 8 + Fm.caret.x(ty, s, p, px) : U32), (y + 8 + (U32.from_nat(Nat.sub(r, top)) * L.Ty.px(ty, 20) : U32) : U32), 1, L.Ty.px(ty, 16), T.Th.accent(), 255, blink)], Nil{})
    case False{}:
      Nil{}

def Fm.area.h.at(+rows: Nat, +ty: L.Ty) -> U32:
  (U32.from_nat(rows) * L.Ty.px(ty, 20) + 12 : U32)

def Fm.area.h(+rows: Nat) -> U32:
  Fm.area.h.at(rows, L.Ty.none())

# a field of `rows` lines (shift+Enter starts a new line); it shows the
# lines around the caret
def Fm.area(+ty: L.Ty, +f: FS, +c: D.Clip, +x: U32, +y: U32, +w: U32, +rows: Nat, +name: String, +action: String, +pre: String, +ph: String) -> L.Out:
  +id = S.Fld.id(name, action, pre)
  +on = String.eq(FS.focus(f), id)
  +s = Fm.text(f, name)
  +px = {(w - L.Ty.px(ty, 16) : U32) : U32}
  +ls = L.Ty.wrap(ty, 2n, s, px)
  +top = Bool.pick(Nat, on, Nat.sub(Fm.caret.row(ty, s, FS.caret(f), px), Nat.sub(rows, 1n)), 0n)
  +h = Fm.area.h.at(rows, ty)
  L.Out{List.concat(&2, R.Op, [
      L.Lay.box(c, x, y, w, h, 0, T.Th.bg(), Bool.pick(U32, on, T.Th.accent(), T.Th.line())),
      Bool.pick(List<&2, R.Op>, String.is_empty(s), L.Ty.text(ty, 2n, ph, c, (x + 8 : U32), (y + 6 : U32), L.Ty.px(ty, 20), T.Th.faint()),
        Fm.rows(List.take(&2, String, List.drop(&2, String, ls, top), rows), ty, c, (x + 8 : U32), (y + 6 : U32), L.Ty.px(ty, 20), T.Th.text())),
      Fm.area.caret(on, FS.blink(f), ty, s, FS.caret(f), px, top, rows, c, x, y)]),
    [L.Lay.hit(x, y, w, h, "focus", id)]}

# a button cut to c (accent or plain text)
def Fm.button(+ty: L.Ty, +c: D.Clip, +x: U32, +y: U32, +label: String, +color: U32, action: String, value: String) -> L.Out:
  L.Lay.button(ty, c, x, y, label, color, action, value)

def Fm.button.w(+ty: L.Ty, +label: String) -> U32:
  L.Lay.button.w(ty, label)

# a small heading in the meta face
def Fm.label(+ty: L.Ty, +c: D.Clip, +s: String, +x: U32, +y: U32, +color: U32) -> L.Out:
  L.Out.ops(L.Ty.text(ty, 2n, s, c, x, y, 20, color))
