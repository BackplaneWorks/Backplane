import Base
import ../core/json.bend as J
import ../core/thin.bend as T
import ../core/thin-live.bend as TL
import ../core/i18n.bend as I
import ../web/dom.bend as D
import ../mobile/screen.bend as SC
import ../mobile/thin.bend as MT
import ./thin-view.bend as V
import ./thin-body.bend as BW

# A thin view's reply when only its thread's live row changed (fast hub R1).
# Streamed deltas come 20 a second per busy thread; re-rendering the whole
# page (sidebar, mobile screen) for each was most of the hub's CPU on a real
# log. Between full renders (Hub.thin.render's memo: at most 1 s apart, and
# anything but the live row starts a full one) the last reply is kept and
# only the live row is written again, exactly as the renderers write it:
# - web: the element `bp-memo` keyed by the live entry's id (View.memo.loc):
#   its text kid is the new preview, its attributes (stamp, lang) kept;
# - mobile: the entry with the live id (Mob.own.s.loc for an assistant
#   message): text and blocks (Mob.nodes of D.Md.render.loc) new, images and
#   stamp kept, its "partial" marker from the new window (MT.Partial.entry_if),
#   in `shown` and in `overlay.j`;
# - the reply's `window` is the new one; every field keeps its place.
# test/thin_splice_test.bend holds a spliced reply equal to a full render of
# the same inputs at the same clock. Anything unexpected (the live row not
# found exactly once) answers None: the caller renders in full.

# key's value replaced where it stands (Obj.put moves a key to the front)
def Jx.hit(+fs: J.Json, +key: String) -> Bool:
  match fs:
    case J.Field{+k, _, _}:
      String.eq(k, key)
    case _:
      False{}

def Jx.set.go(fs: J.Json, hit: Bool, +key: String, +v: J.Json) -> J.Json:
  match fs hit:
    case J.Field{_, _, +tail} True{}:
      J.Field{key, v, tail}
    case J.Field{+k, val, +tail} False{}:
      J.Field{k, val, Jx.set.go(tail, Jx.hit(tail, key), key, v)}
    case _ _:
      fs

def Jx.set(+j: J.Json, +key: String, +v: J.Json) -> J.Json:
  match j:
    case J.Obj{+fs}:
      J.Obj{Jx.set.go(fs, Jx.hit(fs, key), key, v)}
    case _:
      j

# The live node
# -------------

# web: the live memo; mobile: the live assistant entry
def Live.is.k(+web: Bool, +o: J.Json, +id: String) -> Bool:
  match web:
    case True{}:
      Bool.and(String.eq(J.Json.get_str(o, "tag", ""), "bp-memo"), String.eq(J.Json.get_str(o, "key", ""), id))
    case False{}:
      Bool.and(String.eq(J.Json.get_str(o, "id", ""), id), String.eq(J.Json.get_str(o, "kind", ""), "assistant"))

def Live.is(+j: J.Json, +web: Bool, +id: String) -> Bool:
  match j:
    case J.Obj{+fs}:
      Live.is.k(web, J.Obj{fs}, id)
    case _:
      False{}

# how many live nodes a tree holds (hit: whether j is one)
def Live.count(j: J.Json, hit: Bool, +web: Bool, +id: String, +n: Nat) -> Nat:
  match j hit:
    case _ True{}:
      Nat.add(n, 1n)
    case J.Obj{+fs} False{}:
      Live.count(fs, False{}, web, id, n)
    case J.Arr{+xs} False{}:
      Live.count(xs, False{}, web, id, n)
    case J.Item{+h, +t} False{}:
      Live.count(t, False{}, web, id, Live.count(h, Live.is(h, web, id), web, id, n))
    case J.Field{_, +v, +t} False{}:
      Live.count(t, False{}, web, id, Live.count(v, Live.is(v, web, id), web, id, n))
    case _ False{}:
      n

def Live.one(+j: J.Json, +web: Bool, +id: String) -> Bool:
  Nat.is_eq(Live.count(j, Live.is(j, web, id), web, id, 0n), 1n)

def Live.web(+old: J.Json, +id: String, +text: String) -> J.Json:
  J.Json.obj([J.KV{"type", J.Str{"element"}}, J.KV{"tag", J.Str{"bp-memo"}}, J.KV{"key", J.Str{id}},
    J.KV{"attrs", J.Json.get_obj(old, "attrs")}, J.KV{"kids", J.Json.arr([J.Json.obj([J.KV{"type", J.Str{"text"}}, J.KV{"text", J.Str{text}}])])}])

def Live.mobile(+old: J.Json, +id: String, +text: String, +lc: I.Loc, +part: J.Json) -> J.Json:
  MT.Partial.entry_if(part, J.Json.obj([J.KV{"id", J.Json.str(id)}, J.KV{"kind", J.Json.str("assistant")}, J.KV{"text", J.Json.str(text)},
    J.KV{"blocks", J.Json.arr(SC.Mob.nodes(D.Md.render.loc(lc, text)))}, J.KV{"images", J.Json.get_obj(old, "images")}, J.KV{"stamp", J.Json.get_obj(old, "stamp")}]))

def Live.node(+web: Bool, +old: J.Json, +id: String, +text: String, +lc: I.Loc, +part: J.Json) -> J.Json:
  match web:
    case True{}:
      Live.web(old, id, text)
    case False{}:
      Live.mobile(old, id, text, lc, part)

# the tree with its live node written again (hit: whether j is it)
def Live.put(j: J.Json, hit: Bool, +web: Bool, +id: String, +text: String, +lc: I.Loc, +part: J.Json) -> J.Json:
  match j hit:
    case +o True{}:
      Live.node(web, o, id, text, lc, part)
    case J.Obj{+fs} False{}:
      J.Obj{Live.put(fs, False{}, web, id, text, lc, part)}
    case J.Arr{+xs} False{}:
      J.Arr{Live.put(xs, False{}, web, id, text, lc, part)}
    case J.Item{+h, +t} False{}:
      J.Item{Live.put(h, Live.is(h, web, id), web, id, text, lc, part), Live.put(t, False{}, web, id, text, lc, part)}
    case J.Field{+k, +v, +t} False{}:
      J.Field{k, Live.put(v, Live.is(v, web, id), web, id, text, lc, part), Live.put(t, False{}, web, id, text, lc, part)}
    case +o False{}:
      o

def Live.tree(+j: J.Json, +web: Bool, +id: String, +text: String, +lc: I.Loc, +part: J.Json) -> J.Json:
  Live.put(j, Live.is(j, web, id), web, id, text, lc, part)

# The reply
# ---------

# whether the live row's chunk marker is on (thin-view.bend's Partial.live)
def Splice.partial(+s: TL.Stream) -> Bool:
  match s:
    case TL.Stream{_, size, blocks, cut, _}:
      Bool.and(Nat.is_gt(size, 0n), Bool.or(cut, Nat.is_gt(T.Blocks.n(blocks), 16n)))

def Splice.part(+window: J.Json, +id: String) -> J.Json:
  MT.Partial.find(J.Json.elems(J.Json.get_obj(window, "partial")), id)

def Splice.mobile.ok(+res: J.Json, +id: String) -> Bool:
  Bool.and(Live.one(J.Json.get_obj(res, "shown"), False{}, id), Live.one(J.Json.get_obj(J.Json.get_obj(res, "overlay"), "j"), False{}, id))

def Splice.ok.k(+web: Bool, +res: J.Json, +id: String) -> Bool:
  match web:
    case True{}:
      Live.one(J.Json.get_obj(res, "shown"), True{}, id)
    case False{}:
      Splice.mobile.ok(res, id)

def Splice.mobile.go(+res: J.Json, +window: J.Json, +id: String, +text: String, +lc: I.Loc) -> J.Json:
  +part = Splice.part(window, id)
  Jx.set(Jx.set(Jx.set(res, "window", window), "shown", Live.tree(J.Json.get_obj(res, "shown"), False{}, id, text, lc, part)),
    "overlay", Jx.set(J.Json.get_obj(res, "overlay"), "j", Live.tree(J.Json.get_obj(J.Json.get_obj(res, "overlay"), "j"), False{}, id, text, lc, part)))

def Splice.go(+web: Bool, +res: J.Json, +window: J.Json, +id: String, +text: String, +lc: I.Loc) -> J.Json:
  match web:
    case True{}:
      Jx.set(Jx.set(res, "window", window), "shown", Live.tree(J.Json.get_obj(res, "shown"), True{}, id, text, lc, J.Null{}))
    case False{}:
      Splice.mobile.go(res, window, id, text, lc)

def Splice.k(+ok: Bool, +web: Bool, +res: J.Json, +window: J.Json, +id: String, +text: String, +lc: I.Loc) -> Maybe<&2, J.Json>:
  match ok:
    case True{}:
      Some{Splice.go(web, res, window, id, text, lc)}
    case False{}:
      None{}

# The window: its page part is the read model's (unchanged while the memo's
# key is), its last chunk marker the live row's (V.Render.window =
# V.Window.live of V.Page.window): the old live marker out, the new in.
def Splice.notlive(xs: List<&2, J.Json>, +id: String) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> t:
      List.append(&2, J.Json, Bool.pick(List<&2, J.Json>, String.eq(J.Json.get_str(x, "id", ""), id), Nil{}, [x]), Splice.notlive(t, id))

def Splice.window(+old: J.Json, +stream: TL.Stream, +thread: String, +epoch: String, +target: J.Json) -> J.Json:
  Jx.set(old, "partial", J.Json.arr(List.append(&2, J.Json, Splice.notlive(J.Json.elems(J.Json.get_obj(old, "partial")), TL.Stream.id(thread)),
    V.Partial.live.body(J.Json.get_bool(old, "bodyPages", False{}), stream, thread, epoch, target))))

# the reply res (a full render's, whose stream had the same chunk marker)
# with stream's live row and its window: None when its live row is not
# found exactly once where the renderer puts it
def Splice.reply(+web: Bool, +res: J.Json, +stream: TL.Stream, +thread: String, +epoch: String, +target: J.Json, +lc: I.Loc) -> Maybe<&2, J.Json>:
  Splice.k(Splice.ok.k(web, res, TL.Stream.id(thread)), web, res, Splice.window(J.Json.get_obj(res, "window"), stream, thread, epoch, target), TL.Stream.id(thread), BW.Body.live.preview(Bool.and(web, J.Json.get_bool(J.Json.get_obj(res, "window"), "bodyPages", False{})), stream), lc)
