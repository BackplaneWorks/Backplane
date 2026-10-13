import Base
import ../core/model.bend as M
import ../core/client.bend as U
import ../core/bot.bend as BT
import ../core/botui.bend as BU
import ../core/cron.bend as CR
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
import ../core/plane.bend as PC
import ../core/uber.bend as UB
import ../core/eq.bend as Q

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

# a cat's row: the cat, its name (with its machine when elsewhere), and
# its mood; choosing it does what BU.act says
def Bv.row(+ty: L.Ty, +cats: Map<&2, CA.CatF>, +ms: Nat, +r: BU.BRow, +c: D.Clip, +y: U32) -> L.Out:
  match r:
    case BU.BRow{+id, _, _, +look, +mood, +note, +sel, _, _, _}:
      +nw = L.Ty.width(ty, 2n, note)
      L.Out{List.concat(&2, R.Op, [
          Bool.pick(List<&2, R.Op>, sel, [D.Draw.rect(c, 6, (y + 1 : U32), 252, 34, T.Th.raise(), 255), D.Draw.rect(c, 6, (y + 1 : U32), 2, 34, T.Th.accent(), 255)], Nil{}),
          CA.Cats.draw(cats, look, mood, ms, c, 14, (y + 4 : U32), 28),
          L.Ty.text(ty, 0n, L.Ty.fit(ty, 0n, BU.BU.row.title(r), L.U32.dim(192, nw)), c, 50, y, 36, Bool.pick(U32, sel, T.Th.text(), T.Th.soft())),
          L.Ty.text(ty, 2n, note, c, (252 - nw : U32), y, 36, Bv.note.color(mood))]),
        [L.Lay.hit(0, y, 264, 36, BU.BU.act(r), id)]}

def Bv.rows(rs: List<&2, BU.BRow>, +ty: L.Ty, +cats: Map<&2, CA.CatF>, +ms: Nat, +c: D.Clip, +y: U32) -> L.Col:
  match rs:
    case Nil{}:
      L.Col{L.Out.none(), y}
    case r <> t:
      L.Lay.col.cat(Bv.row(ty, cats, ms, r, c, y), Bv.rows(t, ty, cats, ms, c, (y + 36 : U32)))

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
          F.Fm.field(ty, f, c, 12, y, 240, "name", "bot-create", "", U.Bots.name.hint(F.FS.ui(f))),
          F.Fm.area(ty, f, c, 12, (y + 34 : U32), 240, 3n, "persona", "bot-create", "", "Who it is, what it does"),
          Bv.provs(["claude", "codex", "grok", "muse", "antigravity"], ty, Bv.prov(F.FS.ui(f)), c, 12, (y + 112 : U32)),
          F.Fm.button(ty, c, 12, (y + 146 : U32), "Create", T.Th.accent(), "bot-create", "")]),
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
      L.Lay.col.cat(F.Fm.field(ty, f, c, 12, y, 240, "rname", "room-save", "", "Room name"),
        L.Lay.col.then(Bv.picks(BU.BU.picks(F.FS.ui(f)), ty, c, 12, (y + 34 : U32)), +y2 =>
          L.Col{F.Fm.button(ty, c, 12, y2, "Save", T.Th.accent(), "room-save", ""), (y2 + 36 : U32)}))
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
type Sd is Data:
  Sd{ty: L.Ty, cats: Map<&2, CA.CatF>, f: F.FS, ms: Nat, bn: Bool, rn: Bool, rows: List<&2, BU.BRow>, rooms: List<&2, BU.RRow>, up: Maybe<&2, BU.BRow>, cn: String, board: String, bsel: Bool}

def Bv.sd(+ty: L.Ty, +cats: Map<&2, CA.CatF>, +n: S.Nui) -> Sd:
  Sd{ty, cats, F.Fm.fs(n), S.Nui.ms_of(n), BU.BU.open(S.Nui.ui(n), "@bnew"), BU.BU.open(S.Nui.ui(n), "@rnew"), BU.BU.rows(S.Nui.ui(n)), BU.BU.rooms(S.Nui.ui(n)), BU.BU.uber(S.Nui.ui(n)), "", U.BoardUi.entry(S.Nui.ui(n)), U.BoardUi.is(S.Nui.ui(n))}

# the Board's entry, above Rooms: "Board · 4 open"
def Bv.brow(+ty: L.Ty, +label: String, +sel: Bool, +c: D.Clip, +y: U32) -> L.Out:
  L.Out{List.concat(&2, R.Op, [
      Bool.pick(List<&2, R.Op>, sel, [D.Draw.rect(c, 6, (y + 1 : U32), 252, 30, T.Th.raise(), 255), D.Draw.rect(c, 6, (y + 1 : U32), 2, 30, T.Th.accent(), 255)], Nil{}),
      L.Lay.icon(ty, "check", c, 16, (y + 8 : U32), T.Th.faint()),
      L.Ty.text(ty, 0n, L.Ty.fit(ty, 0n, label, 200), c, 38, y, 32, Bool.pick(U32, sel, T.Th.text(), T.Th.soft()))]),
    [L.Lay.hit(0, y, 264, 32, "tb-open", "")]}

def Bv.side.k(+ty: L.Ty, +cats: Map<&2, CA.CatF>, +f: F.FS, +ms: Nat, +bn: Bool, +rn: Bool, rows: List<&2, BU.BRow>, rooms: List<&2, BU.RRow>, +bd: String, +bs: Bool, +c: D.Clip, +y: U32) -> L.Col:
  L.Lay.col.cat(L.Out.cat(L.Out.ops([D.Draw.rect(c, 12, (y + 4 : U32), 240, 1, T.Th.line(), 255)]), Bv.head(ty, c, "Bots", "bot-new", bn, (y + 10 : U32))),
    L.Lay.col.then(Bv.bform(bn, ty, f, c, (y + 40 : U32)), y1 =>
      L.Lay.col.then(Bv.rows(rows, ty, cats, ms, c, y1), +y2 =>
        L.Lay.col.cat(Bv.brow(ty, bd, bs, c, (y2 + 4 : U32)),
          L.Lay.col.cat(Bv.head(ty, c, "Rooms", "room-new", rn, (y2 + 44 : U32)),
            L.Lay.col.then(Bv.rform(rn, ty, f, c, (y2 + 74 : U32)), y3 =>
              Bv.rrows(rooms, ty, c, y3)))))))

# the sidebar under the projects: a rule, the cats, then the rooms
def Bv.side.of(sd: Sd, +c: D.Clip, +y: U32) -> L.Col:
  match sd:
    case Sd{+ty, +cats, +f, +ms, +bn, +rn, rows, rooms, _, _, +bd, +bs}:
      Bv.side.k(ty, cats, f, ms, bn, rn, rows, rooms, bd, bs, c, y)

def Bv.side(+ty: L.Ty, +cats: Map<&2, CA.CatF>, +n: S.Nui, +c: D.Clip, +y: U32) -> L.Col:
  Bv.side.of(Bv.sd(ty, cats, n), c, y)

# The uberbot's row above the search field (nothing when there is none):
# its cat, its name and, under it, what waits on the person and what runs
def Bv.uber.h() -> U32:
  48

def Bv.uber.row(+ty: L.Ty, +cats: Map<&2, CA.CatF>, +ms: Nat, r: BU.BRow, +cn: String, +c: D.Clip) -> L.Out:
  match r:
    case BU.BRow{_, _, _, +look, +mood, _, +sel, _, _, _}:
      L.Out{List.concat(&2, R.Op, [
          Bool.pick(List<&2, R.Op>, sel, [D.Draw.rect(c, 6, 54, 252, 42, T.Th.raise(), 255), D.Draw.rect(c, 6, 54, 2, 42, T.Th.accent(), 255)], Nil{}),
          CA.Cats.draw(cats, look, mood, ms, c, 14, 60, 28),
          L.Ty.text(ty, 0n, L.Ty.fit(ty, 0n, BU.BU.row.title(r), 192), c, 50, 54, 24, Bool.pick(U32, sel, T.Th.text(), T.Th.soft())),
          L.Ty.text(ty, 2n, L.Ty.fit(ty, 2n, Bool.pick(String, String.is_empty(cn), "Nothing needs you", cn), 192), c, 50, 74, 20, Bool.pick(U32, String.is_empty(cn), T.Th.faint(), T.Th.dim()))]),
        [L.Lay.hit(0, 52, 264, 44, "uber", "")]}

def Bv.uber.out(sd: Sd) -> L.Out:
  match sd:
    case Sd{+ty, +cats, _, +ms, _, _, _, _, Some{r}, +cn, _, _}:
      Bv.uber.row(ty, cats, ms, r, cn, D.Clip{0, 52, 264, 100})
    case _:
      L.Out.none()

def Bv.uber.top(sd: Sd) -> U32:
  match sd:
    case Sd{_, _, _, _, _, _, _, _, Some{_}, _, _, _}:
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

def Bv.warm(+u: U.Ui, c: Map<&2, CA.CatF>, +clock: Nat) -> Map<&2, CA.CatF>:
  Bv.warm.go(List.append(&2, BU.BRow, Bv.uber.rows(BU.BU.uber(u)), BU.BU.rows(u)), c, clock)

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
def Bv.awake(+n: S.Nui, +ms: Nat) -> Bool:
  Bool.and(Bool.and(Bool.and(S.Nui.has_focus(n), Bool.not(S.Lbx.on(n))), Bv.present(n, ms)), Bool.or(BU.BU.moving(S.Nui.ui(n)), BU.BU.moving.go(Bv.uber.rows(BU.BU.uber(S.Nui.ui(n))))))

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
def Bv.moving(+n: S.Nui, +ms: Nat) -> Bool:
  Bool.and(Bv.awake(n, ms), Nat.is_eq(Bv.calm.left(n, ms), 0n))

# the cats' clock ("cat.clock"): a frame at or past the next cat frame
# ("cat.due", main.bend's Shell.cats) while a cat moves sets it to its
# time; any other frame (a key typed) keeps it, so the cats' ops stay as
# they were and no tile of them is drawn again
def Bv.clock(+n: S.Nui, +ms: Nat) -> Nat:
  Bool.pick(Nat, Bool.and(Bv.moving(n, ms), Nat.is_ge(ms, Bv.info.nat(n, "cat.due"))), ms, Bv.info.nat(n, "cat.clock"))

def Bv.stamp(+n: S.Nui, +ms: Nat) -> S.Nui:
  S.Nui.with_ui(n, U.Ui.info_set(S.Nui.ui(n), "cat.clock", Nat.show(Bv.clock(n, ms))))

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
      +kw = L.Ty.width(ty, 2n, kind)
      +fw = {Bool.pick(U32, own, L.Lay.button.w(ty, "Forget"), 0) : U32}
      +ls = L.Ty.wrap(ty, 3n, text, (w - 24 : U32))
      +th = {Bool.pick(U32, String.is_empty(tags), 0, 20) : U32}
      +h = {(48 + Bv.nl(ls) * 22 + th : U32) : U32}
      L.Col{L.Out.all([
          L.Out.ops(L.Lay.box(c, x, y, w, h, 0, T.Th.panel(), T.Th.line())),
          L.Out.ops(L.Ty.text(ty, 0n, L.Ty.fit(ty, 0n, key, L.U32.dim(w, (kw + fw + 48 : U32))), c, (x + 12 : U32), (y + 6 : U32), 28, T.Th.text())),
          L.Out.ops(L.Ty.text(ty, 2n, kind, c, (x + w - fw - 20 - kw : U32), (y + 6 : U32), 28, T.Th.accent())),
          Bv.when(own, F.Fm.button(ty, c, (x + w - fw - 8 : U32), (y + 6 : U32), "Forget", T.Th.soft(), "mem-forget", key)),
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
  L.Lay.col.cat(L.Out.cat(Bv.h2(ty, c, "Memory", x, y),
      Bv.faint(ty, c, Bool.pick(String, List.is_empty(&2, BT.Mem, ms), "Nothing remembered yet. It keeps what it learns about you, your preferences, facts and how-tos here.",
        Nat.show(List.length(&2, BT.Mem, ms)) ++ " items. The ones about you and your preferences go into every turn."), x, w, (y + 30 : U32))),
    Bv.mems(ms, ty, BU.BU.owns(bot), c, x, w, (y + 60 : U32)))

# Routines
# --------

def Bv.onoff(+on: Bool) -> String:
  Bool.pick(String, on, "On", "Off")

def Bv.routine(+ty: L.Ty, r: BT.Routine, +now: Nat, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  match r:
    case BT.Routine{+id, _, +name, +cron, +prompt, +on, +last, _, _}:
      +b1 = L.Lay.button.w(ty, "Delete")
      +b2 = L.Lay.button.w(ty, "Edit")
      +b3 = L.Lay.button.w(ty, "Run now")
      +b4 = L.Lay.button.w(ty, Bv.onoff(on))
      +bx = {(x + w - 12 - b1 - b2 - b3 - b4 - 18 : U32) : U32}
      L.Col{L.Out.all([
          L.Out.ops(L.Lay.box(c, x, y, w, 90, 0, T.Th.panel(), T.Th.line())),
          L.Out.ops(L.Ty.text(ty, 0n, L.Ty.fit(ty, 0n, name, L.U32.dim((bx - x : U32), 24)), c, (x + 12 : U32), (y + 8 : U32), 28, Bool.pick(U32, on, T.Th.text(), T.Th.dim()))),
          F.Fm.chip(ty, c, bx, (y + 8 : U32), Bv.onoff(on), on, "routine-toggle", id),
          F.Fm.button(ty, c, (bx + b4 + 6 : U32), (y + 8 : U32), "Run now", T.Th.soft(), "routine-run", id),
          F.Fm.button(ty, c, (bx + b4 + b3 + 12 : U32), (y + 8 : U32), "Edit", T.Th.soft(), "routine-edit", id),
          F.Fm.button(ty, c, (bx + b4 + b3 + b2 + 18 : U32), (y + 8 : U32), "Delete", T.Th.soft(), "routine-delete", id),
          Bv.faint(ty, c, CR.Cron.describe(cron), (x + 12 : U32), (w - 24 : U32), (y + 40 : U32)),
          L.Out.ops(L.Ty.text(ty, 2n, L.Ty.fit(ty, 2n, prompt, (w - 24 : U32)), c, (x + 12 : U32), (y + 62 : U32), 20, T.Th.soft()))]),
        (y + 98 : U32)}

# a routine of a bot elsewhere: its hub runs it on asking, nothing more
def Bv.routine.far(+ty: L.Ty, r: BT.Routine, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  match r:
    case BT.Routine{+id, _, +name, +cron, +prompt, +on, _, _, _}:
      +b3 = L.Lay.button.w(ty, "Run now")
      +bx = {(x + w - 12 - b3 : U32) : U32}
      L.Col{L.Out.all([
          L.Out.ops(L.Lay.box(c, x, y, w, 90, 0, T.Th.panel(), T.Th.line())),
          L.Out.ops(L.Ty.text(ty, 0n, L.Ty.fit(ty, 0n, name, L.U32.dim((bx - x : U32), 24)), c, (x + 12 : U32), (y + 8 : U32), 28, Bool.pick(U32, on, T.Th.text(), T.Th.dim()))),
          F.Fm.button(ty, c, bx, (y + 8 : U32), "Run now", T.Th.soft(), "routine-run", id),
          Bv.faint(ty, c, CR.Cron.describe(cron) ++ Bool.pick(String, on, "", " · off"), (x + 12 : U32), (w - 24 : U32), (y + 40 : U32)),
          L.Out.ops(L.Ty.text(ty, 2n, L.Ty.fit(ty, 2n, prompt, (w - 24 : U32)), c, (x + 12 : U32), (y + 62 : U32), 20, T.Th.soft()))]),
        (y + 98 : U32)}

def Bv.routine.k(own: Bool, +ty: L.Ty, r: BT.Routine, +now: Nat, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  match own:
    case True{}:
      Bv.routine(ty, r, now, c, x, w, y)
    case False{}:
      Bv.routine.far(ty, r, c, x, w, y)

def Bv.routines(rs: List<&2, BT.Routine>, +ty: L.Ty, +own: Bool, +now: Nat, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  match rs:
    case Nil{}:
      L.Col{L.Out.none(), y}
    case r <> t:
      L.Lay.col.then(Bv.routine.k(own, ty, r, now, c, x, w, y), y2 => Bv.routines(t, ty, own, now, c, x, w, y2))

def Bv.oform(+ty: L.Ty, +f: F.FS, +editing: Bool, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  L.Col{L.Out.all([
      L.Out.ops(L.Ty.text(ty, 0n, Bool.pick(String, editing, "Change routine", "New routine"), c, x, y, 28, T.Th.soft())),
      F.Fm.field(ty, f, c, x, (y + 32 : U32), U32.min(w, 280), "oname", "routine-save", "", "Name"),
      F.Fm.field(ty, f, c, (x + U32.min(w, 280) + 8 : U32), (y + 32 : U32), L.U32.dim(w, (U32.min(w, 280) + 8 : U32)), "ocron", "routine-save", "", "When: @daily, 0 9 * * 1-5"),
      Bv.faint(ty, c, CR.Cron.describe(F.Fm.text(f, "ocron")), (x + U32.min(w, 280) + 8 : U32), w, (y + 62 : U32)),
      F.Fm.area(ty, f, c, x, (y + 86 : U32), w, 4n, "oprompt", "routine-save", "", "What it should do each time"),
      F.Fm.button(ty, c, x, (y + 186 : U32), Bool.pick(String, editing, "Save", "Add routine"), T.Th.accent(), "routine-save", "")]),
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
  L.Lay.col.cat(L.Out.cat(Bv.h2(ty, c, "Routines", x, y),
      Bv.faint(ty, c, Bool.pick(String, List.is_empty(&2, BT.Routine, rs), "None yet. A routine gives it a prompt on a schedule, in this hub's time zone.",
        "Each runs once when due, even after the hub was off."), x, w, (y + 30 : U32))),
    L.Lay.col.then(Bv.routines(rs, ty, BU.BU.owns(bot), U.Side.now(F.FS.ui(f)), c, x, w, (y + 60 : U32)), y2 =>
      Bv.oform.if(BU.BU.owns(bot), ty, f, BU.BU.open(F.FS.ui(f), "@onew"), c, x, w, (y2 + 12 : U32))))

# Webhooks
# --------

def Bv.hook(+ty: L.Ty, h: BT.Hook, +now: Nat, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  match h:
    case BT.Hook{+id, _, +name, _, +revoked, +last, +k}:
      +rw = (L.Lay.button.w(ty, "Revoke") + L.Lay.button.w(ty, "Token") + 8 : U32)
      +tw = L.Lay.button.w(ty, "Token")
      L.Col{L.Out.all([
          L.Out.ops(L.Lay.box(c, x, y, w, 66, 0, T.Th.panel(), T.Th.line())),
          L.Out.ops(L.Ty.text(ty, 0n, L.Ty.fit(ty, 0n, name, (w - rw - 40 : U32)), c, (x + 12 : U32), (y + 6 : U32), 28, Bool.pick(U32, revoked, T.Th.dim(), T.Th.text()))),
          Bv.faint(ty, c, "POST /hook/" ++ id ++ " · " ++ Bool.pick(String, revoked, "revoked", Bool.pick(String, Nat.is_eq(k, 0n), "never called", Nat.show(k) ++ " calls, last " ++ U.Time.ago(now, last) ++ " ago")),
            (x + 12 : U32), (w - rw - 40 : U32), (y + 36 : U32)),
          Bool.pick(L.Out, revoked, L.Out.none(), F.Fm.button(ty, c, (x + w - rw - 12 : U32), (y + 19 : U32), "Token", T.Th.soft(), "hook-token", id)),
          Bool.pick(L.Out, revoked, L.Out.none(), F.Fm.button(ty, c, (x + w - rw + tw - 4 : U32), (y + 19 : U32), "Revoke", T.Th.soft(), "hook-revoke", id))]),
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
      +cw = L.Lay.button.w(ty, "Copy")
      L.Col{L.Out.all([
          L.Out.ops(L.Lay.box(c, x, y, w, 72, 0, T.Th.sel(), T.Th.accent())),
          L.Out.ops(L.Ty.text(ty, 2n, L.Ty.fit(ty, 2n, label, (w - cw - 36 : U32)), c, (x + 12 : U32), (y + 6 : U32), 24, T.Th.accent())),
          L.Out.ops(L.Ty.text(ty, 4n, L.Ty.fit(ty, 4n, s, (w - cw - 36 : U32)), c, (x + 12 : U32), (y + 34 : U32), 28, T.Th.text())),
          F.Fm.button(ty, c, (x + w - cw - 12 : U32), (y + 34 : U32), "Copy", T.Th.accent(), "copy", s)]),
        (y + 84 : U32)}

def Bv.secret(+ty: L.Ty, +label: String, +s: String, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  Bv.secret.k(String.is_empty(s), ty, label, s, c, x, w, y)

def Bv.token.btns(+ty: L.Ty, +hid: String, +c: D.Clip, +x: U32, +y: U32) -> L.Col:
  +nw = L.Lay.button.w(ty, "New token")
  +ow = L.Lay.button.w(ty, "Turn off")
  L.Col{L.Out.all([F.Fm.button(ty, c, x, y, "New token", T.Th.soft(), "hook-retoken", hid),
      F.Fm.button(ty, c, (x + nw + 8 : U32), y, "Turn off", T.Th.soft(), "hook-untoken", hid),
      F.Fm.button(ty, c, (x + nw + ow + 16 : U32), y, "Done", T.Th.soft(), "hook-done", "")]),
    (y + 40 : U32)}

# a webhook's bearer token (for senders that cannot sign, like a Pebble
# Index ring's app) and the URL to post to, shown until hidden
def Bv.token.k(none: Bool, +ty: L.Ty, +url: String, +tok: String, +hid: String, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  match none:
    case True{}:
      L.Col{L.Out.none(), y}
    case False{}:
      L.Lay.col.then(Bv.secret(ty, Bool.pick(String, String.starts_with(url, "https://"), "Post to", "Post to (plain HTTP: the ring's app wants HTTPS, see tailscale serve)"), url, c, x, w, y), y2 =>
        L.Lay.col.then(Bv.secret(ty, "Header for the Pebble app (Index 01 settings, Webhook); send Transcription", "Authorization: Bearer " ++ tok, c, x, w, y2), y3 =>
          L.Lay.col.then(Bv.secret(ty, "Bearer token", tok, c, x, w, y3), y4 => Bv.token.btns(ty, hid, c, x, y4))))

def Bv.token(+ty: L.Ty, +u: U.Ui, +bot: String, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  Bv.token.k(Bool.or(String.is_empty(U.Ui.scratch(u, "@hook.token")),
      Bool.not(String.eq(BT.Hook.bot(BT.World.hook(M.State.bots(U.Ui.state(u)), U.Ui.scratch(u, "@hook.tokfor"))), bot))),
    ty, U.Ui.scratch(u, "@hook.tokurl"), U.Ui.scratch(u, "@hook.token"), U.Ui.scratch(u, "@hook.tokfor"), c, x, w, y)

def Bv.htab(+ty: L.Ty, +f: F.FS, +bot: String, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  +u = F.FS.ui(f)
  +hs = BU.BU.hooks(u, bot)
  +nw = L.Lay.button.w(ty, "New webhook")
  L.Lay.col.cat(L.Out.all([Bv.h2(ty, c, "Webhooks", x, y),
      Bv.faint(ty, c, "Signed POSTs, or ones with the hook's bearer token, start a turn; the body reaches it as data, never as instructions.", x, w, (y + 30 : U32)),
      F.Fm.field(ty, f, c, x, (y + 58 : U32), (w - nw - 8 : U32), "hname", "hook-new", "", "Name, e.g. GitHub pushes"),
      F.Fm.button(ty, c, (x + w - nw : U32), (y + 58 : U32), "New webhook", T.Th.accent(), "hook-new", "")]),
    L.Lay.col.then(Bv.secret(ty, "Secret for /hook/" ++ U.Ui.scratch(u, "@hook.id") ++ ", shown once. Sign with HMAC-SHA256.", U.Ui.scratch(u, "@hook.secret"), c, x, w, (y + 98 : U32)), y2 =>
      L.Lay.col.then(Bv.token(ty, u, bot, c, x, w, y2), y3 => Bv.hooks(hs, ty, U.Side.now(u), c, x, w, y3))))

# Settings
# --------

def Bv.del(armed: Bool, +ty: L.Ty, +name: String, +id: String, +c: D.Clip, +x: U32, +y: U32) -> L.Out:
  match armed:
    case True{}:
      L.Out.cat(F.Fm.button(ty, c, x, y, "Delete " ++ name, T.Th.bad(), "bot-delete", id),
        F.Fm.button(ty, c, (x + L.Lay.button.w(ty, "Delete " ++ name) + 8 : U32), y, "Cancel", T.Th.soft(), "bfield", "delask" ++ Bv.sep()))
    case False{}:
      F.Fm.button(ty, c, x, y, "Delete bot", T.Th.soft(), "bfield", "delask" ++ Bv.sep() ++ "1")

def Bv.peer(+ty: L.Ty, p: BT.Peer, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  match p:
    case BT.Peer{+id, +name, +url, _, +revoked}:
      +rw = L.Lay.button.w(ty, "Revoke")
      L.Col{L.Out.all([
          L.Out.ops(L.Ty.text(ty, 0n, L.Ty.fit(ty, 0n, name, (w - rw - 16 : U32)), c, x, y, 28, Bool.pick(U32, revoked, T.Th.dim(), T.Th.text()))),
          Bv.faint(ty, c, Bool.pick(String, revoked, "revoked", url), x, (w - rw - 16 : U32), (y + 26 : U32)),
          Bool.pick(L.Out, revoked, L.Out.none(), F.Fm.button(ty, c, (x + w - rw : U32), (y + 8 : U32), "Revoke", T.Th.soft(), "peer-revoke", id))]),
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
  L.Lay.col.cat(L.Out.all([Bv.h2(ty, c, "Machines", x, y),
      Bv.faint(ty, c, "Bots here and on linked hubs can talk. An invite links one hub to another.", x, w, (y + 30 : U32)),
      Bv.row2(ty, f, "purl", "peer-invite", "This hub's address as the other reaches it", "Make invite", "", c, x, w, (y + 58 : U32)),
      Bv.row2(ty, f, "invite", "peer-join", "Paste an invite (bp1:...)", "Join", "", c, x, w, (y + 94 : U32))]),
    L.Lay.col.then(Bv.secret(ty, "Invite, shown once: paste it on the other hub", U.Ui.scratch(u, "@peer.invite"), c, x, w, (y + 134 : U32)), y2 =>
      Bv.peers(BU.BU.peers(u), ty, c, x, w, y2)))

def Bv.gurl.k(none: Bool, +ty: L.Ty, +s: String, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  match none:
    case True{}:
      L.Col{L.Out.none(), y}
    case False{}:
      +ow = L.Lay.button.w(ty, "Open")
      +cw = L.Lay.button.w(ty, "Copy")
      L.Col{L.Out.all([
          L.Out.ops(L.Ty.text(ty, 2n, L.Ty.fit(ty, 2n, s, (w - ow - cw - 20 : U32)), c, x, y, 28, T.Th.accent())),
          F.Fm.button(ty, c, (x + w - ow - cw - 8 : U32), y, "Open", T.Th.accent(), "open-url", s),
          F.Fm.button(ty, c, (x + w - cw : U32), y, "Copy", T.Th.soft(), "copy", s)]),
        (y + 36 : U32)}

# the user's own OAuth client, shown on asking
def Bv.gown(open: Bool, +ty: L.Ty, +f: F.FS, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  match open:
    case False{}:
      L.Col{L.Out.none(), y}
    case True{}:
      +hw = U32.shr((w - 8 : U32))
      L.Col{L.Out.all([
          F.Fm.field(ty, f, c, x, y, hw, "gid", "google", "connect", "Client id (Desktop app)"),
          F.Fm.field(ty, f, c, (x + hw + 8 : U32), y, hw, "gsecret", "google", "connect", "Client secret"),
          F.Fm.button(ty, c, x, (y + 36 : U32), "Save client", T.Th.soft(), "google", "config"),
          F.Fm.button(ty, c, (x + L.Lay.button.w(ty, "Save client") + 8 : U32), (y + 36 : U32), "Hide", T.Th.soft(), "bfield", "gown" ++ Bv.sep())]),
        (y + 74 : U32)}

# one signed-in account and its own sign-out
def Bv.gacct(+ty: L.Ty, +e: String, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  +bw = L.Lay.button.w(ty, "Sign out")
  L.Col{L.Out.all([
      L.Out.ops(L.Ty.text(ty, 2n, L.Ty.fit(ty, 2n, e, (w - bw - 12 : U32)), c, x, y, 28, T.Th.text())),
      F.Fm.button(ty, c, (x + w - bw : U32), y, "Sign out", T.Th.soft(), "google", "signout:" ++ e)]),
    (y + 36 : U32)}

def Bv.gaccts(xs: List<&2, String>, +ty: L.Ty, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  match xs:
    case Nil{}:
      L.Col{L.Out.none(), y}
    case e <> t:
      L.Lay.col.then(Bv.gacct(ty, e, c, x, w, y), y2 => Bv.gaccts(t, ty, c, x, w, y2))

# connect with the shared client or the user's own; every account signed
# in, each with its own sign-out
def Bv.google(+ty: L.Ty, +f: F.FS, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  +u = F.FS.ui(f)
  +st = U.Ui.scratch(u, "@google")
  +add = Bool.pick(String, List.is_empty(&2, String, U.Bots.google.accts(u)), "Connect Google", "Add account")
  +cw = L.Lay.button.w(ty, add)
  L.Lay.col.then(L.Col{L.Out.all([Bv.h2(ty, c, "Google", x, y),
      Bv.faint(ty, c, Bool.pick(String, String.is_empty(st), "Gmail and Calendar for bots: connect, then allow access. Connect again for another account.", st), x, w, (y + 30 : U32)),
      F.Fm.button(ty, c, x, (y + 58 : U32), add, T.Th.accent(), "google", "connect"),
      F.Fm.button(ty, c, (x + cw + 8 : U32), (y + 58 : U32), "Own client", T.Th.soft(), "bfield", "gown" ++ Bv.sep() ++ "1")]), (y + 96 : U32)}, +y0 =>
    L.Lay.col.then(Bv.gaccts(U.Bots.google.accts(u), ty, c, x, w, y0), +y1 =>
    L.Lay.col.then(Bv.gown(String.eq(F.Fm.text(f, "gown"), "1"), ty, f, c, x, w, y1), +y2 =>
      L.Lay.col.then(Bv.gurl.k(String.is_empty(U.Ui.scratch(u, "@google.url")), ty, U.Ui.scratch(u, "@google.url"), c, x, w, y2), +y3 =>
        L.Col{Bv.row2(ty, f, "gpaste", "google", "Browser on another device? Paste where it landed", "Finish", "paste", c, x, w, y3), (y3 + 40 : U32)}))))

def Bv.stab(+ty: L.Ty, +f: F.FS, b: BT.Bot, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  match b:
    case BT.Bot{+id, +name, _, +look, _, _, _}:
      +fw = L.Lay.button.w(ty, "New face")
      L.Lay.col.then(L.Col{L.Out.all([
          Bv.h2(ty, c, "Persona", x, y),
          Bv.faint(ty, c, "Who " ++ name ++ " is and how it works. It reads this at the start of every turn.", x, w, (y + 30 : U32)),
          F.Fm.area(ty, f, c, x, (y + 58 : U32), w, 8n, "persona", "bot-edit", "", "A helpful cat"),
          F.Fm.button(ty, c, x, (y + 234 : U32), "Save", T.Th.accent(), "bot-edit", ""),
          F.Fm.button(ty, c, (x + L.Lay.button.w(ty, "Save") + 8 : U32), (y + 234 : U32), "New face", T.Th.soft(), "bot-look", Nat.show(Nat.mod(1n+look, CT.Cat.looks()))),
          Bv.del(String.eq(F.Fm.text(f, "delask"), "1"), ty, name, id, c, (x + L.Lay.button.w(ty, "Save") + fw + 16 : U32), (y + 234 : U32))]), (y + 290 : U32)}, y2 =>
        L.Lay.col.then(Bv.machines(ty, f, c, x, w, y2), y3 =>
          Bv.google(ty, f, c, x, w, (y3 + 16 : U32))))

# Browser
# -------
# The bot's page (layout.bend's Lay.page).

def Bv.browser(+ty: L.Ty, +u: U.Ui, +id: String, +name: String, +x0: U32, +x1: U32, +top: U32, +h: U32) -> L.Out:
  L.Lay.page(ty, u, id, "No page yet. Ask " ++ name ++ " to open one.", x0, x1, top, h)

# A bot
# -----

def Bv.tabs(ts: List<&2, String>, +ty: L.Ty, +cur: String, +x: U32, +y: U32) -> L.Out:
  match ts:
    case Nil{}:
      L.Out.none()
    case +t <> rest:
      L.Out.cat(F.Fm.chip(ty, D.Clip.all(), x, y, BU.BU.tab.label(t), String.eq(t, cur), "bot-tab", t),
        Bv.tabs(rest, ty, cur, (x + L.Lay.button.w(ty, BU.BU.tab.label(t)) + 6 : U32), y))

def Bv.busy(s: M.Status) -> Bool:
  match s:
    case M.StWorking{}:
      True{}
    case M.StQueued{}:
      True{}
    case M.StWaiting{}:
      True{}
    case _:
      False{}

# the stop button, at the header's right (x1: where it ends)
def Bv.stop(on: Bool, +ty: L.Ty, +x1: U32) -> L.Out:
  match on:
    case True{}:
      F.Fm.button(ty, D.Clip.all(), (x1 - 16 - L.Lay.button.w(ty, "Stop") : U32), 12, "Stop", T.Th.soft(), "interrupt", "")
    case False{}:
      L.Out.none()

# where the header's right side ends before the Viewer chip (when shown)
def Bv.right(+viewer: Bool, +ty: L.Ty, +x1: U32) -> U32:
  Bool.pick(U32, viewer, (x1 - 8 - L.Lay.button.w(ty, "Viewer") : U32), x1)

# a bot's header (BU.head, the same for a bot here, one mirrored from a
# linked machine and one only reported by one): its cat (64 px), its name
# with where it lives, its mood (and sub), what goes at its right (Stop),
# its own viewer (board, schematic, 3D, browser, diff) when it has one,
# and its tabs
def Bv.header(+ty: L.Ty, +cats: Map<&2, CA.CatF>, +u: U.Ui, +ms: Nat, hd: BU.Head, +sub: String, right: L.Out, +x0: U32, +x1: U32) -> L.Out:
  match hd:
    case BU.Head{+title, +look, +mood, +note, ts, +tab, +viewer, _}:
      +nw = L.Ty.width(ty, 1n, title)
      L.Out.all([
        L.Out{[D.Draw.rect(D.Clip.all(), x0, 0, (x1 - x0 : U32), Bv.hy(), T.Th.bg(), 255), D.Draw.rect(D.Clip.all(), x0, (Bv.hy() - 1 : U32), (x1 - x0 : U32), 1, T.Th.line(), 255)],
          [L.Lay.hit(x0, 0, (x1 - x0 : U32), Bv.hy(), "", "")]},
        L.Out.ops(CA.Cats.draw(cats, look, mood, ms, D.Clip.all(), (x0 + 20 : U32), 12, 64)),
        L.Out.ops(L.Ty.text(ty, 1n, title, D.Clip.all(), (x0 + 100 : U32), 10, 30, T.Th.text())),
        L.Out.ops(L.Ty.text(ty, 2n, note ++ sub, D.Clip.all(), (x0 + 112 + nw : U32), 10, 30, Bv.note.color(mood))),
        right,
        Bv.when(viewer, F.Fm.chip(ty, D.Clip.all(), (x1 - 16 - L.Lay.button.w(ty, "Viewer") : U32), 12, "Viewer", U.Ui.panel(u), "panel", "")),
        Bv.tabs(ts, ty, tab, (x0 + 100 : U32), 48)])

def Bv.tab.n(+t: String) -> Nat:
  S.Nui.pos(BU.BU.tabs(), t, 0n)

# a tab's pane (Chat is the thread itself, laid out by Bv.bot.k)
def Bv.pane.k(k: Nat, +ty: L.Ty, +f: F.FS, +b: BT.Bot, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  match k:
    case 1n:
      SV.Spv.draw(ty, f, BU.BU.space(F.FS.ui(f), b), BT.Bot.id(b), c, x, w, y)
    case 3n:
      Bv.memory(ty, F.FS.ui(f), BT.Bot.id(b), c, x, w, y)
    case 4n:
      Bv.rtab(ty, f, BT.Bot.id(b), c, x, w, y)
    case 5n:
      Bv.htab(ty, f, BT.Bot.id(b), c, x, w, y)
    case _:
      Bv.stab(ty, f, b, c, x, w, y)

def Bv.bot.k(k: Nat, +ty: L.Ty, +memo: L.TLMemo, +pl: PC.Plane, +n: S.Nui, +b: BT.Bot, +x0: U32, +x1: U32) -> L.Out:
  match k:
    case 0n:
      L.Out.hits_below(Bv.hy(), L.Lay.thread.at(ty, memo, n, x0, x1, (Bv.hy() - 52 : U32)))
    case 2n:
      Bv.browser(ty, S.Nui.ui(n), BT.Bot.id(b), BT.Bot.name(b), x0, x1, (Bv.hy() + 16 : U32), S.Nui.height(n))
    case 7n:
      +top = {(Bv.hy() + 24 : U32) : U32}
      +off = Bv.off(S.Nui.ui(n))
      Bv.pane(PV.Pl.draw(pl, PC.Plane.home(S.Nui.ui(n)), ty, D.Clip{x0, Bv.hy(), x1, S.Nui.height(n)}, (x0 + 24 : U32), (x1 - x0 - 48 : U32), top, off, L.U32.dim(S.Nui.height(n), top), U.Side.now(S.Nui.ui(n))),
        x0, x1, Bv.hy(), off, S.Nui.height(n))
    case +j:
      +top = {(Bv.hy() + 24 : U32) : U32}
      +off = Bv.off(S.Nui.ui(n))
      Bv.pane(Bv.pane.k(j, ty, F.Fm.fs(n), b, D.Clip{x0, Bv.hy(), x1, S.Nui.height(n)}, Bv.pane.x(x0, x1), Bv.pane.w(x0, x1), (top - off : U32)),
        x0, x1, Bv.hy(), off, S.Nui.height(n))

# a bot, here or mirrored from a linked machine: its header, then the tab
# BU.head picks (the tabs it has)
def Bv.bot.h(+ty: L.Ty, +memo: L.TLMemo, +pl: PC.Plane, +cats: Map<&2, CA.CatF>, +n: S.Nui, +b: BT.Bot, +hd: BU.Head, +x0: U32, +x1: U32) -> L.Out:
  +u = S.Nui.ui(n)
  +st = U.Ui.state(u)
  +th = BT.Bot.thread(b)
  L.Out.cat(Bv.bot.k(Bv.tab.n(BU.Head.tab(hd)), ty, memo, pl, n, b, x0, x1),
    Bv.header(ty, cats, u, S.Nui.ms_of(n), hd, " · " ++ M.Thread.provider(U.Ui.thread(st, th)),
      Bv.stop(Bv.busy(U.Side.status(st, U.Ui.thread(st, th))), ty, Bv.right(BU.Head.viewer(hd), ty, x1)), x0, x1))

def Bv.bot(+ty: L.Ty, +memo: L.TLMemo, +pl: PC.Plane, +cats: Map<&2, CA.CatF>, +n: S.Nui, +b: BT.Bot, +x0: U32, +x1: U32) -> L.Out:
  Bv.bot.h(ty, memo, pl, cats, n, b, BU.BU.head.bot(S.Nui.ui(n), b), x0, x1)

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

def Bv.wraps.none() -> Map<&2, PW>:
  Map.new(&2, PW)

# the plane kept with the posts (Bv.plane.keep), an empty one before any
def Bv.plane.none() -> PC.Plane:
  PC.Plane{Nil{}, Nil{}, Nil{}, Nil{}, Nil{}, Nil{}, 0n, Nil{}, Nil{}, 0n}

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

def Bv.sd.w(+ty: L.Ty, +cats: Map<&2, CA.CatF>, +n: S.Nui, +ws: Map<&2, PW>) -> Sd:
  Sd{ty, cats, F.Fm.fs(n), S.Nui.ms_of(n), BU.BU.open(S.Nui.ui(n), "@bnew"), BU.BU.open(S.Nui.ui(n), "@rnew"), BU.BU.rows(S.Nui.ui(n)), BU.BU.rooms(S.Nui.ui(n)),
    BU.BU.uber(S.Nui.ui(n)), Bv.plane.words(ws), U.BoardUi.entry(S.Nui.ui(n)), U.BoardUi.is(S.Nui.ui(n))}


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
      PostL{id, BU.BU.member(u, from), U.Time.ago(now, at), Bv.wrap(ws, ty, id, text, w)} <> Bv.posts.l(t, ws, ty, u, now, w)

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
    Bv.row2(ty, f, name, action, ph, "Send", "", D.Clip.all(), x, w, (h - 44 : U32)))

def Bv.top(+ty: L.Ty, +title: String, +sub: String, +x0: U32, +x1: U32, right: L.Out) -> L.Out:
  +tw = L.Ty.width(ty, 1n, title)
  L.Out.all([L.Out.ops([D.Draw.rect(D.Clip.all(), x0, 0, (x1 - x0 : U32), 52, T.Th.bg(), 255), D.Draw.rect(D.Clip.all(), x0, 51, (x1 - x0 : U32), 1, T.Th.line(), 255)]),
    L.Out.ops(L.Ty.text(ty, 1n, title, D.Clip.all(), (x0 + 24 : U32), 0, 52, T.Th.text())),
    L.Out.ops(L.Ty.text(ty, 2n, L.Ty.fit(ty, 2n, sub, L.U32.dim((x1 - x0 : U32), (tw + 200 : U32))), D.Clip.all(), (x0 + 36 + tw : U32), 0, 52, T.Th.faint())),
    right])

def Bv.room(+ty: L.Ty, +ws: Map<&2, PW>, +n: S.Nui, r: BT.Room, +x0: U32, +x1: U32) -> L.Out:
  match r:
    case BT.Room{+id, +name, ms, cr, de}:
      +u = S.Nui.ui(n)
      +h = S.Nui.height(n)
      +dw = L.Lay.button.w(ty, "Delete room")
      L.Out.all([
        Bv.chat(ty, Bv.wraps.group(ws, id, Bv.pane.w(x0, x1)), BU.BU.posts(u, id), u, "Nothing said here yet. Every bot in the room hears what is posted.", x0, x1, 52, (h - 60 : U32)),
        Bv.say(ty, F.Fm.fs(n), "rpost", "room-post", "Write to the room", x0, x1, h),
        Bv.top(ty, "# " ++ name, BU.BU.members(u, BT.Room{id, name, ms, cr, de}), x0, x1,
          F.Fm.button(ty, D.Clip.all(), (x1 - 16 - dw : U32), 12, "Delete room", T.Th.soft(), "room-delete", id))])

# a bot only reported by a linked machine: the bot's view (BU.head: its
# cat, "name · machine", its mood, Chat alone) over the person's
# conversation with it
def Bv.remote(+ty: L.Ty, +ws: Map<&2, PW>, +cats: Map<&2, CA.CatF>, +n: S.Nui, +id: String, +x0: U32, +x1: U32) -> L.Out:
  +u = S.Nui.ui(n)
  +h = S.Nui.height(n)
  L.Out.all([
    Bv.chat(ty, Bv.wraps.group(ws, BT.Room.dm("you", id), Bv.pane.w(x0, x1)), BU.BU.remote.posts(u, id), u, "Write to " ++ id ++ ". It answers from its own machine.", x0, x1, Bv.hy(), (h - 60 : U32)),
    Bv.say(ty, F.Fm.fs(n), "rtext", "remote-send", "Write to " ++ id, x0, x1, h),
    Bv.header(ty, cats, u, S.Nui.ms_of(n), BU.BU.head(u, BU.VRemote{id}), "", L.Out.none(), x0, x1)])

# The Board (core/tbdata.bend, client.bend's BoardUi)
# -------------------------------------------------
# The main area while the scratch "@board" is on: the filter chips, the
# field that adds an item, and the sections. A row is a status box (empty
# todo, half doing, ✓ done, ! blocked, – dropped), the title and, dimmed, its
# owner, project, due date and group (overdue in red); the box ticks it on,
# the row opens its detail: title, notes, links and Blocked, Drop, Delete.
# Positions are `rel` from the top of the content, which sits at `top`,
# scrolled up by `off`; only the pieces on show are drawn (as Pl.* does),
# and none whose top has scrolled above the window.

# what every piece needs: the faces and fields, the clip, the column (x, w)
# and the part of the content on show
type TbC is Data:
  TbC{ty: L.Ty, f: F.FS, c: D.Clip, x: U32, w: U32, top: U32, off: U32, vh: U32}

def TbC.y(+k: TbC, +rel: U32) -> U32:
  match k:
    case TbC{_, _, _, _, _, +top, +off, _}:
      L.U32.dim((top + rel : U32), off)

def TbC.x(+k: TbC) -> U32:
  match k:
    case TbC{_, _, _, +x, _, _, _, _}:
      x

# a piece of height h at rel is on show
def TbC.vis(+k: TbC, +rel: U32, +h: U32) -> Bool:
  match k:
    case TbC{_, _, _, _, _, +top, +off, +vh}:
      Bool.and(PV.Pl.vis(rel, h, off, vh), U32.is_ge((top + rel : U32), off))

def Bv.tb.rh() -> U32:
  50

def Bv.tb.color(+s: String) -> U32:
  Bool.pick(U32, String.eq(s, "doing"), T.Th.accent(), Bool.pick(U32, String.eq(s, "done"), T.Th.ok(), Bool.pick(U32, String.eq(s, "blocked"), T.Th.warn(), T.Th.faint())))

# the status box: 18 px, square, what the status looks like inside
def Bv.tb.mark(+ty: L.Ty, +c: D.Clip, +s: String, +x: U32, +y: U32) -> List<&2, R.Op>:
  List.concat(&2, R.Op, [L.Lay.box(c, x, y, 18, 18, 0, T.Th.bg(), Bv.tb.color(s)),
    Bool.pick(List<&2, R.Op>, String.eq(s, "doing"), [D.Draw.rect(c, (x + 3 : U32), (y + 3 : U32), 6, 12, T.Th.accent(), 255)], Nil{}),
    Bool.pick(List<&2, R.Op>, String.eq(s, "done"), L.Lay.icon(ty, "check", c, (x + 1 : U32), (y + 1 : U32), T.Th.ok()), Nil{}),
    Bool.pick(List<&2, R.Op>, String.eq(s, "blocked"), L.Ty.text(ty, 2n, "!", c, (x + 7 : U32), (y - 1 : U32), 20, T.Th.warn()), Nil{}),
    Bool.pick(List<&2, R.Op>, String.eq(s, "dropped"), [D.Draw.rect(c, (x + 4 : U32), (y + 8 : U32), 10, 2, T.Th.faint(), 255)], Nil{})])

def Bv.tb.sub.color(+soon: Bool, +over: Bool) -> U32:
  Bool.pick(U32, over, T.Th.bad(), Bool.pick(U32, soon, T.Th.warn(), T.Th.faint()))

def Bv.tb.row.out(r: U.BRow, +k: TbC, +y: U32) -> L.Out:
  match r:
    case U.BRow{+id, +title, +sub, +status, +soon, +over, +open, _, _, _}:
      match k:
        case TbC{+ty, _, +c, +x, +w, _, _, _}:
          +gone = {Bool.or(String.eq(status, "done"), String.eq(status, "dropped")) : Bool}
          L.Out.all([
            L.Out{List.concat(&2, R.Op, [Bool.pick(List<&2, R.Op>, open, [D.Draw.rect(c, x, y, w, Bv.tb.rh(), T.Th.panel(), 255)], Nil{}),
                [D.Draw.rect(c, x, (y + Bv.tb.rh() - 1 : U32), w, 1, T.Th.line(), 255)],
                L.Ty.text(ty, 0n, L.Ty.fit(ty, 0n, title, (w - 56 : U32)), c, (x + 44 : U32), y, 28, Bool.pick(U32, gone, T.Th.faint(), T.Th.text())),
                L.Ty.text(ty, 2n, L.Ty.fit(ty, 2n, sub, (w - 56 : U32)), c, (x + 44 : U32), (y + 24 : U32), 20, Bv.tb.sub.color(soon, over))]),
              [L.Lay.hit(x, y, w, Bv.tb.rh(), "tb-row", id)]},
            L.Out{Bv.tb.mark(ty, c, status, (x + 12 : U32), (y + 14 : U32)), [L.Lay.hit(x, y, 40, Bv.tb.rh(), "tb-tick", id)]}])

# a piece of out, when it is on show
def Bv.tb.if(on: Bool, r: U.BRow, +k: TbC, +rel: U32) -> L.Out:
  match on:
    case True{}:
      Bv.tb.row.out(r, k, TbC.y(k, rel))
    case False{}:
      L.Out.none()

def Bv.tb.nl(ls: List<&2, String>) -> U32:
  U32.from_nat(List.length(&2, String, ls))

# an open row's detail pieces: the title and Rename, the notes and Save
# notes, the links, the status buttons
def Bv.tb.dh(+links: List<&2, String>) -> U32:
  (6 + 34 + 6 + 72 + 6 + 34 + Bv.tb.nl(links) * 22 + 6 + 34 + 8 : U32)

def Bv.tb.link(+k: TbC, l: String, +y: U32) -> L.Out:
  match k:
    case TbC{+ty, _, +c, +x, +w, _, _, _}:
      L.Out{L.Ty.text(ty, 2n, L.Ty.fit(ty, 2n, l, (w - 52 : U32)), c, (x + 44 : U32), y, 22, T.Th.accent()), [L.Lay.hit((x + 44 : U32), y, (w - 52 : U32), 22, U.BoardUi.link.act(l), l)]}

def Bv.tb.link.if(on: Bool, +k: TbC, l: String, +y: U32) -> L.Out:
  match on:
    case True{}:
      Bv.tb.link(k, l, y)
    case False{}:
      L.Out.none()

def Bv.tb.links(ls: List<&2, String>, +k: TbC, +rel: U32) -> L.Out:
  match ls:
    case Nil{}:
      L.Out.none()
    case l <> t:
      L.Out.cat(Bv.tb.link.if(TbC.vis(k, rel, 22), k, l, TbC.y(k, rel)), Bv.tb.links(t, k, (rel + 22 : U32)))

def Bv.tb.part.if(on: Bool, o: L.Out) -> L.Out:
  match on:
    case True{}:
      o
    case False{}:
      L.Out.none()

def Bv.tb.title.out(+k: TbC, +id: String, +y: U32) -> L.Out:
  match k:
    case TbC{+ty, +f, +c, +x, +w, _, _, _}:
      +rw = L.Lay.button.w(ty, "Rename")
      L.Out.cat(F.Fm.field(ty, f, c, (x + 44 : U32), y, (w - 44 - rw - 14 : U32), "tbedit", "tb-title", "", "Title"),
        F.Fm.button(ty, c, (x + w - 6 - rw : U32), y, "Rename", T.Th.soft(), "tb-title", id))

def Bv.tb.notes.out(+k: TbC, +y: U32) -> L.Out:
  match k:
    case TbC{+ty, +f, +c, +x, +w, _, _, _}:
      F.Fm.area(ty, f, c, (x + 44 : U32), y, (w - 52 : U32), 3n, "tbnotes", "", "", "Notes")

def Bv.tb.save.out(+k: TbC, +id: String, +y: U32) -> L.Out:
  match k:
    case TbC{+ty, _, +c, +x, +w, _, _, _}:
      F.Fm.button(ty, c, (x + w - 6 - L.Lay.button.w(ty, "Save notes") : U32), y, "Save notes", T.Th.soft(), "tb-notes", id)

def Bv.tb.btns.out(+k: TbC, +id: String, +status: String, +armed: Bool, +y: U32) -> L.Out:
  match k:
    case TbC{+ty, _, +c, +x, +w, _, _, _}:
      +dl = Bool.pick(String, armed, "Delete?", "Delete")
      +first = Bool.pick(String, String.eq(status, "blocked"), "Unblock", "Blocked")
      L.Out.all([
        F.Fm.button(ty, c, (x + 44 : U32), y, first, T.Th.soft(), "tb-status", id ++ "\u{9}" ++ Bool.pick(String, String.eq(status, "blocked"), "todo", "blocked")),
        F.Fm.button(ty, c, (x + 54 + L.Lay.button.w(ty, first) : U32), y, Bool.pick(String, String.eq(status, "dropped"), "Reopen", "Drop"), T.Th.soft(), "tb-status",
          id ++ "\u{9}" ++ Bool.pick(String, String.eq(status, "dropped"), "todo", "dropped")),
        F.Fm.button(ty, c, (x + w - 6 - L.Lay.button.w(ty, dl) : U32), y, dl, Bool.pick(U32, armed, T.Th.bad(), T.Th.soft()), "tb-del", id)])

def Bv.tb.detail(r: U.BRow, +k: TbC, +rel: U32) -> L.Out:
  match r:
    case U.BRow{+id, _, _, +status, _, _, _, _, +links, +armed}:
      +ly = {(rel + 6 + 34 + 6 + 72 + 6 + 34 : U32) : U32}
      L.Out.all([
        Bv.tb.part.if(TbC.vis(k, (rel + 6 : U32), 28), Bv.tb.title.out(k, id, TbC.y(k, (rel + 6 : U32)))),
        Bv.tb.part.if(TbC.vis(k, (rel + 46 : U32), 72), Bv.tb.notes.out(k, TbC.y(k, (rel + 46 : U32)))),
        Bv.tb.part.if(TbC.vis(k, (rel + 124 : U32), 28), Bv.tb.save.out(k, id, TbC.y(k, (rel + 124 : U32)))),
        Bv.tb.links(links, k, ly),
        Bv.tb.part.if(TbC.vis(k, (ly + Bv.tb.nl(links) * 22 + 6 : U32), 28), Bv.tb.btns.out(k, id, status, armed, TbC.y(k, (ly + Bv.tb.nl(links) * 22 + 6 : U32))))])

def Bv.tb.open.if(open: Bool, r: U.BRow, +k: TbC, +rel: U32) -> L.Out:
  match open:
    case True{}:
      Bv.tb.detail(r, k, rel)
    case False{}:
      L.Out.none()

def Bv.tb.row.k(r: U.BRow, +k: TbC, +rel: U32) -> L.Col:
  match r:
    case U.BRow{_, _, _, _, _, _, +open, _, +links, _}:
      L.Col{L.Out.cat(Bv.tb.if(TbC.vis(k, rel, Bv.tb.rh()), r, k, rel), Bv.tb.open.if(open, r, k, (rel + Bv.tb.rh() : U32))),
        (rel + Bv.tb.rh() + Bool.pick(U32, open, Bv.tb.dh(links), 0) : U32)}

def Bv.tb.rows(rs: List<&2, U.BRow>, +k: TbC, +rel: U32) -> L.Col:
  match rs:
    case Nil{}:
      L.Col{L.Out.none(), rel}
    case r <> t:
      L.Lay.col.then(Bv.tb.row.k(r, k, rel), +y2 =>
        Bv.tb.rows(t, k, y2))

def Bv.tb.label.if(on: Bool, +k: TbC, +s: String, +y: U32) -> L.Out:
  match on:
    case True{}:
      match k:
        case TbC{+ty, _, +c, +x, _, _, _, _}:
          L.Out.ops(L.Ty.text(ty, 2n, s, c, (x + 4 : U32), y, 28, T.Th.dim()))
    case False{}:
      L.Out.none()

# a section: its title and count, then its rows
def Bv.tb.sec(s: U.BSec, +k: TbC, +rel: U32) -> L.Col:
  match s:
    case U.BSec{+title, +n, rs}:
      L.Lay.col.cat(Bv.tb.label.if(TbC.vis(k, rel, 28), k, title ++ " " ++ Nat.show(n), TbC.y(k, rel)),
        L.Lay.col.then(Bv.tb.rows(rs, k, (rel + 28 : U32)), +y2 =>
          L.Col{L.Out.none(), (y2 + 12 : U32)}))

def Bv.tb.secs(ss: List<&2, U.BSec>, +k: TbC, +rel: U32) -> L.Col:
  match ss:
    case Nil{}:
      L.Col{L.Out.none(), rel}
    case s <> t:
      L.Lay.col.then(Bv.tb.sec(s, k, rel), +y2 =>
        Bv.tb.secs(t, k, y2))

# the filter chips, wrapped at the column's right edge; the rows they take
def Bv.tb.chips.wrap(+k: TbC, +l: String, +x: U32) -> Bool:
  match k:
    case TbC{+ty, _, _, +x0, +w, _, _, _}:
      Bool.and(U32.is_gt(x, x0), U32.is_gt((x + L.Lay.button.w(ty, l) : U32), (x0 + w : U32)))

def Bv.tb.chips(cs: List<&2, U.BChip>, +k: TbC, +x: U32, +y: U32, +n: U32) -> L.Col:
  match cs:
    case Nil{}:
      L.Col{L.Out.none(), (n + 1 : U32)}
    case U.BChip{+l, +a, +v, +on} <> t:
      match k:
        case TbC{+ty, _, +c, +x0, _, _, _, _}:
          +wr = Bv.tb.chips.wrap(k, l, x)
          +cx = {Bool.pick(U32, wr, x0, x) : U32}
          +cy = {Bool.pick(U32, wr, (y + 34 : U32), y) : U32}
          L.Lay.col.cat(F.Fm.chip(ty, c, cx, cy, l, on, a, v), Bv.tb.chips(t, k, (cx + L.Lay.button.w(ty, l) + 8 : U32), cy, (n + Bool.pick(U32, wr, 1, 0) : U32)))

def Bv.tb.add.out(+k: TbC, +y: U32) -> L.Out:
  match k:
    case TbC{+ty, +f, +c, +x, +w, _, _, _}:
      +aw = L.Lay.button.w(ty, "Add")
      L.Out.cat(F.Fm.field(ty, f, c, x, y, (w - aw - 8 : U32), "tbtitle", "tb-add", "", U.BoardUi.hint()), F.Fm.button(ty, c, (x + w - aw : U32), y, "Add", T.Th.accent(), "tb-add", ""))

def Bv.tb.empty.if(none: Bool, +k: TbC, +y: U32) -> L.Out:
  match none:
    case True{}:
      match k:
        case TbC{+ty, _, +c, +x, _, _, _, _}:
          L.Out.ops(L.Ty.text(ty, 0n, U.BoardUi.empty(), c, (x + 4 : U32), y, 28, T.Th.faint()))
    case False{}:
      L.Out.none()

# the content: add field, chips, sections
def Bv.tb.col(+u: U.Ui, +k: TbC) -> L.Col:
  +ss = U.BoardUi.sections(u)
  L.Lay.col.cat(Bv.tb.part.if(TbC.vis(k, 16, 28), Bv.tb.add.out(k, TbC.y(k, 16))),
    L.Lay.col.then(Bv.tb.chips(U.BoardUi.chips(u), k, TbC.x(k), TbC.y(k, 60), 0), +rows =>
      L.Lay.col.then(L.Col{L.Out.none(), (60 + rows * 34 + 10 : U32)}, +r1 =>
        L.Lay.col.cat(Bv.tb.empty.if(List.is_empty(&2, U.BSec, ss), k, TbC.y(k, r1)),
          Bv.tb.secs(ss, k, Bool.pick(U32, List.is_empty(&2, U.BSec, ss), (r1 + 40 : U32), r1)))))

def Bv.board.head(+ty: L.Ty, +u: U.Ui, +x0: U32, +x1: U32) -> L.Out:
  L.Out.all([
    L.Out{[D.Draw.rect(D.Clip.all(), x0, 0, (x1 - x0 : U32), 51, T.Th.bg(), 255), D.Draw.rect(D.Clip.all(), x0, 51, (x1 - x0 : U32), 1, T.Th.line(), 255)], [L.Lay.hit(x0, 0, (x1 - x0 : U32), 52, "", "")]},
    L.Out.ops(L.Ty.text(ty, 0n, U.BoardUi.title(), D.Clip.all(), (x0 + 24 : U32), 0, 52, T.Th.text())),
    L.Out.ops(L.Ty.text(ty, 2n, Nat.show(U.BoardUi.count(u)) ++ " open", D.Clip.all(), (x0 + 24 + L.Ty.width(ty, 0n, U.BoardUi.title()) + 12 : U32), 0, 52, T.Th.faint())),
    L.Lay.button(ty, D.Clip.all(), (x1 - 16 - L.Lay.button.w(ty, "Close") : U32), 12, "Close", T.Th.soft(), "tb-close", "")])

# the Board in the main area
def Bv.board(+ty: L.Ty, +n: S.Nui, +x0: U32, +x1: U32) -> L.Out:
  +u = S.Nui.ui(n)
  +h = {S.Nui.height(n) : U32}
  +k = {TbC{ty, F.Fm.fs(n), D.Clip.all(), Bv.pane.x(x0, x1), Bv.pane.w(x0, x1), 52, Bv.off(u), L.U32.dim(h, 52)} : TbC}
  L.Out.cat(Bv.pane(Bv.tb.col(u, k), x0, x1, 52, Bv.off(u), h), Bv.board.head(ty, u, x0, x1))

# The main area and the frame
# ---------------------------

def Bv.view(v: BU.View, +ty: L.Ty, +memo: L.TLMemo, +cats: Map<&2, CA.CatF>, +ws: Map<&2, PW>, +n: S.Nui, +x1: U32) -> Maybe<&2, L.Out>:
  match v:
    case BU.VThread{}:
      None{}
    case BU.VBot{b}:
      Some{Bv.bot(ty, memo, Bv.plane.get(ws), cats, n, b, 264, x1)}
    case BU.VRoom{r}:
      Some{Bv.room(ty, ws, n, r, 264, x1)}
    case BU.VRemote{+id}:
      Some{Bv.remote(ty, ws, cats, n, id, 264, x1)}

# the Board, when it is open, else a bot's, a room's or a remote bot's
def Bv.main.b(board: Bool, +ty: L.Ty, +memo: L.TLMemo, +cats: Map<&2, CA.CatF>, +ws: Map<&2, PW>, +n: S.Nui) -> Maybe<&2, L.Out>:
  match board:
    case True{}:
      Some{Bv.board(ty, L.Lay.shrink(n, L.Lay.term_h(n)), 264, L.Lay.x1(n))}
    case False{}:
      Bv.view(BU.BU.view(S.Nui.ui(n)), ty, memo, cats, ws, L.Lay.shrink(n, L.Lay.term_h(n)), L.Lay.x1(n))

def Bv.main.k(settings: Bool, +ty: L.Ty, +memo: L.TLMemo, +cats: Map<&2, CA.CatF>, +ws: Map<&2, PW>, +n: S.Nui) -> Maybe<&2, L.Out>:
  match settings:
    case True{}:
      None{}
    case False{}:
      Bv.main.b(U.BoardUi.is(S.Nui.ui(n)), ty, memo, cats, ws, n)

# a bot's, room's or remote bot's main area (None: the usual one)
def Bv.main.w(+ty: L.Ty, +memo: L.TLMemo, +cats: Map<&2, CA.CatF>, +ws: Map<&2, PW>, +n: S.Nui) -> Maybe<&2, L.Out>:
  Bv.main.k(U.Ui.flag(S.Nui.ui(n), "settings"), ty, memo, cats, ws, n)

def Bv.main(+ty: L.Ty, +memo: L.TLMemo, +cats: Map<&2, CA.CatF>, +n: S.Nui) -> Maybe<&2, L.Out>:
  Bv.main.w(ty, memo, cats, Bv.wraps.none(), n)

def Bv.frame.k(+sd: Sd, +ty: L.Ty, +memo: L.TLMemo, +cats: Map<&2, CA.CatF>, +ws: Map<&2, PW>, +n: S.Nui) -> L.Out:
  L.Lay.frame.u(ty, memo, n, Bv.uber.out(sd), Bv.uber.top(sd), c => y => Bv.side.of(sd, c, y), Bv.main.w(ty, memo, cats, ws, n))

# the whole window, topmost first, with the bots in it; ws: the posts
# wrapped for the frame before (Bv.wraps)
def Bv.frame.w(+ty: L.Ty, +memo: L.TLMemo, +cats: Map<&2, CA.CatF>, +ws: Map<&2, PW>, +n: S.Nui) -> L.Out:
  Bv.frame.k(Bv.sd.w(ty, cats, n, ws), ty, memo, cats, ws, n)

# (every post wrapped afresh)
def Bv.frame(+ty: L.Ty, +memo: L.TLMemo, +cats: Map<&2, CA.CatF>, +n: S.Nui) -> L.Out:
  Bv.frame.w(ty, memo, cats, Bv.wraps.none(), n)

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
      String.eq(BU.BU.tab.of(u, BT.Bot.id(b)), "status")
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

def Bv.plane.keep.k(same: Bool, +ws: Map<&2, PW>, +u: U.Ui, +key: String) -> Map<&2, PW>:
  match same:
    case True{}:
      ws
    case False{}:
      Map.set(&2, PW, Map.set(&2, PW, ws, "@plane", PWPlane{key, Bv.plane.build(Bv.plane.shown(u), u)}), "@plane.n", PWCounts{key, PC.Plane.tally.words(u, U.Side.now(u))})

def Bv.plane.keep(+ws: Map<&2, PW>, +u: U.Ui) -> Map<&2, PW>:
  +key = {PC.Plane.key(u, U.Side.now(u)) ++ Bool.pick(String, Bv.plane.shown(u), "|s", "") : String}
  Bv.plane.keep.k(String.eq(Bv.plane.key(ws), key), ws, u, key)

def Bv.plane.refresh(m: Maybe<&2, BU.BRow>, +ws: Map<&2, PW>, +u: U.Ui) -> Map<&2, PW>:
  match m:
    case Some{_}:
      Bv.plane.keep(ws, u)
    case None{}:
      ws

def Bv.wraps(+ty: L.Ty, +n: S.Nui, +ws: Map<&2, PW>) -> Map<&2, PW>:
  Bv.plane.refresh(BU.BU.uber(S.Nui.ui(n)), Bv.wraps.k(U.Ui.flag(S.Nui.ui(n), "settings"), ty, n, ws), S.Nui.ui(n))
