import Base
import ../core/json.bend as J
import ../core/model.bend as M
import ../core/codec.bend as C
import ../core/client.bend as U
import ../core/vt.bend as VT
import ./nui.bend as N
import ./layout.bend as L
import ./side_rows.bend as SR
import ./side_row_types.bend as ST
import ../core/emb.bend as EM
import ../core/subs.bend as SB

# The main area's memo. A frame lays the main area (the open thread, its
# Subagents panel, composer...) out again only when something it shows may
# have changed; otherwise it takes the last frame's. With agents streaming
# in other threads most frames changed nothing there.
#
# What may have changed is told by touches: each hub message counts one,
# and every change it folds marks the threads it touches (client.bend's
# Touch.of, which names every kind of change; anything not clearly one
# thread's touches all). Any other message (a log, another machine's
# state, the person's input) touches all. The key is the open thread's,
# its parent's and its children's last touches, the last touch of all, and
# every field of the window the main area reads (Mm.key). Equal keys, the
# same main area (test/native/memo_replay.bend checks it on a real log).

# n: messages counted; g: the last that touched all; r: the last that may
# have changed which threads are whose parent or child otherwise than by
# touching both (a setting, a state handed over: client.bend's Touch.of
# names it "!"); m: each thread's last
# lg: the threads touched, newest first, with the message that touched
# them (only the index reads it: MM.Ix.up takes those since its own count,
# and Shell.paint.go consumes it once the index has; every reader supplies
# its own cursor to TouchK.consumed, which keeps it until all have read)
type TkE is Data:
  TkE{id: String, at: Nat}

type TouchK is Data:
  TouchK{n: Nat, g: Nat, r: Nat, m: Map<&2, Nat>, lg: List<&2, TkE>}

def TouchK.none() -> TouchK:
  TouchK{0n, 0n, 0n, Map.new(&2, Nat), Nil{}}

def TouchK.of(ks: List<&2, TouchK>) -> TouchK:
  match ks:
    case Nil{}:
      TouchK.none()
    case k <> _:
      k

def TouchK.tick(t: TouchK) -> TouchK:
  match t:
    case TouchK{n, g, r, m, l}:
      TouchK{1n+n, g, r, m, l}

# The timeline/main area still need every global touch. The sidebar index
# separately tracks global changes to its inputs: a rename only replaces
# one record, while linked titles elsewhere still invalidate globally.
# Keep this internal stamp in the existing reserved map, not a new native
# record field; old/manual TouchK values fall back conservatively to g.
def TouchK.index.value(r: Map<&2, Nat> & Nat) -> Nat:
  (_, v) = r
  v

def TouchK.index_g(t: TouchK) -> Nat:
  match t:
    case TouchK{_, g, _, m, _}:
      TouchK.index.value(Map.get(Nat, g, m, "#ix.g"))

def TouchK.all(t: TouchK) -> TouchK:
  match t:
    case TouchK{+n, _, _, m, l}:
      TouchK{1n+n, 1n+n, 1n+n, Map.set(&2, Nat, m, "#ix.g", 1n+n), l}

def TouchK.one.k(i: Nat, +id: String, t: TouchK) -> TouchK:
  match i t:
    case 0n TouchK{+n, _, r, m, l}:
      TouchK{n, n, r, Map.set(&2, Nat, m, "#ix.g", n), l}
    case 1n TouchK{+n, g, _, m, l}:
      TouchK{n, g, n, m, l}
    case _ TouchK{+n, g, r, m, l}:
      TouchK{n, g, r, Map.set(&2, Nat, m, id, n), TkE{id, n} <> l}

def TouchK.ids(ids: List<&2, String>, t: TouchK) -> TouchK:
  match ids:
    case Nil{}:
      t
    case +id <> rest:
      TouchK.ids(rest, TouchK.one.k(C.Codec.pos(["*", "!"], id), id, t))

# The sidebar's stamp ("#side": the last message whose changes it may
# show). A tool step logged in a thread (ActivityLogged) adds only an
# entry to that thread's log (model.bend's State.post), and the sidebar
# reads no thread's entries, only its record, settings and the index; a
# subagent's log ("@sub:<thread>") is read for its row's badge, so its
# steps count. Every other change counts (memo_replay checks each kept
# sidebar against a fresh one at every event).
def Sm.blind(c: M.Change) -> Bool:
  match c:
    case M.ActivityLogged{+thread, _, _, _, _, _}:
      Bool.not(String.starts_with(thread, "@"))
    case _:
      False{}

def TouchK.side.k(blind: Bool, t: TouchK) -> TouchK:
  match blind t:
    case True{} _:
      t
    case False{} TouchK{+n, g, r, m, l}:
      TouchK{n, g, r, Map.set(&2, Nat, m, "#side", n), l}

def TouchK.rename.with(+id: String, t: TouchK, ig: Nat) -> TouchK:
  match t:
    case TouchK{+n, _, r, m, l}:
      TouchK{n, n, r, Map.set(&2, Nat, Map.set(&2, Nat, m, "#ix.g", ig), id, n), TkE{id, n} <> l}

def TouchK.rename(+id: String, +t: TouchK) -> TouchK:
  TouchK.rename.with(id, t, TouchK.index_g(t))

def TouchK.change.known(+c: M.Change, t: TouchK) -> TouchK:
  match c:
    case M.ThreadRenamed{id, _, _}:
      TouchK.side.k(False{}, TouchK.rename(id, t))
    case _:
      TouchK.side.k(Sm.blind(c), TouchK.ids(U.Touch.of(c), t))

# a change folded (one the client cannot read touches all)
def TouchK.change(m: Maybe<&2, M.Change>, t: TouchK) -> TouchK:
  match m:
    case Some{+c}:
      TouchK.change.known(c, t)
    case None{}:
      TouchK.ids(["*", "!"], t)

def TouchK.items(xs: List<&2, J.Json>, t: TouchK) -> TouchK:
  match xs:
    case Nil{}:
      t
    case x <> rest:
      TouchK.items(rest, TouchK.change(C.Change.decode(x), t))

# a thread's streamed text grew: its count of pieces ("~<thread>")
def TouchK.live.k(r: Map<&2, Nat> & Nat, +th: String, +n: Nat, +g: Nat, +rr: Nat, l: List<&2, TkE>) -> TouchK:
  (m, v) = r
  TouchK{n, g, rr, Map.set(&2, Nat, m, "~" ++ th, 1n+v), l}

def TouchK.live(+th: String, t: TouchK) -> TouchK:
  match t:
    case TouchK{+n, +g, +r, m, l}:
      TouchK.live.k(Map.get(Nat, 0n, m, "~" ++ th), th, n, g, r, l)

def TouchK.msg.k(i: Nat, +o: J.Json, t: TouchK) -> TouchK:
  match i:
    case 0n:
      TouchK.items(J.Json.elems(J.Json.get_obj(o, "items")), TouchK.tick(t))
    case 1n:
      TouchK.live(J.Json.get_str(o, "thread", ""), t)
    case 2n:
      t
    case 3n:
      t
    case _:
      TouchK.all(t)

# a hub message: its changes touch what they touch; a thread's streamed
# text counts one more piece of it (TouchK.live: the open thread's count
# is in the key); its end (which the client does not read) and a heartbeat
# touch nothing; anything else touches all
def TouchK.msg(+o: J.Json, t: TouchK) -> TouchK:
  TouchK.msg.k(C.Codec.pos(["changes", "delta", "hb", "delta_end"], J.Json.get_str(o, "t", "")), o, t)

def TouchK.at.k(r: Map<&2, Nat> & Nat) -> Nat:
  (_, v) = r
  v

def TouchK.at(+t: TouchK, +id: String) -> Nat:
  match t:
    case TouchK{_, _, _, +m, _}:
      TouchK.at.k(Map.get(Nat, 0n, m, id))

def TouchK.m(t: TouchK) -> Map<&2, Nat>:
  match t:
    case TouchK{_, _, _, m, _}:
      m

def TouchK.lg(t: TouchK) -> List<&2, TkE>:
  match t:
    case TouchK{_, _, _, _, l}:
      l

# the log emptied only after every reader has caught up (TouchK.consumed)
def TouchK.clear(t: TouchK) -> TouchK:
  match t:
    case TouchK{n, g, r, m, _}:
      TouchK{n, g, r, m, Nil{}}

# Every log reader must supply its cursor. A lagging reader keeps the log
# whole; an empty reader list keeps it too. The window currently has one,
# IxK, and consumes only after that index has been forced.
def TouchK.caught(ns: List<&2, Nat>, +at: Nat, ok: Bool) -> Bool:
  match ns:
    case Nil{}:
      ok
    case n <> rest:
      TouchK.caught(rest, at, Bool.and(ok, Nat.is_ge(n, at)))

def TouchK.consumed.k(caught: Bool, t: TouchK) -> TouchK:
  match caught:
    case True{}:
      TouchK.clear(t)
    case False{}:
      t

def TouchK.consumed(+t: TouchK, +readers: List<&2, Nat>) -> TouchK:
  match t:
    case TouchK{at, _, _, _, _}:
      TouchK.consumed.k(TouchK.caught(readers, at, Bool.not(List.is_empty(&2, Nat, readers))), t)

def TouchK.g(+t: TouchK) -> Nat:
  match t:
    case TouchK{_, g, _, _, _}:
      g

def TouchK.r(+t: TouchK) -> Nat:
  match t:
    case TouchK{_, _, r, _, _}:
      r

# The key
# -------

def Mm.children(ts: List<&2, M.Task>) -> List<&2, String>:
  match ts:
    case Nil{}:
      Nil{}
    case M.Task{_, _, +c, _, _, _, _, _, _} <> t:
      c <> Mm.children(t)

def Mm.stamps(ids: List<&2, String>, +t: TouchK) -> String:
  match ids:
    case Nil{}:
      ""
    case +id <> rest:
      id ++ "=" ++ Nat.show(TouchK.at(t, id)) ++ "," ++ Mm.stamps(rest, t)

def Mm.strs(xs: List<&2, Sigma<&2, &2, String, _ => String>>) -> String:
  match xs:
    case Nil{}:
      ""
    case (+k, +v) <> t:
      k ++ "\u{1f}" ++ v ++ "\u{1e}" ++ Mm.strs(t)

def Mm.flags(xs: List<&2, Sigma<&2, &2, String, _ => Bool>>) -> String:
  match xs:
    case Nil{}:
      ""
    case (+k, +v) <> t:
      k ++ Bool.pick(String, v, "+", "-") ++ Mm.flags(t)

# info the main area never reads, or reads only as part of the key
# otherwise: the clock (now is in the key), the caret's timing (its phase,
# caret.on, is kept), the cats', the frame's own bookkeeping, the hover
# and the performance overlay (both drawn over the whole frame), a page's
# frames (shown only in the viewer and a bot's view: neither is kept)
def Mm.volatile(+k: String) -> Bool:
  Bool.or(Nat.is_lt(C.Codec.pos(["clock", "act.at", "cat.due", "cat.clock", "caret.due", "caret.at", "caret.sig", "frame.hub", "frame.late", "hover", "perf.hud", "caret.real"], k), 12n),
    Bool.or(String.starts_with(k, "img:shot:"), String.starts_with(k, "shot.")))

def Mm.info.k(skip: Bool, +k: String, +v: String, +rest: String) -> String:
  match skip:
    case True{}:
      rest
    case False{}:
      k ++ "\u{1f}" ++ v ++ "\u{1e}" ++ rest

def Mm.info(xs: List<&2, Sigma<&2, &2, String, _ => String>>) -> String:
  match xs:
    case Nil{}:
      ""
    case (+k, +v) <> t:
      Mm.info.k(Mm.volatile(k), k, v, Mm.info(t))

def Mm.hit(h: N.Hit) -> String:
  match h:
    case N.Hit{x0, y0, x1, y1, a, v}:
      U32.show(x0) ++ "," ++ U32.show(y0) ++ "," ++ U32.show(x1) ++ "," ++ U32.show(y1) ++ "," ++ a ++ "," ++ v

def Mm.b(b: Bool) -> String:
  Bool.pick(String, b, "1", "0")

# the main area's right edge (Lay.frame.u's: half the window with the
# viewer open)
def Mm.x1(+pa: Bool, +w: U32) -> U32:
  Bool.pick(U32, pa, (264 + U32.shr((w - 264 : U32)) : U32), w)

def Mm.x1.of(+n: N.Nui) -> U32:
  Mm.x1(U.Ui.panel(N.Nui.ui(n)), N.Nui.width(n))

# The clock in the main area is not in the key: what it moves is found
# once, when the main area is laid out, as the first time anything there
# would be drawn differently by the clock alone (Mm.due), and the memo is
# used only before then. That is the next minute (the timeline's dates,
# anything shown to the minute), the day after which a child's, a
# launch's or a run's row hides (core/subs.bend's Subs.old), the end of
# the thread's snooze (the header), and when a terminal in the chat goes
# inactive (core/emb.bend's Emb.live). (Before, the key held the second
# for every thread: the parent's place, "" for none, counted as a child,
# so each tick missed.)

# the earlier of two times (0: none)
def Mm.first(+a: Nat, +b: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_eq(a, 0n), b, Bool.pick(Nat, Nat.is_eq(b, 0n), a, Bool.pick(Nat, Nat.is_lt(a, b), a, b)))

# a time still ahead of now, else none
def Mm.ahead(+at: Nat, +now: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_gt(at, now), at, 0n)

# a row of one that ended at `at` (0: unknown, never hidden) hides once
# now - at > Subs.age()
def Mm.hides(+at: Nat, +now: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_eq(at, 0n), 0n, Mm.ahead(Nat.add(Nat.add(at, SB.Subs.age()), 1n), now))

def Mm.due.ends(rs: List<&2, String>, +st: M.State, +now: Nat) -> Nat:
  match rs:
    case Nil{}:
      0n
    case +r <> t:
      Mm.first(Mm.hides(SB.Subs.end(st, r), now), Mm.due.ends(t, st, now))

def Mm.due.aggs(as: List<&2, SB.SubAgg>, +now: Nat) -> Nat:
  match as:
    case Nil{}:
      0n
    case SB.SubAgg{_, _, _, _, _, _, +at} <> t:
      Mm.first(Mm.hides(at, now), Mm.due.aggs(t, now))

def Mm.due.term(term: Bool, e: M.Entry, +st: M.State, +now: Nat) -> Nat:
  match term e:
    case True{} M.Act{+id, +th, _, _, _, +at}:
      Mm.ahead(Nat.add(EM.Emb.touched(st, EM.Emb.key(th, id), at), EM.Emb.idle()), now)
    case _ _:
      0n

def Mm.due.terms(es: List<&2, M.Entry>, +st: M.State, +now: Nat) -> Nat:
  match es:
    case Nil{}:
      0n
    case +e <> t:
      Mm.first(Mm.due.term(EM.Emb.is(e), e, st, now), Mm.due.terms(t, st, now))

def Mm.memo.its(m: L.TLMemo) -> List<&2, L.TLE>:
  match m:
    case L.TLMemo{_, _, its, _}:
      its

# the first time the main area would be drawn differently by the clock
# alone, after now (never 0: the next minute bounds it)
def Mm.due(+n: N.Nui, +rels: List<&2, String>, +memo: L.TLMemo) -> Nat:
  match n:
    case N.Nui{U.Ui{+st, +sel, _, _, _, _, +now, _, _, _, _, _, _, _, _, _, _}, _, _, _, _, _, _, _, _, _, _, _, _}:
      Mm.first(Mm.first(Nat.mul(Nat.add(Nat.div(now, 60n), 1n), 60n), Mm.ahead(U.Side.until(st, sel), now)),
        Mm.first(Mm.first(Mm.due.ends(rels, st, now), Mm.due.aggs(SB.Subs.aggs(st, sel), now)), Mm.due.terms(L.TL.memo.es(Mm.memo.its(memo)), st, now)))

# the main area is memoized only for a thread's view: not the settings,
# not the empty window, not with a terminal anywhere (embedded screens read
# the terminals' state, which no touch follows)
def Mm.memo.ok(+op: Map<&2, Bool>, +sel: String, +tr: Map<&2, VT.Vt>) -> Bool:
  Bool.and(Bool.and(Bool.not(U.Flag.get(op, "settings")), Bool.not(String.is_empty(sel))), List.is_empty(&2, String, Map.keys(&2, VT.Vt, tr)))

def Mm.key.go(+n: N.Nui, +t: TouchK, +rels: List<&2, String>) -> String:
  match n:
    case N.Nui{U.Ui{_, +sel, +dr, _, +op, on, _, ad, pa, v, rid, w, er, note, +nf, +ob, _}, +ww, +hh, _, _, _, sc, ca, fo, path, +hi, _, _}:
      "s" ++ Mm.stamps(sel <> rels, t) ++ "|lang" ++ L.TL.locale(N.Nui.ui(n))
        ++ "|g" ++ Nat.show(TouchK.g(t)) ++ "|w" ++ U32.show(ww) ++ "," ++ U32.show(hh) ++ "," ++ U32.show(sc) ++ "," ++ Nat.show(ca) ++ "," ++ fo ++ "," ++ path
        ++ "|x" ++ U32.show(Mm.x1(pa, ww)) ++ "|cs" ++ N.Cs.region(hi) ++ "|ch" ++ Mm.hit(L.Comp.hit(hi))
        ++ "|u" ++ Mm.b(on) ++ Mm.b(ad) ++ Mm.b(pa) ++ Mm.b(w) ++ "," ++ v ++ "," ++ Nat.show(rid) ++ "," ++ er ++ "," ++ note
        ++ "|f" ++ Mm.flags(Map.to_list(&2, Bool, op)) ++ "|d" ++ Mm.strs(Map.to_list(&2, String, dr)) ++ "|o" ++ Mm.strs(Map.to_list(&2, String, ob))
        ++ "|l" ++ Nat.show(TouchK.at(t, "~" ++ sel)) ++ "|i" ++ Mm.info(Map.to_list(&2, String, nf)) ++ "|m" ++ U.Ui.scratch(N.Nui.ui(n), "@ime")

def Mm.key.k(ok: Bool, +n: N.Nui, +t: TouchK, +rels: List<&2, String>) -> String:
  match ok:
    case False{}:
      ""
    case True{}:
      Mm.key.go(n, t, rels)

# the main area's key ("": not memoized); rels: the open thread's parent
# and children (Mm.rels)
def Mm.key(+n: N.Nui, +t: TouchK, +rels: List<&2, String>) -> String:
  match n:
    case N.Nui{U.Ui{_, +sel, _, _, +op, _, _, _, _, _, _, _, _, _, _, _, +tr}, _, _, _, _, _, _, _, _, _, _, _, _}:
      Mm.key.k(Mm.memo.ok(op, sel, U.MailvLocal.terms(tr)), n, t, rels)

# The memo
# --------
# key: the main area's; rk: the open thread with its own last touch and
# the last change to whose parent or child any thread is otherwise (r):
# a delegation or a launch touches both ends, so rels, found by walking
# every task, is found again only when rk changes; due: the first time
# the clock alone would change it (Mm.due), until which it may be used

type MainM is Data:
  MainM{key: String, rk: String, rels: List<&2, String>, out: L.Out, due: Nat}

def Mm.rk(+n: N.Nui, +t: TouchK) -> String:
  U.Ui.sel(N.Nui.ui(n)) ++ "|" ++ Nat.show(TouchK.at(t, U.Ui.sel(N.Nui.ui(n)))) ++ "|" ++ Nat.show(TouchK.r(t))

def Mm.rels.of(+n: N.Nui) -> List<&2, String>:
  match n:
    case N.Nui{U.Ui{+st, +sel, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}, _, _, _, _, _, _, _, _, _, _, _, _}:
      List.concat(&2, String, [[U.Tasks.up(st, sel)], Mm.children(U.Tasks.of(st, sel)), M.Launch.of(st, sel)])

def Mm.rels.k(same: Bool, ms: List<&2, MainM>, +n: N.Nui) -> List<&2, String>:
  match same ms:
    case True{} MainM{_, _, rs, _, _} <> _:
      rs
    case _ _:
      Mm.rels.of(n)

def MainM.rk.is(+rk: String, ms: List<&2, MainM>) -> Bool:
  match ms:
    case Nil{}:
      False{}
    case MainM{_, +k, _, _, _} <> _:
      String.eq(k, rk)

# the open thread's parent and children, the memo's while rk holds
def Mm.rels(+rk: String, +ms: List<&2, MainM>, +n: N.Nui) -> List<&2, String>:
  Mm.rels.k(MainM.rk.is(rk, ms), ms, n)

def MainM.same(+key: String, ms: List<&2, MainM>) -> Bool:
  match ms:
    case Nil{}:
      False{}
    case MainM{+k, _, _, _, _} <> _:
      Bool.and(Bool.not(String.is_empty(key)), String.eq(k, key))

def MainM.due(ms: List<&2, MainM>) -> Nat:
  match ms:
    case Nil{}:
      0n
    case MainM{_, _, _, _, d} <> _:
      d

# a memo is used only before its due time, and for the same key (law
# mm_late_misses)
def Mm.use.k(late: Bool, +same: Bool) -> Bool:
  match late:
    case True{}:
      False{}
    case False{}:
      same

def Mm.use(+key: String, +ms: List<&2, MainM>, +now: Nat) -> Bool:
  Mm.use.k(Nat.is_ge(now, MainM.due(ms)), MainM.same(key, ms))

def MainM.out(ms: List<&2, MainM>) -> L.Out:
  match ms:
    case Nil{}:
      L.Out.none()
    case MainM{_, _, _, o, _} <> _:
      o

# the main area as Lay.frame.u lays it out (Lay.main.m with none given)
def Mm.lay(+ty: L.Ty, +memo: L.TLMemo, +n: N.Nui) -> L.Out:
  L.Lay.main(ty, memo, L.Lay.shrink(n, L.Lay.term_h(n)), 264, Mm.x1.of(n))

def Mm.main.k(hit: Bool, +key: String, +rk: String, +rels: List<&2, String>, +ms: List<&2, MainM>, +ty: L.Ty, +memo: L.TLMemo, +n: N.Nui) -> MainM:
  match hit:
    case True{}:
      MainM{key, rk, rels, MainM.out(ms), MainM.due(ms)}
    case False{}:
      MainM{key, rk, rels, Mm.lay(ty, memo, n), Mm.due(n, rels, memo)}

def Mm.main.s(+key: String, +rk: String, +rels: List<&2, String>, +ms: List<&2, MainM>, +ty: L.Ty, +memo: L.TLMemo, +n: N.Nui) -> MainM:
  Mm.main.k(Mm.use(key, ms, U.Side.now(N.Nui.ui(n))), key, rk, rels, ms, ty, memo, n)

def Mm.main.r(+rk: String, +rels: List<&2, String>, +ms: List<&2, MainM>, +ty: L.Ty, +memo: L.TLMemo, +n: N.Nui, +t: TouchK) -> MainM:
  Mm.main.s(Mm.key(n, t, rels), rk, rels, ms, ty, memo, n)

def Mm.main.q(+rk: String, +ms: List<&2, MainM>, +ty: L.Ty, +memo: L.TLMemo, +n: N.Nui, +t: TouchK) -> MainM:
  Mm.main.r(rk, Mm.rels(rk, ms, n), ms, ty, memo, n, t)

# the main area: the memo's when its key is this frame's, else laid out
def Mm.main(+ms: List<&2, MainM>, +ty: L.Ty, +memo: L.TLMemo, +n: N.Nui, +t: TouchK) -> MainM:
  Mm.main.q(Mm.rk(n, t), ms, ty, memo, n, t)

# a fresh main area kept as the memo (memo_replay)
def Mm.fresh(+ms: List<&2, MainM>, o: L.Out, +n: N.Nui, +t: TouchK, +memo: L.TLMemo) -> MainM:
  MainM{Mm.key(n, t, Mm.rels(Mm.rk(n, t), ms, n)), Mm.rk(n, t), Mm.rels(Mm.rk(n, t), ms, n), o, Mm.due(n, Mm.rels(Mm.rk(n, t), ms, n), memo)}

# the memo says the main area is unchanged
def Mm.hits(+ms: List<&2, MainM>, +n: N.Nui, +t: TouchK) -> Bool:
  Mm.use(Mm.key(n, t, Mm.rels(Mm.rk(n, t), ms, n)), ms, U.Side.now(N.Nui.ui(n)))

def MainM.key(m: MainM) -> String:
  match m:
    case MainM{k, _, _, _, _}:
      k

def MainM.key.of(ms: List<&2, MainM>) -> String:
  match ms:
    case Nil{}:
      ""
    case MainM{k, _, _, _, _} <> _:
      k

def MainM.keep(m: MainM) -> List<&2, MainM>:
  match m:
    case MainM{+k, rk, rs, o, d}:
      Bool.pick(List<&2, MainM>, String.is_empty(k), Nil{}, [MainM{k, rk, rs, o, d}])

# The timeline's memo
# -------------------
# The timeline's memos (L.Lay.memos) are brought up to a frame only when
# the open thread's entries may have changed: the key is the open thread,
# its last touch (its entries and record), the last touch of all, the
# timeline's width, the page shown ("@more.<thread>"), where it is held
# ("@tl.pin.<thread>", "@away.<thread>") and the language: all that
# L.Lay.memo.es and the entries' parts (L.TL.memo.item: the entry, the
# width, the thread, the faces) read; "" while the timeline is hidden. Otherwise only
# the streamed reply's settled part is brought up (L.TL.live.get), as
# L.Lay.memo does.

def Mm.tlkey.k(on: Bool, +n: N.Nui, +t: TouchK) -> String:
  match on:
    case False{}:
      ""
    case True{}:
      U.Ui.sel(N.Nui.ui(n)) ++ "|" ++ Nat.show(TouchK.at(t, U.Ui.sel(N.Nui.ui(n)))) ++ "|" ++ Nat.show(TouchK.g(t)) ++ "|" ++ U32.show(L.Lay.tl.px(n))
        ++ "|" ++ U.Ui.scratch(N.Nui.ui(n), "@more." ++ U.Ui.sel(N.Nui.ui(n))) ++ "|" ++ U.Ui.scratch(N.Nui.ui(n), "@tl.pin." ++ U.Ui.sel(N.Nui.ui(n)))
        ++ "|" ++ U.Ui.scratch(N.Nui.ui(n), "@away." ++ U.Ui.sel(N.Nui.ui(n))) ++ "|" ++ L.TL.locale(N.Nui.ui(n))

def Mm.tlkey(+n: N.Nui, +t: TouchK) -> String:
  Mm.tlkey.k(U.Ui.timeline(N.Nui.ui(n)), n, t)

def Mm.tlkey.same(+key: String, ks: List<&2, String>) -> Bool:
  match ks:
    case Nil{}:
      False{}
    case +k <> _:
      Bool.and(Bool.not(String.is_empty(key)), String.eq(k, key))

def Mm.memos.live.k(+ty: L.Ty, +n: N.Nui, +m: L.TLMemo) -> L.TLMemo:
  L.TL.memo.with_live(m, L.TL.live.local(L.TL.locale(N.Nui.ui(n)), L.TL.live.get(ty, L.TL.memo.lv(m), U.Live.head(U.Ui.live.of(N.Nui.ui(n), U.Ui.sel(N.Nui.ui(n)))), L.Lay.tl.px(n))))

# the memos with the one in front (the open thread's, made for this key)
# given the streamed reply's settled part
def Mm.memos.live(+ty: L.Ty, +n: N.Nui, ms: List<&2, L.TLMemo>) -> List<&2, L.TLMemo>:
  match ms:
    case Nil{}:
      Nil{}
    case m <> t:
      Mm.memos.live.k(ty, n, m) <> t

# The sidebar's memo
# ------------------
# Its body (L.Lay.sidebar.a: the threads, the links, the bots and rooms)
# reads the whole state but no thread's own log: it is kept while no hub
# message changed what it may show (TouchK's "#side", Sm.blind; anything
# touching all counts) and the window's fields it reads are
# the same: a key typed, the thread scrolled, the pointer moved. The clock
# counts to the second (a snooze ends to the second). The drafts are not
# in it: the sidebar draws none (memo_replay types into the composer and
# checks each kept sidebar against a fresh one).

def TouchK.n(+t: TouchK) -> Nat:
  match t:
    case TouchK{n, _, _, _, _}:
      n

# the caret, unless it is the composer's (in the main area, never drawn
# in the sidebar): a key typed there keeps the sidebar's memo
def Sm.caret(+fo: String, +ca: Nat) -> String:
  Bool.pick(String, String.eq(fo, "composer"), "-", Nat.show(ca))

def Sm.key(+n: N.Nui, +t: TouchK) -> String:
  match n:
    case N.Nui{U.Ui{_, +sel, _, _, +op, on, now, ad, pa, v, rid, w, er, note, +nf, +ob, _}, +ww, +hh, _, _, sd, _, ca, +fo, path, _, _, _}:
      "s" ++ Nat.show(TouchK.at(t, "#side")) ++ "," ++ Nat.show(TouchK.g(t)) ++ "|" ++ sel ++ "|lang" ++ L.TL.locale(N.Nui.ui(n)) ++ "|t" ++ Nat.show(now) ++ "|w" ++ U32.show(ww) ++ "," ++ U32.show(hh) ++ "," ++ U32.show(sd) ++ "," ++ Sm.caret(fo, ca) ++ "," ++ fo ++ "," ++ path
        ++ "|u" ++ Mm.b(on) ++ Mm.b(ad) ++ Mm.b(pa) ++ Mm.b(w) ++ "," ++ v ++ "," ++ Nat.show(rid) ++ "," ++ er ++ "," ++ note
        ++ "|f" ++ Mm.flags(Map.to_list(&2, Bool, op)) ++ "|o" ++ Mm.strs(Map.to_list(&2, String, ob))
        ++ "|i" ++ Mm.info(Map.to_list(&2, String, nf))

type SideM is Data:
  SideM{key: String, out: L.Out, rows: Map<&2, ST.SideRowM>}

def SideM.fresh(key: String, out: L.Out) -> SideM:
  SideM{key, out, Map.new(&2, ST.SideRowM)}

def SideM.of(ms: List<&2, SideM>) -> SideM:
  match ms:
    case Nil{}: SideM.fresh("", L.Out.none())
    case m <> _: m

def SideM.same(+key: String, ms: List<&2, SideM>) -> Bool:
  match ms:
    case Nil{}:
      False{}
    case SideM{+k, _, _} <> _:
      String.eq(k, key)

def SideM.out(ms: List<&2, SideM>) -> L.Out:
  match ms:
    case Nil{}:
      L.Out.none()
    case SideM{_, o, _} <> _:
      o

def SideM.rows(ms: List<&2, SideM>) -> Map<&2, ST.SideRowM>:
  match ms:
    case Nil{}: Map.new(&2, ST.SideRowM)
    case SideM{_, _, rows} <> _: rows

def SideM.from(+key: String, +side: L.SideCol) -> SideM:
  SideM{key, L.SideCol.out(side), SR.Memo.map(L.SideCol.rows(side), Map.new(&2, ST.SideRowM))}

# Native typography is loaded once. Shell.paint.again drops SideM before
# laying out after a glyph fill, so row maps never cross font generations.
# Locale and every visible presentation input are still resolved per row.
def SideM.context(ty: L.Ty, n: N.Nui, ix: U.SIx) -> SR.SideRowCtx:
  match n:
    case N.Nui{U.Ui{st, sel, _, _, _, _, now, _, _, _, _, _, _, _, _, _, _}, _, _, _, _, _, _, _, _, _, _, _, _}:
      SR.SideRowCtx{[ty], [st], [ix], sel, now, 0n}

type SidePaint is Data:
  SidePaint{context: SR.SideRowCtx, rows: Map<&2, ST.SideRowM>}

def SideM.row.one(paint: SidePaint, req: ST.SideRowReq) -> ST.SideRowStep:
  match paint:
    case SidePaint{ctx, rows}: SR.Row.memo(req, ctx, rows)

# A single pointer keeps the native sidebar call below Bend's flattened
# argument limit; the presentation context itself stays immutable.
def SideM.row(paints: List<&2, SidePaint>, req: ST.SideRowReq) -> ST.SideRowStep:
  match paints:
    case p <> _: SideM.row.one(p, req)
    case Nil{}: SideM.row.one(SidePaint{SR.SideRowCtx{Nil{}, Nil{}, Nil{}, "", 0n, 0n}, Map.new(&2, ST.SideRowM)}, req)

def SideM.paint(ty: L.Ty, n: N.Nui, ix: U.SIx, ms: List<&2, SideM>) -> List<&2, SidePaint>:
  [SidePaint{SideM.context(ty, n, ix), SideM.rows(ms)}]

# Keep the native call's deep values boxed while preparing its painter.
type SideInput is Data:
  SideInput{ty: List<&2, L.Ty>, nui: List<&2, N.Nui>, index: List<&2, U.SIx>}

def SideInput.ty(input: SideInput) -> L.Ty:
  match input:
    case SideInput{ty, _, _}: SR.Ctx.ty(ty)

def SideInput.nui.of(ns: List<&2, N.Nui>) -> N.Nui:
  match ns:
    case n <> _: n
    case Nil{}: N.Nui.init(U.Ui.init(0n), 1400, 900)

def SideInput.nui(input: SideInput) -> N.Nui:
  match input:
    case SideInput{_, ns, _}: SideInput.nui.of(ns)

def SideInput.index(input: SideInput) -> U.SIx:
  match input:
    case SideInput{_, _, ix}: SR.Ctx.index(ix)

def SideInput.paint(+input: SideInput, ms: List<&2, SideM>) -> List<&2, SidePaint>:
  SideM.paint(SideInput.ty(input), SideInput.nui(input), SideInput.index(input), ms)

# The sidebar's index
# -------------------
# core's Side.ix walks every thread, task and setting; the sidebar and the
# Status plane's counts each made it every frame. The window keeps one
# (IxK) and brings it up to date: the same while no hub message came, made
# again whole after a change of all, of whose child a thread is (r), or a
# setting applied here ("local.touch"), else only the threads touched
# since are looked at again: their entries in place (Ix.one), or the whole
# index again when one came, went or moved. test/native/memo_replay.bend
# checks each against Side.ix on a real log.

type IxK is Data:
  IxK{n: Nat, lt: String, ix: U.SIx}

def IxK.ix(k: IxK) -> U.SIx:
  match k:
    case IxK{_, _, x}:
      x

def Ix.full(+u: U.Ui, +n: Nat) -> IxK:
  IxK{n, U.Ui.info(u, "local.touch"), U.Side.ix(U.Ui.state(u))}

# (the recursion made once, outside the choice: a Bool.pick with it in both
# branches does the rest twice for every element)
def Ix.keep.s(keep: Bool, +k: String, rest: List<&2, String>) -> List<&2, String>:
  match keep:
    case True{}:
      k <> rest
    case False{}:
      rest

def Ix.keep.a(keep: Bool, a: M.Ask, rest: List<&2, M.Ask>) -> List<&2, M.Ask>:
  match keep:
    case True{}:
      a <> rest
    case False{}:
      rest

# the threads touched after message n0
# (the counts kept beside the threads' touches, "~<thread>" a thread's
# streamed pieces and "#..." stamps, name no thread: one taken for a
# thread found no record and rebuilt the index whole, every frame once a
# thread had streamed more pieces than there were changes)
def Ix.mark(+k: String) -> Bool:
  Bool.or(String.starts_with(k, "~"), String.starts_with(k, "#"))

# the threads touched since n0, from the log (newest first: the walk stops
# at the first entry the index has already taken)
def Ix.since.lg.new(es: List<&2, TkE>, +n0: Nat) -> Bool:
  match es:
    case Nil{}:
      False{}
    case TkE{_, at} <> _:
      Nat.is_gt(at, n0)

def Ix.since.lg.go(es: List<&2, TkE>, +n0: Nat, new: Bool, acc: List<&2, String>) -> List<&2, String>:
  match es new:
    case Nil{} _:
      List.reverse(&2, String, acc)
    case +h <> +t False{}:
      List.reverse(&2, String, acc)
    case TkE{id, _} <> +t True{}:
      Ix.since.lg.go(t, n0, Ix.since.lg.new(t, n0), id <> acc)

def Ix.since.lg(+es: List<&2, TkE>, +n0: Nat) -> List<&2, String>:
  Ix.since.lg.go(es, n0, Ix.since.lg.new(es, n0), Nil{})

def Ix.since(xs: List<&2, Sigma<&2, &2, String, _ => Nat>>, +n0: Nat) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case (+k, +v) <> +t:
      Ix.keep.s(Bool.and(Nat.is_gt(v, n0), Bool.not(Ix.mark(k))), k, Ix.since(t, n0))

def Ix.has(ts: List<&2, M.Thread>, +id: String) -> Bool:
  match ts:
    case Nil{}:
      False{}
    case M.Thread{+i, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _} <> t:
      Bool.or(String.eq(i, id), Ix.has(t, id))

def Ix.swap.k(same: Bool, t: M.Thread, +r: M.Thread, rest: List<&2, M.Thread>) -> List<&2, M.Thread>:
  match same:
    case True{}:
      r <> rest
    case False{}:
      t <> rest

# the list with the thread of r's id replaced by r
def Ix.swap(ts: List<&2, M.Thread>, +r: M.Thread) -> List<&2, M.Thread>:
  match ts:
    case Nil{}:
      Nil{}
    case M.Thread{+i, p, ti, c, a, tu, pe, ar, o, sa, ua, pi, en, br, wt, pr, mo, rm, im, se, de} <> t:
      Ix.swap.k(String.eq(i, M.Thread.id(r)), M.Thread{i, p, ti, c, a, tu, pe, ar, o, sa, ua, pi, en, br, wt, pr, mo, rm, im, se, de}, r, Ix.swap(t, r))

def Ix.list.k(r: Map<&2, List<&2, M.Thread>> & List<&2, M.Thread>) -> List<&2, M.Thread>:
  (_, xs) = r
  xs

def Ix.asks.k(r: Map<&2, List<&2, M.Ask>> & List<&2, M.Ask>) -> List<&2, M.Ask>:
  (_, xs) = r
  xs

def Ix.mine(as: List<&2, M.Ask>, +id: String) -> List<&2, M.Ask>:
  match as:
    case Nil{}:
      Nil{}
    case M.Ask{i, +th, k, t, d, o, op, an} <> +rest:
      Ix.keep.a(String.eq(th, id), M.Ask{i, th, k, t, d, o, op, an}, Ix.mine(rest, id))

def Ix.asks.put(xs: List<&2, M.Ask>, +id: String, m: Map<&2, List<&2, M.Ask>>) -> Map<&2, List<&2, M.Ask>>:
  match xs:
    case Nil{}:
      Map.del(&2, List<&2, M.Ask>, m, id)
    case +a <> t:
      Map.set(&2, List<&2, M.Ask>, m, id, a <> t)

# a thread's open asks, as Side.ix files them
def Ix.asks(+st: M.State, +id: String, m: Map<&2, List<&2, M.Ask>>) -> Map<&2, List<&2, M.Ask>>:
  Ix.asks.put(Ix.asks.k(Map.get(List<&2, M.Ask>, Nil{}, U.SIx.asks.go(List.reverse(&2, M.Ask, Ix.mine(M.Asks.all(st), id)), Map.new(&2, List<&2, M.Ask>)), id)), id, m)

def Ix.yes(r: Map<&2, String> & Bool) -> Bool:
  (_, b) = r
  b

def Ix.yes.t(r: Map<&2, M.Thread> & Bool) -> Bool:
  (_, b) = r
  b

def Ix.sg(s: M.State) -> Map<&2, String>:
  match s:
    case M.State{_, _, _, _, _, sg, _, _, _, _, _}:
      sg

# a thread's own rows, as Work.rows.map keeps them (an entry while it has
# a "bg." or "watch." setting at all)
def Ix.work(+st: M.State, +id: String, +m: Map<&2, Nat>) -> Map<&2, Nat>:
  Bool.pick(Map<&2, Nat>, Bool.or(Ix.yes(Map.has(&2, String, Ix.sg(st), "bg." ++ id)), Ix.yes(Map.has(&2, String, Ix.sg(st), "watch." ++ id))),
    Map.set(&2, Nat, m, id, M.Work.rows(st, id)), Map.del(&2, Nat, m, id))

type IxAcc is Data:
  IxAcc{ok: Bool, by: Map<&2, List<&2, M.Thread>>, asks: Map<&2, List<&2, M.Ask>>, work: Map<&2, Nat>}

def IxAcc.no(a: IxAcc) -> IxAcc:
  match a:
    case IxAcc{_, b, x, w}:
      IxAcc{False{}, b, x, w}

type Ix2 is Data:
  Ix2{ok: Bool, by: Map<&2, List<&2, M.Thread>>}

# a thread's place: where it is filed (its project's list, unless it is a
# delegated child) must be where it was; then it is put in again in place
def Ix.place.k(kid: Bool, there: Bool, r: M.Thread, +p: String, +by: Map<&2, List<&2, M.Thread>>, ok: Bool) -> Ix2:
  match kid there:
    case True{} True{}:
      Ix2{False{}, by}
    case True{} False{}:
      Ix2{ok, by}
    case False{} True{}:
      Ix2{ok, Map.set(&2, List<&2, M.Thread>, by, p, Ix.swap(Ix.list.k(Map.get(List<&2, M.Thread>, Nil{}, by, p)), r))}
    case False{} False{}:
      Ix2{False{}, by}

def Ix.project(t: M.Thread) -> String:
  match t:
    case M.Thread{_, p, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      p

def Ix.place(m: Maybe<&2, M.Thread>, +kids: Map<&2, Bool>, +id: String, +by: Map<&2, List<&2, M.Thread>>, ok: Bool) -> Ix2:
  match m:
    case Some{+r}:
      Ix.place.k(U.Flag.get(kids, id), Ix.has(Ix.list.k(Map.get(List<&2, M.Thread>, Nil{}, by, Ix.project(r))), id), r, Ix.project(r), by, ok)
    case None{}:
      Ix2{False{}, by}

def Ix.one.k(p: Ix2, +st: M.State, +id: String, x: Map<&2, List<&2, M.Ask>>, w: Map<&2, Nat>) -> IxAcc:
  match p:
    case Ix2{ok, b}:
      IxAcc{ok, b, Ix.asks(st, id, x), Ix.work(st, id, w)}

def Ix.thread.k(r: Map<&2, M.Thread> & M.Thread, has: Bool) -> Maybe<&2, M.Thread>:
  (_, t) = r
  Bool.pick(Maybe<&2, M.Thread>, has, Some{t}, None{})

def Ix.thread(+st: M.State, +id: String) -> Maybe<&2, M.Thread>:
  match st:
    case M.State{_, +ts, _, _, _, _, _, _, _, _, _}:
      Ix.thread.k(Map.get(M.Thread, M.Thread.blank(id), ts, id), Ix.yes.t(Map.has(&2, M.Thread, ts, id)))

def Ix.one(a: IxAcc, +st: M.State, +kids: Map<&2, Bool>, +id: String) -> IxAcc:
  match a:
    case IxAcc{ok, b, x, w}:
      Ix.one.k(Ix.place(Ix.thread(st, id), kids, id, b, ok), st, id, x, w)

def Ix.all(ids: List<&2, String>, a: IxAcc, +st: M.State, +kids: Map<&2, Bool>) -> IxAcc:
  match ids:
    case Nil{}:
      a
    case +id <> t:
      Ix.all(t, Ix.one(a, st, kids, id), st, kids)

def Ix.inc.k(a: IxAcc, +u: U.Ui, +n: Nat, +lt: String) -> IxK:
  match a:
    case IxAcc{True{}, b, x, w}:
      IxK{n, lt, U.SIx{b, x, M.Work.kids.map(U.Ui.state(u)), w}}
    case IxAcc{False{}, _, _, _}:
      Ix.full(u, n)

def Ix.inc(ix: U.SIx, +ids: List<&2, String>, +u: U.Ui, +n: Nat, +lt: String) -> IxK:
  match ix:
    case U.SIx{b, x, _, w}:
      Ix.inc.k(Ix.all(ids, IxAcc{True{}, b, x, w}, U.Ui.state(u), U.Kids.set(U.Ui.state(u))), u, n, lt)

def Ix.up.k(i: Nat, k: IxK, +t: TouchK, +u: U.Ui) -> IxK:
  match i k:
    case 0n _:
      k
    case 1n _:
      Ix.full(u, TouchK.n(t))
    case _ IxK{n0, lt, ix}:
      Ix.inc(ix, Ix.since.lg(TouchK.lg(t), n0), u, TouchK.n(t), lt)

def Ix.mode(+n0: Nat, +lt0: String, +t: TouchK, +u: U.Ui) -> Nat:
  Bool.pick(Nat, Bool.not(String.eq(lt0, U.Ui.info(u, "local.touch"))), 1n,
    Bool.pick(Nat, Bool.or(Nat.is_gt(TouchK.index_g(t), n0), Nat.is_gt(TouchK.r(t), n0)), 1n,
      Bool.pick(Nat, Nat.is_eq(TouchK.n(t), n0), 0n, 2n)))

def IxK.n(k: IxK) -> Nat:
  match k:
    case IxK{n, _, _}:
      n

def IxK.lt(k: IxK) -> String:
  match k:
    case IxK{_, l, _}:
      l

# the index for this frame, from the last frame's
def Ix.up(ks: List<&2, IxK>, +t: TouchK, +u: U.Ui) -> IxK:
  match ks:
    case Nil{}:
      Ix.full(u, TouchK.n(t))
    case +k <> _:
      Ix.up.k(Ix.mode(IxK.n(k), IxK.lt(k), t, u), k, t, u)

def Ix.ids.n(ts: List<&2, M.Thread>, +acc: Nat) -> Nat:
  match ts:
    case Nil{}:
      acc
    case M.Thread{+i, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _} <> t:
      Ix.ids.n(t, Nat.add(acc, String.length(i)))

# The index is made in full each frame (its lists walked): one left to be
# made later would keep the last frame's state and be built on, unmade,
# frame after frame
def Ix.lens(xs: List<&2, Sigma<&2, &2, String, _ => List<&2, M.Thread>>>, +acc: Nat) -> Nat:
  match xs:
    case Nil{}:
      acc
    case (_, v) <> t:
      Ix.lens(t, Nat.add(acc, Ix.ids.n(v, 0n)))

def IxK.force(k: IxK) -> Nat:
  match k:
    case IxK{n, lt, U.SIx{by, a, kd, w}}:
      Nat.add(Nat.add(n, Ix.lens(Map.to_list(&2, List<&2, M.Thread>, by), 0n)),
        Nat.add(List.length(&2, String, Map.keys(&2, List<&2, M.Ask>, a)), Nat.add(List.length(&2, String, Map.keys(&2, Nat, kd)), List.length(&2, String, Map.keys(&2, Nat, w)))))

# A Mailroom body memo. The header/cats, sidebar and overlays are composed
# afresh. No new touch-log reader: revision comes from the mail client.
type BotM is Data:
  BotM{key: String, out: L.Out, at: Nat, due: Nat, hit: Bool}

def BotM.empty() -> BotM:
  BotM{"", L.Out.none(), 0n, 0n, False{}}

def BotM.key(m: BotM) -> String:
  match m:
    case BotM{k, _, _, _, _}: k

def BotM.out(m: BotM) -> L.Out:
  match m:
    case BotM{_, o, _, _, _}: o

def BotM.hit(m: BotM) -> Bool:
  match m:
    case BotM{_, _, _, _, h}: h

def BotM.same(+key: String, +now: Nat, m: BotM) -> Bool:
  match m:
    case BotM{k, _, at, +due, _}:
      Bool.and(String.eq(key, k), Bool.and(Nat.is_ge(now, at), Bool.or(Nat.is_eq(due, 0n), Nat.is_lt(now, due))))

def BotM.kept(m: BotM) -> BotM:
  match m:
    case BotM{k, o, at, due, _}: BotM{k, o, at, due, True{}}

# Mv.ago and I18n.ago change at relative age boundaries, rather than the
# wall clock's minute. Future timestamps still say now until at+60.
def BotM.age_due(+now: Nat, +at: Nat) -> Nat:
  +d = Nat.sub(now, at)
  +unit = Bool.pick(Nat, Nat.is_lt(d, 3600n), 60n, Bool.pick(Nat, Nat.is_lt(d, 86400n), 3600n, 86400n))
  Nat.add(at, Nat.mul(Nat.add(Nat.div(d, unit), 1n), unit))

def Mm.mail.fields(xs: List<&2, String>, +u: U.Ui) -> String:
  match xs:
    case Nil{}: ""
    case +x <> t: x ++ "\u{1f}" ++ J.Json.show(J.Str{U.Ui.scratch(u, x)}) ++ "\u{1e}" ++ Mm.mail.fields(t, u)

# ---- mail ui ----
# Exact Mailroom body inputs. The plane signature is computed only when
# the existing PW/@plane is rebuilt. Age deadlines live in BotM, rather
# than this key. Unrelated thread drafts and animation clocks stay out.
def Mm.mail(+n: N.Nui, +plane_key: String) -> String:
  +u = N.Nui.ui(n)
  U32.show(N.Nui.width(n)) ++ "," ++ U32.show(N.Nui.height(n)) ++ "," ++ U32.show(Mm.x1.of(n))
    ++ "|mail=" ++ Nat.show(U.Mailv.touch(u)) ++ "|page=" ++ Nat.show(U.Mailv.rev(u)) ++ "|plane=" ++ plane_key
    ++ "|bot=" ++ U.Mailv.owner(u) ++ "|filter=" ++ U.Mailv.key(u)
    ++ "|origin=" ++ J.Json.show(J.Str{U.Ui.info(u, "origin")}) ++ "|generation=" ++ J.Json.show(J.Str{U.Ui.info(u, "local.generation")})
    ++ "|tab=" ++ U.Ui.scratch(u, "@btab") ++ "|theme=" ++ U32.show(N.Nui.look(n))
    ++ "|view=" ++ Mm.mail.fields(["@mr.run", "@mr.machine", "@mr.conv", "@mr.to", "@mr.status", "@mr.project", "@mr.bot", "@f.mrsearch", "@f.mrproject", "@f.mrreply"], u)
    ++ "|scroll=" ++ U.Ui.info(u, "pane:mr") ++ "," ++ U.Ui.info(u, "pane:mrbody")
    ++ "|edit=" ++ N.Nui.focus_of(n) ++ "," ++ Nat.show(N.Nui.caret_of(n)) ++ "," ++ Nat.show(N.Nui.mark(n)) ++ Mm.b(N.Blink.on(n))
    ++ "|lang=" ++ L.TL.locale(u)
