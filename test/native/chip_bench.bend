import Base
import ../../src/core/client.bend as U
import ../../src/core/model.bend as M
import ../../src/core/codec.bend as C
import ../../src/core/json.bend as J
import ../../src/web/dom.bend as DM
import ../../src/app/nui.bend as N
import ../../src/app/layout.bend as L
import ./memo_replay.bend as MR
import ../../src/core/ref.bend as RF
import ../../src/core/refview.bend as RV
import ../../src/core/chip.bend as CH
import ../../src/core/links.bend as LK
import ../../src/server/argv.bend as AV
import ../../src/core/utf8.bend as U8
import ../../src/core/images.bend as IM

# Where the time of a chip-heavy memo miss goes: the 200-ref thread of
# memo_replay's chip case, one stage at a time (ms, repeated).
#   scripts/build-app.sh test/native/chip_bench.bend build/chip_bench && build/chip_bench

def texts(es: List<&2, M.Entry>) -> List<&2, String>:
  match es:
    case Nil{}:
      Nil{}
    case M.Msg{_, _, _, +t, _} <> r:
      t <> texts(r)
    case _ <> r:
      texts(r)

def nodes(ts: List<&2, String>) -> List<&2, List<&2, DM.Node>>:
  match ts:
    case Nil{}:
      Nil{}
    case +t <> r:
      DM.Md.render(t) <> nodes(r)

def lines.on(+ty: L.Ty, +st: M.State, ns: List<&2, List<&2, DM.Node>>, +acc: Nat) -> Nat:
  match ns:
    case Nil{}:
      acc
    case +n <> r:
      lines.on(ty, st, r, Nat.add(acc, List.length(&2, L.TLine, L.TL.rblocks(ty, n, L.Rz.blocks(st, n), 900))))

def lines.off(+ty: L.Ty, ns: List<&2, List<&2, DM.Node>>, +acc: Nat) -> Nat:
  match ns:
    case Nil{}:
      acc
    case +n <> r:
      lines.off(ty, r, Nat.add(acc, List.length(&2, L.TLine, L.TL.blocks(ty, n, 900))))

def deps(+st: M.State, ns: List<&2, List<&2, DM.Node>>, +acc: Nat) -> Nat:
  match ns:
    case Nil{}:
      acc
    case +n <> r:
      deps(st, r, Nat.add(acc, List.length(&2, String, L.Rz.deps(L.Rz.blocks(st, n)))))

def plen(+p: L.TLPart) -> Nat:
  match p:
    case L.TLPart{+h, +ps, +t, +d, _, _}:
      Nat.add(Nat.add(List.length(&2, L.TLine, h), List.length(&2, String, ps)), Nat.add(List.length(&2, L.TLine, t), List.length(&2, String, d)))
    case L.TLCached{+h, +ps, +t, +d, _, _, _}:
      Nat.add(Nat.add(List.length(&2, L.TLine, h), List.length(&2, String, ps)), Nat.add(List.length(&2, L.TLine, t), List.length(&2, String, d)))

def items.len(its: List<&2, L.TLE>, +acc: Nat) -> Nat:
  match its:
    case Nil{}:
      acc
    case L.TLE{_, +p} <> r:
      items.len(r, Nat.add(acc, plen(p)))

def memo.items(m: L.TLMemo) -> List<&2, L.TLE>:
  match m:
    case L.TLMemo{_, _, +its, _}:
      its

def paths(ts: List<&2, String>, +acc: Nat) -> Nat:
  match ts:
    case Nil{}:
      acc
    case +t <> r:
      paths(r, Nat.add(acc, List.length(&2, String, L.TL.imgs(t))))

def parts.new(+ty: L.Ty, +st: M.State, es: List<&2, M.Entry>, +acc: Nat) -> Nat:
  match es:
    case Nil{}:
      acc
    case +e <> +r:
      parts.new(ty, st, r, Nat.add(acc, plen(L.TL.part.own.c(ty, st, e, 900))))

def parts.old(+ty: L.Ty, es: List<&2, M.Entry>, +acc: Nat) -> Nat:
  match es:
    case Nil{}:
      acc
    case +e <> +r:
      parts.old(ty, r, Nat.add(acc, plen(L.TL.part.own(ty, e, 900))))

def uris(+i: Nat, acc: List<&2, String>) -> List<&2, String>:
  match i:
    case 0n:
      acc
    case 1n+j:
      uris(j, MR.Ch.ref(j) <> acc)

def some.n(+m: Maybe<&2, RF.Ref>) -> Nat:
  match m:
    case None{}:
      0n
    case Some{_}:
      1n

def some.c(+m: Maybe<&2, RV.Card>) -> Nat:
  match m:
    case None{}:
      0n
    case Some{_}:
      1n

def parsed(us: List<&2, String>, +acc: Nat) -> Nat:
  match us:
    case Nil{}:
      acc
    case +u <> +r:
      parsed(r, Nat.add(acc, some.n(RF.Ref.parse(u))))

def cards.one(+st: M.State, +m: Maybe<&2, RF.Ref>) -> Nat:
  match m:
    case None{}:
      0n
    case Some{+r}:
      some.c(RV.Card.of(st, r))

def cards.of(+st: M.State, us: List<&2, String>, +acc: Nat) -> Nat:
  match us:
    case Nil{}:
      acc
    case +u <> +r:
      cards.of(st, r, Nat.add(acc, cards.one(st, RF.Ref.parse(u))))

def cp.len(+cp: L.Cp) -> Nat:
  match cp:
    case L.Cp{+t, _, _, _}:
      String.length(t)

def chips.of(+st: M.State, us: List<&2, String>, +acc: Nat) -> Nat:
  match us:
    case Nil{}:
      acc
    case +u <> +r:
      chips.of(st, r, Nat.add(acc, cp.len(L.Cp.piece(st, LK.PRef{u, u, False{}}))))

def cps.force(cs: List<&2, L.Cp>, +acc: Nat) -> Nat:
  match cs:
    case Nil{}:
      acc
    case L.Cp{+t, +l, +c, +d} <> +r:
      cps.force(r, Nat.add(acc, Nat.add(Nat.add(String.length(t), String.length(l)), Nat.add(String.length(c), List.length(&2, String, d)))))

def rz.force(rz: List<&2, List<&2, L.Cp>>, +acc: Nat) -> Nat:
  match rz:
    case Nil{}:
      acc
    case +cs <> +r:
      rz.force(r, cps.force(cs, acc))

def cps.wrap(+ty: L.Ty, cs: List<&2, L.Cp>, +acc: Nat) -> Nat:
  match cs:
    case Nil{}:
      acc
    case L.Cp{+t, _, _, _} <> +r:
      cps.wrap(ty, r, Nat.add(acc, List.length(&2, String, L.Ty.wrap(ty, 3n, t, 900))))

def rz.wrap(+ty: L.Ty, rz: List<&2, List<&2, L.Cp>>, +acc: Nat) -> Nat:
  match rz:
    case Nil{}:
      acc
    case +cs <> +r:
      rz.wrap(ty, r, cps.wrap(ty, cs, acc))

def cps.dec(cs: List<&2, L.Cp>, +acc: Nat) -> Nat:
  match cs:
    case Nil{}:
      acc
    case L.Cp{_, _, +c, _} <> +r:
      cps.dec(r, Nat.add(acc, List.length(&2, CH.Chip, CH.Chip.dec(c))))

def rz.dec(rz: List<&2, List<&2, L.Cp>>, +acc: Nat) -> Nat:
  match rz:
    case Nil{}:
      acc
    case +cs <> +r:
      rz.dec(r, cps.dec(cs, acc))

def rzs.of(+st: M.State, ns: List<&2, List<&2, DM.Node>>) -> List<&2, List<&2, List<&2, L.Cp>>>:
  match ns:
    case Nil{}:
      Nil{}
    case +n <> +r:
      L.Rz.blocks(st, n) <> rzs.of(st, r)

def rzs.force(rs: List<&2, List<&2, List<&2, L.Cp>>>, +acc: Nat) -> Nat:
  match rs:
    case Nil{}:
      acc
    case +z <> +r:
      rzs.force(r, rz.force(z, acc))

def rzs.lay(+ty: L.Ty, ns: List<&2, List<&2, DM.Node>>, rs: List<&2, List<&2, List<&2, L.Cp>>>, +acc: Nat) -> Nat:
  match ns rs:
    case +n <> +nr +z <> +r:
      rzs.lay(ty, nr, r, Nat.add(acc, List.length(&2, L.TLine, L.TL.rblocks(ty, n, z, 900))))
    case _ _:
      acc

def rzs.wrap(+ty: L.Ty, rs: List<&2, List<&2, List<&2, L.Cp>>>, +acc: Nat) -> Nat:
  match rs:
    case Nil{}:
      acc
    case +z <> +r:
      rzs.wrap(ty, r, rz.wrap(ty, z, acc))

def rzs.dec(rs: List<&2, List<&2, List<&2, L.Cp>>>, +acc: Nat) -> Nat:
  match rs:
    case Nil{}:
      acc
    case +z <> +r:
      rzs.dec(r, rz.dec(z, acc))

def runs.one(+ty: L.Ty, +l: String, +e: String, +mode: Nat) -> Nat:
  match mode:
    case 0n:
      List.length(&2, CH.Run, CH.Chip.cut(l, e))
    case _:
      String.length(L.TL.dr.of(ty, l, e))

def runs.len(+ty: L.Ty, ls: List<&2, String>, es: List<&2, String>, +mode: Nat, +acc: Nat) -> Nat:
  match ls es:
    case +l <> +lt +e <> +et:
      runs.len(ty, lt, et, mode, Nat.add(acc, runs.one(ty, l, e, mode)))
    case _ _:
      acc

def cps.runs.k(+ty: L.Ty, +w: List<&2, String>, +l: String, +c: String, +mode: Nat) -> Nat:
  runs.len(ty, w, CH.Chip.encs(w, l, CH.Chip.dec(c)), mode, 0n)

def cps.runs(+ty: L.Ty, cs: List<&2, L.Cp>, +mode: Nat, +acc: Nat) -> Nat:
  match cs:
    case Nil{}:
      acc
    case L.Cp{+t, +l, +c, _} <> +r:
      cps.runs(ty, r, mode, Nat.add(acc, cps.runs.k(ty, L.Ty.wrap(ty, 3n, t, 900), l, c, mode)))

def rz.runs(+ty: L.Ty, rz: List<&2, List<&2, L.Cp>>, +mode: Nat, +acc: Nat) -> Nat:
  match rz:
    case Nil{}:
      acc
    case +cs <> +r:
      rz.runs(ty, r, mode, cps.runs(ty, cs, mode, acc))

def rzs.runs(+ty: L.Ty, rs: List<&2, List<&2, List<&2, L.Cp>>>, +mode: Nat, +acc: Nat) -> Nat:
  match rs:
    case Nil{}:
      acc
    case +z <> +r:
      rzs.runs(ty, r, mode, rz.runs(ty, z, mode, acc))

def cps(+st: M.State, ns: List<&2, List<&2, DM.Node>>, +acc: Nat) -> Nat:
  match ns:
    case Nil{}:
      acc
    case +n <> r:
      cps(st, r, Nat.add(acc, List.length(&2, List<&2, L.Cp>, L.Rz.blocks(st, n))))

def main() -> IO(Unit):
  do IO<Unit>:
    +ty : L.Ty <- L.Ty.load()
    args : List<String> <- AV.Argv.args()
    +as : List<&2, String> = MR.copy(args)
    bs : List<&2, U32> <- MR.read(MR.arg(as, 0n))
    +xs : List<&2, J.Json> = MR.items(MR.lines(U8.Lines.feed(U8.Lines.new(), bs)), Nil{})
    +u : U.Ui = MR.real(List.append(&2, J.Json, xs, MR.Ch.json(MR.Ch.setup())), "c")
    +st : M.State = U.Ui.state(u)
    +es : List<&2, M.Entry> = M.State.mine(st, "c")
    +ns : List<&2, List<&2, DM.Node>> = nodes(texts(es))
    t0 : Nat <- IO.now()
    +a : Nat = List.length(&2, List<&2, DM.Node>, ns)
    +t1 : Nat <- IO.now()
    +b : Nat = lines.off(ty, ns, 0n)
    +t2 : Nat <- IO.now()
    +c : Nat = lines.on(ty, st, ns, 0n)
    +t3 : Nat <- IO.now()
    +d : Nat = deps(st, ns, 0n)
    +t4 : Nat <- IO.now()
    +e : Nat = cps(st, ns, 0n)
    +t5 : Nat <- IO.now()
    +f : Nat = items.len(L.TL.cm.make(ty, es, st, 900, "c"), 0n)
    +t6 : Nat <- IO.now()
    +g : Nat = paths(texts(es), 0n)
    +t7 : Nat <- IO.now()
    +h : Nat = parts.new(ty, st, es, 0n)
    +t8 : Nat <- IO.now()
    +rs : List<&2, List<&2, List<&2, L.Cp>>> = rzs.of(st, ns)
    +s1 : Nat = rzs.force(rs, 0n)
    +t8r : Nat <- IO.now()
    +s2 : Nat = rzs.lay(ty, ns, rs, 0n)
    +t8l : Nat <- IO.now()
    +s3 : Nat = rzs.wrap(ty, rs, 0n)
    +t8w : Nat <- IO.now()
    +s4 : Nat = rzs.dec(rs, 0n)
    +t8e : Nat <- IO.now()
    +q1 : Nat = rzs.runs(ty, rs, 0n, 0n)
    +t8q : Nat <- IO.now()
    +q2 : Nat = rzs.runs(ty, rs, 1n, 0n)
    +t8s : Nat <- IO.now()
    +us : List<&2, String> = uris(200n, Nil{})
    +m1a : Nat = parsed(us, 0n)
    +t8p : Nat <- IO.now()
    +m1b : Nat = cards.of(st, us, 0n)
    +t8c : Nat <- IO.now()
    +m1c : Nat = chips.of(st, us, 0n)
    +t8d : Nat <- IO.now()
    +h2 : Nat = parts.old(ty, es, 0n)
    +t8b : Nat <- IO.now()
    +n0 : N.Nui = MR.prepared(N.Nui.init(u, 1400, 900), 0n)
    +m1 : L.TLMemo = L.Lay.memo(ty, n0, L.TL.memo.none())
    +k1 : Nat = items.len(memo.items(m1), 0n)
    +t9 : Nat <- IO.now()
    +tz0 : Nat <- IO.now()
    +mz : L.TLMemo = L.Lay.memo.d(ty, n0, ["*"], m1)
    +kz : Nat = List.length(&2, L.TLE, memo.items(mz))
    +tz1 : Nat <- IO.now()
    +kz2 : Nat = items.len(memo.items(mz), 0n)
    +tz2 : Nat <- IO.now()
    IO.print("Lay.memo dirty, spine only " ++ Nat.show(Nat.sub(tz1, tz0)) ++ " ms (" ++ Nat.show(kz) ++ "), then the parts forced " ++ Nat.show(Nat.sub(tz2, tz1)) ++ " ms (" ++ Nat.show(kz2) ++ ")")
    +m2 : L.TLMemo = L.Lay.memo.d(ty, n0, ["*"], m1)
    +k2 : Nat = items.len(memo.items(m2), 0n)
    +t10 : Nat <- IO.now()
    +m3 : L.TLMemo = L.Lay.memo.d(ty, n0, ["*"], m2)
    +k3 : Nat = items.len(memo.items(m3), 0n)
    t11 : Nat <- IO.now()
    IO.print("render " ++ Nat.show(Nat.sub(t1, t0)) ++ " ms (" ++ Nat.show(a) ++ " messages)")
    IO.print("blocks plain " ++ Nat.show(Nat.sub(t2, t1)) ++ " ms, " ++ Nat.show(b) ++ " lines")
    IO.print("blocks chips " ++ Nat.show(Nat.sub(t3, t2)) ++ " ms, " ++ Nat.show(c) ++ " lines")
    IO.print("deps         " ++ Nat.show(Nat.sub(t4, t3)) ++ " ms, " ++ Nat.show(d) ++ " ids")
    IO.print("Nd.cp only   " ++ Nat.show(Nat.sub(t5, t4)) ++ " ms, " ++ Nat.show(e) ++ " chars")
    IO.print("memo.make    " ++ Nat.show(Nat.sub(t6, t5)) ++ " ms, " ++ Nat.show(f) ++ " items")
    IO.print("Img.paths    " ++ Nat.show(Nat.sub(t7, t6)) ++ " ms, " ++ Nat.show(g) ++ " paths")
    IO.print("part.own.c   " ++ Nat.show(Nat.sub(t8, t7)) ++ " ms, " ++ Nat.show(h))
    IO.print("Rz (resolve+force) " ++ Nat.show(Nat.sub(t8r, t8)) ++ " ms " ++ Nat.show(s1) ++ "; rblocks from Rz " ++ Nat.show(Nat.sub(t8l, t8r)) ++ " ms " ++ Nat.show(s2) ++ "; wrap of the texts alone " ++ Nat.show(Nat.sub(t8w, t8l)) ++ " ms " ++ Nat.show(s3) ++ "; Chip.dec alone " ++ Nat.show(Nat.sub(t8e, t8w)) ++ " ms " ++ Nat.show(s4))
    IO.print("wrap+encs+Chip.cut " ++ Nat.show(Nat.sub(t8q, t8e)) ++ " ms (" ++ Nat.show(q1) ++ "); wrap+encs+TL.dr.of (cut, widths, strings) " ++ Nat.show(Nat.sub(t8s, t8q)) ++ " ms (" ++ Nat.show(q2) ++ ")")
    IO.print("200 refs: parse " ++ Nat.show(Nat.sub(t8p, t8s)) ++ " ms, parse+Card.of " ++ Nat.show(Nat.sub(t8c, t8p)) ++ " ms, full resolve (parse, card, chip, cp) " ++ Nat.show(Nat.sub(t8d, t8c)) ++ " ms " ++ Nat.show(Nat.add(m1a, Nat.add(m1b, m1c))))
    IO.print("part.own old " ++ Nat.show(Nat.sub(t8b, t8d)) ++ " ms, " ++ Nat.show(h2))
    IO.print("Lay.memo new  " ++ Nat.show(Nat.sub(t9, t8b)) ++ " ms, " ++ Nat.show(k1))
    IO.print("Lay.memo dirty " ++ Nat.show(Nat.sub(t10, t9)) ++ " ms, " ++ Nat.show(k2))
    IO.print("Lay.memo dirty " ++ Nat.show(Nat.sub(t11, t10)) ++ " ms, " ++ Nat.show(k3))
