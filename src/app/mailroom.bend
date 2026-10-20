import Base
import ../core/client.bend as U
import ../core/mailview.bend as MV
import ../core/mail.bend as ML
import ../core/model.bend as M
import ../core/bot.bend as BT
import ../core/plane.bend as P
import ../core/uber.bend as UB
import ../gfx/draw.bend as D
import ../gfx/raster.bend as R
import ./layout.bend as L
import ./forms.bend as F
import ./nui.bend as N
import ./theme.bend as T
import ./plane.bend as PV
import ./mm.bend as MM
import ../core/json.bend as J

# Native Mailroom. The host supplies its existing PW/@plane value.
# Only a visible MV slice is fetched/drawn; bodies are already parsed and
# grouped in Ui's typed Mailv store. No timeline, tiles or frame hooks.
def Mr.all(+p: P.Plane) -> List<&2, P.PRow>:
  List.concat(&2, P.PRow, [P.Plane.need(p), P.Plane.run(p), P.Plane.locks(p), P.Plane.routines(p), P.Plane.rooms(p), P.Plane.idle(p), P.Plane.bots(p)])

def Mr.state(r: P.PRow) -> String:
  match r:
    case P.PRow{_, _, _, _, _, _, _, s, _, _}: s

def Mr.keep(on: Bool, r: P.PRow, rest: List<&2, P.PRow>) -> List<&2, P.PRow>:
  match on:
    case True{}: r <> rest
    case False{}: rest

def Mr.routines(rs: List<&2, P.PRow>, +running: Bool) -> List<&2, P.PRow>:
  match rs:
    case Nil{}: Nil{}
    case +r <> t:
      Mr.keep(Bool.not(Bool.xor(String.eq(Mr.state(r), "working"), running)), r, Mr.routines(t, running))

def Mr.running(+p: P.Plane) -> List<&2, P.PRow>:
  List.concat(&2, P.PRow, [P.Plane.run(p), P.Plane.locks(p), Mr.routines(P.Plane.routines(p), True{})])

def Mr.run_owned(r: Maybe<&2, ML.Run>, +bot: String) -> Bool:
  match r:
    case Some{x}: String.eq(ML.Run.bot(x), bot)
    case None{}: False{}

def Mr.owned(+r: P.PRow, +u: U.Ui) -> Bool:
  match r:
    case P.PRow{_, +id, _, +val, _, _, +machine, _, _, _}:
      Bool.and(Bool.or(String.is_empty(U.Ui.scratch(u, "@mr.machine")), String.eq(machine, U.Ui.scratch(u, "@mr.machine"))),
        Bool.or(String.eq(U.Mailv.bot(u), "*"), Bool.or(String.eq(id, U.Mailv.bot(u)), Bool.or(String.eq(id, BT.Bot.thread(BT.World.bot(M.State.bots(U.Ui.state(u)), U.Mailv.bot(u)))),
          Bool.or(String.starts_with(val, U.Mailv.bot(u) ++ "|"), Mr.run_owned(ML.Box.run_of(M.State.mail(U.Ui.state(u)), id), U.Mailv.bot(u)))))))

def Mr.for_bot(rs: List<&2, P.PRow>, +u: U.Ui) -> List<&2, P.PRow>:
  match rs:
    case Nil{}: Nil{}
    case +r <> t: Mr.keep(Mr.owned(r, u), r, Mr.for_bot(t, u))

def Mr.chip_at(i: Nat, +p: P.Plane) -> List<&2, P.PRow>:
  match i:
    case 0n: P.Plane.idle(p)
    case 1n: Mr.routines(P.Plane.routines(p), False{})
    case 2n: P.Plane.rooms(p)
    case 3n: Mr.all(p)
    case _: Nil{}

# Chips retain every Plane row's state and native click action. Rooms are
# also shown under Unread until their room: conversation has been filed.
def Mr.chip_rows(+p: P.Plane, +status: String) -> List<&2, P.PRow>:
  Mr.chip_at(U.Keys.pos(["today", "due", "unread", "overview"], status), p)

def Mr.label(+ty: L.Ty, +c: D.Clip, +s: String, +x: U32, +y: U32) -> L.Out:
  F.Fm.label(ty, c, s, x, y, T.Th.dim())

def Mr.machines(ms: List<&2, P.PMach>, +ty: L.Ty, +u: U.Ui, +c: D.Clip, +x: U32, +y: U32) -> L.Out:
  match ms:
    case Nil{}: L.Out.none()
    case P.PMach{+name, +state} <> t:
      +w = (L.Ty.width(ty, 2n, name) + 32 : U32)
      L.Out.cat(L.Out{List.append(&2, R.Op, [D.Draw.disc(c, U32.to_f32((x + 5 : U32)), U32.to_f32((y + 14 : U32)), 4.0, PV.Pl.machine.dot(state), 255)], L.Ty.text(ty, 2n, name, c, (x + 16 : U32), y, 28, T.Th.dim())), [L.Lay.hit(x, y, w, 28, "mail-filter", "machine=" ++ Bool.pick(String, String.eq(state, "here"), "", name))]}, Mr.machines(t, ty, u, c, (x + w : U32), y))

# The same zero-alpha cat anchor protocol as Bv.Spot. The existing cat
# pass draws these; Mailroom introduces no animation clock or frame loop.
def Mr.cat(+b: BT.Bot, c: D.Clip, +x: U32, +y: U32) -> L.Out:
  match c:
    case D.Clip{+a, +bb, +cc, +d}:
      L.Out{[D.Clip.cut(D.Clip{a, bb, cc, d}, x, y, (x + 24 : U32), (y + 24 : U32), U32.or(U32.shln(64, 24n), U32.or(U32.shln(U32.and(x, 4095), 12n), U32.and(y, 4095))), 0, R.SRect{})],
        [L.Lay.hit(0, 0, 0, 0, "cat", Nat.show(BT.Bot.look(b)) ++ "|sleep|24|" ++ U32.show(x) ++ "|" ++ U32.show(y) ++ "|" ++ U32.show(a) ++ "|" ++ U32.show(bb) ++ "|" ++ U32.show(cc) ++ "|" ++ U32.show(d))]}

def Mr.bot_chips(rs: List<&2, P.PRow>, +ty: L.Ty, +u: U.Ui, +c: D.Clip, +x0: U32, +x1: U32, +x: U32, +y: U32) -> L.Col:
  match rs:
    case Nil{}: L.Col{L.Out.none(), (y + 38 : U32)}
    case P.PRow{_, +id, _, _, +title, _, _, +state, _, _} <> t:
      +w = (L.Lay.button.w(ty, title) + 40 : U32)
      +wrap = U32.is_gt((x + w : U32), x1)
      +xx = Bool.pick(U32, wrap, x0, x)
      +yy = Bool.pick(U32, wrap, (y + 34 : U32), y)
      L.Lay.col.cat(L.Out.all([F.Fm.chip(ty, c, (xx + 30 : U32), yy, title, String.eq(U.Mailv.bot(u), id), "mail-filter", "bot=" ++ id),
        Mr.cat(BT.World.bot(M.State.bots(U.Ui.state(u)), id), c, xx, (yy + 2 : U32)),
        L.Out.ops([D.Draw.disc(c, U32.to_f32((xx + 23 : U32)), U32.to_f32((yy + 23 : U32)), 3.0, PV.Pl.dot(state), 255)])]),
        Mr.bot_chips(t, ty, u, c, x0, x1, (xx + w + 8 : U32), yy))

def Mr.chips(ids: List<&2, String>, labels: List<&2, String>, +ty: L.Ty, +u: U.Ui, +c: D.Clip, +x0: U32, +x1: U32, +x: U32, +y: U32) -> L.Col:
  match ids labels:
    case +id <> t +label <> ls:
      +bw = L.Lay.button.w(ty, label)
      +wrap = U32.is_gt((x + bw : U32), x1)
      +xx = Bool.pick(U32, wrap, x0, x)
      +yy = Bool.pick(U32, wrap, (y + 34 : U32), y)
      L.Lay.col.cat(F.Fm.chip(ty, c, xx, yy, label, String.eq(U.Ui.scratch(u, "@mr.status"), id), "mail-filter", id), Mr.chips(t, ls, ty, u, c, x0, x1, (xx + bw + 6 : U32), yy))
    case _ _: L.Col{L.Out.none(), (y + 38 : U32)}

def Mr.rows(rs: List<&2, P.PRow>, +ty: L.Ty, +u: U.Ui, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  match rs:
    case Nil{}: L.Col{L.Out.none(), y}
    case r <> t:
      L.Lay.col.cat(PV.Pl.row(False{}, r, ty, c, x, w, y, U.Side.now(u)), Mr.rows(t, ty, u, c, x, w, (y + 30 : U32)))

def Mr.run_rows(open: Bool, +rs: List<&2, P.PRow>, +ty: L.Ty, +u: U.Ui, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  match open:
    case False{}: L.Col{L.Out.none(), y}
    case True{}: Mr.rows(rs, ty, u, c, x, w, y)

# Overscan rows share the list's draw clip; their hits must share it too.
# Otherwise a row above the list can cover search or a pinned control.
def Mr.row.hit(c: D.Clip, +x: U32, +w: U32, +y: U32, id: String) -> N.Hit:
  match c:
    case D.Clip{+x0, +y0, x1, y1}:
      L.Lay.hit(U32.max(x, x0), U32.max(y, y0),
        L.U32.dim(U32.min((x + w : U32), x1), U32.max(x, x0)),
        L.U32.dim(U32.min((y + 52 : U32), y1), U32.max(y, y0)), "mail-open", id)

def Mr.row(+r: MV.Row, +ty: L.Ty, +u: U.Ui, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Out:
  L.Out{List.concat(&2, R.Op, [[D.Draw.rect(c, x, y, w, 52, Bool.pick(U32, String.eq(MV.Row.id(r), U.Ui.scratch(u, "@mr.conv")), T.Th.sel(), T.Th.bg()), 255), D.Draw.rect(c, x, (y + 51 : U32), w, 1, T.Th.line(), 255)],
    [D.Draw.disc(c, U32.to_f32((x + 8 : U32)), U32.to_f32((y + 15 : U32)), 3.0, Bool.pick(U32, MV.Row.unread(r), T.Th.accent(), T.Th.faint()), 255)],
    L.Ty.text(ty, 0n, L.Ty.fit(ty, 0n, MV.Row.subject(r), L.U32.dim(w, 100)), c, (x + 20 : U32), y, 26, Bool.pick(U32, MV.Row.unread(r), T.Th.text(), T.Th.soft())),
    L.Ty.text(ty, 2n, MV.Mv.ago(U.Side.now(u), MV.Row.at(r)), c, (x + w - 65 : U32), y, 26, T.Th.faint()),
    L.Ty.text(ty, 2n, L.Ty.fit(ty, 2n, MV.Row.who(r) ++ " · " ++ MV.Row.project(r) ++ " · " ++ MV.Row.head(r), L.U32.dim(w, 28)), c, (x + 20 : U32), (y + 25 : U32), 24, T.Th.dim())]), [Mr.row.hit(c, x, w, y, MV.Row.id(r))]}

def Mr.row_if(show: Bool, +r: MV.Row, +ty: L.Ty, +u: U.Ui, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Out:
  match show:
    case True{}: Mr.row(r, ty, u, c, x, w, y)
    case False{}: L.Out.none()

def Mr.convs(rs: List<&2, MV.Row>, +ty: L.Ty, +u: U.Ui, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Out:
  match rs:
    case Nil{}: L.Out.none()
    case +r <> t:
      L.Out.cat(Mr.row_if(Bool.not(String.is_empty(MV.Row.id(r))), r, ty, u, c, x, w, y), Mr.convs(t, ty, u, c, x, w, (y + 52 : U32)))

def Mr.list.rows(search: Bool, +u: U.Ui, +first: Nat, +count: Nat) -> List<&2, MV.Row>:
  match search:
    case False{}: MV.Mv.rows(U.Mailv.cache(u), U.Mailv.key(u), first, count)
    case True{}: List.take(&2, MV.Row, List.drop(&2, MV.Row, MV.Find.hits(U.Mailv.find(u)), first), count)

def Mr.list.at(span: MV.Span, +ty: L.Ty, +u: U.Ui, +c: D.Clip, +x: U32, +w: U32, +y: U32, +bottom: U32, +total: Nat, +search: Bool) -> L.Out:
  match span:
    case MV.Span{+first, +count}:
      L.Out.cat(L.Out{Nil{}, [L.Lay.hit(x, y, w, L.U32.dim(bottom, y), "pane", "mr|" ++ Nat.show(Nat.sub(total, Nat.div(U32.to_nat(L.U32.dim(bottom, y)), 52n))))]},
        Mr.convs(Mr.list.rows(search, u, first, count), ty, u, c, x, w, (y + U32.from_nat(Nat.mul(first, 52n)) - U32.from_nat(Nat.mul(U.Mailv.scroll(u), 52n)) : U32)))

def Mr.list(+ty: L.Ty, +u: U.Ui, +x: U32, +w: U32, +y: U32, +bottom: U32) -> L.Out:
  +search = Bool.not(String.is_empty(U.Ui.scratch(u, "@f.mrsearch")))
  +total = Bool.pick(Nat, search, List.length(&2, MV.Row, MV.Find.hits(U.Mailv.find(u))), MV.Mv.total(U.Mailv.cache(u), U.Mailv.key(u)))
  Mr.list.at(MV.Mv.visible(total, Nat.mul(U.Mailv.scroll(u), 52n), U32.to_nat(L.U32.dim(bottom, y))), ty, u, D.Clip{x, y, (x + w : U32), bottom}, x, w, y, bottom, total, search)

def Mr.opts(os: List<&2, MV.Opt>, +ty: L.Ty, +u: U.Ui, +c: D.Clip, +x0: U32, +x1: U32, +x: U32, +y: U32) -> L.Col:
  match os:
    case Nil{}: L.Col{L.Out.none(), (y + 34 : U32)}
    case MV.Opt{+addr, +label} <> t:
      +wrap = U32.is_gt((x + L.Lay.button.w(ty, label) : U32), x1)
      +xx = Bool.pick(U32, wrap, x0, x)
      +yy = Bool.pick(U32, wrap, (y + 32 : U32), y)
      L.Lay.col.cat(F.Fm.chip(ty, c, xx, yy, label, String.eq(U.Ui.scratch(u, "@mr.to"), addr), "mail-to", addr), Mr.opts(t, ty, u, c, x0, x1, (xx + L.Lay.button.w(ty, label) + 5 : U32), yy))

def Mr.bubble(b: MV.Bub, +ty: L.Ty, +u: U.Ui, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  match b:
    case MV.Bub{_, _, +who, +kind, +want, +at, +text, _}:
      +ls = L.Ty.wrap(ty, 2n, text, L.U32.dim(w, 24))
      +h = (38 + U32.from_nat(List.length(&2, String, ls)) * L.Ty.px(ty, 20) : U32)
      L.Col{L.Out.all([L.Out.ops([D.Draw.rect(c, x, y, w, h, T.Th.raise(), 255)]), Mr.label(ty, c, who ++ " · " ++ kind ++ " · " ++ want ++ " · " ++ MV.Mv.ago(U.Side.now(u), at), (x + 10 : U32), (y + 6 : U32)), L.Out.ops(F.Fm.rows(ls, ty, c, (x + 10 : U32), (y + 30 : U32), L.Ty.px(ty, 20), T.Th.text()))]), (y + h + 8 : U32)}

def Mr.bubbles(bs: List<&2, MV.Bub>, +ty: L.Ty, +u: U.Ui, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  match bs:
    case Nil{}: L.Col{L.Out.none(), y}
    case b <> t: L.Lay.col.then(Mr.bubble(b, ty, u, c, x, w, y), yy => Mr.bubbles(t, ty, u, c, x, w, yy))

def Mr.conversation.body(col: L.Col, +x: U32, +w: U32, +y: U32, +bottom: U32, +off: U32) -> L.Out:
  match col:
    case L.Col{o, h}:
      L.Out.cat(L.Out{Nil{}, [L.Lay.hit(x, y, w, L.U32.dim(bottom, y), "pane", "mrbody|" ++ Nat.show(Nat.div(Nat.sub(U32.to_nat((h + off : U32)), U32.to_nat(bottom)), 30n)))]}, o)

def Mr.conversation(+ty: L.Ty, +f: F.FS, +x: U32, +w: U32, +y: U32, +bottom: U32) -> L.Out:
  +u = F.FS.ui(f)
  +rd = MV.Mv.read_of(U.Mailv.cache(u), U.Ui.scratch(u, "@mr.conv"))
  +c = {D.Clip{x, y, (x + w : U32), bottom} : D.Clip}
  +off = U32.from_nat(Nat.mul(M.Switch.num(U.Ui.info(u, "pane:mrbody"), 0n), 30n))
  L.Out.all([F.Fm.button(ty, c, x, y, "Back", T.Th.soft(), "mail-close", ""),
    Mr.label(ty, c, L.Ty.fit(ty, 0n, MV.Read.subject(rd), L.U32.dim(w, 80)), (x + 64 : U32), y),
    F.Fm.button(ty, c, x, (y + 28 : U32), "Load earlier", T.Th.soft(), "mail-more", ""),
    F.Fm.button(ty, c, (x + 120 : U32), (y + 28 : U32), "Done", T.Th.soft(), "mail-mark", "done"),
    Mr.conversation.body(Mr.bubbles(U.Mailv.bubbles(u), ty, u, D.Clip{x, (y + 62 : U32), (x + w : U32), (bottom - 130 : U32)}, x, w, (y + 62 - off : U32)), x, w, (y + 62 : U32), (bottom - 130 : U32), off),
    L.Lay.col.out(Mr.opts(U.Mailv.targets(u), ty, u, c, x, (x + w : U32), x, (bottom - 124 : U32))),
    F.Fm.area(ty, f, c, x, (bottom - 86 : U32), L.U32.dim(w, 76), 3n, "mrreply", "mail-send", "", "Reply to a participant"),
    F.Fm.button(ty, c, (x + w - 68 : U32), (bottom - 64 : U32), "Send", T.Th.accent(), "mail-send", "")])

def Mr.deploy_note(rs: List<&2, P.PRow>) -> String:
  match rs:
    case Nil{}: ""
    case P.PRow{+kind, _, _, _, _, _, _, +state, _, _} <> t:
      Bool.pick(String, String.eq(kind, "deploy"), " · deploy " ++ state, Mr.deploy_note(t))

def Mr.has_id(rs: List<&2, P.PRow>, +id: String) -> Bool:
  match rs:
    case Nil{}: False{}
    case r <> t: Bool.or(String.eq(P.PRow.id(r), id), Mr.has_id(t, id))

def Mr.live_runs(rs: List<&2, ML.Run>, +u: U.Ui, +already: List<&2, P.PRow>) -> List<&2, P.PRow>:
  match rs:
    case Nil{}: Nil{}
    case +r <> t:
      Mr.keep(Bool.and(ML.Run.live(r), Bool.and(Bool.or(String.eq(U.Mailv.bot(u), "*"), String.eq(U.Mailv.bot(u), ML.Run.bot(r))), Bool.not(Mr.has_id(already, ML.Run.thread(r))))),
        P.PRow{"run", ML.Run.thread(r), "select", ML.Run.thread(r), "Run · " ++ ML.Run.bot(r), ML.Run.conv(r), "", "working", ML.Run.at(r), False{}}, Mr.live_runs(t, u, already))

def Mr.pins(+p: P.Plane, +ty: L.Ty, +u: U.Ui, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  +need = Mr.for_bot(P.Plane.need(p), u)
  +run = List.append(&2, P.PRow, Mr.for_bot(Mr.running(p), u), Mr.live_runs(Map.values(&2, ML.Run, ML.Box.runs(M.State.mail(U.Ui.state(u)))), u, Mr.running(p)))
  L.Lay.col.cat(Mr.label(ty, c, "Needs you · " ++ Nat.show(List.length(&2, P.PRow, need)), x, y),
    L.Lay.col.then(Mr.rows(need, ty, u, c, x, w, (y + 24 : U32)), +y2 =>
      L.Lay.col.cat(F.Fm.chip(ty, c, x, y2, Nat.show(List.length(&2, P.PRow, run)) ++ " running" ++ Mr.deploy_note(P.Plane.run(p)) ++ " · " ++ Nat.show(List.length(&2, P.PRow, P.Plane.locks(p))) ++ " locks", U.Mailv.run_open(u), "mail-run", ""),
        L.Lay.col.then(Mr.run_rows(U.Mailv.run_open(u), run, ty, u, c, x, w, (y2 + 34 : U32)), y3 =>
          Mr.rows(Mr.for_bot(Mr.chip_rows(p, U.Ui.scratch(u, "@mr.status")), u), ty, u, c, x, w, y3)))))

def Mr.columns(open: Bool, wide: Bool, +p: P.Plane, +ty: L.Ty, +f: F.FS, +x: U32, +w: U32, +y: U32, +bottom: U32) -> L.Out:
  match open wide:
    case True{} False{}: Mr.conversation(ty, f, x, w, y, bottom)
    case True{} True{}:
      +left = ((w - 24 : U32) / 2 : U32)
      L.Out.cat(L.Lay.col.out(L.Lay.col.then(Mr.pins(p, ty, F.FS.ui(f), D.Clip{x, y, (x + left : U32), bottom}, x, left, y), yy => L.Col{Mr.list(ty, F.FS.ui(f), x, left, yy, bottom), bottom})), Mr.conversation(ty, f, (x + left + 24 : U32), left, y, bottom))
    case False{} _:
      L.Lay.col.out(L.Lay.col.then(Mr.pins(p, ty, F.FS.ui(f), D.Clip{x, y, (x + w : U32), bottom}, x, w, y), yy => L.Col{Mr.list(ty, F.FS.ui(f), x, w, yy, bottom), bottom}))

def Mr.owner_chip(+u: U.Ui) -> P.PRow:
  +id = U.Mailv.owner(u)
  +b = BT.World.bot(M.State.bots(U.Ui.state(u)), id)
  P.PRow{"bot", id, "mail-filter", "bot=" ++ id, BT.Bot.name(b), "", "", P.Plane.state(U.Side.phase(U.Ui.state(u), U.Ui.thread(U.Ui.state(u), BT.Bot.thread(b)))), 0n, False{}}

def Mr.bots(on: Bool, +p: P.Plane, +ty: L.Ty, +u: U.Ui, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  match on:
    case False{}: L.Col{L.Out.none(), y}
    case True{}:
      L.Lay.col.cat(F.Fm.chip(ty, c, x, y, "All mail", String.eq(U.Mailv.bot(u), "*"), "mail-filter", "bot=*"), Mr.bot_chips(Mr.owner_chip(u) <> P.Plane.bots(p), ty, u, c, x, (x + w : U32), (x + 90 : U32), y))

def Mr.filters(+p: P.Plane, +ty: L.Ty, +u: U.Ui, +c: D.Clip, +x: U32, +w: U32, +y: U32) -> L.Col:
  Mr.chips(["unread", "act", "fyi", "snoozed", "done", "due", "today", "overview", "project="], ["Unread", "Act", "FYI", "Snoozed", "Done", "Due", "Today " ++ Nat.show(P.Plane.idle_n(p)), "Overview", "Project " ++ U.Ui.scratch(u, "@mr.project")], ty, u, c, x, (x + w : U32), x, y)

def Mr.draw(+p: P.Plane, +ty: L.Ty, +f: F.FS, +x: U32, +w: U32, +top: U32, +bottom: U32) -> L.Out:
  +u = F.FS.ui(f)
  +c = {D.Clip{x, top, (x + w : U32), bottom} : D.Clip}
  L.Lay.col.out(L.Lay.col.cat(Mr.machines(P.Plane.machines(p), ty, u, c, x, top),
    L.Lay.col.then(Mr.bots(UB.Uber.is(U.Ui.state(u), U.Mailv.owner(u)), p, ty, u, c, x, w, (top + 32 : U32)), +y1 =>
      L.Lay.col.then(Mr.filters(p, ty, u, c, x, w, y1), +y2 =>
        L.Lay.col.cat(L.Out.cat(F.Fm.field(ty, f, c, x, y2, ((w - 12 : U32) / 2 : U32), "mrsearch", "mail-search", "", "Search mail"), F.Fm.field(ty, f, c, (x + (w / 2 : U32) + 6 : U32), y2, ((w - 12 : U32) / 2 : U32), "mrproject", "mail-filter", "project=", "Project")),
          L.Col{Mr.columns(Bool.not(String.is_empty(U.Ui.scratch(u, "@mr.conv"))), U32.is_ge(w, 760), p, ty, f, x, w, (y2 + 46 : U32), bottom), bottom})))))

# Deadline calculation follows the same branches and row slice as draw.
# It runs only when rebuilding the body, never for a kept frame.
def Mr.col_y(col: L.Col) -> U32:
  match col:
    case L.Col{_, y}: y

def Mr.due_row(+r: MV.Row, +now: Nat) -> Nat:
  Bool.pick(Nat, String.is_empty(MV.Row.id(r)), 0n, MM.BotM.age_due(now, MV.Row.at(r)))

def Mr.due_rows(rs: List<&2, MV.Row>, +now: Nat) -> Nat:
  match rs:
    case Nil{}: 0n
    case +r <> t: MM.Mm.first(Mr.due_row(r, now), Mr.due_rows(t, now))

def Mr.due_bubbles(bs: List<&2, MV.Bub>, +now: Nat) -> Nat:
  match bs:
    case Nil{}: 0n
    case MV.Bub{_, _, _, _, _, at, _, _} <> t: MM.Mm.first(MM.BotM.age_due(now, at), Mr.due_bubbles(t, now))

def Mr.due_pin(r: P.PRow, +now: Nat) -> Nat:
  match r:
    case P.PRow{+kind, _, _, _, _, _, _, _, at, _}:
      Bool.pick(Nat, String.eq(kind, "lock"), 0n, MM.BotM.age_due(now, at))

def Mr.due_pins(rs: List<&2, P.PRow>, +now: Nat) -> Nat:
  match rs:
    case Nil{}: 0n
    case r <> t: MM.Mm.first(Mr.due_pin(r, now), Mr.due_pins(t, now))

def Mr.pin_rows(+p: P.Plane, +u: U.Ui) -> List<&2, P.PRow>:
  +run = List.append(&2, P.PRow, Mr.for_bot(Mr.running(p), u), Mr.live_runs(Map.values(&2, ML.Run, ML.Box.runs(M.State.mail(U.Ui.state(u)))), u, Mr.running(p)))
  List.concat(&2, P.PRow, [Mr.for_bot(P.Plane.need(p), u), List.take(&2, P.PRow, run, Bool.pick(Nat, U.Mailv.run_open(u), List.length(&2, P.PRow, run), 0n)), Mr.for_bot(Mr.chip_rows(p, U.Ui.scratch(u, "@mr.status")), u)])

def Mr.list_due_span(span: MV.Span, +u: U.Ui, search: Bool) -> Nat:
  match span:
    case MV.Span{first, count}: Mr.due_rows(Mr.list.rows(search, u, first, count), U.Side.now(u))

def Mr.list_due(+u: U.Ui, +height: U32) -> Nat:
  +search = Bool.not(String.is_empty(U.Ui.scratch(u, "@f.mrsearch")))
  +total = Bool.pick(Nat, search, List.length(&2, MV.Row, MV.Find.hits(U.Mailv.find(u))), MV.Mv.total(U.Mailv.cache(u), U.Mailv.key(u)))
  Mr.list_due_span(MV.Mv.visible(total, Nat.mul(U.Mailv.scroll(u), 52n), U32.to_nat(height)), u, search)

def Mr.columns_due(open: Bool, wide: Bool, +p: P.Plane, +u: U.Ui, +y: U32, +bottom: U32) -> Nat:
  match open wide:
    case True{} False{}: Mr.due_bubbles(U.Mailv.bubbles(u), U.Side.now(u))
    case _ _:
      +pins = Mr.pin_rows(p, u)
      +list_y = (y + 58 + U32.from_nat(Nat.mul(List.length(&2, P.PRow, pins), 30n)) : U32)
      MM.Mm.first(Mr.due_pins(pins, U.Side.now(u)), MM.Mm.first(Mr.list_due(u, L.U32.dim(bottom, list_y)), Bool.pick(Nat, open, Mr.due_bubbles(U.Mailv.bubbles(u), U.Side.now(u)), 0n)))

def Mr.due(+p: P.Plane, +ty: L.Ty, +u: U.Ui, +x: U32, +w: U32, +top: U32, +bottom: U32) -> Nat:
  +c = {D.Clip{x, top, (x + w : U32), bottom} : D.Clip}
  +y1 = Mr.col_y(Mr.bots(UB.Uber.is(U.Ui.state(u), U.Mailv.owner(u)), p, ty, u, c, x, w, (top + 32 : U32)))
  +y2 = Mr.col_y(Mr.filters(p, ty, u, c, x, w, y1))
  Mr.columns_due(Bool.not(String.is_empty(U.Ui.scratch(u, "@mr.conv"))), U32.is_ge(w, 760), p, u, (y2 + 46 : U32), bottom)

# Stable data identity, computed once per existing plane rebuild. Clock
# bucket changes whose plane data is unchanged leave this signature alone.
def Mr.stamp_row(r: P.PRow) -> J.Json:
  match r:
    case P.PRow{k, i, a, v, t, s, m, st, at, away}:
      J.Json.arr([J.Str{k}, J.Str{i}, J.Str{a}, J.Str{v}, J.Str{t}, J.Str{s}, J.Str{m}, J.Str{st}, J.Json.nat(at), J.Flag{away}])

def Mr.stamp_rows(rs: List<&2, P.PRow>) -> List<&2, J.Json>:
  match rs:
    case Nil{}: Nil{}
    case r <> t: Mr.stamp_row(r) <> Mr.stamp_rows(t)

def Mr.stamp_machines(ms: List<&2, P.PMach>) -> List<&2, J.Json>:
  match ms:
    case Nil{}: Nil{}
    case P.PMach{n, s} <> t: J.Json.arr([J.Str{n}, J.Str{s}]) <> Mr.stamp_machines(t)

def Mr.plane_stamp(p: P.Plane) -> String:
  match p:
    case P.Plane{need, run, bots, rooms, routines, idle, idle_n, machines, locks}:
      J.Json.show(J.Json.arr([J.Json.arr(Mr.stamp_rows(need)), J.Json.arr(Mr.stamp_rows(run)), J.Json.arr(Mr.stamp_rows(bots)), J.Json.arr(Mr.stamp_rows(rooms)), J.Json.arr(Mr.stamp_rows(routines)), J.Json.arr(Mr.stamp_rows(idle)), J.Json.nat(idle_n), J.Json.arr(Mr.stamp_machines(machines)), J.Json.arr(Mr.stamp_rows(locks))]))
