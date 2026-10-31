import Base
import ./kicad.bend as K
import ./host.bend as H
import ../core/components.bend as C
import ../core/json.bend as J
import ../core/utf8.bend as U

# Caller must use its thread's trusted cwd. No client-provided path or
# arbitrary-file endpoint is accepted here; .backplane.json remains canonical.
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

def ComponentsFx.get(cwd: String, offset: String, pcbRev: String, schRev: String) -> IO(String):
  import "./effects/components_cache.js"

def ComponentsFx.first(cwd: String, pcb: String, sch: String) -> IO(String):
  import "./effects/components_cache.js"

# every page of a first read, so the hub answers the pages after it from the
# cache (a page child parses both files again for one page)
def Components.pages.put(fuel: Nat, go: Bool, +data: J.Json, +off: Nat, +cwd: String, +pcb: String, +sch: String) -> IO(Unit):
  match fuel go:
    case 1n+f True{}:
      +pcbRev = J.Json.get_str(data, "sourceRevision", "")
      +schRev = J.Json.get_str(data, "schematicRevision", "")
      +page = C.Components.page(data, off, pcbRev, schRev)
      +next = J.Json.get_nat(J.Json.get_obj(page, "page"), "next", 0n)
      do IO<Unit>:
        w : String <- ComponentsFx.put(cwd, pcb, sch, pcbRev, schRev, Nat.show(off), Components.response(page))
        Components.pages.put(f, Bool.and(String.is_empty(J.Json.get_str(page, "error", "")), Nat.is_gt(next, off)), data, next, cwd, pcb, sch)
    case _ _:
      IO.pure(Unit, Unit{})

def Components.page.files(fs: K.Files, +cwd: String, +offset: Nat, +pcbRev: String, +schRev: String) -> IO(Unit):
  match fs:
    case K.Files{_, +pcb, +sch, _}:
      do IO<Unit>:
        +a : ComponentRevision <- Components.version(pcb)
        +b : ComponentRevision <- Components.version(sch)
        +data : J.Json <- Components.load(Bool.and(Components.small(a), Components.small(b)), pcb, sch, a, b)
        IO.print(Components.response(C.Components.page(data, offset, pcbRev, schRev)))
        Components.pages.put(4097n, Bool.and(Nat.is_eq(offset, 0n), Bool.and(String.is_empty(pcbRev), String.is_empty(J.Json.get_str(data, "error", "")))), data, 0n, cwd, pcb, sch)

# Subsequent pages send both stamps from the first page. The unchanged outer
# response cap includes its envelope and trailing newline.
def Components.page.child(+cwd: String, offset: Nat, pcb: String, sch: String) -> IO(Unit):
  IO.bind(K.Files, Unit, K.Files.read(cwd), fs => Components.page.files(fs, cwd, offset, pcb, sch))
