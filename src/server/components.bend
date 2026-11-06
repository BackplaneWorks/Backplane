import Base
import ./kicad.bend as K
import ./host.bend as H
import ../core/components.bend as C
import ../core/json.bend as J
import ../core/utf8.bend as U

# Caller must use its thread's trusted cwd. An explicit board is resolved
# inside that project; .backplane.json remains canonical when none is named.
type ComponentRevision is Data:
  ComponentRevision{bytes: U32, stamp: String, found: Bool}

def Components.revision(r: Result<&1, &1, U32 & String, U32 & String>) -> ComponentRevision:
  match r:
    case Done{sm}:
      (s, m) = sm
      ComponentRevision{s, m, True{}}
    case Fail{_}: ComponentRevision{0, "", False{}}

def Components.version(path: String) -> IO(ComponentRevision):
  IO.bind(Result<&1, &1, U32 & String, U32 & String>, ComponentRevision, H.FS.revision(path), r => IO.pure(ComponentRevision, Components.revision(r)))

def Components.same(a: ComponentRevision, b: ComponentRevision) -> Bool:
  match a b:
    case ComponentRevision{s, m, f} ComponentRevision{t, n, g}:
      Bool.and(U32.is_eq(s, t), Bool.and(String.eq(m, n), Bool.not(Bool.xor(f, g))))

def Components.small(r: ComponentRevision) -> Bool:
  match r:
    case ComponentRevision{s, _, _}: U32.is_le(s, 16777216)

def Components.error(s: String) -> J.Json:
  J.Json.obj([J.KV{"error", J.Str{s}}])

def Components.stamp(r: ComponentRevision) -> String:
  match r:
    case ComponentRevision{_, stamp, _}: stamp

def Components.tag(data: J.Json, pcb: ComponentRevision, sch: ComponentRevision) -> J.Json:
  match data:
    case J.Obj{fields}:
      J.Obj{J.Field{"sourceRevision", J.Str{Components.stamp(pcb)}, J.Field{"schematicRevision", J.Str{Components.stamp(sch)}, fields}}}
    case _: Components.error("Invalid saved component metadata.")

def Components.checked(stable: Bool, data: J.Json) -> J.Json:
  match stable:
    case True{}: data
    case False{}: Components.error("Saved CAD changed while reading component metadata; retry after the save finishes.")

def Components.load(ok: Bool, +pcb: String, +sch: String, +a: ComponentRevision, +b: ComponentRevision) -> IO(J.Json):
  match ok:
    case False{}: IO.pure(J.Json, Components.error("Saved CAD exceeds the 16 MiB component-preview input limit."))
    case True{}:
      do IO<J.Json>:
        bs : List<&2, U32> <- K.Io.bytes(pcb)
        ss : List<&2, U32> <- K.Io.bytes(sch)
        data : J.Json = C.Components.read(bs, ss, pcb, sch)
        c : ComponentRevision <- Components.version(pcb)
        d : ComponentRevision <- Components.version(sch)
        return Components.checked(Bool.and(Components.same(a, c), Components.same(b, d)), Components.tag(data, a, b))

def Components.files(fs: K.Files) -> IO(J.Json):
  match fs:
    case K.Files{_, +pcb, +sch, _}:
      do IO<J.Json>:
        +a : ComponentRevision <- Components.version(pcb)
        +b : ComponentRevision <- Components.version(sch)
        Components.load(Bool.and(Components.small(a), Components.small(b)), pcb, sch, a, b)

def Components.read(cwd: String) -> IO(J.Json):
  IO.bind(K.Files, J.Json, K.Files.read(cwd), Components.files)

def Components.bounded(ok: Bool, text: String) -> String:
  match ok:
    case True{}: text
    case False{}: "{\"error\":\"Component metadata exceeds the 220000-byte preview response limit; this board needs paged metadata.\"}"

def Components.response(+data: J.Json) -> String:
  +text = J.Json.show(J.Json.obj([J.KV{"components", data}]))
  Components.bounded(Nat.is_le(C.Components.utf8.bytes(text), 219999n), text)

def Components.child(cwd: String) -> IO(Unit):
  IO.bind(J.Json, Unit, Components.read(cwd), data => IO.print(Components.response(data)))

# Pages kept per (project, PCB revision, schematic revision): see
# effects/components_cache.js. put answers "", get the page's exact response
# or "" when anything differs.
def ComponentsFx.put(cwd: String, pcb: String, sch: String, pcbRev: String, schRev: String, offset: String, text: String) -> IO(String):
  import "./effects/components_cache.js"

def ComponentsFx.get(cwd: String, offset: String, pcbRev: String, schRev: String, path: String) -> IO(String):
  import "./effects/components_cache.js"

def ComponentsFx.first(cwd: String, pcb: String, sch: String) -> IO(String):
  import "./effects/components_cache.js"

def ComponentsFx.sources(cwd: String, path: String, pcb: String, sch: String) -> IO(String):
  import "./effects/components_cache.js"

def Components.sources.files(fs: K.Files, +cwd: String, path: String) -> IO(J.Json):
  match fs:
    case K.Files{_, pcb, sch, _}:
      IO.bind(String, J.Json, ComponentsFx.sources(cwd, path, pcb, sch), text =>
        IO.pure(J.Json, Maybe.default(&2, J.Json, J.Json.parse(text), Components.error("Could not resolve the selected board."))))

def Components.sources(+cwd: String, path: String) -> IO(J.Json):
  IO.bind(K.Files, J.Json, K.Files.read(cwd), fs => Components.sources.files(fs, cwd, path))

# A first read's pages, in order: all are kept, so the hub answers the pages
# after the first from the cache (a page child parses both files again for one)
def Components.chain(fuel: Nat, go: Bool, +data: J.Json, +off: Nat, acc: List<&2, J.Json>) -> List<&2, J.Json>:
  match fuel go:
    case 1n+f True{}:
      +page = C.Components.page(data, off, J.Json.get_str(data, "sourceRevision", ""), J.Json.get_str(data, "schematicRevision", ""))
      +next = J.Json.get_nat(J.Json.get_obj(page, "page"), "next", 0n)
      Components.chain(f, Bool.and(String.is_empty(J.Json.get_str(page, "error", "")), Nat.is_gt(next, off)), data, next, page <> acc)
    case _ _: List.reverse(&2, J.Json, acc)

def Components.offsets(ps: List<&2, J.Json>) -> List<&2, J.Json>:
  match ps:
    case Nil{}: Nil{}
    case +p <> t: J.Json.nat(J.Json.get_nat(J.Json.get_obj(p, "page"), "offset", 0n)) <> Components.offsets(t)

def Components.page.add(p: J.Json, offs: List<&2, J.Json>) -> J.Json:
  match p:
    case J.Obj{fs}: J.Obj{J.Field{"offsets", J.Json.arr(offs), fs}}
    case _: p

# The first page names every page's offset ("page.offsets", 0 first), so
# the viewer can ask the rest at once (web/viewer-rpc.js) rather than one
# round trip after another (6 of them for the real-data copy's board,
# +RTT each). An old viewer reads the pages as before.
def Components.first.offsets(+page: J.Json, offs: List<&2, J.Json>) -> J.Json:
  match page:
    case J.Obj{J.Field{k, cs, J.Field{pk, pg, rest}}}: J.Obj{J.Field{k, cs, J.Field{pk, Components.page.add(pg, offs), rest}}}
    case _: page

def Components.first.go(+ps: List<&2, J.Json>) -> List<&2, J.Json>:
  match ps:
    case Nil{}: Nil{}
    case +p <> t: Components.first.offsets(p, Components.offsets(ps)) <> t

def Components.put.all(ps: List<&2, J.Json>, +cwd: String, +pcb: String, +sch: String) -> IO(Unit):
  match ps:
    case Nil{}: IO.pure(Unit, Unit{})
    case +p <> t:
      do IO<Unit>:
        w : String <- ComponentsFx.put(cwd, pcb, sch, J.Json.get_str(p, "sourceRevision", ""), J.Json.get_str(p, "schematicRevision", ""), Nat.show(J.Json.get_nat(J.Json.get_obj(p, "page"), "offset", 0n)), Components.response(p))
        Components.put.all(t, cwd, pcb, sch)

def Components.first.print(ps: List<&2, J.Json>) -> IO(Unit):
  match ps:
    case Nil{}: IO.pure(Unit, Unit{})
    case +p <> _: IO.print(Components.response(p))

# a first read: its pages worked out once, the first (with every offset)
# printed, all of them kept for the hub
def Components.page.first(+data: J.Json, +cwd: String, +pcb: String, +sch: String) -> IO(Unit):
  +ps = Components.first.go(Components.chain(4097n, True{}, data, 0n, Nil{}))
  do IO<Unit>:
    Components.first.print(ps)
    Components.put.all(ps, cwd, pcb, sch)

def Components.page.at(first: Bool, +data: J.Json, +cwd: String, +offset: Nat, +pcbRev: String, +schRev: String, +pcb: String, +sch: String) -> IO(Unit):
  match first:
    case True{}: Components.page.first(data, cwd, pcb, sch)
    case False{}: IO.print(Components.response(C.Components.page(data, offset, pcbRev, schRev)))

def Components.page.files(fs: K.Files, +cwd: String, +offset: Nat, +pcbRev: String, +schRev: String) -> IO(Unit):
  match fs:
    case K.Files{_, +pcb, +sch, _}:
      do IO<Unit>:
        +a : ComponentRevision <- Components.version(pcb)
        +b : ComponentRevision <- Components.version(sch)
        +data : J.Json <- Components.load(Bool.and(Components.small(a), Components.small(b)), pcb, sch, a, b)
        Components.page.at(Bool.and(Nat.is_eq(offset, 0n), Bool.and(String.is_empty(pcbRev), String.is_empty(J.Json.get_str(data, "error", "")))), data, cwd, offset, pcbRev, schRev, pcb, sch)

# Subsequent pages send both stamps from the first page. The unchanged outer
# response cap includes its envelope and trailing newline.
def Components.page.sources(ok: Bool, +sources: J.Json, cwd: String, offset: Nat, pcb: String, sch: String) -> IO(Unit):
  match ok:
    case True{}:
      Components.page.files(K.Files{"", J.Json.get_str(sources, "source", ""), J.Json.get_str(sources, "schematic", ""), ""}, cwd, offset, pcb, sch)
    case False{}:
      IO.print(Components.response(sources))

def Components.page.child.at(+cwd: String, path: String, offset: Nat, pcb: String, sch: String) -> IO(Unit):
  IO.bind(J.Json, Unit, Components.sources(cwd, path), +sources =>
    Components.page.sources(String.is_empty(J.Json.get_str(sources, "error", "")), sources, cwd, offset, pcb, sch))

def Components.page.child(cwd: String, offset: Nat, pcb: String, sch: String) -> IO(Unit):
  Components.page.child.at(cwd, "", offset, pcb, sch)
