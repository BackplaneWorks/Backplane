import Base
import ../../src/core/codec.bend as C
import ../../src/core/client.bend as U
import ../../src/core/model.bend as M
import ../../src/app/nui.bend as N
import ../../src/app/win.bend as W
import ../../src/app/layout.bend as L
import ../../src/app/theme.bend as T
import ../../src/gfx/raster.bend as R
import ../../src/gfx/tiles.bend as TI
import ../../src/app/bots.bend as BV
import ../../src/app/cats.bend as CA
import ../../src/core/json.bend as J
import ../../src/core/utf8.bend as U8
import ../../src/server/argv.bend as AV
import ../../src/app/mm.bend as MM
def ui_sel(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}:
      u

# what one frame leaves: the window state (with its hit regions), the memo
# and the tiles
type Kept is Data:
  Kept{n: N.Nui, memo: L.TLMemo, frames: TI.Frames}

type Times is Data:
  Times{ev: Nat, memo: Nat, lay: Nat, tiles: Nat, worst: Nat, each: String}

def hits(o: L.Out) -> List<&2, N.Hit>:
  match o:
    case L.Out{_, hs}:
      hs

def ops(o: L.Out) -> List<&2, R.Op>:
  match o:
    case L.Out{os, _}:
      os

def with_hits(n: N.Nui, hs: List<&2, N.Hit>, +ms: Nat) -> N.Nui:
  match n:
    case N.Nui{u, w, h, mx, my, sd, sc, ca, fo, pa, _, _, vw}:
      N.Nui{u, w, h, mx, my, sd, sc, ca, fo, pa, hs, ms, vw}

def prepared(+n: N.Nui, +ms: Nat) -> N.Nui:
  N.Lbx.stamp(N.Blink.stamp(N.Hover.stamp(N.Nui.prepare(n), ms), ms), ms)

def read.k(r: File & Result<&1, &1, U32 & String, List<&2, U32>>) -> IO(List<&2, U32>):
  (f, res) = r
  do IO<List<&2, U32>>:
    File.close(f)
    bs : List<&2, U32> <- IO.pass(List<&2, U32>, res)
    return bs

def read(+path: String) -> IO(List<&2, U32>):
  do IO<List<&2, U32>>:
    f : File <- IO.try(File, File.open(path, "r"))
    r : File & Result<&1, &1, U32 & String, List<&2, U32>> <- File.read_bytes(f, 200000000)
    read.k(r)

def copy(xs: List<&1, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      x <> copy(t)

def arg(+xs: List<&2, String>, +i: Nat) -> String:
  Maybe.default(&2, String, List.head(&2, String, List.drop(&2, String, xs, i)), "")

# the made-up log, or the real one named on the command line

# The main area's memo checked on a real log: the log less its last K
# events gives the window's state, then each of the K comes as a hub
# message of its own, the client's clock moved to its time. At each step the main area is laid out from scratch
# and, when the memo's key says nothing it shows changed, compared with the
# memo's: they must be the same, op for op and hit for hit.
#   scripts/build-app.sh test/native/memo_replay.bend build/memo_replay
#   build/memo_replay EVENTS THREAD [K]

def lines(r: U8.Lines & List<&2, String>) -> List<&2, String>:
  (_, ls) = r
  ls

def items(ls: List<&2, String>, acc: List<&2, J.Json>) -> List<&2, J.Json>:
  match ls:
    case Nil{}:
      List.reverse(&2, J.Json, acc)
    case l <> t:
      items(t, Maybe.default(&2, J.Json, J.Json.parse(l), J.Null{}) <> acc)

def real(xs: List<&2, J.Json>, +sel: String) -> U.Ui:
  ui_sel(U.Ui.act(U.Ui.recv(U.Ui.init(1790180000n), J.Json.obj([J.KV{"t", J.Str{"log"}}, J.KV{"version", J.Str{"0.1.0"}},
    J.KV{"items", J.Json.arr(xs)}])), "select", sel))

def Eqh.hits(xs: List<&2, N.Hit>, ys: List<&2, N.Hit>) -> Bool:
  match xs ys:
    case Nil{} Nil{}:
      True{}
    case N.Hit{a0, a1, a2, a3, a4, a5} <> at N.Hit{b0, b1, b2, b3, b4, b5} <> bt:
      Bool.and(Bool.and(Bool.and(U32.is_eq(a0, b0), U32.is_eq(a1, b1)), Bool.and(U32.is_eq(a2, b2), U32.is_eq(a3, b3))),
        Bool.and(Bool.and(String.eq(a4, b4), String.eq(a5, b5)), Eqh.hits(at, bt)))
    case _ _:
      False{}

def Eqh.out(a: L.Out, b: L.Out) -> Bool:
  match a b:
    case L.Out{ao, ah} L.Out{bo, bh}:
      Bool.and(TI.Eq.ops(ao, bo), Eqh.hits(ah, bh))

type Tally is Data:
  Tally{steps: Nat, hits: Nat, bad: Nat, off: Nat, why: String}

def Tally.add(t: Tally, +hit: Bool, +ok: Bool, off: Bool, +what: String) -> Tally:
  match t:
    case Tally{s, h, b, o, +w}:
      Tally{1n+s, Nat.add(h, Bool.pick(Nat, hit, 1n, 0n)), Nat.add(b, Bool.pick(Nat, Bool.and(hit, Bool.not(ok)), 1n, 0n)), Nat.add(o, Bool.pick(Nat, off, 1n, 0n)),
        Bool.pick(String, Bool.and(hit, Bool.not(ok)), w ++ "\n  " ++ what, w)}

def Tally.show(t: Tally) -> String:
  match t:
    case Tally{s, h, b, o, w}:
      "steps " ++ Nat.show(s) ++ ", memo hits " ++ Nat.show(h) ++ ", memo hits unlike a fresh layout " ++ Nat.show(b) ++ ", steps not memoized " ++ Nat.show(o) ++ w

def exact(ok: Bool, label: String) -> IO(Unit):
  match ok:
    case True{}:
      IO.pure(Unit, Unit{})
    case False{}:
      IO.die(Unit, 1, "memo_replay failed: " ++ label)

def Tally.ok(t: Tally) -> Bool:
  match t:
    case Tally{_, _, bad, _, _}:
      Nat.is_eq(bad, 0n)

def with_ui(n: N.Nui, u: U.Ui) -> N.Nui:
  match n:
    case N.Nui{_, w, h, mx, my, sd, sc, ca, fo, pa, hi, ms, vw}:
      N.Nui{u, w, h, mx, my, sd, sc, ca, fo, pa, hi, ms, vw}

def msg(x: J.Json) -> J.Json:
  J.Json.obj([J.KV{"t", J.Str{"changes"}}, J.KV{"items", J.Json.arr([x])}])

# each event its own message: the state, the touches, the main area again
def steps(xs: List<&2, J.Json>, +ty: L.Ty, +n0: N.Nui, +memo0: L.TLMemo, +mm: List<&2, MM.MainM>, tc: MM.TouchK, t: Tally) -> Tally:
  match xs:
    case Nil{}:
      t
    case +x <> rest:
      +o = msg(x)
      +n = prepared(with_ui(n0, U.Ui.tick(U.Ui.recv(N.Nui.ui(n0), o), J.Json.get_nat(x, "at", U.Side.now(N.Nui.ui(n0))))), 0n)
      +tc2 = MM.TouchK.msg(o, tc)
      +memo = L.Lay.memo(ty, n, memo0)
      +fresh = MM.Mm.lay(ty, memo, n)
      +keep = MM.MainM.keep(MM.Mm.fresh(mm, fresh, n, tc2, memo))
      steps(rest, ty, with_hits(n, hits(fresh), 0n), memo, keep, tc2,
        Tally.add(t, MM.Mm.hits(mm, n, tc2), Eqh.out(MM.MainM.out(mm), fresh), List.is_empty(&2, MM.MainM, keep), String.take(J.Json.show(x), 160n)))

def step_nui(s: N.Step) -> N.Nui:
  match s:
    case N.Step{n, _, _}:
      n

# Typing, with no hub message: the sidebar's memo checked the same way
type Side is Data:
  Side{steps: Nat, hits: Nat, bad: Nat}

def Side.add(t: Side, +hit: Bool, +ok: Bool) -> Side:
  match t:
    case Side{s, h, b}:
      Side{1n+s, Nat.add(h, Bool.pick(Nat, hit, 1n, 0n)), Nat.add(b, Bool.pick(Nat, Bool.and(hit, Bool.not(ok)), 1n, 0n))}

def Side.ok(t: Side) -> Bool:
  match t:
    case Side{_, _, bad}:
      Nat.is_eq(bad, 0n)

def Side.show(t: Side) -> String:
  match t:
    case Side{s, h, b}:
      "typing: steps " ++ Nat.show(s) ++ ", sidebar memo hits " ++ Nat.show(h) ++ ", unlike a fresh one " ++ Nat.show(b)

def side.fresh(+ty: L.Ty, +n: N.Nui) -> L.Out:
  BV.Bv.side.a(ty, CA.Cats.none(), BV.Bv.wraps(ty, n, BV.Bv.wraps.none()), n)

def side.ev(+c: U32, +i: Nat) -> W.WinEv:
  match i:
    case 0n:
      W.WMove{(400 + c : U32), 500, 0}
    case _:
      W.WKey{c, c, 0, True{}, 0}

def typing(cs: String, +ty: L.Ty, +n0: N.Nui, +tc: MM.TouchK, +ms: List<&2, MM.SideM>, t: Side) -> Side:
  match cs:
    case SNil{}:
      t
    case SCon{Chr{+c}, rest}:
      +n = prepared(step_nui(N.Nui.event(n0, side.ev(c, Nat.mod(U32.to_nat(c), 4n)))), 0n)
      +key = MM.Sm.key(n, tc)
      +fresh = side.fresh(ty, n)
      typing(rest, ty, n, tc, [MM.SideM.fresh(key, fresh)], Side.add(t, MM.SideM.same(key, ms), Eqh.out(MM.SideM.out(ms), fresh)))


# The sidebar's index brought up to date (MM.Ix.up) against Side.ix made
# whole, at each step: every thread filed, every field of it, the asks, the
# children counted and the rows
def Fp.b(b: Bool) -> String:
  Bool.pick(String, b, "1", "0")

def Fp.ov(o: M.Override) -> String:
  match o:
    case M.NoOverride{}:
      "n"
    case M.ForcedSettled{}:
      "s"
    case M.ForcedActive{}:
      "a"

def Fp.thread(t: M.Thread) -> String:
  match t:
    case M.Thread{id, p, ti, c, a, tu, pe, ar, o, sa, ua, pi, en, br, wt, pr, mo, rm, im, se, de}:
      id ++ "|" ++ p ++ "|" ++ ti ++ "|" ++ Nat.show(c) ++ "|" ++ Nat.show(a) ++ "|" ++ C.Turn.name(tu) ++ "|" ++ Nat.show(pe) ++ "|" ++ Fp.b(ar) ++ "|" ++ Fp.ov(o)
        ++ "|" ++ Nat.show(sa) ++ "|" ++ Nat.show(ua) ++ "|" ++ Fp.b(pi) ++ "|" ++ C.Env.name(en) ++ "|" ++ br ++ "|" ++ wt ++ "|" ++ pr ++ "|" ++ mo
        ++ "|" ++ C.Runtime.name(rm) ++ "|" ++ C.Interaction.name(im) ++ "|" ++ se ++ "|" ++ Fp.b(de) ++ ";"

def Fp.threads(ts: List<&2, M.Thread>) -> String:
  match ts:
    case Nil{}:
      ""
    case t <> rest:
      Fp.thread(t) ++ Fp.threads(rest)

def Fp.by(xs: List<&2, Sigma<&2, &2, String, _ => List<&2, M.Thread>>>) -> String:
  match xs:
    case Nil{}:
      ""
    case (+k, v) <> t:
      "[" ++ k ++ "]" ++ Fp.threads(v) ++ Fp.by(t)

def Fp.askl(as: List<&2, M.Ask>) -> String:
  match as:
    case Nil{}:
      ""
    case M.Ask{i, th, k, tl, d, o, op, an} <> t:
      i ++ "," ++ th ++ "," ++ k ++ "," ++ tl ++ "," ++ d ++ "," ++ o ++ "," ++ Fp.b(op) ++ "," ++ an ++ ";" ++ Fp.askl(t)

def Fp.asks(xs: List<&2, Sigma<&2, &2, String, _ => List<&2, M.Ask>>>) -> String:
  match xs:
    case Nil{}:
      ""
    case (+k, v) <> t:
      "[" ++ k ++ "]" ++ Fp.askl(v) ++ Fp.asks(t)

def Fp.nats(xs: List<&2, Sigma<&2, &2, String, _ => Nat>>) -> String:
  match xs:
    case Nil{}:
      ""
    case (+k, v) <> t:
      k ++ "=" ++ Nat.show(v) ++ ";" ++ Fp.nats(t)

def Fp.ix(x: U.SIx) -> String:
  match x:
    case U.SIx{by, a, k, w}:
      Fp.by(Map.to_list(&2, List<&2, M.Thread>, by)) ++ "#" ++ Fp.asks(Map.to_list(&2, List<&2, M.Ask>, a)) ++ "#" ++ Fp.nats(Map.to_list(&2, Nat, k)) ++ "#" ++ Fp.nats(Map.to_list(&2, Nat, w))

type IxT is Data:
  IxT{steps: Nat, bad: Nat, why: String}

def IxT.add(t: IxT, +ok: Bool, +what: String) -> IxT:
  match t:
    case IxT{s, b, +w}:
      IxT{1n+s, Nat.add(b, Bool.pick(Nat, ok, 0n, 1n)), Bool.pick(String, ok, w, w ++ "\n  " ++ what)}

def IxT.ok(t: IxT) -> Bool:
  match t:
    case IxT{_, bad, _}:
      Nat.is_eq(bad, 0n)

def IxT.show(t: IxT) -> String:
  match t:
    case IxT{s, b, w}:
      "index: steps " ++ Nat.show(s) ++ ", kept index unlike Side.ix " ++ Nat.show(b) ++ w

def ixsteps(xs: List<&2, J.Json>, +u0: U.Ui, +tc: MM.TouchK, +k: MM.IxK, t: IxT) -> IxT:
  match xs:
    case Nil{}:
      t
    case +x <> rest:
      +o = msg(x)
      +u = U.Ui.recv(u0, o)
      +tc2 = MM.TouchK.msg(o, tc)
      +k2 = MM.Ix.up([k], tc2, u)
      ixsteps(rest, u, MM.TouchK.consumed(tc2, [MM.IxK.n(k2)]), k2, IxT.add(t, String.eq(Fp.ix(MM.IxK.ix(k2)), Fp.ix(U.Side.ix(U.Ui.state(u)))), String.take(J.Json.show(x), 160n)))

# Why the main area's key changes: each event as its own hub message, the
# clock moved to the event's time (as the client's tick does), and the
# key compared with the last; the first part that differs is counted (for
# the info map, the key that changed; for the touches, whose)
def Why.name(+x: String) -> String:
  String.take(Maybe.default(&2, String, List.head(&2, String, String.split(x, '\u{1f}')), ""), 24n)

def Why.ents.k(same: Bool, +x: String, +y: String, rest: String) -> String:
  match same:
    case True{}:
      rest
    case False{}:
      Why.name(x) ++ "/" ++ Why.name(y)

def Why.ents(xs: List<&2, String>, ys: List<&2, String>) -> String:
  match xs ys:
    case Nil{} Nil{}:
      "same"
    case +x <> xt +y <> yt:
      Why.ents.k(String.eq(x, y), x, y, Why.ents(xt, yt))
    case _ _:
      "count"

def Why.seg.k(same: Bool, +x: String, +y: String, rest: String) -> String:
  match same:
    case True{}:
      rest
    case False{}:
      Bool.pick(String, String.starts_with(y, "i"), "info:" ++ Why.ents(String.split(x, '\u{1e}'), String.split(y, '\u{1e}')), String.take(y, 6n))

def Why.segs(xs: List<&2, String>, ys: List<&2, String>) -> String:
  match xs ys:
    case Nil{} Nil{}:
      "same"
    case +x <> xt +y <> yt:
      Why.seg.k(String.eq(x, y), x, y, Why.segs(xt, yt))
    case _ _:
      "parts"

def Why.of(+a: String, +b: String) -> String:
  Why.segs(String.split(a, '|'), String.split(b, '|'))

def Why.bump.k(r: Map<&2, Nat> & Nat, +k: String) -> Map<&2, Nat>:
  (m, v) = r
  Map.set(&2, Nat, m, k, 1n+v)

def Why.bump(m: Map<&2, Nat>, +k: String) -> Map<&2, Nat>:
  Why.bump.k(Map.get(Nat, 0n, m, k), k)

def Why.show(xs: List<&2, Sigma<&2, &2, String, _ => Nat>>) -> String:
  match xs:
    case Nil{}:
      ""
    case (+k, v) <> t:
      "\n  " ++ Nat.show(v) ++ "  " ++ k ++ Why.show(t)

def Why.key(+n: N.Nui, +t: MM.TouchK) -> String:
  MM.Mm.key(n, t, MM.Mm.rels.of(n))

def why(xs: List<&2, J.Json>, +n0: N.Nui, +tc: MM.TouchK, +k0: String, m: Map<&2, Nat>) -> Map<&2, Nat>:
  match xs:
    case Nil{}:
      m
    case +x <> rest:
      +o = msg(x)
      +n = prepared(with_ui(n0, U.Ui.tick(U.Ui.recv(N.Nui.ui(n0), o), J.Json.get_nat(x, "at", U.Side.now(N.Nui.ui(n0))))), 0n)
      +tc2 = MM.TouchK.msg(o, tc)
      +k = Why.key(n, tc2)
      why(rest, n, tc2, k, Why.bump(m, J.Json.get_str(x, "$", "") ++ " -> " ++ Why.of(k0, k)))

# Streamed text: after each event, a piece of the open thread's reply
# streams in, then a piece of another thread's, each as the hub sends it
# ({"t":"delta"}); the window's main area (MM.Mm.main, its memo kept from
# the frame before) is compared with one laid out afresh at every step.
# Another thread's text must leave the memo in use.
type ST is Data:
  ST{steps: Nat, own: Nat, other: Nat, others: Nat, bad: Nat, why: String}

def ST.add(t: ST, +kind: Nat, +hit: Bool, +ok: Bool, +what: String) -> ST:
  match t:
    case ST{s, o, h, hs, b, +w}:
      ST{1n+s, Nat.add(o, Bool.pick(Nat, Bool.and(Nat.is_eq(kind, 1n), hit), 1n, 0n)), Nat.add(h, Bool.pick(Nat, Bool.and(Nat.is_eq(kind, 2n), hit), 1n, 0n)),
        Nat.add(hs, Bool.pick(Nat, Nat.is_eq(kind, 2n), 1n, 0n)), Nat.add(b, Bool.pick(Nat, ok, 0n, 1n)), Bool.pick(String, ok, w, w ++ "\n  " ++ what)}

def ST.ok(t: ST) -> Bool:
  match t:
    case ST{_, _, _, _, bad, _}:
      Nat.is_eq(bad, 0n)

def ST.show(t: ST) -> String:
  match t:
    case ST{s, o, h, hs, b, w}:
      "streaming: steps " ++ Nat.show(s) ++ ", own text kept the memo " ++ Nat.show(o) ++ ", another thread's text kept it " ++ Nat.show(h) ++ " of " ++ Nat.show(hs)
        ++ ", main area unlike a fresh one " ++ Nat.show(b) ++ w

def piece(+i: Nat) -> String:
  match i:
    case 0n:
      "The regulator's enable pin is pulled up "
    case 1n:
      "through R12 to VBAT, so the output\n\n"
    case 2n:
      "- one item with `code`\n"
    case _:
      "and **C7** stays.\n\n```\nfence\n"

def delta(+th: String, +i: Nat) -> J.Json:
  J.Json.obj([J.KV{"t", J.Str{"delta"}}, J.KV{"thread", J.Str{th}}, J.KV{"text", J.Str{piece(Nat.mod(i, 4n))}}])

def ss.recv(+n: N.Nui, +o: J.Json, +tick: Nat) -> N.Nui:
  prepared(with_ui(n, U.Ui.tick(U.Ui.recv(N.Nui.ui(n), o), tick)), 0n)

def ss.check.k(+kind: Nat, +ty: L.Ty, +pm0: List<&2, MM.MainM>, +n: N.Nui, +memo: L.TLMemo, +pm: List<&2, MM.MainM>, +what: String, t: ST) -> ST:
  ST.add(t, kind, MM.MainM.same(MM.MainM.key.of(pm), pm0), Eqh.out(MM.MainM.out(pm), MM.Mm.lay(ty, memo, n)), what)

def ss.main(+ty: L.Ty, +n: N.Nui, +memo: L.TLMemo, +pm: List<&2, MM.MainM>, +tc: MM.TouchK) -> List<&2, MM.MainM>:
  MM.MainM.keep(MM.Mm.main(pm, ty, memo, n, tc))

def streams(xs: List<&2, J.Json>, +i: Nat, +ty: L.Ty, +n0: N.Nui, memo0: L.TLMemo, +pm0: List<&2, MM.MainM>, +tc0: MM.TouchK, t: ST) -> ST:
  match xs:
    case Nil{}:
      t
    case +x <> rest:
      +o = msg(x)
      +what = String.take(J.Json.show(x), 120n)
      +n1 = ss.recv(n0, o, J.Json.get_nat(x, "at", U.Side.now(N.Nui.ui(n0))))
      +tc1 = MM.TouchK.msg(o, tc0)
      +m1 = L.Lay.memo(ty, n1, memo0)
      +p1 = ss.main(ty, n1, m1, pm0, tc1)
      +t1 = ss.check.k(0n, ty, pm0, n1, m1, p1, what, t)
      +d2 = delta(U.Ui.sel(N.Nui.ui(n1)), i)
      +n2 = ss.recv(n1, d2, U.Side.now(N.Nui.ui(n1)))
      +tc2 = MM.TouchK.msg(d2, tc1)
      +m2 = L.Lay.memo(ty, n2, m1)
      +p2 = ss.main(ty, n2, m2, p1, tc2)
      +t2 = ss.check.k(1n, ty, p1, n2, m2, p2, "own text after " ++ what, t1)
      +d3 = delta("t-elsewhere", i)
      +n3 = ss.recv(n2, d3, U.Side.now(N.Nui.ui(n2)))
      +tc3 = MM.TouchK.msg(d3, tc2)
      +m3 = L.Lay.memo(ty, n3, m2)
      +p3 = ss.main(ty, n3, m3, p2, tc3)
      +t3 = ss.check.k(2n, ty, p2, n3, m3, p3, "other text after " ++ what, t2)
      streams(rest, 1n+i, ty, with_hits(n3, hits(MM.MainM.out(p3)), 0n), m3, p3, tc3, t3)

# The index again with the open thread streaming three pieces after each
# event: its count of pieces soon passes the count of changes, and no step
# may take it for a touched thread (MM.Ix.since), which would rebuild the
# index whole; the kept index stays Side.ix
def ix.marked(ks: List<&2, String>) -> Nat:
  match ks:
    case Nil{}:
      0n
    case +k <> t:
      Nat.add(Bool.pick(Nat, MM.Ix.mark(k), 1n, 0n), ix.marked(t))

# the counts the old rule took for a thread: a mark above the changes
def ix.over(xs: List<&2, Sigma<&2, &2, String, _ => Nat>>, +n0: Nat) -> Nat:
  match xs:
    case Nil{}:
      0n
    case (+k, +v) <> t:
      Nat.add(Bool.pick(Nat, Bool.and(MM.Ix.mark(k), Nat.is_gt(v, n0)), 1n, 0n), ix.over(t, n0))

def ix.d(+u: U.Ui, +tc: MM.TouchK, +th: String, +i: Nat) -> U.Ui:
  U.Ui.recv(u, delta(th, i))

def ix.dt(+tc: MM.TouchK, +th: String, +i: Nat) -> MM.TouchK:
  MM.TouchK.msg(delta(th, i), tc)

def ixs.done(t: IxT, +bad: Nat, old: Nat) -> IxT:
  match t:
    case IxT{steps, wrong, why}:
      IxT{steps, Nat.add(wrong, bad), why ++ ", steps that took a count of pieces for a thread " ++ Nat.show(bad) ++ " (the rule before took one in " ++ Nat.show(old) ++ ")"}

def ixs.stream(xs: List<&2, J.Json>, +i: Nat, +th: String, +u0: U.Ui, +tc: MM.TouchK, +k: MM.IxK, +bad: Nat, +old: Nat, t: IxT) -> IxT:
  match xs:
    case Nil{}:
      ixs.done(t, bad, old)
    case +x <> rest:
      +o = msg(x)
      +u1 = U.Ui.recv(u0, o)
      +u = ix.d(ix.d(ix.d(u1, tc, th, i), tc, th, 1n+i), tc, th, Nat.add(i, 2n))
      +tc2 = ix.dt(ix.dt(ix.dt(MM.TouchK.msg(o, tc), th, i), th, 1n+i), th, Nat.add(i, 2n))
      +marks = ix.marked(MM.Ix.since(Map.to_list(&2, Nat, MM.TouchK.m(tc2)), MM.IxK.n(k)))
      +k2 = MM.Ix.up([k], tc2, u)
      +over = Bool.pick(Nat, Nat.is_gt(ix.over(Map.to_list(&2, Nat, MM.TouchK.m(tc2)), MM.IxK.n(k)), 0n), 1n, 0n)
      ixs.stream(rest, Nat.add(i, 3n), th, u, tc2, k2, Nat.add(bad, marks), Nat.add(old, over), IxT.add(t, String.eq(Fp.ix(MM.IxK.ix(k2)), Fp.ix(U.Side.ix(U.Ui.state(u)))), String.take(J.Json.show(x), 160n)))
# The sidebar's memo over the same events: each event as its own hub
# message, the clock at its time; a kept sidebar used when its key holds is
# compared with a fresh one
def sides(xs: List<&2, J.Json>, +ty: L.Ty, +n0: N.Nui, +tc: MM.TouchK, +ms: List<&2, MM.SideM>, t: Side) -> Side:
  match xs:
    case Nil{}:
      t
    case +x <> rest:
      +o = msg(x)
      +n = prepared(with_ui(n0, U.Ui.tick(U.Ui.recv(N.Nui.ui(n0), o), J.Json.get_nat(x, "at", U.Side.now(N.Nui.ui(n0))))), 0n)
      +tc2 = MM.TouchK.msg(o, tc)
      +key = MM.Sm.key(n, tc2)
      +fresh = side.fresh(ty, n)
      sides(rest, ty, n, tc2, [MM.SideM.fresh(key, fresh)], Side.add(t, MM.SideM.same(key, ms), Eqh.out(MM.SideM.out(ms), fresh)))

# The timeline's memos as the window brings them up (only the streamed
# reply's part while MM.Mm.tlkey holds) against memos brought up in full,
# over every event and a piece of the open thread's text after each: the
# main areas laid out from each must be the same
type TlT is Data:
  TlT{steps: Nat, kept: Nat, bad: Nat, why: String}

def TlT.add(t: TlT, +kept: Bool, +ok: Bool, +what: String) -> TlT:
  match t:
    case TlT{s, k, b, +w}:
      TlT{1n+s, Nat.add(k, Bool.pick(Nat, kept, 1n, 0n)), Nat.add(b, Bool.pick(Nat, ok, 0n, 1n)), Bool.pick(String, ok, w, w ++ "\n  " ++ what)}

def TlT.ok(t: TlT) -> Bool:
  match t:
    case TlT{_, _, bad, _}:
      Nat.is_eq(bad, 0n)

def TlT.show(t: TlT) -> String:
  match t:
    case TlT{s, k, b, w}:
      "timeline memos: steps " ++ Nat.show(s) ++ ", brought up only for the streamed text " ++ Nat.show(k) ++ ", main area unlike one from full memos " ++ Nat.show(b) ++ w

def tl.win(same: Bool, +ty: L.Ty, +n: N.Nui, ms: List<&2, L.TLMemo>) -> List<&2, L.TLMemo>:
  match same:
    case True{}:
      MM.Mm.memos.live(ty, n, ms)
    case False{}:
      L.Lay.memos(ty, n, ms)

# the events, each followed by a piece of the open thread's text
def tl.msgs(xs: List<&2, J.Json>, +sel: String, +i: Nat) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> rest:
      msg(x) <> delta(sel, i) <> tl.msgs(rest, sel, 1n+i)

def tl.check(+ty: L.Ty, +n: N.Nui, +w: List<&2, L.TLMemo>, +f: L.TLMemo, +kept: Bool, +what: String, t: TlT) -> TlT:
  TlT.add(t, kept, Eqh.out(MM.Mm.lay(ty, L.TL.memos.now(w), n), MM.Mm.lay(ty, f, n)), what)

def tls(os: List<&2, J.Json>, +ty: L.Ty, +n0: N.Nui, +wm0: List<&2, L.TLMemo>, +mk0: String, f0: L.TLMemo, +tc0: MM.TouchK, t: TlT) -> TlT:
  match os:
    case Nil{}:
      t
    case +o <> rest:
      +n1 = ss.recv(n0, o, Nat.add(U.Side.now(N.Nui.ui(n0)), 1n))
      +tc1 = MM.TouchK.msg(o, tc0)
      +k1 = MM.Mm.tlkey(n1, tc1)
      +same = MM.Mm.tlkey.same(k1, [mk0])
      +w1 = tl.win(same, ty, n1, wm0)
      +f1 = L.Lay.memo(ty, n1, f0)
      tls(rest, ty, n1, w1, k1, f1, tc1, tl.check(ty, n1, w1, f1, same, String.take(J.Json.show(o), 120n), t))

# The open thread's parent and children as the main area's memo keeps them
# (MM.Mm.rels: found again only when its rk changes) against found afresh,
# at every event
def rels.k(same: Bool, +rs: List<&2, String>, +n: N.Nui) -> List<&2, String>:
  match same:
    case True{}:
      rs
    case False{}:
      MM.Mm.rels.of(n)

def rels.go(xs: List<&2, J.Json>, +n0: N.Nui, +tc0: MM.TouchK, +rk0: String, +rs0: List<&2, String>, t: IxT) -> IxT:
  match xs:
    case Nil{}:
      t
    case +x <> rest:
      +o = msg(x)
      +n = ss.recv(n0, o, J.Json.get_nat(x, "at", U.Side.now(N.Nui.ui(n0))))
      +tc = MM.TouchK.msg(o, tc0)
      +rk = MM.Mm.rk(n, tc)
      +rs = rels.k(String.eq(rk, rk0), rs0, n)
      rels.go(rest, n, tc, rk, rs, IxT.add(t, String.eq(String.join(rs, ","), String.join(MM.Mm.rels.of(n), ",")), String.take(J.Json.show(x), 120n)))

# ui.memo timed over the events as the window does it before each frame
# (the timeline's memos brought up, the sidebar's index brought up and
# forced), each event followed by one piece of the open thread's text
def mt.win(same: Bool, +ty: L.Ty, +n: N.Nui, ms: List<&2, L.TLMemo>) -> List<&2, L.TLMemo>:
  match same:
    case True{}:
      MM.Mm.memos.live(ty, n, ms)
    case False{}:
      L.Lay.memos(ty, n, ms)

# Baseline uses the existing pre-PR2 operations: the full timeline memo
# walk and Ix.since over the touch map. Both modes share the event stream,
# state, force/checksum, clock and timer boundaries. This is a measurement
# switch in the replay harness, not a second window implementation.
def mt.memos(baseline: Bool, same: Bool, +ty: L.Ty, +n: N.Nui, ms: List<&2, L.TLMemo>) -> List<&2, L.TLMemo>:
  match baseline:
    case True{}:
      L.Lay.memos(ty, n, ms)
    case False{}:
      mt.win(same, ty, n, ms)

def mt.ix.old(mode: Nat, +k: MM.IxK, +tc: MM.TouchK, +u: U.Ui) -> MM.IxK:
  match mode:
    case 0n:
      k
    case 1n:
      MM.Ix.full(u, MM.TouchK.n(tc))
    case _:
      MM.Ix.inc(MM.IxK.ix(k), MM.Ix.since(Map.to_list(&2, Nat, MM.TouchK.m(tc)), MM.IxK.n(k)), u, MM.TouchK.n(tc), MM.IxK.lt(k))

def mt.ix(baseline: Bool, +k: MM.IxK, +tc: MM.TouchK, +u: U.Ui) -> MM.IxK:
  match baseline:
    case True{}:
      mt.ix.old(MM.Ix.mode(MM.IxK.n(k), MM.IxK.lt(k), tc, u), k, tc, u)
    case False{}:
      MM.Ix.up([k], tc, u)

def mt.touch(baseline: Bool, tc: MM.TouchK, n: Nat) -> MM.TouchK:
  match baseline:
    case True{}:
      tc
    case False{}:
      MM.TouchK.consumed(tc, [n])

def mt.done(+acc: Nat, cs: Nat) -> IO(Nat):
  do IO<Nat>:
    IO.print("ui.memo parts over the events: " ++ Nat.show(acc) ++ " ms in all (check " ++ Nat.show(cs) ++ ")")
    return acc

def mt.key(baseline: Bool, +n: N.Nui, tc: MM.TouchK, mk0: String) -> String:
  match baseline:
    case True{}:
      mk0
    case False{}:
      MM.Mm.tlkey(n, tc)

def mt.step(+baseline: Bool, +ty: L.Ty, +o: J.Json, +n0: N.Nui, +ms0: List<&2, L.TLMemo>, +mk0: String, +k0: MM.IxK, +tc0: MM.TouchK, +acc: Nat, +cs: Nat, rest: List<&2, J.Json>, go: List<&2, J.Json> -> N.Nui -> List<&2, L.TLMemo> -> String -> MM.IxK -> MM.TouchK -> Nat -> Nat -> IO(Nat)) -> IO(Nat):
  do IO<Nat>:
    +n : N.Nui = ss.recv(n0, o, Nat.add(U.Side.now(N.Nui.ui(n0)), 1n))
    +tc : MM.TouchK = MM.TouchK.msg(o, tc0)
    +a : Nat <- IO.now()
    +mk : String = mt.key(baseline, n, tc, mk0)
    +ms : List<&2, L.TLMemo> = mt.memos(baseline, MM.Mm.tlkey.same(mk, [mk0]), ty, n, ms0)
    +z : Nat = L.TL.memo.size(L.TL.memos.now(ms))
    +k : MM.IxK = mt.ix(baseline, k0, tc, N.Nui.ui(n))
    +f : Nat = MM.IxK.force(k)
    +b : Nat <- IO.now()
    go(rest, n, ms, mk, k, mt.touch(baseline, tc, MM.IxK.n(k)), Nat.add(acc, Nat.sub(b, a)), Nat.mod(Nat.add(cs, Nat.add(z, f)), 1000000n))

def mt.go(fuel: Nat, +baseline: Bool, os: List<&2, J.Json>, +ty: L.Ty, +n0: N.Nui, +ms0: List<&2, L.TLMemo>, +mk0: String, +k0: MM.IxK, +tc0: MM.TouchK, +acc: Nat, +cs: Nat) -> IO(Nat):
  match fuel os:
    case 0n _:
      mt.done(acc, cs)
    case 1n+_ Nil{}:
      mt.done(acc, cs)
    case 1n+f +o <> rest:
      mt.step(baseline, ty, o, n0, ms0, mk0, k0, tc0, acc, cs, rest, r => n => ms => mk => k => tc => a => c => mt.go(f, baseline, r, ty, n, ms, mk, k, tc, a, c))

# Clear only after all readers have caught up, including a deliberately
# lagging second reader. Repeated IDs at the same message stamp stay new.
def log.checks() -> IO(Unit):
  do IO<Unit>:
    +tc : MM.TouchK = MM.TouchK{10n, 0n, 0n, Map.new(&2, Nat), [MM.TkE{"a", 10n}, MM.TkE{"b", 10n}, MM.TkE{"a", 8n}, MM.TkE{"old", 2n}]}
    exact(String.eq(String.join(MM.Ix.since.lg(MM.TouchK.lg(tc), 8n), ","), "a,b"), "new-first cutoff")
    exact(Bool.not(List.is_empty(&2, MM.TkE, MM.TouchK.lg(MM.TouchK.consumed(tc, [10n, 8n])))), "lagging reader retains log")
    exact(Bool.not(List.is_empty(&2, MM.TkE, MM.TouchK.lg(MM.TouchK.consumed(tc, Nil{})))), "no reader retains log")
    exact(List.is_empty(&2, MM.TkE, MM.TouchK.lg(MM.TouchK.consumed(tc, [10n, 10n]))), "all readers consumed log")
    IO.print("change log: cutoff and reader retention exact")

def replay(+as: List<&2, String>, +ty: L.Ty, +xs: List<&2, J.Json>, +k: Nat, +cut: Nat, +u: U.Ui) -> IO(Unit):
  do IO<Unit>:
    log.checks()
    +n : N.Nui = prepared(N.Nui.init(u, 1400, 900), 0n)
    +memo : L.TLMemo = L.Lay.memo(ty, n, L.TL.memo.none())
    IO.print("why the key changed (event -> first part that differs):" ++ Why.show(Map.to_list(&2, Nat, why(List.drop(&2, J.Json, xs, cut), n, MM.TouchK.none(), Why.key(n, MM.TouchK.none()), Map.new(&2, Nat)))))
    +t : Tally = steps(List.drop(&2, J.Json, xs, cut), ty, with_hits(n, hits(MM.Mm.lay(ty, memo, n)), 0n), memo, Nil{}, MM.TouchK.none(), Tally{0n, 0n, 0n, 0n, ""})
    IO.print(Tally.show(t))
    exact(Tally.ok(t), "main area")
    +ns : N.Nui = prepared(N.Nui.init(u, 1400, 900), 0n)
    +ms0 : L.TLMemo = L.Lay.memo(ty, ns, L.TL.memo.none())
    +stream : ST = streams(List.drop(&2, J.Json, xs, cut), 0n, ty, with_hits(ns, hits(MM.Mm.lay(ty, ms0, ns)), 0n), ms0, Nil{}, MM.TouchK.none(), ST{0n, 0n, 0n, 0n, 0n, ""})
    IO.print(ST.show(stream))
    exact(ST.ok(stream), "stream")
    +streamix : IxT = ixs.stream(List.drop(&2, J.Json, xs, cut), 0n, arg(as, 1n), u, MM.TouchK.none(), MM.Ix.full(u, 0n), 0n, 0n, IxT{0n, 0n, ""})
    IO.print("streaming, " ++ IxT.show(streamix))
    exact(IxT.ok(streamix), "streaming index")
    +nt : N.Nui = prepared(N.Nui.init(u, 1400, 900), 0n)
    +mt : L.TLMemo = L.Lay.memo(ty, nt, L.TL.memo.none())
    +timeline : TlT = tls(tl.msgs(List.drop(&2, J.Json, xs, cut), arg(as, 1n), 0n), ty, nt, [mt], "", mt, MM.TouchK.none(), TlT{0n, 0n, 0n, ""})
    IO.print(TlT.show(timeline))
    exact(TlT.ok(timeline), "timeline")
    +nr : N.Nui = prepared(N.Nui.init(u, 1400, 900), 0n)
    +relations : IxT = rels.go(List.drop(&2, J.Json, xs, cut), nr, MM.TouchK.none(), "", Nil{}, IxT{0n, 0n, ""})
    IO.print("relations, " ++ IxT.show(relations))
    exact(IxT.ok(relations), "relations")
    +nm : N.Nui = prepared(N.Nui.init(u, 1400, 900), 0n)
    +mm0 : L.TLMemo = L.Lay.memo(ty, nm, L.TL.memo.none())
    tm : Nat <- mt.go(Nat.mul(2n, k), False{}, tl.msgs(List.drop(&2, J.Json, xs, cut), arg(as, 1n), 0n), ty, nm, [mm0], "", MM.Ix.full(u, 0n), MM.TouchK.none(), 0n, 0n)
    +index : IxT = ixsteps(List.drop(&2, J.Json, xs, cut), u, MM.TouchK.none(), MM.Ix.full(u, 0n), IxT{0n, 0n, ""})
    IO.print(IxT.show(index))
    exact(IxT.ok(index), "index")
    +n2 : N.Nui = prepared(N.Nui.init(u, 1400, 900), 0n)
    +n3 : N.Nui = prepared(N.Nui.init(u, 1400, 900), 0n)
    +sidebar : Side = sides(List.drop(&2, J.Json, xs, cut), ty, n3, MM.TouchK.none(), Nil{}, Side{0n, 0n, 0n})
    IO.print(Side.show(sidebar))
    exact(Side.ok(sidebar), "sidebar")
    +typed : Side = typing("typing into the composer, and the pointer moving about", ty, n2, MM.TouchK.none(), [MM.SideM.fresh(MM.Sm.key(n2, MM.TouchK.none()), side.fresh(ty, n2))], Side{0n, 0n, 0n})
    IO.print(Side.show(typed))
    exact(Side.ok(typed), "typed")

# Optional fourth argument selects timing only; correctness remains the
# default. A/B runs use one binary and alternate baseline/current modes.
def timed(baseline: Bool, +ty: L.Ty, +xs: List<&2, J.Json>, +k: Nat, +u: U.Ui) -> IO(Unit):
  do IO<Unit>:
    +n : N.Nui = prepared(N.Nui.init(u, 1400, 900), 0n)
    +memo : L.TLMemo = L.Lay.memo(ty, n, L.TL.memo.none())
    tm : Nat <- mt.go(Nat.mul(2n, k), baseline, tl.msgs(xs, U.Ui.sel(u), 0n), ty, n, [memo], "", MM.Ix.full(u, 0n), MM.TouchK.none(), 0n, 0n)
    return Unit{}

def run(mode: String, +as: List<&2, String>, +ty: L.Ty, +xs: List<&2, J.Json>, +k: Nat, +cut: Nat, +u: U.Ui) -> IO(Unit):
  match mode:
    case "baseline":
      timed(True{}, ty, List.drop(&2, J.Json, xs, cut), k, u)
    case "current":
      timed(False{}, ty, List.drop(&2, J.Json, xs, cut), k, u)
    case _:
      replay(as, ty, xs, k, cut, u)

def main() -> IO(Unit):
  do IO<Unit>:
    args : List<String> <- AV.Argv.args()
    +as : List<&2, String> = copy(args)
    +ty : L.Ty <- L.Ty.load()
    bs : List<&2, U32> <- read(arg(as, 0n))
    +xs : List<&2, J.Json> = items(lines(U8.Lines.feed(U8.Lines.new(), bs)), Nil{})
    +k : Nat = Maybe.default(&2, Nat, Nat.read(arg(as, 2n)), 300n)
    +cut : Nat = Nat.sub(List.length(&2, J.Json, xs), k)
    +u : U.Ui = real(List.take(&2, J.Json, xs, cut), arg(as, 1n))
    run(arg(as, 3n), as, ty, xs, k, cut, u)
