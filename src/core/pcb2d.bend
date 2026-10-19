import Base
import ./board.bend as B
import ./sexpr.bend as S
import ./pcb3d.bend as P

# The 2D board scene adds text to the geometry shared with the 3D face.
# Keep that geometry parser separate: the 3D face already adds its own
# silkscreen text. Both views use the same KiCad stroke-font layout.

def Text.items(gs: List<&2, P.Got>, +sig: U32, +info: String) -> List<&2, B.Item>:
  match gs:
    case Nil{}:
      Nil{}
    case P.GText{B.Item{k, _, l, p, _}} <> t:
      B.Item{k, sig, l, p, info} <> Text.items(t, sig, info)
    case _ <> t:
      Text.items(t, sig, info)

def Text.draw(at: B.Pt, +angle: F32, +x: S.Sx, +s: String, +up: Bool, +key: String, +sig: U32, +ctx: String) -> List<&2, B.Item>:
  Text.items(P.Text.draw(s, at, angle, x, up, B.Layer.id(S.Sx.attr(x, "layer")), key, Nil{}), sig,
    "text" ++ B.Info.kv("text", s) ++ ctx)

def Text.visible.k(hide: Bool, +at: B.Pt, +angle: F32, +x: S.Sx, +s: String, +up: Bool, +key: String, +sig: U32, +ctx: String) -> List<&2, B.Item>:
  match hide:
    case True{}:
      Nil{}
    case False{}:
      Text.draw(at, angle, x, s, up, key, sig, ctx)

def Text.visible(+at: B.Pt, +angle: F32, +x: S.Sx, +s: String, +up: Bool, +key: String, +sig: U32, +ctx: String) -> List<&2, B.Item>:
  Text.visible.k(Bool.or(String.is_empty(s), Bool.or(P.Sx.flag(x, "hide"), P.Sx.flag(S.Sx.get(x, "effects"), "hide"))),
    at, angle, x, s, up, key, sig, ctx)

def Text.child.k(text: Bool, +x: S.Sx, +pl: B.Place, +ref: String, +val: String, +key: String, +sig: U32, +ctx: String) -> List<&2, B.Item>:
  match text:
    case True{}:
      +at = S.Sx.get(x, "at")
      Text.visible(B.Geo.place(pl, B.Geo.pt(at)), S.Sx.f(at, 2n), x, P.Text.subst(S.Sx.arg(x, 1n), ref, val), True{}, key, sig, ctx)
    case False{}:
      Nil{}

def Text.children(xs: List<&2, S.Sx>, +pl: B.Place, +ref: String, +val: String, +key: String, +sig: U32, +ctx: String, +i: Nat) -> List<&2, B.Item>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> t:
      List.append(&2, B.Item,
        Text.child.k(Bool.or(String.eq(S.Sx.name(x), "fp_text"), String.eq(S.Sx.name(x), "property")), x, pl, ref, val,
          key ++ "/text/" ++ B.Board.key(x, Nat.show(i)), sig, ctx),
        Text.children(t, pl, ref, val, key, sig, ctx, 1n+i))

def Text.form.k(graphic: Bool, footprint: Bool, +x: S.Sx, +key: String, +sig: U32) -> List<&2, B.Item>:
  match graphic footprint:
    case True{} _:
      +at = S.Sx.get(x, "at")
      Text.visible(B.Geo.pt(at), S.Sx.f(at, 2n), x, S.Sx.arg(x, 0n), False{}, key ++ "/text", sig, "")
    case False{} True{}:
      Text.children(S.Sx.args(x), B.Fp.place(x), B.Info.prop(x, "Reference"), B.Info.prop(x, "Value"), key, sig, B.Info.footprint(x), 0n)
    case False{} False{}:
      Nil{}

def Pcb2.form(+x: S.Sx, +i: Nat, +sig: U32) -> List<&2, B.Item>:
  List.append(&2, B.Item, B.Board.form(x, i, sig),
    Text.form.k(String.eq(S.Sx.name(x), "gr_text"), String.eq(S.Sx.name(x), "footprint"), x, B.Board.key(x, "n" ++ Nat.show(i)), sig))

def Pcb2.one(+bytes: List<&2, U32>, +i: Nat) -> List<&2, B.Item>:
  Pcb2.form(S.Sx.parse(bytes), i, B.Board.hash(bytes, 2166136261))

def Pcb2.seq(fs: List<&2, B.Form>) -> List<&2, B.Item>:
  match fs:
    case Nil{}:
      Nil{}
    case B.Form{+i, bytes} <> t:
      List.append(&2, B.Item, Pcb2.one(bytes, i), Pcb2.seq(t))

def Pcb2.par(d: Nat, fs: List<&2, B.Form>) -> List<&2, B.Item>:
  match d:
    case 0n:
      Pcb2.seq(fs)
    case 1n+e0:
      +e = e0
      B.Board.split(fs, a => b => (
        x y = Pcb2.par(e, a) Pcb2.par(e, b)
        List.append(&2, B.Item, x, y)))

def Pcb2.items.k(m: Maybe<&2, List<&2, List<&2, U32>>>) -> Maybe<&2, List<&2, B.Item>>:
  match m:
    case Some{fs}:
      Some{Pcb2.par(8n, B.Board.number(fs, 0n))}
    case None{}:
      None{}

def Pcb2.items(bs: List<&2, U32>) -> Maybe<&2, List<&2, B.Item>>:
  Pcb2.items.k(S.Sx.forms(bs))
