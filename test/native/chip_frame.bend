import Base
import ../../src/core/client.bend as U
import ../../src/core/json.bend as J
import ../../src/core/utf8.bend as U8
import ../../src/app/nui.bend as N
import ../../src/app/layout.bend as L
import ../../src/gfx/raster.bend as R
import ../../src/server/argv.bend as AV
import ../../src/core/model.bend as M
import ../../src/core/chip.bend as CH
import ../../src/app/chipx.bend as CX

# The window's frame of a thread of chips (links spec 3c): the memo of the
# thread named on the command line, then its main area laid out again and
# again (every frame during streaming does), timed. Builds against main
# too (the same calls), so the same log gives both sides.
#   scripts/build-app.sh test/native/chip_frame.bend build/chip_frame
#   build/chip_frame EVENTS.jsonl THREAD [REPEAT]

def read.k(r: File & Result<&1, &1, U32 & String, List<&2, U32>>) -> IO(List<&2, U32>):
  (f, res) = r
  do IO<List<&2, U32>>:
    File.close(f)
    bs : List<&2, U32> <- IO.pass(List<&2, U32>, res)
    return bs

def read(+path: String) -> IO(List<&2, U32>):
  do IO<List<&2, U32>>:
    f : File <- IO.try(File, File.open(path, "r"))
    r : File & Result<&1, &1, U32 & String, List<&2, U32>> <- File.read_bytes(f, 400000000)
    read.k(r)

def copy(xs: List<&1, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      x <> copy(t)

def arg(+xs: List<&2, String>, +i: Nat) -> String:
  Maybe.default(&2, String, List.head(&2, String, List.drop(&2, String, xs, i)), "")

def lines(r: U8.Lines & List<&2, String>) -> List<&2, String>:
  (_, ls) = r
  ls

def items(ls: List<&2, String>, acc: List<&2, J.Json>) -> List<&2, J.Json>:
  match ls:
    case Nil{}:
      List.reverse(&2, J.Json, acc)
    case l <> t:
      items(t, Maybe.default(&2, J.Json, J.Json.parse(l), J.Null{}) <> acc)

def ui_sel(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}:
      u

def ops(o: L.Out) -> Nat:
  match o:
    case L.Out{os, hs}:
      Nat.add(List.length(&2, R.Op, os), List.length(&2, N.Hit, hs))

def frames(n: Nat, +ty: L.Ty, +memo: L.TLMemo, +nui: N.Nui, +acc: Nat) -> Nat:
  match n:
    case 0n:
      acc
    case 1n+k:
      frames(k, ty, memo, nui, Nat.add(acc, ops(L.Lay.frame(ty, memo, nui))))

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

def rz.n(rz: List<&2, List<&2, L.Cp>>, +acc: Nat) -> Nat:
  match rz:
    case Nil{}:
      acc
    case +cs <> +r:
      rz.n(r, Nat.add(acc, List.length(&2, L.Cp, cs)))

def rzs.n(+st: M.State, es: List<&2, M.Entry>, +acc: Nat) -> Nat:
  match es:
    case Nil{}:
      acc
    case +e <> +r:
      rzs.n(st, r, Nat.add(acc, rz.n(L.Rz.entry(st, e), 0n)))

def lay.dr(+ty: L.Ty, ls: List<&2, String>, rss: List<&2, List<&2, CH.Run>>, +acc: Nat) -> Nat:
  match ls rss:
    case +l <> +t +rs <> +rt:
      lay.dr(ty, t, rt, Nat.add(acc, String.length(L.TL.dr.runs(ty, rs))))
    case _ _:
      acc

def lay.k(+ty: L.Ty, +w: List<&2, String>, +c: String, +mode: Nat) -> Nat:
  match mode:
    case 0n:
      List.length(&2, String, w)
    case 1n:
      Nat.add(List.length(&2, String, w), List.length(&2, CH.Chip, CH.Chip.dec(c)))
    case 2n:
      List.length(&2, List<&2, CH.Run>, CH.Chip.lines(w, CH.Chip.dec(c)))
    case _:
      lay.dr(ty, w, CH.Chip.lines(w, CH.Chip.dec(c)), 0n)

def lay.measure(+ty: L.Ty, +t: String, +c: String, mode: Nat) -> Nat:
  match mode:
    case 4n:
      List.length(&2, L.ChipMeasure, L.TL.chip.measures(ty, CH.Chip.dec(c)))
    case 5n:
      List.length(&2, String, L.TL.chip.wrap.lines(String.split(t, '\n'), ty, L.TL.chip.measures(ty, CH.Chip.dec(c)), 900))
    case _:
      List.length(&2, L.TLine, L.TL.rpara(ty, L.Cp{t, "", c, Nil{}}, 900))

def lay.cp.choose(old: Bool, +ty: L.Ty, +t: String, +c: String, +mode: Nat) -> Nat:
  match old:
    case True{}:
      lay.k(ty, L.Ty.wrap(ty, 3n, t, 900), c, mode)
    case False{}:
      lay.measure(ty, t, c, mode)

def lay.cp(+ty: L.Ty, +cp: L.Cp, +mode: Nat) -> Nat:
  match cp:
    case L.Cp{+t, _, +c, _}:
      lay.cp.choose(Nat.is_lt(mode, 4n), ty, t, c, mode)

def lay.cps(+ty: L.Ty, cs: List<&2, L.Cp>, +mode: Nat, +acc: Nat) -> Nat:
  match cs:
    case Nil{}:
      acc
    case +c <> +r:
      lay.cps(ty, r, mode, Nat.add(acc, lay.cp(ty, c, mode)))

def lay.rz(+ty: L.Ty, rz: List<&2, List<&2, L.Cp>>, +mode: Nat, +acc: Nat) -> Nat:
  match rz:
    case Nil{}:
      acc
    case +cs <> +r:
      lay.rz(ty, r, mode, lay.cps(ty, cs, mode, acc))

def lay.es(+ty: L.Ty, +st: M.State, es: List<&2, M.Entry>, +mode: Nat, +acc: Nat) -> Nat:
  match es:
    case Nil{}:
      acc
    case +e <> +r:
      lay.es(ty, st, r, mode, lay.rz(ty, L.Rz.entry(st, e), mode, acc))

def timed(+n: Nat, +ty: L.Ty, +memo: L.TLMemo, +nui: N.Nui) -> IO(Unit):
  do IO<Unit>:
    +t0 : Nat <- IO.now()
    +k : Nat = frames(n, ty, memo, nui, 0n)
    +t1 : Nat <- IO.now()
    IO.print("frames " ++ Nat.show(n) ++ ": " ++ Nat.show(Nat.sub(t1, t0)) ++ " ms total, " ++ Nat.show(Nat.div(Nat.sub(t1, t0), n)) ++ " ms each (" ++ Nat.show(Nat.div(k, n)) ++ " ops and hits a frame)")

def summary(n: Nat, +m: L.TLMemo, +acc: Nat) -> Nat:
  match n:
    case 0n: acc
    case 1n+k: summary(k, m, Nat.add(acc, List.length(&2, String, CX.Cache.memo(m))))

def scan(es: List<&2, M.Entry>, +sel: String, acc: Nat) -> Nat:
  match es:
    case Nil{}: acc
    case e <> t: scan(t, sel, Nat.add(acc, Bool.pick(Nat, L.TL.cm.chip(e, sel), 1n, 0n)))

def scans(n: Nat, +es: List<&2, M.Entry>, +sel: String, +acc: Nat) -> Nat:
  match n:
    case 0n: acc
    case 1n+k: scans(k, es, sel, Nat.add(acc, scan(es, sel, 0n)))

def memo.items(m: L.TLMemo) -> List<&2, L.TLE>:
  match m:
    case L.TLMemo{_, _, xs, _}: xs

def redo(n: Nat, +ty: L.Ty, +nui: N.Nui, +dirty: List<&2, String>, +memo: L.TLMemo, +acc: Nat) -> Nat:
  match n:
    case 0n: acc
    case 1n+k:
      +m = L.Lay.memo.d(ty, nui, dirty, memo)
      redo(k, ty, nui, dirty, m, Nat.add(acc, Nat.add(items.len(memo.items(m), 0n), List.length(&2, String, CX.Cache.memo(m)))))

def invalidation(+ty: L.Ty, +nui: N.Nui, +memo: L.TLMemo, target: String) -> IO(Unit):
  do IO<Unit>:
    +t0 : Nat <- IO.now()
    +a : Nat = summary(20n, memo, 0n)
    +t1 : Nat <- IO.now()
    +b : Nat = scans(20n, L.Lay.memo.es(False{}, nui), U.Ui.sel(N.Nui.ui(nui)), 0n)
    +t2 : Nat <- IO.now()
    +c : Nat = redo(20n, ty, nui, ["unrelated"], memo, 0n)
    +t3 : Nat <- IO.now()
    +d : Nat = redo(20n, ty, nui, [target], memo, 0n)
    +t4 : Nat <- IO.now()
    +e : Nat = redo(20n, ty, nui, ["*"], memo, 0n)
    +t5 : Nat <- IO.now()
    IO.print("invalidation x20 ms: summary " ++ Nat.show(Nat.sub(t1, t0)) ++ ", recognize " ++ Nat.show(Nat.sub(t2, t1)) ++ ", unrelated " ++ Nat.show(Nat.sub(t3, t2)) ++ ", one target " ++ Nat.show(Nat.sub(t4, t3)) ++ ", all targets " ++ Nat.show(Nat.sub(t5, t4)) ++ "; check " ++ Nat.show(Nat.add(a, Nat.add(b, Nat.add(c, Nat.add(d, e))))))

def main() -> IO(Unit):
  do IO<Unit>:
    args : List<String> <- AV.Argv.args()
    +as : List<&2, String> = copy(args)
    bs : List<&2, U32> <- read(arg(as, 0n))
    +xs : List<&2, J.Json> = items(lines(U8.Lines.feed(U8.Lines.new(), bs)), Nil{})
    +rep : Nat = Maybe.default(&2, Nat, Nat.read(arg(as, 2n)), 40n)
    +ty : L.Ty <- L.Ty.load()
    +u : U.Ui = ui_sel(U.Ui.act(U.Ui.recv(U.Ui.init(1790180000n), J.Json.obj([J.KV{"t", J.Str{"log"}}, J.KV{"version", J.Str{"0.1.0"}}, J.KV{"items", J.Json.arr(xs)}])), "select", arg(as, 1n)))
    +nui : N.Nui = N.Nui.init(u, 1400, 900)
    +t0 : Nat <- IO.now()
    +memo : L.TLMemo = L.Lay.memo(ty, nui, L.TL.memo.none())
    +k0 : Nat = L.TL.memo.size(memo)
    +t1 : Nat <- IO.now()
    IO.print("memo of " ++ Nat.show(k0) ++ " entries: " ++ Nat.show(Nat.sub(t1, t0)) ++ " ms")
    +es : List<&2, M.Entry> = L.Lay.memo.es(False{}, nui)
    +st : M.State = U.Ui.state(N.Nui.ui(nui))
    +ta : Nat <- IO.now()
    +ka : Nat = items.len(L.TL.memo.make(ty, es, 900, U.Ui.sel(N.Nui.ui(nui))), 0n)
    +tb : Nat <- IO.now()
    +kb : Nat = rzs.n(st, es, 0n)
    +tc : Nat <- IO.now()
    +kc : Nat = items.len(L.TL.cm.make(ty, es, st, 900, U.Ui.sel(N.Nui.ui(nui))), 0n)
    +td : Nat <- IO.now()
    +kd : Nat = items.len(L.TL.cm.make(ty, es, st, 900, U.Ui.sel(N.Nui.ui(nui))), 0n)
    +te : Nat <- IO.now()
    +tf : Nat <- IO.now()
    +m0 : Nat = lay.es(ty, st, es, 0n, 0n)
    +tg : Nat <- IO.now()
    +m1 : Nat = lay.es(ty, st, es, 1n, 0n)
    +th : Nat <- IO.now()
    +m2 : Nat = lay.es(ty, st, es, 2n, 0n)
    +ti : Nat <- IO.now()
    +m3 : Nat = lay.es(ty, st, es, 3n, 0n)
    +tj : Nat <- IO.now()
    +ma : Nat = lay.es(ty, st, es, 4n, 0n)
    +tk : Nat <- IO.now()
    +mb : Nat = lay.es(ty, st, es, 5n, 0n)
    +tl : Nat <- IO.now()
    +mc : Nat = lay.es(ty, st, es, 6n, 0n)
    +tm : Nat <- IO.now()
    IO.print("new resolution+measure " ++ Nat.show(Nat.sub(tk, tj)) ++ " ms; +wrap " ++ Nat.show(Nat.sub(tl, tk)) ++ " ms; +lines " ++ Nat.show(Nat.sub(tm, tl)) ++ " ms " ++ Nat.show(Nat.add(ma, Nat.add(mb, mc))))
    IO.print("resolution+wrap " ++ Nat.show(Nat.sub(tg, tf)) ++ " ms; +Chip.dec " ++ Nat.show(Nat.sub(th, tg)) ++ " ms (cum, each re-resolves); +Chip.lines " ++ Nat.show(Nat.sub(ti, th)) ++ " ms; +dr runs " ++ Nat.show(Nat.sub(tj, ti)) ++ " ms " ++ Nat.show(Nat.add(m0, Nat.add(m1, Nat.add(m2, m3)))))
    IO.print("old path " ++ Nat.show(Nat.sub(tb, ta)) ++ " ms (" ++ Nat.show(ka) ++ "), resolution alone " ++ Nat.show(Nat.sub(tc, tb)) ++ " ms (" ++ Nat.show(kb) ++ "), chip path " ++ Nat.show(Nat.sub(td, tc)) ++ " ms (" ++ Nat.show(kc) ++ "), again " ++ Nat.show(Nat.sub(te, td)) ++ " ms (" ++ Nat.show(kd) ++ ")")
    invalidation(ty, nui, memo, arg(as, 3n))
    timed(1n, ty, memo, nui)
    timed(rep, ty, memo, nui)
    timed(rep, ty, memo, nui)
