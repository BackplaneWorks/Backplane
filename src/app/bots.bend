import Base
import ./side_row_types.bend as ST
import ../core/json.bend as J
import ../core/i18n.bend as I
import ../core/msgs.bend as MSG
import ../core/model.bend as M
import ../core/client.bend as U
import ../core/bot.bend as BT
import ../core/botui.bend as BU
import ../core/cron.bend as CR
import ../core/sched.bend as SC
import ../core/tz.bend as TZ
import ../core/cat.bend as CT
import ../gfx/raster.bend as R
import ../gfx/draw.bend as D
import ./theme.bend as T
import ./nui.bend as S
import ./layout.bend as L
import ./cats.bend as CA
import ./forms.bend as F
import ./space.bend as SV
import ./plane.bend as PV
import ./mailroom.bend as MR
import ./mm.bend as MM
import ../core/mailview.bend as MV
import ../core/plane.bend as PC
import ../core/uber.bend as UB
import ../core/task.bend as TK
import ../core/eq.bend as Q
import ../gfx/tiles.bend as TI

# The bots in the window (docs/bots.md): their cats under the projects in
# the sidebar, the rooms, and the main area for a bot (its tabs), a room or
# a bot on another machine. What is shown comes from core/botui.bend; the
# actions are the client's (client.bend's Bots section). Chosen by match,
# so only the screen on show is laid out.

def Bv.sep() -> String:
  SCon{Chr{31}, SNil{}}

# Sidebar
# -------

# a section's name and its + button
def Bv.head(+ty: L.Ty, +c: D.Clip, +label: String, action: String, +on: Bool, +y: U32) -> L.Out:
  L.Out{List.append(&2, R.Op, L.Ty.text(ty, 2n, label, c, 16, y, 28, T.Th.faint()), L.Lay.icon(ty, Bool.pick(String, on, "x", "plus"), c, 236, (y + 6 : U32), T.Th.faint())),
    [L.Lay.hit(228, y, 30, 28, action, "")]}

def Bv.note.color(+mood: String) -> U32:
  Bool.pick(U32, String.eq(mood, "wait"), T.Th.warn(), Bool.pick(U32, String.eq(mood, "error"), T.Th.bad(), T.Th.faint()))

# A cat on show is drawn by the window (main.bend's Catv), not here: the
# layout leaves an anchor where it sits (an op that draws nothing, its
# colour tagged with the cat's place) and a hit that names it ("cat", of
# no area, so the pointer never finds it). A frame draws the cats at their
# anchors; a cat frame poses them again without laying the frame out.
type Spot is Data:
  Spot{look: Nat, mood: String, px: U32, x: U32, y: U32, c: D.Clip}

# the place an anchor's colour carries in its low 24 bits
def Spot.key(+x: U32, +y: U32) -> U32:
  U32.or(U32.shln(U32.and(x, 4095), 12n), U32.and(y, 4095))

# a colour's top byte 64 marks a cat's anchor (128 and up: a caret,
# draw.bend's Draw.caret)
def Spot.is(+col: U32) -> Bool:
  U32.is_eq(U32.shrn(col, 24n), 64)

def Spot.anchor(+c: D.Clip, +x: U32, +y: U32, +px: U32) -> R.Op:
  D.Clip.cut(c, x, y, (x + px : U32), (y + px : U32), U32.or(U32.shln(64, 24n), Spot.key(x, y)), 0, R.SRect{})

def Spot.hit(+look: Nat, +mood: String, c: D.Clip, +x: U32, +y: U32, +px: U32) -> S.Hit:
  match c:
    case D.Clip{+cx0, +cy0, +cx1, +cy1}:
      L.Lay.hit(0, 0, 0, 0, "cat", Nat.show(look) ++ "|" ++ mood ++ "|" ++ U32.show(px) ++ "|" ++ U32.show(x) ++ "|" ++ U32.show(y)
        ++ "|" ++ U32.show(cx0) ++ "|" ++ U32.show(cy0) ++ "|" ++ U32.show(cx1) ++ "|" ++ U32.show(cy1))

def Spot.part(+xs: List<&2, String>, +i: Nat) -> String:
  Maybe.default(&2, String, List.get(&2, String, xs, i), "")

def Spot.u32(+xs: List<&2, String>, +i: Nat) -> U32:
  Maybe.default(&2, U32, U32.read(Spot.part(xs, i)), 0)

def Spot.of.parts(+xs: List<&2, String>) -> Spot:
  Spot{Maybe.default(&2, Nat, Nat.read(Spot.part(xs, 0n)), 0n), Spot.part(xs, 1n), Spot.u32(xs, 2n), Spot.u32(xs, 3n), Spot.u32(xs, 4n),
    D.Clip{Spot.u32(xs, 5n), Spot.u32(xs, 6n), Spot.u32(xs, 7n), Spot.u32(xs, 8n)}}

def Spot.of(+v: String) -> Spot:
  Spot.of.parts(String.split(v, '|'))

# a - b, or 0 below it
def Spot.less(+a: U32, +b: U32) -> U32:
  Bool.pick(U32, U32.is_ge(a, b), (a - b : U32), 0)

# a cat whose square (and as much again around it) is cut away by
# its clip draws nothing: it is not posed at all (a bot row scrolled out
# of the sidebar)
def Spot.seen(sp: Spot) -> Bool:
  match sp:
    case Spot{_, _, +px, +x, +y, D.Clip{+cx0, +cy0, +cx1, +cy1}}:
      Bool.and(Bool.and(U32.is_lt(U32.max(Spot.less(x, px), cx0), U32.min((x + px + px : U32), cx1)), U32.is_lt(U32.max(Spot.less(y, px), cy0), U32.min((y + px + px : U32), cy1))), U32.is_gt(px, 0))

def Spot.keep(seen: Bool, sp: Spot, rest: List<&2, Spot>) -> List<&2, Spot>:
  match seen:
    case True{}:
      sp <> rest
    case False{}:
      rest

def Spot.take.k(cat: Bool, +v: String, rest: List<&2, Spot>) -> List<&2, Spot>:
  match cat:
    case True{}:
      Spot.keep(Spot.seen(Spot.of(v)), Spot.of(v), rest)
    case False{}:
      rest

# the cats a frame's hits name, in order
def Spot.all(hs: List<&2, S.Hit>) -> List<&2, Spot>:
  match hs:
    case Nil{}:
      Nil{}
    case S.Hit{_, _, _, _, +a, +v} <> t:
      Spot.take.k(String.eq(a, "cat"), v, Spot.all(t))

# a cat's ops posed from the cache (CA.Cats.draw, as the layout drew them)
def Spot.ops(sp: Spot, +cache: Map<&2, CA.CatF>, +ms: Nat) -> List<&2, R.Op>:
  match sp:
    case Spot{+look, +mood, +px, +x, +y, c}:
      CA.Cats.draw(cache, look, mood, ms, c, x, y, px)

def Spot.pick(hit: Bool, sp: Spot, m: Maybe<&2, Spot>) -> Maybe<&2, Spot>:
  match hit:
    case True{}:
      Some{sp}
    case False{}:
      m

# the cat whose anchor has key k
def Spot.has(sps: List<&2, Spot>, +k: U32) -> Maybe<&2, Spot>:
  match sps:
    case Nil{}:
      None{}
    case Spot{lk, md, +px, +x, +y, c} <> t:
      Spot.pick(U32.is_eq(Spot.key(x, y), k), Spot{lk, md, px, x, y, c}, Spot.has(t, k))

def Spot.at(m: Maybe<&2, Spot>, +cache: Map<&2, CA.CatF>, +ms: Nat) -> List<&2, R.Op>:
  match m:
    case Some{sp}:
      Spot.ops(sp, cache, ms)
    case None{}:
      Nil{}

def Spot.inline.k(cat: Bool, o: R.Op, +sps: List<&2, Spot>, +cache: Map<&2, CA.CatF>, +ms: Nat, rest: List<&2, R.Op>) -> List<&2, R.Op>:
  match cat o:
    case True{} R.Op{x0, y0, x1, y1, +col, a, s}:
      R.Op{x0, y0, x1, y1, col, a, s} <> TI.Ops.cat(Spot.at(Spot.has(sps, U32.and(col, 16777215)), cache, ms), rest)
    case False{} _:
      o <> rest

# the ops with each cat drawn at its anchor (where the layout put it)
def Spot.inline(ops: List<&2, R.Op>, +sps: List<&2, Spot>, +cache: Map<&2, CA.CatF>, +ms: Nat) -> List<&2, R.Op>:
  match ops:
    case Nil{}:
      Nil{}
    case R.Op{x0, y0, x1, y1, +col, a, s} <> t:
      Spot.inline.k(Spot.is(col), R.Op{x0, y0, x1, y1, col, a, s}, sps, cache, ms, Spot.inline(t, sps, cache, ms))

# where a cat's ops reach (key: its anchor's)
type Box is Data:
  Box{key: U32, x0: U32, y0: U32, x1: U32, y1: U32}

def Box.grow(os: List<&2, R.Op>, b: Box) -> Box:
  match os b:
    case Nil{} _:
      b
    case R.Op{+x0, +y0, +x1, +y1, _, _, _} <> t Box{k, +bx0, +by0, +bx1, +by1}:
      Box.grow(t, Box{k, U32.min(x0, bx0), U32.min(y0, by0), U32.max(x1, bx1), U32.max(y1, by1)})

def Box.of(sp: Spot, +cache: Map<&2, CA.CatF>, +ms: Nat) -> Box:
  match sp:
    case Spot{lk, md, +px, +x, +y, c}:
      Box.grow(Spot.ops(Spot{lk, md, px, x, y, c}, cache, ms), Box{Spot.key(x, y), 65535, 65535, 0, 0})

def Box.all(sps: List<&2, Spot>, +cache: Map<&2, CA.CatF>, +ms: Nat) -> List<&2, Box>:
  match sps:
    case Nil{}:
      Nil{}
    case sp <> t:
      Box.of(sp, cache, ms) <> Box.all(t, cache, ms)

def Box.meets(b: Box, +x0: U32, +y0: U32, +x1: U32, +y1: U32) -> Bool:
  match b:
    case Box{_, +bx0, +by0, +bx1, +by1}:
      Bool.and(Bool.and(U32.is_lt(x0, bx1), U32.is_lt(bx0, x1)), Bool.and(U32.is_lt(y0, by1), U32.is_lt(by0, y1)))

def Box.any(bs: List<&2, Box>, +x0: U32, +y0: U32, +x1: U32, +y1: U32) -> Bool:
  match bs:
    case Nil{}:
      False{}
    case b <> t:
      Bool.or(Box.meets(b, x0, y0, x1, y1), Box.any(t, x0, y0, x1, y1))

def Box.drop.k(same: Bool, b: Box, rest: List<&2, Box>) -> List<&2, Box>:
  match same:
    case True{}:
      rest
    case False{}:
      b <> rest

def Box.drop(bs: List<&2, Box>, +k: U32) -> List<&2, Box>:
  match bs:
    case Nil{}:
      Nil{}
    case Box{+bk, b0, b1, b2, b3} <> t:
      Box.drop.k(U32.is_eq(bk, k), Box{bk, b0, b1, b2, b3}, Box.drop(t, k))

# an op (not an anchor) over a cat still to come
def Spot.over(cat: Bool, bs: List<&2, Box>, +x0: U32, +y0: U32, +x1: U32, +y1: U32) -> Bool:
  match cat:
    case True{}:
      False{}
    case False{}:
      Box.any(bs, x0, y0, x1, y1)

# past an anchor, its cat is no longer to come
def Spot.left(cat: Bool, bs: List<&2, Box>, +col: U32) -> List<&2, Box>:
  match cat:
    case True{}:
      Box.drop(bs, U32.and(col, 16777215))
    case False{}:
      bs

# something is drawn over a cat: an op above its anchor (ops are topmost
# first) reaches where the cat draws (bs: the cats whose anchors are yet
# to come)
def Spot.covered(ops: List<&2, R.Op>, bs: List<&2, Box>) -> Bool:
  match ops bs:
    case _ Nil{}:
      False{}
    case Nil{} _ <> _:
      False{}
    case R.Op{+x0, +y0, +x1, +y1, +col, _, _} <> t +b <> +bt:
      Bool.or(Spot.over(Spot.is(col), b <> bt, x0, y0, x1, y1), Spot.covered(t, Spot.left(Spot.is(col), b <> bt, col)))

def Spot.all.ops(sps: List<&2, Spot>, +cache: Map<&2, CA.CatF>, +ms: Nat) -> List<&2, R.Op>:
  match sps:
    case Nil{}:
      Nil{}
    case sp <> t:
      TI.Ops.cat(Spot.ops(sp, cache, ms), Spot.all.ops(t, cache, ms))

# a frame's cats: their ops, kept for cat frames (in front of the rest,
# as Fronted.ops has them), or none when the cats are drawn in place
type Fronted is Data:
  Fronted{ops: List<&2, R.Op>, cats: List<&2, Spot>, cs: List<&2, R.Op>}

def Fronted.k(front: Bool, +ops: List<&2, R.Op>, +sps: List<&2, Spot>, +cs: List<&2, R.Op>, +cache: Map<&2, CA.CatF>, +ms: Nat) -> Fronted:
  match front:
    case True{}:
      Fronted{TI.Ops.cat(cs, ops), sps, cs}
    case False{}:
      Fronted{Spot.inline(ops, sps, cache, ms), Nil{}, Nil{}}

# the cats drawn into a frame's ops: in front of the rest when nothing is
# drawn over one (the same pixels; cat frames then change only them), else
# each at its anchor
def Fronted.of(+ops: List<&2, R.Op>, +hs: List<&2, S.Hit>, +cache: Map<&2, CA.CatF>, +ms: Nat) -> Fronted:
  +sps = Spot.all(hs)
  Fronted.k(Bool.and(Bool.not(List.is_empty(&2, Spot, sps)), Bool.not(Spot.covered(ops, Box.all(sps, cache, ms)))), ops, sps, Spot.all.ops(sps, cache, ms), cache, ms)

# a frame laid out with its cats drawn in place (for frames made without
# the window: snapshots, benches)
def Bv.with_cats(o: L.Out, +cache: Map<&2, CA.CatF>, +ms: Nat) -> L.Out:
  match o:
    case L.Out{+ops, +hs}:
      L.Out{Spot.inline(ops, Spot.all(hs), cache, ms), hs}

# a cat, as the layout leaves it
def Bv.cat(+look: Nat, +mood: String, +c: D.Clip, +x: U32, +y: U32, +px: U32) -> L.Out:
  L.Out{[Spot.anchor(c, x, y, px)], [Spot.hit(look, mood, c, x, y, px)]}

# a cat's row: the cat, its name (with its machine when elsewhere), and
# its mood; choosing it does what BU.act says
def Bv.row(+u: U.Ui, +ty: L.Ty, +cats: Map<&2, CA.CatF>, +ms: Nat, +r: BU.BRow, +c: D.Clip, +y: U32) -> L.Out:
  match r:
    case BU.BRow{+id, _, _, +look, +mood, _, +sel, _, _, _}:
      +note = {BU.Mood.note.loc(L.Ty.loc(ty), mood) : String}
      +badge = Bool.pick(String, Nat.is_eq(MV.Badge.n(U.Mailv.badge(u, id)), 0n), "", " · " ++ MV.Badge.text(U.Mailv.badge(u, id)))
      +nw = L.Ty.width(ty, 2n, note ++ badge)
      L.Out{List.concat(&2, R.Op, [
          Bool.pick(List<&2, R.Op>, sel, [D.Draw.rect(c, 6, (y + 1 : U32), 252, 34, T.Th.raise(), 255), D.Draw.rect(c, 6, (y + 1 : U32), 2, 34, T.Th.accent(), 255)], Nil{}),
          Spot.anchor(c, 14, (y + 4 : U32), 28) <> Nil{},
          L.Ty.text(ty, 0n, L.Ty.fit(ty, 0n, BU.BU.row.title(r), L.U32.dim(192, nw)), c, 50, y, 36, Bool.pick(U32, sel, T.Th.text(), T.Th.soft())),
          L.Ty.text(ty, 2n, note ++ badge, c, (252 - nw : U32), y, 36, Bv.note.color(mood))]),
        [L.Lay.hit(0, y, 264, 36, BU.BU.act(r), id), Spot.hit(look, mood, c, 14, (y + 4 : U32), 28)]}

def Bv.rows(rs: List<&2, BU.BRow>, +u: U.Ui, +ty: L.Ty, +cats: Map<&2, CA.CatF>, +ms: Nat, +c: D.Clip, +y: U32) -> L.Col:
  match rs:
    case Nil{}:
      L.Col{L.Out.none(), y}
    case r <> t:
      L.Lay.col.cat(Bv.row(u, ty, cats, ms, r, c, y), Bv.rows(t, u, ty, cats, ms, c, (y + 36 : U32)))

# the provider chips of the new-bot form (the default one until picked)
def Bv.provs(ps: List<&2, String>, +ty: L.Ty, +cur: String, +c: D.Clip, +x: U32, +y: U32) -> L.Out:
  match ps:
    case Nil{}:
      L.Out.none()
    case +p <> t:
      L.Out.cat(F.Fm.chip(ty, c, x, y, p, String.eq(p, cur), "bfield", "provider" ++ Bv.sep() ++ p), Bv.provs(t, ty, cur, c, (x + L.Lay.button.w(ty, p) + 6 : U32), y))

def Bv.prov(+u: U.Ui) -> String:
  Bool.pick(String, String.is_empty(BU.BU.field(u, "provider")), M.Setting.get(U.Ui.state(u), "agent.provider", "claude"), BU.BU.field(u, "provider"))

# a new bot: its name, who it is, which agent runs it
def Bv.bform(open: Bool, +ty: L.Ty, +f: F.FS, +c: D.Clip, +y: U32) -> L.Col:
  match open:
    case True{}:
      L.Col{L.Out.all([
          F.Fm.field(ty, f, c, 12, y, 240, "name", "bot-create", "", U.Bots.name.hint.loc(L.Ty.loc(ty), F.FS.ui(f))),
          F.Fm.area(ty, f, c, 12, (y + 34 : U32), 240, 3n, "persona", "bot-create", "", MSG.T.bots.persona.placeholder(L.Ty.loc(ty))),
          Bv.provs(["claude", "codex", "grok", "muse", "antigravity"], ty, Bv.prov(F.FS.ui(f)), c, 12, (y + 112 : U32)),
          F.Fm.button(ty, c, 12, (y + 146 : U32), MSG.T.common.create(L.Ty.loc(ty)), T.Th.accent(), "bot-create", "")]),
        (y + 182 : U32)}
    case False{}:
      L.Col{L.Out.none(), y}

def Bv.picks(ps: List<&2, BU.Pick>, +ty: L.Ty, +c: D.Clip, +x: U32, +y: U32) -> L.Col:
  match ps:
    case Nil{}:
      L.Col{L.Out.none(), (y + 34 : U32)}
    case BU.Pick{+id, +name, +on} <> t:
      +w = L.Lay.button.w(ty, name)
      +wrap = Bool.and(U32.is_gt(x, 12), U32.is_gt((x + w : U32), 252))
      +cx = {Bool.pick(U32, wrap, 12, x) : U32}
      +cy = {Bool.pick(U32, wrap, (y + 34 : U32), y) : U32}
      L.Lay.col.cat(F.Fm.chip(ty, c, cx, cy, name, on, "room-pick", id), Bv.picks(t, ty, c, (cx + w + 6 : U32), cy))

# a new room: its name and the bots in it
def Bv.rform(open: Bool, +ty: L.Ty, +f: F.FS, +c: D.Clip, +y: U32) -> L.Col:
  match open:
    case True{}:
      L.Lay.col.cat(F.Fm.field(ty, f, c, 12, y, 240, "rname", "room-save", "", MSG.T.bots.room.name(L.Ty.loc(ty))),
        L.Lay.col.then(Bv.picks(BU.BU.picks(F.FS.ui(f)), ty, c, 12, (y + 34 : U32)), +y2 =>
          L.Col{F.Fm.button(ty, c, 12, y2, MSG.T.common.save(L.Ty.loc(ty)), T.Th.accent(), "room-save", ""), (y2 + 36 : U32)}))
    case False{}:
      L.Col{L.Out.none(), y}

def Bv.rrow(+ty: L.Ty, r: BU.RRow, +c: D.Clip, +y: U32) -> L.Out:
  match r:
    case BU.RRow{+id, +name, +k, +sel}:
      +cnt = Nat.show(k)
      L.Out{List.concat(&2, R.Op, [
          Bool.pick(List<&2, R.Op>, sel, [D.Draw.rect(c, 6, (y + 1 : U32), 252, 30, T.Th.raise(), 255), D.Draw.rect(c, 6, (y + 1 : U32), 2, 30, T.Th.accent(), 255)], Nil{}),
          L.Ty.text(ty, 0n, "#", c, 20, y, 32, T.Th.faint()),
          L.Ty.text(ty, 0n, L.Ty.fit(ty, 0n, name, 180), c, 38, y, 32, Bool.pick(U32, sel, T.Th.text(), T.Th.soft())),
          L.Ty.text(ty, 2n, cnt, c, (252 - L.Ty.width(ty, 2n, cnt) : U32), y, 32, T.Th.faint())]),
        [L.Lay.hit(0, y, 264, 32, "room", id)]}

def Bv.rrows(rs: List<&2, BU.RRow>, +ty: L.Ty, +c: D.Clip, +y: U32) -> L.Col:
  match rs:
    case Nil{}:
      L.Col{L.Out.none(), (y + 12 : U32)}
    case r <> t:
      L.Lay.col.cat(Bv.rrow(ty, r, c, y), Bv.rrows(t, ty, c, (y + 32 : U32)))

# the sidebar under the projects: a rule, the cats, then the rooms
# what the sidebar's bots section needs, gathered before the sidebar's
# layout calls it (one value, so the call back holds little)
# the cats' rows made once for a frame (BU.rows and the uberbot's), for
# the stamps, the cats' cache and the sidebar alike (main.bend's Shell)
type BRows is Data:
  BRows{rows: List<&2, BU.BRow>, up: Maybe<&2, BU.BRow>}

def BRows.of(+u: U.Ui) -> BRows:
  BRows{BU.BU.rows(u), BU.BU.uber(u)}

def BRows.rows(b: BRows) -> List<&2, BU.BRow>:
  match b:
    case BRows{rs, _}:
      rs

def BRows.up(b: BRows) -> Maybe<&2, BU.BRow>:
  match b:
    case BRows{_, u}:
      u

type Sd is Data:
  Sd{ty: L.Ty, cats: Map<&2, CA.CatF>, f: F.FS, ms: Nat, bn: Bool, rn: Bool, rows: List<&2, BU.BRow>, rooms: List<&2, BU.RRow>, up: Maybe<&2, BU.BRow>, cn: String}

def Bv.sd(+ty: L.Ty, +cats: Map<&2, CA.CatF>, +n: S.Nui) -> Sd:
  Sd{ty, cats, F.Fm.fs(n), S.Nui.ms_of(n), BU.BU.open(S.Nui.ui(n), "@bnew"), BU.BU.open(S.Nui.ui(n), "@rnew"), BU.BU.rows(S.Nui.ui(n)), BU.BU.rooms(S.Nui.ui(n)), BU.BU.uber(S.Nui.ui(n)), ""}

def Bv.side.k(+ty: L.Ty, +cats: Map<&2, CA.CatF>, +f: F.FS, +ms: Nat, +bn: Bool, +rn: Bool, rows: List<&2, BU.BRow>, rooms: List<&2, BU.RRow>, +c: D.Clip, +y: U32) -> L.Col:
  L.Lay.col.cat(L.Out.cat(L.Out.ops([D.Draw.rect(c, 12, (y + 4 : U32), 240, 1, T.Th.line(), 255)]), Bv.head(ty, c, MSG.T.tour.title.bots(L.Ty.loc(ty)), "bot-new", bn, (y + 10 : U32))),
    L.Lay.col.then(Bv.bform(bn, ty, f, c, (y + 40 : U32)), y1 =>
      L.Lay.col.then(Bv.rows(rows, F.FS.ui(f), ty, cats, ms, c, y1), +y2 =>
        L.Lay.col.cat(Bv.head(ty, c, MSG.T.bots.rooms(L.Ty.loc(ty)), "room-new", rn, (y2 + 8 : U32)),
          L.Lay.col.then(Bv.rform(rn, ty, f, c, (y2 + 38 : U32)), y3 =>
            Bv.rrows(rooms, ty, c, y3))))))

# the sidebar under the projects: a rule, the cats, then the rooms
def Bv.side.of(sd: Sd, +c: D.Clip, +y: U32) -> L.Col:
  match sd:
    case Sd{+ty, +cats, +f, +ms, +bn, +rn, rows, rooms, _, _}:
      Bv.side.k(ty, cats, f, ms, bn, rn, rows, rooms, c, y)

def Bv.side(+ty: L.Ty, +cats: Map<&2, CA.CatF>, +n: S.Nui, +c: D.Clip, +y: U32) -> L.Col:
  Bv.side.of(Bv.sd(ty, cats, n), c, y)

# The uberbot's row above the search field (nothing when there is none):
# its cat, its name and, under it, what waits on the person and what runs
def Bv.uber.h() -> U32:
  48

def Bv.uber.top(sd: Sd) -> U32:
  match sd:
    case Sd{_, _, _, _, _, _, _, _, Some{_}, _}:
      Bv.uber.h()
    case _:
      0

# the selected cat is also drawn large at the top of its view
def Bv.warm.sel(sel: Bool, c: Map<&2, CA.CatF>, +look: Nat, +mood: String, +clock: Nat) -> Map<&2, CA.CatF>:
  match sel:
    case True{}:
      CA.Cats.warm(c, look, mood, 64, clock)
    case False{}:
      c

# the clock a cat is posed at: a cat that holds still (asleep, ready,
# failed, away: CA.Cats.moving) always at 0, so it is posed once, not on
# every tick another cat moves
def Bv.warm.at(+mood: String, +clock: Nat) -> Nat:
  Bool.pick(Nat, CA.Cats.moving(mood), clock, 0n)

# Layouts may request a mood/size beyond the sidebar and selected header.
# Warm exactly those visible requests and retain them for subsequent frames.
def Spot.warm(sps: List<&2, Spot>, c: Map<&2, CA.CatF>, +clock: Nat) -> Map<&2, CA.CatF>:
  match sps:
    case Nil{}: c
    case Spot{look, +mood, px, x, y, _} <> t:
      Spot.warm(t, CA.Cats.visible(c, look, mood, px, Bv.warm.at(mood, clock), x, y), clock)

# the cats on show, posed into the cache at the clock before a frame
def Bv.warm.go(rs: List<&2, BU.BRow>, c: Map<&2, CA.CatF>, +clock: Nat) -> Map<&2, CA.CatF>:
  match rs:
    case Nil{}:
      c
    case BU.BRow{_, _, _, +look, +mood, _, sel, _, _, _} <> t:
      +at = Bv.warm.at(mood, clock)
      Bv.warm.go(t, Bv.warm.sel(sel, CA.Cats.warm(c, look, mood, 28, at), look, mood, at), clock)

def Bv.uber.rows(m: Maybe<&2, BU.BRow>) -> List<&2, BU.BRow>:
  match m:
    case Some{r}:
      [r]
    case None{}:
      Nil{}

def Bv.warm.r(+br: BRows, c: Map<&2, CA.CatF>, +clock: Nat) -> Map<&2, CA.CatF>:
  Bv.warm.go(List.append(&2, BU.BRow, Bv.uber.rows(BRows.up(br)), BRows.rows(br)), c, clock)

def Bv.warm(+u: U.Ui, c: Map<&2, CA.CatF>, +clock: Nat) -> Map<&2, CA.CatF>:
  Bv.warm.r(BRows.of(u), c, clock)

def Bv.info.nat(+n: S.Nui, +k: String) -> Nat:
  Maybe.default(&2, Nat, Nat.read(U.Ui.info(S.Nui.ui(n), k)), 0n)

# how long cats go on moving after the person last touched the window
def Bv.idle() -> Nat:
  60000n

# someone is at the window: input ("act.at", main.bend's Shell.acted)
# within Bv.idle. A focused window on an unattended desktop keeps the keys
# for hours, and a cat drawn 30 times a second there starves the hub.
def Bv.present(+n: S.Nui, +ms: Nat) -> Bool:
  Nat.is_le(Nat.sub(ms, Bv.info.nat(n, "act.at")), Bv.idle())

# a shown cat moves, the window has the keys and someone is there
def Bv.awake.r(+n: S.Nui, +ms: Nat, +br: BRows) -> Bool:
  Bool.and(Bool.and(Bool.and(S.Nui.has_focus(n), Bool.not(S.Lbx.on(n))), Bv.present(n, ms)), Bool.or(BU.BU.moving.go(BRows.rows(br)), BU.BU.moving.go(Bv.uber.rows(BRows.up(br)))))

def Bv.awake(+n: S.Nui, +ms: Nat) -> Bool:
  Bv.awake.r(n, ms, BRows.of(S.Nui.ui(n)))

# the cats hold still while the person types, scrolls or points: a cat
# frame is a whole frame, and 30 of them a second took the cores keys and
# the sidebar's wheel wait for
def Bv.calm() -> Nat:
  1000n

# how long until the cats move again, input last at `at` (0: they may now)
def Bv.calm.after(+ms: Nat, +at: Nat) -> Nat:
  Nat.sub(Bv.calm(), Nat.sub(ms, at))

def Bv.calm.left(+n: S.Nui, +ms: Nat) -> Nat:
  Bv.calm.after(ms, Bv.info.nat(n, "act.at"))

# frames for the cats: awake, and no input for Bv.calm
def Bv.moving.r(+n: S.Nui, +ms: Nat, +br: BRows) -> Bool:
  Bool.and(Bv.awake.r(n, ms, br), Nat.is_eq(Bv.calm.left(n, ms), 0n))

def Bv.moving(+n: S.Nui, +ms: Nat) -> Bool:
  Bv.moving.r(n, ms, BRows.of(S.Nui.ui(n)))

# the cats' clock ("cat.clock"): a frame at or past the next cat frame
# ("cat.due", main.bend's Shell.cats) while a cat moves sets it to its
# time; any other frame (a key typed) keeps it, so the cats' ops stay as
# they were and no tile of them is drawn again
def Bv.clock.r(+n: S.Nui, +ms: Nat, +br: BRows) -> Nat:
  Bool.pick(Nat, Bool.and(Bv.moving.r(n, ms, br), Nat.is_ge(ms, Bv.info.nat(n, "cat.due"))), ms, Bv.info.nat(n, "cat.clock"))

def Bv.clock(+n: S.Nui, +ms: Nat) -> Nat:
  Bv.clock.r(n, ms, BRows.of(S.Nui.ui(n)))

def Bv.stamp.r(+n: S.Nui, +ms: Nat, +br: BRows) -> S.Nui:
  S.Nui.with_ui(n, U.Ui.info_set(U.Ui.info_set(S.Nui.ui(n), "cat.clock", Nat.show(Bv.clock.r(n, ms, br))), "local.mailroom", "on"))

def Bv.stamp(+n: S.Nui, +ms: Nat) -> S.Nui:
  Bv.stamp.r(n, ms, BRows.of(S.Nui.ui(n)))

def Bv.clock.of(+n: S.Nui) -> Nat:
  Bv.info.nat(n, "cat.clock")

# Panes
# -----
# A tab other than Chat is a column under the bot's header that scrolls
# on the wheel ("@set.sc", its hit "set-bg" says how far it may go).

def Bv.hy() -> U32:
  88

def Bv.pane.w(+x0: U32, +x1: U32) -> U32:
  U32.min((x1 - x0 - 64 : U32), 760)

def Bv.pane.x(+x0: U32, +x1: U32) -> U32:
  (x0 + U32.shr((x1 - x0 - Bv.pane.w(x0, x1) : U32)) : U32)

def Bv.off(+u: U.Ui) -> U32:
  Maybe.default(&2, U32, U32.read(U.Ui.scratch(u, "@set.sc")), 0)

# a pane's column laid out from top - off, with its scroll region
def Bv.pane(col: L.Col, +x0: U32, +x1: U32, +top: U32, +off: U32, +h: U32) -> L.Out:
  match col:
    case L.Col{o, +y}:
      L.Out.cat(L.Out{Nil{}, [L.Lay.hit(x0, top, (x1 - x0 : U32), (h - top : U32), "set-bg", U32.show(L.U32.dim((y + off + 40 : U32), h))), L.Lay.hit(0, 0, 0, 0, "", "")]},
        L.Out.hits_below(top, o))

def Bv.h2(+ty: L.Ty, +c: D.Clip, +s: String, +x: U32, +y: U32) -> L.Out:
  L.Out.ops(L.Ty.text(ty, 1n, s, c, x, y, 28, T.Th.text()))

def Bv.faint(+ty: L.Ty, +c: D.Clip, +s: String, +x: U32, +w: U32, +y: U32) -> L.Out:
  L.Out.ops(L.Ty.text(ty, 2n, L.Ty.fit(ty, 2n, s, w), c, x, y, 20, T.Th.faint()))

# lines of wrapped chat text
def Bv.lines(ls: List<&2, String>, +ty: L.Ty, +c: D.Clip, +x: U32, +y: U32, +color: U32) -> List<&2, R.Op>:
  match ls:
    case Nil{}:
      Nil{}
    case l <> t:
      List.append(&2, R.Op, L.Ty.text(ty, 3n, l, c, x, y, 22, color), Bv.lines(t, ty, c, x, (y + 22 : U32), color))

def Bv.nl(+ls: List<&2, String>) -> U32:
  U32.from_nat(List.length(&2, String, ls))

# Memory
# ------

# o when on, else nothing
def Bv.when(on: Bool, o: L.Out) -> L.Out:
  match on:
    case True{}:
      o
    case False{}:
      L.Out.none()

# a memory; Forget only for a bot here (BU.owns)
def Bv.mem(+ty: L.Ty, m: BT.Mem, +own: Bool, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  match m:
    case BT.Mem{_, +key, +kind, +text, +tags, _, _}:
      +label0 = MSG.T.bots.memory.forget(L.Ty.loc(ty))
      +kw = L.Ty.width(ty, 2n, kind)
      +fw = {Bool.pick(U32, own, L.Lay.button.w(ty, label0), 0) : U32}
      +ls = L.Ty.wrap(ty, 3n, text, (w - 24 : U32))
      +th = {Bool.pick(U32, String.is_empty(tags), 0, 20) : U32}
      +h = {(48 + Bv.nl(ls) * 22 + th : U32) : U32}
      L.Col{L.Out.all([
          L.Out.ops(L.Lay.box(c, x, y, w, h, 0, T.Th.panel(), T.Th.line())),
          L.Out.ops(L.Ty.text(ty, 0n, L.Ty.fit(ty, 0n, key, L.U32.dim(w, (kw + fw + 48 : U32))), c, (x + 12 : U32), (y + 6 : U32), 28, T.Th.text())),
          L.Out.ops(L.Ty.text(ty, 2n, kind, c, (x + w - fw - 20 - kw : U32), (y + 6 : U32), 28, T.Th.accent())),
          Bv.when(own, F.Fm.button(ty, c, (x + w - fw - 8 : U32), (y + 6 : U32), label0, T.Th.soft(), "mem-forget", key)),
          L.Out.ops(Bv.lines(ls, ty, c, (x + 12 : U32), (y + 38 : U32), T.Th.soft())),
          Bv.faint(ty, c, tags, (x + 12 : U32), (w - 24 : U32), (y + 40 + Bv.nl(ls) * 22 : U32))]),
        (y + h + 8 : U32)}

def Bv.mems(ms: List<&2, BT.Mem>, +ty: L.Ty, +own: Bool, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  match ms:
    case Nil{}:
      L.Col{L.Out.none(), y}
    case m <> t:
      L.Lay.col.then(Bv.mem(ty, m, own, c, x, w, y), y2 => Bv.mems(t, ty, own, c, x, w, y2))

def Bv.memory(+ty: L.Ty, +u: U.Ui, +bot: String, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  +ms = BU.BU.mems(u, bot)
  L.Lay.col.cat(L.Out.cat(Bv.h2(ty, c, MSG.T.bots.tab.memory(L.Ty.loc(ty)), x, y),
      Bv.faint(ty, c, Bool.pick(String, List.is_empty(&2, BT.Mem, ms), MSG.T.bots.memory.empty(L.Ty.loc(ty)),
        MSG.T.bots.memory.count(L.Ty.loc(ty), Nat.show(List.length(&2, BT.Mem, ms)))), x, w, (y + 30 : U32))),
    Bv.mems(ms, ty, BU.BU.owns(bot), c, x, w, (y + 60 : U32)))

# Routines
# --------

def Bv.onoff.loc(+l: I.Loc, +on: Bool) -> String:
  Bool.pick(String, on, MSG.T.settings.lang.on(l), MSG.T.settings.lang.off(l))

# a routine's card: its name and buttons, a kind chip (a bell for a
# reminder) with when it runs, when it is next (in the person's zone) or when
# it was done, and the prompt

def Bv.onoff(+on: Bool) -> String:
  Bv.onoff.loc(I.En{}, on)
def Bv.kind.chip(+ty: L.Ty, +c: D.Clip, +x: U32, +y: U32, +kind: String, +label: String, +dim: Bool) -> L.Out:
  +bell = {String.eq(kind, "reminder") : Bool}
  +cw = {(L.Lay.button.w(ty, label) + Bool.pick(U32, bell, 18, 0) : U32) : U32}
  L.Out.ops(List.concat(&2, R.Op, [L.Lay.box(c, x, y, cw, 22, 0, T.Th.raise(), T.Th.line()),
    Bool.pick(List<&2, R.Op>, bell, L.Lay.icon(ty, "bell", c, (x + 6 : U32), (y + 3 : U32), Bool.pick(U32, dim, T.Th.faint(), T.Th.accent())), Nil{}),
    L.Ty.text(ty, 2n, label, c, (x + 10 + Bool.pick(U32, bell, 18, 0) : U32), y, 22, Bool.pick(U32, dim, T.Th.faint(), T.Th.dim()))]))

def Bv.chip.w(+ty: L.Ty, +kind: String, +label: String) -> U32:
  (L.Lay.button.w(ty, label) + Bool.pick(U32, String.eq(kind, "reminder"), 18, 0) : U32)

# the next line: "Next Wed 14 Oct 09:30 EEST · in 3h", the zone when it is not yours; "Done · ..." when done
def Bv.next.text.loc(+l: I.Loc, +next: String, +rel: String, +zone: String, +done: Bool, +dtext: String, +on: Bool) -> String:
  Bool.pick(String, done, dtext, Bool.pick(String, String.is_empty(next), Bool.pick(String, on, "", MSG.T.settings.lang.off(l)),
    MSG.T.routine.next(l, next, Bool.pick(String, String.is_empty(rel), "none", "some"), rel, Bool.pick(String, String.eq(zone, MSG.T.sched.zone.you(l)), "none", "some"), zone)))

# the line under the chip: a repeating routine's next time, Off, or Done;
# none for a pending one-time routine, whose time is on the chip's line

def Bv.next.text(+next: String, +rel: String, +zone: String, +done: Bool, +dtext: String, +on: Bool) -> String:
  Bv.next.text.loc(I.En{}, next, rel, zone, done, dtext, on)
def Bv.routine.gap(w0: SC.SRow, +on: Bool) -> U32:
  match w0:
    case SC.SRow{_, _, _, +next, _, _, +done, _}:
      Bool.pick(U32, Bool.or(done, Bool.or(Bool.not(on), Bool.not(String.is_empty(next)))), 22, 0)

def Bv.routine.lines(+ty: L.Ty, w0: SC.SRow, +prompt: String, +on: Bool, +gap: U32, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Out:
  match w0:
    case SC.SRow{+kind, +label, +when, +next, +rel, +zone, +done, +dtext}:
      +dim = {Bool.or(done, Bool.not(on)) : Bool}
      L.Out.all([Bv.kind.chip(ty, c, (x + 12 : U32), (y + 34 : U32), kind, label, dim),
        Bv.faint(ty, c, when, (x + 12 + Bv.chip.w(ty, kind, label) + 8 : U32), (w - 24 - Bv.chip.w(ty, kind, label) - 8 : U32), (y + 36 : U32)),
        Bv.faint(ty, c, Bv.next.text.loc(L.Ty.loc(ty), next, rel, zone, done, dtext, on), (x + 12 : U32), (w - 24 : U32), (y + 60 : U32)),
        L.Out.ops(L.Ty.text(ty, 2n, L.Ty.fit(ty, 2n, prompt, (w - 24 : U32)), c, (x + 12 : U32), (y + 60 + gap : U32), 20, T.Th.soft()))])

def Bv.routine.btns(+ty: L.Ty, +done: Bool, +on: Bool, +c: D.Clip, +x: U32, +w: U32, +y: U32, +id: String) -> L.Out:
  match done:
    case True{}:
      +label0 = MSG.T.common.delete(L.Ty.loc(ty))
      F.Fm.button(ty, c, (x + w - 12 - L.Lay.button.w(ty, label0) : U32), (y + 8 : U32), label0, T.Th.soft(), "routine-delete", id)
    case False{}:
      +label0 = MSG.T.common.delete(L.Ty.loc(ty))
      +label1 = MSG.T.queue.edit(L.Ty.loc(ty))
      +label2 = MSG.T.routine.run(L.Ty.loc(ty))
      +label3 = Bv.onoff.loc(L.Ty.loc(ty), on)
      L.Out.all([
          F.Fm.chip(ty, c, (x + w - 12 - L.Lay.button.w(ty, label0) - L.Lay.button.w(ty, label1) - L.Lay.button.w(ty, label2) - L.Lay.button.w(ty, label3) - 18 : U32), (y + 8 : U32), label3, on, "routine-toggle", id),
          F.Fm.button(ty, c, (x + w - 12 - L.Lay.button.w(ty, label0) - L.Lay.button.w(ty, label1) - L.Lay.button.w(ty, label2) - 12 : U32), (y + 8 : U32), label2, T.Th.soft(), "routine-run", id),
          F.Fm.button(ty, c, (x + w - 12 - L.Lay.button.w(ty, label0) - L.Lay.button.w(ty, label1) - 6 : U32), (y + 8 : U32), label1, T.Th.soft(), "routine-edit", id),
          F.Fm.button(ty, c, (x + w - 12 - L.Lay.button.w(ty, label0) : U32), (y + 8 : U32), label0, T.Th.soft(), "routine-delete", id)])

# buttons: all for a bot here; a bot elsewhere's hub runs it on asking, nothing more
def Bv.routine.own(own: Bool, +ty: L.Ty, +done: Bool, +on: Bool, +c: D.Clip, +x: U32, +w: U32, +y: U32, +id: String) -> L.Out:
  match own:
    case True{}:
      Bv.routine.btns(ty, done, on, c, x, w, y, id)
    case False{}:
      +label0 = MSG.T.routine.run(L.Ty.loc(ty))
      F.Fm.button(ty, c, (x + w - 12 - L.Lay.button.w(ty, label0) : U32), (y + 8 : U32), label0, T.Th.soft(), "routine-run", id)

def Bv.routine.card.k(+ty: L.Ty, +w0: SC.SRow, +r: BT.Routine, +own: Bool, +gap: U32, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  L.Col{L.Out.all([
      L.Out.ops(L.Lay.box(c, x, y, w, (86 + gap : U32), 0, T.Th.panel(), T.Th.line())),
      L.Out.ops(L.Ty.text(ty, 0n, L.Ty.fit(ty, 0n, BT.Routine.name(r), (w - 40 : U32)), c, (x + 12 : U32), (y + 6 : U32), 28, Bool.pick(U32, Bool.or(BT.Routine.done(r), Bool.not(BT.Routine.on(r))), T.Th.dim(), T.Th.text()))),
      Bv.routine.own(own, ty, BT.Routine.done(r), BT.Routine.on(r), c, x, w, y, BT.Routine.id(r)),
      Bv.routine.lines(ty, w0, BT.Routine.prompt(r), BT.Routine.on(r), gap, c, x, w, y)]),
    (y + 94 + gap : U32)}

def Bv.routine.card(+ty: L.Ty, +w0: SC.SRow, +r: BT.Routine, +own: Bool, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  Bv.routine.card.k(ty, w0, r, own, Bv.routine.gap(w0, BT.Routine.on(r)), c, x, w, y)

def Bv.routine.k(own: Bool, +ty: L.Ty, +u: U.Ui, +r: BT.Routine, +now: Nat, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  Bv.routine.card(ty, SC.Sched.row(L.Ty.loc(ty), U.Ui.state(u), r, now, 0n, True{}), r, own, c, x, w, y)

def Bv.routines(rs: List<&2, BT.Routine>, +ty: L.Ty, +u: U.Ui, +own: Bool, +now: Nat, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  match rs:
    case Nil{}:
      L.Col{L.Out.none(), y}
    case r <> t:
      L.Lay.col.then(Bv.routine.k(own, ty, u, r, now, c, x, w, y), y2 => Bv.routines(t, ty, u, own, now, c, x, w, y2))

def Bv.oform(+ty: L.Ty, +f: F.FS, +editing: Bool, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  L.Col{L.Out.all([
      L.Out.ops(L.Ty.text(ty, 0n, Bool.pick(String, editing, MSG.T.routine.change(L.Ty.loc(ty)), MSG.T.routine.new(L.Ty.loc(ty))), c, x, y, 28, T.Th.soft())),
      F.Fm.field(ty, f, c, x, (y + 32 : U32), U32.min(w, 280), "oname", "routine-save", "", MSG.T.bots.name(L.Ty.loc(ty))),
      F.Fm.field(ty, f, c, (x + U32.min(w, 280) + 8 : U32), (y + 32 : U32), L.U32.dim(w, (U32.min(w, 280) + 8 : U32)), "ocron", "routine-save", "", MSG.T.routine.time.placeholder(L.Ty.loc(ty))),
      Bv.faint(ty, c, U.Routine.hint.loc(L.Ty.loc(ty), F.FS.ui(f)), x, w, (y + 62 : U32)),
      F.Fm.area(ty, f, c, x, (y + 86 : U32), w, 4n, "oprompt", "routine-save", "", MSG.T.routine.prompt(L.Ty.loc(ty))),
      F.Fm.button(ty, c, x, (y + 186 : U32), Bool.pick(String, editing, MSG.T.common.save(L.Ty.loc(ty)), MSG.T.routine.add(L.Ty.loc(ty))), T.Th.accent(), "routine-save", "")]),
    (y + 230 : U32)}

# the new routine form, for a bot here
def Bv.oform.if(own: Bool, +ty: L.Ty, +f: F.FS, +editing: Bool, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  match own:
    case True{}:
      Bv.oform(ty, f, editing, c, x, w, y)
    case False{}:
      L.Col{L.Out.none(), y}

def Bv.rtab(+ty: L.Ty, +f: F.FS, +bot: String, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  +rs = BU.BU.routines(F.FS.ui(f), bot)
  L.Lay.col.cat(L.Out.cat(Bv.h2(ty, c, MSG.T.bots.tab.routines(L.Ty.loc(ty)), x, y),
      Bv.faint(ty, c, Bool.pick(String, List.is_empty(&2, BT.Routine, rs), MSG.T.routine.empty(L.Ty.loc(ty), SC.Zone.person(U.Ui.state(F.FS.ui(f)))),
        MSG.T.routine.note(L.Ty.loc(ty), SC.Zone.person(U.Ui.state(F.FS.ui(f))))), x, w, (y + 30 : U32))),
    L.Lay.col.then(Bv.routines(rs, ty, F.FS.ui(f), BU.BU.owns(bot), U.Side.now(F.FS.ui(f)), c, x, w, (y + 60 : U32)), y2 =>
      Bv.oform.if(BU.BU.owns(bot), ty, f, BU.BU.open(F.FS.ui(f), "@onew"), c, x, w, (y2 + 12 : U32))))

# Webhooks
# --------

def Bv.hook(+ty: L.Ty, h: BT.Hook, +now: Nat, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  match h:
    case BT.Hook{+id, _, +name, _, +revoked, +last, +k}:
      +label0 = MSG.T.hooks.revoke(L.Ty.loc(ty))
      +label1 = MSG.T.hooks.token(L.Ty.loc(ty))
      +rw = (L.Lay.button.w(ty, label0) + L.Lay.button.w(ty, label1) + 8 : U32)
      +tw = L.Lay.button.w(ty, label1)
      L.Col{L.Out.all([
          L.Out.ops(L.Lay.box(c, x, y, w, 66, 0, T.Th.panel(), T.Th.line())),
          L.Out.ops(L.Ty.text(ty, 0n, L.Ty.fit(ty, 0n, name, (w - rw - 40 : U32)), c, (x + 12 : U32), (y + 6 : U32), 28, Bool.pick(U32, revoked, T.Th.dim(), T.Th.text()))),
          Bv.faint(ty, c, MSG.T.hooks.status(L.Ty.loc(ty), id, Bool.pick(String, revoked, "revoked", Bool.pick(String, Nat.is_eq(k, 0n), "never", "called")), Nat.show(k), I.I18n.ago(L.Ty.loc(ty), now, last)),
            (x + 12 : U32), (w - rw - 40 : U32), (y + 36 : U32)),
          Bool.pick(L.Out, revoked, L.Out.none(), F.Fm.button(ty, c, (x + w - rw - 12 : U32), (y + 19 : U32), label1, T.Th.soft(), "hook-token", id)),
          Bool.pick(L.Out, revoked, L.Out.none(), F.Fm.button(ty, c, (x + w - rw + tw - 4 : U32), (y + 19 : U32), label0, T.Th.soft(), "hook-revoke", id))]),
        (y + 74 : U32)}

def Bv.hooks(hs: List<&2, BT.Hook>, +ty: L.Ty, +now: Nat, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  match hs:
    case Nil{}:
      L.Col{L.Out.none(), y}
    case h <> t:
      L.Lay.col.then(Bv.hook(ty, h, now, c, x, w, y), y2 => Bv.hooks(t, ty, now, c, x, w, y2))

# a secret shown once, with a button to copy it
def Bv.secret.k(none: Bool, +ty: L.Ty, +label: String, +s: String, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  match none:
    case True{}:
      L.Col{L.Out.none(), y}
    case False{}:
      +label0 = MSG.T.common.copy(L.Ty.loc(ty))
      +cw = L.Lay.button.w(ty, label0)
      L.Col{L.Out.all([
          L.Out.ops(L.Lay.box(c, x, y, w, 72, 0, T.Th.sel(), T.Th.accent())),
          L.Out.ops(L.Ty.text(ty, 2n, L.Ty.fit(ty, 2n, label, (w - cw - 36 : U32)), c, (x + 12 : U32), (y + 6 : U32), 24, T.Th.accent())),
          L.Out.ops(L.Ty.text(ty, 4n, L.Ty.fit(ty, 4n, s, (w - cw - 36 : U32)), c, (x + 12 : U32), (y + 34 : U32), 28, T.Th.text())),
          F.Fm.button(ty, c, (x + w - cw - 12 : U32), (y + 34 : U32), label0, T.Th.accent(), "copy", s)]),
        (y + 84 : U32)}

def Bv.secret(+ty: L.Ty, +label: String, +s: String, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  Bv.secret.k(String.is_empty(s), ty, label, s, c, x, w, y)

def Bv.token.btns(+ty: L.Ty, +hid: String, +c: D.Clip, +x: U32, +y: U32) -> L.Col:
  +label0 = MSG.T.hooks.token.new(L.Ty.loc(ty))
  +label1 = MSG.T.hooks.off(L.Ty.loc(ty))
  +nw = L.Lay.button.w(ty, label0)
  +ow = L.Lay.button.w(ty, label1)
  L.Col{L.Out.all([F.Fm.button(ty, c, x, y, label0, T.Th.soft(), "hook-retoken", hid),
      F.Fm.button(ty, c, (x + nw + 8 : U32), y, label1, T.Th.soft(), "hook-untoken", hid),
      F.Fm.button(ty, c, (x + nw + ow + 16 : U32), y, MSG.T.tour.done(L.Ty.loc(ty)), T.Th.soft(), "hook-done", "")]),
    (y + 40 : U32)}

# a webhook's bearer token (for senders that cannot sign, like a Pebble
# Index ring's app) and the URL to post to, shown until hidden
def Bv.token.k(none: Bool, +ty: L.Ty, +url: String, +tok: String, +hid: String, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  match none:
    case True{}:
      L.Col{L.Out.none(), y}
    case False{}:
      L.Lay.col.then(Bv.secret(ty, Bool.pick(String, String.starts_with(url, "https://"), MSG.T.hooks.post(L.Ty.loc(ty)), MSG.T.hooks.post.http(L.Ty.loc(ty))), url, c, x, w, y), y2 =>
        L.Lay.col.then(Bv.secret(ty, MSG.T.hooks.pebble.header(L.Ty.loc(ty)), "Authorization: Bearer " ++ tok, c, x, w, y2), y3 =>
          L.Lay.col.then(Bv.secret(ty, MSG.T.hooks.bearer(L.Ty.loc(ty)), tok, c, x, w, y3), y4 => Bv.token.btns(ty, hid, c, x, y4))))

def Bv.token(+ty: L.Ty, +u: U.Ui, +bot: String, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  Bv.token.k(Bool.or(String.is_empty(U.Ui.scratch(u, "@hook.token")),
      Bool.not(String.eq(BT.Hook.bot(BT.World.hook(M.State.bots(U.Ui.state(u)), U.Ui.scratch(u, "@hook.tokfor"))), bot))),
    ty, U.Ui.scratch(u, "@hook.tokurl"), U.Ui.scratch(u, "@hook.token"), U.Ui.scratch(u, "@hook.tokfor"), c, x, w, y)

def Bv.htab(+ty: L.Ty, +f: F.FS, +bot: String, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  +label0 = MSG.T.hooks.new(L.Ty.loc(ty))
  +u = F.FS.ui(f)
  +hs = BU.BU.hooks(u, bot)
  +nw = L.Lay.button.w(ty, label0)
  L.Lay.col.cat(L.Out.all([Bv.h2(ty, c, MSG.T.bots.tab.hooks(L.Ty.loc(ty)), x, y),
      Bv.faint(ty, c, MSG.T.hooks.note(L.Ty.loc(ty)), x, w, (y + 30 : U32)),
      F.Fm.field(ty, f, c, x, (y + 58 : U32), (w - nw - 8 : U32), "hname", "hook-new", "", MSG.T.hooks.name(L.Ty.loc(ty))),
      F.Fm.button(ty, c, (x + w - nw : U32), (y + 58 : U32), label0, T.Th.accent(), "hook-new", "")]),
    L.Lay.col.then(Bv.secret(ty, MSG.T.hooks.secret(L.Ty.loc(ty), U.Ui.scratch(u, "@hook.id")), U.Ui.scratch(u, "@hook.secret"), c, x, w, (y + 98 : U32)), y2 =>
      L.Lay.col.then(Bv.token(ty, u, bot, c, x, w, y2), y3 => Bv.hooks(hs, ty, U.Side.now(u), c, x, w, y3))))

# Settings
# --------

def Bv.del(armed: Bool, +ty: L.Ty, +name: String, +id: String, +c: D.Clip, +x: U32, +y: U32) -> L.Out:
  match armed:
    case True{}:
      +label = MSG.T.bots.delete.named(L.Ty.loc(ty), name)
      L.Out.cat(F.Fm.button(ty, c, x, y, label, T.Th.bad(), "bot-delete", id),
        F.Fm.button(ty, c, (x + L.Lay.button.w(ty, label) + 8 : U32), y, MSG.T.common.cancel(L.Ty.loc(ty)), T.Th.soft(), "bfield", "delask" ++ Bv.sep()))
    case False{}:
      F.Fm.button(ty, c, x, y, MSG.T.bots.delete(L.Ty.loc(ty)), T.Th.soft(), "bfield", "delask" ++ Bv.sep() ++ "1")

def Bv.peer(+ty: L.Ty, p: BT.Peer, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  match p:
    case BT.Peer{+id, +name, +url, _, +revoked}:
      +label0 = MSG.T.hooks.revoke(L.Ty.loc(ty))
      +rw = L.Lay.button.w(ty, label0)
      L.Col{L.Out.all([
          L.Out.ops(L.Ty.text(ty, 0n, L.Ty.fit(ty, 0n, name, (w - rw - 16 : U32)), c, x, y, 28, Bool.pick(U32, revoked, T.Th.dim(), T.Th.text()))),
          Bv.faint(ty, c, Bool.pick(String, revoked, "revoked", url), x, (w - rw - 16 : U32), (y + 26 : U32)),
          Bool.pick(L.Out, revoked, L.Out.none(), F.Fm.button(ty, c, (x + w - rw : U32), (y + 8 : U32), label0, T.Th.soft(), "peer-revoke", id))]),
        (y + 54 : U32)}

def Bv.peers(ps: List<&2, BT.Peer>, +ty: L.Ty, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  match ps:
    case Nil{}:
      L.Col{L.Out.none(), y}
    case p <> t:
      L.Lay.col.then(Bv.peer(ty, p, c, x, w, y), y2 => Bv.peers(t, ty, c, x, w, y2))

def Bv.row2(+ty: L.Ty, +f: F.FS, +name: String, +action: String, +ph: String, +label: String, +value: String, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Out:
  +bw = L.Lay.button.w(ty, label)
  L.Out.cat(F.Fm.field(ty, f, c, x, y, (w - bw - 8 : U32), name, action, value, ph), F.Fm.button(ty, c, (x + w - bw : U32), y, label, T.Th.accent(), action, value))

def Bv.machines(+ty: L.Ty, +f: F.FS, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  +u = F.FS.ui(f)
  L.Lay.col.cat(L.Out.all([Bv.h2(ty, c, MSG.T.bots.machines(L.Ty.loc(ty)), x, y),
      Bv.faint(ty, c, MSG.T.bots.machines.note(L.Ty.loc(ty)), x, w, (y + 30 : U32)),
      Bv.row2(ty, f, "purl", "peer-invite", MSG.T.bots.invite.address(L.Ty.loc(ty)), MSG.T.bots.invite.make(L.Ty.loc(ty)), "", c, x, w, (y + 58 : U32)),
      Bv.row2(ty, f, "invite", "peer-join", MSG.T.bots.invite.paste(L.Ty.loc(ty)), MSG.T.bots.invite.join(L.Ty.loc(ty)), "", c, x, w, (y + 94 : U32))]),
    L.Lay.col.then(Bv.secret(ty, MSG.T.bots.invite.once(L.Ty.loc(ty)), U.Ui.scratch(u, "@peer.invite"), c, x, w, (y + 134 : U32)), y2 =>
      Bv.peers(BU.BU.peers(u), ty, c, x, w, y2)))

def Bv.gurl.k(none: Bool, +ty: L.Ty, +s: String, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  match none:
    case True{}:
      L.Col{L.Out.none(), y}
    case False{}:
      +label0 = MSG.T.menu.file.open(L.Ty.loc(ty))
      +label1 = MSG.T.common.copy(L.Ty.loc(ty))
      +ow = L.Lay.button.w(ty, label0)
      +cw = L.Lay.button.w(ty, label1)
      L.Col{L.Out.all([
          L.Out.ops(L.Ty.text(ty, 2n, L.Ty.fit(ty, 2n, s, (w - ow - cw - 20 : U32)), c, x, y, 28, T.Th.accent())),
          F.Fm.button(ty, c, (x + w - ow - cw - 8 : U32), y, label0, T.Th.accent(), "open-url", s),
          F.Fm.button(ty, c, (x + w - cw : U32), y, label1, T.Th.soft(), "copy", s)]),
        (y + 36 : U32)}

# the user's own OAuth client, shown on asking
def Bv.gown(open: Bool, +ty: L.Ty, +f: F.FS, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  match open:
    case False{}:
      L.Col{L.Out.none(), y}
    case True{}:
      +label0 = MSG.T.google.client.save(L.Ty.loc(ty))
      +hw = U32.shr((w - 8 : U32))
      L.Col{L.Out.all([
          F.Fm.field(ty, f, c, x, y, hw, "gid", "google", "connect", MSG.T.google.client.id(L.Ty.loc(ty))),
          F.Fm.field(ty, f, c, (x + hw + 8 : U32), y, hw, "gsecret", "google", "connect", MSG.T.google.client.secret(L.Ty.loc(ty))),
          F.Fm.button(ty, c, x, (y + 36 : U32), label0, T.Th.soft(), "google", "config"),
          F.Fm.button(ty, c, (x + L.Lay.button.w(ty, label0) + 8 : U32), (y + 36 : U32), MSG.T.common.hide(L.Ty.loc(ty)), T.Th.soft(), "bfield", "gown" ++ Bv.sep())]),
        (y + 74 : U32)}

# the signature editor under an account (client.bend's Bots.sig)
def Bv.gsig(editing: Bool, +ty: L.Ty, +f: F.FS, +e: String, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  match editing:
    case False{}:
      L.Col{L.Out.none(), y}
    case True{}:
      +label0 = MSG.T.common.save(L.Ty.loc(ty))
      +ah = F.Fm.area.h(3n)
      L.Col{L.Out.all([
          F.Fm.area(ty, f, c, x, y, w, 3n, "gsig", "google", "", MSG.T.google.signature.placeholder(L.Ty.loc(ty), e)),
          F.Fm.button(ty, c, x, (y + ah + 6 : U32), label0, T.Th.accent(), "google", "sig-save"),
          F.Fm.button(ty, c, (x + L.Lay.button.w(ty, label0) + 8 : U32), (y + ah + 6 : U32), MSG.T.common.cancel(L.Ty.loc(ty)), T.Th.soft(), "google", "sig-cancel")]),
        (y + ah + 48 : U32)}

# one signed-in account, its signature (what mail sent from it ends with) and its own sign-out
def Bv.gacct(+ty: L.Ty, +f: F.FS, +e: String, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  +label0 = MSG.T.agy.signout(L.Ty.loc(ty))
  +label1 = MSG.T.google.signature(L.Ty.loc(ty))
  +bw = L.Lay.button.w(ty, label0)
  +sw = L.Lay.button.w(ty, label1)
  L.Lay.col.then(L.Col{L.Out.all([
      L.Out.ops(L.Ty.text(ty, 2n, L.Ty.fit(ty, 2n, e, (w - bw - sw - 24 : U32)), c, x, y, 28, T.Th.text())),
      F.Fm.button(ty, c, (x + w - bw - sw - 8 : U32), y, label1, T.Th.soft(), "google", "sig-edit:" ++ e),
      F.Fm.button(ty, c, (x + w - bw : U32), y, label0, T.Th.soft(), "google", "signout:" ++ e)]),
    (y + 36 : U32)}, y2 => Bv.gsig(String.eq(U.Bots.sig.editing(F.FS.ui(f)), e), ty, f, e, c, x, w, y2))

def Bv.gaccts(xs: List<&2, String>, +ty: L.Ty, +f: F.FS, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  match xs:
    case Nil{}:
      L.Col{L.Out.none(), y}
    case e <> t:
      L.Lay.col.then(Bv.gacct(ty, f, e, c, x, w, y), y2 => Bv.gaccts(t, ty, f, c, x, w, y2))

# connect with the shared client or the user's own; every account signed
# in, each with its own sign-out
def Bv.google(+ty: L.Ty, +f: F.FS, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  +u = F.FS.ui(f)
  +st = U.Ui.scratch(u, "@google")
  +add = Bool.pick(String, List.is_empty(&2, String, U.Bots.google.accts(u)), MSG.T.google.connect(L.Ty.loc(ty)), MSG.T.agy.account.add(L.Ty.loc(ty)))
  +cw = L.Lay.button.w(ty, add)
  L.Lay.col.then(L.Col{L.Out.all([Bv.h2(ty, c, "Google", x, y),
      Bv.faint(ty, c, Bool.pick(String, String.is_empty(st), MSG.T.google.note(L.Ty.loc(ty)), st), x, w, (y + 30 : U32)),
      F.Fm.button(ty, c, x, (y + 58 : U32), add, T.Th.accent(), "google", "connect"),
      F.Fm.button(ty, c, (x + cw + 8 : U32), (y + 58 : U32), MSG.T.google.client.own(L.Ty.loc(ty)), T.Th.soft(), "bfield", "gown" ++ Bv.sep() ++ "1")]), (y + 96 : U32)}, +y0 =>
    L.Lay.col.then(Bv.gaccts(U.Bots.google.accts(u), ty, f, c, x, w, y0), +y1 =>
    L.Lay.col.then(Bv.gown(String.eq(F.Fm.text(f, "gown"), "1"), ty, f, c, x, w, y1), +y2 =>
      L.Lay.col.then(Bv.gurl.k(String.is_empty(U.Ui.scratch(u, "@google.url")), ty, U.Ui.scratch(u, "@google.url"), c, x, w, y2), +y3 =>
        L.Col{Bv.row2(ty, f, "gpaste", "google", MSG.T.google.callback(L.Ty.loc(ty)), MSG.T.common.finish(L.Ty.loc(ty)), "paste", c, x, w, y3), (y3 + 40 : U32)}))))

def Bv.stab(+ty: L.Ty, +f: F.FS, b: BT.Bot, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  match b:
    case BT.Bot{+id, +name, _, +look, _, _, _}:
      +label0 = MSG.T.bots.face.new(L.Ty.loc(ty))
      +label1 = MSG.T.common.save(L.Ty.loc(ty))
      +fw = L.Lay.button.w(ty, label0)
      L.Lay.col.then(L.Col{L.Out.all([
          Bv.h2(ty, c, MSG.T.bots.persona(L.Ty.loc(ty)), x, y),
          Bv.faint(ty, c, MSG.T.bots.persona.note(L.Ty.loc(ty), name), x, w, (y + 30 : U32)),
          F.Fm.area(ty, f, c, x, (y + 58 : U32), w, 8n, "persona", "bot-edit", "", MSG.T.bots.persona.example(L.Ty.loc(ty))),
          F.Fm.button(ty, c, x, (y + 234 : U32), label1, T.Th.accent(), "bot-edit", ""),
          F.Fm.button(ty, c, (x + L.Lay.button.w(ty, label1) + 8 : U32), (y + 234 : U32), label0, T.Th.soft(), "bot-look", Nat.show(Nat.mod(1n+look, CT.Cat.looks()))),
          Bv.del(String.eq(F.Fm.text(f, "delask"), "1"), ty, name, id, c, (x + L.Lay.button.w(ty, label1) + fw + 16 : U32), (y + 234 : U32))]), (y + 290 : U32)}, y2 =>
        L.Lay.col.then(Bv.machines(ty, f, c, x, w, y2), y3 =>
          Bv.google(ty, f, c, x, w, (y3 + 16 : U32))))

# Browser
# -------
# The bot's page (layout.bend's Lay.page) under a strip that says who has it
# and what the person can do (client.bend's Teach.strip): teach it a task,
# take it over, hand it back. While the person drives, the frame takes their
# clicks, wheel and keys (nui.bend's Bb) and an address bar shows. Once a
# recording is stopped the tab is the save sheet.

# the strip's buttons, right to left from x1 (the first is leftmost)
def Bv.tbtns(bs: List<&2, U.TBtn>, +ty: L.Ty, +c: D.Clip, +y: U32, +x1: U32) -> L.Out:
  match bs:
    case Nil{}:
      L.Out.none()
    case U.TBtn{+label, +action, +value, +primary} <> t:
      +w = F.Fm.button.w(ty, label)
      L.Out.cat(F.Fm.button(ty, c, (x1 - w : U32), y, label, Bool.pick(U32, primary, T.Th.accent(), T.Th.soft()), action, value),
        Bv.tbtns(t, ty, c, y, (x1 - w - 8 : U32)))

def Bv.urlbar(+ty: L.Ty, +f: F.FS, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Out:
  +label0 = MSG.T.tour.back(L.Ty.loc(ty))
  +label1 = MSG.T.common.reload(L.Ty.loc(ty))
  +bw = F.Fm.button.w(ty, label0)
  +rw = F.Fm.button.w(ty, label1)
  L.Out.all([F.Fm.button(ty, c, x, y, label0, T.Th.soft(), "bb-back", ""),
    F.Fm.button(ty, c, (x + bw + 8 : U32), y, label1, T.Th.soft(), "bb-reload", ""),
    Bv.row2(ty, f, "bburl", "bb-nav", MSG.T.browser.address(L.Ty.loc(ty)), MSG.T.common.go(L.Ty.loc(ty)), "", c, (x + bw + rw + 16 : U32), (w - bw - rw - 16 : U32), y)])

def Bv.strip.k(s: U.TStrip, +ty: L.Ty, +f: F.FS, +input: Bool, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  match s:
    case U.TStrip{+mode, +title, +clock, +steps, bs}:
      +rec = {Nat.is_eq(mode, 1n) : Bool}
      +meta = {Bool.pick(String, rec, MSG.T.teach.steps(L.Ty.loc(ty), clock, Nat.show(steps), Bool.pick(String, Nat.is_eq(steps, 1n), "one", "other")), "") : String}
      +ub = {Bool.pick(U32, input, 40, 0) : U32}
      +nb = {Bool.pick(U32, rec, 40, 0) : U32}
      L.Col{L.Out.all([
          L.Out.ops(L.Ty.text(ty, 1n, title, c, x, y, 30, T.Th.accent())),
          L.Out.ops(L.Ty.text(ty, 2n, meta, c, (x + L.Ty.width(ty, 1n, title) + 12 : U32), y, 30, T.Th.faint())),
          Bv.tbtns(List.reverse(&2, U.TBtn, bs), ty, c, y, (x + w : U32)),
          Bv.when(input, Bv.urlbar(ty, f, c, x, w, (y + 40 : U32))),
          Bv.when(rec, Bv.row2(ty, f, "tknote", "tk-note", MSG.T.teach.note.placeholder(L.Ty.loc(ty)), MSG.T.teach.note.add(L.Ty.loc(ty)), "", c, x, w, (y + 40 + ub : U32)))]),
        (y + 44 + ub + nb : U32)}

# the page under its strip
def Bv.live(+ty: L.Ty, +f: F.FS, +b: BT.Bot, +x0: U32, +x1: U32, +top: U32, +h: U32) -> L.Out:
  +u = F.FS.ui(f)
  +id = BT.Bot.id(b)
  +input = {U.Teach.input(u, id) : Bool}
  L.Lay.col.out(L.Lay.col.then(Bv.strip.k(U.Teach.strip.loc(L.Ty.loc(ty), u, id), ty, f, input, D.Clip.all(), (x0 + 24 : U32), (x1 - x0 - 48 : U32), top), +y2 =>
    L.Col{L.Lay.page.in(ty, u, id, MSG.T.bots.page.empty(L.Ty.loc(ty), BT.Bot.name(b)), x0, x1, y2, h, input), y2}))

# one recorded step in the save sheet: what it did, its typed value as a
# chip (a secret shows none: it is never kept), and once marked a parameter
# its name
def Bv.srow(r: U.TStepRow, +ty: L.Ty, +f: F.FS, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  match r:
    case U.TStepRow{+n, +text, _, +chip, +secret, +marked, +pname}:
      +label = {L.Ty.fit(ty, 2n, chip, 200) : String}
      +cw = {Bool.pick(U32, String.is_empty(chip), 0, F.Fm.button.w(ty, label)) : U32}
      +sw = {Bool.pick(U32, secret, L.Ty.width(ty, 2n, "private"), 0) : U32}
      L.Col{L.Out.all([
          L.Out.ops(L.Ty.text(ty, 2n, L.Ty.fit(ty, 2n, Nat.show(n) ++ ". " ++ text, L.U32.dim(w, (cw + sw + 24 : U32))), c, x, y, 28, T.Th.soft())),
          Bv.when(secret, L.Out.ops(L.Ty.text(ty, 2n, "private", c, (x + w - sw : U32), y, 28, T.Th.faint()))),
          Bv.when(Bool.not(String.is_empty(chip)), F.Fm.chip(ty, c, (x + w - cw : U32), y, label, marked, "tk-mark", Nat.show(n))),
          Bv.when(marked, F.Fm.field(ty, f, c, (x + 24 : U32), (y + 32 : U32), 280, "tkp." ++ Nat.show(n), "tk-save", "", pname))]),
        (y + Bool.pick(U32, marked, 76, 34) : U32)}

def Bv.srows(rs: List<&2, U.TStepRow>, +ty: L.Ty, +f: F.FS, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  match rs:
    case Nil{}:
      L.Col{L.Out.none(), y}
    case r <> t:
      L.Lay.col.then(Bv.srow(r, ty, f, c, x, w, y), y2 => Bv.srows(t, ty, f, c, x, w, y2))

def Bv.sheet(+ty: L.Ty, +f: F.FS, +b: BT.Bot, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  +label0 = MSG.T.common.save(L.Ty.loc(ty))
  L.Lay.col.cat(L.Out.all([Bv.h2(ty, c, MSG.T.teach.save(L.Ty.loc(ty)), x, y),
      Bv.faint(ty, c, MSG.T.teach.name.note(L.Ty.loc(ty)), x, w, (y + 30 : U32)),
      F.Fm.field(ty, f, c, x, (y + 58 : U32), w, "tkname", "tk-save", "", MSG.T.bots.name(L.Ty.loc(ty))),
      F.Fm.field(ty, f, c, x, (y + 98 : U32), w, "tkdesc", "tk-save", "", MSG.T.teach.description(L.Ty.loc(ty)))]),
    L.Lay.col.then(Bv.srows(U.Teach.rows.loc(L.Ty.loc(ty), F.FS.ui(f), BT.Bot.id(b)), ty, f, c, x, w, (y + 148 : U32)), +y2 =>
      L.Col{L.Out.all([F.Fm.button(ty, c, x, (y2 + 8 : U32), label0, T.Th.accent(), "tk-save", ""),
          F.Fm.button(ty, c, (x + F.Fm.button.w(ty, label0) + 8 : U32), (y2 + 8 : U32), MSG.T.teach.discard(L.Ty.loc(ty)), T.Th.soft(), "tk-discard", "")]), (y2 + 56 : U32)}))

def Bv.browser.k(sheet: Bool, +ty: L.Ty, +f: F.FS, +b: BT.Bot, +x0: U32, +x1: U32, +h: U32) -> L.Out:
  match sheet:
    case True{}:
      +top = {(Bv.hy() + 24 : U32) : U32}
      +off = Bv.off(F.FS.ui(f))
      Bv.pane(Bv.sheet(ty, f, b, D.Clip{x0, Bv.hy(), x1, h}, Bv.pane.x(x0, x1), Bv.pane.w(x0, x1), (top - off : U32)), x0, x1, Bv.hy(), off, h)
    case False{}:
      Bv.live(ty, f, b, x0, x1, (Bv.hy() + 16 : U32), h)

def Bv.browser(+ty: L.Ty, +f: F.FS, +b: BT.Bot, +x0: U32, +x1: U32, +h: U32) -> L.Out:
  Bv.browser.k(Nat.is_eq(U.Teach.mode(F.FS.ui(f), BT.Bot.id(b)), 2n), ty, f, b, x0, x1, h)

# Tasks
# -----
# What the bot has learned (client.bend's Teach.tasks): a row each, opened
# for its run form, then its recent runs with their screenshots.

def Bv.status.color(+s: String) -> U32:
  Bool.pick(U32, String.eq(s, "ok"), T.Th.accent(), Bool.pick(U32, Bool.or(String.eq(s, "failed"), String.eq(s, "stopped")), T.Th.bad(), T.Th.faint()))

# a parameter's field in the run form
def Bv.pfields(ps: List<&2, TK.TkParam>, +ty: L.Ty, +f: F.FS, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  match ps:
    case Nil{}:
      L.Col{L.Out.none(), y}
    case TK.TkParam{+name, +label, _, +example} <> t:
      L.Lay.col.cat(F.Fm.field(ty, f, c, x, y, w, U.Teach.pfield(name), "", "", TK.Tk.first(label, name) ++ Bool.pick(String, String.is_empty(example), "", " (" ++ example ++ ")")),
        Bv.pfields(t, ty, f, c, x, w, (y + 40 : U32)))

def Bv.task(t: U.TTask, +ty: L.Ty, +f: F.FS, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  match t:
    case U.TTask{+slug, +name, +desc, +ps, +open, +asking, +last}:
      +label0 = MSG.T.common.run(L.Ty.loc(ty))
      +lw = {L.Ty.width(ty, 2n, last) : U32}
      +np = {U32.from_nat(List.length(&2, TK.TkParam, ps)) : U32}
      +h = {Bool.pick(U32, open, (56 + np * 40 + 44 : U32), 56) : U32}
      +head = L.Out.all([
          L.Out.ops(L.Lay.box(c, x, y, w, h, 0, T.Th.panel(), Bool.pick(U32, open, T.Th.accent(), T.Th.line()))),
          L.Out.ops(L.Ty.text(ty, 0n, L.Ty.fit(ty, 0n, name, L.U32.dim(w, (lw + 40 : U32))), c, (x + 12 : U32), (y + 4 : U32), 28, T.Th.text())),
          L.Out.ops(L.Ty.text(ty, 2n, last, c, (x + w - lw - 12 : U32), (y + 4 : U32), 28, Bv.status.color(last))),
          Bv.faint(ty, c, desc, (x + 12 : U32), (w - 24 : U32), (y + 30 : U32))])
      L.Lay.col.cat(head,
        L.Lay.col.cat(Bv.when(open, L.Out.all([
            L.Lay.col.out(Bv.pfields(ps, ty, f, c, (x + 12 : U32), (w - 24 : U32), (y + 60 : U32))),
            F.Fm.button(ty, c, (x + 12 : U32), (y + h - 40 : U32), label0, T.Th.accent(), "tk-run", slug),
            F.Fm.button(ty, c, (x + 20 + F.Fm.button.w(ty, label0) : U32), (y + h - 40 : U32), Bool.pick(String, asking, MSG.T.menu.delete.confirm(L.Ty.loc(ty)), MSG.T.common.delete(L.Ty.loc(ty))), Bool.pick(U32, asking, T.Th.bad(), T.Th.soft()), "tk-del", slug)])),
          L.Col{L.Out{Nil{}, [L.Lay.hit(x, y, w, h, "tk-open", slug)]}, (y + h + 8 : U32)}))

def Bv.tasks(ts: List<&2, U.TTask>, +ty: L.Ty, +f: F.FS, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  match ts:
    case Nil{}:
      L.Col{L.Out.none(), y}
    case t <> r:
      L.Lay.col.then(Bv.task(t, ty, f, c, x, w, y), y2 => Bv.tasks(r, ty, f, c, x, w, y2))

# a run's screenshots, 120 by 80 each
def Bv.shots(ss: List<&2, String>, +ty: L.Ty, +u: U.Ui, +x: U32, +y: U32) -> L.Out:
  match ss:
    case Nil{}:
      L.Out.none()
    case +s <> t:
      L.Out.cat(L.Lay.thumb(ty, L.Lay.tshot(U.Ui.info(u, "listen"), s), U.Ui.info(u, "img:" ++ L.Lay.tshot(U.Ui.info(u, "listen"), s)), x, y, 120, 80),
        Bv.shots(t, ty, u, (x + 132 : U32), y))

def Bv.run(r: U.TRun, +ty: L.Ty, +u: U.Ui, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  match r:
    case U.TRun{_, +slug, +status, +n, +summary, +shots}:
      +sw = {L.Ty.width(ty, 2n, status) : U32}
      +h = {Bool.pick(U32, List.is_empty(&2, String, shots), 56, 148) : U32}
      L.Col{L.Out.all([
          L.Out.ops(L.Lay.box(c, x, y, w, h, 0, T.Th.panel(), T.Th.line())),
          L.Out.ops(L.Ty.text(ty, 0n, L.Ty.fit(ty, 0n, slug, L.U32.dim(w, (sw + 40 : U32))), c, (x + 12 : U32), (y + 4 : U32), 28, T.Th.text())),
          L.Out.ops(L.Ty.text(ty, 2n, status, c, (x + w - sw - 12 : U32), (y + 4 : U32), 28, Bv.status.color(status))),
          Bv.faint(ty, c, summary, (x + 12 : U32), (w - 24 : U32), (y + 30 : U32)),
          Bv.shots(shots, ty, u, (x + 12 : U32), (y + 56 : U32))]),
        (y + h + 8 : U32)}

def Bv.runs(rs: List<&2, U.TRun>, +ty: L.Ty, +u: U.Ui, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  match rs:
    case Nil{}:
      L.Col{L.Out.none(), y}
    case r <> t:
      L.Lay.col.then(Bv.run(r, ty, u, c, x, w, y), y2 => Bv.runs(t, ty, u, c, x, w, y2))

def Bv.runs.head(some: Bool, +ty: L.Ty, +c: D.Clip, +x: U32, +y: U32) -> L.Out:
  match some:
    case True{}:
      Bv.h2(ty, c, MSG.T.teach.runs(L.Ty.loc(ty)), x, y)
    case False{}:
      L.Out.none()

def Bv.ttab(+ty: L.Ty, +f: F.FS, +bot: String, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  +u = F.FS.ui(f)
  +ts = U.Teach.tasks(u, bot)
  +rs = List.take(&2, U.TRun, U.Teach.runs.shown(u, bot), 8n)
  L.Lay.col.cat(L.Out.cat(Bv.h2(ty, c, MSG.T.bots.tab.tasks(L.Ty.loc(ty)), x, y),
      Bv.faint(ty, c, Bool.pick(String, List.is_empty(&2, U.TTask, ts), MSG.T.teach.empty(L.Ty.loc(ty)), MSG.T.teach.run.note(L.Ty.loc(ty))), x, w, (y + 30 : U32))),
    L.Lay.col.then(Bv.tasks(ts, ty, f, c, x, w, (y + 60 : U32)), +y2 =>
      L.Lay.col.cat(Bv.runs.head(Bool.not(List.is_empty(&2, U.TRun, rs)), ty, c, x, (y2 + 12 : U32)),
        Bv.runs(rs, ty, u, c, x, w, (y2 + 52 : U32)))))

# A bot
# -----

def Bv.tabs(ts: List<&2, String>, +u: U.Ui, +id: String, +ty: L.Ty, +cur: String, +x: U32, +y: U32) -> L.Out:
  match ts:
    case Nil{}:
      L.Out.none()
    case +t <> rest:
      +label = BU.BU.tab.label.for(L.Ty.loc(ty), u, id, t)
      L.Out.cat(F.Fm.chip(ty, D.Clip.all(), x, y, label, String.eq(t, cur), "bot-tab", t),
        Bv.tabs(rest, u, id, ty, cur, (x + L.Lay.button.w(ty, label) + 6 : U32), y))

def Bv.busy(s: M.Status) -> Bool:
  match s:
    case M.StWorking{}:
      True{}
    case M.StWaiting{}:
      True{}
    case _:
      False{}

# the stop button, at the header's right (x1: where it ends)
def Bv.stop(on: Bool, +ty: L.Ty, +x1: U32) -> L.Out:
  match on:
    case True{}:
      +label0 = MSG.T.composer.stop(L.Ty.loc(ty))
      F.Fm.button(ty, D.Clip.all(), (x1 - 16 - L.Lay.button.w(ty, label0) : U32), 12, label0, T.Th.soft(), "interrupt", "")
    case False{}:
      L.Out.none()

# where the header's right side ends before the Viewer chip (when shown)
def Bv.right(+viewer: Bool, +ty: L.Ty, +x1: U32) -> U32:
  Bool.pick(U32, viewer, (x1 - 8 - L.Lay.button.w(ty, MSG.T.thread.viewer(L.Ty.loc(ty))) : U32), x1)

# a bot's header (BU.head, the same for a bot here, one mirrored from a
# linked machine and one only reported by one): its cat (64 px), its name
# with where it lives, its mood (and sub), what goes at its right (Stop),
# its own viewer (board, schematic, 3D, browser, diff) when it has one,
# and its tabs
def Bv.header(+ty: L.Ty, +cats: Map<&2, CA.CatF>, +u: U.Ui, +ms: Nat, hd: BU.Head, +sub: String, right: L.Out, +x0: U32, +x1: U32) -> L.Out:
  match hd:
    case BU.Head{+title, +look, +mood, +note, ts, +tab, +viewer, _}:
      +label0 = MSG.T.thread.viewer(L.Ty.loc(ty))
      +nw = L.Ty.width(ty, 1n, title)
      L.Out.all([
        L.Out{[D.Draw.rect(D.Clip.all(), x0, 0, (x1 - x0 : U32), Bv.hy(), T.Th.bg(), 255), D.Draw.rect(D.Clip.all(), x0, (Bv.hy() - 1 : U32), (x1 - x0 : U32), 1, T.Th.line(), 255)],
          [L.Lay.hit(x0, 0, (x1 - x0 : U32), Bv.hy(), "", "")]},
        Bv.cat(look, mood, D.Clip.all(), (x0 + 20 : U32), 12, 64),
        L.Out.ops(L.Ty.text(ty, 1n, title, D.Clip.all(), (x0 + 100 : U32), 10, 30, T.Th.text())),
        L.Out.ops(L.Ty.text(ty, 2n, note ++ sub, D.Clip.all(), (x0 + 112 + nw : U32), 10, 30, Bv.note.color(mood))),
        right,
        Bv.when(viewer, F.Fm.chip(ty, D.Clip.all(), (x1 - 16 - L.Lay.button.w(ty, label0) : U32), 12, label0, U.Ui.panel(u), "panel", "")),
        Bv.tabs(ts, u, U.Mailv.owner(u), ty, tab, (x0 + 100 : U32), 48)])

def Bv.tab.n(+t: String) -> Nat:
  S.Nui.pos(List.append(&2, String, BU.BU.tabs.mail(""), ["status"]), t, 0n)

# a tab's pane (Chat is the thread itself, laid out by Bv.bot.k)
def Bv.pane.k(k: Nat, +ty: L.Ty, +f: F.FS, +b: BT.Bot, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  match k:
    case 2n:
      SV.Spv.draw(ty, f, BU.BU.space(F.FS.ui(f), b), BT.Bot.id(b), c, x, w, y)
    case 4n:
      Bv.ttab(ty, f, BT.Bot.id(b), c, x, w, y)
    case 5n:
      Bv.memory(ty, F.FS.ui(f), BT.Bot.id(b), c, x, w, y)
    case 6n:
      Bv.rtab(ty, f, BT.Bot.id(b), c, x, w, y)
    case 7n:
      Bv.htab(ty, f, BT.Bot.id(b), c, x, w, y)
    case _:
      Bv.stab(ty, f, b, c, x, w, y)

# Rooms and bots elsewhere
# ------------------------
# A conversation: its posts, newest at the bottom (the wheel goes back,
# "@chat.sc" from the bottom), and a line to write in.

type PostL is Data:
  PostL{id: String, from: String, when: String, lines: List<&2, String>}

# Wrapped posts kept from frame to frame (main.bend's Kept). The outer
# map holds eight recently used conversation/width groups and their order;
# each group's map holds its posts under "<id>:<width>". A post is reused
# only for the same text. A missing glyph's fetch starts every group over
# (Shell.paint.again). The nested map sits behind a List to keep native
# state records from flattening it into too many fields.
type PW is Data:
  PW{ok: Bool, text: String, lines: List<&2, String>}
  PWGroup{posts: List<&2, Map<&2, PW>>}
  PWKeys{keys: List<&2, String>}
  PWPlane{key: String, plane: PC.Plane}
  PWCounts{key: String, words: String}
  PWMail{memo: MM.BotM}
  PWPaint{key: String}

def Bv.wraps.none() -> Map<&2, PW>:
  Map.new(&2, PW)

# the plane kept with the posts (Bv.plane.keep), an empty one before any
def Bv.plane.none() -> PC.Plane:
  PC.Plane{Nil{}, Nil{}, Nil{}, Nil{}, Nil{}, Nil{}, 0n, Nil{}, Nil{}}

def Bv.plane.got(p: PW) -> PC.Plane:
  match p:
    case PWPlane{_, pl}:
      pl
    case _:
      Bv.plane.none()

def Bv.plane.snd(r: Map<&2, PW> & PW) -> PW:
  (_, p) = r
  p

def Bv.plane.get(+ws: Map<&2, PW>) -> PC.Plane:
  Bv.plane.got(Bv.plane.snd(Map.get(PW, PWPlane{"", Bv.plane.none()}, ws, "@plane")))

# the sidebar row's count, kept beside the plane (it is not built unless the Status tab shows)
def Bv.plane.words.got(p: PW) -> String:
  match p:
    case PWCounts{_, w}:
      w
    case _:
      ""

def Bv.plane.words(+ws: Map<&2, PW>) -> String:
  Bv.plane.words.got(Bv.plane.snd(Map.get(PW, PWCounts{"", ""}, ws, "@plane.n")))

def Bv.sd.r(+ty: L.Ty, +cats: Map<&2, CA.CatF>, +n: S.Nui, +ws: Map<&2, PW>, +br: BRows) -> Sd:
  Sd{ty, cats, F.Fm.fs(n), S.Nui.ms_of(n), BU.BU.open(S.Nui.ui(n), "@bnew"), BU.BU.open(S.Nui.ui(n), "@rnew"), BRows.rows(br), BU.BU.rooms(S.Nui.ui(n)),
    BRows.up(br), Bv.plane.words(ws)}

def Bv.sd.w(+ty: L.Ty, +cats: Map<&2, CA.CatF>, +n: S.Nui, +ws: Map<&2, PW>) -> Sd:
  Bv.sd.r(ty, cats, n, ws, BRows.of(S.Nui.ui(n)))

def Bv.wrap.key(+id: String, +w: U32) -> String:
  id ++ ":" ++ U32.show(w)

def Bv.wrap.got(r: Map<&2, PW> & PW) -> PW:
  (_, p) = r
  p

def Bv.wrap.same(same: Bool, ls: List<&2, String>, +ty: L.Ty, +text: String, +w: U32) -> List<&2, String>:
  match same:
    case True{}:
      ls
    case False{}:
      L.Ty.wrap(ty, 3n, text, w)

def Bv.wrap.k(p: PW, +ty: L.Ty, +text: String, +w: U32) -> List<&2, String>:
  match p:
    case PW{True{}, +t, ls}:
      Bv.wrap.same(Q.Eq.str(t, text), ls, ty, text, w)
    case _:
      L.Ty.wrap(ty, 3n, text, w)

# a post's lines: the kept ones when they wrapped the same text, else
# wrapped now
def Bv.wrap(+ws: Map<&2, PW>, +ty: L.Ty, +id: String, +text: String, +w: U32) -> List<&2, String>:
  Bv.wrap.k(Bv.wrap.got(Map.get(PW, PW{False{}, "", Nil{}}, ws, Bv.wrap.key(id, w))), ty, text, w)

# Room ids include a remote DM's stable conversation key. Width is part
# of the group key, so returning after a resize can reuse either layout.
def Bv.wraps.key(+room: String, +w: U32) -> String:
  "c:" ++ Bv.wrap.key(room, w)

def Bv.wraps.group.k(p: PW) -> Map<&2, PW>:
  match p:
    case PWGroup{ws <> _}:
      ws
    case _:
      Bv.wraps.none()

def Bv.wraps.group(+ws: Map<&2, PW>, +room: String, +w: U32) -> Map<&2, PW>:
  Bv.wraps.group.k(Bv.wrap.got(Map.get(PW, PW{False{}, "", Nil{}}, ws, Bv.wraps.key(room, w))))

def Bv.posts.l(ps: List<&2, BT.Post>, +ws: Map<&2, PW>, +ty: L.Ty, +u: U.Ui, +now: Nat, +w: U32) -> List<&2, PostL>:
  match ps:
    case Nil{}:
      Nil{}
    case BT.Post{+id, _, +from, +text, _, +at} <> t:
      PostL{id, BU.BU.member(u, from), I.I18n.ago(L.Ty.loc(ty), now, at), Bv.wrap(ws, ty, id, text, w)} <> Bv.posts.l(t, ws, ty, u, now, w)

def Bv.posts.h(ps: List<&2, PostL>, +acc: U32) -> U32:
  match ps:
    case Nil{}:
      acc
    case PostL{_, _, _, +ls} <> t:
      Bv.posts.h(t, (acc + 36 + Bv.nl(ls) * 22 : U32))

# a post's height: its name line, its lines, and the gap under it
def Bv.post.h(+ls: List<&2, String>) -> U32:
  (36 + Bv.nl(ls) * 22 : U32)

# a post on screen: its name, when, its lines, and a line region of no
# width (never clicked; its y says how far the posts slid since the last
# frame, nui.bend's Hits.slide, so the tiles reuse what they drew)
def Bv.post.draw(on: Bool, +ty: L.Ty, +id: String, +from: String, +when: String, +ls: List<&2, String>, +c: D.Clip, +x: U32, +y: U32) -> L.Out:
  match on:
    case True{}:
      L.Out{List.concat(&2, R.Op, [
          L.Ty.text(ty, 0n, from, c, x, y, 24, Bool.pick(U32, String.eq(from, "you"), T.Th.soft(), T.Th.accent())),
          L.Ty.text(ty, 2n, when, c, (x + L.Ty.width(ty, 0n, from) + 10 : U32), y, 24, T.Th.faint()),
          Bv.lines(ls, ty, c, x, (y + 24 : U32), T.Th.text())]),
        [L.Lay.hit(x, y, 0, Bv.post.h(ls), "chat-line", id)]}
    case False{}:
      L.Out.none()

# the posts from y down; oy is where y is in the posts (0 at the first),
# and only those that reach into lo..hi there (with a margin for glyphs
# past their lines) are drawn: the clip cuts the rest to nothing
def Bv.posts.draw(ps: List<&2, PostL>, +ty: L.Ty, +c: D.Clip, +x: U32, +y: U32, +oy: Nat, +lo: Nat, +hi: Nat) -> L.Out:
  match ps:
    case Nil{}:
      L.Out.none()
    case PostL{+id, +from, +when, +ls} <> t:
      +h = U32.to_nat(Bv.post.h(ls))
      L.Out.cat(Bv.post.draw(Bool.and(Nat.is_gt(Nat.add(Nat.add(oy, h), 16n), lo), Nat.is_lt(oy, Nat.add(hi, 16n))), ty, id, from, when, ls, c, x, y),
        Bv.posts.draw(t, ty, c, x, (y + Bv.post.h(ls) : U32), Nat.add(oy, h), lo, hi))

def Bv.chat.off(+u: U.Ui) -> U32:
  Maybe.default(&2, U32, U32.read(U.Ui.scratch(u, "@chat.sc")), 0)

# the posts from top to bottom, the newest at the bottom unless scrolled;
# lo..hi are the clip's rows in the posts' own coordinates (the first post
# at 0), and only posts there are drawn
def Bv.chat(+ty: L.Ty, +ws: Map<&2, PW>, ps: List<&2, BT.Post>, +u: U.Ui, +empty: String, +x0: U32, +x1: U32, +top: U32, +bottom: U32) -> L.Out:
  +w = Bv.pane.w(x0, x1)
  +x = Bv.pane.x(x0, x1)
  +pl = Bv.posts.l(ps, ws, ty, u, U.Side.now(u), w)
  +total = Bv.posts.h(pl, 0)
  +view = {L.U32.dim(bottom, (top + 16 : U32)) : U32}
  +most = L.U32.dim(total, view)
  +off = U32.min(Bv.chat.off(u), most)
  +fits = U32.is_le(total, view)
  +y = {Bool.pick(U32, fits, (top + 16 : U32), ((bottom - total : U32) + off : U32)) : U32}
  +lo = Bool.pick(Nat, fits, 0n, Nat.sub(Nat.add(U32.to_nat(top), U32.to_nat(total)), Nat.add(U32.to_nat(bottom), U32.to_nat(off))))
  +hi = Bool.pick(Nat, fits, U32.to_nat(total), Nat.sub(U32.to_nat(total), U32.to_nat(off)))
  L.Out.cat(L.Out{Bool.pick(List<&2, R.Op>, U32.is_eq(total, 0), L.Ty.text(ty, 2n, empty, D.Clip.all(), x, (top + 16 : U32), 28, T.Th.faint()), Nil{}),
      [L.Lay.hit(x0, top, (x1 - x0 : U32), (bottom - top : U32), "chat-bg", U32.show(most))]},
    Bv.posts.draw(pl, ty, D.Clip{x0, top, x1, bottom}, x, y, 0n, lo, hi))

# the line at the bottom: Enter or Send posts it
def Bv.say(+ty: L.Ty, +f: F.FS, +name: String, +action: String, +ph: String, +x0: U32, +x1: U32, +h: U32) -> L.Out:
  +w = Bv.pane.w(x0, x1)
  +x = Bv.pane.x(x0, x1)
  L.Out.cat(L.Out.ops([D.Draw.rect(D.Clip.all(), x0, (h - 60 : U32), (x1 - x0 : U32), 1, T.Th.line(), 255)]),
    Bv.row2(ty, f, name, action, ph, MSG.T.composer.send(L.Ty.loc(ty)), "", D.Clip.all(), x, w, (h - 44 : U32)))

def Bv.top(+ty: L.Ty, +title: String, +sub: String, +x0: U32, +x1: U32, right: L.Out) -> L.Out:
  +tw = L.Ty.width(ty, 1n, title)
  L.Out.all([L.Out.ops([D.Draw.rect(D.Clip.all(), x0, 0, (x1 - x0 : U32), 52, T.Th.bg(), 255), D.Draw.rect(D.Clip.all(), x0, 51, (x1 - x0 : U32), 1, T.Th.line(), 255)]),
    L.Out.ops(L.Ty.text(ty, 1n, title, D.Clip.all(), (x0 + 24 : U32), 0, 52, T.Th.text())),
    L.Out.ops(L.Ty.text(ty, 2n, L.Ty.fit(ty, 2n, sub, L.U32.dim((x1 - x0 : U32), (tw + 200 : U32))), D.Clip.all(), (x0 + 36 + tw : U32), 0, 52, T.Th.faint())),
    right])

def Bv.room(+ty: L.Ty, +ws: Map<&2, PW>, +n: S.Nui, r: BT.Room, +x0: U32, +x1: U32) -> L.Out:
  match r:
    case BT.Room{+id, +name, ms, cr, de}:
      +label0 = MSG.T.room.delete(L.Ty.loc(ty))
      +u = S.Nui.ui(n)
      +h = S.Nui.height(n)
      +dw = L.Lay.button.w(ty, label0)
      L.Out.all([
        Bv.chat(ty, Bv.wraps.group(ws, id, Bv.pane.w(x0, x1)), BU.BU.posts(u, id), u, MSG.T.room.empty(L.Ty.loc(ty)), x0, x1, 52, (h - 60 : U32)),
        Bv.say(ty, F.Fm.fs(n), "rpost", "room-post", MSG.T.room.write(L.Ty.loc(ty)), x0, x1, h),
        Bv.top(ty, "# " ++ name, BU.BU.members(u, BT.Room{id, name, ms, cr, de}), x0, x1,
          F.Fm.button(ty, D.Clip.all(), (x1 - 16 - dw : U32), 12, label0, T.Th.soft(), "room-delete", id))])

# a bot only reported by a linked machine: the bot's view (BU.head: its
# cat, "name · machine", its mood, Chat alone) over the person's
# conversation with it
def Bv.remote(+ty: L.Ty, +ws: Map<&2, PW>, +cats: Map<&2, CA.CatF>, +n: S.Nui, +id: String, +x0: U32, +x1: U32) -> L.Out:
  +u = S.Nui.ui(n)
  +h = S.Nui.height(n)
  L.Out.all([
    Bv.chat(ty, Bv.wraps.group(ws, BT.Room.dm("you", id), Bv.pane.w(x0, x1)), BU.BU.remote.posts(u, id), u, MSG.T.bots.remote.empty(L.Ty.loc(ty), id), x0, x1, Bv.hy(), (h - 60 : U32)),
    Bv.say(ty, F.Fm.fs(n), "rtext", "remote-send", MSG.T.bots.remote.write(L.Ty.loc(ty), id), x0, x1, h),
    Bv.header(ty, cats, u, S.Nui.ms_of(n), BU.BU.head(u, BU.VRemote{id}), "", L.Out.none(), x0, x1)])

# the whole window with its main area given (a bot's or a room's when it
# has one, else the one given: main.bend's memo of it, MM.Mm.main)
def Bv.main.or(m: Maybe<&2, L.Out>, given: L.Out) -> Maybe<&2, L.Out>:
  match m:
    case Some{o}:
      Some{o}
    case None{}:
      Some{given}

# the same with the sidebar's body given too (L.Lay.sidebar.a: main.bend's
# memo of it), or laid out (none)
# the frame with the main area and the sidebar's body given (main.bend's
# memos): the sidebar's parts that go under the body are not made again
def Bv.frame.given(+ty: L.Ty, +memo: L.TLMemo, +n: S.Nui, main: Maybe<&2, L.Out>, a: L.Out) -> L.Out:
  L.Lay.frame.us(ty, memo, n, L.Out.none(), 0, c => y => L.Col{L.Out.none(), y}, main, Some{a})

# The posts on show wrapped for this frame, reusing the selected group's
# unchanged texts. Rebuilding this one group removes posts no longer in
# the conversation's bounded window; other groups stay until evicted.
def Bv.wraps.put(ps: List<&2, BT.Post>, +ws: Map<&2, PW>, +ty: L.Ty, +w: U32, acc: Map<&2, PW>) -> Map<&2, PW>:
  match ps:
    case Nil{}:
      acc
    case BT.Post{+id, _, _, +text, _, _} <> t:
      Bv.wraps.put(t, ws, ty, w, Map.set(&2, PW, acc, Bv.wrap.key(id, w), PW{True{}, text, Bv.wrap(ws, ty, id, text, w)}))

def Bv.wraps.max() -> Nat:
  8n

def Bv.wraps.keys.k(p: PW) -> List<&2, String>:
  match p:
    case PWKeys{ks}:
      ks
    case _:
      Nil{}

def Bv.wraps.keys(+ws: Map<&2, PW>) -> List<&2, String>:
  Bv.wraps.keys.k(Bv.wrap.got(Map.get(PW, PWKeys{Nil{}}, ws, "@wraps")))

def Bv.wraps.drop.k(same: Bool, k: String, rest: List<&2, String>) -> List<&2, String>:
  match same:
    case True{}:
      rest
    case False{}:
      k <> rest

def Bv.wraps.drop(ks: List<&2, String>, +key: String) -> List<&2, String>:
  match ks:
    case Nil{}:
      Nil{}
    case +k <> t:
      Bv.wraps.drop.k(String.eq(k, key), k, Bv.wraps.drop(t, key))

def Bv.wraps.evict(ks: List<&2, String>, ws: Map<&2, PW>) -> Map<&2, PW>:
  match ks:
    case Nil{}:
      ws
    case k <> t:
      Bv.wraps.evict(t, Map.del(&2, PW, ws, k))

def Bv.wraps.keep(+ws: Map<&2, PW>, +key: String, posts: Map<&2, PW>) -> Map<&2, PW>:
  +keys = {key <> Bv.wraps.drop(Bv.wraps.keys(ws), key) : List<&2, String>}
  Map.set(&2, PW, Map.set(&2, PW,
      Bv.wraps.evict(List.drop(&2, String, keys, Bv.wraps.max()), ws), key, PWGroup{[posts]}),
    "@wraps", PWKeys{List.take(&2, String, keys, Bv.wraps.max())})

def Bv.wraps.conversation(+ps: List<&2, BT.Post>, +room: String, +ws: Map<&2, PW>, +ty: L.Ty, +w: U32) -> Map<&2, PW>:
  Bv.wraps.keep(ws, Bv.wraps.key(room, w), Bv.wraps.put(ps, Bv.wraps.group(ws, room, w), ty, w, Bv.wraps.none()))

def Bv.wraps.of(v: BU.View, +u: U.Ui, +ws: Map<&2, PW>, +ty: L.Ty, +w: U32) -> Map<&2, PW>:
  match v:
    case BU.VRoom{BT.Room{+id, _, _, _, _}}:
      Bv.wraps.conversation(BU.BU.posts(u, id), id, ws, ty, w)
    case BU.VRemote{+id}:
      Bv.wraps.conversation(BU.BU.remote.posts(u, id), BT.Room.dm("you", id), ws, ty, w)
    case _:
      ws

def Bv.wraps.k(settings: Bool, +ty: L.Ty, +n: S.Nui, +ws: Map<&2, PW>) -> Map<&2, PW>:
  match settings:
    case True{}:
      ws
    case False{}:
      Bv.wraps.of(BU.BU.view(S.Nui.ui(n)), S.Nui.ui(n), ws, ty, Bv.pane.w(264, L.Lay.x1(n)))

# The status plane, built again only when its key changes (core's
# Plane.key) and only while there is an uberbot
def Bv.plane.key.of(p: PW) -> String:
  match p:
    case PWPlane{+k, _}:
      k
    case _:
      ""

def Bv.plane.key(+ws: Map<&2, PW>) -> String:
  Bv.plane.key.of(Bv.plane.snd(Map.get(PW, PWPlane{"", Bv.plane.none()}, ws, "@plane")))

# whether the uberbot's Status tab is up (the only place the plane is drawn)
def Bv.plane.shown.k(v: BU.View, +u: U.Ui) -> Bool:
  match v:
    case BU.VBot{+b}:
      Bool.or(String.eq(BU.BU.tab.of(u, BT.Bot.id(b)), "status"), String.eq(BU.BU.tab.of(u, BT.Bot.id(b)), "mail"))
    case _:
      False{}

def Bv.plane.shown(+u: U.Ui) -> Bool:
  Bv.plane.shown.k(BU.BU.view(u), u)

def Bv.plane.build(shown: Bool, +u: U.Ui) -> PC.Plane:
  match shown:
    case True{}:
      PC.Plane.of(u, U.Side.now(u))
    case False{}:
      Bv.plane.none()

def Bv.plane.store(+ws: Map<&2, PW>, +key: String, +plane: PC.Plane, words: String) -> Map<&2, PW>:
  Map.set(&2, PW, Map.set(&2, PW, Map.set(&2, PW, ws, "@plane", PWPlane{key, plane}), "@plane.n", PWCounts{key, words}), "@plane.paint", PWPaint{MR.Mr.plane_stamp(plane)})

def Bv.plane.paint.got(p: PW) -> String:
  match p:
    case PWPaint{k}: k
    case _: ""

def Bv.plane.paint(+ws: Map<&2, PW>) -> String:
  Bv.plane.paint.got(Bv.plane.snd(Map.get(PW, PWPaint{""}, ws, "@plane.paint")))

def Bv.plane.keep.k(same: Bool, +ws: Map<&2, PW>, +u: U.Ui, +key: String, +ix: U.SIx) -> Map<&2, PW>:
  match same:
    case True{}:
      ws
    case False{}:
      Bv.plane.store(ws, key, Bv.plane.build(Bv.plane.shown(u), u), PC.Plane.tally.words.ix(u, ix, U.Side.now(u)))

# ---- mail ui ----
def Bv.mail.draw(+ws: Map<&2, PW>, +ty: L.Ty, +n: S.Nui) -> L.Out:
  MR.Mr.draw(Bv.plane.get(ws), ty, F.Fm.fs(n), 282, (MM.Mm.x1.of(n) - 300 : U32), (Bv.hy() + 12 : U32), (S.Nui.height(n) - 12 : U32))

def Bv.mail.key(p: PW) -> String:
  match p:
    case PWMail{m}: MM.BotM.key(m)
    case _: ""

def Bv.mail.got(p: PW, +ws: Map<&2, PW>, +ty: L.Ty, +n: S.Nui) -> L.Out:
  match p:
    case PWMail{m}: MM.BotM.out(m)
    case _: Bv.mail.draw(ws, ty, n)

def Bv.mail.out(+ws: Map<&2, PW>, +ty: L.Ty, +n: S.Nui) -> L.Out:
  Bv.mail.got(Bv.plane.snd(Map.get(PW, PW{False{}, "", Nil{}}, ws, "@mail.pane")), ws, ty, n)

def Bv.bot.k(k: Nat, +ty: L.Ty, +memo: L.TLMemo, +ws: Map<&2, PW>, +n: S.Nui, +b: BT.Bot, +x0: U32, +x1: U32) -> L.Out:
  match k:
    case 0n:
      L.Out.hits_below(Bv.hy(), L.Lay.thread.at(ty, memo, n, x0, x1, (Bv.hy() - 52 : U32)))
    case 1n:
      Bv.mail.out(ws, ty, n)
    case 3n:
      Bv.browser(ty, F.Fm.fs(n), b, x0, x1, S.Nui.height(n))
    case 9n:
      +top = {(Bv.hy() + 24 : U32) : U32}
      +off = Bv.off(S.Nui.ui(n))
      Bv.pane(PV.Pl.draw(Bv.plane.get(ws), PC.Plane.home(S.Nui.ui(n)), ty, D.Clip{x0, Bv.hy(), x1, S.Nui.height(n)}, (x0 + 24 : U32), (x1 - x0 - 48 : U32), top, off, L.U32.dim(S.Nui.height(n), top), U.Side.now(S.Nui.ui(n))),
        x0, x1, Bv.hy(), off, S.Nui.height(n))
    case +j:
      +top = {(Bv.hy() + 24 : U32) : U32}
      +off = Bv.off(S.Nui.ui(n))
      Bv.pane(Bv.pane.k(j, ty, F.Fm.fs(n), b, D.Clip{x0, Bv.hy(), x1, S.Nui.height(n)}, Bv.pane.x(x0, x1), Bv.pane.w(x0, x1), (top - off : U32)),
        x0, x1, Bv.hy(), off, S.Nui.height(n))

# a bot, here or mirrored from a linked machine: its header, then the tab
# BU.head picks (the tabs it has)
def Bv.bot.h(+ty: L.Ty, +memo: L.TLMemo, +ws: Map<&2, PW>, +cats: Map<&2, CA.CatF>, +n: S.Nui, +b: BT.Bot, +hd: BU.Head, +x0: U32, +x1: U32) -> L.Out:
  +u = S.Nui.ui(n)
  +st = U.Ui.state(u)
  +th = BT.Bot.thread(b)
  L.Out.cat(Bv.bot.k(Bv.tab.n(BU.Head.tab(hd)), ty, memo, ws, n, b, x0, x1),
    Bv.header(ty, cats, u, S.Nui.ms_of(n), hd, " · " ++ M.Thread.provider(U.Ui.thread(st, th)),
      Bv.stop(Bv.busy(U.Side.status(st, U.Ui.thread(st, th))), ty, Bv.right(BU.Head.viewer(hd), ty, x1)), x0, x1))

def Bv.bot(+ty: L.Ty, +memo: L.TLMemo, +ws: Map<&2, PW>, +cats: Map<&2, CA.CatF>, +n: S.Nui, +b: BT.Bot, +x0: U32, +x1: U32) -> L.Out:
  Bv.bot.h(ty, memo, ws, cats, n, b, BU.BU.head.bot(S.Nui.ui(n), b), x0, x1)

# The main area and the frame
# ---------------------------

def Bv.view(v: BU.View, +ty: L.Ty, +memo: L.TLMemo, +cats: Map<&2, CA.CatF>, +ws: Map<&2, PW>, +n: S.Nui, +x1: U32) -> Maybe<&2, L.Out>:
  match v:
    case BU.VThread{}:
      None{}
    case BU.VBot{b}:
      Some{Bv.bot(ty, memo, ws, cats, n, b, 264, x1)}
    case BU.VRoom{r}:
      Some{Bv.room(ty, ws, n, r, 264, x1)}
    case BU.VRemote{+id}:
      Some{Bv.remote(ty, ws, cats, n, id, 264, x1)}

def Bv.main.k(settings: Bool, +ty: L.Ty, +memo: L.TLMemo, +cats: Map<&2, CA.CatF>, +ws: Map<&2, PW>, +n: S.Nui) -> Maybe<&2, L.Out>:
  match settings:
    case True{}:
      None{}
    case False{}:
      Bv.view(BU.BU.view(S.Nui.ui(n)), ty, memo, cats, ws, L.Lay.shrink(n, L.Lay.term_h(n)), L.Lay.x1(n))

# a bot's, room's or remote bot's main area (None: the usual one)
def Bv.main.w(+ty: L.Ty, +memo: L.TLMemo, +cats: Map<&2, CA.CatF>, +ws: Map<&2, PW>, +n: S.Nui) -> Maybe<&2, L.Out>:
  Bv.main.k(U.Ui.flag(S.Nui.ui(n), "settings"), ty, memo, cats, ws, n)

def Bv.main(+ty: L.Ty, +memo: L.TLMemo, +cats: Map<&2, CA.CatF>, +n: S.Nui) -> Maybe<&2, L.Out>:
  Bv.main.w(ty, memo, cats, Bv.wraps.none(), n)

def Bv.mail.memo(p: PW) -> MM.BotM:
  match p:
    case PWMail{m}: m
    case _: MM.BotM.empty()

def Bv.mail.cached(+ws: Map<&2, PW>) -> MM.BotM:
  Bv.mail.memo(Bv.plane.snd(Map.get(PW, PWMail{MM.BotM.empty()}, ws, "@mail.pane")))

def Bv.mail.hit(+ws: Map<&2, PW>, +n: S.Nui) -> Nat:
  Bool.pick(Nat, Bool.and(U.Mailv.shown(S.Nui.ui(n)), MM.BotM.hit(Bv.mail.cached(ws))), 1n, 0n)

def Bv.mail.keep.same(same: Bool, +ws: Map<&2, PW>, +ty: L.Ty, +n: S.Nui, +key: String) -> Map<&2, PW>:
  match same:
    case True{}: Map.set(&2, PW, ws, "@mail.pane", PWMail{MM.BotM.kept(Bv.mail.cached(ws))})
    case False{}:
      Map.set(&2, PW, ws, "@mail.pane", PWMail{MM.BotM{key, Bv.mail.draw(ws, ty, n), U.Side.now(S.Nui.ui(n)), MR.Mr.due(Bv.plane.get(ws), ty, S.Nui.ui(n), 282, (MM.Mm.x1.of(n) - 300 : U32), (Bv.hy() + 12 : U32), (S.Nui.height(n) - 12 : U32)), False{}}})

def Bv.mail.keep.on(on: Bool, +ws: Map<&2, PW>, +ty: L.Ty, +n: S.Nui) -> Map<&2, PW>:
  match on:
    case False{}: ws
    case True{}:
      +key = MM.Mm.mail(n, Bv.plane.paint(ws))
      Bv.mail.keep.same(MM.BotM.same(key, U.Side.now(S.Nui.ui(n)), Bv.mail.cached(ws)), ws, ty, n, key)

def Bv.mail.keep(+ws: Map<&2, PW>, +ty: L.Ty, +n: S.Nui) -> Map<&2, PW>:
  Bv.mail.keep.on(U.Mailv.shown(S.Nui.ui(n)), ws, ty, L.Lay.shrink(n, L.Lay.term_h(n)))

def Bv.plane.cache_key(+u: U.Ui) -> String:
  J.Json.show(J.Str{U.Ui.info(u, "origin")}) ++ "|" ++ J.Json.show(J.Str{U.Ui.info(u, "local.generation")}) ++ "|lang=" ++ L.TL.locale(u) ++ "|" ++ Nat.show(Nat.sub(U.Ui.seq(u), U.Mailv.touch(u))) ++ "|" ++ U.Ui.scratch(u, "@mr.plane") ++ "|" ++ U.Ui.info(u, "local.touch") ++ "|" ++ Nat.show(Nat.div(U.Side.now(u), 60n)) ++ "|" ++ PC.Plane.key.peers(BU.BU.peers.of(u), u)

def Bv.plane.keep(+ws: Map<&2, PW>, +u: U.Ui, +ix: U.SIx) -> Map<&2, PW>:
  +key = {Bv.plane.cache_key(u) ++ Bool.pick(String, Bv.plane.shown(u), "|s", "") : String}
  Bv.plane.keep.k(String.eq(Bv.plane.key(ws), key), ws, u, key, ix)

def Bv.plane.refresh.mail(shown: Bool, +ws: Map<&2, PW>, +u: U.Ui, +ix: U.SIx) -> Map<&2, PW>:
  match shown:
    case True{}: Bv.plane.keep(ws, u, ix)
    case False{}: ws

def Bv.plane.refresh(m: Maybe<&2, BU.BRow>, +ws: Map<&2, PW>, +u: U.Ui, +ix: U.SIx) -> Map<&2, PW>:
  match m:
    case Some{_}:
      Bv.plane.keep(ws, u, ix)
    case None{}:
      Bv.plane.refresh.mail(U.Mailv.shown(u), ws, u, ix)

# (with the window's index of the state, main.bend's MM.Ix)
def Bv.wraps.ix(+ty: L.Ty, +n: S.Nui, +ws: Map<&2, PW>, +ix: U.SIx) -> Map<&2, PW>:
  Bv.mail.keep(Bv.plane.refresh(BU.BU.uber(S.Nui.ui(n)), Bv.wraps.k(U.Ui.flag(S.Nui.ui(n), "settings"), ty, n, ws), S.Nui.ui(n), ix), ty, n)

def Bv.wraps(+ty: L.Ty, +n: S.Nui, +ws: Map<&2, PW>) -> Map<&2, PW>:
  Bv.wraps.ix(ty, n, ws, U.Side.ix(U.Ui.state(S.Nui.ui(n))))

def Bv.mail.need_count(+words: String) -> Nat:
  Bool.pick(Nat, String.contains(words, "need you"), M.Switch.num(Maybe.default(&2, String, List.head(&2, String, String.split(words, ' ')), "0"), 0n), 0n)

def Bv.uber.row(+u: U.Ui, +ty: L.Ty, +cats: Map<&2, CA.CatF>, +ms: Nat, r: BU.BRow, +cn: String, +c: D.Clip) -> L.Out:
  match r:
    case BU.BRow{+id, _, _, +look, +mood, _, +sel, _, _, _}:
      L.Out{List.concat(&2, R.Op, [
          Bool.pick(List<&2, R.Op>, sel, [D.Draw.rect(c, 6, 54, 252, 42, T.Th.raise(), 255), D.Draw.rect(c, 6, 54, 2, 42, T.Th.accent(), 255)], Nil{}),
          Spot.anchor(c, 14, 60, 28) <> Nil{},
          L.Ty.text(ty, 2n, MV.Mv.badge_text(Nat.add(U.Mailv.act_unread(u), Bv.mail.need_count(cn))), c, 234, 54, 24, T.Th.warn()),
          L.Ty.text(ty, 0n, L.Ty.fit(ty, 0n, BU.BU.row.title(r), 162), c, 50, 54, 24, Bool.pick(U32, sel, T.Th.text(), T.Th.soft())),
          L.Ty.text(ty, 2n, L.Ty.fit(ty, 2n, Bool.pick(String, String.is_empty(cn), MSG.T.bots.nothing(L.Ty.loc(ty)), cn), 192), c, 50, 74, 20, Bool.pick(U32, String.is_empty(cn), T.Th.faint(), T.Th.dim()))]),
        [L.Lay.hit(0, 52, 264, 44, "uber-bot", id ++ "|mail"), Spot.hit(look, mood, c, 14, 60, 28)]}

def Bv.uber.out(sd: Sd) -> L.Out:
  match sd:
    case Sd{+ty, +cats, +f, +ms, _, _, _, _, Some{r}, +cn}:
      Bv.uber.row(F.FS.ui(f), ty, cats, ms, r, cn, D.Clip{0, 52, 264, 100})
    case _:
      L.Out.none()

def Bv.side.rows.k(~PC: Data, ~paint: PC -> ST.SideRowReq -> ST.SideRowStep, +sd: Sd, +ty: L.Ty, +n: S.Nui, +ix: U.SIx, pc: PC) -> L.SideCol:
  L.Lay.sidebar.a.ix.m(~PC, ~paint, ty, n, S.Nui.height(n), ix, Bv.uber.out(sd), Bv.uber.top(sd), c => y => Bv.side.of(sd, c, y), pc)

def Bv.side.rows.ixr(~PC: Data, ~paint: PC -> ST.SideRowReq -> ST.SideRowStep, +ty: L.Ty, +cats: Map<&2, CA.CatF>, +ws: Map<&2, PW>, +n: S.Nui, +ix: U.SIx, +br: BRows, pc: PC) -> L.SideCol:
  Bv.side.rows.k(~PC, ~paint, Bv.sd.r(ty, cats, n, ws, br), ty, n, ix, pc)

def Bv.frame.with.k(+sd: Sd, +ty: L.Ty, +memo: L.TLMemo, +n: S.Nui, main: Maybe<&2, L.Out>) -> L.Out:
  L.Lay.frame.u(ty, memo, n, Bv.uber.out(sd), Bv.uber.top(sd), c => y => Bv.side.of(sd, c, y), main)

def Bv.frame.with(+ty: L.Ty, +memo: L.TLMemo, +cats: Map<&2, CA.CatF>, +ws: Map<&2, PW>, +n: S.Nui, main: Maybe<&2, L.Out>) -> L.Out:
  Bv.frame.with.k(Bv.sd.w(ty, cats, n, ws), ty, memo, n, main)

# the sidebar's body laid out (L.Lay.sidebar.a, with the bots)
def Bv.side.a.k(+sd: Sd, +ty: L.Ty, +n: S.Nui, +ix: U.SIx) -> L.Out:
  L.Lay.sidebar.a.ix(ty, n, S.Nui.height(n), ix, Bv.uber.out(sd), Bv.uber.top(sd), c => y => Bv.side.of(sd, c, y))

# (with the window's index of the state and its cats' rows of the frame)
def Bv.side.a.ixr(+ty: L.Ty, +cats: Map<&2, CA.CatF>, +ws: Map<&2, PW>, +n: S.Nui, +ix: U.SIx, +br: BRows) -> L.Out:
  Bv.side.a.k(Bv.sd.r(ty, cats, n, ws, br), ty, n, ix)

def Bv.side.a.ix(+ty: L.Ty, +cats: Map<&2, CA.CatF>, +ws: Map<&2, PW>, +n: S.Nui, +ix: U.SIx) -> L.Out:
  Bv.side.a.ixr(ty, cats, ws, n, ix, BRows.of(S.Nui.ui(n)))

def Bv.side.a(+ty: L.Ty, +cats: Map<&2, CA.CatF>, +ws: Map<&2, PW>, +n: S.Nui) -> L.Out:
  Bv.side.a.ix(ty, cats, ws, n, U.Side.ix(U.Ui.state(S.Nui.ui(n))))

def Bv.frame.k(+sd: Sd, +ty: L.Ty, +memo: L.TLMemo, +cats: Map<&2, CA.CatF>, +ws: Map<&2, PW>, +n: S.Nui) -> L.Out:
  L.Lay.frame.u(ty, memo, n, Bv.uber.out(sd), Bv.uber.top(sd), c => y => Bv.side.of(sd, c, y), Bv.main.w(ty, memo, cats, ws, n))

# the whole window, topmost first, with the bots in it; ws: the posts
# wrapped for the frame before (Bv.wraps)
def Bv.frame.w(+ty: L.Ty, +memo: L.TLMemo, +cats: Map<&2, CA.CatF>, +ws: Map<&2, PW>, +n: S.Nui) -> L.Out:
  Bv.frame.k(Bv.sd.w(ty, cats, n, ws), ty, memo, cats, ws, n)

# (every post wrapped afresh)
def Bv.frame(+ty: L.Ty, +memo: L.TLMemo, +cats: Map<&2, CA.CatF>, +n: S.Nui) -> L.Out:
  Bv.frame.w(ty, memo, cats, Bv.wraps.none(), n)
