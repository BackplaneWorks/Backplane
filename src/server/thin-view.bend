import Base
import ../core/json.bend as J
import ../core/model.bend as M
import ../core/bot.bend as B
import ../core/botui.bend as BI
import ../core/client.bend as U
import ../core/presentation.bend as P
import ../core/thin.bend as T
import ../core/far.bend as FR
import ../core/hub.bend as HB
import ../core/subs.bend as SB
import ../core/thin-live.bend as TL
import ../mobile/thin.bend as MT
import ../web/dom.bend as D
import ../web/presentation.bend as WP
import ../web/sidebar-slot.bend as SS
import ../core/msgs.bend as MSG
import ../core/thin-wire.bend as W
import ../core/mail.bend as ML
import ../web/app.bend as A
import ../mobile/session.bend as MS
import ../mobile/hubs.bend as H
import ../mobile/folded-actions.bend as FA

# Pure server projection. The authoritative state is used for normal action
# decisions; only the bounded selected entry window reaches a renderer.
# No caller-supplied canonical state or global cursor is decoded.
def Obj.drop.hit(+fs: J.Json, +key: String) -> Bool:
  match fs:
    case J.Field{+k, _, _}: String.eq(k, key)
    case _: False{}

def Obj.drop.go(+fs: J.Json, +key: String, +hit: Bool) -> J.Json:
  match fs hit:
    case J.Field{_, _, +tail} True{}: Obj.drop.go(tail, key, Obj.drop.hit(tail, key))
    case J.Field{+k, +v, +tail} False{}: J.Field{k, v, Obj.drop.go(tail, key, Obj.drop.hit(tail, key))}
    case _ _: J.End{}

def Obj.drop(+fs: J.Json, +key: String) -> J.Json:
  Obj.drop.go(fs, key, Obj.drop.hit(fs, key))

def Obj.put(+j: J.Json, +key: String, +value: J.Json) -> J.Json:
  match j:
    case J.Obj{+fs}: J.Obj{J.Field{key, value, Obj.drop(fs, key)}}
    case _: J.Json.obj([J.KV{key, value}])

def Target.json(+kind: String, +id: String) -> J.Json:
  J.Json.obj([J.KV{"kind", J.Str{kind}}, J.KV{"id", J.Str{id}}])

def Target.thread.kind(+kind: String, +st: M.State, +id: String) -> String:
  match kind:
    case "thread": id
    case "bot": U.Bots.chat.of(st, id, B.Bot.thread(B.World.bot(M.State.bots(st), id)))
    case _: ""

def Target.thread(+st: M.State, +target: J.Json) -> String:
  Target.thread.kind(J.Json.get_str(target, "kind", ""), st, J.Json.get_str(target, "id", ""))

def Target.bind.kind(+kind: String, +u: U.Ui, +st: M.State, +id: String) -> U.Ui:
  match kind:
    case "thread": U.Bots.views(U.Bots.sel.set(u, id), "", "", "")
    case "bot": U.Bots.views(U.Bots.sel.set(u, Target.thread.kind("bot", st, id)), id, "", "")
    case "room": U.Bots.views(u, "", id, "")
    case _: U.Bots.views(U.Bots.sel.set(u, ""), "", "", "")

def Target.bind(+u: U.Ui, +st: M.State, +target: J.Json) -> U.Ui:
  Target.bind.kind(J.Json.get_str(target, "kind", ""), u, st, J.Json.get_str(target, "id", ""))

def Target.after.room(+empty: Bool, +u: U.Ui) -> J.Json:
  match empty:
    case False{}: Target.json("room", U.Ui.scratch(u, "@room"))
    case True{}: Bool.pick(J.Json, String.is_empty(U.Ui.sel.of(u)), Target.json("recent", ""), Target.json("thread", U.Ui.sel.of(u)))

def Target.after.bot(+empty: Bool, +u: U.Ui) -> J.Json:
  match empty:
    case False{}: Target.json("bot", U.Bots.cur(u))
    case True{}: Target.after.room(String.is_empty(U.Ui.scratch(u, "@room")), u)

def Target.after(+u: U.Ui) -> J.Json:
  Target.after.bot(String.is_empty(U.Bots.cur(u)), u)

def Threads.get(+st: M.State) -> Map<&2, M.Thread>:
  match st:
    case M.State{_, +ts, _, _, _, _, _, _, _, _, _}: ts

def Threads.has.k( p: Map<&2, M.Thread> & Bool) -> Bool:
  (_, b) = p
  b

def Target.exists.kind(+kind: String, +st: M.State, +id: String) -> Bool:
  match kind:
    case "thread": Threads.has.k(Map.has(&2, M.Thread, Threads.get(st), id))
    case "bot": B.Bot.live(B.World.bot(M.State.bots(st), id))
    case "room": B.Room.live(B.World.room(M.State.bots(st), id))
    case "recent": True{}
    case _: False{}

def Target.exists(+st: M.State, +target: J.Json) -> Bool:
  Bool.and(W.Target.valid(target), Target.exists.kind(J.Json.get_str(target, "kind", ""), st, J.Json.get_str(target, "id", "")))

def Threads.pick(+ids: List<&2, String>, +st: M.State, +acc: Map<&2, M.Thread>) -> Map<&2, M.Thread>:
  match ids:
    case Nil{}: acc
    case +id <> +tail: Threads.pick(tail, st, Map.set(&2, M.Thread, acc, id, M.State.thread.get(st, id)))

def Projects.id(+t: M.Thread) -> String:
  match t:
    case M.Thread{_, +p, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}: p

def Projects.get.k( p: Map<&2, M.Project> & M.Project) -> M.Project:
  (_, project) = p
  project

def Projects.pick(+ts: List<&2, M.Thread>, +ps: Map<&2, M.Project>, +acc: Map<&2, M.Project>) -> Map<&2, M.Project>:
  match ts:
    case Nil{}: acc
    case +t <> +tail:
      +id = Projects.id(t)
      Projects.pick(tail, ps, Map.set(&2, M.Project, acc, id, Projects.get.k(Map.get(M.Project, M.Project.blank(id), ps, id))))

def State.window.empty(+st: M.State, +threads: Map<&2, M.Thread>) -> M.State:
  match st:
    case M.State{+ps, _, _, +n, +tk, +settings, +queued, +asks, _, +nav, +bw}:
      M.State{ps, threads, Nil{}, n, tk, settings, queued, asks, Map.new(&2, M.Device), nav,
        [M.Ext{M.Ext.world(M.Ext.of(bw)), Map.new(&2, List<&2, M.Entry>), M.Ext.mail(M.Ext.of(bw))}]}

def State.window.add(+es: List<&2, M.Entry>, +st: M.State) -> M.State:
  match es:
    case Nil{}: st
    case +e <> +tail: State.window.add(tail, M.State.post(st, e))

# A launched thread's report stored cut short (before reports were kept
# whole) reads whole: hub.bend's Lr.whole checks the row is exactly the old
# cut form of the reply its id links to; any other row is read as stored.
def Message.whole.k(+row: T.Row, +text: String) -> T.Row:
  match row:
    case T.Row{+n, +e, +terms, +blocks}:
      Bool.pick(T.Row, String.eq(text, T.Entry.text(e)), T.Row{n, e, terms, blocks}, T.Row{n, T.Entry.with_text(e, text), terms, T.Blocks.of(text)})

def Message.whole.e(+st: M.State, +row: T.Row, +e: M.Entry) -> T.Row:
  match e:
    case M.Msg{+id, _, _, +text, +at}: Message.whole.k(row, HB.Lr.whole(st, id, text, at))
    case _: row

# A mirror row already carries its owner's body. Never repair it from a
# same-named local launch receipt, which belongs to another source.
def Message.whole.source(far: Bool, +st: M.State, +row: T.Row) -> T.Row:
  match far:
    case True{}: row
    case False{}: Message.whole.e(st, row, T.Row.entry(row))

def Message.whole(+st: M.State, +row: T.Row) -> T.Row:
  Message.whole.source(FR.Fr.is(M.Idx.thread(T.Row.entry(row))), st, row)

# the window's entries: each row as shown (T.Row.preview), a report stored
# cut short whole again (Message.whole), so no message reads cut
def State.window.rows(ids: List<&2, String>, +st: M.State, +index: T.Index, acc: List<&2, M.Entry>) -> List<&2, M.Entry>:
  match ids:
    case Nil{}: acc
    case id <> tail: State.window.rows(tail, st, index, T.Row.preview(Message.whole(st, T.Index.row.k(Map.get(T.Row, T.Row.blank(), T.Index.rows(index), id)))) <> acc)

# Auxiliary steps belong only to this selected conversation. Keep their
# existing bounded window in original order, without importing another body.
def State.window.subs(+st: M.State, +target: J.Json) -> List<&2, M.Entry>:
  List.reverse(&2, M.Entry, List.take(&2, M.Entry, M.State.mine(st, M.Bg.log(Target.thread(st, target))), SB.Subs.window()))

def State.window(+st: M.State, +index: T.Index, +target: J.Json, +rows: List<&2, String>) -> M.State:
  # Metadata feeds the sidebar; selected conversation history and its own
  # agent steps feed the timeline and inspector independently.
  State.window.add(State.window.rows(rows, st, index, Nil{}),
    State.window.add(State.window.subs(st, target), State.window.empty(st, Threads.get(st))))

# A route restores only transient selection. The normal bot tab policy
# decides whether that content route is still available for this exact bot.
def Route.json( +bot: String, +tab: String) -> J.Json:
  J.Json.obj([J.KV{"kind", J.Str{"bot-tab"}}, J.KV{"bot", J.Str{bot}}, J.KV{"tab", J.Str{tab}}])

def Route.tab(+tab: String) -> Bool:
  BI.BU.tab.has(["chat", "mail", "status", "space", "browser", "tasks", "memory", "routines", "hooks", "settings"], tab)

def Route.fields( +fs: J.Json, +count: Nat) -> Bool:
  match fs:
    case J.Obj{fields}: Route.fields(fields, count)
    case J.End{}: Nat.is_eq(count, 3n)
    case J.Field{+key, J.Str{_}, tail}: Bool.and(Bool.or(String.eq(key, "kind"), Bool.or(String.eq(key, "bot"), String.eq(key, "tab"))), Route.fields(tail, Nat.add(count, 1n)))
    case _: False{}

def Route.valid(+route: J.Json, +target: J.Json) -> Bool:
  Bool.and(Route.fields(route, 0n), Bool.and(String.eq(J.Json.get_str(route, "kind", ""), "bot-tab"),
    Bool.and(String.eq(J.Json.get_str(target, "kind", ""), "bot"), Bool.and(Bool.not(String.is_empty(J.Json.get_str(route, "bot", ""))),
      Bool.and(String.eq(J.Json.get_str(route, "bot", ""), J.Json.get_str(target, "id", "")), Route.tab(J.Json.get_str(route, "tab", "")))))))

def Route.choose( +allowed: Bool, +u: U.Ui, +tab: String, +bot: String) -> U.Ui:
  match allowed:
    case True{}: U.Ui.scratch_set(u, "@btab", tab)
    case False{}: U.Ui.scratch_set(u, "@btab", BI.BU.tab.default(u, bot))

def Route.restore( +valid: Bool, +u: U.Ui, +route: J.Json, +bot: String) -> U.Ui:
  match valid:
    case False{}: U.Ui.scratch_set(u, "@btab", "")
    case True{}: Route.choose(BI.BU.nav.accept(BI.BU.tabs.for(u, bot), J.Json.get_str(route, "tab", "")), u, J.Json.get_str(route, "tab", ""), bot)

def Route.pack_if( +bot: Bool, +u: U.Ui) -> J.Json:
  match bot:
    case False{}: P.Session.pack(u)
    case True{}: Obj.put(P.Session.pack(u), "route", Route.json(U.Bots.cur(u), BI.BU.tab.of(u, U.Bots.cur(u))))

def Route.pack(+u: U.Ui) -> J.Json:
  Route.pack_if(Bool.not(String.is_empty(U.Bots.cur(u))), u)

# These explicit device renderer hints affect presentation only. They cannot
# change the authoritative state, issue commands or enable automation.
def Session.native_local(+u: U.Ui, +p: J.Json) -> U.Ui:
  H.Hubs.local.ui(3n, u, "mailroom-renderer", J.Json.get_str(p, "nativeRenderer", ""))

def Session.renderer( +web: Bool, +u: U.Ui, +p: J.Json) -> U.Ui:
  match web:
    case True{}:
      A.stamp(U.Ui.info_set(U.Ui.info_set(U.Ui.info_set(u, "local.terminal.output", Bool.pick(String, String.eq(J.Json.get_str(p, "terminalOutput", ""), "pane-v1"), "pane-v1", "")), "local.workspace.layout", Bool.pick(String, String.eq(J.Json.get_str(p, "workspaceLayout", ""), "rail-v1"), "rail-v1", "legacy")),
        "local.workspace.chrome", Bool.pick(String, String.eq(J.Json.get_str(p, "workspaceChrome", ""), "top-v1"), "top-v1", "none")))
    case False{}: Session.native_local(u, p)

def Session.input(+st: M.State, +info: J.Json, +live: J.Json, +now: Nat, +p: J.Json) -> U.Ui:
  Target.bind(U.Ui.scratch_set(P.Session.input(st, info, live, now, J.Json.get_str(p, "cid", ""), J.Json.get_obj(p, "session")), "@presentation", ""), st, J.Json.get_obj(p, "target"))

def Session.current(+web: Bool, +st: M.State, +info: J.Json, +live: J.Json, +now: Nat, +p: J.Json) -> U.Ui:
  +target = J.Json.get_obj(p, "target")
  +route = J.Json.get_obj(J.Json.get_obj(p, "session"), "route")
  Route.restore(Route.valid(route, target), Session.renderer(web, Session.input(st, info, live, now, p), p), route, J.Json.get_str(target, "id", ""))

def Action.web(+empty: Bool, +u: U.Ui, +action: String, +value: String) -> U.Act:
  match empty:
    case True{}: U.Act{u, Nil{}}
    case False{}: A.act(u, action, value)

# Mobile commands keep their normal qualified hub routing separately.
def Render.mobile(+u: U.Ui, +key: String, +cid: String, +now: Nat) -> J.Json:
  MS.Session.result.shown(H.Hubs{cid, now, key, [H.Hub{key, u}]}, Nil{}, key, H.Hubs.shown(H.Hubs.loc(H.Hubs{cid, now, key, [H.Hub{key, u}]}), [H.Hub{key, u}], key))

def Render.shown(+web: Bool, +u: U.Ui, +p: J.Json, +now: Nat) -> J.Json:
  match web:
    case True{}: A.presentation_view(u)
    case False{}: J.Json.get_obj(Render.mobile(u, J.Json.get_str(p, "hub", ""), J.Json.get_str(p, "cid", ""), now), "screen")

# Pure metadata for explicit chunks. No body text or implicit load request.
def Partial.one(+partial: Bool, +row: T.Row, +cursor: String) -> J.Json:
  match partial:
    case False{}: J.Null{}
    case True{}: J.Json.obj([J.KV{"id", J.Str{M.Idx.id(T.Row.entry(row))}}, J.KV{"version", J.Str{Nat.show(T.Row.n(row))}}, J.KV{"cursor", J.Str{cursor}}, J.KV{"hasMore", J.Flag{True{}}}, J.KV{"complete", J.Flag{False{}}}, J.KV{"reason", J.Str{"byte-limit"}}])

def Partial.include(+part: J.Json, +acc: List<&2, J.Json>) -> List<&2, J.Json>:
  match part:
    case J.Null{}: acc
    case +p: p <> acc

def Partial.rows(+ids: List<&2, String>, +index: T.Index, +epoch: String, +target: J.Json, +acc: List<&2, J.Json>) -> List<&2, J.Json>:
  match ids:
    case Nil{}: List.reverse(&2, J.Json, acc)
    case +id <> +tail:
      +r = T.Index.row.k(Map.get(T.Row, T.Row.blank(), T.Index.rows(index), id))
      +part = Partial.one(T.Row.partial(r), r, W.Cursor.write(epoch, target, "message:" ++ id, 1n))
      Partial.rows(tail, index, epoch, target, Partial.include(part, acc))

def Page.last(+ids: List<&2, String>, +acc: String) -> String:
  match ids:
    case Nil{}: acc
    case +id <> +tail: Page.last(tail, id)

def Page.number(+id: String) -> Nat:
  Maybe.default(&2, Nat, P.Session.nat(id, P.Session.nat.max()), 0n)

def Ids.json(+xs: List<&2, String>) -> List<&2, J.Json>:
  match xs:
    case Nil{}: Nil{}
    case +x <> +tail: J.Str{x} <> Ids.json(tail)

def Page.older( +hasRows: Bool, +index: T.Index, +th: String, +last: Nat) -> List<&2, String>:
  match hasRows:
    case False{}: Nil{}
    case True{}: T.Index.page(index, th, last, 1n)

def Anchor.safe_if( +valid: Bool, +a: J.Json) -> J.Json:
  match valid:
    case False{}: J.Json.obj([J.KV{"id", J.Str{""}}, J.KV{"offset", J.Num{"0"}}])
    case True{}: J.Json.obj([J.KV{"id", J.Str{J.Json.get_str(a, "id", "")}}, J.KV{"offset", J.Json.nat(Maybe.default(&2, Nat, W.Wire.nat(a, "offset", 4294967295n), 0n))}])

def Anchor.safe(+a: J.Json) -> J.Json:
  Anchor.safe_if(Bool.and(W.Wire.within(J.Json.get_str(a, "id", ""), 1024n), Maybe.is_some(&2, Nat, W.Wire.nat(a, "offset", 4294967295n))), a)

# The next older page starts halfway up this one (Page.mid): the two share
# half their rows, so the rows a reader is looking at stay on the page
# while the earlier half comes in above them, and the view holds still
# (host.js's holdFix keeps the entry at the top where it was). A page is
# never more than 32 rows, however long the thread.
def Page.mid(+ids: List<&2, String>, +last: String) -> String:
  Maybe.default(&2, String, List.head(&2, String, List.drop(&2, String, ids, 15n)), last)

def Page.window(+index: T.Index, +th: String, +p: J.Json, +epoch: String, +ids: List<&2, String>) -> J.Json:
  +last = Page.last(ids, "")
  +older = Page.older(Bool.not(List.is_empty(&2, String, ids)), index, th, Page.number(last))
  J.Json.obj([J.KV{"ids", J.Json.arr(Ids.json(ids))}, J.KV{"anchor", Anchor.safe(J.Json.get_obj(p, "anchor"))}, J.KV{"older", J.Str{Bool.pick(String, List.is_empty(&2, String, older), "", W.Cursor.write(epoch, J.Json.get_obj(p, "target"), "", Page.number(Page.mid(ids, last))))}}, J.KV{"hasMore", J.Flag{Bool.not(List.is_empty(&2, String, older))}}, J.KV{"remaining", J.Json.nat(Bool.pick(Nat, List.is_empty(&2, String, ids), 0n, T.Index.remaining(index, th, Page.number(last))))}, J.KV{"partial", J.Json.arr(Partial.rows(ids, index, epoch, J.Json.get_obj(p, "target"), Nil{}))}])

type ActResult is Data:
  ActResult{ui: U.Ui, commands: List<&2, J.Json>}

def ActResult.web(+a: U.Act) -> ActResult:
  match a:
    case U.Act{+u, +cs}: ActResult{u, P.Session.commands(cs)}

def ActResult.mobile(+a: H.HAct, +key: String) -> ActResult:
  match a:
    case H.HAct{+hs, +cs, _}: ActResult{H.Hubs.ui(hs, key), MS.Session.commands(cs)}

def Action.run(+web: Bool, +u: U.Ui, +p: J.Json, +now: Nat) -> ActResult:
  match web:
    case True{}: ActResult.web(Action.web(String.is_empty(J.Json.get_str(p, "action", "")), u, J.Json.get_str(p, "action", ""), J.Json.get_str(p, "value", "")))
    case False{}: ActResult.mobile(MS.Session.act(u, J.Json.get_str(p, "hub", ""), J.Json.get_str(p, "cid", ""), now, J.Json.get_str(p, "action", ""), J.Json.get_str(p, "value", "")), J.Json.get_str(p, "hub", ""))

# A streamed reply is shown whole (TL.Stream.preview), so it is never partial.
def Partial.live( +s: TL.Stream, +thread: String, +epoch: String, +target: J.Json) -> List<&2, J.Json>:
  []

def Window.live(+window: J.Json, +parts: List<&2, J.Json>) -> J.Json:
  Obj.put(window, "partial", J.Json.arr(List.append(&2, J.Json, J.Json.elems(J.Json.get_obj(window, "partial")), parts)))

def Render.window(+s: TL.Stream, +index: T.Index, +thread: String, +p: J.Json, +epoch: String, +rows: List<&2, String>) -> J.Json:
  Window.live(Page.window(index, thread, p, epoch, rows), Partial.live(s, thread, epoch, J.Json.get_obj(p, "target")))

def Render.ui(+u: U.Ui, +st: M.State, +index: T.Index, +target: J.Json, +rows: List<&2, String>, +stream: TL.Stream, +now: Nat) -> U.Ui:
  U.Ui.adopt.raw(U.Ui.scratch_set(U.Ui.scratch_set(u, "@more." ++ Target.thread(st, target), "1"), "@away." ++ Target.thread(st, target), ""), State.window.add(TL.Stream.entries(Nat.is_gt(TL.Stream.size(stream), 0n), stream, Target.thread(st, target), now), State.window(st, index, target, rows)))

def Partial.buttons( +parts: List<&2, J.Json>, +u: U.Ui) -> List<&2, D.Node>:
  match parts:
    case Nil{}: Nil{}
    case +part <> tail:
      D.Dom.button("thin-message-more", "thin-message-more", J.Json.show(J.Json.obj([J.KV{"entryId", J.Str{J.Json.get_str(part, "id", "")}}, J.KV{"cursor", J.Str{""}}])),
        MSG.T.common.more(U.Lang.loc(u)), MSG.T.common.more(U.Lang.loc(u))) <> Partial.buttons(tail, u)

# Earlier rows come in as the reader scrolls up (no button): the mark at
# the timeline's top is what host.js's earlierCheck looks for when the view
# is within two screens of it
def Render.older( +more: Bool, +u: U.Ui) -> List<&2, D.Node>:
  match more:
    case False{}: Nil{}
    case True{}: [D.El{"div", "earlier", [D.At{"class", "earlier"}, D.At{"data-earlier", "1"}], Nil{}}]

# Keep the normal app root and its percentage-height chain. Paging and
# continuation controls belong inside the existing scrollable timeline.
def Render.timeline.id(attrs: List<&2, D.Attr>) -> Bool:
  match attrs:
    case Nil{}: False{}
    case D.At{"id", "timeline"} <> _: True{}
    case _ <> tail: Render.timeline.id(tail)

def Render.timeline.children(hit: Bool, kids: List<&2, D.Node>, before: List<&2, D.Node>, after: List<&2, D.Node>) -> List<&2, D.Node>:
  match hit:
    case False{}: kids
    case True{}: List.append(&2, D.Node, before, List.append(&2, D.Node, kids, after))

def Render.timeline.go(nodes: List<&2, D.Node>, +before: List<&2, D.Node>, +after: List<&2, D.Node>, acc: List<&2, D.Node>) -> List<&2, D.Node>:
  match nodes:
    case Nil{}: acc
    case D.Txt{text} <> tail: Render.timeline.go(tail, before, after, D.Txt{text} <> acc)
    case D.El{tag, key, +attrs, kids} <> tail:
      Render.timeline.go(tail, before, after, D.El{tag, key, attrs,
        Render.timeline.children(Render.timeline.id(attrs), List.reverse(&2, D.Node, Render.timeline.go(kids, before, after, Nil{})), before, after)} <> acc)

def Render.timeline(node: D.Node, before: List<&2, D.Node>, after: List<&2, D.Node>) -> D.Node:
  WP.Node.read.first(Render.timeline.go([node], before, after, Nil{}))

def Render.web(+u: U.Ui, +window: J.Json) -> J.Json:
  WP.Node.write(Render.timeline(A.view(u), Render.older(J.Json.get_bool(window, "hasMore", False{}), u), Partial.buttons(J.Json.elems(J.Json.get_obj(window, "partial")), u)))

def Render.web.cached.checked(valid: Bool, shown: J.Json, sidebar: J.Json, u: U.Ui, window: J.Json) -> J.Json:
  match valid:
    case True{}: SS.Slot.replace(shown, sidebar)
    case False{}: Render.web(u, window)

def Render.web.cached(+u: U.Ui, +window: J.Json, +sidebar: J.Json) -> J.Json:
  match sidebar:
    case J.Null{}: Render.web(u, window)
    case _:
      +shown = WP.Node.write(Render.timeline(A.view_sidebar(u, True{}), Render.older(J.Json.get_bool(window, "hasMore", False{}), u), Partial.buttons(J.Json.elems(J.Json.get_obj(window, "partial")), u)))
      Render.web.cached.checked(Bool.and(Nat.is_eq(SS.Slot.count(shown), 1n), SS.Slot.is(sidebar, "aside")), shown, sidebar, u, window)

# A normal complete reply with a server-owned, unchanged sidebar region.
# Its session, window, commands, timeline and all other regions remain fresh.
def Render.web.prepared(+a: ActResult, prepared: U.Ui, +window: J.Json, sidebar: J.Json) -> J.Json:
  match a:
    case ActResult{+u, cmds}:
      Obj.put(Obj.put(Obj.put(Obj.put(J.Json.obj([J.KV{"shown", Render.web.cached(prepared, window, sidebar)}]),
        "remoteReady", J.Flag{True{}}), "session", Route.pack(u)), "cmds", J.Json.arr(cmds)), "window", window)

def Render.web.indexed.full(+u: U.Ui, +window: J.Json, signals: Map<&2, Nat>) -> J.Json:
  WP.Node.write(Render.timeline(A.view_sidebar_indexed(u, False{}, signals), Render.older(J.Json.get_bool(window, "hasMore", False{}), u), Partial.buttons(J.Json.elems(J.Json.get_obj(window, "partial")), u)))

def Render.web.indexed.checked(valid: Bool, shown: J.Json, sidebar: J.Json, u: U.Ui, window: J.Json, signals: Map<&2, Nat>) -> J.Json:
  match valid:
    case True{}: SS.Slot.replace(shown, sidebar)
    case False{}: Render.web.indexed.full(u, window, signals)

def Render.web.indexed(+u: U.Ui, +window: J.Json, +sidebar: J.Json, +signals: Map<&2, Nat>) -> J.Json:
  match sidebar:
    case J.Null{}: Render.web.indexed.full(u, window, signals)
    case _:
      +shown = WP.Node.write(Render.timeline(A.view_sidebar_indexed(u, True{}, signals), Render.older(J.Json.get_bool(window, "hasMore", False{}), u), Partial.buttons(J.Json.elems(J.Json.get_obj(window, "partial")), u)))
      Render.web.indexed.checked(Bool.and(Nat.is_eq(SS.Slot.count(shown), 1n), SS.Slot.is(sidebar, "aside")), shown, sidebar, u, window, signals)

def Render.web.prepared.indexed(+a: ActResult, prepared: U.Ui, +window: J.Json, sidebar: J.Json, signals: Map<&2, Nat>) -> J.Json:
  match a:
    case ActResult{+u, cmds}:
      Obj.put(Obj.put(Obj.put(Obj.put(J.Json.obj([J.KV{"shown", Render.web.indexed(prepared, window, sidebar, signals)}]),
        "remoteReady", J.Flag{True{}}), "session", Route.pack(u)), "cmds", J.Json.arr(cmds)), "window", window)

def Render.web.metadata.full(+u: U.Ui, +window: J.Json, signals: Map<&2, Nat>, +plain: U.SIx) -> J.Json:
  WP.Node.write(Render.timeline(A.view_sidebar_metadata(u, False{}, plain, signals), Render.older(J.Json.get_bool(window, "hasMore", False{}), u), Partial.buttons(J.Json.elems(J.Json.get_obj(window, "partial")), u)))

def Render.web.metadata.checked(valid: Bool, shown: J.Json, sidebar: J.Json, u: U.Ui, window: J.Json, signals: Map<&2, Nat>, +plain: U.SIx) -> J.Json:
  match valid:
    case True{}: SS.Slot.replace(shown, sidebar)
    case False{}: Render.web.metadata.full(u, window, signals, plain)

def Render.web.metadata(+u: U.Ui, +window: J.Json, +sidebar: J.Json, +signals: Map<&2, Nat>, +plain: U.SIx) -> J.Json:
  match sidebar:
    case J.Null{}: Render.web.metadata.full(u, window, signals, plain)
    case _:
      +shown = WP.Node.write(Render.timeline(A.view_sidebar_metadata(u, True{}, plain, signals), Render.older(J.Json.get_bool(window, "hasMore", False{}), u), Partial.buttons(J.Json.elems(J.Json.get_obj(window, "partial")), u)))
      Render.web.metadata.checked(Bool.and(Nat.is_eq(SS.Slot.count(shown), 1n), SS.Slot.is(sidebar, "aside")), shown, sidebar, u, window, signals, plain)

def Render.web.prepared.metadata(+a: ActResult, prepared: U.Ui, +window: J.Json, sidebar: J.Json, signals: Map<&2, Nat>, +plain: U.SIx) -> J.Json:
  match a:
    case ActResult{+u, cmds}:
      Obj.put(Obj.put(Obj.put(Obj.put(J.Json.obj([J.KV{"shown", Render.web.metadata(prepared, window, sidebar, signals, plain)}]),
        "remoteReady", J.Flag{True{}}), "session", Route.pack(u)), "cmds", J.Json.arr(cmds)), "window", window)

# Current recipient info overlays a canonical-only signal text map.
def Render.web.materialized.full(+u: U.Ui, +window: J.Json, signals: Map<&2, Nat>, +plain: U.SIx, +text: Map<&2, String>) -> J.Json:
  WP.Node.write(Render.timeline(A.view_sidebar_materialized(u, False{}, plain, signals, text), Render.older(J.Json.get_bool(window, "hasMore", False{}), u), Partial.buttons(J.Json.elems(J.Json.get_obj(window, "partial")), u)))

def Render.web.materialized.checked(valid: Bool, shown: J.Json, sidebar: J.Json, u: U.Ui, window: J.Json, signals: Map<&2, Nat>, +plain: U.SIx, +text: Map<&2, String>) -> J.Json:
  match valid:
    case True{}: SS.Slot.replace(shown, sidebar)
    case False{}: Render.web.materialized.full(u, window, signals, plain, text)

def Render.web.materialized(+u: U.Ui, +window: J.Json, +sidebar: J.Json, +signals: Map<&2, Nat>, +plain: U.SIx, +text: Map<&2, String>) -> J.Json:
  match sidebar:
    case J.Null{}: Render.web.materialized.full(u, window, signals, plain, text)
    case _:
      +shown = WP.Node.write(Render.timeline(A.view_sidebar_materialized(u, True{}, plain, signals, text), Render.older(J.Json.get_bool(window, "hasMore", False{}), u), Partial.buttons(J.Json.elems(J.Json.get_obj(window, "partial")), u)))
      Render.web.materialized.checked(Bool.and(Nat.is_eq(SS.Slot.count(shown), 1n), SS.Slot.is(sidebar, "aside")), shown, sidebar, u, window, signals, plain, text)

def Render.web.prepared.materialized(+a: ActResult, prepared: U.Ui, +window: J.Json, sidebar: J.Json, signals: Map<&2, Nat>, +plain: U.SIx, +text: Map<&2, String>) -> J.Json:
  match a:
    case ActResult{+u, cmds}:
      Obj.put(Obj.put(Obj.put(Obj.put(J.Json.obj([J.KV{"shown", Render.web.materialized(prepared, window, sidebar, signals, plain, text)}]),
        "remoteReady", J.Flag{True{}}), "session", Route.pack(u)), "cmds", J.Json.arr(cmds)), "window", window)

def Render.native_thread( +th: J.Json, +window: J.Json) -> J.Json:
  match th:
    case J.Null{}: J.Null{}
    case +t: Obj.put(t, "earlier", J.Json.nat(J.Json.get_nat(window, "remaining", 0n)))

def Render.native_screen( +screen: J.Json, +window: J.Json) -> J.Json:
  +partial = FA.Folded.screen(MT.Partial.screen(screen, window))
  Obj.put(partial, "thread", Render.native_thread(J.Json.get_obj(partial, "thread"), window))

def Render.display( +web: Bool, +u: U.Ui, +p: J.Json, +now: Nat, +window: J.Json) -> J.Json:
  match web:
    case True{}: J.Json.obj([J.KV{"shown", Render.web(u, window)}])
    case False{}:
      +result = Render.mobile(u, J.Json.get_str(p, "hub", ""), J.Json.get_str(p, "cid", ""), now)
      +overlay = J.Json.get_obj(result, "shown")
      J.Json.obj([J.KV{"shown", Render.native_screen(J.Json.get_obj(result, "screen"), window)},
        J.KV{"overlay", Obj.put(overlay, "j", Render.native_screen(J.Json.get_obj(overlay, "j"), window))}])

def Render.current_live( +same: Bool, +web: Bool, +u: U.Ui, +cmds: List<&2, J.Json>, +st: M.State, +index: T.Index, +p: J.Json, +epoch: String, +now: Nat, +rows: List<&2, String>, +stream: TL.Stream) -> J.Json:
  match same:
    case False{}: J.Json.obj([J.KV{"remoteReady", J.Flag{False{}}}, J.KV{"nextTarget", Target.after(u)}, J.KV{"session", Route.pack(u)}, J.KV{"cmds", J.Json.arr(cmds)}])
    case True{}:
      +window = Render.window(stream, index, Target.thread(st, J.Json.get_obj(p, "target")), p, epoch, rows)
      Obj.put(Obj.put(Obj.put(Obj.put(Render.display(web, Render.ui(u, st, index, J.Json.get_obj(p, "target"), rows, stream, now), p, now, window),
        "remoteReady", J.Flag{True{}}), "session", Route.pack(u)), "cmds", J.Json.arr(cmds)), "window", window)

def Render.current( +same: Bool, +web: Bool, +u: U.Ui, +cmds: List<&2, J.Json>, +st: M.State, +index: T.Index, +p: J.Json, +epoch: String, +now: Nat, +rows: List<&2, String>) -> J.Json:
  Render.current_live(same, web, u, cmds, st, index, p, epoch, now, rows, TL.Stream.blank())

def Render.acted_live( +a: ActResult, +web: Bool, +st: M.State, +index: T.Index, +p: J.Json, +epoch: String, +now: Nat, +rows: List<&2, String>, +stream: TL.Stream) -> J.Json:
  match a:
    case ActResult{+u, cmds}: Render.current_live(W.Target.same(Target.after(u), J.Json.get_obj(p, "target")), web, u, cmds, st, index, p, epoch, now, rows, stream)

def Render.acted( +a: ActResult, +web: Bool, +st: M.State, +index: T.Index, +p: J.Json, +epoch: String, +now: Nat, +rows: List<&2, String>) -> J.Json:
  Render.acted_live(a, web, st, index, p, epoch, now, rows, TL.Stream.blank())

def Render.view(+web: Bool, +st: M.State, +index: T.Index, +info: J.Json, +live: J.Json, +now: Nat, +p: J.Json, +epoch: String, +rows: List<&2, String>) -> J.Json:
  Render.acted(Action.run(web, Session.current(web, st, info, live, now, p), p, now), web, st, index, p, epoch, now, rows)

def Search.row.kind(+e: M.Entry) -> String:
  match e:
    case M.Msg{_, _, _, _, _}: "message"
    case _: "activity"

def Search.row(+r: T.Row) -> J.Json:
  J.Json.obj([J.KV{"id", J.Str{M.Idx.id(T.Row.entry(r))}}, J.KV{"thread", J.Str{M.Idx.thread(T.Row.entry(r))}}, J.KV{"ordinal", J.Str{Nat.show(T.Row.n(r))}},
    J.KV{"kind", J.Str{Search.row.kind(T.Row.entry(r))}}, J.KV{"text", J.Str{String.take(T.Blocks.get(T.Row.blocks(r), 0n), 256n)}}, J.KV{"partial", J.Flag{Nat.is_gt(T.Blocks.n(T.Row.blocks(r)), 1n)}}])

def Search.rows(+ids: List<&2, String>, +index: T.Index) -> List<&2, J.Json>:
  match ids:
    case Nil{}: Nil{}
    case +id <> +tail: Search.row(T.Index.row.k(Map.get(T.Row, T.Row.blank(), T.Index.rows(index), id))) <> Search.rows(tail, index)

def Search.result(+page: T.SearchPage, +index: T.Index, +p: J.Json, +epoch: String) -> J.Json:
  match page:
    case T.SearchPage{+ids, +visited, +last, +more}:
      J.Json.obj([J.KV{"remoteReady", J.Flag{False{}}}, J.KV{"cmds", J.Json.arr(Nil{})}, J.KV{"search", J.Json.obj([
        J.KV{"query", J.Str{J.Json.get_str(p, "query", "")}}, J.KV{"rows", J.Json.arr(Search.rows(ids, index))},
        J.KV{"visited", J.Json.nat(visited)}, J.KV{"hasMore", J.Flag{more}}, J.KV{"cursor", J.Str{Bool.pick(String, more, W.Cursor.write(epoch, J.Json.get_obj(p, "target"), J.Json.get_str(p, "query", ""), Page.number(last)), "")}}])}])

def Search.render(+index: T.Index, +st: M.State, +p: J.Json, +epoch: String, +before: Nat) -> J.Json:
  Search.result(T.Index.search(index, J.Json.get_str(p, "query", ""), Target.thread(st, J.Json.get_obj(p, "target")), before), index, p, epoch)

def Message.result(+row: T.Row, +start: Nat, +p: J.Json, +epoch: String) -> J.Json:
  +n = T.Blocks.n(T.Row.blocks(row))
  +count = Bool.pick(Nat, Nat.is_lt(Nat.sub(n, start), 16n), Nat.sub(n, start), 16n)
  +next = Nat.add(start, count)
  J.Json.obj([J.KV{"remoteReady", J.Flag{False{}}}, J.KV{"cmds", J.Json.arr(Nil{})}, J.KV{"message", J.Json.obj([
    J.KV{"id", J.Str{M.Idx.id(T.Row.entry(row))}}, J.KV{"thread", J.Str{M.Idx.thread(T.Row.entry(row))}}, J.KV{"version", J.Str{Nat.show(T.Row.n(row))}},
    J.KV{"text", J.Str{T.Blocks.join(count, T.Row.blocks(row), start, Nil{})}},
    J.KV{"hasMore", J.Flag{Nat.is_lt(next, n)}}, J.KV{"complete", J.Flag{Nat.is_ge(next, n)}},
    J.KV{"cursor", J.Str{Bool.pick(String, Nat.is_lt(next, n), W.Cursor.write(epoch, J.Json.get_obj(p, "target"), "message:" ++ Nat.show(T.Row.n(row)), next), "")}}])}])

# A chunk is scoped to the selected thread and exact immutable row version.
# Empty cursors may only resolve one of the at-most-32 visible rows. Continued
# chunks use their immutable ordinal, never a full event or transcript scan.
type MessageRead is Data:
  MessageRead{code: String, row: T.Row, start: Nat}

def Message.owns(+row: T.Row, +thread: String, +id: String) -> Bool:
  Bool.and(Nat.is_gt(T.Row.n(row), 0n), Bool.and(String.eq(M.Idx.id(T.Row.entry(row)), id),
    Bool.or(String.eq(M.Idx.thread(T.Row.entry(row)), thread), String.eq(M.Link.from(M.Idx.id(T.Row.entry(row))), thread))))

def Message.find_next( +hit: Bool, +row: T.Row, +rest: T.Row) -> T.Row:
  match hit:
    case True{}: row
    case False{}: rest

# Carry the found row in the tail walk so no eager branch recursively scans.
def Message.find( +rows: List<&2, String>, +index: T.Index, +thread: String, +id: String, +found: T.Row) -> T.Row:
  match rows:
    case Nil{}: found
    case ordinal <> tail:
      +row = T.Index.row(index, Page.number(ordinal))
      Message.find(tail, index, thread, id, Message.find_next(Bool.and(Nat.is_eq(T.Row.n(found), 0n), Message.owns(row, thread, id)), row, found))

def Message.initial( +valid: Bool, +row: T.Row) -> MessageRead:
  match valid:
    case True{}: MessageRead{"", row, 0n}
    case False{}: MessageRead{"NOT_FOUND", row, 0n}

def Message.offset( +valid: Bool, +row: T.Row, +offset: Nat) -> MessageRead:
  match valid:
    case True{}: MessageRead{"", row, offset}
    case False{}: MessageRead{"STALE_CURSOR", row, 0n}

def Message.continued( +offset: Maybe<&2, Nat>, +row: T.Row, +thread: String, +id: String) -> MessageRead:
  match offset:
    case None{}: MessageRead{"STALE_CURSOR", row, 0n}
    case Some{+at}: Message.offset(Bool.and(Message.owns(row, thread, id), Bool.and(Nat.is_gt(at, 0n), Nat.is_lt(at, T.Blocks.n(T.Row.blocks(row))))), row, at)

def Message.ordinal( +ordinal: Maybe<&2, Nat>, +index: T.Index, +thread: String, +p: J.Json, +epoch: String, +st: M.State) -> MessageRead:
  match ordinal:
    case None{}: MessageRead{"STALE_CURSOR", T.Row.blank(), 0n}
    case Some{+n}: Message.continued(W.Cursor.read(J.Json.get_str(p, "cursor", ""), epoch, J.Json.get_obj(p, "target"), "message:" ++ Nat.show(n)), Message.whole(st, T.Index.row(index, n)), thread, J.Json.get_str(p, "entryId", ""))

def Message.cursor_query( +valid: Bool, +query: String, +index: T.Index, +thread: String, +p: J.Json, +epoch: String, +st: M.State) -> MessageRead:
  match valid:
    case False{}: MessageRead{"STALE_CURSOR", T.Row.blank(), 0n}
    case True{}: Message.ordinal(P.Session.nat(String.drop(query, 8n), P.Session.nat.max()), index, thread, p, epoch, st)

def Message.cursor_short( +valid: Bool, +index: T.Index, +thread: String, +p: J.Json, +epoch: String, +st: M.State) -> MessageRead:
  match valid:
    case False{}: MessageRead{"LIMIT", T.Row.blank(), 0n}
    case True{}:
      +query = J.Json.get_str(Maybe.default(&2, J.Json, J.Json.parse(J.Json.get_str(p, "cursor", "")), J.Null{}), "query", "")
      Message.cursor_query(String.starts_with(query, "message:"), query, index, thread, p, epoch, st)

def Message.read_empty( +empty: Bool, +index: T.Index, +thread: String, +p: J.Json, +epoch: String, +rows: List<&2, String>, +st: M.State) -> MessageRead:
  match empty:
    case True{}:
      +row = Message.whole(st, Message.find(rows, index, thread, J.Json.get_str(p, "entryId", ""), T.Row.blank()))
      Message.initial(Message.owns(row, thread, J.Json.get_str(p, "entryId", "")), row)
    case False{}: Message.cursor_short(W.Wire.within(J.Json.get_str(p, "cursor", ""), 1024n), index, thread, p, epoch, st)

# st: the read model, for a report stored cut short (Message.whole)
def Message.read( +index: T.Index, +thread: String, +p: J.Json, +epoch: String, +rows: List<&2, String>, +st: M.State) -> MessageRead:
  Message.read_empty(String.is_empty(J.Json.get_str(p, "cursor", "")), index, thread, p, epoch, rows, st)

# Query validation never silently drops a >64-codepoint token. Such a query
# cannot be reinterpreted as a less restrictive conjunction of other tokens.
type QueryState is Data:
  QueryState{words: Nat, chars: Nat, valid: Bool}

def Query.letter( +letter: Bool, +words: Nat, +chars: Nat, +valid: Bool) -> QueryState:
  match letter:
    case False{}: QueryState{words, 0n, valid}
    case True{}: QueryState{Nat.add(words, Bool.pick(Nat, Nat.is_eq(chars, 0n), 1n, 0n)), Nat.add(chars, 1n), Bool.and(valid, Bool.and(Nat.is_lt(chars, 64n), Bool.or(Nat.is_gt(chars, 0n), Nat.is_lt(words, 8n))))}

def Query.go( +text: String, +q: QueryState) -> Bool:
  match text q:
    case SNil{} QueryState{words, _, valid}: Bool.and(valid, Nat.is_gt(words, 0n))
    case SCon{Chr{c}, tail} QueryState{words, chars, valid}: Query.go(tail, Query.letter(T.Word.char(c), words, chars, valid))

def Query.valid(+query: String) -> Bool:
  Bool.and(W.Wire.within(query, 256n), Query.go(String.to_lower(query), QueryState{0n, 0n, True{}}))

# Live continuations share the message reader but use an exact process-local
# stream version. A new delta invalidates its old cursor rather than joining
# content from two incarnations of a streaming message.
def LiveMessage.result(+stream: TL.Stream, +start: Nat, +thread: String, +p: J.Json, +epoch: String) -> J.Json:
  +blocks = TL.Stream.blocks(stream)
  +n = T.Blocks.n(blocks)
  +count = Bool.pick(Nat, Nat.is_lt(Nat.sub(n, start), 16n), Nat.sub(n, start), 16n)
  +next = Nat.add(start, count)
  J.Json.obj([J.KV{"remoteReady", J.Flag{False{}}}, J.KV{"cmds", J.Json.arr(Nil{})}, J.KV{"message", J.Json.obj([
    J.KV{"id", J.Str{TL.Stream.id(thread)}}, J.KV{"thread", J.Str{thread}}, J.KV{"version", J.Str{Nat.show(TL.Stream.version(stream))}},
    J.KV{"text", J.Str{T.Blocks.join(count, blocks, start, Nil{})}}, J.KV{"hasMore", J.Flag{Nat.is_lt(next, n)}}, J.KV{"complete", J.Flag{Nat.is_ge(next, n)}},
    J.KV{"cursor", J.Str{Bool.pick(String, Nat.is_lt(next, n), W.Cursor.write(epoch, J.Json.get_obj(p, "target"), "live:" ++ Nat.show(TL.Stream.version(stream)), next), "")}}])}])

def HubCursor.empty( +empty: Bool, +p: J.Json, +epoch: String, +query: String) -> Maybe<&2, Nat>:
  match empty:
    case True{}: Some{0n}
    case False{}: W.Cursor.read(J.Json.get_str(p, "cursor", ""), epoch, J.Json.get_obj(p, "target"), query)

def LiveMessage.start(+stream: TL.Stream, +p: J.Json, +epoch: String) -> Maybe<&2, Nat>:
  HubCursor.empty(String.is_empty(J.Json.get_str(p, "cursor", "")), p, epoch, "live:" ++ Nat.show(TL.Stream.version(stream)))
