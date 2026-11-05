import Base
import ../core/i18n.bend as I
import ../core/json.bend as J
import ../core/model.bend as M
import ../core/presentation.bend as P
import ../core/thin.bend as T
import ../core/thin-live.bend as TL
import ../core/fold.bend as FO
import ../core/message-context.bend as MC
import ../core/lazy-tools.bend as LT
import ../core/thin-wire.bend as W
import ./thin-body.bend as BW
import ../core/hist.bend as HI

# The window of a thread's page (thin-view.bend's Render.window.context and what it needs), as its own module: pure,
# no web and no hub imports, so the native presenter can import it. The defs are copies of thin-view.bend's, same
# names and bodies; test/gpui/thin_window_test.bend states that both give the same JSON (the split is behaviour
# preserving), and thin-view.bend still defines its own for the JS hub. Retire the copies when the JS hub goes.

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

def Partial.one(+partial: Bool, +row: T.Row, +cursor: String) -> J.Json:
  match partial:
    case False{}: J.Null{}
    case True{}: J.Json.obj([J.KV{"id", J.Str{M.Idx.id(T.Row.entry(row))}}, J.KV{"version", J.Str{Nat.show(T.Row.n(row))}}, J.KV{"cursor", J.Str{cursor}}, J.KV{"hasMore", J.Flag{True{}}}, J.KV{"complete", J.Flag{False{}}}, J.KV{"reason", J.Str{"byte-limit"}}])

def Partial.include(+part: J.Json, +acc: List<&2, J.Json>) -> List<&2, J.Json>:
  match part:
    case J.Null{}: acc
    case +p: p <> acc

def Partial.row.body(large: Bool, +r: T.Row, epoch: String, target: J.Json, id: String) -> J.Json:
  match large:
    case True{}: BW.Body.part(r, epoch, target)
    case False{}: Partial.one(T.Row.partial(r), r, W.Cursor.write(epoch, target, "message:" ++ id, 1n))

def Partial.rows.body(+enabled: Bool, +p: J.Json, +ids: List<&2, String>, +index: T.Index, +epoch: String, +target: J.Json, +acc: List<&2, J.Json>) -> List<&2, J.Json>:
  match ids:
    case Nil{}: List.reverse(&2, J.Json, acc)
    case +id <> +tail:
      +r = T.Index.row.k(Map.get(T.Row, T.Row.blank(), T.Index.rows(index), id))
      +part = Partial.row.body(Bool.and(enabled, BW.Body.large(r, p)), r, epoch, target, id)
      Partial.rows.body(enabled, p, tail, index, epoch, target, Partial.include(part, acc))

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

# the cursor of the page before the one ids ends at ("" when there is none)

def Page.older.cursor(+index: T.Index, +th: String, +p: J.Json, +epoch: String, +ids: List<&2, String>) -> String:
  +last = Page.last(ids, "")
  +older = Page.older(Bool.not(List.is_empty(&2, String, ids)), index, th, Page.number(last))
  Bool.pick(String, List.is_empty(&2, String, older), "", W.Cursor.write(epoch, J.Json.get_obj(p, "target"), "", Page.number(Page.mid(ids, last))))

def Page.first(+ids: List<&2, String>) -> String:
  match ids:
    case Nil{}: ""
    case +id <> _: id

# "remaining" (older): the entries older than the window's oldest row (its
# last id) and the history below the index; "newer": those newer than its
# newest row (its first id), 0 on the newest page; "total": the thread's,
# older + the window's ids + newer. All in index rows, window.ids' unit
# (a tool call is a row of its own, folded on the page or not), so a page
# can size the history it has not loaded (thin-host.js's scrollbar).

def Page.newer(+empty: Bool, +index: T.Index, +th: String, +ids: List<&2, String>) -> Nat:
  Bool.pick(Nat, empty, 0n, T.Index.newer(index, th, Page.number(Page.first(ids))))

# Future presentation fitting is not known until that page is rendered.
# Serialize reads through each accepted window's actual older cursor; an
# index-only 32-row prediction could skip entries after an 8/2/1-row fit.

def Page.window(+index: T.Index, +th: String, +p: J.Json, +epoch: String, +ids: List<&2, String>) -> J.Json:
  +last = Page.last(ids, "")
  +older = Page.older(Bool.not(List.is_empty(&2, String, ids)), index, th, Page.number(last))
  J.Json.obj([J.KV{"ids", J.Json.arr(Ids.json(ids))}, J.KV{"anchor", Anchor.safe(J.Json.get_obj(p, "anchor"))}, J.KV{"older", J.Str{Page.older.cursor(index, th, p, epoch, ids)}}, J.KV{"olderNext", J.Str{""}}, J.KV{"hasMore", J.Flag{Bool.not(List.is_empty(&2, String, older))}}, J.KV{"remaining", J.Json.nat(Bool.pick(Nat, List.is_empty(&2, String, ids), 0n, T.Index.remaining(index, th, Page.number(last))))}, J.KV{"newer", J.Json.nat(Page.newer(List.is_empty(&2, String, ids), index, th, ids))}, J.KV{"total", J.Json.nat(Nat.add(Nat.add(Bool.pick(Nat, List.is_empty(&2, String, ids), 0n, T.Index.remaining(index, th, Page.number(last))), List.length(&2, String, ids)), Page.newer(List.is_empty(&2, String, ids), index, th, ids)))}, J.KV{"partial", J.Json.arr(Partial.rows.body(Nat.is_eq(J.Json.get_nat(p, "bodyPages", 0n), 1n), p, ids, index, epoch, J.Json.get_obj(p, "target"), Nil{}))}])

def Window.live(+window: J.Json, +parts: List<&2, J.Json>) -> J.Json:
  Obj.put(window, "partial", J.Json.arr(List.append(&2, J.Json, J.Json.elems(J.Json.get_obj(window, "partial")), parts)))

def Partial.live.body(on: Bool, +s: TL.Stream, +thread: String, +epoch: String, +target: J.Json) -> List<&2, J.Json>:
  Nil{}

def Render.window.marker(on: Bool, window: J.Json) -> J.Json:
  match on:
    case False{}: window
    case True{}: Obj.put(window, "bodyPages", J.Flag{True{}})

def Render.window(+s: TL.Stream, +index: T.Index, +thread: String, +p: J.Json, +epoch: String, +rows: List<&2, String>) -> J.Json:
  +on = Nat.is_eq(J.Json.get_nat(p, "bodyPages", 0n), 1n)
  +window = Window.live(Page.window(index, thread, p, epoch, rows), Partial.live.body(on, s, thread, epoch, J.Json.get_obj(p, "target")))
  Render.window.marker(on, window)

# Keep wire cursors/counts in source rows. Retention and initial message
# context use the identities actually displayed after folding instead.

def Render.window.context(+st: M.State, +s: TL.Stream, +index: T.Index, +thread: String, +p: J.Json, +epoch: String, +rows: List<&2, String>) -> J.Json:
  +es = HI.Hist.seen(LT.Tools.entries(rows, index, Nil{}))
  +around = LT.Tools.context(FO.Fold.context(Nil{}, M.State.mine(st, thread), Nil{}, FO.Fold.shown(es)))
  +roles = MC.Context.roles(es, FO.Fold.roles.all(I.En{}, around, es, LT.Tools.info(p), False{}), st, thread, LT.Tools.info(p))
  Obj.put(Obj.put(Obj.put(Render.window(s, index, thread, p, epoch, rows),
    "historyUnits", J.Json.arr(Ids.json(MC.Context.units.stored(es, roles, MC.Context.roots(M.State.mine(st, MC.Context.runs.key(thread)), Map.new(&2, String)))))),
    "messageIds", J.Json.arr(Ids.json(MC.Context.messages(es)))),
    "contextMessages", J.Json.arr(Ids.json(MC.Context.targets(st, thread))))

# Away from the end (host.js's hold), the page says what came after the
# newest entry the reader had then ("@away.<thread>", sent by the page):
# client.bend's "N new ↓" (Hold.label), counted over the whole thread, not
# the page on show, a reply streaming in counting as new; "" for nothing

