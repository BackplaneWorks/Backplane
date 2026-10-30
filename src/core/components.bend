import Base
import ./sexpr.bend as S
import ./board.bend as B
import ./schematic.bend as SC
import ./json.bend as J
import ./utf8.bend as U

# Read-only component metadata from saved CAD. UUID/path associations are
# explicit; neither reference proximity nor mesh RGB is an identity. Symbol
# previews use the schematic's own cached library and instantiated unit.

def Components.num(x: F32) -> J.Json:
  J.Num{F32.show(x)}

def Components.pt(p: B.Pt) -> J.Json:
  match p:
    case B.Pt{x, y}: J.Json.arr([Components.num(x), Components.num(y)])

def Components.pts(ps: List<&2, B.Pt>) -> List<&2, J.Json>:
  match ps:
    case Nil{}: Nil{}
    case p <> t: Components.pt(p) <> Components.pts(t)

def Components.prim(p: B.Prim) -> J.Json:
  match p:
    case B.PSeg{a, b, w}:
      J.Json.obj([J.KV{"kind", J.Str{"line"}}, J.KV{"points", J.Json.arr([Components.pt(a), Components.pt(b)])}, J.KV{"width", Components.num(w)}])
    case B.PLine{ps, w}:
      J.Json.obj([J.KV{"kind", J.Str{"line"}}, J.KV{"points", J.Json.arr(Components.pts(ps))}, J.KV{"width", Components.num(w)}])
    case B.PCirc{c, r, w, f}:
      J.Json.obj([J.KV{"kind", J.Str{"circle"}}, J.KV{"center", Components.pt(c)}, J.KV{"radius", Components.num(r)}, J.KV{"width", Components.num(w)}, J.KV{"fill", J.Flag{f}}])
    case B.PPoly{ps, w, f}:
      J.Json.obj([J.KV{"kind", J.Str{"polygon"}}, J.KV{"points", J.Json.arr(Components.pts(ps))}, J.KV{"width", Components.num(w)}, J.KV{"fill", J.Flag{f}}])
    case B.PTint{ps, c, a}:
      J.Json.obj([J.KV{"kind", J.Str{"polygon"}}, J.KV{"points", J.Json.arr(Components.pts(ps))}, J.KV{"width", Components.num(0.0)}, J.KV{"fill", J.Flag{True{}}}, J.KV{"color", J.Json.u32(c)}, J.KV{"alpha", J.Json.u32(a)}])

def Components.items(xs: List<&2, B.Item>) -> List<&2, J.Json>:
  match xs:
    case Nil{}: Nil{}
    case B.Item{+k, _, +l, p, _} <> t:
      J.Json.obj([J.KV{"key", J.Str{k}}, J.KV{"layer", J.Json.u32(l)}, J.KV{"shape", Components.prim(p)}]) <> Components.items(t)

def Components.forms(m: Maybe<&2, List<&2, List<&2, U32>>>) -> List<&2, List<&2, U32>>:
  match m:
    case Some{xs}: xs
    case None{}: Nil{}

def Components.keep.sx(on: Bool, x: S.Sx, rest: List<&2, S.Sx>) -> List<&2, S.Sx>:
  match on:
    case True{}: x <> rest
    case False{}: rest

def Components.keep.json(on: Bool, x: J.Json, rest: List<&2, J.Json>) -> List<&2, J.Json>:
  match on:
    case True{}: x <> rest
    case False{}: rest

def Components.named.one(on: Bool, bytes: List<&2, U32>, rest: List<&2, S.Sx>) -> List<&2, S.Sx>:
  match on:
    case True{}: S.Sx.parse(bytes) <> rest
    case False{}: rest

def Components.named(xs: List<&2, List<&2, U32>>, +name: String) -> List<&2, S.Sx>:
  match xs:
    case Nil{}: Nil{}
    case +h <> t:
      Components.named.one(String.eq(SC.Sch.head(h), name), h, Components.named(t, name))

def Components.last(s: String) -> String:
  Maybe.default(&2, String, List.last(&2, String, String.split(s, '/')), "")

def Components.match.symbol(+fp: S.Sx, +sym: S.Sx) -> Bool:
  Bool.and(Bool.not(String.is_empty(S.Sx.attr(fp, "path"))),
    Bool.and(String.eq(Components.last(S.Sx.attr(fp, "path")), S.Sx.attr(sym, "uuid")),
      String.eq(B.Info.prop(fp, "Reference"), B.Info.prop(sym, "Reference"))))

def Components.matches(syms: List<&2, S.Sx>, +fp: S.Sx) -> List<&2, S.Sx>:
  match syms:
    case Nil{}: Nil{}
    case +s <> t:
      Components.keep.sx(Components.match.symbol(fp, s), s, Components.matches(t, fp))

def Components.one.symbol(xs: List<&2, S.Sx>) -> Maybe<&2, S.Sx>:
  match xs:
    case x <> Nil{}: Some{x}
    case _: None{}

def Components.lib.exists(m: Maybe<&2, SC.Lib>) -> Bool:
  match m:
    case Some{_}: True{}
    case None{}: False{}

def Components.symbol.items(found: Bool, +s: S.Sx, +lib: List<&2, SC.Lib>, +source: String) -> J.Json:
  match found:
    case False{}: J.Null{}
    case True{}:
      J.Json.obj([J.KV{"source", J.Str{source}}, J.KV{"uuid", J.Str{S.Sx.attr(s, "uuid")}}, J.KV{"libId", J.Str{S.Sx.attr(s, "lib_id")}},
        J.KV{"unit", J.Json.u32(SC.Sym.num(s, "unit"))}, J.KV{"items", J.Json.arr(Components.items(SC.Sch.symbol(s, S.Sx.attr(s, "uuid"), 0, lib, Map.new(&2, String))))}])

def Components.symbol(m: Maybe<&2, S.Sx>, +lib: List<&2, SC.Lib>, +source: String) -> J.Json:
  match m:
    case Some{+s}:
      Components.symbol.items(Components.lib.exists(SC.Lib.find(lib, SC.Sch.cache.id(s))), s, lib, source)
    case None{}: J.Null{}

def Components.prop(m: Maybe<&2, S.Sx>, +name: String) -> String:
  match m:
    case Some{s}: B.Info.prop(s, name)
    case None{}: ""

def Components.mpn(+x: S.Sx) -> String:
  +v = B.Info.prop(x, "MPN")
  Bool.pick(String, String.is_empty(v), B.Info.prop(x, "mpn"), v)

def Components.mpn.symbol(m: Maybe<&2, S.Sx>) -> String:
  match m:
    case Some{s}: Components.mpn(s)
    case None{}: ""

def Components.or(+a: String, b: String) -> String:
  Bool.pick(String, String.is_empty(a), b, a)

def Components.pad(+x: S.Sx, +pl: B.Place) -> J.Json:
  +at = S.Sx.get(x, "at")
  +size = S.Sx.get(x, "size")
  J.Json.obj([J.KV{"number", J.Str{S.Sx.arg(x, 0n)}}, J.KV{"uuid", J.Str{S.Sx.attr(x, "uuid")}},
    J.KV{"type", J.Str{S.Sx.arg(x, 1n)}}, J.KV{"shape", J.Str{S.Sx.arg(x, 2n)}}, J.KV{"local", Components.pt(B.Geo.pt(at))},
    J.KV{"board", Components.pt(B.Geo.place(pl, B.Geo.pt(at)))}, J.KV{"angle", Components.num(S.Sx.f(at, 2n))},
    J.KV{"size", J.Json.arr([Components.num(S.Sx.f(size, 0n)), Components.num(S.Sx.f(size, 1n))])},
    J.KV{"roundrectRatio", Components.num(S.Sx.f(S.Sx.get(x, "roundrect_rratio"), 0n))}, J.KV{"net", J.Str{B.Info.net(x)}}, J.KV{"drill", Components.num(S.Sx.f(S.Sx.get(x, "drill"), 0n))}])

def Components.pads(xs: List<&2, S.Sx>, +pl: B.Place) -> List<&2, J.Json>:
  match xs:
    case Nil{}: Nil{}
    case +x <> t:
      Components.keep.json(String.eq(S.Sx.name(x), "pad"), Components.pad(x, pl), Components.pads(t, pl))

def Components.model(+x: S.Sx) -> J.Json:
  J.Json.obj([J.KV{"path", J.Str{S.Sx.arg(x, 0n)}},
    J.KV{"offset", J.Json.arr([Components.num(S.Sx.f(S.Sx.get(S.Sx.get(x, "offset"), "xyz"), 0n)), Components.num(S.Sx.f(S.Sx.get(S.Sx.get(x, "offset"), "xyz"), 1n)), Components.num(S.Sx.f(S.Sx.get(S.Sx.get(x, "offset"), "xyz"), 2n))])},
    J.KV{"scale", J.Json.arr([Components.num(S.Sx.f(S.Sx.get(S.Sx.get(x, "scale"), "xyz"), 0n)), Components.num(S.Sx.f(S.Sx.get(S.Sx.get(x, "scale"), "xyz"), 1n)), Components.num(S.Sx.f(S.Sx.get(S.Sx.get(x, "scale"), "xyz"), 2n))])},
    J.KV{"rotate", J.Json.arr([Components.num(S.Sx.f(S.Sx.get(S.Sx.get(x, "rotate"), "xyz"), 0n)), Components.num(S.Sx.f(S.Sx.get(S.Sx.get(x, "rotate"), "xyz"), 1n)), Components.num(S.Sx.f(S.Sx.get(S.Sx.get(x, "rotate"), "xyz"), 2n))])}])

def Components.models(xs: List<&2, S.Sx>) -> List<&2, J.Json>:
  match xs:
    case Nil{}: Nil{}
    case +x <> t:
      Components.keep.json(String.eq(S.Sx.name(x), "model"), Components.model(x), Components.models(t))

# Pad shapes are supplied separately with their source shape names. Do not
# silently replace an unsupported custom/trapezoid pad with a rectangle.
def Components.graphics(xs: List<&2, S.Sx>, +key: String, acc: List<&2, B.Item>) -> List<&2, B.Item>:
  match xs:
    case Nil{}: acc
    case +x <> t:
      Components.graphics(t, key, List.append(&2, B.Item, B.Board.graphic(x, key, 0, B.Place{0.0, 0.0, 0.0}, ""), acc))

def Components.footprint(+x: S.Sx, +syms: List<&2, S.Sx>, +lib: List<&2, SC.Lib>, +schpath: String) -> J.Json:
  +sym = Components.one.symbol(Components.matches(syms, x))
  +at = B.Fp.at(x)
  J.Json.obj([J.KV{"id", J.Str{S.Sx.attr(x, "uuid")}}, J.KV{"reference", J.Str{B.Info.prop(x, "Reference")}}, J.KV{"value", J.Str{B.Info.prop(x, "Value")}},
    J.KV{"mpn", J.Str{Components.or(Components.mpn(x), Components.mpn.symbol(sym))}}, J.KV{"footprint", J.Str{S.Sx.arg(x, 0n)}},
    J.KV{"layer", J.Str{S.Sx.attr(x, "layer")}}, J.KV{"at", J.Json.arr([Components.num(S.Sx.f(at, 0n)), Components.num(S.Sx.f(at, 1n)), Components.num(S.Sx.f(at, 2n))])},
    J.KV{"schematicPath", J.Str{S.Sx.attr(x, "path")}}, J.KV{"pads", J.Json.arr(Components.pads(S.Sx.args(x), B.Fp.place(x)))},
    J.KV{"graphics", J.Json.arr(Components.items(Components.graphics(S.Sx.args(x), S.Sx.attr(x, "uuid"), Nil{})))},
    J.KV{"models", J.Json.arr(Components.models(S.Sx.args(x)))}, J.KV{"symbol", Components.symbol(sym, lib, schpath)}])

def Components.footprints(xs: List<&2, S.Sx>, +syms: List<&2, S.Sx>, +lib: List<&2, SC.Lib>, +schpath: String) -> List<&2, J.Json>:
  match xs:
    case Nil{}: Nil{}
    case x <> t: Components.footprint(x, syms, lib, schpath) <> Components.footprints(t, syms, lib, schpath)

def Components.parsed(valid: Bool, +pcb: List<&2, U32>, +sch: List<&2, U32>, +pcbpath: String, +schpath: String) -> J.Json:
  match valid:
    case False{}: J.Json.obj([J.KV{"error", J.Str{"Board unavailable or incomplete; save a complete .kicad_pcb first."}}])
    case True{}:
      +ps = Components.forms(S.Sx.forms(pcb))
      +ss = Components.forms(S.Sx.forms(sch))
      J.Json.obj([J.KV{"schema", J.Json.u32(1)}, J.KV{"thicknessMm", Components.num(S.Sx.f(S.Sx.get(S.Sx.parse(SC.Sch.find(ps, "general")), "thickness"), 0n))}, J.KV{"source", J.Str{pcbpath}}, J.KV{"schematic", J.Str{schpath}},
        J.KV{"fingerprint", J.Str{U32.show(B.Board.hash(sch, B.Board.hash(pcb, 2166136261)))}},
        J.KV{"symbolLimit", J.Str{"Only exact unique UUID and reference matches in the supplied root schematic are previewed; hierarchical or ambiguous associations remain unavailable."}},
        J.KV{"assemblyLimit", J.Str{"Mesh triangles have no component identity; selection refers to saved footprint geometry, not an identified 3D body."}},
        J.KV{"components", J.Json.arr(Components.footprints(Components.named(ps, "footprint"), Components.named(ss, "symbol"), SC.Lib.table(S.Sx.parse(SC.Sch.find(ss, "lib_symbols"))), schpath))}])

def Components.complete(m: Maybe<&2, List<&2, List<&2, U32>>>) -> Bool:
  match m:
    case Some{_}: True{}
    case None{}: False{}

def Components.read(+pcb: List<&2, U32>, sch: List<&2, U32>, pcbpath: String, schpath: String) -> J.Json:
  Components.parsed(Bool.and(String.eq(SC.Sch.head(pcb), "kicad_pcb"), Components.complete(S.Sx.forms(pcb))), pcb, sch, pcbpath, schpath)

def Components.text(pcb: String, sch: String, pcbpath: String, schpath: String) -> String:
  J.Json.show(Components.read(U.Utf8.encode(pcb), U.Utf8.encode(sch), pcbpath, schpath))

# A small export used by parser regression tests and browser-side fixtures.
def inspect(pcb: String, sch: String, pcbpath: String, schpath: String) -> String:
  Components.text(pcb, sch, pcbpath, schpath)

# Bounded transport of complete saved metadata. Never shorten a component row.
# The client assembles one exact PCB+schematic revision before publishing it.
type ComponentPage is Data:
  ComponentPage{rows: List<&2, J.Json>, count: Nat, bytes: Nat}

def Components.utf8.width(a: Bool, b: Bool, c: Bool) -> Nat:
  match a b c:
    case True{} _ _: 1n
    case False{} True{} _: 2n
    case False{} False{} True{}: 3n
    case False{} False{} False{}: 4n

def Components.utf8.count(s: String, n: Nat) -> Nat:
  match s:
    case SNil{}: n
    case SCon{Chr{+c}, t}:
      Components.utf8.count(t, Nat.add(n, Components.utf8.width(U32.is_lt(c, 128), U32.is_lt(c, 2048), U32.is_lt(c, 65536))))

def Components.utf8.bytes(s: String) -> Nat:
  Components.utf8.count(s, 0n)

def Components.page.bytes(data: J.Json) -> Nat:
  Components.utf8.bytes(J.Json.show(data))

def Components.page.fits(xs: List<&2, J.Json>, count: Nat, bytes: Nat) -> Bool:
  match xs:
    case Nil{}: True{}
    case h <> _: Bool.and(Nat.is_lt(count, 32n), Nat.is_le((bytes + Components.page.bytes(h) + 1n : Nat), 180000n))

def Components.page.pack(xs: List<&2, J.Json>, count: Nat, bytes: Nat, rows: List<&2, J.Json>, on: Bool) -> ComponentPage:
  match xs on:
    case Nil{} _: ComponentPage{List.reverse(&2, J.Json, rows), count, bytes}
    case _ False{}: ComponentPage{List.reverse(&2, J.Json, rows), count, bytes}
    case +h <> +t True{}:
      +n : Nat = (count + 1n : Nat)
      +b : Nat = (bytes + Components.page.bytes(h) + 1n : Nat)
      Components.page.pack(t, n, b, h <> rows, Components.page.fits(t, n, b))

def Components.page.field(drop: Bool, k: String, v: J.Json, rest: J.Json) -> J.Json:
  match drop:
    case True{}: rest
    case False{}: J.Field{k, v, rest}

def Components.page.fields(fs: J.Json) -> J.Json:
  match fs:
    case J.Field{+k, v, t}:
      Components.page.field(Bool.or(String.eq(k, "components"), String.eq(k, "page")), k, v, Components.page.fields(t))
    case _: J.End{}

def Components.page.next(done: Bool, end: Nat) -> J.Json:
  match done:
    case True{}: J.Null{}
    case False{}: J.Json.nat(end)

def Components.page.result.valid(ok: Bool, fs: J.Json, rows: List<&2, J.Json>, +count: Nat, +offset: Nat, +total: Nat) -> J.Json:
  match ok:
    case False{}: J.Json.obj([J.KV{"error", J.Str{"One component exceeds the bounded page budget; metadata cannot be truncated."}}])
    case True{}:
      +done = Nat.is_ge((offset + count : Nat), total)
      page = J.Json.obj([J.KV{"offset", J.Json.nat(offset)}, J.KV{"next", Components.page.next(done, (offset + count : Nat))}, J.KV{"total", J.Json.nat(total)}, J.KV{"complete", J.Flag{done}}])
      J.Obj{J.Field{"components", J.Json.arr(rows), J.Field{"page", page, Components.page.fields(fs)}}}

def Components.page.result(data: J.Json, +offset: Nat, +total: Nat, packed: ComponentPage) -> J.Json:
  match data packed:
    case J.Obj{fs} ComponentPage{rows, +count, _}:
      Components.page.result.valid(Bool.or(Nat.is_gt(count, 0n), Nat.is_eq(offset, total)), fs, rows, count, offset, total)
    case _ _: J.Json.obj([J.KV{"error", J.Str{"Invalid component metadata page."}}])

def Components.page.checked(ok: Bool, +data: J.Json, +offset: Nat) -> J.Json:
  match ok:
    case False{}: J.Json.obj([J.KV{"error", J.Str{"Component metadata revision/offset is invalid; restart paging from the saved board."}}])
    case True{}:
      +xs = J.Json.elems(J.Json.get_obj(data, "components"))
      +rest = List.drop(&2, J.Json, xs, offset)
      Components.page.result(data, offset, List.length(&2, J.Json, xs), Components.page.pack(rest, 0n, 0n, Nil{}, Components.page.fits(rest, 0n, 0n)))

def Components.page.snapshot(+data: J.Json, +offset: Nat, +pcb: String, +sch: String) -> J.Json:
  +stamp = J.Json.get_str(data, "sourceRevision", "")
  +total = List.length(&2, J.Json, J.Json.elems(J.Json.get_obj(data, "components")))
  Components.page.checked(
    Bool.and(String.is_empty(J.Json.get_str(data, "error", "")),
      Bool.and(Bool.not(String.is_empty(stamp)),
        Bool.and(Bool.and(Nat.is_le(offset, total), Nat.is_le(total, 4096n)),
          Bool.or(Bool.and(Nat.is_eq(offset, 0n), Bool.and(String.is_empty(pcb), String.is_empty(sch))),
            Bool.and(Bool.not(String.is_empty(pcb)), Bool.and(String.eq(pcb, stamp), String.eq(sch, J.Json.get_str(data, "schematicRevision", "")))))))), data, offset)

def Components.page.array(data: J.Json) -> Bool:
  match data:
    case J.Arr{_}: True{}
    case _: False{}

def Components.page.guard(ok: Bool, +data: J.Json, offset: Nat, pcb: String, sch: String) -> J.Json:
  match ok:
    case True{}: Components.page.snapshot(data, offset, pcb, sch)
    case False{}:
      J.Json.obj([J.KV{"error", J.Str{J.Json.get_str(data, "error", "Invalid component metadata schema or rows.")}}])

def Components.page(+data: J.Json, offset: Nat, pcb: String, sch: String) -> J.Json:
  Components.page.guard(Bool.and(U32.is_eq(J.Json.get_u32(data, "schema", 0), 1),
    Components.page.array(J.Json.get_obj(data, "components"))), data, offset, pcb, sch)

# Actual emitted-code test seam.
def page(text: String, offset: Nat, pcb: String, sch: String) -> String:
  J.Json.show(Components.page(Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{}), offset, pcb, sch))
