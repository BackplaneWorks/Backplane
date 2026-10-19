import Base
import ./sexpr.bend as S
import ./board.bend as B
import ./glb.bend as G
import ./strokefont.bend as ST

# What a .kicad_pcb says about its board in 3D, beyond the drawable items
# (board.bend): its thickness, its stackup's colours, the silkscreen text,
# and the 3D model of each footprint with the transform KiCad places it by.
#
# World axes are the 3D viewer's: X is board x, Y is up (the board's
# bottom face at 0, its top at the thickness), Z is board y, all in mm.

# the colours of the board (0xRRGGBB): its body (FR4 at the edges), the
# solder mask and its opacity (0..255), silkscreen, the finish on bare
# copper (pads), and copper under the mask
type Look is Data:
  BoardLook{body: U32, mask: U32, mask_a: U32, silk: U32, finish: U32, copper: U32}

# a footprint's model: the file as the board names it (${VARS} and all)
# and the transform from the model's millimetres to the world
type Model3 is Data:
  Model3{path: String, m: G.M4}

type Pcb3 is Data:
  Pcb3{thick: F32, look: Look, text: List<&2, B.Item>, models: List<&2, Model3>}

# what one top-level form contributes
type Got is Data:
  GThick{t: F32}
  GLook{l: Look}
  GText{i: B.Item}
  GModel{m: Model3}

# Colours
# -------

def Rgb.of(+r: U32, +g: U32, +b: U32) -> U32:
  U32.or(U32.shln(r, 16n), U32.or(U32.shln(g, 8n), b))

def Look.default() -> Look:
  BoardLook{Rgb.of(109, 116, 75), Rgb.of(20, 51, 36), 212, Rgb.of(245, 245, 245), Rgb.of(178, 156, 0), Rgb.of(184, 115, 50)}

def Hex.digit(+c: U32) -> U32:
  Bool.pick(U32, U32.is_le(c, 57), (c - 48 : U32), Bool.pick(U32, U32.is_ge(c, 97), (c - 87 : U32), (c - 55 : U32)))

def Hex.go(n: Nat, cs: String, +acc: U32) -> U32:
  match n cs:
    case 1n+k SCon{Chr{+c}, t}:
      Hex.go(k, t, U32.or(U32.shln(acc, 4n), Hex.digit(c)))
    case _ _:
      acc

# "#RRGGBB" or "#RRGGBBAA" (the alpha is dropped)
def Hex.rgb(+s: String) -> U32:
  Hex.go(6n, String.drop(s, 1n), 0)

def Look.named.go(ns: List<&2, String>, cs: List<&2, U32>, +s: String, +dv: U32) -> U32:
  match ns cs:
    case n <> nt c <> ct:
      Bool.pick(U32, String.eq(n, s), c, Look.named.go(nt, ct, s, dv))
    case _ _:
      dv

# a stackup colour: KiCad's named colours, "#RRGGBB[AA]", or dv
def Look.named(+s: String, +dv: U32) -> U32:
  Bool.pick(U32, String.starts_with(s, "#"), Hex.rgb(s),
    Look.named.go(["Green", "Light Green", "Saturated Green", "Red", "Light Red", "Red/Orange", "Blue", "Light Blue 1",
        "Light Blue 2", "Green/Blue", "Black", "White", "Purple", "Light Purple", "Yellow", "FR4 natural", "FR4 natural, dark",
        "PTFE natural", "Polyimide", "Phenolic natural", "Aluminum"],
      [Rgb.of(20, 51, 36), Rgb.of(91, 168, 12), Rgb.of(13, 104, 11), Rgb.of(181, 19, 21), Rgb.of(210, 40, 14), Rgb.of(239, 53, 41),
        Rgb.of(2, 59, 162), Rgb.of(54, 79, 116), Rgb.of(61, 85, 130), Rgb.of(21, 70, 80), Rgb.of(11, 11, 11), Rgb.of(245, 245, 245),
        Rgb.of(32, 2, 53), Rgb.of(119, 31, 91), Rgb.of(194, 195, 0), Rgb.of(109, 116, 75), Rgb.of(51, 43, 22),
        Rgb.of(179, 174, 160), Rgb.of(160, 99, 28), Rgb.of(146, 99, 47), Rgb.of(213, 213, 213)], s, dv))

# the finish on bare copper: gold for ENIG and hard gold, else tin
def Look.finish(+s: String) -> U32:
  Bool.pick(U32, Bool.or(String.is_empty(s), Bool.or(String.starts_with(s, "ENIG"), Bool.or(String.starts_with(s, "Hard"), String.eq(s, "None")))),
    Rgb.of(178, 156, 0), Rgb.of(200, 200, 205))

# a stackup layer's colour by its type, or dv
def Stack.color.go(xs: List<&2, S.Sx>, +ty: String, +dv: U32) -> U32:
  match xs:
    case Nil{}:
      dv
    case +x <> t:
      Bool.pick(U32, Bool.and(String.eq(S.Sx.attr(x, "type"), ty), Bool.not(String.is_empty(S.Sx.attr(x, "color")))),
        Look.named(S.Sx.attr(x, "color"), dv), Stack.color.go(t, ty, dv))

# the first dielectric's colour (its "type" is core or prepreg)
def Stack.body.go(xs: List<&2, S.Sx>, +dv: U32) -> U32:
  match xs:
    case Nil{}:
      dv
    case +x <> t:
      Bool.pick(U32, Bool.and(Bool.or(String.eq(S.Sx.attr(x, "type"), "core"), String.eq(S.Sx.attr(x, "type"), "prepreg")),
          Bool.not(String.is_empty(S.Sx.attr(x, "color")))),
        Look.named(S.Sx.attr(x, "color"), dv), Stack.body.go(t, dv))

def Stack.look(+st: S.Sx, d: Look) -> Look:
  match d:
    case BoardLook{+body, +mask, +ma, +silk, _, +copper}:
      +ls = S.Sx.all(S.Sx.args(st), "layer")
      BoardLook{Stack.body.go(ls, body), Stack.color.go(ls, "Top Solder Mask", mask), ma, Stack.color.go(ls, "Top Silk Screen", silk),
        Look.finish(S.Sx.attr(st, "copper_finish")), copper}

# Model placement
# ---------------

def Deg.rad(+d: F32) -> F32:
  F32.mul(d, F32.div(F32.pi(), 180.0))

def M4.move(+x: F32, +y: F32, +z: F32) -> G.M4:
  G.M4{1.0, 0.0, 0.0, x, 0.0, 1.0, 0.0, y, 0.0, 0.0, 1.0, z}

def M4.scale(+x: F32, +y: F32, +z: F32) -> G.M4:
  G.M4{x, 0.0, 0.0, 0.0, 0.0, y, 0.0, 0.0, 0.0, 0.0, z, 0.0}

def M4.rz(+deg: F32) -> G.M4:
  +c = F32.cos(Deg.rad(deg))
  +s = F32.sin(Deg.rad(deg))
  G.M4{c, F32.neg(s), 0.0, 0.0, s, c, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0}

def M4.ry(+deg: F32) -> G.M4:
  +c = F32.cos(Deg.rad(deg))
  +s = F32.sin(Deg.rad(deg))
  G.M4{c, 0.0, s, 0.0, 0.0, 1.0, 0.0, 0.0, F32.neg(s), 0.0, c, 0.0}

def M4.rx(+deg: F32) -> G.M4:
  +c = F32.cos(Deg.rad(deg))
  +s = F32.sin(Deg.rad(deg))
  G.M4{1.0, 0.0, 0.0, 0.0, 0.0, c, F32.neg(s), 0.0, 0.0, s, c, 0.0}

# KiCad's 3D axes (x right, y up the board, z out of the top) to the world
def M4.world() -> G.M4:
  G.M4{1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, F32.neg(1.0), 0.0, 0.0}

def M4.chain(ms: List<&2, G.M4>) -> G.M4:
  match ms:
    case Nil{}:
      G.M4.id()
    case m <> t:
      G.M4.mul(m, M4.chain(t))

def Xyz.at(+x: S.Sx, +name: String, +i: Nat, +dv: F32) -> F32:
  +v = S.Sx.get(S.Sx.get(x, name), "xyz")
  Bool.pick(F32, String.is_empty(S.Sx.arg(v, i)), dv, S.Sx.f(v, i))

# as KiCad draws it: to the footprint's place on its side of the board,
# turned by its angle, flipped when on the back; then the model's offset,
# its rotation (z, y, x, each negated) and scale
def Model3.matrix(+fx: F32, +fy: F32, +fa: F32, +back: Bool, +thick: F32, +x: S.Sx) -> G.M4:
  M4.chain([M4.world(), M4.move(fx, F32.neg(fy), Bool.pick(F32, back, 0.0, thick)), M4.rz(fa),
    Bool.pick(G.M4, back, M4.rx(180.0), G.M4.id()),
    M4.move(Xyz.at(x, "offset", 0n, 0.0), Xyz.at(x, "offset", 1n, 0.0), Xyz.at(x, "offset", 2n, 0.0)),
    M4.rz(F32.neg(Xyz.at(x, "rotate", 2n, 0.0))), M4.ry(F32.neg(Xyz.at(x, "rotate", 1n, 0.0))), M4.rx(F32.neg(Xyz.at(x, "rotate", 0n, 0.0))),
    M4.scale(Xyz.at(x, "scale", 0n, 1.0), Xyz.at(x, "scale", 1n, 1.0), Xyz.at(x, "scale", 2n, 1.0))])

# a bare atom among the arguments (hide, dnp, ...), or (name yes)
def Sx.flag.go(xs: List<&2, S.Sx>, +name: String) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case +x <> t:
      Bool.or(Bool.or(String.eq(S.Sx.text(x), name), Bool.and(String.eq(S.Sx.name(x), name), Bool.not(String.eq(S.Sx.arg(x, 0n), "no")))),
        Sx.flag.go(t, name))

def Sx.flag(+x: S.Sx, +name: String) -> Bool:
  Sx.flag.go(S.Sx.args(x), name)

# (opacity 0) hides a model
def Model3.clear(+x: S.Sx) -> Bool:
  Bool.and(Bool.not(String.is_empty(S.Sx.attr(x, "opacity"))), F32.is_le(S.Sx.f(S.Sx.get(x, "opacity"), 0n), 0.0))

# an embedded file's checksum in the footprint's (embedded_files ...)
def Emb.sum(fs: List<&2, S.Sx>, +name: String) -> String:
  match fs:
    case Nil{}:
      ""
    case +f <> t:
      Bool.pick(String, String.eq(S.Sx.attr(f, "name"), name), S.Sx.attr(f, "checksum"), Emb.sum(t, name))

# "kicad-embed://NAME" gains "#CHECKSUM", so its extracted copy is found
# again without reading the board
def Model3.path(+p: String, +fs: List<&2, S.Sx>) -> String:
  Bool.pick(String, String.starts_with(p, "kicad-embed://"), p ++ "#" ++ Emb.sum(fs, String.drop(p, 14n)), p)

def Model3.one(hide: Bool, +x: S.Sx, +fx: F32, +fy: F32, +fa: F32, +back: Bool, +thick: F32, +fs: List<&2, S.Sx>, rest: List<&2, Got>) -> List<&2, Got>:
  match hide:
    case True{}:
      rest
    case False{}:
      GModel{Model3{Model3.path(S.Sx.arg(x, 0n), fs), Model3.matrix(fx, fy, fa, back, thick, x)}} <> rest

def Model3.all(ms: List<&2, S.Sx>, +fx: F32, +fy: F32, +fa: F32, +back: Bool, +thick: F32, +fs: List<&2, S.Sx>, rest: List<&2, Got>) -> List<&2, Got>:
  match ms:
    case Nil{}:
      rest
    case +x <> t:
      Model3.one(Bool.or(Sx.flag(x, "hide"), Model3.clear(x)), x, fx, fy, fa, back, thick, fs,
        Model3.all(t, fx, fy, fa, back, thick, fs, rest))

# Silkscreen text
# ---------------

def Silk.layer(+l: String) -> U32:
  Bool.pick(U32, String.eq(l, "F.SilkS"), 16, Bool.pick(U32, String.eq(l, "B.SilkS"), 3, 0))

def Just.h(+j: S.Sx) -> U32:
  Bool.pick(U32, Sx.flag(j, "left"), 0, Bool.pick(U32, Sx.flag(j, "right"), 2, 1))

def Just.v(+j: S.Sx) -> U32:
  Bool.pick(U32, Sx.flag(j, "top"), 0, Bool.pick(U32, Sx.flag(j, "bottom"), 2, 1))

# an angle in degrees into (-180, 180]
def Deg.norm(+a: F32) -> F32:
  F32.sub(a, F32.mul(360.0, F32.floor(F32.div(F32.add(a, 179.999), 360.0))))

# kept upright: text past vertical turns half round, its anchor sides swapped
def Up.flip(+a: F32) -> Bool:
  Bool.or(F32.is_gt(a, 90.001), F32.is_le(a, F32.neg(89.999)))

def Up.swap(+j: U32) -> U32:
  (2 - j : U32)

def Text.strokes(ps: List<&2, B.Prim>, +layer: U32, +key: String, +i: Nat, rest: List<&2, Got>) -> List<&2, Got>:
  match ps:
    case Nil{}:
      rest
    case p <> t:
      GText{B.Item{key ++ "/" ++ Nat.show(i), 0, layer, p, ""}} <> Text.strokes(t, layer, key, 1n+i, rest)

def Text.draw(+s: String, +at: B.Pt, +a: F32, +x: S.Sx, +up: Bool, +layer: U32, +key: String, rest: List<&2, Got>) -> List<&2, Got>:
  match at:
    case B.Pt{+px, +py}:
      +fx = S.Sx.get(x, "effects")
      +font = S.Sx.get(fx, "font")
      +j = S.Sx.get(fx, "justify")
      +h = S.Sx.f(S.Sx.get(font, "size"), 0n)
      +w = S.Sx.f(S.Sx.get(font, "size"), 1n)
      +th = S.Sx.f(S.Sx.get(font, "thickness"), 0n)
      +pen = Bool.pick(F32, F32.is_gt(th, 0.0), th, F32.div(h, 8.0))
      +na = Deg.norm(a)
      +turn = Bool.and(up, Up.flip(na))
      Text.strokes(ST.Stroke.text.full(s, px, py, h, w, pen, Deg.rad(Bool.pick(F32, turn, F32.add(na, 180.0), na)),
          Bool.pick(U32, turn, Up.swap(Just.h(j)), Just.h(j)), Bool.pick(U32, turn, Up.swap(Just.v(j)), Just.v(j)),
          Sx.flag(j, "mirror"), Sx.flag(font, "italic")),
        layer, key, 0n, rest)

# "${REFERENCE}" and "${VALUE}" as the footprint's
def Text.subst(+s: String, +ref: String, +val: String) -> String:
  Bool.pick(String, String.eq(s, "${REFERENCE}"), ref, Bool.pick(String, String.eq(s, "${VALUE}"), val, s))

def Text.fp.k(show: Bool, +s: String, +x: S.Sx, +pl: B.Place, +layer: U32, +key: String, rest: List<&2, Got>) -> List<&2, Got>:
  match show:
    case True{}:
      +at = S.Sx.get(x, "at")
      Text.draw(s, B.Geo.place(pl, B.Geo.pt(at)), S.Sx.f(at, 2n), x, True{}, layer, key, rest)
    case False{}:
      rest

# a footprint's property or fp_text, when it is visible on a silk layer
def Text.fp(+x: S.Sx, +pl: B.Place, +ref: String, +val: String, +key: String, rest: List<&2, Got>) -> List<&2, Got>:
  +prop = String.eq(S.Sx.name(x), "property")
  +s = Text.subst(S.Sx.arg(x, 1n), ref, val)
  +layer = Silk.layer(S.Sx.attr(x, "layer"))
  Text.fp.k(Bool.and(Bool.and(Bool.or(prop, String.eq(S.Sx.name(x), "fp_text")), U32.is_ne(layer, 0)),
      Bool.and(Bool.not(Bool.or(Sx.flag(x, "hide"), Sx.flag(S.Sx.get(x, "effects"), "hide"))), Bool.not(String.is_empty(s)))),
    s, x, pl, layer, key, rest)

def Text.fps(xs: List<&2, S.Sx>, +pl: B.Place, +ref: String, +val: String, +key: String, +i: Nat, rest: List<&2, Got>) -> List<&2, Got>:
  match xs:
    case Nil{}:
      rest
    case +x <> t:
      Text.fp(x, pl, ref, val, key ++ "/t" ++ Nat.show(i), Text.fps(t, pl, ref, val, key, 1n+i, rest))

def Text.gr.k(show: Bool, +x: S.Sx, +layer: U32, +key: String) -> List<&2, Got>:
  match show:
    case True{}:
      +at = S.Sx.get(x, "at")
      Text.draw(S.Sx.arg(x, 0n), B.Geo.pt(at), S.Sx.f(at, 2n), x, False{}, layer, key, Nil{})
    case False{}:
      Nil{}

# Forms
# -----

def Fp.got(+x: S.Sx, +thick: F32, +key: String) -> List<&2, Got>:
  +at = B.Fp.at(x)
  +fx = S.Sx.f(at, 0n)
  +fy = S.Sx.f(at, 1n)
  +fa = S.Sx.f(at, 2n)
  +args = S.Sx.args(x)
  Text.fps(args, B.Place{fx, fy, Deg.rad(fa)}, B.Info.prop(x, "Reference"), B.Info.prop(x, "Value"), key, 0n,
    Bool.pick(List<&2, Got>, Sx.flag(S.Sx.get(x, "attr"), "dnp"), Nil{},
      Model3.all(S.Sx.all(args, "model"), fx, fy, fa, String.eq(S.Sx.attr(x, "layer"), "B.Cu"), thick,
        S.Sx.all(S.Sx.args(S.Sx.get(x, "embedded_files")), "file"), Nil{})))

def Form.got.k(i: Nat, +x: S.Sx, +thick: F32, +key: String) -> List<&2, Got>:
  match i:
    case 0n:
      [GThick{S.Sx.f(S.Sx.get(x, "thickness"), 0n)}]
    case 1n:
      [GLook{Stack.look(S.Sx.get(x, "stackup"), Look.default())}]
    case 2n:
      Fp.got(x, thick, key)
    case 3n:
      Text.gr.k(U32.is_ne(Silk.layer(S.Sx.attr(x, "layer")), 0), x, Silk.layer(S.Sx.attr(x, "layer")), key)
    case _:
      Nil{}

def Form.names() -> List<&2, String>:
  ["general", "setup", "footprint", "gr_text"]

def Form.got(+x: S.Sx, +thick: F32, +i: Nat) -> List<&2, Got>:
  Form.got.k(B.Board.index(Form.names(), S.Sx.name(x)), x, thick, "t" ++ Nat.show(i))

# does a form's text start "(name"? (a cheap look before parsing it)
def Form.is.go(bs: List<&2, U32>, s: String) -> Bool:
  match bs s:
    case _ SNil{}:
      True{}
    case b <> bt SCon{Chr{c}, st}:
      Bool.and(U32.is_eq(b, c), Form.is.go(bt, st))
    case Nil{} _:
      False{}

# whether the head is a space or the opening parenthesis
def Form.sp(bs: List<&2, U32>) -> Bool:
  match bs:
    case +b <> _:
      Bool.or(U32.is_eq(b, 40), U32.is_le(b, 32))
    case Nil{}:
      False{}

def Form.skip(bs: List<&2, U32>, sp: Bool) -> List<&2, U32>:
  match bs sp:
    case _ <> +t True{}:
      Form.skip(t, Form.sp(t))
    case _ _:
      bs

def Form.wanted(+bs: List<&2, U32>) -> Bool:
  +h = Form.skip(bs, Form.sp(bs))
  Bool.or(Bool.or(Form.is.go(h, "footprint"), Form.is.go(h, "gr_text")), Bool.or(Form.is.go(h, "general"), Form.is.go(h, "setup")))

def Form.one.k(want: Bool, +bs: List<&2, U32>, +thick: F32, +i: Nat) -> List<&2, Got>:
  match want:
    case True{}:
      Form.got(S.Sx.parse(bs), thick, i)
    case False{}:
      Nil{}

# Parallel reading
# ----------------
# Forms are dealt alternately into two hands, as board.bend does, so the
# halves cost about the same.

type Hand is Data:
  Hand{a: List<&2, B.Form>, b: List<&2, B.Form>}

def Hand.deal(fs: List<&2, B.Form>, h: Hand) -> Hand:
  match fs h:
    case Nil{} _:
      h
    case f <> t Hand{a, b}:
      Hand.deal(t, Hand{b, f <> a})

def Hand.split(h: Hand, k: List<&2, B.Form> -> List<&2, B.Form> -> List<&2, Got>) -> List<&2, Got>:
  match h:
    case Hand{a, b}:
      k(a, b)

def Got.seq(fs: List<&2, B.Form>, +thick: F32) -> List<&2, Got>:
  match fs:
    case Nil{}:
      Nil{}
    case B.Form{+i, +bs} <> t:
      List.append(&2, Got, Form.one.k(Form.wanted(bs), bs, thick, i), Got.seq(t, thick))

def Got.par(d: Nat, fs: List<&2, B.Form>, +thick: F32) -> List<&2, Got>:
  match d:
    case 0n:
      Got.seq(fs, thick)
    case 1n+e0:
      +e = e0
      Hand.split(Hand.deal(fs, Hand{Nil{}, Nil{}}), a => b => (
        x y = Got.par(e, a, thick) Got.par(e, b, thick)
        List.append(&2, Got, x, y)))

def Thick.of(+x: S.Sx) -> F32:
  +t = S.Sx.f(S.Sx.get(x, "thickness"), 0n)
  Bool.pick(F32, F32.is_gt(t, 0.0), t, 1.6)

# the thickness is needed before the models can be placed: read it first
def Thick.go(fs: List<&2, List<&2, U32>>) -> F32:
  match fs:
    case Nil{}:
      1.6
    case +f <> t:
      Bool.pick(F32, Form.is.go(Form.skip(f, Form.sp(f)), "general"), Thick.of(S.Sx.parse(f)), Thick.go(t))

def Pcb3.fold(gs: List<&2, Got>, p: Pcb3) -> Pcb3:
  match gs p:
    case Nil{} _:
      p
    case GThick{_} <> t _:
      Pcb3.fold(t, p)
    case GLook{l} <> t Pcb3{th, _, tx, ms}:
      Pcb3.fold(t, Pcb3{th, l, tx, ms})
    case GText{i} <> t Pcb3{th, lk, tx, ms}:
      Pcb3.fold(t, Pcb3{th, lk, i <> tx, ms})
    case GModel{m} <> t Pcb3{th, lk, tx, ms}:
      Pcb3.fold(t, Pcb3{th, lk, tx, m <> ms})

def Pcb3.forms(+fs: List<&2, List<&2, U32>>) -> Pcb3:
  +thick = Thick.go(fs)
  Pcb3.fold(Got.par(6n, B.Board.number(fs, 0n), thick), Pcb3{thick, Look.default(), Nil{}, Nil{}})

def Pcb3.k(m: Maybe<&2, List<&2, List<&2, U32>>>) -> Maybe<&2, Pcb3>:
  match m:
    case Some{+fs}:
      Some{Pcb3.forms(fs)}
    case None{}:
      None{}

# the 3D facts of a board file, or None while the file is half written
def Pcb3.of(bs: List<&2, U32>) -> Maybe<&2, Pcb3>:
  Pcb3.k(S.Sx.forms(bs))

# Model files
# -----------

# ${NAME} in a path, replaced by what lookup(NAME) gives ("" leaves it)
type KV is Data:
  KV{k: String, v: String}

def Env.get(vs: List<&2, KV>, +k: String) -> String:
  match vs:
    case Nil{}:
      ""
    case KV{+a, +b} <> t:
      Bool.pick(String, String.eq(a, k), b, Env.get(t, k))

# reading a path: inside ${...}, just after a "$", the name so far, and
# the text so far
type ES is Data:
  ES{inv: Bool, dol: Bool, name: String, out: String}

def Env.chr(+c: U32) -> String:
  SCon{Chr{c}, SNil{}}

def Env.value(+v: String, +name: String) -> String:
  Bool.pick(String, String.is_empty(v), "${" ++ name ++ "}", v)

def Env.step.k(inv: Bool, dol: Bool, +c: U32, +name: String, +out: String, +vs: List<&2, KV>) -> ES:
  match inv dol:
    case True{} _:
      Bool.pick(ES, U32.is_eq(c, 125), ES{False{}, False{}, "", out ++ Env.value(Env.get(vs, name), name)}, ES{True{}, False{}, name ++ Env.chr(c), out})
    case False{} True{}:
      Bool.pick(ES, U32.is_eq(c, 123), ES{True{}, False{}, "", out}, ES{False{}, U32.is_eq(c, 36), "", out ++ "$" ++ Bool.pick(String, U32.is_eq(c, 36), "", Env.chr(c))})
    case False{} False{}:
      ES{False{}, U32.is_eq(c, 36), "", out ++ Bool.pick(String, U32.is_eq(c, 36), "", Env.chr(c))}

def Env.step(+c: U32, st: ES, +vs: List<&2, KV>) -> ES:
  match st:
    case ES{inv, dol, +name, +out}:
      Env.step.k(inv, dol, c, name, out, vs)

def Env.go(s: String, st: ES, +vs: List<&2, KV>) -> ES:
  match s:
    case SNil{}:
      st
    case SCon{Chr{+c}, t}:
      Env.go(t, Env.step(c, st, vs), vs)

def Env.fin(st: ES) -> String:
  match st:
    case ES{True{}, _, name, out}:
      out ++ "${" ++ name
    case ES{False{}, True{}, _, out}:
      out ++ "$"
    case ES{False{}, False{}, _, out}:
      out

# every ${NAME} in s replaced from vs (unknown names stay as they are)
def Env.sub(s: String, +vs: List<&2, KV>) -> String:
  Env.fin(Env.go(s, ES{False{}, False{}, "", ""}, vs))

# the models grouped by file, each with every transform it is placed by
type Group3 is Data:
  Group3{path: String, ms: List<&2, G.M4>}

def Group3.hit(gs: List<&2, Group3>, +path: String) -> Bool:
  match gs:
    case Group3{p, _} <> _:
      String.eq(p, path)
    case Nil{}:
      False{}

def Group3.add(gs: List<&2, Group3>, hit: Bool, +path: String, m: G.M4) -> List<&2, Group3>:
  match gs hit:
    case Group3{p, ms} <> t True{}:
      Group3{p, m <> ms} <> t
    case g <> +t False{}:
      g <> Group3.add(t, Group3.hit(t, path), path, m)
    case Nil{} _:
      [Group3{path, [m]}]

def Group3.all(ms: List<&2, Model3>, +gs: List<&2, Group3>) -> List<&2, Group3>:
  match ms:
    case Nil{}:
      gs
    case Model3{+path, m} <> t:
      Group3.all(t, Group3.add(gs, Group3.hit(gs, path), path, m))

def Pcb3.groups(p: Pcb3) -> List<&2, Group3>:
  match p:
    case Pcb3{_, _, _, ms}:
      Group3.all(ms, Nil{})

# a model's triangles (model mm) placed by m, onto acc
def Place.v(v: G.V3, +i: Nat) -> F32:
  match v:
    case G.V3{x, y, z}:
      Bool.pick(F32, Nat.is_eq(i, 0n), x, Bool.pick(F32, Nat.is_eq(i, 1n), y, z))

def Place.tri(+a: G.V3, +b: G.V3, +c: G.V3, +col: U32) -> G.Tri3:
  G.Tri3{Place.v(a, 0n), Place.v(a, 1n), Place.v(a, 2n), Place.v(b, 0n), Place.v(b, 1n), Place.v(b, 2n), Place.v(c, 0n), Place.v(c, 1n), Place.v(c, 2n), col}

def Place.tris(ts: List<&2, G.Tri3>, +m: G.M4, acc: List<&2, G.Tri3>) -> List<&2, G.Tri3>:
  match ts:
    case Nil{}:
      acc
    case G.Tri3{+ax, +ay, +az, +bx, +by, +bz, +cx, +cy, +cz, +col} <> t:
      Place.tris(t, m, Place.tri(G.M4.app(m, ax, ay, az), G.M4.app(m, bx, by, bz), G.M4.app(m, cx, cy, cz), col) <> acc)

# the model at every place it is used
def Place.all(ms: List<&2, G.M4>, +ts: List<&2, G.Tri3>, acc: List<&2, G.Tri3>) -> List<&2, G.Tri3>:
  match ms:
    case Nil{}:
      acc
    case +m <> t:
      Place.all(t, ts, Place.tris(ts, m, acc))
