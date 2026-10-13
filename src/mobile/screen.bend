import Base
import ../core/json.bend as J
import ../core/model.bend as M
import ../core/client.bend as U
import ../web/dom.bend as D
import ../web/view.bend as V
import ./notify.bend as N
import ./view.bend as VW
import ./picker.bend as PK
import ../core/todo.bend as TD
import ../core/attach.bend as AT
import ../core/images.bend as IM
import ../core/links.bend as LK
import ../core/fold.bend as FO
import ../core/subs.bend as SB
import ../core/diff.bend as DF
import ../core/vt.bend as VT
import ../core/codec.bend as C
import ../core/onboard.bend as OB
import ../core/botui.bend as B
import ./bots.bend as MB
import ../core/hist.bend as HS
import ../core/review.bend as RVW
import ../core/tools.bend as TO
import ../core/emb.bend as EM

# Ui -> the screen the phone apps draw, as JSON. SwiftUI and Compose map
# it onto native lists, toolbars and text fields; every decision (order,
# shelf, status, labels, markdown blocks) is made here or in core.

def Mob.st(u: U.Ui) -> M.State:
  match u:
    case U.Ui{st, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      st

def Mob.state(t: M.TurnState) -> String:
  match t:
    case M.Running{}:
      "run"
    case M.Failed{}:
      "fail"
    case M.Interrupted{}:
      "stop"
    case _:
      "idle"

# a swipe button: what it says, the action it sends with its value, its
# tone ("danger", "settle", ""), and choices (a snooze) whose values it
# sends instead
def Mob.swipe(label: String, action: String, value: String, tone: String, opts: List<&2, J.Json>) -> J.Json:
  J.Json.obj([J.KV{"label", J.Json.str(label)}, J.KV{"action", J.Json.str(action)}, J.KV{"value", J.Json.str(value)},
    J.KV{"tone", J.Json.str(tone)}, J.KV{"options", J.Json.arr(opts)}])

def Mob.opt(label: String, value: String) -> J.Json:
  J.Json.obj([J.KV{"label", J.Json.str(label)}, J.KV{"value", J.Json.str(value)}])

def Mob.snoozes(+id: String) -> List<&2, J.Json>:
  [Mob.opt("1 hour", id ++ "|1h"), Mob.opt("Tomorrow", id ++ "|tomorrow"), Mob.opt("1 week", id ++ "|1w")]

# the swipe from the leading edge: settle an open thread, bring back a
# settled one, wake a snoozed one (shelf 0 open, 1 settled, 2 snoozed,
# 3 archived: none)
def Mob.lead(shelf: Nat, +id: String) -> List<&2, J.Json>:
  match shelf:
    case 0n:
      [Mob.swipe("Settle", "row-settle", id, "settle", Nil{})]
    case 1n:
      [Mob.swipe("Unsettle", "row-unsettle", id, "settle", Nil{})]
    case 2n:
      [Mob.swipe("Wake", "row-snooze", id ++ "|off", "settle", Nil{})]
    case _:
      Nil{}

# the swipe from the trailing edge: delete (asks first), snooze
def Mob.trail(+id: String) -> List<&2, J.Json>:
  [Mob.swipe("Delete", "row-delete", id, "danger", Nil{}), Mob.swipe("Snooze", "row-snooze", "", "", Mob.snoozes(id))]

# a row's time: since its last activity, or (snoozed) until it wakes
def Mob.when(shelf: Nat, +st: M.State, +id: String, +a: Nat, +now: Nat) -> String:
  match shelf:
    case 2n:
      U.Time.ago(U.Side.until(st, id), now)
    case _:
      U.Time.ago(now, a)

def Mob.row(t: M.Thread, +status: String, +ix: U.SIx, +st: M.State, +now: Nat, +shelf: Nat) -> J.Json:
  match t:
    case M.Thread{+id, _, title, _, +a, turn, _, _, _, _, _, pinned, _, _, _, _, _, _, _, _, _}:
      J.Json.obj([J.KV{"id", J.Json.str(id)}, J.KV{"title", J.Json.str(U.Label.title(st, id, title))}, J.KV{"state", J.Json.str(Mob.state(turn))},
        J.KV{"ago", J.Json.str(Mob.when(shelf, st, id, a, now))}, J.KV{"agents", J.Json.str(SB.Subs.badge(SB.Subs.count.ix(ix, st, id)))}, J.KV{"pinned", J.Json.bool(pinned)},
        J.KV{"status", J.Json.str(status)}, J.KV{"lead", J.Json.arr(Mob.lead(shelf, id))}, J.KV{"trail", J.Json.arr(Mob.trail(id))}])

def Mob.rows(ts: List<&2, M.Thread>, +ix: U.SIx, +st: M.State, +now: Nat, +shelf: Nat) -> List<&2, J.Json>:
  match ts:
    case Nil{}:
      Nil{}
    case +t <> rest:
      Mob.row(t, M.Phase.name(U.Side.phase.ix(ix, st, t)), ix, st, now, shelf) <> Mob.rows(rest, ix, st, now, shelf)

# snoozed threads sit on their own shelf, soonest to wake first
def Mob.project(r: U.SRow, +open: Map<&2, Bool>, +ix: U.SIx, +st: M.State, +now: Nat) -> J.Json:
  match r:
    case U.SRow{_, M.Project{+id, title, root, _, _}, M.Shelves{pinned, active, +settled, hidden, snoozed}}:
      J.Json.obj([J.KV{"id", J.Json.str(id)}, J.KV{"title", J.Json.str(U.Label.title(st, id, title))}, J.KV{"root", J.Json.str(root)},
        J.KV{"open", J.Json.bool(U.Flag.get(open, id))},
        J.KV{"threads", J.Json.arr(Mob.rows(List.append(&2, M.Thread, pinned, active), ix, st, now, 0n))},
        J.KV{"snoozedShelf", J.Json.str("Snoozed")},
        J.KV{"snoozed", J.Json.arr(Mob.rows(snoozed, ix, st, now, 2n))},
        J.KV{"shelf", J.Json.str("Settled " ++ Nat.show(List.length(&2, M.Thread, settled)))},
        J.KV{"settled", J.Json.arr(Mob.rows(settled, ix, st, now, 1n))},
        J.KV{"value", J.Json.str("arch:" ++ id)}, J.KV{"archOpen", J.Json.bool(U.Flag.get(open, "arch:" ++ id))},
        J.KV{"archived", J.Json.arr(Mob.rows(M.Archive.of(hidden), ix, st, now, 3n))}])

def Mob.quiet(p: J.Json, q: Bool) -> J.Json:
  match p:
    case J.Obj{fs}:
      J.Obj{J.Field{"quiet", J.Json.bool(q), fs}}
    case x:
      x

# a project elsewhere still catching up shows faded, as a quiet one does
def Mob.syncing(r: U.SRow, +st: M.State) -> Bool:
  match r:
    case U.SRow{_, M.Project{+id, _, _, _, _}, _}:
      U.Label.syncing(st, id)

def Mob.projects(rs: List<&2, U.SRow>, +open: Map<&2, Bool>, +ix: U.SIx, +st: M.State, +now: Nat, +q: Bool) -> List<&2, J.Json>:
  match rs:
    case Nil{}:
      Nil{}
    case +r <> t:
      Mob.quiet(Mob.project(r, open, ix, st, now), Bool.or(q, Mob.syncing(r, st))) <> Mob.projects(t, open, ix, st, now, q)

# the projects as the sidebar groups them (core's Side.groups): live ones,
# then the quiet ones ("quiet": shown faded), then the older ones, only
# while their shelf is open or a search is on
def Mob.older.k(show: Bool, rs: List<&2, U.SRow>, +open: Map<&2, Bool>, +ix: U.SIx, +st: M.State, +now: Nat) -> List<&2, J.Json>:
  match show:
    case True{}:
      Mob.projects(rs, open, ix, st, now, True{})
    case False{}:
      Nil{}

def Mob.grouped(g: U.SGroups, +open: Map<&2, Bool>, +ix: U.SIx, +st: M.State, +now: Nat, +q: String) -> List<&2, J.Json>:
  match g:
    case U.SGroups{l, qs, o}:
      List.concat(&2, J.Json, [Mob.projects(l, open, ix, st, now, False{}), Mob.projects(qs, open, ix, st, now, True{}),
        Mob.older.k(Bool.or(U.Flag.get(open, "old-projects"), Bool.not(String.is_empty(q))), o, open, ix, st, now)])

# the folded shelf of older projects: how many and whether it is open
# ("side-older" flips it); null while there are none or a search shows them
def Mob.older.j(+n: Nat, +open: Bool, +q: String) -> J.Json:
  Bool.pick(J.Json, Bool.or(Nat.is_eq(n, 0n), Bool.not(String.is_empty(q))), J.Null{},
    J.Json.obj([J.KV{"count", J.Json.nat(n)}, J.KV{"open", J.Json.bool(open)}]))

def Mob.older(g: U.SGroups, +open: Map<&2, Bool>, +q: String) -> J.Json:
  match g:
    case U.SGroups{_, _, o}:
      Mob.older.j(List.length(&2, U.SRow, o), U.Flag.get(open, "old-projects"), q)

# the active view's rows (core's Active.rows): a thread row with its
# project's name, "faded" for one settled lately, and "urg"/"at", which
# order rows merged from several hubs the same way
def Mob.arow.k(j: J.Json, +urg: Nat, +at: Nat, +pt: String, +faded: Bool) -> J.Json:
  match j:
    case J.Obj{fs}:
      J.Obj{J.Field{"project", J.Json.str(pt), J.Field{"faded", J.Json.bool(faded), J.Field{"urg", J.Json.nat(urg), J.Field{"at", J.Json.nat(at), fs}}}}}
    case x:
      x

def Mob.arow(r: U.ARow, +ix: U.SIx, +st: M.State, +now: Nat) -> J.Json:
  match r:
    case U.ARow{+urg, +at, +t, _, +pt, +faded}:
      Mob.arow.k(Mob.row(t, M.Phase.name(U.Side.phase.ix(ix, st, t)), ix, st, now, Bool.pick(Nat, faded, 1n, 0n)), urg, at, pt, faded)

def Mob.arows(rs: List<&2, U.ARow>, +ix: U.SIx, +st: M.State, +now: Nat) -> List<&2, J.Json>:
  match rs:
    case Nil{}:
      Nil{}
    case r <> t:
      Mob.arow(r, ix, st, now) <> Mob.arows(t, ix, st, now)

# the settled rows only while their fold is open or a search is on
def Mob.active(on: Bool, +st: M.State, +now: Nat, +q: String, +o: Map<&2, Bool>) -> List<&2, J.Json>:
  match on:
    case True{}:
      Mob.arows(U.Active.folded(U.Active.open(o, q), U.Active.parts(U.Active.rows(st, now, q))), U.Side.ix(st), st, now)
    case False{}:
      Nil{}

# the active view's fold of settled rows: how many and whether it is open
# ("side-settled" flips it); null while there are none or a search shows them
def Mob.settled(+st: M.State, +now: Nat, +q: String, +o: Map<&2, Bool>) -> J.Json:
  Mob.older.j(U.Active.faded_n(U.Active.parts(U.Active.rows(st, now, q))), U.Flag.get(o, "active-settled"), q)

# (only worked out while the active view is on)
def Mob.settled.if(on: Bool, +st: M.State, +now: Nat, +q: String, +o: Map<&2, Bool>) -> J.Json:
  match on:
    case True{}:
      Mob.settled(st, now, q, o)
    case False{}:
      J.Null{}

# the active view's rows and its fold, from the rows split once (a phone
# screen built them twice); nothing is worked out while it is off
type MSide is Data:
  MSide{rows: List<&2, J.Json>, settled: J.Json}

def Mob.side.k(+p: U.AParts, +ix: U.SIx, +st: M.State, +now: Nat, +q: String, +o: Map<&2, Bool>) -> MSide:
  MSide{Mob.arows(U.Active.folded(U.Active.open(o, q), p), ix, st, now), Mob.older.j(U.Active.faded_n(p), U.Flag.get(o, "active-settled"), q)}

def Mob.side(on: Bool, +ix: U.SIx, +st: M.State, +now: Nat, +q: String, +o: Map<&2, Bool>) -> MSide:
  match on:
    case True{}:
      Mob.side.k(U.Active.parts(U.Active.rows.ix(st, ix, now, q)), ix, st, now, q, o)
    case False{}:
      MSide{Nil{}, J.Null{}}

def MSide.rows(m: MSide) -> List<&2, J.Json>:
  match m:
    case MSide{r, _}:
      r

def MSide.settled(m: MSide) -> J.Json:
  match m:
    case MSide{_, s}:
      s

# a row's or project's menu (core's Menu): its title and items, each
# sending its action with its value; null while none is open, or while a
# delete or remove it armed asks through "deleting"/"removing"
def Mob.mitems(xs: List<&2, U.MItem>) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case U.MItem{l, a, v, t} <> r:
      J.Json.obj([J.KV{"label", J.Json.str(l)}, J.KV{"action", J.Json.str(a)}, J.KV{"value", J.Json.str(v)}, J.KV{"tone", J.Json.str(t)}]) <> Mob.mitems(r)

def Mob.rmenu.k(none: Bool, +u: U.Ui) -> J.Json:
  match none:
    case True{}:
      J.Null{}
    case False{}:
      J.Json.obj([J.KV{"title", J.Json.str(U.Menu.title(u))}, J.KV{"kind", J.Json.str(U.Menu.kind(U.Menu.open(u)))}, J.KV{"items", J.Json.arr(Mob.mitems(U.Menu.items(u, False{})))}])

def Mob.rmenu(+u: U.Ui) -> J.Json:
  Mob.rmenu.k(Bool.or(String.is_empty(U.Menu.open(u)), Bool.not(String.is_empty(U.Ui.scratch(u, "@del") ++ U.Ui.scratch(u, "@rm")))), u)

# markdown blocks from the same Md.render the web page uses; built onto a
# reversed accumulator so a long reply's many blocks cost no stack (only
# nesting does)
# a link's target, the attribute a phone needs, and whether it is a
# file's (a tap opens the file's menu: "file-menu" with href)
# an ordered item's number as written ("n")
def Mob.num(n: Bool, +v: String, rest: List<&2, J.KV>) -> List<&2, J.KV>:
  match n:
    case True{}:
      J.KV{"n", J.Json.str(v)} <> rest
    case False{}:
      rest

def Mob.href(as: List<&2, D.Attr>) -> List<&2, J.KV>:
  match as:
    case Nil{}:
      Nil{}
    case D.At{+k, +v} <> t:
      Bool.pick(List<&2, J.KV>, String.eq(k, "href"), [J.KV{"href", J.Json.str(v)}, J.KV{"file", J.Json.bool(Bool.not(LK.Link.is_web(v)))}], Mob.num(String.eq(k, "value"), v, Mob.href(t)))
    case _ <> t:
      Mob.href(t)

def Mob.nodes.go(ns: List<&2, D.Node>, acc: List<&2, J.Json>) -> List<&2, J.Json>:
  match ns:
    case Nil{}:
      acc
    case D.El{tag, _, as, kids} <> t:
      Mob.nodes.go(t, J.Json.obj(J.KV{"tag", J.Json.str(tag)} <> J.KV{"kids", J.Json.arr(List.reverse(&2, J.Json, Mob.nodes.go(kids, Nil{})))} <> Mob.href(as)) <> acc)
    case D.Txt{text} <> t:
      Mob.nodes.go(t, J.Json.obj([J.KV{"text", J.Json.str(text)}]) <> acc)

def Mob.nodes(ns: List<&2, D.Node>) -> List<&2, J.Json>:
  List.reverse(&2, J.Json, Mob.nodes.go(ns, Nil{}))

# the Review tab's report as nodes, only while that tab is open
def Mob.review(on: Bool, +u: U.Ui) -> J.Json:
  match on:
    case True{}:
      J.Json.arr(Mob.nodes(D.Md.render(U.RpView.md(U.Rp.view(u)))))
    case False{}:
      J.Null{}

# a chip for an attachment line: its name, path, whether it is an image,
# and where the hub serves it
def Mob.chip(+p: String) -> J.Json:
  J.Json.obj([J.KV{"label", J.Json.str(AT.Att.label(p))}, J.KV{"path", J.Json.str(p)}, J.KV{"image", J.Json.bool(AT.Att.image(p))},
    J.KV{"url", J.Json.str(IM.Img.url(p))}])

def Mob.chips(ps: List<&2, String>) -> List<&2, J.Json>:
  match ps:
    case Nil{}:
      Nil{}
    case +p <> t:
      Mob.chip(p) <> Mob.chips(t)

def Mob.urls(ps: List<&2, String>) -> List<&2, J.Json>:
  match ps:
    case Nil{}:
      Nil{}
    case +p <> t:
      J.Json.obj([J.KV{"path", J.Json.str(p)}, J.KV{"url", J.Json.str(IM.Img.url(p))}]) <> Mob.urls(t)

# The images a reply names, found once when it arrives and kept in the
# scratch ("@img:<id>": "=" then the paths, a line each; the scratch, unlike
# the info map, outlives a reconnect's new log): finding them
# walks the text char by char, which a phone's QuickJS did for every reply
# on every screen, most of a thread screen's cost
def Mob.imgs.key(+id: String) -> String:
  "@img:" ++ id

def Mob.imgs.found(may: Bool, +text: String) -> List<&2, String>:
  match may:
    case True{}:
      IM.Img.paths(text)
    case False{}:
      Nil{}

# how many of the newest replies have their images found
def Mob.imgs.max() -> Nat:
  30n

def Mob.imgs.put(+id: String, +text: String, u: U.Ui) -> U.Ui:
  U.Ui.scratch_set(u, Mob.imgs.key(id), "=" ++ String.join(Mob.imgs.found(IM.Img.may(text), text), "\n"))

# the walk over entries, newest first: how many replies may still be
# looked at, and whether it is done (a reply looked at before: every older
# one was too, or is past the newest Mob.imgs.max())
type ImgW is Data:
  ImgW{k: Nat, done: Bool, u: U.Ui}

def Mob.imgs.reply(fresh: Bool, +k: Nat, +id: String, +text: String, u: U.Ui) -> ImgW:
  match fresh k:
    case True{} 1n+j:
      ImgW{j, False{}, Mob.imgs.put(id, text, u)}
    case _ _:
      ImgW{k, True{}, u}

def Mob.imgs.step(e: M.Entry, w: ImgW) -> ImgW:
  match e w:
    case M.Msg{+id, _, M.Assistant{}, +text, _} ImgW{+k, False{}, +u}:
      Mob.imgs.reply(String.is_empty(U.Ui.scratch(u, Mob.imgs.key(id))), k, id, text, u)
    case _ _:
      w

# (the walk stops once done: every frame asks, and a log holds thousands
# of entries)
def Mob.imgs.seen.go(es: List<&2, M.Entry>, w: ImgW) -> ImgW:
  match es w:
    case e <> t ImgW{+k, False{}, +u}:
      Mob.imgs.seen.go(t, Mob.imgs.step(e, ImgW{k, False{}, u}))
    case _ _:
      w

def Mob.imgs.ui(w: ImgW) -> U.Ui:
  match w:
    case ImgW{_, _, u}:
      u

# the newest replies the client holds have their images found, each once
# (entries come newest first)
def Mob.imgs.seen(+u: U.Ui) -> U.Ui:
  Mob.imgs.ui(Mob.imgs.seen.go(U.Ui.entries(U.Ui.state.of(u)), ImgW{Mob.imgs.max(), False{}, u}))

def Mob.imgs.list(+s: String) -> List<&2, String>:
  Bool.pick(List<&2, String>, String.is_empty(s), Nil{}, String.split(s, '\n'))

def Mob.imgs(+dr: Map<&2, String>, +id: String) -> List<&2, J.Json>:
  Mob.urls(Mob.imgs.list(String.drop(U.Str.get(dr, Mob.imgs.key(id)), 1n)))

# a terminal's row or any other activity's: matched, not Bool.pick (which
# built both, so every tool call's summary was cut as a terminal's title)
def Mob.act.k(term: Bool, +id: String, +th: String, +tone: String, +kind: String, +summary: String) -> J.Json:
  match term:
    case True{}:
      J.Json.obj([J.KV{"id", J.Json.str(id)}, J.KV{"kind", J.Json.str("emb")}, J.KV{"key", J.Json.str(EM.Emb.key(th, id))}, J.KV{"text", J.Json.str(EM.Emb.title(summary))}])
    case False{}:
      J.Json.obj([J.KV{"id", J.Json.str(id)}, J.KV{"kind", J.Json.str("act")}, J.KV{"tone", J.Json.str(tone)},
        J.KV{"label", J.Json.str(RVW.Review.label(tone, kind, summary))}, J.KV{"text", J.Json.str(RVW.Review.say(tone, kind, summary))}])

# a tool call or other activity; a design step (core/hist.bend) is a row
# that shows it in the viewer ("hist-show" with its value)
def Mob.act(step: Bool, e: M.Entry) -> J.Json:
  match step e:
    case True{} M.Act{+id, _, _, _, _, _}:
      J.Json.obj([J.KV{"id", J.Json.str(id)}, J.KV{"kind", J.Json.str("step")}, J.KV{"text", J.Json.str(HS.Hist.row(e))}, J.KV{"value", J.Json.str(id)}])
    case _ M.Act{+id, +th, +tone, +kind, +summary, _}:
      Mob.act.k(String.eq(tone, EM.Emb.tone()), id, th, tone, kind, summary)
    case _ M.Msg{+id, _, _, text, _}:
      J.Json.obj([J.KV{"id", J.Json.str(id)}, J.KV{"kind", J.Json.str("act")}, J.KV{"tone", J.Json.str("")}, J.KV{"label", J.Json.str("")}, J.KV{"text", J.Json.str(text)}])

# an entry of the thread itself: a user message is what was written and
# its attachments; a reply its markdown and the images it names
def Mob.own(e: M.Entry, +dr: Map<&2, String>) -> J.Json:
  match e:
    case M.Msg{id, _, M.User{}, +text, _}:
      J.Json.obj([J.KV{"id", J.Json.str(id)}, J.KV{"kind", J.Json.str("user")}, J.KV{"text", J.Json.str(AT.Att.body(text))},
        J.KV{"attachments", J.Json.arr(Mob.chips(AT.Att.paths(text)))}])
    case M.Msg{+id, _, _, +text, _}:
      J.Json.obj([J.KV{"id", J.Json.str(id)}, J.KV{"kind", J.Json.str("assistant")}, J.KV{"text", J.Json.str(text)},
        J.KV{"blocks", J.Json.arr(Mob.nodes(D.Md.render(text)))}, J.KV{"images", J.Json.arr(Mob.imgs(dr, id))}])
    case M.Act{+id, +th, +tone, +kind, +summary, +at}:
      Mob.act(String.eq(tone, HS.Step.tone()), M.Act{id, th, tone, kind, summary, at})

# a row that opens another thread (value: its id)
def Mob.link(+id: String, +text: String, +to: String) -> J.Json:
  J.Json.obj([J.KV{"id", J.Json.str(id)}, J.KV{"kind", J.Json.str("link")}, J.KV{"text", J.Json.str(text)}, J.KV{"value", J.Json.str(to)}])

def Mob.from(none: Bool, +id: String, +from: String, +st: M.State) -> List<&2, J.Json>:
  match none:
    case True{}:
      Nil{}
    case False{}:
      [Mob.link(id ++ ":from", "← from " ++ U.Tasks.title(st, from), from)]

# what an entry shows as, as the web's View.entry.k: this thread's own
# (headed by the thread that sent it), or a message it sent to another
def Mob.entry.k(mine: Bool, e: M.Entry, +st: M.State, +dr: Map<&2, String>) -> List<&2, J.Json>:
  match mine e:
    case True{} M.Msg{+id, t, r, x, at}:
      List.append(&2, J.Json, Mob.from(String.is_empty(M.Link.from(id)), id, M.Link.from(id), st), [Mob.own(M.Msg{id, t, r, x, at}, dr)])
    case True{} _:
      [Mob.own(e, dr)]
    case False{} M.Msg{+id, +t, _, x, _}:
      [Mob.link(id, "→ " ++ U.Tasks.title(st, t) ++ ": " ++ DF.Diff.first(x), t)]
    case False{} _:
      Nil{}

def Mob.entry(+e: M.Entry, +st: M.State, +sel: String, +dr: Map<&2, String>) -> List<&2, J.Json>:
  Mob.entry.k(String.eq(V.View.entry_thread(e), sel), e, st, dr)

# a run of tool calls as one row; "fold" with its value opens or shuts it
def Mob.fold(+id: String, +label: String, +open: Bool) -> J.Json:
  J.Json.obj([J.KV{"id", J.Json.str("fold:" ++ id)}, J.KV{"kind", J.Json.str("fold")}, J.KV{"text", J.Json.str(label)},
    J.KV{"open", J.Json.bool(open)}, J.KV{"value", J.Json.str(FO.Fold.ask(open, id))}])

# xs pushed onto a reversed accumulator
def Mob.onto(xs: List<&2, J.Json>, acc: List<&2, J.Json>) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      acc
    case x <> t:
      Mob.onto(t, x <> acc)

def Mob.shown(open: Bool, xs: List<&2, J.Json>, acc: List<&2, J.Json>) -> List<&2, J.Json>:
  match open:
    case True{}:
      Mob.onto(xs, acc)
    case False{}:
      acc

# an entry's rows as its fold role says
def Mob.role(r: FO.Role, xs: List<&2, J.Json>, acc: List<&2, J.Json>) -> List<&2, J.Json>:
  match r:
    case FO.RShow{}:
      Mob.onto(xs, acc)
    case FO.RHead{+id, +label, +open}:
      Mob.shown(open, xs, Mob.fold(id, label, open) <> acc)
    case FO.RIn{open}:
      Mob.shown(open, xs, acc)
    case FO.RWork{+id, +label, open, inner}:
      match open:
        case True{}:
          Mob.role(inner, xs, Mob.fold(id, label, True{}) <> acc)
        case False{}:
          Mob.fold(id, label, False{}) <> acc

# a thread's entries, tail-recursive (a thread can hold thousands)
def Mob.entries.go(es: List<&2, M.Entry>, rs: List<&2, FO.Role>, +st: M.State, +sel: String, +dr: Map<&2, String>, acc: List<&2, J.Json>) -> List<&2, J.Json>:
  match es rs:
    case Nil{} _:
      acc
    case e <> t r <> rt:
      Mob.entries.go(t, rt, st, sel, dr, Mob.role(r, Mob.entry(e, st, sel, dr), acc))
    case e <> t Nil{}:
      Mob.entries.go(t, Nil{}, st, sel, dr, Mob.onto(Mob.entry(e, st, sel, dr), acc))

# oldest first, runs of tool calls folded (the last stays open while its
# turn runs); a reply's images are the ones found when it came
# (Mob.imgs.seen)
def Mob.entries(+es: List<&2, M.Entry>, +st: M.State, +sel: String, +info: Map<&2, String>, +dr: Map<&2, String>, +running: Bool) -> List<&2, J.Json>:
  List.reverse(&2, J.Json, Mob.entries.go(es, FO.Fold.roles(es, info, running), st, sel, dr, Nil{}))

# a toolbar button (core/tools.bend): its icon, what it says, what it does
# (tip) and the action it sends
def Mob.tool(t: TO.Tool) -> J.Json:
  match t:
    case TO.Tool{i, l, tip, a, v, on, dg, _}:
      J.Json.obj([J.KV{"icon", J.Json.str(i)}, J.KV{"label", J.Json.str(l)}, J.KV{"tip", J.Json.str(tip)}, J.KV{"action", J.Json.str(a)},
        J.KV{"value", J.Json.str(v)}, J.KV{"on", J.Json.bool(on)}, J.KV{"danger", J.Json.bool(dg)}])

def Mob.tools(ts: List<&2, TO.Tool>) -> List<&2, J.Json>:
  match ts:
    case Nil{}:
      Nil{}
    case t <> r:
      Mob.tool(t) <> Mob.tools(r)

# the toolbar's: stop, then under the menu where it sits and forking
def Mob.head(running: Bool, pinned: Bool, settled: Bool, archived: Bool, forked: Bool) -> List<&2, J.Json>:
  Mob.tools(List.concat(&2, TO.Tool, [TO.Tools.stop(running), [TO.Tools.pin(pinned), TO.Tools.settle(settled), TO.Tools.archive(archived), TO.Tools.fork()], TO.Tools.back(forked)]))

def Mob.live(empty: Bool, running: Bool) -> String:
  match empty running:
    case True{} True{}:
      "Working"
    case _ _:
      ""

def Mob.strs(xs: List<&2, String>) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      J.Json.str(x) <> Mob.strs(t)

# a choice of a menu: what it says, the value it sends, whether it is on
def Mob.choice(label: String, value: String, on: Bool) -> J.Json:
  J.Json.obj([J.KV{"label", J.Json.str(label)}, J.KV{"value", J.Json.str(value)}, J.KV{"on", J.Json.bool(on)}])

# a model's name in the menu: the CLI's label for it ("Opus 5.5 ·
# claude-opus-5-5"), else its name. The picker reads the models info
# parsed once (parsing its text for each model, twice, was most of what
# a thread's screen cost on a phone's JS engine)
def Mob.label.k(+l: String, +m: String) -> String:
  Bool.pick(String, String.is_empty(l), Bool.pick(String, String.is_empty(m), "default", m), l)

def Mob.model_name(+about: J.Json, +m: String) -> String:
  Mob.label.k(J.Json.get_str(J.Json.get_obj(about, "labels"), Bool.pick(String, String.is_empty(m), "default", m), ""), m)

def Mob.models(ms: List<&2, String>, +about: J.Json, +cur: String) -> List<&2, J.Json>:
  match ms:
    case Nil{}:
      Nil{}
    case +m <> t:
      Mob.choice(Mob.model_name(about, m), m, String.eq(m, cur)) <> Mob.models(t, about, cur)

def Mob.efforts(es: List<&2, String>, +cur: String, +prov: String) -> List<&2, J.Json>:
  match es:
    case Nil{}:
      Nil{}
    case +e <> t:
      Mob.choice(U.Chip.name(e), e, U.Chip.on(e, cur, prov)) <> Mob.efforts(t, cur, prov)

# the composer's model chip and its menu: the thread's provider's models
# (its default first; "model" sends one) and the efforts that model takes
# ("effort")
# (j: the models info, parsed; about: its part for the provider)
def Mob.picker.j(+j: J.Json, +about: J.Json, +provider: String, +model: String, +effort: String, +ctx: String) -> J.Json:
  J.Json.obj([J.KV{"label", J.Json.str(U.Effort.label(U.Models.label(provider, model), effort) ++ ctx)},
    J.KV{"models", J.Json.arr(Mob.models("" <> U.Models.strs(J.Json.elems(J.Json.get_obj(j, provider))), about, model))},
    J.KV{"efforts", J.Json.arr(Mob.efforts(U.Effort.chips.of(U.Effort.known(J.Json.get(J.Json.get_obj(about, "efforts"), Bool.pick(String, String.is_empty(model), "default", model)), provider)), effort, provider))},
    J.KV{"providers", J.Json.arr(Mob.efforts(U.Chip.providers(), effort, provider))}])

def Mob.picker.p(+j: J.Json, +provider: String, +model: String, +effort: String, +ctx: String) -> J.Json:
  Mob.picker.j(j, J.Json.get_obj(J.Json.get_obj(j, "about"), provider), provider, model, effort, ctx)

def Mob.picker.ctx(+info: String, +provider: String, +model: String, +effort: String, +ctx: String) -> J.Json:
  Mob.picker.p(Maybe.default(&2, J.Json, J.Json.parse(info), J.Null{}), provider, model, effort, ctx)

def Mob.picker(+info: String, +provider: String, +model: String, +effort: String) -> J.Json:
  Mob.picker.ctx(info, provider, model, effort, "")

def Mob.qbtns(bs: List<&2, U.QBtn>) -> List<&2, J.Json>:
  match bs:
    case Nil{}:
      Nil{}
    case U.QBtn{l, a, v} <> t:
      J.Json.obj([J.KV{"label", J.Json.str(l)}, J.KV{"action", J.Json.str(a)}, J.KV{"value", J.Json.str(v)}]) <> Mob.qbtns(t)

# the queue above the composer: each waiting message with its buttons
# (U.Qu.buttons)
def Mob.queue(qs: List<&2, M.QItem>, +first: Bool, +run: Bool) -> List<&2, J.Json>:
  match qs:
    case Nil{}:
      Nil{}
    case M.QItem{+msg, +text} <> t:
      J.Json.obj([J.KV{"msg", J.Json.str(msg)}, J.KV{"text", J.Json.str(U.Qu.label(msg, text))}, J.KV{"tag", J.Json.str(U.Qu.tag(run, msg))},
        J.KV{"buttons", J.Json.arr(Mob.qbtns(U.Qu.row_buttons(run, first, msg)))}]) <> Mob.queue(t, False{}, run)

def Mob.tdlines(rs: List<&2, TD.TRow>) -> List<&2, J.Json>:
  match rs:
    case Nil{}:
      Nil{}
    case +r <> rest:
      J.Json.obj([J.KV{"text", J.Json.str(TD.TRow.text(r))}, J.KV{"status", J.Json.str(TD.TRow.status(r))}]) <> Mob.tdlines(rest)

# the agent's todo list (U.Todos.shown), or null while hidden: its head,
# whether it is open and what a tap on the head sends (fold, with toggle),
# its lines (every step once shown all: the app scrolls them), and a long
# list's last line ("Show all 24", "Show fewer"; "" when it fits) and its value
def Mob.todos.k(+open: Bool, +all: Bool, +th: String, +ts: List<&2, TD.Todo>, +info: Map<&2, String>) -> J.Json:
  J.Json.obj([J.KV{"head", J.Json.str(TD.Todo.head(ts))}, J.KV{"open", J.Json.bool(open)}, J.KV{"toggle", J.Json.str(U.Todos.toggle(open, th))},
    J.KV{"lines", J.Json.arr(Mob.tdlines(U.Todos.rows(info, th, U.Todos.cap.all(), ts)))},
    J.KV{"more", J.Json.str(Bool.pick(String, open, U.Todos.more(all, ts), ""))}, J.KV{"moreValue", J.Json.str(U.Todos.more.ask(all, th))}])

def Mob.todos(+info: Map<&2, String>, +th: String, +ts: List<&2, TD.Todo>) -> J.Json:
  Bool.pick(J.Json, List.is_empty(&2, TD.Todo, ts), J.Null{}, Mob.todos.k(U.Todos.open(info, th), U.Todos.all(info, th), th, ts, info))

# Asks: what the agent waits on the user for. Each button sends "answer"
# with its value; "primary" marks the one to lead with
def Mob.btn(+label: String, +value: String, primary: Bool) -> J.Json:
  J.Json.obj([J.KV{"label", J.Json.str(label)}, J.KV{"value", J.Json.str(value)}, J.KV{"primary", J.Json.bool(primary)}])

def Mob.ask.opts(ls: List<&2, String>, +id: String) -> List<&2, J.Json>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      Mob.btn(l, id ++ "|" ++ l, False{}) <> Mob.ask.opts(t, id)

def Mob.ask.draft(empty: Bool, +id: String, +draft: String) -> J.Json:
  match empty:
    case True{}:
      Mob.btn("Dismiss", id ++ "|", False{})
    case False{}:
      Mob.btn("Answer with draft", id ++ "|" ++ String.trim(draft), True{})

def Mob.ask.btns(i: Nat, +id: String, +o: String, +draft: String) -> List<&2, J.Json>:
  match i:
    case 0n:
      [Mob.btn("Implement", id ++ "|implement", True{}), Mob.btn("Dismiss", id ++ "|dismiss", False{})]
    case 1n:
      List.append(&2, J.Json, Mob.ask.opts(String.split(o, '\n'), id), [Mob.ask.draft(String.is_empty(String.trim(draft)), id, draft)])
    case _:
      [Mob.btn("Allow", id ++ "|accept", True{}), Mob.btn("Allow for session", id ++ "|session", False{}), Mob.btn("Deny", id ++ "|decline", False{})]

def Mob.ask.head(i: Nat, +tool: String) -> String:
  match i:
    case 0n:
      "Proposed plan"
    case 1n:
      "The agent asks"
    case _:
      "Allow " ++ tool ++ "?"

def Mob.ask.body(plan: Bool, +d: String) -> List<&2, J.Json>:
  match plan:
    case True{}:
      Mob.nodes(D.Md.render(d))
    case False{}:
      Nil{}

def Mob.ask.std(a: M.Ask, +draft: String) -> J.Json:
  match a:
    case M.Ask{+id, _, +k, +tool, +d, +o, _, _}:
      J.Json.obj([J.KV{"id", J.Json.str(id)}, J.KV{"kind", J.Json.str(k)},
        J.KV{"head", J.Json.str(Mob.ask.head(C.Codec.pos(["plan", "input"], k), tool))},
        J.KV{"detail", J.Json.str(Bool.pick(String, String.eq(k, "plan"), "", d))},
        J.KV{"blocks", J.Json.arr(Mob.ask.body(String.eq(k, "plan"), d))},
        J.KV{"buttons", J.Json.arr(Mob.ask.btns(C.Codec.pos(["plan", "input"], k), id, o, draft))}])

def Mob.ask.k(web: Bool, a: M.Ask, +draft: String, nf: Map<&2, String>) -> J.Json:
  match web:
    case True{}:
      MB.MBots.ask(U.Teach.ask.from(a, nf))
    case False{}:
      Mob.ask.std(a, draft)

def Mob.ask(a: M.Ask, +draft: String, nf: Map<&2, String>) -> J.Json:
  match a:
    case M.Ask{_, _, +k, _, _, _, _, _}:
      Mob.ask.k(String.eq(k, "browser"), a, draft, nf)

def Mob.asks(as: List<&2, M.Ask>, +draft: String, nf: Map<&2, String>) -> List<&2, J.Json>:
  match as:
    case Nil{}:
      Nil{}
    case a <> t:
      Mob.ask(a, draft, nf) <> Mob.asks(t, draft, nf)

# Subagents (core/subs.bend): what this thread delegated ("select" with
# the row's key opens its thread) and the agent's own ("fold" with its
# value opens or shuts its steps), and the thread that delegated this one
def Mob.sub.step(s: SB.SubStep) -> J.Json:
  match s:
    case SB.SubStep{+kind, +text}:
      J.Json.obj([J.KV{"kind", J.Json.str(SB.Subs.step.label(kind))}, J.KV{"text", J.Json.str(text)}])

def Mob.sub.steps(ss: List<&2, SB.SubStep>) -> List<&2, J.Json>:
  match ss:
    case Nil{}:
      Nil{}
    case s <> t:
      Mob.sub.step(s) <> Mob.sub.steps(t)

def Mob.sub(r: SB.SubRow) -> J.Json:
  match r:
    case SB.SubRow{+key, +agent, +title, +who, +word, +doing, +live, +open, n, ss}:
      J.Json.obj([J.KV{"id", J.Json.str(key)}, J.KV{"agent", J.Json.bool(agent)}, J.KV{"title", J.Json.str(title)}, J.KV{"who", J.Json.str(who)},
        J.KV{"state", J.Json.str(word)}, J.KV{"doing", J.Json.str(doing)}, J.KV{"live", J.Json.bool(live)}, J.KV{"open", J.Json.bool(open)},
        J.KV{"n", J.Json.nat(n)}, J.KV{"steps", J.Json.arr(Mob.sub.steps(ss))},
        J.KV{"act", J.Json.str(Bool.pick(String, agent, "fold", "select"))}, J.KV{"value", J.Json.str(Bool.pick(String, agent, SB.Subs.ask(open, key), key))},
        J.KV{"cancel", J.Json.str(SB.Subs.cancel(SB.SubRow{key, agent, title, who, word, doing, live, open, 0n, Nil{}}))}])

def Mob.subs.go(rs: List<&2, SB.SubRow>) -> List<&2, J.Json>:
  match rs:
    case Nil{}:
      Nil{}
    case r <> rest:
      Mob.sub(r) <> Mob.subs.go(rest)

# the panel: how many are at work ("" with none), its word ("3 at work",
# "3 done"), whether it is open, the head's tap (the "fold" action with
# value), and its rows, none while shut (subs.bend's panel: done, it folds)
def Mob.subs.k(+open: Bool, +th: String, +rs: List<&2, SB.SubRow>) -> J.Json:
  J.Json.obj([J.KV{"busy", J.Json.str(SB.Subs.busy(SB.Subs.live(rs)))}, J.KV{"word", J.Json.str(SB.Subs.panel.word(rs))},
    J.KV{"open", J.Json.bool(open)}, J.KV{"act", J.Json.str("fold")}, J.KV{"value", J.Json.str(SB.Subs.panel.ask(open, th))},
    J.KV{"rows", J.Json.arr(Mob.subs.go(SB.Subs.panel.rows(open, rs)))}])

def Mob.subs(+info: Map<&2, String>, +th: String, +rs: List<&2, SB.SubRow>) -> J.Json:
  Bool.pick(J.Json, List.is_empty(&2, SB.SubRow, rs), J.Null{}, Mob.subs.k(SB.Subs.panel.open(info, th, rs), th, rs))

def Mob.parent(none: Bool, +st: M.State, +thread: String, +p: String) -> J.Json:
  match none:
    case True{}:
      J.Null{}
    case False{}:
      Mob.link(p, U.Tasks.up.label(st, thread, p), p)

# the skills a `$` being typed may complete to ("skill" with the name)
def Mob.skills(rs: List<&2, U.SkillRow>) -> List<&2, J.Json>:
  match rs:
    case Nil{}:
      Nil{}
    case U.SkillRow{n, d, on} <> t:
      J.Json.obj([J.KV{"name", J.Json.str(n)}, J.KV{"desc", J.Json.str(d)}, J.KV{"on", J.Json.bool(on)}]) <> Mob.skills(t)

# Diff: what the thread changed, file by file, lines on a reversed
# accumulator (a diff can run to thousands of lines)
def Mob.dline(l: DF.DLine) -> J.Json:
  match l:
    case DF.DLine{+k, t, +o, +n}:
      J.Json.obj([J.KV{"k", J.Json.u32(k)}, J.KV{"t", J.Json.str(t)}, J.KV{"o", J.Json.str(V.View.dl.no(o))}, J.KV{"n", J.Json.str(V.View.dl.no(n))}])

def Mob.dlines(ls: List<&2, DF.DLine>, acc: List<&2, J.Json>) -> List<&2, J.Json>:
  match ls:
    case Nil{}:
      acc
    case l <> t:
      Mob.dlines(t, Mob.dline(l) <> acc)

def Mob.dhead(empty: Bool, +hd: String, acc: List<&2, J.Json>) -> List<&2, J.Json>:
  match empty:
    case True{}:
      acc
    case False{}:
      J.Json.obj([J.KV{"k", J.Json.u32(4)}, J.KV{"t", J.Json.str(hd)}, J.KV{"o", J.Json.str("")}, J.KV{"n", J.Json.str("")}]) <> acc

def Mob.dhunks(hs: List<&2, DF.DHunk>, acc: List<&2, J.Json>) -> List<&2, J.Json>:
  match hs:
    case Nil{}:
      acc
    case DF.DHunk{+hd, ls} <> t:
      Mob.dhunks(t, Mob.dlines(ls, Mob.dhead(String.is_empty(hd), hd, acc)))

def Mob.dfiles(fs: List<&2, DF.DFile>) -> List<&2, J.Json>:
  match fs:
    case Nil{}:
      Nil{}
    case DF.DFile{_, +nw, st, hs} <> t:
      J.Json.obj([J.KV{"name", J.Json.str(nw)}, J.KV{"status", J.Json.str(st)},
        J.KV{"lines", J.Json.arr(List.reverse(&2, J.Json, Mob.dhunks(hs, Nil{})))}]) <> Mob.dfiles(t)

def Mob.diff.of(+fs: List<&2, DF.DFile>) -> J.Json:
  J.Json.obj([J.KV{"summary", J.Json.str(Bool.pick(String, List.is_empty(&2, DF.DFile, fs), "No changes since this thread began.", DF.Diff.summary(fs)))},
    J.KV{"files", J.Json.arr(Mob.dfiles(fs))}])

# shown once the hub has answered "diff", until "diff-close"
def Mob.diff(open: Bool, +text: String) -> J.Json:
  match open:
    case True{}:
      Mob.diff.of(DF.Diff.parse(text))
    case False{}:
      J.Null{}

# Terminal: the thread's shell as rows of styled runs (colours 0xRRGGBB)
def Mob.runs(rs: List<&2, VT.VRun>) -> List<&2, J.Json>:
  match rs:
    case Nil{}:
      Nil{}
    case VT.VRun{t, +fg, +bg, bo, ul} <> rest:
      J.Json.obj([J.KV{"t", J.Json.str(t)}, J.KV{"fg", J.Json.u32(fg)}, J.KV{"bg", J.Json.u32(bg)}, J.KV{"b", J.Json.bool(bo)}, J.KV{"u", J.Json.bool(ul)}]) <> Mob.runs(rest)

def Mob.tlines(ls: List<&2, VT.VLine>) -> List<&2, J.Json>:
  match ls:
    case Nil{}:
      Nil{}
    case VT.VLine{rs} <> t:
      J.Json.arr(Mob.runs(rs)) <> Mob.tlines(t)

def Mob.cursor(c: VT.VCur) -> J.Json:
  match c:
    case VT.VCur{x, y, on}:
      J.Json.obj([J.KV{"x", J.Json.u32(x)}, J.KV{"y", J.Json.u32(y)}, J.KV{"on", J.Json.bool(on)}])

def Mob.term.of(+v: VT.Vt) -> J.Json:
  J.Json.obj([J.KV{"title", J.Json.str(VT.Vt.title(v))}, J.KV{"fg", J.Json.u32(VT.Vt.fg())}, J.KV{"bg", J.Json.u32(VT.Vt.bg())},
    J.KV{"lines", J.Json.arr(Mob.tlines(VT.Vt.lines(v, 0n)))}, J.KV{"cursor", Mob.cursor(VT.Vt.cursor(v))}])

# Terminals in the chat (core/emb.bend): each of the thread's, by key, its
# title, size, whether it is live or pinned, the value that restarts it,
# and its screen while live. An entry row (kind "emb") names its key; a
# live one is drawn there, a pinned one at the top, an inactive one as a
# gray row whose tap sends "emb-restart" with "restart". Keys go as
# "emb-key" with "key<tab>DOM key<tab>mods", text as "emb-paste" with
# "key<tab>text", the pin as "emb-pin" with the key.
def Mob.emb.k(live: Bool, +u: U.Ui, +key: String) -> J.Json:
  match live:
    case True{}:
      Mob.term.of(U.Ui.term(u, key))
    case False{}:
      J.Null{}

def Mob.embs(vs: List<&2, EM.EV>, +u: U.Ui, +th: String) -> List<&2, J.Json>:
  match vs:
    case Nil{}:
      Nil{}
    case EM.EV{+key, +id, +title, +cols, +rows, +live, +pinned} <> t:
      J.Json.obj([J.KV{"key", J.Json.str(key)}, J.KV{"id", J.Json.str(id)}, J.KV{"title", J.Json.str(title)}, J.KV{"cols", J.Json.u32(cols)}, J.KV{"rows", J.Json.u32(rows)},
        J.KV{"live", J.Json.bool(live)}, J.KV{"pinned", J.Json.bool(pinned)}, J.KV{"restart", J.Json.str(th ++ "\t" ++ id)}, J.KV{"term", Mob.emb.k(live, u, key)}]) <> Mob.embs(t, u, th)

# open while "term-toggle" has it open
def Mob.term(open: Bool, +u: U.Ui, +id: String) -> J.Json:
  match open:
    case True{}:
      Mob.term.of(U.Ui.term(u, id))
    case False{}:
      J.Null{}

# The thread's menu: each item sends its action with its value, or (a
# snooze) offers choices whose values it sends instead; "on" checks it,
# "danger" asks first
def Mob.item(+icon: String, +label: String, +action: String, +value: String, on: Bool, danger: Bool, opts: List<&2, J.Json>) -> J.Json:
  J.Json.obj([J.KV{"icon", J.Json.str(icon)}, J.KV{"label", J.Json.str(label)}, J.KV{"action", J.Json.str(action)}, J.KV{"value", J.Json.str(value)},
    J.KV{"on", J.Json.bool(on)}, J.KV{"danger", J.Json.bool(danger)}, J.KV{"options", J.Json.arr(opts)}])

def Mob.snooze(snoozed: Bool) -> J.Json:
  match snoozed:
    case True{}:
      Mob.item("clock", "Snoozed", "snooze", "off", True{}, False{}, Nil{})
    case False{}:
      Mob.item("clock", "Snooze", "snooze", "", False{}, False{}, [Mob.opt("1 hour", "1h"), Mob.opt("Tomorrow", "tomorrow"), Mob.opt("1 week", "1w")])

def Mob.menu(+id: String, snoozed: Bool) -> List<&2, J.Json>:
  [Mob.item("diff", "Diff", "diff", "", False{}, False{}, Nil{}), Mob.item("terminal", "Terminal", "term-toggle", "", False{}, False{}, Nil{}),
   Mob.item("search", "Find file", "find-open", "files", False{}, False{}, Nil{}), Mob.snooze(snoozed),
   Mob.item("trash", "Delete", "row-delete", id, False{}, True{}, Nil{})]

# a phone opens a thread on its newest 20 entries (a screen shows about
# ten, and each reply's markdown is laid out in the phone's JS engine,
# the most of what opening a thread costs there); "earlier" adds pages
# of core's size (U.Page.size)
def Mob.count(+more: Nat) -> Nat:
  Nat.add(20n, Nat.mul(U.Page.size(), more))

# the newest page of a thread's entries (core's Page): "earlier" says how
# many are left out, and the action "earlier" shows another page
def Mob.page(+drafts: Map<&2, String>, +id: String, +st: M.State) -> U.Win:
  U.Page.newest.from(M.State.mine(st, id), id, U.Page.since(st, id), Mob.count(Maybe.default(&2, Nat, Nat.read(U.Str.get(drafts, "@more." ++ id)), 0n)))

def Mob.paged.es(w: U.Win, +st: M.State, +sel: String, +info: Map<&2, String>, +dr: Map<&2, String>, +running: Bool) -> List<&2, J.Json>:
  Mob.entries(U.Win.shown(w), st, sel, info, dr, running)

# a side question (/btw) and its answer, over the composer until closed
def Mob.btw(+u: U.Ui) -> J.Json:
  Bool.pick(J.Json, String.is_empty(U.Btw.question(u)), J.Null{},
    J.Json.obj([J.KV{"q", J.Json.str(U.Btw.question(u))}, J.KV{"a", J.Json.str(U.Btw.text(U.Btw.reply.of(u)))}]))

def Mob.thread(u: U.Ui, t: M.Thread, pend: List<&2, String>, vw: J.Json, +atts: List<&2, String>, +up: String, term: J.Json, skills: List<&2, J.Json>, +ph: M.Phase, btw: J.Json, embs: J.Json) -> J.Json:
  match u t:
    case U.Ui{+st, +sel, +drafts, +live, _, _, +now, _, pa, _, _, _, _, _, +nf, _, _} M.Thread{+id, _, title, _, _, +turn, _, archived, o, +sa, +ua, pinned, _, branch, _, +pr, +mo, _, _, _, _}:
      +pg = Mob.page(drafts, id, st)
      J.Json.obj([J.KV{"id", J.Json.str(id)}, J.KV{"title", J.Json.str(U.Label.title(st, id, title))}, J.KV{"branch", J.Json.str(branch)},
        J.KV{"state", J.Json.str(Mob.state(turn))},
        J.KV{"tools", J.Json.arr(Mob.head(V.View.running(turn), pinned, TO.Tools.settled(o, Nat.is_gt(sa, ua)), archived, Bool.not(String.is_empty(M.Fork.source(st, id)))))},
        J.KV{"menu", J.Json.arr(Mob.menu(id, M.Snooze.on(now, U.Side.until(st, id))))},
        J.KV{"parent", Mob.parent(String.is_empty(U.Tasks.up(st, id)), st, id, U.Tasks.up(st, id))},
        J.KV{"subs", Mob.subs(nf, id, SB.Subs.rows(st, nf, id))},
        J.KV{"entries", J.Json.arr(Mob.paged.es(pg, st, id, nf, drafts, V.View.running(turn)))},
        J.KV{"earlier", J.Json.nat(U.Win.hidden(pg))},
        J.KV{"sending", J.Json.arr(Mob.strs(pend))},
        J.KV{"asks", J.Json.arr(Mob.asks(U.Asks.open(st, id), U.Str.get(drafts, sel), nf))},
        J.KV{"queued", J.Json.str(M.Queue.of(st, id))},
        J.KV{"queue", J.Json.arr(Mob.queue(M.Queue.items(st, id), True{}, V.View.running(turn)))},
        J.KV{"todos", Mob.todos(nf, id, U.Todos.shown(st, id, V.View.running(turn)))},
        J.KV{"live", J.Json.arr(Mob.nodes(D.Md.render(U.Str.get(live, id))))},
        J.KV{"working", J.Json.str(Bool.pick(String, String.is_empty(U.Str.get(live, id)), M.Phase.label(ph), ""))},
        J.KV{"phase", J.Json.str(M.Phase.name(ph))},
        J.KV{"draft", J.Json.str(U.Str.get(drafts, sel))},
        J.KV{"send", J.Json.str(U.Follow.button(st, sel, V.View.running(turn), U.Follow.stop(V.View.running(turn), U.Str.get(drafts, sel), atts), U.Str.get(drafts, sel)))},
        J.KV{"sendAct", J.Json.str(U.Follow.act(U.Follow.stop(V.View.running(turn), U.Str.get(drafts, sel), atts)))},
        J.KV{"picker", Mob.picker.ctx(U.Str.get(nf, "models"), pr, mo, U.Effort.of(st, sel), U.Usage.tail(st, sel))},
        J.KV{"skills", J.Json.arr(skills)}, J.KV{"btw", btw},
        J.KV{"attaching", J.Json.arr(Mob.chips(atts))}, J.KV{"uploading", J.Json.str(up)},
        J.KV{"chunk", J.Json.nat(AT.Att.chunk())},
        J.KV{"cap", J.Json.nat(AT.Att.cap())},
        J.KV{"diff", Mob.diff(Bool.and(pa, String.eq(U.Str.get(nf, "view.kind:" ++ id), "diff")), U.Str.get(nf, "diff:" ++ id))},
        J.KV{"term", term},
        J.KV{"embs", embs},
        J.KV{"viewer", vw}])

def Mob.main(none: Bool, +u: U.Ui, +sel: String) -> J.Json:
  match none:
    case True{}:
      J.Null{}
    case False{}:
      Mob.thread(u, U.Ui.thread(Mob.st(u), sel), U.Ui.pending(u, sel), VW.View.json(u, Mob.review(String.eq(VW.View.kind(u), "review"), u)), U.Ui.atts(u, sel), U.Ui.uploading(u, sel),
        Mob.term(U.Ui.flag(u, "term"), u, sel), Mob.skills(U.Ui.skills(u)), U.Side.phase(Mob.st(u), U.Ui.thread(Mob.st(u), sel)), Mob.btw(u), J.Json.arr(Mob.embs(U.Emb.views(u, sel), u, sel)))

def Mob.empty(n: Nat) -> String:
  Bool.pick(String, Nat.is_eq(n, 0n), "Add a project to start.", "Pick a thread, or start one in a project.")

def Mob.title(t: M.Thread) -> String:
  match t:
    case M.Thread{_, _, title, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      title

# a delete a row asked for, to confirm: its thread, what to ask, the
# buttons (the yes button sends "row-delete id" again, the no "delete-no")
def Mob.deleting.k(none: Bool, +st: M.State, +id: String) -> J.Json:
  match none:
    case True{}:
      J.Null{}
    case False{}:
      J.Json.obj([J.KV{"id", J.Json.str(id)}, J.KV{"title", J.Json.str("Delete thread?")},
        J.KV{"body", J.Json.str(Mob.title(U.Ui.thread(st, id)))}, J.KV{"yes", J.Json.str("Delete")}, J.KV{"no", J.Json.str("Cancel")}])

# a thread being renamed (client.bend's Ren): its id, what to ask, the title
# so far ("text": a phone makes every "value" an id of its hub); the yes
# button sends "rename-save" with "id\u001ftitle", the no "rename-no"
def Mob.renaming.k(none: Bool, +id: String, +value: String) -> J.Json:
  match none:
    case True{}:
      J.Null{}
    case False{}:
      J.Json.obj([J.KV{"id", J.Json.str(id)}, J.KV{"title", J.Json.str("Rename thread")}, J.KV{"text", J.Json.str(value)},
        J.KV{"yes", J.Json.str("Rename")}, J.KV{"no", J.Json.str("Cancel")}])

def Mob.renaming(+u: U.Ui) -> J.Json:
  Mob.renaming.k(String.is_empty(U.Ren.on(u)), U.Ren.on(u), U.Ui.scratch(u, "@f.title"))

# the first-run tour (core/onboard.bend), while a step is on screen: its
# number, count and counter, its title and text, and the buttons (actions
# "ob-back", "ob-next", "ob-skip", sent with no value to the hub in focus),
# and the choices of the uberbot step (label, action, value: sent as they are)
def Mob.tour.choices(cs: List<&2, U.TChoice>) -> List<&2, J.Json>:
  match cs:
    case Nil{}:
      Nil{}
    case U.TChoice{l, a, v} <> t:
      J.Json.obj([J.KV{"label", J.Json.str(l)}, J.KV{"action", J.Json.str(a)}, J.KV{"value", J.Json.str(v)}]) <> Mob.tour.choices(t)

def Mob.tour.k(+i: Nat, +u: U.Ui) -> J.Json:
  J.Json.obj([J.KV{"step", J.Json.nat(i)}, J.KV{"count", J.Json.nat(OB.Ob.count())}, J.KV{"counter", J.Json.str(U.Tour.counter(u))},
    J.KV{"title", J.Json.str(OB.Ob.title(i))}, J.KV{"body", J.Json.str(U.Tour.body(2n, u))}, J.KV{"first", J.Json.bool(Bool.not(U.Tour.back(u)))},
    J.KV{"next", J.Json.str(OB.Ob.forward(i))}, J.KV{"back", J.Json.str("Back")}, J.KV{"skip", J.Json.str("Skip")},
    J.KV{"choices", J.Json.arr(Mob.tour.choices(U.Tour.choices(u)))}])

def Mob.tour.at(live: Bool, +i: Nat, +u: U.Ui) -> J.Json:
  match live:
    case True{}:
      Mob.tour.k(i, u)
    case False{}:
      J.Null{}

def Mob.tour(+u: U.Ui) -> J.Json:
  Mob.tour.at(U.Tour.on(u), U.Tour.at(u), u)

def Mob.deleting(+u: U.Ui) -> J.Json:
  Mob.deleting.k(String.is_empty(U.Ui.row_deleting(u)), Mob.st(u), U.Ui.row_deleting(u))

# a project remove to confirm ("proj-remove" armed it; "proj-remove" with
# id removes it, "proj-keep" cancels): the phones' second click
def Mob.removing.k(none: Bool, +st: M.State, +id: String) -> J.Json:
  match none:
    case True{}:
      J.Null{}
    case False{}:
      J.Json.obj([J.KV{"id", J.Json.str(id)}, J.KV{"title", J.Json.str("Remove " ++ U.Ui.project_title(st, id) ++ "?")},
        J.KV{"body", J.Json.str("Its threads are kept. Adding its folder again brings it back.")}, J.KV{"yes", J.Json.str("Remove")}, J.KV{"no", J.Json.str("Cancel")}])

def Mob.removing(+u: U.Ui) -> J.Json:
  Mob.removing.k(String.is_empty(U.Ui.scratch(u, "@rm")), Mob.st(u), U.Ui.scratch(u, "@rm"))

# the list's search: always shown at the top of the list (open stays true
# for older apps), its query ("proj-find-q"); it finds projects, or in the
# active view threads by their title or project
def Mob.search(+u: U.Ui) -> J.Json:
  J.Json.obj([J.KV{"open", J.Json.bool(True{})}, J.KV{"query", J.Json.str(U.Ui.scratch(u, "@f.pq"))}, J.KV{"hint", J.Json.str("Search")}])

# Settings, as the web's: rows of a label, a note and buttons, each sending
# its action with its value ("on" marks the current choice)
def Mob.set.btn(+label: String, +action: String, +value: String, on: Bool) -> J.Json:
  J.Json.obj([J.KV{"label", J.Json.str(label)}, J.KV{"action", J.Json.str(action)}, J.KV{"value", J.Json.str(value)}, J.KV{"on", J.Json.bool(on)}])

def Mob.set.chips(xs: List<&2, String>, +cur: String, +key: String) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case +v <> t:
      Mob.set.btn(Bool.pick(String, String.is_empty(v), "default", v), "setting", key ++ "=" ++ v, String.eq(v, cur)) <> Mob.set.chips(t, cur, key)

def Mob.set.row(+label: String, +note: String, btns: List<&2, J.Json>) -> J.Json:
  J.Json.obj([J.KV{"label", J.Json.str(label)}, J.KV{"note", J.Json.str(note)}, J.KV{"buttons", J.Json.arr(btns)}])

# a row with fields to type in: each sends "bfield" (name, text) as it is
# typed; a secret one shows dots
def Mob.set.field(+name: String, +hint: String, secret: Bool, +value: String) -> J.Json:
  J.Json.obj([J.KV{"name", J.Json.str(name)}, J.KV{"hint", J.Json.str(hint)}, J.KV{"secret", J.Json.bool(secret)}, J.KV{"value", J.Json.str(value)}])

def Mob.set.row.f(+label: String, +note: String, fs: List<&2, J.Json>, btns: List<&2, J.Json>) -> J.Json:
  J.Json.obj([J.KV{"label", J.Json.str(label)}, J.KV{"note", J.Json.str(note)}, J.KV{"fields", J.Json.arr(fs)}, J.KV{"buttons", J.Json.arr(btns)}])

# API keys (client.bend's ApiKeys), as on the web: Add and View, the form,
# the names each with Delete
def Mob.keys.rows(ns: List<&2, String>) -> List<&2, J.Json>:
  match ns:
    case Nil{}:
      Nil{}
    case +n <> t:
      Mob.set.row(n, "", [Mob.set.btn("Delete", "key-del", n, False{})]) <> Mob.keys.rows(t)

def Mob.keys.list(open: Bool, +u: U.Ui) -> List<&2, J.Json>:
  match open:
    case True{}:
      Bool.pick(List<&2, J.Json>, List.is_empty(&2, String, U.ApiKeys.names(u)), [Mob.set.row("No API keys yet", "", Nil{})], Mob.keys.rows(U.ApiKeys.names(u)))
    case False{}:
      Nil{}

def Mob.keys.form(open: Bool, +u: U.Ui) -> List<&2, J.Json>:
  match open:
    case True{}:
      [Mob.set.row.f("New API key", U.ApiKeys.form.note(), [Mob.set.field("kname", "Name", False{}, U.Ui.scratch(u, "@f.kname")), Mob.set.field("kval", "Key", True{}, "")],
        [Mob.set.btn("Save", "key-add", "", True{})])]
    case False{}:
      Nil{}

def Mob.keys(+u: U.Ui) -> List<&2, J.Json>:
  List.concat(&2, J.Json, [[Mob.set.row("API keys", U.ApiKeys.note(u), [Mob.set.btn("Add API key", "flag", "keyform", U.Ui.flag(u, "keyform")), Mob.set.btn("View API keys", "flag", "keys", U.Ui.flag(u, "keys"))])],
    Mob.keys.form(U.Ui.flag(u, "keyform"), u), Mob.keys.list(U.Ui.flag(u, "keys"), u)])

def Mob.set.days(+d: String) -> List<&2, J.Json>:
  +n = {Maybe.default(&2, Nat, Nat.read(d), 3n) : Nat}
  [Mob.set.btn("−", "setting", "settle.days=" ++ Nat.show(Bool.pick(Nat, Nat.is_gt(n, 1n), Nat.sub(n, 1n), 1n)), False{}),
   Mob.set.btn("+", "setting", "settle.days=" ++ Nat.show(Bool.pick(Nat, Nat.is_lt(n, 90n), 1n+n, 90n)), False{})]

def Mob.set.days.label(+d: String) -> String:
  +n = {Maybe.default(&2, Nat, Nat.read(d), 3n) : Nat}
  "Settle after " ++ Nat.show(n) ++ Bool.pick(String, Nat.is_eq(n, 1n), " day", " days")

# the version row's button (U.Update.btn)
def Mob.update.btn(i: Nat) -> List<&2, J.Json>:
  match i:
    case 1n:
      [Mob.set.btn("Install update", "update", "", False{})]
    case 2n:
      [Mob.set.btn("Check now", "update-check", "", False{})]
    case 3n:
      [Mob.set.btn("Update now", "update-now", "", False{})]
    case _:
      Nil{}

def Mob.set.strs(xs: List<&2, String>) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      J.Json.str(x) <> Mob.set.strs(t)

def Mob.set.pair.k(on: Bool, +btns: List<&2, J.Json>, +nf: Map<&2, String>) -> J.Json:
  match on:
    case True{}:
      J.Json.obj([J.KV{"label", J.Json.str("Pairing link")}, J.KV{"note", J.Json.str(U.Pair.note(U.Pair.link(nf)))}, J.KV{"buttons", J.Json.arr(btns)},
        J.KV{"qr", J.Json.arr(Mob.set.strs(U.Pair.qr(nf)))}, J.KV{"qrnote", J.Json.str(U.Pair.qr.note())}])
    case False{}:
      Mob.set.row("Pairing link", U.Pair.note(U.Pair.link(nf)), btns)

# the pairing link with Copy and QR code; while "pairqr" is on the row
# carries the code's rows ("qr": '1' dark), drawn under it
def Mob.set.pair(+on: Bool, +nf: Map<&2, String>) -> J.Json:
  +btns = {List.append(&2, J.Json, Bool.pick(List<&2, J.Json>, U.Pair.qr.ready(nf), [Mob.set.btn("QR code", "flag", "pairqr", on)], Nil{}),
    Bool.pick(List<&2, J.Json>, String.is_empty(U.Pair.link(nf)), Nil{}, [Mob.set.btn("Copy", "copy", U.Pair.link(nf), False{})])) : List<&2, J.Json>}
  Mob.set.pair.k(on, btns, nf)

# Your machines (U.Mate.rows): each of the owner's other machines, how its
# sync goes, and On/Off ("sync.<name>")
def Mob.mates.go(rs: List<&2, U.MateRow>) -> List<&2, J.Json>:
  match rs:
    case Nil{}:
      Nil{}
    case U.MateRow{+nm, +note, +on, +key} <> t:
      Mob.set.row(nm, note, Mob.set.chips(["on", "off"], on, key)) <> Mob.mates.go(t)

def Mob.mates(+u: U.Ui) -> List<&2, J.Json>:
  Mob.set.row(U.Mate.title(), U.Mate.lead(u), Nil{}) <> Mob.mates.go(U.Mate.rows(u))

def Mob.settings.of(+u: U.Ui, +st: M.State, +nf: Map<&2, String>, +v: String, +note: String, more: List<&2, J.Json>) -> J.Json:
  +prov = M.Setting.get(st, "agent.provider", "claude")
  +models = U.Str.get(nf, "models")
  +model = M.Setting.get(st, "agent.model", "")
  J.Json.obj([J.KV{"rows", J.Json.arr(List.append(&2, J.Json, List.append(&2, J.Json, [
    Mob.set.row(Mob.set.days.label(M.Setting.get(st, "settle.days", "3")), "Threads idle this long move to Settled. They stay saved.", Mob.set.days(M.Setting.get(st, "settle.days", "3"))),
    Mob.set.row("Continue after a restart", "Turns a restart cuts short go on by themselves.", Mob.set.chips(["on", "off"], Bool.pick(String, M.Carry.on(st), "on", "off"), "restart.continue")),
    Mob.set.row("Provider for new threads", Bool.pick(String, String.is_empty(U.Models.cli(models, prov)), prov ++ " is not installed", U.Models.cli(models, prov)), Mob.set.chips(["claude", "codex", "grok", "muse", "antigravity"], prov, "agent.provider")),
    Mob.set.row("Model for new threads", Bool.pick(String, String.is_empty(model), "default", model) ++ ": " ++ U.Models.label_of(models, prov, model), Mob.set.chips("" <> U.Models.list(models, prov), model, "agent.model")),
    Mob.set.row(U.KiCad.cli.label(nf), U.KiCad.cli.note(nf), Bool.pick(List<&2, J.Json>, String.is_empty(U.Str.get(nf, "kicad.fork")), [Mob.set.btn("Install Backplane KiCad", "kicad-install", "", False{})], Nil{})),
    Mob.set.row(U.FreeCAD.label(nf), U.FreeCAD.note(nf), Nil{}),
    Mob.set.row(U.KiCad.ipc.label(nf), U.KiCad.ipc.note(nf), Bool.pick(List<&2, J.Json>, U.KiCad.ipc.ready(nf), Mob.set.chips(["on", "off"], M.Setting.get(st, "kicad.ipc", "on"), "kicad.ipc"), Nil{})),
    Mob.set.row("Theme", "Unset, the phone follows its own system.", Mob.set.chips(["dark", "light"], M.Setting.get(st, "ui.theme", ""), "ui.theme")),
    Mob.set.row("Thread list", "Projects groups threads by project; active lists what is going on, what needs you first.", Mob.set.chips(["projects", "active"], U.Side.view(st), "sidebar.view")),
    Mob.set.row(U.Tail.label(nf), U.Tail.note(nf), Nil{})], List.append(&2, J.Json, Mob.mates(u), [
    Mob.set.pair(U.Ui.flag(u, "pairqr"), nf),
    Mob.set.row("Update channel", "Stable takes releases; nightly also takes the daily builds.", Mob.set.chips(["stable", "nightly"], M.Setting.get(st, "update.channel", "stable"), "update.channel")),
    Mob.set.row("Backplane " ++ v, U.Update.line.w(st, nf, note), Mob.update.btn(U.Update.btn(st, nf, note)))])), more))}])

# Antigravity (client.bend's Agv.rows), as every client draws them: a
# button "open-url" the app opens in the browser itself
def Mob.agy.btns(bs: List<&2, U.AgBtn>) -> List<&2, J.Json>:
  match bs:
    case Nil{}:
      Nil{}
    case U.AgBtn{+l, +a, +v, on} <> t:
      Mob.set.btn(l, a, v, on) <> Mob.agy.btns(t)

def Mob.agy.fields(fs: List<&2, U.AgField>) -> List<&2, J.Json>:
  match fs:
    case Nil{}:
      Nil{}
    case U.AgField{+n, +h, sec, +v, _} <> t:
      Mob.set.field(n, h, sec, v) <> Mob.agy.fields(t)

def Mob.agy.rows(rs: List<&2, U.AgRow>) -> List<&2, J.Json>:
  match rs:
    case Nil{}:
      Nil{}
    case U.AgRow{+l, +n, fs, bs} <> t:
      Mob.set.row.f(l, n, Mob.agy.fields(fs), Mob.agy.btns(bs)) <> Mob.agy.rows(t)

def Mob.agy(+u: U.Ui) -> List<&2, J.Json>:
  Mob.agy.rows(U.Agv.rows(u))

def Mob.settings.k(+u: U.Ui, more: List<&2, J.Json>) -> J.Json:
  match u:
    case U.Ui{+st, _, _, _, _, _, _, _, _, +v, _, _, _, +note, +nf, _, _}:
      Mob.settings.of(u, st, nf, v, note, more)

# shown while "flag settings" has it open (the API keys' rows last)
def Mob.settings(open: Bool, +u: U.Ui) -> J.Json:
  match open:
    case True{}:
      Mob.settings.k(u, List.concat(&2, J.Json, [Mob.keys(u), Mob.agy(u), [Mob.set.row("Tour", "The short walk through projects, threads, bots, your agents and your machines.", [Mob.set.btn("Show the tour", "ob-open", "", False{})])]]))
    case False{}:
      J.Null{}

# A list of rows that each send an action with a value (the search, the
# file picker, the project picker)
def Mob.items(ps: List<&2, U.PItem>) -> List<&2, J.Json>:
  match ps:
    case Nil{}:
      Nil{}
    case U.PItem{+l, +a, v} <> t:
      J.Json.obj([J.KV{"label", J.Json.str(l)}, J.KV{"action", J.Json.str(a)}, J.KV{"value", J.Json.str(v)}, J.KV{"kind", J.Json.str(U.Palette.kind(a))}]) <> Mob.items(t)

# thread search or the file picker ("find-open" search/files), while open
def Mob.find(open: Bool, +u: U.Ui) -> J.Json:
  match open:
    case True{}:
      J.Json.obj([J.KV{"mode", J.Json.str(U.Palette.mode(u))}, J.KV{"query", J.Json.str(U.Palette.query(u))},
        J.KV{"rows", J.Json.arr(Mob.items(List.take(&2, U.PItem, U.Palette.items(u), 100n)))}])
    case False{}:
      J.Null{}

def Mob.focus.thread(focus: Bool, +u: U.Ui, +sel: String) -> J.Json:
  match focus:
    case True{}:
      Mob.main(String.is_empty(sel), u, sel)
    case False{}:
      J.Null{}

def Mob.focus.bot(focus: Bool, +u: U.Ui) -> J.Json:
  match focus:
    case True{}:
      MB.MBots.json(u)
    case False{}:
      J.Null{}

def Mob.now(u: U.Ui) -> Nat:
  match u:
    case U.Ui{_, _, _, _, _, _, now, _, _, _, _, _, _, _, _, _, _}:
      now

def Mob.open(u: U.Ui) -> Map<&2, Bool>:
  match u:
    case U.Ui{_, _, _, _, o, _, _, _, _, _, _, _, _, _, _, _, _}:
      o

def Mob.scratch(u: U.Ui) -> Map<&2, String>:
  match u:
    case U.Ui{_, _, dr, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      dr

# the whole screen. A hub out of focus (focus False) builds no thread, bot
# or viewer (null: the merged screen reads them from the hub in focus),
# and the sidebar's groups are found once
def Mob.up(j: J.Json) -> Bool:
  match j:
    case J.Null{}:
      False{}
    case _:
      True{}

# something else is presented over the list (so the first-run tour waits,
# and shows once all of it is closed): an error, a menu, a delete, rename or
# remove question, Settings, Find, the folder picker, a bot or room form,
# and in the thread on screen its viewer, diff, terminal, history menu, and
# a bot's routine form. The apps add only what they keep to themselves (a
# picture opened large, the hubs sheet, the camera).
# when this phone last heard its hub (seconds; 0 never): the scratch
# "@heard", stamped while it is online (hubs.bend's One.run)
def Mob.heard(+u: U.Ui) -> Nat:
  M.Switch.num(U.Ui.scratch(u, "@heard"), 0n)

def Mob.heard.k(on: Bool, u: U.Ui, +now: Nat) -> U.Ui:
  match on:
    case True{}:
      U.Ui.scratch_set(u, "@heard", Nat.show(now))
    case False{}:
      u

def Mob.heard.set(u: U.Ui) -> U.Ui:
  match u:
    case U.Ui{st, sel, dr, live, op, +on, +now, ad, pa, v, rid, w, er, note, nf, ob, tr}:
      Mob.heard.k(on, U.Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}, now)

def Mob.covered.thread(+th: J.Json, +bot: J.Json) -> Bool:
  Bool.or(Bool.or(Bool.not(String.is_empty(J.Json.get_str(J.Json.get_obj(th, "viewer"), "open", ""))), Mob.up(J.Json.get_obj(th, "diff"))),
    Bool.or(Bool.or(Mob.up(J.Json.get_obj(th, "term")), Mob.up(J.Json.get_obj(J.Json.get_obj(J.Json.get_obj(th, "viewer"), "hist"), "menu"))),
      J.Json.get_bool(J.Json.get_obj(bot, "routine"), "open", False{})))

def Mob.covered.any(js: List<&2, J.Json>) -> Bool:
  match js:
    case Nil{}:
      False{}
    case h <> t:
      Bool.pick(Bool, Mob.up(h), True{}, Mob.covered.any(t))

def Mob.covered(+e: String, +ups: List<&2, J.Json>, +th: J.Json, +bot: J.Json) -> Bool:
  Bool.or(Bool.or(Bool.not(String.is_empty(e)), Mob.covered.any(ups)), Mob.covered.thread(th, bot))

def Mob.tour.gate(covered: Bool, +u: U.Ui) -> J.Json:
  match covered:
    case True{}:
      J.Null{}
    case False{}:
      Mob.tour(u)

def Mob.screen.go(+focus: Bool, +u: U.Ui, +ix: U.SIx, +g: U.SGroups, +side: MSide) -> J.Json:
  match u:
    case U.Ui{+st, +sel, +dr, _, +o, +on, +now, _, _, +v, _, _, +e, +note, _, _, _}:
      +rmenu = Mob.rmenu(u)
      +deleting = Mob.deleting(u)
      +renaming = Mob.renaming(u)
      +removing = Mob.removing(u)
      +settings = Mob.settings(U.Ui.flag(u, "settings"), u)
      +find = Mob.find(U.Ui.flag(u, "palette"), u)
      +folders = PK.MPick.json(u)
      +newbot = MB.MBots.new(u)
      +newroom = MB.MBots.rnew(u)
      +thread = Mob.focus.thread(focus, u, sel)
      +bot = Mob.focus.bot(focus, u)
      J.Json.obj([J.KV{"online", J.Json.bool(on)}, J.KV{"version", J.Json.str(v)}, J.KV{"seq", J.Json.nat(U.Ui.seq(u))}, J.KV{"error", J.Json.str(e)},
        J.KV{"note", J.Json.str(note)}, J.KV{"sel", J.Json.str(sel)}, J.KV{"making", J.Json.bool(U.Made.waiting(u))}, J.KV{"theme", J.Json.str(M.Setting.get(st, "ui.theme", ""))},
        J.KV{"empty", J.Json.str(Mob.empty(List.length(&2, M.Project, U.Ui.projects(st))))},
        J.KV{"projects", J.Json.arr(Mob.grouped(g, o, ix, st, now, U.Find.q(o, dr)))},
        J.KV{"older", Mob.older(g, o, U.Find.q(o, dr))},
        J.KV{"view", J.Json.str(U.Side.view(st))},
        J.KV{"active", J.Json.arr(MSide.rows(side))},
        J.KV{"settled", MSide.settled(side)},
        J.KV{"rowMenu", rmenu},
        J.KV{"search", Mob.search(u)},
        J.KV{"deleting", deleting},
        J.KV{"renaming", renaming},
        J.KV{"removing", removing},
        J.KV{"tour", Mob.tour.gate(Mob.covered(e, [rmenu, deleting, renaming, removing, settings, find, folders, newbot, newroom], thread, bot), u)},
        J.KV{"settings", settings},
        J.KV{"find", find},
        J.KV{"folders", folders},
        J.KV{"island", N.Note.island.h(N.Note.heard(on, now, Mob.heard(u)), U.Ui.threads(st), st)},
        J.KV{"thread", thread},
        J.KV{"uber", J.Null{}},
        J.KV{"plane", J.Null{}},
        J.KV{"bots", J.Json.arr(MB.MBots.rows(B.BU.rows(u)))},
        J.KV{"rooms", J.Json.arr(MB.MBots.rooms(B.BU.rooms(u)))},
        J.KV{"newBot", newbot},
        J.KV{"newRoom", newroom},
        J.KV{"bot", bot}])

# (the sidebar's rows, Side.rows.ix, made by the caller when it needs them too)
def Mob.screen.ixr(+focus: Bool, +u: U.Ui, +ix: U.SIx, +rows: List<&2, U.SRow>) -> J.Json:
  Mob.screen.go(focus, u, ix, U.Side.groups(U.Find.rows(rows, U.Find.q(Mob.open(u), Mob.scratch(u))), Mob.now(u)),
    Mob.side(String.eq(U.Side.view(U.Ui.state.of(u)), "active"), ix, U.Ui.state.of(u), Mob.now(u), U.Find.q(Mob.open(u), Mob.scratch(u)), Mob.open(u)))

def Mob.screen.ix(+focus: Bool, +u: U.Ui, +ix: U.SIx) -> J.Json:
  Mob.screen.ixr(focus, u, ix, U.Side.rows.ix(U.Ui.state.of(u), ix, Mob.now(u)))

# (what every row asks of the whole state is gathered once a screen:
# core's Side.ix)
def Mob.screen.f(focus: Bool, +u: U.Ui) -> J.Json:
  Mob.screen.ix(focus, u, U.Side.ix(U.Ui.state.of(u)))

def Mob.screen(+u: U.Ui) -> J.Json:
  Mob.screen.f(True{}, u)
