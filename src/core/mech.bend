import Base
import ./glb.bend as G
import ./images.bend as IM
import ./json.bend as J

# Mechanical parts (pure). Agents model parts in FreeCAD and keep them in
# the project's `mech/` folder as STEP (or GLB); `backplane --render
# MODEL` draws each from the same few views into `renders/` beside it,
# named `<stem>.<view>.png`. The hub lists that folder (mech.list: one
# cksum line per file, "<crc> <size> <path>", paths from the project
# root), and every client's Mechanical page shows the parts, each one's
# renders, and the part itself in the 3D viewer.

# the folder a project keeps its parts in
def Mech.dir() -> String:
  "mech"

# .backplane.json can name the default model by its project-relative path.
# Missing, malformed or non-string settings leave the usual default alone.
def Mech.config(+text: String) -> String:
  J.Json.get_str(Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{}), "mech", "")

# a camera direction by name (yaw and pitch as the 3D viewer's standard
# views take them, nui.bend's Cam.key), on a part stood Z up (Mech.upright)
type MView is Data:
  MView{name: String, yaw: F32, pitch: F32}

# the views every render has, in the order pages show them
def Mech.views() -> List<&2, MView>:
  [MView{"iso", 0.785398, 0.61548}, MView{"front", 0.0, 0.0}, MView{"top", 0.0, 1.570796}, MView{"right", 1.570796, 0.0}]

def Mech.view.names(vs: List<&2, MView>) -> List<&2, String>:
  match vs:
    case Nil{}:
      Nil{}
    case MView{n, _, _} <> t:
      n <> Mech.view.names(t)

# CAD parts are Z up; the viewer's world is Y up (glTF's, and the boards'):
# (x, y, z) goes to (x, z, -y), so a part's top faces up and its front
# (-Y in CAD) faces the front view
def Mech.up.tri(t: G.Tri3) -> G.Tri3:
  match t:
    case G.Tri3{ax, ay, az, bx, by, bz, cx, cy, cz, c}:
      G.Tri3{ax, az, F32.neg(ay), bx, bz, F32.neg(by), cx, cz, F32.neg(cy), c}

def Mech.upright.go(ts: List<&2, G.Tri3>, acc: List<&2, G.Tri3>) -> List<&2, G.Tri3>:
  match ts:
    case Nil{}:
      acc
    case t <> r:
      Mech.upright.go(r, Mech.up.tri(t) <> acc)

def Mech.upright(ts: List<&2, G.Tri3>) -> List<&2, G.Tri3>:
  List.reverse(&2, G.Tri3, Mech.upright.go(ts, Nil{}))

# a STEP's model (step2glb writes millimetres) in metres, glTF's unit and
# KiCad's board exports': the phones and the web then place, fit and zoom a
# part as they do a board
def Mech.mm.tri(t: G.Tri3) -> G.Tri3:
  match t:
    case G.Tri3{ax, ay, az, bx, by, bz, cx, cy, cz, c}:
      G.Tri3{F32.mul(ax, 0.001), F32.mul(ay, 0.001), F32.mul(az, 0.001), F32.mul(bx, 0.001), F32.mul(by, 0.001), F32.mul(bz, 0.001),
        F32.mul(cx, 0.001), F32.mul(cy, 0.001), F32.mul(cz, 0.001), c}

def Mech.metres.go(ts: List<&2, G.Tri3>, acc: List<&2, G.Tri3>) -> List<&2, G.Tri3>:
  match ts:
    case Nil{}:
      List.reverse(&2, G.Tri3, acc)
    case t <> r:
      Mech.metres.go(r, Mech.mm.tri(t) <> acc)

def Mech.metres(ts: List<&2, G.Tri3>) -> List<&2, G.Tri3>:
  Mech.metres.go(ts, Nil{})

# Files
# -----

def Mech.ext.is(+p: String, +e: String) -> Bool:
  String.ends_with(String.to_lower(p), e)

def Mech.is_step(+p: String) -> Bool:
  Bool.or(Mech.ext.is(p, ".step"), Mech.ext.is(p, ".stp"))

# a file a part is kept as
def Mech.is_model(+p: String) -> Bool:
  Bool.or(Mech.is_step(p), Mech.ext.is(p, ".glb"))

def Mech.last(+p: String) -> String:
  Maybe.default(&2, String, List.last(&2, String, String.split(p, '/')), p)

# the folder a path is in ("" for none)
def Mech.parent(+p: String) -> String:
  +n = Nat.sub(String.length(p), String.length(Mech.last(p)))
  String.take(p, Bool.pick(Nat, Nat.is_eq(n, 0n), 0n, Nat.sub(n, 1n)))

def Mech.stem.k(+ps: List<&2, String>, +f: String) -> String:
  Bool.pick(String, Nat.is_lt(List.length(&2, String, ps), 2n), f, String.join(List.take(&2, String, ps, Nat.sub(List.length(&2, String, ps), 1n)), "."))

# a file's name without its extension
def Mech.stem(+p: String) -> String:
  Mech.stem.k(String.split(Mech.last(p), '.'), Mech.last(p))

def Mech.in(+dir: String, +f: String) -> String:
  Bool.pick(String, String.is_empty(dir), f, dir ++ "/" ++ f)

# where a model's render from a view goes: renders/<stem>.<view>.png beside it
def Mech.render(+model: String, +view: String) -> String:
  Mech.in(Mech.in(Mech.parent(model), "renders"), Mech.stem(model) ++ "." ++ view ++ ".png")

# a listed file: its path from the project root and its checksum (a
# render drawn again gets a new one, so no client shows a stale picture)
type MFile is Data:
  MFile{path: String, sum: String}

def Mech.line.k(ws: List<&2, String>) -> Maybe<&2, MFile>:
  match ws:
    case +c <> _ <> +r:
      Bool.pick(Maybe<&2, MFile>, List.is_empty(&2, String, r), None{}, Some{MFile{String.join(r, " "), c}})
    case _:
      None{}

# a cksum line: "<crc> <size> <path>" (the path may hold spaces)
def Mech.line(+l: String) -> Maybe<&2, MFile>:
  Mech.line.k(String.split(String.trim(l), ' '))

def Mech.files.k(m: Maybe<&2, MFile>, rest: List<&2, MFile>) -> List<&2, MFile>:
  match m:
    case Some{f}:
      f <> rest
    case None{}:
      rest

def Mech.files.go(ls: List<&2, String>) -> List<&2, MFile>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      Mech.files.k(Mech.line(l), Mech.files.go(t))

# the hub's listing, as files
def Mech.files(+text: String) -> List<&2, MFile>:
  Mech.files.go(String.lines(text))

def Mech.find(fs: List<&2, MFile>, +p: String) -> Maybe<&2, MFile>:
  match fs:
    case Nil{}:
      None{}
    case MFile{+q, +s} <> t:
      Bool.pick(Maybe<&2, MFile>, String.eq(q, p), Some{MFile{q, s}}, Mech.find(t, p))

# Parts
# -----

# a part: its model, its name, and the renders there are of it, in view
# order (a view never rendered is left out)
type MPart is Data:
  MPart{path: String, name: String, shots: List<&2, MFile>}

def Mech.shots.k(m: Maybe<&2, MFile>, rest: List<&2, MFile>) -> List<&2, MFile>:
  match m:
    case Some{f}:
      f <> rest
    case None{}:
      rest

def Mech.shots(vs: List<&2, String>, +fs: List<&2, MFile>, +model: String) -> List<&2, MFile>:
  match vs:
    case Nil{}:
      Nil{}
    case +v <> t:
      Mech.shots.k(Mech.find(fs, Mech.render(model, v)), Mech.shots(t, fs, model))

def Mech.part.k(model: Bool, +p: String, +fs: List<&2, MFile>, rest: List<&2, MPart>) -> List<&2, MPart>:
  match model:
    case True{}:
      MPart{p, Mech.stem(p), Mech.shots(Mech.view.names(Mech.views()), fs, p)} <> rest
    case False{}:
      rest

def Mech.parts.go(xs: List<&2, MFile>, +fs: List<&2, MFile>) -> List<&2, MPart>:
  match xs:
    case Nil{}:
      Nil{}
    case MFile{+p, _} <> t:
      Mech.part.k(Mech.is_model(p), p, fs, Mech.parts.go(t, fs))

# every part the listing holds, in its order (the hub sorts it)
def Mech.parts(+text: String) -> List<&2, MPart>:
  Mech.parts.go(Mech.files(text), Mech.files(text))

# the renders of the first part
def Mech.part.shots.of(ps: List<&2, MPart>) -> List<&2, MFile>:
  match ps:
    case MPart{_, _, fs} <> _:
      fs
    case Nil{}:
      Nil{}

def Mech.is_asm(+name: String) -> Bool:
  Bool.or(String.contains(String.to_lower(name), "assembl"), String.ends_with(String.to_lower(name), "-asm"))

def Mech.asm(ps: List<&2, MPart>) -> Maybe<&2, MPart>:
  match ps:
    case Nil{}:
      None{}
    case MPart{+p, +n, +s} <> t:
      Bool.pick(Maybe<&2, MPart>, Mech.is_asm(n), Some{MPart{p, n, s}}, Mech.asm(t))

def Mech.default.k(m: Maybe<&2, MPart>, +ps: List<&2, MPart>) -> Maybe<&2, MPart>:
  match m:
    case Some{p}:
      Some{p}
    case None{}:
      List.head(&2, MPart, ps)

# the part Mech shows when none was asked for: the assembly (a name with
# "assembly" in it, or ending "-asm"), else the first
def Mech.default(+ps: List<&2, MPart>) -> Maybe<&2, MPart>:
  Mech.default.k(Mech.asm(ps), ps)

def Mech.part.path(p: MPart) -> String:
  match p:
    case MPart{q, _, _}:
      q

def Mech.pick.k(ps: List<&2, MPart>, +want: String) -> Maybe<&2, MPart>:
  match ps:
    case Nil{}:
      None{}
    case MPart{+p, +n, +s} <> t:
      Bool.pick(Maybe<&2, MPart>, String.eq(p, want), Some{MPart{p, n, s}}, Mech.pick.k(t, want))

def Mech.preferred.k(m: Maybe<&2, MPart>, +ps: List<&2, MPart>) -> Maybe<&2, MPart>:
  match m:
    case Some{p}:
      Some{p}
    case None{}:
      Mech.default(ps)

# Only a model in the current parts list can be the configured default.
# Stale paths (and paths outside mech/) fall back to assembly, then first.
def Mech.preferred(+ps: List<&2, MPart>, +primary: String) -> Maybe<&2, MPart>:
  Mech.preferred.k(Mech.pick.k(ps, primary), ps)

def Mech.choose.k(m: Maybe<&2, MPart>, +ps: List<&2, MPart>, +primary: String) -> Maybe<&2, MPart>:
  match m:
    case Some{p}:
      Some{p}
    case None{}:
      Mech.preferred(ps, primary)

# An explicit selection wins over the project's preferred model.
def Mech.choose(+ps: List<&2, MPart>, +want: String, +root: String, +primary: String) -> Maybe<&2, MPart>:
  Mech.choose.k(Mech.pick.k(ps, Bool.pick(String, String.starts_with(want, root ++ "/"), String.drop(want, 1n+String.length(root)), want)), ps, primary)

def Mech.pick.or(m: Maybe<&2, MPart>, +ps: List<&2, MPart>) -> Maybe<&2, MPart>:
  match m:
    case Some{p}:
      Some{p}
    case None{}:
      List.head(&2, MPart, ps)

# the part a page shows: the one asked for (a path from the project root,
# or absolute under it), else the first
def Mech.pick(+ps: List<&2, MPart>, +want: String, +root: String) -> Maybe<&2, MPart>:
  Mech.pick.or(Mech.pick.k(ps, Bool.pick(String, String.starts_with(want, root ++ "/"), String.drop(want, 1n+String.length(root)), want)), ps)

# Pictures
# --------

def Mech.abs(+root: String, +p: String) -> String:
  Bool.pick(String, String.starts_with(p, "/"), p, Mech.in(root, p))

# the window's image key for a render: "v:<sum>:<path>" (a new sum is a
# new image; main.bend's Pics reads the path back)
def Mech.key(+root: String, f: MFile) -> String:
  match f:
    case MFile{p, s}:
      "v:" ++ s ++ ":" ++ Mech.abs(root, p)

# the path an image key names (the key itself when it has no version)
def Mech.key.path(+k: String) -> String:
  Bool.pick(String, String.starts_with(k, "v:"), String.join(List.drop(&2, String, String.split(k, ':'), 2n), ":"), k)

# where a hub serves a render (the sum keeps caches honest)
def Mech.url(+root: String, f: MFile) -> String:
  match f:
    case MFile{p, s}:
      IM.Img.url(Mech.abs(root, p)) ++ "&v=" ++ s

# the view a render shows, from its name (<stem>.<view>.png)
def Mech.view.of(f: MFile) -> String:
  match f:
    case MFile{p, _}:
      Maybe.default(&2, String, List.last(&2, String, String.split(Mech.stem(p), '.')), "")

# What the page says
# ------------------

def Mech.none() -> String:
  "No parts yet. Ask an agent to model one: it designs it in FreeCAD, keeps it in mech/, and renders it here."

def Mech.unrendered(+name: String) -> String:
  "No renders of " ++ name ++ " yet. Ask an agent to render it."
