import Base
import ./kicad.bend as K
import ./plot.bend as PV
import ./proc.bend as PR
import ../core/json.bend as J

# These effects receive only paths established by the trusted thread cwd and
# canonical Files resolver. Binary GLB data never crosses the Bend JSON bridge.
def RenderAssetFx.authorize(cwd: String, path: String, canonical: String, kind: String, revision: String) -> IO(String):
  import "./effects/render_asset.js"

def RenderAssetFx.publish(cwd: String, path: String, canonical: String, kind: String, revision: String, glb: String, missing: String) -> IO(String):
  import "./effects/render_asset.js"

def RenderAssetFx.lookup(id: String) -> IO(String):
  import "./effects/render_asset.js"

def RenderAsset.error(s: String) -> IO(Unit):
  IO.print(J.Json.show(J.Json.obj([J.KV{"error", J.Str{s}}])))

def RenderAsset.exported(r: PR.Ran, +cwd: String, +path: String, +canonical: String, +kind: String, +revision: String, +out: String) -> IO(Unit):
  match r:
    case PR.Ran{0, log}:
      IO.bind(String, Unit, RenderAssetFx.publish(cwd, path, canonical, kind, revision, out, PV.Pv.missing.note(PV.Pv.missing(String.lines(log), 0n))), IO.print)
    case PR.Ran{_, log}: RenderAsset.error("Original GLB export failed: " ++ String.take(String.trim(log), 500n))

def RenderAsset.export(cached: Bool, glb: Bool, step: Bool, +cwd: String, +path: String, +canonical: String, +kind: String, +revision: String, +out: String) -> IO(Unit):
  match cached:
    case True{}: RenderAsset.exported(PR.Ran{0, ""}, cwd, path, canonical, kind, revision, out)
    case False{}:
      IO.bind(PR.Ran, Unit, PV.Pv.glb.run(glb, step, String.eq(kind, "3"), True{}, path, out), r => RenderAsset.exported(r, cwd, path, canonical, kind, revision, out))

def RenderAsset.out(+cwd: String, +path: String, +canonical: String, +kind: String, +revision: String, +glb: Bool, +step: Bool) -> IO(Unit):
  do IO<Unit>:
    +out : String <- PV.Pv.solid.out(glb, step, String.eq(kind, "3"), True{}, path)
    cached : Bool <- K.Io.is_file(out)
    RenderAsset.export(cached, glb, step, cwd, path, canonical, kind, revision, out)

def RenderAsset.authorized(ok: Bool, +info: J.Json, +cwd: String, +canonical: String, +kind: String, +revision: String) -> IO(Unit):
  match ok:
    case False{}: IO.print(J.Json.show(info))
    case True{}:
      RenderAsset.out(cwd, J.Json.get_str(info, "source", ""), canonical, kind, revision,
        String.eq(J.Json.get_str(info, "kind", ""), "glb"), String.eq(J.Json.get_str(info, "kind", ""), "step"))

def RenderAsset.files(f: K.Files, +cwd: String, path: String, +kind: String, +revision: String) -> IO(Unit):
  match f:
    case K.Files{_, +pcb, _, _}:
      do IO<Unit>:
        raw : String <- RenderAssetFx.authorize(cwd, path, pcb, kind, revision)
        +info : J.Json = Maybe.default(&2, J.Json, J.Json.parse(raw), J.Json.obj([J.KV{"error", J.Str{"Unable to authorize saved asset."}}]))
        RenderAsset.authorized(String.is_empty(J.Json.get_str(info, "error", "")), info, cwd, pcb, kind, revision)

# Root routes this same-version child off the hub: --review renderasset DIR PATH KIND REV.
def RenderAsset.child(+cwd: String, path: String, kind: String, expectedRevision: String) -> IO(Unit):
  IO.bind(K.Files, Unit, K.Files.read(cwd), fs => RenderAsset.files(fs, cwd, path, kind, expectedRevision))

# Root authenticates the HTTP request before calling lookup and sending a file.
# The only client capability accepted here is a lowercase SHA-256 identifier.
def RenderAsset.lookup(id: String) -> IO(String):
  RenderAssetFx.lookup(id)
